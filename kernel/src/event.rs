use crate::time::monotonic_instant;

/// A kernel-private identity for one producer of typed events.
///
/// A slot may be reused only with a different generation. Consumers must
/// compare both fields; a slot alone is never an identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct event_source {
    slot: u32,
    generation: u32,
}

impl event_source {
    #[must_use]
    pub(crate) const fn new(slot: u32, generation: u32) -> Self {
        assert!(generation != 0, "event source generation must be nonzero");
        Self { slot, generation }
    }

    #[must_use]
    #[allow(dead_code)] // The slot half is consumed by future source registries.
    pub(crate) const fn slot(self) -> u32 {
        self.slot
    }

    #[must_use]
    pub(crate) const fn generation(self) -> u32 {
        self.generation
    }
}

/// A non-repeating sequence number within one [`event_source`] generation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct event_sequence(u64);

impl event_sequence {
    #[must_use]
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

/// One typed event as observed by a kernel-internal source.
///
/// This intentionally is not part of `anomalous-abi`. Stable rites and oracle
/// records must be derived at a later, policy-aware boundary. In particular,
/// input payloads must never be formatted into telemetry from this record.
pub(crate) struct event_record<T> {
    source: event_source,
    sequence: event_sequence,
    observed_at: monotonic_instant,
    payload: T,
}

impl<T> event_record<T> {
    #[must_use]
    #[cfg(any(
        test,
        feature = "interrupt_test",
        feature = "workspace_dev",
        all(
            feature = "native_interactive_editor_test",
            not(feature = "native_durable_failure_test"),
            not(feature = "native_relaunch_test"),
            not(feature = "elf_application_test")
        )
    ))]
    pub(crate) const fn source(&self) -> event_source {
        self.source
    }

    #[must_use]
    #[cfg(any(
        test,
        feature = "interrupt_test",
        feature = "workspace_dev",
        all(
            feature = "native_interactive_editor_test",
            not(feature = "native_durable_failure_test"),
            not(feature = "native_relaunch_test"),
            not(feature = "elf_application_test")
        )
    ))]
    pub(crate) const fn sequence(&self) -> event_sequence {
        self.sequence
    }

    #[must_use]
    #[cfg(any(test, feature = "interrupt_test"))]
    pub(crate) const fn observed_at(&self) -> monotonic_instant {
        self.observed_at
    }

    #[must_use]
    pub(crate) const fn payload(&self) -> &T {
        &self.payload
    }

    #[must_use]
    pub(crate) fn into_payload(self) -> T {
        self.payload
    }

    #[must_use]
    pub(crate) fn replace_payload<U>(self, payload: U) -> event_record<U> {
        event_record {
            source: self.source,
            sequence: self.sequence,
            observed_at: self.observed_at,
            payload,
        }
    }
}

/// Why an individual publication was not placed in its source channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum event_drop_reason {
    capacity,
    loss_barrier,
    source_discontinuity,
    sequence_exhausted,
}

/// Terminal failure of a source generation's bounded accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum event_terminal_reason {
    source_generation_exhausted,
    sequence_exhausted,
    loss_count_exhausted,
}

/// Immediate result returned to an event producer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct event_drop {
    sequence: Option<event_sequence>,
    reason: event_drop_reason,
    terminal: Option<event_terminal_reason>,
}

#[allow(dead_code)] // Producers may inspect detail beyond today's mask-on-loss policy.
impl event_drop {
    #[must_use]
    pub(crate) const fn sequence(self) -> Option<event_sequence> {
        self.sequence
    }

    #[must_use]
    pub(crate) const fn reason(self) -> event_drop_reason {
        self.reason
    }

    #[must_use]
    pub(crate) const fn terminal_reason(self) -> Option<event_terminal_reason> {
        self.terminal
    }
}

/// Immediate publication status. A `lost` result requires the source owner to
/// apply its fail-closed policy; the eventual consumer receives the aggregate
/// [`event_loss`] record in FIFO order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum event_publication {
    published(event_sequence),
    lost(event_drop),
}

impl event_publication {
    #[must_use]
    pub(crate) const fn is_lost(self) -> bool {
        matches!(self, Self::lost(_))
    }
}

/// An explicit, aggregate discontinuity in a single source's sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct event_loss {
    source: event_source,
    lost_count: u64,
    count_saturated: bool,
    first_sequence: Option<event_sequence>,
    last_sequence: Option<event_sequence>,
    first_observed_at: monotonic_instant,
    last_observed_at: monotonic_instant,
    capacity: bool,
    source_discontinuity: bool,
    sequence_exhausted: bool,
    loss_count_exhausted: bool,
}

