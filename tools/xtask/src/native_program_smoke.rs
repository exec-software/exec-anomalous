//! Strict oracle for a program-bearing launch: one task, a 64 KiB arena, and
//! an `ANPG` grant restated from inside it, console output and edition reads.
//!
//! No framebuffer capture, deliberately. This proof's evidence is entirely in
//! its serial transcript -- the kernel withdraws the aperture and presents
//! nothing -- so there is no deterministic frame to compare, and a frame check
//! that accepted anything would be a check in name only.
use super::*;

pub(super) const feature: &str = "native_program_test";
/// Both admitted bases, restated here rather than imported from the kernel.
/// An oracle that asks the thing under test what to expect cannot notice it
/// changing.
const arena_bases: [u64; 2] = [0x804000, 0x818000];
const arena_bytes: u64 = 64 * 1024;
const descriptor_bytes: u64 = 256;
/// Independently specified console-write and edition-read grant bits.
const granted_services: u64 = 3;
const edition_length: usize = 8195;
const edition_fnv64: u64 = 0x84ca7df1efd6b401;
const edition_item: &str = "opt/anomalous-program-edition";
/// Exact independently specified UTF-8 output, each terminated by one LF.
const console_lines: [&str; 2] = ["Native console: ready — ✓", "Native console: reply checked"];
const console_bytes: u64 =
    (console_lines[0].len() + console_lines[1].len() + console_lines.len()) as u64;
const expected_events: [&str; 12] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.native_program_test.ready",
    "kernel.native_program_test.scenario",
    "kernel.native_program_test.scenario",
    "kernel.native_program_test.passed",
];

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|value| {
            matches!(
                *value,
                "native_manifest_test"
                    | "fault_test"
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
                    | "mixed_workspace_test"
                    | "native_editor_workspace_test"
                    | "ny_object_test"
                    | "elf_application_test"
                    | "linux_application_test"
                    | "veil_application_test"
                    | "shared_veil_test"
                    | "linux_tls_test"
                    | "linux_loader_entry_test"
                    | "native_durable_test"
                    | "native_durable_failure_test"
                    | "native_repeated_editor_test"
                    | "native_interactive_editor_test"
                    | "native_document_test"
                    | "elf_document_test"
                    | "native_relaunch_test"
                    | "elf_relaunch_test"
            )
        })
}

pub(super) fn build() -> Result<built_image, String> {
    prepare_edition_fixture(&repository_root()?.join("build"))?;
    let image = build_image_with_context(&[feature], None)?;
    println!("program launch image inspected: {}", image.path.display());
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let build = repository_root()?.join("build");
    let logs = [
        build.join("qemu_native_program_96M.log"),
        build.join("qemu_native_program_320M.log"),
    ];
    for log in &logs {
        remove_optional_file(log)?;
    }
    build_and_inspect_normal_image()?;
    let image = self::build()?;
    let source = build.join("native-program-edition.bin");
    verify_provisioning_rejections(&image.path, &build, &source)?;
    let small = run_size(&image.path, &source, "96M", &logs[0])?;
    let large = run_size(&image.path, &source, "320M", &logs[1])?;
    verify_size_evidence(small, large)?;
    println!(
        "program launch proved task-bound console output and bounded external edition reads at both arena bases, with checked replies, rejected requests, source cleanup and no edition execution, at 96M and 320M (free frames: {} -> {})",
        small.free_frames, large.free_frames
    );
    Ok(())
}

fn edition_fixture() -> Vec<u8> {
    (0..edition_length)
        .map(|index| (index as u8).wrapping_mul(37).wrapping_add(11))
        .collect()
}

fn fixture_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn prepare_edition_fixture(build: &Path) -> Result<(), String> {
    let bytes = edition_fixture();
    if bytes.len() != edition_length || fixture_hash(&bytes) != edition_fnv64 {
        return Err("program edition fixture differs from its independent length/hash pin".into());
    }
    fs::create_dir_all(build)
        .map_err(|error| format!("create program fixture directory: {error}"))?;
    fs::write(build.join("native-program-edition.bin"), &bytes)
        .map_err(|error| format!("retain program edition fixture: {error}"))
}

