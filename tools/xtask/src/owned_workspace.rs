//! Explicit opt-in low-address workspace artifact. The legacy workspace inspector
//! and payload remain a separate, unchanged high-address contract.
use super::*;

pub(super) const feature: &str = "owned_workspace_test";
const package: &str = "anomalous-workspace-service";
const target_name: &str = "x86_64-anomalous-workspace";
const image_base: u64 = 0x0040_0000;
const image_capacity: u64 = 0x20000;
const data_base: u64 = 0x0080_1000;
const data_capacity: u64 = 0x7000;
const symbol_prefix: &str = "anomalous_workspace_service";

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.contains(&owned_workspace_cpu::feature)
        && workspace_dev::selected(
            &features
                .iter()
                .copied()
                .filter(|value| *value != feature)
                .chain([workspace_dev::feature])
                .collect::<Vec<_>>(),
        )
}

pub(super) struct artifact {
    pub(super) object: PathBuf,
    image: Vec<u8>,
    data: Vec<u8>,
    fault_offset: u64,
    guard_offset: u64,
    nx_offset: u64,
    prefix: &'static str,
    probe_aliases: Vec<(&'static str, u64)>,
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[workspace_dev::feature, feature], None)?;
    println!("native workspace image inspected: {}", image.path.display());
    Ok(image)
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    package_profile(root, build, false)
}