#[allow(dead_code)] // The complete loss contract precedes all oracle projections.
impl event_loss {
    #[must_use]
    pub(crate) const fn source(self) -> event_source {
        self.source
    }

    #[must_use]
    pub(crate) const fn lost_count(self) -> u64 {
        self.lost_count
    }

    #[must_use]
    pub(crate) const fn count_saturated(self) -> bool {
        self.count_saturated
    }

    #[must_use]
    pub(crate) const fn first_sequence(self) -> Option<event_sequence> {
        self.first_sequence
    }

    #[must_use]
    pub(crate) const fn last_sequence(self) -> Option<event_sequence> {
        self.last_sequence
    }

    #[must_use]
    pub(crate) const fn first_observed_at(self) -> monotonic_instant {
        self.first_observed_at
    }

    #[must_use]
    pub(crate) const fn last_observed_at(self) -> monotonic_instant {
        self.last_observed_at
    }

    #[must_use]
    pub(crate) const fn includes_capacity_loss(self) -> bool {
        self.capacity
    }

    #[must_use]
    pub(crate) const fn includes_source_discontinuity(self) -> bool {
        self.source_discontinuity
    }

    #[must_use]
    pub(crate) const fn includes_sequence_exhaustion(self) -> bool {
        self.sequence_exhausted
    }

    #[must_use]
    pub(crate) const fn includes_loss_count_exhaustion(self) -> bool {
        self.loss_count_exhausted
    }

    #[must_use]
    pub(crate) const fn terminal_reason(self) -> Option<event_terminal_reason> {
        if self.loss_count_exhausted {
            Some(event_terminal_reason::loss_count_exhausted)
        } else if self.sequence_exhausted {
            Some(event_terminal_reason::sequence_exhausted)
        } else {
            None
        }
    }
}

/// Either the next typed event or an explicit gap before later events.
pub(crate) enum event_delivery<T> {
    event(event_record<T>),
    loss(event_loss),
}

#[derive(Clone, Copy)]
struct sequence_cursor {
    next: u64,
    exhausted: bool,
}

impl sequence_cursor {
    const fn new() -> Self {
        Self {
            next: 0,
            exhausted: false,
        }
    }

    fn reserve(&mut self) -> Option<event_sequence> {
        if self.exhausted {
            return None;
        }

        let sequence = event_sequence(self.next);
        if self.next == u64::MAX {
            self.exhausted = true;
        } else {
            self.next += 1;
        }
        Some(sequence)
    }
}

#[derive(Clone, Copy)]
struct pending_loss {
    lost_count: u64,
    count_saturated: bool,
    first_sequence: Option<event_sequence>,
    last_sequence: Option<event_sequence>,
    first_observed_at: monotonic_instant,
    last_observed_at: monotonic_instant,
    capacity: bool,
    source_discontinuity: bool,
    sequence_exhausted: bool,
    loss_count_exhausted: bool,
}

impl pending_loss {
    const fn first(
        sequence: Option<event_sequence>,
        observed_at: monotonic_instant,
        reason: event_drop_reason,
    ) -> Self {
        Self {
            lost_count: 1,
            count_saturated: false,
            first_sequence: sequence,
            last_sequence: sequence,
            first_observed_at: observed_at,
            last_observed_at: observed_at,
            capacity: matches!(reason, event_drop_reason::capacity),
            source_discontinuity: matches!(reason, event_drop_reason::source_discontinuity),
            sequence_exhausted: matches!(reason, event_drop_reason::sequence_exhausted),
            loss_count_exhausted: false,
        }
    }

    fn add(
        &mut self,
        sequence: Option<event_sequence>,
        observed_at: monotonic_instant,
        reason: event_drop_reason,
    ) {
        if let Some(count) = self.lost_count.checked_add(1) {
            self.lost_count = count;
        } else {
            self.count_saturated = true;
            self.loss_count_exhausted = true;
        }
        if self.first_sequence.is_none() {
            self.first_sequence = sequence;
        }
        if sequence.is_some() {
            self.last_sequence = sequence;
        }
        self.last_observed_at = observed_at;
        // A barrier drop inherits the pending loss's original cause. Treating
        // the barrier itself as capacity loss would falsely relabel a source
        // discontinuity that happened while queue space was still available.
        self.capacity |= matches!(reason, event_drop_reason::capacity);
        self.source_discontinuity |= matches!(reason, event_drop_reason::source_discontinuity);
        self.sequence_exhausted |= matches!(reason, event_drop_reason::sequence_exhausted);
    }

