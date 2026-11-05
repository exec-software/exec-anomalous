#![cfg_attr(not(test), allow(dead_code))]

//! Fixed copied-message IPC for the bounded control proof.
//!
//! Endpoints and queues are kernel-owned fixed tables. They are independent
//! from the interrupt event channel: pressure is explicit, sequences advance
//! only for accepted messages, and no stored value contains a user pointer.

use core::mem::size_of;
use core::ptr;
use core::sync::atomic::{Ordering, compiler_fence};

use crate::circle::{circle_child_kind, circle_error, circle_table};
use crate::identity::{endpoint_id, fixture_principal_id, next_generation, task_identity};
use crate::sigil::{control_sigil_table, sigil_error, sigil_reference, sigil_rights};
use crate::task::{synthetic_origin_metadata, task_error, task_snapshot, task_state, task_table};

pub(crate) const ipc_payload_capacity: usize = 32;
pub(crate) const ipc_queue_capacity: usize = 2;
pub(crate) const ipc_endpoint_capacity: usize = 4;

#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(test, derive(Debug))]
#[repr(C)]
pub(crate) struct ipc_payload {
    length: u8,
    bytes: [u8; ipc_payload_capacity],
}

impl ipc_payload {
    pub(crate) fn copy_from(bytes: &[u8]) -> Result<Self, ipc_error> {
        if bytes.len() > ipc_payload_capacity {
            return Err(ipc_error::payload_too_large);
        }
        let mut payload = Self {
            length: bytes.len() as u8,
            ..Self::default()
        };
        payload.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(payload)
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.length)).unwrap_or(&[])
    }

    fn is_well_formed(&self) -> bool {
        usize::from(self.length) <= ipc_payload_capacity
            && self.bytes[usize::from(self.length)..]
                .iter()
                .all(|byte| *byte == 0)
    }
}

