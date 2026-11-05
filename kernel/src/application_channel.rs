#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! One bounded producer-to-service copied-packet endpoint.
//!
//! The architecture caller authenticates the live current task, validates its
//! grant, and retains serialized exclusion across preparation, checked user
//! copy, and commit. These identities are correlation, not authentication.
//! No user pointer, desktop policy, packet codec, or presentation assertion is
//! retained here. Delivery and acknowledgement prove neither display nor work
//! completion. Explicit close is required before either bound task retires.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::identity::{endpoint_id, task_identity};

pub(crate) const application_packet_capacity: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum application_channel_error {
    same_task,
    owner_exhausted,
    wrong_binding,
    wrong_producer,
    wrong_service,
    closed,
    backpressure,
    empty,
    already_delivered,
    not_delivered,
    sequence_mismatch,
    sequence_exhausted,
    invalid_length,
    insufficient_capacity,
    stale_preparation,
    copy_failed,
}

impl application_channel_error {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::same_task => "same_task",
            Self::owner_exhausted => "owner_exhausted",
            Self::wrong_binding => "wrong_binding",
            Self::wrong_producer => "wrong_producer",
            Self::wrong_service => "wrong_service",
            Self::closed => "closed",
            Self::backpressure => "backpressure",
            Self::empty => "empty",
            Self::already_delivered => "already_delivered",
            Self::not_delivered => "not_delivered",
            Self::sequence_mismatch => "sequence_mismatch",
            Self::sequence_exhausted => "sequence_exhausted",
            Self::invalid_length => "invalid_length",
            Self::insufficient_capacity => "insufficient_capacity",
            Self::stale_preparation => "stale_preparation",
            Self::copy_failed => "copy_failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct application_channel_binding {
    endpoint: endpoint_id,
    producer: task_identity,
    service: task_identity,
}

impl application_channel_binding {
    pub(crate) fn try_new(
        endpoint: endpoint_id,
        producer: task_identity,
        service: task_identity,
    ) -> Result<Self, application_channel_error> {
        if producer.task() == service.task() {
            return Err(application_channel_error::same_task);
        }
        Ok(Self {
            endpoint,
            producer,
            service,
        })
    }

    pub(crate) const fn endpoint(self) -> endpoint_id {
        self.endpoint
    }
    pub(crate) const fn producer(self) -> task_identity {
        self.producer
    }
    pub(crate) const fn service(self) -> task_identity {
        self.service
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum application_channel_phase {
    empty,
    queued,
    delivered,
    closed,
}

#[must_use = "copy then commit, or discard without changing the endpoint"]
pub(crate) struct application_prepared_submission {
    owner: u64,
    sequence: u64,
    length: usize,
}

#[must_use = "deliver the exact packet, or discard without changing the endpoint"]
pub(crate) struct application_prepared_delivery {
    owner: u64,
    sequence: u64,
    length: usize,
}

/// Correlation for one completed channel operation, never a presentation receipt.
pub(crate) struct application_channel_receipt {
    sequence: u64,
    length: usize,
}

impl application_channel_receipt {
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub(crate) const fn length(&self) -> usize {
        self.length
    }
}

pub(crate) struct application_channel_close_receipt {
    first_close: bool,
    discarded: bool,
}

impl application_channel_close_receipt {
    pub(crate) const fn first_close(&self) -> bool {
        self.first_close
    }
    pub(crate) const fn discarded(&self) -> bool {
        self.discarded
    }
}

// A private process-local brand prevents a prepared token from crossing owners
// even if a trusted constructor mistakenly repeats the same public binding.
static next_owner: AtomicU64 = AtomicU64::new(1);

fn mint_owner(counter: &AtomicU64) -> Result<u64, application_channel_error> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            if current == 0 {
                None
            } else {
                current.checked_add(1)
            }
        })
        .map_err(|_| application_channel_error::owner_exhausted)
}

#[must_use = "close before task retirement and retain the owner through cleanup"]
pub(crate) struct application_channel {
    binding: application_channel_binding,
    owner: u64,
    phase: application_channel_phase,
    next_sequence: u64,
    pending_sequence: u64,
    length: usize,
    bytes: [u8; application_packet_capacity],
}

