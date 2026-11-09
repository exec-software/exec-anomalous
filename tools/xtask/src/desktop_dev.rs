//! Opt-in bootstrap desktop development workflow. No native semantic service or
//! host publication is inferred from these interactive development events.

use super::*;

pub(super) const feature: &str = "desktop_dev";
const ready: &str = r#""event":"desktop_dev.ready""#;
const frame_length: usize = supervised_renderer_oracle::ppm_header.len() + 1024 * 768 * 3;
const timeout: Duration = Duration::from_secs(30);
const event_markers: &[&[u8]] = &[
    b"desktop_dev.ready",
    b"desktop_dev.shell_opened",
    b"desktop_dev.restored",
    b"desktop_dev.failed",
];

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|candidate| {
            matches!(
                *candidate,
                "workspace_dev"
                    | "fault_test"
                    | "circle_test"
                    | "control_test"
                    | "rite_test"
                    | "ny_vessel_test"
                    | "memory_test"
                    | "interrupt_test"
                    | "scheduler_test"
                    | dispatcher_smoke::feature
                    | owned_dispatcher_smoke::feature
                    | owned_multi_region_smoke::feature
                    | owned_source_smoke::feature
                    | owned_tls_smoke::feature
                    | application_profile_smoke::feature
                    | ny_object_smoke::feature
                    | native_application_smoke::feature
                    | native_runtime_smoke::feature
                    | native_document_smoke::feature
                    | elf_document_smoke::feature
                    | native_relaunch_smoke::feature
                    | "elf_application_test"
                    | "linux_application_test"
                    | "veil_application_test"
                    | "shared_veil_test"
                    | "linux_tls_test"
                    | "linux_loader_entry_test"
                    | "supervision_test"
                    | "shell_vessel_test"
                    | "input_wait_test"
                    | "display_vessel_test"
            )
        })
}

pub(super) fn build() -> Result<built_image, String> {
    let root = repository_root()?;
    let build = root.join("build");
    fs::create_dir_all(&build).map_err(|_| "create desktop development build directory")?;
    let host = semantic_presentation_host_run::prepare(&build)?;
    let image = build_image_with_context(
        &[semantic_surface_presentation_feature, feature],
        Some(&host),
    )?;
    if !host.transaction_is_pending() {
        return Err("desktop development build advanced its host fixture".into());
    }
    // Dropping the retained build fixture retires it without publication. The
    // development launcher consumes only its inspected public rendering body.
    drop(host);
    println!(
        "development desktop image inspected: {}",
        image.path.display()
    );
    Ok(image)
}

pub(super) fn inspect_image(root: &Path, elf: &Path, binary: &Path) -> Result<(), String> {
    let listing = run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.as_os_str().to_owned(),
        ],
    )?;
    let listing =
        String::from_utf8(listing).map_err(|_| "desktop image symbol listing is not UTF-8")?;
    if !listing.contains("desktop_dev") {
        return Err("desktop image lacks its development runtime symbols".into());
    }
    let bytes = fs::read(binary).map_err(|_| "read desktop image for inspection")?;
    for marker in event_markers {
        if !bytes.windows(marker.len()).any(|window| window == *marker) {
            return Err("desktop image lacks its development lifecycle markers".into());
        }
    }
    Ok(())
}

pub(super) fn verify_symbols_absent(listing: &str) -> Result<(), String> {
    if listing.contains("desktop_dev") {
        Err("normal kernel retains desktop development symbols".into())
    } else {
        Ok(())
    }
}

pub(super) fn verify_bytes_absent(bytes: &[u8]) -> Result<(), String> {
    if bytes
        .windows(b"desktop_dev".len())
        .any(|window| window == b"desktop_dev")
    {
        Err("normal kernel retains desktop development markers".into())
    } else {
        Ok(())
    }
}

fn interactive_machine(requested: Option<&str>, linux: bool) -> Result<&'static str, String> {
    match requested.unwrap_or("auto") {
        "auto" if linux => Ok("q35,accel=kvm:tcg"),
        "auto" | "tcg" => Ok("q35,accel=tcg"),
        "kvm" => Ok("q35,accel=kvm"),
        _ => Err("ANOMALOUS_QEMU_ACCEL must be auto, kvm, or tcg".into()),
    }
}

