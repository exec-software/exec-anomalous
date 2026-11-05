//! Strict native baseline plus supervised userspace Linux-policy proof.
use super::*;
use native_application_smoke::{run_evidence, smoke_profile};

pub(super) const feature: &str = "linux_tls_test";
const ready_event: &str = "kernel.linux_tls_test.ready";
const scenario_event: &str = "kernel.linux_tls_test.scenario";
const passed_event: &str = "kernel.linux_tls_test.passed";
const profile: smoke_profile = smoke_profile {
    log_prefix: "linux_tls",
    artifact_prefix: "linux-tls",
    records: 26,
    verify: verify_records,
    verify_frame: native_application_smoke::verify_frame,
};

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.contains(&linux_loader_entry_smoke::feature)
        && veil_application_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|value| *value != feature && *value != shared_veil_smoke::feature)
                .chain(core::iter::once(veil_application_smoke::feature))
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!("Linux TLS image inspected: {}", image.path.display());
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
            "Linux TLS proof requires exactly21 native records followed by five veil records"
                .into(),
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
            "tls_slots",
            "fs_only",
            "eager_cpu",
        ],
    )?;
    for (field, value) in [
        ("tasks", 3),
        ("scenarios", 3),
        ("data_frames", 48),
        ("table_frames", 27),
        ("leases", 15),
        ("request_bytes", 80),
        ("tls_slots", 3),
    ] {
        require_number(&ready, ready_event, field, value)?;
    }
    require_bool(&ready, ready_event, "fs_only", true)?;
    require_bool(&ready, ready_event, "eager_cpu", true)?;
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
            "tls_cleared",
            "cpu_restored",
            "full_width_metadata",
            "mapped_fs_bytes",
        ],
    )?;
    require_number(&passed, passed_event, "application_tasks", 9)?;
    for field in [
        "hashes_equal",
        "pixels_equal",
        "abort_unpublished",
        "resources_restored",
        "authority_revoked",
        "tls_cleared",
        "cpu_restored",
        "full_width_metadata",
        "mapped_fs_bytes",
    ] {
        require_bool(&passed, passed_event, field, true)?;
    }
    Ok(evidence)
}

