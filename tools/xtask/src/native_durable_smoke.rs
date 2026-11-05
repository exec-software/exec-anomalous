//! ADR 0063's disposable-disk gate. The challenge is generated after building
//! the image and never recovered from guest output. Media checks below do not
//! call the guest durable codec/store. They certify this bounded happy path,
//! not arbitrary power-loss recovery or a general filesystem.
use super::*;
use anomalous_abi::application::{
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_editor_contract::{frame_byte_count, loaded_editor_state, render_loaded};
use native_document_smoke::verify_numbers;

pub(super) const feature: &str = "native_durable_test";
const phase_event: &str = "kernel.native_durable_test.phase";
const channel_event: &str = "kernel.native_durable_test.channel";
const packet_event: &str = "kernel.native_durable_test.packet";
const device_event: &str = "kernel.native_durable_test.device";
const passed_event: &str = "kernel.native_durable_test.passed";
pub(super) const timeout: Duration = Duration::from_secs(30);
pub(super) const transcript_limit: usize = 64 * 1024;
const media_bytes: usize = 1024 * 1024;
const sector_bytes: usize = 512;
const extent_start: usize = 8 * sector_bytes;
const extent_end: usize = extent_start + 5 * sector_bytes;
const ppm_size: usize = b"P6\n1024 768\n255\n".len() + 1024 * 768 * 3;

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|v| {
            matches!(
                *v,
                "native_durable_failure_test"
                    | "native_repeated_editor_test"
                    | "native_interactive_editor_test"
                    | "native_relaunch_test"
                    | "elf_relaunch_test"
                    | "elf_document_test"
                    | "elf_application_test"
                    | "linux_application_test"
                    | "veil_application_test"
                    | "shared_veil_test"
                    | "linux_tls_test"
                    | "linux_loader_entry_test"
            )
        })
        && native_document_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|v| *v != feature)
                .chain(core::iter::once(native_document_smoke::feature))
                .collect::<Vec<_>>(),
        )
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!("Native durable image inspected: {}", image.path.display());
    Ok(image)
}

pub(super) fn valid_challenge(challenge: &[u8]) -> bool {
    (1..=31).contains(&challenge.len()) && challenge.iter().all(|v| (b' '..=b'~').contains(v))
}

fn fixture(phase: u64, challenge: &[u8]) -> Result<[u8; 48], String> {
    if !matches!(phase, 0 | 2)
        || (phase == 0 && !valid_challenge(challenge))
        || (phase == 2 && !challenge.is_empty())
    {
        return Err("invalid independent durable launch fixture".into());
    }
    let mut bytes = [0; 48];
    bytes[..4].copy_from_slice(b"ANDI");
    bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
    bytes[8..16].copy_from_slice(&phase.to_le_bytes());
    bytes[16] = challenge.len() as u8;
    bytes[17..17 + challenge.len()].copy_from_slice(challenge);
    Ok(bytes)
}

pub(super) fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |value, byte| {
        (value ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

// Explicit expected records; no guest document_store/durable_store execution.
pub(super) fn wire(kind: u8, status: u8, tuple: [u64; 6], text: &[u8]) -> [u8; 128] {
    let mut bytes = [0; 128];
    bytes[..8].copy_from_slice(&[b'A', b'N', b'D', b'C', 1, 0, 128, 0]);
    bytes[8] = kind;
    bytes[9] = status;
    for (i, value) in tuple.into_iter().enumerate() {
        bytes[16 + 8 * i..24 + 8 * i].copy_from_slice(&value.to_le_bytes());
    }
    bytes[64] = text.len() as u8;
    bytes[65..65 + text.len()].copy_from_slice(text);
    bytes
}

fn packet_reference(phase: u64, challenge: &[u8]) -> Result<[Vec<u64>; 4], String> {
    if phase > 2 || !valid_challenge(challenge) {
        return Err("invalid durable reference input".into());
    }
    let d = phase + 1;
    let version: u64 = if phase == 0 { 1 } else { 2 };
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
            if phase == 0 { b"" } else { challenge },
        ),
        wire(9, 2, [old, 1, 2, 0, 0, 0], b""),
        wire(9, 4, [d, 2, 3, 0, 0, 0], b""),
        wire(9, 6, [d, 1, 99, version + 1, 0, 1], b"invalid"),
    ];
    if phase == 0 {
        requests.push(wire(5, 0, [d, 1, 1, 1, 0, 2], challenge));
        replies.push(wire(6, 0, [d, 1, 1, 1, 2, 2], challenge));
    }
    requests.extend([
        wire(3, 0, [d, 1, 2, 0, 0, 0], b""),
        wire(7, 0, [d, 1, 3, 0, 0, 0], b""),
    ]);
    replies.extend([
        wire(4, 0, [d, 1, 2, 0, 2, 0], challenge),
        wire(8, 0, [d, 1, 3, 0, 0, 0], b""),
    ]);
    Ok([
        surface_reference(phase, challenge)?.0,
        Vec::new(),
        requests.iter().map(|v| hash(v)).collect(),
        replies.iter().map(|v| hash(v)).collect(),
    ])
}