fn command(image: &Path, log: &Path, graphical: bool) -> Result<Command, String> {
    // The proof runners retain their existing software-emulation backend.
    // Interactive Linux runs try KVM first; QEMU falls back to TCG if the
    // hardware backend cannot initialize, with bounded private diagnostics.
    let machine = if graphical {
        let requested = match std::env::var("ANOMALOUS_QEMU_ACCEL") {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err("ANOMALOUS_QEMU_ACCEL must be UTF-8".into());
            }
        };
        interactive_machine(requested.as_deref(), cfg!(target_os = "linux"))?
    } else {
        "q35"
    };
    let mut command = Command::new("qemu-system-x86_64");
    command
        .current_dir(repository_root()?)
        .args(["-machine", machine, "-m", "256M", "-drive"])
        .arg(format!("format=raw,file={}", image.display()))
        .args(["-no-reboot", "-no-shutdown", "-serial"])
        .arg(if graphical {
            "stdio".to_string()
        } else {
            format!("file:{}", log.display())
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if graphical {
        println!("Interactive QEMU machine: {machine}");
        command
            .args(["-monitor", "none"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    } else {
        command
            .args(["-display", "none", "-monitor", "stdio"])
            .stdin(Stdio::piped());
    }
    Ok(command)
}

pub(super) fn run(workspace_guard: workspace_lock) -> Result<(), String> {
    let image = build()?;
    launch_image(
        &image.path,
        workspace_guard,
        "Anomalous Desktop Development",
        "Enter opens the bootstrap shell; type desktop to return. Close the QEMU window to stop.",
    )
}

pub(super) fn launch_image(
    image: &Path,
    workspace_guard: workspace_lock,
    title: &str,
    instructions: &str,
) -> Result<(), String> {
    let run_files = run_artifacts::prepare(image, &repository_root()?.join("build"))?;
    let serial_file = OpenOptions::new()
        .write(true)
        .open(&run_files.serial_log)
        .map_err(|_| "open private desktop serial diagnostics")?;
    let stderr_file = OpenOptions::new()
        .write(true)
        .open(&run_files.stderr_log)
        .map_err(|_| "open private desktop QEMU diagnostics")?;
    // The inspected disk is now a complete independent copy. Shared kernel,
    // wrapper and stable image artifacts may be rebuilt while this GUI lives.
    drop(workspace_guard);
    println!("{instructions}");
    println!(
        "Bounded serial diagnostics are private to {}",
        run_files.serial_log.display()
    );
    let child = command(&run_files.image, &run_files.serial_log, true)?
        .args(["-name", title])
        .spawn()
        .map_err(|_| "launch development desktop QEMU")?;
    let mut child = semantic_presentation_child::new(child);
    let stderr = child
        .child_mut()
        .stderr
        .take()
        .ok_or("desktop QEMU diagnostic stream missing")?;
    let diagnostics = thread::spawn(move || copy_bounded_diagnostics(stderr, stderr_file));
    let serial = child
        .child_mut()
        .stdout
        .take()
        .ok_or("desktop QEMU serial stream missing")?;
    let serial_writer = thread::spawn(move || copy_bounded_diagnostics(serial, serial_file));
    let status = child
        .child_mut()
        .wait()
        .map_err(|_| "wait for development desktop QEMU")?;
    diagnostics
        .join()
        .map_err(|_| "desktop QEMU diagnostics reader failed")?
        .map_err(|_| "write bounded private desktop QEMU diagnostics")?;
    serial_writer
        .join()
        .map_err(|_| "desktop serial reader failed")?
        .map_err(|_| "write bounded private desktop serial diagnostics")?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "development desktop QEMU failed; private diagnostics: {}",
            run_files.stderr_log.display()
        ))
    }
}

// Each graphical launch owns its image for exactly the child's lifetime.
// Logs remain private and separate after exit; subsequent launchers never
// unlink or truncate another live run's diagnostics.
struct run_artifacts {
    image: PathBuf,
    serial_log: PathBuf,
    stderr_log: PathBuf,
}

