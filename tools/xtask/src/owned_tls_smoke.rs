//! Strict oracle for task-owned FS bases across native execution and retirement.
use super::*;

pub(super) const feature: &str = "owned_tls_test";
pub(super) const transcript_byte_limit: usize = 64 * 1024;
const record_byte_limit: usize = 4096;
const tls_timeout: Duration = Duration::from_secs(60);
const private_canaries: [u64; 8] = [
    0x61a7_c0de_f17e_0001,
    0x61a7_c0de_f17e_0002,
    0x9ee7_c0de_ba5e_0002,
    0x9ee7_c0de_ba5e_0003,
    0x7451_53aa_f00d_0001,
    0x7451_53aa_f00d_0002,
    0x7451_53bb_face_0001,
    0x7451_53bb_face_0002,
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
    "kernel.owned_tls_test.rollback",
    "kernel.owned_tls_test.ready",
    "kernel.owned_tls_test.operations",
    "kernel.owned_tls_test.progress",
    "kernel.owned_tls_test.fault_contained",
    "kernel.owned_tls_test.released",
    "kernel.owned_tls_test.passed",
];

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|value| {
            matches!(
                *value,
                "owned_source_test"
                    | "mixed_workspace_test"
                    | "native_editor_workspace_test"
                    | "fault_test"
                    | "ny_object_test"
                    | "native_application_test"
                    | "native_runtime_test"
                    | "native_program_test"
                    | "native_manifest_test"
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
    println!("owned TLS image inspected: {}", image.path.display());
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let build = repository_root()?.join("build");
    let logs = [
        build.join("qemu_owned_tls_96M.log"),
        build.join("qemu_owned_tls_320M.log"),
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
            "owned TLS proved unpublished rollback, task-owned FS changes and denials, isolated TLS memory through 64 preemptions, contained fault, 16 survivor ticks, and exact TLS cleanup at 96M and 320M (free frames: {} -> {})",
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
    println!("owned TLS QEMU RAM: {memory_size}");
    run_qemu_with_transcript(command, tls_timeout, |status, transcript| {
        let verified = verify_output(status, transcript)?;
        fs::write(log, transcript)
            .map_err(|error| format!("retain verified owned-tls transcript: {error}"))?;
        evidence = Some(verified);
        Ok(())
    })?;
    evidence.ok_or_else(|| format!("owned TLS produced no evidence at {memory_size}"))
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "owned_tls_test",
        "owned_tls_runtime",
        "task_tls",
        "x86_64_task_tls",
        "x86_64_owned_tls",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains owned TLS proof implementation".into());
        }
    }
    for canary in private_canaries {
        if bytes.windows(8).any(|value| value == canary.to_le_bytes()) {
            return Err("normal image retains an owned TLS private fixture".into());
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
            "QEMU did not report owned-tls success (process status: {:?})",
            status.code()
        ));
    }
    verify_records(transcript)
}

pub(super) fn verify_transcript(transcript: &str) -> Result<(), String> {
    verify_records(transcript).map(|_| ())
}