impl application_channel {
    /// Trusted construction only. The route controller owns endpoint-generation
    /// allocation and must issue a new generation when rebinding a retired route.
    pub(crate) fn try_new(
        binding: application_channel_binding,
    ) -> Result<Self, application_channel_error> {
        Ok(Self {
            binding,
            owner: mint_owner(&next_owner)?,
            phase: application_channel_phase::empty,
            next_sequence: 1,
            pending_sequence: 0,
            length: 0,
            bytes: [0; application_packet_capacity],
        })
    }

    pub(crate) const fn binding(&self) -> application_channel_binding {
        self.binding
    }
    pub(crate) const fn phase(&self) -> application_channel_phase {
        self.phase
    }

    /// Zero is retained internally only as a permanently exhausted cursor.
    pub(crate) const fn next_sequence(&self) -> Option<u64> {
        if self.next_sequence == 0 || matches!(self.phase, application_channel_phase::closed) {
            None
        } else {
            Some(self.next_sequence)
        }
    }

    /// Logical clearing evidence, not a physical or compiler-resistant erasure claim.
    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) const fn queued_length(&self) -> usize {
        self.length
    }

    /// Feature-private observation of every payload byte. This is evidence for
    /// caller-isolation checks, never a userspace capability or content handle.
    #[cfg(feature = "shared_veil_test")]
    pub(crate) fn proof_storage_hash(&self) -> u64 {
        self.bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
        })
    }

    pub(crate) fn storage_is_clear(&self) -> bool {
        self.length == 0 && self.pending_sequence == 0 && self.bytes.iter().all(|byte| *byte == 0)
    }

    pub(crate) fn prepare_submit(
        &self,
        binding: application_channel_binding,
        caller: task_identity,
        sequence: u64,
        length: usize,
    ) -> Result<application_prepared_submission, application_channel_error> {
        self.check_binding(binding)?;
        if caller != self.binding.producer {
            return Err(application_channel_error::wrong_producer);
        }
        self.require_open()?;
        if self.phase != application_channel_phase::empty {
            return Err(application_channel_error::backpressure);
        }
        if self.next_sequence == 0 {
            return Err(application_channel_error::sequence_exhausted);
        }
        if sequence != self.next_sequence {
            return Err(application_channel_error::sequence_mismatch);
        }
        if !(1..=application_packet_capacity).contains(&length) {
            return Err(application_channel_error::invalid_length);
        }
        Ok(application_prepared_submission {
            owner: self.owner,
            sequence,
            length,
        })
    }

    /// The caller supplies already copied trusted bytes, never a live user
    /// slice. A failed user copy must discard the token and clear its scratch.
    /// Every returned rejection leaves this endpoint and the source unchanged.
    pub(crate) fn commit_submit(
        &mut self,
        prepared: application_prepared_submission,
        packet: &[u8],
    ) -> Result<application_channel_receipt, application_channel_error> {
        if prepared.owner != self.owner
            || self.phase != application_channel_phase::empty
            || self.next_sequence != prepared.sequence
            || self.next_sequence == 0
        {
            return Err(application_channel_error::stale_preparation);
        }
        if packet.len() != prepared.length {
            return Err(application_channel_error::invalid_length);
        }
        self.bytes[..packet.len()].copy_from_slice(packet);
        self.length = packet.len();
        self.pending_sequence = prepared.sequence;
        self.next_sequence = prepared.sequence.checked_add(1).unwrap_or(0);
        self.phase = application_channel_phase::queued;
        Ok(application_channel_receipt {
            sequence: prepared.sequence,
            length: prepared.length,
        })
    }

    pub(crate) fn prepare_delivery(
        &self,
        binding: application_channel_binding,
        caller: task_identity,
        capacity: usize,
    ) -> Result<application_prepared_delivery, application_channel_error> {
        self.check_binding(binding)?;
        if caller != self.binding.service {
            return Err(application_channel_error::wrong_service);
        }
        self.require_open()?;
        match self.phase {
            application_channel_phase::empty => return Err(application_channel_error::empty),
            application_channel_phase::delivered => {
                return Err(application_channel_error::already_delivered);
            }
            application_channel_phase::queued => {}
            application_channel_phase::closed => unreachable!(),
        }
        if capacity < self.length {
            return Err(application_channel_error::insufficient_capacity);
        }
        Ok(application_prepared_delivery {
            owner: self.owner,
            sequence: self.pending_sequence,
            length: self.length,
        })
    }

    /// Copy only the immutable packet. The trusted callback must preflight its
    /// complete destination before writing; this owner cannot undo writes made
    /// by a faulty callback. Any returned copy error preserves all endpoint
    /// bytes and metadata, allowing a fresh preparation to retry delivery.
    pub(crate) fn deliver_with<E>(
        &mut self,
        prepared: application_prepared_delivery,
        copy: impl FnOnce(&[u8]) -> Result<(), E>,
    ) -> Result<application_channel_receipt, application_channel_error> {
        if prepared.owner != self.owner
            || self.phase != application_channel_phase::queued
            || prepared.sequence != self.pending_sequence
            || prepared.length != self.length
        {
            return Err(application_channel_error::stale_preparation);
        }
        copy(&self.bytes[..self.length]).map_err(|_| application_channel_error::copy_failed)?;
        self.phase = application_channel_phase::delivered;
        Ok(application_channel_receipt {
            sequence: prepared.sequence,
            length: prepared.length,
        })
    }

    /// Copy and release one queued record under the same exclusive owner borrow.
    /// The trusted callback has the same complete-destination preflight contract
    /// as `deliver_with`: an error preserves the queue, but this owner cannot
    /// undo partial writes made by an incorrect callback. Once the callback
    /// succeeds, clearing the packet and releasing backpressure cannot fail.
    /// No separately fallible acknowledgement follows visible output, and no
    /// caller can observe or act on the intermediate delivered phase.
    pub(crate) fn consume_with<E>(
        &mut self,
        prepared: application_prepared_delivery,
        copy: impl FnOnce(&[u8]) -> Result<(), E>,
    ) -> Result<application_channel_receipt, application_channel_error> {
        let receipt = self.deliver_with(prepared, copy)?;
        self.clear();
        self.phase = application_channel_phase::empty;
        Ok(receipt)
    }

    /// Only the bound service may release one exactly delivered sequence. A
    /// user-visible outcome, policy decision, or presentation result is carried
    /// separately; this operation merely releases transport backpressure.
    pub(crate) fn acknowledge(
        &mut self,
        binding: application_channel_binding,
        caller: task_identity,
        sequence: u64,
    ) -> Result<application_channel_receipt, application_channel_error> {
        self.check_binding(binding)?;
        if caller != self.binding.service {
            return Err(application_channel_error::wrong_service);
        }
        self.require_open()?;
        if self.phase != application_channel_phase::delivered {
            return Err(application_channel_error::not_delivered);
        }
        if sequence != self.pending_sequence {
            return Err(application_channel_error::sequence_mismatch);
        }
        let receipt = application_channel_receipt {
            sequence,
            length: self.length,
        };
        self.clear();
        self.phase = application_channel_phase::empty;
        Ok(receipt)
    }

    /// Trusted route-controller retirement, not an unauthenticated client call.
    /// Closing invalidates all prepared tokens, including an undelivered
    /// submission. It cannot erase a copy already owned by the service.
    pub(crate) fn close(
        &mut self,
        binding: application_channel_binding,
    ) -> Result<application_channel_close_receipt, application_channel_error> {
        self.check_binding(binding)?;
        let first_close = self.phase != application_channel_phase::closed;
        let discarded = self.length != 0;
        self.phase = application_channel_phase::closed;
        self.clear();
        Ok(application_channel_close_receipt {
            first_close,
            discarded,
        })
    }

    fn check_binding(
        &self,
        binding: application_channel_binding,
    ) -> Result<(), application_channel_error> {
        if binding != self.binding {
            Err(application_channel_error::wrong_binding)
        } else {
            Ok(())
        }
    }

    fn require_open(&self) -> Result<(), application_channel_error> {
        if self.phase == application_channel_phase::closed {
            Err(application_channel_error::closed)
        } else {
            Ok(())
        }
    }

    fn clear(&mut self) {
        self.bytes.fill(0);
        self.length = 0;
        self.pending_sequence = 0;
    }
}

