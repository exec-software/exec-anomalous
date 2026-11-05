//! Live physical-input editor. Qualification modes alter only the service's
//! first-save reply delivery; neither startup nor the service supplies edits.
use super::{call, exit, gate_device};
use anomalous_abi::{rite_physical_input_v1, rite_physical_input_v1_size};
use anomalous_desktop_workspace::editor_action;
use anomalous_document_protocol::{
    durable::{durable_error, durable_store, format_disposable},
    durable_startup::mount_ready,
    interactive_startup::{
        arena_byte_count, display_height, display_width, row_bytes, startup_descriptor,
        startup_size,
    },
    operation, record, record_size, service_error,
};
use anomalous_editor_contract::{
    interactive_audit, interactive_audit_size, interactive_editor_action as action,
    interactive_editor_session, interactive_frame_byte_count, interactive_frame_width,
    render_interactive, save_acknowledgement, save_outcome,
};
use anomalous_native_runtime::arena;
use anomalous_workspace_service::{editor_action_for, keyboard_state};
use core::{mem::MaybeUninit, ptr};

struct buffers<'a> {
    pixels: &'a mut [u8; interactive_frame_byte_count],
    row: &'a mut [u8; row_bytes],
    input: &'a mut [u8; rite_physical_input_v1_size],
    audit: &'a mut [u8; interactive_audit_size],
    request: &'a mut [u8; record_size],
    reply: &'a mut [u8; record_size],
    sector: &'a mut [u8; 512],
}

