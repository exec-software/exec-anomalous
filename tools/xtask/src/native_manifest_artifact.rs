//! Host packaging for the separate native manifest-admission loader.
//! Only its inspected payload is embedded; program provisioning is independent.
//! The measured loader occupies five RX pages under its separate named profile.
use super::*;
use elf_application_artifact::{build_vessel, inspect_native_manifest_vessel, retain_vessel};

const prefix: &str = "anomalous_native_manifest_loader";
const payload: &str = "x86_64_native_manifest_payload";
const image_base: u64 = 0x400000;
const startup_address: u64 = 0x801000;
/// An `ANPG` descriptor, twice the native application's 128.
const startup_length: u64 = 256;
/// Independent five-page bound for the measured 18,874-byte loader.
/// Existing native application and program limits remain separate.
const code_page_bytes: usize = 20480;
const startup_entry: [u8; 10] = [0xfc, 0x48, 0xc7, 0xc0, 0, 0, 0, 0, 0xcd, 0xf4];
const transport_entry: [u8; 29] = [
    0x53, 0x48, 0x89, 0xfb, 0x48, 0x8b, 0x03, 0x48, 0x8b, 0x7b, 0x08, 0x48, 0x8b, 0x73, 0x10, 0x48,
    0x8b, 0x53, 0x18, 0x48, 0x8b, 0x4b, 0x20, 0x4c, 0x8b, 0x43, 0x28, 0xcd, 0xf4,
];
const transport_return: [u8; 14] = [
    0x48, 0x89, 0x43, 0x30, 0x48, 0x89, 0x4b, 0x38, 0x48, 0x89, 0x53, 0x40, 0x5b, 0xc3,
];

/// Require one exact symbol; a duplicate or differently typed alias cannot
/// silently win merely by appearing earlier in a tool's symbol listing.
fn exact_symbol(symbols: &str, name: &str, kind: char) -> Result<u64, String> {
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
            "program symbol {name} missing, duplicated or mistyped"
        ));
    }
    u64::from_str_radix(matches[0][2], 16)
        .map_err(|_| format!("program symbol {name} address invalid"))
}

