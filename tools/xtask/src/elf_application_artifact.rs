//! Host qualification and packaging for the bounded userspace ELF adapter.
//! The deployment container still selects the existing native execution ABI.
use super::*;
use anomalous_elf_adapter::{encoded_plan_size, load_plan, parse};

const image_base: u64 = 0x400000;
const data_base: u64 = 0x800000;
const data_canary: u64 = 0x454c_465f_4441_5441;
const client_fault_alias: &str = "x86_64_native_application_user_fault_instruction";
const loader_fault_alias: &str = "x86_64_elf_application_loader_fault_instruction";

pub(super) struct inspected_image {
    pub(super) rx: Vec<u8>,
    pub(super) rw: Vec<u8>,
    pub(super) headers: [elf64_program_header; 2],
    pub(super) fault: u64,
}

pub(super) struct blob {
    pub(super) name: &'static str,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct artifact {
    pub(super) object: PathBuf,
    blobs: Vec<blob>,
    client_fault: u64,
    loader_fault: u64,
    extra_aliases: Vec<(&'static str, u64)>,
}

impl artifact {
    pub(super) fn extend(
        mut self,
        object: PathBuf,
        blobs: Vec<blob>,
        aliases: Vec<(&'static str, u64)>,
    ) -> Self {
        self.object = object;
        self.blobs.extend(blobs);
        self.extra_aliases.extend(aliases);
        self
    }

    fn aliases(&self) -> impl Iterator<Item = (&'static str, u64)> + '_ {
        [
            (client_fault_alias, self.client_fault),
            (loader_fault_alias, self.loader_fault),
        ]
        .into_iter()
        .chain(self.extra_aliases.iter().copied())
    }

    fn bytes(&self) -> Vec<u8> {
        self.blobs
            .iter()
            .flat_map(|b| b.bytes.iter().copied())
            .collect()
    }
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let output = build.join("elf-application");
    fs::create_dir_all(&output).map_err(|e| format!("create ELF artifact directory: {e}"))?;
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
    build_vessel(root, "anomalous-native-client", Some("elf_data_test"), None)?;
    let client_path = retain_vessel(root, &output, "anomalous-native-client", "client.elf")?;
    let client = inspect_vessel(root, &client_path, "anomalous_native_client", true)?;
    let deployment = compact_elf(&client)?;
    let plan = parse(&deployment).map_err(|e| format!("compact ELF validation: {e:?}"))?;
    verify_plan(plan, &client, deployment.len())?;
    let encoded = plan.encode();
    if encoded.len() != encoded_plan_size
        || load_plan::decode(&encoded).map_err(|e| format!("host manifest decode: {e:?}"))? != plan
    {
        return Err("ELF host manifest failed exact roundtrip".into());
    }
    let deployment_path = output.join("deployment.elf");
    fs::write(&deployment_path, &deployment).map_err(|e| e.to_string())?;
    build_vessel(root, "anomalous-elf-loader", None, Some(&deployment_path))?;
    let loader_path = retain_vessel(root, &output, "anomalous-elf-loader", "loader.elf")?;
    let loader = inspect_vessel(root, &loader_path, "anomalous_elf_loader", false)?;
    if count_subslice(&loader.rx, &deployment) != 1 {
        return Err("ELF loader must embed exactly one complete deployment in RX".into());
    }
    let artifact = artifact {
        object: output.join("payload.o"),
        blobs: vec![
            blob {
                name: "x86_64_native_application_payload",
                bytes: client.rx,
            },
            blob {
                name: "x86_64_elf_application_data",
                bytes: client.rw,
            },
            blob {
                name: "x86_64_elf_application_elf",
                bytes: deployment,
            },
            blob {
                name: "x86_64_elf_application_plan",
                bytes: encoded.to_vec(),
            },
            blob {
                name: "x86_64_elf_application_loader",
                bytes: loader.rx,
            },
        ],
        client_fault: client.fault,
        loader_fault: loader.fault,
        extra_aliases: Vec::new(),
    };
    write_wrapper(root, &output, &artifact)?;
    let mut receipt = format!(
        "profile=userspace_elf_native_v1\nentry=0x{image_base:x}\nclient_rx_bytes={}\nclient_rw_file_bytes={}\nclient_rw_memory_bytes={}\nelf_bytes={}\nmanifest_bytes={encoded_plan_size}\nloader_rx_bytes={}\nloader_rw_memory_bytes={}\nclient_fault=0x{:x}\nloader_fault=0x{:x}\ndata_canary=0x{data_canary:x}\n",
        artifact.blobs[0].bytes.len(),
        artifact.blobs[1].bytes.len(),
        client.headers[1].memory_size,
        artifact.blobs[2].bytes.len(),
        artifact.blobs[4].bytes.len(),
        loader.headers[1].memory_size,
        artifact.client_fault,
        artifact.loader_fault,
    );
    for name in [
        "client.elf",
        "loader.elf",
        "deployment.elf",
        "blob_0.bin",
        "blob_1.bin",
        "blob_2.bin",
        "blob_3.bin",
        "blob_4.bin",
        "payload.o",
    ] {
        let digest = run_output_checked(root, "sha256sum", [output.join(name).into_os_string()])?;
        let digest = String::from_utf8(digest).map_err(|_| "ELF hash is not UTF-8")?;
        let value = digest.split_whitespace().next().ok_or("ELF hash missing")?;
        if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("ELF hash malformed".into());
        }
        receipt.push_str(&format!("{name}_sha256={value}\n"));
    }
    fs::write(output.join("qualification.txt"), receipt).map_err(|e| e.to_string())?;
    Ok(artifact)
}

pub(super) fn write_wrapper(root: &Path, output: &Path, artifact: &artifact) -> Result<(), String> {
    let mut assembly =
        String::from(".section .rodata.elf_application,\"a\",@progbits\n.balign 16\n");
    for (index, blob) in artifact.blobs.iter().enumerate() {
        let filename = format!("blob_{index}.bin");
        fs::write(output.join(&filename), &blob.bytes).map_err(|e| e.to_string())?;
        assembly.push_str(&format!(
            ".global {name}_start\n.global {name}_end\n{name}_start:\n.incbin \"{filename}\"\n{name}_end:\n",
            name = blob.name,
        ));
    }
    for (name, value) in artifact.aliases() {
        assembly.push_str(&format!(".global {name}\n.set {name}, 0x{value:x}\n"));
    }
    assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    fs::write(output.join("payload.S"), assembly).map_err(|e| e.to_string())?;
    run_checked(
        output,
        "as",
        ["--64", "--fatal-warnings", "-o", "payload.o", "payload.S"].map(OsString::from),
    )?;
    inspect_wrapper(root, artifact)?;
    Ok(())
}

pub(super) fn build_vessel(
    root: &Path,
    package: &str,
    feature: Option<&str>,
    deployment: Option<&Path>,
) -> Result<(), String> {
    let mut args = vec![
        "build".into(),
        "-Zbuild-std=core,compiler_builtins".into(),
        "-Zbuild-std-features=compiler-builtins-mem".into(),
        "--release".into(),
        "--package".into(),
        package.into(),
        "--target".into(),
        "x86_64-unknown-none".into(),
        "--target-dir".into(),
        root.join("target").into_os_string(),
        "--locked".into(),
        "--no-default-features".into(),
    ];
    if let Some(feature) = feature {
        args.extend(["--features".into(), feature.into()]);
    }
    let mut environment = vec![(
        "CARGO_ENCODED_RUSTFLAGS",
        OsString::from(
            "-C\u{1f}code-model=small\u{1f}-C\u{1f}relocation-model=static\u{1f}-C\u{1f}force-frame-pointers=yes\u{1f}-Z\u{1f}plt=yes",
        ),
    )];
    if let Some(path) = deployment {
        if !path.is_absolute() {
            return Err("ELF deployment path must be absolute".into());
        }
        environment.push(("ANOMALOUS_ELF_DEPLOYMENT", path.as_os_str().to_owned()));
    }
    run_checked_with_environments(
        root,
        env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
        args,
        &environment,
    )
}

pub(super) fn retain_vessel(
    root: &Path,
    output: &Path,
    package: &str,
    name: &str,
) -> Result<PathBuf, String> {
    let destination = output.join(name);
    fs::copy(
        root.join("target/x86_64-unknown-none/release")
            .join(package),
        &destination,
    )
    .map_err(|e| format!("retain {package} ELF: {e}"))?;
    Ok(destination)
}

pub(super) fn inspect_vessel(
    root: &Path,
    path: &Path,
    prefix: &str,
    initialized: bool,
) -> Result<inspected_image, String> {
    inspect_vessel_profile(root, path, prefix, initialized, false, false, false)
}

pub(super) fn inspect_linux_vessel(root: &Path, path: &Path) -> Result<inspected_image, String> {
    inspect_vessel_profile(
        root,
        path,
        "anomalous_linux_client",
        true,
        true,
        false,
        false,
    )
}

pub(super) fn inspect_linux_tls_vessel(
    root: &Path,
    path: &Path,
) -> Result<inspected_image, String> {
    inspect_vessel_profile(
        root,
        path,
        "anomalous_linux_client",
        true,
        true,
        false,
        true,
    )
}

pub(super) fn inspect_native_runtime_vessel(
    root: &Path,
    path: &Path,
) -> Result<inspected_image, String> {
    inspect_vessel_profile(
        root,
        path,
        "anomalous_native_runtime_client",
        false,
        false,
        true,
        false,
    )
}

/// A program-bearing client. Same inspection profile as the native runtime
/// client -- an uninitialized zeroed RW image with a reserved descriptor
/// region -- differing only in its symbol prefix and the length of that
/// region, which its own artifact module checks.
pub(super) fn inspect_native_program_vessel(
    root: &Path,
    path: &Path,
) -> Result<inspected_image, String> {
    inspect_vessel_profile(
        root,
        path,
        "anomalous_native_program_client",
        false,
        false,
        true,
        false,
    )
}

pub(super) fn inspect_native_document_vessel(
    root: &Path,
    path: &Path,
) -> Result<inspected_image, String> {
    inspect_vessel_profile(
        root,
        path,
        "anomalous_native_document_client",
        false,
        false,
        true,
        false,
    )
}

pub(super) fn inspect_native_relaunch_vessel(
    root: &Path,
    path: &Path,
) -> Result<inspected_image, String> {
    inspect_vessel_profile(
        root,
        path,
        "anomalous_native_relaunch_client",
        false,
        false,
        true,
        false,
    )
}

pub(super) fn inspect_native_durable_vessel(
    root: &Path,
    path: &Path,
) -> Result<inspected_image, String> {
    inspect_vessel_profile(
        root,
        path,
        "anomalous_native_durable_client",
        false,
        false,
        true,
        false,
    )
}

fn inspect_vessel_profile(
    root: &Path,
    path: &Path,
    prefix: &str,
    initialized: bool,
    linux: bool,
    native_runtime: bool,
    linux_tls: bool,
) -> Result<inspected_image, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let parsed = parse_elf64(&bytes)?;
    let rx = if native_runtime {
        native_application_artifact::inspect_native_runtime_image(&bytes, &parsed)?
    } else if initialized {
        native_application_artifact::inspect_image_with_canary(&bytes, &parsed, data_canary)?
    } else {
        native_application_artifact::inspect_elf_loader_image(&bytes, &parsed)?
    };
    let headers = [parsed.program_headers[0], parsed.program_headers[1]];
    let symbols = native_application_artifact::listing(root, path)?;
    let symbol = |suffix: &str| symbol_u64_from_listing(&symbols, &format!("{prefix}_{suffix}"));
    for (suffix, expected) in [
        ("entry", image_base),
        ("image_start", image_base),
        ("image_payload_end", image_base + headers[0].file_size),
        ("image_end", align_page(image_base + headers[0].file_size)?),
        ("data_start", data_base),
        ("data_file_end", data_base + headers[1].file_size),
        ("data_end", data_base + headers[1].memory_size),
        ("bss_end", data_base + headers[1].memory_size),
    ] {
        if symbol(suffix)? != expected {
            return Err(format!("{prefix}_{suffix} geometry changed"));
        }
    }
    if symbol("bss_start")? < data_base + headers[1].file_size
        || symbol("bss_start")? > symbol("bss_end")?
    {
        return Err("ELF vessel BSS overlaps initialized file data".into());
    }
    if initialized
        && (symbol("data_canary")? != data_base + 128
            || symbol("data_canary_start")? != data_base + 128
            || symbol("data_canary_end")? != data_base + 136
            || symbol_kind_from_listing(&symbols, &format!("{prefix}_data_canary"))? != 'D')
    {
        return Err("ELF client initialized canary markers changed".into());
    }
    let fault = symbol("fault_instruction")?;
    let text = parsed
        .section_headers
        .iter()
        .find(|s| s.name == ".text")
        .ok_or("ELF vessel text missing")?;
    if fault < image_base
        || fault
            .checked_add(2)
            .is_none_or(|end| end > image_base + text.size)
        || elf_virtual_bytes(&bytes, headers[0], fault, 2)? != [0x0f, 0x0b]
    {
        return Err("ELF vessel fault marker is not executable UD2".into());
    }
    let disassembly = run_output_checked(
        root,
        "objdump",
        [
            "--disassemble".into(),
            "--disassemble-zeroes".into(),
            "--wide".into(),
            "--insn-width=16".into(),
            "--section=.text".into(),
            path.as_os_str().to_owned(),
        ],
    )?;
    fs::write(path.with_extension("disassembly"), &disassembly).map_err(|e| e.to_string())?;
    let disassembly = String::from_utf8(disassembly).map_err(|_| "ELF disassembly is not UTF-8")?;
    if linux {
        let syscalls = [
            symbol("syscall_instruction")?,
            symbol("probe_syscall_instruction")?,
        ];
        if symbol("syscall_return")?
            != syscalls[0]
                .checked_add(2)
                .ok_or("Linux syscall marker overflow")?
            || symbol("probe_syscall_return")?
                != syscalls[1]
                    .checked_add(2)
                    .ok_or("Linux probe marker overflow")?
            || symbol("poll_tail")? != data_base + 8192 - 8
            || headers[1].memory_size != 8192
        {
            return Err("Linux syscall return or poll tail geometry changed".into());
        }
        if linux_tls {
            let profile = native_application_artifact::linux_tls_instruction_profile::from_symbols(
                &symbols,
                &rx,
                text.size,
                symbol("bss_start")?,
                symbol("bss_end")?,
            )?;
            native_application_artifact::inspect_linux_tls_instructions(
                &disassembly,
                &rx,
                text.size,
                fault,
                &syscalls,
                &profile,
            )?;
        } else {
            native_application_artifact::inspect_linux_instructions(
                &disassembly,
                &rx,
                text.size,
                fault,
                &syscalls,
            )?;
        }
    } else {
        native_application_artifact::inspect_instructions(&disassembly, &rx, text.size, fault)?;
    }
    let rw =
        elf_virtual_bytes(&bytes, headers[1], data_base, headers[1].file_size as usize)?.to_vec();
    Ok(inspected_image {
        rx,
        rw,
        headers,
        fault,
    })
}

fn align_page(value: u64) -> Result<u64, String> {
    value
        .checked_add(4095)
        .map(|v| v & !4095)
        .ok_or_else(|| "ELF page rounding overflow".into())
}

pub(super) fn compact_elf(image: &inspected_image) -> Result<Vec<u8>, String> {
    let rx_offset = 4096_u64;
    let rw_offset = align_page(
        rx_offset
            .checked_add(image.rx.len() as u64)
            .ok_or("ELF RX overflow")?,
    )?;
    let end = rw_offset
        .checked_add(image.rw.len() as u64)
        .ok_or("ELF file overflow")?;
    if image.rx.is_empty()
        || image.rx.len() > 32768
        || image.rw.len() < 128
        || image.rw.len() > 8192
        || end > 65536
    {
        return Err("ELF compact image outside bounded profile".into());
    }
    let mut bytes = vec![0; usize::try_from(end).map_err(|_| "ELF file length overflow")?];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&image_base.to_le_bytes());
    bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&2_u16.to_le_bytes());
    for (index, offset) in [rx_offset, rw_offset].into_iter().enumerate() {
        let h = image.headers[index];
        let start = 64 + index * 56;
        bytes[start..start + 4].copy_from_slice(&h.kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&h.flags.to_le_bytes());
        for (field, value) in [
            offset,
            h.virtual_address,
            h.physical_address,
            h.file_size,
            h.memory_size,
            h.alignment,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[start + 8 + field * 8..start + 16 + field * 8]
                .copy_from_slice(&value.to_le_bytes());
        }
    }
    bytes[rx_offset as usize..rx_offset as usize + image.rx.len()].copy_from_slice(&image.rx);
    bytes[rw_offset as usize..].copy_from_slice(&image.rw);
    let plan = parse(&bytes).map_err(|e| format!("generated compact ELF rejected: {e:?}"))?;
    verify_plan(plan, image, bytes.len())?;
    Ok(bytes)
}

