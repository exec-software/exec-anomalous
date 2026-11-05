//! Strict oracle for copied and sealed sources surviving producer retirement.
use super::*;

pub(super) const feature: &str = "owned_source_test";
pub(super) const transcript_byte_limit: usize = 64 * 1024;
const record_byte_limit: usize = 4096;
const source_timeout: Duration = Duration::from_secs(60);
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
    "kernel.owned_source_test.rollback",
    "kernel.owned_source_test.producer",
    "kernel.owned_source_test.retained",
    "kernel.owned_source_test.consumer",
    "kernel.owned_source_test.progress",
    "kernel.owned_source_test.fault_contained",
    "kernel.owned_source_test.released",
    "kernel.owned_source_test.passed",
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
    println!("owned source image inspected: {}", image.path.display());
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let build = repository_root()?.join("build");
    let logs = [
        build.join("qemu_owned_source_96M.log"),
        build.join("qemu_owned_source_320M.log"),
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
            "owned source proved bounded source upload, producer retirement, sealed-source consumption, two complete dispatch lifetimes, and exact cleanup at 96M and 320M (free frames: {} -> {})",
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
    println!("owned source QEMU RAM: {memory_size}");
    run_qemu_with_transcript(command, source_timeout, |status, transcript| {
        let verified = verify_output(status, transcript)?;
        fs::write(log, transcript)
            .map_err(|error| format!("retain verified owned-source transcript: {error}"))?;
        evidence = Some(verified);
        Ok(())
    })?;
    evidence.ok_or_else(|| format!("owned source produced no evidence at {memory_size}"))
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "owned_source",
        "owned_source_test",
        "owned_source_runtime",
        "owned_source_plan",
        "x86_64_owned_source",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains owned source proof implementation".into());
        }
    }
    for canary in private_canaries {
        if bytes.windows(8).any(|value| value == canary.to_le_bytes()) {
            return Err("normal image retains an owned source private fixture".into());
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
            "QEMU did not report owned-source success (process status: {:?})",
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
        return Err("owned source transcript exceeds its byte budget".into());
    }
    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        if line.len() > record_byte_limit {
            return Err(format!(
                "owned source record {} exceeds its byte budget",
                index + 1
            ));
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "owned source has unstructured serial line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed owned source serial record {}: {error}",
                index + 1
            )
        })?;
        let event = expected_events
            .get(records.len())
            .ok_or("owned source transcript contains extra records")?;
        require_string(&record, event, "event", event)?;
        require_string(&record, event, "level", "info")?;
        require_number(&record, event, "schema", 1)?;
        records.push(record);
    }
    if records.len() != expected_events.len() {
        return Err("owned source transcript is missing lifecycle records".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    let free_frames = required_number(&records[4], expected_events[4], "free_frames")?;
    // A complete nine-region pair and its stacks/tables coexist with two sealed
    // two-page sources. Source retention cannot borrow from the cleanup budget.
    if free_frames < 208 {
        return Err(
            "owned source boot budget cannot cover its admitted and retained frames".into(),
        );
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
            ("sources", 2),
            ("source_bytes", 7048),
            ("chunks_per_source", 4),
            ("denials_per_task", 5),
            ("uploaded_bytes", 14096),
            ("original_bytes_overwritten", 10000),
        ],
        &["original_zero_verified"],
    )?;
    validate_record(
        &records[10],
        expected_events[10],
        &[
            ("data_frames", 4),
            ("active_leases", 2),
            ("bound_leases", 2),
        ],
        &[
            "producer_resources_released",
            "producer_identities_retired",
            "alias_removed",
        ],
    )?;
    validate_record(
        &records[11],
        expected_events[11],
        &[
            ("tasks", 2),
            ("regions_per_task", 9),
            ("executable_regions", 2),
            ("pattern_bytes_per_task", 5000),
        ],
        &["initialized_readback", "pattern_verified", "sealed_only"],
    )?;
    validate_record(
        &records[12],
        expected_events[12],
        &[
            ("lifetimes", 2),
            ("preemptions", 128),
            ("task_quanta_per_lifetime", 32),
            ("cross_region_gates", 4),
        ],
        &["canaries_intact", "noninitial_stacks"],
    )?;
    validate_record(
        &records[13],
        expected_events[13],
        &[
            ("faults", 2),
            ("vector", 6),
            ("survivor_ticks_per_lifetime", 16),
        ],
        &["peers_continued"],
    )?;
    validate_record(
        &records[14],
        expected_events[14],
        &[
            ("free_frames", free_frames),
            ("source_clear_failures", 2),
            ("source_cleanup_retries", 2),
        ],
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
        &records[15],
        expected_events[15],
        &[
            ("timer_ticks", 160),
            ("dispatches", 132),
            ("preemptions", 128),
            ("faults", 2),
            ("exits", 2),
            ("sources", 2),
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
        return Err("owned source RAM-size evidence does not match the 224-MiB frame delta".into());
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
            r#"{"schema":1,"event":"kernel.owned_source_test.rollback","level":"info","unpublished_cancellations":2,"stack_clear_failures":2,"cleanup_retries":2,"resources_restored":true}"#,
            r#"{"schema":1,"event":"kernel.owned_source_test.producer","level":"info","tasks":2,"sources":2,"source_bytes":7048,"chunks_per_source":4,"denials_per_task":5,"uploaded_bytes":14096,"original_bytes_overwritten":10000,"original_zero_verified":true}"#,
            r#"{"schema":1,"event":"kernel.owned_source_test.retained","level":"info","data_frames":4,"active_leases":2,"bound_leases":2,"producer_resources_released":true,"producer_identities_retired":true,"alias_removed":true}"#,
            r#"{"schema":1,"event":"kernel.owned_source_test.consumer","level":"info","tasks":2,"regions_per_task":9,"executable_regions":2,"initialized_readback":true,"pattern_bytes_per_task":5000,"pattern_verified":true,"sealed_only":true}"#,
            r#"{"schema":1,"event":"kernel.owned_source_test.progress","level":"info","lifetimes":2,"preemptions":128,"task_quanta_per_lifetime":32,"canaries_intact":true,"noninitial_stacks":true,"cross_region_gates":4}"#,
            r#"{"schema":1,"event":"kernel.owned_source_test.fault_contained","level":"info","faults":2,"vector":6,"survivor_ticks_per_lifetime":16,"peers_continued":true}"#,
            r#"{"schema":1,"event":"kernel.owned_source_test.released","level":"info","free_frames":20000,"resources_restored":true,"source_clear_failures":2,"source_cleanup_retries":2,"storage_cleared":true,"kernel_continuation_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true}"#,
            r#"{"schema":1,"event":"kernel.owned_source_test.passed","level":"info","timer_ticks":160,"dispatches":132,"preemptions":128,"faults":2,"exits":2,"sources":2}"#,
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
    fn accepts_exact_owned_source_evidence_and_portable_line_endings() {
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
    fn rejects_reserved_formats_and_old_profile_events() {
        let valid = valid_transcript();
        for value in ["04", "+4", "-4", "4.0", "4e0", "\"4\"", "true", "null"] {
            let changed = valid.replace(
                "\"chunks_per_source\":4",
                &format!("\"chunks_per_source\":{value}"),
            );
            assert_ne!(changed, valid);
            assert!(
                verify_transcript(&changed).is_err(),
                "accepted numeric spelling {value}"
            );
        }
        for value in ["1", "\"true\"", "True", "false", "null"] {
            let changed =
                valid.replace("\"sealed_only\":true", &format!("\"sealed_only\":{value}"));
            assert_ne!(changed, valid);
            assert!(
                verify_transcript(&changed).is_err(),
                "accepted sealed-only spelling {value}"
            );
        }
        for prefix in [
            "kernel.owned_dispatcher_test.",
            "kernel.owned_multi_region_test.",
        ] {
            assert!(
                verify_transcript(&valid.replace("kernel.owned_source_test.", prefix)).is_err()
            );
        }
        for extra in [
            ",\"reserved\":0",
            ",\"source_bytes_private\":\"private\"",
            ",",
        ] {
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
    fn boot_budget_includes_sources_retained_with_a_complete_consumer_pair() {
        let valid = valid_transcript();
        for (free, accepted) in [(207u64, false), (208, true)] {
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
            "owned_tls_test",
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
                "owned_source_test"
                    | "owned_tls_test"
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
        for other in [
            owned_tls_smoke::feature,
            owned_multi_region_smoke::feature,
            owned_dispatcher_smoke::feature,
            dispatcher_smoke::feature,
            workspace_dev::feature,
            owned_workspace::feature,
            desktop_dev::feature,
        ] {
            let features = [feature, other];
            assert!(!owned_tls_smoke::selected(&features));
            assert!(!owned_multi_region_smoke::selected(&features));
            assert!(!owned_dispatcher_smoke::selected(&features));
            assert!(!dispatcher_smoke::selected(&features));
            assert!(!workspace_dev::selected(&features));
            assert!(!owned_workspace::selected(&features));
            assert!(!desktop_dev::selected(&features));
        }
        for marker in [
            "kernel.owned_source_test.producer",
            "kernel.owned_source_test.retained",
            "kernel.owned_source_test.consumer",
            "owned_source_runtime",
            "owned_source_plan",
            "owned_source::sealed",
            "x86_64_owned_source_gate_entry",
            "x86_64_owned_source_faulting_start",
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
