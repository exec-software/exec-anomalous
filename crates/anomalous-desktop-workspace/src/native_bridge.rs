//! Portable, trusted-local editor/workspace integration. This module authenticates
//! Rust capabilities, not OS tasks. The opt-in native transport remains separate.
//! Desktop copies contain only public window/surface metadata. The sole session
//! owner and its observation/pixel sidecar are distinct, move-only objects.

// This allocation-free portable gate returns complete retained ownership on
// rejection. The default owner is large; the borrowed specialization below
// carries only metadata and references. Native allocation/admission is separate.
#![allow(clippy::result_large_err)]

use super::*;
use anomalous_editor_contract::{
    interactive_audit, interactive_audit_size, interactive_close_result, interactive_editor_action,
    interactive_editor_error, interactive_editor_session, interactive_editor_view,
    interactive_frame_byte_count, interactive_frame_height, interactive_frame_width,
    render_interactive_audit, save_acknowledgement, save_snapshot,
};
use anomalous_window_surface::{borrowed_frame, frame, owned_frame};

#[path = "native_coordinator.rs"]
pub mod native_coordinator;

#[cfg(feature = "native_editor_remote")]
#[path = "native_remote.rs"]
pub mod native_remote;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum bridge_error {
    desktop(desktop_error),
    invalid_observation,
    invalid_surface,
    wrong_binding,
    owner_already_bound,
    observation_mismatch,
    stale_observation,
    stale_action,
    not_focused,
    retired,
    sequence_exhausted,
    backpressure,
    editor(interactive_editor_error),
}
impl From<desktop_error> for bridge_error {
    fn from(value: desktop_error) -> Self {
        Self::desktop(value)
    }
}

/// The first bridge admits only explicitly public fixture content. Ordinary
/// desktop observe grants may read it through the live bridge; a future private
/// document policy must use distinct disclosure grants and observation routes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum editor_disclosure {
    public_fixture,
}

mod storage_sealed {
    pub trait sealed {}
}

/// Storage retaining one immutable content identity across every borrow.
/// This trait is sealed so callers cannot pair metadata with unrelated pixels.
pub trait native_editor_storage: storage_sealed::sealed {
    fn pixels(&self) -> frame<'_>;
}
impl storage_sealed::sealed for owned_frame<interactive_frame_byte_count> {}
impl native_editor_storage for owned_frame<interactive_frame_byte_count> {
    fn pixels(&self) -> frame<'_> {
        self.as_frame()
    }
}
impl storage_sealed::sealed for borrowed_frame<'_, interactive_frame_byte_count> {}
impl native_editor_storage for borrowed_frame<'_, interactive_frame_byte_count> {
    fn pixels(&self) -> frame<'_> {
        self.as_frame()
    }
}

/// Exact ANUI and pixels derived from it, retained outside the desktop snapshot.
/// The default inline owner preserves the portable API. Borrowed storage keeps
/// pixels in a caller-retained allocation and moves only compact metadata.
pub struct native_editor_frame<S = owned_frame<interactive_frame_byte_count>> {
    audit: [u8; interactive_audit_size],
    descriptor: surface_descriptor,
    pixels: S,
}
impl native_editor_frame {
    pub fn from_view(view: &interactive_editor_view) -> Result<Self, bridge_error> {
        let audit = interactive_audit::from_view(view)
            .encode()
            .map_err(|_| bridge_error::invalid_observation)?;
        Self::from_audit(&audit)
    }
    /// Decoding and rendering establish correspondence, never authority. Binding
    /// and submission additionally check the separate live editor owner.
    pub fn from_audit(bytes: &[u8]) -> Result<Self, bridge_error> {
        let view =
            interactive_audit::decode(bytes).map_err(|_| bridge_error::invalid_observation)?;
        let mut pixels = [0; interactive_frame_byte_count];
        render_interactive_audit(&view, &mut pixels)
            .map_err(|_| bridge_error::invalid_observation)?;
        let pixels = owned_frame::try_new(
            interactive_frame_width as u32,
            interactive_frame_height as u32,
            (interactive_frame_width * 3) as u32,
            "Public native editor fixture",
            pixels,
        )
        .map_err(|_| bridge_error::invalid_surface)?;
        Ok(Self {
            audit: bytes
                .try_into()
                .map_err(|_| bridge_error::invalid_observation)?,
            descriptor: pixels.descriptor(),
            pixels,
        })
    }
}

