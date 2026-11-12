//! Strict native editor replacement oracle. Document bytes and surfaces come
//! from explicit checkpoints, independently of the guest store/save machinery.
use super::*;
use anomalous_abi::application::{
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_editor_contract::{frame_byte_count, loaded_editor_state, render_loaded};
use native_application_smoke::{run_evidence, smoke_profile};
use native_document_smoke::verify_numbers;

pub(super) const feature: &str = "native_relaunch_test";
const ready_event: &str = "kernel.native_relaunch_test.ready";
const maintenance_event: &str = "kernel.native_relaunch_test.maintenance";
const scenario_event: &str = "kernel.native_relaunch_test.scenario";
const surface_event: &str = "kernel.native_relaunch_test.surface";
const packet_event: &str = "kernel.native_relaunch_test.packet";
const passed_event: &str = "kernel.native_relaunch_test.passed";
pub(super) const maintenance_flags: [&str; 5] = [
    "old_identity_revoked",
    "cancelled_identity_revoked",
    "service_preserved",
    "route_authority_replaced",
    "editor_generation_advanced",
];
const profile: smoke_profile = smoke_profile {
    log_prefix: "native_relaunch",
    artifact_prefix: "native-relaunch",
    records: 122,
    verify: verify_records,
    verify_frame,
};

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
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
                        "native_relaunch_test"
                            | "elf_document_test"
                            | "elf_application_test"
                            | "native_durable_test"
                            | "native_durable_failure_test"
                            | "native_repeated_editor_test"
                            | "native_document_test"
                            | "native_runtime_test"
                            | "native_program_test"
                            | "native_manifest_test"
                    )
                })
                .chain(core::iter::once(native_application_smoke::feature))
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!(
        "Native editor replacement image inspected: {}",
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
        return Err("native relaunch proof requires exactly 122 ordered records".into());
    }
    let evidence = native_application_smoke::verify_records(&lines[..21].join("\n"))?;
    verify_numbers(
        &json_parser::new(lines[21]).parse_record()?,
        ready_event,
        &ready_numbers(),
        &[],
    )?;
    let mut cursor = 22;
    for scenario in 0..3 {
        let maintenance = json_parser::new(lines[cursor]).parse_record()?;
        cursor += 1;
        verify_numbers(
            &maintenance,
            maintenance_event,
            &maintenance_numbers(scenario),
            &maintenance_flags,
        )?;
        for flag in maintenance_flags {
            require_bool(&maintenance, maintenance_event, flag, true)?;
        }
        let record = json_parser::new(lines[cursor]).parse_record()?;
        cursor += 1;
        verify_numbers(
            &record,
            scenario_event,
            &scenario_numbers(scenario)?,
            &["timer_ticks", "fault_kind", "resources_restored"],
        )?;
        if required_number(&record, scenario_event, "timer_ticks")? > 10_000 {
            return Err("relaunch timer bound exceeded".into());
        }
        require_string(&record, scenario_event, "fault_kind", fault_kind(scenario))?;
        require_bool(&record, scenario_event, "resources_restored", true)?;
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
    if cursor != lines.len() - 1 {
        return Err("relaunch record count mismatch".into());
    }
    let passed = json_parser::new(lines[cursor]).parse_record()?;
    verify_numbers(
        &passed,
        passed_event,
        &passed_numbers(),
        &[
            "resources_restored",
            "authority_revoked",
            "source_language",
            "store_scope",
        ],
    )?;
    require_bool(&passed, passed_event, "resources_restored", true)?;
    require_bool(&passed, passed_event, "authority_revoked", true)?;
    require_string(&passed, passed_event, "source_language", "rust")?;
    require_string(&passed, passed_event, "store_scope", "service_lifetime")?;
    Ok(evidence)
}

pub(super) fn ready_numbers() -> Vec<(&'static str, u64)> {
    vec![
        ("tasks", 2),
        ("scenarios", 3),
        ("channels", 4),
        ("descriptor_bytes", 128),
        ("arena_bytes", 32768),
        ("mappings_per_task", 4),
        ("data_frames", 48),
        ("table_frames", 18),
        ("leases", 12),
    ]
}

