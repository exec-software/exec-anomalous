//! ELF deployment of the unchanged native document client and native personality.
//! Only this profile elides a proven zero RW tail; the original client ELF and
//! full RW bytes remain retained, and reconstructed virtual bytes must match.
use super::*;
use anomalous_elf_adapter::{encoded_plan_size, load_plan, parse};
use elf_application_artifact::{
    blob, build_vessel, compact_elf, inspect_native_document_vessel, inspect_vessel,
    inspected_image, retain_vessel,
};
use native_document_artifact::exact_symbol;

const image_base: u64 = 0x400000;
const data_base: u64 = 0x800000;
const original_data_size: usize = 4224;
const deployment_data_size: usize = 128;
const section_name: &str = ".rodata.elf_document";
const loader_fault_alias: &str = "x86_64_elf_document_loader_fault_instruction";
const blob_names: [&str; 4] = [
    "x86_64_elf_document_data",
    "x86_64_elf_document_elf",
    "x86_64_elf_document_plan",
    "x86_64_elf_document_loader",
];

/// This optimization is deliberately narrower than the shared ELF profile.
/// No nonzero initialized byte, changed extent or later client layout can be
/// silently treated as BSS merely because it happens to fit the general limits.
fn validate_original(image: &inspected_image) -> Result<(), String> {
    if image.rx.is_empty()
        || image.rx.len() > 32768
        || image.rw.len() != original_data_size
        || image.rw.iter().any(|byte| *byte != 0)
    {
        return Err("ELF document requires the exact zeroed native RW profile".into());
    }
    for (index, (address, flags, size)) in [
        (image_base, 5, image.rx.len() as u64),
        (data_base, 6, original_data_size as u64),
    ]
    .into_iter()
    .enumerate()
    {
        let h = image.headers[index];
        if h.kind != 1
            || h.flags != flags
            || h.virtual_address != address
            || h.physical_address != address
            || h.file_size != size
            || h.memory_size != size
            || h.alignment != 4096
            || h.offset < 4096
            || !h.offset.is_multiple_of(4096)
            || h.offset.checked_add(size).is_none()
        {
            return Err("ELF document original load geometry changed".into());
        }
    }
    if image.headers[0]
        .offset
        .checked_add(image.headers[0].file_size)
        .is_none_or(|end| end > image.headers[1].offset)
    {
        return Err("ELF document original file loads overlap".into());
    }
    Ok(())
}

fn compact_document_elf(image: &inspected_image) -> Result<Vec<u8>, String> {
    validate_original(image)?;
    let mut headers = image.headers;
    headers[1].file_size = deployment_data_size as u64;
    let compact = inspected_image {
        rx: image.rx.clone(),
        rw: image.rw[..deployment_data_size].to_vec(),
        headers,
        fault: image.fault,
    };
    // The existing compactor and parser remain unchanged for every old profile.
    let deployment = compact_elf(&compact)?;
    verify_reconstruction(image, &deployment)?;
    Ok(deployment)
}

fn verify_reconstruction(image: &inspected_image, bytes: &[u8]) -> Result<load_plan, String> {
    validate_original(image)?;
    let plan = parse(bytes).map_err(|e| format!("ELF document deployment rejected: {e:?}"))?;
    let expected_rw_offset = (4096 + image.rx.len() as u64)
        .checked_add(4095)
        .ok_or("ELF document offset overflow")?
        & !4095;
    if plan.entry() != image_base
        || plan.image_length() != bytes.len() as u64
        || plan.rx().file_offset() != 4096
        || plan.rx().file_size() != image.rx.len() as u64
        || plan.rx().memory_size() != image.rx.len() as u64
        || plan.rx().virtual_address() != image_base
        || plan.rw().file_offset() != expected_rw_offset
        || plan.rw().file_size() != deployment_data_size as u64
        || plan.rw().memory_size() != original_data_size as u64
        || plan.rw().virtual_address() != data_base
    {
        return Err("ELF document deployment differs from exact admitted geometry".into());
    }
    let segment_bytes = |segment: anomalous_elf_adapter::load_segment| {
        let start = usize::try_from(segment.file_offset()).map_err(|_| "ELF document offset")?;
        let size = usize::try_from(segment.file_size()).map_err(|_| "ELF document size")?;
        bytes
            .get(start..start.checked_add(size).ok_or("ELF document extent")?)
            .ok_or("ELF document truncated segment")
    };
    if segment_bytes(plan.rx())? != image.rx {
        return Err("ELF document executable bytes changed".into());
    }
    // Reconstruct independently from the deployment plan/file, not the input
    // vector that was shortened. Compare the whole original virtual RW extent.
    let mut reconstructed = vec![0; original_data_size];
    reconstructed[..deployment_data_size].copy_from_slice(segment_bytes(plan.rw())?);
    if reconstructed != image.rw {
        return Err("ELF document virtual RW reconstruction differs".into());
    }
    Ok(plan)
}

