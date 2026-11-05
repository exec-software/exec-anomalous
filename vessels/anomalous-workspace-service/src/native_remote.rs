//! Selected three-task workspace. Only this owner consumes physical input and
//! presents complete desktop rows. The editor's sole mutable session and save
//! snapshot remain in its private task; this module receives copied ANER only.
use super::*;
use anomalous_desktop_compositor::client_viewport;
use anomalous_desktop_workspace::native_bridge::{
    editor_disclosure, native_editor_frame,
    native_remote::{remote_editor_bridge, remote_route_binding},
};
use anomalous_desktop_workspace::{client_endpoint, window_id};
use anomalous_document_protocol::remote_startup::{startup_descriptor, startup_size};
use anomalous_editor_contract::{
    interactive_audit, interactive_editor_action, interactive_frame_byte_count,
    remote_protocol::{remote_action, remote_message_size},
};
use anomalous_window_surface::borrowed_frame;
use anomalous_workspace_service::{prepare_remote_pointer_update, remote_editor_action_for};
use core::ptr;

type content<'a> = native_editor_frame<borrowed_frame<'a, interactive_frame_byte_count>>;
type attachment<'a> = remote_editor_bridge<borrowed_frame<'a, interactive_frame_byte_count>>;

#[repr(C)]
struct call_record {
    operation: u64,
    channel: u64,
    pointer: u64,
    length: u64,
    sequence: u64,
    status: u64,
    returned_sequence: u64,
    returned_length: u64,
}
unsafe extern "C" {
    fn anomalous_workspace_native_call(record: *mut call_record);
}
fn call(operation: u64, channel: u64, pointer: u64, length: usize, sequence: u64) -> call_record {
    let mut record = call_record {
        operation,
        channel,
        pointer,
        length: length as u64,
        sequence,
        status: u64::MAX,
        returned_sequence: 0,
        returned_length: 0,
    };
    // SAFETY: The assembly preserves SysV callee-saved registers and changes
    // only the three reply words of this complete private record. The kernel
    // validates all copied addresses against this selected task's live root.
    unsafe { anomalous_workspace_native_call(&mut record) };
    record
}

/// Ok(false) means no packet. Err(true) is an authenticated terminal route status;
/// malformed replies are Err(false) and cannot authorize fence withdrawal.
fn receive(route: u64, sequence: u64, bytes: &mut [u8; remote_message_size]) -> Result<bool, bool> {
    let reply = call(2, route, bytes.as_mut_ptr() as u64, bytes.len(), 0);
    match reply.status {
        0 if reply.returned_sequence == sequence && reply.returned_length == bytes.len() as u64 => {
            Ok(true)
        }
        1 if reply.returned_sequence == 0 && reply.returned_length == 0 => Ok(false),
        5 if reply.returned_sequence == 0 && reply.returned_length == 0 => Err(true),
        _ => Err(false),
    }
}
fn acknowledge(route: u64, sequence: u64) -> Result<(), ()> {
    let reply = call(3, route, 0, 0, sequence);
    if reply.status == 0 && reply.returned_sequence == sequence && reply.returned_length == 0 {
        Ok(())
    } else {
        Err(())
    }
}
fn send(route: u64, sequence: u64, bytes: &[u8]) -> Result<(), ()> {
    let reply = call(1, route, bytes.as_ptr() as u64, bytes.len(), sequence);
    if reply.status == 0
        && reply.returned_sequence == sequence
        && reply.returned_length == bytes.len() as u64
    {
        Ok(())
    } else {
        // The action is already fenced. Never retry an uncertain submission
        // under a fresh action ID or clear the fence on transport failure.
        Err(())
    }
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_workspace_service_run(pointer: *const u8, length: usize) -> ! {
    let _ = run_remote(pointer, length);
    // Unexpected packet, ownership or presentation failures retire this owner.
    // No uncertain action is retried and no incomplete frame is published.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
fn run_remote(pointer: *const u8, length: usize) -> Result<(), ()> {
    if pointer as usize != 0x801000 || length != startup_size {
        return Err(());
    }
    let mut bytes = [0; startup_size];
    // SAFETY: The startup gate just authenticated the exact retained descriptor
    // mapping. The fixed local copy is disjoint and copied before arena borrows.
    unsafe { ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), startup_size) };
    let startup = startup_descriptor::decode(&bytes).map_err(|_| ())?;
    if startup.role() != 0
        || startup.arena_base() != 0x810000
        || startup.arena_length() != 2 * interactive_frame_byte_count as u64
    {
        return Err(());
    }
    let route = remote_route_binding::try_new(
        startup.connection(),
        startup.workspace(),
        [startup.authority().task(), startup.authority().generation()],
        [startup.handles()[0], startup.route_generations()[0]],
        [startup.handles()[1], startup.route_generations()[1]],
    )
    .map_err(|_| ())?;
    // SAFETY: Admission grants this workspace the complete private RW/NX arena
    // and no peer maps it. These two nonoverlapping exclusive borrows are made
    // once, cover the arena exactly, and remain owned by frames/recycling only.
    let slots =
        unsafe { &mut *(startup.arena_base() as *mut [[u8; interactive_frame_byte_count]; 2]) };
    let [first, second] = slots;
    run_desktop_remote(route, first, second)
}

