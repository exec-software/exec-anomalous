//! Unchanged interpreter file replies and contained publication failures.
use super::*;
use linux_loader_entry_artifact::{plan_word_count, stack_end, stack_start};

mod file_reply;

pub(super) const feature: &str = "linux_loader_entry_test";
const events: [&str; 19] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.linux_loader_entry_test.ready",
    "kernel.linux_loader_entry_test.boundary",
    "kernel.linux_loader_entry_test.reply",
    "kernel.linux_loader_entry_test.boundary",
    "kernel.linux_loader_entry_test.reply",
    "kernel.linux_loader_entry_test.boundary",
    "kernel.linux_loader_entry_test.reply",
    "kernel.linux_loader_entry_test.boundary",
    "kernel.linux_loader_entry_test.terminal_copy",
    "kernel.linux_loader_entry_test.file_source",
    "kernel.linux_loader_entry_test.passed",
];
// Seven complete source-import/service lifetimes also run under KVM.
const timeout: Duration = Duration::from_secs(180);
// Local proof budgets, including eight clean service lifetimes and bounded
// actual caller/stage byte witnesses. They are not global path or file limits.
pub(super) const transcript_byte_limit: usize = 512 * 1024;

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|value| {
            matches!(
                *value,
                "fault_test"
                    | "circle_test"
                    | "control_test"
                    | "rite_test"
                    | "ny_vessel_test"
                    | "memory_test"
                    | "interrupt_test"
                    | "supervision_test"
                    | "shell_vessel_test"
                    | "input_wait_test"
                    | "display_vessel_test"
                    | "supervised_renderer_test"
                    | "semantic_surface_transport_test"
                    | "semantic_surface_presentation_test"
                    | "desktop_dev"
            )
        })
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!(
        "Linux loader-entry image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let root = repository_root()?;
    let build = root.join("build");
    let logs = [
        build.join("qemu_linux_loader_entry_96M.log"),
        build.join("qemu_linux_loader_entry_320M.log"),
    ];
    for log in &logs {
        remove_optional_file(log)?;
    }
    build_and_inspect_normal_image()?;
    let image = self::build()?;
    let (plan, source) = linux_loader_entry_artifact::retained_plan(&root)?;
    let file_source = linux_loader_entry_artifact::retained_file_source(&root)?;
    let file_faults = linux_loader_entry_artifact::retained_file_faults(&root)?;
    let mut evidence = Vec::new();
    for (memory, log) in ["96M", "320M"].into_iter().zip(logs) {
        let mut command = qemu_command_with_memory(&image.path, memory)?;
        command.args([
            "-cpu",
            "Nehalem",
            "-display",
            "none",
            "-monitor",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-fw_cfg",
        ]);
        command.arg(format!(
            "name=opt/anomalous-linux-source,file={}",
            build
                .join("linux-loader-entry/source.bin")
                .display()
                .to_string()
                .replace(',', ",,")
        ));
        command.arg("-fw_cfg").arg(format!(
            "name={},file={}",
            linux_loader_entry_artifact::file_source_name,
            build
                .join("linux-loader-entry/file-source.bin")
                .display()
                .to_string()
                .replace(',', ",,")
        ));
        println!("Linux loader-entry QEMU RAM: {memory}");
        let mut observed = None;
        run_qemu_with_transcript(command, timeout, |status, transcript| {
            // Retain attempted output even when its oracle fails. It must never
            // be substituted for a successful transcript in later qualification.
            fs::write(&log, transcript).map_err(|e| e.to_string())?;
            if status.code() != Some(qemu_success_status) {
                return Err(format!("loader entry QEMU status was {:?}", status.code()));
            }
            observed = Some(verify_records(
                transcript,
                &plan,
                &source,
                &file_source,
                file_faults,
            )?);
            Ok(())
        })?;
        evidence.push(observed.ok_or("loader entry emitted no verified evidence")?);
    }
    let delta = (320u64 - 96) * 1024 * 1024 / memory_frame_size;
    if evidence[0].total_frames != memory_total_frames
        || evidence[1].total_frames != memory_total_frames
        || evidence[1].free_frames.checked_sub(evidence[0].free_frames) != Some(delta)
        || evidence[0]
            .unavailable_frames
            .checked_sub(evidence[1].unavailable_frames)
            != Some(delta)
    {
        return Err("loader entry96/320MiB frame delta changed".into());
    }
    println!(
        "Unchanged interpreter file open/read/pread64/fstat, sequence11 mmap return, fresh request observation and post-resume containment verified at 96/320 MiB; no policy is executed for the observed next request, with no libc startup or true completion claim"
    );
    Ok(())
}

pub(super) fn verify_transcript(transcript: &str) -> Result<(), String> {
    let root = repository_root()?;
    let (plan, source) = linux_loader_entry_artifact::retained_plan(&root)?;
    let file_source = linux_loader_entry_artifact::retained_file_source(&root)?;
    let file_faults = linux_loader_entry_artifact::retained_file_faults(&root)?;
    verify_records(transcript, &plan, &source, &file_source, file_faults).map(|_| ())
}

#[derive(Clone, Copy)]
struct run_evidence {
    total_frames: u64,
    free_frames: u64,
    unavailable_frames: u64,
}

fn verify_records(
    transcript: &str,
    plan: &[u64; plan_word_count],
    source: &[u8],
    file_source: &[u8],
    file_faults: [u64; 2],
) -> Result<run_evidence, String> {
    file_reply::verify(transcript, plan, source, file_source, file_faults)
}