/// Constructor failure retains the supplied allocation. Malformed observations
/// fail before rendering; surface-identity exhaustion may return rendered bytes.
/// Neither case publishes content or loses exclusive ownership of the buffer.
pub struct rejected_editor_buffer<'a> {
    error: bridge_error,
    buffer: &'a mut [u8; interactive_frame_byte_count],
}
impl<'a> rejected_editor_buffer<'a> {
    pub const fn error(&self) -> bridge_error {
        self.error
    }
    pub fn into_buffer(self) -> &'a mut [u8; interactive_frame_byte_count] {
        self.buffer
    }
}

impl<'a> native_editor_frame<borrowed_frame<'a, interactive_frame_byte_count>> {
    pub fn from_view_in(
        view: &interactive_editor_view,
        buffer: &'a mut [u8; interactive_frame_byte_count],
    ) -> Result<Self, rejected_editor_buffer<'a>> {
        let audit = match interactive_audit::from_view(view).encode() {
            Ok(audit) => audit,
            Err(_) => {
                return Err(rejected_editor_buffer {
                    error: bridge_error::invalid_observation,
                    buffer,
                });
            }
        };
        Self::from_audit_in(&audit, buffer)
    }
    pub fn from_audit_in(
        bytes: &[u8],
        buffer: &'a mut [u8; interactive_frame_byte_count],
    ) -> Result<Self, rejected_editor_buffer<'a>> {
        let audit = match <[u8; interactive_audit_size]>::try_from(bytes) {
            Ok(audit) => audit,
            Err(_) => {
                return Err(rejected_editor_buffer {
                    error: bridge_error::invalid_observation,
                    buffer,
                });
            }
        };
        let view = match interactive_audit::decode(&audit) {
            Ok(view) => view,
            Err(_) => {
                return Err(rejected_editor_buffer {
                    error: bridge_error::invalid_observation,
                    buffer,
                });
            }
        };
        if render_interactive_audit(&view, buffer).is_err() {
            return Err(rejected_editor_buffer {
                error: bridge_error::invalid_observation,
                buffer,
            });
        }
        let pixels = match borrowed_frame::try_new(
            interactive_frame_width as u32,
            interactive_frame_height as u32,
            (interactive_frame_width * 3) as u32,
            "Public native editor fixture",
            buffer,
        ) {
            Ok(pixels) => pixels,
            Err(rejected) => {
                return Err(rejected_editor_buffer {
                    error: bridge_error::invalid_surface,
                    buffer: rejected.into_pixels(),
                });
            }
        };
        Ok(Self {
            audit,
            descriptor: pixels.descriptor(),
            pixels,
        })
    }
    pub fn into_buffer(self) -> &'a mut [u8; interactive_frame_byte_count] {
        self.pixels.into_pixels()
    }
}
impl<S: native_editor_storage> native_editor_frame<S> {
    pub const fn descriptor(&self) -> surface_descriptor {
        self.descriptor
    }
    pub fn pixels(&self) -> frame<'_> {
        self.pixels.pixels()
    }
    pub fn observation(&self) -> interactive_audit<'_> {
        interactive_audit::decode(&self.audit).expect("validated immutable editor observation")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct binding {
    instance: u64,
    generation: u64,
    window: window_id,
    producer_slot: usize,
    producer_generation: u64,
}

