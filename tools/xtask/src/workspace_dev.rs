//! Native workspace development tooling. Artifact inspection, owned VM lifetimes,
//! and public-fixture captures are separate from desktop semantic authority.
use super::*;

#[path = "workspace_dev/editor_smoke.rs"]
mod editor_smoke;
#[path = "workspace_graphics_smoke.rs"]
mod graphics_smoke;
#[path = "workspace_pointer_smoke.rs"]
mod pointer_smoke;

pub(super) const feature: &str = "workspace_dev";
const package: &str = "anomalous-workspace-service";
const target_name: &str = "x86_64-anomalous-workspace";
const image_base: u64 = 0x0000_6000_0000_1000;
const image_capacity: u64 = 0x40000;
const data_base: u64 = 0x0000_6000_0008_1000;
const data_capacity: u64 = 0x7000;
const symbol_prefix: &str = "anomalous_workspace_service";

pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && !features.iter().any(|value| {
            matches!(
                *value,
                "owned_workspace_test"
                    | "native_editor_workspace_test"
                    | "owned_workspace_cpu_test"
                    | "dispatcher_test"
                    | "owned_dispatcher_test"
                    | "owned_multi_region_test"
                    | "owned_source_test"
                    | "owned_tls_test"
                    | "mixed_workspace_test"
                    | "fault_test"
                    | "ny_object_test"
                    | "native_application_test"
                    | "native_runtime_test"
                    | "native_program_test"
                    | "native_manifest_test"
                    | "native_durable_test"
                    | "native_durable_failure_test"
                    | "native_repeated_editor_test"
                    | "native_interactive_editor_test"
                    | "native_document_test"
                    | "elf_document_test"
                    | "native_relaunch_test"
                    | "elf_relaunch_test"
                    | "elf_application_test"
                    | "linux_application_test"
                    | "veil_application_test"
                    | "shared_veil_test"
                    | "linux_tls_test"
                    | "linux_loader_entry_test"
                    | "application_profile_test"
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
            )
        })
}

pub(super) struct artifact {
    pub(super) object: PathBuf,
    image: Vec<u8>,
    data: Vec<u8>,
    fault_offset: u64,
}

pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[feature], None)?;
    println!("native workspace image inspected: {}", image.path.display());
    Ok(image)
}

pub(super) fn run(guard: workspace_lock) -> Result<(), String> {
    let image = build()?;
    desktop_dev::launch_image(
        &image.path,
        guard,
        "Anomalous Workspace Development",
        "Applications / F1 opens the launcher; arrows or Tab select; Enter opens. F4 opens Editor Lab (memory-only). Tab changes window focus; Esc returns to shell only outside the editor/menu. P opens Appearance; T changes layout; desktop returns from shell.",
    )
}

pub(super) fn package_image(root: &Path, build: &Path) -> Result<artifact, String> {
    let target = root.join(format!("targets/{target_name}.json"));
    verify_semantic_surface_target_cfg(root, &target)?;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    run_checked(
        root,
        cargo,
        [
            OsString::from("-Zjson-target-spec"),
            OsString::from("-Zbuild-std=core,compiler_builtins"),
            OsString::from("-Zbuild-std-features=compiler-builtins-mem"),
            OsString::from("build"),
            OsString::from("--release"),
            OsString::from("--package"),
            OsString::from(package),
            OsString::from("--target"),
            target.into_os_string(),
            OsString::from("--locked"),
            OsString::from("--no-default-features"),
        ],
    )?;
    let cargo_elf = root
        .join("target")
        .join(target_name)
        .join("release")
        .join(package);
    let elf = build.join("workspace_service.elf");
    fs::copy(&cargo_elf, &elf).map_err(|_| "retain workspace service ELF")?;
    let bytes = fs::read(&elf).map_err(|_| "read workspace service ELF")?;
    let parsed = parse_elf64(&bytes)?;
    let (image, data) = inspect_segments(&bytes, &parsed)?;
    let listing = String::from_utf8(run_output_checked(
        root,
        "nm",
        [OsString::from("--format=posix"), elf.as_os_str().to_owned()],
    )?)
    .map_err(|_| "workspace symbol listing is not UTF-8")?;
    for line in listing.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.get(1) == Some(&"U") {
            return Err("workspace service has undefined symbols".into());
        }
    }
    let symbol =
        |suffix: &str| symbol_u64_from_listing(&listing, &format!("{symbol_prefix}_{suffix}"));
    if symbol("entry")? != image_base
        || symbol("image_start")? != image_base
        || symbol("image_payload_end")? != image_base + image.len() as u64
        || symbol("image_end")? != image_base + round_up_to_page(image.len() as u64)?
        || symbol("data_start")? != data_base
        || symbol("data_file_end")? != data_base + data.len() as u64
        || symbol("data_end")? != data_base + parsed.program_headers[1].memory_size
    {
        return Err("workspace ELF symbols disagree with inspected load extents".into());
    }
    let fault_offset = symbol("fault_instruction")?
        .checked_sub(image_base)
        .filter(|offset| *offset + 2 <= image.len() as u64)
        .ok_or("workspace controlled fault lies outside its executable payload")?;
    if image[fault_offset as usize..fault_offset as usize + 2] != [0x0f, 0x0b] {
        return Err("workspace controlled fault is not exact UD2".into());
    }
    fs::write(build.join("workspace_service.bin"), &image)
        .map_err(|_| "write workspace RX payload")?;
    fs::write(build.join("workspace_service_data.bin"), &data)
        .map_err(|_| "write workspace data payload")?;
    let source = format!(
        r#".section .rodata.workspace_service,"a",@progbits
.balign 16
.global x86_64_workspace_service_payload_start
.global x86_64_workspace_service_payload_end
.global x86_64_workspace_service_data_start
.global x86_64_workspace_service_data_end
.global x86_64_workspace_service_fault_instruction
x86_64_workspace_service_payload_start:
.incbin "workspace_service.bin"
x86_64_workspace_service_payload_end:
.set x86_64_workspace_service_fault_instruction, x86_64_workspace_service_payload_start + {fault_offset}
.balign 16
x86_64_workspace_service_data_start:
.incbin "workspace_service_data.bin"
x86_64_workspace_service_data_end:
.section .note.GNU-stack,"",@progbits
"#
    );
    let source_path = build.join("workspace_service_payload.S");
    let object = build.join("workspace_service_payload.o");
    fs::write(&source_path, source).map_err(|_| "write workspace payload wrapper")?;
    run_checked(
        build,
        "as",
        [
            OsString::from("--64"),
            OsString::from("-o"),
            object.as_os_str().to_owned(),
            source_path.as_os_str().to_owned(),
        ],
    )?;
    Ok(artifact {
        object,
        image,
        data,
        fault_offset,
    })
}