pub(super) fn package_profile(root: &Path, build: &Path, cpu: bool) -> Result<artifact, String> {
    let directory = if cpu {
        "owned-workspace-cpu"
    } else {
        "owned-workspace"
    };
    let prefix = if cpu {
        "x86_64_owned_workspace_cpu_service"
    } else {
        "x86_64_owned_workspace_service"
    };
    let build = build.join(directory);
    fs::create_dir_all(&build).map_err(|_| "create owned workspace artifact directory")?;
    let build = build.as_path();
    let target = root.join(format!("targets/{target_name}.json"));
    verify_semantic_surface_target_cfg(root, &target)?;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    run_checked(
        root,
        cargo,
        [
            OsString::from("-Zjson-target-spec"),
            OsString::from("-Zbuild-std=core,compiler_builtins"),
            OsString::from("-Zbuild-std-features=compiler-builtins-mem"),
            OsString::from("build"),
            OsString::from("--release"),
            OsString::from("--package"),
            OsString::from(package),
            OsString::from("--target"),
            target.into_os_string(),
            OsString::from("--locked"),
            OsString::from("--no-default-features"),
            OsString::from("--features"),
            OsString::from(if cpu {
                "owned_cpu_probe"
            } else {
                "owned_admission"
            }),
        ],
    )?;
    let cargo_elf = root
        .join("target")
        .join(target_name)
        .join("release")
        .join(package);
    let elf = build.join("workspace_service.elf");
    fs::copy(&cargo_elf, &elf).map_err(|_| "retain workspace service ELF")?;
    let bytes = fs::read(&elf).map_err(|_| "read workspace service ELF")?;
    let parsed = parse_elf64(&bytes)?;
    let (image, data) = inspect_segments(&bytes, &parsed)?;
    let listing = String::from_utf8(run_output_checked(
        root,
        "nm",
        [OsString::from("--format=posix"), elf.as_os_str().to_owned()],
    )?)
    .map_err(|_| "workspace symbol listing is not UTF-8")?;
    for line in listing.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.get(1) == Some(&"U") {
            return Err("workspace service has undefined symbols".into());
        }
    }
    let symbol = |suffix: &str| exact_symbol(&listing, &format!("{symbol_prefix}_{suffix}"));
    if symbol("entry")? != image_base
        || symbol("image_start")? != image_base
        || symbol("image_payload_end")? != image_base + image.len() as u64
        || symbol("image_end")? != image_base + round_up_to_page(image.len() as u64)?
        || symbol("data_start")? != data_base
        || symbol("data_file_end")? != data_base + data.len() as u64
        || symbol("data_end")? != data_base + parsed.program_headers[1].memory_size
    {
        return Err("workspace ELF symbols disagree with inspected load extents".into());
    }
    let fault_offset = symbol("fault_instruction")?
        .checked_sub(image_base)
        .filter(|offset| {
            offset
                .checked_add(2)
                .is_some_and(|end| end <= image.len() as u64)
        })
        .ok_or("workspace controlled fault lies outside its executable payload")?;
    if image[fault_offset as usize..fault_offset as usize + 2] != [0x0f, 0x0b] {
        return Err("workspace controlled fault is not exact UD2".into());
    }
    if symbol("data_end")? > 0x804000 {
        return Err("owned workspace live data overlaps its wide zero-tail probes".into());
    }
    let guard_offset =
        instruction_offset(&image, symbol("guard_instruction")?, &[0x48, 0x89, 0x02])?;
    let nx_offset = instruction_offset(&image, symbol("nx_instruction")?, &[0xff, 0xe0])?;
    let probe_aliases = if cpu {
        owned_workspace_cpu::inspect_probe(root, build, &elf, &bytes, &parsed, &listing, &image)?
    } else {
        Vec::new()
    };
    fs::write(build.join("workspace_service.bin"), &image)
        .map_err(|_| "write workspace RX payload")?;
    fs::write(build.join("workspace_service_data.bin"), &data)
        .map_err(|_| "write workspace data payload")?;
    let mut source = format!(
        r#".section .rodata.owned_workspace_service,"a",@progbits
.balign 16
.global x86_64_owned_workspace_service_payload_start
.global x86_64_owned_workspace_service_payload_end
.global x86_64_owned_workspace_service_data_start
.global x86_64_owned_workspace_service_data_end
.global x86_64_owned_workspace_service_fault_instruction
.global x86_64_owned_workspace_service_guard_instruction
.global x86_64_owned_workspace_service_nx_instruction
x86_64_owned_workspace_service_payload_start:
.incbin "workspace_service.bin"
x86_64_owned_workspace_service_payload_end:
.set x86_64_owned_workspace_service_fault_instruction, x86_64_owned_workspace_service_payload_start + {fault_offset}
.set x86_64_owned_workspace_service_guard_instruction, x86_64_owned_workspace_service_payload_start + {guard_offset}
.set x86_64_owned_workspace_service_nx_instruction, x86_64_owned_workspace_service_payload_start + {nx_offset}
.balign 16
x86_64_owned_workspace_service_data_start:
.incbin "workspace_service_data.bin"
x86_64_owned_workspace_service_data_end:
.section .note.GNU-stack,"",@progbits
"#
    );
    if cpu {
        source = source
            .replace("x86_64_owned_workspace_service", prefix)
            .replace(
                ".rodata.owned_workspace_service",
                ".rodata.owned_workspace_cpu_service",
            );
        for (name, offset) in &probe_aliases {
            source.push_str(&format!(
                ".global {prefix}_{name}\n.set {prefix}_{name}, {prefix}_payload_start + {offset}\n"
            ));
        }
    }
    let source_path = build.join("workspace_service_payload.S");
    let object = build.join("workspace_service_payload.o");
    fs::write(&source_path, source).map_err(|_| "write workspace payload wrapper")?;
    run_checked(
        build,
        "as",
        [
            OsString::from("--64"),
            OsString::from("-o"),
            object.as_os_str().to_owned(),
            source_path.as_os_str().to_owned(),
        ],
    )?;
    Ok(artifact {
        object,
        image,
        data,
        fault_offset,
        guard_offset,
        nx_offset,
        prefix,
        probe_aliases,
    })
}