// Keep the earlier strict source-read oracle and its hostile fixtures as a
// regression of the preserved proof phase; it cannot accept the new profile.
#[cfg(test)]
fn verify_source_records(
    transcript: &str,
    plan: &[u64; plan_word_count],
    source: &[u8],
    file_source: &[u8],
) -> Result<run_evidence, String> {
    if transcript.len() > transcript_byte_limit {
        return Err("loader transcript exceeds64KiB budget".into());
    }
    let mut records = Vec::new();
    for line in transcript.lines() {
        if line.len() > 4096 {
            return Err("loader transcript line exceeds4096-byte budget".into());
        }
        if line.trim().is_empty() {
            continue;
        }
        let event = events
            .get(records.len())
            .ok_or("loader transcript has extra records")?;
        let record = json_parser::new(line.trim()).parse_record()?;
        require_string(&record, event, "event", event)?;
        require_string(&record, event, "level", "info")?;
        require_number(&record, event, "schema", 1)?;
        records.push(record);
    }
    if records.len() != events.len() {
        return Err("loader transcript is incomplete".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    let ready = &records[8];
    let ready_numbers = [
        ("tasks", 3),
        ("regions", 9),
        ("user_pages", 88),
        ("source_bytes", plan[2]),
        ("source_fnv64", plan[3]),
        ("file_source_bytes", file_source.len() as u64),
        (
            "file_source_fnv64",
            linux_loader_entry_artifact::fnv64(file_source),
        ),
        ("file_source_pages", file_source.len().div_ceil(4096) as u64),
        ("initial_rip", plan[4]),
        ("initial_rsp", plan[5]),
        ("main_entry", plan[12]),
        ("main_phdr", plan[13]),
        ("interpreter_bias", plan[14]),
        ("break_start", plan[15]),
    ];
    let fields = ready_numbers
        .iter()
        .map(|(name, _)| *name)
        .chain(["source_kind", "policy", "cpu"])
        .collect::<Vec<_>>();
    require_control_header(ready, events[8], &fields)?;
    for (name, value) in ready_numbers {
        require_number(ready, events[8], name, value)?;
    }
    require_string(ready, events[8], "source_kind", "privileged_fw_cfg")?;
    require_string(ready, events[8], "policy", "userspace")?;
    if !["GenuineIntel", "AuthenticAMD", "HygonGenuine"]
        .iter()
        .any(|vendor| require_string(ready, events[8], "cpu", vendor).is_ok())
    {
        return Err("unqualified loader CPU vendor".into());
    }
    if source.len() as u64 != plan[2] || linux_loader_entry_artifact::fnv64(source) != plan[3] {
        return Err("oracle source differs from retained plan".into());
    }
    verify_boundary(&records[9], plan, source, 1)?;
    verify_reply(&records[10], &records[9], plan, source, 1)?;
    verify_boundary(&records[11], plan, source, 2)?;
    verify_reply(&records[12], &records[11], plan, source, 2)?;
    verify_boundary(&records[13], plan, source, 3)?;
    verify_reply(&records[14], &records[13], plan, source, 3)?;
    verify_boundary(&records[15], plan, source, 4)?;
    let terminal_bytes = verify_terminal_copy(&records[16], &records[15], plan)?;
    let file_attempts = verify_file_source(&records[17], file_source)?;
    let passed = &records[18];
    require_control_header(
        passed,
        events[18],
        &[
            "raw_calls",
            "caller_returns",
            "native_calls",
            "retired_tasks",
            "copy_attempts",
            "copy_successes",
            "copy_rejections",
            "copy_scratch_cleared",
            "copy_journal_cleared",
            "source_cleared",
            "tls_cleared",
            "cpu_restored",
            "resources_restored",
            "request_cleared",
        ],
    )?;
    for (name, value) in [
        ("raw_calls", 4),
        ("caller_returns", 3),
        ("native_calls", 65 + terminal_bytes + file_attempts),
        ("retired_tasks", 3),
        ("copy_attempts", 54 + terminal_bytes),
        ("copy_successes", 36 + terminal_bytes),
        ("copy_rejections", 18),
    ] {
        require_number(passed, events[18], name, value)?;
    }
    for name in [
        "copy_scratch_cleared",
        "copy_journal_cleared",
        "source_cleared",
        "tls_cleared",
        "cpu_restored",
        "resources_restored",
        "request_cleared",
    ] {
        require_bool(passed, events[18], name, true)?;
    }
    Ok(run_evidence {
        total_frames: required_number(&records[4], events[4], "total_frames")?,
        free_frames: required_number(&records[4], events[4], "free_frames")?,
        unavailable_frames: required_number(&records[4], events[4], "unavailable_frames")?,
    })
}

fn verify_file_source(record: &structured_record, file: &[u8]) -> Result<u64, String> {
    if file.is_empty() || file.len() > linux_loader_entry_artifact::file_source_limit {
        return Err("oracle file source is outside its configured budget".into());
    }
    let length = file.len() as u64;
    let hash = linux_loader_entry_artifact::fnv64(file);
    let chunks = 1 + (length - length.min(17)).div_ceil(4096);
    let numbers = [
        ("sequence", 4),
        ("grant", linux_loader_entry_artifact::file_source_grant),
        ("source_bytes", length),
        ("source_fnv64", hash),
        ("source_pages", length.div_ceil(4096)),
        ("attempts", chunks + 10),
        ("successes", chunks),
        ("rejections", 10),
        ("delivered_bytes", length),
        ("delivered_fnv64", hash),
        ("guest_bytes", length),
        ("guest_fnv64", hash),
        ("guest_chunks", chunks),
    ];
    let flags = ["pending_preserved", "scratch_cleared", "source_cleared"];
    let fields = numbers
        .iter()
        .map(|(name, _)| *name)
        .chain(flags)
        .collect::<Vec<_>>();
    require_control_header(record, events[17], &fields)?;
    for (name, value) in numbers {
        require_number(record, events[17], name, value)?;
    }
    for name in flags {
        require_bool(record, events[17], name, true)?;
    }
    Ok(chunks + 10)
}

fn verify_boundary(
    record: &structured_record,
    plan: &[u64; plan_word_count],
    source: &[u8],
    sequence: u64,
) -> Result<(), String> {
    let event = events[9];
    let raw_fields = [
        "sequence",
        "number",
        "arg0",
        "arg1",
        "arg2",
        "arg3",
        "arg4",
        "arg5",
        "instruction_pointer",
        "syscall_instruction",
        "stack_pointer",
        "rcx",
        "r11",
        "fs_base",
        "result_bits",
    ];
    let fields = raw_fields
        .iter()
        .copied()
        .chain([
            "returned_to_caller",
            "linux_exit_observed",
            "syscall_bytes_verified",
        ])
        .collect::<Vec<_>>();
    require_control_header(record, event, &fields)?;
    for name in raw_fields {
        required_number(record, event, name)?;
    }
    require_number(record, event, "sequence", sequence)?;
    require_number(
        record,
        event,
        "result_bits",
        match sequence {
            1 => plan[15],
            2 | 3 => (-2i64) as u64,
            4 => (-38i64) as u64,
            _ => return Err("unexpected loader boundary sequence".into()),
        },
    )?;
    if sequence == 1 {
        require_number(record, event, "number", 12)?;
        require_number(record, event, "arg0", 0)?;
    } else if sequence == 2 {
        require_number(record, event, "number", 21)?;
        require_number(record, event, "arg1", 4)?;
    } else if matches!(sequence, 3 | 4) {
        require_number(record, event, "number", 257)?;
        // openat's dirfd and flags are Linux ints. High register bits and the
        // mode argument (without creation flags) do not participate in policy.
        if required_number(record, event, "arg0")? as u32 != (-100i32) as u32
            || required_number(record, event, "arg2")? as u32 != 0x80000
        {
            return Err("pathname openat is not AT_FDCWD with O_RDONLY|O_CLOEXEC".into());
        }
    }
    require_number(record, event, "fs_base", 0)?;
    require_bool(record, event, "returned_to_caller", sequence < 4)?;
    require_bool(record, event, "linux_exit_observed", false)?;
    require_bool(record, event, "syscall_bytes_verified", true)?;
    verify_boundary_hardware(record, plan, source)
}

fn verify_boundary_hardware(
    record: &structured_record,
    plan: &[u64; plan_word_count],
    source: &[u8],
) -> Result<(), String> {
    let event = events[9];
    let instruction = required_number(record, event, "syscall_instruction")?;
    let returned = required_number(record, event, "instruction_pointer")?;
    let stack = required_number(record, event, "stack_pointer")?;
    let flags = required_number(record, event, "r11")?;
    if instruction.checked_add(2) != Some(returned)
        || returned != required_number(record, event, "rcx")?
        || instruction < plan[72]
        || returned >= plan[73]
        || !(stack_start..stack_end).contains(&stack)
        || flags & 0x202 != 0x202
        || flags & !0x0025_0fd7 != 0
    {
        return Err("loader raw SYSCALL hardware relationships/owned extents invalid".into());
    }
    let spans = plan[43..70]
        .chunks_exact(3)
        .filter(|span| {
            span[0] <= instruction
                && span[0]
                    .checked_add(span[2])
                    .is_some_and(|end| returned <= end)
        })
        .collect::<Vec<_>>();
    if spans.len() != 1 {
        return Err("syscall does not have one initialized source span".into());
    }
    let offset = spans[0][1]
        .checked_add(instruction - spans[0][0])
        .ok_or("syscall source offset overflow")?;
    let offset = usize::try_from(offset).map_err(|_| "syscall source offset does not fit host")?;
    if source.get(
        offset
            ..offset
                .checked_add(2)
                .ok_or("syscall source extent overflow")?,
    ) != Some(&[0x0f, 0x05])
    {
        return Err("reported loader instruction is not0f05 in unchanged held source".into());
    }
    Ok(())
}

// This proof-only serial encoding is explicitly 22 little-endian words. It is
// neither Rust-layout deserialization nor a new stable native wire format.
fn reply_frame(record: &structured_record, field: &str) -> Result<[u64; 22], String> {
    let value = required_string(record, events[10], field)?;
    if value.len() != 352
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "loader reply {field} is not canonical 176-byte lowercase hex"
        ));
    }
    let mut frame = [0; 22];
    for (word, digits) in frame.iter_mut().zip(value.as_bytes().chunks_exact(16)) {
        let mut bytes = [0; 8];
        for (byte, pair) in bytes.iter_mut().zip(digits.chunks_exact(2)) {
            let nibble = |digit| {
                if digit <= b'9' {
                    digit - b'0'
                } else {
                    digit - b'a' + 10
                }
            };
            *byte = nibble(pair[0]) * 16 + nibble(pair[1]);
        }
        *word = u64::from_le_bytes(bytes);
    }
    Ok(frame)
}

