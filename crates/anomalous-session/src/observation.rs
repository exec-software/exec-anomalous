use core::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum observation_event {
    preparation,
    publication,
    waiting,
    wake,
    receive,
    close,
}

impl observation_event {
    const fn label(self) -> &'static str {
        match self {
            Self::preparation => "preparation",
            Self::publication => "publication",
            Self::waiting => "waiting",
            Self::wake => "wake",
            Self::receive => "receive",
            Self::close => "close",
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum observation_result {
    accepted,
    rejected,
    ready,
    blocked,
    woken,
    received,
    cancelled,
    closed,
}

impl observation_result {
    const fn label(self) -> &'static str {
        match self {
            Self::accepted => "accepted",
            Self::rejected => "rejected",
            Self::ready => "ready",
            Self::blocked => "blocked",
            Self::woken => "woken",
            Self::received => "received",
            Self::cancelled => "cancelled",
            Self::closed => "closed",
        }
    }
}

/// Content-free observation data suitable for a later policy decision.
///
/// Formatting this value does not authorize emission, transport, or retention.
/// Event cadence, volume, and an external timestamp can still disclose activity.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct privacy_observation {
    event: observation_event,
    result: observation_result,
}

impl privacy_observation {
    #[must_use]
    pub const fn new(event: observation_event, result: observation_result) -> Self {
        Self { event, result }
    }

    #[must_use]
    pub const fn event(self) -> observation_event {
        self.event
    }

    #[must_use]
    pub const fn result(self) -> observation_result {
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

impl fmt::Display for privacy_observation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "event={} result={} content_redacted=true history_persisted=false",
            self.event.label(),
            self.result.label()
        )
    }
}

impl fmt::Debug for privacy_observation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::{observation_event, observation_result, privacy_observation};

    #[test]
    fn every_observation_formats_only_the_closed_safe_shape() {
        let events = [
            observation_event::preparation,
            observation_event::publication,
            observation_event::waiting,
            observation_event::wake,
            observation_event::receive,
            observation_event::close,
        ];
        let results = [
            observation_result::accepted,
            observation_result::rejected,
            observation_result::ready,
            observation_result::blocked,
            observation_result::woken,
            observation_result::received,
            observation_result::cancelled,
            observation_result::closed,
        ];
        let sentinel = "private-observation-sentinel";

        for event in events {
            for result in results {
                let observation = privacy_observation::new(event, result);
                let display = std::format!("{observation}");
                let debug = std::format!("{observation:?}");
                assert!(display == debug);
                assert!(display.contains("content_redacted=true"));
                assert!(display.contains("history_persisted=false"));
                assert!(!display.contains(sentinel));
                assert!(!display.contains("session"));
                assert!(!display.contains("consumer"));
                assert!(!display.contains("origin"));
                assert!(!display.contains("length"));
                assert!(!display.contains("digest"));
                assert!(observation.content_redacted());
                assert!(!observation.history_persisted());
            }
        }
    }
}