fn inspect_segments(bytes: &[u8], elf: &parsed_elf64) -> Result<(Vec<u8>, Vec<u8>), String> {
    if elf.kind != 2
        || elf.machine != 62
        || elf.entry != image_base
        || elf.program_headers.len() != 2
    {
        return Err(
            "workspace requires one static x86-64 RX segment and one private RW segment".into(),
        );
    }
    let rx = elf.program_headers[0];
    let rw = elf.program_headers[1];
    for (load, address, flags, capacity) in [
        (rx, image_base, 5, image_capacity),
        (rw, data_base, 6, data_capacity),
    ] {
        if load.kind != 1
            || load.flags != flags
            || load.virtual_address != address
            || load.physical_address != address
            || load.alignment != circle_page_size
            || load.memory_size > capacity
            || load.file_size > load.memory_size
            || load.offset % circle_page_size != address % circle_page_size
            || load
                .offset
                .checked_add(load.file_size)
                .is_none_or(|end| end > bytes.len() as u64)
        {
            return Err("workspace ELF load mapping violates W^X, geometry, or bounds".into());
        }
    }
    if rw.memory_size > 0x3000 {
        return Err("owned workspace live data overlaps its reserved probe tail".into());
    }
    if rx.file_size == 0
        || rx.file_size != rx.memory_size
        || rx.offset + rx.file_size > rw.offset && rw.file_size != 0
    {
        return Err("workspace executable extent is empty, writable, or overlaps its data".into());
    }
    for section in &elf.section_headers {
        if section.size == 0 {
            continue;
        }
        if matches!(section.kind, 4 | 6 | 9 | 11)
            || section.flags & 0x400 != 0
            || section.name.starts_with(".got")
            || section.name.starts_with(".plt")
            || section.name == ".interp"
        {
            return Err(
                "workspace requires no relocations, dynamic imports, TLS, GOT or PLT".into(),
            );
        }
        if section.flags & 2 == 0 {
            continue;
        }
        let end = section
            .address
            .checked_add(section.size)
            .ok_or("workspace section extent overflow")?;
        let load = if section.flags & 1 != 0 { rw } else { rx };
        if section.address < load.virtual_address
            || end > load.virtual_address + load.memory_size
            || section.flags & 1 != 0 && section.flags & 4 != 0
            || !matches!(section.kind, 1 | 8)
            || section.kind == 8 && load.flags != 6
        {
            return Err(
                "workspace allocated section lies outside its checked protection domain".into(),
            );
        }
        if section.kind != 8 {
            let relative = section.address - load.virtual_address;
            if relative + section.size > load.file_size || section.offset != load.offset + relative
            {
                return Err(
                    "workspace allocated section is not exactly backed by its load bytes".into(),
                );
            }
        }
    }
    Ok((
        elf_virtual_bytes(bytes, rx, image_base, rx.file_size as usize)?.to_vec(),
        elf_virtual_bytes(bytes, rw, data_base, rw.file_size as usize)?.to_vec(),
    ))
}

pub(super) fn inspect_linked(root: &Path, elf: &Path, artifact: &artifact) -> Result<(), String> {
    let bytes = fs::read(elf).map_err(|_| "read linked workspace kernel")?;
    let parsed = parse_elf64(&bytes)?;
    let listing = String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.as_os_str().to_owned(),
        ],
    )?)
    .map_err(|_| "linked workspace symbols are not UTF-8")?;
    for (prefix, payload) in [
        (format!("{}_payload", artifact.prefix), &artifact.image),
        (format!("{}_data", artifact.prefix), &artifact.data),
    ] {
        let start = exact_symbol(&listing, &format!("{prefix}_start"))?;
        let end = exact_symbol(&listing, &format!("{prefix}_end"))?;
        if end.checked_sub(start) != Some(payload.len() as u64) {
            return Err("linked workspace payload changed extent".into());
        }
        parsed
            .section_headers
            .iter()
            .find(|s| {
                s.flags & 2 != 0
                    && s.flags & 1 == 0
                    && s.kind == 1
                    && start >= s.address
                    && s.address
                        .checked_add(s.size)
                        .is_some_and(|limit| end <= limit)
            })
            .ok_or("workspace payload is not in read-only kernel storage")?;
        let load = readonly_payload_load(&parsed, start, end)?;
        if elf_virtual_bytes(&bytes, load, start, payload.len())? != payload {
            return Err("linked workspace payload bytes changed".into());
        }
    }
    let start = exact_symbol(&listing, &format!("{}_payload_start", artifact.prefix))?;
    if exact_symbol(&listing, &format!("{}_fault_instruction", artifact.prefix))?
        != start + artifact.fault_offset
    {
        return Err("workspace controlled fault alias moved".into());
    }
    for (suffix, offset) in [
        ("guard_instruction", artifact.guard_offset),
        ("nx_instruction", artifact.nx_offset),
    ]
    .into_iter()
    .chain(artifact.probe_aliases.iter().copied())
    {
        if exact_symbol(&listing, &format!("{}_{suffix}", artifact.prefix))? != start + offset {
            return Err("owned workspace probe alias moved".into());
        }
    }
    Ok(())
}