fn verify_reply(
    record: &structured_record,
    boundary: &structured_record,
    plan: &[u64; plan_word_count],
    source: &[u8],
    sequence: u64,
) -> Result<(), String> {
    let event = events[10];
    require_control_header(
        record,
        event,
        &[
            "sequence",
            "result_bits",
            "frame_before_hex",
            "frame_after_hex",
            "committed",
            "copy_sequence",
            "copy_address",
            "copy_bytes",
            "copy_hex",
        ],
    )?;
    let result = match sequence {
        1 => plan[15],
        2 | 3 => (-2i64) as u64,
        _ => return Err("unexpected loader reply sequence".into()),
    };
    require_number(record, event, "sequence", sequence)?;
    require_number(record, event, "result_bits", result)?;
    verify_copy(record, boundary, plan, source, sequence)?;
    require_bool(record, event, "committed", true)?;
    let before = reply_frame(record, "frame_before_hex")?;
    let after = reply_frame(record, "frame_after_hex")?;
    verify_frame_boundary(&before, boundary)?;
    if after[0] != result || before[1..] != after[1..] {
        return Err("loader reply must preserve the admitted caller frame except RAX".into());
    }
    Ok(())
}

fn verify_frame_boundary(frame: &[u64; 22], boundary: &structured_record) -> Result<(), String> {
    // Correlate every exposed hardware/request word, including all full-width
    // raw arguments even when userspace policy ignores their high bits.
    for (index, field) in [
        (0, "number"),
        (2, "rcx"),
        (3, "arg2"),
        (5, "arg1"),
        (6, "arg0"),
        (7, "arg4"),
        (8, "arg5"),
        (9, "arg3"),
        (10, "r11"),
        (17, "instruction_pointer"),
        (20, "stack_pointer"),
    ] {
        if frame[index] != required_number(boundary, events[9], field)? {
            return Err(format!("loader frame differs from its boundary {field}"));
        }
    }
    if frame[15] != 0x100
        || frame[16] != 0
        || frame[18] != 0x33
        || frame[21] != 0x2b
        || frame[19] != frame[10]
    {
        return Err("loader frame vector/selectors/flags differ".into());
    }
    Ok(())
}

#[cfg(test)]
fn verify_terminal_copy(
    record: &structured_record,
    boundary: &structured_record,
    plan: &[u64; plan_word_count],
) -> Result<u64, String> {
    let event = events[16];
    require_control_header(
        record,
        event,
        &[
            "sequence",
            "frame_before_hex",
            "frame_after_hex",
            "copy_sequence",
            "copy_address",
            "copy_bytes",
            "copy_hex",
            "copy_attempts",
            "copy_successes",
            "copy_rejections",
            "pending_preserved",
        ],
    )?;
    require_number(boundary, events[9], "sequence", 4)?;
    require_number(record, event, "sequence", 4)?;
    require_number(record, event, "copy_sequence", 4)?;
    require_bool(record, event, "pending_preserved", true)?;
    let before = reply_frame(record, "frame_before_hex")?;
    let after = reply_frame(record, "frame_after_hex")?;
    verify_frame_boundary(&before, boundary)?;
    if before != after {
        return Err("terminal copy changed the suspended caller frame".into());
    }
    let length = required_number(record, event, "copy_bytes")?;
    if !(1..=256).contains(&length) {
        return Err("terminal pathname escaped the per-invocation copy budget".into());
    }
    require_number(record, event, "copy_attempts", 6 + length)?;
    require_number(record, event, "copy_successes", length)?;
    require_number(record, event, "copy_rejections", 6)?;
    let address = required_number(boundary, events[9], "arg1")?;
    require_number(record, event, "copy_address", address)?;
    let end = address
        .checked_add(length)
        .ok_or("terminal pathname guest extent overflow")?;
    // This source is mutable task memory. Verify its admitted ownership geometry
    // independently, without comparing bytes to the original source snapshot.
    if plan[10..12] != [stack_start, stack_end]
        || plan[40..43] != [stack_start, (stack_end - stack_start) / 4096, 3]
        || address < stack_start
        || end > stack_end
    {
        return Err("terminal pathname is outside the exact owned RW/NX stack".into());
    }
    let mut covering_region = None;
    for (index, region) in plan[16..43].chunks_exact(3).enumerate() {
        let region_end = region[1]
            .checked_mul(4096)
            .and_then(|bytes| region[0].checked_add(bytes))
            .ok_or("terminal pathname mapping extent overflow")?;
        if region[1] != 0
            && stack_start < region_end
            && region[0] < stack_end
            && (index != 8 || covering_region.replace(index).is_some())
        {
            return Err("terminal pathname has overlapping or foreign mappings".into());
        }
    }
    if covering_region != Some(8) {
        return Err("terminal pathname lacks its admitted stack mapping".into());
    }
    let hex = required_string(record, event, "copy_hex")?;
    if hex.len() != length as usize * 2
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("terminal pathname is not canonical length-bound lowercase hex".into());
    }
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        let nul = pair == b"00";
        if nul != (index + 1 == length as usize) {
            return Err("terminal pathname did not stop at its first NUL".into());
        }
    }
    Ok(length)
}

