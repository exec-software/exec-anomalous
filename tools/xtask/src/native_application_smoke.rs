//! Strict lifecycle and public-frame oracle for the bounded native application.
//! Pixel expectations reuse the portable renderer; independent semantic
//! checkpoints below and renderer pixel tests supply separate evidence.
use super::*;
use anomalous_abi::application::{
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_editor_contract::{editor_state, frame_byte_count, render, theme};

pub(super) const feature: &str = "native_application_test";
const timeout: Duration = Duration::from_secs(30);
const transcript_limit: usize = 64 * 1024;
const ppm_header: &[u8] = b"P6\n1024 768\n255\n";
const ppm_size: usize = 1024 * 768 * 3 + ppm_header.len();
const expected_events: [&str; 21] = [
    "boot.kernel.enter",
    "kernel.fault_path.ready",
    "boot.omen.accepted",
    "boot.memory.map",
    "kernel.memory.ready",
    "boot.display.ready",
    "boot.ready",
    "kernel.interrupts.ready",
    "kernel.native_application_test.ready",
    "kernel.native_application_test.scenario",
    "kernel.native_application_test.scenario",
    "kernel.native_application_test.surface",
    "kernel.native_application_test.surface",
    "kernel.native_application_test.surface",
    "kernel.native_application_test.surface",
    "kernel.native_application_test.surface",
    "kernel.native_application_test.surface",
    "kernel.native_application_test.surface",
    "kernel.native_application_test.presented",
    "kernel.native_application_test.released",
    "kernel.native_application_test.passed",
];

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
                    | "native_runtime_test"
                    | "native_program_test"
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
    let image = build_image_with_context(&[feature], None)?;
    println!(
        "native application image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

pub(super) fn smoke() -> Result<(), String> {
    let directory = prepare_smoke(native_profile)?;
    build_and_inspect_normal_image()?;
    let image = self::build()?;
    smoke_image(&image.path, &directory, native_profile)
}

#[derive(Clone, Copy)]
pub(super) struct smoke_profile {
    pub(super) log_prefix: &'static str,
    pub(super) artifact_prefix: &'static str,
    pub(super) records: usize,
    pub(super) verify: fn(&str) -> Result<run_evidence, String>,
    pub(super) verify_frame: fn(&[u8]) -> Result<(), String>,
}

const native_profile: smoke_profile = smoke_profile {
    log_prefix: "native_application",
    artifact_prefix: "native-application",
    records: 21,
    verify: verify_records,
    verify_frame,
};

pub(super) fn prepare_smoke(profile: smoke_profile) -> Result<PathBuf, String> {
    let directory = repository_root()?.join("build");
    // Clear previous-run evidence once, before this attempt. Every artifact
    // produced by the current attempt survives a later rejection or timeout.
    for suffix in [
        "96M.log",
        "96M.stderr.log",
        "320M.log",
        "320M.stderr.log",
        "capture.log",
        "capture.stderr.log",
    ] {
        remove_optional_file(&directory.join(format!("qemu_{}_{suffix}", profile.log_prefix)))?;
    }
    for suffix in [".ppm", "-capture.json"] {
        remove_optional_file(&directory.join(format!("{}{suffix}", profile.artifact_prefix)))?;
    }
    Ok(directory)
}

pub(super) fn smoke_image(
    image: &Path,
    directory: &Path,
    profile: smoke_profile,
) -> Result<(), String> {
    let small = run_size(
        image,
        "96M",
        &directory.join(format!("qemu_{}_96M.log", profile.log_prefix)),
        profile,
    )?;
    let large = run_size(
        image,
        "320M",
        &directory.join(format!("qemu_{}_320M.log", profile.log_prefix)),
        profile,
    )?;
    verify_size_evidence(small, large)?;
    capture(image, directory, profile)?;
    println!(
        "{} verified two debug-exit boots and separate host-quit capture; free frames {} -> {}",
        profile.artifact_prefix, small.free_frames, large.free_frames
    );
    Ok(())
}

/// Reap on every error, including I/O failures after spawn. There are no pipe
/// reader threads and output goes to retained bounded diagnostic files.
struct child_guard(Child);
impl Drop for child_guard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn diagnostic_limits(paths: &[&Path]) -> Result<(), String> {
    for path in paths {
        match fs::metadata(path) {
            Ok(metadata) if metadata.len() > transcript_limit as u64 => {
                return Err(format!(
                    "{} exceeded the native application diagnostic limit",
                    path.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect {}: {error}", path.display())),
        }
    }
    Ok(())
}

fn wait_exit(child: &mut Child, deadline: Instant, paths: &[&Path]) -> Result<ExitStatus, String> {
    loop {
        diagnostic_limits(paths)?;
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("poll native application QEMU: {error}"))?
        {
            diagnostic_limits(paths)?;
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err("native application QEMU exceeded its 30-second process deadline".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn run_size(
    image: &Path,
    memory: &str,
    log: &Path,
    profile: smoke_profile,
) -> Result<run_evidence, String> {
    let stderr_path = log.with_extension("stderr.log");
    let stdout = File::create(log).map_err(|error| format!("create {}: {error}", log.display()))?;
    let stderr =
        File::create(&stderr_path).map_err(|error| format!("create QEMU diagnostics: {error}"))?;
    let mut command = qemu_command_with_memory(image, memory)?;
    command
        .args([
            "-cpu",
            "Nehalem",
            "-display",
            "none",
            "-monitor",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    let deadline = Instant::now() + timeout;
    let mut child = child_guard(
        command
            .spawn()
            .map_err(|error| format!("launch native application QEMU: {error}"))?,
    );
    let status = wait_exit(&mut child.0, deadline, &[log, &stderr_path])?;
    let transcript = read_bounded_optional_utf8_file(log, transcript_limit)?;
    let evidence = verify_output(status, &transcript, profile.verify)?;
    print!("{transcript}");
    println!(
        "{} {memory} debug-exit status33 verified; retained {}",
        profile.artifact_prefix,
        log.display()
    );
    Ok(evidence)
}

fn capture(image: &Path, directory: &Path, profile: smoke_profile) -> Result<(), String> {
    let log = directory.join(format!("qemu_{}_capture.log", profile.log_prefix));
    let stderr_path = directory.join(format!("qemu_{}_capture.stderr.log", profile.log_prefix));
    let frame_path = directory.join(format!("{}.ppm", profile.artifact_prefix));
    let receipt_path = directory.join(format!("{}-capture.json", profile.artifact_prefix));
    for path in [&log, &frame_path, &receipt_path] {
        remove_optional_file(path)?;
    }
    let monitor_path = frame_path.to_str().ok_or("screendump path is not UTF-8")?;
    if monitor_path.chars().any(char::is_control) {
        return Err("screendump path contains a control character".into());
    }
    let monitor_path = monitor_path.replace('\\', "\\\\").replace('"', "\\\"");
    let stderr = File::create(&stderr_path)
        .map_err(|error| format!("create capture diagnostics: {error}"))?;
    let deadline = Instant::now() + timeout;
    // No isa-debug-exit: the completed guest halts until the host captures and
    // explicitly quits QEMU. Host status0 is not a guest completion status.
    let mut child = child_guard(
        Command::new("qemu-system-x86_64")
            .current_dir(repository_root()?)
            .args(["-machine", "q35", "-cpu", "Nehalem", "-m", "96M"])
            .arg("-drive")
            .arg(format!("format=raw,file={}", image.display()))
            .args(["-no-reboot", "-no-shutdown", "-display", "none", "-serial"])
            .arg(format!("file:{}", log.display()))
            .args(["-monitor", "stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| format!("launch native application capture: {error}"))?,
    );
    loop {
        diagnostic_limits(&[&log, &stderr_path])?;
        let transcript = read_bounded_optional_utf8_file(&log, transcript_limit)?;
        if capture_ready(&transcript, profile.verify) {
            break;
        }
        require_capture_running(&mut child.0, deadline)?;
        thread::sleep(Duration::from_millis(10));
    }
    let monitor = child
        .0
        .stdin
        .as_mut()
        .ok_or("QEMU capture monitor input unavailable")?;
    writeln!(monitor, "screendump \"{monitor_path}\"")
        .and_then(|()| monitor.flush())
        .map_err(|error| format!("request native application screendump: {error}"))?;
    loop {
        diagnostic_limits(&[&log, &stderr_path])?;
        match fs::metadata(&frame_path) {
            Ok(metadata) if metadata.len() > ppm_size as u64 => {
                return Err("native application PPM exceeds fixed full-frame bound".into());
            }
            Ok(metadata) if metadata.len() == ppm_size as u64 => {
                (profile.verify_frame)(&read_frame(&frame_path)?)?;
                break;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect native application screendump: {error}")),
        }
        require_capture_running(&mut child.0, deadline)?;
        thread::sleep(Duration::from_millis(10));
    }
    let monitor = child
        .0
        .stdin
        .as_mut()
        .ok_or("QEMU capture monitor input unavailable")?;
    writeln!(monitor, "quit")
        .and_then(|()| monitor.flush())
        .map_err(|error| format!("quit native application capture: {error}"))?;
    let status = wait_exit(&mut child.0, deadline, &[&log, &stderr_path])?;
    if status.code() != Some(0) {
        return Err(format!(
            "native application host-quit capture exited with {status}"
        ));
    }
    (profile.verify)(&read_bounded_optional_utf8_file(&log, transcript_limit)?)?;
    (profile.verify_frame)(&read_frame(&frame_path)?)?;
    fs::write(&receipt_path, format!("{{\"schema\":1,\"boot_kind\":\"host_quit_capture\",\"isa_debug_exit\":false,\"host_exit_status\":0,\"transcript_records\":{},\"width\":1024,\"height\":768,\"verified_x\":32,\"verified_y\":32,\"verified_width\":384,\"verified_height\":24,\"verified_pixels\":9216}}\n", profile.records))
        .map_err(|error| format!("retain native application capture receipt: {error}"))?;
    println!(
        "native application public frame verified; retained {} and {}",
        frame_path.display(),
        log.display()
    );
    Ok(())
}

fn require_capture_running(child: &mut Child, deadline: Instant) -> Result<(), String> {
    if let Some(status) = child
        .try_wait()
        .map_err(|error| format!("poll capture QEMU: {error}"))?
    {
        return Err(format!("capture QEMU exited prematurely with {status}"));
    }
    if Instant::now() >= deadline {
        return Err("native application capture exceeded its 30-second process deadline".into());
    }
    Ok(())
}

pub(super) fn capture_ready(
    transcript: &str,
    verify: fn(&str) -> Result<run_evidence, String>,
) -> bool {
    transcript.ends_with('\n') && verify(transcript).is_ok()
}

fn read_frame(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("open native application PPM: {error}"))?;
    let mut bytes = Vec::with_capacity(ppm_size + 1);
    file.take((ppm_size + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read native application PPM: {error}"))?;
    Ok(bytes)
}

pub(super) fn verify_frame(bytes: &[u8]) -> Result<(), String> {
    let (_, expected) = reference()?;
    verify_frame_pixels(bytes, &expected)
}

pub(super) fn verify_frame_pixels(
    bytes: &[u8],
    expected: &[u8; frame_byte_count],
) -> Result<(), String> {
    if bytes.len() != ppm_size || !bytes.starts_with(ppm_header) {
        return Err("native application frame requires complete canonical P6 1024x768 RGB8".into());
    }
    let observed = &bytes[ppm_header.len()..];
    for y in 0..24 {
        for x in 0..384 {
            let source = ((y / 3) * 128 + x / 3) * 3;
            let destination = ((32 + y) * 1024 + 32 + x) * 3;
            if observed[destination..destination + 3] != expected[source..source + 3] {
                return Err(format!(
                    "native application frame differs at public fixture pixel ({x},{y})"
                ));
            }
        }
    }
    Ok(())
}

/// These checkpoint values are independent of model output and match the actual
/// production CodeMirror/editorBridge host trace. Rendering is shared, and is
/// therefore a cross-build/copy oracle rather than an independent font oracle.
fn reference() -> Result<([u64; 7], [u8; frame_byte_count]), String> {
    let mut state = editor_state::new();
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
            0 => (&b"const before = 1;"[..], (0, 0), 1, false, 0),
            1 => (&b"const before = 1;"[..], (6, 12), 1, false, 0),
            2..=4 => (&b"const after = 1;"[..], (11, 11), 2, true, 1),
            5 => (&b"const after = 1;"[..], (8, 8), 2, true, 1),
            6 => (&b"const before = 1;"[..], (6, 12), 3, false, 0),
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
        render(&state, &mut pixels);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct run_evidence {
    free_frames: u64,
    unavailable_frames: u64,
    total_frames: u64,
}

fn verify_output(
    status: ExitStatus,
    transcript: &str,
    verify: fn(&str) -> Result<run_evidence, String>,
) -> Result<run_evidence, String> {
    if status.code() != Some(qemu_success_status) {
        return Err(format!(
            "native application debug-exit boot did not report33: {status}"
        ));
    }
    verify(transcript)
}

pub(super) fn verify_transcript(transcript: &str) -> Result<(), String> {
    verify_records(transcript).map(|_| ())
}

pub(super) fn verify_records(transcript: &str) -> Result<run_evidence, String> {
    let mut records = Vec::new();
    for (index, line) in transcript.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('{') {
            return Err(format!(
                "unstructured native application line {}",
                index + 1
            ));
        }
        let record = json_parser::new(line)
            .parse_record()
            .map_err(|error| format!("malformed native application record: {error}"))?;
        let event = expected_events
            .get(records.len())
            .ok_or("extra native application record")?;
        require_string(&record, event, "event", event)?;
        require_string(&record, event, "level", "info")?;
        require_number(&record, event, "schema", 1)?;
        records.push(record);
    }
    if records.len() != expected_events.len() {
        return Err("missing native application records".into());
    }
    validate_scheduler_boot_records(&records[..8])?;
    let ready = &records[8];
    let event = expected_events[8];
    require_control_header(
        ready,
        event,
        &[
            "tasks",
            "scenarios",
            "cpu_profile",
            "cpu_vendor",
            "source_language",
            "code_pages",
            "data_frames",
            "table_frames",
            "channels",
        ],
    )?;
    for (field, value) in [
        ("tasks", 2),
        ("scenarios", 2),
        ("code_pages", 8),
        ("data_frames", 32),
        ("table_frames", 18),
        ("channels", 2),
    ] {
        require_number(ready, event, field, value)?;
    }
    require_string(ready, event, "cpu_profile", "x87_sse2")?;
    require_string(ready, event, "source_language", "rust")?;
    if !matches!(ready.get("cpu_vendor"), Some(json_scalar::string(value)) if matches!(value.as_str(), "GenuineIntel" | "AuthenticAMD" | "HygonGenuine"))
    {
        return Err("native application CPU vendor is not admitted".into());
    }
    for scenario in 0..2 {
        let record = &records[9 + scenario];
        let event = expected_events[9 + scenario];
        let ticks = required_number(record, event, "timer_ticks")?;
        if ticks > 10_000 {
            return Err("native application timer bound exceeded".into());
        }
        let normal = scenario == 0;
        validate_record(
            record,
            event,
            &[
                ("scenario", scenario as u64),
                ("calls", if normal { 112 } else { 12 }),
                ("cooperative_switches", if normal { 14 } else { 0 }),
                ("timer_ticks", ticks),
                ("surface_sends", if normal { 7 } else { 1 }),
                ("input_sends", if normal { 6 } else { 0 }),
                ("surface_deliveries", if normal { 7 } else { 0 }),
                ("input_deliveries", if normal { 6 } else { 0 }),
                ("surface_acks", if normal { 7 } else { 0 }),
                ("input_acks", if normal { 6 } else { 0 }),
                ("denials", if normal { 2 } else { 4 }),
                ("bad_copies", if normal { 2 } else { 1 }),
                ("invalid", u64::from(normal)),
                ("closed", if normal { 0 } else { 2 }),
                ("discarded", u64::from(!normal)),
                ("exits", if normal { 2 } else { 1 }),
                ("faults", u64::from(!normal)),
                ("rows", if normal { 24 } else { 0 }),
            ],
            &[],
        )?;
    }
    let (hashes, _) = reference()?;
    for (index, hash) in hashes.into_iter().enumerate() {
        validate_record(
            &records[11 + index],
            expected_events[11 + index],
            &[
                ("sequence", index as u64 + 1),
                ("public_fixture_hash", hash),
            ],
            &[],
        )?;
    }
    validate_record(
        &records[18],
        expected_events[18],
        &[
            ("x", 32),
            ("y", 32),
            ("width", 384),
            ("height", 24),
            ("frame_commits", 1),
        ],
        &[
            "framebuffer_verified",
            "outside_unchanged",
            "fault_preserved_frame",
            "stage_cleared",
        ],
    )?;
    let free_frames = required_number(&records[4], expected_events[4], "free_frames")?;
    if free_frames < 50 {
        return Err("native application boot frame budget too small".into());
    }
    validate_record(
        &records[19],
        expected_events[19],
        &[("free_frames", free_frames)],
        &[
            "resources_restored",
            "channel_storage_cleared",
            "cpu_state_cleared",
            "kernel_cpu_restored",
            "bootstrap_restored",
            "alias_removed",
        ],
    )?;
    validate_record(
        &records[20],
        expected_events[20],
        &[
            ("scenarios", 2),
            ("tasks", 4),
            ("exits", 3),
            ("faults", 1),
            ("surfaces", 7),
            ("inputs", 6),
        ],
        &[],
    )?;
    Ok(run_evidence {
        free_frames,
        unavailable_frames: required_number(&records[4], expected_events[4], "unavailable_frames")?,
        total_frames: required_number(&records[4], expected_events[4], "total_frames")?,
    })
}

fn validate_record(
    record: &structured_record,
    event: &str,
    numbers: &[(&str, u64)],
    booleans: &[&str],
) -> Result<(), String> {
    let fields = numbers
        .iter()
        .map(|(field, _)| *field)
        .chain(booleans.iter().copied())
        .collect::<Vec<_>>();
    require_control_header(record, event, &fields)?;
    for (field, value) in numbers {
        require_number(record, event, field, *value)?;
    }
    for field in booleans {
        require_bool(record, event, field, true)?;
    }
    Ok(())
}

fn verify_size_evidence(small: run_evidence, large: run_evidence) -> Result<(), String> {
    let delta = (320u64 - 96) * 1024 * 1024 / memory_frame_size;
    if small.total_frames != memory_total_frames
        || large.total_frames != memory_total_frames
        || large.free_frames.checked_sub(small.free_frames) != Some(delta)
        || small
            .unavailable_frames
            .checked_sub(large.unavailable_frames)
            != Some(delta)
    {
        return Err("native application RAM evidence lacks exact224MiB frame delta".into());
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(crate) fn valid_transcript() -> String {
        let mut lines = vec![
            r#"{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":32,"tss":true,"double_fault_ist":1,"nmi_ist":2,"nmi_masked":true}"#.to_owned(),
            r#"{"schema":1,"event":"boot.omen.accepted","level":"info","version":2,"boot_drive":128}"#.to_owned(),
            r#"{"schema":1,"event":"boot.memory.map","level":"info","regions":9,"usable_bytes":81920000}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":4096,"address_limit":4294967296,"total_frames":1048576,"free_frames":20000,"unavailable_frames":1028576,"data_owned_frames":0,"table_owned_frames":0}"#.to_owned(),
            r#"{"schema":1,"event":"boot.display.ready","level":"info","width":1024,"height":768,"pitch":3072,"bpp":24}"#.to_owned(),
            r#"{"schema":1,"event":"boot.ready","level":"info","control_abi":"0.1","framebuffer":true}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":32,"vector_count":16,"pit_input_hz":1193182,"pit_divisor":11932,"timebase_generation":1,"initial_master_mask":255,"initial_slave_mask":255,"master_mask":252,"slave_mask":255,"input_available":true,"input_source_generation":1,"input_queue_capacity":64,"nmi_masked":true,"if_enabled":true}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.native_application_test.ready","level":"info","tasks":2,"scenarios":2,"cpu_profile":"x87_sse2","cpu_vendor":"GenuineIntel","source_language":"rust","code_pages":8,"data_frames":32,"table_frames":18,"channels":2}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.native_application_test.scenario","level":"info","scenario":0,"calls":112,"cooperative_switches":14,"timer_ticks":3,"surface_sends":7,"input_sends":6,"surface_deliveries":7,"input_deliveries":6,"surface_acks":7,"input_acks":6,"denials":2,"bad_copies":2,"invalid":1,"closed":0,"discarded":0,"exits":2,"faults":0,"rows":24}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.native_application_test.scenario","level":"info","scenario":1,"calls":12,"cooperative_switches":0,"timer_ticks":0,"surface_sends":1,"input_sends":0,"surface_deliveries":0,"input_deliveries":0,"surface_acks":0,"input_acks":0,"denials":4,"bad_copies":1,"invalid":0,"closed":2,"discarded":1,"exits":1,"faults":1,"rows":0}"#.to_owned(),
        ];
        for (index, hash) in reference().unwrap().0.into_iter().enumerate() {
            lines.push(format!(r#"{{"schema":1,"event":"kernel.native_application_test.surface","level":"info","sequence":{},"public_fixture_hash":{hash}}}"#, index+1));
        }
        lines.extend([
            r#"{"schema":1,"event":"kernel.native_application_test.presented","level":"info","x":32,"y":32,"width":384,"height":24,"frame_commits":1,"framebuffer_verified":true,"outside_unchanged":true,"fault_preserved_frame":true,"stage_cleared":true}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.native_application_test.released","level":"info","free_frames":20000,"resources_restored":true,"channel_storage_cleared":true,"cpu_state_cleared":true,"kernel_cpu_restored":true,"bootstrap_restored":true,"alias_removed":true}"#.to_owned(),
            r#"{"schema":1,"event":"kernel.native_application_test.passed","level":"info","scenarios":2,"tasks":4,"exits":3,"faults":1,"surfaces":7,"inputs":6}"#.to_owned(),
        ]);
        lines.join("\n")
    }

    fn render_record(record: &structured_record) -> String {
        let fields = record
            .iter()
            .map(|(field, value)| {
                let value = match value {
                    json_scalar::string(value) => format!("{value:?}"),
                    json_scalar::number(value) => value.clone(),
                    json_scalar::boolean(value) => value.to_string(),
                    json_scalar::null => "null".into(),
                };
                format!("{field:?}:{value}")
            })
            .collect::<Vec<_>>();
        format!("{{{}}}", fields.join(","))
    }

    #[test]
    fn native_application_accepts_exact_lifecycle_and_actual_vendor_options() {
        let valid = valid_transcript();
        verify_transcript(&valid).unwrap();
        for vendor in ["AuthenticAMD", "HygonGenuine"] {
            verify_transcript(&valid.replace("GenuineIntel", vendor)).unwrap();
        }
        verify_transcript(&format!("\r\n{}\r\n", valid.replace('\n', "\r\n"))).unwrap();
        let reordered = valid
            .lines()
            .map(|line| render_record(&json_parser::new(line).parse_record().unwrap()))
            .collect::<Vec<_>>()
            .join("\n");
        verify_transcript(&reordered).unwrap();
        let (hashes, pixels) = reference().unwrap();
        assert!(hashes.iter().all(|hash| *hash != 0));
        assert_ne!(hashes[1], hashes[6]); // Same text/selection/theme, distinct revision/sequence.
        assert_eq!(&pixels[pixels.len() - 3..], &[0x14, 0x14, 0x14]);
    }

    #[test]
    fn native_application_rejects_missing_duplicate_reordered_and_extra_records() {
        let valid = valid_transcript();
        let lines = valid.lines().collect::<Vec<_>>();
        for index in 0..lines.len() {
            let mut changed = lines.clone();
            changed.remove(index);
            assert!(verify_transcript(&changed.join("\n")).is_err());
            let mut changed = lines.clone();
            changed.insert(index, lines[index]);
            assert!(verify_transcript(&changed.join("\n")).is_err());
            let mut changed = lines.clone();
            changed[index] = "{\"schema\":1,\"event\":\"unknown\",\"level\":\"info\"}";
            assert!(verify_transcript(&changed.join("\n")).is_err());
            if index + 1 < lines.len() {
                let mut changed = lines.clone();
                changed.swap(index, index + 1);
                assert!(verify_transcript(&changed.join("\n")).is_err());
            }
        }
        for extra in ["unexpected text", "{}", "{\"schema\":1", lines[20]] {
            assert!(verify_transcript(&format!("{valid}\n{extra}")).is_err());
        }
    }

    #[test]
    fn native_application_rejects_every_changed_missing_duplicate_or_extra_native_field() {
        let valid = valid_transcript();
        let lines = valid.lines().map(str::to_owned).collect::<Vec<_>>();
        for index in 8..lines.len() {
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
            for field in ["unknown", "private_payload", "task_identity", "address"] {
                let mut changed = record.clone();
                changed.insert(field.into(), json_scalar::number("42".into()));
                let mut records = lines.clone();
                records[index] = render_record(&changed);
                assert!(verify_transcript(&records.join("\n")).is_err());
            }
        }
    }

    #[test]
    fn native_application_binds_restoration_and_two_size_memory_delta() {
        let valid = valid_transcript();
        for (from, to) in [
            ("\"usable_bytes\":81920000", "\"usable_bytes\":81924096"),
            (
                "\"free_frames\":20000,\"resources_restored\"",
                "\"free_frames\":19999,\"resources_restored\"",
            ),
            (
                "\"unavailable_frames\":1028576",
                "\"unavailable_frames\":1028575",
            ),
        ] {
            let changed = valid.replace(from, to);
            assert_ne!(changed, valid);
            assert!(verify_transcript(&changed).is_err());
        }
        let small = verify_records(&valid).unwrap();
        let large = run_evidence {
            free_frames: 77344,
            unavailable_frames: 971232,
            ..small
        };
        verify_size_evidence(small, large).unwrap();
        for changed in [
            small,
            run_evidence {
                free_frames: large.free_frames + 1,
                ..large
            },
            run_evidence {
                unavailable_frames: large.unavailable_frames + 1,
                ..large
            },
            run_evidence {
                total_frames: large.total_frames + 1,
                ..large
            },
        ] {
            assert!(verify_size_evidence(small, changed).is_err());
        }
        assert!(verify_size_evidence(large, small).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn native_application_requires_debug_exit33_separate_from_capture_readiness() {
        use std::os::unix::process::ExitStatusExt;
        let valid = valid_transcript();
        verify_output(ExitStatus::from_raw(33 << 8), &valid, verify_records).unwrap();
        for code in [0, 1 << 8, 32 << 8, 35 << 8, 9] {
            assert!(verify_output(ExitStatus::from_raw(code), &valid, verify_records).is_err());
        }
        assert!(verify_output(ExitStatus::from_raw(33 << 8), "", verify_records).is_err());
        assert!(!capture_ready(&valid, verify_records));
        let complete = format!("{valid}\n");
        assert!(capture_ready(&complete, verify_records));
        for end in 0..complete.len() {
            assert!(!capture_ready(&complete[..end], verify_records));
        }
        assert!(!capture_ready(&format!("{complete}{{}}\n"), verify_records));
        assert!(!capture_ready(
            &complete.replace("\"surfaces\":7", "\"surfaces\":6"),
            verify_records
        ));
    }

    fn valid_frame() -> Vec<u8> {
        let mut frame = vec![0; ppm_size];
        frame[..ppm_header.len()].copy_from_slice(ppm_header);
        let expected = reference().unwrap().1;
        for y in 0..24 {
            for x in 0..384 {
                let source = ((y / 3) * 128 + x / 3) * 3;
                let destination = ppm_header.len() + ((32 + y) * 1024 + 32 + x) * 3;
                frame[destination..destination + 3].copy_from_slice(&expected[source..source + 3]);
            }
        }
        frame
    }

    #[test]
    fn native_application_frame_requires_exact_p6_bounds_and_every_region_pixel() {
        let mut frame = valid_frame();
        verify_frame(&frame).unwrap();
        for length in [0, ppm_header.len(), ppm_size - 1] {
            assert!(verify_frame(&frame[..length]).is_err());
        }
        frame.push(0);
        assert!(verify_frame(&frame).is_err());
        frame.pop();
        for offset in [0, 4, 8, ppm_header.len() - 2] {
            let old = frame[offset];
            frame[offset] ^= 1;
            assert!(verify_frame(&frame).is_err());
            frame[offset] = old;
        }
        for (x, y) in [(0, 0), (383, 0), (0, 23), (383, 23), (190, 12)] {
            let offset = ppm_header.len() + ((32 + y) * 1024 + 32 + x) * 3;
            frame[offset] ^= 1;
            assert!(verify_frame(&frame).is_err());
            frame[offset] ^= 1;
        }
        // The screenshot oracle intentionally claims only the fixed rectangle;
        // the guest separately checks outside-patch preservation against boot.
        frame[ppm_header.len()] ^= 1;
        verify_frame(&frame).unwrap();
    }

    #[test]
    fn native_application_feature_precedence_is_explicit() {
        assert!(!selected(&[]));
        assert!(selected(&[feature]));
        assert!(selected(&[
            feature,
            "ny_object_test",
            "application_profile_test",
            "owned_dispatcher_test",
            "owned_multi_region_test",
            "owned_source_test",
            "owned_tls_test",
            "dispatcher_test",
            "scheduler_test",
            "smoke_test",
            "workspace_dev",
            "owned_workspace_test",
            "owned_workspace_cpu_test",
        ]));
        for lower in [
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
            "elf_application_test",
            "linux_application_test",
        ] {
            assert!(!selected(&[feature, higher]));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_application_child_guard_reaps_a_deadline_failure() {
        let mut child = child_guard(Command::new("sleep").arg("20").spawn().unwrap());
        let pid = child.0.id();
        assert!(wait_exit(&mut child.0, Instant::now(), &[]).is_err());
        drop(child);
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
    }
}
