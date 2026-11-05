//! Strict admission proof for three retained, heterogeneous native owners.
//! These small assembly fixtures are not the real desktop or editor vessels.
use super::*;

pub(super) const feature: &str = "mixed_workspace_test";
const prefix: &str = "kernel.mixed_workspace_test.";
const user_entry: u64 = 0x400000;
const role_names: [&str; 3] = ["workspace", "editor", "service"];

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.contains(&native_editor_workspace::feature)
        && application_profile_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|value| *value != feature)
                .chain([application_profile_smoke::feature])
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!(
        "mixed owner admission image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    build_and_inspect_normal_image()?;
    let image = build()?;
    let directory = repository_root()?.join("build/mixed-workspace");
    fs::create_dir_all(&directory).map_err(|error| format!("create mixed evidence: {error}"))?;
    for (backend, cpu) in [("tcg", "Nehalem"), ("kvm", "host")] {
        let small = run_size(&image.path, backend, cpu, "96M", &directory)?;
        let large = run_size(&image.path, backend, cpu, "320M", &directory)?;
        verify_size_evidence(small, large)?;
    }
    println!("mixed retained admission passed TCG/KVM at 96M and 320M");
    Ok(())
}

fn run_size(
    image: &Path,
    backend: &str,
    cpu: &str,
    memory: &str,
    directory: &Path,
) -> Result<run_evidence, String> {
    let log = directory.join(format!("qemu_{backend}_{memory}.log"));
    remove_optional_file(&log)?;
    // A file-backed serial sink survives deadline/launch failures, including
    // the shared runner's timeout path which intentionally skips verification.
    let mut command = Command::new("qemu-system-x86_64");
    command.current_dir(repository_root()?).args([
        OsString::from("-machine"),
        OsString::from("q35"),
        OsString::from("-m"),
        OsString::from(memory),
        OsString::from("-drive"),
        OsString::from(format!("format=raw,file={}", image.display())),
        OsString::from("-no-reboot"),
        OsString::from("-serial"),
        OsString::from(format!("file:{}", log.display())),
    ]);
    command.args([
        "-accel",
        backend,
        "-cpu",
        cpu,
        "-display",
        "none",
        "-monitor",
        "none",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ]);
    let mut evidence = None;
    run_qemu_with_transcript(command, smoke_timeout, |status, diagnostic| {
        let transcript = read_bounded_optional_utf8_file(&log, 32768)?;
        print!("{transcript}");
        if status.code() != Some(qemu_success_status) {
            return Err(format!(
                "mixed {backend}/{memory} exited {:?}; transcript retained",
                status.code()
            ));
        }
        if !diagnostic.trim().is_empty() {
            return Err("mixed QEMU emitted unexpected host diagnostics".into());
        }
        evidence = Some(verify_transcript(&transcript)?);
        Ok(())
    })?;
    evidence.ok_or_else(|| "mixed proof produced no transcript".into())
}

#[derive(Clone, Copy)]
pub(super) struct run_evidence {
    free_frames: u64,
    unavailable_frames: u64,
    total_frames: u64,
}

fn verify_size_evidence(small: run_evidence, large: run_evidence) -> Result<(), String> {
    let delta = (320u64 - 96) * 1024 * 1024 / memory_frame_size;
    if small.total_frames != memory_total_frames
        || large.total_frames != memory_total_frames
        || large.free_frames.checked_sub(small.free_frames) != Some(delta)
        || small
            .unavailable_frames
            .checked_sub(large.unavailable_frames)
            != Some(delta)
    {
        return Err("mixed proof did not preserve the exact 224-MiB allocator delta".into());
    }
    Ok(())
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in ["mixed_workspace", "x86_64_mixed_workspace", prefix] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains mixed admission evidence or payload".into());
        }
    }
    for role in 0..3u64 {
        let canary = (0x71716d6978656400u64 + role).to_le_bytes();
        if bytes.windows(canary.len()).any(|window| window == canary) {
            return Err("normal image retains a mixed private fixture canary".into());
        }
    }
    Ok(())
}

