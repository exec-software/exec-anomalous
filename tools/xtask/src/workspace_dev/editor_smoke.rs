//! Physical keyboard editing checked against literal semantic checkpoints.
//! This oracle never calls the vessel's key or pointer adapters. Full frames
//! are cross-build transport evidence; compositor unit tests check pixels
//! independently of this shared host compositor.
use super::*;
use anomalous_desktop_compositor::{chrome_bounds, chrome_target};
use anomalous_desktop_workspace::{
    caret_motion, client_endpoint, client_grants, client_origin, demo_content, desktop,
    desktop_command as command, desktop_control, editor_action as action, editor_feedback,
    launcher_choice, theme_mode, update_ticket, window_id,
};
use anomalous_editor_contract::editor_error;

#[derive(Clone, Copy, Debug)]
struct expected_editor {
    text: &'static [u8],
    selection: (usize, usize),
    revision: u64,
    undo: u8,
    dirty: bool,
    feedback: editor_feedback,
}

const fn state(
    text: &'static [u8],
    selection: (usize, usize),
    revision: u64,
    undo: u8,
    feedback: editor_feedback,
) -> expected_editor {
    expected_editor {
        text,
        selection,
        revision,
        undo,
        // This public scratch fixture starts empty and has no save service.
        // No action in this oracle is allowed to change that empty baseline.
        dirty: !text.is_empty(),
        feedback,
    }
}

const empty: expected_editor = state(b"", (0, 0), 1, 0, editor_feedback::ready);

fn verify_editor(
    view: &anomalous_desktop_workspace::render_view<'_>,
    expected: Option<expected_editor>,
) -> Result<(), String> {
    let editors = view
        .all_windows()
        .filter(|window| window.content() == demo_content::editor)
        .collect::<Vec<_>>();
    let Some(expected) = expected else {
        return if editors.is_empty() {
            Ok(())
        } else {
            Err("literal editor oracle expected no editor window".into())
        };
    };
    if editors.len() != 1 {
        return Err("literal editor oracle requires exactly one editor incarnation".into());
    }
    let actual = editors[0]
        .editor()
        .ok_or("editor window has no semantic editor view")?;
    if actual.text() != expected.text
        || actual.selection() != expected.selection
        || actual.revision() != expected.revision
        || actual.undo_depth() != expected.undo
        || actual.is_dirty() != expected.dirty
        || actual.feedback() != expected.feedback
    {
        return Err(format!(
            "literal editor checkpoint differs: expected {expected:?}; actual text {:?}, selection {:?}, revision {}, undo {}, dirty {}, feedback {:?}",
            actual.text(),
            actual.selection(),
            actual.revision(),
            actual.undo_depth(),
            actual.is_dirty(),
            actual.feedback(),
        ));
    }
    Ok(())
}

struct native_io<'a> {
    child: &'a mut Child,
    monitor: ChildStdin,
    files: &'a captures,
    pages: u64,
    deadline: Instant,
}

struct driver<'a> {
    io: Option<native_io<'a>>,
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    frames: usize,
    expected: Option<expected_editor>,
    buttons: u8,
}

impl<'a> driver<'a> {
    fn new(io: Option<native_io<'a>>) -> Result<Self, String> {
        let (mut model, control, initial) = desktop::new_demo(1024, 768).map_err(|e| e.code())?;
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .map_err(|e| e.code())?;
        let mut result = Self {
            io,
            model,
            control,
            human,
            frames: 0,
            expected: None,
            buttons: 0,
        };
        result.present(initial)?;
        Ok(result)
    }

