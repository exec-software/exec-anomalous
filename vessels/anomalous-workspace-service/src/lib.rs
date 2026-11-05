#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! User-space physical-key adapter for the public native workspace demo.
//! Input observations convey no authority: commands still need a granted
//! local endpoint and exact canonical revision at model preparation.
use anomalous_abi::rite_physical_input_v1;
use anomalous_desktop_compositor::{appearance_controls_visible, editor_input_visible};
use anomalous_desktop_workspace::{
    caret_motion, client_endpoint, cursor_style, demo_content, density_mode, desktop,
    desktop_command, desktop_error, editor_action, launcher_choice, layout_mode, render_view,
    surface_descriptor, theme_mode, update_ticket, window_rect, workspace_id,
};

mod pointer;
pub use pointer::{
    command_batch, pointer_error, pointer_motion_batch_capacity, pointer_read_error, pointer_state,
};

/// Bounded user policy rejections leave the canonical owner intact. Identity,
/// grant, source, and transaction failures must still withdraw the service.
pub const fn expected_policy_rejection(error: desktop_error) -> bool {
    matches!(
        error,
        desktop_error::invalid_geometry
            | desktop_error::window_capacity_exhausted
            | desktop_error::invalid_theme
            | desktop_error::layout_too_small
    )
}

/// A keyboard event clears an armed pointer within its successful action. If
/// unmapped or rejected by bounded policy, cancellation alone must still publish.
/// Unarmed unmapped/rejected keys reserve no transaction and need no frame.
pub fn prepare_keyboard_update(
    model: &mut desktop,
    endpoint: &client_endpoint,
    revision: u64,
    command: Option<desktop_command>,
    cancel_pointer: bool,
) -> Result<Option<update_ticket>, desktop_error> {
    let cancellation = cancel_pointer.then_some(desktop_command::cancel_pointer);
    let Some(command) = command.or(cancellation) else {
        return Ok(None);
    };
    match model.prepare(endpoint, revision, command) {
        Ok(ticket) => Ok(Some(ticket)),
        Err(error) if expected_policy_rejection(error) => match cancellation {
            Some(command) => model.prepare(endpoint, revision, command).map(Some),
            None => Ok(None),
        },
        Err(error) => Err(error),
    }
}

fn other_workspace(workspace: workspace_id) -> workspace_id {
    match workspace {
        workspace_id::first => workspace_id::second,
        workspace_id::second => workspace_id::first,
    }
}

pub fn command_for(
    view: &render_view<'_>,
    code: u8,
    extended: bool,
    shift: bool,
) -> Option<desktop_command> {
    command_for_with_graphics(view, code, extended, shift, None)
}

/// The public Graphics descriptor is supplied by the service that retains its
/// immutable pixels. The launcher never invents a surface or a window identity.
pub fn command_for_with_graphics(
    view: &render_view<'_>,
    code: u8,
    extended: bool,
    shift: bool,
    graphics: Option<surface_descriptor>,
) -> Option<desktop_command> {
    command_for_with_modifiers(
        view,
        code,
        extended,
        keyboard_modifiers {
            shift,
            ..keyboard_modifiers::default()
        },
        graphics,
    )
}

