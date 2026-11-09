//! Strict oracle for sustained scheduling over retained prepared-launch roots.
use super::*;

pub(super) const feature: &str = "application_profile_test";
const private_canaries: [u64; 4] = [
    0x61a7_c0de_f17e_0001,
    0x61a7_c0de_f17e_0002,
    0x9ee7_c0de_ba5e_0002,
    0x9ee7_c0de_ba5e_0003,
];
const expected_events: [&str; 16] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.application_profile_test.ready",
    "kernel.application_profile_test.scenario",
    "kernel.application_profile_test.scenario",
    "kernel.application_profile_test.scenario",
    "kernel.application_profile_test.scenario",
    "kernel.application_profile_test.copy.verified",
    "kernel.application_profile_test.released",
    "kernel.application_profile_test.passed",
];

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|value| {
            matches!(
                *value,
                "mixed_workspace_test"
                    | "native_editor_workspace_test"
                    | "fault_test"
                    | "ny_object_test"
                    | "native_application_test"
                    | "native_runtime_test"
                    | "native_program_test"
                    | "native_durable_test"
                    | "native_durable_failure_test"
                    | "native_repeated_editor_test"
                    | "native_interactive_editor_test"
                    | "native_document_test"
                    | "elf_document_test"
                    | "native_relaunch_test"
                    | "elf_relaunch_test"
                    | "elf_application_test"
                    | "linux_application_test"
                    | "veil_application_test"
                    | "shared_veil_test"
                    | "linux_tls_test"
                    | "linux_loader_entry_test"
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
        "application CPU profile image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let build = repository_root()?.join("build");
    let logs = [
        build.join("qemu_application_profile_96M.log"),
        build.join("qemu_application_profile_320M.log"),
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
        "application profile proved x87/SSE2 isolation, checked user copies, four selected fault classes, and exact cleanup at 96M and 320M (free frames: {} -> {})",
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
    println!("application profile QEMU RAM: {memory_size}");
    run_qemu_with_transcript(command, smoke_timeout, |status, transcript| {
        // Retain failures too: diagnostic evidence must survive a rejected oracle.
        fs::write(log, transcript)
            .map_err(|error| format!("retain application-profile transcript: {error}"))?;
        let verified = verify_output(status, transcript)?;
        evidence = Some(verified);
        Ok(())
    })?;
    evidence.ok_or_else(|| format!("application profile produced no evidence at {memory_size}"))
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "application_profile_test",
        "application_profile_runtime",
        "application_cpu",
        "application_fault",
        "x86_64_application_profile",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains application CPU profile proof implementation".into());
        }
    }
    for canary in private_canaries {
        if bytes.windows(8).any(|value| value == canary.to_le_bytes()) {
            return Err("normal image retains an application profile private fixture".into());
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
            "QEMU did not report application-profile success (process status: {:?})",
            status.code()
        ));
    }
    verify_records(transcript)
}

pub(super) fn verify_transcript(transcript: &str) -> Result<(), String> {
    verify_records(transcript).map(|_| ())
}

