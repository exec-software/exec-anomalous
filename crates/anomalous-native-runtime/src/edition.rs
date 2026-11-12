use crate::program_startup::{granted_handle, maximum_edition_bytes, service};

/// Maximum destination capacity for one experimental retained-edition read.
pub const maximum_read_bytes: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum edition_error {
    family,
    edition,
    length,
    offset,
    exhausted,
    transport,
}

/// The actual register reply, retained so a caller checks both committed fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct edition_receipt {
    pub sequence: u64,
    pub length: u64,
}

/// Transport for one already granted immutable edition, without path lookup.
///
/// A successful call publishes exactly the returned bytes before its receipt.
/// A refused call leaves the destination unchanged. A transport cannot report
/// a retryable error after uncertain or partial publication. The kernel checks
/// authority and writable ownership independently of this client-side wrapper.
pub trait edition_transport {
    fn read_at(
        &mut self,
        handle: u64,
        offset: u64,
        destination: &mut [u8],
        sequence: u64,
    ) -> Result<edition_receipt, edition_error>;
}

pub struct edition_reader<transport: edition_transport> {
    handle: granted_handle,
    edition_length: u64,
    transport: transport,
    next_sequence: u64,
    bytes_read: u64,
}

impl<transport: edition_transport> edition_reader<transport> {
    pub fn new(
        handle: granted_handle,
        edition_length: u64,
        transport: transport,
    ) -> Result<Self, edition_error> {
        if handle.family() != service::file_read {
            return Err(edition_error::family);
        }
        if edition_length == 0 || edition_length > maximum_edition_bytes {
            return Err(edition_error::edition);
        }
        Ok(Self {
            handle,
            edition_length,
            transport,
            next_sequence: 1,
            bytes_read: 0,
        })
    }

    pub const fn reads(&self) -> u64 {
        self.next_sequence - 1
    }