/// Physical modifiers only select a semantic command. Canonical focus, grants,
/// window generation and revision remain checked by the desktop model.
pub fn command_for_with_modifiers(
    view: &render_view<'_>,
    code: u8,
    extended: bool,
    modifiers: keyboard_modifiers,
    graphics: Option<surface_descriptor>,
) -> Option<desktop_command> {
    let shift = modifiers.shift;
    if !extended && code == 0x3b {
        return Some(desktop_command::toggle_launcher);
    }
    if view.launcher_open() {
        let selected = view.launcher_selection();
        return match (code, extended) {
            (0x01, false) => Some(desktop_command::dismiss_launcher),
            (0x48 | 0x4b, true) => Some(desktop_command::select_launcher(selected.previous())),
            (0x50 | 0x4d, true) => Some(desktop_command::select_launcher(selected.next())),
            (0x0f, false) => Some(desktop_command::select_launcher(if shift {
                selected.previous()
            } else {
                selected.next()
            })),
            (0x1c, _) | (0x39, false) if launcher_available(view, selected, graphics) => {
                Some(desktop_command::activate_launcher {
                    choice: selected,
                    graphics: if selected == launcher_choice::graphics {
                        graphics
                    } else {
                        None
                    },
                })
            }
            _ => None,
        };
    }
    if !extended && code == 0x3e && !modifiers.control && !modifiers.alt && !shift {
        return Some(desktop_command::open_editor);
    }
    if editor_focused(view) {
        // Focus routing is exhaustive: unsupported editor keys cannot become
        // desktop actions (notably q/close, m/minimize or arrows/move).
        if !extended && code == 0x0f && !modifiers.control && !modifiers.alt {
            return Some(desktop_command::focus(view.next_focus()?));
        }
        let window = view.focused_window()?;
        return editor_action_for(code, extended, modifiers).map(|action| {
            desktop_command::edit_editor {
                window: window.id(),
                // Physical edits require the same visible line geometry as
                // composition. Pure modifier keys remain inert because they
                // never produce an action; canonical commands remain available
                // independently of this physical-input presentation policy.
                action: if editor_input_visible(view, window) {
                    action
                } else {
                    editor_action::display_unavailable
                },
            }
        });
    }
    if extended && matches!(code, 0x48 | 0x50 | 0x4b | 0x4d) {
        let window = view.focused_window()?;
        let bounds = window.floating_bounds();
        if shift {
            return Some(desktop_command::resize_window {
                window: window.id(),
                width: match code {
                    0x4b => bounds.width().checked_sub(16)?,
                    0x4d => bounds.width().checked_add(16)?,
                    _ => bounds.width(),
                },
                height: match code {
                    0x48 => bounds.height().checked_sub(16)?,
                    0x50 => bounds.height().checked_add(16)?,
                    _ => bounds.height(),
                },
            });
        }
        return Some(desktop_command::move_window {
            window: window.id(),
            x: match code {
                0x4b => bounds.x().checked_sub(16)?,
                0x4d => bounds.x().checked_add(16)?,
                _ => bounds.x(),
            },
            y: match code {
                0x48 => bounds.y().checked_sub(16)?,
                0x50 => bounds.y().checked_add(16)?,
                _ => bounds.y(),
            },
        });
    }
    if extended {
        return None;
    }
    if matches!(code, 0x40 | 0x1c | 0x39) {
        let window = view.focused_window()?;
        let selected = window.appearance_selection()?;
        if !appearance_controls_visible(view, window.id())
            || (code != 0x40 && !view.appearance_enabled(selected))
        {
            return None;
        }
        return Some(if code == 0x40 {
            desktop_command::select_appearance {
                window: window.id(),
                choice: if shift {
                    selected.previous()
                } else {
                    selected.next()
                },
            }
        } else {
            desktop_command::activate_appearance {
                window: window.id(),
                choice: selected,
            }
        });
    }
    Some(match code {
        0x19 => desktop_command::open_appearance,
        0x0f => desktop_command::focus(view.next_focus()?),
        0x14 => desktop_command::set_layout(match view.layout() {
            layout_mode::floating => layout_mode::horizontal,
            layout_mode::horizontal => layout_mode::vertical,
            layout_mode::vertical => layout_mode::floating,
        }),
        0x2e => desktop_command::set_theme(match view.theme() {
            theme_mode::light => theme_mode::dark,
            theme_mode::dark => theme_mode::high_contrast,
            theme_mode::high_contrast => theme_mode::light,
        }),
        0x20 => desktop_command::set_density(match view.density() {
            density_mode::comfortable => density_mode::compact,
            density_mode::compact => density_mode::comfortable,
        }),
        0x22 => desktop_command::set_depth(!view.depth()),
        0x1e => desktop_command::set_reduced_motion(!view.reduced_motion()),
        0x21 => {
            let mut settings = view.theme_settings();
            settings.font_scale = if settings.font_scale == 1 { 2 } else { 1 };
            desktop_command::set_theme_settings(settings)
        }
        0x2f => {
            let mut settings = view.theme_settings();
            settings.cursor = match settings.cursor {
                cursor_style::arrow => cursor_style::crosshair,
                cursor_style::crosshair => cursor_style::arrow,
            };
            desktop_command::set_theme_settings(settings)
        }
        0x2d if view.layout() == layout_mode::floating => {
            let window = view.focused_window()?;
            if window.maximized() {
                desktop_command::restore_geometry(window.id())
            } else {
                desktop_command::maximize(window.id())
            }
        }
        0x02 => desktop_command::switch_workspace(workspace_id::first),
        0x03 => desktop_command::switch_workspace(workspace_id::second),
        0x10 => desktop_command::close(view.focused_window()?.id()),
        0x32 => desktop_command::minimize(view.focused_window()?.id()),
        0x13 => desktop_command::restore(
            view.all_windows()
                .find(|window| window.minimized() && window.workspace() == view.active_workspace())?
                .id(),
        ),
        0x11 => desktop_command::move_to_workspace {
            window: view.focused_window()?.id(),
            workspace: other_workspace(view.active_workspace()),
        },
        0x31 => desktop_command::create {
            content: demo_content::activity,
            workspace: view.active_workspace(),
            bounds: window_rect::try_new(120, 140, 480, 320).ok()?,
        },
        _ => return None,
    })
}

/// Whether ordinary developer shortcuts must yield to an active editor.
pub fn editor_focused(view: &render_view<'_>) -> bool {
    view.focused_window()
        .is_some_and(|window| window.focused() && window.editor().is_some())
}

/// Loss changes modifier interpretation, never text or window authority. Even
/// with the launcher open, the focused editor receives observable reset feedback.
pub fn input_reset_command(view: &render_view<'_>) -> Option<desktop_command> {
    let window = view.focused_window()?;
    (window.focused() && window.editor().is_some()).then_some(desktop_command::edit_editor {
        window: window.id(),
        action: editor_action::input_reset,
    })
}

pub fn editor_action_for(
    code: u8,
    extended: bool,
    modifiers: keyboard_modifiers,
) -> Option<editor_action> {
    if matches!(
        (code, extended),
        (0x2a | 0x36 | 0x3a, false) | (0x1d | 0x38, _)
    ) {
        return None;
    }
    if modifiers.alt {
        return Some(editor_action::unsupported_key);
    }
    if modifiers.control {
        return Some(match (code, extended, modifiers.shift) {
            (0x1e, false, false) => editor_action::select_all,
            (0x2c, false, false) => editor_action::undo,
            (0x1f, false, false) => editor_action::save_unavailable,
            _ => editor_action::unsupported_key,
        });
    }
    let action = match (code, extended) {
        (0x01, false) => editor_action::collapse_selection,
        (0x0e, false) => editor_action::backspace,
        (0x53, true) => editor_action::delete_forward,
        (0x4b | 0x4d | 0x47 | 0x4f, true) => editor_action::move_caret {
            motion: match code {
                0x4b => caret_motion::left,
                0x4d => caret_motion::right,
                0x47 => caret_motion::start,
                _ => caret_motion::end,
            },
            extend: modifiers.shift,
        },
        (_, false) => ascii_for(code, modifiers.shift, modifiers.caps_lock)
            .map_or(editor_action::unsupported_key, editor_action::insert),
        _ => editor_action::unsupported_key,
    };
    Some(action)
}

