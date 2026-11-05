use core::cell::UnsafeCell;

/// Kernel-private identity for one monotonic clock generation.
///
/// Resetting or replacing a clock requires a new generation. A consumer must
/// never order instants from different identities by their numeric tick alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct timebase_id {
    slot: u32,
    generation: u32,
}

impl timebase_id {
    #[must_use]
    pub(crate) const fn new(slot: u32, generation: u32) -> Self {
        assert!(generation != 0, "timebase generation must be nonzero");
        Self { slot, generation }
    }

    #[must_use]
    #[allow(dead_code)] // The slot half is consumed by future timebase registries.
    pub(crate) const fn slot(self) -> u32 {
        self.slot
    }

    #[must_use]
    #[cfg(any(
        test,
        feature = "interrupt_test",
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ))]
    pub(crate) const fn generation(self) -> u32 {
        self.generation
    }
}

/// A coarse monotonic point produced by a specific timebase generation.
///
/// `saturated` means at least one requested advance could not be represented.
/// Once set it remains set, and `tick` remains `u64::MAX`; time never wraps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct monotonic_instant {
    timebase: timebase_id,
    tick: u64,
    saturated: bool,
}

impl monotonic_instant {
    #[must_use]
    pub(crate) const fn new(timebase: timebase_id, tick: u64) -> Self {
        Self {
            timebase,
            tick,
            saturated: false,
        }
    }

    const fn saturated(timebase: timebase_id) -> Self {
        Self {
            timebase,
            tick: u64::MAX,
            saturated: true,
        }
    }

    #[must_use]
    #[cfg(any(
        test,
        feature = "interrupt_test",
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ))]
    pub(crate) const fn timebase(self) -> timebase_id {
        self.timebase
    }

    #[must_use]
    #[cfg(any(
        test,
        feature = "interrupt_test",
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ))]
    pub(crate) const fn tick(self) -> u64 {
        self.tick
    }

    #[must_use]
    pub(crate) const fn is_saturated(self) -> bool {
        self.saturated
    }

    /// Compare two points only when they came from the same timebase.
    #[must_use]
    #[cfg(any(test, feature = "interrupt_test"))]
    pub(crate) fn ticks_since(self, earlier: Self) -> Option<u64> {
        if self.saturated
            || earlier.saturated
            || self.timebase != earlier.timebase
            || self.tick < earlier.tick
        {
            return None;
        }
        Some(self.tick - earlier.tick)
    }
}

/// Allocation-free state for one coarse monotonic timebase.
pub(crate) struct monotonic_clock {
    timebase: timebase_id,
    tick: u64,
    saturated: bool,
}

impl monotonic_clock {
    #[must_use]
    pub(crate) const fn new(timebase: timebase_id) -> Self {
        Self {
            timebase,
            tick: 0,
            saturated: false,
        }
    }

    #[must_use]
    pub(crate) const fn now(&self) -> monotonic_instant {
        if self.saturated {
            monotonic_instant::saturated(self.timebase)
        } else {
            monotonic_instant::new(self.timebase, self.tick)
        }
    }

    /// Advance by a hardware-defined number of coarse ticks.
    ///
    /// Overflow saturates both the value and the failure indicator. A zero
    /// delta observes the clock without changing its saturation state.
    pub(crate) fn advance(&mut self, delta: u64) -> monotonic_instant {
        if delta == 0 || self.saturated {
            return self.now();
        }

        if let Some(tick) = self.tick.checked_add(delta) {
            self.tick = tick;
        } else {
            self.tick = u64::MAX;
            self.saturated = true;
        }
        self.now()
    }
}

/// Interior-mutability wrapper for a boot-CPU interrupt clock.
struct interrupt_clock {
    clock: UnsafeCell<monotonic_clock>,
}

// SAFETY: All accessors are unsafe and require the caller to exclude interrupt
// re-entry and every other CPU. The wrapper itself supplies no synchronization.
unsafe impl Sync for interrupt_clock {}

impl interrupt_clock {
    const fn new(timebase: timebase_id) -> Self {
        Self {
            clock: UnsafeCell::new(monotonic_clock::new(timebase)),
        }
    }

    /// # Safety
    ///
    /// Maskable interrupts must be disabled, no NMI or exception path may
    /// access this clock, and no other CPU may access it until this call ends.
    unsafe fn advance_if_disabled(&self, delta: u64) -> monotonic_instant {
        // SAFETY: The caller guarantees exclusive access to the inner clock.
        unsafe { &mut *self.clock.get() }.advance(delta)
    }

    /// # Safety
    ///
    /// The caller must meet the complete exclusion contract documented by
    /// [`Self::advance_if_disabled`].
    unsafe fn now_if_disabled(&self) -> monotonic_instant {
        // SAFETY: The caller excludes mutation for the duration of this read.
        unsafe { &*self.clock.get() }.now()
    }
}