fn verify_plan(plan: load_plan, image: &inspected_image, length: usize) -> Result<(), String> {
    if plan.entry() != image_base || plan.image_length() != length as u64 {
        return Err("ELF plan entry/length changed".into());
    }
    for (index, segment) in [plan.rx(), plan.rw()].into_iter().enumerate() {
        let h = image.headers[index];
        let raw = if index == 0 { &image.rx } else { &image.rw };
        if segment.virtual_address() != h.virtual_address
            || segment.file_size() != h.file_size
            || segment.memory_size() != h.memory_size
            || segment.file_size() != raw.len() as u64
        {
            return Err("ELF plan does not preserve inspected load geometry".into());
        }
    }
    Ok(())
}

fn check_symbols(symbols: &str, artifact: &artifact, start: u64) -> Result<(), String> {
    let mut cursor = start;
    for blob in &artifact.blobs {
        let end = cursor
            .checked_add(blob.bytes.len() as u64)
            .ok_or("ELF blob symbol overflow")?;
        for (suffix, value) in [("start", cursor), ("end", end)] {
            let name = format!("{}_{suffix}", blob.name);
            if symbol_u64_from_listing(symbols, &name)? != value
                || symbol_kind_from_listing(symbols, &name)? != 'R'
            {
                return Err("ELF wrapper blob symbols differ from source".into());
            }
        }
        cursor = end;
    }
    for (name, value) in artifact.aliases() {
        if symbol_u64_from_listing(symbols, name)? != value
            || symbol_kind_from_listing(symbols, name)? != 'A'
        {
            return Err("ELF wrapper fault aliases differ from source".into());
        }
    }
    Ok(())
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
        || allocated.len() != 1
        || allocated[0].name != ".rodata.elf_application"
        || allocated[0].kind != 1
        || allocated[0].flags != 2
        || allocated[0].address != 0
        || native_application_artifact::section_bytes(&bytes, allocated[0])? != artifact.bytes()
        || notes.len() != 1
        || notes[0].kind != 1
        || notes[0].flags != 0
        || notes[0].size != 0
    {
        return Err("ELF wrapper bytes/permissions/relocations/stack declaration changed".into());
    }
    let symbols = native_application_artifact::listing(root, &artifact.object)?;
    check_symbols(&symbols, artifact, 0)?;
    if symbols.lines().count() != artifact.blobs.len() * 2 + 2 + artifact.extra_aliases.len() {
        return Err("ELF wrapper symbol set is not exact".into());
    }
    Ok(())
}

