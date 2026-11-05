//! One editor owner and a separate durable service under the mixed native ABI.
//! Input, rendering and presentation belong exclusively to the workspace. All
//! application bytes are copied through the kernel's installed directed routes.
use super::{call, exit, gate_device};
use anomalous_document_protocol::{
    durable::{durable_error, durable_store, format_disposable},
    durable_startup::mount_ready,
    held_save_release::held_save_release,
    operation, record, record_size,
    remote_startup::{startup_descriptor, startup_size},
    service_error,
};
use anomalous_editor_contract::{
    interactive_editor_session, remote_editor_session, remote_effect, remote_message,
    remote_message_size, remote_request_size, save_acknowledgement, save_outcome,
};
use anomalous_native_runtime::arena;
use core::{mem::MaybeUninit, ptr};

struct buffers<'a> {
    action: &'a mut [u8; remote_request_size],
    message: &'a mut [u8; remote_message_size],
    request: &'a mut [u8; record_size],
    reply: &'a mut [u8; record_size],
    sector: &'a mut [u8; 512],
}

fn pause() {
    let reply = call(4, 0, 0, 0, 0);
    assert_eq!(reply.status, 0);
    assert_eq!(reply.returned_length, 0);
}

fn send(channel: u64, sequence: u64, bytes: &[u8]) -> bool {
    loop {
        let reply = call(1, channel, bytes.as_ptr() as u64, bytes.len(), sequence);
        match reply.status {
            0 => {
                assert_eq!(reply.returned_sequence, sequence);
                assert_eq!(reply.returned_length, bytes.len() as u64);
                return true;
            }
            1 => pause(),
            5 => return false,
            _ => panic!("remote send rejected"),
        }
    }
}

fn await_ack(channel: u64, sequence: u64) -> bool {
    loop {
        let reply = call(6, channel, 0, 0, sequence);
        match reply.status {
            0 => {
                assert_eq!(reply.returned_sequence, sequence);
                return true;
            }
            1 => pause(),
            5 => return false,
            _ => panic!("remote acknowledgement rejected"),
        }
    }
}

/// The kernel authenticates this task as the exact installed route receiver and
/// copies the complete record before success. Metadata inside it cannot select
/// another owner. None means empty; Err means the route is terminally closed.
fn receive(channel: u64, sequence: u64, bytes: &mut [u8]) -> Result<bool, ()> {
    let reply = call(2, channel, bytes.as_mut_ptr() as u64, bytes.len(), 0);
    match reply.status {
        0 => {
            assert_eq!(reply.returned_sequence, sequence);
            assert_eq!(reply.returned_length, bytes.len() as u64);
            let ack = call(3, channel, 0, 0, sequence);
            assert_eq!(ack.status, 0);
            assert_eq!(ack.returned_sequence, sequence);
            Ok(true)
        }
        1 => Ok(false),
        5 => Err(()),
        _ => panic!("remote receive rejected"),
    }
}

fn wait_receive(channel: u64, sequence: u64, bytes: &mut [u8]) -> bool {
    loop {
        match receive(channel, sequence, bytes) {
            Ok(true) => return true,
            Ok(false) => pause(),
            Err(()) => return false,
        }
    }
}

fn publish(
    channel: u64,
    sequence: &mut u64,
    message: remote_message,
    buffer: &mut [u8; remote_message_size],
) -> bool {
    let next = sequence.checked_add(1).unwrap();
    buffer.copy_from_slice(&message.encode());
    if !send(channel, *sequence, buffer) || !await_ack(channel, *sequence) {
        return false;
    }
    *sequence = next;
    true
}

fn mark_unknown(session: &mut remote_editor_session, request: record) {
    if !session.view().is_closed() && session.view().save_outcome() == save_outcome::awaiting {
        session
            .mark_save_unknown(
                request.document(),
                request.generation(),
                request.request(),
                request.base_version(),
            )
            .unwrap();
    }
}