fn verify_records(transcript: &str) -> Result<run_evidence, String> {
    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "application profile has unstructured serial line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed application profile serial record {}: {error}",
                index + 1
            )
        })?;
        let event = expected_events
            .get(records.len())
            .ok_or("application profile transcript contains extra records")?;
        require_string(&record, event, "event", event)?;
        require_string(&record, event, "level", "info")?;
        require_number(&record, event, "schema", 1)?;
        records.push(record);
    }
    if records.len() != expected_events.len() {
        return Err("application profile transcript is missing lifecycle records".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    require_control_header(
        &records[8],
        expected_events[8],
        &[
            "tasks",
            "scenarios",
            "quantum_switches",
            "survivor_ticks",
            "data_frames",
            "table_frames",
            "active_leases",
            "bound_leases",
            "cpu_profile",
            "cpu_vendor",
            "tls",
        ],
    )?;
    for (field, value) in [
        ("tasks", 2),
        ("scenarios", 4),
        ("quantum_switches", 64),
        ("survivor_ticks", 16),
        ("data_frames", 18),
        ("table_frames", 18),
        ("active_leases", 10),
        ("bound_leases", 10),
    ] {
        require_number(&records[8], expected_events[8], field, value)?;
    }
    require_string(&records[8], expected_events[8], "cpu_profile", "x87_sse2")?;
    if !matches!(records[8].get("cpu_vendor"), Some(json_scalar::string(value))
        if matches!(value.as_str(), "GenuineIntel" | "AuthenticAMD" | "HygonGenuine"))
    {
        return Err("application profile requires an admitted CPU vendor".into());
    }
    require_bool(&records[8], expected_events[8], "tls", false)?;
    for (scenario, vector) in [0, 6, 13, 14].into_iter().enumerate() {
        validate_record(
            &records[9 + scenario],
            expected_events[9 + scenario],
            &[
                ("scenario", scenario as u64),
                ("first_fault_vector", vector),
                ("last_fault_vector", 6),
                ("timer_ticks", 80),
                ("dispatches", 66),
                ("preemptions", 64),
                ("task_0_quanta", 32),
                ("task_1_quanta", 32),
            ],
            &[
                "initial_state_clean",
                "all_xmm_preserved",
                "x87_preserved",
                "mxcsr_preserved",
                "peer_continued",
                "last_fault_returned",
            ],
        )?;
    }
    validate_record(
        &records[13],
        expected_events[13],
        &[("successful_operations", 20), ("rejected_operations", 40)],
        &[
            "odd_cross_page",
            "destination_unchanged",
            "code_unchanged",
            "identity_bound",
        ],
    )?;
    let free_frames = required_number(&records[4], expected_events[4], "free_frames")?;
    if free_frames < 36 {
        return Err("application profile boot budget is too small".into());
    }
    validate_record(
        &records[14],
        expected_events[14],
        &[("free_frames", free_frames)],
        &[
            "resources_restored",
            "storage_cleared",
            "cpu_state_cleared",
            "kernel_cpu_restored",
            "bootstrap_restored",
            "tss_restored",
            "ts_restored",
            "alias_removed",
        ],
    )?;
    validate_record(
        &records[15],
        expected_events[15],
        &[
            ("scenarios", 4),
            ("tasks", 8),
            ("timer_ticks", 320),
            ("dispatches", 264),
            ("preemptions", 256),
            ("faults", 8),
            ("exits", 0),
        ],
        &[],
    )?;
    Ok(run_evidence {
        free_frames,
        unavailable_frames: required_number(&records[4], expected_events[4], "unavailable_frames")?,
        total_frames: required_number(&records[4], expected_events[4], "total_frames")?,
    })
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
            "application profile RAM-size evidence does not match the 224-MiB frame delta".into(),
        );
    }
    Ok(())
}

