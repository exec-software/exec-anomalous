use super::*;
use anomalous_desktop_workspace::{
    client_grants, client_origin, desktop_control, editor_view, window_id,
};

fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    assert!(model.publish(control, ticket).is_ok());
}

struct fixture {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    keyboard: keyboard_state,
    sequence: u64,
    editor: window_id,
}
impl fixture {
    fn new() -> Self {
        let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        let revision = model.observe(&human).unwrap().revision();
        let ticket = model
            .prepare(&human, revision, desktop_command::open_editor)
            .unwrap();
        publish(&mut model, &control, ticket);
        let editor = model
            .observe(&human)
            .unwrap()
            .focused_window()
            .unwrap()
            .id();
        Self {
            model,
            control,
            human,
            keyboard: keyboard_state::new(),
            sequence: 0,
            editor,
        }
    }
    fn view(&self) -> render_view<'_> {
        self.model.observe(&self.human).unwrap()
    }
    fn state(&self) -> editor_view {
        self.view().window(self.editor).unwrap().editor().unwrap()
    }
    fn apply(&mut self, command: desktop_command) {
        let revision = self.view().revision();
        let ticket = self.model.prepare(&self.human, revision, command).unwrap();
        publish(&mut self.model, &self.control, ticket);
    }
    fn command(&self, code: u8, extended: bool) -> Option<desktop_command> {
        command_for_with_modifiers(
            &self.view(),
            code,
            extended,
            self.keyboard.modifiers(),
            None,
        )
    }
    fn key(&mut self, code: u8, extended: bool, pressed: bool) -> bool {
        let event = rite_physical_input_v1::key(1, self.sequence, code, extended, pressed).unwrap();
        self.sequence += 1;
        let accepted = self.keyboard.consume(&event).unwrap();
        let command = accepted.and_then(|(code, extended)| self.command(code, extended));
        if let Some(command) = command {
            self.apply(command);
            true
        } else {
            false
        }
    }
    fn tap(&mut self, code: u8, extended: bool) -> bool {
        let published = self.key(code, extended, true);
        assert!(!self.key(code, extended, false));
        published
    }
}

#[test]
fn all_printable_us_keys_have_explicit_case_and_shift_policy() {
    let rows = [
        (0x10, &b"qwertyuiop"[..]),
        (0x1e, &b"asdfghjkl"[..]),
        (0x2c, &b"zxcvbnm"[..]),
    ];
    for (first, row) in rows {
        for (index, byte) in row.iter().copied().enumerate() {
            let code = first + index as u8;
            for shift in [false, true] {
                for caps_lock in [false, true] {
                    assert_eq!(
                        ascii_for(code, shift, caps_lock),
                        Some(if shift ^ caps_lock {
                            byte.to_ascii_uppercase()
                        } else {
                            byte
                        })
                    );
                }
            }
        }
    }
    for (index, (plain, shifted)) in b"1234567890".iter().zip(b"!@#$%^&*()").enumerate() {
        assert_eq!(ascii_for(0x02 + index as u8, false, true), Some(*plain));
        assert_eq!(ascii_for(0x02 + index as u8, true, true), Some(*shifted));
    }
    for (code, plain, shifted) in [
        (0x0c, b'-', b'_'),
        (0x0d, b'=', b'+'),
        (0x1a, b'[', b'{'),
        (0x1b, b']', b'}'),
        (0x27, b';', b':'),
        (0x28, b'\'', b'"'),
        (0x29, b'`', b'~'),
        (0x2b, b'\\', b'|'),
        (0x33, b',', b'<'),
        (0x34, b'.', b'>'),
        (0x35, b'/', b'?'),
        (0x39, b' ', b' '),
    ] {
        assert_eq!(ascii_for(code, false, true), Some(plain));
        assert_eq!(ascii_for(code, true, false), Some(shifted));
    }
    for code in [
        0x01, 0x0e, 0x0f, 0x1c, 0x1d, 0x2a, 0x36, 0x38, 0x3a, 0x3e, 0x53, 0x58,
    ] {
        assert_eq!(ascii_for(code, false, false), None);
    }
}

