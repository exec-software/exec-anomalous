//! Strict native baseline plus supervised userspace Linux-policy proof.
use super::*;
use native_application_smoke::{run_evidence, smoke_profile};

pub(super) const feature: &str = "veil_application_test";
const ready_event: &str = "kernel.veil_application_test.ready";
const scenario_event: &str = "kernel.veil_application_test.scenario";
const passed_event: &str = "kernel.veil_application_test.passed";
const profile: smoke_profile = smoke_profile {
    log_prefix: "veil_application",
    artifact_prefix: "veil-application",
    records: 26,
    verify: verify_records,
    verify_frame: native_application_smoke::verify_frame,
};

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.contains(&"linux_tls_test")
        && !features.contains(&"linux_loader_entry_test")
        && !features.contains(&"shared_veil_test")
        && linux_application_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|value| *value != feature)
                .chain(core::iter::once(linux_application_smoke::feature))
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!("Userspace veil image inspected: {}", image.path.display());
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
            "Veil proof requires exactly21 native records followed by five veil records".into(),
        );
    }
    // Preserve the complete native baseline and require exact trailing positions;
    // arbitrary feature-prefixed records must never be filtered out or ignored.
    let evidence = native_application_smoke::verify_records(&lines[..21].join("\n"))?;
    let ready = json_parser::new(lines[21]).parse_record()?;
    require_control_header(
        &ready,
        ready_event,
        &[
            "tasks",
            "scenarios",
            "data_frames",
            "table_frames",
            "leases",
            "request_bytes",
            "policy",
            "startup",
        ],
    )?;
    for (field, value) in [
        ("tasks", 3),
        ("scenarios", 3),
        ("data_frames", 48),
        ("table_frames", 27),
        ("leases", 15),
        ("request_bytes", 80),
    ] {
        require_number(&ready, ready_event, field, value)?;
    }
    require_string(&ready, ready_event, "policy", "userspace")?;
    require_string(&ready, ready_event, "startup", "fixed_fixture")?;
    for scenario in 0..3 {
        let record = json_parser::new(lines[22 + scenario]).parse_record()?;
        verify_scenario(&record, scenario)?;
    }
    let passed = json_parser::new(lines[25]).parse_record()?;
    require_control_header(
        &passed,
        passed_event,
        &[
            "application_tasks",
            "hashes_equal",
            "pixels_equal",
            "abort_unpublished",
            "resources_restored",
            "authority_revoked",
        ],
    )?;
    require_number(&passed, passed_event, "application_tasks", 9)?;
    for field in [
        "hashes_equal",
        "pixels_equal",
        "abort_unpublished",
        "resources_restored",
        "authority_revoked",
    ] {
        require_bool(&passed, passed_event, field, true)?;
    }
    Ok(evidence)
}

fn verify_scenario(record: &structured_record, scenario: usize) -> Result<(), String> {
    let ticks = required_number(record, scenario_event, "timer_ticks")?;
    if ticks > 10_000 {
        return Err("Veil proof timer bound exceeded".into());
    }
    // Derived from the fixed client and policy source:27 successful poll
    // copyouts +7 submits +6 consumes. The normal trace rejects four caller
    // copies, three request/query probes and six stale/invalid completions.
    // The two fault routes stage one submit; only the producer-fault route
    // commits it. Timers resume the same task and do not add retries.
    let numbers = [
        ("scenario", scenario as u64),
        ("raw_calls", [65, 6, 6][scenario]),
        ("completions", [65, 6, 5][scenario]),
        ("stages", [40, 1, 1][scenario]),
        ("rejected", [13, 4, 12][scenario]),
        ("aborted", [0, 0, 1][scenario]),
        ("cooperative_switches", [14, 0, 0][scenario]),
        ("timer_ticks", ticks),
        ("surface_sends", [7, 1, 0][scenario]),
        ("input_sends", [6, 0, 0][scenario]),
        ("surface_deliveries", [7, 0, 0][scenario]),
        ("input_deliveries", [6, 0, 0][scenario]),
        ("surface_acks", [7, 0, 0][scenario]),
        ("input_acks", [6, 0, 0][scenario]),
        ("discarded", [0, 1, 0][scenario]),
        ("exits", [3, 2, 1][scenario]),
        ("faults", [0, 1, 2][scenario]),
        ("rows", [24, 0, 0][scenario]),
    ];
    let fields = numbers.iter().map(|(field, _)| *field).collect::<Vec<_>>();
    require_control_header(record, scenario_event, &fields)?;
    for (field, value) in numbers {
        require_number(record, scenario_event, field, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_transcript() -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            native_application_smoke::tests::valid_transcript(),
            r#"{"schema":1,"event":"kernel.veil_application_test.ready","level":"info","tasks":3,"scenarios":3,"data_frames":48,"table_frames":27,"leases":15,"request_bytes":80,"policy":"userspace","startup":"fixed_fixture"}"#,
            r#"{"schema":1,"event":"kernel.veil_application_test.scenario","level":"info","scenario":0,"raw_calls":65,"completions":65,"stages":40,"rejected":13,"aborted":0,"cooperative_switches":14,"timer_ticks":3,"surface_sends":7,"input_sends":6,"surface_deliveries":7,"input_deliveries":6,"surface_acks":7,"input_acks":6,"discarded":0,"exits":3,"faults":0,"rows":24}"#,
            r#"{"schema":1,"event":"kernel.veil_application_test.scenario","level":"info","scenario":1,"raw_calls":6,"completions":6,"stages":1,"rejected":4,"aborted":0,"cooperative_switches":0,"timer_ticks":0,"surface_sends":1,"input_sends":0,"surface_deliveries":0,"input_deliveries":0,"surface_acks":0,"input_acks":0,"discarded":1,"exits":2,"faults":1,"rows":0}"#,
            r#"{"schema":1,"event":"kernel.veil_application_test.scenario","level":"info","scenario":2,"raw_calls":6,"completions":5,"stages":1,"rejected":12,"aborted":1,"cooperative_switches":0,"timer_ticks":0,"surface_sends":0,"input_sends":0,"surface_deliveries":0,"input_deliveries":0,"surface_acks":0,"input_acks":0,"discarded":0,"exits":1,"faults":2,"rows":0}"#,
            r#"{"schema":1,"event":"kernel.veil_application_test.passed","level":"info","application_tasks":9,"hashes_equal":true,"pixels_equal":true,"abort_unpublished":true,"resources_restored":true,"authority_revoked":true}"#,
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
    fn veil_requires_native_lifecycle_then_exact_policy_and_containment_records() {
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
    fn veil_rejects_every_missing_changed_duplicate_and_extra_policy_field() {
        let lines = valid_transcript()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for index in 21..26 {
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
    fn veil_capture_waits_for_complete_comparison_and_uses_separate_evidence_names() {
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
        assert_eq!(profile.log_prefix, "veil_application");
        assert_eq!(profile.artifact_prefix, "veil-application");
        assert_eq!(profile.records, 26);
    }

    #[test]
    fn veil_application_precedence_is_explicit_without_widening_native_selection() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        assert!(!native_application_smoke::selected(&[
            feature,
            native_application_smoke::feature
        ]));
        for lower in [
            "linux_application_test",
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
            assert!(!linux_application_smoke::selected(&combined));
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
