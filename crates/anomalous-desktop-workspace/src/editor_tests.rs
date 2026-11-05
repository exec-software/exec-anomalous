use super::*;

struct fixture {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    editor: window_id,
}

impl fixture {
    fn new() -> Self {
        let (mut model, control, initial) = desktop::new(1024, 768).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        let revision = model.observe(&human).unwrap().revision();
        let ticket = model
            .prepare(&human, revision, desktop_command::open_editor)
            .unwrap();
        let editor = publish(&mut model, &control, ticket)
            .created_window()
            .unwrap();
        Self {
            model,
            control,
            human,
            editor,
        }
    }

    fn view(&self) -> render_view<'_> {
        self.model.observe(&self.human).unwrap()
    }
    fn editor(&self) -> editor_view {
        self.view().window(self.editor).unwrap().editor().unwrap()
    }
    fn apply(&mut self, command: desktop_command) -> publication {
        let revision = self.view().revision();
        let ticket = self.model.prepare(&self.human, revision, command).unwrap();
        publish(&mut self.model, &self.control, ticket)
    }
    fn edit(&mut self, action: editor_action) -> editor_view {
        self.apply(desktop_command::edit_editor {
            window: self.editor,
            action,
        });
        self.editor()
    }
}

fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) -> publication {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    model.publish(control, ticket).ok().unwrap()
}

fn edit(window: window_id, action: editor_action) -> desktop_command {
    desktop_command::edit_editor { window, action }
}

#[test]
fn scratch_state_and_feedback_publish_together_and_cancel_without_effects() {
    let mut f = fixture::new();
    let initial = f.editor();
    assert_eq!(initial.text(), b"");
    assert_eq!(initial.selection(), (0, 0));
    assert_eq!(initial.revision(), 1);
    assert_eq!(initial.undo_depth(), 0);
    assert!(!initial.is_dirty());
    assert_eq!(initial.feedback(), editor_feedback::ready);
    let revision = f.view().revision();
    let ticket = f
        .model
        .prepare(
            &f.human,
            revision,
            edit(f.editor, editor_action::insert(b'a')),
        )
        .unwrap();
    assert_eq!(f.editor(), initial);
    let candidate = f
        .model
        .candidate(&ticket)
        .unwrap()
        .window(f.editor)
        .unwrap()
        .editor()
        .unwrap();
    assert_eq!(candidate.text(), b"a");
    assert_eq!(candidate.feedback(), editor_feedback::edited);
    assert_eq!(candidate.revision(), 2);
    f.model.cancel(&f.control, ticket).ok().unwrap();
    assert_eq!(f.editor(), initial);
    assert_eq!(f.view().revision(), revision);
    let ticket = f
        .model
        .prepare(
            &f.human,
            revision,
            edit(f.editor, editor_action::insert(b'a')),
        )
        .unwrap();
    f.model.begin(&f.control, &ticket).unwrap();
    assert_eq!(
        f.model.observe(&f.human).err(),
        Some(desktop_error::unavailable)
    );
    let rejection = f.model.publish(&f.control, ticket).err().unwrap();
    assert_eq!(rejection.error(), desktop_error::wrong_phase);
    let ticket = rejection.into_ticket();
    f.model.attest_presented(&f.control, &ticket).unwrap();
    f.model.publish(&f.control, ticket).ok().unwrap();
    assert_eq!(f.editor(), candidate);
}

