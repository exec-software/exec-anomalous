//! Host packaging of a native durable client/service alongside the native baseline.
//! Host ELF is a build input; no foreign execution personality is introduced.
use super::*;
use elf_application_artifact::{build_vessel, inspect_native_durable_vessel, retain_vessel};

const prefix: &str = "anomalous_native_durable_client";
const payload: &str = "x86_64_native_durable_failure_payload";
const fault_names: [&str; 2] = ["fault_instruction", "cut_fault_instruction"];
const image_base: u64 = 0x400000;
const startup_address: u64 = 0x801000;
const startup_length: u64 = 192;
const fault_instruction: [u8; 2] = [0x0f, 0x0b];
const startup_entry: [u8; 10] = [0xfc, 0x48, 0xc7, 0xc0, 0, 0, 0, 0, 0xcd, 0xf4];
const transport_entry: [u8; 25] = [
    0x53, 0x48, 0x89, 0xfb, 0x48, 0x8b, 0x03, 0x48, 0x8b, 0x7b, 0x08, 0x48, 0x8b, 0x73, 0x10, 0x48,
    0x8b, 0x53, 0x18, 0x48, 0x8b, 0x4b, 0x20, 0xcd, 0xf4,
];
const transport_return: [u8; 14] = [
    0x48, 0x89, 0x43, 0x28, 0x48, 0x89, 0x4b, 0x30, 0x48, 0x89, 0x53, 0x38, 0x5b, 0xc3,
];

/// Require one exact symbol; a duplicate or differently typed alias cannot
/// silently win merely by appearing earlier in a tool's symbol listing.
pub(super) fn exact_symbol(symbols: &str, name: &str, kind: char) -> Result<u64, String> {
    let matches = symbols
        .lines()
        .filter_map(|line| {
            let words = line.split_whitespace().collect::<Vec<_>>();
            (words.first() == Some(&name)).then_some(words)
        })
        .collect::<Vec<_>>();
    if matches.len() != 1
        || !matches!(matches[0].len(), 3 | 4)
        || matches[0][1].as_bytes() != [kind as u8]
        || (matches[0].len() == 4 && u64::from_str_radix(matches[0][3], 16).is_err())
    {
        return Err(format!(
            "durable symbol {name} missing, duplicated or mistyped"
        ));
    }
    u64::from_str_radix(matches[0][2], 16)
        .map_err(|_| format!("durable symbol {name} address invalid"))
}

fn expect_symbol(symbols: &str, name: &str, kind: char, expected: u64) -> Result<(), String> {
    if exact_symbol(symbols, name, kind)? != expected {
        return Err(format!("durable symbol {name} address changed"));
    }
    Ok(())
}

fn instruction_bytes(
    image: &[u8],
    text_end: u64,
    address: u64,
    length: usize,
) -> Result<&[u8], String> {
    let end = address
        .checked_add(length as u64)
        .ok_or("durable instruction extent overflow")?;
    let offset = address
        .checked_sub(image_base)
        .ok_or("durable instruction below text")?;
    if end > text_end {
        return Err("durable instruction escaped initialized text".into());
    }
    let offset = usize::try_from(offset).map_err(|_| "durable instruction offset overflow")?;
    image
        .get(
            offset
                ..offset
                    .checked_add(length)
                    .ok_or("durable instruction index overflow")?,
        )
        .ok_or_else(|| "durable instruction escaped payload".into())
}