fn run_desktop_remote(
    route: remote_route_binding,
    first: &mut [u8; interactive_frame_byte_count],
    second: &mut [u8; interactive_frame_byte_count],
) -> Result<(), ()> {
    let (mut model, control, initial) =
        desktop::new_demo(frame_width, frame_height).map_err(|_| ())?;
    let human = model
        .bind_client(&control, client_origin::human, client_grants::all)
        .map_err(|_| ())?;
    let mut rites = rite_bridge { sequence: 0 };
    present_desktop(&mut model, &control, initial, &mut rites, None)?;
    let mut port = model
        .provision_remote_editor(&control, route)
        .map_err(|_| ())?;
    let mut packet = [0; remote_message_size];
    while !receive(route.observation_route()[0], 1, &mut packet).map_err(|_| ())? {
        rites.idle()?;
    }
    // Receipt identity/order comes from the installed startup route and checked
    // gate reply above, never from ANER's public correlation fields.
    port.admit_initial_checked(route, 1, &packet)
        .map_err(|_| ())?;
    acknowledge(route.observation_route()[0], 1)?;
    let first = content::from_audit_in(port.audit_bytes().ok_or(())?, first).map_err(|_| ())?;
    let revision = model.observe(&human).map_err(|_| ())?.revision();
    let ticket = model
        .prepare(
            &human,
            revision,
            desktop_command::open_native_editor(first.descriptor()),
        )
        .map_err(|_| ())?;
    let window = model
        .candidate(&ticket)
        .map_err(|_| ())?
        .windows()
        .find(|window| window.content() == demo_content::native_editor)
        .ok_or(())?
        .id();
    present_desktop(
        &mut model,
        &control,
        ticket,
        &mut rites,
        Some((window, &first.pixels())),
    )?;
    let mut remote = model
        .bind_remote_editor(
            &control,
            &human,
            window,
            port,
            first,
            editor_disclosure::public_fixture,
        )
        .map_err(|_| ())?;
    let input_grant = remote
        .bind_input(&model, &control, &human)
        .map_err(|_| ())?;
    let mut available = second;
    let mut delivery = 2u64;
    let mut action_pending = false;
    let mut keyboard = keyboard_state::new();
    let mut pointer = pointer_state::new();
    let mut pending_input = None;
    loop {
        match receive(route.observation_route()[0], delivery, &mut packet) {
            Ok(true) => {
                let completed = remote
                    .admit_checked_message(&mut model, route, delivery, &packet)
                    .map_err(|_| ())?;
                acknowledge(route.observation_route()[0], delivery)?;
                delivery = delivery.checked_add(1).ok_or(())?;
                if completed.is_some() {
                    action_pending = false;
                }
            }
            Ok(false) => {}
            Err(true) => {
                remote
                    .withdraw_terminal(&mut model, &control, route)
                    .map_err(|_| ())?;
                let revision = model.observe(&human).map_err(|_| ())?.revision();
                let ticket = model
                    .prepare_native_editor_withdrawal(&control, &human, revision, window)
                    .map_err(|_| ())?;
                present_desktop(&mut model, &control, ticket, &mut rites, None)?;
                return rites.exit();
            }
            Err(false) => return Err(()),
        }
        if action_pending {
            // Queued physical input remains unconsumed until the exact result.
            // Reply draining and cooperative peer progress stay live while every
            // desktop publication/focus/modal/grant change remains fenced.
            rites.idle()?;
            continue;
        }
        let latest = interactive_audit::decode(remote.latest_audit()).map_err(|_| ())?;
        if latest.sequence != remote.observe(&model, &human).map_err(|_| ())?.sequence {
            let (returned, closed) = publish_latest(
                &mut model,
                &control,
                &human,
                &mut remote,
                &mut rites,
                available,
            )?;
            available = returned;
            if closed {
                return rites.exit();
            }
        }
        let input = match pending_input.take() {
            Some(input) => input,
            None => match rites.input()? {
                Some(input) => input,
                None => {
                    rites.idle()?;
                    continue;
                }
            },
        };
        if input.is_pointer() || input.is_pointer_loss() {
            let view = model.observe(&human).map_err(|_| ())?;
            let (batch, next) = pointer
                .consume_ready(&view, &input, || rites.input())
                .map_err(|_| ())?;
            pending_input = next;
            if let Some(batch) = batch {
                let revision = view.revision();
                let (ticket, close) = prepare_remote_pointer_update(
                    &mut model,
                    &human,
                    revision,
                    window,
                    batch.commands(),
                    &mut pointer,
                )
                .map_err(|_| ())?;
                present_attached(&mut model, &control, &human, ticket, &remote, &mut rites)?;
                if close {
                    let revision = model.observe(&human).map_err(|_| ())?.revision();
                    let request = remote
                        .submit_action(
                            &mut model,
                            &input_grant,
                            revision,
                            remote_action::request_close,
                        )
                        .map_err(|_| ())?;
                    send(
                        route.action_route()[0],
                        request.action_id(),
                        &request.encode(),
                    )?;
                    action_pending = true;
                }
            }
            continue;
        }
        let key = keyboard.consume(&input).map_err(|_| ())?;
        let view = model.observe(&human).map_err(|_| ())?;
        let cancellation = if key.is_some() || input.is_loss() {
            pointer.cancel_for_keyboard(&view).is_some()
        } else {
            false
        };
        let native_focused = view
            .focused_window()
            .is_some_and(|selected| selected.id() == window && selected.focused());
        let remote_action = if native_focused && !view.launcher_open() {
            if input.is_loss() {
                Some(remote_action::edit(interactive_editor_action::input_reset))
            } else if let Some((code, extended)) = key {
                let modifiers = keyboard.modifiers();
                if (!extended && code == 0x3b)
                    || (!extended && code == 0x0f && !modifiers.control && !modifiers.alt)
                {
                    None
                } else {
                    let viewport = client_viewport(&view, view.window(window).ok_or(())?);
                    remote_editor_action_for(code, extended, modifiers).map(|action| {
                        if viewport.width >= 128 && viewport.height >= 48 {
                            action
                        } else {
                            remote_action::edit(interactive_editor_action::unsupported_key)
                        }
                    })
                }
            } else {
                None
            }
        } else {
            None
        };
        if let Some(action) = remote_action {
            if cancellation {
                let revision = view.revision();
                let ticket = model
                    .prepare(&human, revision, desktop_command::cancel_pointer)
                    .map_err(|_| ())?;
                present_attached(&mut model, &control, &human, ticket, &remote, &mut rites)?;
            }
            let revision = model.observe(&human).map_err(|_| ())?.revision();
            let request = remote
                .submit_action(&mut model, &input_grant, revision, action)
                .map_err(|_| ())?;
            send(
                route.action_route()[0],
                request.action_id(),
                &request.encode(),
            )?;
            action_pending = true;
            continue;
        }
        let command = if let Some((code, extended)) = key {
            if native_focused && !view.launcher_open() {
                match (code, extended) {
                    (0x3b, false) => Some(desktop_command::toggle_launcher),
                    (0x0f, false) if !keyboard.modifiers().control && !keyboard.modifiers().alt => {
                        view.next_focus().map(desktop_command::focus)
                    }
                    _ => None,
                }
            } else if code == 0x01 && !extended && !view.launcher_open() && !editor_focused(&view) {
                return rites.exit();
            } else {
                command_for_with_modifiers(&view, code, extended, keyboard.modifiers(), None)
            }
        } else if input.is_loss() {
            input_reset_command(&view)
        } else {
            None
        };
        let revision = view.revision();
        if let Some(ticket) =
            prepare_keyboard_update(&mut model, &human, revision, command, cancellation)
                .map_err(|_| ())?
        {
            present_attached(&mut model, &control, &human, ticket, &remote, &mut rites)?;
        }
    }
}

