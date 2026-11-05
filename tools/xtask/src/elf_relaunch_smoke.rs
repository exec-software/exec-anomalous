//! ELF/native editor replacement comparison against the preceding independent native
//! oracle. Each route must match known wire bytes and semantic checkpoints;
//! agreement between the two routes alone cannot establish correctness.
use super::*;
use native_application_smoke::{run_evidence, smoke_profile};
use native_document_smoke::verify_numbers;
use native_relaunch_smoke::{
    fault_kind, maintenance_flags, maintenance_numbers, packet_reference, scenario_numbers,
    surface_reference,
};

pub(super) const feature: &str = "elf_relaunch_test";
const ready_event: &str = "kernel.elf_relaunch_test.ready";
const loaded_event: &str = "kernel.elf_relaunch_test.loaded";
const maintenance_event: &str = "kernel.elf_relaunch_test.maintenance";
const scenario_event: &str = "kernel.elf_relaunch_test.scenario";
const surface_event: &str = "kernel.elf_relaunch_test.surface";
const packet_event: &str = "kernel.elf_relaunch_test.packet";
const passed_event: &str = "kernel.elf_relaunch_test.passed";
const profile: smoke_profile = smoke_profile {
    log_prefix: "elf_relaunch",
    artifact_prefix: "elf-relaunch",
    records: 222,
    verify: verify_records,
    verify_frame: native_relaunch_smoke::verify_frame,
};

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && native_relaunch_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|value| *value != feature)
                .chain(core::iter::once(native_relaunch_smoke::feature))
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!("ELF relaunch image inspected: {}", image.path.display());
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
        return Err("ELF relaunch proof requires exactly 222 ordered records".into());
    }
    let evidence = native_application_smoke::verify_records(&lines[..21].join("\n"))?;
    verify_numbers(
        &json_parser::new(lines[21]).parse_record()?,
        ready_event,
        &ready_numbers(),
        &[],
    )?;
    let loaded = json_parser::new(lines[22]).parse_record()?;
    verify_numbers(
        &loaded,
        loaded_event,
        &loaded_numbers(),
        &["loader_reclaimed", "expanded_rw_equal"],
    )?;
    require_bool(&loaded, loaded_event, "loader_reclaimed", true)?;
    require_bool(&loaded, loaded_event, "expanded_rw_equal", true)?;

    let mut cursor = 23;
    for scenario in 0..3 {
        for route in 0..2 {
            let maintenance = json_parser::new(lines[cursor]).parse_record()?;
            cursor += 1;
            let mut numbers = maintenance_numbers(scenario);
            numbers.push(("route", route));
            verify_numbers(
                &maintenance,
                maintenance_event,
                &numbers,
                &maintenance_flags,
            )?;
            for flag in maintenance_flags {
                require_bool(&maintenance, maintenance_event, flag, true)?;
            }
            let record = json_parser::new(lines[cursor]).parse_record()?;
            cursor += 1;
            let mut numbers = scenario_numbers(scenario)?;
            numbers.push(("route", route));
            verify_numbers(
                &record,
                scenario_event,
                &numbers,
                &["timer_ticks", "fault_kind", "resources_restored"],
            )?;
            if required_number(&record, scenario_event, "timer_ticks")? > 10_000 {
                return Err("ELF relaunch timer bound exceeded".into());
            }
            require_string(&record, scenario_event, "fault_kind", fault_kind(scenario))?;
            require_bool(&record, scenario_event, "resources_restored", true)?;

            // Both routes independently match known native checkpoints. Equal
            // wrong results cannot pass. Only individually bounded timer ticks
            // may differ; ownership, bytes and lifecycle counters stay exact.
            for incarnation in 0..2 {
                for (index, value) in surface_reference(scenario, incarnation)?
                    .0
                    .into_iter()
                    .enumerate()
                {
                    verify_numbers(
                        &json_parser::new(lines[cursor]).parse_record()?,
                        surface_event,
                        &[
                            ("scenario", scenario as u64),
                            ("route", route),
                            ("incarnation", incarnation as u64),
                            ("sequence", index as u64 + 1),
                            ("hash", value),
                        ],
                        &[],
                    )?;
                    cursor += 1;
                }
                let (requests, replies) =
                    packet_reference(scenario, incarnation, 0xd101 + scenario as u64);
                for (channel, hashes) in [(2, requests), (3, replies)] {
                    for (index, value) in hashes.into_iter().enumerate() {
                        verify_numbers(
                            &json_parser::new(lines[cursor]).parse_record()?,
                            packet_event,
                            &[
                                ("scenario", scenario as u64),
                                ("route", route),
                                ("incarnation", incarnation as u64),
                                ("channel", channel),
                                ("sequence", index as u64 + 1),
                                ("hash", value),
                            ],
                            &[],
                        )?;
                        cursor += 1;
                    }
                }
            }
        }
    }
    if cursor != lines.len() - 1 {
        return Err("ELF relaunch record count mismatch".into());
    }
    let record = json_parser::new(lines[cursor]).parse_record()?;
    verify_numbers(
        &record,
        passed_event,
        &passed_numbers(),
        &[
            "hashes_equal",
            "pixels_equal",
            "resources_restored",
            "authority_revoked",
            "source_language",
            "store_scope",
            "personality",
        ],
    )?;
    for field in [
        "hashes_equal",
        "pixels_equal",
        "resources_restored",
        "authority_revoked",
    ] {
        require_bool(&record, passed_event, field, true)?;
    }
    require_string(&record, passed_event, "source_language", "rust")?;
    require_string(&record, passed_event, "store_scope", "service_lifetime")?;
    require_string(&record, passed_event, "personality", "native_int_f4")?;
    Ok(evidence)
}

