//! Repeated native editor edits and three durable save acknowledgements.
//! Expected B/C/D bytes and packet states are derived independently after the
//! image is built. Recovery is never given those bytes or a predicted outcome.
//! The kernel uses the existing opaque durable profile and event schema.
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
    bounded_file, child_guard, command, common_sector, diagnostic_limits, fresh_media, hash,
    run_boot, sha256, timeout, transcript_limit, u32_at, u64_at, wire, write_new,
};

pub(super) const feature: &str = "native_repeated_editor_test";
const phase_event: &str = "kernel.native_durable_test.phase";
const channel_event: &str = "kernel.native_durable_test.channel";
const packet_event: &str = "kernel.native_durable_test.packet";
const device_event: &str = "kernel.native_durable_test.device";
const passed_event: &str = "kernel.native_durable_test.passed";
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
                .filter(|v| *v != feature)
                .chain(core::iter::once(native_durable_smoke::feature))
                .collect::<Vec<_>>(),
        )
}
pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[native_durable_smoke::feature, feature], None)?;
    println!(
        "Repeated native editor image inspected: {}",
        image.path.display()
    );
    Ok(image)
}
fn challenge_from_entropy(entropy: [u8; 8]) -> String {
    // Twenty-one cells fit completely at the recovered caret position zero.
    // Keep all eight random bytes while making the final changed glyph visible.
    format!(
        "edit-{}",
        entropy
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

fn valid_challenge(challenge: &[u8]) -> bool {
    (3..=31).contains(&challenge.len()) && challenge.iter().all(|v| (b' '..=b'~').contains(v))
}
fn fixture(phase: u64, challenge: &[u8]) -> Result<[u8; 48], String> {
    if !matches!(phase, 0 | 2)
        || (phase == 0 && !valid_challenge(challenge))
        || (phase == 2 && !challenge.is_empty())
    {
        return Err("invalid independent repeated-editor fixture".into());
    }
    let mut bytes = [0; 48];
    bytes[..4].copy_from_slice(b"ANDI");
    bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
    bytes[8..16].copy_from_slice(&phase.to_le_bytes());
    bytes[16] = challenge.len() as u8;
    bytes[17..17 + challenge.len()].copy_from_slice(challenge);
    Ok(bytes)
}
fn advance(byte: u8) -> u8 {
    if byte == b'~' { b'!' } else { byte + 1 }
}
fn text_versions(challenge: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    if !valid_challenge(challenge) {
        return Err("invalid independent repeated save text".into());
    }
    let mut c = challenge.to_vec();
    c[0] = advance(c[0]);
    let mut d = challenge.to_vec();
    let last = d.len() - 1;
    d[last] = advance(d[last]);
    Ok((c, d))
}

#[derive(Debug, Eq, PartialEq)]
struct media_checkpoint {
    sequence: u64,
    epoch: u64,
    version: u64,
    text: Vec<u8>,
}

fn decode_media(bytes: &[u8]) -> Result<Vec<media_checkpoint>, String> {
    if bytes.len() != media_bytes {
        return Err("dedicated disk length changed".into());
    }
    let header = &bytes[extent_start..extent_start + sector_bytes];
    common_sector(header, b"ANDURHDR")?;
    if u32_at(header, 12) != 5 || header[32..508].iter().any(|b| *b != 0) {
        return Err("durable media header is not canonical".into());
    }
    let mut values = Vec::new();
    // The completed happy path must have BOTH valid adjacent checkpoints.
    // This deliberately does not emulate guest fallback after a torn write.
    for slot in 0..2 {
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
            return Err("durable checkpoint is not canonical or not committed".into());
        }
        values.push(media_checkpoint {
            sequence,
            epoch,
            version,
            text: body[65..65 + length].to_vec(),
        });
    }
    values.sort_by_key(|v| v.sequence);
    let older = &values[0];
    let newer = &values[1];
    let mount = older.epoch.checked_add(1) == Some(newer.epoch)
        && older.version == newer.version
        && older.text == newer.text;
    let save = older.epoch == newer.epoch && older.version.checked_add(1) == Some(newer.version);
    if older.sequence.checked_add(1) != Some(newer.sequence) || !(mount || save) {
        return Err("durable media checkpoints are not adjacent legal transitions".into());
    }
    Ok(values)
}

fn verify_media(bytes: &[u8], before: &[u8], epoch: u64, challenge: &[u8]) -> Result<(), String> {
    if !matches!(epoch, 2 | 3) {
        return Err("invalid repeated-editor mount checkpoint".into());
    }
    if before.len() != media_bytes
        || bytes.len() != before.len()
        || bytes[..extent_start] != before[..extent_start]
        || bytes[extent_end..] != before[extent_end..]
    {
        return Err("disk write escaped the exact five-sector grant".into());
    }
    let text = text_versions(challenge)?.1;
    let observed = decode_media(bytes)?;
    let expected = [epoch - 1, epoch].map(|epoch| media_checkpoint {
        sequence: epoch + 4,
        epoch,
        version: 4,
        text: text.clone(),
    });
    if observed != expected {
        return Err(
            "repeated save media differs from independent finalD/version4/mount epochs".into(),
        );
    }
    if epoch == 3 {
        if bytes[extent_start..extent_start + sector_bytes]
            != before[extent_start..extent_start + sector_bytes]
        {
            return Err("cold mount changed immutable header".into());
        }
        // Pre-cold active sequence6 occupies slot1; the new mount writes slot0.
        if bytes[extent_start + 3 * sector_bytes..extent_end]
            != before[extent_start + 3 * sector_bytes..extent_end]
        {
            return Err("cold mount changed the previously committed active pair".into());
        }
    }
    Ok(())
}

type surface_checkpoint<'a> = (&'a [u8], u64, usize, usize, bool, u8);