/// The general vessel inspector admits the ELF and complete CPU instruction
/// profile first. These additional checks bind the specific native startup and
/// unexpected-fault instruction boundary to the symbols used by the kernel wrapper.
fn inspect_client_contract(
    symbols: &str,
    image: &[u8],
    text_address: u64,
    text_size: u64,
    rw: &[u8],
    rw_memory_size: u64,
) -> Result<[u64; 2], String> {
    let text_end = text_address
        .checked_add(text_size)
        .ok_or("durable text overflow")?;
    if text_address != image_base || text_size == 0 || text_size > image.len() as u64 {
        return Err("durable initialized text geometry changed".into());
    }
    let symbol = |suffix: &str, kind| exact_symbol(symbols, &format!("{prefix}_{suffix}"), kind);
    if symbol("entry", 'T')? != image_base
        || symbol("startup_return", 'T')? != image_base + startup_entry.len() as u64
        || instruction_bytes(image, text_end, image_base, startup_entry.len())? != startup_entry
    {
        return Err("durable startup register-preserving gate changed".into());
    }
    let returned = symbol("startup_return", 'T')?;
    let continuation = instruction_bytes(image, text_end, returned, 7)?;
    let displacement = i32::from_le_bytes(continuation[1..5].try_into().unwrap());
    let run = symbol("run", 'T')?;
    if continuation[0] != 0xe8
        || continuation[5..] != [0x0f, 0x0b]
        || returned
            .checked_add(5)
            .and_then(|next| next.checked_add_signed(i64::from(displacement)))
            != Some(run)
    {
        return Err("durable startup continuation does not call the exact Rust entry".into());
    }
    instruction_bytes(image, text_end, run, 1)?;
    let call = symbol("call", 'T')?;
    let syscall_return = symbol("syscall_return", 'T')?;
    if call.checked_add(transport_entry.len() as u64) != Some(syscall_return)
        || instruction_bytes(image, text_end, call, transport_entry.len())? != transport_entry
        || instruction_bytes(image, text_end, syscall_return, transport_return.len())?
            != transport_return
    {
        return Err("durable native transport register contract changed".into());
    }
    let mut faults = [0; 2];
    for (index, suffix) in fault_names.iter().enumerate() {
        faults[index] = symbol(suffix, 'T')?;
    }
    for address in faults {
        if instruction_bytes(image, text_end, address, 2)? != fault_instruction {
            return Err("durable failure marker changed".into());
        }
    }
    if faults[0] == faults[1] {
        return Err("durable failure markers alias".into());
    }
    for suffix in ["startup", "descriptor_start"] {
        if symbol(suffix, 'D')? != startup_address {
            return Err("durable startup address changed".into());
        }
    }
    if symbol("descriptor_end", 'D')? != startup_address + startup_length
        || rw.len() < (4096 + startup_length) as usize
        || rw_memory_size < rw.len() as u64
        || rw_memory_size > 8192
        || rw.iter().any(|byte| *byte != 0)
    {
        return Err("durable startup storage or zeroed RW profile changed".into());
    }
    Ok(faults)
}

pub(super) struct artifact {
    pub(super) object: PathBuf,
    baseline: native_application_artifact::artifact,
    image: Vec<u8>,
    faults: [u64; 2],
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let baseline = native_application_artifact::package_image(root, build)?;
    let output = build.join("native-durable-failure");
    fs::create_dir_all(&output).map_err(|e| format!("create durable artifact directory: {e}"))?;
    let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
    for name in ["qualification.txt", "client.elf", "payload.o", "durable.o"] {
        remove_optional_file(&output.join(name))?;
    }
    build_vessel(
        root,
        "anomalous-native-durable-client",
        Some("failure_proof"),
        None,
    )?;
    let path = retain_vessel(
        root,
        &output,
        "anomalous-native-durable-client",
        "client.elf",
    )?;
    let client = inspect_native_durable_vessel(root, &path)?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let symbols = native_application_artifact::listing(root, &path)?;
    let text = elf
        .section_headers
        .iter()
        .find(|s| s.name == ".text")
        .ok_or("durable text missing")?;
    let faults = inspect_client_contract(
        &symbols,
        &client.rx,
        text.address,
        text.size,
        &client.rw,
        client.headers[1].memory_size,
    )?;
    if faults[0] != client.fault {
        return Err("durable generic and specific fault markers disagree".into());
    }
    fs::write(output.join("client_rx.bin"), &client.rx).map_err(|e| e.to_string())?;
    let mut assembly = format!(
        ".section .rodata.native_durable_failure,\"a\",@progbits\n.balign 16\n.global {payload}_start\n.global {payload}_end\n{payload}_start:\n.incbin \"client_rx.bin\"\n{payload}_end:\n"
    );
    for (suffix, address) in fault_names.iter().zip(faults) {
        assembly.push_str(&format!(".global x86_64_native_durable_failure_user_{suffix}\n.set x86_64_native_durable_failure_user_{suffix}, 0x{address:x}\n"));
    }
    assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    fs::write(output.join("payload.S"), assembly).map_err(|e| e.to_string())?;
    run_checked(
        &output,
        "as",
        [
            "--64".into(),
            "--fatal-warnings".into(),
            "-o".into(),
            "durable.o".into(),
            "payload.S".into(),
        ],
    )?;
    run_checked(
        &output,
        "ld",
        [
            "-r".into(),
            "--build-id=none".into(),
            "-o".into(),
            "payload.o".into(),
            baseline.object.as_os_str().to_owned(),
            "durable.o".into(),
        ],
    )?;
    let artifact = artifact {
        object: output.join("payload.o"),
        baseline,
        image: client.rx,
        faults,
    };
    inspect_wrapper(root, &artifact)?;
    let mut receipt = format!(
        "profile=native_durable_failure_v2\nentry=0x400000\nstartup=0x801000\nstartup_bytes=192\narena_bytes=32768\nrx_bytes={}\nrw_bytes={}\n",
        artifact.image.len(),
        client.headers[1].memory_size
    );
    for name in ["client.elf", "client_rx.bin", "durable.o", "payload.o"] {
        let hash = run_output_checked(root, "sha256sum", [output.join(name).into_os_string()])?;
        receipt.push_str(&String::from_utf8(hash).map_err(|_| "durable hash is not UTF8")?);
    }
    fs::write(output.join("qualification.txt"), receipt).map_err(|e| e.to_string())?;
    Ok(artifact)
}

