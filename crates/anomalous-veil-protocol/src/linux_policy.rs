//! Userspace-only Linux x86-64 policy for two preopened Anomalous record
//! devices. These are not pipes, a VFS, libc, blocking poll or general POSIX.
//! Every backend mutation is staged; the caller completes the proposal through
//! the native supervisor only after evaluation succeeds.

use crate::{
    channel_phase, channel_snapshot, completion, completion_action, invocation, native_status,
    packet_capacity, stage_kind, stage_receipt,
};

const bad_fd: i64 = -9;
const again: i64 = -11;
const bad_address: i64 = -14;
const no_device: i64 = -19;
const invalid_argument: i64 = -22;
const not_implemented: i64 = -38;
const overflow: i64 = -75;
const message_size: i64 = -90;
const poll_capacity: usize = 8;
const poll_entry_size: usize = 8;
const poll_input: u16 = 1;
const poll_output: u16 = 4;
const poll_error: u16 = 8;
const poll_hangup: u16 = 16;
const poll_invalid: u16 = 32;

/// Native capabilities 0/1 are configured outside the wire. Only this module
/// assigns them Linux descriptor numbers or interprets their states as pollfd
/// readiness. The backend is bound to one authenticated pending invocation.
pub trait backend {
    fn query(&mut self, capability: u64) -> Result<channel_snapshot, native_status>;
    fn caller_read(&mut self, address: u64, destination: &mut [u8]) -> Result<(), native_status>;
    fn stage(
        &mut self,
        kind: stage_kind,
        address: u64,
        length: u64,
        source: &[u8],
    ) -> Result<stage_receipt, native_status>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum policy_error {
    invalid_stage_receipt,
}

pub fn evaluate(
    request: invocation,
    backend: &mut impl backend,
) -> Result<completion, policy_error> {
    // Linux dispatch uses a signed int number. All six raw argument words are
    // preserved by the transport, including currently unused arguments.
    let number = request.number() as u32 as i32;
    let arguments = request.arguments();
    match number {
        0 => read(request.sequence(), arguments, backend),
        1 => write(request.sequence(), arguments, backend),
        7 => poll(request.sequence(), arguments, backend),
        24 => Ok(completion {
            result_bits: 0,
            action: completion_action::yield_caller,
            stage_token: 0,
        }),
        60 => Ok(completion {
            result_bits: arguments[0] as u8 as u64,
            action: completion_action::retire_caller,
            stage_token: 0,
        }),
        _ => Ok(completion::returned(not_implemented)),
    }
}

fn record_length(value: u64) -> Result<usize, i64> {
    if value > packet_capacity as u64 {
        Err(message_size)
    } else {
        Ok(value as usize)
    }
}

fn native_error(status: native_status) -> i64 {
    match status {
        native_status::not_ready => again,
        native_status::denied => bad_fd,
        native_status::bad_copy => bad_address,
        native_status::closed => no_device,
        native_status::too_small => message_size,
        native_status::exhausted => overflow,
        native_status::invalid | native_status::ok => invalid_argument,
    }
}

fn staged(
    sequence: u64,
    receipt: stage_receipt,
    length: usize,
    result: i64,
) -> Result<completion, policy_error> {
    if receipt.token != sequence || receipt.length != length as u64 {
        return Err(policy_error::invalid_stage_receipt);
    }
    Ok(completion {
        result_bits: result as u64,
        action: completion_action::return_to_caller,
        stage_token: receipt.token,
    })
}

fn write(
    sequence: u64,
    arguments: [u64; 6],
    backend: &mut impl backend,
) -> Result<completion, policy_error> {
    if arguments[0] as u32 != 3 {
        return Ok(completion::returned(bad_fd));
    }
    let length = match record_length(arguments[2]) {
        Ok(0) => return Ok(completion::returned(0)),
        Ok(length) => length,
        Err(error) => return Ok(completion::returned(error)),
    };
    let snapshot = match backend.query(0) {
        Ok(value) => value,
        Err(error) => return Ok(completion::returned(native_error(error))),
    };
    match snapshot.phase() {
        channel_phase::closed => return Ok(completion::returned(no_device)),
        channel_phase::queued | channel_phase::delivered => return Ok(completion::returned(again)),
        channel_phase::empty => {}
    }
    if snapshot.exhausted() {
        return Ok(completion::returned(overflow));
    }
    match backend.stage(stage_kind::submit, arguments[1], length as u64, &[]) {
        Ok(receipt) => staged(sequence, receipt, length, length as i64),
        Err(error) => Ok(completion::returned(native_error(error))),
    }
}

fn read(
    sequence: u64,
    arguments: [u64; 6],
    backend: &mut impl backend,
) -> Result<completion, policy_error> {
    if arguments[0] as u32 != 4 {
        return Ok(completion::returned(bad_fd));
    }
    let capacity = match record_length(arguments[2]) {
        Ok(0) => return Ok(completion::returned(0)),
        Ok(length) => length,
        Err(error) => return Ok(completion::returned(error)),
    };
    let snapshot = match backend.query(1) {
        Ok(value) => value,
        Err(error) => return Ok(completion::returned(native_error(error))),
    };
    match snapshot.phase() {
        channel_phase::closed => return Ok(completion::returned(0)),
        channel_phase::empty | channel_phase::delivered => return Ok(completion::returned(again)),
        channel_phase::queued => {}
    }
    if capacity < snapshot.length() {
        return Ok(completion::returned(message_size));
    }
    match backend.stage(stage_kind::consume, arguments[1], capacity as u64, &[]) {
        Ok(receipt) => staged(
            sequence,
            receipt,
            snapshot.length(),
            snapshot.length() as i64,
        ),
        Err(native_status::closed) => Ok(completion::returned(0)),
        Err(error) => Ok(completion::returned(native_error(error))),
    }
}

fn poll(
    sequence: u64,
    arguments: [u64; 6],
    backend: &mut impl backend,
) -> Result<completion, policy_error> {
    let count = arguments[1] as u32 as usize;
    let timeout = arguments[2] as u32 as i32;
    if count > poll_capacity || timeout != 0 {
        return Ok(completion::returned(invalid_argument));
    }
    if count == 0 {
        return Ok(completion::returned(0));
    }
    let mut scratch = [0; poll_capacity * poll_entry_size];
    let entries = &mut scratch[..count * poll_entry_size];
    if let Err(error) = backend.caller_read(arguments[0], entries) {
        return Ok(completion::returned(native_error(error)));
    }
    let snapshots = match (backend.query(0), backend.query(1)) {
        (Ok(output), Ok(input)) => [output, input],
        (Err(error), _) | (_, Err(error)) => return Ok(completion::returned(native_error(error))),
    };
    let ready = poll_results(entries, snapshots);
    let result = match backend.stage(
        stage_kind::copyout,
        arguments[0],
        entries.len() as u64,
        entries,
    ) {
        Ok(receipt) => staged(sequence, receipt, entries.len(), ready as i64),
        Err(error) => Ok(completion::returned(native_error(error))),
    };
    scratch.fill(0);
    result
}

fn poll_flags(fd: i32, events: u16, snapshots: [channel_snapshot; 2]) -> u16 {
    match fd {
        value if value < 0 => 0,
        3 => match snapshots[0].phase() {
            channel_phase::closed => poll_error | poll_hangup,
            channel_phase::empty if snapshots[0].exhausted() => poll_error,
            channel_phase::empty => events & poll_output,
            channel_phase::queued | channel_phase::delivered => 0,
        },
        4 => match snapshots[1].phase() {
            channel_phase::closed => poll_hangup,
            channel_phase::queued => events & poll_input,
            channel_phase::empty | channel_phase::delivered => 0,
        },
        _ => poll_invalid,
    }
}

fn poll_results(entries: &mut [u8], snapshots: [channel_snapshot; 2]) -> usize {
    let mut ready = 0;
    for entry in entries.chunks_exact_mut(poll_entry_size) {
        let fd = i32::from_le_bytes(entry[..4].try_into().unwrap());
        let events = u16::from_le_bytes(entry[4..6].try_into().unwrap());
        let flags = poll_flags(fd, events, snapshots);
        entry[6..8].copy_from_slice(&flags.to_le_bytes());
        ready += usize::from(flags != 0);
    }
    ready
}

#[cfg(test)]
mod tests;
