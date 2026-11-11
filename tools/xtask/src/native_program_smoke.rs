//! Strict oracle for a program-bearing launch: one task, a 64 KiB arena, and
//! an `ANPG` grant restated from inside it and a bounded console service.
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
/// `console_write`, the only service either scenario names.
const console_write: u64 = 1;
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
    let small = run_size(&image.path, "96M", &logs[0])?;
    let large = run_size(&image.path, "320M", &logs[1])?;
    verify_size_evidence(small, large)?;
    println!(
        "program launch proved an ANPG grant restated from a private 64 KiB arena at both admitted bases, with exact UTF-8 console output, checked replies, rejected invalid requests and no edition execution, at 96M and 320M (free frames: {} -> {})",
        small.free_frames, large.free_frames
    );
    Ok(())
}

fn run_size(image: &Path, memory_size: &str, log: &Path) -> Result<run_evidence, String> {
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

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "native_program_test",
        "native_program_profile",
        "program_console",
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
    if free_frames < 36 {
        return Err("program launch boot budget is too small".into());
    }
    // One task and one code page: a program launch that quietly became a pair,
    // or grew the native application's eight RX pages, fails here.
    //
    // The frame counts are the kernel's own measurement of what it owned while
    // the task ran: 25 data frames is 1 code + 2 data + 16 arena + 2 stack plus
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
        ],
    )?;
    for (field, value) in [
        ("tasks", 1),
        ("scenarios", 2),
        ("code_pages", 1),
        ("descriptor_bytes", descriptor_bytes),
        ("arena_bytes", arena_bytes),
        ("mappings", 4),
        ("data_frames", 25),
        ("table_frames", 9),
        ("leases", 6),
        ("free_frames", free_frames),
    ] {
        require_number(&records[8], expected_events[8], field, value)?;
    }
    if !matches!(records[8].get("vendor"), Some(json_scalar::string(value))
        if matches!(value.as_str(), "GenuineIntel" | "AuthenticAMD" | "HygonGenuine"))
    {
        return Err("program launch requires an admitted CPU vendor".into());
    }
    // This proof exercises the granted console service. The edition metadata
    // remains inert: no edition is mapped or interpreted by this fixture.
    require_bool(&records[8], expected_events[8], "edition_mapped", false)?;
    require_bool(&records[8], expected_events[8], "console_transport", true)?;
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
    ] {
        require_number(&records[11], expected_events[11], field, value)?;
    }
    require_bool(&records[11], expected_events[11], "grants_restated", true)?;
    require_bool(&records[11], expected_events[11], "console_verified", true)?;
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
/// Scenario 0 grants a console handle paired with `console_write` and twelve
/// argument bytes; scenario 1 grants nothing and passes no arguments, so the
/// console pairing is proved in both directions. Scenario 0 commits two writes
/// and rejects fourteen malformed or unauthorized requests; scenario 1 rejects
/// both requests without a grant. Neither executes an edition.
/// `console_transport` is `true` only where the grant was present.
fn verify_scenario(record: &structured_record, event: &str, scenario: usize) -> Result<(), String> {
    let console = scenario == 0;
    let numbers = [
        ("scenario", scenario as u64),
        ("arena_base", arena_bases[scenario]),
        ("zeroed_bytes", arena_bytes),
        ("calls", if console { 18 } else { 4 }),
        ("console_attempts", if console { 16 } else { 2 }),
        ("console_writes", if console { 2 } else { 0 }),
        ("console_rejected", if console { 14 } else { 2 }),
        ("console_bytes", if console { console_bytes } else { 0 }),
        ("console_last_sequence", if console { 2 } else { 0 }),
        ("invalid", 0),
        ("exits", 1),
        ("granted_services", if console { console_write } else { 0 }),
        ("console_granted", u64::from(console)),
        ("argument_bytes", if console { 12 } else { 0 }),
        ("edition_bytes", if console { 4096 } else { 64 }),
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
            r#"{"schema":1,"event":"kernel.native_program_test.ready","level":"info","vendor":"GenuineIntel","tasks":1,"scenarios":2,"code_pages":1,"descriptor_bytes":256,"arena_bytes":65536,"mappings":4,"data_frames":25,"table_frames":9,"leases":6,"free_frames":20000,"edition_mapped":false,"console_transport":true}"#
                .to_owned(),
        );
        lines.push(
            r#"{"schema":1,"event":"kernel.native_program_test.scenario","level":"info","scenario":0,"arena_base":8404992,"zeroed_bytes":65536,"calls":18,"console_attempts":16,"console_writes":2,"console_rejected":14,"console_bytes":60,"console_last_sequence":2,"timer_ticks":3,"invalid":0,"exits":1,"granted_services":1,"console_granted":1,"console_transport":true,"argument_bytes":12,"edition_bytes":4096,"grant_restated":true}"#
                .to_owned(),
        );
        lines.push(
            r#"{"schema":1,"event":"kernel.native_program_test.scenario","level":"info","scenario":1,"arena_base":8486912,"zeroed_bytes":65536,"calls":4,"console_attempts":2,"console_writes":0,"console_rejected":2,"console_bytes":0,"console_last_sequence":0,"timer_ticks":0,"invalid":0,"exits":1,"granted_services":0,"console_granted":0,"console_transport":false,"argument_bytes":0,"edition_bytes":64,"grant_restated":true}"#
                .to_owned(),
        );
        lines.push(
            r#"{"schema":1,"event":"kernel.native_program_test.passed","level":"info","program_tasks":2,"descriptor_bytes":256,"arena_bytes":65536,"zeroed_bytes":131072,"grants_restated":true,"editions_executed":0,"console_writes":2,"console_rejected":16,"console_bytes":60,"console_verified":true,"resources_restored":true,"authority_revoked":true,"source_language":"rust"}"#
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