fn verify_scenario(record: &structured_record, scenario: usize) -> Result<(), String> {
    let ticks = required_number(record, scenario_event, "timer_ticks")?;
    if ticks > 10_000 {
        return Err("Linux TLS proof timer bound exceeded".into());
    }
    // Native calls include service and veil gates. Bind every capture/restore
    // to observed gates, Linux calls, IRQs and the one intentional fault entry.
    let native_calls = required_number(record, scenario_event, "native_calls")?;
    let peer_zero = required_number(record, scenario_event, "peer_zero_entries")?;
    let raw_calls = [83, 24, 4][scenario];
    let completions = [83, 24, 3][scenario];
    let stages = [51, 12, 2][scenario];
    let rejected = [25, 16, 16][scenario];
    let queries = [11, 11, 3][scenario];
    let minimum_native = 2 + raw_calls + completions + stages + rejected + queries;
    if !(minimum_native..=10_000).contains(&native_calls)
        || !(2..=native_calls + ticks + u64::from(scenario == 2)).contains(&peer_zero)
        || (scenario != 2 && ticks == 0)
    {
        return Err("Linux TLS gate or timer observations outside qualified bounds".into());
    }
    let tls_entries = native_calls + raw_calls + ticks + u64::from(scenario != 0);
    let numbers = [
        ("scenario", scenario as u64),
        ("raw_calls", [83, 24, 4][scenario]),
        ("completions", [83, 24, 3][scenario]),
        ("stages", [51, 12, 2][scenario]),
        ("rejected", [25, 16, 16][scenario]),
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
        ("native_calls", native_calls),
        ("tls_queries", queries),
        ("tls_stages", [5, 5, 1][scenario]),
        ("tls_commits", [5, 5, 0][scenario]),
        ("tls_entries", tls_entries),
        ("tls_returns", tls_entries),
        ("tls_timer_samples", [1, 1, 0][scenario]),
        ("caller_fp_entries", raw_calls + u64::from(scenario == 1)),
        ("peer_zero_entries", peer_zero),
        ("tls_retired", 3),
        ("full_width_commits", [1, 1, 0][scenario]),
        ("retired_caller_fs", [0x800020, 0x800020, 0][scenario]),
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
        [native_application_smoke::tests::valid_transcript(),
            r#"{"schema":1,"level":"info","event":"kernel.linux_tls_test.ready","tasks":3,"scenarios":3,"data_frames":48,"table_frames":27,"leases":15,"request_bytes":80,"policy":"userspace","startup":"fixed_fixture","tls_slots":3,"fs_only":true,"eager_cpu":true}"#.into(),
            r#"{"schema":1,"level":"info","event":"kernel.linux_tls_test.scenario","scenario":0,"raw_calls":83,"completions":83,"stages":51,"rejected":25,"aborted":0,"cooperative_switches":14,"timer_ticks":3,"surface_sends":7,"input_sends":6,"surface_deliveries":7,"input_deliveries":6,"surface_acks":7,"input_acks":6,"discarded":0,"exits":3,"faults":0,"rows":24,"native_calls":400,"tls_queries":11,"tls_stages":5,"tls_commits":5,"tls_entries":486,"tls_returns":486,"tls_timer_samples":1,"caller_fp_entries":83,"peer_zero_entries":2,"tls_retired":3,"full_width_commits":1,"retired_caller_fs":8388640}"#.into(),
            r#"{"schema":1,"level":"info","event":"kernel.linux_tls_test.scenario","scenario":1,"raw_calls":24,"completions":24,"stages":12,"rejected":16,"aborted":0,"cooperative_switches":0,"timer_ticks":1,"surface_sends":1,"input_sends":0,"surface_deliveries":0,"input_deliveries":0,"surface_acks":0,"input_acks":0,"discarded":1,"exits":2,"faults":1,"rows":0,"native_calls":120,"tls_queries":11,"tls_stages":5,"tls_commits":5,"tls_entries":146,"tls_returns":146,"tls_timer_samples":1,"caller_fp_entries":25,"peer_zero_entries":2,"tls_retired":3,"full_width_commits":1,"retired_caller_fs":8388640}"#.into(),
            r#"{"schema":1,"level":"info","event":"kernel.linux_tls_test.scenario","scenario":2,"raw_calls":4,"completions":3,"stages":2,"rejected":16,"aborted":1,"cooperative_switches":0,"timer_ticks":0,"surface_sends":0,"input_sends":0,"surface_deliveries":0,"input_deliveries":0,"surface_acks":0,"input_acks":0,"discarded":0,"exits":1,"faults":2,"rows":0,"native_calls":40,"tls_queries":3,"tls_stages":1,"tls_commits":0,"tls_entries":45,"tls_returns":45,"tls_timer_samples":0,"caller_fp_entries":4,"peer_zero_entries":2,"tls_retired":3,"full_width_commits":0,"retired_caller_fs":0}"#.into(),
            r#"{"schema":1,"level":"info","event":"kernel.linux_tls_test.passed","application_tasks":9,"hashes_equal":true,"pixels_equal":true,"abort_unpublished":true,"resources_restored":true,"authority_revoked":true,"tls_cleared":true,"cpu_restored":true,"full_width_metadata":true,"mapped_fs_bytes":true}"#.into(),
        ].join("\n")
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
        assert_eq!(profile.log_prefix, "linux_tls");
        assert_eq!(profile.artifact_prefix, "linux-tls");
        assert_eq!(profile.records, 26);
    }

    #[test]
    fn tls_capture_counts_commit_abort_and_irq_witnesses_are_correlated() {
        let lines = valid_transcript()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for (index, field, value) in [
            (22, "tls_entries", 487),
            (22, "tls_returns", 487),
            (22, "native_calls", 401),
            (22, "timer_ticks", 4),
            (22, "tls_timer_samples", 0),
            (22, "peer_zero_entries", 1),
            (23, "caller_fp_entries", 24),
            (24, "tls_commits", 1),
            (24, "retired_caller_fs", 0x800020),
        ] {
            let mut changed = lines.clone();
            let mut record = json_parser::new(&changed[index]).parse_record().unwrap();
            record.insert(field.into(), json_scalar::number(value.to_string()));
            changed[index] = render_record(&record);
            assert!(
                verify_transcript(&changed.join("\n")).is_err(),
                "accepted {index}:{field}"
            );
        }
    }

    #[test]
    fn linux_tls_precedence_is_explicit_without_widening_native_selection() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        assert!(!native_application_smoke::selected(&[
            feature,
            native_application_smoke::feature
        ]));
        for lower in [
            "veil_application_test",
            "shared_veil_test",
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
            assert!(!veil_application_smoke::selected(&combined));
            assert!(!shared_veil_smoke::selected(&combined));
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
            linux_loader_entry_smoke::feature,
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
