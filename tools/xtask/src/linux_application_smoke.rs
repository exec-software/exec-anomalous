//! Strict native baseline plus Linux startup/SYSCALL record-device comparison.
use super::*;
use native_application_smoke::{run_evidence, smoke_profile};

pub(super) const feature: &str = "linux_application_test";
const loaded_event: &str = "kernel.linux_application_test.loaded";
const scenario_event: &str = "kernel.linux_application_test.scenario";
const passed_event: &str = "kernel.linux_application_test.passed";
const profile: smoke_profile = smoke_profile {
    log_prefix: "linux_application",
    artifact_prefix: "linux-application",
    records: 25,
    verify: verify_records,
    verify_frame: native_application_smoke::verify_frame,
};

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.contains(&"veil_application_test")
        && !features.contains(&"shared_veil_test")
        && !features.contains(&"linux_tls_test")
        && !features.contains(&"linux_loader_entry_test")
        && elf_application_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|value| {
                    *value != feature
                        && *value != elf_document_smoke::feature
                        && *value != native_relaunch_smoke::feature
                        && *value != elf_relaunch_smoke::feature
                })
                .chain(core::iter::once(elf_application_smoke::feature))
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!(
        "Linux application image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let directory = native_application_smoke::prepare_smoke(profile)?;
    build_and_inspect_normal_image()?;
    let image = self::build()?;
    native_application_smoke::smoke_image(&image.path, &directory, profile)
}

pub(super) fn verify_transcript(transcript: &str) -> Result<(), String> {
    verify_records(transcript).map(|_| ())
}

fn verify_records(transcript: &str) -> Result<run_evidence, String> {
    let lines = transcript
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != profile.records {
        return Err(
            "Linux application requires exactly 21 native records followed by four Linux records"
                .into(),
        );
    }
    // Do not filter arbitrary Linux-prefixed events out of the transcript. Their
    // exact trailing positions are part of the lifecycle contract, and the
    // original native oracle must accept every preceding record unchanged.
    let evidence = native_application_smoke::verify_records(&lines[..21].join("\n"))?;
    let loaded = json_parser::new(lines[21]).parse_record()?;
    require_control_header(
        &loaded,
        loaded_event,
        &[
            "loader_tasks",
            "calls",
            "denials",
            "bad_copies",
            "invalid",
            "accepted",
            "plan_bytes",
            "initialized_rw",
            "zero_fill_verified",
            "loader_reclaimed",
        ],
    )?;
    for (field, expected) in [
        ("loader_tasks", 2),
        ("calls", 15),
        ("denials", 1),
        ("bad_copies", 1),
        ("invalid", 8),
        ("accepted", 1),
        ("plan_bytes", 96),
    ] {
        require_number(&loaded, loaded_event, field, expected)?;
    }
    for field in ["initialized_rw", "zero_fill_verified", "loader_reclaimed"] {
        require_bool(&loaded, loaded_event, field, true)?;
    }
    for scenario in 0..2 {
        let record = json_parser::new(lines[22 + scenario]).parse_record()?;
        verify_scenario(&record, scenario)?;
    }
    let passed = json_parser::new(lines[24]).parse_record()?;
    require_control_header(
        &passed,
        passed_event,
        &[
            "routes",
            "application_tasks",
            "loader_tasks",
            "surfaces_per_route",
            "inputs_per_route",
            "hashes_equal",
            "pixels_equal",
            "resources_restored",
            "personality",
        ],
    )?;
    for (field, expected) in [
        ("routes", 2),
        ("application_tasks", 8),
        ("loader_tasks", 2),
        ("surfaces_per_route", 7),
        ("inputs_per_route", 6),
    ] {
        require_number(&passed, passed_event, field, expected)?;
    }
    for field in ["hashes_equal", "pixels_equal", "resources_restored"] {
        require_bool(&passed, passed_event, field, true)?;
    }
    require_string(
        &passed,
        passed_event,
        "personality",
        "linux_syscall_native_int_f4",
    )?;
    Ok(evidence)
}

