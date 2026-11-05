//! Bounded user-space gestures. Geometry and control picking come from the same
//! compositor that draws the pixels; only canonical commands mutate the model.
use anomalous_abi::rite_physical_input_v1;
use anomalous_desktop_compositor::{chrome_bounds, chrome_target, hit_test_chrome};
use anomalous_desktop_workspace::{
    demo_content, desktop_command, launcher_choice, layout_mode, minimum_window_height,
    minimum_window_width, render_view, surface_descriptor, window_id, window_rect,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum pointer_error {
    wrong_input_kind,
    unexpected_source,
    sequence_discontinuity,
}

#[derive(Debug, Eq, PartialEq)]
pub enum pointer_read_error<E> {
    input(pointer_error),
    poll(E),
}

/// A ready motion run cannot monopolize input delivery or delay presentation
/// indefinitely. Button/source boundaries are retained as one owned lookahead.
pub const pointer_motion_batch_capacity: usize = 16;

/// One physical edge or bounded steady motion run becomes one transaction.
pub struct command_batch {
    commands: [desktop_command; 4],
    length: usize,
}
impl command_batch {
    fn new(x: u32, y: u32, buttons: u8) -> Self {
        let mut batch = Self {
            commands: [desktop_command::set_pointer { x, y }; 4],
            length: 1,
        };
        batch.push(desktop_command::set_pointer_buttons(buttons));
        batch
    }
    fn push(&mut self, command: desktop_command) {
        assert!(self.length < self.commands.len());
        self.commands[self.length] = command;
        self.length += 1;
    }
    pub fn commands(&self) -> &[desktop_command] {
        &self.commands[..self.length]
    }
}

#[derive(Clone, Copy)]
struct pointer_capture {
    window: window_id,
    bounds: window_rect,
    start_x: u32,
    start_y: u32,
    resize: bool,
    expected_revision: u64,
}

#[derive(Clone, Copy)]
struct control_capture {
    target: chrome_target,
    expected_revision: u64,
}

#[derive(Clone)]
pub struct pointer_state {
    generation: Option<u32>,
    sequence: Option<u64>,
    start_sequence_required: bool,
    buttons: u8,
    await_release: bool,
    capture: Option<pointer_capture>,
    control: Option<control_capture>,
    graphics: Option<surface_descriptor>,
}
impl Default for pointer_state {
    fn default() -> Self {
        Self::new()
    }
}
impl pointer_state {
    /// A newly bound source must report all buttons up before any gesture.
    /// A button carried into a fresh session is never treated as a new press.
    pub const fn new() -> Self {
        Self {
            generation: None,
            sequence: None,
            start_sequence_required: false,
            buttons: 0,
            await_release: true,
            capture: None,
            control: None,
            graphics: None,
        }
    }
    /// Configure the descriptor whose immutable pixels the caller retains.
    pub const fn with_graphics_surface(graphics: surface_descriptor) -> Self {
        Self {
            graphics: Some(graphics),
            ..Self::new()
        }
    }
    /// A keyboard action, failed transaction, or another actor changing the
    /// model ends the gesture. A held button cannot silently recapture a window.
    pub fn cancel_capture(&mut self) {
        self.capture = None;
        self.control = None;
        self.await_release = self.buttons != 0;
    }
    /// Call only after validating an accepted keyboard make or loss. A mapped
    /// keyboard action clears the arm in its own transaction; otherwise this
    /// cancellation command publishes that same canonical change on its own.
    pub fn cancel_for_keyboard(&mut self, view: &render_view<'_>) -> Option<desktop_command> {
        self.cancel_capture();
        view.pointer_armed()
            .map(|_| desktop_command::cancel_pointer)
    }
    pub fn consume(
        &mut self,
        view: &render_view<'_>,
        input: &rite_physical_input_v1,
    ) -> Result<Option<command_batch>, pointer_error> {
        self.validate_source(input)?;
        if input.is_pointer_loss() {
            self.generation = Some(input.new_generation());
            self.sequence = None;
            self.start_sequence_required = true;
            self.buttons = 0;
            self.cancel_capture();
            self.await_release = true;
            let position = view.pointer();
            return Ok(Some(command_batch::new(position.x(), position.y(), 0)));
        }
        self.record_pointer(input);
        let position = view.pointer();
        let x = offset(
            position.x(),
            i64::from(input.delta_x()),
            0,
            view.width() - 1,
        );
        let y = offset(
            position.y(),
            i64::from(input.delta_y()),
            0,
            view.height() - 1,
        );
        Ok(Some(self.consume_position(view, x, y, input.buttons())))
    }

    /// Consume only immediately ready motion with the same established source
    /// and button mask, preserving every raw sequence and each delta's clamp.
    /// Presses, releases, loss, and fresh-source observations remain singleton
    /// transactions. Any other input is returned untouched for the caller's
    /// next iteration; polling never waits and stops after sixteen packets.
    ///
    /// The caller must retain the returned lookahead and publish the resulting
    /// ordinary model transaction before consuming it. Read/validation failure
    /// leaves this adapter unchanged; already-read input must not be retried.
    pub fn consume_ready<E>(
        &mut self,
        view: &render_view<'_>,
        first: &rite_physical_input_v1,
        mut poll: impl FnMut() -> Result<Option<rite_physical_input_v1>, E>,
    ) -> Result<(Option<command_batch>, Option<rite_physical_input_v1>), pointer_read_error<E>>
    {
        let mut next = self.clone();
        let steady = first.is_pointer()
            && next.generation == Some(first.source_generation())
            && next.sequence.is_some()
            && !next.start_sequence_required
            && next.buttons == first.buttons();
        if !steady {
            let batch = next
                .consume(view, first)
                .map_err(pointer_read_error::input)?;
            *self = next;
            return Ok((batch, None));
        }
        next.validate_source(first)
            .map_err(pointer_read_error::input)?;
        next.record_pointer(first);
        let position = view.pointer();
        let mut x = offset(
            position.x(),
            i64::from(first.delta_x()),
            0,
            view.width() - 1,
        );
        let mut y = offset(
            position.y(),
            i64::from(first.delta_y()),
            0,
            view.height() - 1,
        );
        let mut lookahead = None;
        for _ in 1..pointer_motion_batch_capacity {
            let Some(input) = poll().map_err(pointer_read_error::poll)? else {
                break;
            };
            if !input.is_pointer()
                || input.source_generation() != first.source_generation()
                || input.buttons() != first.buttons()
            {
                lookahead = Some(input);
                break;
            }
            next.validate_source(&input)
                .map_err(pointer_read_error::input)?;
            next.record_pointer(&input);
            x = offset(x, i64::from(input.delta_x()), 0, view.width() - 1);
            y = offset(y, i64::from(input.delta_y()), 0, view.height() - 1);
        }
        // Geometry is anchored to the original capture, not intermediate
        // candidates. A run creates one successor and advances its expected
        // revision once; no unacknowledged model state is ever published.
        let batch = next.consume_position(view, x, y, first.buttons());
        *self = next;
        Ok((Some(batch), lookahead))
    }

    fn validate_source(&self, input: &rite_physical_input_v1) -> Result<(), pointer_error> {
        if !input.is_pointer() && !input.is_pointer_loss() {
            return Err(pointer_error::wrong_input_kind);
        }
        if self
            .generation
            .is_some_and(|old| old != input.source_generation())
        {
            return Err(pointer_error::unexpected_source);
        }
        if self
            .sequence
            .is_some_and(|last| last.checked_add(1) != Some(input.sequence()))
        {
            return Err(pointer_error::sequence_discontinuity);
        }
        if self.start_sequence_required && input.sequence() != 0 {
            return Err(pointer_error::sequence_discontinuity);
        }
        Ok(())
    }

    fn record_pointer(&mut self, input: &rite_physical_input_v1) {
        self.generation = Some(input.source_generation());
        self.sequence = Some(input.sequence());
        self.start_sequence_required = false;
    }

    fn consume_position(
        &mut self,
        view: &render_view<'_>,
        x: u32,
        y: u32,
        buttons: u8,
    ) -> command_batch {
        let left = buttons == 1;
        let pressed = left && self.buttons == 0 && !self.await_release;
        let released = buttons == 0 && self.buttons == 1 && !self.await_release;
        self.buttons = buttons;
        let mut batch = command_batch::new(x, y, buttons);
        // Every non-left button, including a solitary right button, closes the
        // gesture until the complete chord is released. A partial release must
        // never become a fresh left press.
        if buttons != 0 && !left {
            self.cancel_capture();
            return batch;
        }
        // Dismiss on the outside press, then suppress its entire gesture. The
        // release must not reach the window or panel control now underneath.
        if pressed
            && view.launcher_open()
            && !matches!(
                hit_test_chrome(view, x, y),
                chrome_target::launcher
                    | chrome_target::launcher_item(_)
                    | chrome_target::launcher_background
            )
        {
            batch.push(desktop_command::dismiss_launcher);
            self.cancel_capture();
            return batch;
        }
        if let Some(mut control) = self.control {
            let valid = control.expected_revision == view.revision()
                && view.pointer_armed() == Some(control.target)
                && control_available(view, control.target, self.graphics);
            if !valid {
                self.cancel_capture();
            } else if left {
                // Retain the original identity outside its bounds. The painter
                // depresses it only while the topmost hit is this same target.
                batch.push(desktop_command::arm_pointer(control.target));
                control.expected_revision = view.revision().saturating_add(1);
                self.control = Some(control);
            } else if released && hit_test_chrome(view, x, y) == control.target {
                activate(view, control.target, self.graphics, &mut batch);
            }
        } else if let Some(mut capture) = self.capture {
            let valid = capture.expected_revision == view.revision()
                && view.layout() == layout_mode::floating
                && view.window(capture.window).is_some_and(|window| {
                    window.focused()
                        && !window.minimized()
                        && !window.maximized()
                        && window.workspace() == view.active_workspace()
                });
            if !valid {
                self.cancel_capture();
            } else if left {
                let dx = i64::from(x) - i64::from(capture.start_x);
                let dy = i64::from(y) - i64::from(capture.start_y);
                let area = view.work_area();
                if capture.resize {
                    batch.push(desktop_command::resize_window {
                        window: capture.window,
                        width: offset(
                            capture.bounds.width(),
                            dx,
                            minimum_window_width,
                            area.x() + area.width() - capture.bounds.x(),
                        ),
                        height: offset(
                            capture.bounds.height(),
                            dy,
                            minimum_window_height,
                            area.y() + area.height() - capture.bounds.y(),
                        ),
                    });
                } else {
                    batch.push(desktop_command::move_window {
                        window: capture.window,
                        x: offset(
                            capture.bounds.x(),
                            dx,
                            area.x(),
                            area.x() + area.width() - capture.bounds.width(),
                        ),
                        y: offset(
                            capture.bounds.y(),
                            dy,
                            area.y(),
                            area.y() + area.height() - capture.bounds.height(),
                        ),
                    });
                }
                capture.expected_revision = view.revision().saturating_add(1);
                self.capture = Some(capture);
            }
        } else if pressed {
            let target = hit_test_chrome(view, x, y);
            use chrome_target::*;
            match target {
                window(id) | titlebar(id) | resize(id) => {
                    batch.push(desktop_command::focus(id));
                    if matches!(target, titlebar(_) | resize(_))
                        && view.layout() == layout_mode::floating
                        && let Some(item) = view.window(id).filter(|item| !item.maximized())
                    {
                        self.capture = Some(pointer_capture {
                            window: id,
                            bounds: item.floating_bounds(),
                            start_x: x,
                            start_y: y,
                            resize: matches!(target, resize(_)),
                            expected_revision: view.revision().saturating_add(1),
                        });
                    }
                }
                desktop => {}
                _ if control_available(view, target, self.graphics) => {
                    batch.push(desktop_command::arm_pointer(target));
                    self.control = Some(control_capture {
                        target,
                        expected_revision: view.revision().saturating_add(1),
                    });
                }
                _ => {}
            }
        }
        if buttons == 0 {
            self.cancel_capture();
        }
        batch
    }
}

/// Pixel visibility comes from the shared compositor layout; semantic action
/// validity remains enforced by the model again during preparation.
fn control_available(
    view: &render_view<'_>,
    target: chrome_target,
    graphics: Option<surface_descriptor>,
) -> bool {
    if chrome_bounds(view, target).is_none() {
        return false;
    }
    use chrome_target::*;
    match target {
        desktop | window(_) | titlebar(_) | resize(_) | launcher_background | theme | density
        | font | cursor | layout => false,
        maximize(_) => view.layout() == layout_mode::floating,
        launcher => true,
        launcher_item(choice) => {
            view.launcher_open() && super::launcher_available(view, choice, graphics)
        }
        appearance_launcher => {
            view.all_windows().count() < anomalous_desktop_workspace::window_capacity
                || view
                    .all_windows()
                    .any(|item| item.content() == demo_content::appearance)
        }
        appearance_choice(_, choice) => view.appearance_enabled(choice),
        _ => true,
    }
}

fn activate(
    view: &render_view<'_>,
    target: chrome_target,
    graphics: Option<surface_descriptor>,
    batch: &mut command_batch,
) {
    use chrome_target::*;
    match target {
        minimize(id) => batch.push(desktop_command::minimize(id)),
        maximize(id) => {
            if view.window(id).is_some_and(|item| item.maximized()) {
                batch.push(desktop_command::restore_geometry(id));
                batch.push(desktop_command::focus(id));
            } else {
                batch.push(desktop_command::maximize(id));
            }
        }
        close(id) => batch.push(desktop_command::close(id)),
        task(id) => {
            batch.push(desktop_command::restore(id));
            batch.push(desktop_command::focus(id));
        }
        workspace(id) => batch.push(desktop_command::switch_workspace(id)),
        appearance_launcher => batch.push(desktop_command::open_appearance),
        appearance_choice(id, choice) => {
            batch.push(desktop_command::activate_appearance { window: id, choice });
        }
        launcher => batch.push(desktop_command::toggle_launcher),
        launcher_item(choice) => {
            batch.push(desktop_command::select_launcher(choice));
            batch.push(desktop_command::activate_launcher {
                choice,
                graphics: if choice == launcher_choice::graphics {
                    graphics
                } else {
                    None
                },
            });
        }
        desktop | window(_) | titlebar(_) | resize(_) | launcher_background | theme | density
        | font | cursor | layout => {}
    }
}

fn offset(value: u32, delta: i64, minimum: u32, maximum: u32) -> u32 {
    (i64::from(value) + delta).clamp(i64::from(minimum), i64::from(maximum)) as u32
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use anomalous_desktop_workspace::{
        appearance_choice, client_endpoint, client_grants, client_origin, desktop, desktop_control,
        theme_mode, update_ticket, window_capacity, workspace_id,
    };
    use std::vec::Vec;

    struct fixture {
        model: desktop,
        control: desktop_control,
        endpoint: client_endpoint,
        pointer: pointer_state,
        generation: u32,
        sequence: u64,
    }
    impl fixture {
        fn new() -> Self {
            let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
            let endpoint = model
                .bind_client(&control, client_origin::human, client_grants::all)
                .unwrap();
            publish(&mut model, &control, initial);
            let mut fixture = Self {
                model,
                control,
                endpoint,
                pointer: pointer_state::new(),
                generation: 1,
                sequence: 0,
            };
            fixture.input(0, 0, 0);
            fixture
        }
        fn view(&self) -> render_view<'_> {
            self.model.observe(&self.endpoint).unwrap()
        }
        fn apply(&mut self, command: desktop_command) {
            let revision = self.view().revision();
            let ticket = self
                .model
                .prepare(&self.endpoint, revision, command)
                .unwrap();
            publish(&mut self.model, &self.control, ticket);
        }
        fn input(&mut self, dx: i16, dy: i16, buttons: u8) -> Vec<desktop_command> {
            let event =
                rite_physical_input_v1::pointer(self.generation, self.sequence, dx, dy, buttons)
                    .unwrap();
            self.sequence += 1;
            let view = self.model.observe(&self.endpoint).unwrap();
            let batch = self.pointer.consume(&view, &event).unwrap().unwrap();
            let commands = batch.commands().to_vec();
            assert_eq!(commands[1], desktop_command::set_pointer_buttons(buttons));
            let revision = view.revision();
            let ticket = self
                .model
                .prepare_batch(&self.endpoint, revision, batch.commands())
                .unwrap();
            publish(&mut self.model, &self.control, ticket);
            assert_eq!(self.view().pointer_buttons(), buttons);
            commands
        }
        fn at(&mut self, x: u32, y: u32, buttons: u8) -> Vec<desktop_command> {
            let pointer = self.view().pointer();
            let dx = i16::try_from(i64::from(x) - i64::from(pointer.x())).unwrap();
            let dy = i16::try_from(i64::from(y) - i64::from(pointer.y())).unwrap();
            self.input(dx, dy, buttons)
        }
        fn point(&self, target: chrome_target) -> (u32, u32) {
            let bounds = chrome_bounds(&self.view(), target).unwrap();
            let point = (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
            assert_eq!(hit_test_chrome(&self.view(), point.0, point.1), target);
            point
        }
        fn press(&mut self, target: chrome_target) -> Vec<desktop_command> {
            let (x, y) = self.point(target);
            self.at(x, y, 1)
        }
        fn click(&mut self, target: chrome_target) -> Vec<desktop_command> {
            self.input(0, 0, 0);
            self.press(target);
            self.input(0, 0, 0)
        }
        fn focused(&self) -> window_id {
            self.view().focused_window().unwrap().id()
        }
        fn ready_motion(&mut self, motion: &[(i16, i16)], buttons: u8) -> Vec<desktop_command> {
            assert!(!motion.is_empty() && motion.len() <= pointer_motion_batch_capacity);
            let mut events = motion.iter().enumerate().map(|(index, (dx, dy))| {
                rite_physical_input_v1::pointer(
                    self.generation,
                    self.sequence + index as u64,
                    *dx,
                    *dy,
                    buttons,
                )
                .unwrap()
            });
            let first = events.next().unwrap();
            let view = self.model.observe(&self.endpoint).unwrap();
            let revision = view.revision();
            let (batch, lookahead) = self
                .pointer
                .consume_ready(&view, &first, || Ok::<_, ()>(events.next()))
                .unwrap();
            assert!(lookahead.is_none() && events.next().is_none());
            let batch = batch.unwrap();
            let commands = batch.commands().to_vec();
            let ticket = self
                .model
                .prepare_batch(&self.endpoint, revision, batch.commands())
                .unwrap();
            // No cursor or semantic update publishes before acknowledgment.
            assert_eq!(self.view().revision(), revision);
            publish(&mut self.model, &self.control, ticket);
            assert_eq!(self.view().revision(), revision + 1);
            self.sequence += motion.len() as u64;
            commands
        }
    }
    fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) {
        model.begin(control, &ticket).unwrap();
        model.attest_presented(control, &ticket).unwrap();
        assert!(model.publish(control, ticket).is_ok());
    }

    fn same_pointer_state(left: &pointer_state, right: &pointer_state) {
        assert_eq!(
            (
                left.generation,
                left.sequence,
                left.start_sequence_required,
                left.buttons,
                left.await_release
            ),
            (
                right.generation,
                right.sequence,
                right.start_sequence_required,
                right.buttons,
                right.await_release
            )
        );
        let capture = |state: &pointer_state| {
            state.capture.map(|value| {
                (
                    value.window,
                    value.bounds,
                    value.start_x,
                    value.start_y,
                    value.resize,
                    value.expected_revision,
                )
            })
        };
        let control = |state: &pointer_state| {
            state
                .control
                .map(|value| (value.target, value.expected_revision))
        };
        assert_eq!(left.graphics, right.graphics);
        assert_eq!(capture(left), capture(right));
        assert_eq!(control(left), control(right));
    }

    #[test]
    fn ready_motion_clamps_every_delta_before_the_next_and_publishes_once() {
        let mut f = fixture::new();
        f.at(5, 5, 0);
        assert_eq!(
            f.ready_motion(&[(-10, -10), (10, 10)], 0)[0],
            desktop_command::set_pointer { x: 10, y: 10 }
        );
        f.at(1020, 765, 0);
        assert_eq!(
            f.ready_motion(&[(10, 10), (-10, -10)], 0)[0],
            desktop_command::set_pointer { x: 1013, y: 757 }
        );
        assert_eq!(f.pointer.sequence, Some(f.sequence - 1));
    }

    #[test]
    fn ready_motion_preserves_original_drag_and_resize_anchors() {
        let motion = [
            (i16::MAX, i16::MAX),
            (-31, -47),
            (i16::MIN, i16::MIN),
            (73, 59),
        ];
        for resize in [false, true] {
            let mut grouped = fixture::new();
            let mut serial = fixture::new();
            let id = grouped.focused();
            let target = if resize {
                chrome_target::resize(id)
            } else {
                chrome_target::titlebar(id)
            };
            grouped.press(target);
            serial.press(target);
            grouped.ready_motion(&motion, 1);
            for (dx, dy) in motion {
                serial.input(dx, dy, 1);
            }
            assert_eq!(grouped.view().pointer(), serial.view().pointer());
            assert_eq!(
                grouped.view().window(id).unwrap().bounds(),
                serial.view().window(id).unwrap().bounds()
            );
            assert_eq!(grouped.pointer.sequence, serial.pointer.sequence);
            // The once-advanced expected revision must admit the next motion.
            assert_eq!(grouped.input(11, 17, 1), serial.input(11, 17, 1));
            assert!(grouped.pointer.capture.is_some());
        }
    }

    #[test]
    fn ready_motion_retains_original_control_and_release_activates_once() {
        let mut f = fixture::new();
        f.press(chrome_target::launcher);
        let before = f.view().launcher_open();
        let commands = f.ready_motion(&[(200, 0), (-200, 0)], 1);
        assert_eq!(
            commands[2],
            desktop_command::arm_pointer(chrome_target::launcher)
        );
        assert_eq!(f.view().launcher_open(), before);
        assert_eq!(f.view().pointer_armed(), Some(chrome_target::launcher));
        assert_eq!(f.input(0, 0, 0)[2], desktop_command::toggle_launcher);
        assert_eq!(f.input(0, 0, 0).len(), 2);
        assert!(f.view().launcher_open());
    }

    #[test]
    fn ready_motion_stops_before_every_input_boundary_and_returns_exact_lookahead() {
        for boundary in 0..7 {
            let mut f = fixture::new();
            f.press(chrome_target::launcher);
            let first = rite_physical_input_v1::pointer(1, f.sequence, 1, 0, 1).unwrap();
            let next = match boundary {
                0 => rite_physical_input_v1::pointer(1, f.sequence + 1, 0, 0, 0),
                1 => rite_physical_input_v1::pointer(1, f.sequence + 1, 0, 0, 3),
                2 => rite_physical_input_v1::pointer_loss(1, 2, f.sequence + 1, f.sequence + 1, 1),
                3 => rite_physical_input_v1::key(1, 0, 0x30, false, true),
                4 => rite_physical_input_v1::key(1, 0, 0x30, false, false),
                5 => rite_physical_input_v1::loss(1, 2, 0, 0, 1),
                _ => rite_physical_input_v1::pointer(2, 0, 0, 0, 1),
            }
            .unwrap();
            let expected = *next.encode();
            let mut next = Some(next);
            let mut polls = 0;
            let view = f.model.observe(&f.endpoint).unwrap();
            let revision = view.revision();
            let (batch, lookahead) = f
                .pointer
                .consume_ready(&view, &first, || {
                    polls += 1;
                    assert_eq!(polls, 1, "must not read across the boundary");
                    Ok::<_, ()>(next.take())
                })
                .unwrap();
            assert_eq!(polls, 1);
            assert_eq!(*lookahead.unwrap().encode(), expected);
            assert_eq!(f.pointer.sequence, Some(f.sequence));
            assert_eq!(
                batch.unwrap().commands()[2],
                desktop_command::arm_pointer(chrome_target::launcher)
            );
            assert_eq!(f.pointer.control.unwrap().expected_revision, revision + 1);
            assert_eq!(view.revision(), revision);
        }
    }

    #[test]
    fn ready_motion_caps_at_sixteen_without_reading_a_seventeenth_packet() {
        let mut f = fixture::new();
        let first = rite_physical_input_v1::pointer(1, f.sequence, 1, 0, 0).unwrap();
        let mut polls = 0;
        let view = f.model.observe(&f.endpoint).unwrap();
        let start = view.pointer().x();
        let (batch, lookahead) = f
            .pointer
            .consume_ready(&view, &first, || {
                polls += 1;
                Ok::<_, ()>(Some(
                    rite_physical_input_v1::pointer(1, f.sequence + polls, 1, 0, 0).unwrap(),
                ))
            })
            .unwrap();
        assert_eq!(polls, 15);
        assert!(lookahead.is_none());
        assert_eq!(f.pointer.sequence, Some(f.sequence + 15));
        assert_eq!(
            batch.unwrap().commands()[0],
            desktop_command::set_pointer {
                x: start + 16,
                y: view.pointer().y()
            }
        );
    }

    #[test]
    fn ready_motion_never_polls_after_fresh_source_button_edges_or_loss() {
        let mut f = fixture::new();
        for buttons in [1, 3, 2, 0] {
            let input = rite_physical_input_v1::pointer(1, f.sequence, 0, 0, buttons).unwrap();
            let view = f.model.observe(&f.endpoint).unwrap();
            let (batch, next) = f
                .pointer
                .consume_ready(&view, &input, || -> Result<_, ()> {
                    panic!("button edge must not poll");
                })
                .unwrap();
            assert!(batch.is_some() && next.is_none());
            f.sequence += 1;
        }
        let loss = rite_physical_input_v1::pointer_loss(1, 2, f.sequence, f.sequence, 1).unwrap();
        let view = f.model.observe(&f.endpoint).unwrap();
        f.pointer
            .consume_ready(&view, &loss, || -> Result<_, ()> {
                panic!("loss must not poll")
            })
            .unwrap();
        let fresh = rite_physical_input_v1::pointer(2, 0, 0, 0, 0).unwrap();
        f.pointer
            .consume_ready(&view, &fresh, || -> Result<_, ()> {
                panic!("fresh generation must not poll")
            })
            .unwrap();
        let mut unbound = pointer_state::new();
        unbound
            .consume_ready(&view, &fresh, || -> Result<_, ()> {
                panic!("unbound source must not poll")
            })
            .unwrap();
    }

    #[test]
    fn rejected_ready_motion_preserves_source_capture_and_control_state() {
        for control in [false, true] {
            for failure in 0..3 {
                let mut f = fixture::new();
                let target = if control {
                    chrome_target::launcher
                } else {
                    chrome_target::titlebar(f.focused())
                };
                f.press(target);
                let before = f.pointer.clone();
                let first = rite_physical_input_v1::pointer(1, f.sequence, 3, 4, 1).unwrap();
                let mut polls = 0;
                let view = f.model.observe(&f.endpoint).unwrap();
                let error = f
                    .pointer
                    .consume_ready(&view, &first, || {
                        polls += 1;
                        if failure == 0 {
                            Err("read failure")
                        } else {
                            let sequence = if failure == 1 {
                                f.sequence
                            } else {
                                f.sequence + 2
                            };
                            Ok(Some(
                                rite_physical_input_v1::pointer(1, sequence, 5, 6, 1).unwrap(),
                            ))
                        }
                    })
                    .err()
                    .unwrap();
                assert_eq!(polls, 1);
                assert_eq!(
                    error,
                    if failure == 0 {
                        pointer_read_error::poll("read failure")
                    } else {
                        pointer_read_error::input(pointer_error::sequence_discontinuity)
                    }
                );
                same_pointer_state(&before, &f.pointer);
            }
        }
    }

    #[test]
    fn ready_motion_keeps_stale_revision_and_chord_cancellation_in_force() {
        let mut f = fixture::new();
        let id = f.focused();
        f.press(chrome_target::titlebar(id));
        let bounds = f.view().window(id).unwrap().bounds();
        f.apply(desktop_command::set_theme(theme_mode::dark));
        assert_eq!(f.ready_motion(&[(1, 2), (3, 4)], 1).len(), 2);
        assert_eq!(f.view().window(id).unwrap().bounds(), bounds);
        assert!(f.pointer.await_release && f.pointer.capture.is_none());
        f.input(0, 0, 0);
        f.press(chrome_target::launcher);
        f.input(0, 0, 3);
        assert_eq!(f.ready_motion(&[(1, 0), (-1, 0)], 3).len(), 2);
        assert!(f.pointer.await_release && f.pointer.control.is_none());
        f.input(0, 0, 1);
        f.input(0, 0, 0);
        assert_eq!(f.view().theme(), theme_mode::dark);
    }

    #[test]
    fn fresh_source_requires_neutral_before_any_control_or_drag_press() {
        for target in [
            chrome_target::launcher,
            chrome_target::titlebar(fixture::new().focused()),
        ] {
            let mut f = fixture::new();
            f.pointer = pointer_state::new();
            let (x, y) = f.point(target);
            for buttons in [1, 3, 2, 1] {
                assert_eq!(f.at(x, y, buttons).len(), 2);
                assert_eq!(f.view().pointer_armed(), None);
                assert!(f.pointer.control.is_none());
                assert!(f.pointer.capture.is_none());
                assert!(f.pointer.await_release);
            }
            assert_eq!(f.input(0, 0, 0).len(), 2);
            assert!(!f.pointer.await_release);
            assert_eq!(f.press(target).len(), 3);
            assert!(f.pointer.control.is_some() || f.pointer.capture.is_some());
        }
    }

    #[test]
    fn accepted_keyboard_make_and_loss_cancel_local_and_canonical_control_state() {
        for loss in [false, true] {
            let mut f = fixture::new();
            f.press(chrome_target::launcher);
            let mut keyboard = super::super::keyboard_state::new();
            let input = if loss {
                rite_physical_input_v1::loss(1, 2, 0, 0, 1).unwrap()
            } else {
                rite_physical_input_v1::key(1, 0, 0x30, false, true).unwrap()
            };
            let accepted = keyboard.consume(&input).unwrap();
            assert!(accepted.is_some() || input.is_loss());
            let view = f.model.observe(&f.endpoint).unwrap();
            let cancellation = f.pointer.cancel_for_keyboard(&view);
            assert_eq!(cancellation, Some(desktop_command::cancel_pointer));
            assert!(f.pointer.control.is_none());
            assert!(f.pointer.await_release);
            let revision = view.revision();
            let command = accepted.and_then(|(code, extended)| {
                super::super::command_for(&view, code, extended, keyboard.shift())
            });
            assert_eq!(command, None);
            let ticket = super::super::prepare_keyboard_update(
                &mut f.model,
                &f.endpoint,
                revision,
                command,
                cancellation.is_some(),
            )
            .unwrap()
            .unwrap();
            publish(&mut f.model, &f.control, ticket);
            assert_eq!(f.view().pointer_armed(), None);
            assert_eq!(f.input(0, 0, 1).len(), 2);
            assert_eq!(f.input(0, 0, 0).len(), 2);
            assert!(!f.view().launcher_open());
            let view = f.model.observe(&f.endpoint).unwrap();
            assert_eq!(f.pointer.cancel_for_keyboard(&view), None);
        }
    }

    #[test]
    fn title_drag_clamps_cursor_and_geometry_without_changing_size() {
        let mut f = fixture::new();
        let id = f.focused();
        let original = f.view().window(id).unwrap().bounds();
        let commands = f.press(chrome_target::titlebar(id));
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[2], desktop_command::focus(id));
        assert!(f.pointer.capture.is_some());
        f.input(i16::MAX, i16::MAX, 1);
        let area = f.view().work_area();
        let moved = f.view().window(id).unwrap().bounds();
        assert_eq!(
            (f.view().pointer().x(), f.view().pointer().y()),
            (1023, 767)
        );
        assert_eq!(
            (moved.x(), moved.y()),
            (
                area.x() + area.width() - original.width(),
                area.y() + area.height() - original.height()
            )
        );
        assert_eq!(
            (moved.width(), moved.height()),
            (original.width(), original.height())
        );
        f.input(i16::MIN, i16::MIN, 1);
        let moved = f.view().window(id).unwrap().bounds();
        assert_eq!((moved.x(), moved.y()), (area.x(), area.y()));
        assert_eq!((f.view().pointer().x(), f.view().pointer().y()), (0, 0));
        f.input(0, 0, 0);
        assert!(f.pointer.capture.is_none());
        assert_eq!(f.input(100, 100, 0).len(), 2);
        assert_eq!(f.view().window(id).unwrap().bounds(), moved);
    }

    #[test]
    fn resize_grip_clamps_to_workarea_and_minimum_size() {
        let mut f = fixture::new();
        let id = f.focused();
        let original = f.view().window(id).unwrap().bounds();
        f.press(chrome_target::resize(id));
        assert!(f.pointer.capture.unwrap().resize);
        f.input(i16::MAX, i16::MAX, 1);
        let area = f.view().work_area();
        let bounds = f.view().window(id).unwrap().bounds();
        assert_eq!((bounds.x(), bounds.y()), (original.x(), original.y()));
        assert_eq!(
            (bounds.x() + bounds.width(), bounds.y() + bounds.height()),
            (area.x() + area.width(), area.y() + area.height())
        );
        f.input(i16::MIN, i16::MIN, 1);
        let bounds = f.view().window(id).unwrap().bounds();
        assert_eq!(
            (bounds.width(), bounds.height()),
            (minimum_window_width, minimum_window_height)
        );
    }

    #[test]
    fn decorations_minimize_restore_maximize_and_close_the_selected_window() {
        let mut f = fixture::new();
        let id = f.focused();
        let floating = f.view().window(id).unwrap().floating_bounds();
        let commands = f.click(chrome_target::minimize(id));
        assert_eq!(commands[2], desktop_command::minimize(id));
        assert!(f.view().window(id).unwrap().minimized());
        let commands = f.click(chrome_target::task(id));
        assert_eq!(
            commands[2..],
            [desktop_command::restore(id), desktop_command::focus(id)]
        );
        assert!(f.view().window(id).unwrap().focused());
        f.click(chrome_target::maximize(id));
        assert!(f.view().window(id).unwrap().maximized());
        assert_eq!(f.view().window(id).unwrap().bounds(), f.view().work_area());
        f.click(chrome_target::maximize(id));
        assert!(!f.view().window(id).unwrap().maximized());
        assert_eq!(f.view().window(id).unwrap().bounds(), floating);
        f.click(chrome_target::close(id));
        assert!(f.view().window(id).is_none());
        assert!(f.pointer.capture.is_none());
    }

    #[test]
    fn panel_launcher_and_appearance_use_the_same_commands_as_keyboard() {
        for (target, code) in [
            (chrome_target::launcher, 0x3b),
            (chrome_target::appearance_launcher, 0x19),
        ] {
            let mut f = fixture::new();
            let expected = super::super::command_for(&f.view(), code, false, false).unwrap();
            let commands = f.click(target);
            assert_eq!(commands[2..], [expected]);
            assert!(f.pointer.capture.is_none());
        }
        let mut f = fixture::new();
        let commands = f.click(chrome_target::workspace(workspace_id::second));
        assert_eq!(
            commands[2],
            desktop_command::switch_workspace(workspace_id::second)
        );
        assert_eq!(f.view().active_workspace(), workspace_id::second);
        assert_eq!(f.view().windows().count(), 0);
        for removed in [
            chrome_target::theme,
            chrome_target::density,
            chrome_target::font,
            chrome_target::cursor,
            chrome_target::layout,
        ] {
            assert_eq!(chrome_bounds(&f.view(), removed), None);
        }
    }

    #[test]
    fn revision_changes_and_explicit_keyboard_cancellation_end_drag_until_release() {
        for explicit in [false, true] {
            let mut f = fixture::new();
            let id = f.focused();
            f.press(chrome_target::titlebar(id));
            let original = f.view().window(id).unwrap().bounds();
            if explicit {
                f.pointer.cancel_capture();
            } else {
                f.apply(desktop_command::set_theme(theme_mode::dark));
            }
            assert_eq!(f.input(20, 10, 1).len(), 2);
            assert_eq!(f.view().window(id).unwrap().bounds(), original);
            assert!(f.pointer.capture.is_none());
            assert_eq!(f.input(20, 10, 1).len(), 2);
            f.input(0, 0, 0);
            f.press(chrome_target::titlebar(id));
            assert!(f.pointer.capture.is_some());
            assert_eq!(f.input(20, 10, 1).len(), 3);
            assert_ne!(f.view().window(id).unwrap().bounds(), original);
        }
    }

    #[test]
    fn generation_loss_cancels_drag_and_requires_all_buttons_released() {
        let mut f = fixture::new();
        let id = f.focused();
        f.press(chrome_target::titlebar(id));
        let loss = rite_physical_input_v1::pointer_loss(1, 2, f.sequence, f.sequence, 1).unwrap();
        let view = f.model.observe(&f.endpoint).unwrap();
        let position = view.pointer();
        let revision = view.revision();
        assert_eq!(view.pointer_buttons(), 1);
        let batch = f.pointer.consume(&view, &loss).unwrap().unwrap();
        assert_eq!(
            batch.commands(),
            [
                desktop_command::set_pointer {
                    x: position.x(),
                    y: position.y(),
                },
                desktop_command::set_pointer_buttons(0),
            ]
        );
        let ticket = f
            .model
            .prepare_batch(&f.endpoint, revision, batch.commands())
            .unwrap();
        assert_eq!(f.view().pointer_buttons(), 1);
        assert_eq!(f.model.candidate(&ticket).unwrap().pointer_buttons(), 0);
        publish(&mut f.model, &f.control, ticket);
        assert_eq!(f.view().pointer(), position);
        assert_eq!(f.view().pointer_buttons(), 0);
        assert!(f.pointer.await_release);
        assert!(f.pointer.capture.is_none());
        f.generation = 2;
        f.sequence = 0;
        assert_eq!(f.input(0, 0, 1).len(), 2);
        assert_eq!(f.input(0, 0, 2).len(), 2);
        assert_eq!(f.input(0, 0, 1).len(), 2);
        assert!(f.pointer.await_release);
        f.input(0, 0, 0);
        assert!(!f.pointer.await_release);
        assert_eq!(f.press(chrome_target::titlebar(id)).len(), 3);
        assert!(f.pointer.capture.is_some());
    }

    #[test]
    fn wrong_kinds_generations_gaps_and_duplicates_cannot_change_gesture_identity() {
        let f = fixture::new();
        let view = f.view();
        let mut state = pointer_state::new();
        assert_eq!(
            state
                .consume(
                    &view,
                    &rite_physical_input_v1::key(1, 0, 0x1c, false, true).unwrap()
                )
                .err(),
            Some(pointer_error::wrong_input_kind)
        );
        assert_eq!(
            state
                .consume(&view, &rite_physical_input_v1::loss(1, 2, 0, 0, 1).unwrap())
                .err(),
            Some(pointer_error::wrong_input_kind)
        );
        state
            .consume(
                &view,
                &rite_physical_input_v1::pointer(1, 10, 0, 0, 0).unwrap(),
            )
            .unwrap();
        for (generation, sequence, expected) in [
            (2, 11, pointer_error::unexpected_source),
            (1, 10, pointer_error::sequence_discontinuity),
            (1, 12, pointer_error::sequence_discontinuity),
        ] {
            let event = rite_physical_input_v1::pointer(generation, sequence, 0, 0, 1).unwrap();
            assert_eq!(state.consume(&view, &event).err(), Some(expected));
            assert_eq!(state.generation, Some(1));
            assert_eq!(state.sequence, Some(10));
            assert_eq!(state.buttons, 0);
        }
        assert!(
            state
                .consume(
                    &view,
                    &rite_physical_input_v1::pointer(1, 11, 0, 0, 0).unwrap()
                )
                .is_ok()
        );
    }

    #[test]
    fn full_capacity_launcher_opens_and_reuses_apps_but_disabled_rows_consume_input() {
        let mut f = fixture::new();
        for _ in 2..window_capacity {
            f.apply(desktop_command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds: window_rect::try_new(120, 140, 480, 320).unwrap(),
            });
        }
        assert_eq!(
            f.click(chrome_target::launcher)[2],
            desktop_command::toggle_launcher
        );
        assert!(f.view().launcher_open());
        assert_eq!(f.view().all_windows().count(), window_capacity);
        assert!(!f.view().launcher_enabled(launcher_choice::appearance));
        let disabled = chrome_bounds(
            &f.view(),
            chrome_target::launcher_item(launcher_choice::appearance),
        )
        .unwrap();
        let (x, y) = (
            disabled.x + disabled.width / 2,
            disabled.y + disabled.height / 2,
        );
        assert_eq!(
            hit_test_chrome(&f.view(), x, y),
            chrome_target::launcher_background
        );
        assert_eq!(f.at(x, y, 1).len(), 2);
        let (x, y) = f.point(chrome_target::launcher_item(launcher_choice::welcome));
        assert_eq!(f.at(x, y, 1).len(), 2);
        assert_eq!(f.input(0, 0, 0).len(), 2);
        assert!(f.view().launcher_open());
        assert_eq!(
            f.click(chrome_target::launcher_item(launcher_choice::welcome))
                .len(),
            4
        );
        assert!(!f.view().launcher_open());
        assert_eq!(f.view().all_windows().count(), window_capacity);
        assert_eq!(
            f.view().focused_window().unwrap().content(),
            demo_content::welcome
        );
    }

    #[test]
    fn appearance_activation_requires_release_on_original_control_and_occurs_once() {
        let mut f = fixture::new();
        f.apply(desktop_command::open_appearance);
        let id = f.focused();
        let target = chrome_target::appearance_choice(id, appearance_choice::dark);
        let commands = f.press(target);
        assert_eq!(commands[2], desktop_command::arm_pointer(target));
        assert_eq!(f.view().pointer_armed(), Some(target));
        assert_eq!(f.view().theme(), theme_mode::light);
        assert_eq!(f.input(0, 0, 1)[2], desktop_command::arm_pointer(target));
        assert_eq!(f.view().theme(), theme_mode::light);
        let commands = f.input(0, 0, 0);
        assert_eq!(
            commands[2],
            desktop_command::activate_appearance {
                window: id,
                choice: appearance_choice::dark,
            }
        );
        assert_eq!(f.view().theme(), theme_mode::dark);
        assert_eq!(f.view().pointer_armed(), None);
        assert_eq!(
            f.view().window(id).unwrap().appearance_selection(),
            Some(appearance_choice::dark)
        );
        assert!(f.pointer.capture.is_none());
        assert!(f.pointer.control.is_none());
        assert_eq!(f.input(0, 0, 0).len(), 2);
        assert_eq!(f.view().theme(), theme_mode::dark);
    }

    #[test]
    fn leaving_control_allows_reentry_but_never_switches_target() {
        for reenter in [false, true] {
            let mut f = fixture::new();
            let original = chrome_target::launcher;
            let other = chrome_target::workspace(workspace_id::second);
            let (original_x, original_y) = f.point(original);
            let (other_x, other_y) = f.point(other);
            f.press(original);
            assert_eq!(
                f.at(other_x, other_y, 1)[2],
                desktop_command::arm_pointer(original)
            );
            assert_eq!(f.view().pointer_armed(), Some(original));
            assert!(!f.view().launcher_open());
            if reenter {
                assert_eq!(
                    f.at(original_x, original_y, 1)[2],
                    desktop_command::arm_pointer(original)
                );
            }
            let commands = f.input(0, 0, 0);
            assert_eq!(commands.len(), if reenter { 3 } else { 2 });
            assert_eq!(f.view().launcher_open(), reenter);
            assert_eq!(f.view().active_workspace(), workspace_id::first);
            assert_eq!(f.view().pointer_armed(), None);
        }
    }

    #[test]
    fn press_elsewhere_then_enter_control_cannot_activate_on_release() {
        for start in [
            chrome_target::desktop,
            chrome_target::window(fixture::new().focused()),
        ] {
            let mut f = fixture::new();
            if start == chrome_target::desktop {
                assert_eq!(hit_test_chrome(&f.view(), 0, 0), start);
                assert_eq!(f.at(0, 0, 1).len(), 2);
            } else {
                f.press(start);
            }
            let (x, y) = f.point(chrome_target::launcher);
            assert_eq!(f.at(x, y, 1).len(), 2);
            assert_eq!(f.view().pointer_armed(), None);
            assert_eq!(f.input(0, 0, 0).len(), 2);
            assert!(!f.view().launcher_open());
        }
    }

    #[test]
    fn every_nonleft_chord_cancels_controls_and_requires_complete_release() {
        for chord in [2, 3, 4, 5, 6, 7] {
            let mut f = fixture::new();
            f.press(chrome_target::launcher);
            assert_eq!(f.input(0, 0, chord).len(), 2);
            assert_eq!(f.view().pointer_armed(), None);
            assert!(f.pointer.await_release);
            assert!(f.pointer.control.is_none());
            assert_eq!(f.input(0, 0, 1).len(), 2);
            assert_eq!(f.input(0, 0, 3).len(), 2);
            assert_eq!(f.input(0, 0, 1).len(), 2);
            assert_eq!(f.input(0, 0, 0).len(), 2);
            assert!(!f.view().launcher_open());
            assert!(!f.pointer.await_release);
            f.click(chrome_target::launcher);
            assert!(f.view().launcher_open());
        }
    }

    #[test]
    fn revision_change_or_explicit_cancellation_ends_control_until_release() {
        for explicit in [false, true] {
            let mut f = fixture::new();
            f.press(chrome_target::launcher);
            if explicit {
                f.pointer.cancel_capture();
            } else {
                f.apply(desktop_command::focus(f.focused()));
                assert_eq!(f.view().pointer_armed(), None);
            }
            assert_eq!(f.input(0, 0, 1).len(), 2);
            assert_eq!(f.input(0, 0, 0).len(), 2);
            assert!(!f.view().launcher_open());
            assert_eq!(f.view().pointer_armed(), None);
            f.click(chrome_target::launcher);
            assert!(f.view().launcher_open());
        }
    }

    #[test]
    fn replaced_window_incarnation_cannot_receive_stale_release() {
        let mut f = fixture::new();
        let old = f.focused();
        let bounds = f.view().window(old).unwrap().bounds();
        f.press(chrome_target::close(old));
        assert!(f.view().window(old).is_some());
        f.apply(desktop_command::close(old));
        f.apply(desktop_command::create {
            content: demo_content::activity,
            workspace: workspace_id::first,
            bounds,
        });
        let new = f.focused();
        assert_ne!(old, new);
        let (x, y) = f.point(chrome_target::close(new));
        assert_eq!(f.at(x, y, 0).len(), 2);
        assert!(f.view().window(new).is_some());
        assert_eq!(f.view().pointer_armed(), None);
    }

    #[test]
    fn disabled_or_hidden_target_after_press_cancels_the_original_control() {
        for hide in [false, true] {
            let mut f = fixture::new();
            f.apply(desktop_command::open_appearance);
            let id = f.focused();
            f.press(chrome_target::appearance_choice(
                id,
                appearance_choice::dark,
            ));
            if hide {
                f.apply(desktop_command::minimize(id));
            } else {
                let mut settings = f.view().theme_settings();
                settings.overrides.decorations = Some(f.view().theme_definition().decorations);
                f.apply(desktop_command::set_theme_settings(settings));
                assert!(!f.view().appearance_enabled(appearance_choice::dark));
            }
            assert_eq!(f.input(0, 0, 0).len(), 2);
            assert_eq!(f.view().theme(), theme_mode::light);
            assert_eq!(f.view().pointer_armed(), None);
        }
    }

    #[test]
    fn pointer_loss_disarms_control_and_never_activates_it() {
        let mut f = fixture::new();
        f.press(chrome_target::launcher);
        let loss = rite_physical_input_v1::pointer_loss(1, 2, f.sequence, f.sequence, 1).unwrap();
        let view = f.model.observe(&f.endpoint).unwrap();
        let revision = view.revision();
        let batch = f.pointer.consume(&view, &loss).unwrap().unwrap();
        assert_eq!(batch.commands().len(), 2);
        let ticket = f
            .model
            .prepare_batch(&f.endpoint, revision, batch.commands())
            .unwrap();
        publish(&mut f.model, &f.control, ticket);
        assert_eq!(f.view().pointer_armed(), None);
        assert!(!f.view().launcher_open());
        f.generation = 2;
        f.sequence = 0;
        assert_eq!(f.input(0, 0, 1).len(), 2);
        assert_eq!(f.input(0, 0, 0).len(), 2);
        assert!(!f.view().launcher_open());
        f.click(chrome_target::launcher);
        assert!(f.view().launcher_open());
    }

    #[test]
    fn hidden_small_or_occluded_appearance_content_never_receives_pointer_activation() {
        let mut f = fixture::new();
        f.apply(desktop_command::open_appearance);
        let id = f.focused();
        let target = chrome_target::appearance_choice(id, appearance_choice::dark);
        let (x, y) = f.point(target);
        let bounds = f.view().window(id).unwrap().bounds();
        f.apply(desktop_command::create {
            content: demo_content::activity,
            workspace: workspace_id::first,
            bounds,
        });
        let commands = f.at(x, y, 1);
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, desktop_command::activate_appearance { .. }))
        );
        assert_eq!(f.view().theme(), theme_mode::light);
        f.input(0, 0, 0);
        f.apply(desktop_command::focus(id));
        f.apply(desktop_command::resize_window {
            window: id,
            width: minimum_window_width,
            height: minimum_window_height,
        });
        assert_eq!(chrome_bounds(&f.view(), target), None);
        let commands = f.at(x, y, 1);
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, desktop_command::activate_appearance { .. }))
        );
        f.input(0, 0, 0);
        f.apply(desktop_command::minimize(id));
        assert_eq!(chrome_bounds(&f.view(), target), None);
        let commands = f.at(x, y, 1);
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, desktop_command::activate_appearance { .. }))
        );
    }

    #[test]
    fn appearance_launcher_reopens_at_capacity_but_never_attempts_impossible_creation() {
        let mut f = fixture::new();
        for _ in 2..window_capacity {
            f.apply(desktop_command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds: window_rect::try_new(120, 140, 480, 320).unwrap(),
            });
        }
        let commands = f.click(chrome_target::appearance_launcher);
        assert_eq!(commands.len(), 2);
        assert_eq!(f.view().all_windows().count(), window_capacity);

        f.apply(desktop_command::close(f.focused()));
        f.apply(desktop_command::open_appearance);
        let id = f.focused();
        f.apply(desktop_command::minimize(id));
        let commands = f.click(chrome_target::appearance_launcher);
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[2], desktop_command::open_appearance);
        assert_eq!(f.focused(), id);
        assert!(!f.view().window(id).unwrap().minimized());
        assert_eq!(f.view().all_windows().count(), window_capacity);
    }

    #[test]
    fn disabled_appearance_choice_cannot_be_activated_by_pointer() {
        let mut f = fixture::new();
        f.apply(desktop_command::open_appearance);
        let id = f.focused();
        let target = chrome_target::appearance_choice(id, appearance_choice::dark);
        let (x, y) = f.point(target);
        let mut settings = f.view().theme_settings();
        settings.overrides.decorations = Some(f.view().theme_definition().decorations);
        f.apply(desktop_command::set_theme_settings(settings));
        assert!(!f.view().appearance_enabled(appearance_choice::dark));
        assert_ne!(hit_test_chrome(&f.view(), x, y), target);
        let commands = f.at(x, y, 1);
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, desktop_command::activate_appearance { .. }))
        );
        assert_eq!(f.view().theme(), theme_mode::light);
    }

    #[test]
    fn nonleft_buttons_and_nonfloating_titles_do_not_create_drag_capture() {
        let mut f = fixture::new();
        let id = f.focused();
        let (x, y) = f.point(chrome_target::titlebar(id));
        for buttons in [2, 4, 6, 0] {
            assert_eq!(f.at(x, y, buttons).len(), 2);
            assert!(f.pointer.capture.is_none());
        }
        f.apply(desktop_command::maximize(id));
        f.press(chrome_target::titlebar(id));
        assert!(f.pointer.capture.is_none());
        f.input(0, 0, 0);
        f.apply(desktop_command::set_layout(layout_mode::horizontal));
        f.press(chrome_target::titlebar(id));
        assert!(f.pointer.capture.is_none());
        let before = f.view().window(id).unwrap().bounds();
        assert_eq!(f.input(20, 10, 1).len(), 2);
        assert_eq!(f.view().window(id).unwrap().bounds(), before);
    }

    #[test]
    fn a_declared_fresh_pointer_generation_must_begin_at_sequence_zero() {
        let f = fixture::new();
        let view = f.view();
        let mut state = pointer_state::new();
        state
            .consume(
                &view,
                &rite_physical_input_v1::pointer(1, 7, 0, 0, 0).unwrap(),
            )
            .unwrap();
        state
            .consume(
                &view,
                &rite_physical_input_v1::pointer_loss(1, 2, 8, 8, 1).unwrap(),
            )
            .unwrap();
        assert_eq!(
            state
                .consume(
                    &view,
                    &rite_physical_input_v1::pointer(2, 1, 0, 0, 0).unwrap()
                )
                .err(),
            Some(pointer_error::sequence_discontinuity)
        );
        assert_eq!(state.generation, Some(2));
        assert_eq!(state.sequence, None);
        assert!(state.await_release);
        assert!(
            state
                .consume(
                    &view,
                    &rite_physical_input_v1::pointer(2, 0, 0, 0, 0).unwrap()
                )
                .is_ok()
        );
        assert_eq!(state.sequence, Some(0));
        assert!(!state.await_release);
    }
    #[test]
    fn launcher_rows_activate_all_five_apps_only_on_matching_release_and_reuse_identity() {
        let pixels = [0u8; 3];
        let frame = anomalous_window_surface::frame::try_new(1, 1, 3, "Public", &pixels).unwrap();
        for choice in launcher_choice::all {
            let mut f = fixture::new();
            f.pointer = pointer_state::with_graphics_surface(frame.descriptor());
            f.input(0, 0, 0);
            f.click(chrome_target::launcher);
            let count = f.view().all_windows().count();
            let focus = f.focused();
            let target = chrome_target::launcher_item(choice);
            assert_eq!(f.press(target)[2], desktop_command::arm_pointer(target));
            assert!(f.view().launcher_open());
            assert_eq!(f.focused(), focus);
            assert_eq!(f.view().all_windows().count(), count);
            assert_eq!(f.input(0, 0, 1)[2], desktop_command::arm_pointer(target));
            let commands = f.input(0, 0, 0);
            assert_eq!(
                commands[2..],
                [
                    desktop_command::select_launcher(choice),
                    desktop_command::activate_launcher {
                        choice,
                        graphics: (choice == launcher_choice::graphics)
                            .then_some(frame.descriptor()),
                    }
                ]
            );
            assert!(!f.view().launcher_open());
            assert_eq!(
                f.view().focused_window().unwrap().content(),
                choice.content()
            );
            if choice == launcher_choice::graphics {
                assert_eq!(
                    f.view().focused_window().unwrap().surface(),
                    Some(frame.descriptor())
                );
            }
            let id = f.focused();
            let count = f.view().all_windows().count();
            f.click(chrome_target::launcher);
            f.click(target);
            assert_eq!(f.focused(), id);
            assert_eq!(f.view().all_windows().count(), count);
        }
    }

    #[test]
    fn outside_dismissal_consumes_window_panel_and_drag_gesture_until_release() {
        for target_index in 0..4 {
            let mut f = fixture::new();
            let id = f.focused();
            let window = f.view().window(id).unwrap();
            let bounds = window.bounds();
            let target = match target_index {
                0 => chrome_target::close(id),
                1 => chrome_target::titlebar(id),
                2 => chrome_target::workspace(workspace_id::second),
                _ => chrome_target::appearance_launcher,
            };
            // Record the ordinary target before the overlay becomes topmost.
            let (x, y) = f.point(target);
            f.click(chrome_target::launcher);
            assert_eq!(hit_test_chrome(&f.view(), x, y), target);
            assert_eq!(f.at(x, y, 1)[2..], [desktop_command::dismiss_launcher]);
            assert!(!f.view().launcher_open());
            assert!(f.pointer.await_release);
            assert_eq!(f.input(20, 0, 1).len(), 2);
            assert_eq!(f.input(0, 0, 0).len(), 2);
            assert_eq!(f.focused(), id);
            assert_eq!(f.view().window(id).unwrap().bounds(), bounds);
            assert_eq!(f.view().active_workspace(), workspace_id::first);
            assert_eq!(f.view().all_windows().count(), 2);
            assert!(f.pointer.capture.is_none() && f.pointer.control.is_none());
        }
    }

    #[test]
    fn launcher_row_release_cannot_retarget_or_survive_keyboard_revision_or_source_loss() {
        for cancellation in 0..4 {
            let mut f = fixture::new();
            f.click(chrome_target::launcher);
            let target = chrome_target::launcher_item(launcher_choice::activity);
            f.press(target);
            match cancellation {
                0 => {
                    let (x, y) = f.point(chrome_target::launcher_item(launcher_choice::welcome));
                    f.at(x, y, 1);
                }
                1 => {
                    f.apply(desktop_command::select_launcher(
                        launcher_choice::appearance,
                    ));
                }
                2 => {
                    let view = f.model.observe(&f.endpoint).unwrap();
                    assert_eq!(
                        f.pointer.cancel_for_keyboard(&view),
                        Some(desktop_command::cancel_pointer)
                    );
                    f.apply(desktop_command::cancel_pointer);
                }
                _ => {
                    let loss =
                        rite_physical_input_v1::pointer_loss(1, 2, f.sequence, f.sequence, 1)
                            .unwrap();
                    let view = f.model.observe(&f.endpoint).unwrap();
                    let revision = view.revision();
                    let batch = f.pointer.consume(&view, &loss).unwrap().unwrap();
                    let ticket = f
                        .model
                        .prepare_batch(&f.endpoint, revision, batch.commands())
                        .unwrap();
                    publish(&mut f.model, &f.control, ticket);
                    f.generation = 2;
                    f.sequence = 0;
                }
            }
            assert_eq!(f.input(0, 0, 0).len(), 2);
            assert!(f.view().launcher_open());
            assert_eq!(f.view().all_windows().count(), 2);
            assert_eq!(f.view().pointer_armed(), None);
        }
    }
}