pub(crate) const boot_timebase_generation: u32 = 1;
const boot_timebase: timebase_id = timebase_id::new(0, boot_timebase_generation);
static coarse_time: interrupt_clock = interrupt_clock::new(boot_timebase);

/// Advance the BSP's coarse monotonic timebase by one timer interrupt.
///
/// # Safety
///
/// This may be called only with maskable interrupts disabled on the boot CPU.
/// No NMI, exception path, or other CPU may call either clock accessor. The
/// architecture interrupt layer owns that exclusion until an SMP clock replaces
/// this bounded bootstrap implementation.
pub(crate) unsafe fn advance_from_interrupt() -> monotonic_instant {
    // SAFETY: The caller inherits and must uphold the clock's exclusion contract.
    unsafe { coarse_time.advance_if_disabled(1) }
}

/// Read the BSP's coarse monotonic time while interrupts remain disabled.
///
/// # Safety
///
/// The caller must meet the complete exclusion contract documented by
/// [`advance_from_interrupt`]. The returned instant remains a valid observation,
/// but may be older than the clock as soon as interrupts are restored.
pub(crate) unsafe fn now_if_disabled() -> monotonic_instant {
    // SAFETY: The caller inherits and must uphold the clock's exclusion contract.
    unsafe { coarse_time.now_if_disabled() }
}

#[cfg(test)]
mod tests {
    use super::{interrupt_clock, monotonic_clock, timebase_id};

    const timebase: timebase_id = timebase_id::new(4, 2);

    #[test]
    fn timebase_identity_includes_slot_and_generation() {
        assert_eq!(timebase.slot(), 4);
        assert_eq!(timebase.generation(), 2);
        assert_ne!(timebase, timebase_id::new(4, 3));
        assert_ne!(timebase, timebase_id::new(5, 2));
    }

    #[test]
    #[should_panic(expected = "timebase generation must be nonzero")]
    fn zero_timebase_generation_is_rejected() {
        let _ = timebase_id::new(1, 0);
    }

    #[test]
    fn clock_advances_monotonically_and_zero_is_an_observation() {
        let mut clock = monotonic_clock::new(timebase);
        let origin = clock.now();
        assert_eq!(origin.timebase(), timebase);
        assert_eq!(origin.tick(), 0);
        assert!(!origin.is_saturated());

        let third = clock.advance(3);
        assert_eq!(third.tick(), 3);
        assert_eq!(third.ticks_since(origin), Some(3));
        assert_eq!(clock.advance(0), third);
    }

    #[test]
    fn instants_from_different_generations_are_not_comparable() {
        let first = monotonic_clock::new(timebase).now();
        let second = monotonic_clock::new(timebase_id::new(4, 3)).now();
        assert_eq!(first.ticks_since(second), None);
        assert_eq!(second.ticks_since(first), None);
    }

    #[test]
    fn reversed_instants_do_not_underflow() {
        let mut clock = monotonic_clock::new(timebase);
        let earlier = clock.advance(4);
        let later = clock.advance(2);
        assert_eq!(earlier.ticks_since(later), None);
        assert_eq!(later.ticks_since(earlier), Some(2));
    }

    #[test]
    fn overflow_saturates_and_never_wraps() {
        let mut clock = monotonic_clock::new(timebase);
        clock.tick = u64::MAX - 1;
        let maximum = clock.advance(1);
        assert_eq!(maximum.tick(), u64::MAX);
        assert!(!maximum.is_saturated());

        let saturated = clock.advance(1);
        assert_eq!(saturated.tick(), u64::MAX);
        assert!(saturated.is_saturated());
        assert_eq!(saturated.ticks_since(maximum), None);
        assert_eq!(clock.advance(u64::MAX), saturated);
        assert_eq!(clock.advance(0), saturated);
    }

    #[test]
    fn large_delta_saturates_without_partial_wrap() {
        let mut clock = monotonic_clock::new(timebase);
        clock.tick = 10;
        let saturated = clock.advance(u64::MAX);
        assert_eq!(saturated.tick(), u64::MAX);
        assert!(saturated.is_saturated());
    }

    #[test]
    fn interrupt_wrapper_serializes_under_caller_exclusion() {
        let clock = interrupt_clock::new(timebase);
        // SAFETY: This local clock is inaccessible to interrupt and other CPU
        // contexts for the complete test.
        unsafe {
            assert_eq!(clock.now_if_disabled().tick(), 0);
            assert_eq!(clock.advance_if_disabled(1).tick(), 1);
            assert_eq!(clock.now_if_disabled().tick(), 1);
        }
    }
}
