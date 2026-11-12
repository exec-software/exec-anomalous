//! Independent native manifest-loader fixture packaging and serial oracle.
//! Provisioning hashes bind external bytes; they do not authenticate a publisher.
use super::*;

pub(super) const feature: &str = "native_manifest_test";
const arena_bases: [u64; 2] = [0x804000, 0x828000];
const arena_bytes: u64 = 131072;
const code_pages: u64 = 5;
const stack_bytes: u64 = 32768;
const data_frames: u64 = code_pages + 2 + 32 + 8 + 4;
const marker: &str = "Native manifest: console admitted";
const ready_event: &str = "kernel.native_manifest_test.ready";
const scenario_event: &str = "kernel.native_manifest_test.scenario";
const passed_event: &str = "kernel.native_manifest_test.passed";
const boot_events: [&str; 8] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
];

struct edition_spec {
    path: &'static str,
    bytes: usize,
    sha256: &'static str,
    intent: &'static str,
    services: u64,
}

const editions: [edition_spec; 4] = [
    edition_spec {
        path: "native/arithmetic.nil",
        bytes: 636,
        sha256: "b015b6d807ab0b01821364d3d71bbe1d38dacdb073910dcba42f41c91ee86b63",
        intent: "Calculate a bounded integer result without external services",
        services: 0,
    },
    edition_spec {
        path: "native/console.nil",
        bytes: 379,
        sha256: "7dd9640422d35d31386b72393162ce4d08451956384d624b53b34714fe4a051f",
        intent: "Write one greeting to the granted console and return zero",
        services: 1,
    },
    edition_spec {
        path: "native/large.nil",
        bytes: 8261,
        sha256: "184a478dec89d4d096610b49f6fa6d5c3be0917a490a8e369678605ea4b19994",
        intent: "Calculate a chunked bounded result without external services",
        services: 0,
    },
    edition_spec {
        path: "compiler-console.nil",
        bytes: 1012,
        sha256: "6df7742ffb25c7f89d3ac6ce432cc0c11eb8aac6f76d5252858cbd15c160e876",
        intent: "Write one greeting to the granted console and return zero",
        services: 1,
    },
];

struct fixture_spec {
    name: &'static str,
    edition: usize,
    ceiling: u64,
    bytes: usize,
    fnv64: u64,
    sha256: &'static str,
    reason: &'static str,
    reason_code: u32,
}

// Computed independently with Python struct/hashlib over complete compiler
// outputs. Neither the manifest encoder nor the guest supplies these expected
// identities, field values or refusal outcomes.
const fixtures: [fixture_spec; 10] = [
    fixture_spec {
        name: "arithmetic_none",
        edition: 0,
        ceiling: 0,
        bytes: 856,
        fnv64: 0x991225a06ba96b59,
        sha256: "e5c9bdf3a50f6006d0287f7dc13d1437eab22352a953b42a6ea5679649d63cea",
        reason: "none",
        reason_code: 0,
    },
    fixture_spec {
        name: "arithmetic_console",
        edition: 0,
        ceiling: 1,
        bytes: 856,
        fnv64: 0x991225a06ba96b59,
        sha256: "e5c9bdf3a50f6006d0287f7dc13d1437eab22352a953b42a6ea5679649d63cea",
        reason: "none",
        reason_code: 0,
    },
    fixture_spec {
        name: "console_console",
        edition: 1,
        ceiling: 1,
        bytes: 596,
        fnv64: 0x771a45936b84d907,
        sha256: "585cc723c757edf3219eb4564c5a7cb3a38eedbbd48a9545a270c3bfd18f2fed",
        reason: "none",
        reason_code: 0,
    },
    fixture_spec {
        name: "console_none",
        edition: 1,
        ceiling: 0,
        bytes: 596,
        fnv64: 0x771a45936b84d907,
        sha256: "585cc723c757edf3219eb4564c5a7cb3a38eedbbd48a9545a270c3bfd18f2fed",
        reason: "manifest_ceiling",
        reason_code: 13,
    },
    fixture_spec {
        name: "large_none",
        edition: 2,
        ceiling: 0,
        bytes: 8481,
        fnv64: 0x350c6643497d1c52,
        sha256: "c50495674cf37efec7706cfdf5074f633ff6e205d92f652c82be4f8164a861e0",
        reason: "none",
        reason_code: 0,
    },
    fixture_spec {
        name: "stale_identity",
        edition: 1,
        ceiling: 1,
        bytes: 596,
        fnv64: 0x7716e3936b81fcaa,
        sha256: "c2f64c2c21a615177be6efba3d3fea954d386972d62726170f9887f4f72caa52",
        reason: "manifest_identity",
        reason_code: 11,
    },
    fixture_spec {
        name: "understated_surface",
        edition: 1,
        ceiling: 1,
        bytes: 596,
        fnv64: 0xa6f696dbe63f7866,
        sha256: "432c7c1a632f7eb9a03e6f68b2f91200bf7e0ea0f4c22cf18c9ff0481a36f9da",
        reason: "manifest_mismatch",
        reason_code: 12,
    },
    fixture_spec {
        name: "changed_intent",
        edition: 1,
        ceiling: 1,
        bytes: 596,
        fnv64: 0xe5724f97d60c1f3f,
        sha256: "9e70a6089e2cfc5f3870457b64ae1a72488a1bb464943f60438aa4df0d6a97f9",
        reason: "manifest_mismatch",
        reason_code: 12,
    },
    fixture_spec {
        name: "reserved_encoding",
        edition: 1,
        ceiling: 1,
        bytes: 596,
        fnv64: 0xf45d095b5821617e,
        sha256: "9afd6e1a423f536e9ba887932c2bda1e153f4b406d9ad33fb9ea01d8b90c7493",
        reason: "manifest_encoding",
        reason_code: 7,
    },
    fixture_spec {
        name: "unsupported_compiler",
        edition: 3,
        ceiling: 1,
        bytes: 1229,
        fnv64: 0x880a5f195f25bec8,
        sha256: "ab53e16086a243198fb9d5c9e84a17b0fef60d0a1765f71302145d07961913f2",
        reason: "manifest_unsupported",
        reason_code: 3,
    },
];