impl Drop for application_channel {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{circle_id, task_id};

    fn task(slot: usize, generation: u64) -> task_identity {
        task_identity::new(
            task_id::try_from_parts(slot, generation).unwrap(),
            circle_id::try_from_parts(slot + 10, generation).unwrap(),
        )
    }

    fn binding() -> application_channel_binding {
        application_channel_binding::try_new(
            endpoint_id::try_from_parts(3, 9).unwrap(),
            task(1, 4),
            task(2, 7),
        )
        .unwrap()
    }

    fn channel() -> application_channel {
        application_channel::try_new(binding()).unwrap()
    }

    fn submit(channel: &mut application_channel, bytes: &[u8]) {
        let bound = channel.binding();
        let token = channel
            .prepare_submit(
                bound,
                bound.producer(),
                channel.next_sequence().unwrap(),
                bytes.len(),
            )
            .unwrap();
        channel.commit_submit(token, bytes).unwrap();
    }

    fn deliver(channel: &mut application_channel) {
        let bound = channel.binding();
        let token = channel
            .prepare_delivery(bound, bound.service(), application_packet_capacity)
            .unwrap();
        channel.deliver_with(token, |_| Ok::<(), ()>(())).unwrap();
    }

    fn snapshot(
        channel: &application_channel,
    ) -> (
        application_channel_phase,
        u64,
        u64,
        usize,
        [u8; application_packet_capacity],
    ) {
        (
            channel.phase,
            channel.next_sequence,
            channel.pending_sequence,
            channel.length,
            channel.bytes,
        )
    }