    fn view(&self) -> Result<anomalous_desktop_workspace::render_view<'_>, String> {
        self.model.observe(&self.human).map_err(|e| e.code().into())
    }

    fn editor(&self) -> Result<window_id, String> {
        self.view()?
            .all_windows()
            .find(|window| window.content() == demo_content::editor)
            .map(|window| window.id())
            .ok_or_else(|| "editor oracle lost its window".into())
    }

    fn present(&mut self, ticket: update_ticket) -> Result<Vec<u8>, String> {
        let view = self.model.candidate(&ticket).map_err(|e| e.code())?;
        verify_editor(&view, self.expected)?;
        self.frames += 1;
        let capture = if let Some(io) = &mut self.io {
            wait_for(
                io.child,
                &io.files.log_part,
                &frame_record(self.frames),
                io.deadline,
            )?;
            io.files.profile.verify(
                &read_bounded_optional_utf8_file(&io.files.log_part, transcript_limit)?,
                io.pages,
                self.frames,
                None,
            )?;
            let expected = reference_frame(&view)?;
            capture_frame(io.child, &mut io.monitor, io.files, &expected, io.deadline)
                .map_err(|error| format!("editor frame {}: {error}", self.frames))?
        } else {
            // Host policy tests exercise the literal trace without compiling,
            // launching a guest, or repeatedly rasterizing whole screens.
            Vec::new()
        };
        self.model
            .begin(&self.control, &ticket)
            .map_err(|e| e.code())?;
        self.model
            .attest_presented(&self.control, &ticket)
            .map_err(|e| e.code())?;
        self.model
            .publish(&self.control, ticket)
            .map_err(|_| "publish editor reference frame")?;
        Ok(capture)
    }

    fn key(
        &mut self,
        key: &str,
        command: command,
        expected: Option<expected_editor>,
    ) -> Result<Vec<u8>, String> {
        let revision = self.view()?.revision();
        let ticket = self
            .model
            .prepare(&self.human, revision, command)
            .map_err(|e| e.code())?;
        self.expected = expected;
        if let Some(io) = &mut self.io {
            type_monitor_named_key(&mut io.monitor, key)?;
        }
        self.present(ticket)
    }

    fn edit(
        &mut self,
        key: &str,
        action: action,
        expected: expected_editor,
    ) -> Result<Vec<u8>, String> {
        let window = self.editor()?;
        if self.view()?.focused_window().map(|value| value.id()) != Some(window) {
            return Err("physical editor action expected editor focus".into());
        }
        self.key(key, command::edit_editor { window, action }, Some(expected))
    }

    fn ignored(&mut self, keys: &[&str]) -> Result<(), String> {
        if let Some(io) = &mut self.io {
            for key in keys {
                type_monitor_named_key(&mut io.monitor, key)?;
            }
        }
        // The next accepted key is an ordered processing barrier. Its exact
        // frame count and literal state expose any effect from these keys; an
        // arbitrary sleep alone would not prove the guest consumed them.
        Ok(())
    }

    fn packet(&mut self, physical: &str, commands: &[command]) -> Result<(), String> {
        let revision = self.view()?.revision();
        let ticket = self
            .model
            .prepare_batch(&self.human, revision, commands)
            .map_err(|e| e.code())?;
        if let Some(io) = &mut self.io {
            writeln!(io.monitor, "{physical}")
                .and_then(|()| io.monitor.flush())
                .map_err(|_| "send editor physical pointer packet")?;
        }
        self.present(ticket)?;
        Ok(())
    }

    fn target(&mut self, target: chrome_target) -> Result<(), String> {
        let bounds = chrome_bounds(&self.view()?, target).ok_or("editor control has no bounds")?;
        let (x, y) = (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
        loop {
            let position = self.view()?.pointer();
            let dx = (i64::from(x) - i64::from(position.x())).clamp(-64, 64) as i32;
            let dy = (i64::from(y) - i64::from(position.y())).clamp(-64, 64) as i32;
            if dx == 0 && dy == 0 {
                return Ok(());
            }
            let x = (i64::from(position.x()) + i64::from(dx)).clamp(0, 1023) as u32;
            let y = (i64::from(position.y()) + i64::from(dy)).clamp(0, 767) as u32;
            self.packet(
                &format!("mouse_move {dx} {dy}"),
                &[
                    command::set_pointer { x, y },
                    command::set_pointer_buttons(self.buttons),
                ],
            )?;
        }
    }

    fn button(&mut self, buttons: u8, effect: command) -> Result<(), String> {
        let position = self.view()?.pointer();
        let (x, y) = (position.x(), position.y());
        self.buttons = buttons;
        self.packet(
            &format!("mouse_button {buttons}"),
            &[
                command::set_pointer { x, y },
                command::set_pointer_buttons(buttons),
                effect,
            ],
        )
    }

    fn close_editor(&mut self) -> Result<window_id, String> {
        let window = self.editor()?;
        let target = chrome_target::close(window);
        self.target(target)?;
        self.button(1, command::arm_pointer(target))?;
        self.expected = None;
        self.button(0, command::close(window))?;
        if self.view()?.window(window).is_some() {
            return Err("closed editor incarnation remains live".into());
        }
        Ok(window)
    }

    fn withdraw(&mut self) -> Result<(), String> {
        if self.editor().is_ok() || self.view()?.launcher_open() {
            return Err("editor proof must close editor and launcher before shell exit".into());
        }
        if let Some(io) = &mut self.io {
            type_monitor_named_key(&mut io.monitor, "esc")?;
            wait_for(
                io.child,
                &io.files.log_part,
                bootstrap_shell_ready_event,
                io.deadline,
            )?;
            type_monitor_line(&mut io.monitor, "version")?;
            wait_for(
                io.child,
                &io.files.log_part,
                r#""event":"bootstrap_shell.prompt","level":"debug","after_sequence":1"#,
                io.deadline,
            )?;
        }
        Ok(())
    }
}