impl Default for ipc_payload {
    fn default() -> Self {
        Self {
            length: 0,
            bytes: [0; ipc_payload_capacity],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ipc_error {
    payload_too_large,
    sender_not_running,
    receiver_not_running,
    receiver_not_live,
    invalid_endpoint,
    stale_endpoint,
    endpoint_capacity_exhausted,
    endpoint_generation_exhausted,
    receiver_mismatch,
    would_block,
    sequence_exhausted,
    copy_failed,
    corrupt_slot,
    circle_accounting_mismatch,
    task(task_error),
    circle(circle_error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ipc_authorized_error {
    sigil(sigil_error),
    ipc(ipc_error),
}

/// Attribution copied from a fresh kernel task-table lookup.
///
/// The fixture principal and origin do not authenticate a person, model,
/// machine, service, or remote peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ipc_provenance {
    task: task_identity,
    principal: fixture_principal_id,
    origin: synthetic_origin_metadata,
}

impl ipc_provenance {
    fn from_snapshot(snapshot: task_snapshot) -> Self {
        Self {
            task: snapshot.identity(),
            principal: snapshot.principal(),
            origin: snapshot.origin(),
        }
    }

    pub(crate) const fn task(self) -> task_identity {
        self.task
    }

    pub(crate) const fn principal(self) -> fixture_principal_id {
        self.principal
    }

    pub(crate) const fn origin(self) -> synthetic_origin_metadata {
        self.origin
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub(crate) struct ipc_message {
    endpoint: endpoint_id,
    sequence: u64,
    provenance: ipc_provenance,
    payload: ipc_payload,
}

impl ipc_message {
    pub(crate) const fn endpoint(&self) -> endpoint_id {
        self.endpoint
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn provenance(&self) -> ipc_provenance {
        self.provenance
    }

    pub(crate) const fn payload(&self) -> &ipc_payload {
        &self.payload
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ipc_endpoint_snapshot {
    endpoint: endpoint_id,
    receiver: task_identity,
    depth: usize,
    capacity: usize,
    next_sequence: Option<u64>,
}

impl ipc_endpoint_snapshot {
    pub(crate) const fn endpoint(self) -> endpoint_id {
        self.endpoint
    }

    pub(crate) const fn receiver(self) -> task_identity {
        self.receiver
    }

    pub(crate) const fn depth(self) -> usize {
        self.depth
    }

    pub(crate) const fn capacity(self) -> usize {
        self.capacity
    }

    pub(crate) const fn next_sequence(self) -> Option<u64> {
        self.next_sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ipc_accounting {
    pub endpoints: usize,
    pub messages: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ipc_receiver {
    identity: task_identity,
    principal: fixture_principal_id,
}

impl ipc_receiver {
    fn from_snapshot(snapshot: task_snapshot) -> Self {
        Self {
            identity: snapshot.identity(),
            principal: snapshot.principal(),
        }
    }
}

// Retained records use integer-only representations so all-zero is a valid
// intermediate state for complete volatile byte clearing.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, Eq, PartialEq))]
#[repr(C)]
struct ipc_slot {
    occupied: u64,
    sequence: u64,
    task_slot: u64,
    task_generation: u64,
    circle_slot: u64,
    circle_generation: u64,
    principal: u64,
    origin: u64,
    payload_length: u64,
    payload_bytes: [u8; ipc_payload_capacity],
}

impl ipc_slot {
    const fn empty() -> Self {
        Self {
            occupied: 0,
            sequence: 0,
            task_slot: 0,
            task_generation: 0,
            circle_slot: 0,
            circle_generation: 0,
            principal: 0,
            origin: 0,
            payload_length: 0,
            payload_bytes: [0; ipc_payload_capacity],
        }
    }

    fn is_clear(&self) -> bool {
        self.occupied == 0
            && self.sequence == 0
            && self.task_slot == 0
            && self.task_generation == 0
            && self.circle_slot == 0
            && self.circle_generation == 0
            && self.principal == 0
            && self.origin == 0
            && self.payload_length == 0
            && self.payload_bytes.iter().all(|byte| *byte == 0)
    }

    fn write(&mut self, sequence: u64, provenance: ipc_provenance, payload: ipc_payload) {
        *self = Self {
            occupied: 1,
            sequence,
            task_slot: provenance.task().task().slot() as u64,
            task_generation: provenance.task().task().generation_value(),
            circle_slot: provenance.task().circle().slot() as u64,
            circle_generation: provenance.task().circle().generation_value(),
            principal: provenance.principal().value(),
            origin: encode_origin(provenance.origin()),
            payload_length: u64::from(payload.length),
            payload_bytes: payload.bytes,
        };
    }

    fn message(&self, endpoint: endpoint_id) -> Option<ipc_message> {
        if self.occupied != 1 || self.payload_length > ipc_payload_capacity as u64 {
            return None;
        }
        let task = crate::identity::task_id::try_from_parts(
            usize::try_from(self.task_slot).ok()?,
            self.task_generation,
        )?;
        let circle = crate::identity::circle_id::try_from_parts(
            usize::try_from(self.circle_slot).ok()?,
            self.circle_generation,
        )?;
        let principal = fixture_principal_id::new(self.principal)?;
        let origin = decode_origin(self.origin)?;
        let payload = ipc_payload {
            length: self.payload_length as u8,
            bytes: self.payload_bytes,
        };
        if !payload.is_well_formed() {
            return None;
        }
        Some(ipc_message {
            endpoint,
            sequence: self.sequence,
            provenance: ipc_provenance {
                task: task_identity::new(task, circle),
                principal,
                origin,
            },
            payload,
        })
    }

    fn clear_and_verify(&mut self) -> bool {
        volatile_zero_and_verify(self)
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, Eq, PartialEq))]
#[repr(C)]
struct endpoint_slot<const queue_capacity: usize> {
    generation: u64,
    active: u64,
    receiver_task_slot: u64,
    receiver_task_generation: u64,
    receiver_circle_slot: u64,
    receiver_circle_generation: u64,
    receiver_principal: u64,
    head: u64,
    tail: u64,
    depth: u64,
    next_sequence: u64,
    sequence_exhausted: u64,
    slots: [ipc_slot; queue_capacity],
}

impl<const queue_capacity: usize> endpoint_slot<queue_capacity> {
    const fn empty() -> Self {
        Self {
            generation: 0,
            active: 0,
            receiver_task_slot: 0,
            receiver_task_generation: 0,
            receiver_circle_slot: 0,
            receiver_circle_generation: 0,
            receiver_principal: 0,
            head: 0,
            tail: 0,
            depth: 0,
            next_sequence: 0,
            sequence_exhausted: 0,
            slots: [ipc_slot::empty(); queue_capacity],
        }
    }

    fn new(generation: u64, receiver: ipc_receiver) -> Self {
        Self {
            generation,
            active: 1,
            receiver_task_slot: receiver.identity.task().slot() as u64,
            receiver_task_generation: receiver.identity.task().generation_value(),
            receiver_circle_slot: receiver.identity.circle().slot() as u64,
            receiver_circle_generation: receiver.identity.circle().generation_value(),
            receiver_principal: receiver.principal.value(),
            head: 0,
            tail: 0,
            depth: 0,
            next_sequence: 0,
            sequence_exhausted: 0,
            slots: [ipc_slot::empty(); queue_capacity],
        }
    }

    fn receiver(&self) -> Option<ipc_receiver> {
        if self.active != 1 {
            return None;
        }
        let task = crate::identity::task_id::try_from_parts(
            usize::try_from(self.receiver_task_slot).ok()?,
            self.receiver_task_generation,
        )?;
        let circle = crate::identity::circle_id::try_from_parts(
            usize::try_from(self.receiver_circle_slot).ok()?,
            self.receiver_circle_generation,
        )?;
        Some(ipc_receiver {
            identity: task_identity::new(task, circle),
            principal: fixture_principal_id::new(self.receiver_principal)?,
        })
    }

    fn is_structurally_valid(&self, endpoint: endpoint_id) -> bool {
        let Ok(head) = usize::try_from(self.head) else {
            return false;
        };
        let Ok(tail) = usize::try_from(self.tail) else {
            return false;
        };
        let Ok(depth) = usize::try_from(self.depth) else {
            return false;
        };
        let topology_valid = if queue_capacity == 0 {
            head == 0 && tail == 0 && depth == 0
        } else {
            head < queue_capacity
                && tail < queue_capacity
                && head
                    .checked_add(depth)
                    .is_some_and(|end| tail == end % queue_capacity)
                && self.slots.iter().enumerate().all(|(slot, record)| {
                    let distance = (slot + queue_capacity - head) % queue_capacity;
                    let expected_occupied = distance < depth;
                    let expected_sequence = if self.sequence_exhausted == 1 {
                        u64::MAX
                            .checked_sub((depth.saturating_sub(1)) as u64)
                            .and_then(|start| start.checked_add(distance as u64))
                    } else {
                        self.next_sequence
                            .checked_sub(depth as u64)
                            .and_then(|start| start.checked_add(distance as u64))
                    };
                    (expected_occupied
                        && record.occupied == 1
                        && record
                            .message(endpoint)
                            .is_some_and(|message| Some(message.sequence()) == expected_sequence))
                        || (!expected_occupied && record.occupied == 0 && record.is_clear())
                })
        };
        self.active == 1
            && self.generation == endpoint.generation_value()
            && self.receiver().is_some()
            && self.sequence_exhausted <= 1
            && (self.sequence_exhausted == 0 || self.next_sequence == u64::MAX)
            && depth <= queue_capacity
            && topology_valid
    }

    fn clear_preserving_generation(&mut self) -> bool {
        let generation = self.generation;
        if !volatile_zero_and_verify(self) {
            return false;
        }
        // Tombstone generation is the sole retained value. Volatile storage
        // and reread make reuse verification cover the final representation.
        unsafe {
            ptr::write_volatile(ptr::addr_of_mut!(self.generation), generation);
        }
        compiler_fence(Ordering::SeqCst);
        (unsafe { ptr::read_volatile(ptr::addr_of!(self.generation)) == generation })
            && self.active == 0
            && self.receiver_task_slot == 0
            && self.receiver_task_generation == 0
            && self.receiver_circle_slot == 0
            && self.receiver_circle_generation == 0
            && self.receiver_principal == 0
            && self.head == 0
            && self.tail == 0
            && self.depth == 0
            && self.next_sequence == 0
            && self.sequence_exhausted == 0
            && self.slots.iter().all(ipc_slot::is_clear)
    }
}

pub(crate) struct ipc_table<const endpoint_capacity: usize, const queue_capacity: usize> {
    endpoints: [endpoint_slot<queue_capacity>; endpoint_capacity],
}

pub(crate) type control_ipc_table = ipc_table<ipc_endpoint_capacity, ipc_queue_capacity>;

impl<const endpoint_capacity: usize, const queue_capacity: usize>
    ipc_table<endpoint_capacity, queue_capacity>
{
    pub(crate) const fn new() -> Self {
        Self {
            endpoints: [endpoint_slot::empty(); endpoint_capacity],
        }
    }

    pub(crate) fn create_endpoint(
        &mut self,
        circles: &mut circle_table,
        tasks: &task_table,
        receiver: task_identity,
    ) -> Result<endpoint_id, ipc_error> {
        let snapshot = tasks.snapshot(receiver).map_err(ipc_error::task)?;
        if snapshot.state().is_terminal() {
            return Err(ipc_error::receiver_not_live);
        }
        let receiver = ipc_receiver::from_snapshot(snapshot);
        let (slot, generation) = self.next_endpoint()?;

        circles
            .attach_child(receiver.identity.circle(), circle_child_kind::endpoint)
            .map_err(ipc_error::circle)?;
        self.endpoints[slot] = endpoint_slot::new(generation.get(), receiver);
        Ok(endpoint_id::from_parts(slot, generation))
    }

    pub(crate) fn snapshot(
        &self,
        endpoint: endpoint_id,
    ) -> Result<ipc_endpoint_snapshot, ipc_error> {
        let record = self.endpoint(endpoint)?;
        if !record.is_structurally_valid(endpoint) {
            return Err(ipc_error::corrupt_slot);
        }
        let receiver = record.receiver().ok_or(ipc_error::corrupt_slot)?;
        Ok(ipc_endpoint_snapshot {
            endpoint,
            receiver: receiver.identity,
            depth: record.depth as usize,
            capacity: queue_capacity,
            next_sequence: if record.sequence_exhausted == 1 {
                None
            } else {
                Some(record.next_sequence)
            },
        })
    }

    fn send(
        &mut self,
        tasks: &task_table,
        sender: task_identity,
        endpoint: endpoint_id,
        payload: ipc_payload,
    ) -> Result<u64, ipc_error> {
        let snapshot = tasks.snapshot(sender).map_err(ipc_error::task)?;
        if snapshot.state() != task_state::running {
            return Err(ipc_error::sender_not_running);
        }
        if !payload.is_well_formed() {
            return Err(ipc_error::payload_too_large);
        }
        let endpoint_record = self.endpoint(endpoint)?;
        if !endpoint_record.is_structurally_valid(endpoint) {
            return Err(ipc_error::corrupt_slot);
        }
        let stored_receiver = endpoint_record.receiver().ok_or(ipc_error::corrupt_slot)?;
        let current_receiver = tasks
            .snapshot(stored_receiver.identity)
            .map_err(ipc_error::task)?;
        if current_receiver.state().is_terminal()
            || ipc_receiver::from_snapshot(current_receiver) != stored_receiver
        {
            return Err(ipc_error::receiver_not_live);
        }
        let record = self.endpoint_mut(endpoint)?;
        if !record.is_structurally_valid(endpoint) {
            return Err(ipc_error::corrupt_slot);
        }
        if record.depth as usize == queue_capacity {
            return Err(ipc_error::would_block);
        }
        if record.sequence_exhausted == 1 {
            return Err(ipc_error::sequence_exhausted);
        }
        if queue_capacity == 0 {
            return Err(ipc_error::would_block);
        }
        let tail = record.tail as usize;
        if !record.slots[tail].is_clear() {
            return Err(ipc_error::corrupt_slot);
        }

        let sequence = record.next_sequence;
        record.slots[tail].write(sequence, ipc_provenance::from_snapshot(snapshot), payload);
        record.tail = ((tail + 1) % queue_capacity) as u64;
        record.depth += 1;
        if sequence == u64::MAX {
            record.sequence_exhausted = 1;
        } else {
            record.next_sequence = sequence + 1;
        }
        Ok(sequence)
    }

    /// Validate a destination without side effects, run an infallible copy
    /// commit, and only then consume the head message.
    fn receive_with<P, C>(
        &mut self,
        tasks: &task_table,
        receiver: task_identity,
        endpoint: endpoint_id,
        preflight: P,
        commit: C,
    ) -> Result<ipc_message, ipc_error>
    where
        P: FnOnce(&ipc_message) -> bool,
        C: FnOnce(&ipc_message),
    {
        let snapshot = tasks.snapshot(receiver).map_err(ipc_error::task)?;
        if snapshot.state() != task_state::running {
            return Err(ipc_error::receiver_not_running);
        }
        let expected_receiver = ipc_receiver::from_snapshot(snapshot);
        let record = self.endpoint_mut(endpoint)?;
        if !record.is_structurally_valid(endpoint) {
            return Err(ipc_error::corrupt_slot);
        }
        if record.receiver() != Some(expected_receiver) {
            return Err(ipc_error::receiver_mismatch);
        }
        if record.depth == 0 {
            return Err(ipc_error::would_block);
        }
        if queue_capacity == 0 {
            return Err(ipc_error::corrupt_slot);
        }
        let head = record.head as usize;
        let message = record.slots[head]
            .message(endpoint)
            .ok_or(ipc_error::corrupt_slot)?;
        if !preflight(&message) {
            return Err(ipc_error::copy_failed);
        }
        commit(&message);
        assert!(
            record.slots[head].clear_and_verify(),
            "fatal: consumed IPC slot did not clear"
        );
        record.head = ((head + 1) % queue_capacity) as u64;
        record.depth -= 1;
        Ok(message)
    }

    /// Authorize and enqueue as one serialized kernel operation. There is no
    /// crate-visible unguarded send surface.
    pub(crate) fn send_authorized(
        &mut self,
        tasks: &task_table,
        sigils: &control_sigil_table,
        sender: task_identity,
        reference: sigil_reference,
        endpoint: endpoint_id,
        payload: ipc_payload,
    ) -> Result<u64, ipc_authorized_error> {
        sigils
            .authorize_endpoint(tasks, sender, reference, endpoint, sigil_rights::send)
            .map_err(ipc_authorized_error::sigil)?;
        self.send(tasks, sender, endpoint, payload)
            .map_err(ipc_authorized_error::ipc)
    }

    /// Authorize, preflight a fixed destination without side effects, perform
    /// an infallible copy commit, and only then consume the queue head.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn receive_authorized_with<P, C>(
        &mut self,
        tasks: &task_table,
        sigils: &control_sigil_table,
        receiver: task_identity,
        reference: sigil_reference,
        endpoint: endpoint_id,
        preflight: P,
        commit: C,
    ) -> Result<ipc_message, ipc_authorized_error>
    where
        P: FnOnce(&ipc_message) -> bool,
        C: FnOnce(&ipc_message),
    {
        sigils
            .authorize_endpoint(tasks, receiver, reference, endpoint, sigil_rights::receive)
            .map_err(ipc_authorized_error::sigil)?;
        self.receive_with(tasks, receiver, endpoint, preflight, commit)
            .map_err(ipc_authorized_error::ipc)
    }

    pub(crate) fn withdraw_endpoint(
        &mut self,
        circles: &mut circle_table,
        tasks: &task_table,
        receiver: task_identity,
        endpoint: endpoint_id,
    ) -> Result<usize, ipc_error> {
        let snapshot = tasks.snapshot(receiver).map_err(ipc_error::task)?;
        let expected_receiver = ipc_receiver::from_snapshot(snapshot);
        let endpoint_slot = self.endpoint_index(endpoint)?;
        let record = &self.endpoints[endpoint_slot];
        if !record.is_structurally_valid(endpoint) {
            return Err(ipc_error::corrupt_slot);
        }
        if record.receiver() != Some(expected_receiver) {
            return Err(ipc_error::receiver_mismatch);
        }
        let messages = record.depth as usize;
        let circle_snapshot = circles
            .snapshot(receiver.circle())
            .map_err(ipc_error::circle)?;
        if circle_snapshot.children().endpoints() == 0 {
            return Err(ipc_error::circle_accounting_mismatch);
        }
        circles
            .detach_child(receiver.circle(), circle_child_kind::endpoint)
            .map_err(ipc_error::circle)?;
        assert!(
            self.endpoints[endpoint_slot].clear_preserving_generation(),
            "fatal: withdrawn IPC endpoint did not clear"
        );
        Ok(messages)
    }

    pub(crate) fn withdraw_receiver(
        &mut self,
        circles: &mut circle_table,
        tasks: &task_table,
        receiver: task_identity,
    ) -> Result<ipc_accounting, ipc_error> {
        let snapshot = tasks.snapshot(receiver).map_err(ipc_error::task)?;
        let expected_receiver = ipc_receiver::from_snapshot(snapshot);
        let mut endpoints = 0usize;
        let mut messages = 0usize;
        for (slot, record) in self.endpoints.iter().enumerate() {
            if record.active == 0 {
                continue;
            }
            let endpoint = endpoint_id::try_from_parts(slot, record.generation)
                .ok_or(ipc_error::corrupt_slot)?;
            if record.active != 1 || !record.is_structurally_valid(endpoint) {
                return Err(ipc_error::corrupt_slot);
            }
            if record.receiver() == Some(expected_receiver) {
                endpoints += 1;
                messages += record.depth as usize;
            }
        }
        let amount = u16::try_from(endpoints).map_err(|_| ipc_error::circle_accounting_mismatch)?;
        let circle_count = circles
            .snapshot(receiver.circle())
            .map_err(ipc_error::circle)?
            .children()
            .endpoints();
        if circle_count != amount {
            return Err(ipc_error::circle_accounting_mismatch);
        }
        circles
            .detach_children(receiver.circle(), circle_child_kind::endpoint, amount)
            .map_err(ipc_error::circle)?;
        for record in &mut self.endpoints {
            if record.receiver() == Some(expected_receiver) {
                assert!(
                    record.clear_preserving_generation(),
                    "fatal: withdrawn IPC receiver record did not clear"
                );
            }
        }
        Ok(ipc_accounting {
            endpoints,
            messages,
        })
    }

    pub(crate) fn accounting(&self) -> ipc_accounting {
        ipc_accounting {
            endpoints: self
                .endpoints
                .iter()
                .filter(|record| record.active == 1)
                .count(),
            messages: self
                .endpoints
                .iter()
                .filter(|record| record.active == 1)
                .map(|record| record.depth as usize)
                .sum(),
        }
    }

    fn next_endpoint(&self) -> Result<(usize, core::num::NonZeroU64), ipc_error> {
        let mut saw_free = false;
        for (slot, endpoint) in self.endpoints.iter().enumerate() {
            if endpoint.active == 1 {
                continue;
            }
            if endpoint.active != 0 {
                return Err(ipc_error::corrupt_slot);
            }
            saw_free = true;
            if let Some(generation) = next_generation(endpoint.generation) {
                return Ok((slot, generation));
            }
        }
        Err(if saw_free {
            ipc_error::endpoint_generation_exhausted
        } else {
            ipc_error::endpoint_capacity_exhausted
        })
    }

    fn endpoint(&self, endpoint: endpoint_id) -> Result<&endpoint_slot<queue_capacity>, ipc_error> {
        let slot = self.endpoint_index(endpoint)?;
        Ok(&self.endpoints[slot])
    }

    fn endpoint_mut(
        &mut self,
        endpoint: endpoint_id,
    ) -> Result<&mut endpoint_slot<queue_capacity>, ipc_error> {
        let slot = self.endpoint_index(endpoint)?;
        Ok(&mut self.endpoints[slot])
    }

    fn endpoint_index(&self, endpoint: endpoint_id) -> Result<usize, ipc_error> {
        let Some(record) = self.endpoints.get(endpoint.slot()) else {
            return Err(ipc_error::invalid_endpoint);
        };
        if record.active != 1 || record.generation != endpoint.generation_value() {
            return Err(ipc_error::stale_endpoint);
        }
        Ok(endpoint.slot())
    }
}

fn encode_origin(origin: synthetic_origin_metadata) -> u64 {
    match origin {
        synthetic_origin_metadata::human_role_fixture => 1,
        synthetic_origin_metadata::machine_role_fixture => 2,
        synthetic_origin_metadata::receiver_role_fixture => 3,
    }
}

fn decode_origin(value: u64) -> Option<synthetic_origin_metadata> {
    match value {
        1 => Some(synthetic_origin_metadata::human_role_fixture),
        2 => Some(synthetic_origin_metadata::machine_role_fixture),
        3 => Some(synthetic_origin_metadata::receiver_role_fixture),
        _ => None,
    }
}

fn volatile_zero_and_verify<T>(value: &mut T) -> bool {
    let bytes = (value as *mut T).cast::<u8>();
    for offset in 0..size_of::<T>() {
        // SAFETY: `bytes` spans the complete uniquely borrowed integer-only
        // table record, whose all-zero representation is valid.
        unsafe {
            ptr::write_volatile(bytes.add(offset), 0);
        }
    }
    compiler_fence(Ordering::SeqCst);
    (0..size_of::<T>()).all(|offset| {
        // SAFETY: every byte was initialized above and remains borrowed.
        unsafe { ptr::read_volatile(bytes.add(offset)) == 0 }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circle::circle_table;
    use crate::scheduler::scheduler_table;
    use crate::task::{task_binding, task_bounds};

    fn bounds() -> task_bounds {
        task_bounds::new(0x1000, 0x2000, 0x1000, 0x3000, 0x4000, 0x4000).expect("valid task bounds")
    }

    fn add_task(
        circles: &mut circle_table,
        tasks: &mut task_table,
        principal: u64,
        origin: synthetic_origin_metadata,
    ) -> task_identity {
        let circle = circles.create().expect("circle");
        tasks
            .create(
                circles,
                task_binding::new(
                    circle,
                    fixture_principal_id::new(principal).expect("principal"),
                    origin,
                    bounds(),
                ),
            )
            .expect("task")
    }

    fn payload(bytes: &[u8]) -> ipc_payload {
        ipc_payload::copy_from(bytes).expect("bounded payload")
    }

    fn all_rights() -> sigil_rights {
        sigil_rights::send
            .union(sigil_rights::receive)
            .union(sigil_rights::delegate)
            .union(sigil_rights::revoke)
    }

    #[test]
    fn payload_is_bounded_copied_and_zero_padded() {
        let source = [0x5a; ipc_payload_capacity];
        let copied = payload(&source);
        assert_eq!(copied.as_bytes(), source);
        assert!(copied.is_well_formed());
        assert_eq!(
            ipc_payload::copy_from(&[0; ipc_payload_capacity + 1]),
            Err(ipc_error::payload_too_large)
        );

        let short = payload(&[1, 2, 3]);
        assert_eq!(short.as_bytes(), &[1, 2, 3]);
        assert!(short.bytes[3..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn endpoint_ids_are_kernel_allocated_bounded_and_generational() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            3,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let mut endpoints = [None; ipc_endpoint_capacity];
        for endpoint in &mut endpoints {
            *endpoint = Some(
                ipc.create_endpoint(&mut circles, &tasks, receiver)
                    .expect("within endpoint capacity"),
            );
        }
        assert_eq!(
            ipc.create_endpoint(&mut circles, &tasks, receiver),
            Err(ipc_error::endpoint_capacity_exhausted)
        );
        assert_eq!(ipc.accounting().endpoints, ipc_endpoint_capacity);
        assert_eq!(
            circles
                .snapshot(receiver.circle())
                .expect("circle")
                .children()
                .endpoints(),
            ipc_endpoint_capacity as u16
        );

        let old = endpoints[0].expect("first endpoint");
        ipc.withdraw_endpoint(&mut circles, &tasks, receiver, old)
            .expect("withdraw endpoint");
        assert_eq!(ipc.snapshot(old), Err(ipc_error::stale_endpoint));
        let replacement = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("replacement endpoint");
        assert_eq!(replacement.slot(), old.slot());
        assert_eq!(replacement.generation_value(), 2);
    }

    #[test]
    fn endpoint_generation_exhaustion_never_wraps_or_mutates_circle() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            3,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let mut ipc: ipc_table<1, ipc_queue_capacity> = ipc_table::new();
        ipc.endpoints[0].generation = u64::MAX;
        let before = circles.snapshot(receiver.circle()).expect("circle");

        assert_eq!(
            ipc.create_endpoint(&mut circles, &tasks, receiver),
            Err(ipc_error::endpoint_generation_exhausted)
        );
        assert_eq!(circles.snapshot(receiver.circle()), Ok(before));
        assert_eq!(ipc.accounting().endpoints, 0);
    }

    #[test]
    fn accepted_send_stamps_fresh_kernel_provenance_and_exact_sequence() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        tasks.start(sender).expect("start sender");

        assert_eq!(ipc.send(&tasks, sender, endpoint, payload(b"copy")), Ok(0));
        let slot = &ipc.endpoints[endpoint.slot()].slots[0];
        let message = slot.message(endpoint).expect("stored message");
        assert_eq!(message.endpoint(), endpoint);
        assert_eq!(message.sequence(), 0);
        assert_eq!(message.provenance().task(), sender);
        assert_eq!(message.provenance().principal().value(), 10);
        assert_eq!(
            message.provenance().origin(),
            synthetic_origin_metadata::human_role_fixture
        );
        assert_eq!(message.payload().as_bytes(), b"copy");
        assert_eq!(
            ipc.snapshot(endpoint).expect("endpoint").next_sequence(),
            Some(1)
        );
    }

    #[test]
    fn exited_sender_snapshot_and_exited_receiver_are_revalidated() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        tasks.start(sender).expect("start sender");
        tasks.exit(sender).expect("exit sender");
        let before = ipc.endpoints[endpoint.slot()];
        assert_eq!(
            ipc.send(&tasks, sender, endpoint, payload(b"late")),
            Err(ipc_error::sender_not_running)
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before);

        tasks.start(receiver).expect("start receiver");
        tasks.exit(receiver).expect("exit receiver");
        let second_sender = add_task(
            &mut circles,
            &mut tasks,
            20,
            synthetic_origin_metadata::machine_role_fixture,
        );
        tasks.start(second_sender).expect("start second sender");
        assert_eq!(
            ipc.send(&tasks, second_sender, endpoint, payload(b"late")),
            Err(ipc_error::receiver_not_live)
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before);
    }

    #[test]
    fn suspended_receiver_remains_live_but_faulted_receiver_does_not() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("ready receiver endpoint");
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, receiver)
            .expect("admit receiver");
        scheduler
            .admit(&mut circles, &mut tasks, sender)
            .expect("admit sender");
        assert_eq!(
            circles
                .snapshot(receiver.circle())
                .expect("receiver circle")
                .children()
                .task_contexts(),
            1
        );
        assert_eq!(
            circles
                .snapshot(sender.circle())
                .expect("sender circle")
                .children()
                .task_contexts(),
            1
        );

        assert_eq!(
            scheduler
                .start_next(&mut tasks)
                .expect("start receiver")
                .task(),
            receiver
        );
        scheduler
            .preempt_current(&mut tasks, receiver)
            .expect("suspend receiver");
        assert_eq!(
            tasks.snapshot(receiver).expect("receiver").state(),
            task_state::suspended
        );
        let suspended_endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("suspended receiver remains live");
        assert_eq!(
            ipc.send(&tasks, sender, endpoint, payload(b"queued")),
            Ok(0)
        );
        let queued = ipc.endpoints[endpoint.slot()];
        assert_eq!(
            ipc.receive_with(&tasks, receiver, endpoint, |_| true, |_| {}),
            Err(ipc_error::receiver_not_running)
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], queued);

        scheduler
            .preempt_current(&mut tasks, sender)
            .expect("resume receiver");
        let message = ipc
            .receive_with(&tasks, receiver, endpoint, |_| true, |_| {})
            .expect("running receiver consumes message");
        assert_eq!(message.payload().as_bytes(), b"queued");
        scheduler
            .fault_current(&mut tasks, receiver)
            .expect("fault receiver and continue sender");
        assert_eq!(
            tasks.snapshot(receiver).expect("receiver").state(),
            task_state::faulted
        );

        let before_fault = (
            ipc.accounting(),
            ipc.endpoints[endpoint.slot()],
            ipc.endpoints[suspended_endpoint.slot()],
            circles
                .snapshot(receiver.circle())
                .expect("receiver circle"),
        );
        assert_eq!(
            ipc.create_endpoint(&mut circles, &tasks, receiver),
            Err(ipc_error::receiver_not_live)
        );
        assert_eq!(
            ipc.send(&tasks, sender, endpoint, payload(b"late")),
            Err(ipc_error::receiver_not_live)
        );
        assert_eq!(
            ipc.receive_with(&tasks, receiver, endpoint, |_| true, |_| {}),
            Err(ipc_error::receiver_not_running)
        );
        assert_eq!(
            (
                ipc.accounting(),
                ipc.endpoints[endpoint.slot()],
                ipc.endpoints[suspended_endpoint.slot()],
                circles
                    .snapshot(receiver.circle())
                    .expect("receiver circle"),
            ),
            before_fault
        );

        scheduler
            .withdraw_terminal_context(&mut circles, &mut tasks, receiver)
            .expect("withdraw faulted receiver context");
        assert_eq!(
            circles
                .snapshot(receiver.circle())
                .expect("receiver circle")
                .children()
                .task_contexts(),
            0
        );
        assert_eq!(
            circles
                .snapshot(sender.circle())
                .expect("sender circle")
                .children()
                .task_contexts(),
            1
        );
    }

    #[test]
    fn full_send_is_nonblocking_and_does_not_consume_sequence() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        tasks.start(sender).expect("start sender");
        assert_eq!(ipc.send(&tasks, sender, endpoint, payload(b"a")), Ok(0));
        assert_eq!(ipc.send(&tasks, sender, endpoint, payload(b"b")), Ok(1));
        let before = ipc.endpoints[endpoint.slot()];

        assert_eq!(
            ipc.send(&tasks, sender, endpoint, payload(b"c")),
            Err(ipc_error::would_block)
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before);
        assert_eq!(ipc.snapshot(endpoint).expect("endpoint").depth(), 2);
        assert_eq!(
            ipc.snapshot(endpoint).expect("endpoint").next_sequence(),
            Some(2)
        );
    }

    #[test]
    fn failed_copy_out_leaves_queue_and_destination_unchanged() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        tasks.start(sender).expect("start sender");
        ipc.send(&tasks, sender, endpoint, payload(b"private-marker"))
            .expect("send");
        tasks.exit(sender).expect("exit sender");
        tasks.start(receiver).expect("start receiver");
        let before = ipc.endpoints[endpoint.slot()];
        let mut destination: Option<ipc_message> = None;

        assert_eq!(
            ipc.receive_with(&tasks, receiver, endpoint, |_| false, |_| {}),
            Err(ipc_error::copy_failed)
        );
        assert_eq!(destination, None);
        assert_eq!(ipc.endpoints[endpoint.slot()], before);

        let delivered = ipc
            .receive_with(
                &tasks,
                receiver,
                endpoint,
                |_| true,
                |message| destination = Some(*message),
            )
            .expect("completed copy-out");
        assert_eq!(destination, Some(delivered));
        assert_eq!(delivered.payload().as_bytes(), b"private-marker");
        assert_eq!(ipc.snapshot(endpoint).expect("endpoint").depth(), 0);
        assert!(ipc.endpoints[endpoint.slot()].slots[0].is_clear());
    }

    #[test]
    fn receive_is_fifo_exact_owner_and_empty_is_failure_atomic() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let wrong_receiver = add_task(
            &mut circles,
            &mut tasks,
            40,
            synthetic_origin_metadata::machine_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        tasks.start(sender).expect("start sender");
        ipc.send(&tasks, sender, endpoint, payload(b"first"))
            .expect("first send");
        ipc.send(&tasks, sender, endpoint, payload(b"second"))
            .expect("second send");
        tasks.exit(sender).expect("exit sender");
        tasks.start(wrong_receiver).expect("start wrong receiver");
        let before_wrong = ipc.endpoints[endpoint.slot()];
        assert_eq!(
            ipc.receive_with(&tasks, wrong_receiver, endpoint, |_| true, |_| {}),
            Err(ipc_error::receiver_mismatch)
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before_wrong);
        tasks.exit(wrong_receiver).expect("exit wrong receiver");
        tasks.start(receiver).expect("start receiver");

        let first = ipc
            .receive_with(&tasks, receiver, endpoint, |_| true, |_| {})
            .expect("first receive");
        let second = ipc
            .receive_with(&tasks, receiver, endpoint, |_| true, |_| {})
            .expect("second receive");
        assert_eq!(
            (first.sequence(), first.payload().as_bytes()),
            (0, b"first".as_slice())
        );
        assert_eq!(
            (second.sequence(), second.payload().as_bytes()),
            (1, b"second".as_slice())
        );
        let empty = ipc.endpoints[endpoint.slot()];
        assert_eq!(
            ipc.receive_with(&tasks, receiver, endpoint, |_| true, |_| {}),
            Err(ipc_error::would_block)
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], empty);
    }

    #[test]
    fn sequence_accepts_max_once_then_never_wraps() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        ipc.endpoints[endpoint.slot()].next_sequence = u64::MAX;
        tasks.start(sender).expect("start sender");
        assert_eq!(
            ipc.send(&tasks, sender, endpoint, payload(b"last")),
            Ok(u64::MAX)
        );
        let before = ipc.endpoints[endpoint.slot()];
        assert_eq!(
            ipc.send(&tasks, sender, endpoint, payload(b"never")),
            Err(ipc_error::sequence_exhausted)
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before);
        assert_eq!(
            ipc.snapshot(endpoint).expect("endpoint").next_sequence(),
            None
        );
    }

    #[test]
    fn zero_capacity_queue_returns_backpressure_without_indexing() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc: ipc_table<1, 0> = ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        tasks.start(sender).expect("start sender");
        let before = ipc.endpoints[0];
        assert_eq!(
            ipc.send(&tasks, sender, endpoint, payload(b"x")),
            Err(ipc_error::would_block)
        );
        assert_eq!(ipc.endpoints[0], before);
    }

    #[test]
    fn corrupt_ring_topology_is_rejected_before_callback_or_mutation() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        tasks.start(sender).expect("start sender");
        ipc.send(&tasks, sender, endpoint, payload(b"x"))
            .expect("send");
        tasks.exit(sender).expect("exit sender");
        tasks.start(receiver).expect("start receiver");
        ipc.endpoints[endpoint.slot()].tail = 0;
        let corrupt = ipc.endpoints[endpoint.slot()];
        let mut callback_called = false;

        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        assert_eq!(
            ipc.receive_with(
                &tasks,
                receiver,
                endpoint,
                |_| {
                    callback_called = true;
                    true
                },
                |_| {},
            ),
            Err(ipc_error::corrupt_slot)
        );
        assert!(!callback_called);
        assert_eq!(ipc.endpoints[endpoint.slot()], corrupt);
    }

    #[test]
    fn withdrawal_clears_complete_queue_and_endpoint_record_before_reuse() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        tasks.start(sender).expect("start sender");
        ipc.send(&tasks, sender, endpoint, payload(b"private-marker"))
            .expect("send");
        tasks.exit(sender).expect("exit sender");

        assert_eq!(
            ipc.withdraw_endpoint(&mut circles, &tasks, receiver, endpoint),
            Ok(1)
        );
        let tombstone = &ipc.endpoints[endpoint.slot()];
        assert_eq!(tombstone.generation, 1);
        assert_eq!(tombstone.active, 0);
        assert_eq!(tombstone.receiver_task_generation, 0);
        assert_eq!(tombstone.receiver_principal, 0);
        assert_eq!(tombstone.next_sequence, 0);
        assert!(tombstone.slots.iter().all(ipc_slot::is_clear));
        assert_eq!(
            circles
                .snapshot(receiver.circle())
                .expect("circle")
                .children()
                .endpoints(),
            0
        );
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::stale_endpoint));
    }

    #[test]
    fn receiver_bulk_withdrawal_is_counted_and_failure_atomic() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        for _ in 0..ipc_endpoint_capacity {
            ipc.create_endpoint(&mut circles, &tasks, receiver)
                .expect("endpoint");
        }
        let result = ipc
            .withdraw_receiver(&mut circles, &tasks, receiver)
            .expect("bulk withdrawal");
        assert_eq!(
            result,
            ipc_accounting {
                endpoints: ipc_endpoint_capacity,
                messages: 0
            }
        );
        assert_eq!(ipc.accounting().endpoints, 0);
        assert!(ipc.endpoints.iter().all(|record| record.active == 0));

        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("replacement endpoint");
        circles
            .detach_child(receiver.circle(), circle_child_kind::endpoint)
            .expect("induce mismatch");
        let before = ipc.endpoints[endpoint.slot()];
        assert_eq!(
            ipc.withdraw_receiver(&mut circles, &tasks, receiver),
            Err(ipc_error::circle_accounting_mismatch)
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before);
    }

    #[test]
    fn invalid_stale_and_wrong_receiver_operations_do_not_mutate_endpoint() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let wrong = add_task(
            &mut circles,
            &mut tasks,
            40,
            synthetic_origin_metadata::machine_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        let before = ipc.endpoints[endpoint.slot()];
        let invalid = endpoint_id::try_from_parts(ipc_endpoint_capacity, 1).expect("invalid id");
        let stale = endpoint_id::try_from_parts(endpoint.slot(), 2).expect("stale id");
        assert_eq!(ipc.snapshot(invalid), Err(ipc_error::invalid_endpoint));
        assert_eq!(ipc.snapshot(stale), Err(ipc_error::stale_endpoint));
        assert_eq!(
            ipc.withdraw_endpoint(&mut circles, &tasks, wrong, endpoint),
            Err(ipc_error::receiver_mismatch)
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before);
    }

    #[test]
    fn composite_api_enforces_direction_namespace_and_receiver_ownership() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let outsider = add_task(
            &mut circles,
            &mut tasks,
            20,
            synthetic_origin_metadata::machine_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        let mut sigils = control_sigil_table::new();
        let root = sigils
            .issue_root(&mut circles, &tasks, sender, endpoint, all_rights())
            .expect("root");
        tasks.start(sender).expect("start sender");
        let sender_receive_only = sigils
            .attenuate(
                &mut circles,
                &tasks,
                sender,
                root,
                sender,
                endpoint,
                sigil_rights::receive,
            )
            .expect("sender receive-only binding");
        let receiver_receive = sigils
            .attenuate(
                &mut circles,
                &tasks,
                sender,
                root,
                receiver,
                endpoint,
                sigil_rights::receive,
            )
            .expect("receiver binding");
        let receiver_send_only = sigils
            .attenuate(
                &mut circles,
                &tasks,
                sender,
                root,
                receiver,
                endpoint,
                sigil_rights::send,
            )
            .expect("receiver send-only binding");
        let outsider_receive = sigils
            .attenuate(
                &mut circles,
                &tasks,
                sender,
                root,
                outsider,
                endpoint,
                sigil_rights::receive,
            )
            .expect("outsider receive binding");
        sigils
            .attenuate(
                &mut circles,
                &tasks,
                sender,
                root,
                outsider,
                endpoint,
                sigil_rights::receive,
            )
            .expect("outsider second binding");
        let copied_outsider_reference = sigils
            .attenuate(
                &mut circles,
                &tasks,
                sender,
                root,
                outsider,
                endpoint,
                sigil_rights::receive,
            )
            .expect("outsider third binding");

        let before_direction = ipc.endpoints[endpoint.slot()];
        let sigil_before = sigils.accounting();
        assert_eq!(
            ipc.send_authorized(
                &tasks,
                &sigils,
                sender,
                sender_receive_only,
                endpoint,
                payload(b"denied")
            ),
            Err(ipc_authorized_error::sigil(
                sigil_error::insufficient_rights
            ))
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before_direction);
        assert_eq!(sigils.accounting(), sigil_before);

        let wrong_endpoint =
            endpoint_id::try_from_parts(endpoint.slot(), 2).expect("wrong endpoint");
        assert_eq!(
            ipc.send_authorized(
                &tasks,
                &sigils,
                sender,
                root,
                wrong_endpoint,
                payload(b"denied")
            ),
            Err(ipc_authorized_error::sigil(sigil_error::endpoint_mismatch))
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before_direction);
        assert_eq!(
            ipc.send_authorized(
                &tasks,
                &sigils,
                sender,
                copied_outsider_reference,
                endpoint,
                payload(b"denied")
            ),
            Err(ipc_authorized_error::sigil(sigil_error::stale_reference))
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before_direction);

        assert_eq!(
            ipc.send_authorized(
                &tasks,
                &sigils,
                sender,
                root,
                endpoint,
                payload(b"accepted")
            ),
            Ok(0)
        );
        tasks.exit(sender).expect("exit sender");
        tasks.start(outsider).expect("start outsider");
        let before_outsider = ipc.endpoints[endpoint.slot()];
        assert_eq!(
            ipc.receive_authorized_with(
                &tasks,
                &sigils,
                outsider,
                outsider_receive,
                endpoint,
                |_| true,
                |_| {},
            ),
            Err(ipc_authorized_error::ipc(ipc_error::receiver_mismatch))
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before_outsider);
        tasks.exit(outsider).expect("exit outsider");
        tasks.start(receiver).expect("start receiver");
        assert_eq!(
            ipc.receive_authorized_with(
                &tasks,
                &sigils,
                receiver,
                receiver_send_only,
                endpoint,
                |_| true,
                |_| {},
            ),
            Err(ipc_authorized_error::sigil(
                sigil_error::insufficient_rights
            ))
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before_outsider);

        let mut delivered = None;
        let message = ipc
            .receive_authorized_with(
                &tasks,
                &sigils,
                receiver,
                receiver_receive,
                endpoint,
                |_| true,
                |message| delivered = Some(*message),
            )
            .expect("authorized receive");
        assert_eq!(delivered, Some(message));
        assert_eq!(message.payload().as_bytes(), b"accepted");
    }

    #[test]
    fn composite_receive_preflight_and_revocation_preserve_queue_on_error() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        let mut sigils = control_sigil_table::new();
        let root = sigils
            .issue_root(&mut circles, &tasks, sender, endpoint, all_rights())
            .expect("root");
        tasks.start(sender).expect("start sender");
        let receiver_reference = sigils
            .attenuate(
                &mut circles,
                &tasks,
                sender,
                root,
                receiver,
                endpoint,
                sigil_rights::receive,
            )
            .expect("receiver binding");
        let revoked_root = sigils
            .issue_root(&mut circles, &tasks, sender, endpoint, all_rights())
            .expect("second root");
        let revoked_receiver_reference = sigils
            .attenuate(
                &mut circles,
                &tasks,
                sender,
                revoked_root,
                receiver,
                endpoint,
                sigil_rights::receive,
            )
            .expect("revoked receiver binding");
        ipc.send_authorized(&tasks, &sigils, sender, root, endpoint, payload(b"message"))
            .expect("send");
        assert_eq!(
            sigils.revoke_lineage(&tasks, sender, revoked_root, endpoint),
            Ok(crate::sigil::sigil_revocation::revoked { bindings: 2 })
        );
        tasks.exit(sender).expect("exit sender");
        tasks.start(receiver).expect("start receiver");

        let before_preflight = ipc.endpoints[endpoint.slot()];
        let mut commit_called = false;
        assert_eq!(
            ipc.receive_authorized_with(
                &tasks,
                &sigils,
                receiver,
                receiver_reference,
                endpoint,
                |_| false,
                |_| commit_called = true,
            ),
            Err(ipc_authorized_error::ipc(ipc_error::copy_failed))
        );
        assert!(!commit_called);
        assert_eq!(ipc.endpoints[endpoint.slot()], before_preflight);

        assert_eq!(
            sigils.accounting().bindings,
            4,
            "revocation retains bindings until withdrawal"
        );
        assert_eq!(
            ipc.receive_authorized_with(
                &tasks,
                &sigils,
                receiver,
                revoked_receiver_reference,
                endpoint,
                |_| true,
                |_| {},
            ),
            Err(ipc_authorized_error::sigil(sigil_error::revoked))
        );
        assert_eq!(ipc.endpoints[endpoint.slot()], before_preflight);
    }

    #[test]
    fn endpoint_reuse_cannot_retarget_an_old_sigil() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let old_endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("old endpoint");
        let mut sigils = control_sigil_table::new();
        let old_sigil = sigils
            .issue_root(
                &mut circles,
                &tasks,
                sender,
                old_endpoint,
                sigil_rights::send,
            )
            .expect("old endpoint sigil");
        ipc.withdraw_endpoint(&mut circles, &tasks, receiver, old_endpoint)
            .expect("withdraw old endpoint");
        let replacement = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("replacement endpoint");
        assert_eq!(replacement.slot(), old_endpoint.slot());
        assert_ne!(
            replacement.generation_value(),
            old_endpoint.generation_value()
        );
        tasks.start(sender).expect("start sender");
        let before = ipc.endpoints[replacement.slot()];

        assert_eq!(
            ipc.send_authorized(
                &tasks,
                &sigils,
                sender,
                old_sigil,
                replacement,
                payload(b"denied")
            ),
            Err(ipc_authorized_error::sigil(sigil_error::endpoint_mismatch))
        );
        assert_eq!(ipc.endpoints[replacement.slot()], before);
        assert_eq!(
            ipc.send_authorized(
                &tasks,
                &sigils,
                sender,
                old_sigil,
                old_endpoint,
                payload(b"denied")
            ),
            Err(ipc_authorized_error::ipc(ipc_error::stale_endpoint))
        );
        assert_eq!(ipc.endpoints[replacement.slot()], before);
    }

    #[test]
    fn identical_local_reference_bits_resolve_only_current_namespace() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        let mut sigils = control_sigil_table::new();
        let sender_reference = sigils
            .issue_root(&mut circles, &tasks, sender, endpoint, sigil_rights::send)
            .expect("sender binding");
        let receiver_reference = sigils
            .issue_root(
                &mut circles,
                &tasks,
                receiver,
                endpoint,
                sigil_rights::receive,
            )
            .expect("receiver binding");
        assert_eq!(sender_reference, receiver_reference);

        tasks.start(sender).expect("start sender");
        assert_eq!(
            ipc.send_authorized(
                &tasks,
                &sigils,
                sender,
                receiver_reference,
                endpoint,
                payload(b"local")
            ),
            Ok(0),
            "bits name the sender's independently installed local binding"
        );
        tasks.exit(sender).expect("exit sender");
        tasks.start(receiver).expect("start receiver");
        assert!(
            ipc.receive_authorized_with(
                &tasks,
                &sigils,
                receiver,
                sender_reference,
                endpoint,
                |_| true,
                |_| {},
            )
            .is_ok(),
            "the same bits name the receiver's independent local binding"
        );
    }

    #[test]
    fn malformed_message_fields_and_sequence_topology_fail_closed() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let receiver = add_task(
            &mut circles,
            &mut tasks,
            30,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            10,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut ipc = control_ipc_table::new();
        let endpoint = ipc
            .create_endpoint(&mut circles, &tasks, receiver)
            .expect("endpoint");
        tasks.start(sender).expect("start sender");
        ipc.send(&tasks, sender, endpoint, payload(b"x"))
            .expect("send");
        let valid = ipc.endpoints[endpoint.slot()];

        ipc.endpoints[endpoint.slot()].slots[0].sequence = 9;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].slots[0].principal = 0;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].slots[0].task_generation = 0;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].slots[0].origin = 99;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].slots[0].payload_length = (ipc_payload_capacity + 1) as u64;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].slots[0].payload_bytes[1] = 1;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].slots[0].occupied = 2;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].next_sequence = 0;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].sequence_exhausted = 1;
        ipc.endpoints[endpoint.slot()].slots[0].sequence = u64::MAX;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].receiver_principal = 0;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::corrupt_slot));
        ipc.endpoints[endpoint.slot()] = valid;
        ipc.endpoints[endpoint.slot()].generation = 2;
        assert_eq!(ipc.snapshot(endpoint), Err(ipc_error::stale_endpoint));
    }
}
