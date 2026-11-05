//! Native completed-I/O service cuts and lost save replies. Expected contents
//! originate on the host after image construction, never in guest transcripts.
//! These contained service faults and clean QEMU exits are not power-loss cuts.
use super::*;
use anomalous_abi::application::{
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_editor_contract::{frame_byte_count, loaded_editor_state, render_loaded};
use native_document_smoke::verify_numbers;
#[cfg(test)]
use native_durable_smoke::crc32c;
use native_durable_smoke::{
    bounded_file, common_sector, fresh_media, hash, run_boot, sha256, u32_at, u64_at,
    valid_challenge, wire, write_new,
};

pub(super) const feature: &str = "native_durable_failure_test";
const phase_event: &str = "kernel.native_durable_failure_test.phase";
const channel_event: &str = "kernel.native_durable_failure_test.channel";
const packet_event: &str = "kernel.native_durable_failure_test.packet";
const device_event: &str = "kernel.native_durable_failure_test.device";
const passed_event: &str = "kernel.native_durable_failure_test.passed";
const media_bytes: usize = 1024 * 1024;
const sector_bytes: usize = 512;
const extent_start: usize = 8 * sector_bytes;
const extent_end: usize = extent_start + 5 * sector_bytes;
const ppm_size: usize = b"P6\n1024 768\n255\n".len() + 1024 * 768 * 3;

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && native_durable_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|v| *v != feature && *v != native_repeated_smoke::feature)
                .chain(core::iter::once(native_durable_smoke::feature))
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[native_durable_smoke::feature, feature], None)?;
    println!(
        "Native durable failure image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

fn fixture(phase: u64, case: u64, challenge: &[u8]) -> Result<[u8; 56], String> {
    if !matches!(phase, 0 | 2)
        || !(1..=7).contains(&case)
        || (phase == 0 && !valid_challenge(challenge))
        || (phase == 2 && !challenge.is_empty())
    {
        return Err("invalid independent durable failure fixture".into());
    }
    let mut bytes = [0; 56];
    bytes[..4].copy_from_slice(b"ANDI");
    bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
    bytes[8..16].copy_from_slice(&phase.to_le_bytes());
    bytes[16] = challenge.len() as u8;
    bytes[17..17 + challenge.len()].copy_from_slice(challenge);
    bytes[48..56].copy_from_slice(&case.to_le_bytes());
    Ok(bytes)
}

fn newer_text(challenge: &[u8]) -> Result<Vec<u8>, String> {
    if !valid_challenge(challenge) {
        return Err("invalid attempted save".into());
    }
    let mut text = challenge.to_vec();
    text[0] = if text[0] == b'~' { b'!' } else { text[0] + 1 };
    Ok(text)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct checkpoint {
    sequence: u64,
    epoch: u64,
    version: u64,
    text: Vec<u8>,
}

// Every valid pair is canonical and checksummed; an invalid inactive pair is
// tolerated only by the separate interrupted-update oracle, never by the old
// happy-path verifier. CRC is error detection, not authenticity or freshness.
fn decode_pair(bytes: &[u8], slot: usize) -> Result<checkpoint, String> {
    let start = extent_start + (1 + slot * 2) * sector_bytes;
    let body = &bytes[start..start + sector_bytes];
    let commit = &bytes[start + sector_bytes..start + 2 * sector_bytes];
    common_sector(body, b"ANDURBOD")?;
    common_sector(commit, b"ANDURCMT")?;
    let sequence = u64_at(body, 40);
    let epoch = u64_at(body, 48);
    let version = u64_at(body, 56);
    let length = usize::from(body[64]);
    if sequence == 0
        || version == 0
        || epoch.checked_add(version) != Some(sequence)
        || (epoch == 0 && (sequence != 1 || version != 1))
        || u64_at(body, 32) != slot as u64
        || (sequence - 1) % 2 != slot as u64
        || body[12..16] != [0; 4]
        || commit[12..16] != [0; 4]
        || body[32..64] != commit[32..64]
        || length > 31
        || body[65..65 + length]
            .iter()
            .any(|b| !(b' '..=b'~').contains(b))
        || body[65 + length..508].iter().any(|b| *b != 0)
        || u32_at(commit, 64) != u32_at(body, 508)
        || commit[68..508].iter().any(|b| *b != 0)
    {
        return Err("noncanonical or uncommitted durable failure checkpoint".into());
    }
    Ok(checkpoint {
        sequence,
        epoch,
        version,
        text: body[65..65 + length].to_vec(),
    })
}

fn decode_media(bytes: &[u8]) -> Result<Vec<checkpoint>, String> {
    if bytes.len() != media_bytes {
        return Err("dedicated disk length changed".into());
    }
    let header = &bytes[extent_start..extent_start + sector_bytes];
    common_sector(header, b"ANDURHDR")?;
    if u32_at(header, 12) != 5 || header[32..508].iter().any(|b| *b != 0) {
        return Err("durable failure media header is not canonical".into());
    }
    let mut pairs = (0..2)
        .filter_map(|slot| decode_pair(bytes, slot).ok())
        .collect::<Vec<_>>();
    if pairs.is_empty() {
        return Err("no committed checkpoint survived".into());
    }
    pairs.sort_by_key(|v| v.sequence);
    if pairs.len() == 2 {
        let old = &pairs[0];
        let new = &pairs[1];
        let mount = old.epoch.checked_add(1) == Some(new.epoch)
            && old.version == new.version
            && old.text == new.text;
        let save = old.epoch == new.epoch && old.version.checked_add(1) == Some(new.version);
        if old.sequence.checked_add(1) != Some(new.sequence) || !(mount || save) {
            return Err("valid checkpoint pairs disagree on legal progression".into());
        }
    }
    Ok(pairs)
}

fn verify_guards(bytes: &[u8], before: &[u8]) -> Result<(), String> {
    if bytes.len() != media_bytes
        || before.len() != media_bytes
        || bytes[..extent_start] != before[..extent_start]
        || bytes[extent_end..] != before[extent_end..]
    {
        return Err("write escaped the exact five-sector grant".into());
    }
    Ok(())
}

fn verify_cut_media(
    bytes: &[u8],
    original: &[u8],
    case: u64,
    challenge: &[u8],
) -> Result<checkpoint, String> {
    if !(1..=7).contains(&case) || !valid_challenge(challenge) {
        return Err("invalid independent cut expectation".into());
    }
    verify_guards(bytes, original)?;
    let pairs = decode_media(bytes)?;
    // The mounted empty A in slot1 is never the save target. Require its exact
    // canonical semantics even when slot0 is incomplete or contains new B.
    let old = checkpoint {
        sequence: 2,
        epoch: 1,
        version: 1,
        text: vec![],
    };
    if decode_pair(bytes, 1)? != old {
        return Err("last durable A was changed".into());
    }
    let initial = checkpoint {
        sequence: 1,
        epoch: 0,
        version: 1,
        text: vec![],
    };
    let new = checkpoint {
        sequence: 3,
        epoch: 1,
        version: 2,
        text: challenge.to_vec(),
    };
    if pairs
        .iter()
        .any(|v| *v != initial && *v != old && *v != new)
    {
        return Err("disk contains a checkpoint outside independently known A/B".into());
    }
    let latest = pairs.last().unwrap();
    let permitted = match case {
        1..=3 => latest == &old,
        4 | 7 => latest == &old || latest == &new,
        5 | 6 => latest == &new,
        _ => false,
    };
    if !permitted {
        return Err("recovery contradicts the selected flush boundary".into());
    }
    if case == 1 && pairs != [initial, old] {
        return Err("before-save cut mutated the inactive checkpoint".into());
    }
    Ok(latest.clone())
}

fn verify_cold_media(bytes: &[u8], before: &[u8], chosen: &checkpoint) -> Result<(), String> {
    verify_guards(bytes, before)?;
    if bytes[extent_start..extent_start + sector_bytes]
        != before[extent_start..extent_start + sector_bytes]
    {
        return Err("cold mount changed immutable header".into());
    }
    let pairs = decode_media(bytes)?;
    let expected = checkpoint {
        sequence: chosen.sequence.checked_add(1).ok_or("sequence overflow")?,
        epoch: 2,
        version: chosen.version,
        text: chosen.text.clone(),
    };
    if pairs != [chosen.clone(), expected] {
        return Err("cold mount did not preserve selected contents and advance only epoch".into());
    }
    let start = extent_start + (1 + ((chosen.sequence - 1) % 2) as usize * 2) * sector_bytes;
    if bytes[start..start + 2 * sector_bytes] != before[start..start + 2 * sector_bytes] {
        return Err("cold mount modified its active committed pair".into());
    }
    Ok(())
}

fn surface_reference(
    phase: u64,
    case: u64,
    challenge: &[u8],
    version: u64,
) -> Result<(Vec<u64>, [u8; frame_byte_count]), String> {
    let newer = newer_text(challenge)?;
    let recovered = if version == 1 { &b""[..] } else { challenge };
    let mut hashes = Vec::new();
    let mut pixels = [0; frame_byte_count];
    let count = if phase == 2 {
        1
    } else if case == 6 {
        4
    } else {
        3
    };
    for i in 0..count {
        let text = if phase == 2 {
            recovered
        } else {
            match i {
                0 => b"",
                1 => challenge,
                _ => &newer,
            }
        };
        let caret = if phase == 2 || i == 0 { 0 } else { text.len() };
        let mut state =
            loaded_editor_state::from_text(text).map_err(|e| format!("reference text: {e:?}"))?;
        state
            .set_selection(caret, caret)
            .map_err(|e| format!("reference selection: {e:?}"))?;
        render_loaded(&state, &mut pixels);
        let surface = application_surface_v1::try_new(
            application_surface_state {
                request_sequence: i as u64 + 1,
                document_revision: if phase == 2 || i == 0 {
                    1
                } else if i == 1 {
                    2
                } else {
                    4
                },
                text,
                selection_anchor: caret as u8,
                selection_head: caret as u8,
                dirty: phase == 0 && i != 0,
                theme: application_theme::dark,
                undo_depth: u8::from(phase == 0 && i != 0),
            },
            &pixels,
        )
        .map_err(|e| format!("reference surface: {e:?}"))?;
        let mut bytes = [0; application_surface_v1_size];
        surface
            .encode_into(&mut bytes)
            .map_err(|e| format!("reference encoding: {e:?}"))?;
        hashes.push(hash(&bytes));
    }
    Ok((hashes, pixels))
}

fn packet_reference(
    phase: u64,
    case: u64,
    challenge: &[u8],
    version: u64,
) -> Result<[Vec<u64>; 4], String> {
    let d = if phase == 0 { 1u64 } else { 2 };
    let previous = if d == 1 { 2 } else { 1 };
    let initial = if phase == 0 || version == 1 {
        &b""[..]
    } else {
        challenge
    };
    let mut ready = [0; 128];
    ready[..8].copy_from_slice(&[b'A', b'N', b'D', b'M', 1, 0, 128, 0]);
    ready[8..16].copy_from_slice(&d.to_le_bytes());
    ready[16..24].copy_from_slice(&version.to_le_bytes());
    ready[24..32].copy_from_slice(&(d + version).to_le_bytes());
    let mut requests = vec![
        wire(1, 0, [d, 0, 1, 0, 0, 0], b""),
        wire(3, 0, [previous, 1, 2, 0, 0, 0], b""),
        wire(3, 0, [d, 2, 3, 0, 0, 0], b""),
        wire(5, 0, [d, 1, 99, version + 1, 0, 1], b"invalid"),
    ];
    let mut replies = vec![
        ready,
        wire(2, 0, [d, 1, 1, 0, version, 0], initial),
        wire(9, 2, [previous, 1, 2, 0, 0, 0], b""),
        wire(9, 4, [d, 2, 3, 0, 0, 0], b""),
        wire(9, 6, [d, 1, 99, version + 1, 0, 1], b"invalid"),
    ];
    if phase == 0 {
        requests.push(wire(5, 0, [d, 1, 1, 1, 0, 2], challenge));
        if case == 6 {
            replies.push(wire(6, 0, [d, 1, 1, 1, 2, 2], challenge));
        }
    }
    if phase == 2 || case == 6 {
        requests.extend([
            wire(3, 0, [d, 1, 2, 0, 0, 0], b""),
            wire(7, 0, [d, 1, 3, 0, 0, 0], b""),
        ]);
        replies.extend([
            wire(
                4,
                0,
                [d, 1, 2, 0, if phase == 0 { 2 } else { version }, 0],
                if phase == 0 { challenge } else { initial },
            ),
            wire(8, 0, [d, 1, 3, 0, 0, 0], b""),
        ]);
    }
    Ok([
        surface_reference(phase, case, challenge, version)?.0,
        Vec::new(),
        requests.iter().map(|v| hash(v)).collect(),
        replies.iter().map(|v| hash(v)).collect(),
    ])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct memory_evidence {
    free: u64,
    unavailable: u64,
    total: u64,
}

fn phase_numbers(phase: u64, case: u64) -> Vec<(&'static str, u64)> {
    // Derived from the reviewed cooperative client schedule before any VM run.
    // Actual timer observations remain bounded separately and never determine
    // expected calls, copied bytes, channel operations or device commands.
    let fault = phase == 0 && case != 6;
    let (calls, switches, copies) = if phase == 2 {
        (143, 16, 52)
    } else if case == 6 {
        (192, 24, 62)
    } else if case == 7 {
        (156, 17, 50)
    } else {
        (148 + case, 17, 50)
    };
    vec![
        ("phase", phase),
        ("case", case),
        ("starts", 2),
        ("zeroed_bytes", 65536),
        ("calls", calls),
        ("switches", switches),
        ("arena_copies", copies),
        ("denials", if phase == 2 { 5 } else { 4 }),
        ("bad_copies", 4),
        ("invalid", 9),
        ("rows", 24),
        ("exits", if fault { 1 } else { 2 }),
        ("faults", u64::from(fault)),
        ("closed", if fault { 2 } else { 0 }),
        ("discarded", u64::from(fault)),
        (
            "completed_operations",
            if phase == 2 {
                0
            } else if case == 7 {
                3
            } else if case == 6 {
                4
            } else {
                case - 1
            },
        ),
        ("io_errors", if phase == 0 && case == 7 { 4 } else { 0 }),
    ]
}

fn channel_numbers(phase: u64, case: u64, channel: usize, count: u64) -> Vec<(&'static str, u64)> {
    vec![
        ("phase", phase),
        ("case", case),
        ("channel", channel as u64),
        ("sends", count),
        ("deliveries", count),
        (
            "acks",
            count - u64::from(phase == 0 && case != 6 && channel == 2),
        ),
    ]
}

fn device_numbers(phase: u64, case: u64) -> (Vec<(&'static str, u64)>, u64, u64) {
    let (reads, writes, flushes, lost) = if phase == 2 {
        (5, 2, 2, 0)
    } else {
        match case {
            1 => (10, 5, 4, 0),
            2 => (10, 6, 4, 0),
            3 => (10, 6, 5, 0),
            4 => (10, 7, 5, 0),
            5 | 6 => (10, 7, 6, 0),
            7 => (10, 7, 5, 1),
            _ => unreachable!(),
        }
    };
    let commands = 1 + reads + writes + flushes + lost;
    let numbers = vec![
        ("phase", phase),
        ("case", case),
        ("commands_issued", commands),
        ("commands_completed", commands - lost),
        ("sectors_read", reads),
        ("sectors_written", writes),
        ("flushes", flushes),
        (
            "port_writes",
            7 * (1 + reads + writes) + 3 * (flushes + lost) + 256 * writes,
        ),
    ];
    // A successfully completed FLUSH reads at least11 statuses. The injected
    // lost observation consumes only select's6 samples, then finish sees its
    // exhausted budget before the first completion sample. No later poisoned
    // request may add reads or writes.
    (
        numbers,
        530 + reads * 528 + writes * 272 + flushes * 11 + lost * 6,
        commands * 1_001_000,
    )
}

pub(super) fn verify_transcript_with_challenge(
    transcript: &str,
    phase: u64,
    case: u64,
    challenge: &[u8],
    recovered_version: u64,
) -> Result<(), String> {
    verify_records(transcript, phase, case, challenge, recovered_version).map(|_| ())
}

fn verify_records(
    transcript: &str,
    phase: u64,
    case: u64,
    challenge: &[u8],
    version: u64,
) -> Result<memory_evidence, String> {
    if !matches!(phase, 0 | 2)
        || !(1..=7).contains(&case)
        || !valid_challenge(challenge)
        || !matches!(version, 1 | 2)
        || (phase == 0 && version != 1)
        || (phase == 2 && matches!(case, 1..=3) && version != 1)
        || (phase == 2 && matches!(case, 5 | 6) && version != 2)
        || !transcript.ends_with('\n')
    {
        return Err(
            "failure transcript needs exact phase/case and independent recovery expectation".into(),
        );
    }
    let packets = packet_reference(phase, case, challenge, version)?;
    let expected_len = 28 + packets.iter().map(Vec::len).sum::<usize>();
    let lines = transcript
        .lines()
        .filter(|v| !v.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.len() != expected_len {
        return Err(format!(
            "failure transcript requires exactly{expected_len} ordered records"
        ));
    }
    native_application_smoke::verify_records(&lines[..21].join("\n"))?;
    let memory = json_parser::new(lines[4]).parse_record()?;
    let memory = memory_evidence {
        free: required_number(&memory, "kernel.memory.ready", "free_frames")?,
        unavailable: required_number(&memory, "kernel.memory.ready", "unavailable_frames")?,
        total: required_number(&memory, "kernel.memory.ready", "total_frames")?,
    };
    let record = json_parser::new(lines[21]).parse_record()?;
    verify_numbers(
        &record,
        phase_event,
        &phase_numbers(phase, case),
        &["timer_ticks", "resources_restored", "arm_seen"],
    )?;
    if required_number(&record, phase_event, "timer_ticks")? > 10_000 {
        return Err("failure timer bound exceeded".into());
    }
    require_bool(&record, phase_event, "resources_restored", true)?;
    require_bool(&record, phase_event, "arm_seen", phase == 0)?;
    let mut cursor = 22;
    for (channel, packets) in packets.into_iter().enumerate() {
        verify_numbers(
            &json_parser::new(lines[cursor]).parse_record()?,
            channel_event,
            &channel_numbers(phase, case, channel, packets.len() as u64),
            &[],
        )?;
        cursor += 1;
        for (index, hash) in packets.into_iter().enumerate() {
            verify_numbers(
                &json_parser::new(lines[cursor]).parse_record()?,
                packet_event,
                &[
                    ("phase", phase),
                    ("case", case),
                    ("channel", channel as u64),
                    ("sequence", index as u64 + 1),
                    ("hash", hash),
                ],
                &[],
            )?;
            cursor += 1;
        }
    }
    let device = json_parser::new(lines[cursor]).parse_record()?;
    cursor += 1;
    let (numbers, minimum, maximum) = device_numbers(phase, case);
    verify_numbers(&device, device_event, &numbers, &["port_reads", "poisoned"])?;
    let reads = required_number(&device, device_event, "port_reads")?;
    if reads < minimum || reads > maximum {
        return Err("failure port reads outside bounded mechanism".into());
    }
    require_bool(&device, device_event, "poisoned", phase == 0 && case == 7)?;
    let passed = json_parser::new(lines[cursor]).parse_record()?;
    verify_numbers(
        &passed,
        passed_event,
        &[("case", case), ("phases", 1), ("zeroed_bytes", 65536)],
        &[
            "resources_restored",
            "authority_revoked",
            "quarantine_denied",
            "store_scope",
        ],
    )?;
    require_bool(&passed, passed_event, "resources_restored", true)?;
    require_bool(&passed, passed_event, "authority_revoked", true)?;
    require_bool(
        &passed,
        passed_event,
        "quarantine_denied",
        phase == 0 && case == 7,
    )?;
    require_string(&passed, passed_event, "store_scope", "dedicated_disk")?;
    if cursor + 1 != lines.len() {
        return Err("failure trailing records".into());
    }
    Ok(memory)
}

pub(super) fn smoke() -> Result<(), String> {
    let image = build()?;
    let original_hash = sha256(&image.path)?;
    let result = smoke_built(&image);
    if sha256(&image.path)? != original_hash {
        return Err("failure proof mutated its original boot image".into());
    }
    result
}

fn smoke_built(image: &built_image) -> Result<(), String> {
    let root = repository_root()?;
    let base = root.join("build/native-durable-failure");
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = base.join(format!("smoke-{now}-{}", std::process::id()));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let boot_hash = sha256(&image.path)?;
    let boot = directory.join("boot.img");
    let mut copy = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&boot)
        .map_err(|e| e.to_string())?;
    std::io::copy(
        &mut File::open(&image.path).map_err(|e| e.to_string())?,
        &mut copy,
    )
    .map_err(|e| e.to_string())?;
    copy.sync_all().map_err(|e| e.to_string())?;
    drop(copy);
    if sha256(&boot)? != boot_hash {
        return Err("disposable boot image differs".into());
    }
    let original = fresh_media();
    let mut results = Vec::new();
    let mut memories = Vec::new();
    for case in 1..=7 {
        let mut entropy = [0; 8];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut entropy))
            .map_err(|e| format!("independent challenge entropy: {e}"))?;
        let challenge = format!(
            "cut{case}-{}",
            entropy
                .iter()
                .map(|v| format!("{v:02x}"))
                .collect::<String>()
        );
        let case_directory = directory.join(format!("case-{case}"));
        fs::create_dir(&case_directory).map_err(|e| e.to_string())?;
        let input = case_directory.join("initial.fwcfg");
        let cold_input = case_directory.join("cold.fwcfg");
        write_new(&input, &fixture(0, case, challenge.as_bytes())?)?;
        write_new(&cold_input, &fixture(2, case, b"")?)?;
        write_new(&case_directory.join("challenge.txt"), challenge.as_bytes())?;
        for memory in ["96M", "320M"] {
            let disk = case_directory.join(format!("initial-{memory}.raw"));
            write_new(&disk, &original)?;
            let (status, transcript) = run_boot(
                &boot,
                &disk,
                &input,
                memory,
                &case_directory.join(format!("initial-{memory}.log")),
            )?;
            if status.code() != Some(33) {
                return Err(format!("case{case} initial did not exit33: {status}"));
            }
            let first = verify_records(&transcript, 0, case, challenge.as_bytes(), 1)?;
            let cut = bounded_file(&disk, media_bytes)?;
            let chosen = verify_cut_media(&cut, &original, case, challenge.as_bytes())?;
            let snapshot = case_directory.join(format!("before-cold-{memory}.raw"));
            write_new(&snapshot, &cut)?;
            let cut_hash = sha256(&snapshot)?;
            // Cold recovery writes only a new, exact copy. The pre-mount image
            // remains immutable evidence of the flush boundary's outcome.
            let cold_disk = case_directory.join(format!("cold-{memory}.raw"));
            write_new(&cold_disk, &cut)?;
            let (status, transcript) = run_boot(
                &boot,
                &cold_disk,
                &cold_input,
                memory,
                &case_directory.join(format!("cold-{memory}.log")),
            )?;
            if status.code() != Some(33) {
                return Err(format!("case{case} cold did not exit33: {status}"));
            }
            let second =
                verify_records(&transcript, 2, case, challenge.as_bytes(), chosen.version)?;
            if first != second {
                return Err("same-size exact-image memory accounting changed".into());
            }
            verify_cold_media(&bounded_file(&cold_disk, media_bytes)?, &cut, &chosen)?;
            if sha256(&snapshot)? != cut_hash {
                return Err("pre-cold evidence was mutated".into());
            }
            if case == 1 {
                memories.push(first);
            }
            if memory == "96M" {
                capture(
                    &boot,
                    &input,
                    &case_directory,
                    &original,
                    0,
                    case,
                    challenge.as_bytes(),
                    1,
                )?;
                capture(
                    &boot,
                    &cold_input,
                    &case_directory,
                    &cut,
                    2,
                    case,
                    challenge.as_bytes(),
                    chosen.version,
                )?;
            }
            results.push(format!("{{\"case\":{case},\"memory\":{memory:?},\"challenge\":{challenge:?},\"recovered_version\":{},\"before_cold_sha256\":{cut_hash:?},\"after_cold_sha256\":{:?}}}",
                chosen.version, sha256(&cold_disk)?));
        }
    }
    if memories.len() != 2
        || memories[0].total != memories[1].total
        || memories[1].free.checked_sub(memories[0].free) != Some(57344)
        || memories[0].unavailable.checked_sub(memories[1].unavailable) != Some(57344)
    {
        return Err("durable failure boot memory delta differs from224MiB".into());
    }
    if sha256(&boot)? != boot_hash {
        return Err("failure proof changed disposable boot copy".into());
    }
    let receipt = format!(
        "{{\"schema\":1,\"image_sha256\":{boot_hash:?},\"evidence_directory\":{:?},\"successful_debug_exit_boots\":28,\"capture_host_quit_boots\":14,\"data_disk_bytes\":{media_bytes},\"extent_lba\":8,\"extent_sectors\":5,\"cache\":\"writeback\",\"flush_enabled\":true,\"guards_unchanged\":true,\"cold_answer_supplied\":false,\"power_loss_claim\":false,\"results\":[{}]}}\n",
        directory.to_string_lossy(),
        results.join(",")
    );
    write_new(&directory.join("receipt.json"), receipt.as_bytes())?;
    fs::write(base.join("smoke-receipt.json"), receipt).map_err(|e| e.to_string())?;
    println!(
        "Native durable service-cut/lost-reply proof verified seven cases, two memory sizes, cold media and public pixels; retained {}",
        directory.display()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn capture(
    image: &Path,
    input: &Path,
    directory: &Path,
    initial_disk: &[u8],
    phase: u64,
    case: u64,
    challenge: &[u8],
    version: u64,
) -> Result<(), String> {
    let before = sha256(image)?;
    let result = capture_inner(
        image,
        input,
        directory,
        initial_disk,
        phase,
        case,
        challenge,
        version,
    );
    if sha256(image)? != before {
        return Err("failure capture changed boot image".into());
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn capture_inner(
    image: &Path,
    input: &Path,
    directory: &Path,
    initial_disk: &[u8],
    phase: u64,
    case: u64,
    challenge: &[u8],
    version: u64,
) -> Result<(), String> {
    use native_durable_smoke::{
        child_guard, command, diagnostic_limits, timeout, transcript_limit,
    };
    let disk = directory.join(format!("capture-{phase}.raw"));
    write_new(&disk, initial_disk)?;
    let log = directory.join(format!("capture-{phase}.log"));
    let err = directory.join(format!("capture-{phase}.stderr.log"));
    let frame = directory.join(format!("capture-{phase}.ppm"));
    let mut command = command(image, &disk, input, "96M")?;
    command
        .args(["-no-shutdown", "-serial"])
        .arg(format!("file:{}", log.display()))
        .args(["-monitor", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(&err).map_err(|e| e.to_string())?));
    let mut child = child_guard(command.spawn().map_err(|e| e.to_string())?);
    let deadline = Instant::now() + timeout;
    loop {
        diagnostic_limits(&[&log, &err])?;
        let transcript = read_bounded_optional_utf8_file(&log, transcript_limit)?;
        if verify_transcript_with_challenge(&transcript, phase, case, challenge, version).is_ok() {
            break;
        }
        capture_running(&mut child.0, deadline)?;
        thread::sleep(Duration::from_millis(10));
    }
    let path = frame
        .to_str()
        .ok_or("capture path not UTF8")?
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let monitor = child.0.stdin.as_mut().ok_or("capture monitor missing")?;
    writeln!(monitor, "screendump \"{path}\"")
        .and_then(|()| monitor.flush())
        .map_err(|e| e.to_string())?;
    let pixels = surface_reference(phase, case, challenge, version)?.1;
    loop {
        if let Ok(metadata) = fs::metadata(&frame) {
            if metadata.len() > ppm_size as u64 {
                return Err("failure capture exceeded byte bound".into());
            }
            if metadata.len() == ppm_size as u64 {
                native_application_smoke::verify_frame_pixels(
                    &bounded_file(&frame, ppm_size)?,
                    &pixels,
                )?;
                break;
            }
        }
        diagnostic_limits(&[&log, &err])?;
        capture_running(&mut child.0, deadline)?;
        thread::sleep(Duration::from_millis(10));
    }
    let monitor = child.0.stdin.as_mut().ok_or("capture monitor missing")?;
    writeln!(monitor, "quit")
        .and_then(|()| monitor.flush())
        .map_err(|e| e.to_string())?;
    loop {
        diagnostic_limits(&[&log, &err])?;
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            if status.code() != Some(0) {
                return Err(format!("failure capture host quit failed: {status}"));
            }
            break;
        }
        if Instant::now() >= deadline {
            return Err("failure capture quit deadline".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    verify_transcript_with_challenge(
        &read_bounded_optional_utf8_file(&log, transcript_limit)?,
        phase,
        case,
        challenge,
        version,
    )?;
    native_application_smoke::verify_frame_pixels(&bounded_file(&frame, ppm_size)?, &pixels)?;
    let after = bounded_file(&disk, media_bytes)?;
    if phase == 0 {
        verify_cut_media(&after, initial_disk, case, challenge)?;
    } else {
        let chosen = verify_cut_media(initial_disk, &fresh_media(), case, challenge)?;
        if chosen.version != version {
            return Err("capture expectation differs from precold media".into());
        }
        verify_cold_media(&after, initial_disk, &chosen)?;
    }
    write_new(&directory.join(format!("capture-{phase}-receipt.json")), b"{\"schema\":1,\"host_exit_status\":0,\"isa_debug_exit\":false,\"width\":1024,\"height\":768,\"verified_x\":32,\"verified_y\":32,\"verified_width\":384,\"verified_height\":24,\"verified_pixels\":9216}\n")
}

fn capture_running(child: &mut Child, deadline: Instant) -> Result<(), String> {
    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
        return Err(format!("failure capture exited prematurely: {status}"));
    }
    if Instant::now() >= deadline {
        return Err("failure capture exceeded30-second deadline".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const challenge: &[u8] = b"host retained attempted B";

    fn seal(bytes: &mut [u8]) {
        let checksum = crc32c(&bytes[..508]);
        bytes[508..512].copy_from_slice(&checksum.to_le_bytes());
    }
    fn sector(magic: &[u8; 8]) -> [u8; 512] {
        let mut bytes = [0; 512];
        bytes[..8].copy_from_slice(magic);
        bytes[8..12].copy_from_slice(&[1, 0, 0, 2]);
        bytes[16..24].copy_from_slice(&0xa110u64.to_le_bytes());
        bytes[24..32].copy_from_slice(&0xd001u64.to_le_bytes());
        bytes
    }
    fn write_pair(bytes: &mut [u8], epoch: u64, version: u64, text: &[u8]) {
        let sequence = epoch + version;
        let slot = (sequence - 1) % 2;
        let mut body = sector(b"ANDURBOD");
        for (offset, value) in [(32, slot), (40, sequence), (48, epoch), (56, version)] {
            body[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        body[64] = text.len() as u8;
        body[65..65 + text.len()].copy_from_slice(text);
        seal(&mut body);
        let mut commit = sector(b"ANDURCMT");
        commit[32..64].copy_from_slice(&body[32..64]);
        commit[64..68].copy_from_slice(&body[508..512]);
        seal(&mut commit);
        let start = extent_start + (1 + slot as usize * 2) * 512;
        bytes[start..start + 512].copy_from_slice(&body);
        bytes[start + 512..start + 1024].copy_from_slice(&commit);
    }
    fn mounted() -> Vec<u8> {
        let mut bytes = fresh_media();
        let mut header = sector(b"ANDURHDR");
        header[12..16].copy_from_slice(&5u32.to_le_bytes());
        seal(&mut header);
        bytes[extent_start..extent_start + 512].copy_from_slice(&header);
        write_pair(&mut bytes, 0, 1, b"");
        write_pair(&mut bytes, 1, 1, b"");
        bytes
    }
    fn saved() -> Vec<u8> {
        let mut bytes = mounted();
        write_pair(&mut bytes, 1, 2, challenge);
        bytes
    }

    #[test]
    fn exact_failure_fixture_never_encodes_a_cold_answer() {
        for case in 1..=7 {
            let input = fixture(0, case, challenge).unwrap();
            assert_eq!(&input[..8], &[b'A', b'N', b'D', b'I', 2, 0, 0, 0]);
            assert_eq!(u64_at(&input, 48), case);
            assert_eq!(&input[17..17 + challenge.len()], challenge);
            let cold = fixture(2, case, b"").unwrap();
            assert!(cold[16..48].iter().all(|v| *v == 0));
            assert!(fixture(2, case, challenge).is_err());
        }
        for case in [0, 8, u64::MAX] {
            assert!(fixture(0, case, challenge).is_err());
        }
        for phase in [1, 3, u64::MAX] {
            assert!(fixture(phase, 1, challenge).is_err());
        }
        for text in [&b""[..], &b"bad\n"[..], &[b'x'; 32][..]] {
            assert!(fixture(0, 1, text).is_err());
        }
        assert_eq!(newer_text(b"~value").unwrap(), b"!value");
        assert_eq!(newer_text(b"value").unwrap(), b"walue");
    }

    #[test]
    fn each_flush_boundary_requires_its_explicit_known_content() {
        let blank = fresh_media();
        let old = mounted();
        let new = saved();
        for case in 1..=7 {
            assert_eq!(
                verify_cut_media(&old, &blank, case, challenge).is_ok(),
                !matches!(case, 5 | 6)
            );
            assert_eq!(
                verify_cut_media(&new, &blank, case, challenge).is_ok(),
                case >= 4
            );
        }
        let mut incomplete = old.clone();
        incomplete[extent_start + 512..extent_start + 1024]
            .copy_from_slice(&new[extent_start + 512..extent_start + 1024]);
        for case in [2, 3, 4, 7] {
            let chosen = verify_cut_media(&incomplete, &blank, case, challenge).unwrap();
            assert_eq!(
                (chosen.epoch, chosen.version, chosen.text),
                (1, 1, Vec::new())
            );
        }
        for case in [1, 5, 6] {
            assert!(verify_cut_media(&incomplete, &blank, case, challenge).is_err());
        }
        for value in [newer_text(challenge).unwrap(), b"not attempted".to_vec()] {
            let mut wrong = old.clone();
            write_pair(&mut wrong, 1, 2, &value);
            for case in 1..=7 {
                assert!(verify_cut_media(&wrong, &blank, case, challenge).is_err());
            }
        }
        // Every possible torn inactive-body prefix is classified only as A or
        // the independently known B; no arbitrary checksummed payload passes.
        for prefix in 0..=512 {
            let mut partial = old.clone();
            partial[extent_start + 512..extent_start + 512 + prefix]
                .copy_from_slice(&new[extent_start + 512..extent_start + 512 + prefix]);
            assert_eq!(
                verify_cut_media(&partial, &blank, 4, challenge)
                    .unwrap()
                    .version,
                1
            );
            let mut partial_commit = incomplete.clone();
            partial_commit[extent_start + 1024..extent_start + 1024 + prefix]
                .copy_from_slice(&new[extent_start + 1024..extent_start + 1024 + prefix]);
            let chosen = verify_cut_media(&partial_commit, &blank, 4, challenge).unwrap();
            assert!(chosen.version == 1 || (chosen.version == 2 && chosen.text == challenge));
        }
    }

    #[test]
    fn immutable_header_active_pair_and_external_guards_cannot_be_forged() {
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
        let before = fresh_media();
        let good = saved();
        for offset in (extent_start..extent_start + 512)
            .chain(extent_start + 1536..extent_end)
            .chain([0, extent_start - 1, extent_end, media_bytes - 1])
        {
            let mut changed = good.clone();
            changed[offset] ^= 1;
            assert!(
                verify_cut_media(&changed, &before, 5, challenge).is_err(),
                "accepted offset{offset}"
            );
        }
        for (offset, value) in [(12, 6u64), (16, 0xa111), (24, 0xd002), (32, 1)] {
            let mut changed = good.clone();
            let header = &mut changed[extent_start..extent_start + 512];
            header[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            seal(header);
            assert!(verify_cut_media(&changed, &before, 5, challenge).is_err());
        }
        let mut future = mounted();
        write_pair(&mut future, 2, 2, challenge);
        assert!(verify_cut_media(&future, &before, 4, challenge).is_err());
        let mut no_active = saved();
        no_active[extent_start + 1536..extent_end].fill(0);
        assert!(verify_cut_media(&no_active, &before, 5, challenge).is_err());
        for length in [0, extent_start, extent_end, media_bytes - 1] {
            assert!(decode_media(&good[..length]).is_err());
        }
    }

    #[test]
    fn cold_mount_requires_exact_selected_bytes_version_and_fresh_epoch() {
        for before in [mounted(), saved()] {
            let chosen = verify_cut_media(&before, &fresh_media(), 4, challenge).unwrap();
            let mut cold = before.clone();
            write_pair(&mut cold, 2, chosen.version, &chosen.text);
            verify_cold_media(&cold, &before, &chosen).unwrap();
            assert!(verify_cold_media(&before, &before, &chosen).is_err());
            for (epoch, version, text) in [
                (3, chosen.version, chosen.text.clone()),
                (2, chosen.version + 1, chosen.text.clone()),
                (2, chosen.version, newer_text(challenge).unwrap()),
            ] {
                let mut wrong = before.clone();
                write_pair(&mut wrong, epoch, version, &text);
                assert!(verify_cold_media(&wrong, &before, &chosen).is_err());
            }
        }
    }

    fn event(name: &str, numbers: &[(&str, u64)], extra: &str) -> String {
        let mut out = format!(r#"{{"schema":1,"event":"{name}","level":"info""#);
        for (key, value) in numbers {
            out.push_str(&format!(",{key:?}:{value}"));
        }
        out.push_str(extra);
        out.push('}');
        out
    }
    fn valid_transcript(phase: u64, case: u64, version: u64) -> String {
        let mut lines = vec![native_application_smoke::tests::valid_transcript()];
        lines.push(event(
            phase_event,
            &phase_numbers(phase, case),
            &format!(
                ",\"timer_ticks\":3,\"resources_restored\":true,\"arm_seen\":{}",
                phase == 0
            ),
        ));
        for (channel, packets) in packet_reference(phase, case, challenge, version)
            .unwrap()
            .into_iter()
            .enumerate()
        {
            lines.push(event(
                channel_event,
                &channel_numbers(phase, case, channel, packets.len() as u64),
                "",
            ));
            for (index, hash) in packets.into_iter().enumerate() {
                lines.push(event(
                    packet_event,
                    &[
                        ("phase", phase),
                        ("case", case),
                        ("channel", channel as u64),
                        ("sequence", index as u64 + 1),
                        ("hash", hash),
                    ],
                    "",
                ));
            }
        }
        let (mut numbers, minimum, _) = device_numbers(phase, case);
        numbers.push(("port_reads", minimum));
        lines.push(event(
            device_event,
            &numbers,
            &format!(",\"poisoned\":{}", phase == 0 && case == 7),
        ));
        lines.push(event(passed_event, &[("case", case), ("phases", 1), ("zeroed_bytes", 65536)],
            &format!(",\"resources_restored\":true,\"authority_revoked\":true,\"quarantine_denied\":{},\"store_scope\":\"dedicated_disk\"", phase == 0 && case == 7)));
        lines.join("\n") + "\n"
    }
    fn render_record(record: &structured_record) -> String {
        format!(
            "{{{}}}",
            record
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
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    fn expectations() -> Vec<(u64, u64, u64)> {
        let mut values = Vec::new();
        for case in 1..=7 {
            values.push((0, case, 1));
            for version in 1..=2 {
                if (case <= 3 && version == 1)
                    || (matches!(case, 4 | 7))
                    || (matches!(case, 5 | 6) && version == 2)
                {
                    values.push((2, case, version));
                }
            }
        }
        values
    }

    #[test]
    fn exact_cut_and_cold_transcripts_require_host_expectations_and_order() {
        for (phase, case, version) in expectations() {
            let text = valid_transcript(phase, case, version);
            verify_transcript_with_challenge(&text, phase, case, challenge, version).unwrap();
            assert_eq!(
                text.lines().count(),
                if phase == 2 {
                    42
                } else if case == 6 {
                    47
                } else {
                    41
                }
            );
            if phase == 0 || version == 2 {
                assert!(
                    verify_transcript_with_challenge(
                        &text,
                        phase,
                        case,
                        b"other attempted save",
                        version
                    )
                    .is_err()
                );
            }
            assert!(
                verify_transcript_with_challenge(
                    &text,
                    phase,
                    if case == 7 { 1 } else { case + 1 },
                    challenge,
                    version
                )
                .is_err()
            );
            assert!(
                verify_transcript_with_challenge(
                    &text,
                    if phase == 0 { 2 } else { 0 },
                    case,
                    challenge,
                    version
                )
                .is_err()
            );
            if phase == 2 {
                assert!(
                    verify_transcript_with_challenge(&text, phase, case, challenge, 3 - version)
                        .is_err()
                );
            }
            let lines = text.lines().collect::<Vec<_>>();
            for index in 21..lines.len() {
                let mut removed = lines.clone();
                removed.remove(index);
                assert!(
                    verify_transcript_with_challenge(
                        &(removed.join("\n") + "\n"),
                        phase,
                        case,
                        challenge,
                        version
                    )
                    .is_err()
                );
                let mut duplicated = lines.clone();
                duplicated.insert(index, lines[index]);
                assert!(
                    verify_transcript_with_challenge(
                        &(duplicated.join("\n") + "\n"),
                        phase,
                        case,
                        challenge,
                        version
                    )
                    .is_err()
                );
                if index + 1 < lines.len() {
                    let mut swapped = lines.clone();
                    swapped.swap(index, index + 1);
                    assert!(
                        verify_transcript_with_challenge(
                            &(swapped.join("\n") + "\n"),
                            phase,
                            case,
                            challenge,
                            version
                        )
                        .is_err()
                    );
                }
            }
            let final_start = text[..text.len() - 1].rfind('\n').unwrap() + 1;
            for length in final_start..text.len() {
                assert!(
                    verify_transcript_with_challenge(
                        &text[..length],
                        phase,
                        case,
                        challenge,
                        version
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn every_evidence_field_is_required_typed_exact_and_nonduplicated() {
        for (phase, case, version) in [(0, 1, 1), (0, 6, 1), (0, 7, 1), (2, 4, 1), (2, 4, 2)] {
            let text = valid_transcript(phase, case, version);
            let lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
            for index in 21..lines.len() {
                let record = json_parser::new(&lines[index]).parse_record().unwrap();
                for (field, value) in &record {
                    for replacement in [
                        None,
                        Some(json_scalar::null),
                        Some(match value {
                            json_scalar::number(_) => json_scalar::number(u64::MAX.to_string()),
                            json_scalar::string(value) => {
                                json_scalar::string(format!("{value}.forged"))
                            }
                            json_scalar::boolean(value) => json_scalar::boolean(!value),
                            json_scalar::null => unreachable!(),
                        }),
                    ] {
                        let mut changed = record.clone();
                        if let Some(value) = replacement {
                            changed.insert(field.clone(), value);
                        } else {
                            changed.remove(field);
                        }
                        let mut modified = lines.clone();
                        modified[index] = render_record(&changed);
                        assert!(
                            verify_transcript_with_challenge(
                                &(modified.join("\n") + "\n"),
                                phase,
                                case,
                                challenge,
                                version
                            )
                            .is_err(),
                            "accepted {phase}/{case}/{index}/{field}"
                        );
                    }
                    let mut duplicate = lines.clone();
                    duplicate[index].pop();
                    duplicate[index].push_str(&format!(",{field:?}:null}}"));
                    assert!(
                        verify_transcript_with_challenge(
                            &(duplicate.join("\n") + "\n"),
                            phase,
                            case,
                            challenge,
                            version
                        )
                        .is_err()
                    );
                }
                let mut changed = record.clone();
                changed.insert(
                    "extra".into(),
                    json_scalar::string("forged event substring".into()),
                );
                let mut modified = lines.clone();
                modified[index] = render_record(&changed);
                assert!(
                    verify_transcript_with_challenge(
                        &(modified.join("\n") + "\n"),
                        phase,
                        case,
                        challenge,
                        version
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn failure_selection_preserves_every_higher_profile_and_suppresses_old_path() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        let lower = [
            feature,
            "native_durable_test",
            "native_document_test",
            "native_runtime_test",
            "native_application_test",
            "application_profile_test",
            "owned_dispatcher_test",
            "owned_multi_region_test",
            "owned_source_test",
            "owned_tls_test",
            "dispatcher_test",
            "scheduler_test",
            "smoke_test",
        ];
        assert!(selected(&lower));
        assert!(!native_durable_smoke::selected(&lower));
        for higher in [
            "native_relaunch_test",
            "elf_relaunch_test",
            "elf_document_test",
            "elf_application_test",
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
            assert!(
                !selected(&[feature, higher]),
                "failure profile overrode {higher}"
            );
        }
    }
}
