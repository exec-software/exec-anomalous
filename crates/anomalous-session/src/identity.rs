use core::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum identity_error {
    zero_slot,
    zero_generation,
    zero_consumer,
    zero_route_epoch,
    generation_exhausted,
    route_epoch_exhausted,
}

impl identity_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::zero_slot => "zero_slot",
            Self::zero_generation => "zero_generation",
            Self::zero_consumer => "zero_consumer",
            Self::zero_route_epoch => "zero_route_epoch",
            Self::generation_exhausted => "generation_exhausted",
            Self::route_epoch_exhausted => "route_epoch_exhausted",
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

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct session_identity {
    slot: u64,
    generation: u64,
}

impl session_identity {
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

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct consumer_identity {
    value: u64,
    generation: u64,
}

impl consumer_identity {
    pub const fn try_new(value: u64, generation: u64) -> Result<Self, identity_error> {
        if value == 0 {
            return Err(identity_error::zero_consumer);
        }
        if generation == 0 {
            return Err(identity_error::zero_generation);
        }
        Ok(Self { value, generation })
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.value
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
            value: self.value,
            generation,
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct route_epoch(u64);

impl route_epoch {
    pub const fn try_new(value: u64) -> Result<Self, identity_error> {
        if value == 0 {
            return Err(identity_error::zero_route_epoch);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Result<Self, identity_error> {
        let Some(value) = self.0.checked_add(1) else {
            return Err(identity_error::route_epoch_exhausted);
        };
        Ok(Self(value))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct language_version {
    major: u16,
    minor: u16,
}

impl language_version {
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct session_revision(pub(crate) u64);

impl session_revision {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct submission_sequence(pub(crate) u64);

impl submission_sequence {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct wait_identity {
    pub(crate) session: session_identity,
    pub(crate) consumer: consumer_identity,
    pub(crate) route: route_epoch,
    pub(crate) sequence: u64,
}

impl wait_identity {
    #[must_use]
    pub const fn session(self) -> session_identity {
        self.session
    }

    #[must_use]
    pub const fn consumer(self) -> consumer_identity {
        self.consumer
    }

    #[must_use]
    pub const fn route(self) -> route_epoch {
        self.route
    }

    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

#[cfg(test)]
mod tests {
    use super::{consumer_identity, identity_error, route_epoch, session_identity};

    #[test]
    fn identities_require_nonzero_parts() {
        assert!(matches!(
            session_identity::try_new(0, 1),
            Err(identity_error::zero_slot)
        ));
        assert!(matches!(
            session_identity::try_new(1, 0),
            Err(identity_error::zero_generation)
        ));
        assert!(matches!(
            consumer_identity::try_new(0, 1),
            Err(identity_error::zero_consumer)
        ));
        assert!(matches!(
            consumer_identity::try_new(1, 0),
            Err(identity_error::zero_generation)
        ));
        assert!(matches!(
            route_epoch::try_new(0),
            Err(identity_error::zero_route_epoch)
        ));
    }

    #[test]
    fn generations_and_route_epochs_never_wrap() {
        let session = session_identity::try_new(7, u64::MAX).expect("valid session");
        let consumer = consumer_identity::try_new(9, u64::MAX).expect("valid consumer");
        let route = route_epoch::try_new(u64::MAX).expect("valid route");

        assert!(matches!(
            session.next_generation(),
            Err(identity_error::generation_exhausted)
        ));
        assert!(matches!(
            consumer.next_generation(),
            Err(identity_error::generation_exhausted)
        ));
        assert!(matches!(
            route.next(),
            Err(identity_error::route_epoch_exhausted)
        ));
    }

    #[test]
    fn successful_generation_changes_preserve_local_identity() {
        let session = session_identity::try_new(7, 4).expect("valid session");
        let consumer = consumer_identity::try_new(9, 6).expect("valid consumer");
        let route = route_epoch::try_new(8).expect("valid route");

        let next_session = session.next_generation().expect("next session");
        let next_consumer = consumer.next_generation().expect("next consumer");
        let next_route = route.next().expect("next route");
        assert!(next_session.slot() == session.slot());
        assert!(next_session.generation() == 5);
        assert!(next_consumer.value() == consumer.value());
        assert!(next_consumer.generation() == 7);
        assert!(next_route.get() == 9);
    }
}