    #[test]
    fn complete_packets_remain_isolated_until_exact_acknowledgement() {
        let mut channel = channel();
        let bound = channel.binding();
        for length in [application_packet_capacity, 1] {
            let sequence = channel.next_sequence().unwrap();
            let mut source = [0x5a; application_packet_capacity];
            let token = channel
                .prepare_submit(bound, bound.producer(), sequence, length)
                .unwrap();
            let submitted = channel.commit_submit(token, &source[..length]).unwrap();
            assert_eq!(
                (submitted.sequence(), submitted.length()),
                (sequence, length)
            );
            source.fill(0xa5);
            let token = channel
                .prepare_delivery(bound, bound.service(), length)
                .unwrap();
            let receipt = channel
                .deliver_with(token, |bytes| {
                    assert_eq!(bytes, &[0x5a; application_packet_capacity][..length]);
                    Ok::<(), ()>(())
                })
                .unwrap();
            assert_eq!((receipt.sequence(), receipt.length()), (sequence, length));
            assert_eq!(channel.phase(), application_channel_phase::delivered);
            assert!(!channel.storage_is_clear());
            let unchanged = snapshot(&channel);
            assert_eq!(
                channel
                    .acknowledge(bound, bound.service(), sequence + 1)
                    .err(),
                Some(application_channel_error::sequence_mismatch)
            );
            assert_eq!(snapshot(&channel), unchanged);
            channel
                .acknowledge(bound, bound.service(), sequence)
                .unwrap();
            assert!(channel.storage_is_clear());
            assert_eq!(channel.phase(), application_channel_phase::empty);
            assert_eq!(channel.next_sequence(), Some(sequence + 1));
            assert_eq!(
                channel.acknowledge(bound, bound.service(), sequence).err(),
                Some(application_channel_error::not_delivered)
            );
        }
    }

    #[test]
    fn stale_bindings_and_wrong_roles_cannot_observe_or_mutate_packets() {
        let mut channel = channel();
        let bound = channel.binding();
        submit(&mut channel, b"private packet");
        let original = snapshot(&channel);
        let stale = [
            application_channel_binding {
                endpoint: endpoint_id::try_from_parts(3, 10).unwrap(),
                ..bound
            },
            application_channel_binding {
                producer: task(1, 5),
                ..bound
            },
            application_channel_binding {
                service: task(2, 8),
                ..bound
            },
            application_channel_binding {
                producer: task_identity::new(
                    bound.producer().task(),
                    circle_id::try_from_parts(11, 5).unwrap(),
                ),
                ..bound
            },
        ];
        for binding in stale {
            assert_eq!(
                channel
                    .prepare_submit(binding, bound.producer(), 2, 1)
                    .err(),
                Some(application_channel_error::wrong_binding)
            );
            assert_eq!(
                channel
                    .prepare_delivery(binding, bound.service(), 4096)
                    .err(),
                Some(application_channel_error::wrong_binding)
            );
            assert_eq!(
                channel.acknowledge(binding, bound.service(), 1).err(),
                Some(application_channel_error::wrong_binding)
            );
            assert_eq!(
                channel.close(binding).err(),
                Some(application_channel_error::wrong_binding)
            );
            assert_eq!(snapshot(&channel), original);
        }
        assert_eq!(
            channel.prepare_submit(bound, bound.service(), 2, 1).err(),
            Some(application_channel_error::wrong_producer)
        );
        assert_eq!(
            channel
                .prepare_delivery(bound, bound.producer(), 4096)
                .err(),
            Some(application_channel_error::wrong_service)
        );
        assert_eq!(
            channel.acknowledge(bound, bound.producer(), 1).err(),
            Some(application_channel_error::wrong_service)
        );
        assert_eq!(snapshot(&channel), original);
    }