fn verify_scenario(record: &structured_record, scenario: usize) -> Result<(), String> {
    let ticks = required_number(record, scenario_event, "timer_ticks")?;
    if ticks > 10_000 {
        return Err("Linux application timer bound exceeded".into());
    }
    let normal = scenario == 0;
    // Source-derived fixed cooperative trace: 65 Linux + 67 native calls in
    // normal execution; six Linux + six native calls in the queued-fault route.
    // PIT interrupts resume the same task, so retries cannot vary these counts.
    let numbers = [
        ("scenario", scenario as u64),
        ("calls", if normal { 132 } else { 12 }),
        ("cooperative_switches", if normal { 14 } else { 0 }),
        ("timer_ticks", ticks),
        ("surface_sends", if normal { 7 } else { 1 }),
        ("input_sends", if normal { 6 } else { 0 }),
        ("surface_deliveries", if normal { 7 } else { 0 }),
        ("input_deliveries", if normal { 6 } else { 0 }),
        ("surface_acks", if normal { 7 } else { 0 }),
        ("input_acks", if normal { 6 } else { 0 }),
        ("denials", if normal { 2 } else { 4 }),
        ("bad_copies", if normal { 5 } else { 1 }),
        ("invalid", if normal { 8 } else { 2 }),
        ("closed", if normal { 0 } else { 2 }),
        ("discarded", u64::from(!normal)),
        ("exits", if normal { 2 } else { 1 }),
        ("faults", u64::from(!normal)),
        ("rows", if normal { 24 } else { 0 }),
        ("sys_read", if normal { 12 } else { 1 }),
        ("sys_write", if normal { 11 } else { 3 }),
        ("sys_poll", if normal { 32 } else { 0 }),
        ("sys_yield", if normal { 7 } else { 0 }),
        ("sys_exit", u64::from(normal)),
        ("sys_unknown", 2),
    ];
    let fields = numbers.iter().map(|(field, _)| *field).collect::<Vec<_>>();
    require_control_header(record, scenario_event, &fields)?;
    for (field, expected) in numbers {
        require_number(record, scenario_event, field, expected)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_transcript() -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}",
            native_application_smoke::tests::valid_transcript(),
            r#"{"schema":1,"event":"kernel.linux_application_test.loaded","level":"info","loader_tasks":2,"calls":15,"denials":1,"bad_copies":1,"invalid":8,"accepted":1,"plan_bytes":96,"initialized_rw":true,"zero_fill_verified":true,"loader_reclaimed":true}"#,
            r#"{"schema":1,"event":"kernel.linux_application_test.scenario","level":"info","scenario":0,"calls":132,"cooperative_switches":14,"timer_ticks":3,"surface_sends":7,"input_sends":6,"surface_deliveries":7,"input_deliveries":6,"surface_acks":7,"input_acks":6,"denials":2,"bad_copies":5,"invalid":8,"closed":0,"discarded":0,"exits":2,"faults":0,"rows":24,"sys_read":12,"sys_write":11,"sys_poll":32,"sys_yield":7,"sys_exit":1,"sys_unknown":2}"#,
            r#"{"schema":1,"event":"kernel.linux_application_test.scenario","level":"info","scenario":1,"calls":12,"cooperative_switches":0,"timer_ticks":0,"surface_sends":1,"input_sends":0,"surface_deliveries":0,"input_deliveries":0,"surface_acks":0,"input_acks":0,"denials":4,"bad_copies":1,"invalid":2,"closed":2,"discarded":1,"exits":1,"faults":1,"rows":0,"sys_read":1,"sys_write":3,"sys_poll":0,"sys_yield":0,"sys_exit":0,"sys_unknown":2}"#,
            r#"{"schema":1,"event":"kernel.linux_application_test.passed","level":"info","routes":2,"application_tasks":8,"loader_tasks":2,"surfaces_per_route":7,"inputs_per_route":6,"hashes_equal":true,"pixels_equal":true,"resources_restored":true,"personality":"linux_syscall_native_int_f4"}"#,
        )
    }

    fn render_record(record: &structured_record) -> String {
        let fields = record
            .iter()
            .map(|(key, value)| {
                let value = match value {
                    json_scalar::string(value) => format!("{value:?}"),
                    json_scalar::number(value) => value.clone(),
                    json_scalar::boolean(value) => value.to_string(),
                    json_scalar::null => "null".into(),
                };
                format!("{key:?}:{value}")
            })
            .collect::<Vec<_>>();
        format!("{{{}}}", fields.join(","))
    }

    #[test]
    fn linux_application_requires_native_lifecycle_then_exact_loader_and_comparison_records() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        for vendor in ["AuthenticAMD", "HygonGenuine"] {
            verify_transcript(&valid.replace("GenuineIntel", vendor)).unwrap();
        }
        verify_transcript(&format!("\r\n{}\r\n", valid.replace('\n', "\r\n"))).unwrap();
        assert!(native_application_smoke::verify_transcript(&valid).is_err());
        assert!(verify_transcript(&native_application_smoke::tests::valid_transcript()).is_err());
        let lines = valid.lines().collect::<Vec<_>>();
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
        for extra in ["unexpected text", "{}", lines[22]] {
            assert!(verify_transcript(&format!("{valid}\n{extra}")).is_err());
        }
        assert!(verify_transcript(&valid.replace("\"surfaces\":7", "\"surfaces\":6")).is_err());
    }

    #[test]
    fn linux_application_rejects_every_missing_changed_duplicate_and_extra_loader_field() {
        let lines = valid_transcript()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for index in 21..25 {
            let record = json_parser::new(&lines[index]).parse_record().unwrap();
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
                    let mut records = lines.clone();
                    records[index] = render_record(&changed);
                    assert!(
                        verify_transcript(&records.join("\n")).is_err(),
                        "accepted {index}:{field}"
                    );
                }
                let mut duplicate = render_record(&record);
                duplicate.pop();
                duplicate.push_str(&format!(",{field:?}:null}}"));
                let mut records = lines.clone();
                records[index] = duplicate;
                assert!(verify_transcript(&records.join("\n")).is_err());
            }
            for field in ["unknown", "address", "private_payload", "task_identity"] {
                let mut changed = record.clone();
                changed.insert(field.into(), json_scalar::number("1".into()));
                let mut records = lines.clone();
                records[index] = render_record(&changed);
                assert!(verify_transcript(&records.join("\n")).is_err());
            }
        }
    }

    #[test]
    fn linux_capture_waits_for_complete_comparison_and_uses_separate_evidence_names() {
        let valid = format!("{}\n", valid_transcript());
        for length in 0..valid.len() {
            assert!(!native_application_smoke::capture_ready(
                &valid[..length],
                profile.verify
            ));
        }
        assert!(native_application_smoke::capture_ready(
            &valid,
            profile.verify
        ));
        assert!(!native_application_smoke::capture_ready(
            &format!("{valid}{{}}\n"),
            profile.verify
        ));
        assert_eq!(profile.log_prefix, "linux_application");
        assert_eq!(profile.artifact_prefix, "linux-application");
        assert_eq!(profile.records, 25);
    }

    #[test]
    fn linux_application_precedence_is_explicit_without_widening_native_selection() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        assert!(!native_application_smoke::selected(&[
            feature,
            native_application_smoke::feature
        ]));
        for lower in [
            "elf_application_test",
            "native_application_test",
            "ny_object_test",
            "application_profile_test",
            "owned_dispatcher_test",
            "owned_multi_region_test",
            "owned_source_test",
            "owned_tls_test",
            "dispatcher_test",
            "workspace_dev",
            "owned_workspace_test",
            "owned_workspace_cpu_test",
        ] {
            let combined = [feature, lower];
            assert!(selected(&combined));
            assert!(!elf_application_smoke::selected(&combined));
            assert!(!native_application_smoke::selected(&combined));
            assert!(!ny_object_smoke::selected(&combined));
            assert!(!application_profile_smoke::selected(&combined));
            assert!(!owned_dispatcher_smoke::selected(&combined));
            assert!(!owned_multi_region_smoke::selected(&combined));
            assert!(!owned_source_smoke::selected(&combined));
            assert!(!owned_tls_smoke::selected(&combined));
            assert!(!dispatcher_smoke::selected(&combined));
            assert!(!workspace_dev::selected(&combined));
            assert!(!owned_workspace::selected(&combined));
        }
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
            assert!(!selected(&[feature, higher]));
        }
    }
}
