#[path = "ps2_keyboard.rs"]
mod keyboard;

use crate::event::{
    event_channel, event_delivery, event_drop, event_publication, event_sequence, event_source,
    event_terminal_reason,
};
use crate::time::monotonic_instant;
use core::cell::UnsafeCell;
use keyboard::scan_set_1_decoder;

#[cfg(any(
    test,
    feature = "interrupt_test",
    feature = "desktop_dev",
    feature = "workspace_dev",
    all(
        feature = "native_interactive_editor_test",
        not(any(
            feature = "native_durable_failure_test",
            feature = "native_relaunch_test",
            feature = "elf_application_test"
        ))
    )
))]
pub(crate) use keyboard::key_state;
pub(crate) use keyboard::{
    bootstrap_input_decoder, bootstrap_input_delivery, key_event, physical_input_delivery,
    physical_key_transition,
};

/// Fixed capacity of the kernel-private bootstrap PS/2 source channel.
pub(crate) const ps2_queue_capacity: usize = 64;
/// Generation projected in privacy-safe input readiness telemetry.
pub(crate) const ps2_source_generation: u32 = 1;

const ps2_source_slot: u32 = 1;
const ps2_source: event_source = ps2_event_source(ps2_source_generation);
static ps2_samples: interrupt_ps2_state = interrupt_ps2_state::new();

const fn ps2_event_source(generation: u32) -> event_source {
    event_source::new(ps2_source_slot, generation)
}

/// One opaque byte accepted from a PS/2 scan-set-1 transport.
///
/// This type deliberately has no `Debug` or display implementation. Keyboard
/// samples and all values derived from them are private content, never
/// telemetry. Only aggregate delivery loss may cross an observability boundary.
#[derive(Clone, Copy, Eq, PartialEq)]
struct ps2_sample(u8);

impl ps2_sample {
    const fn new(scan_byte: u8) -> Self {
        Self(scan_byte)
    }

    const fn scan_byte(self) -> u8 {
        self.0
    }
}

/// Controller condition that invalidated or displaced a keyboard sample.
///
/// The reason is safe aggregate state; the rejected byte itself must not be
/// logged or otherwise projected into telemetry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ps2_discontinuity {
    controller_error,
    auxiliary_data,
}

/// Immediate result of accepting one byte from the scan-set-1 transport.
///
/// Prefix and valid Pause bytes are accepted without allocating a physical
/// event identity. Only a completed physical transition is published. A
/// `lost` result means the architecture must contain IRQ1 until the consumer
/// drains and acknowledges the explicit loss boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ps2_sample_publication {
    accepted_without_transition,
    published(event_sequence),
    lost(event_drop),
    terminal(event_terminal_reason),
}

impl ps2_sample_publication {
    const fn from_event(publication: event_publication) -> Self {
        match publication {
            event_publication::published(sequence) => Self::published(sequence),
            event_publication::lost(drop) => Self::lost(drop),
        }
    }

    #[must_use]
    pub(crate) const fn is_lost(self) -> bool {
        matches!(self, Self::lost(_) | Self::terminal(_))
    }

    #[must_use]
    pub(crate) const fn terminal_reason(self) -> Option<event_terminal_reason> {
        match self {
            Self::accepted_without_transition | Self::published(_) => None,
            Self::lost(drop) => drop.terminal_reason(),
            Self::terminal(reason) => Some(reason),
        }
    }
}

/// Decode and publish a controller-validated scan byte from the keyboard ISR.
///
/// # Safety
///
/// Maskable interrupts must be disabled on the boot CPU. No NMI, exception
/// path, or other CPU may access the PS/2 channel until this call returns. The
/// architecture layer must mask IRQ1 after any result for which
/// [`ps2_sample_publication::is_lost`] is true. Valid prefix-only bytes return
/// `accepted_without_transition` and consume no physical event sequence.
pub(crate) unsafe fn publish_ps2_sample(
    scan_byte: u8,
    observed_at: monotonic_instant,
) -> ps2_sample_publication {
    // SAFETY: The caller inherits and must uphold the state's complete
    // interrupt and CPU exclusion contract.
    unsafe {
        ps2_samples.with_exclusive(|state| state.publish(ps2_sample::new(scan_byte), observed_at))
    }
}