/// The application side of the local bridge. This owns the only mutable editor
/// session. Service replies are authenticated by the embedding document route,
/// not by a desktop descriptor or input ticket.
///
/// ```compile_fail
/// use anomalous_desktop_workspace::native_bridge::native_editor_owner;
/// fn copied<T: Copy>() {}
/// copied::<native_editor_owner>();
/// ```
/// ```compile_fail
/// use anomalous_desktop_workspace::native_bridge::native_editor_owner;
/// fn cloned<T: Clone>() {}
/// cloned::<native_editor_owner>();
/// ```
pub struct native_editor_owner {
    key: Option<binding>,
    session: interactive_editor_session,
}
impl native_editor_owner {
    pub const fn new(session: interactive_editor_session) -> Self {
        Self { key: None, session }
    }
    pub const fn session(&self) -> &interactive_editor_session {
        &self.session
    }
    pub fn frame(&self) -> Result<native_editor_frame, bridge_error> {
        native_editor_frame::from_view(&self.session.view())
    }
    pub fn frame_in<'a>(
        &self,
        buffer: &'a mut [u8; interactive_frame_byte_count],
    ) -> Result<
        native_editor_frame<borrowed_frame<'a, interactive_frame_byte_count>>,
        rejected_editor_buffer<'a>,
    > {
        native_editor_frame::from_view_in(&self.session.view(), buffer)
    }
    pub fn acknowledge_save(
        &mut self,
        reply: save_acknowledgement<'_>,
    ) -> Result<(), interactive_editor_error> {
        self.session.acknowledge_save(reply)
    }
    pub fn reject_save(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), interactive_editor_error> {
        self.session
            .reject_save(document, generation, request, base_version)
    }
    pub fn mark_save_unknown(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), interactive_editor_error> {
        self.session
            .mark_save_unknown(document, generation, request, base_version)
    }
}

pub struct rejected_editor_frame<S = owned_frame<interactive_frame_byte_count>> {
    error: bridge_error,
    frame: native_editor_frame<S>,
}
impl<S: native_editor_storage> rejected_editor_frame<S> {
    pub const fn error(&self) -> bridge_error {
        self.error
    }
    pub fn into_frame(self) -> native_editor_frame<S> {
        self.frame
    }
}

/// Workspace-side state. It has no document handle, request counter, pending
/// save bytes or mutable editor. Dropping it does not undo any application event.
pub struct native_editor_bridge<S = owned_frame<interactive_frame_byte_count>> {
    key: binding,
    displayed: native_editor_frame<S>,
    surface_sequence: u64,
    next_action: u64,
    in_flight: Option<u64>,
    retired: bool,
}

/// Retains the desktop transaction and exact next observation/pixels together.
/// Dropping an unpublished ticket leaves the desktop unavailable/pending; it is
/// not implicit cancellation or an acknowledgement to the editor.
#[must_use = "publish or cancel the complete retained editor update"]
pub struct native_editor_update<S = owned_frame<interactive_frame_byte_count>> {
    key: binding,
    base_sequence: u64,
    surface_sequence: u64,
    ticket: update_ticket,
    frame: native_editor_frame<S>,
}
impl<S: native_editor_storage> native_editor_update<S> {
    /// The authorized presenter retains this immutable content through every row.
    pub const fn frame(&self) -> &native_editor_frame<S> {
        &self.frame
    }
}
pub struct rejected_editor_update<S = owned_frame<interactive_frame_byte_count>> {
    error: bridge_error,
    update: native_editor_update<S>,
}
impl<S: native_editor_storage> rejected_editor_update<S> {
    pub const fn error(&self) -> bridge_error {
        self.error
    }
    pub fn into_update(self) -> native_editor_update<S> {
        self.update
    }
}

/// A receipt is descriptive; no bridge API accepts it as publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct native_editor_publication {
    pub desktop: publication,
    pub observation_sequence: u64,
    pub closed: bool,
}