pub(super) fn inspect_linked(root: &Path, elf: &Path) -> Result<(), String> {
    let directory = root.join("build/mixed-workspace");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("create mixed artifact directory: {error}"))?;
    let bytes = fs::read(elf).map_err(|error| format!("read mixed kernel: {error}"))?;
    let parsed = parse_elf64(&bytes)?;
    let symbols = String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.into(),
        ],
    )?)
    .map_err(|_| "mixed symbols are not UTF-8")?;
    for name in [
        "x86_64_mixed_workspace_gate_f3",
        "x86_64_mixed_workspace_gate_f4",
        "x86_64_mixed_workspace_dispatch",
    ] {
        if symbol_kind_from_listing(&symbols, name)? != 'T' {
            return Err(format!(
                "mixed privileged entry {name} is not executable text"
            ));
        }
    }
    let mut ranges = Vec::new();
    let mut manifests = Vec::new();
    for (index, role) in role_names.into_iter().enumerate() {
        let name = format!("x86_64_mixed_workspace_{role}");
        let start = symbol_u64_from_listing(&symbols, &format!("{name}_start"))?;
        let end = symbol_u64_from_listing(&symbols, &format!("{name}_end"))?;
        let length = end
            .checked_sub(start)
            .filter(|size| *size > 0 && *size <= 32768)
            .ok_or("mixed fixture length is empty or exceeds its fixed code bound")?;
        for suffix in ["start", "end", "failed_instruction"] {
            if symbol_kind_from_listing(&symbols, &format!("{name}_{suffix}"))? != 'R' {
                return Err("mixed fixture is not linked read-only".into());
            }
        }
        let load = fixture_load(&parsed, start, end)?;
        if ranges
            .iter()
            .any(|(prior_start, prior_end)| start < *prior_end && *prior_start < end)
        {
            return Err("mixed fixture ranges overlap".into());
        }
        ranges.push((start, end));
        let payload = elf_virtual_bytes(&bytes, load, start, length as usize)?;
        if count_subslice(&bytes, payload) != 1 {
            return Err("mixed fixture is duplicated in the kernel".into());
        }
        let failed = symbol_u64_from_listing(&symbols, &format!("{name}_failed_instruction"))?;
        let mut faults = vec![
            failed
                .checked_sub(start)
                .ok_or("mixed fault precedes payload")?,
        ];
        if role == "service" {
            if symbol_kind_from_listing(&symbols, "x86_64_mixed_workspace_service_cut_instruction")?
                != 'R'
            {
                return Err("mixed service cut is not read-only".into());
            }
            let cut = symbol_u64_from_listing(
                &symbols,
                "x86_64_mixed_workspace_service_cut_instruction",
            )?;
            faults.push(cut.checked_sub(start).ok_or("mixed cut precedes payload")?);
        }
        for offset in &faults {
            if payload.get(*offset as usize..*offset as usize + 2) != Some(&[0x0f, 0x0b]) {
                return Err("mixed selected fault is not an exact in-payload UD2".into());
            }
        }
        let binary = directory.join(format!("{role}.bin"));
        fs::write(&binary, payload).map_err(|error| format!("retain mixed payload: {error}"))?;
        let disassembly = run_output_checked(
            root,
            "objdump",
            [
                OsString::from("-D"),
                OsString::from("-b"),
                OsString::from("binary"),
                OsString::from("-m"),
                OsString::from("i386:x86-64"),
                OsString::from("--adjust-vma=0x400000"),
                OsString::from("--insn-width=16"),
                OsString::from("-w"),
                binary.into_os_string(),
            ],
        )?;
        fs::write(directory.join(format!("{role}.disassembly")), &disassembly)
            .map_err(|error| format!("retain mixed disassembly: {error}"))?;
        let disassembly =
            String::from_utf8(disassembly).map_err(|_| "mixed disassembly is not UTF-8")?;
        inspect_fixture(&disassembly, payload, &faults, role)?;
        manifests.push(format!(
            "{{\"role\":{role:?},\"index\":{index},\"bytes\":{length},\"fnv32\":{},\"entry\":{user_entry},\"explicit_stack_bytes\":0,\"calls\":0,\"indirect_transfers\":0}}",
            fnv_1a(payload),
        ));
    }
    fs::write(directory.join("kernel.symbols"), symbols)
        .map_err(|error| format!("retain mixed symbols: {error}"))?;
    fs::write(directory.join("manifest.json"),format!(
        "{{\"profile\":\"mixed_workspace_test\",\"artifact_kind\":\"assembly_admission_fixture\",\"roles\":[{}]}}\n",manifests.join(",")
    )).map_err(|error|format!("retain mixed manifest: {error}"))?;
    Ok(())
}

fn fixture_load(
    parsed: &parsed_elf64,
    start: u64,
    end: u64,
) -> Result<elf64_program_header, String> {
    if start >= end {
        return Err("mixed fixture extent is empty".into());
    }
    let sections = parsed
        .section_headers
        .iter()
        .filter(|section| {
            section.flags & 2 != 0
                && section.address < end
                && section.address.saturating_add(section.size) > start
        })
        .collect::<Vec<_>>();
    let loads = parsed
        .program_headers
        .iter()
        .copied()
        .filter(|load| {
            load.kind == 1
                && load.virtual_address < end
                && load.virtual_address.saturating_add(load.memory_size) > start
        })
        .collect::<Vec<_>>();
    // This kernel links readonly .rodata into its combined RX container LOAD.
    // The retained task mapping, rather than the container, supplies user RX.
    if sections.len() != 1
        || sections[0].kind != 1
        || sections[0].name != ".rodata"
        || sections[0].flags != 2
        || start < sections[0].address
        || end
            > sections[0]
                .address
                .checked_add(sections[0].size)
                .unwrap_or(0)
        || loads.len() != 1
        || loads[0].flags != 5
        || start < loads[0].virtual_address
        || end
            > loads[0]
                .virtual_address
                .checked_add(loads[0].file_size)
                .unwrap_or(0)
    {
        return Err("mixed fixture has overlapping or non-read-only placement".into());
    }
    Ok(loads[0])
}