fn inspect_segments(bytes: &[u8], elf: &parsed_elf64) -> Result<(Vec<u8>, Vec<u8>), String> {
    if elf.kind != 2
        || elf.machine != 62
        || elf.entry != image_base
        || elf.program_headers.len() != 2
    {
        return Err(
            "workspace requires one static x86-64 RX segment and one private RW segment".into(),
        );
    }
    let rx = elf.program_headers[0];
    let rw = elf.program_headers[1];
    for (load, address, flags, capacity) in [
        (rx, image_base, 5, image_capacity),
        (rw, data_base, 6, data_capacity),
    ] {
        if load.kind != 1
            || load.flags != flags
            || load.virtual_address != address
            || load.physical_address != address
            || load.alignment != circle_page_size
            || load.memory_size > capacity
            || load.file_size > load.memory_size
            || load.offset % circle_page_size != address % circle_page_size
            || load
                .offset
                .checked_add(load.file_size)
                .is_none_or(|end| end > bytes.len() as u64)
        {
            return Err("workspace ELF load mapping violates W^X, geometry, or bounds".into());
        }
    }
    if rx.file_size == 0
        || rx.file_size != rx.memory_size
        || rx.offset + rx.file_size > rw.offset && rw.file_size != 0
    {
        return Err("workspace executable extent is empty, writable, or overlaps its data".into());
    }
    for section in &elf.section_headers {
        if section.size == 0 {
            continue;
        }
        if matches!(section.kind, 4 | 6 | 9 | 11)
            || section.flags & 0x400 != 0
            || section.name.starts_with(".got")
            || section.name.starts_with(".plt")
            || section.name == ".interp"
        {
            return Err(
                "workspace requires no relocations, dynamic imports, TLS, GOT or PLT".into(),
            );
        }
        if section.flags & 2 == 0 {
            continue;
        }
        let end = section
            .address
            .checked_add(section.size)
            .ok_or("workspace section extent overflow")?;
        let load = if section.flags & 1 != 0 { rw } else { rx };
        if section.address < load.virtual_address
            || end > load.virtual_address + load.memory_size
            || section.flags & 1 != 0 && section.flags & 4 != 0
            || !matches!(section.kind, 1 | 8)
            || section.kind == 8 && load.flags != 6
        {
            return Err(
                "workspace allocated section lies outside its checked protection domain".into(),
            );
        }
        if section.kind != 8 {
            let relative = section.address - load.virtual_address;
            if relative + section.size > load.file_size || section.offset != load.offset + relative
            {
                return Err(
                    "workspace allocated section is not exactly backed by its load bytes".into(),
                );
            }
        }
    }
    Ok((
        elf_virtual_bytes(bytes, rx, image_base, rx.file_size as usize)?.to_vec(),
        elf_virtual_bytes(bytes, rw, data_base, rw.file_size as usize)?.to_vec(),
    ))
}