fn inspect_wrapper(root: &Path, artifact: &artifact) -> Result<(), String> {
    let bytes = fs::read(&artifact.object).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let sections = elf
        .section_headers
        .iter()
        .filter(|s| s.flags & 2 != 0 && s.size != 0)
        .collect::<Vec<_>>();
    if elf.kind != 1
        || elf.machine != 62
        || elf.entry != 0
        || !elf.program_headers.is_empty()
        || elf.section_headers.iter().any(|s| matches!(s.kind, 4 | 9))
        || sections.len() != 2
    {
        return Err(
            "durable wrapper must contain only two readonly relocation-free payloads".into(),
        );
    }
    for section in sections {
        if section.kind != 1 || section.flags != 2 || section.address != 0 {
            return Err("durable wrapper payload permissions changed".into());
        }
        let expected = match section.name.as_str() {
            ".rodata.native_application" => fs::read(
                artifact
                    .baseline
                    .object
                    .parent()
                    .ok_or("baseline directory")?
                    .join("client_rx.bin"),
            )
            .map_err(|e| e.to_string())?,
            ".rodata.native_durable_failure" => artifact.image.clone(),
            _ => return Err("durable wrapper contains undeclared bytes".into()),
        };
        if native_application_artifact::section_bytes(&bytes, section)? != expected {
            return Err("durable wrapper changed inspected source bytes".into());
        }
    }
    let notes = elf
        .section_headers
        .iter()
        .filter(|s| s.name == ".note.GNU-stack")
        .collect::<Vec<_>>();
    if notes.len() != 1 || notes[0].kind != 1 || notes[0].flags != 0 || notes[0].size != 0 {
        return Err("durable wrapper lost nonexecutable stack declaration".into());
    }
    check_symbols(
        &native_application_artifact::listing(root, &artifact.object)?,
        artifact,
        0,
    )
}

fn check_symbols(symbols: &str, artifact: &artifact, start: u64) -> Result<(), String> {
    inspect_wrapper_symbols(symbols, artifact.image.len(), artifact.faults, start)
}

fn inspect_wrapper_symbols(
    symbols: &str,
    image_length: usize,
    faults: [u64; 2],
    start: u64,
) -> Result<(), String> {
    let mut expected = vec![
        (format!("{payload}_start"), start, 'R'),
        (
            format!("{payload}_end"),
            start
                .checked_add(image_length as u64)
                .ok_or("durable payload overflow")?,
            'R',
        ),
    ];
    for (suffix, value) in fault_names.iter().zip(faults) {
        expected.push((
            format!("x86_64_native_durable_failure_user_{suffix}"),
            value,
            'A',
        ));
    }
    for (name, value, kind) in &expected {
        expect_symbol(symbols, name, *kind, *value)?;
    }
    for name in symbols
        .lines()
        .filter_map(|line| line.split_whitespace().next())
    {
        if name.starts_with("x86_64_native_durable_failure_")
            && !expected.iter().any(|entry| entry.0 == name)
        {
            return Err("durable wrapper exports an undeclared alias".into());
        }
    }
    Ok(())
}

pub(super) fn inspect_linked(
    root: &Path,
    kernel: &Path,
    artifact: &artifact,
) -> Result<(), String> {
    inspect_linked_with_copies(root, kernel, artifact, 1)
}

