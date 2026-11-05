//! Pinned, build-time qualification of one Ny scalar object and startup shim.
//! This host inspector is not an executable loader or a general Ny runtime.
use super::*;

const generation: &str = "c541621251d8502d7d7c547d24f10322bc894e5943dc21a1abaaed59c71fd385";
const pinned_tools: [(&str, &str); 3] = [
    (
        "nycompiler",
        "1550ac380dbb50a4c7b36e9f69ca9f63bf71cb888bbe7b7da59b2d18174c3f3f",
    ),
    (
        "nyassembler",
        "3be1dfc1d6929fb4924fc86b31062c91908c4533f743be5d0c253291baa3378a",
    ),
    (
        "nyinterpreter",
        "031d1ceb15098959cd1c8b1ec499f0413454f200751e08b871901e2e7140dc84",
    ),
];
const image_base: u64 = 0x400000;
const data_base: u64 = 0x800000;
const bss_base: u64 = 0x800100;
const mailbox_magic: u64 = 0x4e59_5f53_5441_5254;
const abi_message: &[u8] = b"Ny native runtime error: incompatible nyruntime ABI (expected 1)\n\0";
const context_message: &[u8] = b"Ny Anomalous probe: injected context_end failure\n\0";
const imports: [&str; 7] = [
    "calloc",
    "memcpy",
    "nyrt_abi_version",
    "nyrt_context_begin",
    "nyrt_context_end",
    "nyrt_fail",
    "strlen",
];
const user_aliases: [(&str, &str); 9] = [
    ("compiled_start", "compiledcode_start"),
    ("compiled_end", "compiledcode_end"),
    ("fail_ud2", "fail_ud2"),
    ("start_return", "startgate_return"),
    ("exit_return", "exit_return"),
    ("bss_start", "bss_start"),
    ("bss_end", "bss_end"),
    ("context_message_start", "injected_message"),
    ("context_message_end", "injected_message_end"),
];

pub(super) struct artifact {
    pub(super) object: PathBuf,
    image: Vec<u8>,
    data: Vec<u8>,
    aliases: BTreeMap<String, u64>,
}