/// One accepted Set1 make is one edit; held repeats are filtered by
/// keyboard_state. This bounded layout is US printable ASCII, not an IME.
fn ascii_for(code: u8, shift: bool, caps_lock: bool) -> Option<u8> {
    let letter = match code {
        0x10..=0x19 => Some(b"qwertyuiop"[usize::from(code - 0x10)]),
        0x1e..=0x26 => Some(b"asdfghjkl"[usize::from(code - 0x1e)]),
        0x2c..=0x32 => Some(b"zxcvbnm"[usize::from(code - 0x2c)]),
        _ => None,
    };
    if let Some(letter) = letter {
        return Some(if shift ^ caps_lock {
            letter.to_ascii_uppercase()
        } else {
            letter
        });
    }
    if (0x02..=0x0b).contains(&code) {
        return Some((if shift { b"!@#$%^&*()" } else { b"1234567890" })[usize::from(code - 0x02)]);
    }
    let (plain, shifted) = match code {
        0x0c => (b'-', b'_'),
        0x0d => (b'=', b'+'),
        0x1a => (b'[', b'{'),
        0x1b => (b']', b'}'),
        0x27 => (b';', b':'),
        0x28 => (b'\'', b'"'),
        0x29 => (b'`', b'~'),
        0x2b => (b'\\', b'|'),
        0x33 => (b',', b'<'),
        0x34 => (b'.', b'>'),
        0x35 => (b'/', b'?'),
        0x39 => (b' ', b' '),
        _ => return None,
    };
    Some(if shift { shifted } else { plain })
}