    const fn finish(self, source: event_source) -> event_loss {
        event_loss {
            source,
            lost_count: self.lost_count,
            count_saturated: self.count_saturated,
            first_sequence: self.first_sequence,
            last_sequence: self.last_sequence,
            first_observed_at: self.first_observed_at,
            last_observed_at: self.last_observed_at,
            capacity: self.capacity,
            source_discontinuity: self.source_discontinuity,
            sequence_exhausted: self.sequence_exhausted,
            loss_count_exhausted: self.loss_count_exhausted,
        }
    }
}

/// A bounded, allocation-free FIFO owned by exactly one event source.
///
/// Full queues drop the newest publication. Once any loss occurs, later
/// publications are also dropped until the consumer receives the loss record.
/// That barrier guarantees the observable order is: all older queued events,
/// one aggregate loss record, then any later accepted event.
pub(crate) struct event_channel<T, const capacity: usize> {
    source: event_source,
    slots: [Option<event_record<T>>; capacity],
    head: usize,
    tail: usize,
    length: usize,
    sequence: sequence_cursor,
    pending_loss: Option<pending_loss>,
    terminal: Option<event_terminal_reason>,
}

impl<T, const capacity: usize> event_channel<T, capacity> {
    #[must_use]
    pub(crate) const fn new(source: event_source) -> Self {
        Self {
            source,
            slots: [const { None }; capacity],
            head: 0,
            tail: 0,
            length: 0,
            sequence: sequence_cursor::new(),
            pending_loss: None,
            terminal: None,
        }
    }

    #[must_use]
    pub(crate) const fn delivery_ready(&self) -> bool {
        self.length != 0 || self.pending_loss.is_some()
    }

    #[must_use]
    pub(crate) const fn terminal_reason(&self) -> Option<event_terminal_reason> {
        self.terminal
    }

    pub(crate) fn publish(
        &mut self,
        payload: T,
        observed_at: monotonic_instant,
    ) -> event_publication {
        if let Some(terminal) = self.terminal {
            return event_publication::lost(event_drop {
                sequence: None,
                reason: match terminal {
                    event_terminal_reason::source_generation_exhausted => {
                        event_drop_reason::source_discontinuity
                    }
                    event_terminal_reason::sequence_exhausted => {
                        event_drop_reason::sequence_exhausted
                    }
                    event_terminal_reason::loss_count_exhausted => event_drop_reason::loss_barrier,
                },
                terminal: Some(terminal),
            });
        }
        let sequence = self.sequence.reserve();
        if sequence.is_none() {
            self.add_loss(None, observed_at, event_drop_reason::sequence_exhausted);
            return event_publication::lost(event_drop {
                sequence: None,
                reason: event_drop_reason::sequence_exhausted,
                terminal: Some(event_terminal_reason::sequence_exhausted),
            });
        }

        let sequence = sequence.expect("checked above");
        if self.pending_loss.is_some() {
            self.add_loss(Some(sequence), observed_at, event_drop_reason::loss_barrier);
            return event_publication::lost(event_drop {
                sequence: Some(sequence),
                reason: event_drop_reason::loss_barrier,
                terminal: self.terminal,
            });
        }
        if self.length == capacity {
            self.add_loss(Some(sequence), observed_at, event_drop_reason::capacity);
            return event_publication::lost(event_drop {
                sequence: Some(sequence),
                reason: event_drop_reason::capacity,
                terminal: self.terminal,
            });
        }

        self.slots[self.tail] = Some(event_record {
            source: self.source,
            sequence,
            observed_at,
            payload,
        });
        self.tail += 1;
        if self.tail == capacity {
            self.tail = 0;
        }
        self.length += 1;
        event_publication::published(sequence)
    }

    /// Record that the source transport lost synchronization or data before a
    /// typed payload could be published.
    pub(crate) fn record_source_discontinuity(
        &mut self,
        observed_at: monotonic_instant,
    ) -> event_publication {
        if let Some(terminal) = self.terminal {
            return event_publication::lost(event_drop {
                sequence: None,
                reason: match terminal {
                    event_terminal_reason::source_generation_exhausted => {
                        event_drop_reason::source_discontinuity
                    }
                    event_terminal_reason::sequence_exhausted => {
                        event_drop_reason::sequence_exhausted
                    }
                    event_terminal_reason::loss_count_exhausted => event_drop_reason::loss_barrier,
                },
                terminal: Some(terminal),
            });
        }
        let sequence = self.sequence.reserve();
        let reason = if sequence.is_some() {
            event_drop_reason::source_discontinuity
        } else {
            event_drop_reason::sequence_exhausted
        };
        self.add_loss(sequence, observed_at, reason);
        event_publication::lost(event_drop {
            sequence,
            reason,
            terminal: self.terminal,
        })
    }