struct object_inspection {
    text: Vec<u8>,
    relocated: Vec<usize>,
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let output = build.join("ny-object");
    fs::create_dir_all(&output).map_err(|e| format!("create Ny artifact directory: {e}"))?;
    remove_optional_file(&output.join("qualification.txt"))?;
    let source = root.join("vessels/anomalous-ny-probe");
    let inputs = ["probe.ny", "entry.S", "probe.ld"];
    let before = inputs
        .iter()
        .map(|name| sha256(root, &source.join(name)))
        .collect::<Result<Vec<_>, _>>()?;
    let sdk = checked_sdk(root)?;
    let nil = output.join("probe.nil");
    let ny_object = output.join("probe.o");
    let entry_object = output.join("entry.o");
    let elf = output.join("probe.elf");
    let wrapper = output.join("payload.o");
    for path in [&nil, &ny_object, &entry_object, &elf, &wrapper] {
        remove_optional_file(path)?;
    }
    run_stage(
        root,
        &sdk.join("nycompiler"),
        &[
            "--no-bundle".into(),
            "--no-stdlib".into(),
            "--output".into(),
            nil.as_os_str().into(),
            source.join("probe.ny").into_os_string(),
        ],
        &output,
        "compiler",
        0,
        60,
    )?;
    run_stage(
        root,
        &sdk.join("nyassembler"),
        &[
            "--unmanaged".into(),
            "--output".into(),
            ny_object.as_os_str().into(),
            nil.as_os_str().into(),
        ],
        &output,
        "assembler",
        0,
        60,
    )?;
    let raw = fs::read(&ny_object).map_err(|e| format!("read Ny object: {e}"))?;
    let object_info = inspect_object(&raw)?;
    // The VM returns the Ny i32 through main; the host OS exposes its low byte.
    // This is an independent source/NIL semantic oracle, not native execution.
    run_stage(
        root,
        &sdk.join("nyinterpreter"),
        &["--no-ai".into(), nil.as_os_str().into()],
        &output,
        "interpreter",
        128,
        300,
    )?;
    if !fs::read(output.join("interpreter.stdout"))
        .map_err(|e| e.to_string())?
        .is_empty()
        || !fs::read(output.join("interpreter.stderr"))
            .map_err(|e| e.to_string())?
            .is_empty()
    {
        return Err("Ny scalar interpreter oracle emitted unexpected output".into());
    }
    run_stage(
        root,
        Path::new("as"),
        &[
            "--64".into(),
            "--fatal-warnings".into(),
            "-o".into(),
            entry_object.as_os_str().into(),
            source.join("entry.S").into_os_string(),
        ],
        &output,
        "startup",
        0,
        60,
    )?;
    run_stage(
        root,
        Path::new("ld"),
        &[
            "--build-id=none".into(),
            "--fatal-warnings".into(),
            "--no-undefined".into(),
            "--orphan-handling=error".into(),
            "-nostdlib".into(),
            "-static".into(),
            "-z".into(),
            "max-page-size=0x1000".into(),
            "-T".into(),
            source.join("probe.ld").into_os_string(),
            "-o".into(),
            elf.as_os_str().into(),
            entry_object.as_os_str().into(),
            ny_object.as_os_str().into(),
        ],
        &output,
        "link",
        0,
        60,
    )?;
    let bytes = fs::read(&elf).map_err(|e| format!("read linked Ny image: {e}"))?;
    let parsed = parse_elf64(&bytes)?;
    let (image, data) = inspect_segments(&bytes, &parsed)?;
    let symbols = listing(root, &elf)?;
    let sym =
        |suffix: &str| symbol_u64_from_listing(&symbols, &format!("anomalous_ny_probe_{suffix}"));
    if sym("entry")? != image_base
        || sym("image_start")? != image_base
        || sym("image_end")? != image_base + image.len() as u64
        || sym("data_start")? != data_base
        || sym("data_end")? != data_base + 128
        || sym("bss_start")? != bss_base
        || sym("bss_end")? != data_base + parsed.program_headers[1].memory_size
    {
        return Err("Ny linked markers disagree with inspected load extents".into());
    }
    let compiled_start = sym("compiledcode_start")?;
    let compiled_end = sym("compiledcode_end")?;
    let compiled = elf_virtual_bytes(
        &bytes,
        parsed.program_headers[0],
        compiled_start,
        object_info.text.len(),
    )?;
    if compiled_end.checked_sub(compiled_start) != Some(object_info.text.len() as u64)
        || compiled
            .iter()
            .zip(&object_info.text)
            .enumerate()
            .any(|(offset, (actual, original))| {
                actual != original
                    && !object_info
                        .relocated
                        .iter()
                        .any(|relocation| (*relocation..*relocation + 4).contains(&offset))
            })
    {
        return Err("Ny compiler text was changed beyond declared link relocations".into());
    }
    let mut aliases = BTreeMap::new();
    for (alias, marker) in user_aliases {
        aliases.insert(format!("x86_64_ny_object_user_{alias}"), sym(marker)?);
    }
    let abi_offset = unique_offset(&image, abi_message)?;
    aliases.insert(
        "x86_64_ny_object_user_abi_message_start".into(),
        image_base + abi_offset as u64,
    );
    aliases.insert(
        "x86_64_ny_object_user_abi_message_length".into(),
        abi_message.len() as u64,
    );
    let message_start = sym("injected_message")?;
    if sym("injected_message_end")?.checked_sub(message_start) != Some(context_message.len() as u64)
        || elf_virtual_bytes(
            &bytes,
            parsed.program_headers[0],
            message_start,
            context_message.len(),
        )? != context_message
        || unique_offset(&image, context_message)? as u64 + image_base != message_start
    {
        return Err("Ny context failure message differs from the fixed fixture".into());
    }
    aliases.insert(
        "x86_64_ny_object_user_context_message_length".into(),
        context_message.len() as u64,
    );
    let gates = ["start_gate", "fail_gate", "exit_gate", "unsupported_gate"]
        .map(sym)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let traps = ["fail_ud2", "exit_return", "unsupported_return"]
        .map(sym)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    for (gate, after) in [
        ("start_gate", "startgate_return"),
        ("fail_gate", "fail_ud2"),
        ("exit_gate", "exit_return"),
        ("unsupported_gate", "unsupported_return"),
    ] {
        if sym(gate)?.checked_add(2) != Some(sym(after)?) {
            return Err("Ny gate return marker moved".into());
        }
    }
    let disassembly = run_output_checked(
        root,
        "objdump",
        [
            OsString::from("--disassemble"),
            OsString::from("--wide"),
            OsString::from("--insn-width=16"),
            elf.as_os_str().into(),
        ],
    )?;
    fs::write(output.join("probe.disassembly"), &disassembly).map_err(|e| e.to_string())?;
    let disassembly = String::from_utf8(disassembly).map_err(|_| "Ny disassembly is not UTF-8")?;
    let text = parsed
        .section_headers
        .iter()
        .find(|s| s.name == ".text")
        .ok_or("Ny .text missing")?;
    inspect_instructions(
        &disassembly,
        &image,
        text.size,
        &gates,
        &traps,
        symbol_u64_from_listing(&symbols, "nyrt_context_end")?,
    )?;
    fs::write(output.join("probe_rx.bin"), &image).map_err(|e| e.to_string())?;
    fs::write(output.join("probe_data.bin"), &data).map_err(|e| e.to_string())?;
    let mut assembly = String::from(
        ".section .rodata.ny_object,\"a\",@progbits\n.balign 16\n.global x86_64_ny_object_payload_start\n.global x86_64_ny_object_payload_end\n.global x86_64_ny_object_data_start\n.global x86_64_ny_object_data_end\nx86_64_ny_object_payload_start:\n.incbin \"probe_rx.bin\"\nx86_64_ny_object_payload_end:\n.balign 16\nx86_64_ny_object_data_start:\n.incbin \"probe_data.bin\"\nx86_64_ny_object_data_end:\n",
    );
    for (name, value) in &aliases {
        assembly.push_str(&format!(".global {name}\n.set {name}, 0x{value:x}\n"));
    }
    assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    fs::write(output.join("payload.S"), assembly).map_err(|e| e.to_string())?;
    run_stage(
        &output,
        Path::new("as"),
        &[
            "--64".into(),
            "--fatal-warnings".into(),
            "-o".into(),
            "payload.o".into(),
            "payload.S".into(),
        ],
        &output,
        "wrapper",
        0,
        60,
    )?;
    let artifact = artifact {
        object: wrapper,
        image,
        data,
        aliases,
    };
    inspect_wrapper(root, &artifact)?;
    if checked_sdk(root)? != sdk
        || inputs
            .iter()
            .map(|name| sha256(root, &source.join(name)))
            .collect::<Result<Vec<_>, _>>()?
            != before
    {
        return Err("Ny pinned SDK or fixture inputs changed during qualification".into());
    }
    let mut record = format!(
        "profile=ny_scalar_startup_v1\nsdk_generation={generation}\nsource_sha256={}\nentry_sha256={}\nlinker_sha256={}\ninterpreter_exit=128\nresult_i32=50000000\n",
        before[0], before[1], before[2]
    );
    for (name, digest) in pinned_tools {
        record.push_str(&format!("{name}_sha256={digest}\n"));
    }
    for name in [
        "probe.nil",
        "probe.o",
        "entry.o",
        "probe.elf",
        "probe_rx.bin",
        "probe_data.bin",
        "payload.o",
    ] {
        record.push_str(&format!(
            "{name}_sha256={}\n",
            sha256(root, &output.join(name))?
        ));
    }
    fs::write(output.join("qualification.txt"), record).map_err(|e| e.to_string())?;
    Ok(artifact)
}

fn checked_sdk(root: &Path) -> Result<PathBuf, String> {
    let sdk = fs::canonicalize(root.join("runtime")).map_err(|_| {
        "Ny runtime missing; refresh it with scripts/refresh-ny-runtime.sh before this opt-in proof"
    })?;
    if sha256(root, &sdk.join("manifest.json"))? != generation {
        return Err("Ny runtime generation/manifest differs from the qualified pin".into());
    }
    for (name, expected) in pinned_tools {
        let path = sdk.join(name);
        if !fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_file()
            || sha256(root, &path)? != expected
        {
            return Err(format!("Ny pinned tool changed: {name}"));
        }
    }
    Ok(sdk)
}

fn sha256(root: &Path, path: &Path) -> Result<String, String> {
    let output = run_output_checked(root, "sha256sum", [path.as_os_str().to_owned()])?;
    let text = String::from_utf8(output).map_err(|_| "sha256sum output is not UTF-8")?;
    let value = text
        .split_whitespace()
        .next()
        .ok_or("sha256sum omitted hash")?;
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("sha256sum hash malformed".into());
    }
    Ok(value.into())
}

