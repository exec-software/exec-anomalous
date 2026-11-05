#![cfg_attr(not(test), allow(dead_code))]

//! Opaque, generational identities for bounded kernel-owned tables.
//!
//! An identity names one table slot during one nonzero generation. Possessing
//! an identity is not authority: the owning table remains responsible for
//! checking that the slot is live and that its generation still matches.

use core::num::NonZeroU64;

macro_rules! define_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub(crate) struct $name {
            slot: usize,
            generation: NonZeroU64,
        }

        impl $name {
            pub(crate) const fn from_parts(slot: usize, generation: NonZeroU64) -> Self {
                Self { slot, generation }
            }

            pub(crate) const fn try_from_parts(slot: usize, generation: u64) -> Option<Self> {
                let Some(generation) = NonZeroU64::new(generation) else {
                    return None;
                };
                Some(Self::from_parts(slot, generation))
            }

            pub(crate) const fn slot(self) -> usize {
                self.slot
            }

            pub(crate) const fn generation(self) -> NonZeroU64 {
                self.generation
            }

            pub(crate) const fn generation_value(self) -> u64 {
                self.generation.get()
            }
        }
    };
}

define_identity!(circle_id);
define_identity!(task_id);
define_identity!(endpoint_id);
define_identity!(sigil_id);
define_identity!(lineage_id);
define_identity!(supervision_id);

/// Immutable pairing used whenever task-local state crosses a kernel boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct task_identity {
    task: task_id,
    circle: circle_id,
}

impl task_identity {
    pub(crate) const fn new(task: task_id, circle: circle_id) -> Self {
        Self { task, circle }
    }

    pub(crate) const fn task(self) -> task_id {
        self.task
    }

    pub(crate) const fn circle(self) -> circle_id {
        self.circle
    }
}

/// Opaque identity for a synthetic principal used only by bounded proofs.
///
/// This value records attribution metadata. It is not authentication and does
/// not grant authority.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct fixture_principal_id(NonZeroU64);

impl fixture_principal_id {
    pub(crate) const fn new(value: u64) -> Option<Self> {
        let Some(value) = NonZeroU64::new(value) else {
            return None;
        };
        Some(Self(value))
    }

    pub(crate) const fn value(self) -> u64 {
        self.0.get()
    }
}

pub(crate) const fn next_generation(current: u64) -> Option<NonZeroU64> {
    let Some(next) = current.checked_add(1) else {
        return None;
    };
    NonZeroU64::new(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! identity_contract {
        ($test_name:ident, $identity:ident) => {
            #[test]
            fn $test_name() {
                assert!($identity::try_from_parts(3, 0).is_none());

                let identity = $identity::try_from_parts(3, 7).expect("nonzero generation");
                assert_eq!(identity.slot(), 3);
                assert_eq!(identity.generation_value(), 7);
                assert_eq!(identity.generation().get(), 7);
                assert_eq!(
                    identity,
                    $identity::from_parts(3, NonZeroU64::new(7).expect("nonzero"))
                );
            }
        };
    }

    identity_contract!(circle_identity_is_opaque_and_nonzero, circle_id);
    identity_contract!(task_identity_is_opaque_and_nonzero, task_id);
    identity_contract!(endpoint_identity_is_opaque_and_nonzero, endpoint_id);
    identity_contract!(sigil_identity_is_opaque_and_nonzero, sigil_id);
    identity_contract!(lineage_identity_is_opaque_and_nonzero, lineage_id);
    identity_contract!(supervision_identity_is_opaque_and_nonzero, supervision_id);

    #[test]
    fn task_identity_keeps_task_and_circle_distinct() {
        let task = task_id::try_from_parts(1, 2).expect("task identity");
        let circle = circle_id::try_from_parts(3, 4).expect("circle identity");
        let identity = task_identity::new(task, circle);

        assert_eq!(identity.task(), task);
        assert_eq!(identity.circle(), circle);
    }

    #[test]
    fn fixture_principal_is_nonzero_metadata() {
        assert!(fixture_principal_id::new(0).is_none());
        assert_eq!(
            fixture_principal_id::new(9)
                .expect("nonzero fixture principal")
                .value(),
            9
        );
    }

    #[test]
    fn generation_advancement_never_wraps_or_produces_zero() {
        assert_eq!(next_generation(0).map(NonZeroU64::get), Some(1));
        assert_eq!(next_generation(41).map(NonZeroU64::get), Some(42));
        assert_eq!(next_generation(u64::MAX), None);
    }
}
