//! Public client pixels checked through the native presentation path. Expected
//! commands are chosen here, independently of the vessel's keyboard adapter.
//! Pixel comparison uses the same compositor across host/native builds; the
//! compositor's separate pixel tests supply independent clipping evidence.
use super::*;
use anomalous_desktop_compositor::{composition_frame, surface_binding};
use anomalous_desktop_workspace::{
    client_endpoint, client_grants, client_origin, demo_content, desktop,
    desktop_command as command, desktop_control, launcher_choice, render_view, update_ticket,
    window_id,
};
use anomalous_window_surface::{
    canvas, demo_byte_count, demo_height, demo_stride, demo_width, draw_graphics_demo, frame,
    graphics_demo_label,
};

const keys: [&str; 25] = [
    "f1",
    "up",
    "up",
    "ret",
    "f3",
    "f3",
    "right",
    "shift-right",
    "x",
    "x",
    "t",
    "t",
    "t",
    "m",
    "r",
    "2",
    "f2",
    "c",
    "c",
    "c",
    "q",
    "f2",
    "f3",
    "1",
    "2",
];

#[derive(Clone, Copy)]
struct expected_surface {
    window: window_id,
    scene: usize,
    sequence: u64,
}

struct reference<'a> {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    sources: &'a [frame<'a>; 2],
    expected: Option<expected_surface>,
    retired: Option<window_id>,
}
impl<'a> reference<'a> {
    fn new(sources: &'a [frame<'a>; 2]) -> Result<(Self, update_ticket), String> {
        let (mut model, control, ticket) = desktop::new_demo(1024, 768).map_err(|e| e.code())?;
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .map_err(|e| e.code())?;
        Ok((
            Self {
                model,
                control,
                human,
                sources,
                expected: None,
                retired: None,
            },
            ticket,
        ))
    }

    fn view(&self) -> Result<render_view<'_>, String> {
        self.model.observe(&self.human).map_err(|e| e.code().into())
    }

    fn prepare(&mut self, key: &str) -> Result<update_ticket, String> {
        let view = self.view()?;
        let revision = view.revision();
        if key == "f3" {
            let expected = self
                .expected
                .ok_or("graphics replacement needs a live window")?;
            if view.focused_window().map(|window| window.id()) != Some(expected.window) {
                return Err("graphics replacement needs its focused window".into());
            }
            let owner = self
                .model
                .bind_surface(&self.control, &self.human, expected.window)
                .map_err(|e| e.code())?;
            let next = expected_surface {
                scene: 1 - expected.scene,
                sequence: expected.sequence + 1,
                ..expected
            };
            let ticket = self
                .model
                .prepare_surface(&owner, revision, self.sources[next.scene].descriptor())
                .map_err(|e| e.code())?;
            self.expected = Some(next);
            return Ok(ticket);
        }
        let action = match key {
            "f1" => command::toggle_launcher,
            "up" => command::select_launcher(match view.launcher_selection() {
                launcher_choice::welcome => launcher_choice::editor,
                launcher_choice::editor => launcher_choice::graphics,
                _ => return Err("graphics trace has an unexpected launcher selection".into()),
            }),
            "ret" => command::activate_launcher {
                choice: launcher_choice::graphics,
                graphics: Some(self.sources[0].descriptor()),
            },
            "f2" => command::open_graphics(self.sources[0].descriptor()),
            "x" => {
                let window = view
                    .focused_window()
                    .ok_or("graphics maximize needs focus")?;
                if window.maximized() {
                    command::restore_geometry(window.id())
                } else {
                    command::maximize(window.id())
                }
            }
            _ => smoke_command(key, &view)?,
        };
        let ticket = self
            .model
            .prepare(&self.human, revision, action)
            .map_err(|e| e.code())?;
        if matches!(key, "f2" | "ret") && self.expected.is_none() {
            let window = self
                .model
                .candidate(&ticket)
                .map_err(|e| e.code())?
                .all_windows()
                .find(|window| window.content() == demo_content::graphics)
                .ok_or("graphics open did not create a window")?
                .id();
            if let Some(retired) = self.retired
                && (window.slot() != retired.slot() || window.generation() <= retired.generation())
            {
                return Err("graphics close/reopen failed exact slot-generation reuse".into());
            }
            self.expected = Some(expected_surface {
                window,
                scene: 0,
                sequence: 1,
            });
        } else if let command::close(window) = action {
            if self.expected.map(|expected| expected.window) != Some(window) {
                return Err("graphics smoke closed the wrong window".into());
            }
            self.expected = None;
            self.retired = Some(window);
        }
        Ok(ticket)
    }

    fn frame(&self, ticket: &update_ticket) -> Result<Vec<u8>, String> {
        let view = self.model.candidate(ticket).map_err(|e| e.code())?;
        let mut bindings = Vec::new();
        for window in view
            .all_windows()
            .filter(|window| window.surface().is_some())
        {
            let expected = self
                .expected
                .ok_or("unexpected attached graphics content")?;
            let descriptor = self.sources[expected.scene].descriptor();
            if window.id() != expected.window
                || window.surface() != Some(descriptor)
                || window.surface_sequence() != expected.sequence
            {
                return Err(
                    "graphics candidate differs from expected content identity/sequence".into(),
                );
            }
            for projection in [view.machine(), view.accessibility()] {
                let projected = projection
                    .window(window.id())
                    .ok_or("graphics projection missing")?;
                if projected.surface() != window.surface()
                    || projected.surface_sequence() != window.surface_sequence()
                    || projected.bounds() != window.bounds()
                    || projected.minimized() != window.minimized()
                    || projected.workspace() != window.workspace()
                {
                    return Err(
                        "graphics semantic projections differ from the canonical view".into(),
                    );
                }
            }
            if descriptor.width() != demo_width
                || descriptor.height() != demo_height
                || descriptor.stride() != demo_stride
                || descriptor.label() != graphics_demo_label(expected.scene as u8)
            {
                return Err("graphics descriptor geometry/public description changed".into());
            }
            bindings.push(surface_binding::new(
                window.id(),
                &self.sources[expected.scene],
            ));
        }
        if bindings.len() != usize::from(self.expected.is_some()) {
            return Err("graphics surface count differs from expected lifecycle".into());
        }
        let composed = composition_frame::try_new(&view, &bindings).map_err(|e| e.code())?;
        let mut bytes = Vec::with_capacity(frame_length);
        bytes.extend_from_slice(b"P6\n1024 768\n255\n");
        let mut row = [0; 3072];
        for y in 0..768 {
            composed.compose_row(y, &mut row).map_err(|e| e.code())?;
            bytes.extend_from_slice(&row);
        }
        Ok(bytes)
    }

    fn publish(&mut self, ticket: update_ticket) -> Result<(), String> {
        self.model
            .begin(&self.control, &ticket)
            .map_err(|e| e.code())?;
        self.model
            .attest_presented(&self.control, &ticket)
            .map_err(|e| e.code())?;
        self.model
            .publish(&self.control, ticket)
            .map_err(|_| "publish graphics host reference")?;
        Ok(())
    }
}