/// Record a controller error or non-keyboard byte as an explicit gap.
///
/// # Safety
///
/// The caller must meet the complete exclusion contract documented by
/// [`publish_ps2_sample`]. IRQ1 must remain masked after this call until the
/// consumer observes and resets at the loss boundary.
pub(crate) unsafe fn record_ps2_discontinuity(
    observed_at: monotonic_instant,
    _reason: ps2_discontinuity,
) -> ps2_sample_publication {
    // SAFETY: The caller inherits and must uphold the channel's complete
    // interrupt and CPU exclusion contract.
    unsafe { ps2_samples.with_exclusive(|state| state.record_discontinuity(observed_at)) }
}

/// Whether the PS/2 consumer can make progress without restoring interrupts.
///
/// # Safety
///
/// The caller must meet the complete exclusion contract documented by
/// [`publish_ps2_sample`]. The result becomes stale when interrupts resume.
#[must_use]
pub(crate) unsafe fn ps2_delivery_ready_if_disabled() -> bool {
    // SAFETY: The caller excludes interrupt and other-CPU mutation.
    unsafe { ps2_samples.with_exclusive(|state| state.delivery_ready()) }
}

/// Pop the next physical transition or explicit loss boundary.
///
/// # Safety
///
/// The caller must meet the complete exclusion contract documented by
/// [`publish_ps2_sample`]. The returned content remains kernel-private and must
/// not be included in telemetry.
pub(crate) unsafe fn poll_physical_input_if_disabled() -> Option<physical_input_delivery> {
    // SAFETY: The caller excludes interrupt and other-CPU mutation.
    unsafe { ps2_samples.with_exclusive(ps2_input_state::pop) }
}

/// Result of installing a resynchronized PS/2 source generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ps2_rearm_status {
    ready(u32),
    pending_delivery,
    terminal(event_terminal_reason),
}

struct ps2_input_state<const capacity: usize> {
    transitions: event_channel<physical_key_transition, capacity>,
    decoder: scan_set_1_decoder,
    source_generation: u32,
    resynchronization_required: bool,
    terminal: Option<event_terminal_reason>,
}

impl<const capacity: usize> ps2_input_state<capacity> {
    const fn new() -> Self {
        Self {
            transitions: event_channel::new(ps2_source),
            decoder: scan_set_1_decoder::new(),
            source_generation: ps2_source_generation,
            resynchronization_required: false,
            terminal: None,
        }
    }

    fn publish(
        &mut self,
        sample: ps2_sample,
        observed_at: monotonic_instant,
    ) -> ps2_sample_publication {
        if let Some(reason) = self.terminal_reason() {
            return ps2_sample_publication::terminal(reason);
        }
        if self.resynchronization_required {
            return ps2_sample_publication::from_event(
                self.transitions.record_source_discontinuity(observed_at),
            );
        }

        let transition = match self.decoder.decode(sample) {
            Ok(None) => return ps2_sample_publication::accepted_without_transition,
            Ok(Some(transition)) => transition,
            Err(_) => {
                self.resynchronization_required = true;
                return ps2_sample_publication::from_event(
                    self.transitions.record_source_discontinuity(observed_at),
                );
            }
        };
        let publication = self.transitions.publish(transition, observed_at);
        if publication.is_lost() {
            self.resynchronization_required = true;
        }
        ps2_sample_publication::from_event(publication)
    }

    fn record_discontinuity(&mut self, observed_at: monotonic_instant) -> ps2_sample_publication {
        if let Some(reason) = self.terminal_reason() {
            return ps2_sample_publication::terminal(reason);
        }
        self.resynchronization_required = true;
        ps2_sample_publication::from_event(
            self.transitions.record_source_discontinuity(observed_at),
        )
    }

    const fn delivery_ready(&self) -> bool {
        self.transitions.delivery_ready()
    }

    fn pop(&mut self) -> Option<physical_input_delivery> {
        match self.transitions.pop()? {
            event_delivery::event(record) => Some(physical_input_delivery::transition(record)),
            event_delivery::loss(loss) => Some(physical_input_delivery::loss(loss)),
        }
    }

    const fn resynchronization_required(&self) -> bool {
        self.resynchronization_required
    }