fn launcher_available(
    view: &render_view<'_>,
    choice: launcher_choice,
    graphics: Option<surface_descriptor>,
) -> bool {
    view.launcher_enabled(choice)
        && (choice != launcher_choice::graphics
            || graphics.is_some()
            || view.all_windows().any(|window| {
                window.content() == demo_content::graphics && window.surface().is_some()
            }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum keyboard_error {
    wrong_input_kind,
    unexpected_source,
    sequence_discontinuity,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct keyboard_modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub caps_lock: bool,
}

pub struct keyboard_state {
    generation: Option<u32>,
    sequence: Option<u64>,
    held: [bool; 256],
    caps_lock: bool,
}
impl keyboard_state {
    pub const fn new() -> Self {
        Self {
            generation: None,
            sequence: None,
            held: [false; 256],
            caps_lock: false,
        }
    }
    pub fn shift(&self) -> bool {
        self.held[0x2a] || self.held[0x36]
    }
    pub fn control(&self) -> bool {
        self.held[0x1d] || self.held[0x9d]
    }
    pub fn alt(&self) -> bool {
        self.held[0x38] || self.held[0xb8]
    }
    pub const fn caps_lock(&self) -> bool {
        self.caps_lock
    }
    pub fn modifiers(&self) -> keyboard_modifiers {
        keyboard_modifiers {
            shift: self.shift(),
            control: self.control(),
            alt: self.alt(),
            caps_lock: self.caps_lock(),
        }
    }
    pub fn consume(
        &mut self,
        input: &rite_physical_input_v1,
    ) -> Result<Option<(u8, bool)>, keyboard_error> {
        if input.is_pointer() || input.is_pointer_loss() {
            return Err(keyboard_error::wrong_input_kind);
        }
        if self
            .generation
            .is_some_and(|generation| generation != input.source_generation())
        {
            return Err(keyboard_error::unexpected_source);
        }
        if self
            .sequence
            .is_some_and(|last| last.checked_add(1) != Some(input.sequence()))
        {
            return Err(keyboard_error::sequence_discontinuity);
        }
        if input.is_loss() {
            self.generation = Some(input.new_generation());
            self.sequence = None;
            self.held.fill(false);
            self.caps_lock = false;
            return Ok(None);
        }
        self.generation = Some(input.source_generation());
        self.sequence = Some(input.sequence());
        let code = input.make_code();
        let index = usize::from(code) + if input.extended() { 128 } else { 0 };
        let was_pressed = self.held[index];
        self.held[index] = input.pressed();
        if input.pressed() && !was_pressed && !input.extended() && code == 0x3a {
            self.caps_lock = !self.caps_lock;
        }
        Ok((input.pressed() && !was_pressed).then_some((code, input.extended())))
    }
}

impl Default for keyboard_state {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anomalous_desktop_workspace::{
        appearance_choice, client_endpoint, client_grants, client_origin, desktop, desktop_control,
        minimum_window_height, minimum_window_width, update_ticket,
    };

    fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) {
        model.begin(control, &ticket).unwrap();
        model.attest_presented(control, &ticket).unwrap();
        model.publish(control, ticket).ok().unwrap();
    }

    fn apply(
        model: &mut desktop,
        control: &desktop_control,
        endpoint: &client_endpoint,
        command: desktop_command,
    ) {
        let revision = model.observe(endpoint).unwrap().revision();
        let ticket = model.prepare(endpoint, revision, command).unwrap();
        publish(model, control, ticket);
    }

    fn key(sequence: u64, code: u8, extended: bool, pressed: bool) -> rite_physical_input_v1 {
        rite_physical_input_v1::key(1, sequence, code, extended, pressed).unwrap()
    }

    #[test]
    fn keyboard_update_publishes_cancellation_for_unmapped_and_rejected_armed_input_only() {
        for armed in [false, true] {
            for rejected in [false, true] {
                let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
                let endpoint = model
                    .bind_client(&control, client_origin::human, client_grants::all)
                    .unwrap();
                publish(&mut model, &control, initial);
                if armed {
                    let revision = model.observe(&endpoint).unwrap().revision();
                    let ticket = model
                        .prepare_batch(
                            &endpoint,
                            revision,
                            &[
                                desktop_command::set_pointer { x: 600, y: 700 },
                                desktop_command::set_pointer_buttons(1),
                                desktop_command::arm_pointer(
                                    anomalous_desktop_workspace::desktop_target::launcher,
                                ),
                            ],
                        )
                        .unwrap();
                    publish(&mut model, &control, ticket);
                }
                let view = model.observe(&endpoint).unwrap();
                let id = view.focused_window().unwrap().id();
                let bounds = view.window(id).unwrap().bounds();
                let revision = view.revision();
                let command = rejected.then_some(desktop_command::move_window {
                    window: id,
                    x: u32::MAX,
                    y: u32::MAX,
                });
                let ticket =
                    prepare_keyboard_update(&mut model, &endpoint, revision, command, armed)
                        .unwrap();
                assert_eq!(ticket.is_some(), armed);
                assert_eq!(model.observe(&endpoint).unwrap().revision(), revision);
                if let Some(ticket) = ticket {
                    assert_eq!(model.candidate(&ticket).unwrap().pointer_armed(), None);
                    publish(&mut model, &control, ticket);
                }
                let view = model.observe(&endpoint).unwrap();
                assert_eq!(view.revision(), revision + u64::from(armed));
                assert_eq!(view.pointer_armed(), None);
                assert_eq!(view.window(id).unwrap().bounds(), bounds);
            }
        }
    }

    #[test]
    fn keyboard_action_clears_arm_in_one_update_and_unexpected_failures_propagate() {
        let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
        let endpoint = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        let revision = model.observe(&endpoint).unwrap().revision();
        let ticket = model
            .prepare_batch(
                &endpoint,
                revision,
                &[
                    desktop_command::set_pointer { x: 600, y: 700 },
                    desktop_command::set_pointer_buttons(1),
                    desktop_command::arm_pointer(
                        anomalous_desktop_workspace::desktop_target::launcher,
                    ),
                ],
            )
            .unwrap();
        publish(&mut model, &control, ticket);
        let revision = model.observe(&endpoint).unwrap().revision();
        let command = Some(desktop_command::set_theme(theme_mode::dark));
        assert!(
            prepare_keyboard_update(&mut model, &endpoint, revision - 1, command, true).is_err()
        );
        assert!(model.observe(&endpoint).unwrap().pointer_armed().is_some());
        let ticket = prepare_keyboard_update(&mut model, &endpoint, revision, command, true)
            .unwrap()
            .unwrap();
        publish(&mut model, &control, ticket);
        let view = model.observe(&endpoint).unwrap();
        assert_eq!(view.revision(), revision + 1);
        assert_eq!(view.pointer_armed(), None);
        assert_eq!(view.theme(), theme_mode::dark);
    }

    #[test]
    fn repeats_are_suppressed_until_exact_release_and_modifier_tracks_both_shifts() {
        let mut state = keyboard_state::new();
        assert_eq!(
            state.consume(&key(1, 0x2a, false, true)),
            Ok(Some((0x2a, false)))
        );
        assert!(state.shift());
        assert_eq!(state.consume(&key(2, 0x2a, false, true)), Ok(None));
        state.consume(&key(3, 0x36, false, true)).unwrap();
        state.consume(&key(4, 0x2a, false, false)).unwrap();
        assert!(state.shift());
        state.consume(&key(5, 0x36, false, false)).unwrap();
        assert!(!state.shift());
        assert_eq!(
            state.consume(&key(6, 0x2a, false, true)),
            Ok(Some((0x2a, false)))
        );
    }

    #[test]
    fn declared_loss_resets_modifiers_repeats_and_source_sequence() {
        let mut state = keyboard_state::new();
        state.consume(&key(8, 0x2a, false, true)).unwrap();
        let loss = rite_physical_input_v1::loss(1, 2, 9, 14, 6).unwrap();
        assert_eq!(state.consume(&loss), Ok(None));
        assert!(!state.shift());
        let fresh = rite_physical_input_v1::key(2, 0, 0x2a, false, true).unwrap();
        assert_eq!(state.consume(&fresh), Ok(Some((0x2a, false))));
        assert!(state.shift());
    }

    #[test]
    fn missing_duplicate_or_unannounced_generation_fails_closed() {
        for bad in [
            key(10, 0x2e, false, true),
            key(12, 0x2e, false, true),
            rite_physical_input_v1::key(2, 11, 0x2e, false, true).unwrap(),
        ] {
            let mut state = keyboard_state::new();
            state.consume(&key(10, 0x2a, false, true)).unwrap();
            assert!(state.consume(&bad).is_err());
        }
        let mut exhausted = keyboard_state::new();
        exhausted
            .consume(&key(u64::MAX, 0x2a, false, true))
            .unwrap();
        assert_eq!(
            exhausted.consume(&key(0, 0x2e, false, true)),
            Err(keyboard_error::sequence_discontinuity)
        );
    }

    #[test]
    fn extended_key_identity_does_not_alias_nonextended_shift() {
        let mut state = keyboard_state::new();
        state.consume(&key(0, 0x2a, true, true)).unwrap();
        assert!(!state.shift());
        assert_eq!(
            state.consume(&key(1, 0x2a, false, true)),
            Ok(Some((0x2a, false)))
        );
        assert!(state.shift());
    }

    #[test]
    fn commands_use_canonical_focus_bounds_and_configuration() {
        let (mut model, control, ticket) = desktop::new_demo(1024, 768).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        model.begin(&control, &ticket).unwrap();
        model.attest_presented(&control, &ticket).unwrap();
        model.publish(&control, ticket).ok().unwrap();
        let view = model.observe(&human).unwrap();
        let focused = view.focused_window().unwrap();
        let bounds = focused.floating_bounds();
        assert_eq!(
            command_for(&view, 0x0f, false, false),
            Some(desktop_command::focus(view.next_focus().unwrap()))
        );
        assert_eq!(
            command_for(&view, 0x4d, true, false),
            Some(desktop_command::move_window {
                window: focused.id(),
                x: bounds.x() + 16,
                y: bounds.y()
            })
        );
        assert_eq!(
            command_for(&view, 0x4d, true, true),
            Some(desktop_command::resize_window {
                window: focused.id(),
                width: bounds.width() + 16,
                height: bounds.height()
            })
        );
        assert_eq!(
            command_for(&view, 0x14, false, false),
            Some(desktop_command::set_layout(layout_mode::horizontal))
        );
        assert_eq!(
            command_for(&view, 0x2e, false, false),
            Some(desktop_command::set_theme(theme_mode::dark))
        );
        assert_eq!(
            command_for(&view, 0x20, false, false),
            Some(desktop_command::set_density(density_mode::compact))
        );
        assert_eq!(command_for(&view, 0x13, false, false), None);
        assert_eq!(command_for(&view, 0x58, false, false), None);
    }

    #[test]
    fn appearance_open_selection_and_activation_use_shared_canonical_commands() {
        let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        let view = model.observe(&human).unwrap();
        assert_eq!(
            command_for(&view, 0x19, false, false),
            Some(desktop_command::open_appearance)
        );
        for code in [0x40, 0x1c, 0x39] {
            assert_eq!(command_for(&view, code, false, false), None);
        }
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::open_appearance,
        );
        let view = model.observe(&human).unwrap();
        let window = view.focused_window().unwrap();
        let id = window.id();
        let selected = window.appearance_selection().unwrap();
        let settings = view.theme_settings();
        assert!(appearance_controls_visible(&view, id));
        for shift in [false, true] {
            assert_eq!(
                command_for(&view, 0x40, false, shift),
                Some(desktop_command::select_appearance {
                    window: id,
                    choice: if shift {
                        selected.previous()
                    } else {
                        selected.next()
                    },
                })
            );
        }
        let command = command_for(&view, 0x40, false, false).unwrap();
        apply(&mut model, &control, &human, command);
        let view = model.observe(&human).unwrap();
        assert_eq!(view.theme_settings(), settings);
        assert_eq!(
            view.window(id).unwrap().appearance_selection(),
            Some(selected.next())
        );

        apply(
            &mut model,
            &control,
            &human,
            desktop_command::select_appearance {
                window: id,
                choice: appearance_choice::dark,
            },
        );
        let view = model.observe(&human).unwrap();
        for code in [0x1c, 0x39] {
            assert_eq!(
                command_for(&view, code, false, false),
                Some(desktop_command::activate_appearance {
                    window: id,
                    choice: appearance_choice::dark
                })
            );
        }
        let command = command_for(&view, 0x1c, false, false).unwrap();
        apply(&mut model, &control, &human, command);
        assert_eq!(model.observe(&human).unwrap().theme(), theme_mode::dark);
    }

    #[test]
    fn appearance_keyboard_controls_require_visible_fitting_focused_content() {
        let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::open_appearance,
        );
        let id = model
            .observe(&human)
            .unwrap()
            .focused_window()
            .unwrap()
            .id();
        let view = model.observe(&human).unwrap();
        for code in [0x40, 0x1c, 0x39] {
            assert_eq!(command_for(&view, code, true, false), None);
        }
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::resize_window {
                window: id,
                width: minimum_window_width,
                height: minimum_window_height,
            },
        );
        let view = model.observe(&human).unwrap();
        assert!(!appearance_controls_visible(&view, id));
        for code in [0x40, 0x1c, 0x39] {
            assert_eq!(command_for(&view, code, false, false), None);
            assert_eq!(command_for(&view, code, false, true), None);
        }
        apply(&mut model, &control, &human, desktop_command::minimize(id));
        let view = model.observe(&human).unwrap();
        for code in [0x40, 0x1c, 0x39] {
            assert_eq!(command_for(&view, code, false, false), None);
        }
        assert_eq!(
            command_for(&view, 0x19, false, false),
            Some(desktop_command::open_appearance)
        );
    }

    #[test]
    fn disabled_appearance_selection_can_be_traversed_but_not_activated() {
        let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::open_appearance,
        );
        let view = model.observe(&human).unwrap();
        let id = view.focused_window().unwrap().id();
        let mut settings = view.theme_settings();
        settings.overrides.decorations = Some(view.theme_definition().decorations);
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::set_theme_settings(settings),
        );
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::select_appearance {
                window: id,
                choice: appearance_choice::dark,
            },
        );
        let view = model.observe(&human).unwrap();
        assert!(appearance_controls_visible(&view, id));
        assert!(!view.appearance_enabled(appearance_choice::dark));
        for code in [0x1c, 0x39] {
            assert_eq!(command_for(&view, code, false, false), None);
        }
        assert_eq!(
            command_for(&view, 0x40, false, false),
            Some(desktop_command::select_appearance {
                window: id,
                choice: appearance_choice::high_contrast
            })
        );
    }

    #[test]
    fn only_bounded_policy_failures_allow_the_native_owner_to_continue() {
        for error in [
            desktop_error::invalid_geometry,
            desktop_error::window_capacity_exhausted,
            desktop_error::invalid_theme,
            desktop_error::layout_too_small,
        ] {
            assert!(expected_policy_rejection(error));
        }
        for error in [
            desktop_error::grant_denied,
            desktop_error::client_denied,
            desktop_error::control_denied,
            desktop_error::stale_revision,
            desktop_error::stale_window,
            desktop_error::stale_ticket,
            desktop_error::backpressure,
            desktop_error::unavailable,
            desktop_error::sequence_exhausted,
            desktop_error::wrong_phase,
        ] {
            assert!(!expected_policy_rejection(error));
        }
    }
    #[test]
    fn launcher_keyboard_navigation_is_modal_and_escape_returns_to_normal_handling() {
        let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        let open = command_for(&model.observe(&human).unwrap(), 0x3b, false, false).unwrap();
        assert_eq!(open, desktop_command::toggle_launcher);
        apply(&mut model, &control, &human, open);
        let view = model.observe(&human).unwrap();
        let id = view.focused_window().unwrap().id();
        let bounds = view.window(id).unwrap().bounds();
        assert!(view.launcher_open());
        for (code, extended, shift, expected) in [
            (0x48, true, false, launcher_choice::editor),
            (0x4b, true, true, launcher_choice::editor),
            (0x50, true, false, launcher_choice::inspector),
            (0x4d, true, true, launcher_choice::inspector),
            (0x0f, false, false, launcher_choice::inspector),
            (0x0f, false, true, launcher_choice::editor),
        ] {
            assert_eq!(
                command_for(&view, code, extended, shift),
                Some(desktop_command::select_launcher(expected))
            );
        }
        for code in [
            0x02, 0x03, 0x10, 0x11, 0x13, 0x14, 0x19, 0x1e, 0x20, 0x21, 0x22, 0x2d, 0x2e, 0x2f,
            0x31, 0x32, 0x3c, 0x3d, 0x40, 0x44, 0x57, 0x58,
        ] {
            for shift in [false, true] {
                assert_eq!(
                    command_for(&view, code, false, shift),
                    None,
                    "modal scan code {code}"
                );
            }
        }
        assert_eq!(view.window(id).unwrap().bounds(), bounds);
        let dismiss = command_for(&view, 0x01, false, false).unwrap();
        assert_eq!(dismiss, desktop_command::dismiss_launcher);
        apply(&mut model, &control, &human, dismiss);
        let view = model.observe(&human).unwrap();
        assert!(!view.launcher_open());
        // With no popup, Escape is left to the service's normal exit path.
        assert_eq!(command_for(&view, 0x01, false, false), None);
        assert!(matches!(
            command_for(&view, 0x4d, true, false),
            Some(desktop_command::move_window { .. })
        ));
    }

    #[test]
    fn launcher_keyboard_activates_every_choice_with_only_graphics_receiving_a_surface() {
        let pixels = [0u8; 3];
        let frame = anomalous_window_surface::frame::try_new(1, 1, 3, "Public", &pixels).unwrap();
        for choice in launcher_choice::all {
            let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
            let human = model
                .bind_client(&control, client_origin::human, client_grants::all)
                .unwrap();
            publish(&mut model, &control, initial);
            apply(
                &mut model,
                &control,
                &human,
                desktop_command::toggle_launcher,
            );
            apply(
                &mut model,
                &control,
                &human,
                desktop_command::select_launcher(choice),
            );
            let view = model.observe(&human).unwrap();
            let expected = desktop_command::activate_launcher {
                choice,
                graphics: (choice == launcher_choice::graphics).then_some(frame.descriptor()),
            };
            for (code, extended) in [(0x1c, false), (0x1c, true), (0x39, false)] {
                assert_eq!(
                    command_for_with_graphics(
                        &view,
                        code,
                        extended,
                        false,
                        Some(frame.descriptor())
                    ),
                    Some(expected)
                );
            }
            if choice == launcher_choice::graphics {
                assert_eq!(command_for(&view, 0x1c, false, false), None);
            }
            apply(&mut model, &control, &human, expected);
            let view = model.observe(&human).unwrap();
            assert!(!view.launcher_open());
            assert_eq!(view.focused_window().unwrap().content(), choice.content());
            let id = view.focused_window().unwrap().id();
            if choice == launcher_choice::graphics {
                assert_eq!(view.window(id).unwrap().surface(), Some(frame.descriptor()));
                apply(
                    &mut model,
                    &control,
                    &human,
                    desktop_command::toggle_launcher,
                );
                apply(
                    &mut model,
                    &control,
                    &human,
                    desktop_command::select_launcher(choice),
                );
                // Existing Graphics may be focused without replacing its immutable surface.
                let command =
                    command_for(&model.observe(&human).unwrap(), 0x1c, false, false).unwrap();
                apply(&mut model, &control, &human, command);
                assert_eq!(
                    model
                        .observe(&human)
                        .unwrap()
                        .focused_window()
                        .unwrap()
                        .id(),
                    id
                );
            }
        }
    }

    #[test]
    fn disabled_launcher_choice_can_be_selected_but_enter_does_not_reserve_a_transaction() {
        let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        for _ in 2..anomalous_desktop_workspace::window_capacity {
            apply(
                &mut model,
                &control,
                &human,
                desktop_command::create {
                    content: demo_content::activity,
                    workspace: workspace_id::first,
                    bounds: window_rect::try_new(120, 140, 480, 320).unwrap(),
                },
            );
        }
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::toggle_launcher,
        );
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::select_launcher(launcher_choice::appearance),
        );
        let view = model.observe(&human).unwrap();
        assert!(!view.launcher_enabled(launcher_choice::appearance));
        let revision = view.revision();
        let command = command_for(&view, 0x1c, false, false);
        assert_eq!(command, None);
        assert!(
            prepare_keyboard_update(&mut model, &human, revision, command, false)
                .unwrap()
                .is_none()
        );
        assert_eq!(model.observe(&human).unwrap().revision(), revision);
        // F1 closes; its next opening resets selection to the first public app.
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::toggle_launcher,
        );
        apply(
            &mut model,
            &control,
            &human,
            desktop_command::toggle_launcher,
        );
        assert_eq!(
            model.observe(&human).unwrap().launcher_selection(),
            launcher_choice::welcome
        );
    }
}

