use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

use anomalous_session::{close_reason, close_receipt, consumer_identity, language_version};
use anomalous_session::{
    prepared_wait, publish_receipt, receive_preparation, route_epoch, semantic_action,
    semantic_action_kind, session_error, session_identity, session_submission, shell_session,
    submission_origin, submission_sequence, wait_cancellation, wait_commit, wait_identity,
    wake_acknowledgement,
};

#[cfg(test)]
use anomalous_session::session_revision;

use crate::identity::{
    focus_identity, principal_identity, request_sequence, router_identity, source_identity,
};

const human_source_slot: u64 = 1;
const machine_source_slot: u64 = 2;
const first_generation: u64 = 1;
static next_router_instance: AtomicU64 = AtomicU64::new(first_generation);

#[derive(Clone, Copy, Eq, PartialEq)]
struct router_instance(u64);

fn allocate_router_instance() -> Result<router_instance, router_error> {
    next_router_instance
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(router_instance)
        .map_err(|_| router_error::router_instance_exhausted)
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum route_target {
    primary,
    secondary,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum source_kind {
    human_focus,
    machine_explicit,
}

impl source_kind {
    const fn origin(self) -> submission_origin {
        match self {
            Self::human_focus => submission_origin::human_text,
            Self::machine_explicit => submission_origin::machine_text,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum router_error {
    duplicate_route,
    router_instance_exhausted,
    control_denied,
    source_slot_occupied,
    source_slot_empty,
    source_generation_exhausted,
    source_route_not_retired,
    source_denied,
    focus_already_active,
    focus_inactive,
    focus_denied,
    shared_route_active,
    focus_generation_exhausted,
    invalid_focus_transition,
    target_closed,
    target_not_empty,
    request_sequence_exhausted,
    stale_preparation,
    session_failure(session_error),
    invariant_violation,
}

impl router_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::duplicate_route => "duplicate_route",
            Self::router_instance_exhausted => "router_instance_exhausted",
            Self::control_denied => "control_denied",
            Self::source_slot_occupied => "source_slot_occupied",
            Self::source_slot_empty => "source_slot_empty",
            Self::source_generation_exhausted => "source_generation_exhausted",
            Self::source_route_not_retired => "source_route_not_retired",
            Self::source_denied => "source_denied",
            Self::focus_already_active => "focus_already_active",
            Self::focus_inactive => "focus_inactive",
            Self::focus_denied => "focus_denied",
            Self::shared_route_active => "shared_route_active",
            Self::focus_generation_exhausted => "focus_generation_exhausted",
            Self::invalid_focus_transition => "invalid_focus_transition",
            Self::target_closed => "target_closed",
            Self::target_not_empty => "target_not_empty",
            Self::request_sequence_exhausted => "request_sequence_exhausted",
            Self::stale_preparation => "stale_preparation",
            Self::session_failure(_) => "session_failure",
            Self::invariant_violation => "invariant_violation",
        }
    }

    #[must_use]
    pub const fn session_error(self) -> Option<session_error> {
        match self {
            Self::session_failure(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Debug for router_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for router_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct session_route_descriptor {
    session: session_identity,
    consumer: consumer_identity,
    route: route_epoch,
    language: language_version,
}

impl session_route_descriptor {
    #[must_use]
    pub const fn new(
        session: session_identity,
        consumer: consumer_identity,
        route: route_epoch,
        language: language_version,
    ) -> Self {
        Self {
            session,
            consumer,
            route,
            language,
        }
    }

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
    pub const fn language(self) -> language_version {
        self.language
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct route_stamp {
    target: route_target,
    session: session_identity,
    consumer: consumer_identity,
    route: route_epoch,
}

impl route_stamp {
    fn from_session(target: route_target, session: &shell_session) -> Self {
        Self {
            target,
            session: session.identity(),
            consumer: session.consumer(),
            route: session.route(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct request_cursor {
    next: u64,
    exhausted: bool,
}

impl request_cursor {
    const fn new() -> Self {
        Self {
            next: first_generation,
            exhausted: false,
        }
    }

    fn current(self) -> Option<request_sequence> {
        if self.exhausted {
            return None;
        }
        request_sequence::try_new(self.next).ok()
    }

    fn commit_validated(&mut self) {
        if self.next == u64::MAX {
            self.exhausted = true;
        } else {
            self.next += 1;
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct source_binding {
    identity: source_identity,
    principal: principal_identity,
    kind: source_kind,
    target: Option<route_stamp>,
    requests: request_cursor,
}

struct source_slot {
    slot: u64,
    generation: u64,
    exhausted: bool,
    binding: Option<source_binding>,
    replacement: Option<source_replacement_requirement>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum source_replacement_requirement {
    human_focus_retired,
    machine_route_retired(route_stamp),
}

impl source_slot {
    const fn new(slot: u64) -> Self {
        Self {
            slot,
            generation: first_generation,
            exhausted: false,
            binding: None,
            replacement: None,
        }
    }

    fn admit(
        &mut self,
        instance: router_instance,
        router: router_identity,
        principal: principal_identity,
        kind: source_kind,
        target: Option<route_stamp>,
    ) -> Result<host_verified_source, router_error> {
        if self.binding.is_some() {
            return Err(router_error::source_slot_occupied);
        }
        if self.exhausted {
            return Err(router_error::source_generation_exhausted);
        }
        let identity = source_identity::try_new(self.slot, self.generation)
            .map_err(|_| router_error::invariant_violation)?;
        self.binding = Some(source_binding {
            identity,
            principal,
            kind,
            target,
            requests: request_cursor::new(),
        });
        self.replacement = None;
        Ok(host_verified_source {
            instance,
            router,
            identity,
            principal,
            kind,
        })
    }

    fn binding_for(
        &self,
        instance: router_instance,
        router: router_identity,
        source: &host_verified_source,
    ) -> Result<source_binding, router_error> {
        if source.instance != instance || source.router != router {
            return Err(router_error::source_denied);
        }
        let Some(binding) = self.binding else {
            return Err(router_error::source_denied);
        };
        if source.identity != binding.identity
            || source.principal != binding.principal
            || source.kind != binding.kind
        {
            return Err(router_error::source_denied);
        }
        Ok(binding)
    }

    fn revoke_validated(&mut self) {
        self.replacement = self.binding.and_then(|binding| match binding.kind {
            source_kind::human_focus => Some(source_replacement_requirement::human_focus_retired),
            source_kind::machine_explicit => binding
                .target
                .map(source_replacement_requirement::machine_route_retired),
        });
        self.binding = None;
        if self.generation == u64::MAX {
            self.exhausted = true;
        } else {
            self.generation += 1;
        }
    }
}

/// Move-only proof that the trusted host asserted one exact source binding.
///
/// The router does not verify credentials. Possession means only that the
/// trusted owner admitted this source after its external verification step.
pub struct host_verified_source {
    instance: router_instance,
    router: router_identity,
    identity: source_identity,
    principal: principal_identity,
    kind: source_kind,
}

impl host_verified_source {
    #[must_use]
    pub fn identity(&self) -> source_identity {
        self.identity
    }

    #[must_use]
    pub fn principal(&self) -> principal_identity {
        self.principal
    }

    #[must_use]
    pub fn kind(&self) -> source_kind {
        self.kind
    }
}

/// Move-only authority for the trusted router owner.
pub struct router_control {
    instance: router_instance,
    router: router_identity,
}

pub struct focus_authority {
    instance: router_instance,
    router: router_identity,
    identity: focus_identity,
    stamp: route_stamp,
}

impl focus_authority {
    #[must_use]
    pub fn identity(&self) -> focus_identity {
        self.identity
    }

    #[must_use]
    pub fn target(&self) -> route_target {
        self.stamp.target
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct focus_binding {
    identity: focus_identity,
    stamp: route_stamp,
}

pub struct focus_change_rejection {
    error: router_error,
    authority: focus_authority,
}

impl focus_change_rejection {
    #[must_use]
    pub fn error(&self) -> router_error {
        self.error
    }

    #[must_use]
    pub fn authority(&self) -> &focus_authority {
        &self.authority
    }

    #[must_use]
    pub fn into_authority(self) -> focus_authority {
        self.authority
    }
}

pub struct focus_transfer {
    previous: route_target,
    authority: focus_authority,
    close: close_receipt,
}

impl focus_transfer {
    #[must_use]
    pub fn previous_target(&self) -> route_target {
        self.previous
    }

    #[must_use]
    pub fn authority(&self) -> &focus_authority {
        &self.authority
    }

    #[must_use]
    pub fn close_receipt(&self) -> &close_receipt {
        &self.close
    }

    #[must_use]
    pub fn into_parts(self) -> (focus_authority, close_receipt) {
        (self.authority, self.close)
    }
}

pub struct focus_revocation {
    target: route_target,
    close: close_receipt,
}

impl focus_revocation {
    #[must_use]
    pub fn target(&self) -> route_target {
        self.target
    }

    #[must_use]
    pub fn close_receipt(&self) -> &close_receipt {
        &self.close
    }

    #[must_use]
    pub fn into_close_receipt(self) -> close_receipt {
        self.close
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct routed_stamp {
    instance: router_instance,
    router: router_identity,
    source: source_identity,
    principal: principal_identity,
    kind: source_kind,
    origin: submission_origin,
    request: request_sequence,
    route: route_stamp,
    focus: Option<focus_identity>,
}

pub struct prepared_routed_submission {
    stamp: routed_stamp,
    action: semantic_action,
}

impl prepared_routed_submission {
    #[must_use]
    pub fn source(&self) -> source_identity {
        self.stamp.source
    }

    #[must_use]
    pub fn principal(&self) -> principal_identity {
        self.stamp.principal
    }

    #[must_use]
    pub fn origin(&self) -> submission_origin {
        self.stamp.origin
    }

    #[must_use]
    pub fn request_sequence(&self) -> request_sequence {
        self.stamp.request
    }

    #[must_use]
    pub fn target(&self) -> route_target {
        self.stamp.route.target
    }

    #[must_use]
    pub fn focus(&self) -> Option<focus_identity> {
        self.stamp.focus
    }

    /// Exposes the content-derived action discriminator locally.
    #[must_use]
    pub fn action_kind(&self) -> Option<semantic_action_kind> {
        self.action.kind()
    }

    #[must_use]
    pub fn into_action(self) -> semantic_action {
        self.action
    }
}

pub struct route_prepare_rejection {
    error: router_error,
    action: semantic_action,
}

impl route_prepare_rejection {
    #[must_use]
    pub fn error(&self) -> router_error {
        self.error
    }

    #[must_use]
    pub fn action(&self) -> &semantic_action {
        &self.action
    }

    #[must_use]
    pub fn into_action(self) -> semantic_action {
        self.action
    }
}

pub struct route_publish_rejection {
    error: router_error,
    submission: prepared_routed_submission,
}

impl route_publish_rejection {
    #[must_use]
    pub fn error(&self) -> router_error {
        self.error
    }

    #[must_use]
    pub fn submission(&self) -> &prepared_routed_submission {
        &self.submission
    }

    #[must_use]
    pub fn into_submission(self) -> prepared_routed_submission {
        self.submission
    }

    #[must_use]
    pub fn into_action(self) -> semantic_action {
        self.submission.into_action()
    }
}

pub struct routed_publish_receipt {
    stamp: routed_stamp,
    publish: publish_receipt,
}

impl routed_publish_receipt {
    #[must_use]
    pub fn source(&self) -> source_identity {
        self.stamp.source
    }

    #[must_use]
    pub fn principal(&self) -> principal_identity {
        self.stamp.principal
    }

    #[must_use]
    pub fn origin(&self) -> submission_origin {
        self.stamp.origin
    }

    #[must_use]
    pub fn request_sequence(&self) -> request_sequence {
        self.stamp.request
    }

    #[must_use]
    pub fn target(&self) -> route_target {
        self.stamp.route.target
    }

    #[must_use]
    pub fn focus(&self) -> Option<focus_identity> {
        self.stamp.focus
    }

    #[must_use]
    pub fn submission_sequence(&self) -> submission_sequence {
        self.publish.sequence()
    }

    #[must_use]
    pub fn wake(&self) -> Option<wait_identity> {
        self.publish.wake()
    }
}

#[must_use = "close active routes before dropping the router owner"]
pub struct session_router {
    instance: router_instance,
    identity: router_identity,
    primary: shell_session,
    secondary: shell_session,
    human: source_slot,
    machine: source_slot,
    focus_generation: u64,
    focus_exhausted: bool,
    focus: Option<focus_binding>,
}

impl session_router {
    pub fn new(
        identity: router_identity,
        primary: session_route_descriptor,
        secondary: session_route_descriptor,
    ) -> Result<(Self, router_control), router_error> {
        if primary.session.slot() == secondary.session.slot()
            || primary.consumer.value() == secondary.consumer.value()
        {
            return Err(router_error::duplicate_route);
        }
        let instance = allocate_router_instance()?;
        let router = Self {
            instance,
            identity,
            primary: shell_session::new(
                primary.session,
                primary.consumer,
                primary.route,
                primary.language,
            ),
            secondary: shell_session::new(
                secondary.session,
                secondary.consumer,
                secondary.route,
                secondary.language,
            ),
            human: source_slot::new(human_source_slot),
            machine: source_slot::new(machine_source_slot),
            focus_generation: first_generation,
            focus_exhausted: false,
            focus: None,
        };
        Ok((
            router,
            router_control {
                instance,
                router: identity,
            },
        ))
    }

    #[must_use]
    pub fn identity(&self) -> router_identity {
        self.identity
    }

    #[must_use]
    pub fn session_identity(&self, target: route_target) -> session_identity {
        self.route(target).identity()
    }

    #[must_use]
    pub fn consumer_identity(&self, target: route_target) -> consumer_identity {
        self.route(target).consumer()
    }

    #[must_use]
    pub fn route_epoch(&self, target: route_target) -> route_epoch {
        self.route(target).route()
    }

    #[cfg(test)]
    pub(crate) fn route_revision(&self, target: route_target) -> session_revision {
        self.route(target).revision()
    }

    #[cfg(test)]
    pub(crate) fn route_is_open(&self, target: route_target) -> bool {
        self.route(target).is_open()
    }

    #[cfg(test)]
    pub(crate) fn route_queue_depth(&self, target: route_target) -> u8 {
        self.route(target).queue_depth()
    }

    pub fn admit_human_source(
        &mut self,
        control: &router_control,
        principal: principal_identity,
    ) -> Result<host_verified_source, router_error> {
        self.validate_control(control)?;
        if self.human.replacement.is_some() && self.focus.is_some() {
            return Err(router_error::source_route_not_retired);
        }
        self.human.admit(
            self.instance,
            self.identity,
            principal,
            source_kind::human_focus,
            None,
        )
    }

    pub fn admit_machine_source(
        &mut self,
        control: &router_control,
        principal: principal_identity,
    ) -> Result<host_verified_source, router_error> {
        self.validate_control(control)?;
        let target = route_target::secondary;
        if !self.route(target).is_open() {
            return Err(router_error::target_closed);
        }
        let stamp = self.route_stamp(target);
        if matches!(
            self.machine.replacement,
            Some(source_replacement_requirement::machine_route_retired(previous))
                if previous == stamp
        ) {
            return Err(router_error::source_route_not_retired);
        }
        self.machine.admit(
            self.instance,
            self.identity,
            principal,
            source_kind::machine_explicit,
            Some(stamp),
        )
    }

    pub fn revoke_source(
        &mut self,
        control: &router_control,
        kind: source_kind,
    ) -> Result<(), router_error> {
        self.validate_control(control)?;
        let slot = self.source_slot_mut(kind);
        if slot.binding.is_none() {
            return Err(router_error::source_slot_empty);
        }
        slot.revoke_validated();
        Ok(())
    }

    pub fn activate_focus(
        &mut self,
        control: &router_control,
    ) -> Result<focus_authority, router_error> {
        self.validate_control(control)?;
        if self.focus.is_some() {
            return Err(router_error::focus_already_active);
        }
        if self.focus_exhausted {
            return Err(router_error::focus_generation_exhausted);
        }
        if self.human.binding.is_none() {
            return Err(router_error::source_slot_empty);
        }
        let target = route_target::primary;
        self.validate_focus_target(target)?;
        let identity = focus_identity::try_new(self.focus_generation)
            .map_err(|_| router_error::invariant_violation)?;
        let stamp = self.route_stamp(target);
        self.focus = Some(focus_binding { identity, stamp });
        Ok(focus_authority {
            instance: self.instance,
            router: self.identity,
            identity,
            stamp,
        })
    }

    pub fn transfer_focus(
        &mut self,
        control: &router_control,
        authority: focus_authority,
    ) -> Result<focus_transfer, focus_change_rejection> {
        if let Err(error) = self.validate_control(control) {
            return Err(focus_change_rejection { error, authority });
        }
        let current = match self.validate_focus_authority(&authority) {
            Ok(current) => current,
            Err(error) => return Err(focus_change_rejection { error, authority }),
        };
        if self.human.binding.is_none() {
            return Err(focus_change_rejection {
                error: router_error::source_denied,
                authority,
            });
        }
        if current.stamp.target != route_target::primary {
            return Err(focus_change_rejection {
                error: router_error::invalid_focus_transition,
                authority,
            });
        }
        let target = route_target::secondary;
        if let Err(error) = self.validate_focus_target(target) {
            return Err(focus_change_rejection { error, authority });
        }
        let Some(next_generation) = self.focus_generation.checked_add(1) else {
            return Err(focus_change_rejection {
                error: router_error::focus_generation_exhausted,
                authority,
            });
        };
        let close = match self
            .route_mut(current.stamp.target)
            .close(close_reason::route_replaced)
        {
            Ok(close) => close,
            Err(error) => {
                return Err(focus_change_rejection {
                    error: router_error::session_failure(error),
                    authority,
                });
            }
        };

        let identity = match focus_identity::try_new(next_generation) {
            Ok(identity) => identity,
            Err(_) => unreachable!(),
        };
        let stamp = self.route_stamp(target);
        self.focus_generation = next_generation;
        self.focus = Some(focus_binding { identity, stamp });
        Ok(focus_transfer {
            previous: current.stamp.target,
            authority: focus_authority {
                instance: self.instance,
                router: self.identity,
                identity,
                stamp,
            },
            close,
        })
    }

    pub fn revoke_focus(
        &mut self,
        control: &router_control,
        authority: focus_authority,
    ) -> Result<focus_revocation, focus_change_rejection> {
        if let Err(error) = self.validate_control(control) {
            return Err(focus_change_rejection { error, authority });
        }
        let current = match self.validate_focus_authority(&authority) {
            Ok(current) => current,
            Err(error) => return Err(focus_change_rejection { error, authority }),
        };
        if current.stamp.target == route_target::secondary && self.machine.binding.is_some() {
            return Err(focus_change_rejection {
                error: router_error::shared_route_active,
                authority,
            });
        }
        let close = match self
            .route_mut(current.stamp.target)
            .close(close_reason::route_replaced)
        {
            Ok(close) => close,
            Err(error) => {
                return Err(focus_change_rejection {
                    error: router_error::session_failure(error),
                    authority,
                });
            }
        };
        self.focus = None;
        if self.focus_generation == u64::MAX {
            self.focus_exhausted = true;
        } else {
            self.focus_generation += 1;
        }
        Ok(focus_revocation {
            target: current.stamp.target,
            close,
        })
    }

    pub fn replace_closed_route(
        &mut self,
        control: &router_control,
        target: route_target,
    ) -> Result<(), router_error> {
        self.validate_control(control)?;
        if self
            .focus
            .is_some_and(|binding| binding.stamp.target == target)
        {
            return Err(router_error::focus_denied);
        }
        self.route_mut(target)
            .replace_closed()
            .map_err(router_error::session_failure)
    }

    pub fn retire_unfocused_route(
        &mut self,
        control: &router_control,
        target: route_target,
    ) -> Result<close_receipt, router_error> {
        self.validate_control(control)?;
        if self
            .focus
            .is_some_and(|binding| binding.stamp.target == target)
        {
            return Err(router_error::focus_denied);
        }
        self.route_mut(target)
            .close(close_reason::source_failed)
            .map_err(router_error::session_failure)
    }

    // Returning private ownership on rejection is intentionally allocation-free.
    #[allow(clippy::result_large_err)]
    pub fn prepare_human_submission(
        &self,
        source: &host_verified_source,
        focus: &focus_authority,
        action: semantic_action,
    ) -> Result<prepared_routed_submission, route_prepare_rejection> {
        let binding = match self.human.binding_for(self.instance, self.identity, source) {
            Ok(binding) if binding.kind == source_kind::human_focus => binding,
            Ok(_) | Err(_) => {
                return Err(route_prepare_rejection {
                    error: router_error::source_denied,
                    action,
                });
            }
        };
        let focus = match self.validate_focus_authority(focus) {
            Ok(focus) => focus,
            Err(error) => return Err(route_prepare_rejection { error, action }),
        };
        let Some(request) = binding.requests.current() else {
            return Err(route_prepare_rejection {
                error: router_error::request_sequence_exhausted,
                action,
            });
        };
        Ok(prepared_routed_submission {
            stamp: routed_stamp {
                instance: self.instance,
                router: self.identity,
                source: binding.identity,
                principal: binding.principal,
                kind: binding.kind,
                origin: binding.kind.origin(),
                request,
                route: focus.stamp,
                focus: Some(focus.identity),
            },
            action,
        })
    }

    // Returning private ownership on rejection is intentionally allocation-free.
    #[allow(clippy::result_large_err)]
    pub fn prepare_machine_submission(
        &self,
        source: &host_verified_source,
        action: semantic_action,
    ) -> Result<prepared_routed_submission, route_prepare_rejection> {
        let binding = match self
            .machine
            .binding_for(self.instance, self.identity, source)
        {
            Ok(binding) if binding.kind == source_kind::machine_explicit => binding,
            Ok(_) | Err(_) => {
                return Err(route_prepare_rejection {
                    error: router_error::source_denied,
                    action,
                });
            }
        };
        let Some(route) = binding.target else {
            return Err(route_prepare_rejection {
                error: router_error::invariant_violation,
                action,
            });
        };
        if self.route_stamp(route.target) != route || !self.route(route.target).is_open() {
            return Err(route_prepare_rejection {
                error: router_error::source_denied,
                action,
            });
        }
        let Some(request) = binding.requests.current() else {
            return Err(route_prepare_rejection {
                error: router_error::request_sequence_exhausted,
                action,
            });
        };
        Ok(prepared_routed_submission {
            stamp: routed_stamp {
                instance: self.instance,
                router: self.identity,
                source: binding.identity,
                principal: binding.principal,
                kind: binding.kind,
                origin: binding.kind.origin(),
                request,
                route,
                focus: None,
            },
            action,
        })
    }

    // Returning the complete routed owner on rejection is intentionally
    // allocation-free.
    #[allow(clippy::result_large_err)]
    pub fn publish(
        &mut self,
        submission: prepared_routed_submission,
    ) -> Result<routed_publish_receipt, route_publish_rejection> {
        let prepared_routed_submission { stamp, action } = submission;
        let binding = match self.validate_routed_stamp(stamp) {
            Ok(binding) => binding,
            Err(error) => {
                return Err(route_publish_rejection {
                    error,
                    submission: prepared_routed_submission { stamp, action },
                });
            }
        };
        let Some(current_request) = binding.requests.current() else {
            return Err(route_publish_rejection {
                error: router_error::request_sequence_exhausted,
                submission: prepared_routed_submission { stamp, action },
            });
        };
        if current_request != stamp.request {
            return Err(route_publish_rejection {
                error: router_error::stale_preparation,
                submission: prepared_routed_submission { stamp, action },
            });
        }

        let prepared = match self
            .route(stamp.route.target)
            .prepare_submission(stamp.origin, action)
        {
            Ok(prepared) => prepared,
            Err(rejection) => {
                return Err(route_publish_rejection {
                    error: router_error::session_failure(rejection.error()),
                    submission: prepared_routed_submission {
                        stamp,
                        action: rejection.into_action(),
                    },
                });
            }
        };
        let publish = match self.route_mut(stamp.route.target).publish(prepared) {
            Ok(publish) => publish,
            Err(rejection) => {
                let error = rejection.error();
                let action = rejection.into_submission().into_action();
                return Err(route_publish_rejection {
                    error: router_error::session_failure(error),
                    submission: prepared_routed_submission { stamp, action },
                });
            }
        };

        let slot = self.source_slot_mut(stamp.kind);
        let Some(live) = slot.binding.as_mut() else {
            unreachable!();
        };
        live.requests.commit_validated();
        Ok(routed_publish_receipt { stamp, publish })
    }

    pub fn prepare_receive(
        &self,
        target: route_target,
        consumer: consumer_identity,
    ) -> Result<receive_preparation, session_error> {
        self.route(target).prepare_receive(consumer)
    }

    pub fn commit_wait(
        &mut self,
        target: route_target,
        wait: prepared_wait,
    ) -> Result<wait_commit, session_error> {
        self.route_mut(target).commit_wait(wait)
    }

    pub fn acknowledge_wake(
        &mut self,
        target: route_target,
        consumer: consumer_identity,
        wait: wait_identity,
    ) -> Result<wake_acknowledgement, session_error> {
        self.route_mut(target).acknowledge_wake(consumer, wait)
    }

    pub fn cancel_wait(
        &mut self,
        target: route_target,
        consumer: consumer_identity,
        wait: wait_identity,
    ) -> Result<wait_cancellation, session_error> {
        self.route_mut(target).cancel_wait(consumer, wait)
    }

    pub fn receive(
        &mut self,
        target: route_target,
        consumer: consumer_identity,
    ) -> Result<session_submission, session_error> {
        self.route_mut(target).receive(consumer)
    }

    fn validate_control(&self, control: &router_control) -> Result<(), router_error> {
        if control.instance != self.instance || control.router != self.identity {
            return Err(router_error::control_denied);
        }
        Ok(())
    }

    fn validate_focus_target(&self, target: route_target) -> Result<(), router_error> {
        let route = self.route(target);
        if !route.is_open() {
            return Err(router_error::target_closed);
        }
        if route.queue_depth() != 0 {
            return Err(router_error::target_not_empty);
        }
        Ok(())
    }

    fn validate_focus_authority(
        &self,
        authority: &focus_authority,
    ) -> Result<focus_binding, router_error> {
        if authority.instance != self.instance || authority.router != self.identity {
            return Err(router_error::focus_denied);
        }
        let Some(current) = self.focus else {
            return Err(router_error::focus_inactive);
        };
        if authority.identity != current.identity || authority.stamp != current.stamp {
            return Err(router_error::focus_denied);
        }
        if self.route_stamp(current.stamp.target) != current.stamp
            || !self.route(current.stamp.target).is_open()
        {
            return Err(router_error::focus_denied);
        }
        Ok(current)
    }

    fn validate_routed_stamp(&self, stamp: routed_stamp) -> Result<source_binding, router_error> {
        if stamp.instance != self.instance || stamp.router != self.identity {
            return Err(router_error::source_denied);
        }
        let slot = self.source_slot(stamp.kind);
        let Some(binding) = slot.binding else {
            return Err(router_error::source_denied);
        };
        if binding.identity != stamp.source
            || binding.principal != stamp.principal
            || binding.kind != stamp.kind
            || binding.kind.origin() != stamp.origin
        {
            return Err(router_error::source_denied);
        }
        match stamp.kind {
            source_kind::human_focus => {
                let Some(focus) = stamp.focus else {
                    return Err(router_error::focus_denied);
                };
                let Some(current) = self.focus else {
                    return Err(router_error::focus_inactive);
                };
                if current.identity != focus || current.stamp != stamp.route {
                    return Err(router_error::focus_denied);
                }
            }
            source_kind::machine_explicit => {
                if stamp.focus.is_some() || binding.target != Some(stamp.route) {
                    return Err(router_error::source_denied);
                }
            }
        }
        if self.route_stamp(stamp.route.target) != stamp.route
            || !self.route(stamp.route.target).is_open()
        {
            return Err(router_error::source_denied);
        }
        Ok(binding)
    }

    fn source_slot(&self, kind: source_kind) -> &source_slot {
        match kind {
            source_kind::human_focus => &self.human,
            source_kind::machine_explicit => &self.machine,
        }
    }

    fn source_slot_mut(&mut self, kind: source_kind) -> &mut source_slot {
        match kind {
            source_kind::human_focus => &mut self.human,
            source_kind::machine_explicit => &mut self.machine,
        }
    }

    fn route(&self, target: route_target) -> &shell_session {
        match target {
            route_target::primary => &self.primary,
            route_target::secondary => &self.secondary,
        }
    }

    fn route_mut(&mut self, target: route_target) -> &mut shell_session {
        match target {
            route_target::primary => &mut self.primary,
            route_target::secondary => &mut self.secondary,
        }
    }

    fn route_stamp(&self, target: route_target) -> route_stamp {
        route_stamp::from_session(target, self.route(target))
    }

    #[cfg(test)]
    pub(crate) fn set_request_sequence_for_test(&mut self, kind: source_kind, next: u64) {
        let binding = self
            .source_slot_mut(kind)
            .binding
            .as_mut()
            .expect("active test source");
        binding.requests = request_cursor {
            next,
            exhausted: false,
        };
    }

    #[cfg(test)]
    pub(crate) fn set_focus_generation_for_test(&mut self, generation: u64) {
        self.focus_generation = generation;
        if let Some(focus) = self.focus.as_mut() {
            focus.identity = focus_identity::try_new(generation).expect("nonzero test focus");
        }
    }

    #[cfg(test)]
    pub(crate) fn set_source_generation_for_test(&mut self, kind: source_kind, generation: u64) {
        let slot = self.source_slot_mut(kind);
        assert!(slot.binding.is_none());
        slot.generation = generation;
        slot.exhausted = false;
    }

    #[cfg(test)]
    pub(crate) fn prepare_final_submission_sequence_for_test(
        &mut self,
        target: route_target,
    ) -> Result<(), session_error> {
        self.route_mut(target)
            .prepare_final_submission_sequence_for_kernel_conformance()
    }

    #[cfg(test)]
    pub(crate) fn prepare_close_only_revision_for_test(
        &mut self,
        target: route_target,
    ) -> Result<(), session_error> {
        self.route_mut(target)
            .prepare_close_only_revision_for_kernel_conformance()
    }

    #[cfg(test)]
    pub(crate) fn prepare_exhausted_revision_for_test(
        &mut self,
        target: route_target,
    ) -> Result<(), session_error> {
        self.route_mut(target)
            .prepare_exhausted_revision_for_kernel_conformance()
    }
}