fn run_stage(
    root: &Path,
    program: &Path,
    arguments: &[OsString],
    directory: &Path,
    label: &str,
    expected: i32,
    seconds: u64,
) -> Result<(), String> {
    let stdout =
        File::create(directory.join(format!("{label}.stdout"))).map_err(|e| e.to_string())?;
    let stderr =
        File::create(directory.join(format!("{label}.stderr"))).map_err(|e| e.to_string())?;
    let mut child = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .env_remove("LD_LIBRARY_PATH")
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|e| format!("start Ny {label}: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.code() == Some(expected) {
                Ok(())
            } else {
                Err(format!(
                    "Ny {label} exited {status}; inspect retained stage logs"
                ))
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "Ny {label} exceeded {seconds}s; retained stage logs"
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[derive(Clone)]
struct raw_section {
    header: elf64_section_header,
    link: usize,
    info: usize,
    alignment: u64,
    entry_size: u64,
}
#[derive(Clone)]
struct raw_symbol {
    name: String,
    info: u8,
    section: usize,
    value: u64,
    size: u64,
}

fn section_bytes<'a>(bytes: &'a [u8], section: &elf64_section_header) -> Result<&'a [u8], String> {
    let start = usize::try_from(section.offset).map_err(|_| "Ny section offset overflow")?;
    let end = start
        .checked_add(usize::try_from(section.size).map_err(|_| "Ny section size overflow")?)
        .ok_or("Ny section extent overflow")?;
    bytes
        .get(start..end)
        .ok_or_else(|| "Ny section truncated".into())
}

fn inspect_object(bytes: &[u8]) -> Result<object_inspection, String> {
    if bytes.len() > 64 * 1024 {
        return Err("Ny compiler object exceeds 64 KiB".into());
    }
    let elf = parse_elf64(bytes)?;
    if elf.kind != 1
        || elf.machine != 62
        || elf.entry != 0
        || !elf.program_headers.is_empty()
        || elf_u64(bytes, 32)? != 0
        || bytes[7..16].iter().any(|b| *b != 0)
        || elf.section_headers.len() != 11
    {
        return Err("Ny compiler object has unexpected ELF profile".into());
    }
    let table = elf_u64(bytes, 40)?;
    let mut sections = Vec::new();
    for (index, header) in elf.section_headers.iter().enumerate() {
        let at = table_entry_offset(table, index, 64, bytes.len())?;
        sections.push(raw_section {
            header: header.clone(),
            link: elf_u32(bytes, at + 40)? as usize,
            info: elf_u32(bytes, at + 44)? as usize,
            alignment: elf_u64(bytes, at + 48)?,
            entry_size: elf_u64(bytes, at + 56)?,
        });
    }
    let expected = [
        ("", 0, 0, 0, 0),
        (".strtab", 3, 0, 1, 0),
        (".text", 1, 6, 16, 0),
        (".rela.text", 4, 0x40, 8, 24),
        (".rodata.cst8", 1, 0x12, 8, 8),
        (".rodata.str1.1", 1, 0x32, 1, 1),
        (".bss", 8, 3, 8, 0),
        (".note.GNU-stack", 1, 0, 1, 0),
        (".eh_frame", 0x7000_0001, 2, 8, 0),
        (".rela.eh_frame", 4, 0x40, 8, 24),
        (".symtab", 2, 0, 8, 24),
    ];
    for (index, (section, (name, kind, flags, alignment, entry_size))) in
        sections.iter().zip(expected).enumerate()
    {
        let s = &section.header;
        if s.name != name
            || s.kind != kind
            || s.flags != flags
            || s.address != 0
            || section.alignment != alignment
            || section.entry_size != entry_size
            || (alignment > 0 && !s.offset.is_multiple_of(alignment))
            || (!matches!(index, 0 | 7) && s.size == 0)
            || (matches!(index, 0 | 7) && s.size != 0)
            || (entry_size != 0 && !s.size.is_multiple_of(entry_size))
            || (!matches!(index, 3 | 9 | 10) && (section.link != 0 || section.info != 0))
        {
            return Err(format!(
                "Ny compiler section {index} violates the pinned profile"
            ));
        }
    }
    let mut file_regions = Vec::new();
    for section in &sections {
        let s = &section.header;
        if s.kind == 8 || s.size == 0 {
            continue;
        }
        let end = s
            .offset
            .checked_add(s.size)
            .ok_or("Ny object file extent overflow")?;
        if s.offset < 64
            || end > table
            || file_regions
                .iter()
                .any(|(start, previous_end)| s.offset < *previous_end && *start < end)
        {
            return Err("Ny object sections overlap another file region".into());
        }
        file_regions.push((s.offset, end));
    }
    if elf_u16(bytes, 62)? != 1
        || sections[2].header.size > 2048
        || sections[6].header.size > 256
        || sections[10].link != 1
        || sections[10].info == 0
    {
        return Err("Ny compiler section/symbol bounds changed".into());
    }
    let strings = section_bytes(bytes, &sections[1].header)?;
    let symbols_bytes = section_bytes(bytes, &sections[10].header)?;
    if symbols_bytes.len() > 64 * 24 {
        return Err("Ny compiler symbol count exceeds bound".into());
    }
    let mut symbols = Vec::new();
    let mut undefined = Vec::new();
    let mut globals = Vec::new();
    for (index, entry) in symbols_bytes.chunks_exact(24).enumerate() {
        let symbol = raw_symbol {
            name: elf_string(strings, elf_u32(entry, 0)? as usize)?,
            info: entry[4],
            section: elf_u16(entry, 6)? as usize,
            value: elf_u64(entry, 8)?,
            size: elf_u64(entry, 16)?,
        };
        let binding = symbol.info >> 4;
        let kind = symbol.info & 15;
        if entry[5] != 0 || binding > 1 || (index < sections[10].info) != (binding == 0) {
            return Err("Ny compiler symbol binding/visibility changed".into());
        }
        if index == 0 {
            if entry.iter().any(|b| *b != 0) {
                return Err("Ny null symbol is not zero".into());
            }
        } else if symbol.section == 0 {
            if symbol.info != 0x10
                || symbol.value != 0
                || symbol.size != 0
                || !imports.contains(&symbol.name.as_str())
            {
                return Err(
                    "Ny compiler has an unsupported import (including weak symbols)".into(),
                );
            }
            undefined.push(symbol.name.clone());
        } else if symbol.section == 0xfff1 {
            if symbol.info != 4
                || symbol.name != "ny_native"
                || symbol.value != 0
                || symbol.size != 0
            {
                return Err("Ny compiler absolute symbol is unsupported".into());
            }
        } else {
            let section = sections
                .get(symbol.section)
                .ok_or("Ny symbol section out of range")?;
            if !matches!(symbol.section, 2 | 4 | 5 | 6)
                || symbol
                    .value
                    .checked_add(symbol.size)
                    .is_none_or(|end| end > section.header.size)
                || !matches!(kind, 0..=3)
                || (kind == 2 && (symbol.section != 2 || symbol.size == 0))
                || (kind == 1 && symbol.section != 6)
            {
                return Err("Ny compiler symbol extent/type is unsupported".into());
            }
            if binding == 1 {
                globals.push(symbol.name.clone());
                if symbol.name != "main" || symbol.info != 0x12 {
                    return Err("Ny compiler export changed".into());
                }
            }
        }
        symbols.push(symbol);
    }
    undefined.sort();
    globals.sort();
    if undefined != imports || globals != ["main"] || sections[10].info >= symbols.len() {
        return Err("Ny compiler import/export set is not exact".into());
    }
    let mut relocated = Vec::new();
    let mut referenced_imports = Vec::new();
    for (index, target) in [(3, 2), (9, 8)] {
        let section = &sections[index];
        if section.link != 10 || section.info != target {
            return Err("Ny relocation links changed".into());
        }
        for entry in section_bytes(bytes, &section.header)?.chunks_exact(24) {
            let offset = elf_u64(entry, 0)?;
            let info = elf_u64(entry, 8)?;
            let symbol = symbols
                .get((info >> 32) as usize)
                .ok_or("Ny relocation symbol out of range")?;
            let kind = info as u32;
            let addend = elf_u64(entry, 16)? as i64;
            if offset
                .checked_add(4)
                .is_none_or(|end| end > sections[target].header.size)
            {
                return Err("Ny relocation write exceeds target section".into());
            }
            let valid = if target == 8 {
                kind == 2
                    && symbol.section == 2
                    && addend >= 0
                    && (addend as u64) < sections[2].header.size
            } else {
                match kind {
                    4 => {
                        referenced_imports.push(symbol.name.clone());
                        symbol.section == 0
                            && imports.contains(&symbol.name.as_str())
                            && addend == -4
                    }
                    2 => {
                        matches!(symbol.section, 2 | 4 | 5 | 6)
                            && addend >= -8
                            && i128::from(symbol.value) + i128::from(addend)
                                < i128::from(sections[symbol.section].header.size)
                    }
                    10 => {
                        symbol.section == 5
                            && addend >= 0
                            && symbol
                                .value
                                .checked_add(addend as u64)
                                .is_some_and(|value| value < sections[5].header.size)
                    }
                    _ => false,
                }
            };
            if !valid {
                return Err(
                    "Ny relocation requires an unsupported runtime/linker operation".into(),
                );
            }
            if target == 2 {
                let offset = offset as usize;
                if relocated.iter().any(|old: &usize| old.abs_diff(offset) < 4) {
                    return Err("Ny relocation writes overlap".into());
                }
                relocated.push(offset);
            }
        }
    }
    referenced_imports.sort();
    referenced_imports.dedup();
    if referenced_imports != imports {
        return Err("Ny import relocations do not cover the exact shim contract".into());
    }
    Ok(object_inspection {
        text: section_bytes(bytes, &sections[2].header)?.to_vec(),
        relocated,
    })
}

