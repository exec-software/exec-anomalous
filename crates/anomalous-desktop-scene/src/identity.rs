use core::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum identity_error {
    zero_slot,
    zero_generation,
    generation_exhausted,
    zero_revision,
    revision_exhausted,
}

impl identity_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::zero_slot => "zero_slot",
            Self::zero_generation => "zero_generation",
            Self::generation_exhausted => "generation_exhausted",
            Self::zero_revision => "zero_revision",
            Self::revision_exhausted => "revision_exhausted",
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

generational_identity!(scene_identity);
generational_identity!(node_identity);
generational_identity!(action_identity);

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct scene_revision(u64);

impl scene_revision {
    pub const fn try_new(value: u64) -> Result<Self, identity_error> {
        if value == 0 {
            return Err(identity_error::zero_revision);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Result<Self, identity_error> {
        let Some(value) = self.0.checked_add(1) else {
            return Err(identity_error::revision_exhausted);
        };
        Ok(Self(value))
    }
}