#[test]
fn repeated_edit_feedback_and_rejection_preserve_the_immediate_undo_state() {
    let mut f = fixture::new();
    for (index, byte) in b"abc".iter().enumerate() {
        let editor = f.edit(editor_action::insert(*byte));
        assert_eq!(editor.text(), &b"abc"[..index + 1]);
        assert_eq!(editor.revision(), index as u64 + 2);
        assert_eq!(
            editor.feedback(),
            if index == 0 {
                editor_feedback::edited
            } else {
                editor_feedback::history_replaced
            }
        );
    }
    let before = f.editor();
    let rejected = f.edit(editor_action::insert(b'\n'));
    assert_eq!(
        rejected.feedback(),
        editor_feedback::rejected(editor_error::unsupported_text)
    );
    assert_eq!(rejected.text(), before.text());
    assert_eq!(rejected.selection(), before.selection());
    assert_eq!(rejected.revision(), before.revision());
    assert_eq!(rejected.undo_depth(), before.undo_depth());
    let undone = f.edit(editor_action::undo);
    assert_eq!(undone.text(), b"ab");
    assert_eq!(undone.selection(), (2, 2));
    assert_eq!(undone.feedback(), editor_feedback::undone);
    let again = f.edit(editor_action::undo);
    assert_eq!(again.text(), undone.text());
    assert_eq!(again.revision(), undone.revision());
    assert_eq!(
        again.feedback(),
        editor_feedback::rejected(editor_error::no_undo)
    );
    f.edit(editor_action::insert(b'd'));
    assert_eq!(f.editor().text(), b"abd");
}

#[test]
fn capacity_status_is_visible_but_text_history_and_later_valid_edits_remain_sound() {
    let mut f = fixture::new();
    for _ in 0..31 {
        f.edit(editor_action::insert(b'x'));
    }
    let before = f.editor();
    let rejected = f.edit(editor_action::insert(b'y'));
    assert_eq!(
        rejected.feedback(),
        editor_feedback::rejected(editor_error::text_capacity)
    );
    assert_eq!(rejected.text(), before.text());
    assert_eq!(rejected.selection(), before.selection());
    assert_eq!(rejected.revision(), before.revision());
    assert_eq!(rejected.undo_depth(), 1);
    let undone = f.edit(editor_action::undo);
    assert_eq!(undone.text(), &[b'x'; 30]);
    assert_eq!(undone.selection(), (30, 30));
    f.edit(editor_action::insert(b'z'));
    assert_eq!(f.editor().text()[30], b'z');
}

#[test]
fn caret_deletion_noops_and_status_only_actions_do_not_discard_undo() {
    let mut f = fixture::new();
    f.edit(editor_action::insert(b'a'));
    f.edit(editor_action::insert(b'b'));
    assert_eq!(
        f.edit(editor_action::select_all).feedback(),
        editor_feedback::selection_changed
    );
    assert_eq!(
        f.edit(editor_action::select_all).feedback(),
        editor_feedback::unchanged
    );
    assert_eq!(
        f.edit(editor_action::collapse_selection).selection(),
        (2, 2)
    );
    assert_eq!(
        f.edit(editor_action::delete_forward).feedback(),
        editor_feedback::unchanged
    );
    f.edit(editor_action::move_caret {
        motion: caret_motion::start,
        extend: false,
    });
    assert_eq!(
        f.edit(editor_action::backspace).feedback(),
        editor_feedback::unchanged
    );
    for (action, feedback) in [
        (
            editor_action::save_unavailable,
            editor_feedback::save_unavailable,
        ),
        (
            editor_action::unsupported_key,
            editor_feedback::unsupported_key,
        ),
        (editor_action::input_reset, editor_feedback::input_reset),
    ] {
        let before = f.editor();
        let after = f.edit(action);
        assert_eq!(after.feedback(), feedback);
        assert_eq!(after.text(), before.text());
        assert_eq!(after.selection(), before.selection());
        assert_eq!(after.revision(), before.revision());
        assert_eq!(after.undo_depth(), before.undo_depth());
    }
    assert_eq!(f.edit(editor_action::undo).text(), b"a");
    f.edit(editor_action::move_caret {
        motion: caret_motion::end,
        extend: false,
    });
    f.edit(editor_action::insert(b'b'));
    f.edit(editor_action::move_caret {
        motion: caret_motion::left,
        extend: false,
    });
    assert_eq!(f.edit(editor_action::delete_forward).text(), b"a");
    let undone = f.edit(editor_action::undo);
    assert_eq!(undone.text(), b"ab");
    assert_eq!(undone.selection(), (1, 1));
}