impl run_artifacts {
    fn prepare(inspected_image: &Path, build: &Path) -> Result<Self, String> {
        let runs = build.join("desktop-runs");
        fs::create_dir_all(&runs).map_err(|_| "create desktop run storage")?;
        let mut claimed = None;
        for attempt in 0..1024 {
            let path = runs.join(format!("run-{}-{attempt}", std::process::id()));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => {
                    claimed = Some(path);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err("claim private desktop run directory".into()),
            }
        }
        let directory = claimed.ok_or("private desktop run directory capacity exhausted")?;
        let artifacts = Self {
            image: directory.join("desktop-dev.img"),
            serial_log: directory.join("qemu_desktop_dev.log"),
            stderr_log: directory.join("qemu_desktop_dev_stderr.log"),
        };
        let mut source = File::open(inspected_image).map_err(|_| "open inspected desktop disk")?;
        if source
            .metadata()
            .map_err(|_| "inspect desktop disk extent")?
            .len()
            != disk_size
        {
            return Err("inspected desktop disk has an invalid extent".into());
        }
        create_private_capture_file(&artifacts.image)?;
        let mut target = OpenOptions::new()
            .write(true)
            .open(&artifacts.image)
            .map_err(|_| "open private desktop disk")?;
        if std::io::copy(&mut source, &mut target).map_err(|_| "copy inspected desktop disk")?
            != disk_size
        {
            return Err("private desktop disk copy has an invalid extent".into());
        }
        target
            .sync_all()
            .map_err(|_| "finish private desktop disk copy")?;
        create_private_capture_file(&artifacts.serial_log)?;
        create_private_capture_file(&artifacts.stderr_log)?;
        Ok(artifacts)
    }
}

impl Drop for run_artifacts {
    fn drop(&mut self) {
        // run() declares this owner before the child guard, so unwinding or an
        // early error reaps QEMU before this disk can be removed.
        let _ = fs::remove_file(&self.image);
    }
}

// Drain the child's diagnostics for its full lifetime while retaining at most
// 64 KiB. QEMU cannot block on a full stderr pipe or disclose text to stdout.
fn copy_bounded_diagnostics(mut input: impl Read, mut output: impl Write) -> std::io::Result<()> {
    let mut remaining = semantic_presentation_stderr_limit;
    let mut buffer = [0_u8; 8192];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            return output.flush();
        }
        let retained = count.min(remaining);
        output.write_all(&buffer[..retained])?;
        remaining -= retained;
    }
}

struct capture_artifacts {
    log_part: PathBuf,
    frame_part: PathBuf,
    log: PathBuf,
    initial: PathBuf,
    restored: PathBuf,
    published: bool,
}

impl capture_artifacts {
    fn new(build: &Path) -> Result<Self, String> {
        let artifacts = Self {
            log_part: build.join("qemu_desktop_dev_smoke.log.part"),
            frame_part: build.join("desktop-dev.ppm.part"),
            log: build.join("qemu_desktop_dev_smoke.log"),
            initial: build.join("desktop-dev.ppm"),
            restored: build.join("desktop-dev-restored.ppm"),
            published: false,
        };
        for path in artifacts.paths() {
            remove_optional_file(path)?;
        }
        create_private_capture_file(&artifacts.log_part)?;
        create_private_capture_file(&artifacts.frame_part)?;
        Ok(artifacts)
    }

    fn paths(&self) -> [&Path; 5] {
        [
            &self.log_part,
            &self.frame_part,
            &self.log,
            &self.initial,
            &self.restored,
        ]
    }

    fn publish(&mut self, transcript: &str, initial: &[u8], restored: &[u8]) -> Result<(), String> {
        // Only validated, public desktop frames are retained. Shell pixels are
        // never captured, including the private echo used by the smoke test.
        for path in [&self.log, &self.initial, &self.restored] {
            create_private_capture_file(path)?;
        }
        fs::write(&self.log, transcript)
            .and_then(|()| fs::write(&self.initial, initial))
            .and_then(|()| fs::write(&self.restored, restored))
            .map_err(|_| "publish verified desktop development artifacts")?;
        remove_optional_file(&self.log_part)?;
        remove_optional_file(&self.frame_part)?;
        self.published = true;
        Ok(())
    }
}