fn pause() -> bool {
    let result = call(4, 0, 0, 0, 0);
    assert_eq!(result.status, 0);
    assert!(result.returned_sequence <= 2);
    assert_eq!(result.returned_length, 0);
    result.returned_sequence == 0
}
fn send(channel: u64, sequence: u64, bytes: &[u8]) -> bool {
    loop {
        let result = call(1, channel, bytes.as_ptr() as u64, bytes.len(), sequence);
        match result.status {
            0 => {
                assert_eq!(result.returned_sequence, sequence);
                assert_eq!(result.returned_length, bytes.len() as u64);
                return true;
            }
            1 => {
                pause();
            }
            5 => return false,
            _ => panic!("interactive send rejected"),
        }
    }
}
fn ack(channel: u64, sequence: u64) {
    let result = call(3, channel, 0, 0, sequence);
    assert_eq!(result.status, 0);
    assert_eq!(result.returned_sequence, sequence);
}
fn await_ack(channel: u64, sequence: u64) -> bool {
    loop {
        let result = call(6, channel, 0, 0, sequence);
        match result.status {
            0 => {
                assert_eq!(result.returned_sequence, sequence);
                return true;
            }
            1 => {
                pause();
            }
            5 => return false,
            _ => panic!("interactive ack rejected"),
        }
    }
}
/// None means currently empty; Err means the authenticated peer has retired.
fn receive(channel: u64, sequence: u64, bytes: &mut [u8]) -> Result<bool, ()> {
    let result = call(2, channel, bytes.as_mut_ptr() as u64, bytes.len(), 0);
    match result.status {
        0 => {
            assert_eq!(result.returned_sequence, sequence);
            assert_eq!(result.returned_length, bytes.len() as u64);
            ack(channel, sequence);
            Ok(true)
        }
        1 => Ok(false),
        5 => Err(()),
        _ => panic!("interactive receive rejected"),
    }
}
fn wait_receive(channel: u64, sequence: u64, bytes: &mut [u8]) -> bool {
    loop {
        match receive(channel, sequence, bytes) {
            Ok(true) => return true,
            Ok(false) => {
                pause();
            }
            Err(()) => return false,
        }
    }
}
fn rpc(
    handles: [u64; 4],
    sequence: u64,
    value: record,
    buffers: &mut buffers<'_>,
) -> Option<record> {
    buffers.request.copy_from_slice(&value.encode());
    if !send(handles[2], sequence, buffers.request)
        || !await_ack(handles[2], sequence)
        || !wait_receive(handles[3], sequence.checked_add(1).unwrap(), buffers.reply)
    {
        return None;
    }
    Some(record::decode(buffers.reply).unwrap())
}
fn present(session: &interactive_editor_session, buffers: &mut buffers<'_>, frame: &mut u64) {
    render_interactive(&session.view(), buffers.pixels);
    for y in 0..display_height {
        for x in 0..display_width {
            let index = ((y / 3) * interactive_frame_width + x / 3) * 3;
            buffers.row[x * 3..x * 3 + 3].copy_from_slice(&buffers.pixels[index..index + 3]);
        }
        let reply = call(7, y as u64, buffers.row.as_ptr() as u64, row_bytes, 0);
        assert_eq!(
            (reply.status, reply.returned_sequence, reply.returned_length),
            (0, 0, 0)
        );
    }
    let reply = call(16, 0, 0, 0, 0);
    *frame = frame.checked_add(1).unwrap();
    assert_eq!(
        (reply.status, reply.returned_sequence, reply.returned_length),
        (0, *frame, 0)
    );
}
fn publish(
    session: &interactive_editor_session,
    buffers: &mut buffers<'_>,
    handles: [u64; 4],
    audit_sequence: &mut u64,
    frame: &mut u64,
    peer: &mut bool,
) {
    present(session, buffers, frame);
    if *peer {
        *audit_sequence = audit_sequence.checked_add(1).unwrap();
        buffers.audit.copy_from_slice(
            &interactive_audit::from_view(&session.view())
                .encode()
                .unwrap(),
        );
        *peer = send(handles[0], *audit_sequence, buffers.audit)
            && await_ack(handles[0], *audit_sequence);
    }
}
fn unknown(session: &mut interactive_editor_session) {
    if !session.view().is_closed() && session.view().save_outcome() == save_outcome::awaiting {
        let pending = session.model().state().pending_save().unwrap();
        let tuple = (
            pending.document(),
            pending.generation(),
            pending.request(),
            pending.base_version(),
        );
        session
            .mark_save_unknown(tuple.0, tuple.1, tuple.2, tuple.3)
            .unwrap();
    }
}
fn process_saved(session: &mut interactive_editor_session, request: record, reply: record) {
    if reply.kind() == operation::saved {
        session
            .acknowledge_save(save_acknowledgement {
                document: reply.document(),
                generation: reply.generation(),
                request: reply.request(),
                base_version: reply.base_version(),
                revision: reply.revision(),
                version: reply.version(),
                text: reply.text(),
            })
            .unwrap();
    } else {
        let error = reply.status().unwrap();
        assert_eq!(reply, record::rejection_for(&request, error).unwrap());
        if error == service_error::stale_request {
            unknown(session);
        } else {
            session
                .reject_save(
                    request.document(),
                    request.generation(),
                    request.request(),
                    request.base_version(),
                )
                .unwrap();
        }
    }
}
fn mapped(action: editor_action) -> action {
    match action {
        editor_action::insert(byte) => action::insert(byte),
        editor_action::backspace => action::backspace,
        editor_action::delete_forward => action::delete_forward,
        editor_action::move_caret { motion, extend } => action::move_caret { motion, extend },
        editor_action::select_all => action::select_all,
        editor_action::collapse_selection => action::collapse_selection,
        editor_action::undo => action::undo,
        editor_action::input_reset => action::input_reset,
        _ => action::unsupported_key,
    }
}
fn editor_probes(startup: &startup_descriptor, buffers: &mut buffers<'_>) {
    let guessed_grant = 0x7001 + startup.phase() * 0x100;
    buffers.sector.fill(0x73);
    for operation in [12, 13] {
        assert_eq!(
            call(
                operation,
                guessed_grant,
                buffers.sector.as_mut_ptr() as u64,
                512,
                0
            )
            .status,
            2
        );
        assert!(buffers.sector.iter().all(|byte| *byte == 0x73));
    }
    assert_eq!(call(14, guessed_grant, 0, 0, 0).status, 2);
    buffers.input.fill(0x6d);
    assert_eq!(call(15, 0, 0, rite_physical_input_v1_size, 0).status, 3);
    assert_eq!(
        call(
            15,
            0,
            buffers.input.as_mut_ptr() as u64,
            rite_physical_input_v1_size - 1,
            0
        )
        .status,
        4
    );
    assert!(buffers.input.iter().all(|byte| *byte == 0x6d));
    assert_eq!(call(16, 0, 0, 0, 0).status, 4);
    assert_eq!(
        call(7, 1, buffers.row.as_ptr() as u64, row_bytes, 0).status,
        4
    );
}
fn editor(
    startup: &startup_descriptor,
    buffers: &mut buffers<'_>,
    slot: &mut MaybeUninit<interactive_editor_session>,
    keyboard: &mut keyboard_state,
) -> ! {
    let handles = startup.handles();
    editor_probes(startup, buffers);
    assert!(wait_receive(handles[3], 1, buffers.reply));
    let ready = mount_ready::decode(buffers.reply).unwrap();
    let opened = rpc(
        handles,
        1,
        record::open(ready.document(), 1).unwrap(),
        buffers,
    )
    .unwrap();
    assert_eq!(
        (
            opened.kind(),
            opened.document(),
            opened.request(),
            opened.version()
        ),
        (operation::opened, ready.document(), 1, ready.version())
    );
    let session = slot.write(
        interactive_editor_session::from_open(
            opened.document(),
            opened.generation(),
            opened.version(),
            opened.text(),
        )
        .unwrap(),
    );
    let mut transport = 1_u64;
    let mut audit_sequence = 0;
    let mut frame = 0;
    let mut published = 0;
    let mut peer = true;
    let mut pending: Option<record> = None;
    loop {
        if session.view().is_closed() {
            publish(
                session,
                buffers,
                handles,
                &mut audit_sequence,
                &mut frame,
                &mut peer,
            );
            exit();
        }
        if !peer {
            unknown(session);
        }
        if let Some(request) = pending {
            match receive(handles[3], transport.checked_add(1).unwrap(), buffers.reply) {
                Ok(true) => {
                    process_saved(session, request, record::decode(buffers.reply).unwrap());
                    pending = None;
                }
                Ok(false) => {}
                Err(()) => {
                    peer = false;
                    unknown(session);
                    pending = None;
                }
            }
        }
        if published != session.view().sequence() {
            publish(
                session,
                buffers,
                handles,
                &mut audit_sequence,
                &mut frame,
                &mut peer,
            );
            published = session.view().sequence();
            // Publish a pending save before accepting a subsequent physical key.
            if !peer {
                unknown(session);
                continue;
            }
        }
        if session.view().is_closed() {
            exit();
        }
        let result = call(
            15,
            0,
            buffers.input.as_mut_ptr() as u64,
            rite_physical_input_v1_size,
            0,
        );
        match result.status {
            1 => {
                if !pause() {
                    peer = false;
                }
                continue;
            }
            0 => {
                assert_eq!(result.returned_length, rite_physical_input_v1_size as u64);
            }
            _ => panic!("physical input rejected"),
        }
        let event = rite_physical_input_v1::decode(*buffers.input).unwrap();
        assert_eq!(event.sequence(), result.returned_sequence);
        let key = keyboard.consume(&event).unwrap();
        if event.is_loss() {
            session.apply(action::input_reset).unwrap();
            continue;
        }
        let Some((code, extended)) = key else {
            continue;
        };
        let modifiers = keyboard.modifiers();
        if code == 0x10 && !extended && modifiers.control && !modifiers.alt && !modifiers.shift {
            session.request_close().unwrap();
            if session.view().is_closed()
                && peer
                && pending.is_none()
                && session.view().save_outcome() == save_outcome::idle
            {
                transport = transport.checked_add(1).unwrap();
                let close =
                    record::close(opened.document(), opened.generation(), transport).unwrap();
                match rpc(handles, transport, close, buffers) {
                    Some(reply) => assert_eq!(
                        (
                            reply.kind(),
                            reply.document(),
                            reply.generation(),
                            reply.request()
                        ),
                        (
                            operation::closed,
                            close.document(),
                            close.generation(),
                            close.request()
                        )
                    ),
                    None => peer = false,
                }
            }
            continue;
        }
        let Some(selected) = editor_action_for(code, extended, modifiers) else {
            continue;
        };
        if selected == editor_action::save_unavailable {
            // A second save while one is pending is an atomic no-op. Never
            // reserve a fresh request ID or replace its retained bytes.
            if !peer {
                session.apply(action::unsupported_key).unwrap();
                continue;
            }
            if let Ok(snapshot) = session.begin_save() {
                let request = record::save(
                    snapshot.document(),
                    snapshot.generation(),
                    snapshot.request(),
                    snapshot.base_version(),
                    snapshot.revision(),
                    snapshot.text(),
                )
                .unwrap();
                // Make pending visible and obtain its audit ACK before the
                // service can consume this save, including reply-loss mode.
                publish(
                    session,
                    buffers,
                    handles,
                    &mut audit_sequence,
                    &mut frame,
                    &mut peer,
                );
                published = session.view().sequence();
                transport = transport.checked_add(1).unwrap();
                buffers.request.copy_from_slice(&request.encode());
                peer = peer && send(handles[2], transport, buffers.request);
                if peer {
                    pending = Some(request);
                } else {
                    unknown(session);
                }
            }
        } else {
            session.apply(mapped(selected)).unwrap();
        }
    }
}