fn exercise(driver: &mut driver<'_>) -> Result<(Vec<u8>, Vec<u8>), String> {
    use editor_feedback as feedback;
    let initial = driver.key("f4", command::open_editor, Some(empty))?;
    let original = driver.editor()?;
    for (index, (key, byte, text)) in [
        ("c", b'c', b"c".as_slice()),
        ("t", b't', b"ct".as_slice()),
        ("n", b'n', b"ctn".as_slice()),
        ("p", b'p', b"ctnp".as_slice()),
        ("q", b'q', b"ctnpq".as_slice()),
        ("w", b'w', b"ctnpqw".as_slice()),
        ("1", b'1', b"ctnpqw1".as_slice()),
        ("2", b'2', b"ctnpqw12".as_slice()),
        ("shift-a", b'A', b"ctnpqw12A".as_slice()),
        ("shift-1", b'!', b"ctnpqw12A!".as_slice()),
        ("spc", b' ', b"ctnpqw12A! ".as_slice()),
        ("minus", b'-', b"ctnpqw12A! -".as_slice()),
        ("shift-minus", b'_', b"ctnpqw12A! -_".as_slice()),
    ]
    .into_iter()
    .enumerate()
    {
        driver.edit(
            key,
            action::insert(byte),
            state(
                text,
                (text.len(), text.len()),
                index as u64 + 2,
                1,
                if index == 0 {
                    feedback::edited
                } else {
                    feedback::history_replaced
                },
            ),
        )?;
    }
    for (key, motion, extend, selection) in [
        ("home", caret_motion::start, false, (0, 0)),
        ("right", caret_motion::right, false, (1, 1)),
        ("shift-right", caret_motion::right, true, (1, 2)),
    ] {
        driver.edit(
            key,
            action::move_caret { motion, extend },
            state(
                b"ctnpqw12A! -_",
                selection,
                14,
                1,
                feedback::selection_changed,
            ),
        )?;
    }
    driver.edit(
        "shift-x",
        action::insert(b'X'),
        state(b"cXnpqw12A! -_", (2, 2), 15, 1, feedback::history_replaced),
    )?;
    driver.edit(
        "backspace",
        action::backspace,
        state(b"cnpqw12A! -_", (1, 1), 16, 1, feedback::history_replaced),
    )?;
    driver.edit(
        "delete",
        action::delete_forward,
        state(b"cpqw12A! -_", (1, 1), 17, 1, feedback::history_replaced),
    )?;
    driver.edit(
        "end",
        action::move_caret {
            motion: caret_motion::end,
            extend: false,
        },
        state(b"cpqw12A! -_", (11, 11), 17, 1, feedback::selection_changed),
    )?;
    driver.edit(
        "shift-home",
        action::move_caret {
            motion: caret_motion::start,
            extend: true,
        },
        state(b"cpqw12A! -_", (11, 0), 17, 1, feedback::selection_changed),
    )?;
    driver.edit(
        "z",
        action::insert(b'z'),
        state(b"z", (1, 1), 18, 1, feedback::history_replaced),
    )?;
    driver.edit(
        "ctrl-z",
        action::undo,
        state(b"cpqw12A! -_", (11, 0), 19, 0, feedback::undone),
    )?;
    driver.edit(
        "ctrl-z",
        action::undo,
        state(
            b"cpqw12A! -_",
            (11, 0),
            19,
            0,
            feedback::rejected(editor_error::no_undo),
        ),
    )?;
    driver.edit(
        "esc",
        action::collapse_selection,
        state(b"cpqw12A! -_", (0, 0), 19, 0, feedback::selection_changed),
    )?;
    driver.edit(
        "ctrl-s",
        action::save_unavailable,
        state(b"cpqw12A! -_", (0, 0), 19, 0, feedback::save_unavailable),
    )?;
    let protected = state(b"cpqw12A! -_", (0, 0), 19, 0, feedback::unsupported_key);
    for key in [
        "ret",
        "f2",
        "f3",
        "f10",
        "f11",
        "f12",
        "shift-f12",
        "ctrl-tab",
    ] {
        driver.edit(key, action::unsupported_key, protected)?;
    }
    driver.key("f1", command::toggle_launcher, Some(protected))?;
    driver.ignored(&["q", "f4", "f12"])?;
    driver.key(
        "down",
        command::select_launcher(launcher_choice::inspector),
        Some(protected),
    )?;
    driver.key("esc", command::dismiss_launcher, Some(protected))?;
    driver.key("f4", command::open_editor, Some(protected))?;
    if driver.editor()? != original {
        return Err("F4 replaced an existing editor incarnation".into());
    }
    let next = driver
        .view()?
        .next_focus()
        .ok_or("editor oracle has no alternate focus")?;
    driver.key("tab", command::focus(next), Some(protected))?;
    driver.key("c", command::set_theme(theme_mode::dark), Some(protected))?;
    driver.key("f4", command::open_editor, Some(protected))?;
    let retired = driver.close_editor()?;
    driver.key("f4", command::open_editor, Some(empty))?;
    if driver.editor()? == retired {
        return Err("editor close/reopen reused a stale window incarnation".into());
    }

    const capacity_text: &[u8] = b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
    for length in 1..=31 {
        driver.edit(
            "x",
            action::insert(b'x'),
            state(
                &capacity_text[..length],
                (length, length),
                length as u64 + 1,
                1,
                if length == 1 {
                    feedback::edited
                } else {
                    feedback::history_replaced
                },
            ),
        )?;
    }
    let capacity = driver.edit(
        "z",
        action::insert(b'z'),
        state(
            capacity_text,
            (31, 31),
            32,
            1,
            feedback::rejected(editor_error::text_capacity),
        ),
    )?;
    driver.edit(
        "backspace",
        action::backspace,
        state(
            &capacity_text[..30],
            (30, 30),
            33,
            1,
            feedback::history_replaced,
        ),
    )?;
    driver.edit(
        "ctrl-z",
        action::undo,
        state(capacity_text, (31, 31), 34, 0, feedback::undone),
    )?;
    driver.edit(
        "ctrl-a",
        action::select_all,
        state(capacity_text, (0, 31), 34, 0, feedback::selection_changed),
    )?;
    driver.edit(
        "backspace",
        action::backspace,
        state(b"", (0, 0), 35, 1, feedback::edited),
    )?;
    driver.edit(
        "backspace",
        action::backspace,
        state(b"", (0, 0), 35, 1, feedback::unchanged),
    )?;
    driver.edit(
        "ctrl-s",
        action::save_unavailable,
        state(b"", (0, 0), 35, 1, feedback::save_unavailable),
    )?;
    driver.close_editor()?;
    driver.key("f1", command::toggle_launcher, None)?;
    driver.key(
        "up",
        command::select_launcher(launcher_choice::editor),
        None,
    )?;
    driver.key(
        "ret",
        command::activate_launcher {
            choice: launcher_choice::editor,
            graphics: None,
        },
        Some(empty),
    )?;
    driver.close_editor()?;
    driver.withdraw()?;
    Ok((initial, capacity))
}