pub(super) fn inspect_linked(
    root: &Path,
    kernel: &Path,
    artifact: &artifact,
) -> Result<(), String> {
    let bytes = fs::read(kernel).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let symbols = native_application_artifact::listing(root, kernel)?;
    inspect_embedding(&bytes, &elf, &symbols, artifact)
}

fn inspect_embedding(
    bytes: &[u8],
    elf: &parsed_elf64,
    symbols: &str,
    artifact: &artifact,
) -> Result<(), String> {
    let start = symbol_u64_from_listing(symbols, "x86_64_native_application_payload_start")?;
    check_symbols(symbols, artifact, start)?;
    let payload = artifact.bytes();
    let end = start
        .checked_add(payload.len() as u64)
        .ok_or("ELF wrapper extent overflow")?;
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
        .filter(|p| {
            p.kind == 1
                && p.virtual_address < end
                && p.virtual_address
                    .checked_add(p.memory_size)
                    .is_none_or(|limit| start < limit)
        })
        .collect::<Vec<_>>();
    if start < symbol_u64_from_listing(symbols, "kernel_rodata_start")?
        || end > symbol_u64_from_listing(symbols, "kernel_rodata_end")?
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
        || elf_virtual_bytes(bytes, loads[0], start, payload.len())? != payload
        || count_subslice(bytes, &payload) != 1
    {
        return Err("ELF linked wrapper changed or lacks unique readonly backing".into());
    }
    // Client RX/RW also occur inside the compact ELF and again inside the
    // loader's embedded ELF. Permit exactly these inspected copies, never an
    // extra writable/debug/unlabelled copy elsewhere in the kernel artifact.
    for blob in &artifact.blobs {
        if blob.bytes.is_empty()
            || count_subslice(bytes, &blob.bytes) != count_subslice(&payload, &blob.bytes)
        {
            return Err("ELF linked blob multiplicity differs from inspected wrapper".into());
        }
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "elf_application",
        "anomalous_elf_loader",
        "anomalous_native_client_data_canary",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
            return Err("normal image retains ELF application fixture".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_fixture() -> inspected_image {
        let rx = vec![0x31, 0xc0, 0xcd, 0xf4, 0x0f, 0x0b, 0xc3];
        let mut rw = vec![0; 136];
        rw[128..].copy_from_slice(&data_canary.to_le_bytes());
        inspected_image {
            headers: [
                elf64_program_header {
                    kind: 1,
                    flags: 5,
                    offset: 0x3000,
                    virtual_address: image_base,
                    physical_address: image_base,
                    file_size: rx.len() as u64,
                    memory_size: rx.len() as u64,
                    alignment: 4096,
                },
                elf64_program_header {
                    kind: 1,
                    flags: 6,
                    offset: 0x8000,
                    virtual_address: data_base,
                    physical_address: data_base,
                    file_size: rw.len() as u64,
                    memory_size: 4096,
                    alignment: 4096,
                },
            ],
            rx,
            rw,
            fault: image_base + 4,
        }
    }

    #[test]
    fn elf_compaction_preserves_file_data_bss_and_geometry() {
        let image = image_fixture();
        let bytes = compact_elf(&image).unwrap();
        let plan = parse(&bytes).unwrap();
        assert_eq!(bytes.len(), 0x2000 + 136);
        assert_eq!(plan.rx().file_offset(), 0x1000);
        assert_eq!(plan.rw().file_offset(), 0x2000);
        assert_eq!(plan.rw().memory_size(), 4096);
        assert_eq!(&bytes[0x1000..0x1007], image.rx);
        assert_eq!(&bytes[0x2000..], image.rw);
        assert!(bytes[176..0x1000].iter().all(|v| *v == 0));
        assert!(bytes[0x1007..0x2000].iter().all(|v| *v == 0));
        assert_eq!(&bytes[40..48], &[0; 8]);
        assert_eq!(&bytes[58..64], &[0; 6]);
        assert_eq!(
            u64::from_le_bytes(bytes[88..96].try_into().unwrap()),
            image_base
        );
        assert_eq!(
            u64::from_le_bytes(bytes[144..152].try_into().unwrap()),
            data_base
        );
        assert_eq!(load_plan::decode(&plan.encode()).unwrap(), plan);
    }

    #[test]
    fn elf_compaction_rejects_changed_sizes_permissions_and_excess() {
        for (index, field) in [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)] {
            let mut image = image_fixture();
            match field {
                0 => image.headers[index].file_size += 1,
                1 => image.headers[index].flags = 7,
                _ => image.headers[index].virtual_address += 4096,
            }
            assert!(compact_elf(&image).is_err());
        }
        let mut image = image_fixture();
        image.rx.clear();
        assert!(compact_elf(&image).is_err());
        image.rx.resize(32769, 0);
        assert!(compact_elf(&image).is_err());
        let mut image = image_fixture();
        image.rw.resize(127, 0);
        assert!(compact_elf(&image).is_err());
        image.rw.resize(8193, 0);
        assert!(compact_elf(&image).is_err());
        assert!(align_page(u64::MAX).is_err());
    }

    fn embedding() -> (Vec<u8>, parsed_elf64, String, artifact) {
        let client = image_fixture();
        let deployment = compact_elf(&client).unwrap();
        let plan = parse(&deployment).unwrap().encode().to_vec();
        let mut loader = vec![0x90; 17];
        loader.extend_from_slice(&deployment);
        let artifact = artifact {
            object: PathBuf::new(),
            blobs: vec![
                blob {
                    name: "x86_64_native_application_payload",
                    bytes: client.rx,
                },
                blob {
                    name: "x86_64_elf_application_data",
                    bytes: client.rw,
                },
                blob {
                    name: "x86_64_elf_application_elf",
                    bytes: deployment,
                },
                blob {
                    name: "x86_64_elf_application_plan",
                    bytes: plan,
                },
                blob {
                    name: "x86_64_elf_application_loader",
                    bytes: loader,
                },
            ],
            client_fault: client.fault,
            loader_fault: image_base + 3,
            extra_aliases: Vec::new(),
        };
        let mut bytes = vec![0; 256];
        bytes.extend_from_slice(&artifact.bytes());
        let start = 0x100100;
        let mut cursor = start;
        let mut symbols = String::new();
        for blob in &artifact.blobs {
            symbols.push_str(&format!("{}_start R {cursor:x}\n", blob.name));
            cursor += blob.bytes.len() as u64;
            symbols.push_str(&format!("{}_end R {cursor:x}\n", blob.name));
        }
        symbols.push_str(&format!("{client_fault_alias} A {:x}\n{loader_fault_alias} A {:x}\nkernel_rodata_start R {start:x}\nkernel_rodata_end R {cursor:x}\n", artifact.client_fault, artifact.loader_fault));
        let elf = parsed_elf64 {
            kind: 2,
            machine: 62,
            entry: 0x100000,
            program_headers: vec![elf64_program_header {
                kind: 1,
                flags: 5,
                offset: 0,
                virtual_address: 0x100000,
                physical_address: 0x100000,
                file_size: bytes.len() as u64,
                memory_size: bytes.len() as u64,
                alignment: 4096,
            }],
            section_headers: vec![elf64_section_header {
                name: ".rodata".into(),
                kind: 1,
                flags: 2,
                address: start,
                offset: 256,
                size: cursor - start,
            }],
        };
        (bytes, elf, symbols, artifact)
    }

    #[test]
    fn elf_embedding_allows_only_exact_nested_deployment_copies() {
        let (bytes, elf, symbols, artifact) = embedding();
        inspect_embedding(&bytes, &elf, &symbols, &artifact).unwrap();
        assert_eq!(
            count_subslice(&artifact.bytes(), &artifact.blobs[2].bytes),
            2
        );
        assert_eq!(
            count_subslice(&artifact.bytes(), &artifact.blobs[0].bytes),
            3
        );
        for blob in &artifact.blobs {
            let mut extra = bytes.clone();
            extra.extend_from_slice(&blob.bytes);
            assert!(inspect_embedding(&extra, &elf, &symbols, &artifact).is_err());
        }
        let mut changed = bytes.clone();
        changed[256] ^= 1;
        assert!(inspect_embedding(&changed, &elf, &symbols, &artifact).is_err());
    }

    #[test]
    fn composite_embedding_binds_extra_aliases_and_shared_data_copies() {
        let (mut bytes, mut elf, mut symbols, base) = embedding();
        let start = 0x100100 + base.bytes().len() as u64;
        let extra = base.blobs[1].bytes.clone();
        let end = start + extra.len() as u64;
        let artifact = base.extend(
            PathBuf::new(),
            vec![blob {
                name: "x86_64_linux_application_data",
                bytes: extra.clone(),
            }],
            vec![(
                "x86_64_linux_application_syscall_instruction",
                image_base + 2,
            )],
        );
        bytes.extend_from_slice(&extra);
        elf.program_headers[0].file_size = bytes.len() as u64;
        elf.program_headers[0].memory_size = bytes.len() as u64;
        elf.section_headers[0].size += extra.len() as u64;
        symbols = symbols.replace(
            &format!("kernel_rodata_end R {start:x}"),
            &format!("kernel_rodata_end R {end:x}"),
        );
        symbols.push_str(&format!("x86_64_linux_application_data_start R {start:x}\nx86_64_linux_application_data_end R {end:x}\nx86_64_linux_application_syscall_instruction A {:x}\n", image_base + 2));
        inspect_embedding(&bytes, &elf, &symbols, &artifact).unwrap();
        assert!(
            inspect_embedding(
                &bytes,
                &elf,
                &symbols.replace(
                    "x86_64_linux_application_syscall_instruction A",
                    "x86_64_linux_application_syscall_instruction T"
                ),
                &artifact
            )
            .is_err()
        );
        bytes.extend_from_slice(&extra);
        assert!(inspect_embedding(&bytes, &elf, &symbols, &artifact).is_err());
    }

    #[test]
    fn elf_embedding_rejects_aliases_writable_domains_and_wrong_symbols() {
        let (bytes, elf, symbols, artifact) = embedding();
        let mut changed = elf.clone();
        changed.program_headers[0].flags = 7;
        assert!(inspect_embedding(&bytes, &changed, &symbols, &artifact).is_err());
        let mut changed = elf.clone();
        let mut load = changed.program_headers[0];
        load.virtual_address = 0x100103;
        load.file_size = 1;
        load.memory_size = 1;
        changed.program_headers.push(load);
        assert!(inspect_embedding(&bytes, &changed, &symbols, &artifact).is_err());
        let mut changed = elf.clone();
        changed.section_headers[0].flags = 3;
        assert!(inspect_embedding(&bytes, &changed, &symbols, &artifact).is_err());
        let mut changed = elf.clone();
        changed
            .section_headers
            .push(changed.section_headers[0].clone());
        assert!(inspect_embedding(&bytes, &changed, &symbols, &artifact).is_err());
        for (old, new) in [
            (
                "x86_64_elf_application_data_start R",
                "x86_64_elf_application_data_start D",
            ),
            (
                "x86_64_elf_application_loader_fault_instruction A",
                "x86_64_elf_application_loader_fault_instruction R",
            ),
            (
                "kernel_rodata_start R 100100",
                "kernel_rodata_start R 100101",
            ),
            (
                "x86_64_native_application_payload_start R 100100",
                "x86_64_native_application_payload_start R 100101",
            ),
        ] {
            assert!(
                inspect_embedding(&bytes, &elf, &symbols.replace(old, new), &artifact).is_err()
            );
        }
        assert!(
            check_symbols("", &artifact, u64::MAX)
                .unwrap_err()
                .contains("overflow")
        );
    }

    #[test]
    fn elf_application_markers_are_excluded_from_normal_images() {
        verify_absent("kernel_entry T 100000", b"normal").unwrap();
        for name in [
            "x86_64_elf_application_plan_start",
            "anomalous_elf_loader_entry",
            "anomalous_native_client_data_canary",
        ] {
            assert!(verify_absent(name, b"").is_err());
            assert!(verify_absent("", name.as_bytes()).is_err());
        }
    }
}