#[test]
fn editor_focus_consumes_desktop_letters_arrows_and_development_keys() {
    let mut f = fixture::new();
    let original = f.view();
    let bounds = original.window(f.editor).unwrap().bounds();
    let theme = original.theme();
    let layout = original.layout();
    let workspace = original.active_workspace();
    let window_count = original.all_windows().count();
    for code in [
        0x10, 0x32, 0x14, 0x2e, 0x20, 0x22, 0x1e, 0x21, 0x2f, 0x2d, 0x02, 0x03, 0x13, 0x11, 0x31,
        0x19,
    ] {
        assert!(
            matches!(f.command(code,false),Some(desktop_command::edit_editor { window, action: editor_action::insert(_) }) if window==f.editor)
        );
        assert!(f.tap(code, false));
    }
    assert_eq!(f.state().text(), b"qmtcdgafvx12rwnp");
    for code in [0x3c, 0x3d, 0x44, 0x57, 0x58, 0x1c] {
        assert_eq!(
            f.command(code, false),
            Some(desktop_command::edit_editor {
                window: f.editor,
                action: editor_action::unsupported_key
            })
        );
        f.tap(code, false);
    }
    for (code, motion) in [
        (0x4b, caret_motion::left),
        (0x4d, caret_motion::right),
        (0x47, caret_motion::start),
        (0x4f, caret_motion::end),
    ] {
        assert_eq!(
            f.command(code, true),
            Some(desktop_command::edit_editor {
                window: f.editor,
                action: editor_action::move_caret {
                    motion,
                    extend: false
                }
            })
        );
        f.tap(code, true);
    }
    f.tap(0x01, false);
    let view = f.view();
    assert!(editor_focused(&view));
    assert_eq!(view.window(f.editor).unwrap().bounds(), bounds);
    assert_eq!(view.theme(), theme);
    assert_eq!(view.layout(), layout);
    assert_eq!(view.active_workspace(), workspace);
    assert_eq!(view.all_windows().count(), window_count);
    assert_eq!(f.state().text(), b"qmtcdgafvx12rwnp");
}

#[test]
fn real_make_release_edits_replace_delete_and_one_step_undo_without_fake_save() {
    let mut f = fixture::new();
    for code in [0x1e, 0x30, 0x2e] {
        assert!(f.tap(code, false));
    }
    assert_eq!(f.state().text(), b"abc");
    assert_eq!(f.state().revision(), 4);
    f.key(0x2a, false, true);
    f.tap(0x4b, true);
    f.key(0x2a, false, false);
    assert_eq!(f.state().selection(), (3, 2));
    f.tap(0x2d, false);
    assert_eq!(f.state().text(), b"abx");
    f.key(0x1d, false, true);
    f.tap(0x2c, false);
    f.key(0x1d, false, false);
    assert_eq!(f.state().text(), b"abc");
    assert_eq!(f.state().selection(), (3, 2));
    f.tap(0x53, true);
    assert_eq!(f.state().text(), b"ab");
    f.tap(0x0e, false);
    assert_eq!(f.state().text(), b"a");
    let revision = f.state().revision();
    let undo = f.state().undo_depth();
    f.key(0x1d, true, true);
    f.tap(0x1f, false);
    f.key(0x1d, true, false);
    assert_eq!(f.state().text(), b"a");
    assert_eq!(f.state().revision(), revision);
    assert_eq!(f.state().undo_depth(), undo);
    assert!(f.state().is_dirty());
    assert_eq!(f.state().feedback().code(), "save_unavailable");
    f.key(0x1d, false, true);
    f.tap(0x1e, false);
    f.key(0x1d, false, false);
    assert_eq!(f.state().selection(), (0, 1));
    f.tap(0x01, false);
    assert_eq!(f.state().selection(), (1, 1));
}

#[test]
fn capacity_rejection_publishes_feedback_without_mutating_document_or_undo() {
    let mut f = fixture::new();
    for _ in 0..31 {
        f.tap(0x1e, false);
    }
    let revision = f.state().revision();
    let selection = f.state().selection();
    let undo = f.state().undo_depth();
    let desktop_revision = f.view().revision();
    f.tap(0x30, false);
    assert_eq!(f.state().text(), &[b'a'; 31]);
    assert_eq!(f.state().revision(), revision);
    assert_eq!(f.state().selection(), selection);
    assert_eq!(f.state().undo_depth(), undo);
    assert_eq!(f.view().revision(), desktop_revision + 1);
    assert_eq!(f.state().feedback().code(), "text_capacity");
    f.key(0x1d, false, true);
    f.tap(0x2c, false);
    f.key(0x1d, false, false);
    assert_eq!(f.state().text(), &[b'a'; 30]);
}

