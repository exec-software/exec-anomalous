//! Host packaging of a program-bearing client alongside the native baseline.
//! Host ELF is a build input; no foreign execution personality is introduced.
//!
//! Only the program payload, deliberately, unlike the native runtime artifact
//! which carries the native application baseline beside its own. A program
//! build leaves `native_application_runtime`'s two-task half unreachable, and
//! `allow(dead_code)` is a lint rather than a `cfg`, so the reasonable worry
//! is that those still-compiled functions take the address of
//! `x86_64_native_application_payload_start` and need the baseline to resolve
//! it. They do not survive to ask: the release build drops them and the
//! kernel link runs with `--gc-sections`. Carrying the baseline anyway is
//! worse than useless -- its section is then collected and its own linked
//! inspection fails on the symbol it just lost. If a future change makes that
//! half reachable again, the link fails with an undefined symbol, which is
//! the right way to find out.
use super::*;
use elf_application_artifact::{build_vessel, inspect_native_program_vessel, retain_vessel};

const prefix: &str = "anomalous_native_program_client";
const payload: &str = "x86_64_native_program_payload";
const image_base: u64 = 0x400000;
const startup_address: u64 = 0x801000;
/// An `ANPG` descriptor, twice the native application's 128.
const startup_length: u64 = 256;
/// One RX page. The program profile maps exactly one, so an image that
/// outgrew it must be refused here as well as by the vessel's own linker
/// assertion: this is the side that knows what the kernel will map.
const code_page_bytes: usize = 4096;
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
    // The single mapped code page is a kernel-side fact, so it is checked on
    // the host where the failure can name it, not discovered at boot by a
    // geometry refusal that cannot say which side was wrong.
    if image.len() > code_page_bytes {
        return Err(format!(
            "program image is {} bytes and exceeds its single {code_page_bytes}-byte code page",
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
    let output = build.join("native-program");
    fs::create_dir_all(&output).map_err(|e| format!("create program artifact directory: {e}"))?;
    let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
    for name in ["qualification.txt", "client.elf", "payload.o", "program.o"] {
        remove_optional_file(&output.join(name))?;
    }
    build_vessel(root, "anomalous-native-program-client", None, None)?;
    let path = retain_vessel(
        root,
        &output,
        "anomalous-native-program-client",
        "client.elf",
    )?;
    let client = inspect_native_program_vessel(root, &path)?;
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
        ".section .rodata.native_program,\"a\",@progbits\n.balign 16\n.global {payload}_start\n.global {payload}_end\n{payload}_start:\n.incbin \"client_rx.bin\"\n{payload}_end:\n"
    );
    assembly.push_str(&format!(
        ".global x86_64_native_program_user_fault_instruction\n.set x86_64_native_program_user_fault_instruction, 0x{fault:x}\n"
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
            "program.o".into(),
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
            "program.o".into(),
        ],
    )?;
    let artifact = artifact {
        object: output.join("payload.o"),
        image: client.rx,
        fault,
    };
    inspect_wrapper(root, &artifact)?;
    let mut receipt = format!(
        "profile=program_launch_v1\nentry=0x400000\nstartup=0x801000\nstartup_bytes=256\narena_bytes=65536\ncode_pages=1\nrx_bytes={}\nrw_bytes={}\n",
        artifact.image.len(),
        client.headers[1].memory_size
    );
    for name in ["client.elf", "client_rx.bin", "program.o", "payload.o"] {
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
        if section.name != ".rodata.native_program" {
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
            "x86_64_native_program_user_fault_instruction".into(),
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
        if name.starts_with("x86_64_native_program_")
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
        "native_program_profile",
        "kernel.native_program_test",
        "x86_64_native_program",
        "anomalous_native_program_client",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
            return Err("normal image retains program launch proof".into());
        }
    }
    Ok(())
}
