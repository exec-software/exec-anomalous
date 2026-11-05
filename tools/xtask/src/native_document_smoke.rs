//! Strict native document lifecycle and public-frame oracle. Expected document
//! replies and editor metadata are assembled from explicit checkpoints without
//! invoking the guest service or the opened-editor save implementation.
use super::*;
use anomalous_abi::application::{
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_editor_contract::{frame_byte_count, loaded_editor_state, render_loaded};
use native_application_smoke::{run_evidence, smoke_profile};
pub(super) const feature: &str = "native_document_test";
const ready_event: &str = "kernel.native_document_test.ready";
const scenario_event: &str = "kernel.native_document_test.scenario";
const surface_event: &str = "kernel.native_document_test.surface";
const packet_event: &str = "kernel.native_document_test.packet";
const passed_event: &str = "kernel.native_document_test.passed";
const profile: smoke_profile = smoke_profile {
    log_prefix: "native_document",
    artifact_prefix: "native-document",
    records: 123,
    verify: verify_records,
    verify_frame,
};
pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && native_runtime_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|v| *v != feature)
                .chain(core::iter::once(native_runtime_smoke::feature))
                .collect::<Vec<_>>(),
        )
}
pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!("Native document image inspected: {}", image.path.display());
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
        return Err("native document proof requires exactly 123 ordered records".into());
    }
    let evidence = native_application_smoke::verify_records(&lines[..21].join("\n"))?;
    verify_numbers(
        &json_parser::new(lines[21]).parse_record()?,
        ready_event,
        &ready_numbers(),
        &[],
    )?;
    let mut cursor = 22;
    for scenario in 0..4 {
        let record = json_parser::new(lines[cursor]).parse_record()?;
        cursor += 1;
        verify_numbers(
            &record,
            scenario_event,
            &scenario_numbers(scenario),
            &["timer_ticks", "fault_kind", "resources_restored"],
        )?;
        if required_number(&record, scenario_event, "timer_ticks")? > 10_000 {
            return Err("document timer bound exceeded".into());
        }
        require_string(&record, scenario_event, "fault_kind", fault_kind(scenario))?;
        require_bool(&record, scenario_event, "resources_restored", true)?;
        for (index, value) in surface_reference(scenario)?.0.into_iter().enumerate() {
            verify_numbers(
                &json_parser::new(lines[cursor]).parse_record()?,
                surface_event,
                &[
                    ("scenario", scenario as u64),
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
    if cursor != lines.len() - 1 {
        return Err("document record count mismatch".into());
    }
    let record = json_parser::new(lines[cursor]).parse_record()?;
    verify_numbers(
        &record,
        passed_event,
        &passed_numbers(),
        &[
            "resources_restored",
            "authority_revoked",
            "source_language",
            "store_scope",
        ],
    )?;
    require_bool(&record, passed_event, "resources_restored", true)?;
    require_bool(&record, passed_event, "authority_revoked", true)?;
    require_string(&record, passed_event, "source_language", "rust")?;
    require_string(&record, passed_event, "store_scope", "service_lifetime")?;
    Ok(evidence)
}
pub(super) fn verify_numbers(
    record: &structured_record,
    event: &str,
    numbers: &[(&str, u64)],
    extra: &[&str],
) -> Result<(), String> {
    let mut fields = numbers.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    fields.extend_from_slice(extra);
    require_control_header(record, event, &fields)?;
    for (name, value) in numbers {
        require_number(record, event, name, *value)?;
    }
    Ok(())
}
pub(super) fn ready_numbers() -> Vec<(&'static str, u64)> {
    vec![
        ("tasks", 2),
        ("scenarios", 4),
        ("channels", 4),
        ("descriptor_bytes", 128),
        ("arena_bytes", 32768),
        ("mappings_per_task", 4),
        ("data_frames", 48),
        ("table_frames", 18),
        ("leases", 12),
    ]
}
fn passed_numbers() -> Vec<(&'static str, u64)> {
    vec![
        ("application_tasks", 8),
        ("service_lifetimes", 4),
        ("normal_runs", 2),
        ("contained_faults", 2),
        ("zeroed_bytes", 262144),
    ]
}
pub(super) fn fault_kind(scenario: usize) -> &'static str {
    match scenario {
        1 => "before_commit",
        2 => "after_commit",
        _ => "none",
    }
}
pub(super) fn scenario_numbers(scenario: usize) -> Vec<(&'static str, u64)> {
    let normal = matches!(scenario, 0 | 3);
    let (surfaces, inputs, requests, replies) = if normal { (8, 4, 14, 13) } else { (3, 2, 6, 4) };
    vec![
        ("scenario", scenario as u64),
        ("starts", 2),
        ("zeroed_bytes", 65536),
        ("calls", if normal { 276 } else { 107 }),
        ("cooperative_switches", if normal { 42 } else { 17 }),
        ("surfaces", surfaces),
        ("inputs", inputs),
        ("requests", requests),
        ("replies", replies),
        ("surface_deliveries", surfaces),
        ("input_deliveries", inputs),
        ("request_deliveries", requests),
        ("reply_deliveries", replies),
        ("surface_acks", surfaces),
        ("input_acks", inputs),
        ("request_acks", if normal { requests } else { requests - 1 }),
        ("reply_acks", replies),
        ("denials", 4),
        ("bad_copies", 3),
        ("invalid", 2),
        ("closed", if normal { 0 } else { 2 }),
        ("discarded", if normal { 0 } else { 1 }),
        ("rows", if normal { 24 } else { 0 }),
        ("exits", if normal { 2 } else { 1 }),
        ("faults", if normal { 0 } else { 1 }),
        ("arena_copies", if normal { 102 } else { 30 }),
    ]
}
pub(super) fn verify_frame(bytes: &[u8]) -> Result<(), String> {
    let (_, pixels) = surface_reference(3)?;
    native_application_smoke::verify_frame_pixels(bytes, &pixels)
}
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |value, byte| {
        (value ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}
pub(super) fn surface_reference(
    scenario: usize,
) -> Result<(Vec<u64>, [u8; frame_byte_count]), String> {
    let (a, b, c): (&[u8], &[u8], &[u8]) = if scenario == 3 {
        (
            b"const before = 2;",
            b"const after = 2;",
            b"const later = 2;",
        )
    } else {
        (
            b"const before = 1;",
            b"const after = 1;",
            b"const later = 1;",
        )
    };
    let checkpoints = [
        (a, (0, 0), 1, false, 0),
        (a, (6, 12), 1, false, 0),
        (b, (11, 11), 2, true, 1),
        (a, (6, 12), 3, false, 0),
        (c, (11, 11), 4, true, 1),
        (c, (11, 11), 4, true, 1),
        (c, (11, 11), 4, false, 1),
        (c, (0, 0), 1, false, 0),
    ];
    let mut pixels = [0; frame_byte_count];
    let mut hashes = Vec::new();
    let count = if matches!(scenario, 1 | 2) { 3 } else { 8 };
    for (index, (text, selection, revision, dirty, undo)) in
        checkpoints.into_iter().take(count).enumerate()
    {
        let mut state =
            loaded_editor_state::from_text(text).map_err(|e| format!("surface text: {e:?}"))?;
        state
            .set_selection(selection.0, selection.1)
            .map_err(|e| format!("surface selection: {e:?}"))?;
        render_loaded(&state, &mut pixels);
        let packet = application_surface_v1::try_new(
            application_surface_state {
                request_sequence: index as u64 + 1,
                document_revision: revision,
                text,
                selection_anchor: selection.0 as u8,
                selection_head: selection.1 as u8,
                dirty,
                theme: application_theme::dark,
                undo_depth: undo,
            },
            &pixels,
        )
        .map_err(|e| format!("surface packet: {e:?}"))?;
        let mut bytes = [0; application_surface_v1_size];
        packet
            .encode_into(&mut bytes)
            .map_err(|e| format!("surface encode: {e:?}"))?;
        hashes.push(hash(&bytes));
    }
    Ok((hashes, pixels))
}

// Build canonical expected bytes independently of document_store::execute.
fn wire(kind: u8, status: u8, tuple: [u64; 6], text: &[u8]) -> [u8; 128] {
    let mut bytes = [0; 128];
    bytes[..8].copy_from_slice(&[b'A', b'N', b'D', b'C', 1, 0, 128, 0]);
    bytes[8] = kind;
    bytes[9] = status;
    for (index, value) in tuple.into_iter().enumerate() {
        bytes[16 + index * 8..24 + index * 8].copy_from_slice(&value.to_le_bytes());
    }
    bytes[64] = text.len() as u8;
    bytes[65..65 + text.len()].copy_from_slice(text);
    bytes
}
pub(super) fn packet_reference(scenario: usize) -> (Vec<u64>, Vec<u64>) {
    let d = 0xd001 + scenario as u64;
    let old = if scenario == 0 { d + 1 } else { d - 1 };
    let (a, b, c): (&[u8], &[u8], &[u8]) = if scenario == 3 {
        (
            b"const before = 2;",
            b"const after = 2;",
            b"const later = 2;",
        )
    } else {
        (
            b"const before = 1;",
            b"const after = 1;",
            b"const later = 1;",
        )
    };
    let mut malformed = wire(1, 0, [d, 0, 1, 0, 0, 0], b"");
    malformed[0] ^= 1;
    let mut requests = vec![
        malformed,
        wire(1, 0, [d, 0, 1, 0, 0, 0], b""),
        wire(3, 0, [old, 1, 2, 0, 0, 0], b""),
        wire(3, 0, [d, 2, 3, 0, 0, 0], b""),
        wire(5, 0, [d, 1, 90, 9, 0, 1], b"inert"),
        wire(5, 0, [d, 1, 1, 1, 0, 2], b),
    ];
    let mut replies = vec![
        wire(2, 0, [d, 1, 1, 0, 1, 0], a),
        wire(9, 2, [old, 1, 2, 0, 0, 0], b""),
        wire(9, 4, [d, 2, 3, 0, 0, 0], b""),
        wire(9, 6, [d, 1, 90, 9, 0, 1], b"inert"),
    ];
    if matches!(scenario, 0 | 3) {
        requests.extend([
            wire(3, 0, [d, 1, 4, 0, 0, 0], b""),
            wire(5, 0, [d, 1, 2, 2, 0, 4], c),
            wire(7, 0, [d, 1, 5, 0, 0, 0], b""),
            wire(3, 0, [d, 1, 6, 0, 0, 0], b""),
            wire(1, 0, [d, 0, 7, 0, 0, 0], b""),
            wire(3, 0, [d, 1, 8, 0, 0, 0], b""),
            wire(3, 0, [d, 2, 9, 0, 0, 0], b""),
            wire(7, 0, [d, 2, 10, 0, 0, 0], b""),
        ]);
        replies.extend([
            wire(6, 0, [d, 1, 1, 1, 2, 2], b),
            wire(4, 0, [d, 1, 4, 0, 2, 0], b),
            wire(6, 0, [d, 1, 2, 2, 3, 4], c),
            wire(8, 0, [d, 1, 5, 0, 0, 0], b""),
            wire(9, 5, [d, 1, 6, 0, 0, 0], b""),
            wire(2, 0, [d, 2, 7, 0, 3, 0], c),
            wire(9, 4, [d, 1, 8, 0, 0, 0], b""),
            wire(4, 0, [d, 2, 9, 0, 3, 0], c),
            wire(8, 0, [d, 2, 10, 0, 0, 0], b""),
        ]);
    }
    (
        requests.iter().map(|bytes| hash(bytes)).collect(),
        replies.iter().map(|bytes| hash(bytes)).collect(),
    )
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
        ];
        for scenario in 0..4 {
            lines.push(event(
                scenario_event,
                &scenario_numbers(scenario),
                &format!(
                    r#", "timer_ticks":3,"fault_kind":"{}","resources_restored":true"#,
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
                            ("channel", channel),
                            ("sequence", index as u64 + 1),
                            ("hash", hash),
                        ],
                        "",
                    ));
                }
            }
        }
        lines.push(event(passed_event,&passed_numbers(),r#", "resources_restored":true,"authority_revoked":true,"source_language":"rust","store_scope":"service_lifetime""#));
        lines.join("\n")
    }
    #[test]
    fn complete_ordered_lifecycle_is_required_and_prior_oracle_is_preserved() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        verify_transcript(&valid.replace("GenuineIntel", "AuthenticAMD")).unwrap();
        assert!(native_application_smoke::verify_transcript(&valid).is_err());
        let lines = valid.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 123);
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
    fn metadata_and_hashes_reject_omissions_mutations_duplicates_and_private_fields() {
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
    fn final_capture_is_reopened_second_document_and_requires_complete_records() {
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
        verify_frame(&ppm).unwrap();
        assert!(native_application_smoke::verify_frame(&ppm).is_err());
        ppm[header.len() + (32 * 1024 + 32) * 3] ^= 1;
        assert!(verify_frame(&ppm).is_err());
        assert!(verify_frame(&ppm[..ppm.len() - 1]).is_err());
        let text = valid_transcript() + "\n";
        let mut prefix = String::new();
        for line in text.lines() {
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
    }
    #[test]
    fn explicit_wire_layout_round_trips_and_invalid_probe_stays_invalid() {
        use anomalous_document_protocol::record;
        let valid = wire(5, 0, [0xd001, 1, 1, 1, 0, 2], b"const after = 1;");
        assert_eq!(
            valid,
            record::save(0xd001, 1, 1, 1, 2, b"const after = 1;")
                .unwrap()
                .encode()
        );
        assert_eq!(record::decode(&valid).unwrap().encode(), valid);
        let mut malformed = wire(1, 0, [0xd001, 0, 1, 0, 0, 0], b"");
        malformed[0] ^= 1;
        assert!(record::decode(&malformed).is_err());
        for scenario in 0..4 {
            let (requests, replies) = packet_reference(scenario);
            assert_eq!(
                (requests.len(), replies.len()),
                if matches!(scenario, 0 | 3) {
                    (14, 13)
                } else {
                    (6, 4)
                }
            );
        }
        let first = surface_reference(0).unwrap();
        let last = surface_reference(3).unwrap();
        assert_ne!(first.1, last.1);
        for (a, b) in first.0.iter().zip(last.0.iter()) {
            assert_ne!(a, b);
        }
    }
    #[test]
    fn native_document_selection_respects_foreign_profile_precedence() {
        for lower in [
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
            let values = [feature, lower];
            assert!(selected(&values));
            assert!(!native_runtime_smoke::selected(&values));
            assert!(!native_application_smoke::selected(&values));
            assert!(!ny_object_smoke::selected(&values));
            assert!(!application_profile_smoke::selected(&values));
            assert!(!owned_dispatcher_smoke::selected(&values));
            assert!(!owned_multi_region_smoke::selected(&values));
            assert!(!owned_source_smoke::selected(&values));
            assert!(!owned_tls_smoke::selected(&values));
            assert!(!dispatcher_smoke::selected(&values));
            assert!(!workspace_dev::selected(&values));
            assert!(!owned_workspace::selected(&values));
        }
        for (higher, selector) in [
            (
                elf_application_smoke::feature,
                elf_application_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                linux_application_smoke::feature,
                linux_application_smoke::selected,
            ),
            (
                veil_application_smoke::feature,
                veil_application_smoke::selected,
            ),
            (shared_veil_smoke::feature, shared_veil_smoke::selected),
        ] {
            let values = [feature, native_runtime_smoke::feature, higher];
            assert!(!selected(&values));
            assert!(selector(&values));
        }
        assert!(!selected(all_kernel_features));
    }
}