fn validate_record(
    record: &structured_record,
    event: &str,
    numbers: &[(&str, u64)],
    booleans: &[&str],
) -> Result<(), String> {
    let fields = numbers
        .iter()
        .map(|(field, _)| *field)
        .chain(booleans.iter().copied())
        .collect::<Vec<_>>();
    require_control_header(record, event, &fields)?;
    for (field, value) in numbers {
        require_number(record, event, field, *value)?;
    }
    for field in booleans {
        require_bool(record, event, field, true)?;
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
            r#"{"schema":1,"event":"kernel.application_profile_test.ready","level":"info","tasks":2,"scenarios":4,"quantum_switches":64,"survivor_ticks":16,"data_frames":18,"table_frames":18,"active_leases":10,"bound_leases":10,"cpu_profile":"x87_sse2","cpu_vendor":"GenuineIntel","tls":false}"#,
            r#"{"schema":1,"event":"kernel.application_profile_test.scenario","level":"info","scenario":0,"first_fault_vector":0,"last_fault_vector":6,"timer_ticks":80,"dispatches":66,"preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"initial_state_clean":true,"all_xmm_preserved":true,"x87_preserved":true,"mxcsr_preserved":true,"peer_continued":true,"last_fault_returned":true}"#,
            r#"{"schema":1,"event":"kernel.application_profile_test.scenario","level":"info","scenario":1,"first_fault_vector":6,"last_fault_vector":6,"timer_ticks":80,"dispatches":66,"preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"initial_state_clean":true,"all_xmm_preserved":true,"x87_preserved":true,"mxcsr_preserved":true,"peer_continued":true,"last_fault_returned":true}"#,
            r#"{"schema":1,"event":"kernel.application_profile_test.scenario","level":"info","scenario":2,"first_fault_vector":13,"last_fault_vector":6,"timer_ticks":80,"dispatches":66,"preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"initial_state_clean":true,"all_xmm_preserved":true,"x87_preserved":true,"mxcsr_preserved":true,"peer_continued":true,"last_fault_returned":true}"#,
            r#"{"schema":1,"event":"kernel.application_profile_test.scenario","level":"info","scenario":3,"first_fault_vector":14,"last_fault_vector":6,"timer_ticks":80,"dispatches":66,"preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"initial_state_clean":true,"all_xmm_preserved":true,"x87_preserved":true,"mxcsr_preserved":true,"peer_continued":true,"last_fault_returned":true}"#,
            r#"{"schema":1,"event":"kernel.application_profile_test.copy.verified","level":"info","successful_operations":20,"rejected_operations":40,"odd_cross_page":true,"destination_unchanged":true,"code_unchanged":true,"identity_bound":true}"#,
            r#"{"schema":1,"event":"kernel.application_profile_test.released","level":"info","free_frames":20000,"resources_restored":true,"storage_cleared":true,"cpu_state_cleared":true,"kernel_cpu_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true}"#,
            r#"{"schema":1,"event":"kernel.application_profile_test.passed","level":"info","scenarios":4,"tasks":8,"timer_ticks":320,"dispatches":264,"preemptions":256,"faults":8,"exits":0}"#,
        ].join("\n")
    }

    fn render(record: &structured_record) -> String {
        let fields = record
            .iter()
            .map(|(field, value)| {
                let value = match value {
                    json_scalar::string(value) => format!("{value:?}"),
                    json_scalar::number(value) => value.clone(),
                    json_scalar::boolean(value) => value.to_string(),
                    json_scalar::null => "null".into(),
                };
                format!("{field:?}:{value}")
            })
            .collect::<Vec<_>>();
        format!("{{{}}}", fields.join(","))
    }

    #[test]
    fn accepts_exact_application_profile_evidence_and_portable_line_endings() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        for vendor in ["AuthenticAMD", "HygonGenuine"] {
            verify_transcript(&valid.replace("GenuineIntel", vendor)).unwrap();
        }
        verify_transcript(&format!("\r\n{}\r\n", valid.replace('\n', "\r\n"))).unwrap();
        let reordered_fields = valid
            .lines()
            .map(|line| render(&json_parser::new(line).parse_record().unwrap()))
            .collect::<Vec<_>>()
            .join("\n");
        verify_transcript(&reordered_fields).unwrap();
    }

    #[test]
    fn rejects_missing_duplicate_reordered_unknown_and_extra_events() {
        let valid = valid_transcript();
        let records = valid.lines().collect::<Vec<_>>();
        for index in 0..records.len() {
            let mut changed = records.clone();
            changed.remove(index);
            assert!(verify_transcript(&changed.join("\n")).is_err());
            let mut changed = records.clone();
            changed.insert(index, records[index]);
            assert!(verify_transcript(&changed.join("\n")).is_err());
            let mut changed = records.clone();
            changed[index] = "{\"schema\":1,\"event\":\"unknown\",\"level\":\"info\"}";
            assert!(verify_transcript(&changed.join("\n")).is_err());
            if index + 1 < records.len() {
                let mut changed = records.clone();
                changed.swap(index, index + 1);
                assert!(verify_transcript(&changed.join("\n")).is_err());
            }
        }
        for extra in ["private payload", "{}", "{\"schema\":1", records[15]] {
            assert!(verify_transcript(&format!("{valid}\n{extra}")).is_err());
        }
    }

    #[test]
    fn rejects_every_native_field_value_type_removal_duplication_and_private_addition() {
        let valid = valid_transcript();
        let records = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for index in 8..records.len() {
            let record = json_parser::new(&records[index]).parse_record().unwrap();
            for (field, value) in &record {
                for replacement in [
                    None,
                    Some(json_scalar::null),
                    Some(match value {
                        json_scalar::number(_) => json_scalar::number(u64::MAX.to_string()),
                        json_scalar::boolean(value) => json_scalar::boolean(!value),
                        json_scalar::string(value) => {
                            json_scalar::string(format!("{value}.changed"))
                        }
                        json_scalar::null => unreachable!(),
                    }),
                ] {
                    let mut changed = record.clone();
                    if let Some(value) = replacement {
                        changed.insert(field.clone(), value);
                    } else {
                        changed.remove(field);
                    }
                    let mut lines = records.clone();
                    lines[index] = render(&changed);
                    assert!(
                        verify_transcript(&lines.join("\n")).is_err(),
                        "accepted {index}:{field} mutation"
                    );
                }
                let mut lines = records.clone();
                let mut duplicate = render(&record);
                duplicate.pop();
                duplicate.push_str(&format!(",{field:?}:null}}"));
                lines[index] = duplicate;
                assert!(
                    verify_transcript(&lines.join("\n")).is_err(),
                    "accepted duplicate {index}:{field}"
                );
            }
            for private in [
                "private_payload",
                "register",
                "instruction_pointer",
                "unknown",
            ] {
                let mut changed = record.clone();
                changed.insert(private.into(), json_scalar::number("42".into()));
                let mut lines = records.clone();
                lines[index] = render(&changed);
                assert!(verify_transcript(&lines.join("\n")).is_err());
            }
        }
    }

    #[test]
    fn binds_release_to_boot_memory_and_requires_the_exact_two_size_delta() {
        let valid = valid_transcript();
        for (from, to) in [
            ("\"usable_bytes\":81920000", "\"usable_bytes\":81924096"),
            (
                "\"free_frames\":20000,\"resources_restored\"",
                "\"free_frames\":19999,\"resources_restored\"",
            ),
            (
                "\"unavailable_frames\":1028576",
                "\"unavailable_frames\":1028575",
            ),
        ] {
            let changed = valid.replace(from, to);
            assert_ne!(changed, valid);
            assert!(verify_transcript(&changed).is_err());
        }
        let small = verify_records(&valid).unwrap();
        let large = run_evidence {
            free_frames: 77344,
            unavailable_frames: 971232,
            ..small
        };
        verify_size_evidence(small, large).unwrap();
        for changed in [
            small,
            run_evidence {
                free_frames: large.free_frames + 1,
                ..large
            },
            run_evidence {
                unavailable_frames: large.unavailable_frames + 1,
                ..large
            },
            run_evidence {
                total_frames: large.total_frames + 1,
                ..large
            },
        ] {
            assert!(verify_size_evidence(small, changed).is_err());
        }
        assert!(verify_size_evidence(large, small).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn requires_successful_qemu_exit_and_valid_transcript_independently() {
        use std::os::unix::process::ExitStatusExt;
        let valid = valid_transcript();
        verify_output(ExitStatus::from_raw(qemu_success_status << 8), &valid).unwrap();
        for status in [0, 1 << 8, 32 << 8, 35 << 8, 9] {
            assert!(verify_output(ExitStatus::from_raw(status), &valid).is_err());
        }
        assert!(verify_output(ExitStatus::from_raw(qemu_success_status << 8), "").is_err());
    }

    #[test]
    fn feature_precedence_and_normal_exclusion_match_the_owned_runtime() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        assert!(selected(&[
            feature,
            "owned_dispatcher_test",
            "owned_multi_region_test",
            "owned_source_test",
            "owned_tls_test",
            "dispatcher_test",
            "scheduler_test",
            "smoke_test",
            "workspace_dev",
            "owned_workspace_test",
            "owned_workspace_cpu_test",
        ]));
        assert!(!selected(all_kernel_features));
        for conflict in all_kernel_features.iter().filter(|value| {
            !matches!(
                **value,
                "application_profile_test"
                    | "owned_dispatcher_test"
                    | "owned_multi_region_test"
                    | "owned_source_test"
                    | "owned_tls_test"
                    | "dispatcher_test"
                    | "scheduler_test"
                    | "smoke_test"
                    | "workspace_dev"
                    | "owned_workspace_test"
                    | "owned_workspace_cpu_test"
            )
        }) {
            assert!(!selected(&[feature, conflict]));
        }
        for other in [
            owned_dispatcher_smoke::feature,
            owned_multi_region_smoke::feature,
            owned_source_smoke::feature,
            owned_tls_smoke::feature,
            dispatcher_smoke::feature,
            workspace_dev::feature,
            owned_workspace::feature,
            desktop_dev::feature,
        ] {
            let features = [feature, other];
            assert!(!owned_dispatcher_smoke::selected(&features));
            assert!(!owned_multi_region_smoke::selected(&features));
            assert!(!owned_source_smoke::selected(&features));
            assert!(!owned_tls_smoke::selected(&features));
            assert!(!dispatcher_smoke::selected(&features));
            assert!(!workspace_dev::selected(&features));
            assert!(!owned_workspace::selected(&features));
            assert!(!desktop_dev::selected(&features));
        }
        for marker in [
            "kernel.application_profile_test.ready",
            "application_profile_runtime",
            "application_cpu",
            "application_fault",
            "x86_64_application_profile_gate_entry",
            "x86_64_application_profile_faulting_start",
        ] {
            assert!(verify_absent(marker, &[]).is_err());
            assert!(verify_absent("", marker.as_bytes()).is_err());
        }
        for canary in private_canaries {
            assert!(verify_absent("", &canary.to_le_bytes()).is_err());
        }
        assert!(verify_absent("bootstrap_shell", b"Threshold").is_ok());
    }
}