fn inspect_segments(bytes: &[u8], elf: &parsed_elf64) -> Result<(Vec<u8>, Vec<u8>), String> {
    if elf.kind != 2
        || elf.machine != 62
        || elf.entry != image_base
        || elf.program_headers.len() != 2
    {
        return Err("Ny image requires exact static x86-64 RX/RW loads".into());
    }
    let rx = elf.program_headers[0];
    let rw = elf.program_headers[1];
    for (load, address, flags, limit) in [(rx, image_base, 5, 4096), (rw, data_base, 6, 8192)] {
        if load.kind != 1
            || load.flags != flags
            || load.virtual_address != address
            || load.physical_address != address
            || load.alignment != 4096
            || load.memory_size > limit
            || load.file_size > load.memory_size
            || !load.offset.is_multiple_of(4096)
            || load
                .offset
                .checked_add(load.file_size)
                .is_none_or(|end| end > bytes.len() as u64)
        {
            return Err("Ny load permission/geometry/backing mismatch".into());
        }
    }
    if rx.file_size == 0
        || rx.file_size != rx.memory_size
        || rx.offset + rx.file_size > rw.offset
        || rw.file_size != 128
        || !(257..=512).contains(&rw.memory_size)
    {
        return Err("Ny RX/data/BSS dimensions are outside the fixture".into());
    }
    let mut allocated = Vec::new();
    for section in &elf.section_headers {
        if matches!(section.kind, 4 | 6 | 9 | 11)
            || section.flags & 0x400 != 0
            || [
                ".got", ".plt", ".interp", ".init", ".fini", ".preinit", ".ctors", ".dtors",
            ]
            .iter()
            .any(|prefix| section.name.starts_with(prefix))
        {
            return Err(
                "Ny final image contains a relocation/dynamic/TLS/constructor surface".into(),
            );
        }
        if section.flags & 2 == 0 || section.size == 0 {
            continue;
        }
        let (load, kind, flags) = match section.name.as_str() {
            ".text" => (rx, 1, 6),
            ".rodata" => (rx, 1, 2),
            ".data" => (rw, 1, 3),
            ".bss" => (rw, 8, 3),
            _ => return Err("Ny final image has an undeclared allocated section".into()),
        };
        let end = section
            .address
            .checked_add(section.size)
            .ok_or("Ny allocated section overflow")?;
        if section.kind != kind
            || section.flags != flags
            || section.address < load.virtual_address
            || end > load.virtual_address + load.memory_size
            || (kind != 8
                && (end > load.virtual_address + load.file_size
                    || section.offset != load.offset + section.address - load.virtual_address))
            || (section.name == ".text" && section.address != image_base)
            || (section.name == ".data" && (section.address != data_base || section.size != 128))
            || (section.name == ".bss"
                && (section.address != bss_base || end != data_base + rw.memory_size))
        {
            return Err("Ny allocated section does not match its protection domain".into());
        }
        if allocated
            .iter()
            .any(|(start, previous_end, _)| section.address < *previous_end && *start < end)
        {
            return Err("Ny allocated sections overlap".into());
        }
        allocated.push((section.address, end, section.name.as_str()));
    }
    let mut names = allocated
        .iter()
        .map(|(_, _, name)| *name)
        .collect::<Vec<_>>();
    names.sort();
    if names != [".bss", ".data", ".rodata", ".text"] {
        return Err("Ny required allocated section set is incomplete".into());
    }
    let image = elf_virtual_bytes(bytes, rx, image_base, rx.file_size as usize)?.to_vec();
    let data = elf_virtual_bytes(bytes, rw, data_base, 128)?.to_vec();
    let mut expected_data = [0; 128];
    expected_data[80..88].copy_from_slice(&mailbox_magic.to_le_bytes());
    if data != expected_data {
        return Err("Ny initialized data differs from the exact startup header".into());
    }
    Ok((image, data))
}

