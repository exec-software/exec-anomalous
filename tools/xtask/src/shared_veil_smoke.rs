//! Strict native baseline followed by a shared two-caller veil lifecycle proof.
use super::*;
use native_application_smoke::{run_evidence, smoke_profile};

pub(super) const feature: &str = "shared_veil_test";
const ready_event: &str = "kernel.shared_veil_test.ready";
const scenario_event: &str = "kernel.shared_veil_test.scenario";
const pair_event: &str = "kernel.shared_veil_test.pair";
const passed_event: &str = "kernel.shared_veil_test.passed";
const profile: smoke_profile = smoke_profile {
    log_prefix: "shared_veil",
    artifact_prefix: "shared-veil",
    records: 32,
    verify: verify_records,
    verify_frame: native_application_smoke::verify_frame,
};

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.contains(&"linux_tls_test")
        && !features.contains(&"linux_loader_entry_test")
        && veil_application_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|value| *value != feature)
                .chain(core::iter::once(veil_application_smoke::feature))
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!(
        "Shared userspace veil image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let directory = native_application_smoke::prepare_smoke(profile)?;
    build_and_inspect_normal_image()?;
    let image = build()?;
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
            "Shared veil proof requires exactly 21 native records and 11 shared-service records"
                .into(),
        );
    }
    let evidence = native_application_smoke::verify_records(&lines[..21].join("\n"))?;
    let ready = json_parser::new(lines[21]).parse_record()?;
    require_control_header(
        &ready,
        ready_event,
        &[
            "tasks",
            "callers",
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
        ("tasks", 5),
        ("callers", 2),
        ("scenarios", 3),
        ("data_frames", 80),
        ("table_frames", 45),
        ("leases", 25),
        ("request_bytes", 80),
    ] {
        require_number(&ready, ready_event, field, value)?;
    }
    require_string(&ready, ready_event, "policy", "userspace")?;
    require_string(&ready, ready_event, "startup", "fixed_fixture")?;
    for scenario in 0..3 {
        verify_scenario(
            &json_parser::new(lines[22 + scenario * 3]).parse_record()?,
            scenario,
        )?;
        for caller in 0..2 {
            verify_pair(
                &json_parser::new(lines[23 + scenario * 3 + caller]).parse_record()?,
                scenario,
                caller,
            )?;
        }
    }
    let passed = json_parser::new(lines[31]).parse_record()?;
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
    require_number(&passed, passed_event, "application_tasks", 15)?;
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
    require_control_header(
        record,
        scenario_event,
        &[
            "scenario",
            "raw_calls",
            "completions",
            "stages",
            "rejected",
            "aborted",
            "timer_ticks",
            "cooperative_switches",
            "caller_changes",
            "isolation_checks",
            "stale_rejections",
            "exits",
            "faults",
        ],
    )?;
    require_number(record, scenario_event, "scenario", scenario as u64)?;
    require_number(record, scenario_event, "aborted", [0, 0, 1][scenario])?;
    require_number(record, scenario_event, "exits", [5, 4, 2][scenario])?;
    require_number(record, scenario_event, "faults", [0, 1, 3][scenario])?;
    // Two unchanged producers: 65 calls / 40 effects each. One shared veil
    // performs its 3 startup and 6 effect-retry probes only once; each healthy
    // caller adds 4 failed copies. FIFO turns yield 15 caller changes, each
    // checked by two controller-injected stale proposals through veil_call.
    // Peer checks count actual veil mechanism gates: 285+274, 25+275, or26.
    for (field, value) in [
        ("raw_calls", [130, 71, 6][scenario]),
        ("completions", [130, 71, 5][scenario]),
        ("stages", [80, 41, 1][scenario]),
        ("rejected", [17, 14, 12][scenario]),
        ("cooperative_switches", [28, 14, 0][scenario]),
        ("caller_changes", [15, 1, 0][scenario]),
        ("isolation_checks", [559, 300, 26][scenario]),
        ("stale_rejections", [30, 2, 0][scenario]),
    ] {
        require_number(record, scenario_event, field, value)?;
    }
    if required_number(record, scenario_event, "timer_ticks")? > 10_000 {
        return Err("Shared veil timer bound exceeded".into());
    }
    Ok(())
}