    pub(crate) fn pop(&mut self) -> Option<event_delivery<T>> {
        if self.length != 0 {
            let record = self.slots[self.head]
                .take()
                .expect("a queued slot must contain its event");
            self.head += 1;
            if self.head == capacity {
                self.head = 0;
            }
            self.length -= 1;
            return Some(event_delivery::event(record));
        }

        self.pending_loss
            .take()
            .map(|loss| event_delivery::loss(loss.finish(self.source)))
    }

    fn add_loss(
        &mut self,
        sequence: Option<event_sequence>,
        observed_at: monotonic_instant,
        reason: event_drop_reason,
    ) {
        if let Some(loss) = self.pending_loss.as_mut() {
            loss.add(sequence, observed_at, reason);
        } else {
            self.pending_loss = Some(pending_loss::first(sequence, observed_at, reason));
        }
        if matches!(reason, event_drop_reason::sequence_exhausted) {
            self.terminal = Some(event_terminal_reason::sequence_exhausted);
        }
        if self
            .pending_loss
            .as_ref()
            .is_some_and(|loss| loss.loss_count_exhausted)
        {
            self.terminal = Some(event_terminal_reason::loss_count_exhausted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        event_channel, event_delivery, event_drop_reason, event_publication, event_source,
        event_terminal_reason,
    };
    use crate::time::{monotonic_instant, timebase_id};

    const source: event_source = event_source::new(7, 3);
    const timebase: timebase_id = timebase_id::new(2, 9);

    fn at(tick: u64) -> monotonic_instant {
        monotonic_instant::new(timebase, tick)
    }

    fn event_payload(delivery: event_delivery<u8>) -> u8 {
        match delivery {
            event_delivery::event(record) => record.into_payload(),
            event_delivery::loss(_) => panic!("expected an event"),
        }
    }

    #[test]
    fn source_identity_includes_slot_and_generation() {
        assert_eq!(source.slot(), 7);
        assert_eq!(source.generation(), 3);
        assert_ne!(source, event_source::new(7, 4));
        assert_ne!(source, event_source::new(8, 3));
    }

    #[test]
    #[should_panic(expected = "event source generation must be nonzero")]
    fn zero_source_generation_is_rejected() {
        let _ = event_source::new(1, 0);
    }

    #[test]
    fn channel_preserves_fifo_metadata_across_wraparound() {
        let mut channel = event_channel::<u8, 3>::new(source);
        for (payload, tick) in [(10, 100), (11, 101), (12, 102)] {
            assert!(matches!(
                channel.publish(payload, at(tick)),
                event_publication::published(_)
            ));
        }
        assert_eq!(event_payload(channel.pop().unwrap()), 10);
        assert!(matches!(
            channel.publish(13, at(103)),
            event_publication::published(sequence) if sequence.get() == 3
        ));

        for (payload, sequence, tick) in [(11, 1, 101), (12, 2, 102), (13, 3, 103)] {
            match channel.pop().unwrap() {
                event_delivery::event(record) => {
                    assert_eq!(record.source(), source);
                    assert_eq!(record.sequence().get(), sequence);
                    assert_eq!(record.observed_at().tick(), tick);
                    assert_eq!(record.into_payload(), payload);
                }
                event_delivery::loss(_) => panic!("unexpected loss"),
            }
        }
        assert!(!channel.delivery_ready());
        assert!(channel.pop().is_none());
    }

    #[test]
    fn full_channel_drops_newest_and_places_loss_before_later_events() {
        let mut channel = event_channel::<u8, 2>::new(source);
        channel.publish(1, at(1));
        channel.publish(2, at(2));
        assert!(matches!(
            channel.publish(3, at(3)),
            event_publication::lost(drop)
                if drop.reason() == event_drop_reason::capacity
                    && drop.sequence().unwrap().get() == 2
        ));

        assert_eq!(event_payload(channel.pop().unwrap()), 1);
        assert!(matches!(
            channel.publish(4, at(4)),
            event_publication::lost(drop)
                if drop.reason() == event_drop_reason::loss_barrier
                    && drop.sequence().unwrap().get() == 3
        ));
        assert_eq!(event_payload(channel.pop().unwrap()), 2);

        let loss = match channel.pop().unwrap() {
            event_delivery::loss(loss) => loss,
            event_delivery::event(_) => panic!("expected loss after older events"),
        };
        assert_eq!(loss.source(), source);
        assert_eq!(loss.lost_count(), 2);
        assert!(!loss.count_saturated());
        assert_eq!(loss.first_sequence().unwrap().get(), 2);
        assert_eq!(loss.last_sequence().unwrap().get(), 3);
        assert_eq!(loss.first_observed_at().tick(), 3);
        assert_eq!(loss.last_observed_at().tick(), 4);
        assert!(loss.includes_capacity_loss());
        assert!(!loss.includes_source_discontinuity());
        assert!(!loss.includes_sequence_exhaustion());

        assert!(matches!(
            channel.publish(5, at(5)),
            event_publication::published(sequence) if sequence.get() == 4
        ));
        assert_eq!(event_payload(channel.pop().unwrap()), 5);
    }

    #[test]
    fn source_discontinuity_is_fifo_and_explicit() {
        let mut channel = event_channel::<u8, 2>::new(source);
        channel.publish(1, at(1));
        assert!(channel.record_source_discontinuity(at(2)).is_lost());
        assert!(matches!(
            channel.publish(2, at(3)),
            event_publication::lost(drop)
                if drop.reason() == event_drop_reason::loss_barrier
        ));
        assert_eq!(event_payload(channel.pop().unwrap()), 1);
        match channel.pop().unwrap() {
            event_delivery::loss(loss) => {
                assert_eq!(loss.lost_count(), 2);
                assert!(loss.includes_source_discontinuity());
                assert!(!loss.includes_capacity_loss());
                assert!(!loss.includes_sequence_exhaustion());
            }
            event_delivery::event(_) => panic!("expected a source discontinuity"),
        }
    }

    #[test]
    fn zero_capacity_channel_reports_loss_without_indexing() {
        let mut channel = event_channel::<u8, 0>::new(source);
        assert!(matches!(
            channel.publish(1, at(1)),
            event_publication::lost(drop) if drop.reason() == event_drop_reason::capacity
        ));
        match channel.pop().unwrap() {
            event_delivery::loss(loss) => assert_eq!(loss.lost_count(), 1),
            event_delivery::event(_) => panic!("zero capacity cannot contain an event"),
        }
    }

    #[test]
    fn sequence_exhaustion_never_wraps_or_reuses_an_identity() {
        let mut channel = event_channel::<u8, 2>::new(source);
        channel.sequence.next = u64::MAX;

        assert!(matches!(
            channel.publish(1, at(1)),
            event_publication::published(sequence) if sequence.get() == u64::MAX
        ));
        assert!(matches!(
            channel.publish(2, at(2)),
            event_publication::lost(drop)
                if drop.reason() == event_drop_reason::sequence_exhausted
                    && drop.sequence().is_none()
                    && drop.terminal_reason()
                        == Some(event_terminal_reason::sequence_exhausted)
        ));
        assert_eq!(event_payload(channel.pop().unwrap()), 1);
        match channel.pop().unwrap() {
            event_delivery::loss(loss) => {
                assert!(loss.includes_sequence_exhaustion());
                assert_eq!(
                    loss.terminal_reason(),
                    Some(event_terminal_reason::sequence_exhausted)
                );
                assert!(loss.first_sequence().is_none());
                assert!(loss.last_sequence().is_none());
            }
            event_delivery::event(_) => panic!("expected exhaustion loss"),
        }
    }

    #[test]
    fn loss_count_saturates_instead_of_wrapping() {
        let mut channel = event_channel::<u8, 0>::new(source);
        channel.publish(1, at(1));
        let loss = channel.pending_loss.as_mut().unwrap();
        loss.lost_count = u64::MAX;
        channel.publish(2, at(2));

        assert_eq!(
            channel.terminal_reason(),
            Some(event_terminal_reason::loss_count_exhausted)
        );

        match channel.pop().unwrap() {
            event_delivery::loss(loss) => {
                assert_eq!(loss.lost_count(), u64::MAX);
                assert!(loss.count_saturated());
                assert!(loss.includes_loss_count_exhaustion());
                assert_eq!(
                    loss.terminal_reason(),
                    Some(event_terminal_reason::loss_count_exhausted)
                );
            }
            event_delivery::event(_) => panic!("expected saturated loss"),
        }
    }
}