#[test]
fn editor_requires_present_grant_exact_revision_identity_visibility_and_focus() {
    let mut f = fixture::new();
    let arranger = f
        .model
        .bind_client(
            &f.control,
            client_origin::machine,
            client_grants::observe.union(client_grants::arrange),
        )
        .unwrap();
    let presenter = f
        .model
        .bind_client(
            &f.control,
            client_origin::machine,
            client_grants::observe.union(client_grants::present),
        )
        .unwrap();
    let original = f.editor();
    let revision = f.view().revision();
    assert_eq!(
        f.model
            .prepare(
                &arranger,
                revision,
                edit(f.editor, editor_action::insert(b'x'))
            )
            .err(),
        Some(desktop_error::grant_denied)
    );
    assert_eq!(
        f.model
            .prepare(
                &presenter,
                revision - 1,
                edit(f.editor, editor_action::insert(b'x'))
            )
            .err(),
        Some(desktop_error::stale_revision)
    );
    let foreign = fixture::new();
    assert_eq!(
        f.model
            .prepare(
                &foreign.human,
                revision,
                edit(f.editor, editor_action::insert(b'x'))
            )
            .err(),
        Some(desktop_error::client_denied)
    );
    assert_eq!(f.editor(), original);
    let ticket = f
        .model
        .prepare(
            &presenter,
            revision,
            edit(f.editor, editor_action::insert(b'p')),
        )
        .unwrap();
    assert_eq!(
        publish(&mut f.model, &f.control, ticket).origin(),
        Some(client_origin::machine)
    );
    let other = f
        .apply(desktop_command::create {
            content: demo_content::welcome,
            workspace: workspace_id::first,
            bounds: window_rect::try_new(50, 100, 300, 200).unwrap(),
        })
        .created_window()
        .unwrap();
    let revision = f.view().revision();
    assert_eq!(
        f.model
            .prepare(
                &f.human,
                revision,
                edit(f.editor, editor_action::insert(b'x'))
            )
            .err(),
        Some(desktop_error::editor_not_focused)
    );
    assert_eq!(
        f.model
            .prepare(&f.human, revision, edit(other, editor_action::insert(b'x')))
            .err(),
        Some(desktop_error::not_editor_window)
    );
    f.apply(desktop_command::focus(f.editor));
    let old = f.editor();
    f.apply(desktop_command::minimize(f.editor));
    let revision = f.view().revision();
    assert_eq!(
        f.model
            .prepare(
                &f.human,
                revision,
                edit(f.editor, editor_action::insert(b'x'))
            )
            .err(),
        Some(desktop_error::window_not_visible)
    );
    assert_eq!(f.editor(), old);
    f.apply(desktop_command::open_editor);
    f.apply(desktop_command::switch_workspace(workspace_id::second));
    let revision = f.view().revision();
    assert_eq!(
        f.model
            .prepare(
                &f.human,
                revision,
                edit(f.editor, editor_action::insert(b'x'))
            )
            .err(),
        Some(desktop_error::window_not_visible)
    );
    assert_eq!(f.editor(), old);
}

#[test]
fn failed_batch_and_revoked_preparation_do_not_publish_editor_or_feedback() {
    let mut f = fixture::new();
    let before = f.editor();
    let revision = f.view().revision();
    assert_eq!(
        f.model
            .prepare_batch(
                &f.human,
                revision,
                &[
                    edit(f.editor, editor_action::insert(b'x')),
                    desktop_command::resize_window {
                        window: f.editor,
                        width: 1,
                        height: 1
                    },
                ]
            )
            .err(),
        Some(desktop_error::invalid_geometry)
    );
    assert_eq!(f.editor(), before);
    assert_eq!(f.view().revision(), revision);
    let machine = f
        .model
        .bind_client(
            &f.control,
            client_origin::machine,
            client_grants::observe.union(client_grants::present),
        )
        .unwrap();
    let ticket = f
        .model
        .prepare(
            &machine,
            revision,
            edit(f.editor, editor_action::save_unavailable),
        )
        .unwrap();
    f.model.revoke_client(&f.control, &machine).unwrap();
    assert_eq!(
        f.model.begin(&f.control, &ticket),
        Err(desktop_error::client_denied)
    );
    f.model.cancel(&f.control, ticket).ok().unwrap();
    assert_eq!(f.editor(), before);
}