fn draw(scene: u8) -> Result<Vec<u8>, String> {
    let mut pixels = vec![0; demo_byte_count];
    let mut target =
        canvas::try_new(demo_width, demo_height, demo_stride, &mut pixels).map_err(|e| e.code())?;
    draw_graphics_demo(&mut target, scene);
    Ok(pixels)
}

pub(super) fn run(
    root: &Path,
    image: &Path,
    pages: u64,
    profile: workspace_profile,
) -> Result<usize, String> {
    let directory = profile.directory(root, "workspace-graphics-smoke");
    fs::create_dir_all(&directory).map_err(|_| "create graphics smoke artifact directory")?;
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
        .map_err(|_| "launch native graphics smoke QEMU")?;
    let mut child = semantic_presentation_child::new(child);
    let result = (|| {
        let child = child.child_mut();
        let mut monitor = child.stdin.take().ok_or("graphics QEMU monitor missing")?;
        let deadline = Instant::now() + Duration::from_secs(90);
        let first = draw(0)?;
        let second = draw(1)?;
        let sources = [
            frame::try_new(
                demo_width,
                demo_height,
                demo_stride,
                graphics_demo_label(0),
                &first,
            )
            .map_err(|e| e.code())?,
            frame::try_new(
                demo_width,
                demo_height,
                demo_stride,
                graphics_demo_label(1),
                &second,
            )
            .map_err(|e| e.code())?,
        ];
        let (mut reference, mut ticket) = reference::new(&sources)?;
        let mut gradient = Vec::new();
        let mut chart = Vec::new();
        for sequence in 1..=keys.len() + 1 {
            wait_for(child, &files.log_part, &frame_record(sequence), deadline)?;
            profile.verify(
                &read_bounded_optional_utf8_file(&files.log_part, transcript_limit)?,
                pages,
                sequence,
                None,
            )?;
            let expected = reference.frame(&ticket)?;
            let captured = capture_frame(child, &mut monitor, &files, &expected, deadline)
                .map_err(|error| format!("graphics frame {sequence}: {error}"))?;
            if sequence == 4 {
                gradient = captured;
            } else if sequence == 5 {
                chart = captured;
            }
            reference.publish(ticket)?;
            if sequence > keys.len() {
                break;
            }
            let key = keys[sequence - 1];
            ticket = reference.prepare(key)?;
            type_monitor_named_key(&mut monitor, key)?;
        }
        type_monitor_named_key(&mut monitor, "esc")?;
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
        Ok::<_, String>((gradient, chart))
    })();
    child.terminate_and_reap();
    let (gradient, chart) = result?;
    let frames = keys.len() + 1;
    let transcript = read_bounded_optional_utf8_file(&files.log_part, transcript_limit)?;
    profile.verify(&transcript, pages, frames, Some("task_exit"))?;
    for (path, bytes) in [
        (&files.log, transcript.as_bytes()),
        (&files.initial, gradient.as_slice()),
        (&files.customized, chart.as_slice()),
    ] {
        create_private_capture_file(path)?;
        fs::write(path, bytes).map_err(|_| "retain verified public graphics artifact")?;
    }
    files.published = true;
    println!(
        "Graphics native exercise passed {frames} exact frames: immutable scene replacement, move/resize/maximize, tiling, minimization, workspace transfer, themes, close/reuse, canonical descriptor projections, and resource-restoring exit"
    );
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_graphics_sequence_preserves_content_through_window_lifecycle() {
        let first = draw(0).unwrap();
        let second = draw(1).unwrap();
        let sources = [
            frame::try_new(
                demo_width,
                demo_height,
                demo_stride,
                graphics_demo_label(0),
                &first,
            )
            .unwrap(),
            frame::try_new(
                demo_width,
                demo_height,
                demo_stride,
                graphics_demo_label(1),
                &second,
            )
            .unwrap(),
        ];
        let (mut reference, initial) = reference::new(&sources).unwrap();
        assert_eq!(reference.frame(&initial).unwrap().len(), frame_length);
        reference.publish(initial).unwrap();
        for key in keys {
            let ticket = reference.prepare(key).unwrap();
            let pixels = reference.frame(&ticket).unwrap();
            assert_eq!(pixels.len(), frame_length);
            reference.publish(ticket).unwrap();
        }
        let expected = reference.expected.unwrap();
        assert_eq!(expected.scene, 1);
        assert_eq!(expected.sequence, 2);
        let retired = reference.retired.unwrap();
        assert_eq!(expected.window.slot(), retired.slot());
        assert!(expected.window.generation() > retired.generation());
    }
}