pub(super) fn inspect_linked(root: &Path, elf: &Path, artifact: &artifact) -> Result<(), String> {
    let bytes = fs::read(elf).map_err(|_| "read linked workspace kernel")?;
    let parsed = parse_elf64(&bytes)?;
    let listing = String::from_utf8(run_output_checked(
        root,
        "nm",
        [
            OsString::from("--defined-only"),
            OsString::from("--format=posix"),
            elf.as_os_str().to_owned(),
        ],
    )?)
    .map_err(|_| "linked workspace symbols are not UTF-8")?;
    for (prefix, payload) in [
        ("x86_64_workspace_service_payload", &artifact.image),
        ("x86_64_workspace_service_data", &artifact.data),
    ] {
        let start = symbol_u64_from_listing(&listing, &format!("{prefix}_start"))?;
        let end = symbol_u64_from_listing(&listing, &format!("{prefix}_end"))?;
        if end.checked_sub(start) != Some(payload.len() as u64) {
            return Err("linked workspace payload changed extent".into());
        }
        parsed
            .section_headers
            .iter()
            .find(|s| {
                s.flags & 2 != 0
                    && s.flags & 1 == 0
                    && s.kind == 1
                    && start >= s.address
                    && end <= s.address + s.size
            })
            .ok_or("workspace payload is not in read-only kernel storage")?;
        let load = readonly_payload_load(&parsed, start, end)?;
        if elf_virtual_bytes(&bytes, load, start, payload.len())? != payload {
            return Err("linked workspace payload bytes changed".into());
        }
    }
    let start = symbol_u64_from_listing(&listing, "x86_64_workspace_service_payload_start")?;
    if symbol_u64_from_listing(&listing, "x86_64_workspace_service_fault_instruction")?
        != start + artifact.fault_offset
    {
        return Err("workspace controlled fault alias moved".into());
    }
    Ok(())
}

fn readonly_payload_load(
    elf: &parsed_elf64,
    start: u64,
    end: u64,
) -> Result<elf64_program_header, String> {
    let mut matching = elf.program_headers.iter().copied().filter(|load| {
        load.kind == 1
            && start >= load.virtual_address
            && load
                .virtual_address
                .checked_add(load.file_size)
                .is_some_and(|limit| end <= limit)
    });
    let load = matching
        .next()
        .ok_or("workspace payload has no backing kernel load")?;
    if start > end || load.flags != 5 || matching.next().is_some() {
        return Err("workspace payload requires one readable nonwritable kernel load".into());
    }
    Ok(load)
}

pub(super) fn verify_absent(symbols: &str, bytes: &[u8]) -> Result<(), String> {
    for marker in [
        "workspace_service",
        "workspace_dev",
        "anomalous_desktop_workspace",
        "anomalous_desktop_compositor",
        "anomalous_desktop_theme",
        "workspace_pointer",
    ] {
        if symbols.contains(marker) || bytes.windows(marker.len()).any(|s| s == marker.as_bytes()) {
            return Err("normal image retains native workspace implementation".into());
        }
    }
    Ok(())
}

// Launcher gestures add complete-frame records to the existing Appearance and
// pointer lifecycle run. Keep capture bounded without truncating that evidence.
const transcript_limit: usize = 128 * 1024;
const frame_length: usize = 16 + 1024 * 768 * 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum workspace_profile {
    legacy,
    owned,
    owned_cpu,
}
impl workspace_profile {
    fn cpu_arguments(self) -> &'static [&'static str] {
        if self == Self::owned_cpu {
            &["-accel", "tcg", "-cpu", "qemu64,vendor=GenuineIntel"]
        } else {
            &[]
        }
    }
    fn normalize(self, transcript: &str) -> Result<Option<String>, String> {
        if self == Self::owned_cpu {
            owned_workspace_cpu::normalize(transcript)
        } else {
            Ok(Some(transcript.to_owned()))
        }
    }
    fn directory(self, root: &Path, name: &str) -> PathBuf {
        match self {
            Self::legacy => root.join("build").join(name),
            Self::owned => root.join("build/owned-workspace/smoke").join(name),
            Self::owned_cpu => root.join("build/owned-workspace-cpu/smoke").join(name),
        }
    }
    fn verify(
        self,
        transcript: &str,
        pages: u64,
        frames: usize,
        fallback: Option<&str>,
    ) -> Result<(), String> {
        match self {
            Self::legacy => verify_transcript(transcript, pages, frames, fallback),
            Self::owned => owned_workspace::verify_transcript(transcript, pages, frames, fallback),
            Self::owned_cpu => {
                owned_workspace_cpu::verify_transcript(transcript, pages, frames, fallback)
            }
        }
    }
    fn pages(self, root: &Path) -> Result<u64, String> {
        match self {
            Self::owned | Self::owned_cpu => Ok(32),
            Self::legacy => Ok(fs::metadata(root.join("build/workspace_service.bin"))
                .map_err(|_| "inspect workspace payload extent")?
                .len()
                .div_ceil(4096)),
        }
    }
}