pub(super) struct artifact {
    pub(super) object: PathBuf,
    native: native_document_artifact::artifact,
    native_sections: Vec<(String, Vec<u8>)>,
    blobs: Vec<blob>,
    loader_fault: u64,
}

impl artifact {
    fn extra_bytes(&self) -> Vec<u8> {
        self.blobs
            .iter()
            .flat_map(|b| b.bytes.iter().copied())
            .collect()
    }
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    // This also applies the unchanged native document startup/transport/fault
    // instruction contract before any deployment-container transformation.
    let native = native_document_artifact::package_image(root, build)?;
    let output = build.join("elf-document");
    fs::create_dir_all(&output).map_err(|e| format!("create ELF document directory: {e}"))?;
    let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
    for name in [
        "qualification.txt",
        "client.elf",
        "loader.elf",
        "payload.o",
        "deployment.elf",
    ] {
        remove_optional_file(&output.join(name))?;
    }
    let native_directory = native.object.parent().ok_or("native document directory")?;
    let client_path = output.join("client.elf");
    fs::copy(native_directory.join("client.elf"), &client_path).map_err(|e| e.to_string())?;
    let client = inspect_native_document_vessel(root, &client_path)?;
    if client.rx != native.image() {
        return Err("ELF document source differs from qualified native payload".into());
    }
    let deployment = compact_document_elf(&client)?;
    let plan = verify_reconstruction(&client, &deployment)?;
    let encoded = plan.encode();
    if load_plan::decode(&encoded) != Ok(plan) {
        return Err("ELF document manifest roundtrip changed".into());
    }
    fs::write(output.join("client_rx.bin"), &client.rx).map_err(|e| e.to_string())?;
    fs::write(output.join("client_rw.bin"), &client.rw).map_err(|e| e.to_string())?;
    fs::write(output.join("plan.bin"), encoded).map_err(|e| e.to_string())?;
    let deployment_path = output.join("deployment.elf");
    fs::write(&deployment_path, &deployment).map_err(|e| e.to_string())?;
    build_vessel(root, "anomalous-elf-loader", None, Some(&deployment_path))?;
    let loader_path = retain_vessel(root, &output, "anomalous-elf-loader", "loader.elf")?;
    let loader = inspect_vessel(root, &loader_path, "anomalous_elf_loader", false)?;
    if count_subslice(&loader.rx, &deployment) != 1
        || loader.headers[1].file_size != 128
        || loader.headers[1].memory_size != 128
        || loader.rw.iter().any(|byte| *byte != 0)
    {
        return Err("ELF document loader source or zeroed startup geometry changed".into());
    }
    fs::write(output.join("loader_rx.bin"), &loader.rx).map_err(|e| e.to_string())?;
    let native_bytes = fs::read(&native.object).map_err(|e| e.to_string())?;
    let native_elf = parse_elf64(&native_bytes)?;
    let native_sections = native_elf
        .section_headers
        .iter()
        .filter(|s| s.flags & 2 != 0 && s.size != 0)
        .map(|s| {
            Ok((
                s.name.clone(),
                native_application_artifact::section_bytes(&native_bytes, s)?.to_vec(),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let artifact = artifact {
        object: output.join("payload.o"),
        native,
        native_sections,
        blobs: vec![
            blob {
                name: blob_names[0],
                bytes: client.rw,
            },
            blob {
                name: blob_names[1],
                bytes: deployment,
            },
            blob {
                name: blob_names[2],
                bytes: encoded.to_vec(),
            },
            blob {
                name: blob_names[3],
                bytes: loader.rx,
            },
        ],
        loader_fault: loader.fault,
    };
    if count_subslice(&artifact.extra_bytes(), artifact.native.image()) != 2 {
        return Err("ELF document must retain exactly deployment and loader client copies".into());
    }
    write_wrapper(&output, &artifact)?;
    inspect_wrapper(root, &artifact)?;
    let mut receipt = format!(
        "profile=userspace_elf_native_document_v1\nentry=0x400000\nstartup=0x801000\nstartup_bytes=128\narena_bytes=32768\nclient_rx_bytes={}\noriginal_rw_file_bytes={original_data_size}\ndeployment_rw_file_bytes={deployment_data_size}\nrw_memory_bytes={original_data_size}\nelf_bytes={}\nmanifest_bytes={encoded_plan_size}\nloader_rx_bytes={}\nloader_slack_bytes={}\nvirtual_rw_equal=true\nclient_unchanged=true\n",
        client.rx.len(),
        artifact.blobs[1].bytes.len(),
        artifact.blobs[3].bytes.len(),
        32768 - artifact.blobs[3].bytes.len(),
    );
    for name in [
        "client.elf",
        "client_rx.bin",
        "client_rw.bin",
        "loader.elf",
        "loader_rx.bin",
        "deployment.elf",
        "plan.bin",
        "extra.o",
        "payload.o",
    ] {
        let hash = run_output_checked(root, "sha256sum", [output.join(name).into_os_string()])?;
        receipt.push_str(&String::from_utf8(hash).map_err(|_| "ELF document hash is not UTF8")?);
    }
    fs::write(output.join("qualification.txt"), receipt).map_err(|e| e.to_string())?;
    Ok(artifact)
}

fn write_wrapper(output: &Path, artifact: &artifact) -> Result<(), String> {
    let mut assembly = format!(".section {section_name},\"a\",@progbits\n.balign 16\n");
    for (index, blob) in artifact.blobs.iter().enumerate() {
        let filename = format!("blob_{index}.bin");
        fs::write(output.join(&filename), &blob.bytes).map_err(|e| e.to_string())?;
        assembly.push_str(&format!(".global {name}_start\n.global {name}_end\n{name}_start:\n.incbin \"{filename}\"\n{name}_end:\n", name=blob.name));
    }
    assembly.push_str(&format!(".global {loader_fault_alias}\n.set {loader_fault_alias}, 0x{:x}\n.section .note.GNU-stack,\"\",@progbits\n", artifact.loader_fault));
    fs::write(output.join("payload.S"), assembly).map_err(|e| e.to_string())?;
    run_checked(
        output,
        "as",
        [
            "--64".into(),
            "--fatal-warnings".into(),
            "-o".into(),
            "extra.o".into(),
            "payload.S".into(),
        ],
    )?;
    run_checked(
        output,
        "ld",
        [
            "-r".into(),
            "--build-id=none".into(),
            "-o".into(),
            "payload.o".into(),
            artifact.native.object.as_os_str().to_owned(),
            "extra.o".into(),
        ],
    )
}

fn inspect_extra_symbols(
    symbols: &str,
    lengths: [usize; 4],
    fault: u64,
    start: u64,
) -> Result<(), String> {
    let mut expected = Vec::new();
    let mut cursor = start;
    for (name, length) in blob_names.into_iter().zip(lengths) {
        let end = cursor
            .checked_add(length as u64)
            .ok_or("ELF document wrapper overflow")?;
        expected.push((format!("{name}_start"), 'R', cursor));
        expected.push((format!("{name}_end"), 'R', end));
        cursor = end;
    }
    expected.push((loader_fault_alias.to_string(), 'A', fault));
    for (name, kind, value) in &expected {
        if exact_symbol(symbols, name, *kind)? != *value {
            return Err("ELF document wrapper alias changed".into());
        }
    }
    for name in symbols
        .lines()
        .filter_map(|line| line.split_whitespace().next())
    {
        if name.starts_with("x86_64_elf_document_") && !expected.iter().any(|entry| entry.0 == name)
        {
            return Err("ELF document wrapper exports an undeclared alias".into());
        }
    }
    Ok(())
}

fn check_symbols(symbols: &str, artifact: &artifact, start: u64) -> Result<(), String> {
    inspect_extra_symbols(
        symbols,
        core::array::from_fn(|i| artifact.blobs[i].bytes.len()),
        artifact.loader_fault,
        start,
    )
}

fn inspect_wrapper(root: &Path, artifact: &artifact) -> Result<(), String> {
    let bytes = fs::read(&artifact.object).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let allocated = elf
        .section_headers
        .iter()
        .filter(|s| s.flags & 2 != 0 && s.size != 0)
        .collect::<Vec<_>>();
    let notes = elf
        .section_headers
        .iter()
        .filter(|s| s.name == ".note.GNU-stack")
        .collect::<Vec<_>>();
    if elf.kind != 1
        || elf.machine != 62
        || elf.entry != 0
        || !elf.program_headers.is_empty()
        || elf.section_headers.iter().any(|s| matches!(s.kind, 4 | 9))
        || artifact.native_sections.len() != 2
        || allocated.len() != 3
        || notes.len() != 1
        || notes[0].kind != 1
        || notes[0].flags != 0
        || notes[0].size != 0
    {
        return Err("ELF document wrapper layout/relocations/stack changed".into());
    }
    let extra = artifact.extra_bytes();
    for section in allocated {
        let expected = if section.name == section_name {
            extra.as_slice()
        } else {
            &artifact
                .native_sections
                .iter()
                .find(|(name, _)| name == &section.name)
                .ok_or("ELF document undeclared allocated section")?
                .1
        };
        if section.kind != 1
            || section.flags != 2
            || section.address != 0
            || native_application_artifact::section_bytes(&bytes, section)? != expected
            || elf
                .section_headers
                .iter()
                .filter(|s| s.name == section.name)
                .count()
                != 1
        {
            return Err("ELF document wrapper bytes or permissions changed".into());
        }
    }
    check_symbols(
        &native_application_artifact::listing(root, &artifact.object)?,
        artifact,
        0,
    )
}

pub(super) fn inspect_linked(
    root: &Path,
    kernel: &Path,
    artifact: &artifact,
) -> Result<(), String> {
    // The one standalone native image plus the two explicitly inspected ELF
    // copies are the complete allowed multiplicity for document executable bytes.
    native_document_artifact::inspect_linked_with_copies(root, kernel, &artifact.native, 3)?;
    let bytes = fs::read(kernel).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let symbols = native_application_artifact::listing(root, kernel)?;
    let start = exact_symbol(&symbols, "x86_64_elf_document_data_start", 'R')?;
    check_symbols(&symbols, artifact, start)?;
    let payload = artifact.extra_bytes();
    let end = start
        .checked_add(payload.len() as u64)
        .ok_or("ELF document linked overflow")?;
    let sections = elf
        .section_headers
        .iter()
        .filter(|s| {
            s.flags & 2 != 0
                && s.address < end
                && s.address
                    .checked_add(s.size)
                    .is_none_or(|limit| start < limit)
        })
        .collect::<Vec<_>>();
    let loads = elf
        .program_headers
        .iter()
        .copied()
        .filter(|s| {
            s.kind == 1
                && s.virtual_address < end
                && s.virtual_address
                    .checked_add(s.memory_size)
                    .is_none_or(|limit| start < limit)
        })
        .collect::<Vec<_>>();
    if start < symbol_u64_from_listing(&symbols, "kernel_rodata_start")?
        || end > symbol_u64_from_listing(&symbols, "kernel_rodata_end")?
        || sections.len() != 1
        || sections[0].kind != 1
        || sections[0].flags != 2
        || start < sections[0].address
        || sections[0]
            .address
            .checked_add(sections[0].size)
            .is_none_or(|limit| end > limit)
        || loads.len() != 1
        || loads[0].flags != 5
        || elf_virtual_bytes(&bytes, loads[0], start, payload.len())? != payload
        || count_subslice(&bytes, &payload) != 1
    {
        return Err("ELF document linked wrapper lost exact unique readonly backing".into());
    }
    // The all-zero RW/short plan need not be unique byte strings in a kernel.
    // Whole-wrapper identity fixes their location. Nontrivial embedded sources
    // additionally reject unexpected writable, debug or otherwise hidden copies.
    for index in [1, 3] {
        let source = &artifact.blobs[index].bytes;
        if count_subslice(&bytes, source) != count_subslice(&payload, source) {
            return Err("ELF document embedded source multiplicity changed".into());
        }
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "elf_document",
        "kernel.elf_document_test",
        "x86_64_elf_document",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
            return Err("normal image retains ELF document proof".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> inspected_image {
        let rx = vec![0x90; 22213];
        inspected_image {
            headers: [
                elf64_program_header {
                    kind: 1,
                    flags: 5,
                    offset: 4096,
                    virtual_address: image_base,
                    physical_address: image_base,
                    file_size: rx.len() as u64,
                    memory_size: rx.len() as u64,
                    alignment: 4096,
                },
                elf64_program_header {
                    kind: 1,
                    flags: 6,
                    offset: 28672,
                    virtual_address: data_base,
                    physical_address: data_base,
                    file_size: original_data_size as u64,
                    memory_size: original_data_size as u64,
                    alignment: 4096,
                },
            ],
            rx,
            rw: vec![0; original_data_size],
            fault: image_base + 4,
        }
    }

    #[test]
    fn zero_tail_compaction_reconstructs_unchanged_native_virtual_bytes() {
        let image = fixture();
        let original = image.rw.clone();
        let deployment = compact_document_elf(&image).unwrap();
        let plan = verify_reconstruction(&image, &deployment).unwrap();
        assert_eq!(deployment.len(), 28800);
        assert_eq!(plan.rw().file_size(), 128);
        assert_eq!(plan.rw().memory_size(), 4224);
        assert_eq!(plan.rx().file_size(), image.rx.len() as u64);
        assert_eq!(image.rw, original);
        assert_eq!(load_plan::decode(&plan.encode()), Ok(plan));
        // The shared legacy compactor still preserves every original file byte.
        let legacy = compact_elf(&image).unwrap();
        assert_eq!(parse(&legacy).unwrap().rw().file_size(), 4224);
        assert_eq!(legacy.len(), 32896);
    }

    #[test]
    fn nonzero_omitted_tail_or_initialized_prefix_is_never_elided() {
        for offset in [0, 127, 128, 129, 4095, 4096, 4223] {
            let mut image = fixture();
            image.rw[offset] = 1;
            assert!(compact_document_elf(&image).is_err(), "offset {offset}");
        }
    }

    #[test]
    fn source_vectors_and_headers_must_describe_exact_native_geometry() {
        for mutation in 0..14 {
            let mut image = fixture();
            match mutation {
                0 => image.rw.push(0),
                1 => {
                    image.rw.pop();
                }
                2 => image.rx.clear(),
                3 => image.rx.resize(32769, 0x90),
                4 => image.headers[0].file_size -= 1,
                5 => image.headers[0].memory_size += 1,
                6 => image.headers[1].file_size -= 1,
                7 => image.headers[1].memory_size += 1,
                8 => image.headers[1].virtual_address += 4096,
                9 => image.headers[1].physical_address += 4096,
                10 => image.headers[1].flags = 7,
                11 => image.headers[1].offset = 4096,
                12 => image.headers[1].offset = u64::MAX - 4095,
                13 => image.headers[0].alignment = 1,
                _ => unreachable!(),
            }
            assert!(compact_document_elf(&image).is_err(), "mutation {mutation}");
        }
    }

    #[test]
    fn deployment_truncation_padding_permissions_and_reconstruction_changes_reject() {
        let image = fixture();
        let deployment = compact_document_elf(&image).unwrap();
        for length in [0, 15, 63, 175, 4095, deployment.len() - 1] {
            assert!(verify_reconstruction(&image, &deployment[..length]).is_err());
        }
        for offset in [0, 176, 4095, 4096, deployment.len() - 1, 64 + 4, 120 + 4] {
            let mut changed = deployment.clone();
            changed[offset] ^= 1;
            assert!(
                verify_reconstruction(&image, &changed).is_err(),
                "offset {offset}"
            );
        }
        for (offset, value) in [(24, image_base + 1), (120 + 32, 129), (120 + 40, 4225)] {
            let mut changed = deployment.clone();
            changed[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            assert!(
                verify_reconstruction(&image, &changed).is_err(),
                "header {offset}"
            );
        }
        let mut changed = deployment;
        changed.push(0);
        assert!(verify_reconstruction(&image, &changed).is_err());
    }

    fn symbols(lengths: [usize; 4], start: u64) -> String {
        let mut output = String::new();
        let mut cursor = start;
        for (name, length) in blob_names.into_iter().zip(lengths) {
            output.push_str(&format!("{name}_start R {cursor:x}\n"));
            cursor += length as u64;
            output.push_str(&format!("{name}_end R {cursor:x}\n"));
        }
        output.push_str(&format!("{loader_fault_alias} A 400060\n"));
        output
    }

    #[test]
    fn wrapper_requires_exact_unique_aliases_and_checked_extents() {
        let lengths = [4224, 28800, 96, 32240];
        let original = symbols(lengths, 0x140000);
        assert!(inspect_extra_symbols(&original, lengths, 0x400060, 0x140000).is_ok());
        for changed in [
            original.replacen(" R ", " D ", 1),
            original.replace(" A 400060", " A 400061"),
            format!("{original}x86_64_elf_document_data_start R 140000\n"),
            format!("{original}x86_64_elf_document_unadmitted A 1\n"),
            original.replace("x86_64_elf_document_plan_end", "missing"),
        ] {
            assert!(inspect_extra_symbols(&changed, lengths, 0x400060, 0x140000).is_err());
        }
        assert!(inspect_extra_symbols(&original, lengths, 0x400060, u64::MAX - 127).is_err());
    }

    #[test]
    fn normal_image_exclusion_checks_symbols_and_bytes() {
        assert!(verify_absent("ordinary_kernel", b"ordinary image").is_ok());
        for marker in [
            "elf_document",
            "kernel.elf_document_test",
            "x86_64_elf_document",
        ] {
            assert!(verify_absent(marker, b"").is_err());
            assert!(verify_absent("", marker.as_bytes()).is_err());
        }
    }
}
