#![forbid(unsafe_code)]
#![cfg_attr(not(test), allow(dead_code))]

//! Retained console binding for the bounded ANPG register gate.
//!
//! This state grants no memory access. The caller must validate the live task
//! and retained root, exclude concurrent execution, and supply checked copying
//! through that owner. One exclusive borrow spans admission through publication.

use anomalous_native_runtime::{
    maximum_arena_bytes, maximum_write_bytes, minimum_arena_bytes, program_status,
    validate_console_text,
};

use super::mapping_plan::user_address_limit;
use super::paging::page_size;
use crate::identity::task_identity;

pub(super) struct write_request {
    pub caller: task_identity,
    pub handle: u64,
    pub address: u64,
    pub length: u64,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct write_reply {
    pub status: u64,
    pub sequence: u64,
    pub length: u64,
}

impl write_reply {
    const fn rejected(status: u64) -> Self {
        Self {
            status,
            sequence: 0,
            length: 0,
        }
    }
}

pub(super) struct state {
    identity: task_identity,
    handle: Option<u64>,
    arena_base: u64,
    arena_end: u64,
    next_sequence: u64,
    bytes_written: u64,
    closed: bool,
}

impl state {
    /// The controller supplies the binding selected before task publication.
    /// A decoded userspace descriptor must never be used to replace it.
    pub(super) fn new(
        identity: task_identity,
        handle: Option<u64>,
        arena_base: u64,
        arena_length: u64,
    ) -> Result<Self, u64> {
        let arena_end = arena_base
            .checked_add(arena_length)
            .filter(|end| *end <= user_address_limit)
            .ok_or(program_status::invalid)?;
        if handle == Some(0)
            || arena_base == 0
            || !arena_base.is_multiple_of(page_size)
            || !arena_length.is_multiple_of(page_size)
            || !(minimum_arena_bytes..=maximum_arena_bytes).contains(&arena_length)
        {
            return Err(program_status::invalid);
        }
        Ok(Self {
            identity,
            handle,
            arena_base,
            arena_end,
            next_sequence: 1,
            bytes_written: 0,
            closed: false,
        })
    }

    pub(super) const fn written(&self) -> u64 {
        self.next_sequence - 1
    }

    pub(super) const fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub(super) const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Revoke before retiring the task or releasing its mapping owner.
    pub(super) fn close(&mut self) {
        self.closed = true;
        self.handle = None;
    }

    /// Copy and validate the whole request before the sole publication call.
    /// Rejections consume neither a sequence nor any output. Scratch is cleared
    /// on entry and every return, including partial copy failures.
    ///
    /// `copy` must read only from the retained current task after complete range
    /// preflight. `publish` commits the complete validated text synchronously;
    /// it must either complete or never return. A sink capable of partial or
    /// recoverable publication needs its own transaction and is not this API.
    pub(super) fn write(
        &mut self,
        request: write_request,
        scratch: &mut [u8; maximum_write_bytes],
        copy: impl FnOnce(u64, &mut [u8]) -> Result<(), ()>,
        publish: impl FnOnce(&str),
    ) -> write_reply {
        scratch.fill(0);
        let result = self.write_inner(request, scratch, copy, publish);
        scratch.fill(0);
        result
    }

