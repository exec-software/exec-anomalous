//! Copied editor integration for a trusted workspace transport adapter.
//!
//! This portable layer cannot authenticate an OS receive. The holder of the
//! controller-provisioned port must verify the actual selected task, installed
//! channel incarnation and successful complete checked copy before invoking an
//! `*_checked` method. Packet fields and the supplied identity values are only
//! correlation. No editor session or mutable document exists in this module.
use super::*;
use anomalous_editor_contract::remote_protocol::{
    remote_action, remote_message, remote_outcome, remote_request,
};

/// Immutable correlation installed independently of application packet bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct remote_route_binding {
    connection: u64,
    workspace: [u64; 2],
    editor: [u64; 2],
    action_route: [u64; 2],
    observation_route: [u64; 2],
}
impl remote_route_binding {
    pub fn try_new(
        connection: u64,
        workspace: [u64; 2],
        editor: [u64; 2],
        action_route: [u64; 2],
        observation_route: [u64; 2],
    ) -> Result<Self, bridge_error> {
        if connection == 0
            || [workspace, editor, action_route, observation_route]
                .iter()
                .any(|identity| identity.contains(&0))
            || workspace[0] == editor[0]
            || action_route[0] == observation_route[0]
        {
            return Err(bridge_error::wrong_binding);
        }
        Ok(Self {
            connection,
            workspace,
            editor,
            action_route,
            observation_route,
        })
    }
    pub const fn connection(self) -> u64 {
        self.connection
    }
    pub const fn workspace(self) -> [u64; 2] {
        self.workspace
    }
    pub const fn editor(self) -> [u64; 2] {
        self.editor
    }
    pub const fn action_route(self) -> [u64; 2] {
        self.action_route
    }
    pub const fn observation_route(self) -> [u64; 2] {
        self.observation_route
    }
}

/// Non-cloneable local authority to assert checked receipt from one live route.
/// Provisioning requires desktop control; IDs or decoded ANUI cannot create it.
/// It is not itself proof that a kernel authenticated a task.
///
/// ```compile_fail
/// use anomalous_desktop_workspace::native_bridge::native_remote::remote_editor_port;
/// fn copied<T: Copy>() {}
/// copied::<remote_editor_port>();
/// ```
pub struct remote_editor_port {
    desktop: u64,
    route: remote_route_binding,
    live: bool,
    next_transport: u64,
    latest: Option<[u8; interactive_audit_size]>,
}
impl remote_editor_port {
    pub const fn connection(&self) -> u64 {
        self.route.connection
    }
    pub const fn binding(&self) -> remote_route_binding {
        self.route
    }
    pub const fn audit_bytes(&self) -> Option<&[u8; interactive_audit_size]> {
        self.latest.as_ref()
    }
    fn decode_checked(
        &self,
        binding: remote_route_binding,
        transport_sequence: u64,
        bytes: &[u8],
    ) -> Result<remote_message, bridge_error> {
        if !self.live {
            return Err(bridge_error::retired);
        }
        if binding != self.route {
            return Err(bridge_error::wrong_binding);
        }
        if transport_sequence != self.next_transport {
            return Err(bridge_error::stale_observation);
        }
        self.next_transport
            .checked_add(1)
            .ok_or(bridge_error::sequence_exhausted)?;
        let message =
            remote_message::decode(bytes).map_err(|_| bridge_error::invalid_observation)?;
        if message.connection() != self.route.connection {
            return Err(bridge_error::wrong_binding);
        }
        Ok(message)
    }
    /// Trusted adapter assertion after an actual route-authenticated receive.
    /// Failure leaves sequence, initial observation and capability unchanged.
    pub fn admit_initial_checked(
        &mut self,
        binding: remote_route_binding,
        transport_sequence: u64,
        bytes: &[u8],
    ) -> Result<(), bridge_error> {
        let message = self.decode_checked(binding, transport_sequence, bytes)?;
        if self.latest.is_some() || message.outcome().is_some() || message.observation().closed {
            return Err(bridge_error::invalid_observation);
        }
        self.latest = Some(*message.audit_bytes());
        self.next_transport += 1;
        Ok(())
    }
}

