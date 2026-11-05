//! ELF/native document comparison against the preceding independent native
//! oracle. Each route must match known wire bytes and semantic checkpoints;
//! agreement between the two routes alone cannot establish correctness.
use super::*;
use native_application_smoke::{run_evidence, smoke_profile};
use native_document_smoke::{
    fault_kind, packet_reference, scenario_numbers, surface_reference, verify_numbers,
};

pub(super) const feature: &str = "elf_document_test";
const ready_event: &str = "kernel.elf_document_test.ready";
const loaded_event: &str = "kernel.elf_document_test.loaded";
const scenario_event: &str = "kernel.elf_document_test.scenario";
const surface_event: &str = "kernel.elf_document_test.surface";
const packet_event: &str = "kernel.elf_document_test.packet";
const passed_event: &str = "kernel.elf_document_test.passed";
const profile: smoke_profile = smoke_profile {
    log_prefix: "elf_document",
    artifact_prefix: "elf-document",
    records: 224,
    verify: verify_records,
    verify_frame: native_document_smoke::verify_frame,
};

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.contains(&"native_relaunch_test")
        && !features.contains(&"elf_relaunch_test")
        && !features.iter().any(|value| {
            matches!(
                *value,
                "linux_application_test"
                    | "veil_application_test"
                    | "shared_veil_test"
                    | "linux_tls_test"
                    | "linux_loader_entry_test"
            )
        })
        && native_application_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|value| {
                    !matches!(
                        *value,
                        "elf_document_test"
                            | "elf_application_test"
                            | "native_durable_test"
                            | "native_durable_failure_test"
                            | "native_repeated_editor_test"
                            | "native_document_test"
                            | "native_runtime_test"
                    )
                })
                .chain(core::iter::once(native_application_smoke::feature))
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!("ELF document image inspected: {}", image.path.display());
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
        return Err("ELF document proof requires exactly 224 ordered records".into());
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
    for scenario in 0..4 {
        for route in 0..2 {
            let record = json_parser::new(lines[cursor]).parse_record()?;
            cursor += 1;
            let mut numbers = scenario_numbers(scenario);
            numbers.push(("route", route));
            verify_numbers(
                &record,
                scenario_event,
                &numbers,
                &["timer_ticks", "fault_kind", "resources_restored"],
            )?;
            if required_number(&record, scenario_event, "timer_ticks")? > 10_000 {
                return Err("ELF document timer bound exceeded".into());
            }
            require_string(&record, scenario_event, "fault_kind", fault_kind(scenario))?;
            require_bool(&record, scenario_event, "resources_restored", true)?;

            // Both routes independently match these same explicit references.
            // Thus every semantic field/hash is equal without normalizing away
            // handles, versions, bytes, counters or fault results. Timer ticks
            // alone may differ, and each remains individually bounded above.
            for (index, value) in surface_reference(scenario)?.0.into_iter().enumerate() {
                verify_numbers(
                    &json_parser::new(lines[cursor]).parse_record()?,
                    surface_event,
                    &[
                        ("scenario", scenario as u64),
                        ("route", route),
                        ("sequence", index as u64 + 1),
                        ("hash", value),
                    ],
                    &[],
                )?;
                cursor += 1;
            }
            let (requests, replies) = packet_reference(scenario);
            for (channel, hashes) in [(2, requests), (3, replies)] {
                for (index, value) in hashes.into_iter().enumerate() {
                    verify_numbers(
                        &json_parser::new(lines[cursor]).parse_record()?,
                        packet_event,
                        &[
                            ("scenario", scenario as u64),
                            ("route", route),
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
    if cursor != lines.len() - 1 {
        return Err("ELF document record count mismatch".into());
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
    let mut numbers = native_document_smoke::ready_numbers();
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
        ("application_tasks", 16),
        ("loader_tasks", 2),
        ("service_lifetimes", 8),
        ("normal_runs", 4),
        ("contained_faults", 4),
        ("zeroed_bytes", 524288),
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
        for scenario in 0..4 {
            for route in 0..2 {
                let mut numbers = scenario_numbers(scenario);
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
                for (index, hash) in surface_reference(scenario)
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
                            ("sequence", index as u64 + 1),
                            ("hash", hash),
                        ],
                        "",
                    ));
                }
                let (requests, replies) = packet_reference(scenario);
                for (channel, hashes) in [(2, requests), (3, replies)] {
                    for (index, hash) in hashes.into_iter().enumerate() {
                        lines.push(event(
                            packet_event,
                            &[
                                ("scenario", scenario as u64),
                                ("route", route),
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
        assert!(native_application_smoke::verify_transcript(&valid).is_err());
        assert!(verify_transcript(&native_application_smoke::tests::valid_transcript()).is_err());
        let lines = valid.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 224);
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
        for extra in ["unexpected text", "{}", lines[223]] {
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
        assert!(verify_transcript(&valid.replace("\"calls\":276", "\"calls\":275")).is_err());
        let first_surface = surface_reference(0).unwrap().0[0];
        assert!(
            verify_transcript(&valid.replace(
                &format!("\"hash\":{first_surface}"),
                &format!("\"hash\":{}", first_surface + 1)
            ))
            .is_err()
        );
        let first_reply = packet_reference(0).1[0];
        assert!(
            verify_transcript(&valid.replace(
                &format!("\"hash\":{first_reply}"),
                &format!("\"hash\":{}", first_reply + 1)
            ))
            .is_err()
        );
        assert!(verify_transcript(&valid.replace("\"route\":1", "\"route\":0")).is_err());
        let mut lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for line in &mut lines[23..223] {
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
    fn capture_waits_for_both_routes_and_uses_the_second_document_frame() {
        let (_, pixels) = surface_reference(3).unwrap();
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
        assert_eq!(profile.log_prefix, "elf_document");
        assert_eq!(profile.artifact_prefix, "elf-document");
    }

    #[test]
    fn explicit_document_comparison_precedence_preserves_higher_profiles() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        for lower in [
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