fn listing(root: &Path, elf: &Path) -> Result<String, String> {
    let undefined = run_output_checked(
        root,
        "nm",
        [
            "--undefined-only".into(),
            "--format=posix".into(),
            elf.as_os_str().to_owned(),
        ],
    )?;
    if !undefined.iter().all(u8::is_ascii_whitespace) {
        return Err("Ny final artifact has unresolved symbols".into());
    }
    String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            "--defined-only".into(),
            "--format=posix".into(),
            elf.as_os_str().to_owned(),
        ],
    )?)
    .map_err(|_| "Ny symbols are not UTF-8".into())
}

fn unique_offset(bytes: &[u8], needle: &[u8]) -> Result<usize, String> {
    if needle.is_empty() {
        return Err("Ny empty message is invalid".into());
    }
    let matches = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(i, v)| (v == needle).then_some(i))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err("Ny message is missing or duplicated".into());
    }
    Ok(matches[0])
}

fn inspect_instructions(
    disassembly: &str,
    image: &[u8],
    text_size: u64,
    gates: &[u64],
    traps: &[u64],
    context_end: u64,
) -> Result<(), String> {
    let allowed = [
        "cld",
        "mov",
        "movabs",
        "movb",
        "movw",
        "movl",
        "movq",
        "movslq",
        "movzbl",
        "movzwl",
        "lea",
        "leaq",
        "push",
        "pushq",
        "pop",
        "popq",
        "popf",
        "popfq",
        "add",
        "addl",
        "addq",
        "sub",
        "subl",
        "subq",
        "inc",
        "incl",
        "incq",
        "dec",
        "decl",
        "decq",
        "and",
        "andl",
        "andq",
        "or",
        "orl",
        "orq",
        "xor",
        "xorl",
        "xorq",
        "cmp",
        "cmpb",
        "cmpw",
        "cmpl",
        "cmpq",
        "test",
        "testb",
        "testl",
        "testq",
        "seta",
        "setae",
        "setb",
        "setbe",
        "sete",
        "setne",
        "setg",
        "setge",
        "setl",
        "setle",
        "setnp",
        "setp",
        "cmove",
        "cmovne",
        "xchg",
        "nop",
        "nopl",
        "nopw",
        "call",
        "callq",
        "ret",
        "retq",
        "jmp",
        "je",
        "jne",
        "jb",
        "jbe",
        "ja",
        "jae",
        "jg",
        "jge",
        "jl",
        "jle",
        "jp",
        "jnp",
        "movsd",
        "addsd",
        "ucomisd",
        "cvtsi2sd",
        "cvtsi2sdl",
        "cvtsi2sdq",
        "xorpd",
        "xorps",
        "pxor",
        "int",
        "ud2",
    ];
    let mut cursor = image_base;
    let mut addresses = Vec::new();
    let mut branches = Vec::new();
    let mut seen_gates = Vec::new();
    let mut seen_traps = Vec::new();
    let mut context_calls = 0;
    let mut scalar_add = false;
    let mut conversion = false;
    for line in disassembly.lines() {
        let Some((address, body)) = line.split_once(':') else {
            continue;
        };
        let address = address.trim();
        if address.is_empty() || !address.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let address =
            u64::from_str_radix(address, 16).map_err(|_| "Ny instruction address invalid")?;
        let mut columns = body
            .trim_start_matches(' ')
            .split('\t')
            .filter(|s| !s.trim().is_empty());
        let raw = columns.next().ok_or("Ny disassembler omitted bytes")?;
        let raw = raw
            .split_whitespace()
            .map(|byte| {
                if byte.len() == 2 {
                    u8::from_str_radix(byte, 16).map_err(|_| "Ny instruction bytes invalid")
                } else {
                    Err("Ny instruction bytes malformed")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let instruction = columns
            .next()
            .ok_or("Ny disassembler omitted instruction")?
            .trim();
        let mut words = instruction.split_whitespace();
        let mut mnemonic = words.next().ok_or("Ny instruction missing")?;
        let mut prefixed = false;
        while matches!(mnemonic, "data16" | "cs") {
            prefixed = true;
            mnemonic = words.next().ok_or("Ny dangling instruction prefix")?;
        }
        let operands = words.collect::<Vec<_>>().join(" ");
        let end = address
            .checked_add(raw.len() as u64)
            .ok_or("Ny instruction extent overflow")?;
        if address != cursor
            || raw.is_empty()
            || raw.len() > 15
            || end > image_base + text_size
            || image.get((address - image_base) as usize..(end - image_base) as usize)
                != Some(raw.as_slice())
            || !allowed.contains(&mnemonic)
            || (prefixed && !matches!(mnemonic, "nop" | "nopl" | "nopw"))
            || [
                "%fs", "%gs", "%ymm", "%zmm", "%k", "%bnd", "%tmm", "%cr", "%dr",
            ]
            .iter()
            .any(|r| operands.contains(r))
        {
            return Err(format!(
                "Ny instruction at 0x{address:x} is outside the scalar/SSE2 profile: {mnemonic}"
            ));
        }
        scalar_add |= mnemonic == "addsd";
        conversion |= mnemonic.starts_with("cvtsi2sd");
        if mnemonic == "int" {
            if operands != "$0xf3" || !gates.contains(&address) {
                return Err("Ny unexpected software gate".into());
            }
            seen_gates.push(address);
        }
        if mnemonic == "ud2" {
            if !traps.contains(&address) {
                return Err("Ny unexpected UD2".into());
            }
            seen_traps.push(address);
        }
        if mnemonic.starts_with('j') || matches!(mnemonic, "call" | "callq") {
            let target = operands
                .split_whitespace()
                .next()
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .ok_or("Ny indirect or malformed branch")?;
            branches.push(target);
            if matches!(mnemonic, "call" | "callq") && target == context_end {
                let at = (address - image_base) as usize;
                if raw.len() != 5
                    || at < 2
                    || image.get(at - 2..at) != Some(&[0x89, 0xc3])
                    || image.get(at + 5..at + 7) != Some(&[0x89, 0xd8])
                {
                    return Err(
                        "Ny generated context_end result preservation contract changed".into(),
                    );
                }
                context_calls += 1;
            }
        }
        addresses.push(address);
        cursor = end;
    }
    let mut expected_gates = gates.to_vec();
    expected_gates.sort();
    seen_gates.sort();
    let mut expected_traps = traps.to_vec();
    expected_traps.sort();
    seen_traps.sort();
    if cursor != image_base + text_size
        || branches.iter().any(|target| !addresses.contains(target))
        || seen_gates != expected_gates
        || seen_traps != expected_traps
        || context_calls != 1
        || !scalar_add
        || !conversion
    {
        return Err(
            "Ny disassembly coverage, direct control flow or required scalar operations changed"
                .into(),
        );
    }
    Ok(())
}

fn wrapper_bytes(artifact: &artifact) -> Vec<u8> {
    let mut expected = artifact.image.clone();
    expected.resize(expected.len().next_multiple_of(16), 0);
    expected.extend_from_slice(&artifact.data);
    expected
}

fn inspect_wrapper(root: &Path, artifact: &artifact) -> Result<(), String> {
    let bytes = fs::read(&artifact.object).map_err(|e| e.to_string())?;
    let elf = parse_elf64(&bytes)?;
    if elf.kind != 1
        || elf.machine != 62
        || elf.entry != 0
        || !elf.program_headers.is_empty()
        || elf.section_headers.iter().any(|s| matches!(s.kind, 4 | 9))
    {
        return Err("Ny wrapper is not relocation-free ET_REL".into());
    }
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
    if allocated.len() != 1
        || allocated[0].name != ".rodata.ny_object"
        || allocated[0].kind != 1
        || allocated[0].flags != 2
        || allocated[0].address != 0
        || section_bytes(&bytes, allocated[0])? != wrapper_bytes(artifact)
        || notes.len() != 1
        || notes[0].kind != 1
        || notes[0].flags != 0
        || notes[0].size != 0
    {
        return Err("Ny wrapper changed readonly bytes or stack declaration".into());
    }
    let symbols = listing(root, &artifact.object)?;
    check_symbols(
        &symbols,
        artifact,
        0,
        artifact.image.len().next_multiple_of(16) as u64,
    )?;
    if symbols.lines().count() != artifact.aliases.len() + 4 {
        return Err("Ny wrapper symbol set is not exact".into());
    }
    Ok(())
}

fn check_symbols(
    symbols: &str,
    artifact: &artifact,
    image_start: u64,
    data_start: u64,
) -> Result<(), String> {
    let image_end = image_start
        .checked_add(artifact.image.len() as u64)
        .ok_or("Ny payload symbol overflow")?;
    let data_end = data_start
        .checked_add(artifact.data.len() as u64)
        .ok_or("Ny data symbol overflow")?;
    for (suffix, value) in [
        ("payload_start", image_start),
        ("payload_end", image_end),
        ("data_start", data_start),
        ("data_end", data_end),
    ] {
        let name = format!("x86_64_ny_object_{suffix}");
        if symbol_u64_from_listing(symbols, &name)? != value
            || symbol_kind_from_listing(symbols, &name)? != 'R'
        {
            return Err("Ny wrapper payload symbol changed".into());
        }
    }
    for (name, value) in &artifact.aliases {
        if symbol_u64_from_listing(symbols, name)? != *value
            || symbol_kind_from_listing(symbols, name)? != 'A'
        {
            return Err("Ny absolute user alias changed".into());
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
    let symbols = listing(root, kernel)?;
    let image_start = symbol_u64_from_listing(&symbols, "x86_64_ny_object_payload_start")?;
    let data_start = symbol_u64_from_listing(&symbols, "x86_64_ny_object_data_start")?;
    check_symbols(&symbols, artifact, image_start, data_start)?;
    if image_start.checked_add(artifact.image.len().next_multiple_of(16) as u64) != Some(data_start)
    {
        return Err("Ny linked wrapper geometry changed".into());
    }
    let rodata_start = symbol_u64_from_listing(&symbols, "kernel_rodata_start")?;
    let rodata_end = symbol_u64_from_listing(&symbols, "kernel_rodata_end")?;
    for (start, payload) in [(image_start, &artifact.image), (data_start, &artifact.data)] {
        let end = start
            .checked_add(payload.len() as u64)
            .ok_or("Ny linked payload overflow")?;
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
        if start < rodata_start
            || end > rodata_end
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
        {
            return Err("Ny linked payload is changed or lacks unique readonly backing".into());
        }
    }
    if count_subslice(&bytes, &artifact.image) != 1 {
        return Err("Ny linked executable payload was duplicated".into());
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "ny_object",
        "anomalous_ny_probe",
        "Ny Anomalous probe:",
        "incompatible nyruntime ABI",
    ] {
        if symbols.contains(marker)
            || bytes
                .windows(marker.len())
                .any(|window| window == marker.as_bytes())
        {
            return Err("normal image retains the Ny object qualification fixture".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put16(bytes: &mut [u8], at: usize, value: u16) {
        bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn put32(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn put64(bytes: &mut [u8], at: usize, value: u64) {
        bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn object_fixture() -> Vec<u8> {
        let names = [
            "",
            ".strtab",
            ".text",
            ".rela.text",
            ".rodata.cst8",
            ".rodata.str1.1",
            ".bss",
            ".note.GNU-stack",
            ".eh_frame",
            ".rela.eh_frame",
            ".symtab",
        ];
        let mut strings = vec![0];
        let mut intern = |name: &str| {
            let at = strings.len() as u32;
            strings.extend_from_slice(name.as_bytes());
            strings.push(0);
            at
        };
        let section_names = names.map(&mut intern);
        let main_name = intern("main");
        let import_names = imports.map(&mut intern);
        let mut symtab = vec![0; 12 * 24];
        for (index, section) in [(1, 2), (2, 6), (3, 5)] {
            symtab[index * 24 + 4] = 3;
            put16(&mut symtab, index * 24 + 6, section);
        }
        put32(&mut symtab, 4 * 24, main_name);
        symtab[4 * 24 + 4] = 0x12;
        put16(&mut symtab, 4 * 24 + 6, 2);
        put64(&mut symtab, 4 * 24 + 16, 64);
        for (i, name) in import_names.into_iter().enumerate() {
            put32(&mut symtab, (i + 5) * 24, name);
            symtab[(i + 5) * 24 + 4] = 0x10;
        }
        let mut relocations = vec![0; 7 * 24];
        for i in 0..7 {
            put64(&mut relocations, i * 24, (i * 4) as u64);
            put64(&mut relocations, i * 24 + 8, ((i as u64 + 5) << 32) | 4);
            put64(&mut relocations, i * 24 + 16, (-4i64) as u64);
        }
        let mut unwind_relocation = vec![0; 24];
        put64(&mut unwind_relocation, 0, 8);
        put64(&mut unwind_relocation, 8, (1 << 32) | 2);
        let contents = [
            vec![],
            strings,
            vec![0; 64],
            relocations,
            vec![0; 16],
            abi_message.to_vec(),
            vec![],
            vec![],
            vec![0; 16],
            unwind_relocation,
            symtab,
        ];
        let kinds = [0, 3, 1, 4, 1, 1, 8, 1, 0x7000_0001, 4, 2];
        let flags = [0, 0, 6, 0x40, 0x12, 0x32, 3, 0, 2, 0x40, 0];
        let alignments = [0, 1, 16, 8, 8, 1, 8, 1, 8, 8, 8];
        let entries = [0, 0, 0, 24, 8, 1, 0, 0, 0, 24, 24];
        let mut bytes = vec![0; 64];
        let mut offsets = [0; 11];
        for i in 1..11 {
            bytes.resize(
                bytes.len().next_multiple_of(alignments[i].max(1) as usize),
                0,
            );
            offsets[i] = bytes.len() as u64;
            bytes.extend_from_slice(&contents[i]);
        }
        bytes.resize(bytes.len().next_multiple_of(8), 0);
        let table = bytes.len();
        bytes.resize(table + 11 * 64, 0);
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        put16(&mut bytes, 16, 1);
        put16(&mut bytes, 18, 62);
        put32(&mut bytes, 20, 1);
        put64(&mut bytes, 40, table as u64);
        put16(&mut bytes, 52, 64);
        put16(&mut bytes, 58, 64);
        put16(&mut bytes, 60, 11);
        put16(&mut bytes, 62, 1);
        for i in 0..11 {
            let at = table + i * 64;
            put32(&mut bytes, at, if i == 0 { 0 } else { section_names[i] });
            put32(&mut bytes, at + 4, kinds[i]);
            put64(&mut bytes, at + 8, flags[i]);
            put64(&mut bytes, at + 24, offsets[i]);
            put64(
                &mut bytes,
                at + 32,
                if i == 6 { 41 } else { contents[i].len() as u64 },
            );
            put32(
                &mut bytes,
                at + 40,
                if i == 10 {
                    1
                } else if i == 3 || i == 9 {
                    10
                } else {
                    0
                },
            );
            put32(
                &mut bytes,
                at + 44,
                match i {
                    3 => 2,
                    9 => 8,
                    10 => 4,
                    _ => 0,
                },
            );
            put64(&mut bytes, at + 48, alignments[i]);
            put64(&mut bytes, at + 56, entries[i]);
        }
        bytes
    }

    fn section_header(bytes: &[u8], index: usize) -> usize {
        elf_u64(bytes, 40).unwrap() as usize + index * 64
    }
    fn section_offset(bytes: &[u8], index: usize) -> usize {
        elf_u64(bytes, section_header(bytes, index) + 24).unwrap() as usize
    }

    #[test]
    fn raw_object_accepts_only_the_closed_import_and_relocation_profile() {
        let valid = object_fixture();
        let inspected = inspect_object(&valid).unwrap();
        assert_eq!(inspected.text.len(), 64);
        assert_eq!(inspected.relocated.len(), 7);
        let reloc = section_offset(&valid, 3);
        let sym = section_offset(&valid, 10);
        for (at, value) in [
            (reloc + 8, (5 << 32) | 19),
            (reloc + 8, (5 << 32) | 2),
            (reloc + 8, (99 << 32) | 4),
            (reloc, 63),
            (reloc + 16, 0),
            (reloc + 24, 0),
        ] {
            let mut changed = valid.clone();
            put64(&mut changed, at, value);
            assert!(
                inspect_object(&changed).is_err(),
                "accepted relocation mutation at {at}"
            );
        }
        let mut weak = valid.clone();
        weak[sym + 5 * 24 + 4] = 0x20;
        assert!(inspect_object(&weak).is_err());
        let mut symbol_extent = valid.clone();
        put64(&mut symbol_extent, sym + 4 * 24 + 16, 65);
        assert!(inspect_object(&symbol_extent).is_err());
        let mut renamed = valid.clone();
        let strings = section_offset(&valid, 1);
        let name = elf_u32(&valid, sym + 5 * 24).unwrap() as usize;
        renamed[strings + name] = b'x';
        assert!(inspect_object(&renamed).is_err());
    }

    #[test]
    fn raw_object_rejects_malformed_headers_sections_and_backing() {
        let valid = object_fixture();
        for (at, value) in [(16, 2u16), (18, 3), (58, 63), (60, 12), (62, 10)] {
            let mut changed = valid.clone();
            put16(&mut changed, at, value);
            assert!(inspect_object(&changed).is_err());
        }
        for (index, field, value) in [
            (2, 8, 7),
            (6, 8, 0x403),
            (3, 40, 1),
            (3, 44, 6),
            (3, 56, 8),
            (2, 32, 4096),
            (6, 32, 257),
            (4, 24, section_offset(&valid, 2) as u64),
        ] {
            let mut changed = valid.clone();
            let at = section_header(&changed, index) + field;
            put64(&mut changed, at, value);
            assert!(
                inspect_object(&changed).is_err(),
                "accepted section {index} field {field}"
            );
        }
        for end in [0, 63, valid.len() - 1] {
            assert!(inspect_object(&valid[..end]).is_err());
        }
        let mut huge = valid;
        huge.resize(65537, 0);
        assert!(inspect_object(&huge).is_err());
    }

    fn segment_fixture() -> (Vec<u8>, parsed_elf64) {
        let mut bytes = vec![0; 0x2080];
        bytes[0x2050..0x2058].copy_from_slice(&mailbox_magic.to_le_bytes());
        let loads = [
            (image_base, 5, 0x1000, 64, 64),
            (data_base, 6, 0x2000, 128, 297),
        ]
        .map(
            |(base, flags, offset, file_size, memory_size)| elf64_program_header {
                kind: 1,
                flags,
                offset,
                virtual_address: base,
                physical_address: base,
                file_size,
                memory_size,
                alignment: 4096,
            },
        );
        let sections = [
            (".text", 1, 6, image_base, 0x1000, 32),
            (".rodata", 1, 2, image_base + 32, 0x1020, 32),
            (".data", 1, 3, data_base, 0x2000, 128),
            (".bss", 8, 3, bss_base, 0x2080, 41),
        ]
        .map(
            |(name, kind, flags, address, offset, size)| elf64_section_header {
                name: name.into(),
                kind,
                flags,
                address,
                offset,
                size,
            },
        );
        (
            bytes,
            parsed_elf64 {
                kind: 2,
                machine: 62,
                entry: image_base,
                program_headers: loads.to_vec(),
                section_headers: sections.to_vec(),
            },
        )
    }

    #[test]
    fn linked_image_requires_exact_permissions_bss_and_initialized_header() {
        let (bytes, elf) = segment_fixture();
        let (image, data) = inspect_segments(&bytes, &elf).unwrap();
        assert_eq!(image.len(), 64);
        assert_eq!(data.len(), 128);
        for (index, flags) in [(0, 7), (0, 4), (1, 7), (1, 4)] {
            let mut changed = elf.clone();
            changed.program_headers[index].flags = flags;
            assert!(inspect_segments(&bytes, &changed).is_err());
        }
        let mut changed = elf.clone();
        changed.program_headers[1].memory_size = 256;
        assert!(inspect_segments(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.section_headers[3].address += 1;
        assert!(inspect_segments(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.section_headers[1].address -= 1;
        assert!(inspect_segments(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.section_headers[1].name = ".init_array".into();
        assert!(inspect_segments(&bytes, &changed).is_err());
        let mut changed = elf.clone();
        changed.section_headers[1].kind = 4;
        assert!(inspect_segments(&bytes, &changed).is_err());
        for at in [0x2000, 0x2050, 0x207f] {
            let mut changed = bytes.clone();
            changed[at] ^= 1;
            assert!(inspect_segments(&changed, &elf).is_err());
        }
        assert!(inspect_segments(&bytes[..bytes.len() - 1], &elf).is_err());
    }

    fn instruction_fixture() -> (Vec<u8>, String) {
        let instructions: &[(&[u8], &str)] = &[
            (&[0x89, 0xc3], "mov %eax,%ebx"),
            (&[0xe8, 14, 0, 0, 0], "call 400015 <nyrt_context_end>"),
            (&[0x89, 0xd8], "mov %ebx,%eax"),
            (&[0xf2, 0x0f, 0x58, 0xc8], "addsd %xmm0,%xmm1"),
            (&[0xf2, 0x0f, 0x2a, 0xc7], "cvtsi2sd %edi,%xmm0"),
            (&[0xcd, 0xf3], "int $0xf3"),
            (&[0x0f, 0x0b], "ud2"),
            (&[0xc3], "ret"),
        ];
        let mut image = Vec::new();
        let mut listing = String::new();
        for (bytes, instruction) in instructions {
            let hex = bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            listing.push_str(&format!(
                "{:x}:\t{hex}\t{instruction}\n",
                image_base + image.len() as u64
            ));
            image.extend_from_slice(bytes);
        }
        (image, listing)
    }

    fn inspect_fixture_instructions(image: &[u8], listing: &str) -> Result<(), String> {
        inspect_instructions(
            listing,
            image,
            image.len() as u64,
            &[image_base + 17],
            &[image_base + 19],
            image_base + 21,
        )
    }

    #[test]
    fn instruction_profile_rejects_tls_avx_system_indirect_and_undecoded_bytes() {
        let (image, listing) = instruction_fixture();
        inspect_fixture_instructions(&image, &listing).unwrap();
        for (old, new) in [
            ("addsd", "vaddsd"),
            ("ret", "syscall"),
            ("ret", "rdtsc"),
            ("%edi,%xmm0", "%fs:0,%xmm0"),
            ("call 400015", "call *%rax"),
            ("call 400015", "call 400016"),
            ("int $0xf3", "int $0x80"),
            ("400013:", "400014:"),
            ("0f 0b", "0f 0c"),
        ] {
            assert!(
                inspect_fixture_instructions(&image, &listing.replace(old, new)).is_err(),
                "accepted {new}"
            );
        }
        assert!(
            inspect_fixture_instructions(
                &image,
                &listing.lines().skip(1).collect::<Vec<_>>().join("\n")
            )
            .is_err()
        );
        assert!(
            inspect_instructions(
                &listing,
                &image,
                image.len() as u64,
                &[],
                &[image_base + 19],
                image_base + 21
            )
            .is_err()
        );
        assert!(
            inspect_instructions(
                &listing,
                &image,
                image.len() as u64,
                &[image_base + 17],
                &[],
                image_base + 21
            )
            .is_err()
        );
    }

    #[test]
    fn generated_result_contract_requires_real_callee_saved_register_bytes() {
        let (mut image, listing) = instruction_fixture();
        image[1] = 0xc1;
        let listing = listing
            .replace("89 c3", "89 c1")
            .replace("mov %eax,%ebx", "mov %eax,%ecx");
        assert!(
            inspect_fixture_instructions(&image, &listing)
                .unwrap_err()
                .contains("result preservation")
        );
    }

    #[test]
    fn wrapper_symbols_reject_overflow_and_message_identity_is_unique() {
        let artifact = artifact {
            object: PathBuf::new(),
            image: vec![0x90; 17],
            data: vec![0; 128],
            aliases: BTreeMap::new(),
        };
        assert!(
            check_symbols("", &artifact, u64::MAX - 1, 32)
                .unwrap_err()
                .contains("overflow")
        );
        assert!(
            check_symbols("", &artifact, 0, u64::MAX - 1)
                .unwrap_err()
                .contains("overflow")
        );
        assert_eq!(wrapper_bytes(&artifact).len(), 160);
        assert_eq!(unique_offset(b"abcde", b"bcd").unwrap(), 1);
        assert!(unique_offset(b"abcabc", b"abc").is_err());
        assert!(unique_offset(b"abc", b"def").is_err());
        assert!(unique_offset(b"abc", b"").is_err());
        verify_absent("kernel_boot T 1000", b"normal kernel").unwrap();
        assert!(verify_absent("x86_64_ny_object_payload_start R 1000", b"").is_err());
        assert!(verify_absent("", context_message).is_err());
        assert!(verify_absent("", abi_message).is_err());
    }
}