pub struct rejected_remote_binding<S> {
    error: bridge_error,
    port: remote_editor_port,
    frame: native_editor_frame<S>,
}
impl<S> rejected_remote_binding<S> {
    pub const fn error(&self) -> bridge_error {
        self.error
    }
    pub fn into_parts(self) -> (remote_editor_port, native_editor_frame<S>) {
        (self.port, self.frame)
    }
}

struct pending_action {
    id: u64,
    expected: u64,
    action: remote_action,
    origin: client_origin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct remote_action_result {
    pub origin: client_origin,
    /// `save_reserved` describes reservation only, never durable success.
    pub outcome: remote_outcome,
}

/// Sole workspace observation/publication owner. It contains no editor session,
/// save snapshot or document authority. Frames remain in caller-owned storage.
pub struct remote_editor_bridge<S = owned_frame<interactive_frame_byte_count>> {
    inner: native_editor_bridge<S>,
    port: remote_editor_port,
    pending: Option<pending_action>,
}

impl desktop {
    /// Trusted local provisioning, not OS task authentication. The native
    /// adapter must install and verify the corresponding kernel routes itself.
    pub fn provision_remote_editor(
        &self,
        control: &desktop_control,
        route: remote_route_binding,
    ) -> Result<remote_editor_port, bridge_error> {
        self.check_control(control)?;
        Ok(remote_editor_port {
            desktop: self.instance,
            route,
            live: true,
            next_transport: 1,
            latest: None,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn bind_remote_editor<S: native_editor_storage>(
        &mut self,
        control: &desktop_control,
        producer: &client_endpoint,
        window: window_id,
        port: remote_editor_port,
        frame: native_editor_frame<S>,
        _disclosure: editor_disclosure,
    ) -> Result<remote_editor_bridge<S>, rejected_remote_binding<S>> {
        let error = if !port.live {
            Some(bridge_error::retired)
        } else if port.desktop != self.instance {
            Some(bridge_error::wrong_binding)
        } else if port.latest.as_ref() != Some(&frame.audit) {
            Some(bridge_error::observation_mismatch)
        } else {
            None
        };
        if let Some(error) = error {
            return Err(rejected_remote_binding { error, port, frame });
        }
        match self.bind_editor_frame(control, producer, window, frame) {
            Ok(inner) => Ok(remote_editor_bridge {
                inner,
                port,
                pending: None,
            }),
            Err(rejected) => Err(rejected_remote_binding {
                error: rejected.error,
                port,
                frame: rejected.frame,
            }),
        }
    }
}

impl<S: native_editor_storage> remote_editor_bridge<S> {
    fn check_live(&self, model: &desktop) -> Result<(), bridge_error> {
        if !self.port.live {
            return Err(bridge_error::retired);
        }
        if self.port.desktop != model.instance {
            return Err(bridge_error::wrong_binding);
        }
        self.inner.check_binding(model)
    }
    pub const fn binding(&self) -> remote_route_binding {
        self.port.route
    }
    pub const fn window(&self) -> window_id {
        self.inner.window()
    }
    pub fn latest_audit(&self) -> &[u8; interactive_audit_size] {
        self.port
            .latest
            .as_ref()
            .expect("bound remote initial observation")
    }
    pub fn bind_input(
        &self,
        model: &desktop,
        control: &desktop_control,
        client: &client_endpoint,
    ) -> Result<native_editor_input, bridge_error> {
        self.check_live(model)?;
        self.inner.bind_input(model, control, client)
    }
    pub fn observe<'a>(
        &'a self,
        model: &desktop,
        observer: &client_endpoint,
    ) -> Result<interactive_audit<'a>, bridge_error> {
        self.check_live(model)?;
        self.inner.observe(model, observer)
    }
    pub fn presented_frame<'a>(
        &'a self,
        model: &desktop,
        observer: &client_endpoint,
    ) -> Result<frame<'a>, bridge_error> {
        self.check_live(model)?;
        self.inner.presented_frame(model, observer)
    }
    pub fn submit_action(
        &mut self,
        model: &mut desktop,
        input: &native_editor_input,
        expected_revision: u64,
        action: remote_action,
    ) -> Result<remote_request, bridge_error> {
        self.check_live(model)?;
        model.require_action_resolved()?;
        if self.pending.is_some() || self.latest_audit() != &self.inner.displayed.audit {
            return Err(bridge_error::backpressure);
        }
        let origin = self.inner.check_action_context(
            model,
            input.slot,
            input.generation,
            expected_revision,
        )?;
        let request = remote_request::try_new(
            self.port.connection(),
            self.inner.next_action,
            self.inner.displayed.observation().sequence,
            action,
        )
        .map_err(|_| bridge_error::invalid_observation)?;
        let local_action = match action {
            remote_action::edit(action) => native_editor_action::edit(action),
            remote_action::save => native_editor_action::save,
            remote_action::request_close => native_editor_action::request_close,
        };
        let ticket = self
            .inner
            .prepare_action(model, input, expected_revision, local_action)?;
        self.pending = Some(pending_action {
            id: ticket.sequence,
            expected: ticket.observation_sequence,
            action,
            origin,
        });
        model.native_action_fence = Some((self.inner.key.generation, ticket.sequence));
        Ok(request)
    }
    /// Drain a checked copied packet even while publication is fenced or a
    /// desktop candidate is pending. A transport ACK never calls this method.
    /// All validation precedes any sequence, observation or fence mutation.
    pub fn admit_checked_message(
        &mut self,
        model: &mut desktop,
        binding: remote_route_binding,
        transport_sequence: u64,
        bytes: &[u8],
    ) -> Result<Option<remote_action_result>, bridge_error> {
        self.check_live(model)?;
        let message = self
            .port
            .decode_checked(binding, transport_sequence, bytes)?;
        let incoming = message.observation();
        let latest =
            interactive_audit::decode(self.latest_audit()).expect("validated remote observation");
        let result = if let Some(outcome) = message.outcome() {
            let pending = self.pending.as_ref().ok_or(bridge_error::stale_action)?;
            if message.action_id() != pending.id
                || message.expected_sequence() != pending.expected
                || incoming.sequence < pending.expected
                || self.inner.in_flight != Some(pending.id)
                || model.native_action_fence != Some((self.inner.key.generation, pending.id))
                || !matches_outcome(pending.action, outcome)
            {
                return Err(bridge_error::wrong_binding);
            }
            Some(remote_action_result {
                origin: pending.origin,
                outcome,
            })
        } else {
            if incoming.sequence < latest.sequence {
                return Err(bridge_error::stale_observation);
            }
            None
        };
        // Closure is terminal for this connection. An older exact completion
        // may drain, but no later packet can reopen or advance its observation.
        if latest.closed && incoming.sequence > latest.sequence {
            return Err(bridge_error::retired);
        }
        if incoming.sequence == latest.sequence && message.audit_bytes() != self.latest_audit() {
            return Err(bridge_error::observation_mismatch);
        }
        if incoming.sequence > latest.sequence {
            self.port.latest = Some(*message.audit_bytes());
        }
        self.port.next_transport += 1;
        if result.is_some() {
            self.pending = None;
            self.inner.in_flight = None;
            model.native_action_fence = None;
        }
        Ok(result)
    }
    pub fn prepare_update(
        &mut self,
        model: &mut desktop,
        expected_revision: u64,
        frame: native_editor_frame<S>,
    ) -> Result<native_editor_update<S>, rejected_editor_frame<S>> {
        let check = self.check_live(model).and_then(|()| {
            if frame.audit != *self.latest_audit() {
                Err(bridge_error::observation_mismatch)
            } else {
                Ok(())
            }
        });
        if let Err(error) = check {
            return Err(rejected_editor_frame { error, frame });
        }
        self.inner
            .prepare_frame_update(model, expected_revision, frame)
    }
    pub fn candidate<'a>(
        &self,
        model: &'a desktop,
        update: &native_editor_update<S>,
    ) -> Result<render_view<'a>, bridge_error> {
        self.check_live(model)?;
        self.inner.candidate(model, update)
    }
    pub fn begin(
        &self,
        model: &mut desktop,
        control: &desktop_control,
        update: &native_editor_update<S>,
    ) -> Result<(), bridge_error> {
        self.check_live(model)?;
        self.inner.begin(model, control, update)
    }
    pub fn attest_presented(
        &self,
        model: &mut desktop,
        control: &desktop_control,
        update: &native_editor_update<S>,
    ) -> Result<(), bridge_error> {
        self.check_live(model)?;
        self.inner.attest_presented(model, control, update)
    }
    pub fn publish_recycling(
        &mut self,
        model: &mut desktop,
        control: &desktop_control,
        update: native_editor_update<S>,
    ) -> Result<(native_editor_publication, native_editor_frame<S>), rejected_editor_update<S>>
    {
        if let Err(error) = self.check_live(model) {
            return Err(rejected_editor_update { error, update });
        }
        self.inner.publish_recycling(model, control, update)
    }
    pub fn cancel(
        &self,
        model: &mut desktop,
        control: &desktop_control,
        update: native_editor_update<S>,
    ) -> Result<native_editor_frame<S>, rejected_editor_update<S>> {
        self.inner.cancel(model, control, update)
    }
    /// Trusted assertion that the exact installed editor task/route is terminal.
    /// No public packet, timeout or service loss may call this recovery path.
    /// It invalidates the attachment before releasing a matching local fence;
    /// the controller must subsequently publish ordinary window withdrawal.
    pub fn withdraw_terminal(
        &mut self,
        model: &mut desktop,
        control: &desktop_control,
        binding: remote_route_binding,
    ) -> Result<(), bridge_error> {
        model.check_control(control)?;
        if !self.port.live || self.inner.retired {
            return Err(bridge_error::retired);
        }
        // Controller recovery must remain possible after producer revocation;
        // it grants no observation, presentation or fresh action authority.
        if self.port.desktop != model.instance
            || self.inner.key.instance != model.instance
            || self.inner.key.generation != model.native_editor_binding
            || binding != self.port.route
        {
            return Err(bridge_error::wrong_binding);
        }
        // A prepared or begun presentation owns its frame until explicit
        // cancellation or full publication. Terminal recovery must not strand it.
        if model.pending.is_some() {
            return Err(bridge_error::backpressure);
        }
        if let Some(pending) = &self.pending {
            if self.inner.in_flight != Some(pending.id)
                || model.native_action_fence != Some((self.inner.key.generation, pending.id))
            {
                return Err(bridge_error::wrong_binding);
            }
        } else if self.inner.in_flight.is_some() || model.native_action_fence.is_some() {
            return Err(bridge_error::wrong_binding);
        }
        self.port.live = false;
        self.inner.retired = true;
        self.pending = None;
        self.inner.in_flight = None;
        model.native_action_fence = None;
        Ok(())
    }
}

fn matches_outcome(action: remote_action, outcome: remote_outcome) -> bool {
    matches!(
        (action, outcome),
        (
            _,
            remote_outcome::stale
                | remote_outcome::rejected(
                    interactive_editor_error::closed | interactive_editor_error::sequence_exhausted
                )
        ) | (remote_action::edit(_), remote_outcome::edit_applied)
            | (
                remote_action::save,
                remote_outcome::save_reserved
                    | remote_outcome::rejected(interactive_editor_error::opened(_))
            )
            | (
                remote_action::request_close,
                remote_outcome::close_confirmation | remote_outcome::closed
            )
    )
}

#[cfg(test)]
#[path = "native_remote_tests.rs"]
mod tests;