    const fn terminal_reason(&self) -> Option<event_terminal_reason> {
        match self.terminal {
            Some(reason) => Some(reason),
            None => self.transitions.terminal_reason(),
        }
    }

    fn complete_resynchronization(&mut self) -> ps2_rearm_status {
        if let Some(reason) = self.terminal_reason() {
            return ps2_rearm_status::terminal(reason);
        }
        if self.delivery_ready() {
            return ps2_rearm_status::pending_delivery;
        }
        if !self.resynchronization_required {
            return ps2_rearm_status::ready(self.source_generation);
        }
        let Some(next_generation) = self.source_generation.checked_add(1) else {
            let reason = event_terminal_reason::source_generation_exhausted;
            self.terminal = Some(reason);
            return ps2_rearm_status::terminal(reason);
        };

        self.transitions = event_channel::new(ps2_event_source(next_generation));
        self.source_generation = next_generation;
        self.decoder.reset_after_loss();
        self.resynchronization_required = false;
        ps2_rearm_status::ready(next_generation)
    }
}

struct interrupt_ps2_state {
    state: UnsafeCell<ps2_input_state<ps2_queue_capacity>>,
}

// SAFETY: Every accessor is unsafe and requires the caller to exclude
// interrupt re-entry and all other CPUs. This wrapper provides no locking.
unsafe impl Sync for interrupt_ps2_state {}

impl interrupt_ps2_state {
    const fn new() -> Self {
        Self {
            state: UnsafeCell::new(ps2_input_state::new()),
        }
    }

    /// # Safety
    ///
    /// The caller must hold the complete IF-disabled, same-CPU exclusion
    /// contract documented by [`publish_ps2_sample`].
    unsafe fn with_exclusive<T>(
        &self,
        operation: impl FnOnce(&mut ps2_input_state<ps2_queue_capacity>) -> T,
    ) -> T {
        // SAFETY: The caller guarantees exclusive access to the contained state.
        operation(unsafe { &mut *self.state.get() })
    }
}

/// Install a fresh source generation after complete transport resynchronization.
///
/// # Safety
///
/// Interrupts must remain disabled under the complete exclusion contract from
/// [`publish_ps2_sample`]. Before calling, the consumer must have received the
/// channel's `event_delivery::loss` and reset every stateful interpretation of
/// this source. [`bootstrap_input_decoder::consume`] resets the bootstrap
/// keymap. The architecture must also have completed its bounded controller
/// and device resynchronization while IRQ1 remained masked; this function does
/// not flush hardware bytes. A successful call resets the producer decoder,
/// advances the nonwrapping source generation, and starts its sequence at zero.
/// IRQ1 may be unmasked only after this function returns
/// `ps2_rearm_status::ready`.
pub(crate) unsafe fn complete_ps2_resynchronization_if_disabled() -> ps2_rearm_status {
    // SAFETY: The caller excludes interrupt and other-CPU mutation.
    unsafe { ps2_samples.with_exclusive(ps2_input_state::complete_resynchronization) }
}

/// Whether a known loss still requires consumer reset and acknowledgement.
///
/// # Safety
///
/// The caller must meet the complete exclusion contract documented by
/// [`publish_ps2_sample`].
#[must_use]
pub(crate) unsafe fn ps2_resynchronization_required_if_disabled() -> bool {
    // SAFETY: The caller excludes interrupt and other-CPU mutation.
    unsafe { ps2_samples.with_exclusive(|state| state.resynchronization_required()) }
}

/// Deterministically fill the empty PS/2 queue and cross its loss boundary.
///
/// This feature-only helper publishes opaque scan samples but emits no
/// telemetry. The proof consumer must drain `ps2_queue_capacity` events, then
/// observe one loss record before completing resynchronization.
///
/// # Safety
///
/// The caller must meet the complete exclusion contract documented by
/// [`publish_ps2_sample`], invoke this only once on an empty source, and pass
/// the returned loss through the architecture's ordinary IRQ1 containment.
#[cfg(feature = "interrupt_test")]
pub(crate) unsafe fn force_ps2_capacity_loss_for_test(
    observed_at: monotonic_instant,
) -> ps2_sample_publication {
    // SAFETY: The caller provides uninterrupted exclusive access for the whole
    // deterministic fill operation.
    let mut publication = unsafe { publish_ps2_sample(0x1e, observed_at) };
    for _ in 0..ps2_queue_capacity {
        // SAFETY: The caller provides the same uninterrupted exclusive access.
        publication = unsafe { publish_ps2_sample(0x1e, observed_at) };
    }
    publication
}