fn program_command(
    image: &Path,
    source: Option<&Path>,
    memory_size: &str,
) -> Result<Command, String> {
    let mut command = qemu_command_with_memory(image, memory_size)?;
    command.args([
        "-cpu",
        "Nehalem",
        "-display",
        "none",
        "-monitor",
        "none",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ]);
    if let Some(source) = source {
        command
            .arg("-fw_cfg")
            .arg(format!("name={edition_item},file={}", source.display()));
    }
    Ok(command)
}

fn run_size(
    image: &Path,
    source: &Path,
    memory_size: &str,
    log: &Path,
) -> Result<run_evidence, String> {
    let command = program_command(image, Some(source), memory_size)?;
    let mut evidence = None;
    println!("program launch QEMU RAM: {memory_size}");
    run_qemu_with_transcript(command, smoke_timeout, |status, transcript| {
        // Retain failures too: diagnostic evidence must survive a rejected oracle.
        fs::write(log, transcript)
            .map_err(|error| format!("retain program launch transcript: {error}"))?;
        evidence = Some(verify_output(status, transcript)?);
        Ok(())
    })?;
    evidence.ok_or_else(|| format!("program launch produced no evidence at {memory_size}"))
}

fn verify_provisioning_rejections(image: &Path, build: &Path, source: &Path) -> Result<(), String> {
    let bytes =
        fs::read(source).map_err(|error| format!("read program edition fixture: {error}"))?;
    if bytes.len() != edition_length || fixture_hash(&bytes) != edition_fnv64 {
        return Err("program edition fixture changed before provisioning probes".into());
    }
    let short = build.join("native-program-edition-short.bin");
    fs::write(&short, &bytes[..bytes.len() - 1])
        .map_err(|error| format!("retain short program edition: {error}"))?;
    let corrupt = build.join("native-program-edition-corrupt.bin");
    let mut changed = bytes;
    changed[4096] ^= 1;
    fs::write(&corrupt, changed)
        .map_err(|error| format!("retain changed program edition: {error}"))?;
    for (case, path, reason) in [
        ("missing", None, "program_source_missing"),
        ("short", Some(short.as_path()), "program_source_size"),
        ("corrupt", Some(corrupt.as_path()), "program_source_hash"),
    ] {
        let log = build.join(format!("qemu_native_program_source_{case}.log"));
        remove_optional_file(&log)?;
        let command = program_command(image, path, "96M")?;
        println!("program edition provisioning rejection: {case}");
        run_qemu_with_transcript(command, smoke_timeout, |status, transcript| {
            fs::write(&log, transcript)
                .map_err(|error| format!("retain program source refusal: {error}"))?;
            verify_source_rejection(status.code(), transcript, reason)
        })?;
    }
    Ok(())
}

fn verify_source_rejection(
    exit_code: Option<i32>,
    transcript: &str,
    reason: &str,
) -> Result<(), String> {
    if exit_code != Some(35)
        || !matches!(
            reason,
            "program_source_missing" | "program_source_size" | "program_source_hash"
        )
    {
        return Err("program source refusal requires its exact failure status and reason".into());
    }
    let records = transcript
        .lines()
        .map(|line| json_parser::new(line).parse_record())
        .collect::<Result<Vec<_>, _>>()?;
    if records.len() != 10 {
        return Err("program source refusal requires eight boot and two failure records".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    let event = "kernel.native_program_test.source_rejected";
    require_control_header(
        &records[8],
        event,
        &["reason", "source_released", "resources_restored"],
    )?;
    require_string(&records[8], event, "reason", reason)?;
    require_bool(&records[8], event, "source_released", true)?;
    require_bool(&records[8], event, "resources_restored", true)?;
    let failed = &records[9];
    let event = "kernel.native_application_test.failed";
    if failed.len() != 4 {
        return Err("program source failure record has unexpected fields".into());
    }
    require_number(failed, event, "schema", 1)?;
    require_string(failed, event, "event", event)?;
    require_string(failed, event, "level", "fatal")?;
    require_string(failed, event, "reason", reason)
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "native_program_test",
        "native_program_profile",
        "program_console",
        "program_edition",
        edition_item,
        "Native console: ready",
        "Native console: reply checked",
        "anomalous_native_program_client",
        "x86_64_native_program",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains program launch proof implementation".into());
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct run_evidence {
    free_frames: u64,
    unavailable_frames: u64,
    total_frames: u64,
}

fn verify_output(status: ExitStatus, transcript: &str) -> Result<run_evidence, String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report program launch success (process status: {:?})",
            status.code()
        ));
    }
    verify_records(transcript)
}

