#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]
#![cfg_attr(feature = "native_remote", allow(dead_code, unused_imports))]

use anomalous_abi::{
    id_128, rite_call, rite_display_patch_v1, rite_exit_payload, rite_operation, rite_payload_kind,
    rite_physical_input_v1, rite_physical_input_v1_size, rite_reply, rite_rights, rite_status,
};
use anomalous_desktop_compositor::{
    composition_frame, frame_height, frame_width, row_byte_count, surface_binding,
};
use anomalous_desktop_workspace::{
    client_grants, client_origin, demo_content, desktop, desktop_command, desktop_control,
    render_view, update_ticket, window_capacity,
};
use anomalous_window_surface::{
    canvas, demo_byte_count, demo_height, demo_stride, demo_width, draw_graphics_demo, frame,
    graphics_demo_label,
};
use anomalous_workspace_service::{
    command_for_with_modifiers, editor_focused, expected_policy_rejection, input_reset_command,
    keyboard_state, pointer_state, prepare_keyboard_update,
};
use core::arch::{asm, global_asm};
use core::{mem, panic::PanicInfo};

mod mailbox_copy;
#[cfg(feature = "native_remote")]
mod native_remote;

const mailbox_address: usize = include!(concat!(env!("OUT_DIR"), "/mailbox.rs"));
const payload_offset: usize = 40;
const display_header_size: usize = 40;
const display_payload_size: u32 = (display_header_size + row_byte_count) as u32;
const _: () = {
    assert!(mem::size_of::<rite_call>() == payload_offset);
    assert!(mem::size_of::<rite_reply>() == payload_offset);
    assert!(mem::size_of::<rite_display_patch_v1>() == display_header_size);
    assert!(payload_offset + display_payload_size as usize <= 4096);
};

global_asm!(include_str!(env!("WORKSPACE_ENTRY")), options(att_syntax));
unsafe extern "C" {
    fn anomalous_workspace_service_rite();
    fn anomalous_workspace_service_controlled_fault() -> !;
    fn anomalous_workspace_service_bad_flags() -> !;
    fn anomalous_workspace_service_bad_mailbox() -> !;
}