#[cfg(test)]
mod tests {
    use super::{
        bootstrap_input_decoder, bootstrap_input_delivery, key_event, key_state,
        physical_input_delivery, ps2_discontinuity, ps2_event_source, ps2_input_state,
        ps2_rearm_status, ps2_sample, ps2_sample_publication, ps2_source,
    };
    use crate::event::{event_channel, event_terminal_reason};
    use crate::time::{monotonic_instant, timebase_id};

    const timebase: timebase_id = timebase_id::new(0, 1);

    fn at(tick: u64) -> monotonic_instant {
        monotonic_instant::new(timebase, tick)
    }

    #[test]
    fn opaque_sample_round_trips_only_inside_input_module() {
        let sample = ps2_sample::new(0x9e);
        assert_eq!(sample.scan_byte(), 0x9e);
        assert_ne!(
            ps2_discontinuity::controller_error,
            ps2_discontinuity::auxiliary_data
        );
    }

    fn assert_accepted_without_transition(publication: ps2_sample_publication) {
        assert!(matches!(
            publication,
            ps2_sample_publication::accepted_without_transition
        ));
        assert!(!publication.is_lost());
        assert!(publication.terminal_reason().is_none());
    }

    fn assert_published(publication: ps2_sample_publication, expected_sequence: u64) {
        assert!(matches!(
            publication,
            ps2_sample_publication::published(sequence)
                if sequence.get() == expected_sequence
        ));
        assert!(!publication.is_lost());
        assert!(publication.terminal_reason().is_none());
    }

    #[test]
    fn aggregate_loss_resets_producer_prefix_and_consumer_modifier_state() {
        let mut state = ps2_input_state::<4>::new();
        let mut input = bootstrap_input_decoder::new();

        assert_published(state.publish(ps2_sample::new(0x2a), at(1)), 0);
        assert_accepted_without_transition(state.publish(ps2_sample::new(0xe0), at(2)));
        assert!(input.consume(state.pop().unwrap()).is_none());

        assert!(state.record_discontinuity(at(3)).is_lost());
        match input.consume(state.pop().unwrap()) {
            Some(bootstrap_input_delivery::loss(loss)) => {
                assert!(loss.includes_source_discontinuity());
            }
            _ => panic!("expected an explicit input loss"),
        }
        assert_eq!(
            state.complete_resynchronization(),
            ps2_rearm_status::ready(2)
        );

        assert_published(state.publish(ps2_sample::new(0x1e), at(4)), 0);
        match input.consume(state.pop().unwrap()) {
            Some(bootstrap_input_delivery::key(record)) => {
                assert_eq!(record.source().generation(), 2);
                assert_eq!(record.sequence().get(), 0);
                assert!(matches!(record.into_payload(), key_event::character(b'a')));
            }
            _ => panic!("loss must clear both extended and shift state"),
        }
    }

    #[test]
    fn transitions_before_loss_are_delivered_before_the_reset_boundary() {
        let mut state = ps2_input_state::<1>::new();
        let mut input = bootstrap_input_decoder::new();
        assert_published(state.publish(ps2_sample::new(0x1e), at(1)), 0);
        assert!(state.publish(ps2_sample::new(0x30), at(2)).is_lost());

        match input.consume(state.pop().unwrap()) {
            Some(bootstrap_input_delivery::key(record)) => {
                assert!(matches!(record.into_payload(), key_event::character(b'a')));
            }
            _ => panic!("expected the older queued key"),
        }
        assert!(matches!(
            input.consume(state.pop().unwrap()),
            Some(bootstrap_input_delivery::loss(_))
        ));
        assert!(state.pop().is_none());
    }

    #[test]
    fn loss_delivery_is_not_mistaken_for_private_content() {
        let mut state = ps2_input_state::<0>::new();
        assert!(state.publish(ps2_sample::new(0x1e), at(1)).is_lost());
        assert!(matches!(
            state.pop(),
            Some(physical_input_delivery::loss(_))
        ));
    }