impl Drop for capture_artifacts {
    fn drop(&mut self) {
        for path in [&self.log_part, &self.frame_part] {
            let _ = fs::remove_file(path);
        }
        if !self.published {
            for path in [&self.log, &self.initial, &self.restored] {
                let _ = fs::remove_file(path);
            }
        }
    }
}

pub(super) fn smoke() -> Result<(), String> {
    let image = build()?;
    let payloads = image
        .semantic_surface
        .as_ref()
        .ok_or("desktop image payload metadata missing")?;
    let source_pages = payloads.source.file_size.div_ceil(circle_page_size);
    let renderer_pages = payloads.consumer.file_size.div_ceil(circle_page_size);
    let mut artifacts = capture_artifacts::new(&repository_root()?.join("build"))?;
    let child = command(&image.path, &artifacts.log_part, false)?
        .spawn()
        .map_err(|_| "launch desktop development smoke QEMU")?;
    let mut child = semantic_presentation_child::new(child);
    let result = exercise(child.child_mut(), &artifacts, source_pages, renderer_pages);
    child.terminate_and_reap();
    let (initial, restored) = result?;
    let transcript = read_bounded_optional_utf8_file(
        &artifacts.log_part,
        semantic_presentation_transcript_limit,
    )?;
    verify_transcript(
        &transcript,
        source_pages,
        renderer_pages,
        shell_records().len(),
    )
    .map_err(
        |_| "desktop development transcript rejected; unverified content and captures were removed",
    )?;
    artifacts.publish(&transcript, &initial, &restored)?;
    println!(
        "development desktop passed: edited command, private echo, two shell visits, exact restored public frames"
    );
    println!(
        "verified captures: {} and {}",
        artifacts.initial.display(),
        artifacts.restored.display()
    );
    Ok(())
}

fn exercise(
    child: &mut Child,
    artifacts: &capture_artifacts,
    source_pages: u64,
    renderer_pages: u64,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let deadline = Instant::now() + timeout;
    let mut monitor = child
        .stdin
        .take()
        .ok_or("desktop QEMU monitor input missing")?;
    wait(child, &artifacts.log_part, ready, deadline)?;
    let initial = capture(
        child,
        &mut monitor,
        artifacts,
        source_pages,
        renderer_pages,
        1,
        deadline,
    )?;
    type_monitor_named_key(&mut monitor, "ret")?;
    wait(
        child,
        &artifacts.log_part,
        bootstrap_shell_ready_event,
        deadline,
    )?;
    type_monitor_text(&mut monitor, "versiom")?;
    type_monitor_named_key(&mut monitor, "backspace")?;
    type_monitor_line(&mut monitor, "n")?;
    wait(
        child,
        &artifacts.log_part,
        bootstrap_shell_version_event,
        deadline,
    )?;
    type_monitor_line(&mut monitor, &format!("echo {privacy_probe}"))?;
    wait(
        child,
        &artifacts.log_part,
        bootstrap_shell_echo_event,
        deadline,
    )?;
    type_monitor_line(&mut monitor, "desktop")?;
    wait(
        child,
        &artifacts.log_part,
        r#""event":"desktop_dev.restored","level":"info","sequence":1"#,
        deadline,
    )?;
    let first_restored = capture(
        child,
        &mut monitor,
        artifacts,
        source_pages,
        renderer_pages,
        first_visit_records(),
        deadline,
    )?;
    if initial != first_restored {
        return Err("desktop restoration changed the public frame".into());
    }
    type_monitor_named_key(&mut monitor, "ret")?;
    wait(
        child,
        &artifacts.log_part,
        r#""event":"desktop_dev.shell_opened","level":"info","sequence":2"#,
        deadline,
    )?;
    type_monitor_line(&mut monitor, "version")?;
    // Sequence one occurs in each fresh shell. Wait for the complete expected
    // prefix rather than accepting the first visit's matching version event.
    wait_record_count(
        child,
        &artifacts.log_part,
        second_version_records(),
        deadline,
    )?;
    type_monitor_line(&mut monitor, "desktop")?;
    wait(
        child,
        &artifacts.log_part,
        r#""event":"desktop_dev.restored","level":"info","sequence":2"#,
        deadline,
    )?;
    let restored = capture(
        child,
        &mut monitor,
        artifacts,
        source_pages,
        renderer_pages,
        shell_records().len(),
        deadline,
    )?;
    if initial != restored {
        return Err("second desktop restoration changed the public frame".into());
    }
    ensure_child_running(child)?;
    Ok((initial, restored))
}