fn ready_numbers() -> Vec<(&'static str, u64)> {
    let mut numbers = native_relaunch_smoke::ready_numbers();
    numbers.push(("routes", 2));
    numbers
}

fn loaded_numbers() -> Vec<(&'static str, u64)> {
    vec![
        ("loader_tasks", 2),
        ("calls", 15),
        ("denials", 1),
        ("bad_copies", 1),
        ("invalid", 8),
        ("accepted", 1),
        ("plan_bytes", 96),
        ("rw_file_bytes", 128),
        ("rw_memory_bytes", 4224),
        ("elided_zero_bytes", 4096),
    ]
}

fn passed_numbers() -> Vec<(&'static str, u64)> {
    vec![
        ("routes", 2),
        ("application_tasks", 18),
        ("loader_tasks", 2),
        ("service_lifetimes", 6),
        ("replacements", 6),
        ("cancelled_tasks", 6),
        ("contained_faults", 4),
        ("zeroed_bytes", 589824),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str, numbers: &[(&str, u64)], extra: &str) -> String {
        let mut record = format!(r#"{{"schema":1,"event":"{name}","level":"info""#);
        for (key, value) in numbers {
            record.push_str(&format!(",{key:?}:{value}"));
        }
        record.push_str(extra);
        record.push('}');
        record
    }

    fn valid_transcript() -> String {
        let mut lines = vec![
            native_application_smoke::tests::valid_transcript(),
            event(ready_event, &ready_numbers(), ""),
            event(
                loaded_event,
                &loaded_numbers(),
                r#", "loader_reclaimed":true,"expanded_rw_equal":true"#,
            ),
        ];
        for scenario in 0..3 {
            for route in 0..2 {
                let flags = maintenance_flags
                    .map(|flag| format!(",{flag:?}:true"))
                    .join("");
                let mut numbers = maintenance_numbers(scenario);
                numbers.push(("route", route));
                lines.push(event(maintenance_event, &numbers, &flags));
                let mut numbers = scenario_numbers(scenario).unwrap();
                numbers.push(("route", route));
                lines.push(event(
                    scenario_event,
                    &numbers,
                    &format!(
                        r#", "timer_ticks":{},"fault_kind":"{}","resources_restored":true"#,
                        route + 2,
                        fault_kind(scenario)
                    ),
                ));
                for incarnation in 0..2 {
                    for (index, hash) in surface_reference(scenario, incarnation)
                        .unwrap()
                        .0
                        .into_iter()
                        .enumerate()
                    {
                        lines.push(event(
                            surface_event,
                            &[
                                ("scenario", scenario as u64),
                                ("route", route),
                                ("incarnation", incarnation as u64),
                                ("sequence", index as u64 + 1),
                                ("hash", hash),
                            ],
                            "",
                        ));
                    }
                    let (requests, replies) =
                        packet_reference(scenario, incarnation, 0xd101 + scenario as u64);
                    for (channel, hashes) in [(2, requests), (3, replies)] {
                        for (index, hash) in hashes.into_iter().enumerate() {
                            lines.push(event(
                                packet_event,
                                &[
                                    ("scenario", scenario as u64),
                                    ("route", route),
                                    ("incarnation", incarnation as u64),
                                    ("channel", channel),
                                    ("sequence", index as u64 + 1),
                                    ("hash", hash),
                                ],
                                "",
                            ));
                        }
                    }
                }
            }
        }
        lines.push(event(
            passed_event,
            &passed_numbers(),
            r#", "hashes_equal":true,"pixels_equal":true,"resources_restored":true,"authority_revoked":true,"source_language":"rust","store_scope":"service_lifetime","personality":"native_int_f4""#,
        ));
        lines.join("\n")
    }

    fn render(record: &structured_record) -> String {
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
    fn exact_route_order_is_required_without_widening_prior_oracles() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        verify_transcript(&valid.replace("GenuineIntel", "AuthenticAMD")).unwrap();
        verify_transcript(&format!("\r\n{}\r\n", valid.replace('\n', "\r\n"))).unwrap();
        assert!(native_document_smoke::verify_transcript(&valid).is_err());
        assert!(native_relaunch_smoke::verify_transcript(&valid).is_err());
        assert!(elf_document_smoke::verify_transcript(&valid).is_err());
        assert!(native_application_smoke::verify_transcript(&valid).is_err());
        assert!(verify_transcript(&native_application_smoke::tests::valid_transcript()).is_err());
        let lines = valid.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 222);
        assert!(valid.len() < 64 * 1024);
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
        for extra in ["unexpected text", "{}", lines[221]] {
            assert!(verify_transcript(&format!("{valid}\n{extra}")).is_err());
        }
    }

    #[test]
    fn every_comparison_field_rejects_omission_mutation_duplicates_and_extras() {
        let lines = valid_transcript()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for index in 21..lines.len() {
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
                    let mut output = lines.clone();
                    output[index] = render(&changed);
                    assert!(
                        verify_transcript(&output.join("\n")).is_err(),
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
                    let mut output = lines.clone();
                    output[index] = render(&changed);
                    assert!(
                        verify_transcript(&output.join("\n")).is_err(),
                        "accepted increment {index}:{field}"
                    );
                }
                let mut duplicate = render(&record);
                duplicate.pop();
                duplicate.push_str(&format!(",{field:?}:null}}"));
                let mut output = lines.clone();
                output[index] = duplicate;
                assert!(verify_transcript(&output.join("\n")).is_err());
            }
            for field in ["unknown", "private_payload", "task_identity", "address"] {
                let mut changed = record.clone();
                changed.insert(field.into(), json_scalar::number("1".into()));
                let mut output = lines.clone();
                output[index] = render(&changed);
                assert!(verify_transcript(&output.join("\n")).is_err());
            }
        }
    }

    #[test]
    fn matching_wrong_routes_fail_and_only_bounded_timers_may_differ() {
        let valid = valid_transcript();
        // Mutate both route results identically: equality alone must not pass.
        assert!(verify_transcript(&valid.replace("\"calls\":292", "\"calls\":291")).is_err());
        let first_surface = surface_reference(0, 0).unwrap().0[0];
        assert!(
            verify_transcript(&valid.replace(
                &format!("\"hash\":{first_surface}"),
                &format!("\"hash\":{}", first_surface + 1)
            ))
            .is_err()
        );
        let first_reply = packet_reference(0, 0, 0xd101).1[0];
        assert!(
            verify_transcript(&valid.replace(
                &format!("\"hash\":{first_reply}"),
                &format!("\"hash\":{}", first_reply + 1)
            ))
            .is_err()
        );
        assert!(verify_transcript(&valid.replace("\"route\":1", "\"route\":0")).is_err());
        let mut lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for line in &mut lines[23..221] {
            let mut record = json_parser::new(line).parse_record().unwrap();
            if record.contains_key("timer_ticks") {
                let ticks = if required_number(&record, scenario_event, "route").unwrap() == 0 {
                    0
                } else {
                    10_000
                };
                record.insert("timer_ticks".into(), json_scalar::number(ticks.to_string()));
                *line = render(&record);
            }
        }
        verify_transcript(&lines.join("\n")).unwrap();
        assert!(
            verify_transcript(
                &lines
                    .join("\n")
                    .replace("\"timer_ticks\":10000", "\"timer_ticks\":10001")
            )
            .is_err()
        );
    }

    #[test]
    fn both_routes_must_recover_the_exact_commit_boundary_and_fresh_lifetime() {
        let valid = valid_transcript();
        let before = surface_reference(1, 1).unwrap().0[0];
        let after = surface_reference(2, 1).unwrap().0[0];
        assert_ne!(before, after);
        assert!(
            verify_transcript(
                &valid.replace(&format!("\"hash\":{after}"), &format!("\"hash\":{before}"))
            )
            .is_err()
        );
        let old_reply = packet_reference(2, 0, 0xd103).1[0];
        let reopened_reply = packet_reference(2, 1, 0xd103).1[0];
        assert_ne!(old_reply, reopened_reply);
        assert!(
            verify_transcript(&valid.replace(
                &format!("\"hash\":{reopened_reply}"),
                &format!("\"hash\":{old_reply}")
            ))
            .is_err()
        );
        assert!(
            verify_transcript(&valid.replace("\"incarnation\":1", "\"incarnation\":0")).is_err()
        );
        assert!(verify_transcript(&valid.replace("\"discarded\":1", "\"discarded\":0")).is_err());
        assert!(
            verify_transcript(
                &valid.replace("\"service_preserved\":true", "\"service_preserved\":false")
            )
            .is_err()
        );
    }

    #[test]
    fn capture_waits_for_both_routes_and_uses_the_reopened_document_frame() {
        let (_, pixels) = surface_reference(2, 1).unwrap();
        let header = b"P6\n1024 768\n255\n";
        let mut ppm = header.to_vec();
        ppm.resize(header.len() + 1024 * 768 * 3, 0);
        for y in 0..24 {
            for x in 0..384 {
                let source = ((y / 3) * 128 + x / 3) * 3;
                let destination = header.len() + ((32 + y) * 1024 + 32 + x) * 3;
                ppm[destination..destination + 3].copy_from_slice(&pixels[source..source + 3]);
            }
        }
        (profile.verify_frame)(&ppm).unwrap();
        assert!(native_application_smoke::verify_frame(&ppm).is_err());
        ppm[header.len() + (32 * 1024 + 32) * 3] ^= 1;
        assert!((profile.verify_frame)(&ppm).is_err());
        assert!((profile.verify_frame)(&ppm[..ppm.len() - 1]).is_err());
        let mut prefix = String::new();
        for line in valid_transcript().lines() {
            assert!(!native_application_smoke::capture_ready(
                &prefix,
                profile.verify
            ));
            prefix.push_str(line);
            prefix.push('\n');
        }
        assert!(native_application_smoke::capture_ready(
            &prefix,
            profile.verify
        ));
        assert!(!native_application_smoke::capture_ready(
            prefix.trim_end(),
            profile.verify
        ));
        assert!(!native_application_smoke::capture_ready(
            &format!("{prefix}{{}}\n"),
            profile.verify
        ));
        assert_eq!(profile.log_prefix, "elf_relaunch");
        assert_eq!(profile.artifact_prefix, "elf-relaunch");
    }

    #[test]
    fn explicit_relaunch_comparison_precedence_preserves_higher_profiles() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        for lower in [
            native_relaunch_smoke::feature,
            elf_document_smoke::feature,
            elf_application_smoke::feature,
            native_document_smoke::feature,
            native_runtime_smoke::feature,
            native_application_smoke::feature,
            ny_object_smoke::feature,
            application_profile_smoke::feature,
            owned_dispatcher_smoke::feature,
            owned_multi_region_smoke::feature,
            owned_source_smoke::feature,
            owned_tls_smoke::feature,
            dispatcher_smoke::feature,
            workspace_dev::feature,
            owned_workspace::feature,
        ] {
            assert!(selected(&[feature, lower]));
        }
        for higher in [
            "linux_application_test",
            "veil_application_test",
            "shared_veil_test",
            "linux_tls_test",
            "linux_loader_entry_test",
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
        assert!(!selected(all_kernel_features));
    }
}