pub(super) fn exact_symbol(listing: &str, name: &str) -> Result<u64, String> {
    let mut found = None;
    for line in listing.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.first().copied() != Some(name) {
            continue;
        }
        if found.is_some()
            || !(3..=4).contains(&fields.len())
            || !matches!(
                fields[1],
                "A" | "a" | "T" | "t" | "R" | "r" | "D" | "d" | "B" | "b"
            )
        {
            return Err(format!(
                "owned workspace symbol {name} is duplicate or malformed"
            ));
        }
        found = Some(
            u64::from_str_radix(fields[2], 16)
                .map_err(|_| "owned workspace symbol value malformed")?,
        );
        if fields.len() == 4 && u64::from_str_radix(fields[3], 16).is_err() {
            return Err("owned workspace symbol size malformed".into());
        }
    }
    found.ok_or_else(|| format!("owned workspace symbol {name} missing"))
}

fn instruction_offset(image: &[u8], address: u64, instruction: &[u8]) -> Result<u64, String> {
    let offset = address
        .checked_sub(image_base)
        .ok_or("owned probe precedes RX")?;
    let start = usize::try_from(offset).map_err(|_| "owned probe offset overflow")?;
    let end = start
        .checked_add(instruction.len())
        .ok_or("owned probe end overflow")?;
    if image.get(start..end) != Some(instruction) {
        return Err("owned workspace probe instruction differs from its exact contract".into());
    }
    Ok(offset)
}

fn readonly_payload_load(
    elf: &parsed_elf64,
    start: u64,
    end: u64,
) -> Result<elf64_program_header, String> {
    let mut matching = elf.program_headers.iter().copied().filter(|load| {
        load.kind == 1
            && start >= load.virtual_address
            && load
                .virtual_address
                .checked_add(load.file_size)
                .is_some_and(|limit| end <= limit)
    });
    let load = matching
        .next()
        .ok_or("workspace payload has no backing kernel load")?;
    if start > end
        || load.flags != 5
        || matching.next().is_some()
        || elf.program_headers.iter().any(|other| {
            if other.kind != 1 {
                return false;
            }
            let same = other.offset == load.offset
                && other.virtual_address == load.virtual_address
                && other.file_size == load.file_size
                && other.memory_size == load.memory_size
                && other.flags == load.flags;
            !same
                && other
                    .virtual_address
                    .checked_add(other.memory_size)
                    .is_none_or(|limit| start < limit && other.virtual_address < end)
        })
    {
        return Err("workspace payload requires one readable nonwritable kernel load".into());
    }
    Ok(load)
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in ["owned_workspace", "x86_64_owned_workspace_service"] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains owned workspace admission".into());
        }
    }
    Ok(())
}

pub(super) fn smoke() -> Result<(), String> {
    let image = build()?;
    workspace_dev::smoke_image(&image, workspace_dev::workspace_profile::owned)?;
    workspace_dev::editor_smoke_image(&image, workspace_dev::workspace_profile::owned)
}