#[test]
fn modal_launcher_focus_changes_and_f4_restore_keep_editor_input_scoped() {
    let mut f = fixture::new();
    f.tap(0x1e, false);
    let revision = f.state().revision();
    f.tap(0x3b, false);
    assert!(f.view().launcher_open());
    for code in [0x30, 0x3c, 0x3d, 0x3e, 0x44, 0x57, 0x58] {
        assert!(!f.tap(code, false));
    }
    assert_eq!(f.state().text(), b"a");
    assert_eq!(f.state().revision(), revision);
    f.tap(0x01, false);
    assert!(!f.view().launcher_open());
    assert!(f.tap(0x0f, false));
    assert!(!editor_focused(&f.view()));
    assert!(!matches!(
        f.command(0x1e, false),
        Some(desktop_command::edit_editor { .. })
    ));
    f.tap(0x3e, false);
    assert!(editor_focused(&f.view()));
    assert_eq!(f.view().focused_window().unwrap().id(), f.editor);
    assert_eq!(f.state().text(), b"a");
    f.apply(desktop_command::minimize(f.editor));
    assert!(!editor_focused(&f.view()));
    f.tap(0x3e, false);
    assert!(editor_focused(&f.view()));
    assert_eq!(f.state().text(), b"a");
}

#[test]
fn held_printable_key_never_replays_across_focus_changes_until_exact_release() {
    let mut f = fixture::new();
    assert!(f.key(0x1e, false, true));
    assert_eq!(f.state().text(), b"a");
    assert!(f.tap(0x0f, false));
    assert!(!editor_focused(&f.view()));
    let away_revision = f.view().revision();
    let away_motion = f.view().reduced_motion();
    // A fresh A make here would toggle the desktop's reduced-motion shortcut.
    // The original held key is still held, so its repeated make does neither.
    assert!(!f.key(0x1e, false, true));
    assert_eq!(f.view().revision(), away_revision);
    assert_eq!(f.view().reduced_motion(), away_motion);
    assert_eq!(f.state().text(), b"a");
    assert!(f.tap(0x3e, false));
    assert!(editor_focused(&f.view()));
    let returned_revision = f.view().revision();
    assert!(!f.key(0x1e, false, true));
    assert_eq!(f.view().revision(), returned_revision);
    assert_eq!(f.state().text(), b"a");
    assert_eq!(f.state().revision(), 2);
    assert!(!f.key(0x1e, false, false));
    assert!(f.key(0x1e, false, true));
    assert!(!f.key(0x1e, false, false));
    assert_eq!(f.state().text(), b"aa");
    assert_eq!(f.state().revision(), 3);
    assert_eq!(f.view().revision(), returned_revision + 1);
}