    #[test]
    fn bounds_copy_failure_and_backpressure_preserve_state() {
        let mut channel = channel();
        let bound = channel.binding();
        let empty = snapshot(&channel);
        for length in [0, 4097, usize::MAX] {
            assert_eq!(
                channel
                    .prepare_submit(bound, bound.producer(), 1, length)
                    .err(),
                Some(application_channel_error::invalid_length)
            );
            assert_eq!(snapshot(&channel), empty);
        }
        for sequence in [0, 2, u64::MAX] {
            assert_eq!(
                channel
                    .prepare_submit(bound, bound.producer(), sequence, 1)
                    .err(),
                Some(application_channel_error::sequence_mismatch)
            );
        }
        let token = channel
            .prepare_submit(bound, bound.producer(), 1, 4)
            .unwrap();
        assert_eq!(
            channel.commit_submit(token, b"wrong length").err(),
            Some(application_channel_error::invalid_length)
        );
        assert_eq!(snapshot(&channel), empty);
        // A failed submit-side copy discards its preparation and never commits.
        {
            let _discarded = channel
                .prepare_submit(bound, bound.producer(), 1, 4)
                .unwrap();
        }
        assert_eq!(snapshot(&channel), empty);
        submit(&mut channel, b"four");
        let queued = snapshot(&channel);
        assert_eq!(
            channel.prepare_delivery(bound, bound.service(), 3).err(),
            Some(application_channel_error::insufficient_capacity)
        );
        assert_eq!(
            channel.prepare_submit(bound, bound.producer(), 2, 4).err(),
            Some(application_channel_error::backpressure)
        );
        assert_eq!(
            channel.acknowledge(bound, bound.service(), 1).err(),
            Some(application_channel_error::not_delivered)
        );
        let token = channel.prepare_delivery(bound, bound.service(), 4).unwrap();
        assert_eq!(
            channel
                .deliver_with(token, |bytes| {
                    assert_eq!(bytes, b"four");
                    Err::<(), ()>(())
                })
                .err(),
            Some(application_channel_error::copy_failed)
        );
        assert_eq!(snapshot(&channel), queued);
        deliver(&mut channel);
        assert_eq!(
            channel.prepare_delivery(bound, bound.service(), 4096).err(),
            Some(application_channel_error::already_delivered)
        );
        assert_eq!(
            channel.prepare_submit(bound, bound.producer(), 2, 4).err(),
            Some(application_channel_error::backpressure)
        );
    }

    #[test]
    fn prepared_tokens_do_not_survive_competing_commits_or_cross_owners() {
        let mut channel = channel();
        let mut other = application_channel::try_new(binding()).unwrap();
        let bound = channel.binding();
        let token = channel
            .prepare_submit(bound, bound.producer(), 1, 3)
            .unwrap();
        assert_eq!(
            other.commit_submit(token, b"one").err(),
            Some(application_channel_error::stale_preparation)
        );
        assert!(other.storage_is_clear());
        let stale = channel
            .prepare_submit(bound, bound.producer(), 1, 3)
            .unwrap();
        submit(&mut channel, b"one");
        let old_delivery = channel.prepare_delivery(bound, bound.service(), 3).unwrap();
        deliver(&mut channel);
        channel.acknowledge(bound, bound.service(), 1).unwrap();
        assert_eq!(
            channel.commit_submit(stale, b"two").err(),
            Some(application_channel_error::stale_preparation)
        );
        submit(&mut channel, b"two");
        let original = snapshot(&channel);
        assert_eq!(
            channel
                .deliver_with(old_delivery, |_| -> Result<(), ()> {
                    panic!("stale copy invoked")
                })
                .err(),
            Some(application_channel_error::stale_preparation)
        );
        assert_eq!(snapshot(&channel), original);
        let other_delivery = channel.prepare_delivery(bound, bound.service(), 3).unwrap();
        submit(&mut other, b"two");
        assert_eq!(
            other
                .deliver_with(other_delivery, |_| -> Result<(), ()> {
                    panic!("foreign copy invoked")
                })
                .err(),
            Some(application_channel_error::stale_preparation)
        );
    }

