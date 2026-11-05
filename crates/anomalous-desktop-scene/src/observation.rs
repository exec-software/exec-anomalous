use core::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum scene_observation_event {
    construction,
    preparation,
    commit,
    action_resolution,
    focus_traversal,
    retirement,
}

impl scene_observation_event {
    const fn label(self) -> &'static str {
        match self {
            Self::construction => "construction",
            Self::preparation => "preparation",
            Self::commit => "commit",
            Self::action_resolution => "action_resolution",
            Self::focus_traversal => "focus_traversal",
            Self::retirement => "retirement",
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum scene_observation_result {
    accepted,
    rejected,
    resolved,
    retired,
}

impl scene_observation_result {
    const fn label(self) -> &'static str {
        match self {
            Self::accepted => "accepted",
            Self::rejected => "rejected",
            Self::resolved => "resolved",
            Self::retired => "retired",
        }
    }
}

/// Content-free local observation data suitable for a later policy decision.
///
/// Event and result are caller-selected shape data, not an authentic operation
/// receipt, audit record, or proof that the named event occurred. A future
/// trusted observer must stamp observations at the operation boundary.
///
/// Formatting does not authorize emission, transport, or retention. Cadence
/// and an external timestamp can still disclose activity.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct scene_privacy_observation {
    event: scene_observation_event,
    result: scene_observation_result,
}

impl scene_privacy_observation {
    #[must_use]
    pub const fn new(event: scene_observation_event, result: scene_observation_result) -> Self {
        Self { event, result }
    }

    #[must_use]
    pub const fn event(self) -> scene_observation_event {
        self.event
    }

    #[must_use]
    pub const fn result(self) -> scene_observation_result {
        self.result
    }

    #[must_use]
    pub const fn content_redacted(self) -> bool {
        true
    }

    #[must_use]
    pub const fn history_persisted(self) -> bool {
        false
    }
}

impl fmt::Display for scene_privacy_observation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "event={} result={} content_redacted=true history_persisted=false",
            self.event.label(),
            self.result.label()
        )
    }
}

impl fmt::Debug for scene_privacy_observation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