#[unsafe(no_mangle)]
#[cfg(not(feature = "native_remote"))]
extern "C" fn anomalous_workspace_service_run() -> ! {
    let _ = run();
    // Any unexpected bridge/model failure withdraws the service. It must never
    // publish an unacknowledged candidate or continue on uncertain pixels.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}

fn run() -> Result<(), ()> {
    // Two complete immutable public content frames fit the existing bounded
    // user stack. No application receives the framebuffer or kernel stage.
    let mut pixels = [[0u8; demo_byte_count]; 2];
    for (scene, pixels) in pixels.iter_mut().enumerate() {
        let mut canvas =
            canvas::try_new(demo_width, demo_height, demo_stride, pixels).map_err(|_| ())?;
        draw_graphics_demo(&mut canvas, scene as u8);
    }
    let frames = [
        frame::try_new(
            demo_width,
            demo_height,
            demo_stride,
            graphics_demo_label(0),
            &pixels[0],
        )
        .map_err(|_| ())?,
        frame::try_new(
            demo_width,
            demo_height,
            demo_stride,
            graphics_demo_label(1),
            &pixels[1],
        )
        .map_err(|_| ())?,
    ];
    run_desktop(&frames)
}

fn run_desktop(frames: &[frame<'_>; 2]) -> Result<(), ()> {
    let (mut model, control, initial) =
        desktop::new_demo(frame_width, frame_height).map_err(|_| ())?;
    let human = model
        .bind_client(&control, client_origin::human, client_grants::all)
        .map_err(|_| ())?;
    let mut bridge = rite_bridge { sequence: 0 };
    present(&mut model, &control, initial, &mut bridge, frames)?;
    let mut keyboard = keyboard_state::new();
    let mut pointer = pointer_state::with_graphics_surface(frames[0].descriptor());
    let mut pending = None;
    loop {
        let Some(input) = (match pending.take() {
            Some(input) => Some(input),
            None => bridge.input()?,
        }) else {
            bridge.idle()?;
            continue;
        };
        if input.is_pointer() || input.is_pointer_loss() {
            let view = model.observe(&human).map_err(|_| ())?;
            // Drain only already-ready, continuous motion. The adapter keeps
            // button/source/keyboard boundaries as a single untouched lookahead
            // and limits work so presentation cannot starve under input load.
            let (batch, next) = pointer
                .consume_ready(&view, &input, || bridge.input())
                .map_err(|_| ())?;
            pending = next;
            if let Some(batch) = batch {
                let revision = view.revision();
                let ticket = match model.prepare_batch(&human, revision, batch.commands()) {
                    Ok(ticket) => ticket,
                    Err(error) if expected_policy_rejection(error) => {
                        pointer.cancel_capture();
                        // The rejected policy candidate has no effects. Keep
                        // the physical cursor/button edge in a fresh bounded
                        // transaction so release cannot leave a stuck control.
                        model
                            .prepare_batch(&human, revision, &batch.commands()[..2])
                            .map_err(|_| ())?
                    }
                    Err(_) => return Err(()),
                };
                present(&mut model, &control, ticket, &mut bridge, frames)?;
            }
            continue;
        }
        // Validate source and sequence before a keyboard event can cancel any
        // canonical pointer state. Repeats and key breaks do not end gestures.
        let key = keyboard.consume(&input).map_err(|_| ())?;
        let cancellation = if key.is_some() || input.is_loss() {
            let view = model.observe(&human).map_err(|_| ())?;
            pointer.cancel_for_keyboard(&view)
        } else {
            None
        };
        let Some((code, extended)) = key else {
            let view = model.observe(&human).map_err(|_| ())?;
            let reset = input
                .is_loss()
                .then(|| input_reset_command(&view))
                .flatten();
            let revision = view.revision();
            if let Some(ticket) =
                prepare_keyboard_update(&mut model, &human, revision, reset, cancellation.is_some())
                    .map_err(|_| ())?
            {
                present(&mut model, &control, ticket, &mut bridge, frames)?;
            }
            continue;
        };
        let view = model.observe(&human).map_err(|_| ())?;
        let editor_has_focus = editor_focused(&view);
        if !extended && !view.launcher_open() && !editor_has_focus {
            match code {
                0x01 => return bridge.exit(),
                // Public development controls exercise containment, without
                // issuing a privileged instruction or a kernel-only hook.
                0x58 if keyboard.shift() => unsafe { anomalous_workspace_service_bad_flags() },
                0x58 => unsafe { anomalous_workspace_service_controlled_fault() },
                0x44 => unsafe { anomalous_workspace_service_bad_mailbox() },
                0x57 => loop {
                    core::hint::spin_loop();
                },
                _ => {}
            }
        }
        let revision = view.revision();
        // F2 opens the public graphics client. F3 replaces its entire immutable
        // content through the same scoped producer API available to local agents.
        if !extended
            && !view.launcher_open()
            && !editor_has_focus
            && code == 0x3d
            && let Some(window) = view
                .focused_window()
                .filter(|w| w.content() == demo_content::graphics)
        {
            let next = usize::from(window.surface() == Some(frames[0].descriptor()));
            let owner = model
                .bind_surface(&control, &human, window.id())
                .map_err(|_| ())?;
            let ticket = model
                .prepare_surface(&owner, revision, frames[next].descriptor())
                .map_err(|_| ())?;
            present(&mut model, &control, ticket, &mut bridge, frames)?;
            continue;
        }
        let command = if !extended && !view.launcher_open() && !editor_has_focus && code == 0x3c {
            Some(desktop_command::open_graphics(frames[0].descriptor()))
        } else {
            command_for_with_modifiers(
                &view,
                code,
                extended,
                keyboard.modifiers(),
                Some(frames[0].descriptor()),
            )
        };
        let Some(ticket) = prepare_keyboard_update(
            &mut model,
            &human,
            revision,
            command,
            cancellation.is_some(),
        )
        .map_err(|_| ())?
        else {
            continue;
        };
        present(&mut model, &control, ticket, &mut bridge, frames)?;
    }
}

fn present(
    model: &mut desktop,
    control: &desktop_control,
    ticket: update_ticket,
    bridge: &mut rite_bridge,
    frames: &[frame<'_>; 2],
) -> Result<(), ()> {
    // Beginning closes observations and commands before any frame byte can
    // change. The last row's successful reply attests the kernel's complete
    // framebuffer copy and readback, after which publication becomes legal.
    // Admission precedes gating and all output. The same immutable source
    // borrows remain live across begin; reborrow the unchanged candidate after
    // begin changes its phase so Rust never aliases the mutable model owner.
    compose_candidate(&model.candidate(&ticket).map_err(|_| ())?, frames)?;
    model.begin(control, &ticket).map_err(|_| ())?;
    let view = model.candidate(&ticket).map_err(|_| ())?;
    let composition = compose_candidate(&view, frames)?;
    let mut pixels = [0; row_byte_count];
    for row in 0..frame_height {
        composition.compose_row(row, &mut pixels).map_err(|_| ())?;
        bridge.row(row, &pixels)?;
    }
    model.attest_presented(control, &ticket).map_err(|_| ())?;
    model.publish(control, ticket).map_err(|_| ())?;
    Ok(())
}

fn compose_candidate<'view, 'pixels>(
    view: &render_view<'view>,
    frames: &'pixels [frame<'pixels>; 2],
) -> Result<composition_frame<'view, 'pixels>, ()> {
    let Some(first) = view.all_windows().next() else {
        return composition_frame::try_new(view, &[]).map_err(|_| ());
    };
    let mut bindings = [surface_binding::new(first.id(), &frames[0]); window_capacity];
    let mut length = 0;
    for window in view.all_windows() {
        if let Some(descriptor) = window.surface() {
            let pixels = frames
                .iter()
                .find(|frame| frame.descriptor() == descriptor)
                .ok_or(())?;
            bindings[length] = surface_binding::new(window.id(), pixels);
            length += 1;
        }
    }
    composition_frame::try_new(view, &bindings[..length]).map_err(|_| ())
}

struct rite_bridge {
    sequence: u64,
}
impl rite_bridge {
    fn call(
        &mut self,
        operation: rite_operation,
        kind: rite_payload_kind,
        rights: u64,
        size: u32,
    ) -> Result<rite_reply, ()> {
        self.sequence = self.sequence.checked_add(1).ok_or(())?;
        let mut trace = *b"workspace-v1!!!!";
        trace[8..].copy_from_slice(&self.sequence.to_le_bytes());
        let trace = id_128::from_bytes(trace);
        write_bytes(
            0,
            &rite_call::new(operation, kind, rights, size, size, trace).encode_le(),
        );
        // SAFETY: The fixed mailbox contains the complete copied call. The
        // assembly shim saves Rust callee-saved registers and supplies only
        // that mailbox address to the kernel's validated user interrupt gate.
        unsafe { anomalous_workspace_service_rite() };
        let mut bytes = [0; 40];
        read_bytes(0, &mut bytes);
        let reply = rite_reply::decode_le(&bytes);
        if !reply.has_supported_header()
            || reply.sequence != self.sequence
            || reply.trace_id != trace
        {
            return Err(());
        }
        Ok(reply)
    }
    fn empty_ok(reply: rite_reply) -> Result<(), ()> {
        if reply.status == rite_status::ok
            && reply.payload_kind == rite_payload_kind::none
            && reply.payload_size == 0
        {
            Ok(())
        } else {
            Err(())
        }
    }
    fn input(&mut self) -> Result<Option<rite_physical_input_v1>, ()> {
        let reply = self.call(
            rite_operation::input_read,
            rite_payload_kind::input_request,
            rite_rights::input_read,
            0,
        )?;
        if reply.status == rite_status::would_block
            && reply.payload_kind == rite_payload_kind::none
            && reply.payload_size == 0
        {
            return Ok(None);
        }
        if reply.status != rite_status::ok
            || reply.payload_kind != rite_payload_kind::physical_input_delivery
            || reply.payload_size as usize != rite_physical_input_v1_size
        {
            return Err(());
        }
        let mut bytes = [0; rite_physical_input_v1_size];
        read_bytes(payload_offset, &mut bytes);
        rite_physical_input_v1::decode(bytes).map(Some).ok_or(())
    }
    fn idle(&mut self) -> Result<(), ()> {
        Self::empty_ok(self.call(
            rite_operation::task_yield,
            rite_payload_kind::none,
            rite_rights::task_yield,
            0,
        )?)
    }
    fn row(&mut self, row: u32, pixels: &[u8; row_byte_count]) -> Result<(), ()> {
        let patch = rite_display_patch_v1::try_new(0, row, frame_width, 1).ok_or(())?;
        let mut bytes = [0; display_header_size];
        bytes[0..2].copy_from_slice(&patch.structure_size().to_le_bytes());
        bytes[2..4].copy_from_slice(&patch.version().to_le_bytes());
        bytes[8..12].copy_from_slice(&patch.x().to_le_bytes());
        bytes[12..16].copy_from_slice(&patch.y().to_le_bytes());
        bytes[16..20].copy_from_slice(&patch.width().to_le_bytes());
        bytes[20..24].copy_from_slice(&patch.height().to_le_bytes());
        bytes[24..28].copy_from_slice(&patch.row_bytes().to_le_bytes());
        bytes[28..30].copy_from_slice(&patch.pixel_format().raw().to_le_bytes());
        bytes[32..36].copy_from_slice(&patch.pixel_byte_length().to_le_bytes());
        write_bytes(payload_offset, &bytes);
        write_bytes(payload_offset + display_header_size, pixels);
        Self::empty_ok(self.call(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            display_payload_size,
        )?)
    }
    fn exit(&mut self) -> Result<(), ()> {
        let payload = rite_exit_payload::new(0);
        write_bytes(payload_offset, &payload.encode_le());
        let _ = self.call(
            rite_operation::task_exit,
            rite_payload_kind::exit_status,
            rite_rights::task_exit,
            mem::size_of::<rite_exit_payload>() as u32,
        )?;
        Err(())
    }
}

fn write_bytes(offset: usize, bytes: &[u8]) {
    assert!(offset <= 4096 && bytes.len() <= 4096 - offset);
    // SAFETY: The checked range lies within the sole mapped normal-memory
    // mailbox page, disjoint from the vessel's private source slice. The rite
    // enters the kernel only after this complete copy returns.
    unsafe { mailbox_copy::write((mailbox_address as *mut u8).add(offset), bytes) };
}
fn read_bytes(offset: usize, bytes: &mut [u8]) {
    assert!(offset <= 4096 && bytes.len() <= 4096 - offset);
    // SAFETY: The checked range lies within the mapped normal-memory mailbox,
    // disjoint from the private destination. The kernel has returned and cannot
    // change this reply while the vessel copies it.
    unsafe { mailbox_copy::read((mailbox_address as *const u8).add(offset), bytes) };
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