#[cfg(test)]
mod editor_input_tests;

/// Intercept a native close button as an editor request. The admitted release
/// first focuses its target; only the complete nonmodal transaction can authorize
/// that request. A bounded-policy fallback publishes cursor state alone.
#[cfg(feature = "native_remote")]
pub fn prepare_remote_pointer_update(
    model: &mut desktop,
    endpoint: &client_endpoint,
    revision: u64,
    window: anomalous_desktop_workspace::window_id,
    commands: &[desktop_command],
    pointer: &mut pointer_state,
) -> Result<(update_ticket, bool), desktop_error> {
    if !(2..=4).contains(&commands.len())
        || !matches!(commands[0], desktop_command::set_pointer { .. })
        || !matches!(commands[1], desktop_command::set_pointer_buttons(_))
    {
        return Err(desktop_error::invalid_batch);
    }
    let mut prepared = [desktop_command::cancel_pointer; 4];
    prepared[..commands.len()].copy_from_slice(commands);
    let mut close = false;
    for command in &mut prepared[..commands.len()] {
        if *command == desktop_command::close(window) {
            *command = desktop_command::focus(window);
            close = true;
        }
    }
    match model.prepare_batch(endpoint, revision, &prepared[..commands.len()]) {
        Ok(ticket) => {
            let candidate = model.candidate(&ticket)?;
            let close = close
                && !candidate.launcher_open()
                && candidate
                    .focused_window()
                    .is_some_and(|focused| focused.id() == window && !focused.minimized());
            Ok((ticket, close))
        }
        Err(error) if expected_policy_rejection(error) => {
            pointer.cancel_capture();
            model
                .prepare_batch(endpoint, revision, &commands[..2])
                .map(|ticket| (ticket, false))
        }
        Err(error) => Err(error),
    }
}

