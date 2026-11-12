#![forbid(unsafe_code)]
#![cfg_attr(not(test), allow(dead_code))]

//! Private retained-edition binding for the ANPG gate, without filesystem policy.
//! The embedding validates the live task/root and keeps one excluded ownership
//! interval through destination preflight, source read, copyout and reply.

use anomalous_native_runtime::{
    maximum_arena_bytes, maximum_edition_bytes, maximum_read_bytes, minimum_arena_bytes,
    program_status,
};

use super::mapping_plan::user_address_limit;
use super::paging::page_size;
use crate::identity::task_identity;

pub(super) struct read_request {
    pub caller: task_identity,
    pub handle: u64,
    pub address: u64,
    pub length: u64,
    pub sequence: u64,
    pub offset: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct read_reply {
    pub status: u64,
    pub sequence: u64,
    pub length: u64,
}

impl read_reply {
    const fn rejected(status: u64) -> Self {
        Self {
            status,
            sequence: 0,
            length: 0,
        }
    }
}

/// All memory authority belongs to the retained embedding, not this state.
pub(super) trait read_io {
    /// Validate the complete requested capacity before any source access.
    fn preflight(&mut self, address: u64, length: usize) -> Result<(), ()>;
    /// Read the exact planned source extent. Failure may dirty scratch only.
    fn read(&mut self, offset: u64, destination: &mut [u8]) -> Result<usize, ()>;
    /// Err guarantees that no destination byte changed. Unexpected partial
    /// hardware publication must be fatal, never a retryable Err.
    fn publish(&mut self, address: u64, bytes: &[u8]) -> Result<(), ()>;
}

pub(super) struct state {
    identity: task_identity,
    handle: Option<u64>,
    edition_length: u64,
    arena_base: u64,
    arena_end: u64,
    next_sequence: u64,
    bytes_read: u64,
    closed: bool,
}

impl state {
    pub(super) fn new(
        identity: task_identity,
        handle: Option<u64>,
        edition_length: u64,
        arena_base: u64,
        arena_length: u64,
    ) -> Result<Self, u64> {
        let arena_end = arena_base
            .checked_add(arena_length)
            .filter(|end| *end <= user_address_limit)
            .ok_or(program_status::invalid)?;
        if handle == Some(0)
            || edition_length == 0
            || edition_length > maximum_edition_bytes
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
            edition_length,
            arena_base,
            arena_end,
            next_sequence: 1,
            bytes_read: 0,
            closed: false,
        })
    }

    pub(super) const fn reads(&self) -> u64 {
        self.next_sequence - 1
    }

    pub(super) const fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    pub(super) const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Revoke before task retirement, mapping withdrawal or source cleanup.
    pub(super) fn close(&mut self) {
        self.closed = true;
        self.handle = None;
    }

    /// Scratch is logically cleared on entry and every returning path. This
    /// does not claim erasure after abort or an unrecoverable hardware fault.
    pub(super) fn read(
        &mut self,
        request: read_request,
        scratch: &mut [u8; maximum_read_bytes],
        io: &mut impl read_io,
    ) -> read_reply {
        scratch.fill(0);
        let result = self.read_inner(request, scratch, io);
        scratch.fill(0);
        result
    }

    fn read_inner(
        &mut self,
        request: read_request,
        scratch: &mut [u8; maximum_read_bytes],
        io: &mut impl read_io,
    ) -> read_reply {
        if self.closed {
            return read_reply::rejected(program_status::closed);
        }
        if request.caller != self.identity
            || self.handle.is_none()
            || self.handle != Some(request.handle)
        {
            return read_reply::rejected(program_status::denied);
        }
        if request.sequence == 0
            || request.sequence == u64::MAX
            || request.sequence != self.next_sequence
            || request.length == 0
            || request.length > maximum_read_bytes as u64
            || request.offset.checked_add(request.length).is_none()
        {
            return read_reply::rejected(program_status::invalid);
        }
        if request.address < self.arena_base
            || request
                .address
                .checked_add(request.length)
                .is_none_or(|end| end > self.arena_end)
        {
            return read_reply::rejected(program_status::bad_address);
        }
        let length = request
            .length
            .min(self.edition_length.saturating_sub(request.offset));
        let Some(next_bytes) = self.bytes_read.checked_add(length) else {
            return read_reply::rejected(program_status::invalid);
        };
        if io
            .preflight(request.address, request.length as usize)
            .is_err()
        {
            return read_reply::rejected(program_status::bad_address);
        }
        if length != 0 {
            let bytes = &mut scratch[..length as usize];
            if io.read(request.offset, bytes) != Ok(bytes.len()) {
                return read_reply::rejected(program_status::unavailable);
            }
            if io.publish(request.address, bytes).is_err() {
                return read_reply::rejected(program_status::bad_address);
            }
        }
        self.next_sequence += 1;
        self.bytes_read = next_bytes;
        read_reply {
            status: program_status::ok,
            sequence: request.sequence,
            length,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::identity::{circle_id, task_id};
    use std::vec::Vec;

    const base: u64 = 0x804000;
    const handle: u64 = 0x41;
    const edition_length: u64 = 4097;

    fn identity(slot: usize, generation: u64, circle_generation: u64) -> task_identity {
        task_identity::new(
            task_id::try_from_parts(slot, generation).unwrap(),
            circle_id::try_from_parts(3, circle_generation).unwrap(),
        )
    }

    fn fixture() -> state {
        state::new(
            identity(2, 7, 4),
            Some(handle),
            edition_length,
            base,
            minimum_arena_bytes,
        )
        .unwrap()
    }

    fn request(sequence: u64, offset: u64, length: u64) -> read_request {
        read_request {
            caller: identity(2, 7, 4),
            handle,
            address: base + 4095,
            length,
            sequence,
            offset,
        }
    }

    #[derive(Clone, Copy, Default)]
    enum failure {
        #[default]
        none,
        preflight,
        source,
        short,
        oversized,
        publication,
    }

    struct io_fixture {
        failure: failure,
        events: Vec<&'static str>,
        preflight: Option<(u64, usize)>,
        destination: [u8; maximum_read_bytes],
    }

    impl Default for io_fixture {
        fn default() -> Self {
            Self {
                failure: failure::none,
                events: Vec::new(),
                preflight: None,
                destination: [0xa5; maximum_read_bytes],
            }
        }
    }

    impl read_io for io_fixture {
        fn preflight(&mut self, address: u64, length: usize) -> Result<(), ()> {
            assert!(self.events.is_empty());
            self.events.push("preflight");
            self.preflight = Some((address, length));
            if matches!(self.failure, failure::preflight) {
                Err(())
            } else {
                Ok(())
            }
        }

        fn read(&mut self, offset: u64, destination: &mut [u8]) -> Result<usize, ()> {
            assert_eq!(self.events, ["preflight"]);
            assert!(destination.iter().all(|byte| *byte == 0));
            assert!(offset + destination.len() as u64 <= edition_length);
            self.events.push("read");
            if matches!(self.failure, failure::source) {
                let dirty = destination.len().min(3);
                destination[..dirty].fill(0x77);
                return Err(());
            }
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = (offset + index as u64) as u8;
            }
            match self.failure {
                failure::short => Ok(destination.len() - 1),
                failure::oversized => Ok(destination.len() + 1),
                _ => Ok(destination.len()),
            }
        }

        fn publish(&mut self, address: u64, bytes: &[u8]) -> Result<(), ()> {
            assert_eq!(self.events, ["preflight", "read"]);
            assert_eq!(Some(address), self.preflight.map(|value| value.0));
            self.events.push("publish");
            if matches!(self.failure, failure::publication) {
                return Err(());
            }
            self.destination[..bytes.len()].copy_from_slice(bytes);
            Ok(())
        }
    }

    fn rejected(owner: &mut state, request: read_request, status: u64) {
        let before = (owner.reads(), owner.bytes_read());
        let mut scratch = [0xcc; maximum_read_bytes];
        let mut io = io_fixture::default();
        assert_eq!(
            owner.read(request, &mut scratch, &mut io),
            read_reply::rejected(status)
        );
        assert_eq!((owner.reads(), owner.bytes_read()), before);
        assert!(io.events.is_empty());
        assert_eq!(io.destination, [0xa5; maximum_read_bytes]);
        assert_eq!(scratch, [0; maximum_read_bytes]);
    }

    #[test]
    fn retained_binding_requires_bounded_source_and_arena() {
        let caller = identity(2, 7, 4);
        assert!(state::new(caller, None, edition_length, base, minimum_arena_bytes).is_ok());
        assert!(state::new(caller, Some(0), edition_length, base, minimum_arena_bytes).is_err());
        for length in [0, maximum_edition_bytes + 1, u64::MAX] {
            assert!(state::new(caller, Some(handle), length, base, minimum_arena_bytes).is_err());
        }
        for (address, length) in [
            (0, minimum_arena_bytes),
            (base + 1, minimum_arena_bytes),
            (base, minimum_arena_bytes - page_size),
            (base, minimum_arena_bytes + 1),
            (base, maximum_arena_bytes + page_size),
            (user_address_limit - page_size, minimum_arena_bytes),
            (u64::MAX - page_size + 1, minimum_arena_bytes),
        ] {
            assert!(state::new(caller, Some(handle), edition_length, address, length).is_err());
        }
    }

    #[test]
    fn wrong_incarnations_handles_and_absent_grants_never_touch_io() {
        let mut owner = fixture();
        for caller in [identity(3, 7, 4), identity(2, 8, 4), identity(2, 7, 5)] {
            let mut value = request(1, 0, 8);
            value.caller = caller;
            rejected(&mut owner, value, program_status::denied);
        }
        for wrong in [0, 0x62, handle + 1, u64::MAX] {
            let mut value = request(0, u64::MAX, 0);
            value.handle = wrong;
            rejected(&mut owner, value, program_status::denied);
        }
        let mut absent = state::new(
            identity(2, 7, 4),
            None,
            edition_length,
            base,
            minimum_arena_bytes,
        )
        .unwrap();
        rejected(&mut absent, request(1, 0, 8), program_status::denied);
    }

    #[test]
    fn malformed_sequences_capacity_offsets_and_full_destination_spans_are_refused() {
        let mut owner = fixture();
        for sequence in [0, 2, u64::MAX] {
            rejected(&mut owner, request(sequence, 0, 8), program_status::invalid);
        }
        for length in [0, maximum_read_bytes as u64 + 1, u64::MAX] {
            rejected(&mut owner, request(1, 0, length), program_status::invalid);
        }
        rejected(&mut owner, request(1, u64::MAX, 1), program_status::invalid);
        for (address, length) in [
            (base - 1, 1),
            (base + minimum_arena_bytes, 1),
            (base + minimum_arena_bytes - 1, 2),
            (u64::MAX, 1),
        ] {
            let mut value = request(1, edition_length, length);
            value.address = address;
            rejected(&mut owner, value, program_status::bad_address);
        }
    }

    #[test]
    fn binary_chunks_tail_and_eof_commit_exact_receipts_after_publication() {
        let mut owner = fixture();
        let mut scratch = [0xcc; maximum_read_bytes];
        let mut total = 0;
        for (index, (offset, delivered)) in [(0, 4096), (4096, 1), (4097, 0), (8192, 0)]
            .into_iter()
            .enumerate()
        {
            let sequence = index as u64 + 1;
            let mut io = io_fixture::default();
            assert_eq!(
                owner.read(request(sequence, offset, 4096), &mut scratch, &mut io),
                read_reply {
                    status: program_status::ok,
                    sequence,
                    length: delivered
                }
            );
            assert_eq!(io.preflight, Some((base + 4095, 4096)));
            if delivered == 0 {
                assert_eq!(io.events, ["preflight"]);
            } else {
                assert_eq!(io.events, ["preflight", "read", "publish"]);
            }
            for (position, byte) in io.destination[..delivered as usize].iter().enumerate() {
                assert_eq!(*byte, (offset + position as u64) as u8);
            }
            assert!(
                io.destination[delivered as usize..]
                    .iter()
                    .all(|byte| *byte == 0xa5)
            );
            assert_eq!(scratch, [0; maximum_read_bytes]);
            total += delivered;
            assert_eq!((owner.reads(), owner.bytes_read()), (sequence, total));
        }
        rejected(&mut owner, request(4, 0, 1), program_status::invalid);
    }

    #[test]
    fn preflight_failure_prevents_source_access_even_for_eof() {
        let mut owner = fixture();
        for offset in [0, edition_length] {
            let mut io = io_fixture {
                failure: failure::preflight,
                ..Default::default()
            };
            let mut scratch = [0xcc; maximum_read_bytes];
            assert_eq!(
                owner.read(request(1, offset, 8), &mut scratch, &mut io),
                read_reply::rejected(program_status::bad_address)
            );
            assert_eq!(io.events, ["preflight"]);
            assert_eq!(io.destination, [0xa5; maximum_read_bytes]);
            assert_eq!(scratch, [0; maximum_read_bytes]);
            assert_eq!((owner.reads(), owner.bytes_read()), (0, 0));
        }
    }

    #[test]
    fn dirty_failed_short_and_oversized_source_reads_cannot_publish() {
        let mut owner = fixture();
        for failure in [failure::source, failure::short, failure::oversized] {
            let mut io = io_fixture {
                failure,
                ..Default::default()
            };
            let mut scratch = [0xcc; maximum_read_bytes];
            assert_eq!(
                owner.read(request(1, 0, 8), &mut scratch, &mut io),
                read_reply::rejected(program_status::unavailable)
            );
            assert_eq!(io.events, ["preflight", "read"]);
            assert_eq!(io.destination, [0xa5; maximum_read_bytes]);
            assert_eq!(scratch, [0; maximum_read_bytes]);
            assert_eq!((owner.reads(), owner.bytes_read()), (0, 0));
        }
        let mut io = io_fixture::default();
        assert_eq!(
            owner
                .read(request(1, 0, 8), &mut [0; maximum_read_bytes], &mut io)
                .sequence,
            1
        );
    }

    #[test]
    fn rejected_publication_preserves_destination_and_retry_sequence() {
        let mut owner = fixture();
        let mut io = io_fixture {
            failure: failure::publication,
            ..Default::default()
        };
        let mut scratch = [0xcc; maximum_read_bytes];
        assert_eq!(
            owner.read(request(1, 0, 8), &mut scratch, &mut io),
            read_reply::rejected(program_status::bad_address)
        );
        assert_eq!(io.events, ["preflight", "read", "publish"]);
        assert_eq!(io.destination, [0xa5; maximum_read_bytes]);
        assert_eq!(scratch, [0; maximum_read_bytes]);
        assert_eq!((owner.reads(), owner.bytes_read()), (0, 0));
        let mut io = io_fixture::default();
        assert_eq!(
            owner.read(request(1, 0, 8), &mut scratch, &mut io).status,
            program_status::ok
        );
        assert_eq!((owner.reads(), owner.bytes_read()), (1, 8));
    }

    #[test]
    fn exact_arena_end_and_last_sequence_work_without_counter_wrap() {
        let mut owner = fixture();
        owner.next_sequence = u64::MAX - 1;
        let mut value = request(u64::MAX - 1, 0, maximum_read_bytes as u64);
        value.address = base + minimum_arena_bytes - maximum_read_bytes as u64;
        assert_eq!(
            owner
                .read(
                    value,
                    &mut [0; maximum_read_bytes],
                    &mut io_fixture::default()
                )
                .status,
            program_status::ok
        );
        rejected(&mut owner, request(u64::MAX, 0, 1), program_status::invalid);
        owner.next_sequence = 1;
        owner.bytes_read = u64::MAX;
        rejected(&mut owner, request(1, 0, 1), program_status::invalid);
    }

    #[test]
    fn close_revokes_numeric_handle_and_remains_terminal() {
        let mut owner = fixture();
        owner.close();
        assert!(owner.is_closed());
        assert_eq!(owner.handle, None);
        rejected(&mut owner, request(1, 0, 8), program_status::closed);
        owner.close();
        rejected(
            &mut owner,
            request(1, edition_length, 8),
            program_status::closed,
        );
    }
}