#[test]
fn small_editor_physical_actions_publish_display_feedback_and_resume_after_enlargement() {
    fn unchanged_editing(actual: editor_view, expected: editor_view) {
        assert_eq!(actual.text(), expected.text());
        assert_eq!(actual.selection(), expected.selection());
        assert_eq!(actual.revision(), expected.revision());
        assert_eq!(actual.undo_depth(), expected.undo_depth());
        assert_eq!(actual.is_dirty(), expected.is_dirty());
    }

    let mut f = fixture::new();
    for code in [0x1e, 0x30, 0x2e] {
        f.tap(code, false);
    }
    f.key(0x2a, false, true);
    f.tap(0x4b, true);
    f.key(0x2a, false, false);
    let original = f.state();
    assert_eq!(original.text(), b"abc");
    assert_eq!(original.selection(), (3, 2));
    let bounds = f.view().window(f.editor).unwrap().floating_bounds();
    f.apply(desktop_command::resize_window {
        window: f.editor,
        width: 120,
        height: 96,
    });
    assert!(!editor_input_visible(
        &f.view(),
        f.view().window(f.editor).unwrap()
    ));

    for (code, extended, control) in [
        (0x2d, false, false), // Insert x.
        (0x0e, false, false), // Backspace.
        (0x53, true, false),  // Delete.
        (0x2c, false, true),  // Ctrl+Z.
        (0x1f, false, true),  // Ctrl+S.
    ] {
        if control {
            assert!(!f.key(0x1d, false, true));
        }
        let before = f.state();
        let revision = f.view().revision();
        let event = rite_physical_input_v1::key(1, f.sequence, code, extended, true).unwrap();
        f.sequence += 1;
        assert_eq!(f.keyboard.consume(&event), Ok(Some((code, extended))));
        let command = f.command(code, extended).unwrap();
        assert_eq!(
            command,
            desktop_command::edit_editor {
                window: f.editor,
                action: editor_action::display_unavailable,
            }
        );
        let ticket = f.model.prepare(&f.human, revision, command).unwrap();
        // A prepared error/status frame does not publish early.
        assert_eq!(f.state(), before);
        let candidate = f
            .model
            .candidate(&ticket)
            .unwrap()
            .window(f.editor)
            .unwrap()
            .editor()
            .unwrap();
        assert_eq!(candidate.feedback().code(), "display_unavailable");
        unchanged_editing(candidate, original);
        publish(&mut f.model, &f.control, ticket);
        assert_eq!(f.state(), candidate);
        assert_eq!(f.view().revision(), revision + 1);
        assert!(!f.key(code, extended, false));
        if control {
            assert!(!f.key(0x1d, false, false));
        }
    }

    // Unmodified navigation remains available even though text is hidden.
    assert!(f.tap(0x3b, false));
    assert!(f.view().launcher_open());
    assert!(f.tap(0x01, false));
    assert!(!f.view().launcher_open());
    assert!(f.tap(0x0f, false));
    assert!(!editor_focused(&f.view()));
    assert!(f.tap(0x3e, false));
    assert!(editor_focused(&f.view()));
    assert_eq!(f.view().focused_window().unwrap().id(), f.editor);
    unchanged_editing(f.state(), original);

    // At font scale 2, the default comfortable chrome consumes 88 vertical
    // pixels, and the editor needs 44*2 content pixels. Check both sides of
    // that literal boundary against the shared paint/input predicate.
    let mut settings = f.view().theme_settings();
    settings.font_scale = 2;
    f.apply(desktop_command::set_theme_settings(settings));
    for (height, visible) in [(175, false), (176, true)] {
        f.apply(desktop_command::resize_window {
            window: f.editor,
            width: 120,
            height,
        });
        assert_eq!(
            editor_input_visible(&f.view(), f.view().window(f.editor).unwrap()),
            visible,
        );
        assert_eq!(
            f.command(0x2d, false),
            Some(desktop_command::edit_editor {
                window: f.editor,
                action: if visible {
                    editor_action::insert(b'x')
                } else {
                    editor_action::display_unavailable
                },
            }),
        );
    }
    f.apply(desktop_command::resize_window {
        window: f.editor,
        width: bounds.width(),
        height: bounds.height(),
    });
    assert!(editor_input_visible(
        &f.view(),
        f.view().window(f.editor).unwrap()
    ));
    unchanged_editing(f.state(), original);
    // Blocked edits did not consume the old undo entry: it still restores ab.
    f.key(0x1d, false, true);
    f.tap(0x2c, false);
    f.key(0x1d, false, false);
    assert_eq!(f.state().text(), b"ab");
    assert_eq!(f.state().selection(), (2, 2));
    assert_eq!(f.state().revision(), 5);
    assert_eq!(f.state().undo_depth(), 0);
    f.tap(0x2d, false);
    assert_eq!(f.state().text(), b"abx");
    assert_eq!(f.state().feedback().code(), "edited");
    f.key(0x1d, false, true);
    f.tap(0x2c, false);
    f.key(0x1d, false, false);
    assert_eq!(f.state().text(), b"ab");
    assert_eq!(f.state().selection(), (2, 2));
    assert_eq!(f.state().revision(), 7);
    assert_eq!(f.state().feedback().code(), "undone");
}

#[test]
fn both_modifier_sides_caps_and_repeats_do_not_mint_text_or_leak_shortcuts() {
    let mut f = fixture::new();
    assert!(!f.key(0x3a, false, true));
    assert!(f.keyboard.caps_lock());
    assert!(!f.key(0x3a, false, true));
    assert!(f.keyboard.caps_lock());
    f.key(0x3a, false, false);
    f.tap(0x1e, false);
    assert_eq!(f.state().text(), b"A");
    f.key(0x2a, false, true);
    f.tap(0x30, false);
    f.key(0x2a, false, false);
    assert_eq!(f.state().text(), b"Ab");
    assert!(f.key(0x2e, false, true));
    let revision = f.view().revision();
    assert!(!f.key(0x2e, false, true));
    assert_eq!(f.view().revision(), revision);
    f.key(0x2e, false, false);
    assert!(f.key(0x2e, false, true));
    f.key(0x2e, false, false);
    assert_eq!(f.state().text(), b"AbCC");
    for code in [0x1d, 0x38] {
        assert!(!f.key(code, false, true));
        assert!(!f.key(code, true, true));
        assert!(!f.key(code, false, false));
        if code == 0x1d {
            assert!(f.keyboard.control());
        } else {
            assert!(f.keyboard.alt());
        }
        assert_eq!(
            f.command(0x10, false),
            Some(desktop_command::edit_editor {
                window: f.editor,
                action: editor_action::unsupported_key
            })
        );
        f.tap(0x10, false);
        assert_eq!(f.state().text(), b"AbCC");
        assert!(!f.key(code, true, false));
    }
    assert!(!f.keyboard.control());
    assert!(!f.keyboard.alt());
    f.tap(0x3a, false);
    assert!(!f.keyboard.caps_lock());
    f.key(0x3a, true, true);
    assert!(!f.keyboard.caps_lock());
}