impl fixture_spec {
    fn reads(&self) -> u64 {
        (self.bytes as u64).div_ceil(4096)
    }
    fn console_calls(&self) -> u64 {
        if self.reason_code == 0 {
            editions[self.edition].services
        } else {
            0
        }
    }
    fn status(&self) -> &'static str {
        if self.reason_code == 0 {
            "native_policy_admitted"
        } else {
            "native_policy_refused"
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err("manifest fixture exceeds its regular-file byte bound".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > maximum {
        return Err("manifest fixture grew beyond its byte bound".into());
    }
    Ok(bytes)
}

fn put64(bytes: &mut [u8], at: usize, value: u64) {
    bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Independent ADR0102 envelope writer, including deliberately false claims
/// for negative fixtures. Production packaging still uses the shared encoder.
fn envelope(edition: &[u8], spec: &edition_spec) -> Vec<u8> {
    let edition_offset = 160 + spec.intent.len();
    let length = edition_offset + edition.len();
    let mut image = vec![0; length];
    image[..8].copy_from_slice(b"ANPROG\r\n");
    image[8..10].copy_from_slice(&1u16.to_le_bytes());
    image[10..12].copy_from_slice(&64u16.to_le_bytes());
    put32(&mut image, 12, 1);
    for (at, value) in [
        (16, 64),
        (24, 96 + spec.intent.len()),
        (32, edition_offset),
        (40, edition.len()),
        (48, length),
    ] {
        put64(&mut image, at, value as u64);
    }
    image[64..72].copy_from_slice(b"ANSMAN\r\n");
    image[72..74].copy_from_slice(&1u16.to_le_bytes());
    image[74..76].copy_from_slice(&96u16.to_le_bytes());
    put32(&mut image, 76, 1);
    put64(&mut image, 80, spec.services);
    put64(&mut image, 88, edition.len() as u64);
    image[96..128].copy_from_slice(&anomalous_program_manifest::sha256(edition));
    put32(&mut image, 128, spec.intent.len() as u32);
    image[160..edition_offset].copy_from_slice(spec.intent.as_bytes());
    image[edition_offset..].copy_from_slice(edition);
    image
}

fn fixture_images(root: &Path) -> Result<Vec<Vec<u8>>, String> {
    let directory = root.join("crates/anomalous-program-manifest/fixtures");
    let mut sources = Vec::new();
    for spec in &editions {
        let bytes = read_bounded(&directory.join(spec.path), 65536)?;
        if bytes.len() != spec.bytes
            || hex(&anomalous_program_manifest::sha256(&bytes)) != spec.sha256
        {
            return Err(format!(
                "complete compiler fixture identity changed: {}",
                spec.path
            ));
        }
        sources.push(bytes);
    }
    let mut images = Vec::new();
    for (index, spec) in fixtures.iter().enumerate() {
        let mut image = envelope(&sources[spec.edition], &editions[spec.edition]);
        match index {
            5 => {
                let last = image.len() - 2;
                image[last] ^= 1;
            }
            6 => put64(&mut image, 80, 0),
            7 => {
                let edition_offset = 160 + editions[1].intent.len();
                let needle = b"Write one greeting";
                let positions = image[edition_offset..]
                    .windows(needle.len())
                    .enumerate()
                    .filter_map(|(index, value)| (value == needle).then_some(index))
                    .collect::<Vec<_>>();
                if positions.len() != 1 {
                    return Err("intent mutation is ambiguous".into());
                }
                image[edition_offset + positions[0]] = b'w';
                let identity = anomalous_program_manifest::sha256(&image[edition_offset..]);
                image[96..128].copy_from_slice(&identity);
            }
            8 => image[56] = 1,
            _ => {}
        }
        verify_fixture(&image, spec)?;
        match anomalous_program_manifest::admit(&image, spec.ceiling) {
            Ok(witness) if spec.reason_code == 0 => {
                let edition = &editions[spec.edition];
                if witness.required_services() != edition.services
                    || witness.granted_services() != edition.services
                    || witness.intent() != edition.intent
                    || hex(&witness.identity()) != edition.sha256
                {
                    return Err(
                        "host admission differs from independently specified compiler fixture"
                            .into(),
                    );
                }
            }
            Err(error) if error.as_str() == spec.reason => {}
            _ => {
                return Err(format!(
                    "manifest fixture {} has an unexpected admission outcome",
                    spec.name
                ));
            }
        }
        images.push(image);
    }
    Ok(images)
}

fn verify_fixture(image: &[u8], spec: &fixture_spec) -> Result<(), String> {
    if image.len() != spec.bytes
        || fnv64(image) != spec.fnv64
        || hex(&anomalous_program_manifest::sha256(image)) != spec.sha256
    {
        return Err(format!(
            "manifest package differs from independent pins: {}",
            spec.name
        ));
    }
    Ok(())
}

fn source_name(index: usize) -> String {
    format!("opt/anomalous-manifest-{index}")
}

pub(super) fn prepare_fixtures(root: &Path, build: &Path) -> Result<Vec<PathBuf>, String> {
    let images = fixture_images(root)?;
    let output = build.join("native-manifest");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut paths = Vec::new();
    let mut receipts = Vec::new();
    for (index, (image, spec)) in images.iter().zip(&fixtures).enumerate() {
        let path = output.join(format!("edition-{index}.re"));
        fs::write(&path, image).map_err(|e| e.to_string())?;
        verify_fixture(&read_bounded(&path, 66208)?, spec)?;
        receipts.push(format!(r#"{{"case":{index},"name":"{}","fw_cfg":"{}","bytes":{},"fnv64":{},"sha256":"{}","ceiling":{},"expected_status":"{}","expected_reason":"{}"}}"#,
            spec.name, source_name(index), spec.bytes, spec.fnv64, spec.sha256, spec.ceiling, spec.status(), spec.reason));
        paths.push(path);
    }
    fs::write(
        output.join("fixtures.json"),
        format!("[{}]\n", receipts.join(",")),
    )
    .map_err(|e| e.to_string())?;
    Ok(paths)
}

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|value| {
            matches!(
                *value,
                "fault_test"
                    | "circle_test"
                    | "control_test"
                    | "rite_test"
                    | "ny_vessel_test"
                    | "memory_test"
                    | "interrupt_test"
                    | "supervision_test"
                    | "shell_vessel_test"
                    | "input_wait_test"
                    | "display_vessel_test"
                    | "supervised_renderer_test"
                    | "semantic_surface_transport_test"
                    | "semantic_surface_presentation_test"
                    | "desktop_dev"
                    | "elf_application_test"
                    | "linux_application_test"
                    | "veil_application_test"
                    | "shared_veil_test"
                    | "linux_tls_test"
                    | "linux_loader_entry_test"
                    | "native_durable_test"
                    | "native_durable_failure_test"
                    | "native_repeated_editor_test"
                    | "native_interactive_editor_test"
                    | "native_document_test"
                    | "elf_document_test"
                    | "native_relaunch_test"
                    | "elf_relaunch_test"
            )
        })
}

pub(super) fn build() -> Result<built_image, String> {
    let root = repository_root()?;
    prepare_fixtures(&root, &root.join("build"))?;
    let image = build_image_with_context(&[feature], None)?;
    println!("native manifest image inspected: {}", image.path.display());
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let root = repository_root()?;
    let build = root.join("build");
    build_and_inspect_normal_image()?;
    let image = self::build()?;
    let sources = (0..fixtures.len())
        .map(|i| build.join(format!("native-manifest/edition-{i}.re")))
        .collect::<Vec<_>>();
    for (path, spec) in sources.iter().zip(&fixtures) {
        verify_fixture(&read_bounded(path, 66208)?, spec)?;
    }
    verify_provisioning_rejections(&image.path, &build, &sources)?;
    let small = run_size(
        &image.path,
        &sources,
        "96M",
        &build.join("qemu_native_manifest_96M.log"),
    )?;
    let large = run_size(
        &image.path,
        &sources,
        "320M",
        &build.join("qemu_native_manifest_320M.log"),
    )?;
    let delta = (320u64 - 96) * 1024 * 1024 / memory_frame_size;
    if small.total_frames != memory_total_frames
        || large.total_frames != memory_total_frames
        || large.free_frames.checked_sub(small.free_frames) != Some(delta)
        || small
            .unavailable_frames
            .checked_sub(large.unavailable_frames)
            != Some(delta)
    {
        return Err("native manifest RAM evidence differs from the 224-MiB frame delta".into());
    }
    println!(
        "native manifest loader admitted four and refused six external packages at 96M and 320M; one witness-gated loader marker, zero edition executions, complete cleanup"
    );
    Ok(())
}

fn command(image: &Path, sources: &[Option<PathBuf>], memory: &str) -> Result<Command, String> {
    if sources.len() != fixtures.len() {
        return Err("manifest source catalog size changed".into());
    }
    let mut command = qemu_command_with_memory(image, memory)?;
    command.args([
        "-cpu",
        "Nehalem",
        "-display",
        "none",
        "-monitor",
        "none",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ]);
    for (index, source) in sources.iter().enumerate() {
        if let Some(source) = source {
            command.arg("-fw_cfg").arg(format!(
                "name={},file={}",
                source_name(index),
                source.display()
            ));
        }
    }
    Ok(command)
}

fn run_size(
    image: &Path,
    sources: &[PathBuf],
    memory: &str,
    log: &Path,
) -> Result<run_evidence, String> {
    for (path, spec) in sources.iter().zip(&fixtures) {
        verify_fixture(&read_bounded(path, 66208)?, spec)?;
    }
    remove_optional_file(log)?;
    let command = command(
        image,
        &sources.iter().cloned().map(Some).collect::<Vec<_>>(),
        memory,
    )?;
    let mut evidence = None;
    run_qemu_with_transcript(command, smoke_timeout, |status, transcript| {
        fs::write(log, transcript).map_err(|e| e.to_string())?;
        if status.code() != Some(qemu_success_status) {
            return Err(format!("native manifest QEMU failed: {:?}", status.code()));
        }
        evidence = Some(verify_records(transcript)?);
        Ok(())
    })?;
    evidence.ok_or_else(|| "native manifest run supplied no evidence".into())
}

fn verify_provisioning_rejections(
    image: &Path,
    build: &Path,
    sources: &[PathBuf],
) -> Result<(), String> {
    let bytes = read_bounded(&sources[0], 66208)?;
    verify_fixture(&bytes, &fixtures[0])?;
    for (name, replacement, reason) in [
        ("missing", None, "program_source_missing"),
        (
            "size",
            Some(bytes[..bytes.len() - 1].to_vec()),
            "program_source_size",
        ),
        (
            "hash",
            Some({
                let mut changed = bytes.clone();
                changed[0] ^= 1;
                changed
            }),
            "program_source_hash",
        ),
    ] {
        let mut selected = sources.iter().cloned().map(Some).collect::<Vec<_>>();
        selected[0] = if let Some(bytes) = replacement {
            let path = build.join(format!("native-manifest/provisioning-{name}.re"));
            fs::write(&path, bytes).map_err(|e| e.to_string())?;
            Some(path)
        } else {
            None
        };
        let log = build.join(format!("qemu_native_manifest_source_{name}.log"));
        remove_optional_file(&log)?;
        run_qemu_with_transcript(
            command(image, &selected, "96M")?,
            smoke_timeout,
            |status, transcript| {
                fs::write(&log, transcript).map_err(|e| e.to_string())?;
                verify_source_rejection(status.code(), transcript, reason)
            },
        )?;
    }
    Ok(())
}

fn verify_source_rejection(
    exit: Option<i32>,
    transcript: &str,
    reason: &str,
) -> Result<(), String> {
    if exit != Some(35)
        || !matches!(
            reason,
            "program_source_missing" | "program_source_size" | "program_source_hash"
        )
    {
        return Err("manifest provisioning refusal status or reason changed".into());
    }
    let records = transcript
        .lines()
        .map(|line| json_parser::new(line).parse_record())
        .collect::<Result<Vec<_>, _>>()?;
    if records.len() != 10 {
        return Err("manifest provisioning refusal needs eight boot and two failure events".into());
    }
    verify_boot(&records[..8])?;
    let event = "kernel.native_manifest_test.source_rejected";
    require_control_header(
        &records[8],
        event,
        &["reason", "source_released", "resources_restored"],
    )?;
    require_string(&records[8], event, "reason", reason)?;
    require_bool(&records[8], event, "source_released", true)?;
    require_bool(&records[8], event, "resources_restored", true)?;
    let event = "kernel.native_application_test.failed";
    if records[9].len() != 4 {
        return Err("manifest provisioning fatal event fields changed".into());
    }
    require_number(&records[9], event, "schema", 1)?;
    require_string(&records[9], event, "event", event)?;
    require_string(&records[9], event, "level", "fatal")?;
    require_string(&records[9], event, "reason", reason)
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for candidate in [
        "native_manifest_test",
        "native_manifest_profile",
        "anomalous_native_manifest_loader",
        "x86_64_native_manifest",
        "opt/anomalous-manifest-",
        "ANMRPT01",
        marker,
    ] {
        if symbols.contains(candidate)
            || bytes
                .windows(candidate.len())
                .any(|bytes| bytes == candidate.as_bytes())
        {
            return Err("normal image retains native manifest proof implementation".into());
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct run_evidence {
    free_frames: u64,
    unavailable_frames: u64,
    total_frames: u64,
}

pub(super) fn verify_transcript(transcript: &str) -> Result<(), String> {
    verify_records(transcript).map(|_| ())
}

fn verify_boot(records: &[structured_record]) -> Result<(), String> {
    if records.len() != boot_events.len() {
        return Err("native manifest boot prerequisite count changed".into());
    }
    for (record, event) in records.iter().zip(boot_events) {
        require_string(record, event, "event", event)?;
    }
    validate_scheduler_boot_records(records)
}

fn verify_records(transcript: &str) -> Result<run_evidence, String> {
    let mut records = Vec::new();
    let mut marker_seen = false;
    for line in transcript.lines() {
        if line == marker {
            if records.len() != 8 || marker_seen {
                return Err("native manifest marker order/count changed".into());
            }
            marker_seen = true;
        } else {
            if line.is_empty() || line.trim() != line || (records.len() == 8 && !marker_seen) {
                return Err(
                    "native manifest transcript contains unexpected text or omits its marker"
                        .into(),
                );
            }
            records.push(json_parser::new(line).parse_record()?);
        }
    }
    if records.len() != 20 || !marker_seen {
        return Err("native manifest lifecycle event count changed".into());
    }
    verify_boot(&records[..8])?;
    let free_frames = required_number(&records[4], "kernel.memory.ready", "free_frames")?;
    if free_frames < data_frames + 9 + 3 {
        return Err("native manifest boot budget is too small".into());
    }
    let ready = &records[8];
    require_control_header(
        ready,
        ready_event,
        &[
            "vendor",
            "tasks",
            "scenarios",
            "code_pages",
            "descriptor_bytes",
            "arena_bytes",
            "stack_bytes",
            "mappings",
            "data_frames",
            "table_frames",
            "leases",
            "free_frames",
            "edition_mapped",
        ],
    )?;
    for (field, value) in [
        ("tasks", 1),
        ("scenarios", 10),
        ("code_pages", code_pages),
        ("descriptor_bytes", 256),
        ("arena_bytes", arena_bytes),
        ("stack_bytes", stack_bytes),
        ("mappings", 4),
        ("data_frames", data_frames),
        ("table_frames", 9),
        ("leases", 6),
        ("free_frames", free_frames),
    ] {
        require_number(ready, ready_event, field, value)?;
    }
    if !matches!(ready.get("vendor"),Some(json_scalar::string(value)) if matches!(value.as_str(),"GenuineIntel"|"AuthenticAMD"|"HygonGenuine"))
    {
        return Err("native manifest CPU vendor is not admitted".into());
    }
    require_bool(ready, ready_event, "edition_mapped", false)?;
    for (index, spec) in fixtures.iter().enumerate() {
        verify_scenario(&records[9 + index], index, spec)?;
    }
    let passed = &records[19];
    require_control_header(
        passed,
        passed_event,
        &[
            "program_tasks",
            "descriptor_bytes",
            "arena_bytes",
            "zeroed_bytes",
            "editions_executed",
            "loader_console_writes",
            "source_released",
            "allocator_available",
            "resources_restored",
            "authority_revoked",
            "edition_frame_preserved",
            "source_language",
        ],
    )?;
    for (field, value) in [
        ("program_tasks", 10),
        ("descriptor_bytes", 256),
        ("arena_bytes", arena_bytes),
        ("zeroed_bytes", 10 * arena_bytes),
        ("editions_executed", 0),
        ("loader_console_writes", 1),
    ] {
        require_number(passed, passed_event, field, value)?;
    }
    for field in [
        "source_released",
        "allocator_available",
        "resources_restored",
        "authority_revoked",
        "edition_frame_preserved",
    ] {
        require_bool(passed, passed_event, field, true)?;
    }
    require_string(passed, passed_event, "source_language", "rust")?;
    Ok(run_evidence {
        free_frames,
        unavailable_frames: required_number(
            &records[4],
            "kernel.memory.ready",
            "unavailable_frames",
        )?,
        total_frames: required_number(&records[4], "kernel.memory.ready", "total_frames")?,
    })
}

fn verify_scenario(
    record: &structured_record,
    index: usize,
    spec: &fixture_spec,
) -> Result<(), String> {
    let console = spec.console_calls();
    let numbers = [
        ("scenario", index as u64),
        ("arena_base", arena_bases[index % 2]),
        ("zeroed_bytes", arena_bytes),
        ("calls", spec.reads() + console + 2),
        ("invalid", 0),
        ("exits", 1),
        ("bootstrap_services", 3),
        ("console_attempts", console),
        ("console_writes", console),
        ("console_rejected", 0),
        ("console_bytes", console * (marker.len() as u64 + 1)),
        ("edition_attempts", spec.reads()),
        ("edition_reads", spec.reads()),
        ("edition_rejected", 0),
        ("edition_read_bytes", spec.bytes as u64),
        ("edition_source_reads", spec.reads()),
        ("allocator_service_borrows", 2),
        ("allocator_nested_denials", 1),
        ("workload_effects", 0),
    ];
    let mut fields = numbers.iter().map(|(field, _)| *field).collect::<Vec<_>>();
    fields.extend(["timer_ticks", "status", "reason", "executed", "report_hex"]);
    require_control_header(record, scenario_event, &fields)?;
    for (field, value) in numbers {
        require_number(record, scenario_event, field, value)?;
    }
    required_number(record, scenario_event, "timer_ticks")?;
    require_string(record, scenario_event, "status", spec.status())?;
    require_string(record, scenario_event, "reason", spec.reason)?;
    require_bool(record, scenario_event, "executed", false)?;
    require_string(record, scenario_event, "report_hex", &expected_report(spec))
}

/// Manual report oracle: literal offsets/values independent of the guest's
/// report codec. Full hex equality checks every reserved and unused byte too.
fn expected_report(spec: &fixture_spec) -> String {
    let mut report = [0; 768];
    report[..8].copy_from_slice(b"ANMRPT01");
    report[8..10].copy_from_slice(&1u16.to_le_bytes());
    report[10..12].copy_from_slice(&768u16.to_le_bytes());
    put32(&mut report, 12, if spec.reason_code == 0 { 1 } else { 2 });
    put32(&mut report, 16, spec.reason_code);
    for (at, value) in [
        (24, spec.ceiling),
        (56, spec.bytes as u64),
        (64, spec.reads()),
        (72, spec.bytes as u64),
    ] {
        put64(&mut report, at, value);
    }
    if spec.reason_code == 0 {
        let edition = &editions[spec.edition];
        for (at, value) in [
            (32, edition.services),
            (40, edition.services),
            (48, edition.bytes as u64),
            (80, spec.console_calls()),
        ] {
            put64(&mut report, at, value);
        }
        for (index, byte) in report[88..120].iter_mut().enumerate() {
            *byte = u8::from_str_radix(&edition.sha256[index * 2..index * 2 + 2], 16).unwrap();
        }
        put32(&mut report, 120, edition.intent.len() as u32);
        report[128..128 + edition.intent.len()].copy_from_slice(edition.intent.as_bytes());
    }
    hex(&report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boot() -> Vec<String> {
        [
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#,
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#,
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#,
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#,
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#,
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#,
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#,
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#,
        ].into_iter().map(str::to_owned).collect()
    }

    fn valid() -> Vec<String> {
        let mut lines = boot();
        lines.push("Native manifest: console admitted".into());
        lines.push(r#"{"schema":1,"event":"kernel.native_manifest_test.ready","level":"info","vendor":"GenuineIntel","tasks":1,"scenarios":10,"code_pages":5,"descriptor_bytes":256,"arena_bytes":131072,"stack_bytes":32768,"mappings":4,"data_frames":51,"table_frames":9,"leases":6,"free_frames":20000,"edition_mapped":false}"#.into());
        for (index, spec) in fixtures.iter().enumerate() {
            let reads = if index == 4 { 3 } else { 1 };
            let console = u64::from(index == 2);
            lines.push(format!(r#"{{"schema":1,"event":"kernel.native_manifest_test.scenario","level":"info","scenario":{index},"arena_base":{},"zeroed_bytes":131072,"calls":{},"timer_ticks":0,"invalid":0,"exits":1,"bootstrap_services":3,"console_attempts":{console},"console_writes":{console},"console_rejected":0,"console_bytes":{},"edition_attempts":{reads},"edition_reads":{reads},"edition_rejected":0,"edition_read_bytes":{},"edition_source_reads":{reads},"allocator_service_borrows":2,"allocator_nested_denials":1,"status":"{}","reason":"{}","executed":false,"workload_effects":0,"report_hex":"{}"}}"#,
                if index%2 == 0 { 8404992 } else { 8552448 },reads+console+2,console*34,spec.bytes,spec.status(),spec.reason,expected_report(spec)));
        }
        lines.push(r#"{"schema":1,"event":"kernel.native_manifest_test.passed","level":"info","program_tasks":10,"descriptor_bytes":256,"arena_bytes":131072,"zeroed_bytes":1310720,"editions_executed":0,"loader_console_writes":1,"source_released":true,"allocator_available":true,"resources_restored":true,"authority_revoked":true,"edition_frame_preserved":true,"source_language":"rust"}"#.into());
        lines
    }

    fn render(record: &structured_record) -> String {
        let fields = record
            .iter()
            .map(|(name, value)| {
                let value = match value {
                    json_scalar::string(value) => format!("{value:?}"),
                    json_scalar::number(value) => value.clone(),
                    json_scalar::boolean(value) => value.to_string(),
                    json_scalar::null => "null".into(),
                };
                format!("{name:?}:{value}")
            })
            .collect::<Vec<_>>();
        format!("{{{}}}", fields.join(","))
    }

    #[test]
    fn complete_compiler_snapshots_and_every_package_match_independent_pins() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let images = fixture_images(&root).unwrap();
        assert_eq!(images.len(), 10);
        assert_eq!(images[4].len(), 8481);
        assert_eq!(fixtures.iter().map(fixture_spec::reads).sum::<u64>(), 12);
        assert_eq!(
            fixtures
                .iter()
                .map(fixture_spec::console_calls)
                .sum::<u64>(),
            1
        );
        assert_eq!(fnv64(b"hello"), 0xa430d84680aabd0b);
        for (image, spec) in images.iter().zip(&fixtures) {
            let mut changed = image.clone();
            changed[0] ^= 1;
            assert!(verify_fixture(&changed, spec).is_err());
            assert!(verify_fixture(&image[..image.len() - 1], spec).is_err());
        }
    }

    #[test]
    fn exact_reports_preserve_padding_and_match_the_shared_diagnostic_decoder() {
        for spec in &fixtures {
            let encoded = expected_report(spec);
            assert_eq!(encoded.len(), 1536);
            let bytes = encoded
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>();
            let report = anomalous_native_runtime::manifest_report::report::decode(&bytes).unwrap();
            assert_eq!(report.ceiling(), spec.ceiling);
            assert_eq!(report.reason(), spec.reason_code);
            assert_eq!(report.read_bytes(), spec.bytes as u64);
            assert_eq!(report.read_calls(), spec.reads());
            assert_eq!(report.console_calls(), spec.console_calls());
            if spec.reason_code != 0 {
                assert_eq!(report.edition_length(), 0);
                assert_eq!(report.required_services(), 0);
                assert_eq!(report.granted_services(), 0);
                assert_eq!(report.identity(), &[0; 32]);
                assert_eq!(report.intent(), "");
            }
        }
    }

    #[test]
    fn native_manifest_oracle_requires_exact_events_order_and_one_marker() {
        let lines = valid();
        verify_transcript(&lines.join("\n")).unwrap();
        verify_transcript(&lines.join("\r\n")).unwrap();
        for index in 0..lines.len() {
            let mut changed = lines.clone();
            changed.remove(index);
            assert!(
                verify_transcript(&changed.join("\n")).is_err(),
                "missing{index}"
            );
            let mut changed = lines.clone();
            changed.insert(index, lines[index].clone());
            assert!(
                verify_transcript(&changed.join("\n")).is_err(),
                "extra{index}"
            );
            if index + 1 < lines.len() {
                let mut changed = lines.clone();
                changed.swap(index, index + 1);
                assert!(
                    verify_transcript(&changed.join("\n")).is_err(),
                    "swapped{index}"
                );
            }
        }
        for extra in [
            "\n",
            "{}",
            "untrusted output",
            " Native manifest: console admitted",
            "Native manifest: console admitted ",
        ] {
            let mut changed = lines.clone();
            changed[8] = extra.into();
            assert!(verify_transcript(&changed.join("\n")).is_err());
        }
    }

    #[test]
    fn native_manifest_oracle_rejects_hostile_fields_reports_and_effects() {
        let lines = valid();
        for index in 9..lines.len() {
            let record = json_parser::new(&lines[index]).parse_record().unwrap();
            for (field, value) in &record {
                let mut changed = record.clone();
                changed.remove(field);
                let mut output = lines.clone();
                output[index] = render(&changed);
                assert!(
                    verify_transcript(&output.join("\n")).is_err(),
                    "missing {index}:{field}"
                );
                let replacement = match value {
                    json_scalar::string(value) => json_scalar::string(format!("{value}.changed")),
                    json_scalar::number(value) if field != "timer_ticks" => {
                        json_scalar::number((value.parse::<u64>().unwrap() + 1).to_string())
                    }
                    json_scalar::number(_) => json_scalar::boolean(false),
                    json_scalar::boolean(value) => json_scalar::boolean(!value),
                    json_scalar::null => unreachable!(),
                };
                changed.insert(field.clone(), replacement);
                output[index] = render(&changed);
                assert!(
                    verify_transcript(&output.join("\n")).is_err(),
                    "changed {index}:{field}"
                );
                changed.insert(field.clone(), json_scalar::null);
                output[index] = render(&changed);
                assert!(
                    verify_transcript(&output.join("\n")).is_err(),
                    "null {index}:{field}"
                );
            }
            let mut changed = record.clone();
            changed.insert("extra".into(), json_scalar::boolean(true));
            let mut output = lines.clone();
            output[index] = render(&changed);
            assert!(verify_transcript(&output.join("\n")).is_err());
            output[index] = lines[index].replacen("{", "{\"schema\":1,", 1);
            assert!(verify_transcript(&output.join("\n")).is_err());
        }
        // Every byte, including zero padding, is independently covered by the
        // host report comparison. Changing one nibble preserves valid hex.
        for index in 10..20 {
            let record = json_parser::new(&lines[index]).parse_record().unwrap();
            let json_scalar::string(report) = &record["report_hex"] else {
                unreachable!()
            };
            for byte in 0..768 {
                let mut bytes = report.as_bytes().to_vec();
                let at = byte * 2 + 1;
                bytes[at] = if bytes[at] == b'0' { b'1' } else { b'0' };
                let mut changed = record.clone();
                changed.insert(
                    "report_hex".into(),
                    json_scalar::string(String::from_utf8(bytes).unwrap()),
                );
                assert!(
                    verify_scenario(&changed, index - 10, &fixtures[index - 10]).is_err(),
                    "report byte {index}:{byte}"
                );
            }
        }
    }

    #[test]
    fn provisioning_refusals_require_exact_failure_and_cleanup_before_any_launch() {
        for reason in [
            "program_source_missing",
            "program_source_size",
            "program_source_hash",
        ] {
            let mut lines = boot();
            lines.push(format!(r#"{{"schema":1,"event":"kernel.native_manifest_test.source_rejected","level":"info","reason":"{reason}","source_released":true,"resources_restored":true}}"#));
            lines.push(format!(r#"{{"schema":1,"event":"kernel.native_application_test.failed","level":"fatal","reason":"{reason}"}}"#));
            let transcript = lines.join("\n");
            verify_source_rejection(Some(35), &transcript, reason).unwrap();
            for exit in [None, Some(0), Some(33), Some(1)] {
                assert!(verify_source_rejection(exit, &transcript, reason).is_err());
            }
            for index in 0..lines.len() {
                let mut changed = lines.clone();
                changed.remove(index);
                assert!(verify_source_rejection(Some(35), &changed.join("\n"), reason).is_err());
                let mut changed = lines.clone();
                changed.insert(index, lines[index].clone());
                assert!(verify_source_rejection(Some(35), &changed.join("\n"), reason).is_err());
            }
            for (from, to) in [
                ("source_released\":true", "source_released\":false"),
                ("resources_restored\":true", "resources_restored\":false"),
                ("fatal", "info"),
                (reason, "program_source_other"),
            ] {
                assert!(
                    verify_source_rejection(Some(35), &transcript.replace(from, to), reason)
                        .is_err()
                );
            }
            assert!(
                verify_source_rejection(Some(35), &format!("{transcript}\n{marker}"), reason)
                    .is_err()
            );
        }
    }

    #[test]
    fn named_feature_has_explicit_precedence_and_is_absent_normally() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        for lower in [
            "native_program_test",
            "native_runtime_test",
            "native_application_test",
            "mixed_workspace_test",
            "native_editor_workspace_test",
            "ny_object_test",
        ] {
            assert!(selected(&[feature, lower]), "{lower}");
        }
        let higher = [
            "fault_test",
            "circle_test",
            "control_test",
            "rite_test",
            "ny_vessel_test",
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
            "elf_application_test",
            "linux_application_test",
            "veil_application_test",
            "shared_veil_test",
            "linux_tls_test",
            "linux_loader_entry_test",
            "native_durable_test",
            "native_durable_failure_test",
            "native_repeated_editor_test",
            "native_interactive_editor_test",
            "native_document_test",
            "elf_document_test",
            "native_relaunch_test",
            "elf_relaunch_test",
        ];
        for higher in higher {
            assert!(!selected(&[feature, higher]), "{higher}");
        }
        let mut all = higher.to_vec();
        all.push(feature);
        assert!(!selected(&all));
        verify_absent("ordinary_kernel", b"ordinary boot").unwrap();
        for candidate in [
            "native_manifest_test",
            "anomalous_native_manifest_loader",
            "x86_64_native_manifest",
            "opt/anomalous-manifest-0",
            "ANMRPT01",
            marker,
        ] {
            assert!(verify_absent(candidate, b"").is_err());
            assert!(verify_absent("", candidate.as_bytes()).is_err());
        }
    }
}