fn process_saved(session: &mut remote_editor_session, request: record, reply: record) {
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
            mark_unknown(session, request);
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

fn probes(startup: &startup_descriptor, buffers: &mut buffers<'_>) {
    // Neither application role owns the former interactive pair's physical
    // input/display gates. Authority is checked before decoding these pointers.
    for operation in [7, 15, 16] {
        assert_eq!(call(operation, 0, 0, 0, 0).status, 2);
    }
    if startup.role() == 1 {
        buffers.sector.fill(0x73);
        for operation in [12, 13] {
            assert_eq!(
                call(operation, 1, buffers.sector.as_mut_ptr() as u64, 512, 0).status,
                2
            );
            assert!(buffers.sector.iter().all(|byte| *byte == 0x73));
        }
        assert_eq!(call(14, 1, 0, 0, 0).status, 2);
    } else {
        // Public handle numerics do not let the service inspect actions or
        // impersonate an editor observation on workspace/editor routes.
        assert_eq!(
            call(
                2,
                startup.handles()[0],
                buffers.action.as_mut_ptr() as u64,
                remote_request_size,
                0
            )
            .status,
            2
        );
        assert_eq!(
            call(
                1,
                startup.handles()[1],
                buffers.message.as_ptr() as u64,
                remote_message_size,
                1
            )
            .status,
            2
        );
    }
}

fn editor(
    startup: &startup_descriptor,
    buffers: &mut buffers<'_>,
    slot: &mut MaybeUninit<remote_editor_session>,
) -> ! {
    let handles = startup.handles();
    if !wait_receive(handles[3], 1, buffers.reply) {
        exit();
    }
    let ready = mount_ready::decode(buffers.reply).unwrap();
    buffers
        .request
        .copy_from_slice(&record::open(ready.document(), 1).unwrap().encode());
    if !send(handles[2], 1, buffers.request)
        || !await_ack(handles[2], 1)
        || !wait_receive(handles[3], 2, buffers.reply)
    {
        exit();
    }
    let opened = record::decode(buffers.reply).unwrap();
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
        remote_editor_session::new(
            startup.connection(),
            interactive_editor_session::from_open(
                opened.document(),
                opened.generation(),
                opened.version(),
                opened.text(),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let mut action_sequence = 1_u64;
    let mut message_sequence = 1_u64;
    let mut request_sequence = 2_u64;
    let mut reply_sequence = 3_u64;
    let mut service_live = true;
    let mut pending: Option<record> = None;
    let mut first_save = true;
    let mut released = false;
    if !publish(
        handles[1],
        &mut message_sequence,
        session.observation(),
        buffers.message,
    ) {
        exit();
    }
    let mut published = session.view().sequence();
    loop {
        if service_live {
            // Even without a pending request, observe terminal service closure.
            match receive(handles[3], reply_sequence, buffers.reply) {
                Ok(true) => {
                    reply_sequence = reply_sequence.checked_add(1).unwrap();
                    let request = pending.take().expect("unsolicited document reply");
                    let reply = record::decode(buffers.reply).unwrap();
                    process_saved(session, request, reply);
                    if reply.kind() == operation::saved {
                        first_save = false;
                    }
                }
                Ok(false) => {}
                Err(()) => {
                    service_live = false;
                    if let Some(request) = pending {
                        mark_unknown(session, request);
                    }
                }
            }
        }
        if published != session.view().sequence() {
            if !publish(
                handles[1],
                &mut message_sequence,
                session.observation(),
                buffers.message,
            ) {
                exit();
            }
            published = session.view().sequence();
        }
        match receive(handles[0], action_sequence, buffers.action) {
            Ok(true) => {
                action_sequence = action_sequence.checked_add(1).unwrap();
                // This call follows the actual successful receive on the
                // kernel-installed W→E route, not a packet's claimed identity.
                let result = session.execute_bytes(buffers.action).unwrap();
                if !publish(
                    handles[1],
                    &mut message_sequence,
                    result.message,
                    buffers.message,
                ) {
                    exit();
                }
                published = session.view().sequence();
                if session.view().is_closed() {
                    exit();
                }
                if let remote_effect::save(snapshot) = result.effect {
                    assert!(pending.is_none());
                    let request = record::save(
                        snapshot.document(),
                        snapshot.generation(),
                        snapshot.request(),
                        snapshot.base_version(),
                        snapshot.revision(),
                        snapshot.text(),
                    )
                    .unwrap();
                    pending = Some(request);
                    released = false;
                    // The reservation completion has been received before any
                    // service can commit this request or lose its reply.
                    buffers.request.copy_from_slice(&request.encode());
                    if service_live
                        && send(handles[2], request_sequence, buffers.request)
                        && await_ack(handles[2], request_sequence)
                    {
                        request_sequence = request_sequence.checked_add(1).unwrap();
                    } else {
                        service_live = false;
                        mark_unknown(session, request);
                    }
                }
                if service_live
                    && startup.proof_mode() == 1
                    && first_save
                    && !released
                    && let Some(request) = pending
                    && session.view().revision() > request.revision()
                {
                    let release =
                        held_save_release::try_new(&request, session.view().revision()).unwrap();
                    buffers.request.copy_from_slice(&release.encode());
                    // Do not wait for this marker's ACK here: a
                    // definitive rejection may already be queued in
                    // the reverse direction. Drain that reply first;
                    // later sends retain ordinary channel backpressure.
                    if send(handles[2], request_sequence, buffers.request) {
                        request_sequence = request_sequence.checked_add(1).unwrap();
                        released = true;
                    } else {
                        service_live = false;
                        mark_unknown(session, request);
                    }
                }
            }
            Ok(false) => pause(),
            Err(()) => exit(),
        }
    }
}

fn service(startup: &startup_descriptor, buffers: &mut buffers<'_>) -> ! {
    let mut device = gate_device {
        handle: startup.device_handle(),
    };
    if startup.phase() == 1 {
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
    if !send(handles[3], 1, buffers.reply) || !await_ack(handles[3], 1) {
        exit();
    }
    let mut request_sequence = 1_u64;
    let mut reply_sequence = 2_u64;
    let mut first_save = true;
    let mut rejected_release: Option<record> = None;
    loop {
        if !wait_receive(handles[2], request_sequence, buffers.request) {
            exit();
        }
        request_sequence = request_sequence.checked_add(1).unwrap();
        if &buffers.request[..4] == b"ANDH" {
            // A later local edit can race a definitive first-save rejection.
            // Accept its exact marker once as a correlation-only no-op; it
            // cannot release a successful reply or execute another request.
            assert_eq!(startup.proof_mode(), 1);
            let release = held_save_release::decode(buffers.request).unwrap();
            assert!(release.matches_request(&rejected_release.take().unwrap()));
            continue;
        }
        rejected_release = None;
        let request = record::decode(buffers.request).unwrap();
        let reply = match store.execute(startup.authority(), &request) {
            Ok(reply) => reply,
            Err(durable_error::rejected(error)) => record::rejection_for(&request, error).unwrap(),
            Err(_) => exit(),
        };
        if startup.proof_mode() == 1
            && first_save
            && request.kind() == operation::save
            && reply.kind() == operation::rejected
        {
            rejected_release = Some(request);
        }
        if request.kind() == operation::save && reply.kind() == operation::saved && first_save {
            first_save = false;
            if startup.proof_mode() == 2 {
                exit();
            }
            if startup.proof_mode() == 1 {
                // Only the separately authenticated E→S document route can
                // release this fixture-held reply. No workspace ANUI/action
                // route is visible here, and ANDC request IDs stay unchanged.
                if !wait_receive(handles[2], request_sequence, buffers.request) {
                    exit();
                }
                let release = held_save_release::decode(buffers.request).unwrap();
                assert!(release.matches_saved(&reply));
                request_sequence = request_sequence.checked_add(1).unwrap();
            }
        }
        buffers.reply.copy_from_slice(&reply.encode());
        if !send(handles[3], reply_sequence, buffers.reply)
            || !await_ack(handles[3], reply_sequence)
        {
            exit();
        }
        reply_sequence = reply_sequence.checked_add(1).unwrap();
    }
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_native_durable_client_run(pointer: *const u8, length: usize) -> ! {
    assert_eq!(pointer as u64, 0x801000);
    assert_eq!(length, startup_size);
    let mut bytes = [0; startup_size];
    // SAFETY: Admission supplied this task's complete private descriptor, which
    // is copied before parsing. Decoded numerics cannot create native grants.
    unsafe { ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), startup_size) };
    let startup = startup_descriptor::decode(&bytes).unwrap();
    assert!((1..=2).contains(&startup.role()));
    // SAFETY: The selected native layout maps exactly this disjoint private
    // RW/NX arena. This is its sole borrow, partitioned into exclusive slices.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(
            startup.arena_base() as *mut u8,
            startup.arena_length() as usize,
        )
    };
    for byte in storage.iter() {
        assert_eq!(unsafe { ptr::read_volatile(byte) }, 0);
    }
    let mut arena = arena::new(storage);
    let state = arena
        .allocate(
            core::mem::size_of::<MaybeUninit<remote_editor_session>>(),
            64,
        )
        .unwrap();
    // SAFETY: Exact-size aligned unique storage, initialized once by editor().
    let slot = unsafe {
        &mut *state
            .as_mut_ptr()
            .cast::<MaybeUninit<remote_editor_session>>()
    };
    let mut buffers = buffers {
        action: arena
            .allocate(remote_request_size, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        message: arena
            .allocate(remote_message_size, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        request: arena.allocate(record_size, 64).unwrap().try_into().unwrap(),
        reply: arena.allocate(record_size, 64).unwrap().try_into().unwrap(),
        sector: arena.allocate(512, 64).unwrap().try_into().unwrap(),
    };
    probes(&startup, &mut buffers);
    if startup.role() == 1 {
        editor(&startup, &mut buffers, slot)
    } else {
        service(&startup, &mut buffers)
    }
}
