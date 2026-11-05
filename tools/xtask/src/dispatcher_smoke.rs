//! Strict oracle for the removable sustained native dispatch proof.
use super::*;

pub(super) const feature: &str = "dispatcher_test";
const private_canaries: [u64; 4] = [
    0x61a7_c0de_f17e_0001,
    0x61a7_c0de_f17e_0002,
    0x9ee7_c0de_ba5e_0002,
    0x9ee7_c0de_ba5e_0003,
];
const expected_events: [&str; 14] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.dispatcher_test.ready",
    "kernel.dispatcher_test.progress",
    "kernel.dispatcher_test.fault_contained",
    "kernel.dispatcher_test.survivor",
    "kernel.dispatcher_test.released",
    "kernel.dispatcher_test.passed",
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
                    | "owned_dispatcher_test"
                    | "owned_multi_region_test"
                    | "owned_source_test"
                    | "owned_tls_test"
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
        "native dispatcher image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let log = repository_root()?.join("build/qemu_dispatcher.log");
    remove_optional_file(&log)?;
    build_and_inspect_normal_image()?;
    let image = build()?;
    let mut command = qemu_command(&image.path)?;
    command.args([
        "-display",
        "none",
        "-monitor",
        "none",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ]);
    let result = run_qemu_with_transcript(command, smoke_timeout, |status, transcript| {
        verify_output(status, transcript)?;
        fs::write(&log, transcript)
            .map_err(|error| format!("retain verified dispatcher transcript: {error}"))
    });
    if result.is_err() {
        remove_optional_file(&log)?;
    }
    result
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in ["dispatcher_test", "x86_64_dispatcher"] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains native dispatcher proof implementation".into());
        }
    }
    for canary in private_canaries {
        if bytes.windows(8).any(|value| value == canary.to_le_bytes()) {
            return Err("normal image retains a native dispatcher private fixture".into());
        }
    }
    Ok(())
}

fn verify_output(status: ExitStatus, transcript: &str) -> Result<(), String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "QEMU did not report dispatcher-test success (process status: {:?})",
            status.code()
        ));
    }
    verify_transcript(transcript)
}