pub(super) fn prepared_record(scenario: &str) -> String {
    format!(
        r#"{{"schema":1,"event":"owned_workspace.prepared","level":"info","scenario":"{scenario}","code_pages":32,"data_pages":8,"stack_pages":16,"transition_pages":4,"table_frames":9,"owner_frames":69,"owner_leases":5,"stage_frames":576,"populated":true,"zero_tail":true,"guards":true}}"#
    )
}

pub(super) fn released_record(scenario: &str, reason: &str, frames: usize) -> String {
    format!(
        r#"{{"schema":1,"event":"owned_workspace.released","level":"info","scenario":"{scenario}","reason":"{reason}","frames_presented":{frames},"wide_data_probe":true,"wide_stack_probe":true,"root_restored":true,"resources_restored":true}}"#
    )
}

/// Validate every admission/retirement record before applying the unchanged
/// workspace frame/input oracle to the remaining, exactly ordered lifecycle.
pub(super) fn verify_transcript(
    transcript: &str,
    pages: u64,
    frames: usize,
    fallback: Option<&str>,
) -> Result<(), String> {
    if pages != 32 || frames == 0 || transcript.len() > 128 * 1024 {
        return Err("owned workspace transcript exceeds its fixed admission".into());
    }
    let body = transcript
        .strip_suffix("\r\n")
        .ok_or("owned workspace requires complete CRLF records")?;
    let mut lines = body.split("\r\n").map(str::to_owned).collect::<Vec<_>>();
    let prelude = [
        prepared_record("guard"),
        released_record("guard", "guard_fault", 0),
        prepared_record("nx"),
        released_record("nx", "nx_fault", 0),
        prepared_record("desktop"),
    ];
    if lines.get(8..13) != Some(prelude.as_slice()) {
        return Err("owned workspace admission and fault cleanup are not exact".into());
    }
    lines.drain(8..13);
    if let Some(reason) = fallback {
        if !matches!(
            reason,
            "controlled_fault" | "user_fault" | "task_exit" | "watchdog"
        ) {
            return Err("owned workspace has an unknown retirement reason".into());
        }
        let index = 10_usize
            .checked_add(frames)
            .ok_or("owned workspace frame index overflow")?;
        if lines.get(index) != Some(&released_record("desktop", reason, frames)) {
            return Err("owned workspace desktop did not restore its exact owner".into());
        }
        lines.remove(index);
    }
    workspace_dev::verify_transcript(&(lines.join("\r\n") + "\r\n"), pages, frames, fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<u8>, parsed_elf64) {
        let bytes = vec![0_u8; 0x2010];
        let load = |flags, offset, address, file_size, memory_size| elf64_program_header {
            kind: 1,
            flags,
            offset,
            virtual_address: address,
            physical_address: address,
            file_size,
            memory_size,
            alignment: 4096,
        };
        let section = |name: &str, flags, address, offset, size, kind| elf64_section_header {
            name: name.into(),
            kind,
            flags,
            address,
            offset,
            size,
        };
        (
            bytes,
            parsed_elf64 {
                kind: 2,
                machine: 62,
                entry: image_base,
                program_headers: vec![
                    load(5, 0x1000, image_base, 64, 64),
                    load(6, 0x2000, data_base, 16, 32),
                ],
                section_headers: vec![
                    section(".text", 6, image_base, 0x1000, 64, 1),
                    section(".data", 3, data_base, 0x2000, 16, 1),
                    section(".bss", 3, data_base + 16, 0x2010, 16, 8),
                ],
            },
        )
    }

    #[test]
    fn low_geometry_rejects_high_profile_permissions_bss_and_bad_backing() {
        let (bytes, valid) = fixture();
        assert_eq!(
            inspect_segments(&bytes, &valid).unwrap(),
            (vec![0; 64], vec![0; 16])
        );
        let mut mutations = Vec::new();
        let mut high = valid.clone();
        high.entry = 0x600000001000;
        mutations.push(high);
        for which in 0..2 {
            for flags in [0, 4, 7] {
                let mut changed = valid.clone();
                changed.program_headers[which].flags = flags;
                mutations.push(changed);
            }
            let mut changed = valid.clone();
            changed.program_headers[which].physical_address += 4096;
            mutations.push(changed);
            let mut changed = valid.clone();
            changed.program_headers[which].offset = u64::MAX;
            mutations.push(changed);
            let mut changed = valid.clone();
            changed.program_headers[which].memory_size = if which == 0 {
                image_capacity + 1
            } else {
                data_capacity + 1
            };
            mutations.push(changed);
        }
        for which in 0..3 {
            let mut changed = valid.clone();
            changed.section_headers[which].address = u64::MAX;
            mutations.push(changed);
            let mut changed = valid.clone();
            changed.section_headers[which].flags |= 0x400;
            mutations.push(changed);
            let mut changed = valid.clone();
            changed.section_headers[which].kind = 4;
            mutations.push(changed);
        }
        let mut changed = valid.clone();
        changed.program_headers[1].memory_size = 0x3001;
        mutations.push(changed);
        let mut changed = valid.clone();
        changed.section_headers[1].flags |= 4;
        mutations.push(changed);
        let mut changed = valid.clone();
        changed.section_headers[0].offset += 1;
        mutations.push(changed);
        let mut changed = valid.clone();
        changed.section_headers[1].name = ".got".into();
        mutations.push(changed);
        let mut changed = valid.clone();
        changed.program_headers[0].file_size = 0;
        mutations.push(changed);
        for changed in mutations {
            assert!(inspect_segments(&bytes, &changed).is_err(), "{changed:?}");
        }
        assert!(inspect_segments(&bytes[..bytes.len() - 1], &valid).is_err());
    }

    #[test]
    fn symbols_require_one_posix_definition_with_optional_size() {
        assert_eq!(
            exact_symbol("entry T 400000 23\n", "entry").unwrap(),
            image_base
        );
        assert_eq!(
            exact_symbol("entry A 400000\n", "entry").unwrap(),
            image_base
        );
        for bad in [
            "entry U",
            "entry T nope",
            "entry T 400000 nope",
            "400000 T entry",
            "entry T 400000\nentry T 400000",
            "entry T 400000 1 extra",
        ] {
            assert!(exact_symbol(bad, "entry").is_err(), "{bad}");
        }
    }

    #[test]
    fn probes_require_exact_instructions_inside_inspected_rx() {
        let image = [0x48, 0x89, 0x02, 0xff, 0xe0, 0x0f, 0x0b];
        assert_eq!(
            instruction_offset(&image, image_base, &[0x48, 0x89, 0x02]).unwrap(),
            0
        );
        assert_eq!(
            instruction_offset(&image, image_base + 3, &[0xff, 0xe0]).unwrap(),
            3
        );
        for address in [
            image_base - 1,
            image_base + 1,
            image_base + image.len() as u64,
            u64::MAX,
        ] {
            assert!(instruction_offset(&image, address, &[0x48, 0x89, 0x02]).is_err());
        }
        assert!(instruction_offset(&image, image_base, &[0x48, 0x89, 0x03]).is_err());
    }

    #[test]
    fn linked_payload_rejects_partial_competing_mapping() {
        let (_, valid) = fixture();
        assert!(readonly_payload_load(&valid, image_base, image_base + 64).is_ok());
        let mut overlap = valid.clone();
        let mut load = overlap.program_headers[0];
        load.virtual_address = image_base + 32;
        load.memory_size = 64;
        load.file_size = 64;
        load.flags = 6;
        overlap.program_headers.push(load);
        assert!(readonly_payload_load(&overlap, image_base, image_base + 64).is_err());
        let mut duplicate = valid.clone();
        duplicate.program_headers.push(duplicate.program_headers[0]);
        assert!(readonly_payload_load(&duplicate, image_base, image_base + 64).is_err());
    }

    fn transcript(fallback: bool) -> String {
        let boot = crate::tests::valid_semantic_presentation_transcript();
        let mut lines = boot
            .split("\r\n")
            .take(8)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        lines.extend([
            prepared_record("guard"),
            released_record("guard", "guard_fault", 0),
            prepared_record("nx"),
            released_record("nx", "nx_fault", 0),
            prepared_record("desktop"),
        ]);
        lines.push(r#"{"schema":1,"event":"workspace_pointer.ready","level":"info","available":true,"generation":1}"#.into());
        lines.push(r#"{"schema":1,"event":"workspace_service.ready","level":"info","image_pages":32,"data_pages":8,"stack_pages":16,"stage_frames":576,"rights":30}"#.into());
        lines.push(r#"{"schema":1,"event":"workspace_service.frame_presented","level":"info","sequence":1,"rows":768,"width":1024,"height":768}"#.into());
        if fallback {
            lines.push(released_record("desktop", "task_exit", 1));
            lines.extend(workspace_dev::fallback_records(1, "task_exit"));
        }
        lines.join("\r\n") + "\r\n"
    }

    #[test]
    fn lifecycle_requires_both_faults_exact_ownership_and_completed_release() {
        for fallback in [false, true] {
            let valid = transcript(fallback);
            let reason = fallback.then_some("task_exit");
            verify_transcript(&valid, 32, 1, reason).unwrap();
            for (from, to) in [
                ("\"table_frames\":9", "\"table_frames\":8"),
                ("\"owner_frames\":69", "\"owner_frames\":68"),
                ("\"owner_leases\":5", "\"owner_leases\":4"),
                ("\"populated\":true", "\"populated\":false"),
                ("\"zero_tail\":true", "\"zero_tail\":false"),
                ("\"guards\":true", "\"guards\":false"),
                ("\"wide_data_probe\":true", "\"wide_data_probe\":false"),
                ("\"wide_stack_probe\":true", "\"wide_stack_probe\":false"),
                ("\"root_restored\":true", "\"root_restored\":false"),
                ("guard_fault", "nx_fault"),
            ] {
                assert!(
                    verify_transcript(&valid.replace(from, to), 32, 1, reason).is_err(),
                    "{from}"
                );
            }
            for index in 0..valid.lines().count() {
                let mut lines = valid
                    .trim_end_matches("\r\n")
                    .split("\r\n")
                    .collect::<Vec<_>>();
                lines.remove(index);
                assert!(verify_transcript(&(lines.join("\r\n") + "\r\n"), 32, 1, reason).is_err());
            }
            assert!(verify_transcript(&(valid.clone() + "{}\r\n"), 32, 1, reason).is_err());
            assert!(verify_transcript(&valid.replace("\r\n", "\n"), 32, 1, reason).is_err());
            assert!(verify_transcript(&valid, 31, 1, reason).is_err());
            assert!(workspace_dev::verify_transcript(&valid, 32, 1, reason).is_err());
        }
    }

    #[test]
    fn opt_in_selection_keeps_existing_precedence_and_normal_exclusion() {
        assert!(selected(&[feature]));
        assert!(selected(&[feature, workspace_dev::feature]));
        assert!(!workspace_dev::selected(&[feature, workspace_dev::feature]));
        assert!(!selected(&[]));
        assert!(!selected(all_kernel_features));
        for other in all_kernel_features.iter().copied().filter(|value| {
            !matches!(
                *value,
                "owned_workspace_test" | "workspace_dev" | "scheduler_test" | "smoke_test"
            )
        }) {
            assert!(!selected(&[feature, other]), "{other}");
        }
        assert!(verify_absent("normal", b"Threshold").is_ok());
        assert!(verify_absent("x86_64_owned_workspace_service_payload_start", b"").is_err());
        assert!(verify_absent("", b"owned_workspace.prepared").is_err());
    }
}
