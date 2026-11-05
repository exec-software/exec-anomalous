//! Strict startup, arena ownership, fault and loaded-document frame oracle.
use super::*;
use anomalous_abi::application::{
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_editor_contract::{frame_byte_count, loaded_editor_state, render_loaded, theme};
use native_application_smoke::{run_evidence, smoke_profile};
pub(super) const feature: &str = "native_runtime_test";
const ready_event: &str = "kernel.native_runtime_test.ready";
const scenario_event: &str = "kernel.native_runtime_test.scenario";
const surface_event: &str = "kernel.native_runtime_test.surface";
const passed_event: &str = "kernel.native_runtime_test.passed";
const profile: smoke_profile = smoke_profile {
    log_prefix: "native_runtime",
    artifact_prefix: "native-runtime",
    records: 41,
    verify: verify_records,
    verify_frame,
};
pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.contains(&native_document_smoke::feature)
        && native_application_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|v| *v != feature)
                .chain(core::iter::once(native_application_smoke::feature))
                .collect::<Vec<_>>(),
        )
}
pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!(
        "Native startup/arena image inspected: {}",
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
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != profile.records {
        return Err(
            "Native runtime requires exactly 21 baseline and 20 startup/arena records".into(),
        );
    }
    let evidence = native_application_smoke::verify_records(&lines[..21].join("\n"))?;
    let ready = json_parser::new(lines[21]).parse_record()?;
    require_control_header(
        &ready,
        ready_event,
        &[
            "tasks",
            "scenarios",
            "descriptor_bytes",
            "arena_bytes",
            "mappings_per_task",
            "data_frames",
            "table_frames",
            "leases",
        ],
    )?;
    for (field, value) in [
        ("tasks", 2),
        ("scenarios", 4),
        ("descriptor_bytes", 128),
        ("arena_bytes", 32768),
        ("mappings_per_task", 4),
        ("data_frames", 48),
        ("table_frames", 18),
        ("leases", 12),
    ] {
        require_number(&ready, ready_event, field, value)?;
    }
    let mut cursor = 22;
    for scenario in [0, 2, 3, 1] {
        verify_scenario(&json_parser::new(lines[cursor]).parse_record()?, scenario)?;
        cursor += 1;
        if scenario < 2 {
            let (hashes, _) = reference(scenario)?;
            for (index, hash) in hashes.into_iter().enumerate() {
                let record = json_parser::new(lines[cursor]).parse_record()?;
                require_control_header(&record, surface_event, &["scenario", "sequence", "hash"])?;
                require_number(&record, surface_event, "scenario", scenario as u64)?;
                require_number(&record, surface_event, "sequence", index as u64 + 1)?;
                require_number(&record, surface_event, "hash", hash)?;
                cursor += 1;
            }
        }
    }
    let passed = json_parser::new(lines[cursor]).parse_record()?;
    require_control_header(
        &passed,
        passed_event,
        &[
            "application_tasks",
            "descriptor_bytes",
            "arena_bytes",
            "documents",
            "layouts",
            "normal_runs",
            "contained_faults",
            "zeroed_bytes",
            "resources_restored",
            "authority_revoked",
            "source_language",
        ],
    )?;
    for (field, value) in [
        ("application_tasks", 8),
        ("descriptor_bytes", 128),
        ("arena_bytes", 32768),
        ("documents", 2),
        ("layouts", 2),
        ("normal_runs", 2),
        ("contained_faults", 2),
        ("zeroed_bytes", 262144),
    ] {
        require_number(&passed, passed_event, field, value)?;
    }
    require_bool(&passed, passed_event, "resources_restored", true)?;
    require_bool(&passed, passed_event, "authority_revoked", true)?;
    require_string(&passed, passed_event, "source_language", "rust")?;
    Ok(evidence)
}
fn verify_scenario(record: &structured_record, scenario: usize) -> Result<(), String> {
    let normal = scenario < 2;
    // Source-derived deterministic gate trace: two starts, seven surface/ six input
    // round trips and 24 final rows. Fault cases stop after one queued surface.
    let numbers = [
        ("scenario", scenario as u64),
        ("document", (scenario % 2) as u64),
        ("layout", (scenario % 2) as u64),
        ("starts", 2),
        ("zeroed_bytes", 65536),
        ("calls", if normal { 114 } else { 14 }),
        ("cooperative_switches", if normal { 14 } else { 0 }),
        ("surfaces", if normal { 7 } else { 1 }),
        ("inputs", if normal { 6 } else { 0 }),
        ("surface_deliveries", if normal { 7 } else { 0 }),
        ("input_deliveries", if normal { 6 } else { 0 }),
        ("surface_acks", if normal { 7 } else { 0 }),
        ("input_acks", if normal { 6 } else { 0 }),
        ("denials", if normal { 4 } else { 6 }),
        ("bad_copies", if normal { 2 } else { 1 }),
        ("invalid", if normal { 1 } else { 0 }),
        ("closed", if normal { 0 } else { 2 }),
        ("discarded", if normal { 0 } else { 1 }),
        ("rows", if normal { 24 } else { 0 }),
        ("exits", if normal { 2 } else { 1 }),
        ("faults", if normal { 0 } else { 1 }),
        ("arena_copies", if normal { 50 } else { 1 }),
    ];
    let mut fields = numbers.iter().map(|(f, _)| *f).collect::<Vec<_>>();
    fields.extend(["timer_ticks", "fault_kind", "resources_restored"]);
    require_control_header(record, scenario_event, &fields)?;
    for (field, value) in numbers {
        require_number(record, scenario_event, field, value)?;
    }
    if required_number(record, scenario_event, "timer_ticks")? > 10_000 {
        return Err("Native runtime timer bound exceeded".into());
    }
    require_string(
        record,
        scenario_event,
        "fault_kind",
        match scenario {
            2 => "guard",
            3 => "nx",
            _ => "none",
        },
    )?;
    require_bool(record, scenario_event, "resources_restored", true)?;
    Ok(())
}
fn verify_frame(bytes: &[u8]) -> Result<(), String> {
    let (_, pixels) = reference(1)?;
    native_application_smoke::verify_frame_pixels(bytes, &pixels)
}
fn reference(document: usize) -> Result<([u64; 7], [u8; frame_byte_count]), String> {
    let initial = match document {
        0 => &b"const before = 1;"[..],
        1 => &b"const before = 2;"[..],
        _ => return Err("unknown document".into()),
    };
    let changed = if document == 0 {
        &b"const after = 1;"[..]
    } else {
        &b"const after = 2;"[..]
    };
    let mut state =
        loaded_editor_state::from_text(initial).map_err(|e| format!("loaded reference: {e:?}"))?;
    let mut hashes = [0; 7];
    let mut pixels = [0; frame_byte_count];
    for (index, hash) in hashes.iter_mut().enumerate() {
        match index {
            0 => {}
            1 => state
                .set_selection(6, 12)
                .map_err(|error| format!("reference selection: {error:?}"))?,
            2 => state
                .insert(b"after")
                .map_err(|error| format!("reference insert: {error:?}"))?,
            3 => state.set_theme(theme::light),
            4 => state.set_theme(theme::dark),
            5 => state
                .set_selection(8, 8)
                .map_err(|error| format!("reference selection: {error:?}"))?,
            6 => state
                .undo()
                .map_err(|error| format!("reference undo: {error:?}"))?,
            _ => unreachable!(),
        }
        let (text, selection, revision, dirty, undo) = match index {
            0 => (initial, (0, 0), 1, false, 0),
            1 => (initial, (6, 12), 1, false, 0),
            2..=4 => (changed, (11, 11), 2, true, 1),
            5 => (changed, (8, 8), 2, true, 1),
            6 => (initial, (6, 12), 3, false, 0),
            _ => unreachable!(),
        };
        let appearance = if index == 3 {
            theme::light
        } else {
            theme::dark
        };
        if state.text() != text
            || state.selection() != selection
            || state.revision() != revision
            || state.is_dirty() != dirty
            || state.undo_depth() != undo
            || state.appearance() != appearance
        {
            return Err(format!(
                "portable editor disagrees with independent host checkpoint {}",
                index + 1
            ));
        }
        render_loaded(&state, &mut pixels);
        let packet = application_surface_v1::try_new(
            application_surface_state {
                request_sequence: index as u64 + 1,
                document_revision: revision,
                text,
                selection_anchor: selection.0 as u8,
                selection_head: selection.1 as u8,
                dirty,
                theme: if index == 3 {
                    application_theme::light
                } else {
                    application_theme::dark
                },
                undo_depth: undo,
            },
            &pixels,
        )
        .map_err(|error| format!("reference surface: {error:?}"))?;
        let mut bytes = [0; application_surface_v1_size];
        packet
            .encode_into(&mut bytes)
            .map_err(|error| format!("reference encode: {error:?}"))?;
        *hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325, |value, byte| {
            (value ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
        });
    }
    Ok((hashes, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid_transcript() -> String {
        let mut lines=vec![native_application_smoke::tests::valid_transcript(),
        r#"{"schema":1,"event":"kernel.native_runtime_test.ready","level":"info","tasks":2,"scenarios":4,"descriptor_bytes":128,"arena_bytes":32768,"mappings_per_task":4,"data_frames":48,"table_frames":18,"leases":12}"#.into()];
        for scenario in [0, 2, 3, 1] {
            let mut record=json_parser::new(if scenario<2 {
                r#"{"schema":1,"event":"kernel.native_runtime_test.scenario","level":"info","scenario":0,"document":0,"layout":0,"starts":2,"zeroed_bytes":65536,"calls":114,"timer_ticks":3,"cooperative_switches":14,"surfaces":7,"inputs":6,"surface_deliveries":7,"input_deliveries":6,"surface_acks":7,"input_acks":6,"denials":4,"bad_copies":2,"invalid":1,"closed":0,"discarded":0,"rows":24,"exits":2,"faults":0,"fault_kind":"none","arena_copies":50,"resources_restored":true}"#
            }else{
                r#"{"schema":1,"event":"kernel.native_runtime_test.scenario","level":"info","scenario":2,"document":0,"layout":0,"starts":2,"zeroed_bytes":65536,"calls":14,"timer_ticks":0,"cooperative_switches":0,"surfaces":1,"inputs":0,"surface_deliveries":0,"input_deliveries":0,"surface_acks":0,"input_acks":0,"denials":6,"bad_copies":1,"invalid":0,"closed":2,"discarded":1,"rows":0,"exits":1,"faults":1,"fault_kind":"guard","arena_copies":1,"resources_restored":true}"#
            }).parse_record().unwrap();
            for (field, value) in [
                ("scenario", scenario),
                ("document", scenario % 2),
                ("layout", scenario % 2),
            ] {
                record.insert(field.into(), json_scalar::number(value.to_string()));
            }
            if scenario == 3 {
                record.insert("fault_kind".into(), json_scalar::string("nx".into()));
            }
            lines.push(render(&record));
            if scenario < 2 {
                for (index, hash) in reference(scenario).unwrap().0.into_iter().enumerate() {
                    lines.push(format!(r#"{{"schema":1,"event":"kernel.native_runtime_test.surface","level":"info","scenario":{scenario},"sequence":{},"hash":{hash}}}"#,index+1));
                }
            }
        }
        lines.push(r#"{"schema":1,"event":"kernel.native_runtime_test.passed","level":"info","application_tasks":8,"descriptor_bytes":128,"arena_bytes":32768,"documents":2,"layouts":2,"normal_runs":2,"contained_faults":2,"zeroed_bytes":262144,"resources_restored":true,"authority_revoked":true,"source_language":"rust"}"#.into());
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
    fn runtime_proof_requires_exact_order_complete_pairs_and_native_baseline() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        verify_transcript(&valid.replace("GenuineIntel", "AuthenticAMD")).unwrap();
        assert!(native_application_smoke::verify_transcript(&valid).is_err());
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
    fn runtime_proof_rejects_missing_changed_duplicate_and_private_fields() {
        let lines = valid_transcript()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for index in 21..41 {
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
    fn loaded_documents_have_distinct_hashes_and_final_public_pixels() {
        let (first_hashes, first) = reference(0).unwrap();
        let (second_hashes, second) = reference(1).unwrap();
        for index in 0..7 {
            assert_ne!(first_hashes[index], second_hashes[index]);
        }
        assert_ne!(first, second);
        let header = b"P6\n1024 768\n255\n";
        let mut ppm = header.to_vec();
        ppm.resize(header.len() + 1024 * 768 * 3, 0);
        for y in 0..24 {
            for x in 0..384 {
                let source = ((y / 3) * 128 + x / 3) * 3;
                let destination = header.len() + ((32 + y) * 1024 + 32 + x) * 3;
                ppm[destination..destination + 3].copy_from_slice(&second[source..source + 3]);
            }
        }
        verify_frame(&ppm).unwrap();
        assert!(native_application_smoke::verify_frame(&ppm).is_err());
        ppm[header.len() + (32 * 1024 + 32) * 3] ^= 1;
        assert!(verify_frame(&ppm).is_err());
        assert!(verify_frame(&ppm[..ppm.len() - 1]).is_err());
        ppm.push(0);
        assert!(verify_frame(&ppm).is_err());
    }
    #[test]
    fn capture_requires_post_fault_fresh_document_and_complete_final_record() {
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
}