fn wait(child: &mut Child, log: &Path, marker: &str, deadline: Instant) -> Result<(), String> {
    loop {
        ensure_child_running(child)?;
        let transcript =
            read_bounded_optional_utf8_file(log, semantic_presentation_transcript_limit)?;
        if transcript.contains(privacy_probe)
            || transcript.contains("\"level\":\"fatal\"")
            || transcript.contains("desktop_dev.failed")
        {
            return Err(
                "desktop smoke received rejected diagnostics; content was not echoed".into(),
            );
        }
        if transcript
            .rfind("\r\n")
            .is_some_and(|end| transcript[..end].contains(marker))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("desktop smoke exceeded its bounded interaction deadline".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_record_count(
    child: &mut Child,
    log: &Path,
    tail_count: usize,
    deadline: Instant,
) -> Result<(), String> {
    loop {
        ensure_child_running(child)?;
        let transcript =
            read_bounded_optional_utf8_file(log, semantic_presentation_transcript_limit)?;
        if transcript.matches("\r\n").count() >= 15 + tail_count {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("desktop smoke shell command deadline exceeded".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[allow(clippy::too_many_arguments)]
fn capture(
    child: &mut Child,
    monitor: &mut ChildStdin,
    artifacts: &capture_artifacts,
    source_pages: u64,
    renderer_pages: u64,
    tail_count: usize,
    deadline: Instant,
) -> Result<Vec<u8>, String> {
    let transcript = read_bounded_optional_utf8_file(
        &artifacts.log_part,
        semantic_presentation_transcript_limit,
    )?;
    verify_transcript(&transcript, source_pages, renderer_pages, tail_count).map_err(
        |_| "desktop capture rejected its lifecycle; unverified content was not retained",
    )?;
    // Reset only this run's private staging file before asking QEMU to write.
    fs::write(&artifacts.frame_part, []).map_err(|_| "clear desktop capture staging file")?;
    writeln!(monitor, "screendump {}", artifacts.frame_part.display())
        .and_then(|()| monitor.flush())
        .map_err(|_| "request desktop frame capture")?;
    loop {
        ensure_child_running(child)?;
        let length = fs::metadata(&artifacts.frame_part)
            .map_err(|_| "inspect desktop frame capture")?
            .len();
        if length == frame_length as u64 {
            break;
        }
        if length > frame_length as u64 || Instant::now() >= deadline {
            return Err(
                "desktop frame capture has an invalid extent or exceeded its deadline".into(),
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
    let frame = fs::read(&artifacts.frame_part).map_err(|_| "read desktop frame capture")?;
    supervised_renderer_oracle::verify_ppm(&frame)
        .map_err(|_| "desktop pixels differ from the independent semantic oracle")?;
    Ok(frame)
}

fn verify_transcript(
    transcript: &str,
    source_pages: u64,
    renderer_pages: u64,
    tail_count: usize,
) -> Result<(), String> {
    if transcript.len() > semantic_presentation_transcript_limit
        || transcript.contains(privacy_probe)
    {
        return Err("desktop transcript violates bounded private diagnostics".into());
    }
    let body = transcript
        .strip_suffix("\r\n")
        .ok_or("desktop transcript lacks CRLF termination")?;
    let lines = body.split("\r\n").collect::<Vec<_>>();
    let shell = shell_records();
    if tail_count == 0 || tail_count > shell.len() || lines.len() != 15 + tail_count {
        return Err("desktop transcript has an incomplete or extra lifecycle".into());
    }
    let mut records = Vec::new();
    for (index, line) in lines[..15].iter().enumerate() {
        let fields = supervised_renderer_canonical_fields(line)?;
        let expected = match index {
            0..=6 => expected_supervised_renderer_serial_fields[index],
            7..=13 => semantic_presentation_feature_fields[index - 7],
            14 => expected_supervised_renderer_serial_fields[7],
            _ => unreachable!(),
        };
        if fields != expected {
            return Err("desktop boot/native record has noncanonical fields".into());
        }
        let record = json_parser::new(line).parse_record()?;
        let expected_event = match index {
            0..=6 => semantic_presentation_expected_events[index],
            7..=13 => semantic_presentation_expected_events[index + 1],
            14 => "kernel.interrupts.ready",
            _ => unreachable!(),
        };
        require_string(&record, "desktop lifecycle", "event", expected_event)?;
        require_string(&record, expected_event, "level", "info")?;
        require_number(&record, expected_event, "schema", 1)?;
        records.push(record);
    }
    validate_control_boot_records(&records[..7])?;
    validate_semantic_presentation_records(
        &records[4],
        &records[7..14],
        source_pages,
        renderer_pages,
    )?;
    validate_scheduler_interrupt_ready(&records[14])?;
    if lines[15..] != shell[..tail_count] {
        return Err("desktop shell lifecycle or private field mismatch".into());
    }
    Ok(())
}

fn shell_records() -> Vec<String> {
    // Exact public records for the bounded keyboard exercise; no command text
    // or arbitrary result strings enter this allowlist.
    let mut records = vec![r#"{"schema":1,"event":"desktop_dev.ready","level":"info","input_available":true,"activation":"enter","return_command":"desktop","width":1024,"height":768}"#.to_string()];
    for visit in 1..=2 {
        records.push(format!(
            r#"{{"schema":1,"event":"desktop_dev.shell_opened","level":"info","sequence":{visit}}}"#
        ));
        records.push(r#"{"schema":1,"event":"bootstrap_shell.ready","level":"info","input":"interrupt_ps2_set_1","input_available":true,"input_source_generation":1,"input_queue_capacity":64,"line_capacity":128}"#.to_string());
        records.push(
            r#"{"schema":1,"event":"bootstrap_shell.prompt","level":"debug","after_sequence":0}"#
                .to_string(),
        );
        append_command(&mut records, 1, "version");
        if visit == 1 {
            append_command(&mut records, 2, "echo");
        }
        append_return(&mut records, if visit == 1 { 3 } else { 2 });
        records.push(format!(
            r#"{{"schema":1,"event":"desktop_dev.restored","level":"info","sequence":{visit}}}"#
        ));
    }
    records
}

fn append_command(records: &mut Vec<String>, sequence: u64, kind: &str) {
    records.push(format!(r#"{{"schema":1,"event":"bootstrap_shell.command.received","level":"info","sequence":{sequence},"kind":"{kind}"}}"#));
    records.push(match kind {
        "version" => format!(r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"version","kernel":"0.1.0","control_abi":"0.1"}}"#),
        "echo" => format!(r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"echo","displayed":true,"content":"redacted"}}"#),
        _ => unreachable!(),
    });
    records.push(format!(r#"{{"schema":1,"event":"bootstrap_shell.command.completed","level":"info","sequence":{sequence},"kind":"{kind}","status":"ok"}}"#));
    records.push(format!(r#"{{"schema":1,"event":"bootstrap_shell.prompt","level":"debug","after_sequence":{sequence}}}"#));
}

fn first_visit_records() -> usize {
    16
}
fn second_version_records() -> usize {
    23
}

fn append_return(records: &mut Vec<String>, sequence: u64) {
    records.push(format!(r#"{{"schema":1,"event":"bootstrap_shell.command.received","level":"info","sequence":{sequence},"kind":"desktop"}}"#));
    records.push(format!(r#"{{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":{sequence},"kind":"desktop","returned":true}}"#));
    records.push(format!(r#"{{"schema":1,"event":"bootstrap_shell.command.completed","level":"info","sequence":{sequence},"kind":"desktop","status":"ok"}}"#));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_transcript(tail_count: usize) -> String {
        let terminal = crate::tests::valid_semantic_presentation_transcript();
        let records = terminal
            .trim_end_matches("\r\n")
            .split("\r\n")
            .collect::<Vec<_>>();
        // The test fixture explicitly models native presentation before IRQ
        // initialization. Production verification never rearranges records.
        let mut desktop = records[..7].to_vec();
        desktop.extend_from_slice(&records[8..]);
        desktop.push(records[7]);
        let tail = shell_records();
        desktop.extend(tail[..tail_count].iter().map(String::as_str));
        desktop.join("\r\n") + "\r\n"
    }

    #[test]
    fn desktop_verifier_checks_real_native_order_and_each_capture_boundary() {
        for count in [1, first_visit_records(), shell_records().len()] {
            verify_transcript(&valid_transcript(count), 3, 5, count)
                .expect("exact development lifecycle");
        }
        assert_eq!(
            shell_records()[first_visit_records() - 1],
            r#"{"schema":1,"event":"desktop_dev.restored","level":"info","sequence":1}"#
        );
        assert!(shell_records()[second_version_records() - 1].contains("after_sequence\":1"));
        assert!(
            verify_transcript(
                &crate::tests::valid_semantic_presentation_transcript(),
                3,
                5,
                1
            )
            .is_err()
        );
        assert!(verify_transcript(&valid_transcript(1), 2, 5, 1).is_err());
        assert!(verify_transcript(&valid_transcript(1), 3, 4, 1).is_err());
    }

    #[test]
    fn desktop_verifier_rejects_private_malformed_reordered_or_mutated_records() {
        let count = shell_records().len();
        let valid = valid_transcript(count);
        let mut mutations = vec![
            valid.replace("\r\n", "\n"),
            valid.replace("\"content\":\"redacted\"", "\"content\":\"privateprobe\""),
            valid.replace("\"rows\":768", "\"rows\":767"),
            valid.replace(
                "\"framebuffer_verified\":true",
                "\"framebuffer_verified\":false",
            ),
            valid.replace("\"returned\":true", "\"returned\":false"),
            valid.replace("\"after_sequence\":0", "\"after_sequence\":1"),
            valid.replace("\"status\":\"ok\"", "\"status\":\"bad\""),
            valid.replacen("\"schema\":1", "\"schema\":1,\"private\":\"sentinel\"", 1),
            valid.replacen("{", "{ ", 1),
            valid.replacen("desktop_dev.restored", "desktop_dev.failed", 1),
            valid.clone() + "{}\r\n",
        ];
        let mut reordered = valid
            .trim_end_matches("\r\n")
            .split("\r\n")
            .collect::<Vec<_>>();
        reordered.swap(7, 14);
        mutations.push(reordered.join("\r\n") + "\r\n");
        for mutated in mutations {
            assert!(verify_transcript(&mutated, 3, 5, count).is_err());
        }
        for removed in 0..(15 + count) {
            let mut lines = valid
                .trim_end_matches("\r\n")
                .split("\r\n")
                .collect::<Vec<_>>();
            lines.remove(removed);
            assert!(verify_transcript(&(lines.join("\r\n") + "\r\n"), 3, 5, count).is_err());
        }
    }

    #[test]
    fn desktop_is_opt_in_and_defers_to_terminal_proofs() {
        assert!(selected(&[feature, semantic_surface_presentation_feature]));
        assert!(!selected(&[]));
        assert!(all_kernel_features.contains(&feature));
        assert!(!selected(all_kernel_features));
        for conflict in [
            "fault_test",
            "circle_test",
            "control_test",
            "rite_test",
            "memory_test",
            "interrupt_test",
            "scheduler_test",
            dispatcher_smoke::feature,
            owned_dispatcher_smoke::feature,
            owned_multi_region_smoke::feature,
            owned_source_smoke::feature,
            owned_tls_smoke::feature,
            "supervision_test",
            "shell_vessel_test",
            "input_wait_test",
            "display_vessel_test",
        ] {
            assert!(!selected(&[
                feature,
                semantic_surface_presentation_feature,
                conflict
            ]));
        }
    }

    #[test]
    fn normal_exclusion_rejects_all_development_namespaces() {
        verify_symbols_absent("kernel_main T 1000").unwrap();
        verify_bytes_absent(b"ordinary bootstrap shell").unwrap();
        assert!(verify_symbols_absent("_ZN16anomalous_kernel11desktop_dev3runE").is_err());
        for marker in event_markers {
            assert!(verify_bytes_absent(marker).is_err());
        }
    }

    #[test]
    fn unverified_desktop_artifacts_are_removed_on_error() {
        let directory =
            std::env::temp_dir().join(format!("anomalous-desktop-capture-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let paths;
        {
            let artifacts = capture_artifacts::new(&directory).unwrap();
            paths = artifacts.paths().map(Path::to_path_buf);
            for path in &paths {
                fs::write(path, b"unverified-private-sentinel").unwrap();
            }
        }
        for path in paths {
            assert!(!path.exists());
        }
        fs::remove_dir(&directory).unwrap();
    }
    #[test]
    fn graphical_qemu_diagnostics_are_bounded_and_drained() {
        let source = vec![b'x'; semantic_presentation_stderr_limit * 2];
        let mut reader = std::io::Cursor::new(source);
        let mut output = Vec::new();
        copy_bounded_diagnostics(&mut reader, &mut output).unwrap();
        assert_eq!(output.len(), semantic_presentation_stderr_limit);
        assert_eq!(
            reader.position(),
            (semantic_presentation_stderr_limit * 2) as u64
        );
    }

    #[test]
    fn interactive_acceleration_prefers_kvm_with_explicit_software_fallback() {
        assert_eq!(
            interactive_machine(None, true).unwrap(),
            "q35,accel=kvm:tcg"
        );
        assert_eq!(
            interactive_machine(Some("auto"), true).unwrap(),
            "q35,accel=kvm:tcg"
        );
        assert_eq!(interactive_machine(None, false).unwrap(), "q35,accel=tcg");
        for linux in [true, false] {
            assert_eq!(
                interactive_machine(Some("tcg"), linux).unwrap(),
                "q35,accel=tcg"
            );
            assert_eq!(
                interactive_machine(Some("kvm"), linux).unwrap(),
                "q35,accel=kvm"
            );
            assert!(interactive_machine(Some(""), linux).is_err());
            assert!(interactive_machine(Some("kvm:other"), linux).is_err());
        }
    }
    #[test]
    fn graphical_run_copy_outlives_build_lock_and_isolates_concurrent_launchers() {
        let directory = std::env::temp_dir().join(format!(
            "anomalous-desktop-run-isolation-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let stable = directory.join("desktop-dev.img");
        let original = vec![0x37_u8; disk_size as usize];
        fs::write(&stable, &original).unwrap();
        let lock = acquire_workspace_lock_in(&directory).unwrap();
        assert!(acquire_workspace_lock_in(&directory).is_err());
        let first = run_artifacts::prepare(&stable, &directory).unwrap();
        fs::write(&first.serial_log, b"first run log").unwrap();
        drop(lock);
        // A second build may replace the stable disk while the first desktop
        // retains its inspected bytes and its own logging namespace.
        let next_lock = acquire_workspace_lock_in(&directory).unwrap();
        let replacement = vec![0x82_u8; disk_size as usize];
        fs::write(&stable, &replacement).unwrap();
        let second = run_artifacts::prepare(&stable, &directory).unwrap();
        drop(next_lock);
        assert_ne!(first.image, second.image);
        assert_eq!(fs::read(&first.image).unwrap(), original);
        assert_eq!(fs::read(&second.image).unwrap(), replacement);
        fs::write(&second.serial_log, b"second run log").unwrap();
        assert_eq!(fs::read(&first.serial_log).unwrap(), b"first run log");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(first.image.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            for path in [&first.image, &first.serial_log, &first.stderr_log] {
                assert_eq!(
                    fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
        let first_image = first.image.clone();
        let first_log = first.serial_log.clone();
        drop(first);
        assert!(!first_image.exists());
        assert_eq!(fs::read(first_log).unwrap(), b"first run log");
        assert!(second.image.exists());
        drop(second);
        fs::remove_dir_all(directory).unwrap();
    }
}