struct captures {
    profile: workspace_profile,
    log_part: PathBuf,
    frame_part: PathBuf,
    log: PathBuf,
    initial: PathBuf,
    customized: PathBuf,
    launcher_light: PathBuf,
    launcher_dark: PathBuf,
    appearance_light: PathBuf,
    appearance_dark: PathBuf,
    published: bool,
}
impl captures {
    #[cfg(test)]
    fn new(build: &Path) -> Result<Self, String> {
        Self::new_profile(build, workspace_profile::legacy)
    }
    fn new_profile(build: &Path, profile: workspace_profile) -> Result<Self, String> {
        let result = Self {
            profile,
            log_part: build.join("workspace_smoke.log.part"),
            frame_part: build.join("workspace_smoke.ppm.part"),
            log: build.join("workspace_smoke.log"),
            initial: build.join("workspace_initial.ppm"),
            customized: build.join("workspace_customized.ppm"),
            launcher_light: build.join("workspace_launcher_light.ppm"),
            launcher_dark: build.join("workspace_launcher_dark.ppm"),
            appearance_light: build.join("workspace_appearance_light.ppm"),
            appearance_dark: build.join("workspace_appearance_dark.ppm"),
            published: false,
        };
        for path in [
            &result.log_part,
            &result.frame_part,
            &result.log,
            &result.initial,
            &result.customized,
            &result.launcher_light,
            &result.launcher_dark,
            &result.appearance_light,
            &result.appearance_dark,
        ] {
            remove_optional_file(path)?;
        }
        create_private_capture_file(&result.log_part)?;
        create_private_capture_file(&result.frame_part)?;
        Ok(result)
    }
}
impl Drop for captures {
    fn drop(&mut self) {
        for path in [&self.log_part, &self.frame_part] {
            let _ = fs::remove_file(path);
        }
        if !self.published {
            for path in [
                &self.log,
                &self.initial,
                &self.customized,
                &self.launcher_light,
                &self.launcher_dark,
                &self.appearance_light,
                &self.appearance_dark,
            ] {
                let _ = fs::remove_file(path);
            }
        }
    }
}