/// The native durable wrapper must retain exactly one inspected payload.
pub(super) fn inspect_linked_with_copies(
    root: &Path,
    kernel: &Path,
    artifact: &artifact,
    expected_copies: usize,
) -> Result<(), String> {
    native_application_artifact::inspect_linked(root, kernel, &artifact.baseline)?;
    let bytes = fs::read(kernel).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let symbols = native_application_artifact::listing(root, kernel)?;
    let start = exact_symbol(&symbols, &format!("{payload}_start"), 'R')?;
    check_symbols(&symbols, artifact, start)?;
    let end = start
        .checked_add(artifact.image.len() as u64)
        .ok_or("durable linked overflow")?;
    let sections = elf
        .section_headers
        .iter()
        .filter(|s| {
            s.flags & 2 != 0
                && s.address < end
                && s.address.checked_add(s.size).is_none_or(|v| start < v)
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
                    .is_none_or(|v| start < v)
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
            .is_none_or(|v| end > v)
        || loads.len() != 1
        || loads[0].flags != 5
        || elf_virtual_bytes(&bytes, loads[0], start, artifact.image.len())? != artifact.image
        || expected_copies == 0
        || count_subslice(&bytes, &artifact.image) != expected_copies
    {
        return Err("durable linked payload changed or lost unique readonly backing".into());
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "native_durable_failure_profile",
        "native_storage_device",
        "native_durable_fixture",
        "opt/anomalous-durable",
        "kernel.native_durable_failure_test",
        "x86_64_native_durable",
        "anomalous_native_durable_client",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
            return Err("normal image retains native durable proof".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<u8>, String, Vec<u8>, [u64; 2]) {
        let mut image = vec![0x90; 256];
        image[..startup_entry.len()].copy_from_slice(&startup_entry);
        image[10] = 0xe8;
        image[11..15].copy_from_slice(&177i32.to_le_bytes());
        image[15..17].copy_from_slice(&[0x0f, 0x0b]);
        image[32..57].copy_from_slice(&transport_entry);
        image[57..71].copy_from_slice(&transport_return);
        image[176..178].copy_from_slice(&fault_instruction);
        image[180..182].copy_from_slice(&fault_instruction);
        image[192] = 0xc3;
        let symbols = [
            ("entry", image_base, 'T'),
            ("startup_return", image_base + 10, 'T'),
            ("run", image_base + 192, 'T'),
            ("call", image_base + 32, 'T'),
            ("syscall_return", image_base + 57, 'T'),
            ("fault_instruction", image_base + 176, 'T'),
            ("cut_fault_instruction", image_base + 180, 'T'),
            ("startup", startup_address, 'D'),
            ("descriptor_start", startup_address, 'D'),
            ("descriptor_end", startup_address + startup_length, 'D'),
        ]
        .into_iter()
        .map(|(suffix, address, kind)| {
            // POSIX nm includes a size for functions and omits it for labels.
            if matches!(suffix, "entry" | "run" | "call") {
                format!("{prefix}_{suffix} {kind} {address:x} 1\n")
            } else {
                format!("{prefix}_{suffix} {kind} {address:x}\n")
            }
        })
        .collect::<String>();
        (
            image,
            symbols,
            vec![0; 4288],
            [image_base + 176, image_base + 180],
        )
    }

    #[test]
    fn exact_startup_transport_and_service_fault_contract_is_accepted() {
        let (image, symbols, rw, faults) = fixture();
        assert_eq!(
            inspect_client_contract(&symbols, &image, image_base, 256, &rw, 4288).unwrap(),
            faults
        );
    }

    #[test]
    fn exact_symbol_uses_posix_nm_functions_and_unsized_aliases() {
        let symbols = "function T 400000 25\nalias A 400063\n";
        assert_eq!(exact_symbol(symbols, "function", 'T').unwrap(), image_base);
        assert_eq!(
            exact_symbol(symbols, "alias", 'A').unwrap(),
            image_base + 0x63
        );
        for malformed in [
            "function T 400000 invalid\n",
            "function T 400000 25 extra\n",
            "function T invalid 25\n",
            "function T 400000\nfunction T 400000 25\n",
            "400000 T function\n",
        ] {
            assert!(exact_symbol(malformed, "function", 'T').is_err());
        }
    }

    #[test]
    fn every_bound_instruction_byte_is_checked_including_gate_origins() {
        let (image, symbols, rw, _) = fixture();
        for (start, length) in [(0, 17), (32, 39), (176, 2), (180, 2)] {
            for offset in start..start + length {
                let mut changed = image.clone();
                changed[offset] ^= 1;
                assert!(
                    inspect_client_contract(&symbols, &changed, image_base, 256, &rw, 4288)
                        .is_err(),
                    "offset {offset}"
                );
            }
        }
    }

    #[test]
    fn marker_retargeting_duplicates_wrong_types_and_prefixes_are_rejected() {
        let (image, symbols, rw, _) = fixture();
        for suffix in [
            "entry",
            "startup_return",
            "run",
            "call",
            "syscall_return",
            "fault_instruction",
            "cut_fault_instruction",
            "startup",
            "descriptor_start",
            "descriptor_end",
        ] {
            let name = format!("{prefix}_{suffix}");
            let line = symbols
                .lines()
                .find(|line| line.starts_with(&format!("{name} ")))
                .unwrap();
            let mut words = line.split_whitespace();
            assert_eq!(words.next(), Some(name.as_str()));
            let kind = words.next().unwrap();
            let address = u64::from_str_radix(words.next().unwrap(), 16).unwrap();
            for replacement in [
                format!("{name} {kind} {:x}", address + 1),
                format!("{name} A {address:x}"),
                format!("wrong_{suffix} {kind} {address:x}"),
                format!("{line}\n{line}"),
                String::new(),
            ] {
                let changed = symbols.replace(line, &replacement);
                assert!(
                    inspect_client_contract(&changed, &image, image_base, 256, &rw, 4288).is_err(),
                    "symbol {suffix}"
                );
            }
        }
    }

    #[test]
    fn startup_must_have_complete_zeroed_private_file_backing() {
        let (image, symbols, rw, _) = fixture();
        for index in [0, 127, 4095, 4096, 4287] {
            let mut changed = rw.clone();
            changed[index] = 1;
            assert!(
                inspect_client_contract(&symbols, &image, image_base, 256, &changed, 4288).is_err()
            );
        }
        for (file_length, memory_length) in [(4287, 4288), (4288, 4287), (4288, 8193)] {
            assert!(
                inspect_client_contract(
                    &symbols,
                    &image,
                    image_base,
                    256,
                    &rw[..file_length],
                    memory_length
                )
                .is_err()
            );
        }
        for (address, length) in [
            (image_base + 1, 256),
            (image_base, 166),
            (image_base, 257),
            (u64::MAX, 256),
            (image_base, u64::MAX),
        ] {
            assert!(inspect_client_contract(&symbols, &image, address, length, &rw, 4288).is_err());
        }
    }

    fn wrapper_symbols(start: u64, length: usize, faults: [u64; 2]) -> String {
        let mut symbols = format!(
            "{payload}_start R {start:x}\n{payload}_end R {:x}\n",
            start + length as u64
        );
        for (suffix, value) in fault_names.iter().zip(faults) {
            symbols.push_str(&format!(
                "x86_64_native_durable_failure_user_{suffix} A {value:x}\n"
            ));
        }
        symbols
    }

    #[test]
    fn wrapper_requires_exact_unique_extents_and_absolute_fault_aliases() {
        let (image, _, _, faults) = fixture();
        for start in [0, 0x130000] {
            let symbols = wrapper_symbols(start, image.len(), faults);
            inspect_wrapper_symbols(&symbols, image.len(), faults, start).unwrap();
            for line in symbols.lines() {
                let parts = line.split_whitespace().collect::<Vec<_>>();
                let address = u64::from_str_radix(parts[2], 16).unwrap();
                for replacement in [
                    format!("{} {} {:x}", parts[0], parts[1], address + 1),
                    format!("{} T {address:x}", parts[0]),
                    format!("{line}\n{line}"),
                    String::new(),
                ] {
                    assert!(
                        inspect_wrapper_symbols(
                            &symbols.replace(line, &replacement),
                            image.len(),
                            faults,
                            start
                        )
                        .is_err()
                    );
                }
            }
            let extra =
                format!("{symbols}x86_64_native_durable_failure_user_undeclared A 400000\n");
            assert!(inspect_wrapper_symbols(&extra, image.len(), faults, start).is_err());
        }
        assert!(inspect_wrapper_symbols("", 256, faults, u64::MAX - 8).is_err());
    }
}