    pub const fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Read a bounded range without maintaining an implicit source offset.
    /// EOF still calls the service and consumes a sequence on exact success.
    /// Only EOF may shorten a successful read; bytes beyond that length are
    /// untouched. An invalid receipt is a transport failure, not short success.
    pub fn read_at(&mut self, offset: u64, destination: &mut [u8]) -> Result<usize, edition_error> {
        if destination.is_empty() || destination.len() > maximum_read_bytes {
            return Err(edition_error::length);
        }
        let capacity = destination.len() as u64;
        if offset.checked_add(capacity).is_none() {
            return Err(edition_error::offset);
        }
        if self.next_sequence == u64::MAX {
            return Err(edition_error::exhausted);
        }
        let expected = capacity.min(self.edition_length.saturating_sub(offset));
        let next_bytes = self
            .bytes_read
            .checked_add(expected)
            .ok_or(edition_error::exhausted)?;
        let reply =
            self.transport
                .read_at(self.handle.raw(), offset, destination, self.next_sequence)?;
        if reply.sequence != self.next_sequence || reply.length != expected {
            return Err(edition_error::transport);
        }
        self.next_sequence += 1;
        self.bytes_read = next_bytes;
        Ok(expected as usize)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::{minimum_arena_bytes, program_descriptor};
    use std::vec::Vec;

    fn descriptor() -> program_descriptor {
        program_descriptor::try_new(
            0x804000,
            minimum_arena_bytes,
            0x41,
            4097,
            0x62,
            service::file_read | service::console_write,
            b"",
        )
        .unwrap()
    }

    #[derive(Default)]
    struct recorder {
        calls: Vec<(u64, u64, usize, u64)>,
        reply: Option<edition_receipt>,
        failed: bool,
    }

    impl edition_transport for recorder {
        fn read_at(
            &mut self,
            handle: u64,
            offset: u64,
            destination: &mut [u8],
            sequence: u64,
        ) -> Result<edition_receipt, edition_error> {
            self.calls
                .push((handle, offset, destination.len(), sequence));
            if self.failed {
                return Err(edition_error::transport);
            }
            let length = (destination.len() as u64).min(4097u64.saturating_sub(offset));
            for (index, byte) in destination[..length as usize].iter_mut().enumerate() {
                *byte = (offset + index as u64) as u8;
            }
            Ok(self.reply.unwrap_or(edition_receipt { sequence, length }))
        }
    }

    fn reader() -> edition_reader<recorder> {
        edition_reader::new(descriptor().edition().unwrap(), 4097, recorder::default()).unwrap()
    }

    #[test]
    fn only_a_file_grant_and_bounded_nonempty_edition_construct_a_reader() {
        assert!(matches!(
            edition_reader::new(descriptor().console().unwrap(), 4097, recorder::default()),
            Err(edition_error::family)
        ));
        for length in [0, maximum_edition_bytes + 1, u64::MAX] {
            assert!(matches!(
                edition_reader::new(descriptor().edition().unwrap(), length, recorder::default()),
                Err(edition_error::edition)
            ));
        }
    }

    #[test]
    fn exact_reads_tail_and_eof_validate_real_receipts_and_preserve_unused_bytes() {
        let mut reader = reader();
        let mut bytes = [0xa5; maximum_read_bytes];
        assert_eq!(reader.read_at(0, &mut bytes), Ok(4096));
        for (index, byte) in bytes.iter().enumerate() {
            assert_eq!(*byte, index as u8);
        }
        bytes.fill(0xa5);
        assert_eq!(reader.read_at(4096, &mut bytes), Ok(1));
        assert_eq!(bytes[0], 0);
        assert!(bytes[1..].iter().all(|byte| *byte == 0xa5));
        bytes.fill(0xa5);
        for offset in [4097, 8192] {
            assert_eq!(reader.read_at(offset, &mut bytes), Ok(0));
            assert!(bytes.iter().all(|byte| *byte == 0xa5));
        }
        assert_eq!((reader.reads(), reader.bytes_read()), (4, 4097));
        assert_eq!(reader.transport.calls.len(), 4);
        for (index, call) in reader.transport.calls.iter().enumerate() {
            assert_eq!((call.0, call.2, call.3), (0x41, 4096, index as u64 + 1));
        }
    }

    #[test]
    fn malformed_receipts_and_transport_failures_do_not_advance_accounting() {
        let mut reader = reader();
        let mut bytes = [0xa5; 8];
        for reply in [
            edition_receipt {
                sequence: 0,
                length: 8,
            },
            edition_receipt {
                sequence: 2,
                length: 8,
            },
            edition_receipt {
                sequence: 1,
                length: 7,
            },
            edition_receipt {
                sequence: 1,
                length: 9,
            },
            edition_receipt {
                sequence: 1,
                length: u64::MAX,
            },
        ] {
            reader.transport.reply = Some(reply);
            assert_eq!(reader.read_at(0, &mut bytes), Err(edition_error::transport));
            assert_eq!((reader.reads(), reader.bytes_read()), (0, 0));
        }
        reader.transport.reply = None;
        reader.transport.failed = true;
        bytes.fill(0xa5);
        assert_eq!(reader.read_at(0, &mut bytes), Err(edition_error::transport));
        assert_eq!(bytes, [0xa5; 8]);
        assert_eq!((reader.reads(), reader.bytes_read()), (0, 0));
        reader.transport.failed = false;
        assert_eq!(reader.read_at(0, &mut bytes), Ok(8));
        assert_eq!((reader.reads(), reader.bytes_read()), (1, 8));
    }

    #[test]
    fn local_bounds_and_exhaustion_never_enter_the_transport() {
        let mut reader = reader();
        assert_eq!(reader.read_at(0, &mut []), Err(edition_error::length));
        assert_eq!(
            reader.read_at(0, &mut [0; maximum_read_bytes + 1]),
            Err(edition_error::length)
        );
        assert_eq!(
            reader.read_at(u64::MAX, &mut [0]),
            Err(edition_error::offset)
        );
        reader.next_sequence = u64::MAX;
        assert_eq!(reader.read_at(0, &mut [0]), Err(edition_error::exhausted));
        reader.next_sequence = 1;
        reader.bytes_read = u64::MAX;
        assert_eq!(reader.read_at(0, &mut [0]), Err(edition_error::exhausted));
        assert!(reader.transport.calls.is_empty());
    }
}
