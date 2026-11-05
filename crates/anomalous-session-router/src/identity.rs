use core::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum identity_error {
    zero_slot,
    zero_generation,
    generation_exhausted,
}

impl identity_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::zero_slot => "zero_slot",
            Self::zero_generation => "zero_generation",
            Self::generation_exhausted => "generation_exhausted",
        }
    }
}

impl fmt::Debug for identity_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for identity_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

macro_rules! generational_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, PartialEq)]
        pub struct $name {
            slot: u64,
            generation: u64,
        }

        impl $name {
            pub const fn try_new(slot: u64, generation: u64) -> Result<Self, identity_error> {
                if slot == 0 {
                    return Err(identity_error::zero_slot);
                }
                if generation == 0 {
                    return Err(identity_error::zero_generation);
                }
                Ok(Self { slot, generation })
            }

            #[must_use]
            pub const fn slot(self) -> u64 {
                self.slot
            }

            #[must_use]
            pub const fn generation(self) -> u64 {
                self.generation
            }

            pub const fn next_generation(self) -> Result<Self, identity_error> {
                let Some(generation) = self.generation.checked_add(1) else {
                    return Err(identity_error::generation_exhausted);
                };
                Ok(Self {
                    slot: self.slot,
                    generation,
                })
            }
        }
    };
}

generational_identity!(router_identity);
generational_identity!(principal_identity);
generational_identity!(source_identity);

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct focus_identity(u64);

impl focus_identity {
    pub const fn try_new(generation: u64) -> Result<Self, identity_error> {
        if generation == 0 {
            return Err(identity_error::zero_generation);
        }
        Ok(Self(generation))
    }

    #[must_use]
    pub const fn generation(self) -> u64 {
        self.0
    }

    pub const fn next_generation(self) -> Result<Self, identity_error> {
        let Some(generation) = self.0.checked_add(1) else {
            return Err(identity_error::generation_exhausted);
        };
        Ok(Self(generation))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct request_sequence(u64);

impl request_sequence {
    pub const fn try_new(value: u64) -> Result<Self, identity_error> {
        if value == 0 {
            return Err(identity_error::zero_generation);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        focus_identity, identity_error, principal_identity, request_sequence, router_identity,
        source_identity,
    };

    #[test]
    fn identities_reject_zero_parts() {
        assert!(matches!(
            router_identity::try_new(0, 1),
            Err(identity_error::zero_slot)
        ));
        assert!(matches!(
            principal_identity::try_new(1, 0),
            Err(identity_error::zero_generation)
        ));
        assert!(matches!(
            source_identity::try_new(0, 0),
            Err(identity_error::zero_slot)
        ));
        assert!(matches!(
            focus_identity::try_new(0),
            Err(identity_error::zero_generation)
        ));
        assert!(matches!(
            request_sequence::try_new(0),
            Err(identity_error::zero_generation)
        ));
    }

    #[test]
    fn identity_generations_never_wrap() {
        let router = router_identity::try_new(1, u64::MAX).expect("valid router");
        let principal = principal_identity::try_new(2, u64::MAX).expect("valid principal");
        let source = source_identity::try_new(3, u64::MAX).expect("valid source");
        let focus = focus_identity::try_new(u64::MAX).expect("valid focus");

        assert!(matches!(
            router.next_generation(),
            Err(identity_error::generation_exhausted)
        ));
        assert!(matches!(
            principal.next_generation(),
            Err(identity_error::generation_exhausted)
        ));
        assert!(matches!(
            source.next_generation(),
            Err(identity_error::generation_exhausted)
        ));
        assert!(matches!(
            focus.next_generation(),
            Err(identity_error::generation_exhausted)
        ));
    }
}