/// Physical native-editor keys select the same semantic actions as the local
/// scratch adapter. Routing/focus and the remote action fence remain the caller's
/// responsibility. Modifiers cannot become text or desktop shortcuts.
#[cfg(feature = "native_remote")]
pub fn remote_editor_action_for(
    code: u8,
    extended: bool,
    modifiers: keyboard_modifiers,
) -> Option<anomalous_editor_contract::remote_protocol::remote_action> {
    use anomalous_editor_contract::{
        interactive_editor_action as edit, remote_protocol::remote_action,
    };
    if code == 0x10 && !extended && modifiers.control && !modifiers.alt && !modifiers.shift {
        return Some(remote_action::request_close);
    }
    Some(match editor_action_for(code, extended, modifiers)? {
        editor_action::save_unavailable => remote_action::save,
        action => remote_action::edit(match action {
            editor_action::insert(byte) => edit::insert(byte),
            editor_action::backspace => edit::backspace,
            editor_action::delete_forward => edit::delete_forward,
            editor_action::move_caret { motion, extend } => edit::move_caret { motion, extend },
            editor_action::select_all => edit::select_all,
            editor_action::collapse_selection => edit::collapse_selection,
            editor_action::undo => edit::undo,
            editor_action::input_reset => edit::input_reset,
            _ => edit::unsupported_key,
        }),
    })
}