    #[test]
    fn atomic_consumption_retries_failed_copy_then_releases_exact_record_once() {
        let mut channel = channel();
        let bound = channel.binding();
        for sequence in [1, 2] {
            submit(&mut channel, b"record");
            let queued = snapshot(&channel);
            let mut output = [0xa5; 6];
            let failed = channel.prepare_delivery(bound, bound.service(), 6).unwrap();
            assert_eq!(
                channel
                    .consume_with(failed, |bytes| {
                        assert_eq!(bytes, b"record");
                        // A checked-copy preflight fails before changing output.
                        Err::<(), ()>(())
                    })
                    .err(),
                Some(application_channel_error::copy_failed)
            );
            assert_eq!(snapshot(&channel), queued);
            assert_eq!(output, [0xa5; 6]);
            let stale = channel.prepare_delivery(bound, bound.service(), 6).unwrap();
            let retry = channel.prepare_delivery(bound, bound.service(), 6).unwrap();
            let receipt = channel
                .consume_with(retry, |bytes| {
                    output.copy_from_slice(bytes);
                    Ok::<(), ()>(())
                })
                .unwrap();
            assert_eq!(output, *b"record");
            assert_eq!((receipt.sequence(), receipt.length()), (sequence, 6));
            assert_eq!(channel.phase(), application_channel_phase::empty);
            assert!(channel.storage_is_clear());
            assert_eq!(channel.next_sequence(), Some(sequence + 1));
            let cleared = snapshot(&channel);
            assert_eq!(
                channel
                    .consume_with(stale, |_| -> Result<(), ()> {
                        panic!("already consumed record exposed twice")
                    })
                    .err(),
                Some(application_channel_error::stale_preparation)
            );
            assert_eq!(snapshot(&channel), cleared);
            assert_eq!(
                channel.acknowledge(bound, bound.service(), sequence).err(),
                Some(application_channel_error::not_delivered)
            );
        }
    }

    #[test]
    fn atomic_consumption_rejects_foreign_delivered_and_revoked_tokens_and_never_wraps() {
        let bound = binding();
        let mut channel = channel();
        let mut other = application_channel::try_new(bound).unwrap();
        submit(&mut channel, b"one");
        submit(&mut other, b"two");
        let foreign = channel.prepare_delivery(bound, bound.service(), 3).unwrap();
        let unchanged = snapshot(&other);
        assert_eq!(
            other
                .consume_with(foreign, |_| -> Result<(), ()> {
                    panic!("different owner exposed a record")
                })
                .err(),
            Some(application_channel_error::stale_preparation)
        );
        assert_eq!(snapshot(&other), unchanged);
        let revoked_queued = other.prepare_delivery(bound, bound.service(), 3).unwrap();
        other.close(bound).unwrap();
        let closed = snapshot(&other);
        assert_eq!(
            other
                .consume_with(revoked_queued, |_| -> Result<(), ()> {
                    panic!("revoked queued record exposed")
                })
                .err(),
            Some(application_channel_error::stale_preparation)
        );
        assert_eq!(snapshot(&other), closed);

        let delivered = channel.prepare_delivery(bound, bound.service(), 3).unwrap();
        deliver(&mut channel);
        let unchanged = snapshot(&channel);
        assert_eq!(
            channel
                .consume_with(delivered, |_| -> Result<(), ()> {
                    panic!("already delivered record consumed")
                })
                .err(),
            Some(application_channel_error::stale_preparation)
        );
        assert_eq!(snapshot(&channel), unchanged);
        channel.acknowledge(bound, bound.service(), 1).unwrap();

        channel.next_sequence = u64::MAX;
        submit(&mut channel, b"last");
        let final_packet = channel.prepare_delivery(bound, bound.service(), 4).unwrap();
        let revoked = channel.prepare_delivery(bound, bound.service(), 4).unwrap();
        let receipt = channel
            .consume_with(final_packet, |bytes| {
                assert_eq!(bytes, b"last");
                Ok::<(), ()>(())
            })
            .unwrap();
        assert_eq!(receipt.sequence(), u64::MAX);
        assert!(channel.storage_is_clear());
        assert_eq!(channel.phase(), application_channel_phase::empty);
        assert_eq!(channel.next_sequence(), None);
        channel.close(bound).unwrap();
        let closed = snapshot(&channel);
        assert_eq!(
            channel
                .consume_with(revoked, |_| -> Result<(), ()> {
                    panic!("closed owner exposed a record")
                })
                .err(),
            Some(application_channel_error::stale_preparation)
        );
        assert_eq!(snapshot(&channel), closed);
    }