pub(super) fn verify_transcript(transcript: &str) -> Result<(), String> {
    verify_records(transcript).map(|_| ())
}

fn record_event(
    record: structured_record,
    records: &mut Vec<structured_record>,
) -> Result<(), String> {
    let event = expected_events
        .get(records.len())
        .ok_or("program launch transcript contains extra records")?;
    require_string(&record, event, "event", event)?;
    require_string(&record, event, "level", "info")?;
    require_number(&record, event, "schema", 1)?;
    records.push(record);
    Ok(())
}

fn verify_records(transcript: &str) -> Result<run_evidence, String> {
    let mut records = Vec::new();
    let mut console_count = 0;
    for (index, raw_line) in transcript.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            return Err("program launch has an unexpected empty serial line".into());
        }
        if !line.starts_with('{') {
            // Scenarios execute after the eight boot records and before their
            // evidence is reported. Text is exact UTF-8; only the line-ending
            // normalization performed by str::lines is allowed. In particular,
            // trimming must not hide additional bytes emitted by the console.
            if records.len() != 8 || console_lines.get(console_count) != Some(&raw_line) {
                return Err(format!(
                    "program launch has unexpected console text at serial line {}",
                    index + 1
                ));
            }
            console_count += 1;
            continue;
        }
        if records.len() == 8 && console_count != console_lines.len() {
            return Err("program launch is missing its exact console output".into());
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed program launch serial record {}: {error}",
                index + 1
            )
        })?;
        record_event(record, &mut records)?;
    }
    if records.len() != expected_events.len() {
        return Err("program launch transcript is missing lifecycle records".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    let free_frames = required_number(&records[4], expected_events[4], "free_frames")?;
    if free_frames < 40 {
        return Err("program launch boot budget is too small".into());
    }
    // One task and two code pages: the measured client outgrew one page, but
    // a launch that became a pair or grew to eight RX pages still fails here.
    //
    // The frame counts are the kernel's own measurement of what it owned while
    // the task ran: 26 data frames is 2 code + 2 data + 16 arena + 2 stack plus
    // a 4-page transition stack, and 9 table frames is the eight-table user
    // plan plus one. `free_frames` must agree with what boot reported, which is
    // how the profile is shown to have observed the same allocator.
    require_control_header(
        &records[8],
        expected_events[8],
        &[
            "vendor",
            "tasks",
            "scenarios",
            "code_pages",
            "descriptor_bytes",
            "arena_bytes",
            "mappings",
            "data_frames",
            "table_frames",
            "leases",
            "free_frames",
            "edition_mapped",
            "console_transport",
            "edition_transport",
            "source_frames",
            "source_leases",
        ],
    )?;
    for (field, value) in [
        ("tasks", 1),
        ("scenarios", 2),
        ("code_pages", 2),
        ("descriptor_bytes", descriptor_bytes),
        ("arena_bytes", arena_bytes),
        ("mappings", 4),
        ("data_frames", 26),
        ("table_frames", 9),
        ("leases", 6),
        ("source_frames", 3),
        ("source_leases", 1),
        ("free_frames", free_frames),
    ] {
        require_number(&records[8], expected_events[8], field, value)?;
    }
    if !matches!(records[8].get("vendor"), Some(json_scalar::string(value))
        if matches!(value.as_str(), "GenuineIntel" | "AuthenticAMD" | "HygonGenuine"))
    {
        return Err("program launch requires an admitted CPU vendor".into());
    }
    // Source bytes are delivered into existing writable arena pages. No source
    // mapping or interpreted edition execution is established by those copies.
    require_bool(&records[8], expected_events[8], "edition_mapped", false)?;
    require_bool(&records[8], expected_events[8], "console_transport", true)?;
    require_bool(&records[8], expected_events[8], "edition_transport", true)?;
    for scenario in 0..2 {
        verify_scenario(
            &records[9 + scenario],
            expected_events[9 + scenario],
            scenario,
        )?;
    }
    require_control_header(
        &records[11],
        expected_events[11],
        &[
            "program_tasks",
            "descriptor_bytes",
            "arena_bytes",
            "zeroed_bytes",
            "grants_restated",
            "editions_executed",
            "console_writes",
            "console_rejected",
            "console_bytes",
            "console_verified",
            "edition_reads",
            "edition_rejected",
            "edition_read_bytes",
            "edition_verified",
            "edition_frame_preserved",
            "source_released",
            "allocator_available",
            "resources_restored",
            "authority_revoked",
            "source_language",
        ],
    )?;
    for (field, value) in [
        ("program_tasks", 2),
        ("descriptor_bytes", descriptor_bytes),
        ("arena_bytes", arena_bytes),
        ("zeroed_bytes", 2 * arena_bytes),
        ("editions_executed", 0),
        ("console_writes", 2),
        ("console_rejected", 16),
        ("console_bytes", console_bytes),
        ("edition_reads", 7),
        ("edition_rejected", 17),
        ("edition_read_bytes", 8203),
    ] {
        require_number(&records[11], expected_events[11], field, value)?;
    }
    require_bool(&records[11], expected_events[11], "grants_restated", true)?;
    require_bool(&records[11], expected_events[11], "console_verified", true)?;
    require_bool(&records[11], expected_events[11], "edition_verified", true)?;
    require_bool(
        &records[11],
        expected_events[11],
        "edition_frame_preserved",
        true,
    )?;
    require_bool(&records[11], expected_events[11], "source_released", true)?;
    require_bool(
        &records[11],
        expected_events[11],
        "allocator_available",
        true,
    )?;
    require_bool(
        &records[11],
        expected_events[11],
        "resources_restored",
        true,
    )?;
    require_bool(&records[11], expected_events[11], "authority_revoked", true)?;
    require_string(&records[11], expected_events[11], "source_language", "rust")?;
    Ok(run_evidence {
        free_frames,
        unavailable_frames: required_number(&records[4], expected_events[4], "unavailable_frames")?,
        total_frames: required_number(&records[4], expected_events[4], "total_frames")?,
    })
}

/// One scenario: a launch at one admitted base with one grant shape.
///
/// Scenario 0 grants console output and edition reads with twelve argument
/// bytes; scenario 1 grants neither and passes no arguments. The first commits
/// two writes and seven reads (including EOF), while raw hostile calls prove
/// refusal without publication or sequence consumption. The second rejects
/// stale, absent and inert metadata handles. Neither executes an edition.
/// `console_transport` is `true` only where the grant was present.
fn verify_scenario(record: &structured_record, event: &str, scenario: usize) -> Result<(), String> {
    let console = scenario == 0;
    let numbers = [
        ("scenario", scenario as u64),
        ("arena_base", arena_bases[scenario]),
        ("zeroed_bytes", arena_bytes),
        ("calls", if console { 39 } else { 7 }),
        ("console_attempts", if console { 16 } else { 2 }),
        ("console_writes", if console { 2 } else { 0 }),
        ("console_rejected", if console { 14 } else { 2 }),
        ("console_bytes", if console { console_bytes } else { 0 }),
        ("console_last_sequence", if console { 2 } else { 0 }),
        ("edition_attempts", if console { 21 } else { 3 }),
        ("edition_reads", if console { 7 } else { 0 }),
        ("edition_rejected", if console { 14 } else { 3 }),
        ("edition_read_bytes", if console { 8203 } else { 0 }),
        ("edition_last_sequence", if console { 7 } else { 0 }),
        ("edition_source_reads", if console { 5 } else { 0 }),
        ("edition_granted", u64::from(console)),
        ("allocator_service_borrows", if console { 2 } else { 0 }),
        ("allocator_nested_denials", if console { 1 } else { 0 }),
        ("invalid", 0),
        ("exits", 1),
        (
            "granted_services",
            if console { granted_services } else { 0 },
        ),
        ("console_granted", u64::from(console)),
        ("argument_bytes", if console { 12 } else { 0 }),
        ("edition_bytes", if console { 8195 } else { 64 }),
    ];
    let mut fields = numbers.iter().map(|(f, _)| *f).collect::<Vec<_>>();
    fields.extend(["timer_ticks", "grant_restated", "console_transport"]);
    require_control_header(record, event, &fields)?;
    for (field, value) in numbers {
        require_number(record, event, field, value)?;
    }
    require_bool(record, event, "console_transport", console)?;
    if required_number(record, event, "timer_ticks")? > 10_000 {
        return Err("program launch timer bound exceeded".into());
    }
    // The whole point: the six words the task stated back are the six words the
    // kernel wrote. The kernel compares them through its own alias and reports
    // the comparison, never the task's copy of them.
    require_bool(record, event, "grant_restated", true)?;
    Ok(())
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
        return Err(
            "program launch RAM-size evidence does not match the 224-MiB frame delta".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_edition_fixture_has_independent_length_and_hash_pins() {
        let bytes = edition_fixture();
        assert_eq!(bytes.len(), 8195);
        assert_eq!(&bytes[..8], &[11, 48, 85, 122, 159, 196, 233, 14]);
        assert_eq!(&bytes[8187..], &[82, 119, 156, 193, 230, 11, 48, 85]);
        assert_eq!(fixture_hash(b"hello"), 0xa430d84680aabd0b);
        assert_eq!(fixture_hash(&bytes), 0x84ca7df1efd6b401);
        assert_eq!(edition_fnv64, 0x84ca7df1efd6b401);
        for offset in [0, 4095, 4096, 8194] {
            let mut changed = bytes.clone();
            changed[offset] ^= 1;
            assert_ne!(fixture_hash(&changed), edition_fnv64);
        }
    }

    fn rejected_source(reason: &str) -> String {
        let mut lines = valid_transcript()
            .lines()
            .take(8)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        lines.push(format!(
            r#"{{"schema":1,"event":"kernel.native_program_test.source_rejected","level":"info","reason":{reason:?},"source_released":true,"resources_restored":true}}"#
        ));
        lines.push(format!(
            r#"{{"schema":1,"event":"kernel.native_application_test.failed","level":"fatal","reason":{reason:?}}}"#
        ));
        lines.join("\n")
    }

    #[test]
    fn source_refusal_requires_exact_reason_cleanup_and_terminal_failure() {
        for reason in [
            "program_source_missing",
            "program_source_size",
            "program_source_hash",
        ] {
            let valid = rejected_source(reason);
            verify_source_rejection(Some(35), &valid, reason).unwrap();
            verify_source_rejection(Some(35), &valid.replace('\n', "\r\n"), reason).unwrap();
            for code in [None, Some(0), Some(33), Some(1)] {
                assert!(verify_source_rejection(code, &valid, reason).is_err());
            }
            for wrong in ["program_source_other", "application_cpu_capabilities"] {
                assert!(
                    verify_source_rejection(Some(35), &valid.replace(reason, wrong), reason)
                        .is_err()
                );
                assert!(verify_source_rejection(Some(35), &valid, wrong).is_err());
            }
            let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
            for index in 0..lines.len() {
                let mut changed = lines.clone();
                changed.remove(index);
                assert!(verify_source_rejection(Some(35), &changed.join("\n"), reason).is_err());
                let mut changed = lines.clone();
                changed.insert(index, lines[index].clone());
                assert!(verify_source_rejection(Some(35), &changed.join("\n"), reason).is_err());
            }
            for extra in ["\n", "\nNative console: ready — ✓", "\n{}"] {
                assert!(
                    verify_source_rejection(Some(35), &format!("{valid}\n{extra}"), reason)
                        .is_err()
                );
            }
            for index in 8..10 {
                let record = json_parser::new(&lines[index]).parse_record().unwrap();
                for (field, value) in &record {
                    let mut changed = record.clone();
                    changed.remove(field);
                    let mut changed_lines = lines.clone();
                    changed_lines[index] = render(&changed);
                    assert!(
                        verify_source_rejection(Some(35), &changed_lines.join("\n"), reason)
                            .is_err()
                    );
                    changed.insert(
                        field.clone(),
                        match value {
                            json_scalar::string(value) => {
                                json_scalar::string(format!("{value}.changed"))
                            }
                            json_scalar::number(_) => json_scalar::number("2".into()),
                            json_scalar::boolean(value) => json_scalar::boolean(!value),
                            json_scalar::null => unreachable!(),
                        },
                    );
                    changed_lines[index] = render(&changed);
                    assert!(
                        verify_source_rejection(Some(35), &changed_lines.join("\n"), reason)
                            .is_err()
                    );
                }
                let mut changed = record.clone();
                changed.insert(
                    "private_bytes".into(),
                    json_scalar::string("payload".into()),
                );
                let mut changed_lines = lines.clone();
                changed_lines[index] = render(&changed);
                assert!(
                    verify_source_rejection(Some(35), &changed_lines.join("\n"), reason).is_err()
                );
            }
        }
    }

    fn valid_transcript() -> String {
        let mut lines = vec![
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#.to_owned(),
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#.to_owned(),
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#.to_owned(),
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#.to_owned(),
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#.to_owned(),
        ];
        // Independently recorded expected output, before the proof records.
        lines.push("Native console: ready — ✓".to_owned());
        lines.push("Native console: reply checked".to_owned());
        lines.push(
            r#"{"schema":1,"event":"kernel.native_program_test.ready","level":"info","vendor":"GenuineIntel","tasks":1,"scenarios":2,"code_pages":2,"descriptor_bytes":256,"arena_bytes":65536,"mappings":4,"data_frames":26,"table_frames":9,"leases":6,"free_frames":20000,"edition_mapped":false,"console_transport":true,"edition_transport":true,"source_frames":3,"source_leases":1}"#
                .to_owned(),
        );
        lines.push(
            r#"{"schema":1,"event":"kernel.native_program_test.scenario","level":"info","scenario":0,"arena_base":8404992,"zeroed_bytes":65536,"calls":39,"console_attempts":16,"console_writes":2,"console_rejected":14,"console_bytes":60,"console_last_sequence":2,"timer_ticks":3,"invalid":0,"exits":1,"granted_services":3,"console_granted":1,"console_transport":true,"argument_bytes":12,"edition_bytes":8195,"grant_restated":true,"edition_attempts":21,"edition_reads":7,"edition_rejected":14,"edition_read_bytes":8203,"edition_last_sequence":7,"edition_source_reads":5,"edition_granted":1,"allocator_service_borrows":2,"allocator_nested_denials":1}"#
                .to_owned(),
        );
        lines.push(
            r#"{"schema":1,"event":"kernel.native_program_test.scenario","level":"info","scenario":1,"arena_base":8486912,"zeroed_bytes":65536,"calls":7,"console_attempts":2,"console_writes":0,"console_rejected":2,"console_bytes":0,"console_last_sequence":0,"timer_ticks":0,"invalid":0,"exits":1,"granted_services":0,"console_granted":0,"console_transport":false,"argument_bytes":0,"edition_bytes":64,"grant_restated":true,"edition_attempts":3,"edition_reads":0,"edition_rejected":3,"edition_read_bytes":0,"edition_last_sequence":0,"edition_source_reads":0,"edition_granted":0,"allocator_service_borrows":0,"allocator_nested_denials":0}"#
                .to_owned(),
        );
        lines.push(
            r#"{"schema":1,"event":"kernel.native_program_test.passed","level":"info","program_tasks":2,"descriptor_bytes":256,"arena_bytes":65536,"zeroed_bytes":131072,"grants_restated":true,"editions_executed":0,"console_writes":2,"console_rejected":16,"console_bytes":60,"console_verified":true,"resources_restored":true,"authority_revoked":true,"source_language":"rust","edition_reads":7,"edition_rejected":17,"edition_read_bytes":8203,"edition_verified":true,"edition_frame_preserved":true,"source_released":true,"allocator_available":true}"#
                .to_owned(),
        );
        lines.join("\n")
    }

    fn render(record: &structured_record) -> String {
        let fields = record
            .iter()
            .map(|(key, value)| {
                let value = match value {
                    json_scalar::string(v) => format!("{v:?}"),
                    json_scalar::number(v) => v.clone(),
                    json_scalar::boolean(v) => v.to_string(),
                    json_scalar::null => "null".into(),
                };
                format!("{key:?}:{value}")
            })
            .collect::<Vec<_>>();
        format!("{{{}}}", fields.join(","))
    }

    #[test]
    fn normal_image_excludes_console_implementation_and_output() {
        assert!(verify_absent("ordinary_kernel", b"ordinary boot").is_ok());
        for marker in [
            "program_console::state::write",
            "program_edition::state::read",
            "opt/anomalous-program-edition",
            "Native console: ready",
            "Native console: reply checked",
        ] {
            assert!(verify_absent(marker, b"").is_err());
            assert!(verify_absent("", marker.as_bytes()).is_err());
        }
    }

    #[test]
    fn program_proof_requires_exact_order_and_complete_records() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        verify_transcript(&valid.replace("GenuineIntel", "AuthenticAMD")).unwrap();
        assert!(verify_transcript(&valid.replace("GenuineIntel", "SomeOtherCPU")).is_err());
        let lines = valid.lines().collect::<Vec<_>>();
        // Console lines are required evidence just like the JSON records.
        for index in 0..lines.len() {
            let mut changed = lines.clone();
            changed.remove(index);
            assert!(verify_transcript(&changed.join("\n")).is_err());
            let mut changed = lines.clone();
            changed.insert(index, lines[index]);
            assert!(verify_transcript(&changed.join("\n")).is_err());
            if index + 1 < lines.len() {
                let mut changed = lines.clone();
                changed.swap(index, index + 1);
                assert!(verify_transcript(&changed.join("\n")).is_err());
            }
        }
        assert!(verify_transcript(&format!("{valid}\n{{}}")).is_err());
        assert!(verify_transcript(&format!("{valid}\nnot json")).is_err());
        assert!(verify_transcript(&format!("\n{valid}")).is_err());
        assert!(verify_transcript(&format!("{valid}\n\n")).is_err());
    }

    #[test]
    fn console_output_requires_exact_utf8_bytes_and_position() {
        let valid = valid_transcript();
        assert_eq!(console_bytes, 60);
        verify_transcript(&valid.replace('\n', "\r\n")).unwrap();
        for (original, changed) in [
            ("Native console: ready — ✓", "Native console: ready - v"),
            ("Native console: ready — ✓", "Native console: ready — �"),
            ("Native console: ready — ✓", " Native console: ready — ✓"),
            ("Native console: ready — ✓", "Native console: ready — ✓ "),
            (
                "Native console: reply checked",
                "Native console: reply unchecked",
            ),
            (
                "Native console: reply checked",
                "Native console: reply checked\nextra",
            ),
            (
                "Native console: reply checked",
                "Native console: reply checked\n",
            ),
            ("Native console: ready — ✓\n", "Native console: ready — ✓"),
            (
                "Native console: reply checked\n",
                "Native console: reply checked",
            ),
        ] {
            assert!(
                verify_transcript(&valid.replace(original, changed)).is_err(),
                "accepted console replacement {changed:?}"
            );
        }
        let lines = valid.lines().collect::<Vec<_>>();
        for source in 8..10 {
            for destination in 0..lines.len() {
                if destination == source {
                    continue;
                }
                let mut moved = lines.clone();
                let line = moved.remove(source);
                moved.insert(destination, line);
                assert!(
                    verify_transcript(&moved.join("\n")).is_err(),
                    "accepted console line {source} at {destination}"
                );
            }
        }
    }

    #[test]
    fn program_proof_rejects_missing_changed_duplicate_and_private_fields() {
        let lines = valid_transcript()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        // Lines: 0-7 boot, 8-9 console output, 10 ready, 11-12 scenarios,
        // 13 passed. Every proof field, including console accounting, is exact.
        for index in 10..14 {
            let record = json_parser::new(&lines[index]).parse_record().unwrap();
            for (field, value) in &record {
                for replacement in [
                    None,
                    Some(json_scalar::null),
                    Some(match value {
                        json_scalar::number(_) => json_scalar::number(u64::MAX.to_string()),
                        json_scalar::boolean(v) => json_scalar::boolean(!v),
                        json_scalar::string(v) => json_scalar::string(format!("{v}.changed")),
                        json_scalar::null => unreachable!(),
                    }),
                ] {
                    let mut changed = record.clone();
                    if let Some(value) = replacement {
                        changed.insert(field.clone(), value);
                    } else {
                        changed.remove(field);
                    }
                    let mut changed_lines = lines.clone();
                    changed_lines[index] = render(&changed);
                    assert!(
                        verify_transcript(&changed_lines.join("\n")).is_err(),
                        "accepted {index}:{field}"
                    );
                }
                if let json_scalar::number(value) = value
                    && field != "timer_ticks"
                {
                    let mut changed = record.clone();
                    changed.insert(
                        field.clone(),
                        json_scalar::number((value.parse::<u64>().unwrap() + 1).to_string()),
                    );
                    let mut changed_lines = lines.clone();
                    changed_lines[index] = render(&changed);
                    assert!(
                        verify_transcript(&changed_lines.join("\n")).is_err(),
                        "accepted increment {index}:{field}"
                    );
                }
                let mut duplicate = render(&record);
                duplicate.pop();
                duplicate.push_str(&format!(",{field:?}:null}}"));
                let mut changed = lines.clone();
                changed[index] = duplicate;
                assert!(verify_transcript(&changed.join("\n")).is_err());
            }
            for field in ["unknown", "private_payload", "task_identity", "address"] {
                let mut changed = record.clone();
                changed.insert(field.into(), json_scalar::number("1".into()));
                let mut changed_lines = lines.clone();
                changed_lines[index] = render(&changed);
                assert!(verify_transcript(&changed_lines.join("\n")).is_err());
            }
        }
    }

    #[test]
    fn the_two_scenarios_cannot_be_swapped_or_duplicated() {
        // Each scenario is pinned to its own base and grant shape, so running
        // the same one twice -- or at the other's base -- is refused. A launch
        // that ignored the requested base would pass a weaker oracle.
        let lines = valid_transcript()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        // Lines: 0-7 boot, 8-9 console output, 10 ready, 11-12 scenarios,
        // 13 passed.
        let mut swapped = lines.clone();
        swapped.swap(11, 12);
        assert!(verify_transcript(&swapped.join("\n")).is_err());
        let mut repeated = lines.clone();
        repeated[12] = lines[11].clone();
        assert!(verify_transcript(&repeated.join("\n")).is_err());
        let mut crossed = lines.clone();
        crossed[11] = lines[11].replace("8404992", "8486912");
        assert!(verify_transcript(&crossed.join("\n")).is_err());
    }

    #[test]
    fn a_free_frame_count_must_match_the_boot_report() {
        let lines = valid_transcript()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        // Line 10 is the ready record, after both required console lines.
        let mut disagreeing = lines.clone();
        disagreeing[10] = lines[10].replace("\"free_frames\":20000", "\"free_frames\":19999");
        assert!(verify_transcript(&disagreeing.join("\n")).is_err());
    }

    #[test]
    fn the_program_feature_excludes_every_other_application_profile() {
        assert!(selected(&[feature]));
        for other in [
            "native_manifest_test",
            "native_document_test",
            "native_relaunch_test",
            "elf_application_test",
            "mixed_workspace_test",
            "ny_object_test",
            "ny_vessel_test",
            "linux_application_test",
            "desktop_dev",
        ] {
            assert!(!selected(&[feature, other]), "admitted {other}");
        }
        assert!(!selected(&["native_runtime_test"]));
        assert!(!native_runtime_smoke::selected(&[feature]));
        assert!(!native_application_smoke::selected(&[feature]));
    }

    #[test]
    fn manifest_selection_excludes_lower_profiles_and_preserves_higher_delegation() {
        let manifest = "native_manifest_test";
        for (lower, selector) in [
            (feature, selected as fn(&[&str]) -> bool),
            (
                native_runtime_smoke::feature,
                native_runtime_smoke::selected,
            ),
            (
                native_application_smoke::feature,
                native_application_smoke::selected,
            ),
            (ny_object_smoke::feature, ny_object_smoke::selected),
            (mixed_workspace::feature, mixed_workspace::selected),
            (
                native_editor_workspace::feature,
                native_editor_workspace::selected,
            ),
            (
                application_profile_smoke::feature,
                application_profile_smoke::selected,
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
        ] {
            assert!(
                !selector(&[lower, manifest]),
                "lower profile {lower} remained selected"
            );
        }
        for (higher, selector) in [
            (
                native_document_smoke::feature,
                native_document_smoke::selected as fn(&[&str]) -> bool,
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
                native_repeated_smoke::feature,
                native_repeated_smoke::selected,
            ),
            (
                native_interactive_smoke::feature,
                native_interactive_smoke::selected,
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
            (linux_tls_smoke::feature, linux_tls_smoke::selected),
            (
                linux_loader_entry_smoke::feature,
                linux_loader_entry_smoke::selected,
            ),
            (ny_vessel_smoke::feature, ny_vessel_smoke::selected),
        ] {
            for combined in [vec![higher, manifest], vec![higher, manifest, feature]] {
                assert!(
                    selector(&combined),
                    "higher profile {higher} lost selection"
                );
                assert!(!selected(&combined), "program profile shadowed {higher}");
            }
        }
        // The interactive desktop also refuses dispatcher-bearing feature
        // combinations, matching its kernel-side scheduler exclusion.
        assert!(!desktop_dev::selected(&[desktop_dev::feature, manifest]));
    }

    #[test]
    fn capture_requires_the_complete_final_record() {
        let text = valid_transcript();
        let mut prefix = String::new();
        for line in text.lines() {
            assert!(verify_transcript(&prefix).is_err());
            prefix.push_str(line);
            prefix.push('\n');
        }
        verify_transcript(&prefix).unwrap();
    }
}
