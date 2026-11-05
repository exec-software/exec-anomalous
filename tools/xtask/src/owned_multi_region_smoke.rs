//! Strict oracle for owned multi-region roots and exact executable intervals.
use super::*;

pub(super) const feature: &str = "owned_multi_region_test";
const multi_region_timeout: Duration = Duration::from_secs(60);
const private_canaries: [u64; 4] = [
    0x61a7_c0de_f17e_0001,
    0x61a7_c0de_f17e_0002,
    0x9ee7_c0de_ba5e_0002,
    0x9ee7_c0de_ba5e_0003,
];
const expected_events: [&str; 15] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.owned_multi_region_test.rollback",
    "kernel.owned_multi_region_test.ready",
    "kernel.owned_multi_region_test.progress",
    "kernel.owned_multi_region_test.fault_contained",
    "kernel.owned_multi_region_test.survivor",
    "kernel.owned_multi_region_test.released",
    "kernel.owned_multi_region_test.passed",
];

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|value| {
            matches!(
                *value,
                "owned_source_test"
                    | "owned_tls_test"
                    | "mixed_workspace_test"
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
                    | "application_profile_test"
                    | "circle_test"
                    | "control_test"
                    | "rite_test"
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
        "owned multi-region image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let build = repository_root()?.join("build");
    let logs = [
        build.join("qemu_owned_multi_region_96M.log"),
        build.join("qemu_owned_multi_region_320M.log"),
    ];
    for log in &logs {
        remove_optional_file(log)?;
    }
    let result = (|| {
        build_and_inspect_normal_image()?;
        let image = self::build()?;
        let small = run_size(&image.path, "96M", &logs[0])?;
        let large = run_size(&image.path, "320M", &logs[1])?;
        verify_size_evidence(small, large)?;
        println!(
            "owned multi-region proved unpublished rollback, nine regions per task, two cross-region gates, 64 preemptions, fault containment, 16 survivor ticks, and exact cleanup at 96M and 320M (free frames: {} -> {})",
            small.free_frames, large.free_frames
        );
        Ok(())
    })();
    if result.is_err() {
        for log in &logs {
            remove_optional_file(log)?;
        }
    }
    result
}

fn run_size(image: &Path, memory_size: &str, log: &Path) -> Result<run_evidence, String> {
    let mut command = qemu_command_with_memory(image, memory_size)?;
    command.args([
        "-display",
        "none",
        "-monitor",
        "none",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ]);
    let mut evidence = None;
    println!("owned multi-region QEMU RAM: {memory_size}");
    run_qemu_with_transcript(command, multi_region_timeout, |status, transcript| {
        let verified = verify_output(status, transcript)?;
        fs::write(log, transcript)
            .map_err(|error| format!("retain verified owned-multi-region transcript: {error}"))?;
        evidence = Some(verified);
        Ok(())
    })?;
    evidence.ok_or_else(|| format!("owned multi-region produced no evidence at {memory_size}"))
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "owned_multi_region_test",
        "owned_multi_region_runtime",
        "x86_64_owned_multi_region",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains owned multi-region proof implementation".into());
        }
    }
    for canary in private_canaries {
        if bytes.windows(8).any(|value| value == canary.to_le_bytes()) {
            return Err("normal image retains an owned multi-region private fixture".into());
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
            "QEMU did not report owned-multi-region success (process status: {:?})",
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
                "owned multi-region has unstructured serial line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed owned multi-region serial record {}: {error}",
                index + 1
            )
        })?;
        let event = expected_events
            .get(records.len())
            .ok_or("owned multi-region transcript contains extra records")?;
        require_string(&record, event, "event", event)?;
        require_string(&record, event, "level", "info")?;
        require_number(&record, event, "schema", 1)?;
        records.push(record);
    }
    if records.len() != expected_events.len() {
        return Err("owned multi-region transcript is missing lifecycle records".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    validate_record(
        &records[8],
        expected_events[8],
        &[
            ("unpublished_cancellations", 2),
            ("stack_clear_failures", 2),
            ("cleanup_retries", 2),
        ],
        &["resources_restored"],
    )?;
    validate_record(
        &records[9],
        expected_events[9],
        &[
            ("tasks", 2),
            ("quantum_switches", 64),
            ("survivor_ticks", 16),
            ("data_frames", 184),
            ("table_frames", 20),
            ("active_leases", 22),
            ("bound_leases", 22),
            ("regions_per_task", 9),
            ("executable_regions", 2),
            ("rejected_return_sites", 8),
        ],
        &[
            "owned_low_roots",
            "prepared_launch",
            "high_alias_stacks",
            "initialized_readback",
        ],
    )?;
    validate_record(
        &records[10],
        expected_events[10],
        &[
            ("preemptions", 64),
            ("task_0_quanta", 32),
            ("task_1_quanta", 32),
            ("cross_region_gates", 2),
        ],
        &[
            "task_0_progress",
            "task_1_progress",
            "canaries_intact",
            "noninitial_stacks",
        ],
    )?;
    validate_record(
        &records[11],
        expected_events[11],
        &[("vector", 6), ("task", 0), ("peer", 1)],
        &["peer_continued"],
    )?;
    validate_record(
        &records[12],
        expected_events[12],
        &[("ticks", 16)],
        &["progress", "dispatches_unchanged"],
    )?;
    let free_frames = required_number(&records[4], expected_events[4], "free_frames")?;
    if free_frames < 204 {
        return Err("owned multi-region boot budget cannot cover its admitted frames".into());
    }
    validate_record(
        &records[13],
        expected_events[13],
        &[("free_frames", free_frames)],
        &[
            "resources_restored",
            "storage_cleared",
            "kernel_continuation_restored",
            "bootstrap_restored",
            "tss_restored",
            "ts_restored",
            "alias_removed",
        ],
    )?;
    validate_record(
        &records[14],
        expected_events[14],
        &[
            ("timer_ticks", 80),
            ("dispatches", 66),
            ("preemptions", 64),
            ("faults", 1),
            ("exits", 1),
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
            "owned multi-region RAM-size evidence does not match the 224-MiB frame delta".into(),
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
            r#"{"schema":1,"event":"kernel.owned_multi_region_test.rollback","level":"info","unpublished_cancellations":2,"stack_clear_failures":2,"cleanup_retries":2,"resources_restored":true}"#,
            r#"{"schema":1,"event":"kernel.owned_multi_region_test.ready","level":"info","tasks":2,"quantum_switches":64,"survivor_ticks":16,"data_frames":184,"table_frames":20,"active_leases":22,"bound_leases":22,"regions_per_task":9,"executable_regions":2,"rejected_return_sites":8,"owned_low_roots":true,"prepared_launch":true,"high_alias_stacks":true,"initialized_readback":true}"#,
            r#"{"schema":1,"event":"kernel.owned_multi_region_test.progress","level":"info","preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"cross_region_gates":2,"task_0_progress":true,"task_1_progress":true,"canaries_intact":true,"noninitial_stacks":true}"#,
            r#"{"schema":1,"event":"kernel.owned_multi_region_test.fault_contained","level":"info","vector":6,"task":0,"peer":1,"peer_continued":true}"#,
            r#"{"schema":1,"event":"kernel.owned_multi_region_test.survivor","level":"info","ticks":16,"progress":true,"dispatches_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.owned_multi_region_test.released","level":"info","free_frames":20000,"resources_restored":true,"storage_cleared":true,"kernel_continuation_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true}"#,
            r#"{"schema":1,"event":"kernel.owned_multi_region_test.passed","level":"info","timer_ticks":80,"dispatches":66,"preemptions":64,"faults":1,"exits":1}"#,
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
    fn accepts_exact_owned_multi_region_evidence_and_portable_line_endings() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
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
        for extra in ["private payload", "{}", "{\"schema\":1", records[14]] {
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
    fn rejects_reserved_json_formats_and_coherent_old_profile_evidence() {
        let valid = valid_transcript();
        for value in [
            "0184", "+184", "-184", "184.0", "1.84e2", "\"184\"", "true", "null",
        ] {
            let changed = valid.replace("\"data_frames\":184", &format!("\"data_frames\":{value}"));
            assert_ne!(changed, valid);
            assert!(
                verify_transcript(&changed).is_err(),
                "accepted numeric spelling {value}"
            );
        }
        for value in ["1", "\"true\"", "True", "false", "null"] {
            let changed = valid.replace(
                "\"initialized_readback\":true",
                &format!("\"initialized_readback\":{value}"),
            );
            assert_ne!(changed, valid);
            assert!(
                verify_transcript(&changed).is_err(),
                "accepted readback spelling {value}"
            );
        }
        let old_shape = valid
            .replace(
                "\"unpublished_cancellations\":2",
                "\"partial_admissions\":1,\"unstarted_withdrawals\":1",
            )
            .replace("\"data_frames\":184", "\"data_frames\":18")
            .replace("\"table_frames\":20", "\"table_frames\":18")
            .replace("\"active_leases\":22", "\"active_leases\":10")
            .replace("\"bound_leases\":22", "\"bound_leases\":10")
            .replace(",\"regions_per_task\":9", "")
            .replace(",\"executable_regions\":2", "")
            .replace(",\"rejected_return_sites\":8", "")
            .replace(",\"initialized_readback\":true", "")
            .replace(",\"cross_region_gates\":2", "");
        assert!(verify_transcript(&old_shape).is_err());
        assert!(
            verify_transcript(&valid.replace(
                "kernel.owned_multi_region_test.",
                "kernel.owned_dispatcher_test."
            ))
            .is_err()
        );
        for extra in [",\"reserved\":0", ",\"initialized_bytes\":\"private\"", ","] {
            let changed = valid.replace(
                "\"unpublished_cancellations\":2",
                &format!("\"unpublished_cancellations\":2{extra}"),
            );
            assert!(verify_transcript(&changed).is_err());
        }
    }

    #[test]
    fn boot_budget_must_cover_all_owned_user_stack_and_table_frames() {
        let valid = valid_transcript();
        for (free, accepted) in [(203u64, false), (204, true)] {
            let changed = valid
                .replace("\"free_frames\":20000", &format!("\"free_frames\":{free}"))
                .replace(
                    "\"unavailable_frames\":1028576",
                    &format!("\"unavailable_frames\":{}", memory_total_frames - free),
                )
                .replace(
                    "\"usable_bytes\":81920000",
                    &format!("\"usable_bytes\":{}", free * memory_frame_size),
                );
            assert_eq!(verify_transcript(&changed).is_ok(), accepted);
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
            "dispatcher_test",
            "scheduler_test",
            "smoke_test",
            "workspace_dev",
            "owned_workspace_test",
            "owned_workspace_cpu_test",
        ]));
        assert!(!selected(all_kernel_features));
        assert!(native_editor_workspace::selected(&[
            feature,
            native_editor_workspace::feature,
        ]));
        assert!(application_profile_smoke::selected(&[
            feature,
            application_profile_smoke::feature
        ]));
        for conflict in all_kernel_features.iter().filter(|value| {
            !matches!(
                **value,
                "owned_multi_region_test"
                    | "owned_dispatcher_test"
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
            dispatcher_smoke::feature,
            workspace_dev::feature,
            owned_workspace::feature,
            desktop_dev::feature,
        ] {
            let features = [feature, other];
            assert!(!owned_dispatcher_smoke::selected(&features));
            assert!(!dispatcher_smoke::selected(&features));
            assert!(!workspace_dev::selected(&features));
            assert!(!owned_workspace::selected(&features));
            assert!(!desktop_dev::selected(&features));
        }
        for marker in [
            "kernel.owned_multi_region_test.ready",
            "owned_multi_region_runtime",
            "x86_64_owned_multi_region_gate_entry",
            "x86_64_owned_multi_region_faulting_start",
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