fn service(startup: &startup_descriptor, buffers: &mut buffers<'_>) -> ! {
    assert_eq!(
        call(7, 0, buffers.row.as_ptr() as u64, row_bytes, 0).status,
        2
    );
    assert_eq!(
        call(
            15,
            0,
            buffers.input.as_mut_ptr() as u64,
            rite_physical_input_v1_size,
            0
        )
        .status,
        2
    );
    assert_eq!(call(16, 0, 0, 0, 0).status, 2);
    let mut device = gate_device {
        handle: startup.device_handle(),
    };
    if startup.phase() == 0 {
        format_disposable(&mut device, startup.identity(), b"").unwrap();
    }
    let mut store =
        durable_store::mount(&mut device, startup.identity(), startup.authority()).unwrap();
    let handles = startup.handles();
    buffers.reply.copy_from_slice(
        &mount_ready::try_new(store.document(), store.version(), store.sequence())
            .unwrap()
            .encode(),
    );
    assert!(send(handles[3], 1, buffers.reply) && await_ack(handles[3], 1));
    let mut request_sequence = 1;
    let mut audit_sequence = 1_u64;
    let mut pending: Option<record> = None;
    let mut submitted = false;
    let mut held = false;
    let mut first_save = true;
    loop {
        match receive(handles[0], audit_sequence, buffers.audit) {
            Ok(true) => {
                let audit = interactive_audit::decode(buffers.audit).unwrap();
                audit_sequence = audit_sequence.checked_add(1).unwrap();
                if audit.closed {
                    exit();
                }
                if held
                    && pending.as_ref().is_some_and(|reply| {
                        audit.pending_request == reply.request()
                            && audit.pending_revision == reply.revision()
                            && audit.revision > reply.revision()
                    })
                {
                    held = false;
                }
            }
            Ok(false) => {}
            Err(()) => exit(),
        }
        if let Some(reply) = pending {
            if !held {
                if !submitted {
                    buffers.reply.copy_from_slice(&reply.encode());
                    let result = call(
                        1,
                        handles[3],
                        buffers.reply.as_ptr() as u64,
                        record_size,
                        request_sequence,
                    );
                    match result.status {
                        0 => {
                            assert_eq!(result.returned_sequence, request_sequence);
                            assert_eq!(result.returned_length, record_size as u64);
                            submitted = true;
                        }
                        1 => {}
                        5 => exit(),
                        _ => panic!("service reply rejected"),
                    }
                } else {
                    match call(6, handles[3], 0, 0, request_sequence).status {
                        0 => {
                            pending = None;
                            submitted = false;
                        }
                        1 => {}
                        5 => exit(),
                        _ => panic!("service reply ack rejected"),
                    }
                }
            }
        } else {
            match receive(handles[2], request_sequence, buffers.request) {
                Ok(true) => {
                    let request = record::decode(buffers.request).unwrap();
                    let reply = match store.execute(startup.authority(), &request) {
                        Ok(reply) => reply,
                        Err(durable_error::rejected(error)) => {
                            record::rejection_for(&request, error).unwrap()
                        }
                        Err(_) => exit(),
                    };
                    request_sequence = request_sequence.checked_add(1).unwrap();
                    if request.kind() == operation::save
                        && reply.kind() == operation::saved
                        && first_save
                    {
                        first_save = false;
                        if startup.proof_mode() == 2 {
                            exit();
                        }
                        held = startup.proof_mode() == 1;
                    }
                    pending = Some(reply);
                }
                Ok(false) => {}
                Err(()) => exit(),
            }
        }
        pause();
    }
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_native_durable_client_run(pointer: *const u8, length: usize) -> ! {
    assert_eq!(pointer as u64, 0x801000);
    assert_eq!(length, startup_size);
    let mut bytes = [0; startup_size];
    // SAFETY: The initial descriptor is admitted private readable data. Copy
    // before parsing; the codec grants no authority beyond the kernel route.
    unsafe { ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), startup_size) };
    let startup = startup_descriptor::decode(&bytes).unwrap();
    // SAFETY: The launch binds this task's disjoint private RW/NX arena. This
    // single borrow owns it; allocator-produced slices remain disjoint forever.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(startup.arena_base() as *mut u8, arena_byte_count as usize)
    };
    for byte in storage.iter() {
        assert_eq!(unsafe { ptr::read_volatile(byte) }, 0);
    }
    let mut arena = arena::new(storage);
    let state = arena
        .allocate(
            core::mem::size_of::<MaybeUninit<interactive_editor_session>>(),
            64,
        )
        .unwrap();
    // SAFETY: Properly aligned, exact-size uniquely owned uninitialized slot.
    let slot = unsafe {
        &mut *state
            .as_mut_ptr()
            .cast::<MaybeUninit<interactive_editor_session>>()
    };
    let keys = arena
        .allocate(core::mem::size_of::<keyboard_state>(), 64)
        .unwrap();
    // SAFETY: One aligned unique slot, initialized before creating its reference.
    let keyboard = unsafe {
        let p = keys.as_mut_ptr().cast::<keyboard_state>();
        p.write(keyboard_state::new());
        &mut *p
    };
    let mut buffers = buffers {
        pixels: arena
            .allocate(interactive_frame_byte_count, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        row: arena.allocate(row_bytes, 64).unwrap().try_into().unwrap(),
        input: arena
            .allocate(rite_physical_input_v1_size, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        audit: arena
            .allocate(interactive_audit_size, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        request: arena.allocate(record_size, 64).unwrap().try_into().unwrap(),
        reply: arena.allocate(record_size, 64).unwrap().try_into().unwrap(),
        sector: arena.allocate(512, 64).unwrap().try_into().unwrap(),
    };
    if startup.role() == 0 {
        editor(&startup, &mut buffers, slot, keyboard)
    } else {
        service(&startup, &mut buffers)
    }
}