#[cfg(all(test, feature = "native_remote"))]
mod remote_input_tests {
    use super::*;
    use anomalous_desktop_compositor::{chrome_bounds, chrome_target, hit_test_chrome};
    use anomalous_desktop_workspace::{
        client_grants, client_origin, desktop_control,
        native_bridge::{
            editor_disclosure, native_editor_frame, native_editor_input,
            native_remote::{remote_editor_bridge, remote_route_binding},
        },
        window_id,
    };
    use anomalous_editor_contract::{
        interactive_editor_action as edit, interactive_editor_session, remote_editor_session,
        remote_protocol::{remote_action, remote_request},
    };

    fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) {
        model.begin(control, &ticket).unwrap();
        model.attest_presented(control, &ticket).unwrap();
        model.publish(control, ticket).ok().unwrap();
    }

    struct pointer_fixture {
        model: desktop,
        control: desktop_control,
        human: client_endpoint,
        bridge: remote_editor_bridge,
        input: native_editor_input,
        pointer: pointer_state,
        sequence: u64,
        other: window_id,
    }
    impl pointer_fixture {
        fn new() -> Self {
            let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
            let human = model
                .bind_client(&control, client_origin::human, client_grants::all)
                .unwrap();
            publish(&mut model, &control, initial);
            let other = model
                .observe(&human)
                .unwrap()
                .all_windows()
                .find(|window| window.content() == demo_content::welcome)
                .unwrap()
                .id();
            let route = remote_route_binding::try_new(1, [1, 1], [2, 1], [1, 1], [2, 1]).unwrap();
            let session = remote_editor_session::new(
                1,
                interactive_editor_session::from_open(1, 1, 1, b"").unwrap(),
            )
            .unwrap();
            let mut port = model.provision_remote_editor(&control, route).unwrap();
            port.admit_initial_checked(route, 1, &session.observation().encode())
                .unwrap();
            let frame = native_editor_frame::from_audit(port.audit_bytes().unwrap()).unwrap();
            let revision = model.observe(&human).unwrap().revision();
            let ticket = model
                .prepare(
                    &human,
                    revision,
                    desktop_command::open_native_editor(frame.descriptor()),
                )
                .unwrap();
            let window = model
                .candidate(&ticket)
                .unwrap()
                .focused_window()
                .unwrap()
                .id();
            publish(&mut model, &control, ticket);
            let bridge = model
                .bind_remote_editor(
                    &control,
                    &human,
                    window,
                    port,
                    frame,
                    editor_disclosure::public_fixture,
                )
                .ok()
                .unwrap();
            let input = bridge.bind_input(&model, &control, &human).unwrap();
            Self {
                model,
                control,
                human,
                bridge,
                input,
                pointer: pointer_state::new(),
                sequence: 0,
                other,
            }
        }
        fn apply(&mut self, command: desktop_command) {
            let revision = self.model.observe(&self.human).unwrap().revision();
            let ticket = self.model.prepare(&self.human, revision, command).unwrap();
            publish(&mut self.model, &self.control, ticket);
        }
        fn commands(&mut self, commands: &[desktop_command]) -> Option<remote_request> {
            let revision = self.model.observe(&self.human).unwrap().revision();
            let (ticket, close) = prepare_remote_pointer_update(
                &mut self.model,
                &self.human,
                revision,
                self.bridge.window(),
                commands,
                &mut self.pointer,
            )
            .unwrap();
            // Match the guest: publication must finish before action admission.
            publish(&mut self.model, &self.control, ticket);
            close.then(|| {
                let revision = self.model.observe(&self.human).unwrap().revision();
                self.bridge
                    .submit_action(
                        &mut self.model,
                        &self.input,
                        revision,
                        remote_action::request_close,
                    )
                    .unwrap()
            })
        }
        fn pointer_input(&mut self, dx: i16, dy: i16, buttons: u8) -> Option<remote_request> {
            let input = rite_physical_input_v1::pointer(1, self.sequence, dx, dy, buttons).unwrap();
            self.sequence += 1;
            let batch = self
                .pointer
                .consume(&self.model.observe(&self.human).unwrap(), &input)
                .unwrap()
                .unwrap();
            self.commands(batch.commands())
        }
    }

    #[test]
    fn native_pointer_close_focuses_before_request_for_focused_and_unfocused_windows() {
        for focused in [true, false] {
            let mut f = pointer_fixture::new();
            if !focused {
                f.apply(desktop_command::focus(f.other));
            }
            let view = f.model.observe(&f.human).unwrap();
            let target = chrome_target::close(f.bridge.window());
            let bounds = chrome_bounds(&view, target).unwrap();
            let (x, y) = (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
            assert_eq!(hit_test_chrome(&view, x, y), target);
            let (dx, dy) = (
                x as i16 - view.pointer().x() as i16,
                y as i16 - view.pointer().y() as i16,
            );
            assert!(f.pointer_input(dx, dy, 0).is_none());
            assert!(f.pointer_input(0, 0, 1).is_none());
            let before = f.model.observe(&f.human).unwrap();
            assert_eq!(
                before.focused_window().unwrap().id() == f.bridge.window(),
                focused
            );
            let request = f.pointer_input(0, 0, 0).unwrap();
            assert_eq!(request.action(), remote_action::request_close);
            assert_eq!(request.action_id(), 1);
            let view = f.model.observe(&f.human).unwrap();
            assert_eq!(view.focused_window().unwrap().id(), f.bridge.window());
            assert!(view.window(f.bridge.window()).is_some());
        }
    }

    #[test]
    fn rejected_native_close_batch_publishes_only_cursor_and_never_submits_action() {
        let mut f = pointer_fixture::new();
        f.apply(desktop_command::focus(f.other));
        let window = f.bridge.window();
        assert!(
            f.commands(&[
                desktop_command::set_pointer { x: 700, y: 160 },
                desktop_command::set_pointer_buttons(0),
                desktop_command::resize_window {
                    window,
                    width: 0,
                    height: 200
                },
                desktop_command::close(window),
            ])
            .is_none()
        );
        let view = f.model.observe(&f.human).unwrap();
        assert_eq!(view.focused_window().unwrap().id(), f.other);
        assert_eq!((view.pointer().x(), view.pointer().y()), (700, 160));
        assert!(view.window(window).is_some());
        // No action ID or fence was consumed by the rejected full transaction.
        let request = f
            .commands(&[
                desktop_command::set_pointer { x: 700, y: 160 },
                desktop_command::set_pointer_buttons(0),
                desktop_command::close(window),
            ])
            .unwrap();
        assert_eq!(request.action_id(), 1);
    }

    #[test]
    fn modal_or_denied_native_close_never_admits_a_remote_request() {
        let mut f = pointer_fixture::new();
        let commands = [
            desktop_command::set_pointer { x: 700, y: 160 },
            desktop_command::set_pointer_buttons(0),
            desktop_command::close(f.bridge.window()),
        ];
        let observer = f
            .model
            .bind_client(&f.control, client_origin::machine, client_grants::observe)
            .unwrap();
        let revision = f.model.observe(&f.human).unwrap().revision();
        assert!(matches!(
            prepare_remote_pointer_update(
                &mut f.model,
                &observer,
                revision,
                f.bridge.window(),
                &commands,
                &mut f.pointer,
            ),
            Err(desktop_error::grant_denied)
        ));
        f.apply(desktop_command::toggle_launcher);
        assert!(f.commands(&commands).is_none());
        assert!(f.model.observe(&f.human).unwrap().launcher_open());
        f.apply(desktop_command::dismiss_launcher);
        assert_eq!(f.commands(&commands).unwrap().action_id(), 1);
    }

    #[test]
    fn native_input_distinguishes_save_close_text_and_modifiers() {
        let plain = keyboard_modifiers {
            shift: false,
            control: false,
            alt: false,
            caps_lock: false,
        };
        let control = keyboard_modifiers {
            control: true,
            ..plain
        };
        assert_eq!(
            remote_editor_action_for(0x10, false, plain),
            Some(remote_action::edit(edit::insert(b'q')))
        );
        assert_eq!(
            remote_editor_action_for(0x10, false, control),
            Some(remote_action::request_close)
        );
        assert_eq!(
            remote_editor_action_for(0x1f, false, control),
            Some(remote_action::save)
        );
        assert_eq!(remote_editor_action_for(0x1d, false, plain), None);
        assert_eq!(
            remote_editor_action_for(
                0x10,
                false,
                keyboard_modifiers {
                    alt: true,
                    ..control
                }
            ),
            Some(remote_action::edit(edit::unsupported_key))
        );
        assert_eq!(
            remote_editor_action_for(
                0x4b,
                true,
                keyboard_modifiers {
                    shift: true,
                    ..plain
                }
            ),
            Some(remote_action::edit(edit::move_caret {
                motion: caret_motion::left,
                extend: true
            }))
        );
    }
}
