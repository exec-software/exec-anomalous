//! Feature-private Linux syscall-number adapter for two Anomalous record devices.
//! The parent dispatcher authenticates the exact live Linux producer and owns
//! the IF-clear exclusion interval. This is neither pipe behavior nor a general
//! Linux VFS: descriptors 3/4 are fixed, nonblocking, task-bound record routes.

#[cfg(not(test))]
use super::*;
#[cfg(test)]
use crate::application_channel::{
    application_channel_error as channel_error, application_channel_phase as channel_phase,
    application_packet_capacity,
};

const bad_fd: i64 = -9;
const again: i64 = -11;
const bad_address: i64 = -14;
#[cfg(not(test))]
const no_device: i64 = -19;
const invalid_argument: i64 = -22;
#[cfg(not(test))]
const overflow: i64 = -75;
const message_size: i64 = -90;
const poll_capacity: usize = 8;
const poll_entry_size: usize = 8;
const poll_input: u16 = 1;
const poll_output: u16 = 4;
const poll_error: u16 = 8;
const poll_hangup: u16 = 16;
const poll_invalid: u16 = 32;

#[cfg(not(test))]
impl runtime {
    /// RAX is the normalized syscall number; arguments use Linux x86-64
    /// RDI/RSI/RDX. Only successful transport counters change here. The parent
    /// accounts negative replies once and handles yield, exit and unknown calls.
    pub(super) fn linux_io(&mut self, value: scheduler_frame) -> Option<i64> {
        match value.accumulator {
            0 => Some(self.linux_read(value)),
            1 => Some(self.linux_write(value)),
            7 => Some(self.linux_poll(value)),
            _ => None,
        }
    }

    fn linux_write(&mut self, value: scheduler_frame) -> i64 {
        if value.destination_index as u32 != 3 {
            return bad_fd;
        }
        let length = match record_length(value.data) {
            Ok(0) => return 0,
            Ok(length) => length,
            Err(error) => return error,
        };
        match self.channels[0].phase() {
            channel_phase::closed => return no_device,
            channel_phase::queued | channel_phase::delivered => return again,
            channel_phase::empty => {}
        }
        let Some(sequence) = self.channels[0].next_sequence() else {
            return overflow;
        };
        let (status, _, copied) =
            self.packet_operation(1, 0, value.source_index, length as u64, sequence);
        match status {
            0 => copied as i64,
            1 => again,
            2 => bad_fd,
            3 => bad_address,
            5 => no_device,
            _ => invalid_argument,
        }
    }

    fn linux_read(&mut self, value: scheduler_frame) -> i64 {
        if value.destination_index as u32 != 4 {
            return bad_fd;
        }
        let length = match record_length(value.data) {
            Ok(0) => return 0,
            Ok(length) => length,
            Err(error) => return error,
        };
        if self.channels[1].phase() == channel_phase::closed {
            return 0;
        }
        let binding = self.channels[1].binding();
        let caller = self.identities[self.selected];
        let prepared = match self.channels[1].prepare_delivery(binding, caller, length) {
            Ok(prepared) => prepared,
            Err(error) => return read_error(error),
        };
        let result = self.channels[1].consume_with(prepared, |bytes| {
            self.execution.copy_to_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                caller,
                value.source_index,
                bytes,
            )
        });
        match result {
            Ok(receipt) => {
                self.last_ack[1] = receipt.sequence();
                self.evidence.deliveries[1] += 1;
                self.evidence.acknowledgements[1] += 1;
                receipt.length() as i64
            }
            Err(error) => read_error(error),
        }
    }

    fn linux_poll(&mut self, value: scheduler_frame) -> i64 {
        let count = match poll_count(value.source_index, value.data) {
            Ok(0) => return 0,
            Ok(count) => count,
            Err(error) => return error,
        };
        let caller = self.identities[self.selected];
        let mut scratch = [0; poll_capacity * poll_entry_size];
        let entries = &mut scratch[..count * poll_entry_size];
        if self
            .execution
            .copy_from_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                caller,
                value.destination_index,
                entries,
            )
            .is_err()
        {
            return bad_address;
        }
        let ready = poll_results(
            entries,
            [self.channels[0].phase(), self.channels[1].phase()],
            self.channels[0].next_sequence().is_some(),
        );
        // copy_to_user preflights the complete array before any output byte.
        // Reusing the input snapshot also preserves every fd/events field.
        let copied = self.execution.copy_to_user(
            &self.tasks,
            &self.circles,
            &self.scheduler,
            caller,
            value.destination_index,
            entries,
        );
        scratch.fill(0);
        if copied.is_err() {
            bad_address
        } else {
            ready as i64
        }
    }
}

fn record_length(length: u64) -> Result<usize, i64> {
    if length > application_packet_capacity as u64 {
        Err(message_size)
    } else {
        Ok(length as usize)
    }
}

fn poll_count(count: u64, timeout: u64) -> Result<usize, i64> {
    // Linux's syscall signature uses unsigned int nfds and signed int timeout.
    let count = count as u32 as usize;
    if count > poll_capacity || timeout as u32 as i32 != 0 {
        Err(invalid_argument)
    } else {
        Ok(count)
    }
}