pub(super) fn smoke() -> Result<(), String> {
    smoke_image(&build()?, workspace_profile::legacy)
}
pub(super) fn smoke_image(image: &built_image, profile: workspace_profile) -> Result<(), String> {
    let root = repository_root()?;
    let directory = profile.directory(&root, "workspace-editor-smoke");
    fs::create_dir_all(&directory).map_err(|_| "create editor smoke directory")?;
    let boot = directory.join("boot.img");
    fs::copy(&image.path, &boot).map_err(|_| "copy private editor smoke boot image")?;
    let pages = profile.pages(&root)?;
    let mut files = captures::new_profile(&directory, profile)?;
    let child = Command::new("qemu-system-x86_64")
        .args(profile.cpu_arguments())
        .current_dir(&root)
        .args(["-machine", "q35", "-m", "256M", "-drive"])
        .arg(format!("format=raw,snapshot=on,file={}", boot.display()))
        .args(["-no-reboot", "-no-shutdown", "-display", "none", "-serial"])
        .arg(format!("file:{}", files.log_part.display()))
        .args(["-monitor", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "launch private editor smoke QEMU")?;
    let mut child = semantic_presentation_child::new(child);
    let result = (|| {
        let child = child.child_mut();
        let monitor = child.stdin.take().ok_or("editor QEMU monitor missing")?;
        let io = native_io {
            child,
            monitor,
            files: &files,
            pages,
            deadline: Instant::now() + Duration::from_secs(300),
        };
        let mut driver = driver::new(Some(io))?;
        let (initial, capacity) = exercise(&mut driver)?;
        Ok::<_, String>((initial, capacity, driver.frames))
    })();
    child.terminate_and_reap();
    let (initial, capacity, frames) = result?;
    let transcript = read_bounded_optional_utf8_file(&files.log_part, transcript_limit)?;
    profile.verify(&transcript, pages, frames, Some("task_exit"))?;
    for (path, bytes) in [
        (&files.log, transcript.as_bytes()),
        (&files.initial, initial.as_slice()),
        (&files.customized, capacity.as_slice()),
    ] {
        create_private_capture_file(path)?;
        fs::write(path, bytes).map_err(|_| "retain verified public editor proof artifact")?;
    }
    files.published = true;
    println!(
        "Native workspace editor passed {frames} exact frames: physical repeated editing, literal text/selection/history/error states, focused shortcut isolation, modal launcher, pointer close/reopen, explicit unavailable save, and resource-restoring shell exit"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_editor_trace_matches_canonical_semantics_without_the_input_adapter() {
        let mut driver = driver::new(None).unwrap();
        let (initial, capacity) = exercise(&mut driver).unwrap();
        assert!(initial.is_empty() && capacity.is_empty());
        assert!(driver.frames > 90 && driver.frames < 140);
        assert!(driver.editor().is_err());
        assert!(!driver.view().unwrap().launcher_open());
    }

    #[test]
    fn literal_oracle_rejects_wrong_text_revision_history_and_feedback() {
        let mut driver = driver::new(None).unwrap();
        driver.key("f4", command::open_editor, Some(empty)).unwrap();
        let view = driver.view().unwrap();
        verify_editor(&view, Some(empty)).unwrap();
        for changed in [
            expected_editor {
                text: b"x",
                ..empty
            },
            expected_editor {
                revision: 2,
                ..empty
            },
            expected_editor { undo: 1, ..empty },
            expected_editor {
                feedback: editor_feedback::save_unavailable,
                ..empty
            },
            expected_editor {
                dirty: true,
                ..empty
            },
            expected_editor {
                selection: (1, 1),
                ..empty
            },
        ] {
            assert!(verify_editor(&view, Some(changed)).is_err());
        }
        assert!(verify_editor(&view, None).is_err());
    }
}