    #[test]
    fn runtime_loss_requires_drain_reset_and_explicit_rearm() {
        let mut state = ps2_input_state::<2>::new();
        assert_published(state.publish(ps2_sample::new(0x1e), at(0)), 0);
        assert_published(state.publish(ps2_sample::new(0x9e), at(1)), 1);
        assert!(matches!(
            state.publish(ps2_sample::new(0x30), at(2)),
            ps2_sample_publication::lost(drop) if drop.sequence().unwrap().get() == 2
        ));
        assert!(state.resynchronization_required());
        assert_eq!(
            state.complete_resynchronization(),
            ps2_rearm_status::pending_delivery
        );

        // A producer bug or late asserted interrupt cannot cross the loss
        // boundary; it extends the explicit contiguous gap instead.
        assert!(state.publish(ps2_sample::new(0xb0), at(3)).is_lost());

        assert!(matches!(
            state.pop(),
            Some(physical_input_delivery::transition(_))
        ));
        assert!(matches!(
            state.pop(),
            Some(physical_input_delivery::transition(_))
        ));
        let loss = match state.pop().unwrap() {
            physical_input_delivery::loss(loss) => loss,
            physical_input_delivery::transition(_) => {
                panic!("loss must follow older transitions")
            }
        };
        assert_eq!(loss.first_sequence().unwrap().get(), 2);
        assert_eq!(loss.last_sequence().unwrap().get(), 3);
        assert_eq!(loss.lost_count(), 2);
        assert!(loss.includes_capacity_loss());
        assert!(loss.includes_source_discontinuity());

        assert_eq!(
            state.complete_resynchronization(),
            ps2_rearm_status::ready(2)
        );
        assert!(!state.resynchronization_required());
        assert_published(state.publish(ps2_sample::new(0x2e), at(4)), 0);
        match state.pop().unwrap() {
            physical_input_delivery::transition(record) => {
                assert_eq!(record.source().generation(), 2);
                assert_eq!(record.sequence().get(), 0);
            }
            physical_input_delivery::loss(_) => panic!("new generation must accept input"),
        }
    }

    #[test]
    fn physical_queue_preserves_metadata_for_press_and_release_records() {
        let mut state = ps2_input_state::<24>::new();
        for index in 0..24u64 {
            let scan_byte = if index.is_multiple_of(2) { 0x1e } else { 0x9e };
            assert_published(
                state.publish(ps2_sample::new(scan_byte), at(100 + index)),
                index,
            );
        }

        for index in 0..24u64 {
            match state.pop().unwrap() {
                physical_input_delivery::transition(record) => {
                    assert_eq!(record.source(), ps2_source);
                    assert_eq!(record.sequence().get(), index);
                    assert_eq!(record.observed_at().tick(), 100 + index);
                    let expected_state = if index.is_multiple_of(2) {
                        key_state::pressed
                    } else {
                        key_state::released
                    };
                    assert!(record.payload().state() == expected_state);
                }
                physical_input_delivery::loss(_) => {
                    panic!("ordinary make and break samples must remain physical records")
                }
            }
        }
    }

    #[test]
    fn framing_bytes_do_not_create_physical_sequence_gaps() {
        let mut state = ps2_input_state::<4>::new();

        assert_accepted_without_transition(state.publish(ps2_sample::new(0xe0), at(10)));
        assert_published(state.publish(ps2_sample::new(0x1d), at(11)), 0);
        match state.pop().unwrap() {
            physical_input_delivery::transition(record) => {
                assert_eq!(record.sequence().get(), 0);
                assert_eq!(record.observed_at().tick(), 11);
                assert!(record.payload().key().is_extended());
                assert!(record.payload().state() == key_state::pressed);
            }
            physical_input_delivery::loss(_) => panic!("valid E0 framing cannot be loss"),
        }

        for (offset, byte) in [0xe1, 0x1d, 0x45, 0xe1, 0x9d, 0xc5].into_iter().enumerate() {
            assert_accepted_without_transition(
                state.publish(ps2_sample::new(byte), at(20 + offset as u64)),
            );
        }
        assert!(!state.delivery_ready());
        assert_published(state.publish(ps2_sample::new(0x1e), at(30)), 1);
        match state.pop().unwrap() {
            physical_input_delivery::transition(record) => {
                assert_eq!(record.sequence().get(), 1);
                assert!(!record.payload().key().is_extended());
            }
            physical_input_delivery::loss(_) => panic!("exact Pause tail must remain valid"),
        }
    }