fn surface_reference(
    phase: u64,
    challenge: &[u8],
) -> Result<(Vec<u64>, [u8; frame_byte_count]), String> {
    let mut pixels = [0; frame_byte_count];
    let mut hashes = Vec::new();
    let count = if phase == 0 { 3 } else { 1 };
    for index in 0..count {
        let edited = phase == 0 && index != 0;
        let text = if phase == 0 && index == 0 {
            &b""[..]
        } else {
            challenge
        };
        let caret = if edited { text.len() } else { 0 };
        let mut state =
            loaded_editor_state::from_text(text).map_err(|e| format!("reference text: {e:?}"))?;
        state
            .set_selection(caret, caret)
            .map_err(|e| format!("reference caret: {e:?}"))?;
        render_loaded(&state, &mut pixels);
        let surface = application_surface_v1::try_new(
            application_surface_state {
                request_sequence: index as u64 + 1,
                document_revision: if edited { 2 } else { 1 },
                text,
                selection_anchor: caret as u8,
                selection_head: caret as u8,
                dirty: phase == 0 && index == 1,
                theme: application_theme::dark,
                undo_depth: u8::from(edited),
            },
            &pixels,
        )
        .map_err(|e| format!("reference surface: {e:?}"))?;
        let mut bytes = [0; application_surface_v1_size];
        surface
            .encode_into(&mut bytes)
            .map_err(|e| format!("reference encode: {e:?}"))?;
        hashes.push(hash(&bytes));
    }
    Ok((hashes, pixels))
}

fn phase_numbers(phase: u64) -> Vec<(&'static str, u64)> {
    // Exact gate counts are tied to the bounded client choreography, not PIT
    // timing. Tests mutate every field independently before VM qualification.
    vec![
        ("phase", phase),
        ("starts", 2),
        ("zeroed_bytes", 65536),
        ("calls", if phase == 0 { 184 } else { 143 }),
        ("switches", if phase == 0 { 22 } else { 16 }),
        ("denials", if phase == 0 { 4 } else { 5 }),
        ("bad_copies", 4),
        ("invalid", 9),
        ("rows", 24),
        ("exits", 2),
        ("arena_copies", if phase == 0 { 60 } else { 52 }),
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
            "durable transcript requires a complete record and independent challenge".into(),
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
            .map(|phase| if *phase == 0 { 23 } else { 19 })
            .sum::<usize>();
    if lines.len() != expected_len {
        return Err(format!(
            "durable transcript requires exactly {expected_len} ordered records"
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
            return Err("durable timer bound exceeded".into());
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
        (15, 9, 8)
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
        return Err("durable port-read evidence exceeds bounded mechanism".into());
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
        return Err("durable trailing records".into());
    }
    Ok(memory)
}

// Independent, noncryptographic Castagnoli CRC. This is error detection, not
// authentication, freshness, rollback prevention or a storage authority.
pub(super) fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0x82f6_3b78
            };
        }
    }
    !crc
}
pub(super) fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}
pub(super) fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