fn verify_pair(record: &structured_record, scenario: usize, caller: usize) -> Result<(), String> {
    let healthy = scenario == 0 || (scenario == 1 && caller == 1);
    let faulted_producer = scenario == 1 && caller == 0;
    let numbers = [
        ("scenario", scenario as u64),
        ("caller", caller as u64),
        (
            "surfaces",
            if healthy {
                7
            } else {
                u64::from(faulted_producer)
            },
        ),
        ("inputs", if healthy { 6 } else { 0 }),
        ("surface_deliveries", if healthy { 7 } else { 0 }),
        ("input_deliveries", if healthy { 6 } else { 0 }),
        ("surface_acks", if healthy { 7 } else { 0 }),
        ("input_acks", if healthy { 6 } else { 0 }),
        ("discarded", u64::from(faulted_producer)),
        ("exits", if healthy { 2 } else { 1 }),
        ("faults", u64::from(!healthy)),
        ("rows", if healthy { 24 } else { 0 }),
    ];
    let mut fields = numbers.iter().map(|(field, _)| *field).collect::<Vec<_>>();
    fields.extend(["hashes_equal", "pixels_equal"]);
    require_control_header(record, pair_event, &fields)?;
    for (field, value) in numbers {
        require_number(record, pair_event, field, value)?;
    }
    require_bool(record, pair_event, "hashes_equal", healthy)?;
    require_bool(record, pair_event, "pixels_equal", healthy)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_transcript() -> String {
        let mut lines = vec![native_application_smoke::tests::valid_transcript(),
            r#"{"schema":1,"event":"kernel.shared_veil_test.ready","level":"info","tasks":5,"callers":2,"scenarios":3,"data_frames":80,"table_frames":45,"leases":25,"request_bytes":80,"policy":"userspace","startup":"fixed_fixture"}"#.into()];
        for scenario in 0..3 {
            lines.push(format!(
                r#"{{"schema":1,"event":"kernel.shared_veil_test.scenario","level":"info","scenario":{},"raw_calls":{},"completions":{},"stages":{},"rejected":{},"aborted":{},"timer_ticks":3,"cooperative_switches":{},"caller_changes":{},"isolation_checks":{},"stale_rejections":{},"exits":{},"faults":{}}}"#,
                scenario,[130,71,6][scenario],[130,71,5][scenario],[80,41,1][scenario],[17,14,12][scenario],[0,0,1][scenario],[28,14,0][scenario],[15,1,0][scenario],[559,300,26][scenario],[30,2,0][scenario],[5,4,2][scenario],[0,1,3][scenario]
            ));
            for caller in 0..2 {
                let healthy = scenario == 0 || (scenario == 1 && caller == 1);
                let first_fault = scenario == 1 && caller == 0;
                lines.push(format!(
                    r#"{{"schema":1,"event":"kernel.shared_veil_test.pair","level":"info","scenario":{},"caller":{},"surfaces":{},"inputs":{},"surface_deliveries":{},"input_deliveries":{},"surface_acks":{},"input_acks":{},"discarded":{},"exits":{},"faults":{},"rows":{},"hashes_equal":{},"pixels_equal":{}}}"#,
                    scenario,caller,if healthy{7}else{usize::from(first_fault)},if healthy{6}else{0},if healthy{7}else{0},if healthy{6}else{0},if healthy{7}else{0},if healthy{6}else{0},usize::from(first_fault),if healthy{2}else{1},usize::from(!healthy),if healthy{24}else{0},healthy,healthy
                ));
            }
        }
        lines.push(r#"{"schema":1,"event":"kernel.shared_veil_test.passed","level":"info","application_tasks":15,"hashes_equal":true,"pixels_equal":true,"abort_unpublished":true,"resources_restored":true,"authority_revoked":true}"#.into());
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
    fn shared_proof_requires_exact_order_complete_pairs_and_native_baseline() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        verify_transcript(&valid.replace("GenuineIntel", "AuthenticAMD")).unwrap();
        assert!(veil_application_smoke::verify_transcript(&valid).is_err());
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
        assert!(verify_transcript(&format!("{valid}\n{{}}")).is_err());
    }

    #[test]
    fn shared_proof_rejects_missing_changed_duplicate_and_private_fields() {
        let lines = valid_transcript()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for index in 21..32 {
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
    fn shared_capture_requires_all_three_scenarios_and_both_callers() {
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
        assert_eq!(profile.records, 32);
        assert_eq!(profile.log_prefix, "shared_veil");
        assert_eq!(profile.artifact_prefix, "shared-veil");
    }

    #[test]
    fn shared_selection_does_not_widen_any_preceding_proof() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        for lower in [
            "veil_application_test",
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
            let features = [feature, lower];
            assert!(selected(&features));
            assert!(!veil_application_smoke::selected(&features));
            assert!(!linux_application_smoke::selected(&features));
            assert!(!elf_application_smoke::selected(&features));
            assert!(!native_application_smoke::selected(&features));
            assert!(!ny_object_smoke::selected(&features));
            assert!(!application_profile_smoke::selected(&features));
            assert!(!owned_dispatcher_smoke::selected(&features));
            assert!(!owned_multi_region_smoke::selected(&features));
            assert!(!owned_source_smoke::selected(&features));
            assert!(!owned_tls_smoke::selected(&features));
            assert!(!dispatcher_smoke::selected(&features));
            assert!(!workspace_dev::selected(&features));
            assert!(!owned_workspace::selected(&features));
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
