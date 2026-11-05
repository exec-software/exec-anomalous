//! Three-role trusted-local coordination for the portable native editor bridge.
//!
//! Numeric task bindings are descriptive inputs from the trusted controller, not
//! OS authentication. Private, non-cloneable role owners enforce this local model.
//! No task dispatch, transport, device grant or durable acknowledgement is created
//! here. The future native adapter must authenticate the corresponding live tasks.

use super::*;
use core::sync::atomic::{AtomicU64, Ordering};

static next_coordinator: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum coordinator_error {
    desktop(desktop_error),
    invalid_binding,
    duplicate_task,
    instance_exhausted,
    wrong_owner,
    wrong_role,
    retired,
    route_closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum native_role {
    workspace,
    editor,
    document,
}
impl native_role {
    const fn index(self) -> usize {
        match self {
            Self::workspace => 0,
            Self::editor => 1,
            Self::document => 2,
        }
    }
    const all: [Self; 3] = [Self::workspace, Self::editor, Self::document];
}

/// Correlation supplied by the local controller; possession confers no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct native_role_binding {
    slot: u64,
    generation: u64,
}
impl native_role_binding {
    pub const fn try_new(slot: u64, generation: u64) -> Result<Self, coordinator_error> {
        if generation == 0 {
            return Err(coordinator_error::invalid_binding);
        }
        Ok(Self { slot, generation })
    }
    pub const fn slot(self) -> u64 {
        self.slot
    }
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// A role capability is provisioned once and cannot be reconstructed from its ID.
///
/// ```compile_fail
/// use anomalous_desktop_workspace::native_bridge::native_coordinator::native_role_owner;
/// fn copied<T: Copy>() {}
/// copied::<native_role_owner>();
/// ```
pub struct native_role_owner {
    instance: u64,
    role: native_role,
    binding: native_role_binding,
}
impl native_role_owner {
    pub const fn role(&self) -> native_role {
        self.role
    }
    pub const fn binding(&self) -> native_role_binding {
        self.binding
    }
}
pub struct native_role_owners {
    pub workspace: native_role_owner,
    pub editor: native_role_owner,
    pub document: native_role_owner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum native_route {
    editor_action,
    editor_observation,
    document_request,
    document_reply,
}
impl native_route {
    pub const all: [Self; 4] = [
        Self::editor_action,
        Self::editor_observation,
        Self::document_request,
        Self::document_reply,
    ];
    pub const fn endpoints(self) -> (native_role, native_role) {
        match self {
            Self::editor_action => (native_role::workspace, native_role::editor),
            Self::editor_observation => (native_role::editor, native_role::workspace),
            Self::document_request => (native_role::editor, native_role::document),
            Self::document_reply => (native_role::document, native_role::editor),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum native_route_access {
    send,
    receive,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct native_retirement {
    pub role: native_role,
    /// In native_route::all order; only routes closed by this retirement.
    pub closed_routes: [bool; 4],
}

/// Explicit provisioned membership and fair ready-role rotation. Pending editor
/// work does not block the workspace role: it must remain able to pump completion.
/// Retirement is permanent; replacement requires fresh provisioned capabilities.
pub struct native_coordinator {
    instance: u64,
    desktop_instance: u64,
    bindings: [native_role_binding; 3],
    live: [bool; 3],
    runnable: [bool; 3],
    next: usize,
}
impl native_coordinator {
    pub fn provision(
        model: &desktop,
        control: &desktop_control,
        bindings: [native_role_binding; 3],
    ) -> Result<(Self, native_role_owners), coordinator_error> {
        model
            .check_control(control)
            .map_err(coordinator_error::desktop)?;
        for index in 0..3 {
            if bindings[index].generation == 0 {
                return Err(coordinator_error::invalid_binding);
            }
            if bindings[..index]
                .iter()
                .any(|other| other.slot == bindings[index].slot)
            {
                return Err(coordinator_error::duplicate_task);
            }
        }
        let instance = next_coordinator
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| coordinator_error::instance_exhausted)?;
        let owner = |role: native_role| native_role_owner {
            instance,
            role,
            binding: bindings[role.index()],
        };
        Ok((
            Self {
                instance,
                desktop_instance: model.instance,
                bindings,
                live: [true; 3],
                runnable: [true; 3],
                next: 0,
            },
            native_role_owners {
                workspace: owner(native_role::workspace),
                editor: owner(native_role::editor),
                document: owner(native_role::document),
            },
        ))
    }
    fn check_identity(&self, owner: &native_role_owner) -> Result<(), coordinator_error> {
        if owner.instance != self.instance || owner.binding != self.bindings[owner.role.index()] {
            return Err(coordinator_error::wrong_owner);
        }
        Ok(())
    }
    fn check_role(
        &self,
        owner: &native_role_owner,
        role: native_role,
    ) -> Result<(), coordinator_error> {
        self.check_identity(owner)?;
        if owner.role != role {
            return Err(coordinator_error::wrong_role);
        }
        Ok(())
    }
    pub fn is_live(&self, owner: &native_role_owner) -> Result<bool, coordinator_error> {
        self.check_identity(owner)?;
        Ok(self.live[owner.role.index()])
    }
    pub fn route_open(&self, route: native_route) -> bool {
        let (producer, consumer) = route.endpoints();
        self.live[producer.index()] && self.live[consumer.index()]
    }
    pub fn authorize_route(
        &self,
        owner: &native_role_owner,
        route: native_route,
        access: native_route_access,
    ) -> Result<(), coordinator_error> {
        self.check_identity(owner)?;
        if !self.live[owner.role.index()] {
            return Err(coordinator_error::retired);
        }
        if !self.route_open(route) {
            return Err(coordinator_error::route_closed);
        }
        let endpoints = route.endpoints();
        let expected = match access {
            native_route_access::send => endpoints.0,
            native_route_access::receive => endpoints.1,
        };
        self.check_role(owner, expected)
    }
    pub fn set_runnable(
        &mut self,
        owner: &native_role_owner,
        runnable: bool,
    ) -> Result<(), coordinator_error> {
        self.check_identity(owner)?;
        if !self.live[owner.role.index()] {
            return Err(coordinator_error::retired);
        }
        self.runnable[owner.role.index()] = runnable;
        Ok(())
    }
    /// Descriptive local scheduling choice; it is not a kernel dispatch permit.
    pub fn next_runnable(&mut self) -> Option<native_role> {
        for offset in 0..3 {
            let index = (self.next + offset) % 3;
            if self.live[index] && self.runnable[index] {
                self.next = (index + 1) % 3;
                return Some(native_role::all[index]);
            }
        }
        None
    }
    pub fn retire(
        &mut self,
        owner: &native_role_owner,
    ) -> Result<native_retirement, coordinator_error> {
        self.check_identity(owner)?;
        if !self.live[owner.role.index()] {
            return Err(coordinator_error::retired);
        }
        let before = native_route::all.map(|route| self.route_open(route));
        self.live[owner.role.index()] = false;
        self.runnable[owner.role.index()] = false;
        Ok(native_retirement {
            role: owner.role,
            closed_routes: core::array::from_fn(|index| {
                before[index] && !self.route_open(native_route::all[index])
            }),
        })
    }
}

fn route_error(error: coordinator_error) -> bridge_error {
    match error {
        coordinator_error::retired | coordinator_error::route_closed => bridge_error::retired,
        coordinator_error::desktop(error) => bridge_error::desktop(error),
        _ => bridge_error::wrong_binding,
    }
}

/// Admitted action plus the exact local route. There is no ACK/timeout release
/// API and no conversion back to the legacy cancellable action ticket. Dropping
/// this owner deliberately leaves the desktop fenced; dropping is not recovery.
#[must_use = "resolve the submitted action or withdraw it with terminal authority"]
pub struct submitted_native_action {
    coordinator: u64,
    workspace: native_role_binding,
    editor: native_role_binding,
    ticket: native_editor_action_ticket,
    origin: client_origin,
}
pub struct rejected_native_submission {
    error: bridge_error,
    submission: submitted_native_action,
}
impl rejected_native_submission {
    pub const fn error(&self) -> bridge_error {
        self.error
    }
    pub fn into_submission(self) -> submitted_native_action {
        self.submission
    }
}

/// An exact executed or rejected outcome. Only this sealed value can complete
/// the asynchronous focus fence. It is not a transport or durable-save receipt.
#[must_use = "complete this exact outcome at its workspace bridge"]
pub struct completed_native_action {
    coordinator: u64,
    workspace: native_role_binding,
    key: binding,
    sequence: u64,
    outcome: Result<native_editor_action_result, bridge_error>,
}
impl completed_native_action {
    pub const fn outcome(&self) -> &Result<native_editor_action_result, bridge_error> {
        &self.outcome
    }
}
pub struct rejected_native_completion {
    error: bridge_error,
    completion: completed_native_action,
}
impl rejected_native_completion {
    pub const fn error(&self) -> bridge_error {
        self.error
    }
    pub fn into_completion(self) -> completed_native_action {
        self.completion
    }
}

impl submitted_native_action {
    pub const fn sequence(&self) -> u64 {
        self.ticket.sequence
    }
    pub fn resolve(
        self,
        coordinator: &native_coordinator,
        editor: &native_role_owner,
        owner: &mut native_editor_owner,
    ) -> Result<completed_native_action, rejected_native_submission> {
        let identity = coordinator
            .check_role(editor, native_role::editor)
            .map_err(route_error);
        if let Err(error) = identity {
            return Err(rejected_native_submission {
                error,
                submission: self,
            });
        }
        if self.coordinator != coordinator.instance
            || self.editor != editor.binding
            || owner.key != Some(self.ticket.key)
        {
            return Err(rejected_native_submission {
                error: bridge_error::wrong_binding,
                submission: self,
            });
        }
        // A workspace may retire after delivering an accepted action. Its prior
        // admission remains valid, but a retired editor can never execute it.
        let outcome = if !coordinator.live[native_role::editor.index()] {
            Err(bridge_error::retired)
        } else if owner.session.view().sequence() != self.ticket.observation_sequence {
            Err(bridge_error::stale_observation)
        } else {
            apply_action(owner, self.ticket.action).map(|effect| native_editor_action_result {
                origin: self.origin,
                effect,
            })
        };
        Ok(completed_native_action {
            coordinator: self.coordinator,
            workspace: self.workspace,
            key: self.ticket.key,
            sequence: self.ticket.sequence,
            outcome,
        })
    }
}
fn apply_action(
    owner: &mut native_editor_owner,
    action: native_editor_action,
) -> Result<native_editor_effect, bridge_error> {
    match action {
        native_editor_action::edit(action) => {
            owner.session.apply(action).map_err(bridge_error::editor)?;
            Ok(native_editor_effect::edited)
        }
        native_editor_action::save => owner
            .session
            .begin_save()
            .map(native_editor_effect::save)
            .map_err(bridge_error::editor),
        native_editor_action::request_close => owner
            .session
            .request_close()
            .map(native_editor_effect::close)
            .map_err(bridge_error::editor),
    }
}

impl<S: native_editor_storage> native_editor_bridge<S> {
    // Keep each authority explicit rather than accepting an unauthenticated ID bundle.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_action(
        &mut self,
        model: &mut desktop,
        coordinator: &native_coordinator,
        workspace: &native_role_owner,
        input: &native_editor_input,
        expected_revision: u64,
        action: native_editor_action,
    ) -> Result<submitted_native_action, bridge_error> {
        coordinator
            .authorize_route(
                workspace,
                native_route::editor_action,
                native_route_access::send,
            )
            .map_err(route_error)?;
        if coordinator.desktop_instance != model.instance {
            return Err(bridge_error::wrong_binding);
        }
        model.require_action_resolved()?;
        let origin =
            self.check_action_context(model, input.slot, input.generation, expected_revision)?;
        let ticket = self.prepare_action(model, input, expected_revision, action)?;
        // All fallible admission checks precede publication of this fence.
        model.native_action_fence = Some((self.key.generation, ticket.sequence));
        Ok(submitted_native_action {
            coordinator: coordinator.instance,
            workspace: workspace.binding,
            editor: coordinator.bindings[native_role::editor.index()],
            ticket,
            origin,
        })
    }
    /// Completion drains an already accepted outcome even if its workspace role
    /// has since retired. It grants no new route and cannot submit another action.
    pub fn complete_action(
        &mut self,
        model: &mut desktop,
        coordinator: &native_coordinator,
        workspace: &native_role_owner,
        completion: completed_native_action,
    ) -> Result<Result<native_editor_action_result, bridge_error>, rejected_native_completion> {
        let check = (|| {
            coordinator
                .check_role(workspace, native_role::workspace)
                .map_err(route_error)?;
            if coordinator.desktop_instance != model.instance
                || completion.coordinator != coordinator.instance
                || completion.workspace != workspace.binding
                || completion.key != self.key
                || self.key.instance != model.instance
                || self.key.generation != model.native_editor_binding
                || self.in_flight != Some(completion.sequence)
                || model.native_action_fence != Some((self.key.generation, completion.sequence))
            {
                return Err(bridge_error::wrong_binding);
            }
            Ok(())
        })();
        if let Err(error) = check {
            return Err(rejected_native_completion { error, completion });
        }
        self.in_flight = None;
        model.native_action_fence = None;
        Ok(completion.outcome)
    }
    /// Trusted terminal recovery consumes the undelivered/unresolved submission,
    /// requires that the provisioned editor is retired, and makes this attachment
    /// unusable before lifting its fence. The controller must separately publish
    /// native window withdrawal. A timeout or packet ACK is insufficient.
    pub fn withdraw_terminal_submission(
        &mut self,
        model: &mut desktop,
        control: &desktop_control,
        coordinator: &native_coordinator,
        submission: submitted_native_action,
    ) -> Result<(), rejected_native_submission> {
        let check = (|| {
            model.check_control(control)?;
            if coordinator.desktop_instance != model.instance
                || submission.coordinator != coordinator.instance
                || submission.editor != coordinator.bindings[native_role::editor.index()]
                || coordinator.live[native_role::editor.index()]
                || submission.ticket.key != self.key
                || self.key.instance != model.instance
                || self.key.generation != model.native_editor_binding
                || self.in_flight != Some(submission.ticket.sequence)
                || model.native_action_fence
                    != Some((self.key.generation, submission.ticket.sequence))
            {
                return Err(bridge_error::wrong_binding);
            }
            Ok(())
        })();
        if let Err(error) = check {
            return Err(rejected_native_submission { error, submission });
        }
        self.retired = true;
        self.in_flight = None;
        model.native_action_fence = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provision(
        model: &desktop,
        control: &desktop_control,
    ) -> (native_coordinator, native_role_owners) {
        native_coordinator::provision(
            model,
            control,
            [
                native_role_binding::try_new(4, 2).unwrap(),
                native_role_binding::try_new(7, 3).unwrap(),
                native_role_binding::try_new(9, 8).unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn routes_are_directional_branded_and_retire_only_their_endpoints() {
        let (model, control, _) = desktop::new(1024, 768).unwrap();
        let (mut coordinator, owners) = provision(&model, &control);
        let (_, foreign) = provision(&model, &control);
        assert_eq!(
            coordinator.authorize_route(
                &foreign.editor,
                native_route::document_request,
                native_route_access::send
            ),
            Err(coordinator_error::wrong_owner)
        );
        for route in native_route::all {
            let (sender, receiver) = route.endpoints();
            for owner in [&owners.workspace, &owners.editor, &owners.document] {
                assert_eq!(
                    coordinator
                        .authorize_route(owner, route, native_route_access::send)
                        .is_ok(),
                    owner.role == sender
                );
                assert_eq!(
                    coordinator
                        .authorize_route(owner, route, native_route_access::receive)
                        .is_ok(),
                    owner.role == receiver
                );
            }
        }
        assert_eq!(
            coordinator.retire(&owners.document).unwrap().closed_routes,
            [false, false, true, true]
        );
        assert!(coordinator.route_open(native_route::editor_action));
        assert!(coordinator.route_open(native_route::editor_observation));
        assert_eq!(
            coordinator.authorize_route(
                &owners.editor,
                native_route::document_request,
                native_route_access::send
            ),
            Err(coordinator_error::route_closed)
        );
        assert_eq!(
            coordinator.set_runnable(&owners.document, true),
            Err(coordinator_error::retired)
        );
        assert_eq!(
            coordinator.retire(&owners.document),
            Err(coordinator_error::retired)
        );
        assert_eq!(
            coordinator.retire(&owners.editor).unwrap().closed_routes,
            [true, true, false, false]
        );
    }

    #[test]
    fn membership_rejects_duplicate_slots_and_wrong_controller() {
        let (model, control, _) = desktop::new(1024, 768).unwrap();
        let (_, other_control, _) = desktop::new(1024, 768).unwrap();
        let a = native_role_binding::try_new(1, 1).unwrap();
        let b = native_role_binding::try_new(1, 2).unwrap();
        let c = native_role_binding::try_new(3, 1).unwrap();
        assert!(matches!(
            native_coordinator::provision(&model, &control, [a, b, c]),
            Err(coordinator_error::duplicate_task)
        ));
        assert!(matches!(
            native_coordinator::provision(&model, &other_control, [a, b, c]),
            Err(coordinator_error::desktop(_))
        ));
        assert_eq!(
            native_role_binding::try_new(1, 0),
            Err(coordinator_error::invalid_binding)
        );
    }

    #[test]
    fn ready_workspace_keeps_turns_while_editor_and_service_progress() {
        let (model, control, _) = desktop::new(1024, 768).unwrap();
        let (mut coordinator, owners) = provision(&model, &control);
        for _ in 0..4 {
            assert_eq!(coordinator.next_runnable(), Some(native_role::workspace));
            assert_eq!(coordinator.next_runnable(), Some(native_role::editor));
            assert_eq!(coordinator.next_runnable(), Some(native_role::document));
        }
        coordinator.set_runnable(&owners.editor, false).unwrap();
        assert_eq!(coordinator.next_runnable(), Some(native_role::workspace));
        assert_eq!(coordinator.next_runnable(), Some(native_role::document));
        coordinator.retire(&owners.document).unwrap();
        assert_eq!(coordinator.next_runnable(), Some(native_role::workspace));
        coordinator.set_runnable(&owners.workspace, false).unwrap();
        assert_eq!(coordinator.next_runnable(), None);
        coordinator.set_runnable(&owners.editor, true).unwrap();
        assert_eq!(coordinator.next_runnable(), Some(native_role::editor));
    }

    struct fixture {
        model: desktop,
        control: desktop_control,
        human: client_endpoint,
        owner: native_editor_owner,
        bridge: native_editor_bridge,
        input: native_editor_input,
        coordinator: native_coordinator,
        roles: native_role_owners,
    }
    impl fixture {
        fn new() -> Self {
            let (mut model, control, initial) = desktop::new(1024, 768).unwrap();
            let human = model
                .bind_client(&control, client_origin::human, client_grants::all)
                .unwrap();
            let producer = model
                .bind_client(
                    &control,
                    client_origin::machine,
                    client_grants::observe.union(client_grants::present),
                )
                .unwrap();
            model.begin(&control, &initial).unwrap();
            model.attest_presented(&control, &initial).unwrap();
            model.publish(&control, initial).ok().unwrap();
            let mut owner = native_editor_owner::new(
                interactive_editor_session::from_open(71, 3, 1, b"a").unwrap(),
            );
            let frame = owner.frame().unwrap();
            let revision = model.observe(&human).unwrap().revision();
            let initial = model
                .prepare(
                    &human,
                    revision,
                    desktop_command::open_native_editor(frame.descriptor()),
                )
                .unwrap();
            model.begin(&control, &initial).unwrap();
            model.attest_presented(&control, &initial).unwrap();
            let window = model
                .publish(&control, initial)
                .ok()
                .unwrap()
                .created_window()
                .unwrap();
            let bridge = model
                .bind_native_editor(
                    &control,
                    &producer,
                    window,
                    &mut owner,
                    frame,
                    editor_disclosure::public_fixture,
                )
                .ok()
                .unwrap();
            let input = bridge.bind_input(&model, &control, &human).unwrap();
            let (coordinator, roles) = provision(&model, &control);
            Self {
                model,
                control,
                human,
                owner,
                bridge,
                input,
                coordinator,
                roles,
            }
        }
        fn submit(&mut self, action: native_editor_action) -> submitted_native_action {
            let revision = self.model.observe(&self.human).unwrap().revision();
            self.bridge
                .submit_action(
                    &mut self.model,
                    &self.coordinator,
                    &self.roles.workspace,
                    &self.input,
                    revision,
                    action,
                )
                .unwrap()
        }
        fn mutation_blocked(&mut self) -> bool {
            let revision = self.model.observe(&self.human).unwrap().revision();
            matches!(
                self.model
                    .prepare(&self.human, revision, desktop_command::toggle_launcher),
                Err(desktop_error::backpressure)
            )
        }
    }
    fn insert(byte: u8) -> native_editor_action {
        native_editor_action::edit(interactive_editor_action::insert(byte))
    }

    #[test]
    fn execution_and_completion_are_separate_and_foreign_completion_retains_ownership() {
        let mut f = fixture::new();
        let submitted = f.submit(insert(b'b'));
        assert!(f.mutation_blocked());
        assert!(matches!(
            f.model.revoke_client(&f.control, &f.human),
            Err(desktop_error::backpressure)
        ));
        let completed = submitted
            .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
            .ok()
            .unwrap();
        assert_eq!(f.owner.session.view().text(), b"ba");
        assert!(f.mutation_blocked());
        let (foreign, foreign_roles) = provision(&f.model, &f.control);
        let rejected = f
            .bridge
            .complete_action(&mut f.model, &foreign, &foreign_roles.workspace, completed)
            .err()
            .unwrap();
        assert_eq!(rejected.error(), bridge_error::wrong_binding);
        assert!(f.mutation_blocked());
        let result = f
            .bridge
            .complete_action(
                &mut f.model,
                &f.coordinator,
                &f.roles.workspace,
                rejected.into_completion(),
            )
            .ok()
            .unwrap()
            .unwrap();
        assert_eq!(result.effect, native_editor_effect::edited);
        assert!(!f.mutation_blocked());
    }

    #[test]
    fn stale_sequence_resolves_without_effect_and_still_requires_exact_completion() {
        let mut f = fixture::new();
        let submitted = f.submit(insert(b'b'));
        // A separately authenticated owner event may race ahead of a queued
        // input. The pending command must not edit that newer session state.
        f.owner
            .session
            .apply(interactive_editor_action::insert(b'x'))
            .unwrap();
        let sequence = f.owner.session.view().sequence();
        let completed = submitted
            .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
            .ok()
            .unwrap();
        assert_eq!(completed.outcome(), &Err(bridge_error::stale_observation));
        assert_eq!(f.owner.session.view().text(), b"xa");
        assert_eq!(f.owner.session.view().sequence(), sequence);
        assert!(f.mutation_blocked());
        assert_eq!(
            f.bridge
                .complete_action(&mut f.model, &f.coordinator, &f.roles.workspace, completed)
                .ok()
                .unwrap(),
            Err(bridge_error::stale_observation)
        );
        assert!(!f.mutation_blocked());
    }

    #[test]
    fn retired_editor_cannot_execute_and_terminal_withdrawal_requires_retirement() {
        let mut f = fixture::new();
        let submitted = f.submit(insert(b'b'));
        let rejected = f
            .bridge
            .withdraw_terminal_submission(&mut f.model, &f.control, &f.coordinator, submitted)
            .err()
            .unwrap();
        assert_eq!(rejected.error(), bridge_error::wrong_binding);
        assert!(f.mutation_blocked());
        f.coordinator.retire(&f.roles.editor).unwrap();
        let completed = rejected
            .into_submission()
            .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
            .ok()
            .unwrap();
        assert_eq!(completed.outcome(), &Err(bridge_error::retired));
        assert_eq!(f.owner.session.view().text(), b"a");
        f.bridge
            .complete_action(&mut f.model, &f.coordinator, &f.roles.workspace, completed)
            .ok()
            .unwrap()
            .unwrap_err();
        let revision = f.model.observe(&f.human).unwrap().revision();
        assert!(matches!(
            f.bridge.submit_action(
                &mut f.model,
                &f.coordinator,
                &f.roles.workspace,
                &f.input,
                revision,
                insert(b'c')
            ),
            Err(bridge_error::retired)
        ));

        let mut f = fixture::new();
        let submitted = f.submit(insert(b'b'));
        f.coordinator.retire(&f.roles.editor).unwrap();
        f.bridge
            .withdraw_terminal_submission(&mut f.model, &f.control, &f.coordinator, submitted)
            .ok()
            .unwrap();
        assert!(f.bridge.retired);
        assert_eq!(f.model.native_action_fence, None);
        assert_eq!(f.owner.session.view().text(), b"a");
        assert!(matches!(
            f.bridge.observe(&f.model, &f.human),
            Err(bridge_error::retired)
        ));
    }

    #[test]
    fn accepted_save_drains_after_workspace_retirement_but_no_new_action_can_start() {
        let mut f = fixture::new();
        let submitted = f.submit(native_editor_action::save);
        f.coordinator.retire(&f.roles.workspace).unwrap();
        let completed = submitted
            .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
            .ok()
            .unwrap();
        assert!(matches!(
            completed.outcome(),
            Ok(native_editor_action_result {
                effect: native_editor_effect::save(_),
                ..
            })
        ));
        assert!(f.mutation_blocked());
        assert!(matches!(
            f.bridge
                .complete_action(&mut f.model, &f.coordinator, &f.roles.workspace, completed)
                .ok()
                .unwrap()
                .unwrap()
                .effect,
            native_editor_effect::save(_)
        ));
        assert!(
            f.coordinator
                .authorize_route(
                    &f.roles.editor,
                    native_route::document_request,
                    native_route_access::send
                )
                .is_ok()
        );
        let revision = f.model.observe(&f.human).unwrap().revision();
        assert!(matches!(
            f.bridge.submit_action(
                &mut f.model,
                &f.coordinator,
                &f.roles.workspace,
                &f.input,
                revision,
                insert(b'b')
            ),
            Err(bridge_error::retired)
        ));
    }

    #[test]
    fn dropping_transport_or_resolution_owner_never_releases_focus() {
        let mut f = fixture::new();
        let submitted = f.submit(insert(b'b'));
        drop(submitted);
        assert!(f.mutation_blocked());
        assert_eq!(f.owner.session.view().text(), b"a");
        let mut f = fixture::new();
        let submitted = f.submit(insert(b'b'));
        let completed = submitted
            .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
            .ok()
            .unwrap();
        drop(completed);
        assert!(f.mutation_blocked());
        assert_eq!(f.owner.session.view().text(), b"ba");
    }

    #[test]
    fn foreign_equal_numeric_binding_cannot_consume_submission_and_exhaustion_is_atomic() {
        let mut f = fixture::new();
        let submitted = f.submit(insert(b'b'));
        let (foreign, foreign_roles) = provision(&f.model, &f.control);
        assert_eq!(foreign_roles.editor.binding(), f.roles.editor.binding());
        let rejected = submitted
            .resolve(&foreign, &foreign_roles.editor, &mut f.owner)
            .err()
            .unwrap();
        assert_eq!(rejected.error(), bridge_error::wrong_binding);
        assert_eq!(f.owner.session.view().text(), b"a");
        assert!(f.mutation_blocked());
        let completed = rejected
            .into_submission()
            .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
            .ok()
            .unwrap();
        f.bridge
            .complete_action(&mut f.model, &f.coordinator, &f.roles.workspace, completed)
            .ok()
            .unwrap()
            .unwrap();

        let mut f = fixture::new();
        f.bridge.next_action = u64::MAX;
        let revision = f.model.observe(&f.human).unwrap().revision();
        assert!(matches!(
            f.bridge.submit_action(
                &mut f.model,
                &f.coordinator,
                &f.roles.workspace,
                &f.input,
                revision,
                insert(b'b')
            ),
            Err(bridge_error::sequence_exhausted)
        ));
        assert_eq!(f.bridge.next_action, u64::MAX);
        assert_eq!(f.bridge.in_flight, None);
        assert_eq!(f.model.native_action_fence, None);
        assert_eq!(f.owner.session.view().text(), b"a");
    }

    #[test]
    fn pending_desktop_candidate_prevents_submission_without_consuming_sequence() {
        let mut f = fixture::new();
        let revision = f.model.observe(&f.human).unwrap().revision();
        let pending = f
            .model
            .prepare(&f.human, revision, desktop_command::toggle_launcher)
            .unwrap();
        assert!(matches!(
            f.bridge.submit_action(
                &mut f.model,
                &f.coordinator,
                &f.roles.workspace,
                &f.input,
                revision,
                insert(b'b')
            ),
            Err(bridge_error::backpressure)
        ));
        assert_eq!(f.bridge.next_action, 1);
        assert_eq!(f.bridge.in_flight, None);
        assert_eq!(f.model.native_action_fence, None);
        f.model.cancel(&f.control, pending).ok().unwrap();
    }

    #[test]
    fn action_owners_and_rejections_carry_no_inline_pixel_arrays() {
        assert!(core::mem::size_of::<submitted_native_action>() <= 256);
        assert!(core::mem::size_of::<completed_native_action>() <= 256);
        assert!(core::mem::size_of::<rejected_native_submission>() <= 256);
        assert!(core::mem::size_of::<rejected_native_completion>() <= 256);
    }
}