    fn write_inner(
        &mut self,
        request: write_request,
        scratch: &mut [u8; maximum_write_bytes],
        copy: impl FnOnce(u64, &mut [u8]) -> Result<(), ()>,
        publish: impl FnOnce(&str),
    ) -> write_reply {
        if self.closed {
            return write_reply::rejected(program_status::closed);
        }
        if request.caller != self.identity
            || self.handle.is_none()
            || self.handle != Some(request.handle)
        {
            return write_reply::rejected(program_status::denied);
        }
        if request.sequence == 0
            || request.sequence == u64::MAX
            || request.sequence != self.next_sequence
            || request.length == 0
            || request.length > maximum_write_bytes as u64
        {
            return write_reply::rejected(program_status::invalid);
        }
        if request.address < self.arena_base
            || request
                .address
                .checked_add(request.length)
                .is_none_or(|end| end > self.arena_end)
        {
            return write_reply::rejected(program_status::bad_address);
        }
        let Some(next_bytes) = self.bytes_written.checked_add(request.length) else {
            return write_reply::rejected(program_status::invalid);
        };
        let bytes = &mut scratch[..request.length as usize];
        if copy(request.address, bytes).is_err() {
            return write_reply::rejected(program_status::bad_address);
        }
        let Ok(text) = validate_console_text(bytes) else {
            return write_reply::rejected(program_status::invalid);
        };
        publish(text);
        self.next_sequence += 1;
        self.bytes_written = next_bytes;
        write_reply {
            status: program_status::ok,
            sequence: request.sequence,
            length: request.length,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::identity::{circle_id, task_id};
    use core::cell::Cell;
    use std::vec::Vec;

    const base: u64 = 0x804000;
    const handle: u64 = 0x62;

    fn identity(task_slot: usize, task_generation: u64, circle_generation: u64) -> task_identity {
        task_identity::new(
            task_id::try_from_parts(task_slot, task_generation).unwrap(),
            circle_id::try_from_parts(3, circle_generation).unwrap(),
        )
    }

    fn fixture() -> state {
        state::new(identity(2, 7, 4), Some(handle), base, minimum_arena_bytes).unwrap()
    }

    fn request(sequence: u64, length: u64) -> write_request {
        write_request {
            caller: identity(2, 7, 4),
            handle,
            address: base + 4094,
            length,
            sequence,
        }
    }

    fn assert_preflight_rejection(owner: &mut state, request: write_request, status: u64) {
        let before = (owner.written(), owner.bytes_written());
        let mut scratch = [0xa5; maximum_write_bytes];
        let result = owner.write(
            request,
            &mut scratch,
            |_, _| panic!("rejected request copied user bytes"),
            |_| panic!("rejected request published bytes"),
        );
        assert_eq!(result, write_reply::rejected(status));
        assert_eq!((owner.written(), owner.bytes_written()), before);
        assert!(scratch.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn controller_binding_requires_an_exact_nonzero_handle_and_program_arena() {
        let caller = identity(2, 7, 4);
        assert!(state::new(caller, None, base, minimum_arena_bytes).is_ok());
        assert!(state::new(caller, Some(0), base, minimum_arena_bytes).is_err());
        for (address, length) in [
            (0, minimum_arena_bytes),
            (base + 1, minimum_arena_bytes),
            (base, 0),
            (base, minimum_arena_bytes - page_size),
            (base, minimum_arena_bytes + 1),
            (base, maximum_arena_bytes + page_size),
            (user_address_limit - page_size, minimum_arena_bytes),
            (u64::MAX - page_size + 1, minimum_arena_bytes),
            (base, u64::MAX),
        ] {
            assert!(state::new(caller, Some(handle), address, length).is_err());
        }
        assert!(
            state::new(
                caller,
                Some(handle),
                user_address_limit - minimum_arena_bytes,
                minimum_arena_bytes,
            )
            .is_ok()
        );
    }

    #[test]
    fn complete_unicode_writes_publish_once_and_return_exact_receipts() {
        let mut owner = fixture();
        let mut scratch = [0xa5; maximum_write_bytes];
        let mut published = Vec::new();
        let mut total = 0;
        for (index, text) in ["héllo — ok\n", "a\tb\n"].into_iter().enumerate() {
            let sequence = index as u64 + 1;
            let copied = Cell::new(false);
            let result = owner.write(
                request(sequence, text.len() as u64),
                &mut scratch,
                |address, into| {
                    assert_eq!(address, base + 4094);
                    assert!(into.iter().all(|byte| *byte == 0));
                    into.copy_from_slice(text.as_bytes());
                    copied.set(true);
                    Ok(())
                },
                |actual| {
                    assert!(copied.get());
                    published.push(actual.as_bytes().to_vec());
                },
            );
            total += text.len() as u64;
            assert_eq!(
                result,
                write_reply {
                    status: program_status::ok,
                    sequence,
                    length: text.len() as u64,
                }
            );
            assert_eq!(owner.written(), sequence);
            assert_eq!(owner.bytes_written(), total);
            assert!(scratch.iter().all(|byte| *byte == 0));
        }
        assert_eq!(published.len(), 2);
        assert_eq!(published[0], "héllo — ok\n".as_bytes());
        assert_eq!(published[1], b"a\tb\n");
    }

    #[test]
    fn wrong_handle_or_identity_cannot_copy_even_when_other_fields_are_malformed() {
        let mut owner = fixture();
        for wrong in [0, 0x41, handle + 1, u64::MAX] {
            let mut value = request(1, 8);
            value.handle = wrong;
            value.address = u64::MAX;
            assert_preflight_rejection(&mut owner, value, program_status::denied);
        }
        for wrong in [identity(3, 7, 4), identity(2, 8, 4), identity(2, 7, 5)] {
            let mut value = request(1, 8);
            value.caller = wrong;
            assert_preflight_rejection(&mut owner, value, program_status::denied);
        }
        let mut absent = state::new(identity(2, 7, 4), None, base, minimum_arena_bytes).unwrap();
        assert_preflight_rejection(&mut absent, request(1, 8), program_status::denied);
    }

    #[test]
    fn invalid_sequences_sizes_and_ranges_leave_no_effects() {
        let mut owner = fixture();
        for sequence in [0, 2, u64::MAX] {
            assert_preflight_rejection(&mut owner, request(sequence, 8), program_status::invalid);
        }
        for length in [0, maximum_write_bytes as u64 + 1, u64::MAX] {
            assert_preflight_rejection(&mut owner, request(1, length), program_status::invalid);
        }
        for (address, length) in [
            (base - 1, 1),
            (base + minimum_arena_bytes, 1),
            (base + minimum_arena_bytes - 1, 2),
            (u64::MAX, 1),
        ] {
            let mut value = request(1, length);
            value.address = address;
            assert_preflight_rejection(&mut owner, value, program_status::bad_address);
        }
    }

    #[test]
    fn copy_and_text_failures_wipe_scratch_and_preserve_retry_sequence() {
        let mut owner = fixture();
        let mut scratch = [0xa5; maximum_write_bytes];
        for partial in [false, true] {
            let result = owner.write(
                request(1, 8),
                &mut scratch,
                |_, into| {
                    if partial {
                        into[..4].fill(b'x');
                    }
                    Err(())
                },
                |_| panic!("failed copy published bytes"),
            );
            assert_eq!(result, write_reply::rejected(program_status::bad_address));
            assert_eq!(owner.written(), 0);
            assert_eq!(owner.bytes_written(), 0);
            assert!(scratch.iter().all(|byte| *byte == 0));
        }
        for text in [
            b"bad\xff".as_slice(),
            b"before\x1b[2J",
            "before\u{009b}2J".as_bytes(),
            b"before\0after",
            b"before\x7fafter",
        ] {
            let result = owner.write(
                request(1, text.len() as u64),
                &mut scratch,
                |_, into| {
                    into.copy_from_slice(text);
                    Ok(())
                },
                |_| panic!("invalid text published a prefix"),
            );
            assert_eq!(result, write_reply::rejected(program_status::invalid));
            assert_eq!(owner.written(), 0);
            assert_eq!(owner.bytes_written(), 0);
            assert!(scratch.iter().all(|byte| *byte == 0));
        }
        let result = owner.write(
            request(1, 2),
            &mut scratch,
            |_, into| {
                into.copy_from_slice(b"ok");
                Ok(())
            },
            |text| assert_eq!(text, "ok"),
        );
        assert_eq!(result.status, program_status::ok);
        assert_eq!(result.sequence, 1);
        assert_eq!(result.length, 2);
        assert_preflight_rejection(&mut owner, request(1, 2), program_status::invalid);
        assert_eq!(owner.written(), 1);
        assert_eq!(owner.bytes_written(), 2);
    }

    #[test]
    fn exact_bounds_are_admitted_and_success_counters_never_wrap() {
        let mut owner = fixture();
        let mut scratch = [0; maximum_write_bytes];
        let mut value = request(1, maximum_write_bytes as u64);
        value.address = base + minimum_arena_bytes - maximum_write_bytes as u64;
        let result = owner.write(
            value,
            &mut scratch,
            |_, into| {
                into.fill(b'x');
                Ok(())
            },
            |text| assert_eq!(text.len(), maximum_write_bytes),
        );
        assert_eq!(result.status, program_status::ok);
        assert_eq!(result.length, maximum_write_bytes as u64);
        owner.bytes_written = u64::MAX;
        assert_preflight_rejection(&mut owner, request(2, 1), program_status::invalid);
        owner.bytes_written = 0;
        owner.next_sequence = u64::MAX;
        assert_preflight_rejection(&mut owner, request(u64::MAX, 1), program_status::invalid);
    }

    #[test]
    fn closed_binding_cannot_copy_publish_or_be_revived_by_a_matching_request() {
        let mut owner = fixture();
        owner.close();
        assert!(owner.is_closed());
        assert_eq!(owner.handle, None);
        assert_preflight_rejection(&mut owner, request(1, 8), program_status::closed);
        owner.close();
        assert_preflight_rejection(&mut owner, request(1, 8), program_status::closed);
    }
}