/// The fixtures are leaf instruction streams. Reject every uninspected byte,
/// indirect transfer and explicit stack access; this is not a kernel-stack audit.
fn inspect_fixture(
    disassembly: &str,
    image: &[u8],
    faults: &[u64],
    role: &str,
) -> Result<(), String> {
    let mut cursor = user_entry;
    let end = user_entry
        .checked_add(image.len() as u64)
        .ok_or("mixed code overflow")?;
    let mut boundaries = Vec::new();
    let mut branches = Vec::new();
    let mut found_faults = Vec::new();
    let mut gates = [0; 2];
    let mut fp = 0;
    let mut initialized = 0;
    let mut loaded = 0;
    let mut seeded = 0;
    let mut arena_fills = 0;
    for line in disassembly.lines() {
        let Some((address, body)) = line.split_once(':') else {
            continue;
        };
        let address = address.trim();
        if address.is_empty() || !address.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let address = u64::from_str_radix(address, 16).map_err(|_| "mixed instruction address")?;
        let mut columns = body.split('\t').filter(|column| !column.trim().is_empty());
        let raw = columns
            .next()
            .ok_or("mixed instruction bytes absent")?
            .split_whitespace()
            .map(|byte| {
                if byte.len() == 2 {
                    u8::from_str_radix(byte, 16).map_err(|_| "mixed byte")
                } else {
                    Err("mixed byte width")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let instruction = columns.next().ok_or("mixed instruction absent")?.trim();
        let (mnemonic, operands) = instruction
            .split_once(char::is_whitespace)
            .unwrap_or((instruction, ""));
        let operands = operands.trim();
        let instruction_end = address
            .checked_add(raw.len() as u64)
            .ok_or("mixed instruction overflow")?;
        if address != cursor
            || raw.is_empty()
            || raw.len() > 15
            || instruction_end > end
            || image.get((address - user_entry) as usize..(instruction_end - user_entry) as usize)
                != Some(raw.as_slice())
        {
            return Err("mixed instruction coverage has a gap, overlap or byte mismatch".into());
        }
        boundaries.push(address);
        cursor = instruction_end;
        if operands.contains("%rsp")
            || operands.contains("%esp")
            || operands.contains("%sp")
            || operands.contains("%rbp")
            || operands.contains("%ebp")
            || operands.contains("%bp")
        {
            return Err("mixed fixture unexpectedly uses the user stack".into());
        }
        match mnemonic {
            "mov" | "movabs" | "movb" | "movw" | "movl" | "movq" | "cmp" | "cmpb" | "cmpq"
            | "cmpl" | "test" | "testq" | "testl" | "xor" | "xorl" | "xorq" | "add" | "sub"
            | "lea" | "inc" | "dec" | "and" | "or" => {
                if operands.contains("%xmm") {
                    if mnemonic != "movq" {
                        return Err("mixed SIMD mnemonic changed".into());
                    }
                    match (raw.as_slice(), operands) {
                        ([0x66, 0x49, 0x0f, 0x6e, 0xc5], "%r13,%xmm0") => seeded += 1,
                        ([0x66, 0x49, 0x0f, 0x7e, 0xc2], "%xmm0,%r10") => {}
                        _ => return Err("mixed fixture has an unqualified SIMD form".into()),
                    }
                    fp += 1;
                } else if operands.contains("%mm")
                    || operands.contains("%st")
                    || operands.contains("%cr")
                    || operands.contains("%dr")
                    || operands.contains("%cs")
                    || operands.contains("%es")
                    || operands.contains("%ds")
                    || operands.contains("%ss")
                    || operands.contains("%fs")
                    || operands.contains("%gs")
                {
                    return Err("mixed fixture contains privileged or unqualified registers".into());
                }
            }
            "jmp" | "je" | "jne" | "jb" | "jbe" | "ja" | "jae" | "jg" | "jge" | "jl" | "jle" => {
                let target = operands
                    .split_whitespace()
                    .next()
                    .ok_or("mixed branch target absent")?;
                let target = u64::from_str_radix(target.trim_start_matches("0x"), 16)
                    .map_err(|_| "mixed branch is indirect or malformed")?;
                branches.push(target);
            }
            "fninit" if raw == [0xdb, 0xe3] && operands.is_empty() => {
                fp += 1;
                initialized += 1;
            }
            "fld1" if raw == [0xd9, 0xe8] && operands.is_empty() && role == "workspace" => {
                fp += 1;
                loaded += 1;
            }
            "fldz" if raw == [0xd9, 0xee] && operands.is_empty() && role == "editor" => {
                fp += 1;
                loaded += 1;
            }
            "fldpi" if raw == [0xd9, 0xeb] && operands.is_empty() && role == "service" => {
                fp += 1;
                loaded += 1;
            }
            "cld" if raw == [0xfc] && operands.is_empty() => {}
            "pause" if raw == [0xf3, 0x90] && operands.is_empty() => {}
            "rep"
                if raw == [0xf3, 0x48, 0xab]
                    && matches!(operands, "stos %rax,%es:(%rdi)" | "stos %rax,(%rdi)") =>
            {
                arena_fills += 1;
            }
            "int" if raw == [0xcd, 0xf3] && operands == "$0xf3" => gates[0] += 1,
            "int" if raw == [0xcd, 0xf4] && operands == "$0xf4" => gates[1] += 1,
            "ud2" if raw == [0x0f, 0x0b] && operands.is_empty() => {
                found_faults.push(address - user_entry)
            }
            _ => {
                return Err(format!(
                    "mixed fixture instruction not admitted: {instruction}"
                ));
            }
        }
    }
    found_faults.sort_unstable();
    let mut expected_faults = faults.to_vec();
    expected_faults.sort_unstable();
    let (expected_gates, expected_fp) = match role {
        "workspace" => ([2, 21], 25),
        "editor" => ([1, 23], 26),
        "service" => ([1, 8], 11),
        _ => return Err("unknown mixed fixture role".into()),
    };
    // Each role initializes its arena once. In particular, the workspace must
    // not refill it before the retained-content check after its peers ran.
    if arena_fills != 1 {
        return Err("mixed fixture must initialize its arena exactly once".into());
    }
    if cursor != end
        || fp != expected_fp
        || initialized != 1
        || loaded != 1
        || seeded != 1
        || found_faults != expected_faults
        || gates != expected_gates
        || branches.iter().any(|target| !boundaries.contains(target))
        || image.last_chunk::<2>() != Some(&[0x0f, 0x0b])
    {
        return Err(
            "mixed fixture lacks complete role-local control/CPU/gate/fault coverage".into(),
        );
    }
    Ok(())
}

const events: [&str; 22] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.mixed_workspace_test.ready",
    "kernel.mixed_workspace_test.started",
    "kernel.mixed_workspace_test.packet",
    "kernel.mixed_workspace_test.started",
    "kernel.mixed_workspace_test.packet",
    "kernel.mixed_workspace_test.packet",
    "kernel.mixed_workspace_test.started",
    "kernel.mixed_workspace_test.packet",
    "kernel.mixed_workspace_test.retired",
    "kernel.mixed_workspace_test.packet",
    "kernel.mixed_workspace_test.packet",
    "kernel.mixed_workspace_test.retired",
    "kernel.mixed_workspace_test.retired",
    "kernel.mixed_workspace_test.passed",
];

pub(super) fn verify_transcript(transcript: &str) -> Result<run_evidence, String> {
    if transcript.len() > 32768 {
        return Err("mixed transcript exceeds its bound".into());
    }
    let mut records = Vec::new();
    for line in transcript
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let event = *events
            .get(records.len())
            .ok_or("mixed transcript has extra records")?;
        let record = json_parser::new(line).parse_record()?;
        require_string(&record, event, "event", event)?;
        require_string(&record, event, "level", "info")?;
        require_number(&record, event, "schema", 1)?;
        records.push(record);
    }
    if records.len() != events.len() {
        return Err("mixed transcript is incomplete".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    let free = required_number(&records[4], events[4], "free_frames")?;
    let live = free
        .checked_sub(144)
        .ok_or("mixed bootstrap has insufficient frames")?;
    exact_record(
        &records[8],
        events[8],
        &[
            ("tasks", 3),
            ("channels", 4),
            ("workspace_rx_pages", 32),
            ("workspace_rw_pages", 8),
            ("workspace_arena_pages", 9),
            ("workspace_stack_pages", 16),
            ("application_rx_pages", 8),
            ("application_rw_pages", 2),
            ("application_arena_pages", 8),
            ("application_stack_pages", 2),
            ("owned_frames", 144),
            ("data_frames", 117),
            ("table_frames", 27),
            ("leases", 18),
            ("zeroed_arena_bytes", 102400),
            ("free_before", free),
            ("free_live", live),
        ],
        &[],
        &[],
    )?;
    for (index, role) in [(9, 0), (11, 1), (14, 2)] {
        exact_record(
            &records[index],
            events[index],
            &[("role", role)],
            &[("cpu_clean", true)],
            &[],
        )?;
    }
    for (index, channel, sequence, producer, consumer, word) in [
        (10, 0, 1, 0, 1, 0x574f524b),
        (12, 1, 1, 1, 0, 0x45444954),
        (13, 2, 1, 1, 2, 0x444f4351),
        (15, 3, 1, 2, 1, 0x444f4352),
        (17, 0, 2, 0, 1, 0x574f5232),
        (18, 1, 2, 1, 0, 0x45444932),
    ] {
        exact_record(
            &records[index],
            events[index],
            &[
                ("channel", channel),
                ("sequence", sequence),
                ("producer", producer),
                ("consumer", consumer),
                ("word", word),
            ],
            &[],
            &[],
        )?;
    }
    for (index, role, faulted, closed, survivors) in [
        (16, 2, true, 2, 2),
        (19, 1, false, 2, 1),
        (20, 0, false, 0, 0),
    ] {
        exact_record(
            &records[index],
            events[index],
            &[
                ("role", role),
                ("closed_routes", closed),
                ("survivors", survivors),
            ],
            &[("faulted", faulted)],
            &[],
        )?;
    }
    let passed = &records[21];
    let event = events[21];
    let mut timers = [0u64; 3];
    let mut stable = [0u64; 3];
    let mut numbers = vec![
        ("calls", 56),
        ("workspace_calls", 23),
        ("editor_calls", 24),
        ("service_calls", 9),
        ("switches", 10),
        ("denials", 6),
        ("bad_copies", 5),
        ("closed", 2),
        ("sends", 6),
        ("deliveries", 6),
        ("acknowledgements", 6),
        ("post_service_packets", 2),
        ("arena_checks", 2),
        ("arena_checked_bytes", 73728),
        ("service_checks", 1),
        ("exits", 2),
        ("faults", 1),
    ];
    for (index, (all, sealed)) in [
        ("workspace_timers", "workspace_stable_timers"),
        ("editor_timers", "editor_stable_timers"),
        ("service_timers", "service_stable_timers"),
    ]
    .into_iter()
    .enumerate()
    {
        timers[index] = required_number(passed, event, all)?;
        stable[index] = required_number(passed, event, sealed)?;
        if !(2..=10000).contains(&timers[index]) || !(2..=timers[index]).contains(&stable[index]) {
            return Err("mixed role lacks bounded genuine CPU timer coverage".into());
        }
        numbers.push((all, timers[index]));
        numbers.push((sealed, stable[index]));
    }
    let captures = 57 + timers.iter().sum::<u64>();
    numbers.extend([
        ("cpu_captures", captures),
        ("cpu_clean_checks", captures),
        ("cpu_user_checks", 54 + stable.iter().sum::<u64>()),
        ("free_after", free),
    ]);
    exact_record(
        passed,
        event,
        &numbers,
        &[
            ("resources_restored", true),
            ("authority_revoked", true),
            ("cpu_restored", true),
        ],
        &[
            ("source_language", "assembly"),
            ("personality", "native_f3_f4"),
        ],
    )?;
    Ok(run_evidence {
        free_frames: free,
        unavailable_frames: required_number(&records[4], events[4], "unavailable_frames")?,
        total_frames: required_number(&records[4], events[4], "total_frames")?,
    })
}

fn exact_record(
    record: &structured_record,
    event: &str,
    numbers: &[(&str, u64)],
    booleans: &[(&str, bool)],
    strings: &[(&str, &str)],
) -> Result<(), String> {
    let fields = numbers
        .iter()
        .map(|(name, _)| *name)
        .chain(booleans.iter().map(|(name, _)| *name))
        .chain(strings.iter().map(|(name, _)| *name))
        .collect::<Vec<_>>();
    require_control_header(record, event, &fields)?;
    for (field, value) in numbers {
        require_number(record, event, field, *value)?;
    }
    for (field, value) in booleans {
        require_bool(record, event, field, *value)?;
    }
    for (field, value) in strings {
        require_string(record, event, field, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_transcript() -> String {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.ready","level":"info","tasks":3,"channels":4,"workspace_rx_pages":32,"workspace_rw_pages":8,"workspace_arena_pages":9,"workspace_stack_pages":16,"application_rx_pages":8,"application_rw_pages":2,"application_arena_pages":8,"application_stack_pages":2,"owned_frames":144,"data_frames":117,"table_frames":27,"leases":18,"zeroed_arena_bytes":102400,"free_before":20000,"free_live":19856}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.started","level":"info","role":0,"cpu_clean":true}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.packet","level":"info","channel":0,"sequence":1,"producer":0,"consumer":1,"word":1464816203}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.started","level":"info","role":1,"cpu_clean":true}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.packet","level":"info","channel":1,"sequence":1,"producer":1,"consumer":0,"word":1162103124}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.packet","level":"info","channel":2,"sequence":1,"producer":1,"consumer":2,"word":1146045265}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.started","level":"info","role":2,"cpu_clean":true}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.packet","level":"info","channel":3,"sequence":1,"producer":2,"consumer":1,"word":1146045266}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.retired","level":"info","role":2,"faulted":true,"closed_routes":2,"survivors":2}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.packet","level":"info","channel":0,"sequence":2,"producer":0,"consumer":1,"word":1464816178}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.packet","level":"info","channel":1,"sequence":2,"producer":1,"consumer":0,"word":1162103090}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.retired","level":"info","role":1,"faulted":false,"closed_routes":2,"survivors":1}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.retired","level":"info","role":0,"faulted":false,"closed_routes":0,"survivors":0}"#,
            r#"{"schema":1,"event":"kernel.mixed_workspace_test.passed","level":"info","calls":56,"workspace_calls":23,"editor_calls":24,"service_calls":9,"switches":10,"denials":6,"bad_copies":5,"closed":2,"sends":6,"deliveries":6,"acknowledgements":6,"post_service_packets":2,"arena_checks":2,"arena_checked_bytes":73728,"service_checks":1,"exits":2,"faults":1,"workspace_timers":2,"editor_timers":2,"service_timers":2,"workspace_stable_timers":2,"editor_stable_timers":2,"service_stable_timers":2,"cpu_captures":63,"cpu_clean_checks":63,"cpu_user_checks":60,"free_after":20000,"resources_restored":true,"authority_revoked":true,"cpu_restored":true,"source_language":"assembly","personality":"native_f3_f4"}"#,
        ].join("\n")
    }

    fn render(record: &structured_record) -> String {
        let fields = record
            .iter()
            .map(|(key, value)| {
                let value = match value {
                    json_scalar::number(value) => value.clone(),
                    json_scalar::string(value) => format!("{value:?}"),
                    json_scalar::boolean(value) => value.to_string(),
                    json_scalar::null => "null".into(),
                };
                format!("{key:?}:{value}")
            })
            .collect::<Vec<_>>();
        format!("{{{}}}", fields.join(","))
    }

    #[test]
    fn transcript_requires_exact_order_shape_identity_and_cleanup() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        verify_transcript(&format!("\r\n{}\r\n", valid.replace('\n', "\r\n"))).unwrap();
        let records = valid.lines().collect::<Vec<_>>();
        for index in 0..records.len() {
            let mut changed = records.clone();
            changed.remove(index);
            assert!(verify_transcript(&changed.join("\n")).is_err());
            let mut changed = records.clone();
            changed.insert(index, records[index]);
            assert!(verify_transcript(&changed.join("\n")).is_err());
            if index + 1 < records.len() {
                let mut changed = records.clone();
                changed.swap(index, index + 1);
                assert!(verify_transcript(&changed.join("\n")).is_err());
            }
        }
        for index in 8..records.len() {
            let original = json_parser::new(records[index]).parse_record().unwrap();
            for (key, value) in &original {
                let mut changed = original.clone();
                changed.remove(key);
                let mut lines = records
                    .iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>();
                lines[index] = render(&changed);
                assert!(
                    verify_transcript(&lines.join("\n")).is_err(),
                    "missing {index}/{key}"
                );
                let mut changed = original.clone();
                changed.insert(key.clone(), json_scalar::null);
                lines[index] = render(&changed);
                assert!(
                    verify_transcript(&lines.join("\n")).is_err(),
                    "null {index}/{key}"
                );
                let value = match value {
                    json_scalar::number(value) => {
                        json_scalar::number((value.parse::<u64>().unwrap() + 1).to_string())
                    }
                    json_scalar::boolean(value) => json_scalar::boolean(!value),
                    json_scalar::string(value) => json_scalar::string(format!("{value}_foreign")),
                    json_scalar::null => unreachable!(),
                };
                let mut changed = original.clone();
                changed.insert(key.clone(), value);
                lines[index] = render(&changed);
                assert!(
                    verify_transcript(&lines.join("\n")).is_err(),
                    "changed {index}/{key}"
                );
            }
            let mut changed = original.clone();
            changed.insert("untrusted_payload".into(), json_scalar::number("0".into()));
            let mut lines = records
                .iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>();
            lines[index] = render(&changed);
            assert!(verify_transcript(&lines.join("\n")).is_err());
        }
        assert!(verify_transcript(&format!("{valid}\nunstructured")).is_err());
        assert!(verify_transcript(&" ".repeat(32769)).is_err());
    }

    #[test]
    fn cpu_relations_and_allocator_delta_are_independently_required() {
        let valid = valid_transcript();
        let mut records = valid
            .lines()
            .map(|line| json_parser::new(line).parse_record().unwrap())
            .collect::<Vec<_>>();
        for role in ["workspace", "editor", "service"] {
            records[21].insert(format!("{role}_timers"), json_scalar::number("3".into()));
            records[21].insert(
                format!("{role}_stable_timers"),
                json_scalar::number("3".into()),
            );
        }
        for (field, value) in [
            ("cpu_captures", "66"),
            ("cpu_clean_checks", "66"),
            ("cpu_user_checks", "63"),
        ] {
            records[21].insert(field.into(), json_scalar::number(value.into()));
        }
        let joined = records.iter().map(render).collect::<Vec<_>>().join("\n");
        verify_transcript(&joined).unwrap();
        records[21].insert(
            "workspace_stable_timers".into(),
            json_scalar::number("4".into()),
        );
        assert!(
            verify_transcript(&records.iter().map(render).collect::<Vec<_>>().join("\n")).is_err()
        );
        let small = verify_transcript(&valid).unwrap();
        let delta = 224 * 1024 * 1024 / 4096;
        let large = run_evidence {
            free_frames: small.free_frames + delta,
            unavailable_frames: small.unavailable_frames - delta,
            ..small
        };
        verify_size_evidence(small, large).unwrap();
        assert!(verify_size_evidence(large, small).is_err());
        assert!(
            verify_size_evidence(
                small,
                run_evidence {
                    free_frames: large.free_frames + 1,
                    ..large
                }
            )
            .is_err()
        );
    }

    #[test]
    fn mixed_selection_is_explicit_and_keeps_every_higher_profile() {
        assert!(!selected(&[]));
        assert!(!selected(&[application_profile_smoke::feature]));
        assert!(selected(&[feature]));
        let lower = [
            (
                application_profile_smoke::feature,
                application_profile_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                owned_dispatcher_smoke::feature,
                owned_dispatcher_smoke::selected,
            ),
            (
                owned_multi_region_smoke::feature,
                owned_multi_region_smoke::selected,
            ),
            (owned_source_smoke::feature, owned_source_smoke::selected),
            (owned_tls_smoke::feature, owned_tls_smoke::selected),
            (dispatcher_smoke::feature, dispatcher_smoke::selected),
            (workspace_dev::feature, workspace_dev::selected),
            (owned_workspace::feature, owned_workspace::selected),
            (owned_workspace_cpu::feature, owned_workspace_cpu::selected),
        ];
        let mut expanded = vec![feature];
        for (name, selector) in lower {
            assert!(selected(&[feature, name]));
            assert!(!selector(&[feature, name]), "lower {name}");
            expanded.push(name);
        }
        assert!(selected(&expanded));
        for (name, selector) in lower {
            assert!(!selector(&expanded), "expanded {name}");
        }
        for higher in all_kernel_features.iter().copied().filter(|name| {
            !lower.iter().any(|(lower, _)| name == lower)
                && !matches!(
                    *name,
                    "mixed_workspace_test" | "scheduler_test" | "smoke_test"
                )
        }) {
            assert!(!selected(&[feature, higher]), "higher {higher}");
        }
        for (name, selector) in [
            (
                ny_object_smoke::feature,
                ny_object_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                native_application_smoke::feature,
                native_application_smoke::selected,
            ),
            (
                native_runtime_smoke::feature,
                native_runtime_smoke::selected,
            ),
            (
                native_durable_smoke::feature,
                native_durable_smoke::selected,
            ),
            (
                native_durable_failure_smoke::feature,
                native_durable_failure_smoke::selected,
            ),
            (
                native_interactive_smoke::feature,
                native_interactive_smoke::selected,
            ),
            (
                native_repeated_smoke::feature,
                native_repeated_smoke::selected,
            ),
            (
                native_document_smoke::feature,
                native_document_smoke::selected,
            ),
            (
                native_relaunch_smoke::feature,
                native_relaunch_smoke::selected,
            ),
            (elf_document_smoke::feature, elf_document_smoke::selected),
            (elf_relaunch_smoke::feature, elf_relaunch_smoke::selected),
            (
                elf_application_smoke::feature,
                elf_application_smoke::selected,
            ),
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected,
            ),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
        ] {
            assert!(selector(&[feature, name]), "higher {name}");
        }
        assert!(!selected(all_kernel_features));
    }

    fn fixture(role: &str) -> (Vec<u8>, String, Vec<u64>) {
        fixture_with_fills(role, 1)
    }

    fn fixture_with_fills(role: &str, fills: usize) -> (Vec<u8>, String, Vec<u64>) {
        let mut instructions: Vec<(Vec<u8>, &str)> = vec![
            (vec![0xeb, 0], "jmp 400002"),
            (vec![0xdb, 0xe3], "fninit"),
            match role {
                "workspace" => (vec![0xd9, 0xe8], "fld1"),
                "editor" => (vec![0xd9, 0xee], "fldz"),
                _ => (vec![0xd9, 0xeb], "fldpi"),
            },
            (vec![0x66, 0x49, 0x0f, 0x6e, 0xc5], "movq %r13,%xmm0"),
        ];
        instructions.extend((0..fills).map(|_| (vec![0xf3, 0x48, 0xab], "rep stos %rax,(%rdi)")));
        let (f3, f4, reads) = match role {
            "workspace" => (2, 21, 22),
            "editor" => (1, 23, 23),
            _ => (1, 8, 8),
        };
        instructions.extend((0..f3).map(|_| (vec![0xcd, 0xf3], "int $0xf3")));
        instructions.extend((0..f4).map(|_| (vec![0xcd, 0xf4], "int $0xf4")));
        instructions
            .extend((0..reads).map(|_| (vec![0x66, 0x49, 0x0f, 0x7e, 0xc2], "movq %xmm0,%r10")));
        if role == "service" {
            instructions.push((vec![0x0f, 0x0b], "ud2"));
        }
        instructions.push((vec![0x0f, 0x0b], "ud2"));
        let mut image = Vec::new();
        let mut listing = String::new();
        let mut faults = Vec::new();
        for (bytes, instruction) in instructions {
            if instruction == "ud2" {
                faults.push(image.len() as u64);
            }
            listing.push_str(&format!(
                "{:x}:\t{}\t{instruction}\n",
                user_entry + image.len() as u64,
                bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
            image.extend(bytes);
        }
        (image, listing, faults)
    }

    #[test]
    fn fixture_inspection_closes_all_bytes_gates_fp_and_branches() {
        for role in role_names {
            let (image, listing, faults) = fixture(role);
            inspect_fixture(&listing, &image, &faults, role).unwrap();
            // GNU objdump pads mnemonics to columns, unlike the compact
            // handwritten fixture above. Both listings describe exact bytes.
            let padded = listing
                .replace("int $", "int    $")
                .replace("movq %", "movq   %")
                .replace("jmp 400002", "jmp    0x400002");
            inspect_fixture(&padded, &image, &faults, role).unwrap();
            inspect_fixture(
                &listing.replace("stos %rax,(%rdi)", "stos %rax,%es:(%rdi)"),
                &image,
                &faults,
                role,
            )
            .unwrap();
            for changed in [
                listing.replacen("jmp 400002", "jmp 500000", 1),
                listing.replacen("jmp 400002", "jmp 400003", 1),
                listing.replacen("jmp 400002", "jmp *%rax", 1),
                listing.replacen("jmp 400002", "call 400002", 1),
                listing.replacen("fninit", "cli", 1),
                listing.replacen("int $0xf4", "int $0xf3", 1),
                listing.replacen("movq %r13,%xmm0", "movq %r13,%xmm1", 1),
                listing.replacen("movq %xmm0,%r10", "movq %xmm0,%rsp", 1),
                listing.replacen("db e3", "db e2", 1),
                listing.lines().skip(1).collect::<Vec<_>>().join("\n"),
                format!("{listing}{}", listing.lines().last().unwrap()),
            ] {
                assert!(
                    inspect_fixture(&changed, &image, &faults, role).is_err(),
                    "{role}: {changed}"
                );
            }
            let mut changed = image.clone();
            changed[2] ^= 1;
            assert!(inspect_fixture(&listing, &changed, &faults, role).is_err());
            assert!(inspect_fixture(&listing, &image, &[], role).is_err());
            assert!(inspect_fixture(&listing, &image, &faults, "foreign").is_err());
        }
    }

    #[test]
    fn fixture_inspection_requires_one_arena_fill_for_each_role() {
        for role in role_names {
            let (image, listing, faults) = fixture_with_fills(role, 1);
            inspect_fixture(&listing, &image, &faults, role).unwrap();
            for fills in [0, 2] {
                // Regenerate addresses, bytes and fault offsets together, so
                // rejection proves the fill invariant rather than a coverage
                // gap, stale branch target or mismatched UD2 marker.
                let (image, listing, faults) = fixture_with_fills(role, fills);
                assert_eq!(
                    inspect_fixture(&listing, &image, &faults, role),
                    Err("mixed fixture must initialize its arena exactly once".into()),
                    "{role}: {fills} fills"
                );
            }
        }
    }

    #[test]
    fn fixture_placement_rejects_partial_overlap_writable_and_zero_tail() {
        let parsed = parsed_elf64 {
            kind: 2,
            machine: 62,
            entry: 0x100000,
            program_headers: vec![elf64_program_header {
                kind: 1,
                flags: 5,
                offset: 0,
                virtual_address: 0x100000,
                physical_address: 0x100000,
                file_size: 0x3000,
                memory_size: 0x3000,
                alignment: 4096,
            }],
            section_headers: vec![elf64_section_header {
                name: ".rodata".into(),
                kind: 1,
                flags: 2,
                address: 0x102000,
                offset: 0x2000,
                size: 4096,
            }],
        };
        fixture_load(&parsed, 0x102100, 0x102200).unwrap();
        let mut changed = parsed.clone();
        let mut alias = changed.program_headers[0];
        alias.virtual_address = 0x102180;
        alias.file_size = 0x100;
        alias.memory_size = 0x100;
        changed.program_headers.push(alias);
        assert!(fixture_load(&changed, 0x102100, 0x102200).is_err());
        let mut changed = parsed.clone();
        let mut alias = changed.section_headers[0].clone();
        alias.address = 0x102080;
        alias.size = 0xc0;
        changed.section_headers.push(alias);
        assert!(fixture_load(&changed, 0x102100, 0x102200).is_err());
        for flags in [0, 3, 6, 7] {
            let mut changed = parsed.clone();
            changed.section_headers[0].flags = flags;
            assert!(fixture_load(&changed, 0x102100, 0x102200).is_err());
        }
        let mut changed = parsed.clone();
        changed.program_headers[0].flags = 7;
        assert!(fixture_load(&changed, 0x102100, 0x102200).is_err());
        let mut changed = parsed.clone();
        changed.program_headers[0].file_size = 0x2180;
        assert!(fixture_load(&changed, 0x102100, 0x102200).is_err());
    }

    #[test]
    fn normal_exclusion_rejects_proof_names_and_each_private_canary() {
        verify_absent("kernel_main T 100000 10", b"normal").unwrap();
        for marker in ["mixed_workspace", "x86_64_mixed_workspace", prefix] {
            assert!(verify_absent(marker, b"").is_err());
            assert!(verify_absent("", marker.as_bytes()).is_err());
        }
        for role in 0..3u64 {
            assert!(verify_absent("", &(0x71716d6978656400u64 + role).to_le_bytes()).is_err());
        }
    }
}