    #[test]
    fn print_screen_framing_publishes_one_press_and_one_release() {
        let mut state = ps2_input_state::<2>::new();

        for (tick, byte) in [(1, 0xe0), (2, 0x2a), (3, 0xe0)] {
            assert_accepted_without_transition(state.publish(ps2_sample::new(byte), at(tick)));
        }
        assert_published(state.publish(ps2_sample::new(0x37), at(4)), 0);

        for (tick, byte) in [(5, 0xe0), (6, 0xb7), (7, 0xe0)] {
            assert_accepted_without_transition(state.publish(ps2_sample::new(byte), at(tick)));
        }
        assert_published(state.publish(ps2_sample::new(0xaa), at(8)), 1);

        for (sequence, tick, expected_state) in
            [(0, 4, key_state::pressed), (1, 8, key_state::released)]
        {
            match state.pop().unwrap() {
                physical_input_delivery::transition(record) => {
                    assert_eq!(record.sequence().get(), sequence);
                    assert_eq!(record.observed_at().tick(), tick);
                    assert!(record.payload().key().is_extended());
                    assert_eq!(record.payload().key().make_code(), 0x37);
                    assert!(record.payload().state() == expected_state);
                }
                physical_input_delivery::loss(_) => {
                    panic!("valid Print Screen framing cannot create loss")
                }
            }
        }
    }

    #[test]
    fn malformed_print_screen_tail_is_an_explicit_discontinuity() {
        let mut state = ps2_input_state::<1>::new();
        for (tick, byte) in [(1, 0xe0), (2, 0x2a), (3, 0xe0)] {
            assert_accepted_without_transition(state.publish(ps2_sample::new(byte), at(tick)));
        }
        assert!(state.publish(ps2_sample::new(0xaa), at(4)).is_lost());
        assert!(state.resynchronization_required());
        match state.pop().unwrap() {
            physical_input_delivery::loss(loss) => {
                assert_eq!(loss.first_sequence().unwrap().get(), 0);
                assert_eq!(loss.last_sequence().unwrap().get(), 0);
                assert_eq!(loss.lost_count(), 1);
                assert!(loss.includes_source_discontinuity());
            }
            physical_input_delivery::transition(_) => {
                panic!("synthetic Shift framing cannot become a physical edge")
            }
        }
    }

    #[test]
    fn malformed_pause_tail_is_an_explicit_modifier_safe_loss() {
        let mut state = ps2_input_state::<4>::new();
        let mut input = bootstrap_input_decoder::new();

        assert_published(state.publish(ps2_sample::new(0x2a), at(1)), 0);
        assert_accepted_without_transition(state.publish(ps2_sample::new(0xe1), at(2)));
        assert_accepted_without_transition(state.publish(ps2_sample::new(0x1d), at(3)));
        // 0xaa is a possible left-shift release, but it is invalid at this
        // exact Pause-tail position. The explicit loss below resets any held
        // modifier instead of silently swallowing that release.
        assert!(matches!(
            state.publish(ps2_sample::new(0xaa), at(4)),
            ps2_sample_publication::lost(drop)
                if drop.sequence().unwrap().get() == 1
        ));
        assert!(state.resynchronization_required());

        assert!(input.consume(state.pop().unwrap()).is_none());
        let loss = match input.consume(state.pop().unwrap()) {
            Some(bootstrap_input_delivery::loss(loss)) => loss,
            _ => panic!("malformed Pause tail must deliver a loss"),
        };
        assert_eq!(loss.first_sequence().unwrap().get(), 1);
        assert_eq!(loss.last_sequence().unwrap().get(), 1);
        assert_eq!(loss.lost_count(), 1);
        assert!(loss.includes_source_discontinuity());
        assert!(!loss.includes_capacity_loss());

        assert_eq!(
            state.complete_resynchronization(),
            ps2_rearm_status::ready(2)
        );
        assert_published(state.publish(ps2_sample::new(0x1e), at(5)), 0);
        match input.consume(state.pop().unwrap()) {
            Some(bootstrap_input_delivery::key(record)) => {
                assert!(matches!(record.into_payload(), key_event::character(b'a')));
            }
            _ => panic!("loss must clear the potentially swallowed shift release"),
        }
    }