pub(super) fn maintenance_numbers(scenario: usize) -> Vec<(&'static str, u64)> {
    vec![
        ("scenario", scenario as u64),
        ("frames_before", 66),
        ("frames_survivor", 33),
        ("frames_after", 66),
        ("leases_before", 12),
        ("leases_survivor", 6),
        ("leases_after", 12),
        ("release_failures", 1),
        ("release_retries", 1),
        ("prepare_failures", 1),
        ("prepare_rollbacks", 1),
        ("publication_failures", 1),
        ("publication_rollbacks", 1),
        ("service_checks", 8),
        ("zeroed_bytes", 32768),
    ]
}

pub(super) fn scenario_numbers(scenario: usize) -> Result<Vec<(&'static str, u64)>, String> {
    let Some(
        [
            calls,
            switches,
            denials,
            bad_copies,
            invalid,
            closed,
            arena_copies,
        ],
    ) = mechanism_counts(scenario)
    else {
        return Err("unknown native relaunch scenario".into());
    };
    let fault = u64::from(scenario != 0);
    let inputs = if scenario == 2 { 8 } else { 7 };
    let requests = 11 + fault;
    let replies = if scenario == 2 { 12 } else { 11 };
    Ok(vec![
        ("scenario", scenario as u64),
        ("starts", 3),
        ("zeroed_bytes", 98304),
        ("calls", calls),
        ("cooperative_switches", switches),
        ("surfaces", 8),
        ("inputs", inputs),
        ("requests", requests),
        ("replies", replies),
        ("surface_deliveries", 8),
        ("input_deliveries", inputs),
        (
            "request_deliveries",
            if scenario == 1 { 11 } else { requests },
        ),
        ("reply_deliveries", 11),
        ("surface_acks", 8),
        ("input_acks", inputs),
        ("request_acks", if scenario == 1 { 11 } else { requests }),
        ("reply_acks", 11),
        ("denials", denials),
        ("bad_copies", bad_copies),
        ("invalid", invalid),
        ("closed", closed),
        ("discarded", fault),
        ("rows", 24),
        ("exits", 3 - fault),
        ("faults", fault),
        ("arena_copies", arena_copies),
    ])
}

fn mechanism_counts(scenario: usize) -> Option<[u64; 7]> {
    // Independently counted from the two client call streams, including every
    // empty poll/yield and the service-only copied successor descriptor.
    match scenario {
        0 => Some([292, 47, 8, 1, 9, 2, 99]),
        1 => Some([293, 47, 8, 1, 9, 2, 100]),
        2 => Some([303, 49, 8, 1, 9, 2, 104]),
        _ => None,
    }
}

pub(super) fn fault_kind(scenario: usize) -> &'static str {
    match scenario {
        0 => "none",
        1 => "before_commit",
        _ => "after_commit",
    }
}

fn passed_numbers() -> Vec<(&'static str, u64)> {
    vec![
        ("application_tasks", 9),
        ("service_lifetimes", 3),
        ("replacements", 3),
        ("cancelled_tasks", 3),
        ("contained_faults", 2),
        ("zeroed_bytes", 294912),
    ]
}

pub(super) fn verify_frame(bytes: &[u8]) -> Result<(), String> {
    let (_, pixels) = surface_reference(2, 1)?;
    native_application_smoke::verify_frame_pixels(bytes, &pixels)
}

fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |value, byte| {
        (value ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

// These checkpoints do not invoke the opened-editor save implementation. The
// portable renderer supplies cross-build pixel evidence, not another font.
pub(super) fn surface_reference(
    scenario: usize,
    incarnation: usize,
) -> Result<(Vec<u64>, [u8; frame_byte_count]), String> {
    let a = b"const before = 1;".as_slice();
    let b = b"const after = 1;".as_slice();
    let c = b"const later = 1;".as_slice();
    let d = b"const newer = 1;".as_slice();
    let recovered = if scenario == 2 { c } else { b };
    let checkpoints = if incarnation == 0 {
        [
            (a, (0, 0), 1, false, 0),
            (b, (11, 11), 2, true, 1),
            (b, (11, 11), 2, false, 1),
            (c, (11, 11), 4, true, 1),
        ]
    } else {
        [
            (recovered, (0, 0), 1, false, 0),
            (d, (11, 11), 2, true, 1),
            (d, (11, 11), 2, false, 1),
            (d, (0, 0), 1, false, 0),
        ]
    };
    let mut pixels = [0; frame_byte_count];
    let mut hashes = Vec::new();
    for (index, (text, selection, revision, dirty, undo)) in checkpoints.into_iter().enumerate() {
        let mut state = loaded_editor_state::from_text(text)
            .map_err(|error| format!("relaunch surface text: {error:?}"))?;
        state
            .set_selection(selection.0, selection.1)
            .map_err(|error| format!("relaunch surface selection: {error:?}"))?;
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
        .map_err(|error| format!("relaunch surface packet: {error:?}"))?;
        let mut bytes = [0; application_surface_v1_size];
        packet
            .encode_into(&mut bytes)
            .map_err(|error| format!("relaunch surface encode: {error:?}"))?;
        hashes.push(hash(&bytes));
    }
    Ok((hashes, pixels))
}

// Canonical ANDC bytes are assembled manually. Neither the service store nor
// its request/reply constructors participate in the expected-wire oracle.
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

pub(super) fn packet_reference(
    scenario: usize,
    incarnation: usize,
    document: u64,
) -> (Vec<u64>, Vec<u64>) {
    let a = b"const before = 1;".as_slice();
    let b = b"const after = 1;".as_slice();
    let c = b"const later = 1;".as_slice();
    let d = b"const newer = 1;".as_slice();
    let (requests, replies) = if incarnation == 0 {
        let mut requests = vec![
            wire(1, 0, [document, 0, 1, 0, 0, 0], b""),
            wire(5, 0, [document, 1, 1, 1, 0, 2], b),
        ];
        let mut replies = vec![
            wire(2, 0, [document, 1, 1, 0, 1, 0], a),
            wire(6, 0, [document, 1, 1, 1, 2, 2], b),
        ];
        if scenario != 0 {
            requests.push(wire(5, 0, [document, 1, 2, 2, 0, 4], c));
        }
        if scenario == 2 {
            replies.push(wire(6, 0, [document, 1, 2, 2, 3, 4], c));
        }
        (requests, replies)
    } else {
        let base = if scenario == 2 { 3 } else { 2 };
        let version = base + 1;
        let recovered = if scenario == 2 { c } else { b };
        (
            vec![
                wire(1, 0, [document, 0, 1, 0, 0, 0], b""),
                wire(5, 0, [document, 1, 2, 2, 0, 4], c),
                wire(5, 0, [document, 2, 1, base, 0, 2], d),
                wire(3, 0, [document, 2, 2, 0, 0, 0], b""),
                wire(7, 0, [document, 2, 3, 0, 0, 0], b""),
                wire(1, 0, [document, 0, 4, 0, 0, 0], b""),
                wire(3, 0, [document, 2, 5, 0, 0, 0], b""),
                wire(3, 0, [document, 3, 6, 0, 0, 0], b""),
                wire(7, 0, [document, 3, 7, 0, 0, 0], b""),
            ],
            vec![
                wire(2, 0, [document, 2, 1, 0, base, 0], recovered),
                wire(9, 4, [document, 1, 2, 2, 0, 4], c),
                wire(6, 0, [document, 2, 1, base, version, 2], d),
                wire(4, 0, [document, 2, 2, 0, version, 0], d),
                wire(8, 0, [document, 2, 3, 0, 0, 0], b""),
                wire(2, 0, [document, 3, 4, 0, version, 0], d),
                wire(9, 4, [document, 2, 5, 0, 0, 0], b""),
                wire(4, 0, [document, 3, 6, 0, version, 0], d),
                wire(8, 0, [document, 3, 7, 0, 0, 0], b""),
            ],
        )
    };
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
        for scenario in 0..3 {
            let flags = maintenance_flags
                .map(|flag| format!(",{flag:?}:true"))
                .join("");
            lines.push(event(
                maintenance_event,
                &maintenance_numbers(scenario),
                &flags,
            ));
            lines.push(event(
                scenario_event,
                &scenario_numbers(scenario).unwrap(),
                &format!(
                    r#", "timer_ticks":3,"fault_kind":"{}","resources_restored":true"#,
                    fault_kind(scenario)
                ),
            ));
            for incarnation in 0..2 {
                for (index, value) in surface_reference(scenario, incarnation)
                    .unwrap()
                    .0
                    .into_iter()
                    .enumerate()
                {
                    lines.push(event(
                        surface_event,
                        &[
                            ("scenario", scenario as u64),
                            ("incarnation", incarnation as u64),
                            ("sequence", index as u64 + 1),
                            ("hash", value),
                        ],
                        "",
                    ));
                }
                let (requests, replies) =
                    packet_reference(scenario, incarnation, 0xd101 + scenario as u64);
                for (channel, hashes) in [(2, requests), (3, replies)] {
                    for (index, value) in hashes.into_iter().enumerate() {
                        lines.push(event(
                            packet_event,
                            &[
                                ("scenario", scenario as u64),
                                ("incarnation", incarnation as u64),
                                ("channel", channel),
                                ("sequence", index as u64 + 1),
                                ("hash", value),
                            ],
                            "",
                        ));
                    }
                }
            }
        }
        lines.push(event(passed_event, &passed_numbers(), r#", "resources_restored":true,"authority_revoked":true,"source_language":"rust","store_scope":"service_lifetime""#));
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
    fn exact_lifetime_order_requires_every_record_and_preserves_prior_oracles() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        verify_transcript(&valid.replace("GenuineIntel", "AuthenticAMD")).unwrap();
        verify_transcript(&format!("\r\n{}\r\n", valid.replace('\n', "\r\n"))).unwrap();
        assert!(native_application_smoke::verify_transcript(&valid).is_err());
        assert!(native_document_smoke::verify_transcript(&valid).is_err());
        assert!(elf_document_smoke::verify_transcript(&valid).is_err());
        assert!(verify_transcript(&native_application_smoke::tests::valid_transcript()).is_err());
        let lines = valid.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 122);
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
        for extra in ["unexpected text", "{}", lines[121]] {
            assert!(verify_transcript(&format!("{valid}\n{extra}")).is_err());
        }
    }

    #[test]
    fn every_lifecycle_field_rejects_omission_mutation_duplicates_and_private_extras() {
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
    fn before_and_after_commit_recovery_and_sequence_resets_remain_distinct() {
        let valid = valid_transcript();
        let b = surface_reference(1, 1).unwrap().0[0];
        let c = surface_reference(2, 1).unwrap().0[0];
        assert_ne!(b, c);
        assert!(
            verify_transcript(&valid.replace(&format!("\"hash\":{c}"), &format!("\"hash\":{b}")))
                .is_err()
        );
        let (requests, replies) = packet_reference(2, 1, 0xd103);
        assert_eq!((requests.len(), replies.len()), (9, 9));
        let wrong = hash(&wire(2, 0, [0xd103, 2, 1, 0, 2, 0], b"const after = 1;"));
        assert!(
            verify_transcript(&valid.replace(
                &format!("\"hash\":{}", replies[0]),
                &format!("\"hash\":{wrong}")
            ))
            .is_err()
        );
        assert!(
            verify_transcript(&valid.replace("\"incarnation\":1", "\"incarnation\":0")).is_err()
        );
        assert!(verify_transcript(&valid.replace("\"discarded\":1", "\"discarded\":0")).is_err());
        for ticks in [0, 10000] {
            verify_transcript(
                &valid.replace("\"timer_ticks\":3", &format!("\"timer_ticks\":{ticks}")),
            )
            .unwrap();
        }
        assert!(
            verify_transcript(&valid.replace("\"timer_ticks\":3", "\"timer_ticks\":10001"))
                .is_err()
        );
    }

    #[test]
    fn capture_requires_all_replacements_and_the_reopened_final_document() {
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
        verify_frame(&ppm).unwrap();
        assert!(native_application_smoke::verify_frame(&ppm).is_err());
        assert!(native_document_smoke::verify_frame(&ppm).is_err());
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
    fn relaunch_selection_is_explicit_and_preserves_higher_profile_priority() {
        assert!(!selected(&[]));
        assert!(!selected(&["native_document_test"]));
        assert!(selected(&[feature]));
        assert!(selected(&[
            feature,
            "native_document_test",
            "elf_document_test",
            "elf_application_test"
        ]));
        for higher in [
            "linux_application_test",
            "veil_application_test",
            "shared_veil_test",
            "linux_tls_test",
            "linux_loader_entry_test",
            "memory_test",
            "supervision_test",
            "desktop_dev",
        ] {
            assert!(!selected(&[feature, higher]), "selected over {higher}");
        }
    }
}