    #[test]
    fn close_clears_every_phase_and_invalidates_all_pending_tokens() {
        for phase in 0..3 {
            let mut channel = channel();
            let bound = channel.binding();
            let pending_submit = channel
                .prepare_submit(bound, bound.producer(), 1, 3)
                .unwrap();
            if phase != 0 {
                submit(&mut channel, b"one");
            }
            let pending_delivery = if phase != 0 {
                Some(channel.prepare_delivery(bound, bound.service(), 3).unwrap())
            } else {
                None
            };
            if phase == 2 {
                deliver(&mut channel);
            }
            let closed = channel.close(bound).unwrap();
            assert!(closed.first_close());
            assert_eq!(closed.discarded(), phase != 0);
            assert!(channel.storage_is_clear());
            assert_eq!(channel.next_sequence(), None);
            assert_eq!(channel.phase(), application_channel_phase::closed);
            assert_eq!(
                channel.commit_submit(pending_submit, b"one").err(),
                Some(application_channel_error::stale_preparation)
            );
            if let Some(pending) = pending_delivery {
                assert_eq!(
                    channel
                        .deliver_with(pending, |_| -> Result<(), ()> {
                            panic!("revoked copy invoked")
                        })
                        .err(),
                    Some(application_channel_error::stale_preparation)
                );
            }
            assert_eq!(
                channel.prepare_submit(bound, bound.producer(), 1, 3).err(),
                Some(application_channel_error::closed)
            );
            assert_eq!(
                channel.prepare_delivery(bound, bound.service(), 3).err(),
                Some(application_channel_error::closed)
            );
            assert_eq!(
                channel.acknowledge(bound, bound.service(), 1).err(),
                Some(application_channel_error::closed)
            );
            let repeated = channel.close(bound).unwrap();
            assert!(!repeated.first_close());
            assert!(!repeated.discarded());
        }
    }

    #[test]
    fn generations_and_sequences_never_wrap_into_reused_authority() {
        let bound = binding();
        assert_eq!(
            application_channel_binding::try_new(
                bound.endpoint(),
                bound.producer(),
                bound.producer()
            ),
            Err(application_channel_error::same_task)
        );
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(mint_owner(&counter), Ok(u64::MAX - 1));
        assert_eq!(
            mint_owner(&counter),
            Err(application_channel_error::owner_exhausted)
        );
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        assert_eq!(
            mint_owner(&AtomicU64::new(0)),
            Err(application_channel_error::owner_exhausted)
        );
        let mut channel = channel();
        channel.next_sequence = u64::MAX;
        submit(&mut channel, b"last");
        deliver(&mut channel);
        channel
            .acknowledge(bound, bound.service(), u64::MAX)
            .unwrap();
        assert_eq!(channel.next_sequence(), None);
        assert!(channel.storage_is_clear());
        let original = snapshot(&channel);
        assert_eq!(
            channel.prepare_submit(bound, bound.producer(), 0, 1).err(),
            Some(application_channel_error::sequence_exhausted)
        );
        assert_eq!(snapshot(&channel), original);
    }
}