    #[test]
    fn every_possible_modifier_release_in_pause_framing_is_explicit_loss() {
        for release in [0xaa, 0xb6, 0x9d, 0xb8] {
            let mut state = ps2_input_state::<1>::new();
            assert_accepted_without_transition(state.publish(ps2_sample::new(0xe1), at(1)));
            assert!(state.publish(ps2_sample::new(release), at(2)).is_lost());
            assert!(state.resynchronization_required());
            match state.pop().unwrap() {
                physical_input_delivery::loss(loss) => {
                    assert_eq!(loss.first_sequence().unwrap().get(), 0);
                    assert_eq!(loss.last_sequence().unwrap().get(), 0);
                    assert_eq!(loss.lost_count(), 1);
                    assert!(loss.includes_source_discontinuity());
                }
                physical_input_delivery::transition(_) => {
                    panic!("malformed Pause framing cannot swallow a modifier release")
                }
            }
        }
    }

    #[test]
    fn protocol_and_device_error_bytes_require_loss_and_rearm() {
        for invalid in super::keyboard::invalid_transport_bytes {
            for prefix in [
                &[][..],
                &[0xe0][..],
                &[0xe1, 0x1d][..],
                &[0xe0, 0x2a, 0xe0][..],
            ] {
                let mut state = ps2_input_state::<2>::new();
                for (index, byte) in prefix.iter().copied().enumerate() {
                    assert_accepted_without_transition(
                        state.publish(ps2_sample::new(byte), at(index as u64 + 1)),
                    );
                }
                let failure_tick = prefix.len() as u64 + 1;
                assert!(matches!(
                    state.publish(ps2_sample::new(invalid), at(failure_tick)),
                    ps2_sample_publication::lost(drop)
                        if drop.sequence().unwrap().get() == 0
                ));
                assert!(state.resynchronization_required());
                match state.pop().unwrap() {
                    physical_input_delivery::loss(loss) => {
                        assert_eq!(loss.first_sequence().unwrap().get(), 0);
                        assert_eq!(loss.last_sequence().unwrap().get(), 0);
                        assert_eq!(loss.lost_count(), 1);
                        assert!(loss.includes_source_discontinuity());
                    }
                    physical_input_delivery::transition(_) => {
                        panic!("protocol responses and device errors cannot become physical keys")
                    }
                }
                assert_eq!(
                    state.complete_resynchronization(),
                    ps2_rearm_status::ready(2)
                );
                assert_published(
                    state.publish(ps2_sample::new(0x1e), at(failure_tick + 1)),
                    0,
                );
                match state.pop().unwrap() {
                    physical_input_delivery::transition(record) => {
                        assert_eq!(record.source().generation(), 2);
                        assert!(!record.payload().key().is_extended());
                    }
                    physical_input_delivery::loss(_) => {
                        panic!("decoder reset must accept new input")
                    }
                }
            }
        }
    }

    #[test]
    fn source_generation_never_wraps_during_resynchronization() {
        let mut state = ps2_input_state::<1>::new();
        state.source_generation = u32::MAX;
        state.transitions = event_channel::new(ps2_event_source(u32::MAX));

        assert!(state.record_discontinuity(at(1)).is_lost());
        match state.pop().unwrap() {
            physical_input_delivery::loss(loss) => {
                assert_eq!(loss.source().generation(), u32::MAX);
            }
            physical_input_delivery::transition(_) => panic!("expected generation-ending loss"),
        }
        assert_eq!(
            state.complete_resynchronization(),
            ps2_rearm_status::terminal(event_terminal_reason::source_generation_exhausted)
        );
        let publication = state.publish(ps2_sample::new(0x1e), at(2));
        assert!(publication.is_lost());
        assert_eq!(
            publication.terminal_reason(),
            Some(event_terminal_reason::source_generation_exhausted)
        );
        assert!(state.resynchronization_required());
    }

    #[test]
    fn completion_without_a_loss_does_not_rotate_generation() {
        let mut state = ps2_input_state::<1>::new();
        assert_eq!(
            state.complete_resynchronization(),
            ps2_rearm_status::ready(1)
        );
        assert_published(state.publish(ps2_sample::new(0x1e), at(1)), 0);
    }
}