#[test]
fn lifecycle_preserves_scratch_until_close_then_old_generation_cannot_edit_replacement() {
    let mut f = fixture::new();
    f.edit(editor_action::insert(b'x'));
    let before = f.editor();
    for command in [
        desktop_command::set_theme(theme_mode::high_contrast),
        desktop_command::move_window {
            window: f.editor,
            x: 32,
            y: 96,
        },
        desktop_command::resize_window {
            window: f.editor,
            width: 400,
            height: 240,
        },
        desktop_command::minimize(f.editor),
        desktop_command::open_editor,
        desktop_command::move_to_workspace {
            window: f.editor,
            workspace: workspace_id::second,
        },
        desktop_command::open_editor,
    ] {
        f.apply(command);
        assert_eq!(f.editor(), before);
    }
    f.apply(desktop_command::toggle_launcher);
    let reset = f.edit(editor_action::input_reset);
    assert_eq!(reset.text(), before.text());
    assert_eq!(reset.feedback(), editor_feedback::input_reset);
    assert!(f.view().launcher_open());
    f.apply(desktop_command::activate_launcher {
        choice: launcher_choice::editor,
        graphics: None,
    });
    assert_eq!(f.view().focused_window().unwrap().id(), f.editor);
    assert_eq!(f.editor(), reset);
    f.apply(desktop_command::close(f.editor));
    let old = f.editor;
    let new = f
        .apply(desktop_command::open_editor)
        .created_window()
        .unwrap();
    assert_eq!(old.slot(), new.slot());
    assert_ne!(old.generation(), new.generation());
    f.editor = new;
    let replacement = f.editor();
    assert_eq!(replacement.text(), b"");
    assert_eq!(replacement.feedback(), editor_feedback::ready);
    let revision = f.view().revision();
    assert_eq!(
        f.model
            .prepare(&f.human, revision, edit(old, editor_action::insert(b'x')))
            .err(),
        Some(desktop_error::stale_window)
    );
    assert_eq!(f.editor(), replacement);
}

#[test]
fn semantic_projections_share_exact_public_editor_state_and_edits_cancel_pointer_arms() {
    let mut f = fixture::new();
    f.edit(editor_action::insert(b'x'));
    for action in [
        editor_action::insert(b'y'),
        editor_action::undo,
        editor_action::undo,
        editor_action::save_unavailable,
        editor_action::unsupported_key,
        editor_action::input_reset,
    ] {
        let revision = f.view().revision();
        let ticket = f
            .model
            .prepare_batch(
                &f.human,
                revision,
                &[
                    desktop_command::set_pointer { x: 100, y: 100 },
                    desktop_command::set_pointer_buttons(1),
                    desktop_command::arm_pointer(desktop_target::close(f.editor)),
                ],
            )
            .unwrap();
        publish(&mut f.model, &f.control, ticket);
        assert!(f.view().pointer_armed().is_some());
        f.edit(action);
        let view = f.view();
        assert_eq!(view.pointer_armed(), None);
        let editor = view.window(f.editor).unwrap().editor().unwrap();
        assert_eq!(
            Some(editor),
            view.machine().window(f.editor).unwrap().editor()
        );
        assert_eq!(
            Some(editor),
            view.accessibility().window(f.editor).unwrap().editor()
        );
        assert!(!editor.feedback().code().is_empty());
        assert!(!editor.feedback().label().is_empty());
    }
}