pub(super) fn verify_transcript(transcript: &str) -> Result<(), String> {
    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "dispatcher transcript contains unstructured serial content on line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line).parse_record().map_err(|error| {
            format!(
                "malformed dispatcher record on serial line {}: {error}",
                index + 1
            )
        })?;
        if records.len() == expected_events.len() {
            return Err("dispatcher transcript contains extra records".into());
        }
        let event = expected_events[records.len()];
        require_string(&record, event, "event", event)?;
        require_string(&record, event, "level", "info")?;
        require_number(&record, event, "schema", 1)?;
        records.push(record);
    }
    if records.len() != expected_events.len() {
        return Err("dispatcher transcript is missing lifecycle records".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    validate_record(
        &records[8],
        expected_events[8],
        &[
            ("tasks", 2),
            ("quantum_switches", 64),
            ("survivor_ticks", 16),
            ("data_frames", 14),
            ("table_frames", 8),
        ],
        &[],
    )?;
    validate_record(
        &records[9],
        expected_events[9],
        &[
            ("preemptions", 64),
            ("task_0_quanta", 32),
            ("task_1_quanta", 32),
        ],
        &["task_0_progress", "task_1_progress", "canaries_intact"],
    )?;
    validate_record(
        &records[10],
        expected_events[10],
        &[("vector", 6), ("task", 0), ("peer", 1)],
        &["peer_continued"],
    )?;
    validate_record(
        &records[11],
        expected_events[11],
        &[("ticks", 16)],
        &["progress", "dispatches_unchanged"],
    )?;
    let free_frames = required_number(&records[4], expected_events[4], "free_frames")?;
    validate_record(
        &records[12],
        expected_events[12],
        &[("free_frames", free_frames)],
        &[
            "resources_restored",
            "storage_cleared",
            "kernel_continuation_restored",
        ],
    )?;
    validate_record(
        &records[13],
        expected_events[13],
        &[
            ("timer_ticks", 80),
            ("dispatches", 66),
            ("preemptions", 64),
            ("faults", 1),
            ("exits", 1),
        ],
        &[],
    )
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
            r#"{"schema":1,"event":"kernel.dispatcher_test.ready","level":"info","tasks":2,"quantum_switches":64,"survivor_ticks":16,"data_frames":14,"table_frames":8}"#,
            r#"{"schema":1,"event":"kernel.dispatcher_test.progress","level":"info","preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"task_0_progress":true,"task_1_progress":true,"canaries_intact":true}"#,
            r#"{"schema":1,"event":"kernel.dispatcher_test.fault_contained","level":"info","vector":6,"task":0,"peer":1,"peer_continued":true}"#,
            r#"{"schema":1,"event":"kernel.dispatcher_test.survivor","level":"info","ticks":16,"progress":true,"dispatches_unchanged":true}"#,
            r#"{"schema":1,"event":"kernel.dispatcher_test.released","level":"info","free_frames":20000,"resources_restored":true,"storage_cleared":true,"kernel_continuation_restored":true}"#,
            r#"{"schema":1,"event":"kernel.dispatcher_test.passed","level":"info","timer_ticks":80,"dispatches":66,"preemptions":64,"faults":1,"exits":1}"#,
        ]
        .join("\n")
    }

    #[test]
    fn accepts_exact_dispatcher_lifecycle_and_portable_line_endings() {
        let valid = valid_transcript();
        verify_transcript(&valid).expect("exact dispatcher lifecycle");
        verify_transcript(&format!("\r\n{}\r\n", valid.replace('\n', "\r\n")))
            .expect("CRLF and blank lines are harmless");
    }

    #[test]
    fn rejects_missing_duplicate_reordered_and_unknown_records() {
        let valid = valid_transcript();
        let records = valid.lines().collect::<Vec<_>>();
        for index in 0..records.len() {
            let mut missing = records.clone();
            missing.remove(index);
            assert!(verify_transcript(&missing.join("\n")).is_err());
            let mut duplicated = records.clone();
            duplicated.insert(index, records[index]);
            assert!(verify_transcript(&duplicated.join("\n")).is_err());
            if index + 1 < records.len() {
                let mut reordered = records.clone();
                reordered.swap(index, index + 1);
                assert!(verify_transcript(&reordered.join("\n")).is_err());
            }
        }
        assert!(
            verify_transcript(
                &valid.replace("dispatcher_test.progress", "dispatcher_test.unknown")
            )
            .is_err()
        );
        assert!(verify_transcript(&format!("{valid}\n{}", records[13])).is_err());
    }

    #[test]
    fn rejects_wrong_counts_failed_invariants_and_unrestored_resources() {
        let valid = valid_transcript();
        for (from, to) in [
            ("\"tasks\":2", "\"tasks\":1"),
            ("\"quantum_switches\":64", "\"quantum_switches\":63"),
            ("\"survivor_ticks\":16", "\"survivor_ticks\":0"),
            ("\"data_frames\":14", "\"data_frames\":13"),
            ("\"table_frames\":8", "\"table_frames\":7"),
            ("\"preemptions\":64", "\"preemptions\":63"),
            ("\"task_0_quanta\":32", "\"task_0_quanta\":31"),
            ("\"task_1_quanta\":32", "\"task_1_quanta\":33"),
            ("\"vector\":6", "\"vector\":14"),
            ("\"task\":0", "\"task\":1"),
            ("\"peer\":1", "\"peer\":0"),
            ("\"ticks\":16", "\"ticks\":15"),
            ("\"timer_ticks\":80", "\"timer_ticks\":79"),
            ("\"dispatches\":66", "\"dispatches\":67"),
            ("\"faults\":1", "\"faults\":0"),
            ("\"exits\":1", "\"exits\":0"),
            (
                "\"free_frames\":20000,\"resources_restored\"",
                "\"free_frames\":19999,\"resources_restored\"",
            ),
            ("\"usable_bytes\":81920000", "\"usable_bytes\":81924096"),
        ] {
            let changed = valid.replace(from, to);
            assert_ne!(changed, valid, "mutation did not apply: {from}");
            assert!(
                verify_transcript(&changed).is_err(),
                "accepted mutation: {from}"
            );
        }
        for field in [
            "task_0_progress",
            "task_1_progress",
            "canaries_intact",
            "peer_continued",
            "progress",
            "dispatches_unchanged",
            "resources_restored",
            "storage_cleared",
            "kernel_continuation_restored",
        ] {
            assert!(
                verify_transcript(
                    &valid.replace(&format!("\"{field}\":true"), &format!("\"{field}\":false"))
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rejects_extra_missing_duplicate_private_fields_and_unstructured_content() {
        let valid = valid_transcript();
        for field in [
            "unknown",
            "private_payload",
            "register",
            "instruction_pointer",
        ] {
            assert!(
                verify_transcript(
                    &valid.replace("\"tasks\":2", &format!("\"tasks\":2,\"{field}\":42"))
                )
                .is_err()
            );
        }
        for (from, to) in [
            ("\"tasks\":2,", ""),
            ("\"tasks\":2", "\"tasks\":2,\"tasks\":2"),
            ("\"schema\":1", "\"schema\":2"),
            ("\"level\":\"info\"", "\"level\":\"fatal\""),
            ("\"level\":\"info\"", "\"level\":\"debug\""),
            ("\"tasks\":2", "\"tasks\":\"2\""),
            ("\"canaries_intact\":true", "\"canaries_intact\":1"),
        ] {
            assert!(verify_transcript(&valid.replace(from, to)).is_err());
        }
        for suffix in ["private payload", "{\"schema\":1", "{}"] {
            assert!(verify_transcript(&format!("{valid}\n{suffix}")).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn requires_successful_debug_exit_and_valid_transcript_independently() {
        use std::os::unix::process::ExitStatusExt;
        let valid = valid_transcript();
        verify_output(ExitStatus::from_raw(qemu_success_status << 8), &valid)
            .expect("status 33 and valid proof");
        for status in [0, 1 << 8, 32 << 8, 35 << 8, 9] {
            assert!(verify_output(ExitStatus::from_raw(status), &valid).is_err());
        }
        assert!(verify_output(ExitStatus::from_raw(qemu_success_status << 8), "").is_err());
    }

    #[test]
    fn feature_is_removable_and_defers_to_higher_priority_proofs() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        assert!(selected(&[
            feature,
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
        for conflict in all_kernel_features.iter().filter(|value| {
            !matches!(
                **value,
                "dispatcher_test"
                    | "scheduler_test"
                    | "smoke_test"
                    | "workspace_dev"
                    | "owned_workspace_test"
                    | "owned_workspace_cpu_test"
            )
        }) {
            assert!(!selected(&[feature, conflict]));
        }
        for marker in [
            "kernel.dispatcher_test.ready",
            "x86_64_dispatcher_entry",
            "x86_64_dispatcher_payload",
        ] {
            assert!(verify_absent(marker, &[]).is_err());
            assert!(verify_absent("", marker.as_bytes()).is_err());
        }
        assert!(verify_absent("bootstrap_shell", b"Threshold").is_ok());
        for canary in private_canaries {
            assert!(verify_absent("", &canary.to_le_bytes()).is_err());
        }
    }
}