fn expect_symbol(symbols: &str, name: &str, kind: char, expected: u64) -> Result<(), String> {
    if exact_symbol(symbols, name, kind)? != expected {
        return Err(format!("program symbol {name} address changed"));
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
        .ok_or("program instruction extent overflow")?;
    let offset = address
        .checked_sub(image_base)
        .ok_or("program instruction below text")?;
    if end > text_end {
        return Err("program instruction escaped initialized text".into());
    }
    let offset = usize::try_from(offset).map_err(|_| "program instruction offset overflow")?;
    image
        .get(
            offset
                ..offset
                    .checked_add(length)
                    .ok_or("program instruction index overflow")?,
        )
        .ok_or_else(|| "program instruction escaped payload".into())
}

/// The general vessel inspector admits the ELF and the complete CPU
/// instruction profile first. These additional checks bind the specific
/// startup gate, the transport stub and the descriptor region to the symbols
/// the kernel profile relies on.
fn inspect_client_contract(
    symbols: &str,
    image: &[u8],
    text_address: u64,
    text_size: u64,
    rw: &[u8],
    rw_memory_size: u64,
) -> Result<u64, String> {
    let text_end = text_address
        .checked_add(text_size)
        .ok_or("program text overflow")?;
    if text_address != image_base || text_size == 0 || text_size > image.len() as u64 {
        return Err("program initialized text geometry changed".into());
    }
    // This measured allowance belongs only to the named manifest loader.
    // The legacy program/native profiles retain their independent bounds.
    if image.len() > code_page_bytes {
        return Err(format!(
            "program image is {} bytes and exceeds its {code_page_bytes}-byte code profile",
            image.len()
        ));
    }
    let symbol = |suffix: &str, kind| exact_symbol(symbols, &format!("{prefix}_{suffix}"), kind);
    if symbol("entry", 'T')? != image_base
        || symbol("startup_return", 'T')? != image_base + startup_entry.len() as u64
        || instruction_bytes(image, text_end, image_base, startup_entry.len())? != startup_entry
    {
        return Err("program startup register-preserving gate changed".into());
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
        return Err("program startup continuation does not call the exact Rust entry".into());
    }
    instruction_bytes(image, text_end, run, 1)?;
    let call = symbol("call", 'T')?;
    let syscall_return = symbol("syscall_return", 'T')?;
    if call.checked_add(transport_entry.len() as u64) != Some(syscall_return)
        || instruction_bytes(image, text_end, call, transport_entry.len())? != transport_entry
        || instruction_bytes(image, text_end, syscall_return, transport_return.len())?
            != transport_return
    {
        return Err("program native transport register contract changed".into());
    }
    // One fault marker and only one. The program vessel arms no guard or NX
    // probe: an unarmed fault is a failed proof, so there is no instruction
    // here that is expected to fault and none to bind.
    let fault = symbol("fault_instruction", 'T')?;
    if instruction_bytes(image, text_end, fault, 2)? != [0x0f, 0x0b] {
        return Err("program fault marker is not an exact UD2".into());
    }
    for suffix in ["startup", "descriptor_start"] {
        if symbol(suffix, 'D')? != startup_address {
            return Err("program startup address changed".into());
        }
    }
    if symbol("descriptor_end", 'D')? != startup_address + startup_length
        || rw.len() < (4096 + startup_length) as usize
        || rw_memory_size < rw.len() as u64
        || rw_memory_size > 8192
        || rw.iter().any(|byte| *byte != 0)
    {
        return Err("program startup storage or zeroed RW profile changed".into());
    }
    Ok(fault)
}

pub(super) struct artifact {
    pub(super) object: PathBuf,
    image: Vec<u8>,
    fault: u64,
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let output = build.join("native-manifest");
    fs::create_dir_all(&output).map_err(|e| format!("create program artifact directory: {e}"))?;
    let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
    for name in ["qualification.txt", "client.elf", "payload.o", "manifest.o"] {
        remove_optional_file(&output.join(name))?;
    }
    build_vessel(root, "anomalous-native-manifest-loader", None, None)?;
    let path = retain_vessel(
        root,
        &output,
        "anomalous-native-manifest-loader",
        "client.elf",
    )?;
    let client = inspect_native_manifest_vessel(root, &path)?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    let symbols = native_application_artifact::listing(root, &path)?;
    let text = elf
        .section_headers
        .iter()
        .find(|s| s.name == ".text")
        .ok_or("program text missing")?;
    let fault = inspect_client_contract(
        &symbols,
        &client.rx,
        text.address,
        text.size,
        &client.rw,
        client.headers[1].memory_size,
    )?;
    if fault != client.fault {
        return Err("program generic and specific fault markers disagree".into());
    }
    fs::write(output.join("client_rx.bin"), &client.rx).map_err(|e| e.to_string())?;
    let mut assembly = format!(
        ".section .rodata.native_manifest,\"a\",@progbits\n.balign 16\n.global {payload}_start\n.global {payload}_end\n{payload}_start:\n.incbin \"client_rx.bin\"\n{payload}_end:\n"
    );
    assembly.push_str(&format!(
        ".global x86_64_native_manifest_user_fault_instruction\n.set x86_64_native_manifest_user_fault_instruction, 0x{fault:x}\n"
    ));
    assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    fs::write(output.join("payload.S"), assembly).map_err(|e| e.to_string())?;
    run_checked(
        &output,
        "as",
        [
            "--64".into(),
            "--fatal-warnings".into(),
            "-o".into(),
            "manifest.o".into(),
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
            "manifest.o".into(),
        ],
    )?;
    let artifact = artifact {
        object: output.join("payload.o"),
        image: client.rx,
        fault,
    };
    inspect_wrapper(root, &artifact)?;
    let mut receipt = format!(
        "profile=native_manifest_v1\nentry=0x400000\nstartup=0x801000\nstartup_bytes=256\ncode_pages=5\ncode_limit_bytes=20480\narena_bytes=131072\nstack_bytes=32768\nrx_bytes={}\nrw_bytes={}\n",
        artifact.image.len(),
        client.headers[1].memory_size
    );
    for name in ["client.elf", "client_rx.bin", "manifest.o", "payload.o"] {
        let hash = run_output_checked(root, "sha256sum", [output.join(name).into_os_string()])?;
        receipt.push_str(&String::from_utf8(hash).map_err(|_| "program hash is not UTF8")?);
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
        || sections.len() != 1
    {
        return Err(
            "program wrapper must contain exactly one readonly relocation-free payload".into(),
        );
    }
    for section in sections {
        if section.kind != 1 || section.flags != 2 || section.address != 0 {
            return Err("program wrapper payload permissions changed".into());
        }
        if section.name != ".rodata.native_manifest" {
            return Err("program wrapper contains undeclared bytes".into());
        }
        if native_application_artifact::section_bytes(&bytes, section)? != artifact.image {
            return Err("program wrapper changed inspected source bytes".into());
        }
    }
    let notes = elf
        .section_headers
        .iter()
        .filter(|s| s.name == ".note.GNU-stack")
        .collect::<Vec<_>>();
    if notes.len() != 1 || notes[0].kind != 1 || notes[0].flags != 0 || notes[0].size != 0 {
        return Err("program wrapper lost nonexecutable stack declaration".into());
    }
    check_symbols(
        &native_application_artifact::listing(root, &artifact.object)?,
        artifact,
        0,
    )
}

fn check_symbols(symbols: &str, artifact: &artifact, start: u64) -> Result<(), String> {
    inspect_wrapper_symbols(symbols, artifact.image.len(), artifact.fault, start)
}

fn inspect_wrapper_symbols(
    symbols: &str,
    image_length: usize,
    fault: u64,
    start: u64,
) -> Result<(), String> {
    let expected = [
        (format!("{payload}_start"), start, 'R'),
        (
            format!("{payload}_end"),
            start
                .checked_add(image_length as u64)
                .ok_or("program payload overflow")?,
            'R',
        ),
        (
            "x86_64_native_manifest_user_fault_instruction".into(),
            fault,
            'A',
        ),
    ];
    for (name, value, kind) in &expected {
        expect_symbol(symbols, name, *kind, *value)?;
    }
    for name in symbols
        .lines()
        .filter_map(|line| line.split_whitespace().next())
    {
        if name.starts_with("x86_64_native_manifest_")
            && !expected.iter().any(|entry| entry.0 == name)
        {
            return Err("program wrapper exports an undeclared alias".into());
        }
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
    let start = exact_symbol(&symbols, &format!("{payload}_start"), 'R')?;
    check_symbols(&symbols, artifact, start)?;
    let end = start
        .checked_add(artifact.image.len() as u64)
        .ok_or("program linked overflow")?;
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
        || count_subslice(&bytes, &artifact.image) != 1
    {
        return Err("program linked payload changed or lost unique readonly backing".into());
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "native_manifest_test",
        "kernel.native_manifest_test",
        "x86_64_native_manifest",
        "anomalous_native_manifest_loader",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
            return Err("normal image retains program launch proof".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<u8>, String, Vec<u8>) {
        let mut image = vec![0x90; 128];
        // Independent byte witness for the five-input register gate and its
        // three exact output fields. R8 carries the edition's read offset.
        image[..17].copy_from_slice(&[
            0xfc, 0x48, 0xc7, 0xc0, 0, 0, 0, 0, 0xcd, 0xf4, 0xe8, 81, 0, 0, 0, 0x0f, 0x0b,
        ]);
        image[32..75].copy_from_slice(&[
            0x53, 0x48, 0x89, 0xfb, 0x48, 0x8b, 0x03, 0x48, 0x8b, 0x7b, 0x08, 0x48, 0x8b, 0x73,
            0x10, 0x48, 0x8b, 0x53, 0x18, 0x48, 0x8b, 0x4b, 0x20, 0x4c, 0x8b, 0x43, 0x28, 0xcd,
            0xf4, 0x48, 0x89, 0x43, 0x30, 0x48, 0x89, 0x4b, 0x38, 0x48, 0x89, 0x53, 0x40, 0x5b,
            0xc3,
        ]);
        image[80..82].copy_from_slice(&[0x0f, 0x0b]);
        image[96] = 0xc3;
        let symbols = [
            ("entry", 0x400000, 'T'),
            ("startup_return", 0x40000a, 'T'),
            ("run", 0x400060, 'T'),
            ("call", 0x400020, 'T'),
            ("syscall_return", 0x40003d, 'T'),
            ("fault_instruction", 0x400050, 'T'),
            ("startup", 0x801000, 'D'),
            ("descriptor_start", 0x801000, 'D'),
            ("descriptor_end", 0x801100, 'D'),
        ]
        .into_iter()
        .map(|(suffix, address, kind)| format!("{prefix}_{suffix} {kind} {address:x}\n"))
        .collect();
        (image, symbols, vec![0; 4352])
    }

    #[test]
    fn edition_offset_and_exact_receipt_stores_are_required() {
        let (image, symbols, rw) = fixture();
        assert_eq!(
            inspect_client_contract(&symbols, &image, 0x400000, 128, &rw, 4352),
            Ok(0x400050)
        );
        for offset in 32..75 {
            let mut changed = image.clone();
            changed[offset] ^= 1;
            assert!(
                inspect_client_contract(&symbols, &changed, 0x400000, 128, &rw, 4352).is_err(),
                "accepted changed transport byte {offset}"
            );
        }
        let old_return = symbols.replace("syscall_return T 40003d", "syscall_return T 400039");
        assert!(inspect_client_contract(&old_return, &image, 0x400000, 128, &rw, 4352).is_err());
    }

    #[test]
    fn manifest_transport_keeps_an_explicit_bound_and_original_descriptor() {
        let (mut image, symbols, rw) = fixture();
        image.resize(20480, 0x90);
        assert!(inspect_client_contract(&symbols, &image, 0x400000, 128, &rw, 4352).is_ok());
        image.push(0);
        assert!(inspect_client_contract(&symbols, &image, 0x400000, 128, &rw, 4352).is_err());
        image.truncate(128);
        let widened = symbols.replace("descriptor_end D 801100", "descriptor_end D 801200");
        assert!(inspect_client_contract(&widened, &image, 0x400000, 128, &rw, 4352).is_err());
    }

    #[test]
    fn manifest_startup_and_continuation_require_exact_bytes_and_symbols() {
        let (image, symbols, rw) = fixture();
        for offset in 0..17 {
            let mut changed = image.clone();
            changed[offset] ^= 1;
            assert!(inspect_client_contract(&symbols, &changed, 0x400000, 128, &rw, 4352).is_err());
        }
        for (old, new) in [
            ("entry T 400000", "entry D 400000"),
            ("run T 400060", "run T 400061"),
            ("startup D 801000", "startup D 801001"),
            ("fault_instruction T 400050", "fault_instruction T 400051"),
        ] {
            assert!(
                inspect_client_contract(
                    &symbols.replace(old, new),
                    &image,
                    0x400000,
                    128,
                    &rw,
                    4352
                )
                .is_err()
            );
        }
        let duplicate = symbols.clone() + "anomalous_native_manifest_loader_entry T 400000\n";
        assert!(inspect_client_contract(&duplicate, &image, 0x400000, 128, &rw, 4352).is_err());
        assert!(inspect_client_contract(&symbols, &image, 0x400001, 128, &rw, 4352).is_err());
        assert!(inspect_client_contract(&symbols, &image, 0x400000, 96, &rw, 4352).is_err());
    }

    #[test]
    fn manifest_descriptor_requires_complete_zeroed_private_storage() {
        let (image, symbols, rw) = fixture();
        for size in [0, 4096, 4351] {
            assert!(
                inspect_client_contract(&symbols, &image, 0x400000, 128, &rw[..size], 4352)
                    .is_err()
            );
        }
        for memory in [4351, 8193] {
            assert!(inspect_client_contract(&symbols, &image, 0x400000, 128, &rw, memory).is_err());
        }
        for offset in [0, 4096, 4351] {
            let mut changed = rw.clone();
            changed[offset] = 1;
            assert!(
                inspect_client_contract(&symbols, &image, 0x400000, 128, &changed, 4352).is_err()
            );
        }
    }

    #[test]
    fn manifest_wrapper_symbols_reject_aliases_duplicates_and_wrong_kinds() {
        let listing = "x86_64_native_manifest_payload_start R 0\nx86_64_native_manifest_payload_end R 80\nx86_64_native_manifest_user_fault_instruction A 400050\n";
        assert!(inspect_wrapper_symbols(listing, 128, 0x400050, 0).is_ok());
        for changed in [
            listing.replace("payload_start R", "payload_start D"),
            listing.replace("payload_end R 80", "payload_end R 81"),
            listing.replace("fault_instruction A 400050", "fault_instruction A 400051"),
            String::from(listing) + "x86_64_native_manifest_payload_start R 0\n",
            String::from(listing) + "x86_64_native_manifest_unreviewed R 40\n",
        ] {
            assert!(inspect_wrapper_symbols(&changed, 128, 0x400050, 0).is_err());
        }
        assert!(inspect_wrapper_symbols(listing, 128, 0x400050, u64::MAX).is_err());
    }
}