pub(super) fn common_sector(bytes: &[u8], magic: &[u8; 8]) -> Result<(), String> {
    if bytes.len() != sector_bytes
        || &bytes[..8] != magic
        || bytes[8..12] != [1, 0, 0, 2]
        || u64_at(bytes, 16) != 0xa110
        || u64_at(bytes, 24) != 0xd001
        || crc32c(&bytes[..508]) != u32_at(bytes, 508)
    {
        return Err("durable media header, identity or checksum mismatch".into());
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct media_checkpoint {
    sequence: u64,
    epoch: u64,
    version: u64,
    text: Vec<u8>,
}

fn decode_media(bytes: &[u8]) -> Result<media_checkpoint, String> {
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
    Ok(values.remove(1))
}

fn verify_media(bytes: &[u8], before: &[u8], epoch: u64, challenge: &[u8]) -> Result<(), String> {
    if before.len() != media_bytes
        || bytes.len() != before.len()
        || bytes[..extent_start] != before[..extent_start]
        || bytes[extent_end..] != before[extent_end..]
    {
        return Err("disk write escaped the exact five-sector grant".into());
    }
    let observed = decode_media(bytes)?;
    if observed
        != (media_checkpoint {
            sequence: epoch + 2,
            epoch,
            version: 2,
            text: challenge.to_vec(),
        })
    {
        return Err(
            "durable media differs from independently retained save challenge/epoch".into(),
        );
    }
    Ok(())
}

pub(super) fn fresh_media() -> Vec<u8> {
    let mut bytes = (0..media_bytes)
        .map(|i| ((i.wrapping_mul(37) ^ (i >> 9) ^ 0xa7) & 255) as u8)
        .collect::<Vec<_>>();
    bytes[extent_start..extent_end].fill(0);
    bytes
}

pub(super) struct child_guard(pub(super) Child);
impl Drop for child_guard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

pub(super) fn bounded_file(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err(format!("{} exceeded its byte bound", path.display()));
    }
    Ok(bytes)
}
pub(super) fn diagnostic_limits(paths: &[&Path]) -> Result<(), String> {
    for path in paths {
        match fs::metadata(path) {
            Ok(m) if m.len() > transcript_limit as u64 => {
                return Err(format!("{} exceeded transcript limit", path.display()));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
fn wait_exit(child: &mut Child, deadline: Instant, paths: &[&Path]) -> Result<ExitStatus, String> {
    loop {
        diagnostic_limits(paths)?;
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err("durable QEMU exceeded 30-second deadline".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn qemu_path(path: &Path) -> Result<&str, String> {
    let value = path.to_str().ok_or("QEMU path is not UTF-8")?;
    if value.contains(',') || value.chars().any(char::is_control) {
        return Err("QEMU path contains option separator or control byte".into());
    }
    Ok(value)
}

pub(super) fn command(
    image: &Path,
    disk: &Path,
    input: &Path,
    memory: &str,
) -> Result<Command, String> {
    let mut command = Command::new("qemu-system-x86_64");
    command.current_dir(repository_root()?)
        .args(["-machine", "q35", "-cpu", "Nehalem", "-m", memory])
        .arg("-drive").arg(format!("format=raw,file={}", qemu_path(image)?))
        .args(["-device", "isa-ide,id=proof_ide,iobase=0x170,iobase2=0x376,irq=15"])
        .arg("-drive").arg(format!("if=none,id=proof_data,file={},format=raw,cache=writeback,cache.no-flush=off,werror=report,rerror=report", qemu_path(disk)?))
        .args(["-device", "ide-hd,drive=proof_data,bus=proof_ide.0,unit=0,logical_block_size=512,physical_block_size=512"])
        .arg("-fw_cfg").arg(format!("name=opt/anomalous-durable,file={}", qemu_path(input)?))
        .args(["-no-reboot", "-display", "none"]);
    Ok(command)
}

pub(super) fn run_boot(
    image: &Path,
    disk: &Path,
    input: &Path,
    memory: &str,
    log: &Path,
) -> Result<(ExitStatus, String), String> {
    let before = sha256(image)?;
    // The inner child guard reaps on every path before this postcondition,
    // including timeouts and failed diagnostic reads.
    let result = run_boot_inner(image, disk, input, memory, log);
    if sha256(image)? != before {
        return Err("durable VM changed its disposable boot-image copy".into());
    }
    result
}

fn run_boot_inner(
    image: &Path,
    disk: &Path,
    input: &Path,
    memory: &str,
    log: &Path,
) -> Result<(ExitStatus, String), String> {
    let err = log.with_extension("stderr.log");
    let mut command = command(image, disk, input, memory)?;
    command
        .args([
            "-serial",
            "stdio",
            "-monitor",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(File::create(log).map_err(|e| e.to_string())?))
        .stderr(Stdio::from(File::create(&err).map_err(|e| e.to_string())?));
    let mut child = child_guard(
        command
            .spawn()
            .map_err(|e| format!("start disposable durable QEMU: {e}"))?,
    );
    let status = wait_exit(&mut child.0, Instant::now() + timeout, &[log, &err])?;
    let transcript = read_bounded_optional_utf8_file(log, transcript_limit)?;
    Ok((status, transcript))
}

fn verify_failed_media_boot(status: ExitStatus, transcript: &str) -> Result<(), String> {
    if status.code() != Some(35) || !transcript.ends_with('\n') {
        return Err("bad media did not terminate with guest failure35".into());
    }
    let lines = transcript
        .lines()
        .filter(|v| !v.trim().is_empty())
        .collect::<Vec<_>>();
    // Failure occurs before buffered baseline evidence is emitted: eight boot
    // readiness records and exactly the attributed unarmed service fault.
    if lines.len() != 9 {
        return Err("bad-media failure transcript length changed".into());
    }
    let last = json_parser::new(lines[8]).parse_record()?;
    if last.len() != 4 {
        return Err("bad-media failure has extra fields".into());
    }
    require_number(&last, "failure", "schema", 1)?;
    require_string(
        &last,
        "failure",
        "event",
        "kernel.native_application_test.failed",
    )?;
    require_string(&last, "failure", "level", "fatal")?;
    require_string(&last, "failure", "reason", "durable_unexpected_user_fault")?;
    let expected: [(&str, &[&str]); 8] = [
        ("boot.kernel.enter", &["arch"]),
        (
            "kernel.fault_path.ready",
            &[
                "vectors",
                "tss",
                "double_fault_ist",
                "nmi_ist",
                "nmi_masked",
            ],
        ),
        ("boot.omen.accepted", &["version", "boot_drive"]),
        ("boot.memory.map", &["regions", "usable_bytes"]),
        (
            "kernel.memory.ready",
            &[
                "frame_size",
                "address_limit",
                "total_frames",
                "free_frames",
                "unavailable_frames",
                "data_owned_frames",
                "table_owned_frames",
            ],
        ),
        ("boot.display.ready", &["width", "height", "pitch", "bpp"]),
        ("boot.ready", &["control_abi", "framebuffer"]),
        (
            "kernel.interrupts.ready",
            &[
                "backend",
                "vector_base",
                "vector_count",
                "pit_input_hz",
                "pit_divisor",
                "timebase_generation",
                "initial_master_mask",
                "initial_slave_mask",
                "master_mask",
                "slave_mask",
                "input_available",
                "input_source_generation",
                "input_queue_capacity",
                "nmi_masked",
                "if_enabled",
            ],
        ),
    ];
    for (line, (event, fields)) in lines[..8].iter().zip(expected) {
        let record = json_parser::new(line).parse_record()?;
        require_control_header(&record, event, fields)?;
    }
    Ok(())
}

pub(super) fn sha256(path: &Path) -> Result<String, String> {
    let output = run_output_checked(
        &repository_root()?,
        "sha256sum",
        [path.as_os_str().to_owned()],
    )?;
    let text = String::from_utf8(output).map_err(|_| "sha256 output invalid")?;
    let digest = text
        .split_whitespace()
        .next()
        .ok_or("sha256 digest missing")?;
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("sha256 digest malformed".into());
    }
    Ok(digest.into())
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("create new {}: {e}", path.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())
}

pub(super) fn smoke() -> Result<(), String> {
    build_and_inspect_normal_image()?;
    let image = build()?;
    let before = sha256(&image.path)?;
    let result = smoke_built(&image);
    if sha256(&image.path)? != before {
        return Err("durable proof changed its original build image".into());
    }
    result
}

fn smoke_built(image: &built_image) -> Result<(), String> {
    let root = repository_root()?;
    let base = root.join("build/native-durable");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = base.join(format!("smoke-{now}-{}", std::process::id()));
    fs::create_dir(&directory).map_err(|e| format!("create fresh evidence directory: {e}"))?;
    let mut entropy = [0; 8];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut entropy))
        .map_err(|e| format!("independent challenge entropy: {e}"))?;
    let challenge = format!(
        "disk-{}",
        entropy
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let input = directory.join("provision.fwcfg");
    let cold_input = directory.join("cold.fwcfg");
    write_new(&input, &fixture(0, challenge.as_bytes())?)?;
    write_new(&cold_input, &fixture(2, b"")?)?;
    write_new(&directory.join("challenge.txt"), challenge.as_bytes())?;
    let boot_hash = sha256(&image.path)?;
    // QEMU IDE disks need a writable backend. Give it a newly created exact
    // copy, never the original build image; each boot/capture checks the copy
    // after the owned child has stopped, even on failure. No global snapshot
    // option can silently make the separate durable data disk ephemeral.
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
    for memory in ["96M", "320M"] {
        let disk = directory.join(format!("data-{memory}.raw"));
        write_new(&disk, &original)?;
        let (status, transcript) = run_boot(
            &boot,
            &disk,
            &input,
            memory,
            &directory.join(format!("provision-{memory}.log")),
        )?;
        if status.code() != Some(33) {
            return Err(format!("durable provision did not exit33: {status}"));
        }
        let first = verify_records(&transcript, 0, challenge.as_bytes())?;
        let before_cold = bounded_file(&disk, media_bytes)?;
        verify_media(&before_cold, &original, 2, challenge.as_bytes())?;
        write_new(
            &directory.join(format!("before-cold-{memory}.raw")),
            &before_cold,
        )?;
        let (status, transcript) = run_boot(
            &boot,
            &disk,
            &cold_input,
            memory,
            &directory.join(format!("cold-{memory}.log")),
        )?;
        if status.code() != Some(33) {
            return Err(format!("durable cold boot did not exit33: {status}"));
        }
        let second = verify_records(&transcript, 2, challenge.as_bytes())?;
        if first != second {
            return Err("same-size exact-image boot memory accounting changed".into());
        }
        verify_media(
            &bounded_file(&disk, media_bytes)?,
            &before_cold,
            3,
            challenge.as_bytes(),
        )?;
        memories.push(first);
        if memory == "96M" {
            capture(
                &boot,
                &cold_input,
                &directory,
                &before_cold,
                challenge.as_bytes(),
            )?;
            for (label, phase, mut bytes) in [
                ("blank-recovery", 2, original.clone()),
                ("corrupt-header", 2, before_cold.clone()),
                ("refuse-reformat", 0, before_cold.clone()),
            ] {
                if label == "corrupt-header" {
                    bytes[extent_start] ^= 1;
                }
                let disk = directory.join(format!("negative-{label}.raw"));
                write_new(&disk, &bytes)?;
                let (status, transcript) = run_boot(
                    &boot,
                    &disk,
                    if phase == 0 { &input } else { &cold_input },
                    memory,
                    &directory.join(format!("negative-{label}.log")),
                )?;
                verify_failed_media_boot(status, &transcript)?;
                if bounded_file(&disk, media_bytes)? != bytes {
                    return Err(format!("{label} mutated media or formatted implicitly"));
                }
            }
        }
    }
    if memories[0].total != memories[1].total
        || memories[1].free.checked_sub(memories[0].free) != Some(57344)
        || memories[0].unavailable.checked_sub(memories[1].unavailable) != Some(57344)
    {
        return Err("durable boot memory delta differs from224MiB".into());
    }
    if sha256(&image.path)? != boot_hash {
        return Err("durable proof mutated its boot image".into());
    }
    let receipt = format!(
        "{{\"schema\":1,\"image_sha256\":\"{boot_hash}\",\"challenge\":{challenge:?},\"evidence_directory\":{:?},\"successful_debug_exit_boots\":4,\"negative_debug_exit_boots\":3,\"capture_host_quit\":true,\"data_disk_bytes\":{media_bytes},\"extent_lba\":8,\"extent_sectors\":5,\"cache\":\"writeback\",\"flush_enabled\":true,\"guards_unchanged\":true,\"cold_answer_supplied\":false}}\n",
        directory.to_string_lossy()
    );
    write_new(&directory.join("receipt.json"), receipt.as_bytes())?;
    fs::write(base.join("smoke-receipt.json"), receipt).map_err(|e| e.to_string())?;
    println!(
        "Native durable proof verified two-size provisioning/cold boots, independent media and public pixels; retained {}",
        directory.display()
    );
    Ok(())
}

fn capture(
    image: &Path,
    input: &Path,
    directory: &Path,
    pre_cold: &[u8],
    challenge: &[u8],
) -> Result<(), String> {
    let before = sha256(image)?;
    let result = capture_inner(image, input, directory, pre_cold, challenge);
    if sha256(image)? != before {
        return Err("durable capture changed its disposable boot-image copy".into());
    }
    result
}

fn capture_inner(
    image: &Path,
    input: &Path,
    directory: &Path,
    pre_cold: &[u8],
    challenge: &[u8],
) -> Result<(), String> {
    // Clone only an already independently verified checkpoint; do not mutate
    // the disk whose two debug-exit lifetimes are part of the main receipt.
    let disk = directory.join("capture.raw");
    write_new(&disk, pre_cold)?;
    let log = directory.join("capture.log");
    let err = directory.join("capture.stderr.log");
    let frame = directory.join("capture.ppm");
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
        if verify_transcript_with_challenge(&transcript, 2, challenge).is_ok() {
            break;
        }
        capture_running(&mut child.0, deadline)?;
        thread::sleep(Duration::from_millis(10));
    }
    let path = qemu_path(&frame)?
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let monitor = child.0.stdin.as_mut().ok_or("capture monitor missing")?;
    writeln!(monitor, "screendump \"{path}\"")
        .and_then(|()| monitor.flush())
        .map_err(|e| e.to_string())?;
    let pixels = surface_reference(2, challenge)?.1;
    loop {
        if let Ok(metadata) = fs::metadata(&frame) {
            if metadata.len() > ppm_size as u64 {
                return Err("durable frame exceeded bound".into());
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
    let status = wait_exit(&mut child.0, deadline, &[&log, &err])?;
    if status.code() != Some(0) {
        return Err(format!("durable host-quit capture failed: {status}"));
    }
    verify_transcript_with_challenge(
        &read_bounded_optional_utf8_file(&log, transcript_limit)?,
        2,
        challenge,
    )?;
    native_application_smoke::verify_frame_pixels(&bounded_file(&frame, ppm_size)?, &pixels)?;
    verify_media(&bounded_file(&disk, media_bytes)?, pre_cold, 3, challenge)?;
    write_new(&directory.join("capture-receipt.json"), b"{\"schema\":1,\"host_exit_status\":0,\"isa_debug_exit\":false,\"width\":1024,\"height\":768,\"verified_x\":32,\"verified_y\":32,\"verified_width\":384,\"verified_height\":24,\"verified_pixels\":9216}\n")
}

fn capture_running(child: &mut Child, deadline: Instant) -> Result<(), String> {
    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
        return Err(format!("durable capture exited prematurely: {status}"));
    }
    if Instant::now() >= deadline {
        return Err("durable capture exceeded30-second deadline".into());
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
            (15, 9, 8)
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
    fn durable_accepts_only_independent_challenge_and_exact_phase_sequences() {
        for phase in [0, 2] {
            let text = valid_transcript(phase);
            verify_transcript_with_challenge(&text, phase, challenge).unwrap();
            assert_eq!(text.lines().count(), if phase == 0 { 65 } else { 42 });
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
    fn durable_rejects_every_missing_changed_duplicate_or_extra_native_field() {
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
    fn media(epoch: u64) -> Vec<u8> {
        let mut bytes = fresh_media();
        let mut header = test_sector(b"ANDURHDR");
        header[12..16].copy_from_slice(&5u32.to_le_bytes());
        seal(&mut header);
        bytes[extent_start..extent_start + 512].copy_from_slice(&header);
        for epoch in [epoch - 1, epoch] {
            let sequence = epoch + 2;
            let slot = (sequence - 1) % 2;
            let mut body = test_sector(b"ANDURBOD");
            let mut commit = test_sector(b"ANDURCMT");
            for out in [&mut body, &mut commit] {
                for (offset, value) in [(32, slot), (40, sequence), (48, epoch), (56, 2)] {
                    out[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
                }
            }
            body[64] = challenge.len() as u8;
            body[65..65 + challenge.len()].copy_from_slice(challenge);
            seal(&mut body);
            commit[64..68].copy_from_slice(&body[508..512]);
            seal(&mut commit);
            let start = extent_start + (1 + slot as usize * 2) * 512;
            bytes[start..start + 512].copy_from_slice(&body);
            bytes[start + 512..start + 1024].copy_from_slice(&commit);
        }
        bytes
    }

    #[test]
    fn durable_media_decoder_has_known_crc_and_checks_every_disk_boundary() {
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
        let original = fresh_media();
        for epoch in [2, 3] {
            let good = media(epoch);
            verify_media(&good, &original, epoch, challenge).unwrap();
            assert!(verify_media(&good, &original, epoch + 1, challenge).is_err());
            assert!(verify_media(&good, &original, epoch, b"wrong answer").is_err());
            for offset in [0, extent_start - 1, extent_end, media_bytes - 1] {
                let mut changed = good.clone();
                changed[offset] ^= 1;
                assert!(verify_media(&changed, &original, epoch, challenge).is_err());
            }
            for offset in extent_start..extent_end {
                let mut changed = good.clone();
                changed[offset] ^= 1;
                assert!(
                    decode_media(&changed).is_err(),
                    "accepted changed sector byte {offset}"
                );
            }
            assert!(decode_media(&good[..good.len() - 1]).is_err());
            let mut extra = good.clone();
            extra.push(0);
            assert!(decode_media(&extra).is_err());
        }
    }

    #[test]
    fn durable_media_rejects_resealed_identity_padding_commit_and_sequence_forgery() {
        let good = media(3);
        // Re-sealing is not authorization; every reserved byte and identity
        // remains constrained independently of the checksums.
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
        // Make a self-consistent but nonadjacent slot1 checkpoint. This must
        // fail the inter-slot transition check rather than just its CRC.
        let start = extent_start + 3 * 512;
        for sector in [start, start + 512] {
            bad[sector + 40..sector + 48].copy_from_slice(&8u64.to_le_bytes());
            bad[sector + 48..sector + 56].copy_from_slice(&6u64.to_le_bytes());
            seal(&mut bad[sector..sector + 512]);
        }
        let crc = bad[start + 508..start + 512].to_vec();
        bad[start + 512 + 64..start + 512 + 68].copy_from_slice(&crc);
        seal(&mut bad[start + 512..start + 1024]);
        assert!(decode_media(&bad).is_err());
        let mut old = media(2);
        // A perfectly valid older image still fails the externally expected
        // epoch. This is a test receipt check, not a disk antirollback claim.
        assert!(verify_media(&old, &fresh_media(), 3, challenge).is_err());
        old[extent_start + 512..extent_start + 1024].fill(0);
        assert!(decode_media(&old).is_err());
    }

    #[test]
    fn durable_fixture_has_no_cold_answer_and_failed_boot_never_counts_as_success() {
        let cold = fixture(2, b"").unwrap();
        assert_eq!(&cold[..8], b"ANDI\x01\0\0\0");
        assert_eq!(u64_at(&cold, 8), 2);
        assert!(cold[16..].iter().all(|b| *b == 0));
        assert!(fixture(2, challenge).is_err());
        assert!(fixture(0, b"").is_err());
        assert!(fixture(1, challenge).is_err());
        assert!(fixture(0, &[b'a'; 32]).is_err());
        assert!(fixture(0, b"bad\x7f").is_err());
        assert!(fixture(0, &[b'a'; 31]).is_ok());
        let baseline = native_application_smoke::tests::valid_transcript();
        let mut lines = baseline
            .lines()
            .take(8)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        lines.push(r#"{"schema":1,"event":"kernel.native_application_test.failed","level":"fatal","reason":"durable_unexpected_user_fault"}"#.into());
        let failed = lines.join("\n") + "\n";
        verify_failed_media_boot(ExitStatus::from_raw(35 << 8), &failed).unwrap();
        for code in [0, 33, 1] {
            assert!(verify_failed_media_boot(ExitStatus::from_raw(code << 8), &failed).is_err());
        }
        assert!(
            verify_failed_media_boot(
                ExitStatus::from_raw(35 << 8),
                &(failed.clone() + &lines[8] + "\n")
            )
            .is_err()
        );
        assert!(
            verify_failed_media_boot(
                ExitStatus::from_raw(35 << 8),
                &failed.replace("durable_unexpected_user_fault", "durable_fixture_invalid")
            )
            .is_err()
        );
        assert!(verify_transcript_with_challenge(&failed, 2, challenge).is_err());
    }

    #[test]
    fn durable_feature_selection_respects_all_higher_profiles_and_dependencies() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        assert!(selected(&[
            feature,
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
            assert!(!selected(&[feature, higher]), "durable overrode {higher}");
        }
    }
}