fn ready_record(pages: u64) -> String {
    format!(
        r#"{{"schema":1,"event":"workspace_service.ready","level":"info","image_pages":{pages},"data_pages":8,"stack_pages":16,"stage_frames":576,"rights":30}}"#
    )
}
fn pointer_record(generation: u32) -> String {
    format!(
        r#"{{"schema":1,"event":"workspace_pointer.ready","level":"info","available":true,"generation":{generation}}}"#
    )
}
fn frame_record(sequence: usize) -> String {
    format!(
        r#"{{"schema":1,"event":"workspace_service.frame_presented","level":"info","sequence":{sequence},"rows":768,"width":1024,"height":768}}"#
    )
}
pub(super) fn fallback_records(frames: usize, reason: &str) -> Vec<String> {
    vec![
        format!(r#"{{"schema":1,"event":"workspace_service.withdrawn","level":"info","reason":"{reason}","frames_presented":{frames},"resources_restored":true}}"#),
        r#"{"schema":1,"event":"bootstrap_shell.ready","level":"info","input":"interrupt_ps2_set_1","input_available":true,"input_source_generation":1,"input_queue_capacity":64,"line_capacity":128}"#.into(),
        r#"{"schema":1,"event":"bootstrap_shell.prompt","level":"debug","after_sequence":0}"#.into(),
        r#"{"schema":1,"event":"bootstrap_shell.command.received","level":"info","sequence":1,"kind":"version"}"#.into(),
        r#"{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":1,"kind":"version","kernel":"0.1.0","control_abi":"0.1"}"#.into(),
        r#"{"schema":1,"event":"bootstrap_shell.command.completed","level":"info","sequence":1,"kind":"version","status":"ok"}"#.into(),
        r#"{"schema":1,"event":"bootstrap_shell.prompt","level":"debug","after_sequence":1}"#.into(),
    ]
}
pub(super) fn verify_transcript(
    transcript: &str,
    pages: u64,
    frames: usize,
    fallback: Option<&str>,
) -> Result<(), String> {
    if transcript.len() > transcript_limit || frames == 0 || pages == 0 || pages > 64 {
        return Err("workspace transcript exceeds its bounded lifecycle".into());
    }
    let body = transcript
        .strip_suffix("\r\n")
        .ok_or("workspace transcript is not CRLF terminated")?;
    let lines = body.split("\r\n").collect::<Vec<_>>();
    let tail = fallback
        .map(|reason| fallback_records(frames, reason))
        .unwrap_or_default();
    if lines.len() != 10 + frames + tail.len() {
        return Err("workspace transcript has missing or unexpected records".into());
    }
    let mut records = Vec::new();
    for (index, line) in lines[..8].iter().enumerate() {
        if supervised_renderer_canonical_fields(line)?
            != expected_supervised_renderer_serial_fields[index]
        {
            return Err("workspace boot record fields are noncanonical".into());
        }
        let record = json_parser::new(line).parse_record()?;
        require_string(
            &record,
            "workspace boot",
            "event",
            semantic_presentation_expected_events[index],
        )?;
        require_string(&record, "workspace boot", "level", "info")?;
        require_number(&record, "workspace boot", "schema", 1)?;
        records.push(record);
    }
    validate_control_boot_records(&records[..7])?;
    validate_scheduler_interrupt_ready(&records[7])?;
    if lines[8] != pointer_record(1) || lines[9] != ready_record(pages) {
        return Err("workspace service readiness differs from inspected payload".into());
    }
    for sequence in 1..=frames {
        if lines[9 + sequence] != frame_record(sequence) {
            return Err("workspace frame sequence is not exact and complete".into());
        }
    }
    if lines[10 + frames..] != tail {
        return Err("workspace withdrawal or shell fallback differs from its contract".into());
    }
    Ok(())
}

fn wait_for(child: &mut Child, log: &Path, marker: &str, deadline: Instant) -> Result<(), String> {
    loop {
        ensure_child_running(child)?;
        let text = read_bounded_optional_utf8_file(log, transcript_limit)?;
        if text.contains("\"level\":\"fatal\"") || text.contains("workspace_service.failed") {
            return Err(
                "workspace runtime failed; unverified diagnostics and captures removed".into(),
            );
        }
        if text
            .rfind("\r\n")
            .is_some_and(|end| text[..end].contains(marker))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("workspace smoke interaction deadline exceeded".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn reference_frame(view: &anomalous_desktop_workspace::render_view<'_>) -> Result<Vec<u8>, String> {
    let mut frame = Vec::with_capacity(frame_length);
    frame.extend_from_slice(b"P6\n1024 768\n255\n");
    let mut row = [0_u8; 3072];
    for y in 0..768 {
        anomalous_desktop_compositor::compose_row(view, y, &mut row)
            .map_err(|_| "compose host reference row")?;
        frame.extend_from_slice(&row);
    }
    Ok(frame)
}
fn capture_frame(
    child: &mut Child,
    monitor: &mut ChildStdin,
    files: &captures,
    expected: &[u8],
    deadline: Instant,
) -> Result<Vec<u8>, String> {
    fs::write(&files.frame_part, []).map_err(|_| "clear workspace capture staging")?;
    writeln!(monitor, "screendump {}", files.frame_part.display())
        .and_then(|()| monitor.flush())
        .map_err(|_| "request public workspace capture")?;
    loop {
        ensure_child_running(child)?;
        let size = fs::metadata(&files.frame_part)
            .map_err(|_| "inspect workspace capture")?
            .len();
        if size == frame_length as u64 {
            break;
        }
        if size > frame_length as u64 || Instant::now() >= deadline {
            return Err("workspace capture extent or deadline rejected".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    let frame = fs::read(&files.frame_part).map_err(|_| "read public workspace capture")?;
    if frame != expected {
        return Err("native workspace frame differs from the host cross-build reference".into());
    }
    Ok(frame)
}

pub(super) fn editor_smoke() -> Result<(), String> {
    editor_smoke::smoke()
}
pub(super) fn editor_smoke_image(
    image: &built_image,
    profile: workspace_profile,
) -> Result<(), String> {
    editor_smoke::smoke_image(image, profile)
}

pub(super) fn smoke() -> Result<(), String> {
    smoke_image(&build()?, workspace_profile::legacy)
}

pub(super) fn smoke_image(image: &built_image, profile: workspace_profile) -> Result<(), String> {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop};
    let root = repository_root()?;
    let pages = profile.pages(&root)?;
    let directory = profile.directory(&root, "");
    fs::create_dir_all(&directory).map_err(|_| "create workspace smoke directory")?;
    let mut files = captures::new_profile(&directory, profile)?;
    let child = Command::new("qemu-system-x86_64")
        .args(profile.cpu_arguments())
        .current_dir(&root)
        .args(["-machine", "q35", "-m", "256M", "-drive"])
        .arg(format!("format=raw,file={}", image.path.display()))
        .args(["-no-reboot", "-no-shutdown", "-display", "none", "-serial"])
        .arg(format!("file:{}", files.log_part.display()))
        .args(["-monitor", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "launch native workspace smoke QEMU")?;
    let mut child = semantic_presentation_child::new(child);
    let result = (|| {
        let child = child.child_mut();
        let mut monitor = child.stdin.take().ok_or("workspace QEMU monitor missing")?;
        let deadline = Instant::now() + Duration::from_secs(90);
        let (mut model, control, mut ticket) =
            desktop::new_demo(1024, 768).map_err(|e| e.code())?;
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .map_err(|e| e.code())?;
        let mut initial = Vec::new();
        let mut customized = Vec::new();
        let keys = [
            "tab",
            "t",
            "c",
            "d",
            "t",
            "t",
            "right",
            "shift-right",
            "m",
            "r",
            "2",
            "1",
            "n",
            "w",
            "2",
            "q",
            "1",
            "c",
            "g",
            "a",
        ];
        for sequence in 1..=keys.len() + 1 {
            wait_for(child, &files.log_part, &frame_record(sequence), deadline)?;
            let transcript = read_bounded_optional_utf8_file(&files.log_part, transcript_limit)?;
            profile.verify(&transcript, pages, sequence, None)?;
            let expected = reference_frame(&model.candidate(&ticket).map_err(|e| e.code())?)?;
            let captured = capture_frame(child, &mut monitor, &files, &expected, deadline)
                .map_err(|error| format!("frame {sequence}: {error}"))?;
            if sequence == 1 {
                initial = captured.clone();
            }
            if sequence == 5 {
                customized = captured;
            }
            model.begin(&control, &ticket).map_err(|e| e.code())?;
            model
                .attest_presented(&control, &ticket)
                .map_err(|e| e.code())?;
            model
                .publish(&control, ticket)
                .map_err(|_| "publish host reference")?;
            if sequence > keys.len() {
                break;
            }
            let view = model.observe(&human).map_err(|e| e.code())?;
            let command = smoke_command(keys[sequence - 1], &view)?;
            ticket = model
                .prepare(&human, view.revision(), command)
                .map_err(|e| e.code())?;
            type_monitor_named_key(&mut monitor, keys[sequence - 1])?;
        }
        type_monitor_named_key(&mut monitor, "f12")?;
        wait_for(
            child,
            &files.log_part,
            bootstrap_shell_ready_event,
            deadline,
        )?;
        type_monitor_line(&mut monitor, "version")?;
        wait_for(
            child,
            &files.log_part,
            r#""event":"bootstrap_shell.prompt","level":"debug","after_sequence":1"#,
            deadline,
        )?;
        Ok::<_, String>((initial, customized, keys.len() + 1))
    })();
    child.terminate_and_reap();
    let (initial, customized, frames) = result?;
    let transcript = read_bounded_optional_utf8_file(&files.log_part, transcript_limit)?;
    profile.verify(&transcript, pages, frames, Some("controlled_fault"))?;
    for (path, bytes) in [
        (&files.log, transcript.as_bytes()),
        (&files.initial, initial.as_slice()),
        (&files.customized, customized.as_slice()),
    ] {
        create_private_capture_file(path)?;
        fs::write(path, bytes).map_err(|_| "retain verified public workspace artifact")?;
    }
    terminal_smoke(&root, &image.path, pages, "watchdog", "f11", profile)?;
    terminal_smoke(&root, &image.path, pages, "task_exit", "esc", profile)?;
    terminal_smoke(&root, &image.path, pages, "user_fault", "f10", profile)?;
    terminal_smoke(
        &root,
        &image.path,
        pages,
        "user_fault",
        "shift-f12",
        profile,
    )?;
    let pointer_frames = pointer_smoke::run(&root, &image.path, pages, profile)?;
    let graphics_frames = graphics_smoke::run(&root, &image.path, pages, profile)?;
    files.published = true;
    println!(
        "native workspace passed: {frames} exact keyboard frames, {pointer_frames} pointer/Appearance frames, {graphics_frames} graphics surface frames, release-activated window/theme controls, drag-away and keyboard/chord cancellation, stale slot-generation rejection, Appearance pointer/keyboard choices and reuse, immutable RGB8 replacement, clipping and window lifecycle, two desktop sessions, controlled fault, watchdog, malformed context/mailbox, orderly exit, full resource restoration and working shell fallback"
    );
    Ok(())
}
fn terminal_smoke(
    root: &Path,
    image: &Path,
    pages: u64,
    reason: &str,
    key: &str,
    profile: workspace_profile,
) -> Result<(), String> {
    let directory = profile.directory(root, &format!("workspace-{reason}-{key}-smoke"));
    fs::create_dir_all(&directory).map_err(|_| "create workspace termination smoke directory")?;
    let mut files = captures::new_profile(&directory, profile)?;
    let child = Command::new("qemu-system-x86_64")
        .args(profile.cpu_arguments())
        .current_dir(root)
        .args(["-machine", "q35", "-m", "256M", "-drive"])
        .arg(format!("format=raw,file={}", image.display()))
        .args(["-no-reboot", "-no-shutdown", "-display", "none", "-serial"])
        .arg(format!("file:{}", files.log_part.display()))
        .args(["-monitor", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "launch workspace termination smoke")?;
    let mut child = semantic_presentation_child::new(child);
    let result = (|| {
        let child = child.child_mut();
        let mut monitor = child.stdin.take().ok_or("workspace monitor missing")?;
        let deadline = Instant::now() + Duration::from_secs(30);
        wait_for(child, &files.log_part, &frame_record(1), deadline)?;
        profile.verify(
            &read_bounded_optional_utf8_file(&files.log_part, transcript_limit)?,
            pages,
            1,
            None,
        )?;
        type_monitor_named_key(&mut monitor, key)?;
        wait_for(
            child,
            &files.log_part,
            bootstrap_shell_ready_event,
            deadline,
        )?;
        type_monitor_line(&mut monitor, "version")?;
        wait_for(
            child,
            &files.log_part,
            r#""event":"bootstrap_shell.prompt","level":"debug","after_sequence":1"#,
            deadline,
        )
    })();
    child.terminate_and_reap();
    result?;
    let transcript = read_bounded_optional_utf8_file(&files.log_part, transcript_limit)?;
    profile.verify(&transcript, pages, 1, Some(reason))?;
    create_private_capture_file(&files.log)?;
    fs::write(&files.log, transcript)
        .map_err(|_| "retain verified workspace termination transcript")?;
    files.published = true;
    Ok(())
}

fn smoke_command(
    key: &str,
    view: &anomalous_desktop_workspace::render_view<'_>,
) -> Result<anomalous_desktop_workspace::desktop_command, String> {
    use anomalous_desktop_workspace::{
        density_mode, desktop_command as command, layout_mode, theme_mode, workspace_id,
    };
    let focused = || {
        view.focused_window()
            .ok_or("smoke requires a focused window")
    };
    Ok(match key {
        "tab" => command::focus(view.next_focus().ok_or("smoke focus missing")?),
        "t" => command::set_layout(match view.layout() {
            layout_mode::floating => layout_mode::horizontal,
            layout_mode::horizontal => layout_mode::vertical,
            layout_mode::vertical => layout_mode::floating,
        }),
        "c" => command::set_theme(match view.theme() {
            theme_mode::light => theme_mode::dark,
            theme_mode::dark => theme_mode::high_contrast,
            theme_mode::high_contrast => theme_mode::light,
        }),
        "g" => command::set_depth(!view.depth()),
        "a" => command::set_reduced_motion(!view.reduced_motion()),
        "n" => command::create {
            content: anomalous_desktop_workspace::demo_content::activity,
            workspace: view.active_workspace(),
            bounds: anomalous_desktop_workspace::window_rect::try_new(120, 140, 480, 320)
                .map_err(|e| e.code())?,
        },
        "q" => command::close(focused()?.id()),
        "w" => command::move_to_workspace {
            window: focused()?.id(),
            workspace: match view.active_workspace() {
                workspace_id::first => workspace_id::second,
                workspace_id::second => workspace_id::first,
            },
        },
        "d" => command::set_density(match view.density() {
            density_mode::comfortable => density_mode::compact,
            density_mode::compact => density_mode::comfortable,
        }),
        "right" => {
            let w = focused()?;
            command::move_window {
                window: w.id(),
                x: w.bounds().x() + 16,
                y: w.bounds().y(),
            }
        }
        "shift-right" => {
            let w = focused()?;
            command::resize_window {
                window: w.id(),
                width: w.bounds().width() + 16,
                height: w.bounds().height(),
            }
        }
        "m" => command::minimize(focused()?.id()),
        "r" => command::restore(
            view.all_windows()
                .find(|w| w.minimized() && w.workspace() == view.active_workspace())
                .ok_or("smoke minimized window missing")?
                .id(),
        ),
        "2" => command::switch_workspace(workspace_id::second),
        "1" => command::switch_workspace(workspace_id::first),
        _ => return Err("unknown workspace smoke control".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript(frames: usize, fallback: Option<&str>) -> String {
        let boot = crate::tests::valid_semantic_presentation_transcript();
        let mut lines = boot
            .split("\r\n")
            .take(8)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        lines.push(pointer_record(1));
        lines.push(ready_record(12));
        lines.extend((1..=frames).map(frame_record));
        if let Some(reason) = fallback {
            lines.extend(fallback_records(frames, reason));
        }
        lines.join("\r\n") + "\r\n"
    }
    #[test]
    fn transcript_requires_exact_frames_and_completed_withdrawal() {
        for reason in [
            None,
            Some("controlled_fault"),
            Some("watchdog"),
            Some("task_exit"),
        ] {
            let valid = transcript(13, reason);
            verify_transcript(&valid, 12, 13, reason).unwrap();
            for bad in [
                valid.replace("\r\n", "\n"),
                valid.replace("\"sequence\":2", "\"sequence\":1"),
                valid.replace("\"rows\":768", "\"rows\":767"),
                valid.replace("\"rights\":30", "\"rights\":31"),
                valid.replace("\"generation\":1", "\"generation\":2"),
                valid.replace("\"available\":true", "\"available\":false"),
                valid.replacen("\"schema\":1", "\"schema\":1,\"private\":\"sentinel\"", 1),
                valid.clone() + "{}\r\n",
            ] {
                assert!(verify_transcript(&bad, 12, 13, reason).is_err());
            }
            for removed in 0..valid.lines().count() {
                let mut lines = valid
                    .trim_end_matches("\r\n")
                    .split("\r\n")
                    .collect::<Vec<_>>();
                lines.remove(removed);
                assert!(verify_transcript(&(lines.join("\r\n") + "\r\n"), 12, 13, reason).is_err());
            }
            assert!(verify_transcript(&valid, 11, 13, reason).is_err());
            assert!(verify_transcript(&valid, 12, 12, reason).is_err());
        }
        assert!(
            verify_transcript(
                &transcript(1, Some("controlled_fault"))
                    .replace("resources_restored\":true", "resources_restored\":false"),
                12,
                1,
                Some("controlled_fault")
            )
            .is_err()
        );
    }
    fn elf_fixture() -> (Vec<u8>, parsed_elf64) {
        let bytes = vec![0_u8; 0x2010];
        let load = |flags, offset, address, file_size, memory_size| elf64_program_header {
            kind: 1,
            flags,
            offset,
            virtual_address: address,
            physical_address: address,
            file_size,
            memory_size,
            alignment: 4096,
        };
        let section = |name: &str, flags, address, offset, size, kind| elf64_section_header {
            name: name.into(),
            kind,
            flags,
            address,
            offset,
            size,
        };
        (
            bytes,
            parsed_elf64 {
                kind: 2,
                machine: 62,
                entry: image_base,
                program_headers: vec![
                    load(5, 0x1000, image_base, 64, 64),
                    load(6, 0x2000, data_base, 16, 32),
                ],
                section_headers: vec![
                    section(".text", 6, image_base, 0x1000, 64, 1),
                    section(".data", 3, data_base, 0x2000, 16, 1),
                    section(".bss", 3, data_base + 16, 0x2010, 16, 8),
                ],
            },
        )
    }
    #[test]
    fn artifact_inspection_rejects_permission_mapping_and_backing_mutations() {
        let (bytes, valid) = elf_fixture();
        assert_eq!(
            inspect_segments(&bytes, &valid).unwrap(),
            (vec![0; 64], vec![0; 16])
        );
        let mut mutations = Vec::new();
        for which in 0..2 {
            for flags in [0, 4, 7] {
                let mut e = valid.clone();
                e.program_headers[which].flags = flags;
                mutations.push(e);
            }
            let mut e = valid.clone();
            e.program_headers[which].physical_address += 4096;
            mutations.push(e);
            let mut e = valid.clone();
            e.program_headers[which].offset = u64::MAX;
            mutations.push(e);
            let mut e = valid.clone();
            e.program_headers[which].memory_size = 0x40001;
            mutations.push(e);
        }
        for which in 0..3 {
            let mut e = valid.clone();
            e.section_headers[which].address = u64::MAX;
            mutations.push(e);
            let mut e = valid.clone();
            e.section_headers[which].flags |= 0x400;
            mutations.push(e);
            let mut e = valid.clone();
            e.section_headers[which].kind = 4;
            mutations.push(e);
        }
        let mut e = valid.clone();
        e.section_headers[1].flags |= 4;
        mutations.push(e);
        let mut e = valid.clone();
        e.section_headers[0].offset += 1;
        mutations.push(e);
        let mut e = valid.clone();
        e.section_headers[1].name = ".got".into();
        mutations.push(e);
        let mut e = valid.clone();
        e.program_headers[0].file_size = 0;
        mutations.push(e);
        for e in mutations {
            assert!(inspect_segments(&bytes, &e).is_err(), "{e:?}");
        }
    }
    #[test]
    fn linked_payload_requires_unique_readable_nonwritable_load() {
        let (_, valid) = elf_fixture();
        assert!(readonly_payload_load(&valid, image_base, image_base + 64).is_ok());
        for flags in [0, 1, 2, 3, 6, 7] {
            let mut changed = valid.clone();
            changed.program_headers[0].flags = flags;
            assert!(readonly_payload_load(&changed, image_base, image_base + 64).is_err());
        }
        let mut changed = valid.clone();
        changed.program_headers.push(changed.program_headers[0]);
        assert!(readonly_payload_load(&changed, image_base, image_base + 64).is_err());
    }
    #[test]
    fn workspace_is_opt_in_excluded_from_normal_and_defers_to_other_proofs() {
        assert!(selected(&[feature]));
        assert!(selected(&[feature, "scheduler_test"]));
        assert!(!selected(&[]));
        assert!(!selected(all_kernel_features));
        for conflict in all_kernel_features
            .iter()
            .filter(|value| !matches!(**value, "workspace_dev" | "scheduler_test" | "smoke_test"))
        {
            assert!(!selected(&[feature, conflict]));
        }
        for marker in [
            "workspace_dev",
            "workspace_service",
            "anomalous_desktop_workspace",
            "anomalous_desktop_compositor",
            "anomalous_desktop_theme",
            "workspace_pointer",
        ] {
            assert!(verify_absent(marker, &[]).is_err());
            assert!(verify_absent("", marker.as_bytes()).is_err());
        }
        assert!(verify_absent("bootstrap_shell", b"Threshold").is_ok());
    }
}