// The copied journal is evidence from successful suspended reads, not an
// authority source. Independently derive all bytes from the retained source and
// its unique read-only mapping; the fixture declares exactly this absent name.
fn verify_copy(
    record: &structured_record,
    boundary: &structured_record,
    plan: &[u64; plan_word_count],
    source: &[u8],
    sequence: u64,
) -> Result<(), String> {
    let event = events[10];
    if sequence == 1 {
        for name in ["copy_sequence", "copy_address", "copy_bytes"] {
            require_number(record, event, name, 0)?;
        }
        return require_string(record, event, "copy_hex", "");
    }
    let (argument, expected): (&str, &[u8]) = match sequence {
        2 => ("arg0", b"/etc/ld.so.preload\0"),
        3 => ("arg1", b"/etc/ld.so.cache\0"),
        _ => return Err("unexpected copied pathname sequence".into()),
    };
    let address = required_number(boundary, events[9], argument)?;
    let end = address
        .checked_add(expected.len() as u64)
        .ok_or("copied pathname guest extent overflow")?;
    require_number(record, event, "copy_sequence", sequence)?;
    require_number(record, event, "copy_address", address)?;
    require_number(record, event, "copy_bytes", expected.len() as u64)?;
    let mut covering_region = None;
    for region in plan[16..43].chunks_exact(3) {
        let region_end = region[1]
            .checked_mul(4096)
            .and_then(|bytes| region[0].checked_add(bytes))
            .ok_or("copied pathname mapping extent overflow")?;
        if region[1] != 0
            && address < region_end
            && region[0] < end
            && (region[2] != 1
                || region[0] > address
                || end > region_end
                || covering_region.replace(region).is_some())
        {
            return Err(
                "copied pathname lacks one complete read-only non-executable mapping".into(),
            );
        }
    }
    if covering_region.is_none() {
        return Err("copied pathname is outside retained mappings".into());
    }
    let mut covering_span = None;
    for span in plan[43..70].chunks_exact(3) {
        let span_end = span[0]
            .checked_add(span[2])
            .ok_or("copied pathname initialized extent overflow")?;
        if span[2] != 0
            && address < span_end
            && span[0] < end
            && (span[0] > address || end > span_end || covering_span.replace(span).is_some())
        {
            return Err("copied pathname lacks one complete initialized source span".into());
        }
    }
    let span = covering_span.ok_or("copied pathname is outside retained initialized source")?;
    let offset = span[1]
        .checked_add(address - span[0])
        .ok_or("copied pathname source offset overflow")?;
    let offset = usize::try_from(offset).map_err(|_| "copied pathname offset does not fit host")?;
    let source_end = offset
        .checked_add(expected.len())
        .ok_or("copied pathname source extent overflow")?;
    let bytes = source
        .get(offset..source_end)
        .ok_or("copied pathname source extent escaped held bytes")?;
    if bytes != expected {
        return Err("copied pathname differs from the exact declared absent namespace name".into());
    }
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    require_string(record, event, "copy_hex", &hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) const test_file_source: &[u8] = &[0x37; 8193];

    fn verify_records(
        transcript: &str,
        plan: &[u64; plan_word_count],
        source: &[u8],
    ) -> Result<run_evidence, String> {
        super::verify_source_records(transcript, plan, source, test_file_source)
    }

    const test_terminal_path: &[u8] = b"\xfflive.bin\0";

    pub(super) fn fixture() -> (String, [u64; plan_word_count], Vec<u8>) {
        let mut plan = [0; plan_word_count];
        let mut source = vec![
            0x90, 0x0f, 0x05, 0x90, 0x90, 0x0f, 0x05, 0x90, 0x90, 0x0f, 0x05, 0x90, 0x90, 0x0f,
            0x05, 0x90,
        ];
        source.extend_from_slice(b"/etc/ld.so.preload\0");
        source.extend_from_slice(b"/etc/ld.so.cache\0");
        plan[2] = source.len() as u64;
        plan[3] = linux_loader_entry_artifact::fnv64(&source);
        plan[4] = 0x60023340;
        plan[5] = stack_end - 352;
        plan[12] = 0x40002db0;
        plan[13] = 0x40000040;
        plan[14] = 0x60000000;
        plan[15] = 0x40009000;
        plan[10..12].copy_from_slice(&[stack_start, stack_end]);
        plan[40..43].copy_from_slice(&[stack_start, (stack_end - stack_start) / 4096, 3]);
        plan[16..19].copy_from_slice(&[0x60001000, 1, 5]);
        plan[19..22].copy_from_slice(&[0x60003000, 1, 1]);
        plan[43..46].copy_from_slice(&[0x60001000, 0, 16]);
        plan[46..49].copy_from_slice(&[0x60003000, 16, 19]);
        plan[49..52].copy_from_slice(&[0x60003100, 35, 17]);
        plan[72..74].copy_from_slice(&[0x60001000, 0x60001010]);
        let boot = [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
        ].join("\n");
        let ready = format!(
            r#"{{"schema":1,"event":"{}","level":"info","tasks":3,"regions":9,"user_pages":88,"source_bytes":{},"source_fnv64":{},"initial_rip":{},"initial_rsp":{},"main_entry":{},"main_phdr":{},"interpreter_bias":{},"break_start":{},"file_source_bytes":8193,"file_source_fnv64":{},"file_source_pages":3,"source_kind":"privileged_fw_cfg","policy":"userspace","cpu":"GenuineIntel"}}"#,
            events[8],
            plan[2],
            plan[3],
            plan[4],
            plan[5],
            plan[12],
            plan[13],
            plan[14],
            plan[15],
            linux_loader_entry_artifact::fnv64(test_file_source)
        );
        let mut records = vec![boot, ready];
        for (sequence, number, arg0, arg1, arg2, rip, stack, flags, result) in [
            (
                1,
                12,
                0,
                17,
                29,
                0x60001003,
                stack_end - 1024,
                514,
                plan[15],
            ),
            (
                2,
                21,
                0x60003000,
                4,
                29,
                0x60001007,
                stack_end - 1064,
                582,
                (-2i64) as u64,
            ),
            (
                3,
                257,
                0xffffff9c,
                0x60003100,
                0x80000,
                0x6000100b,
                stack_end - 1128,
                582,
                (-2i64) as u64,
            ),
            (
                4,
                257,
                0xffffff9c,
                stack_end - 64,
                0x80000,
                0x6000100f,
                stack_end - 1200,
                582,
                (-38i64) as u64,
            ),
        ] {
            records.push(format!(
                r#"{{"schema":1,"event":"{}","level":"info","sequence":{sequence},"number":{number},"arg0":{arg0},"arg1":{arg1},"arg2":{arg2},"arg3":31,"arg4":43,"arg5":59,"instruction_pointer":{rip},"syscall_instruction":{},"stack_pointer":{stack},"rcx":{rip},"r11":{flags},"fs_base":0,"result_bits":{result},"returned_to_caller":{},"linux_exit_observed":false,"syscall_bytes_verified":true}}"#,
                events[9], rip - 2, sequence < 4
            ));
            let mut before =
                core::array::from_fn::<_, 22, _>(|index| 0xfeed0000 + sequence * 32 + index as u64);
            for (index, value) in [
                (0, number),
                (2, rip),
                (3, arg2),
                (5, arg1),
                (6, arg0),
                (7, 43),
                (8, 59),
                (9, 31),
                (10, flags),
                (15, 0x100),
                (16, 0),
                (17, rip),
                (18, 0x33),
                (19, flags),
                (20, stack),
                (21, 0x2b),
            ] {
                before[index] = value;
            }
            if sequence == 4 {
                let length = test_terminal_path.len();
                let hex = test_terminal_path
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                records.push(format!(
                    r#"{{"schema":1,"event":"{}","level":"info","sequence":4,"frame_before_hex":"{}","frame_after_hex":"{}","copy_sequence":4,"copy_address":{arg1},"copy_bytes":{length},"copy_hex":"{hex}","copy_attempts":{},"copy_successes":{length},"copy_rejections":6,"pending_preserved":true}}"#,
                    events[16], frame_hex(&before), frame_hex(&before), 6 + length
                ));
                continue;
            }
            let mut after = before;
            after[0] = result;
            let (copy_sequence, copy_address, copy_bytes, copy_hex) = match sequence {
                1 => (0, 0, 0, ""),
                2 => (2, 0x60003000, 19, "2f6574632f6c642e736f2e7072656c6f616400"),
                3 => (3, 0x60003100, 17, "2f6574632f6c642e736f2e636163686500"),
                _ => unreachable!(),
            };
            records.push(format!(
                r#"{{"schema":1,"event":"{}","level":"info","sequence":{sequence},"result_bits":{result},"frame_before_hex":"{}","frame_after_hex":"{}","committed":true,"copy_sequence":{copy_sequence},"copy_address":{copy_address},"copy_bytes":{copy_bytes},"copy_hex":"{copy_hex}"}}"#,
                events[10], frame_hex(&before), frame_hex(&after)
            ));
        }
        let hash = linux_loader_entry_artifact::fnv64(test_file_source);
        records.push(format!(
            r#"{{"schema":1,"event":"{}","level":"info","sequence":4,"grant":{},"source_bytes":8193,"source_fnv64":{hash},"source_pages":3,"attempts":13,"successes":3,"rejections":10,"delivered_bytes":8193,"delivered_fnv64":{hash},"guest_bytes":8193,"guest_fnv64":{hash},"guest_chunks":3,"pending_preserved":true,"scratch_cleared":true,"source_cleared":true}}"#,
            events[17], linux_loader_entry_artifact::file_source_grant
        ));
        records.push(format!(
            r#"{{"schema":1,"event":"{}","level":"info","raw_calls":4,"caller_returns":3,"native_calls":88,"retired_tasks":3,"copy_attempts":64,"copy_successes":46,"copy_rejections":18,"copy_scratch_cleared":true,"copy_journal_cleared":true,"source_cleared":true,"tls_cleared":true,"cpu_restored":true,"resources_restored":true,"request_cleared":true}}"#,
            events[18]
        ));
        (records.join("\n"), plan, source)
    }

    fn frame_hex(frame: &[u64; 22]) -> String {
        frame
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    #[test]
    fn requires_exact_order_complete_schema_and_real_initialized_syscall_bytes() {
        let (valid, plan, source) = fixture();
        verify_records(&valid, &plan, &source).unwrap();
        let lines = valid.lines().collect::<Vec<_>>();
        for index in 0..lines.len() {
            let mut changed = lines.clone();
            changed.remove(index);
            assert!(verify_records(&changed.join("\n"), &plan, &source).is_err());
            let mut changed = lines.clone();
            changed.insert(index, lines[index]);
            assert!(verify_records(&changed.join("\n"), &plan, &source).is_err());
            if index + 1 < lines.len() {
                let mut changed = lines.clone();
                changed.swap(index, index + 1);
                assert!(verify_records(&changed.join("\n"), &plan, &source).is_err());
            }
        }
        for extra in ["private output", "{}", lines[9]] {
            assert!(verify_records(&format!("{valid}\n{extra}"), &plan, &source).is_err());
        }
        for (record_index, source_index, sequence) in
            [(9, 1, 1), (11, 5, 2), (13, 9, 3), (15, 13, 4)]
        {
            let mut changed = source.clone();
            changed[source_index] = 0x90;
            let mut new_plan = plan;
            new_plan[3] = linux_loader_entry_artifact::fnv64(&changed);
            let record = json_parser::new(lines[record_index])
                .parse_record()
                .unwrap();
            assert!(verify_boundary(&record, &new_plan, &changed, sequence).is_err());
        }
    }

    #[test]
    fn rejects_unknown_duplicate_missing_and_wrong_typed_fields() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for index in 8..events.len() {
            let record = json_parser::new(&lines[index]).parse_record().unwrap();
            for field in record.keys() {
                for remove in [true, false] {
                    let mut changed = record.clone();
                    if remove {
                        changed.remove(field);
                    } else {
                        changed.insert(field.clone(), json_scalar::null);
                    }
                    let mut edited = lines.clone();
                    edited[index] = render(&changed);
                    assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
                }
            }
            let mut changed = record.clone();
            changed.insert("unexpected".into(), json_scalar::boolean(true));
            let mut edited = lines.clone();
            edited[index] = render(&changed);
            assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
            let mut edited = lines.clone();
            edited[index] = edited[index].replacen('{', "{\"schema\":1,", 1);
            assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        }
    }

    #[test]
    fn file_source_requires_exact_host_bytes_guest_consumption_and_cleanup() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        let record = json_parser::new(&lines[17]).parse_record().unwrap();
        for field in [
            "sequence",
            "grant",
            "source_bytes",
            "source_fnv64",
            "source_pages",
            "attempts",
            "successes",
            "rejections",
            "delivered_bytes",
            "delivered_fnv64",
            "guest_bytes",
            "guest_fnv64",
            "guest_chunks",
        ] {
            let mut changed = record.clone();
            let old = required_number(&record, events[17], field).unwrap();
            changed.insert(field.into(), json_scalar::number((old ^ 1).to_string()));
            assert!(
                verify_file_source(&changed, test_file_source).is_err(),
                "{field}"
            );
        }
        for field in ["pending_preserved", "scratch_cleared", "source_cleared"] {
            let mut changed = record.clone();
            changed.insert(field.into(), json_scalar::boolean(false));
            assert!(
                verify_file_source(&changed, test_file_source).is_err(),
                "{field}"
            );
        }
        let mut corrupt = test_file_source.to_vec();
        corrupt[4096] ^= 1;
        assert!(super::verify_source_records(&valid, &plan, &source, &corrupt).is_err());
        assert!(verify_file_source(&record, &[]).is_err());
        assert!(verify_file_source(&record, &test_file_source[..8192]).is_err());
    }

    #[test]
    fn rejects_frame_confusion_and_false_completion_claims() {
        let (valid, plan, source) = fixture();
        for (from, to) in [
            ("\"native_calls\":88", "\"native_calls\":11"),
            ("\"raw_calls\":4", "\"raw_calls\":3"),
            ("\"caller_returns\":3", "\"caller_returns\":2"),
            ("\"committed\":true", "\"committed\":false"),
            (
                "\"linux_exit_observed\":false",
                "\"linux_exit_observed\":true",
            ),
            (
                "\"returned_to_caller\":false",
                "\"returned_to_caller\":true",
            ),
            ("\"rcx\":1610616835", "\"rcx\":1610616834"),
            ("\"r11\":514", "\"r11\":12802"),
            ("\"fs_base\":0", "\"fs_base\":1"),
            (
                "\"source_kind\":\"privileged_fw_cfg\"",
                "\"source_kind\":\"ELF\"",
            ),
            ("\"cpu_restored\":true", "\"cpu_restored\":false"),
        ] {
            assert!(
                verify_records(&valid.replace(from, to), &plan, &source).is_err(),
                "accepted{from}"
            );
        }
        assert!(verify_records(&"x".repeat(65537), &plan, &source).is_err());
        assert!(verify_records(&format!("{}\n{valid}", " ".repeat(4097)), &plan, &source).is_err());
    }

    #[test]
    fn reply_hex_is_exact_lowercase_little_endian_without_layout_assumptions() {
        let text = format!("0807060504030201{}0000000000000080", "00".repeat(20 * 8));
        let record = json_parser::new(&format!(r#"{{"frame_before_hex":"{text}"}}"#))
            .parse_record()
            .unwrap();
        let words = reply_frame(&record, "frame_before_hex").unwrap();
        assert_eq!(words[0], 0x0102_0304_0506_0708);
        assert_eq!(words[21], 1 << 63);
        assert!(words[1..21].iter().all(|word| *word == 0));
        for invalid in [
            text[..350].to_string(),
            format!("{text}00"),
            format!("AB{}", &text[2..]),
            format!("gg{}", &text[2..]),
            format!("é{}", &text[2..]),
            format!("0x{}", &text[2..]),
        ] {
            let mut changed = record.clone();
            changed.insert("frame_before_hex".into(), json_scalar::string(invalid));
            assert!(reply_frame(&changed, "frame_before_hex").is_err());
        }
    }

    #[test]
    fn rejects_every_changed_reply_byte_and_correlates_each_before_frame() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for reply_index in [10, 12, 14] {
            let reply = json_parser::new(&lines[reply_index])
                .parse_record()
                .unwrap();
            let before = reply_frame(&reply, "frame_before_hex").unwrap();
            let after = reply_frame(&reply, "frame_after_hex").unwrap();
            for field in ["frame_before_hex", "frame_after_hex"] {
                for byte in 0..176 {
                    let mut words = if field == "frame_before_hex" {
                        before
                    } else {
                        after
                    };
                    words[byte / 8] ^= 1 << (byte % 8 * 8);
                    let mut changed = reply.clone();
                    changed.insert(field.into(), json_scalar::string(frame_hex(&words)));
                    let mut edited = lines.clone();
                    edited[reply_index] = render(&changed);
                    assert!(
                        verify_records(&edited.join("\n"), &plan, &source).is_err(),
                        "{field} byte {byte}"
                    );
                }
            }
            // Mutating both snapshots equally must not conceal a different
            // invocation or an invalid hardware frame behind a preservation claim.
            for index in [0, 2, 3, 5, 6, 7, 8, 9, 10, 15, 16, 17, 18, 19, 20, 21] {
                let (mut changed_before, mut changed_after) = (before, after);
                changed_before[index] ^= 1;
                changed_after[index] ^= 1;
                let mut changed = reply.clone();
                changed.insert(
                    "frame_before_hex".into(),
                    json_scalar::string(frame_hex(&changed_before)),
                );
                changed.insert(
                    "frame_after_hex".into(),
                    json_scalar::string(frame_hex(&changed_after)),
                );
                let mut edited = lines.clone();
                edited[reply_index] = render(&changed);
                assert!(
                    verify_records(&edited.join("\n"), &plan, &source).is_err(),
                    "word {index}"
                );
            }
        }
    }

    #[test]
    fn requires_three_exact_replies_fresh_fourth_sequence_and_complete_cleanup() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for (index, field, value) in [
            (9, "sequence", 2),
            (9, "number", 13),
            (9, "arg0", 1),
            (9, "result_bits", 0),
            (10, "sequence", 2),
            (10, "result_bits", 0),
            (11, "sequence", 1),
            (11, "number", 12),
            (11, "arg1", 0),
            (11, "result_bits", plan[15]),
            (12, "sequence", 1),
            (12, "result_bits", 0),
            (13, "sequence", 2),
            (13, "number", 21),
            (13, "arg0", 0),
            (13, "arg2", 0),
            (13, "arg2", 0x80001),
            (13, "arg2", 0x80040),
            (13, "result_bits", 0),
            (14, "sequence", 2),
            (14, "result_bits", 0),
            (15, "sequence", 3),
            (15, "result_bits", (-2i64) as u64),
            (18, "raw_calls", 3),
            (18, "caller_returns", 2),
            (18, "native_calls", 34),
            (18, "retired_tasks", 2),
            (18, "copy_attempts", 25),
            (18, "copy_successes", 19),
            (18, "copy_rejections", 6),
        ] {
            let mut changed = json_parser::new(&lines[index]).parse_record().unwrap();
            changed.insert(field.into(), json_scalar::number(value.to_string()));
            let mut edited = lines.clone();
            edited[index] = render(&changed);
            assert!(
                verify_records(&edited.join("\n"), &plan, &source).is_err(),
                "record {index} field {field}"
            );
        }
        for field in [
            "copy_scratch_cleared",
            "copy_journal_cleared",
            "source_cleared",
            "tls_cleared",
            "cpu_restored",
            "resources_restored",
            "request_cleared",
        ] {
            let mut changed = json_parser::new(&lines[18]).parse_record().unwrap();
            changed.insert(field.into(), json_scalar::boolean(false));
            let mut edited = lines.clone();
            edited[18] = render(&changed);
            assert!(
                verify_records(&edited.join("\n"), &plan, &source).is_err(),
                "{field}"
            );
        }
        for number in [0, 12, 21, 60, 158, u64::MAX] {
            let mut changed = json_parser::new(&lines[15]).parse_record().unwrap();
            changed.insert("number".into(), json_scalar::number(number.to_string()));
            let mut edited = lines.clone();
            edited[15] = render(&changed);
            assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        }
    }

    #[test]
    fn openat_uses_linux_int_arguments_and_ignores_unused_mode_but_binds_full_frames() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for (boundary_index, observation_index) in [(13, 14), (15, 16)] {
            for (field, word, value) in [
                ("arg0", 6, 0x12345678ffffff9c),
                ("arg0", 6, (-100i64) as u64),
                ("arg2", 3, 0xaabbccdd00080000),
                ("arg3", 9, u64::MAX),
            ] {
                let mut boundary = json_parser::new(&lines[boundary_index])
                    .parse_record()
                    .unwrap();
                let mut reply = json_parser::new(&lines[observation_index])
                    .parse_record()
                    .unwrap();
                boundary.insert(field.into(), json_scalar::number(value.to_string()));
                let mut edited = lines.clone();
                edited[boundary_index] = render(&boundary);
                // Even ignored policy bits must still correlate with the raw frame.
                assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
                for name in ["frame_before_hex", "frame_after_hex"] {
                    let mut frame = reply_frame(&reply, name).unwrap();
                    frame[word] = value;
                    reply.insert(name.into(), json_scalar::string(frame_hex(&frame)));
                }
                edited[observation_index] = render(&reply);
                verify_records(&edited.join("\n"), &plan, &source).unwrap();
            }
            for (field, word, value) in [
                ("number", 0, 21),
                ("arg0", 6, 0),
                ("arg0", 6, 0xffffff9d),
                ("arg2", 3, 0),
                ("arg2", 3, 0x80001),
                ("arg2", 3, 0x80040),
            ] {
                let mut boundary = json_parser::new(&lines[boundary_index])
                    .parse_record()
                    .unwrap();
                let mut observation = json_parser::new(&lines[observation_index])
                    .parse_record()
                    .unwrap();
                boundary.insert(field.into(), json_scalar::number(value.to_string()));
                for name in ["frame_before_hex", "frame_after_hex"] {
                    let mut frame = reply_frame(&observation, name).unwrap();
                    // A committed reply has a result in RAX; a terminal copy
                    // preserves the request number in both complete snapshots.
                    if word != 0 || name == "frame_before_hex" || observation_index == 16 {
                        frame[word] = value;
                    }
                    observation.insert(name.into(), json_scalar::string(frame_hex(&frame)));
                }
                let mut edited = lines.clone();
                edited[boundary_index] = render(&boundary);
                edited[observation_index] = render(&observation);
                assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
            }
        }
    }

    #[test]
    fn both_copied_paths_require_every_byte_and_terminator_from_successful_reads() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for (reply_index, sequence, offset, length, hex) in [
            (12, 2, 16, 19, "2f6574632f6c642e736f2e7072656c6f616400"),
            (14, 3, 35, 17, "2f6574632f6c642e736f2e636163686500"),
        ] {
            let reply = json_parser::new(&lines[reply_index])
                .parse_record()
                .unwrap();
            let boundary = json_parser::new(&lines[reply_index - 1])
                .parse_record()
                .unwrap();
            let original = required_string(&reply, events[10], "copy_hex").unwrap();
            assert_eq!(original, hex);
            for byte in 0..length {
                let mut changed = reply.clone();
                let mut hex = original.as_bytes().to_vec();
                hex[byte * 2 + 1] = if hex[byte * 2 + 1] == b'0' {
                    b'1'
                } else {
                    b'0'
                };
                changed.insert(
                    "copy_hex".into(),
                    json_scalar::string(String::from_utf8(hex).unwrap()),
                );
                assert!(
                    verify_copy(&changed, &boundary, &plan, &source, sequence).is_err(),
                    "sequence {sequence} copied byte {byte}"
                );
                let mut changed_source = source.clone();
                changed_source[offset + byte] ^= 1;
                let mut changed_plan = plan;
                changed_plan[3] = linux_loader_entry_artifact::fnv64(&changed_source);
                let changed_hex = changed_source[offset..offset + length]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                changed.insert("copy_hex".into(), json_scalar::string(changed_hex));
                // Coherent journal/source corruption still cannot rename the
                // explicitly declared absent namespace entry or omit its NUL.
                assert!(
                    verify_copy(
                        &changed,
                        &boundary,
                        &changed_plan,
                        &changed_source,
                        sequence
                    )
                    .is_err(),
                    "sequence {sequence} source byte {byte}"
                );
            }
            for value in [
                original[..original.len() - 2].to_owned(),
                format!("{original}00"),
                original.to_uppercase(),
                format!("0x{original}"),
                format!("gg{}", &original[2..]),
            ] {
                let mut changed = reply.clone();
                changed.insert("copy_hex".into(), json_scalar::string(value));
                assert!(verify_copy(&changed, &boundary, &plan, &source, sequence).is_err());
            }
            for (field, value) in [
                ("copy_sequence", 0),
                ("copy_sequence", sequence - 1),
                ("copy_sequence", sequence + 1),
                ("copy_address", 0),
                ("copy_bytes", length as u64 - 1),
                ("copy_bytes", length as u64 + 1),
            ] {
                let mut changed = reply.clone();
                changed.insert(field.into(), json_scalar::number(value.to_string()));
                assert!(verify_copy(&changed, &boundary, &plan, &source, sequence).is_err());
            }
        }
        for field in ["copy_sequence", "copy_address", "copy_bytes", "copy_hex"] {
            let mut reply = json_parser::new(&lines[10]).parse_record().unwrap();
            let value = if field == "copy_hex" {
                json_scalar::string("00".into())
            } else {
                json_scalar::number("1".into())
            };
            reply.insert(field.into(), value);
            let mut edited = lines.clone();
            edited[10] = render(&reply);
            assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        }
    }

    #[test]
    fn rejects_stale_reused_and_cross_path_copy_journals() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for (destination, other, sequence) in [(12, 14, 2), (14, 12, 3)] {
            let original = json_parser::new(&lines[destination])
                .parse_record()
                .unwrap();
            let reused = json_parser::new(&lines[other]).parse_record().unwrap();
            for reset_sequence in [false, true] {
                let mut reply = original.clone();
                for name in ["copy_sequence", "copy_address", "copy_bytes", "copy_hex"] {
                    reply.insert(name.into(), reused[name].clone());
                }
                if reset_sequence {
                    reply.insert(
                        "copy_sequence".into(),
                        json_scalar::number(sequence.to_string()),
                    );
                }
                let mut edited = lines.clone();
                edited[destination] = render(&reply);
                assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
                // Even changing the correlated raw pathname pointer to the
                // other RO name cannot swap the two declarations.
                let arg = if sequence == 2 { "arg0" } else { "arg1" };
                let mut boundary = json_parser::new(&lines[destination - 1])
                    .parse_record()
                    .unwrap();
                boundary.insert(arg.into(), reused["copy_address"].clone());
                assert!(verify_copy(&reply, &boundary, &plan, &source, sequence).is_err());
            }
        }
    }

    #[test]
    fn both_paths_require_unique_readonly_initialized_extents_with_checked_arithmetic() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().collect::<Vec<_>>();
        for (reply_index, sequence, argument, span_index, address, length, source_at) in [
            (12, 2, "arg0", 46, 0x60003000u64, 19u64, 16),
            (14, 3, "arg1", 49, 0x60003100u64, 17u64, 35),
        ] {
            let reply = json_parser::new(lines[reply_index]).parse_record().unwrap();
            let boundary = json_parser::new(lines[reply_index - 1])
                .parse_record()
                .unwrap();
            for (word, value) in [
                (19, address + 1),
                (20, 0),
                (20, u64::MAX),
                (21, 0),
                (21, 3),
                (21, 5),
                (21, 7),
                (span_index, address + 1),
                (span_index, u64::MAX - 4),
                (span_index + 1, source.len() as u64),
                (span_index + 1, u64::MAX),
                (span_index + 2, length - 1),
                (span_index + 2, u64::MAX),
            ] {
                let mut changed = plan;
                changed[word] = value;
                assert!(
                    verify_copy(&reply, &boundary, &changed, &source, sequence).is_err(),
                    "sequence {sequence} plan {word}"
                );
            }
            for partial in [false, true] {
                let mut changed = plan;
                changed[22..25].copy_from_slice(&[address + u64::from(partial), 1, 1]);
                assert!(verify_copy(&reply, &boundary, &changed, &source, sequence).is_err());
                let mut changed = plan;
                changed[52..55].copy_from_slice(&[address + u64::from(partial), source_at, length]);
                assert!(verify_copy(&reply, &boundary, &changed, &source, sequence).is_err());
            }
            for value in [0, address - 1, address + 1, u64::MAX - 8] {
                let mut changed_boundary = boundary.clone();
                changed_boundary.insert(argument.into(), json_scalar::number(value.to_string()));
                let mut changed_reply = reply.clone();
                changed_reply.insert(
                    "copy_address".into(),
                    json_scalar::number(value.to_string()),
                );
                assert!(
                    verify_copy(&changed_reply, &changed_boundary, &plan, &source, sequence)
                        .is_err()
                );
            }
            // Either name may relocate, including NUL exactly at a page end.
            // An unrelated pathname pointer is never inferred for the fourth call.
            for value in [0x60003800, 0x60004000 - length] {
                let mut changed = plan;
                changed[span_index] = value;
                let mut changed_boundary = boundary.clone();
                changed_boundary.insert(argument.into(), json_scalar::number(value.to_string()));
                let mut changed_reply = reply.clone();
                changed_reply.insert(
                    "copy_address".into(),
                    json_scalar::number(value.to_string()),
                );
                verify_copy(
                    &changed_reply,
                    &changed_boundary,
                    &changed,
                    &source,
                    sequence,
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn all_boundaries_require_unique_checked_source_spans_and_hardware_relationships() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().collect::<Vec<_>>();
        for (index, sequence) in [(9, 1), (11, 2), (13, 3), (15, 4)] {
            let record = json_parser::new(lines[index]).parse_record().unwrap();
            let mut duplicate = plan;
            duplicate[46..49].copy_from_slice(&plan[43..46]);
            assert!(verify_boundary(&record, &duplicate, &source, sequence).is_err());
            for source_offset in [source.len() as u64, u64::MAX] {
                let mut changed = plan;
                changed[44] = source_offset;
                assert!(verify_boundary(&record, &changed, &source, sequence).is_err());
            }
            for (field, value) in [
                ("instruction_pointer", u64::MAX),
                ("syscall_instruction", u64::MAX),
                ("stack_pointer", stack_start - 1),
                ("stack_pointer", stack_end),
                ("rcx", 0),
                ("r11", 2),
                ("r11", 0x3202),
                ("fs_base", 1),
            ] {
                let mut changed = record.clone();
                changed.insert(field.into(), json_scalar::number(value.to_string()));
                assert!(
                    verify_boundary(&changed, &plan, &source, sequence).is_err(),
                    "sequence {sequence} field {field}"
                );
            }
        }
    }

    fn set_terminal_path(lines: &mut [String], bytes: &[u8], address: u64) {
        let mut boundary = json_parser::new(&lines[15]).parse_record().unwrap();
        boundary.insert("arg1".into(), json_scalar::number(address.to_string()));
        lines[15] = render(&boundary);
        let mut terminal = json_parser::new(&lines[16]).parse_record().unwrap();
        for name in ["frame_before_hex", "frame_after_hex"] {
            let mut frame = reply_frame(&terminal, name).unwrap();
            frame[5] = address;
            terminal.insert(name.into(), json_scalar::string(frame_hex(&frame)));
        }
        let length = bytes.len() as u64;
        for (field, value) in [
            ("copy_address", address),
            ("copy_bytes", length),
            ("copy_attempts", 6 + length),
            ("copy_successes", length),
        ] {
            terminal.insert(field.into(), json_scalar::number(value.to_string()));
        }
        terminal.insert(
            "copy_hex".into(),
            json_scalar::string(bytes.iter().map(|byte| format!("{byte:02x}")).collect()),
        );
        lines[16] = render(&terminal);
        let file = json_parser::new(&lines[17]).parse_record().unwrap();
        let source_attempts = verify_file_source(&file, test_file_source).unwrap();
        let mut passed = json_parser::new(&lines[18]).parse_record().unwrap();
        for (field, value) in [
            ("native_calls", 65 + length + source_attempts),
            ("copy_attempts", 54 + length),
            ("copy_successes", 36 + length),
        ] {
            passed.insert(field.into(), json_scalar::number(value.to_string()));
        }
        lines[18] = render(&passed);
    }

    #[test]
    fn terminal_path_is_live_binary_data_with_first_nul_and_derived_counts() {
        let (valid, plan, source) = fixture();
        let mut maximum = (1u8..=u8::MAX).collect::<Vec<_>>();
        maximum.push(0);
        for bytes in [
            vec![0],
            vec![0xff, 0xfe, b'/', 1, 0x80, 0],
            b"/different/runtime/name\0".to_vec(),
            maximum,
        ] {
            for address in [stack_start, stack_end - bytes.len() as u64] {
                let mut lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
                set_terminal_path(&mut lines, &bytes, address);
                // No initialized source span covers any of these stack bytes.
                verify_records(&lines.join("\n"), &plan, &source).unwrap();
            }
        }
        for length in [0, 257] {
            let mut bytes = vec![b'x'; length];
            if let Some(last) = bytes.last_mut() {
                *last = 0;
            }
            let mut lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
            set_terminal_path(&mut lines, &bytes, stack_start);
            assert!(verify_records(&lines.join("\n"), &plan, &source).is_err());
        }
        for index in 0..test_terminal_path.len() {
            let mut bytes = test_terminal_path.to_vec();
            bytes[index] = if index + 1 == bytes.len() { 0xff } else { 0 };
            let mut lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
            set_terminal_path(&mut lines, &bytes, stack_start);
            assert!(verify_records(&lines.join("\n"), &plan, &source).is_err());
        }
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        let terminal = json_parser::new(&lines[16]).parse_record().unwrap();
        let original = required_string(&terminal, events[16], "copy_hex").unwrap();
        for hex in [
            original.to_uppercase(),
            format!("{original}00"),
            original[..original.len() - 2].to_string(),
            original.replacen('f', "g", 1),
        ] {
            let mut changed = terminal.clone();
            changed.insert("copy_hex".into(), json_scalar::string(hex));
            let mut edited = lines.clone();
            edited[16] = render(&changed);
            assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        }
    }

    #[test]
    fn terminal_path_rejects_foreign_mapping_geometry_crossings_and_overflow() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for (word, value) in [
            (10, stack_start - 4096),
            (11, stack_end + 4096),
            (40, stack_start + 4096),
            (41, 0),
            (41, 15),
            (41, 17),
            (42, 0),
            (42, 1),
            (42, 2),
            (42, 5),
            (42, 7),
            (42, u64::MAX),
        ] {
            let mut changed = plan;
            changed[word] = value;
            assert!(verify_records(&valid, &changed, &source).is_err());
        }
        for region in [
            [stack_start, 16, 3],
            // An overlap elsewhere in the admitted stack invalidates unique
            // ownership even when it does not intersect the copied pathname.
            [stack_start, 1, 1],
            [stack_end - 4096, 1, 1],
            [u64::MAX, 1, 3],
            [0, u64::MAX, 3],
        ] {
            let mut changed = plan;
            changed[22..25].copy_from_slice(&region);
            assert!(verify_records(&valid, &changed, &source).is_err());
        }
        for address in [
            0,
            0x60003000,
            stack_start - 1,
            stack_end - 1,
            stack_end,
            u64::MAX,
        ] {
            let mut edited = lines.clone();
            set_terminal_path(&mut edited, &[b'x', 0], address);
            assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        }
    }

    #[test]
    fn terminal_copy_rejects_reused_journals_false_returns_and_counter_claims() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        let terminal = json_parser::new(&lines[16]).parse_record().unwrap();
        for (field, value) in [
            ("sequence", 3),
            ("copy_sequence", 3),
            ("copy_address", stack_end - 63),
            ("copy_bytes", 9),
            ("copy_attempts", 15),
            ("copy_successes", 9),
            ("copy_rejections", 5),
        ] {
            let mut changed = terminal.clone();
            changed.insert(field.into(), json_scalar::number(value.to_string()));
            let mut edited = lines.clone();
            edited[16] = render(&changed);
            assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        }
        for prior in [12, 14] {
            let old = json_parser::new(&lines[prior]).parse_record().unwrap();
            for reset_sequence in [false, true] {
                let mut changed = terminal.clone();
                for field in ["copy_sequence", "copy_address", "copy_bytes", "copy_hex"] {
                    changed.insert(field.into(), old[field].clone());
                }
                if reset_sequence {
                    changed.insert("copy_sequence".into(), json_scalar::number("4".into()));
                }
                let mut edited = lines.clone();
                edited[16] = render(&changed);
                assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
            }
        }
        let mut changed = terminal.clone();
        changed.insert("pending_preserved".into(), json_scalar::boolean(false));
        let mut edited = lines.clone();
        edited[16] = render(&changed);
        assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        changed = terminal;
        changed.insert("committed".into(), json_scalar::boolean(true));
        edited[16] = render(&changed);
        assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        for (field, value) in [
            ("native_calls", 59),
            ("copy_attempts", 48),
            ("copy_successes", 36),
            ("copy_rejections", 12),
            ("caller_returns", 4),
        ] {
            let mut changed = json_parser::new(&lines[18]).parse_record().unwrap();
            changed.insert(field.into(), json_scalar::number(value.to_string()));
            let mut edited = lines.clone();
            edited[18] = render(&changed);
            assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        }
    }

    #[test]
    fn terminal_copy_preserves_every_frame_byte_and_exact_request_correlation() {
        let (valid, plan, source) = fixture();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        let original = json_parser::new(&lines[16]).parse_record().unwrap();
        for field in ["frame_before_hex", "frame_after_hex"] {
            let text = required_string(&original, events[16], field).unwrap();
            for byte in 0..176 {
                let mut hex = text.as_bytes().to_vec();
                hex[byte * 2] = if hex[byte * 2] == b'0' { b'1' } else { b'0' };
                let mut changed = original.clone();
                changed.insert(
                    field.into(),
                    json_scalar::string(String::from_utf8(hex).unwrap()),
                );
                let mut edited = lines.clone();
                edited[16] = render(&changed);
                assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
            }
        }
        for word in [0, 2, 3, 5, 6, 7, 8, 9, 10, 15, 16, 17, 18, 19, 20, 21] {
            let mut changed = original.clone();
            for field in ["frame_before_hex", "frame_after_hex"] {
                let mut frame = reply_frame(&changed, field).unwrap();
                frame[word] ^= 1;
                changed.insert(field.into(), json_scalar::string(frame_hex(&frame)));
            }
            let mut edited = lines.clone();
            edited[16] = render(&changed);
            assert!(verify_records(&edited.join("\n"), &plan, &source).is_err());
        }
    }

    #[test]
    fn selection_preserves_higher_profiles() {
        assert!(selected(&[feature]));
        assert!(selected(&[
            feature,
            "linux_tls_test",
            "shared_veil_test",
            "workspace_dev"
        ]));
        let lower_profiles = [
            (
                linux_tls_smoke::feature,
                linux_tls_smoke::selected as fn(&[&str]) -> bool,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected,
            ),
            (
                elf_application_smoke::feature,
                elf_application_smoke::selected,
            ),
            (elf_relaunch_smoke::feature, elf_relaunch_smoke::selected),
            (elf_document_smoke::feature, elf_document_smoke::selected),
            (
                native_relaunch_smoke::feature,
                native_relaunch_smoke::selected,
            ),
            (
                native_document_smoke::feature,
                native_document_smoke::selected,
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
                native_durable_failure_smoke::feature,
                native_durable_failure_smoke::selected,
            ),
            (
                native_durable_smoke::feature,
                native_durable_smoke::selected,
            ),
            (
                native_runtime_smoke::feature,
                native_runtime_smoke::selected,
            ),
            (
                native_application_smoke::feature,
                native_application_smoke::selected,
            ),
            (ny_object_smoke::feature, ny_object_smoke::selected),
            (
                native_editor_workspace::feature,
                native_editor_workspace::selected,
            ),
            (mixed_workspace::feature, mixed_workspace::selected),
            (
                application_profile_smoke::feature,
                application_profile_smoke::selected,
            ),
            (owned_source_smoke::feature, owned_source_smoke::selected),
            (owned_tls_smoke::feature, owned_tls_smoke::selected),
            (
                owned_multi_region_smoke::feature,
                owned_multi_region_smoke::selected,
            ),
            (
                owned_dispatcher_smoke::feature,
                owned_dispatcher_smoke::selected,
            ),
            (dispatcher_smoke::feature, dispatcher_smoke::selected),
            (owned_workspace_cpu::feature, owned_workspace_cpu::selected),
            (owned_workspace::feature, owned_workspace::selected),
            (workspace_dev::feature, workspace_dev::selected),
        ];
        let mut expanded = vec![feature];
        for (lower, selector) in lower_profiles {
            let features = [feature, lower];
            assert!(selected(&features), "rejected loader with {lower}");
            assert!(!selector(&features), "also selected {lower}");
            expanded.push(lower);
        }
        assert!(selected(&expanded));
        for (lower, selector) in lower_profiles {
            assert!(!selector(&expanded), "expanded also selected {lower}");
        }
        assert!(!selected(all_kernel_features));
        for higher in [
            "fault_test",
            "circle_test",
            "control_test",
            "rite_test",
            "memory_test",
            "interrupt_test",
            "supervision_test",
            "shell_vessel_test",
            "input_wait_test",
            "display_vessel_test",
            "supervised_renderer_test",
            "semantic_surface_transport_test",
            "semantic_surface_presentation_test",
            "desktop_dev",
        ] {
            assert!(!selected(&[feature, higher]), "accepted{higher}");
        }
    }

    fn render(record: &structured_record) -> String {
        format!(
            "{{{}}}",
            record
                .iter()
                .map(|(name, value)| {
                    let value = match value {
                        json_scalar::string(s) => format!("{s:?}"),
                        json_scalar::number(s) => s.clone(),
                        json_scalar::boolean(b) => b.to_string(),
                        json_scalar::null => "null".into(),
                    };
                    format!("{name:?}:{value}")
                })
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}