/// Explicit local action capability, bound to one existing observer/client and
/// one editor attachment. It cannot be manufactured from public numeric IDs.
pub struct native_editor_input {
    key: binding,
    slot: usize,
    generation: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum native_editor_action {
    edit(interactive_editor_action),
    save,
    request_close,
}
#[must_use = "consume or explicitly cancel the admitted action"]
/// ```compile_fail
/// use anomalous_desktop_workspace::native_bridge::native_editor_action_ticket;
/// fn copied<T: Copy>() {}
/// copied::<native_editor_action_ticket>();
/// ```
pub struct native_editor_action_ticket {
    key: binding,
    sequence: u64,
    desktop_revision: u64,
    observation_sequence: u64,
    slot: usize,
    generation: u64,
    action: native_editor_action,
}
pub struct rejected_editor_action {
    error: bridge_error,
    ticket: Option<native_editor_action_ticket>,
}
impl rejected_editor_action {
    pub const fn error(&self) -> bridge_error {
        self.error
    }
    /// A foreign/unrecognized permit is returned intact. An exact owned permit
    /// rejected by current state was consumed without effects and returns None.
    pub fn into_ticket(self) -> Option<native_editor_action_ticket> {
        self.ticket
    }
    fn consumed(error: bridge_error) -> Self {
        Self {
            error,
            ticket: None,
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub enum native_editor_effect {
    edited,
    save(save_snapshot),
    close(interactive_close_result),
}
#[derive(Debug, Eq, PartialEq)]
pub struct native_editor_action_result {
    pub origin: client_origin,
    pub effect: native_editor_effect,
}

impl desktop {
    /// Trusted assertion that the application route is terminal. This removes
    /// its window without claiming a save, a confirmed application close, or
    /// secure erasure. The controller must withdraw old input/content routes;
    /// the committed removal then makes every old local bridge unusable.
    pub fn prepare_native_editor_withdrawal(
        &mut self,
        control: &desktop_control,
        manager: &client_endpoint,
        expected_revision: u64,
        window: window_id,
    ) -> Result<update_ticket, desktop_error> {
        self.check_control(control)?;
        self.prepare_with(
            manager,
            expected_revision,
            client_grants::observe.union(client_grants::manage),
            |candidate| {
                candidate.remove_native_editor(window)?;
                candidate.normalize()?;
                Ok(None)
            },
        )
    }
    /// Attach a real sole editor owner to an already presented exact frame.
    /// The controller chooses disclosure and binds the producer independently
    /// of observation bytes. A fresh attachment advances the binding generation.
    /// This checkpoint cannot reattach a surviving owner to a replacement desktop.
    pub fn bind_native_editor<S: native_editor_storage>(
        &mut self,
        control: &desktop_control,
        producer: &client_endpoint,
        window: window_id,
        owner: &mut native_editor_owner,
        frame: native_editor_frame<S>,
        _disclosure: editor_disclosure,
    ) -> Result<native_editor_bridge<S>, rejected_editor_frame<S>> {
        if owner.key.is_some() {
            return Err(rejected_editor_frame {
                error: bridge_error::owner_already_bound,
                frame,
            });
        }
        if let Err(error) = matches_owner(owner, &frame) {
            return Err(rejected_editor_frame { error, frame });
        }
        let bridge = self.bind_editor_frame(control, producer, window, frame)?;
        owner.key = Some(bridge.key);
        Ok(bridge)
    }
    // Only the local owner path or the separately authenticated remote path may
    // call this common publication binding. Decoded ANUI alone is insufficient.
    fn bind_editor_frame<S: native_editor_storage>(
        &mut self,
        control: &desktop_control,
        producer: &client_endpoint,
        window: window_id,
        frame: native_editor_frame<S>,
    ) -> Result<native_editor_bridge<S>, rejected_editor_frame<S>> {
        let check = (|| {
            self.check_control(control)?;
            self.require_available()?;
            self.require_action_resolved()?;
            if self.pending.is_some() {
                return Err(bridge_error::backpressure);
            }
            let client = self.check_client(producer)?;
            if !client
                .grants
                .contains(client_grants::observe.union(client_grants::present))
            {
                return Err(desktop_error::grant_denied.into());
            }
            let content = self.committed.window(window)?;
            if content.content != demo_content::native_editor {
                return Err(desktop_error::not_native_editor_window.into());
            }
            if content.surface != Some(frame.descriptor()) || frame.observation().closed {
                return Err(bridge_error::observation_mismatch);
            }
            let generation = self
                .native_editor_binding
                .checked_add(1)
                .ok_or(bridge_error::sequence_exhausted)?;
            Ok((generation, content.surface_sequence))
        })();
        let (generation, surface_sequence) = match check {
            Ok(value) => value,
            Err(error) => return Err(rejected_editor_frame { error, frame }),
        };
        let key = binding {
            instance: self.instance,
            generation,
            window,
            producer_slot: producer.slot,
            producer_generation: producer.generation,
        };
        self.native_editor_binding = generation;
        Ok(native_editor_bridge {
            key,
            displayed: frame,
            surface_sequence,
            next_action: 1,
            in_flight: None,
            retired: false,
        })
    }
}

#[cfg(test)]
#[path = "native_bridge_buffer_tests.rs"]
mod buffer_tests;
#[cfg(test)]
#[path = "native_bridge_tests.rs"]
mod tests;

fn matches_owner<S: native_editor_storage>(
    owner: &native_editor_owner,
    frame: &native_editor_frame<S>,
) -> Result<(), bridge_error> {
    let encoded = interactive_audit::from_view(&owner.session.view())
        .encode()
        .map_err(|_| bridge_error::invalid_observation)?;
    if encoded != frame.audit {
        return Err(bridge_error::observation_mismatch);
    }
    Ok(())
}

impl<S: native_editor_storage> native_editor_bridge<S> {
    pub const fn window(&self) -> window_id {
        self.key.window
    }
    fn producer(&self) -> client_endpoint {
        client_endpoint {
            instance: self.key.instance,
            slot: self.key.producer_slot,
            generation: self.key.producer_generation,
        }
    }
    fn check_binding(&self, model: &desktop) -> Result<(), bridge_error> {
        if self.retired {
            return Err(bridge_error::retired);
        }
        if self.key.instance != model.instance || self.key.generation != model.native_editor_binding
        {
            return Err(bridge_error::wrong_binding);
        }
        let producer = self.producer();
        let client = model.check_client(&producer)?;
        if !client
            .grants
            .contains(client_grants::observe.union(client_grants::present))
        {
            return Err(desktop_error::grant_denied.into());
        }
        let window = model.committed.window(self.key.window)?;
        if window.content != demo_content::native_editor
            || window.surface != Some(self.displayed.descriptor())
            || window.surface_sequence != self.surface_sequence
        {
            return Err(bridge_error::observation_mismatch);
        }
        Ok(())
    }
    fn check_update(
        &self,
        model: &desktop,
        update: &native_editor_update<S>,
    ) -> Result<(), bridge_error> {
        self.check_binding(model)?;
        if update.key != self.key || update.base_sequence != self.displayed.observation().sequence {
            return Err(bridge_error::wrong_binding);
        }
        model.check_ticket(&update.ticket)?;
        Ok(())
    }
    /// Resolve through live local authority, never a copied descriptor alone.
    pub fn observe<'a>(
        &'a self,
        model: &desktop,
        observer: &client_endpoint,
    ) -> Result<interactive_audit<'a>, bridge_error> {
        self.check_binding(model)?;
        model.observe(observer)?;
        Ok(self.displayed.observation())
    }
    pub fn presented_frame<'a>(
        &'a self,
        model: &desktop,
        observer: &client_endpoint,
    ) -> Result<frame<'a>, bridge_error> {
        self.observe(model, observer)?;
        Ok(self.displayed.pixels())
    }
    pub fn bind_input(
        &self,
        model: &desktop,
        control: &desktop_control,
        client: &client_endpoint,
    ) -> Result<native_editor_input, bridge_error> {
        model.check_control(control)?;
        self.observe(model, client)?;
        if !model
            .check_client(client)?
            .grants
            .contains(client_grants::present)
        {
            return Err(desktop_error::grant_denied.into());
        }
        Ok(native_editor_input {
            key: self.key,
            slot: client.slot,
            generation: client.generation,
        })
    }
    pub fn prepare_update(
        &mut self,
        model: &mut desktop,
        owner: &native_editor_owner,
        expected_revision: u64,
        frame: native_editor_frame<S>,
    ) -> Result<native_editor_update<S>, rejected_editor_frame<S>> {
        if owner.key != Some(self.key) {
            return Err(rejected_editor_frame {
                error: bridge_error::wrong_binding,
                frame,
            });
        }
        if let Err(error) = matches_owner(owner, &frame) {
            return Err(rejected_editor_frame { error, frame });
        }
        self.prepare_frame_update(model, expected_revision, frame)
    }
    fn prepare_frame_update(
        &mut self,
        model: &mut desktop,
        expected_revision: u64,
        frame: native_editor_frame<S>,
    ) -> Result<native_editor_update<S>, rejected_editor_frame<S>> {
        let check = (|| {
            self.check_binding(model)?;
            if self.in_flight.is_some() {
                return Err(bridge_error::backpressure);
            }
            let view = frame.observation();
            if view.sequence <= self.displayed.observation().sequence {
                return Err(bridge_error::stale_observation);
            }
            let surface_sequence = self
                .surface_sequence
                .checked_add(1)
                .ok_or(bridge_error::sequence_exhausted)?;
            let ticket = model.prepare_with(
                &self.producer(),
                expected_revision,
                client_grants::observe.union(client_grants::present),
                |candidate| {
                    if view.closed {
                        candidate.remove_native_editor(self.key.window)?;
                    } else {
                        let window = candidate.window_mut(self.key.window)?;
                        window.surface = Some(frame.descriptor());
                        window.surface_sequence = surface_sequence;
                    }
                    candidate.normalize()?;
                    Ok(None)
                },
            )?;
            Ok((ticket, surface_sequence))
        })();
        let (ticket, surface_sequence) = match check {
            Ok(value) => value,
            Err(error) => return Err(rejected_editor_frame { error, frame }),
        };
        Ok(native_editor_update {
            key: self.key,
            base_sequence: self.displayed.observation().sequence,
            surface_sequence,
            ticket,
            frame,
        })
    }
    pub fn candidate<'a>(
        &self,
        model: &'a desktop,
        update: &native_editor_update<S>,
    ) -> Result<render_view<'a>, bridge_error> {
        self.check_update(model, update)?;
        Ok(model.candidate(&update.ticket)?)
    }
    pub fn begin(
        &self,
        model: &mut desktop,
        control: &desktop_control,
        update: &native_editor_update<S>,
    ) -> Result<(), bridge_error> {
        self.check_update(model, update)?;
        Ok(model.begin(control, &update.ticket)?)
    }
    /// Trusted assertion of a complete matching desktop frame, not pixel proof.
    pub fn attest_presented(
        &self,
        model: &mut desktop,
        control: &desktop_control,
        update: &native_editor_update<S>,
    ) -> Result<(), bridge_error> {
        self.check_update(model, update)?;
        Ok(model.attest_presented(control, &update.ticket)?)
    }
    pub fn publish(
        &mut self,
        model: &mut desktop,
        control: &desktop_control,
        update: native_editor_update<S>,
    ) -> Result<native_editor_publication, rejected_editor_update<S>> {
        self.publish_recycling(model, control, update)
            .map(|(publication, _retired)| publication)
    }
    /// Install the next immutable frame and return the old owner for reuse.
    /// Every failure returns the complete pending update and keeps the displayed
    /// owner untouched. All fallible checks precede desktop publication.
    pub fn publish_recycling(
        &mut self,
        model: &mut desktop,
        control: &desktop_control,
        update: native_editor_update<S>,
    ) -> Result<(native_editor_publication, native_editor_frame<S>), rejected_editor_update<S>>
    {
        if let Err(error) = self.check_update(model, &update) {
            return Err(rejected_editor_update { error, update });
        }
        let native_editor_update {
            key,
            base_sequence,
            surface_sequence,
            ticket,
            frame,
        } = update;
        let publication = match model.publish(control, ticket) {
            Ok(value) => value,
            Err(rejected) => {
                return Err(rejected_editor_update {
                    error: rejected.error().into(),
                    update: native_editor_update {
                        key,
                        base_sequence,
                        surface_sequence,
                        ticket: rejected.into_ticket(),
                        frame,
                    },
                });
            }
        };
        // No fallible step is allowed after canonical desktop publication.
        let view = frame.observation();
        let receipt = native_editor_publication {
            desktop: publication,
            observation_sequence: view.sequence,
            closed: view.closed,
        };
        self.retired = view.closed;
        let retired = core::mem::replace(&mut self.displayed, frame);
        self.surface_sequence = surface_sequence;
        Ok((receipt, retired))
    }
    /// Cancellation needs the original controller/ticket, but is permitted after
    /// producer revocation. It returns the retained frame for retry/resolution.
    pub fn cancel(
        &self,
        model: &mut desktop,
        control: &desktop_control,
        update: native_editor_update<S>,
    ) -> Result<native_editor_frame<S>, rejected_editor_update<S>> {
        if update.key != self.key {
            return Err(rejected_editor_update {
                error: bridge_error::wrong_binding,
                update,
            });
        }
        let native_editor_update {
            key,
            base_sequence,
            surface_sequence,
            ticket,
            frame,
        } = update;
        match model.cancel(control, ticket) {
            Ok(()) => Ok(frame),
            Err(rejected) => Err(rejected_editor_update {
                error: rejected.error().into(),
                update: native_editor_update {
                    key,
                    base_sequence,
                    surface_sequence,
                    ticket: rejected.into_ticket(),
                    frame,
                },
            }),
        }
    }
    fn check_action_context(
        &self,
        model: &desktop,
        slot: usize,
        generation: u64,
        revision: u64,
    ) -> Result<client_origin, bridge_error> {
        self.check_binding(model)?;
        let client = client_endpoint {
            instance: self.key.instance,
            slot,
            generation,
        };
        let view = model.observe(&client)?;
        if !model
            .check_client(&client)?
            .grants
            .contains(client_grants::present)
        {
            return Err(desktop_error::grant_denied.into());
        }
        if model.pending.is_some() {
            return Err(bridge_error::backpressure);
        }
        if view.revision() != revision {
            return Err(desktop_error::stale_revision.into());
        }
        if view.launcher_open()
            || view.focused_window().map(|window| window.id()) != Some(self.key.window)
            || model.committed.visible(self.key.window).is_err()
        {
            return Err(bridge_error::not_focused);
        }
        Ok(model.check_client(&client)?.origin)
    }
    pub fn prepare_action(
        &mut self,
        model: &desktop,
        input: &native_editor_input,
        expected_revision: u64,
        action: native_editor_action,
    ) -> Result<native_editor_action_ticket, bridge_error> {
        if input.key != self.key {
            return Err(bridge_error::wrong_binding);
        }
        self.check_action_context(model, input.slot, input.generation, expected_revision)?;
        if self.in_flight.is_some() {
            return Err(bridge_error::backpressure);
        }
        let sequence = self.next_action;
        let next = sequence
            .checked_add(1)
            .ok_or(bridge_error::sequence_exhausted)?;
        self.next_action = next;
        self.in_flight = Some(sequence);
        Ok(native_editor_action_ticket {
            key: self.key,
            sequence,
            desktop_revision: expected_revision,
            observation_sequence: self.displayed.observation().sequence,
            slot: input.slot,
            generation: input.generation,
            action,
        })
    }
    pub fn cancel_action(
        &mut self,
        ticket: native_editor_action_ticket,
    ) -> Result<(), rejected_editor_action> {
        if ticket.key != self.key || self.in_flight != Some(ticket.sequence) {
            return Err(rejected_editor_action {
                error: bridge_error::stale_action,
                ticket: Some(ticket),
            });
        }
        self.in_flight = None;
        Ok(())
    }
    /// This local gate rechecks both participants atomically under mutable
    /// borrows. A remote adapter needs an explicit focus fence/drain protocol.
    /// Exact stale permits are consumed without effects and require resync.
    pub fn consume_action(
        &mut self,
        model: &desktop,
        owner: &mut native_editor_owner,
        ticket: native_editor_action_ticket,
    ) -> Result<native_editor_action_result, rejected_editor_action> {
        if ticket.key != self.key || self.in_flight != Some(ticket.sequence) {
            return Err(rejected_editor_action {
                error: bridge_error::stale_action,
                ticket: Some(ticket),
            });
        }
        self.in_flight = None;
        let origin = self
            .check_action_context(
                model,
                ticket.slot,
                ticket.generation,
                ticket.desktop_revision,
            )
            .map_err(rejected_editor_action::consumed)?;
        if owner.key != Some(self.key) {
            return Err(rejected_editor_action::consumed(
                bridge_error::wrong_binding,
            ));
        }
        if owner.session.view().sequence() != ticket.observation_sequence
            || self.displayed.observation().sequence != ticket.observation_sequence
        {
            return Err(rejected_editor_action::consumed(
                bridge_error::stale_observation,
            ));
        }
        let effect = match ticket.action {
            native_editor_action::edit(action) => {
                owner
                    .session
                    .apply(action)
                    .map_err(bridge_error::editor)
                    .map_err(rejected_editor_action::consumed)?;
                native_editor_effect::edited
            }
            native_editor_action::save => native_editor_effect::save(
                owner
                    .session
                    .begin_save()
                    .map_err(bridge_error::editor)
                    .map_err(rejected_editor_action::consumed)?,
            ),
            native_editor_action::request_close => native_editor_effect::close(
                owner
                    .session
                    .request_close()
                    .map_err(bridge_error::editor)
                    .map_err(rejected_editor_action::consumed)?,
            ),
        };
        Ok(native_editor_action_result { origin, effect })
    }
}