fn present_attached(
    model: &mut desktop,
    control: &desktop_control,
    human: &client_endpoint,
    ticket: update_ticket,
    remote: &attachment<'_>,
    rites: &mut rite_bridge,
) -> Result<(), ()> {
    let pixels = remote.presented_frame(model, human).map_err(|_| ())?;
    present_desktop(
        model,
        control,
        ticket,
        rites,
        Some((remote.window(), &pixels)),
    )
}
fn present_desktop(
    model: &mut desktop,
    control: &desktop_control,
    ticket: update_ticket,
    rites: &mut rite_bridge,
    content: Option<(window_id, &frame<'_>)>,
) -> Result<(), ()> {
    validate_composition(&model.candidate(&ticket).map_err(|_| ())?, content)?;
    model.begin(control, &ticket).map_err(|_| ())?;
    compose_rows(&model.candidate(&ticket).map_err(|_| ())?, content, rites)?;
    model.attest_presented(control, &ticket).map_err(|_| ())?;
    model.publish(control, ticket).map_err(|_| ())?;
    Ok(())
}
fn validate_composition(
    view: &render_view<'_>,
    content: Option<(window_id, &frame<'_>)>,
) -> Result<(), ()> {
    let bindings = content.map(|(window, pixels)| surface_binding::new(window, pixels));
    composition_frame::try_new(view, bindings.as_slice())
        .map(|_| ())
        .map_err(|_| ())
}
fn compose_rows(
    view: &render_view<'_>,
    content: Option<(window_id, &frame<'_>)>,
    rites: &mut rite_bridge,
) -> Result<(), ()> {
    let bindings = content.map(|(window, pixels)| surface_binding::new(window, pixels));
    let composition = composition_frame::try_new(view, bindings.as_slice()).map_err(|_| ())?;
    let mut pixels = [0; row_byte_count];
    for row in 0..frame_height {
        composition.compose_row(row, &mut pixels).map_err(|_| ())?;
        rites.row(row, &pixels)?;
    }
    Ok(())
}
fn publish_latest<'a>(
    model: &mut desktop,
    control: &desktop_control,
    human: &client_endpoint,
    remote: &mut attachment<'a>,
    rites: &mut rite_bridge,
    available: &'a mut [u8; interactive_frame_byte_count],
) -> Result<(&'a mut [u8; interactive_frame_byte_count], bool), ()> {
    let frame = content::from_audit_in(remote.latest_audit(), available).map_err(|_| ())?;
    let revision = model.observe(human).map_err(|_| ())?.revision();
    let update = remote
        .prepare_update(model, revision, frame)
        .map_err(|_| ())?;
    let closed = update.frame().observation().closed;
    let pixels = update.frame().pixels();
    let binding = (!closed).then_some((remote.window(), &pixels));
    validate_composition(&remote.candidate(model, &update).map_err(|_| ())?, binding)?;
    remote.begin(model, control, &update).map_err(|_| ())?;
    compose_rows(
        &remote.candidate(model, &update).map_err(|_| ())?,
        binding,
        rites,
    )?;
    remote
        .attest_presented(model, control, &update)
        .map_err(|_| ())?;
    let (published, returned) = remote
        .publish_recycling(model, control, update)
        .map_err(|_| ())?;
    Ok((returned.into_buffer(), published.closed))
}