fn verify_records(transcript: &str) -> Result<run_evidence, String> {
    if transcript.len() > transcript_byte_limit {
        return Err("owned TLS transcript exceeds its byte budget".into());
    }
    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        if line.len() > record_byte_limit {
            return Err(format!(
                "owned TLS record {} exceeds its byte budget",
                index + 1
            ));
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "owned TLS has unstructured serial line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line)
            .parse_record()
            .map_err(|error| format!("malformed owned TLS serial record {}: {error}", index + 1))?;
        let event = expected_events
            .get(records.len())
            .ok_or("owned TLS transcript contains extra records")?;
        require_string(&record, event, "event", event)?;
        require_string(&record, event, "level", "info")?;
        require_number(&record, event, "schema", 1)?;
        records.push(record);
    }
    if records.len() != expected_events.len() {
        return Err("owned TLS transcript is missing lifecycle records".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    let free_frames = required_number(&records[4], expected_events[4], "free_frames")?;
    if free_frames < 204 {
        return Err("owned TLS boot budget cannot cover its admitted frames".into());
    }
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
            "fs_only",
            "fsgsbase_disabled",
            "gs_zero",
            "kernel_gs_zero",
        ],
    )?;
    validate_record(
        &records[10],
        expected_events[10],
        &[
            ("tasks", 2),
            ("calls", 20),
            ("accepted_sets", 8),
            ("rejected_sets", 4),
            ("gets", 8),
        ],
        &["initial_zero", "full_width_bases", "base_changes_preserved"],
    )?;
    validate_record(
        &records[11],
        expected_events[11],
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
            "tls_memory_correlated",
        ],
    )?;
    validate_record(
        &records[12],
        expected_events[12],
        &[
            ("vector", 6),
            ("task", 0),
            ("peer", 1),
            ("survivor_ticks", 16),
        ],
        &["peer_continued", "survivor_tls_progress"],
    )?;
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
            "tls_storage_cleared",
            "tls_hardware_restored",
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
            ("tls_entries", 106),
            ("tls_returns", 106),
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
        return Err("owned TLS RAM-size evidence does not match the 224-MiB frame delta".into());
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
            r#"{"schema":1,"event":"kernel.owned_tls_test.rollback","level":"info","unpublished_cancellations":2,"stack_clear_failures":2,"cleanup_retries":2,"resources_restored":true}"#,
            r#"{"schema":1,"event":"kernel.owned_tls_test.ready","level":"info","tasks":2,"quantum_switches":64,"survivor_ticks":16,"data_frames":184,"table_frames":20,"active_leases":22,"bound_leases":22,"regions_per_task":9,"executable_regions":2,"rejected_return_sites":8,"owned_low_roots":true,"prepared_launch":true,"high_alias_stacks":true,"initialized_readback":true,"fs_only":true,"fsgsbase_disabled":true,"gs_zero":true,"kernel_gs_zero":true}"#,
            r#"{"schema":1,"event":"kernel.owned_tls_test.operations","level":"info","tasks":2,"calls":20,"accepted_sets":8,"rejected_sets":4,"gets":8,"initial_zero":true,"full_width_bases":true,"base_changes_preserved":true}"#,
            r#"{"schema":1,"event":"kernel.owned_tls_test.progress","level":"info","preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"task_0_progress":true,"task_1_progress":true,"canaries_intact":true,"tls_memory_correlated":true,"cross_region_gates":2}"#,
            r#"{"schema":1,"event":"kernel.owned_tls_test.fault_contained","level":"info","vector":6,"task":0,"peer":1,"peer_continued":true,"survivor_ticks":16,"survivor_tls_progress":true}"#,
            r#"{"schema":1,"event":"kernel.owned_tls_test.released","level":"info","free_frames":20000,"resources_restored":true,"storage_cleared":true,"kernel_continuation_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true,"tls_storage_cleared":true,"tls_hardware_restored":true}"#,
            r#"{"schema":1,"event":"kernel.owned_tls_test.passed","level":"info","timer_ticks":80,"dispatches":66,"preemptions":64,"faults":1,"exits":1,"tls_entries":106,"tls_returns":106}"#,
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
    fn accepts_exact_owned_tls_evidence_and_portable_line_endings() {
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
        for prefix in [
            "kernel.owned_source_test.",
            "kernel.owned_multi_region_test.",
            "kernel.owned_dispatcher_test.",
        ] {
            assert!(verify_transcript(&valid.replace("kernel.owned_tls_test.", prefix)).is_err());
        }
        for extra in [",\"reserved\":0", ",\"initialized_bytes\":\"private\"", ","] {
            let changed = valid.replace(
                "\"unpublished_cancellations\":2",
                &format!("\"unpublished_cancellations\":2{extra}"),
            );
            assert!(verify_transcript(&changed).is_err());
        }
    }

    #[test]
    fn transcript_and_record_byte_budgets_bound_rejection_before_parsing() {
        let valid = valid_transcript();
        assert!(verify_transcript(&format!("{}{}", " ".repeat(record_byte_limit), valid)).is_err());
        assert!(
            verify_transcript(&format!("{}{}", "\n".repeat(transcript_byte_limit), valid)).is_err()
        );
        let exact_total = format!(
            "{}{}",
            "\n".repeat(transcript_byte_limit - valid.len()),
            valid
        );
        verify_transcript(&exact_total).unwrap();
        assert!(verify_transcript(&(exact_total + "\n")).is_err());
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
        assert!(!application_profile_smoke::selected(&[feature]));
        assert!(selected(&[
            feature,
            "owned_multi_region_test",
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
                "owned_tls_test"
                    | "owned_multi_region_test"
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
        assert!(owned_source_smoke::selected(&[
            feature,
            owned_source_smoke::feature
        ]));
        assert!(!selected(&[feature, owned_source_smoke::feature]));
        for other in [
            owned_multi_region_smoke::feature,
            owned_dispatcher_smoke::feature,
            dispatcher_smoke::feature,
            workspace_dev::feature,
            owned_workspace::feature,
            desktop_dev::feature,
        ] {
            let features = [feature, other];
            assert!(!owned_multi_region_smoke::selected(&features));
            assert!(!owned_dispatcher_smoke::selected(&features));
            assert!(!dispatcher_smoke::selected(&features));
            assert!(!workspace_dev::selected(&features));
            assert!(!owned_workspace::selected(&features));
            assert!(!desktop_dev::selected(&features));
        }
        for marker in [
            "kernel.owned_tls_test.ready",
            "owned_tls_runtime",
            "task_tls",
            "x86_64_task_tls_entry",
            "x86_64_owned_tls_gate_entry",
            "x86_64_owned_tls_faulting_start",
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