fn read_error(error: channel_error) -> i64 {
    match error {
        channel_error::wrong_binding
        | channel_error::wrong_producer
        | channel_error::wrong_service => bad_fd,
        channel_error::empty | channel_error::already_delivered => again,
        channel_error::insufficient_capacity | channel_error::invalid_length => message_size,
        channel_error::copy_failed => bad_address,
        channel_error::closed => 0,
        _ => invalid_argument,
    }
}

fn poll_results(bytes: &mut [u8], phases: [channel_phase; 2], can_submit: bool) -> usize {
    let mut ready = 0;
    for entry in bytes.chunks_exact_mut(poll_entry_size) {
        let fd = i32::from_le_bytes(entry[..4].try_into().unwrap());
        let events = u16::from_le_bytes(entry[4..6].try_into().unwrap());
        let result = poll_flags(fd, events, phases, can_submit);
        entry[6..8].copy_from_slice(&result.to_le_bytes());
        ready += usize::from(result != 0);
    }
    ready
}

fn poll_flags(fd: i32, events: u16, phases: [channel_phase; 2], can_submit: bool) -> u16 {
    if fd < 0 {
        return 0;
    }
    match fd {
        3 => match phases[0] {
            channel_phase::closed => poll_error | poll_hangup,
            channel_phase::empty if !can_submit => poll_error,
            channel_phase::empty => events & poll_output,
            channel_phase::queued | channel_phase::delivered => 0,
        },
        4 => match phases[1] {
            channel_phase::closed => poll_hangup,
            channel_phase::queued => events & poll_input,
            channel_phase::empty | channel_phase::delivered => 0,
        },
        _ => poll_invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_distinguishes_queued_delivered_closed_and_exhausted_records() {
        for phase in [
            channel_phase::empty,
            channel_phase::queued,
            channel_phase::delivered,
            channel_phase::closed,
        ] {
            let phases = [phase; 2];
            let output = match phase {
                channel_phase::empty => poll_output,
                channel_phase::closed => poll_error | poll_hangup,
                _ => 0,
            };
            let input = match phase {
                channel_phase::queued => poll_input,
                channel_phase::closed => poll_hangup,
                _ => 0,
            };
            assert_eq!(poll_flags(3, u16::MAX, phases, true), output);
            assert_eq!(poll_flags(4, u16::MAX, phases, true), input);
            assert_eq!(poll_flags(3, 0, phases, true), output & !poll_output);
            assert_eq!(poll_flags(4, 0, phases, true), input & !poll_input);
            assert_eq!(poll_flags(-1, u16::MAX, phases, true), 0);
            assert_eq!(poll_flags(99, 0, phases, true), poll_invalid);
        }
        assert_eq!(
            poll_flags(3, 0, [channel_phase::empty; 2], false),
            poll_error
        );
        assert_eq!(
            poll_flags(3, poll_input, [channel_phase::empty; 2], true),
            0
        );
        assert_eq!(
            poll_flags(4, poll_output, [channel_phase::queued; 2], true),
            0
        );
    }

    #[test]
    fn poll_snapshots_preserve_inputs_replace_all_outputs_and_count_duplicate_entries() {
        let mut bytes = [0; 4 * poll_entry_size];
        for (entry, (fd, events)) in bytes.chunks_exact_mut(poll_entry_size).zip([
            (3i32, 4u16),
            (3, 4),
            (-1, u16::MAX),
            (99, 0),
        ]) {
            entry[..4].copy_from_slice(&fd.to_le_bytes());
            entry[4..6].copy_from_slice(&events.to_le_bytes());
            entry[6..8].copy_from_slice(&0x55aau16.to_le_bytes());
        }
        let original = bytes;
        assert_eq!(poll_results(&mut bytes, [channel_phase::empty; 2], true), 3);
        for ((entry, before), expected) in bytes
            .chunks_exact(poll_entry_size)
            .zip(original.chunks_exact(poll_entry_size))
            .zip([4u16, 4, 0, 32])
        {
            assert_eq!(&entry[..6], &before[..6]);
            assert_eq!(&entry[6..8], &expected.to_le_bytes());
        }
    }

    #[test]
    fn bounded_arguments_keep_linux_integer_widths_and_record_error_distinctions() {
        assert_eq!(record_length(0), Ok(0));
        assert_eq!(record_length(4096), Ok(4096));
        for length in [4097, u64::MAX] {
            assert_eq!(record_length(length), Err(message_size));
        }
        assert_eq!(poll_count(0, 0), Ok(0));
        assert_eq!(poll_count(8, 0), Ok(8));
        assert_eq!(poll_count((1 << 32) | 2, 1 << 32), Ok(2));
        for (count, timeout) in [(9, 0), (u64::MAX, 0), (0, 1), (0, u64::MAX)] {
            assert_eq!(poll_count(count, timeout), Err(invalid_argument));
        }
        for (error, result) in [
            (channel_error::empty, again),
            (channel_error::already_delivered, again),
            (channel_error::insufficient_capacity, message_size),
            (channel_error::copy_failed, bad_address),
            (channel_error::closed, 0),
            (channel_error::wrong_service, bad_fd),
            (channel_error::stale_preparation, invalid_argument),
        ] {
            assert_eq!(read_error(error), result);
        }
    }
}