fn surface_reference(
    phase: u64,
    challenge: &[u8],
) -> Result<(Vec<u64>, [u8; frame_byte_count]), String> {
    let (c, d) = text_versions(challenge)?;
    let n = challenge.len();
    // Text, document revision, anchor, head, dirty, retained undo. These are
    // manually specified reference states, not execution of the guest model.
    let states: Vec<surface_checkpoint<'_>> = if phase == 0 {
        vec![
            (b"", 1, 0, 0, false, 0),
            (&challenge[..1], 2, 1, 1, true, 1),
            (&challenge[..2], 3, 2, 2, true, 1),
            (challenge, 4, n, n, true, 1),
            (&c, 5, 1, 1, true, 1),
            (&c, 5, 1, 1, true, 1),
            (&c, 5, 1, 1, false, 1),
            (challenge, 6, 0, 1, true, 0),
            (&d, 7, n, n, true, 1),
            (&d, 7, n, n, false, 1),
        ]
    } else {
        vec![(&d, 1, 0, 0, false, 0)]
    };
    let mut pixels = [0; frame_byte_count];
    let mut hashes = Vec::new();
    for (i, (text, revision, anchor, head, dirty, undo_depth)) in states.into_iter().enumerate() {
        let mut state =
            loaded_editor_state::from_text(text).map_err(|e| format!("reference text: {e:?}"))?;
        state
            .set_selection(anchor, head)
            .map_err(|e| format!("reference selection: {e:?}"))?;
        render_loaded(&state, &mut pixels);
        let surface = application_surface_v1::try_new(
            application_surface_state {
                request_sequence: i as u64 + 1,
                document_revision: revision,
                text,
                selection_anchor: anchor as u8,
                selection_head: head as u8,
                dirty,
                theme: application_theme::dark,
                undo_depth,
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

fn packet_reference(phase: u64, challenge: &[u8]) -> Result<[Vec<u64>; 4], String> {
    if phase > 2 {
        return Err("invalid repeated reference phase".into());
    }
    let (c, final_text) = text_versions(challenge)?;
    let d = phase + 1;
    let version: u64 = if phase == 0 { 1 } else { 4 };
    let old = if d == 1 { 2 } else { d - 1 };
    let mut ready = [0; 128];
    ready[..8].copy_from_slice(&[b'A', b'N', b'D', b'M', 1, 0, 128, 0]);
    ready[8..16].copy_from_slice(&d.to_le_bytes());
    ready[16..24].copy_from_slice(&version.to_le_bytes());
    ready[24..32].copy_from_slice(&(d + version).to_le_bytes());
    let mut requests = vec![
        wire(1, 0, [d, 0, 1, 0, 0, 0], b""),
        wire(3, 0, [old, 1, 2, 0, 0, 0], b""),
        wire(3, 0, [d, 2, 3, 0, 0, 0], b""),
        wire(5, 0, [d, 1, 99, version + 1, 0, 1], b"invalid"),
    ];
    let mut replies = vec![
        ready,
        wire(
            2,
            0,
            [d, 1, 1, 0, version, 0],
            if phase == 0 { b"" } else { &final_text },
        ),
        wire(9, 2, [old, 1, 2, 0, 0, 0], b""),
        wire(9, 4, [d, 2, 3, 0, 0, 0], b""),
        wire(9, 6, [d, 1, 99, version + 1, 0, 1], b"invalid"),
    ];
    if phase == 0 {
        requests.extend([
            wire(5, 0, [d, 1, 1, 1, 0, 4], challenge),
            wire(3, 0, [d, 1, 2, 0, 0, 0], b""),
            wire(5, 0, [d, 1, 2, 2, 0, 5], &c),
            wire(5, 0, [d, 1, 3, 3, 0, 7], &final_text),
            wire(3, 0, [d, 1, 4, 0, 0, 0], b""),
            wire(7, 0, [d, 1, 5, 0, 0, 0], b""),
        ]);
        replies.extend([
            wire(6, 0, [d, 1, 1, 1, 2, 4], challenge),
            wire(4, 0, [d, 1, 2, 0, 2, 0], challenge),
            wire(6, 0, [d, 1, 2, 2, 3, 5], &c),
            wire(6, 0, [d, 1, 3, 3, 4, 7], &final_text),
            wire(4, 0, [d, 1, 4, 0, 4, 0], &final_text),
            wire(8, 0, [d, 1, 5, 0, 0, 0], b""),
        ]);
    } else {
        requests.extend([
            wire(3, 0, [d, 1, 2, 0, 0, 0], b""),
            wire(7, 0, [d, 1, 3, 0, 0, 0], b""),
        ]);
        replies.extend([
            wire(4, 0, [d, 1, 2, 0, 4, 0], &final_text),
            wire(8, 0, [d, 1, 3, 0, 0, 0], b""),
        ]);
    }
    Ok([
        surface_reference(phase, challenge)?.0,
        Vec::new(),
        requests.iter().map(|v| hash(v)).collect(),
        replies.iter().map(|v| hash(v)).collect(),
    ])
}

fn phase_numbers(phase: u64) -> Vec<(&'static str, u64)> {
    // Derived from the cooperative source call graph before the first VM.
    vec![
        ("phase", phase),
        ("starts", 2),
        ("zeroed_bytes", 65536),
        ("calls", if phase == 0 { 283 } else { 143 }),
        ("switches", if phase == 0 { 42 } else { 16 }),
        ("denials", if phase == 0 { 4 } else { 5 }),
        ("bad_copies", 4),
        ("invalid", 9),
        ("rows", 24),
        ("exits", 2),
        ("arena_copies", if phase == 0 { 86 } else { 52 }),
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct memory_evidence {
    free: u64,
    unavailable: u64,
    total: u64,
}

pub(super) fn verify_transcript_with_challenge(
    transcript: &str,
    first_phase: u64,
    challenge: &[u8],
) -> Result<(), String> {
    verify_records(transcript, first_phase, challenge).map(|_| ())
}

fn verify_records(
    transcript: &str,
    first_phase: u64,
    challenge: &[u8],
) -> Result<memory_evidence, String> {
    if !matches!(first_phase, 0 | 2) || !valid_challenge(challenge) || !transcript.ends_with('\n') {
        return Err(
            "repeated-editor transcript requires a complete record and independent challenge"
                .into(),
        );
    }
    let phases: &[u64] = if first_phase == 0 { &[0, 1] } else { &[2] };
    let lines = transcript
        .lines()
        .filter(|v| !v.trim().is_empty())
        .collect::<Vec<_>>();
    let expected_len = 23
        + phases
            .iter()
            .map(|phase| if *phase == 0 { 36 } else { 19 })
            .sum::<usize>();
    if lines.len() != expected_len {
        return Err(format!(
            "repeated-editor transcript requires exactly {expected_len} ordered records"
        ));
    }
    native_application_smoke::verify_records(&lines[..21].join("\n"))?;
    let memory = json_parser::new(lines[4]).parse_record()?;
    let memory = memory_evidence {
        free: required_number(&memory, "kernel.memory.ready", "free_frames")?,
        unavailable: required_number(&memory, "kernel.memory.ready", "unavailable_frames")?,
        total: required_number(&memory, "kernel.memory.ready", "total_frames")?,
    };
    let mut cursor = 21;
    for &phase in phases {
        let record = json_parser::new(lines[cursor]).parse_record()?;
        cursor += 1;
        verify_numbers(
            &record,
            phase_event,
            &phase_numbers(phase),
            &["timer_ticks", "resources_restored"],
        )?;
        if required_number(&record, phase_event, "timer_ticks")? > 10_000 {
            return Err("repeated-editor timer bound exceeded".into());
        }
        require_bool(&record, phase_event, "resources_restored", true)?;
        for (channel, packets) in packet_reference(phase, challenge)?.into_iter().enumerate() {
            let count = packets.len() as u64;
            verify_numbers(
                &json_parser::new(lines[cursor]).parse_record()?,
                channel_event,
                &[
                    ("phase", phase),
                    ("channel", channel as u64),
                    ("sends", count),
                    ("deliveries", count),
                    ("acks", count),
                ],
                &[],
            )?;
            cursor += 1;
            for (index, hash) in packets.into_iter().enumerate() {
                verify_numbers(
                    &json_parser::new(lines[cursor]).parse_record()?,
                    packet_event,
                    &[
                        ("phase", phase),
                        ("channel", channel as u64),
                        ("sequence", index as u64 + 1),
                        ("hash", hash),
                    ],
                    &[],
                )?;
                cursor += 1;
            }
        }
    }
    let device = json_parser::new(lines[cursor]).parse_record()?;
    cursor += 1;
    let (reads, writes, flushes) = if first_phase == 0 {
        (15, 13, 12)
    } else {
        (5, 2, 2)
    };
    let commands = 1 + reads + writes + flushes;
    // IDENTIFY/read/write each have seven byte OUTs; a flush has three.
    let port_writes = 7 * (1 + reads + writes) + 3 * flushes + 256 * writes;
    verify_numbers(
        &device,
        device_event,
        &[
            ("commands_issued", commands),
            ("commands_completed", commands),
            ("sectors_read", reads),
            ("sectors_written", writes),
            ("flushes", flushes),
            ("port_writes", port_writes),
        ],
        &["port_reads", "poisoned"],
    )?;
    let port_reads = required_number(&device, device_event, "port_reads")?;
    let minimum_reads = 530 + reads * 528 + writes * 272 + flushes * 11;
    if port_reads < minimum_reads || port_reads > commands * 1_001_000 {
        return Err("repeated-editor port-read evidence exceeds bounded mechanism".into());
    }
    require_bool(&device, device_event, "poisoned", false)?;
    verify_numbers(
        &json_parser::new(lines[cursor]).parse_record()?,
        passed_event,
        &[
            ("phases", phases.len() as u64),
            ("zeroed_bytes", phases.len() as u64 * 65536),
        ],
        &["resources_restored", "authority_revoked", "store_scope"],
    )?;
    let passed = json_parser::new(lines[cursor]).parse_record()?;
    require_bool(&passed, passed_event, "resources_restored", true)?;
    require_bool(&passed, passed_event, "authority_revoked", true)?;
    require_string(&passed, passed_event, "store_scope", "dedicated_disk")?;
    if cursor + 1 != lines.len() {
        return Err("repeated-editor trailing records".into());
    }
    Ok(memory)
}

pub(super) fn smoke() -> Result<(), String> {
    let image = build()?;
    let original_hash = sha256(&image.path)?;
    let result = smoke_built(&image);
    // Check the original even when a disposable VM or capture failed.
    if sha256(&image.path)? != original_hash {
        return Err("repeated editor proof changed the original build image".into());
    }
    result
}

fn smoke_built(image: &built_image) -> Result<(), String> {
    let base = repository_root()?.join("build/native-repeated-editor");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = base.join(format!("smoke-{now}-{}", std::process::id()));
    fs::create_dir(&directory).map_err(|e| format!("create fresh evidence directory: {e}"))?;
    // Generated after compilation. Neither the build nor cold startup receives
    // B, C or D as a recovery answer. B reaches only the initial editor.
    let mut entropy = [0; 8];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut entropy))
        .map_err(|e| format!("independent challenge entropy: {e}"))?;
    let challenge = challenge_from_entropy(entropy);
    let (newer, final_text) = text_versions(challenge.as_bytes())?;
    let input = directory.join("provision.fwcfg");
    let cold_input = directory.join("cold.fwcfg");
    write_new(&input, &fixture(0, challenge.as_bytes())?)?;
    write_new(&cold_input, &fixture(2, b"")?)?;
    write_new(&directory.join("challenge-b.txt"), challenge.as_bytes())?;
    write_new(&directory.join("newer-c.txt"), &newer)?;
    write_new(&directory.join("final-d.txt"), &final_text)?;
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
        return Err("disposable boot copy differs from inspected image".into());
    }
    let original = fresh_media();
    let mut memories = Vec::new();
    let mut media_receipts = Vec::new();
    for memory in ["96M", "320M"] {
        let initial_disk = directory.join(format!("initial-{memory}.raw"));
        write_new(&initial_disk, &original)?;
        let (status, transcript) = run_boot(
            &boot,
            &initial_disk,
            &input,
            memory,
            &directory.join(format!("initial-{memory}.log")),
        )?;
        require_debug_exit(status)?;
        let first = verify_records(&transcript, 0, challenge.as_bytes())?;
        let before_cold = bounded_file(&initial_disk, media_bytes)?;
        verify_media(&before_cold, &original, 2, challenge.as_bytes())?;
        let snapshot = directory.join(format!("before-cold-{memory}.raw"));
        write_new(&snapshot, &before_cold)?;
        let before_hash = sha256(&snapshot)?;
        let cold_disk = directory.join(format!("cold-{memory}.raw"));
        write_new(&cold_disk, &before_cold)?;
        let result = run_boot(
            &boot,
            &cold_disk,
            &cold_input,
            memory,
            &directory.join(format!("cold-{memory}.log")),
        );
        if sha256(&snapshot)? != before_hash || sha256(&initial_disk)? != before_hash {
            return Err("cold boot changed the retained pre-cold media".into());
        }
        let (status, transcript) = result?;
        require_debug_exit(status)?;
        let second = verify_records(&transcript, 2, challenge.as_bytes())?;
        if first != second {
            return Err("same-size exact-image boot memory accounting changed".into());
        }
        verify_media(
            &bounded_file(&cold_disk, media_bytes)?,
            &before_cold,
            3,
            challenge.as_bytes(),
        )?;
        let after_hash = sha256(&cold_disk)?;
        media_receipts.push(format!("{{\"memory\":{memory:?},\"before_cold_sha256\":{before_hash:?},\"after_cold_sha256\":{after_hash:?},\"version\":4,\"initial_epoch\":2,\"cold_epoch\":3}}"));
        memories.push(first);
        if memory == "96M" {
            capture(
                &boot,
                &input,
                &directory,
                &original,
                0,
                challenge.as_bytes(),
            )?;
            capture(
                &boot,
                &cold_input,
                &directory,
                &before_cold,
                2,
                challenge.as_bytes(),
            )?;
            if sha256(&snapshot)? != before_hash {
                return Err("capture changed the retained pre-cold media".into());
            }
        }
    }
    if memories[0].total != memories[1].total
        || memories[1].free.checked_sub(memories[0].free) != Some(57344)
        || memories[0].unavailable.checked_sub(memories[1].unavailable) != Some(57344)
    {
        return Err("repeated editor boot memory delta differs from224MiB".into());
    }
    if sha256(&boot)? != boot_hash || sha256(&image.path)? != boot_hash {
        return Err("repeated editor proof changed a boot image".into());
    }
    let newer = String::from_utf8(newer).map_err(|e| e.to_string())?;
    let final_text = String::from_utf8(final_text).map_err(|e| e.to_string())?;
    let receipt = format!(
        "{{\"schema\":1,\"image_sha256\":{boot_hash:?},\"challenge_b\":{challenge:?},\"newer_c\":{newer:?},\"final_d\":{final_text:?},\"evidence_directory\":{:?},\"successful_debug_exit_boots\":4,\"capture_host_quit_boots\":2,\"saves_per_initial_editor\":3,\"initial_surface_frames\":10,\"data_disk_bytes\":{media_bytes},\"extent_lba\":8,\"extent_sectors\":5,\"cache\":\"writeback\",\"flush_enabled\":true,\"guards_unchanged\":true,\"cold_answer_supplied\":false,\"media\":[{}]}}\n",
        directory.to_string_lossy(),
        media_receipts.join(",")
    );
    write_new(&directory.join("receipt.json"), receipt.as_bytes())?;
    fs::write(base.join("smoke-receipt.json"), receipt).map_err(|e| e.to_string())?;
    println!(
        "Repeated native editor verified three save cycles, two-size cold recovery, independent media and public pixels; retained {}",
        directory.display()
    );
    Ok(())
}

fn require_debug_exit(status: ExitStatus) -> Result<(), String> {
    if status.code() == Some(33) {
        Ok(())
    } else {
        Err(format!("repeated editor guest did not exit33: {status}"))
    }
}

fn capture(
    image: &Path,
    input: &Path,
    directory: &Path,
    initial_media: &[u8],
    first_phase: u64,
    challenge: &[u8],
) -> Result<(), String> {
    let before = sha256(image)?;
    let result = capture_inner(
        image,
        input,
        directory,
        initial_media,
        first_phase,
        challenge,
    );
    if sha256(image)? != before {
        return Err("repeated capture changed its disposable boot-image copy".into());
    }
    result
}

fn capture_inner(
    image: &Path,
    input: &Path,
    directory: &Path,
    initial_media: &[u8],
    first_phase: u64,
    challenge: &[u8],
) -> Result<(), String> {
    let stem = if first_phase == 0 {
        "capture-initial"
    } else {
        "capture-cold"
    };
    let disk = directory.join(format!("{stem}.raw"));
    write_new(&disk, initial_media)?;
    let log = directory.join(format!("{stem}.log"));
    let err = directory.join(format!("{stem}.stderr.log"));
    let frame = directory.join(format!("{stem}.ppm"));
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
        if verify_transcript_with_challenge(&transcript, first_phase, challenge).is_ok() {
            break;
        }
        capture_running(&mut child.0, deadline)?;
        thread::sleep(Duration::from_millis(10));
    }
    let path = frame.to_str().ok_or("capture path is not UTF-8")?;
    if path.chars().any(char::is_control) {
        return Err("capture path has a control character".into());
    }
    let path = path.replace('\\', "\\\\").replace('"', "\\\"");
    let monitor = child.0.stdin.as_mut().ok_or("capture monitor missing")?;
    writeln!(monitor, "screendump \"{path}\"")
        .and_then(|()| monitor.flush())
        .map_err(|e| e.to_string())?;
    let final_phase = if first_phase == 0 { 1 } else { 2 };
    let pixels = surface_reference(final_phase, challenge)?.1;
    loop {
        if let Ok(metadata) = fs::metadata(&frame) {
            if metadata.len() > ppm_size as u64 {
                return Err("repeated frame exceeded bound".into());
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
    let status = loop {
        diagnostic_limits(&[&log, &err])?;
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            return Err("repeated capture host quit timed out".into());
        }
        thread::sleep(Duration::from_millis(10));
    };
    if status.code() != Some(0) {
        return Err(format!("repeated capture host quit failed: {status}"));
    }
    verify_transcript_with_challenge(
        &read_bounded_optional_utf8_file(&log, transcript_limit)?,
        first_phase,
        challenge,
    )?;
    native_application_smoke::verify_frame_pixels(&bounded_file(&frame, ppm_size)?, &pixels)?;
    verify_media(
        &bounded_file(&disk, media_bytes)?,
        initial_media,
        if first_phase == 0 { 2 } else { 3 },
        challenge,
    )?;
    let receipt = format!(
        "{{\"schema\":1,\"first_phase\":{first_phase},\"host_exit_status\":0,\"isa_debug_exit\":false,\"width\":1024,\"height\":768,\"verified_x\":32,\"verified_y\":32,\"verified_width\":384,\"verified_height\":24,\"verified_pixels\":9216}}\n"
    );
    write_new(
        &directory.join(format!("{stem}-receipt.json")),
        receipt.as_bytes(),
    )
}

fn capture_running(child: &mut Child, deadline: Instant) -> Result<(), String> {
    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
        return Err(format!("repeated capture exited prematurely: {status}"));
    }
    if Instant::now() >= deadline {
        return Err("repeated capture exceeded30-second deadline".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    const challenge: &[u8] = b"host chose this text";

    fn event(name: &str, numbers: &[(&str, u64)], extra: &str) -> String {
        let mut out = format!(r#"{{"schema":1,"event":"{name}","level":"info""#);
        for (key, value) in numbers {
            out.push_str(&format!(",{key:?}:{value}"));
        }
        out.push_str(extra);
        out.push('}');
        out
    }

    fn valid_transcript(first_phase: u64) -> String {
        let mut lines = vec![native_application_smoke::tests::valid_transcript()];
        let phases: &[u64] = if first_phase == 0 { &[0, 1] } else { &[2] };
        for &phase in phases {
            lines.push(event(
                phase_event,
                &phase_numbers(phase),
                ",\"timer_ticks\":3,\"resources_restored\":true",
            ));
            for (channel, packets) in packet_reference(phase, challenge)
                .unwrap()
                .into_iter()
                .enumerate()
            {
                let count = packets.len() as u64;
                lines.push(event(
                    channel_event,
                    &[
                        ("phase", phase),
                        ("channel", channel as u64),
                        ("sends", count),
                        ("deliveries", count),
                        ("acks", count),
                    ],
                    "",
                ));
                for (index, hash) in packets.into_iter().enumerate() {
                    lines.push(event(
                        packet_event,
                        &[
                            ("phase", phase),
                            ("channel", channel as u64),
                            ("sequence", index as u64 + 1),
                            ("hash", hash),
                        ],
                        "",
                    ));
                }
            }
        }
        let (reads, writes, flushes) = if first_phase == 0 {
            (15, 13, 12)
        } else {
            (5, 2, 2)
        };
        let commands = 1 + reads + writes + flushes;
        lines.push(event(
            device_event,
            &[
                ("commands_issued", commands),
                ("commands_completed", commands),
                ("sectors_read", reads),
                ("sectors_written", writes),
                ("flushes", flushes),
                (
                    "port_writes",
                    7 * (1 + reads + writes) + 3 * flushes + 256 * writes,
                ),
                (
                    "port_reads",
                    530 + reads * 528 + writes * 272 + flushes * 11,
                ),
            ],
            ",\"poisoned\":false",
        ));
        lines.push(event(passed_event, &[("phases", phases.len() as u64), ("zeroed_bytes", phases.len() as u64 * 65536)], ",\"resources_restored\":true,\"authority_revoked\":true,\"store_scope\":\"dedicated_disk\""));
        format!("{}\n", lines.join("\n"))
    }

    fn render(record: &structured_record) -> String {
        format!(
            "{{{}}}",
            record
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
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    #[test]
    fn repeated_accepts_only_independent_challenge_and_exact_phase_sequences() {
        for phase in [0, 2] {
            let text = valid_transcript(phase);
            verify_transcript_with_challenge(&text, phase, challenge).unwrap();
            assert_eq!(text.lines().count(), if phase == 0 { 78 } else { 42 });
            for wrong in [&b""[..], &b"host chose different text"[..], &b"bad\n"[..]] {
                assert!(verify_transcript_with_challenge(&text, phase, wrong).is_err());
            }
            assert!(
                verify_transcript_with_challenge(&text, if phase == 0 { 2 } else { 0 }, challenge)
                    .is_err()
            );
            let lines = text.lines().collect::<Vec<_>>();
            for index in 21..lines.len() {
                let mut missing = lines.clone();
                missing.remove(index);
                assert!(
                    verify_transcript_with_challenge(
                        &(missing.join("\n") + "\n"),
                        phase,
                        challenge
                    )
                    .is_err()
                );
                let mut duplicate = lines.clone();
                duplicate.insert(index, lines[index]);
                assert!(
                    verify_transcript_with_challenge(
                        &(duplicate.join("\n") + "\n"),
                        phase,
                        challenge
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
                            challenge
                        )
                        .is_err()
                    );
                }
            }
            // Every partial final record (including a complete JSON record
            // missing its newline) must remain unready for capture.
            let final_start = text[..text.len() - 1].rfind('\n').unwrap() + 1;
            for length in final_start..text.len() {
                assert!(
                    verify_transcript_with_challenge(&text[..length], phase, challenge).is_err()
                );
            }
        }
    }

    #[test]
    fn repeated_rejects_every_missing_changed_duplicate_or_extra_native_field() {
        for phase in [0, 2] {
            let text = valid_transcript(phase);
            let lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
            for index in 21..lines.len() {
                let record = json_parser::new(&lines[index]).parse_record().unwrap();
                for (field, value) in &record {
                    for replacement in [
                        None,
                        Some(json_scalar::null),
                        Some(match value {
                            json_scalar::number(_) => json_scalar::number(u64::MAX.to_string()),
                            json_scalar::string(v) => json_scalar::string(format!("{v}.changed")),
                            json_scalar::boolean(v) => json_scalar::boolean(!v),
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
                        modified[index] = render(&changed);
                        assert!(
                            verify_transcript_with_challenge(
                                &(modified.join("\n") + "\n"),
                                phase,
                                challenge
                            )
                            .is_err(),
                            "accepted {index}:{field}"
                        );
                    }
                    let mut duplicated = lines.clone();
                    duplicated[index].pop();
                    duplicated[index].push_str(&format!(",{field:?}:null}}"));
                    assert!(
                        verify_transcript_with_challenge(
                            &(duplicated.join("\n") + "\n"),
                            phase,
                            challenge
                        )
                        .is_err()
                    );
                }
                let mut changed = record.clone();
                changed.insert(
                    "private_payload".into(),
                    json_scalar::string("forged".into()),
                );
                let mut modified = lines.clone();
                modified[index] = render(&changed);
                assert!(
                    verify_transcript_with_challenge(
                        &(modified.join("\n") + "\n"),
                        phase,
                        challenge
                    )
                    .is_err()
                );
            }
        }
    }

    fn seal(bytes: &mut [u8]) {
        let crc = crc32c(&bytes[..508]);
        bytes[508..512].copy_from_slice(&crc.to_le_bytes());
    }
    fn test_sector(magic: &[u8; 8]) -> [u8; 512] {
        let mut bytes = [0; 512];
        bytes[..8].copy_from_slice(magic);
        bytes[8..12].copy_from_slice(&[1, 0, 0, 2]);
        bytes[16..24].copy_from_slice(&0xa110u64.to_le_bytes());
        bytes[24..32].copy_from_slice(&0xd001u64.to_le_bytes());
        bytes
    }
    fn checkpoint(bytes: &mut [u8], epoch: u64, version: u64, text: &[u8]) {
        let sequence = epoch + version;
        let slot = (sequence - 1) % 2;
        let mut body = test_sector(b"ANDURBOD");
        let mut commit = test_sector(b"ANDURCMT");
        for out in [&mut body, &mut commit] {
            for (offset, value) in [(32, slot), (40, sequence), (48, epoch), (56, version)] {
                out[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            }
        }
        body[64] = text.len() as u8;
        body[65..65 + text.len()].copy_from_slice(text);
        seal(&mut body);
        commit[64..68].copy_from_slice(&body[508..512]);
        seal(&mut commit);
        let start = extent_start + (1 + slot as usize * 2) * 512;
        bytes[start..start + 512].copy_from_slice(&body);
        bytes[start + 512..start + 1024].copy_from_slice(&commit);
    }
    fn media(epoch: u64) -> Vec<u8> {
        let mut bytes = fresh_media();
        let mut header = test_sector(b"ANDURHDR");
        header[12..16].copy_from_slice(&5u32.to_le_bytes());
        seal(&mut header);
        bytes[extent_start..extent_start + 512].copy_from_slice(&header);
        let mut d = challenge.to_vec();
        *d.last_mut().unwrap() += 1;
        for epoch in [epoch - 1, epoch] {
            checkpoint(&mut bytes, epoch, 4, &d);
        }
        bytes
    }

    #[test]
    fn repeated_media_requires_three_saves_both_pairs_and_cold_preservation() {
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
        let initial = media(2);
        let cold = media(3);
        verify_media(&initial, &fresh_media(), 2, challenge).unwrap();
        verify_media(&cold, &initial, 3, challenge).unwrap();
        assert_eq!(
            decode_media(&initial)
                .unwrap()
                .iter()
                .map(|v| (v.sequence, v.epoch, v.version))
                .collect::<Vec<_>>(),
            [(5, 1, 4), (6, 2, 4)]
        );
        assert_eq!(
            decode_media(&cold)
                .unwrap()
                .iter()
                .map(|v| (v.sequence, v.epoch, v.version))
                .collect::<Vec<_>>(),
            [(6, 2, 4), (7, 3, 4)]
        );
        for epoch in [0, 1, 4, u64::MAX] {
            assert!(verify_media(&initial, &fresh_media(), epoch, challenge).is_err());
        }
        assert!(verify_media(&initial, &initial, 3, challenge).is_err());
        assert!(verify_media(&cold, &initial, 3, b"different answer").is_err());
        assert!(verify_media(&cold, &fresh_media(), 3, challenge).is_err());
        for wrong in [
            challenge.to_vec(),
            text_versions(challenge).unwrap().0,
            Vec::new(),
        ] {
            let mut bad = cold.clone();
            for epoch in [2, 3] {
                checkpoint(&mut bad, epoch, 4, &wrong);
            }
            // A perfectly valid different document still fails the independent
            // final-D expectation; this is not a disk antirollback claim.
            decode_media(&bad).unwrap();
            assert!(verify_media(&bad, &initial, 3, challenge).is_err());
        }
        let mut bad = cold.clone();
        let d = text_versions(challenge).unwrap().1;
        for epoch in [2, 3] {
            checkpoint(&mut bad, epoch, 2, &d);
        }
        decode_media(&bad).unwrap();
        assert!(verify_media(&bad, &initial, 3, challenge).is_err());
        for offset in [0, extent_start - 1, extent_end, media_bytes - 1] {
            let mut bad = cold.clone();
            bad[offset] ^= 1;
            assert!(verify_media(&bad, &initial, 3, challenge).is_err());
        }
        for offset in extent_start..extent_end {
            let mut bad = cold.clone();
            bad[offset] ^= 1;
            assert!(
                decode_media(&bad).is_err(),
                "accepted changed sector byte {offset}"
            );
        }
        assert!(decode_media(&cold[..cold.len() - 1]).is_err());
        let mut extra = cold.clone();
        extra.push(0);
        assert!(decode_media(&extra).is_err());
        let mut missing = cold.clone();
        missing[extent_start + 512..extent_start + 1536].fill(0);
        assert!(decode_media(&missing).is_err());
    }

    #[test]
    fn repeated_media_rejects_resealed_identity_padding_and_sequence_forgery() {
        let good = media(3);
        for sector in 0..5 {
            for offset in [0, 8, 12, 16, 24, 100] {
                let mut bad = good.clone();
                let start = extent_start + sector * 512;
                bad[start + offset] ^= 1;
                seal(&mut bad[start..start + 512]);
                assert!(
                    decode_media(&bad).is_err(),
                    "accepted canonical mutation {sector}:{offset}"
                );
            }
        }
        let mut bad = good.clone();
        // Consistent checksums and slot parity do not excuse a sequence gap.
        checkpoint(&mut bad, 6, 4, &text_versions(challenge).unwrap().1);
        assert!(decode_media(&bad).is_err());
        // Both records individually well formed, but a mount cannot change text.
        let mut bad = good.clone();
        checkpoint(&mut bad, 3, 4, challenge);
        assert!(decode_media(&bad).is_err());
    }

    #[test]
    fn repeated_reference_distinguishes_saved_b_newer_c_final_d_and_undo() {
        assert_eq!(
            text_versions(b"~x~").unwrap(),
            (b"!x~".to_vec(), b"~x!".to_vec())
        );
        let (c, d) = text_versions(challenge).unwrap();
        assert_ne!(c, d);
        assert_ne!(d, challenge);
        assert_eq!(&d[..d.len() - 1], &challenge[..challenge.len() - 1]);
        let packets = packet_reference(0, challenge).unwrap();
        assert_eq!(packets.each_ref().map(Vec::len), [10, 0, 10, 11]);
        assert_eq!(
            packets[2][4],
            hash(&wire(5, 0, [1, 1, 1, 1, 0, 4], challenge))
        );
        assert_eq!(
            packets[3][5],
            hash(&wire(6, 0, [1, 1, 1, 1, 2, 4], challenge))
        );
        assert_eq!(
            packets[3][6],
            hash(&wire(4, 0, [1, 1, 2, 0, 2, 0], challenge))
        );
        assert_eq!(packets[2][6], hash(&wire(5, 0, [1, 1, 2, 2, 0, 5], &c)));
        assert_eq!(packets[3][7], hash(&wire(6, 0, [1, 1, 2, 2, 3, 5], &c)));
        assert_eq!(packets[2][7], hash(&wire(5, 0, [1, 1, 3, 3, 0, 7], &d)));
        assert_eq!(packets[3][8], hash(&wire(6, 0, [1, 1, 3, 3, 4, 7], &d)));
        let expected = [
            (challenge, 6, 0, 1, true, 0),
            (&c[..], 5, 1, 1, true, 1),
            (&c[..], 5, 1, 1, false, 1),
            (&d[..], 7, d.len(), d.len(), false, 1),
        ];
        for (index, (text, revision, anchor, head, dirty, undo_depth)) in
            [7, 5, 6, 9].into_iter().zip(expected)
        {
            let mut editor = loaded_editor_state::from_text(text).unwrap();
            editor.set_selection(anchor, head).unwrap();
            let mut pixels = [0; frame_byte_count];
            render_loaded(&editor, &mut pixels);
            let mut bytes = [0; application_surface_v1_size];
            application_surface_v1::try_new(
                application_surface_state {
                    request_sequence: index as u64 + 1,
                    document_revision: revision,
                    text,
                    selection_anchor: anchor as u8,
                    selection_head: head as u8,
                    dirty,
                    theme: application_theme::dark,
                    undo_depth,
                },
                &pixels,
            )
            .unwrap()
            .encode_into(&mut bytes)
            .unwrap();
            assert_eq!(packets[0][index], hash(&bytes));
        }
        assert_eq!(
            surface_reference(1, challenge).unwrap().1,
            surface_reference(2, challenge).unwrap().1
        );
        for phase in [1, 2] {
            assert_eq!(
                packet_reference(phase, challenge)
                    .unwrap()
                    .each_ref()
                    .map(Vec::len),
                [1, 0, 6, 7]
            );
        }
        let text = valid_transcript(0);
        assert!(
            native_durable_smoke::verify_transcript_with_challenge(&text, 0, challenge).is_err()
        );
    }

    #[test]
    fn generated_challenge_preserves_entropy_and_exposes_the_final_edit_in_pixels() {
        for (entropy, hex) in [
            ([0; 8], "0000000000000000"),
            ([0xff; 8], "ffffffffffffffff"),
        ] {
            let text = challenge_from_entropy(entropy);
            assert_eq!(text, format!("edit-{hex}"));
            assert_eq!(text.len(), 21);
            assert!(valid_challenge(text.as_bytes()));
            let before = loaded_editor_state::from_text(text.as_bytes()).unwrap();
            let mut before_pixels = [0; frame_byte_count];
            render_loaded(&before, &mut before_pixels);
            let after_pixels = surface_reference(2, text.as_bytes()).unwrap().1;
            assert_ne!(before_pixels, after_pixels);
            // A genuine glyph change in the final visible cell, with all
            // preceding cells and the caret left exactly where they were.
            for row in 0..8 {
                let start = row * 128 * 3;
                assert_eq!(
                    &before_pixels[start..start + 120 * 3],
                    &after_pixels[start..start + 120 * 3]
                );
            }
            assert!((0..7).any(|row| {
                let start = (row * 128 + 120) * 3;
                before_pixels[start..start + 5 * 3] != after_pixels[start..start + 5 * 3]
            }));
        }
    }

    #[test]
    fn repeated_fixture_is_answer_free_and_guest_exit_is_independent() {
        let cold = fixture(2, b"").unwrap();
        assert_eq!(&cold[..8], b"ANDI\x01\0\0\0");
        assert_eq!(u64_at(&cold, 8), 2);
        assert!(cold[16..].iter().all(|v| *v == 0));
        assert!(fixture(2, challenge).is_err());
        for wrong in [&b""[..], &b"ab"[..], &b"bad\n"[..], &[b'a'; 32][..]] {
            assert!(fixture(0, wrong).is_err());
        }
        assert!(fixture(0, &[b'~'; 31]).is_ok());
        assert!(fixture(1, challenge).is_err());
        require_debug_exit(ExitStatus::from_raw(33 << 8)).unwrap();
        for code in [0, 1, 35] {
            assert!(require_debug_exit(ExitStatus::from_raw(code << 8)).is_err());
        }
        assert!(require_debug_exit(ExitStatus::from_raw(9)).is_err());
    }

    #[test]
    fn repeated_selection_preserves_failure_and_higher_profile_precedence() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        assert!(selected(&[
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
            "smoke_test"
        ]));
        for higher in [
            "native_durable_failure_test",
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
            assert!(!selected(&[feature, higher]), "repeated overrode {higher}");
        }
        assert!(!native_durable_smoke::selected(&[
            feature,
            native_durable_smoke::feature
        ]));
        assert!(native_durable_failure_smoke::selected(&[
            feature,
            native_durable_failure_smoke::feature
        ]));
    }
}