#[test]
fn declared_input_loss_clears_all_modifiers_and_reports_to_focused_editor_under_menu() {
    let mut f = fixture::new();
    f.tap(0x1e, false);
    f.tap(0x3b, false);
    for (code, extended) in [(0x2a, false), (0x1d, true), (0x38, true), (0x3a, false)] {
        f.key(code, extended, true);
    }
    assert!(
        f.keyboard.shift() && f.keyboard.control() && f.keyboard.alt() && f.keyboard.caps_lock()
    );
    let loss = rite_physical_input_v1::loss(1, 2, f.sequence, f.sequence + 4, 5).unwrap();
    assert_eq!(f.keyboard.consume(&loss), Ok(None));
    assert_eq!(f.keyboard.modifiers(), keyboard_modifiers::default());
    let command = input_reset_command(&f.view()).unwrap();
    assert_eq!(
        command,
        desktop_command::edit_editor {
            window: f.editor,
            action: editor_action::input_reset
        }
    );
    f.apply(command);
    assert!(f.view().launcher_open());
    assert_eq!(f.state().text(), b"a");
    assert_eq!(f.state().revision(), 2);
    assert_eq!(f.state().feedback().code(), "input_reset");
    let fresh = rite_physical_input_v1::key(2, 0, 0x1e, false, true).unwrap();
    assert_eq!(f.keyboard.consume(&fresh), Ok(Some((0x1e, false))));
    assert_eq!(f.command(0x1e, false), None); // Still modal after source reset.
    // Continue the actual new source sequence: release the swallowed letter,
    // dismiss the menu, then type again. No old Shift/Caps/Control/Alt state
    // may capitalize the text, select all, or turn it into an unsupported chord.
    for (sequence, code, pressed) in [
        (1, 0x1e, false),
        (2, 0x01, true),
        (3, 0x01, false),
        (4, 0x1e, true),
        (5, 0x1e, false),
    ] {
        let event = rite_physical_input_v1::key(2, sequence, code, false, pressed).unwrap();
        let accepted = f.keyboard.consume(&event).unwrap();
        if let Some(command) = accepted.and_then(|(code, extended)| f.command(code, extended)) {
            f.apply(command);
        }
    }
    assert!(!f.view().launcher_open());
    assert!(editor_focused(&f.view()));
    assert_eq!(f.keyboard.modifiers(), keyboard_modifiers::default());
    assert_eq!(f.state().text(), b"aa");
    assert_eq!(f.state().selection(), (2, 2));
    assert_eq!(f.state().revision(), 3);
    assert_eq!(f.state().feedback().code(), "history_replaced");
}

#[test]
fn invalid_source_or_sequence_cannot_change_modifiers_or_editor_state() {
    let mut f = fixture::new();
    f.tap(0x1e, false);
    f.key(0x1d, true, true);
    let modifiers = f.keyboard.modifiers();
    let revision = f.view().revision();
    for event in [
        rite_physical_input_v1::key(1, f.sequence - 1, 0x1d, true, false).unwrap(),
        rite_physical_input_v1::key(1, f.sequence + 1, 0x30, false, true).unwrap(),
        rite_physical_input_v1::key(2, f.sequence, 0x1d, true, false).unwrap(),
    ] {
        assert!(f.keyboard.consume(&event).is_err());
        assert_eq!(f.keyboard.modifiers(), modifiers);
        assert_eq!(f.view().revision(), revision);
        assert_eq!(f.state().text(), b"a");
    }
    assert_eq!(
        f.command(0x0f, false),
        Some(desktop_command::edit_editor {
            window: f.editor,
            action: editor_action::unsupported_key,
        })
    );
}
