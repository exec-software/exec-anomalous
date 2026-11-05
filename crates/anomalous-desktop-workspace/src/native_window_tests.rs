use super::*;
use anomalous_window_surface::frame;

fn image(label: &str) -> frame<'static> {
    frame::try_new(2, 2, 6, label, &[20; 12]).unwrap()
}

fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) -> publication {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    model.publish(control, ticket).ok().unwrap()
}

fn setup() -> (desktop, desktop_control, client_endpoint) {
    let (mut model, control, initial) = desktop::new(1024, 768).unwrap();
    let human = model
        .bind_client(&control, client_origin::human, client_grants::all)
        .unwrap();
    assert_eq!(model.native_editor_binding, 0);
    publish(&mut model, &control, initial);
    (model, control, human)
}

fn apply(
    model: &mut desktop,
    control: &desktop_control,
    endpoint: &client_endpoint,
    command: desktop_command,
) -> publication {
    let revision = model.observe(endpoint).unwrap().revision();
    let ticket = model.prepare(endpoint, revision, command).unwrap();
    publish(model, control, ticket)
}

#[test]
fn native_window_publishes_only_surface_metadata_and_has_no_scratch_owner() {
    let (mut model, control, human) = setup();
    let pixels = image("Authorized editor observation");
    let revision = model.observe(&human).unwrap().revision();
    let ticket = model
        .prepare(
            &human,
            revision,
            desktop_command::open_native_editor(pixels.descriptor()),
        )
        .unwrap();
    assert_eq!(model.observe(&human).unwrap().all_windows().count(), 0);
    let window = model.candidate(&ticket).unwrap().windows().next().unwrap();
    let id = window.id();
    assert_eq!(window.content(), demo_content::native_editor);
    assert_eq!(window.content().title(), "Native Editor");
    assert_eq!(window.surface(), Some(pixels.descriptor()));
    assert_eq!(window.surface_sequence(), 1);
    assert!(window.editor().is_none());
    assert!(
        model
            .pending
            .as_ref()
            .unwrap()
            .candidate
            .window(id)
            .unwrap()
            .editor
            .is_none()
    );
    assert_eq!(window.bounds().width(), 640);
    assert_eq!(window.bounds().height(), 448);
    assert_eq!(
        publish(&mut model, &control, ticket).created_window(),
        Some(id)
    );
    let window = model.observe(&human).unwrap().window(id).unwrap();
    assert!(window.focused());
    assert!(window.editor().is_none());
    assert_eq!(
        model.bind_surface(&control, &human, id).err(),
        Some(desktop_error::not_graphics_window)
    );
    assert_eq!(
        model
            .prepare(
                &human,
                revision + 1,
                desktop_command::edit_editor {
                    window: id,
                    action: editor_action::insert(b'x')
                }
            )
            .err(),
        Some(desktop_error::not_editor_window)
    );
}

#[test]
fn native_open_restores_singleton_and_preserves_its_committed_content() {
    let (mut model, control, human) = setup();
    let first = image("First observation");
    let replacement = image("Must not replace restored content");
    let id = apply(
        &mut model,
        &control,
        &human,
        desktop_command::open_native_editor(first.descriptor()),
    )
    .created_window()
    .unwrap();
    apply(&mut model, &control, &human, desktop_command::minimize(id));
    apply(
        &mut model,
        &control,
        &human,
        desktop_command::switch_workspace(workspace_id::second),
    );
    let result = apply(
        &mut model,
        &control,
        &human,
        desktop_command::open_native_editor(replacement.descriptor()),
    );
    assert_eq!(result.created_window(), None);
    let view = model.observe(&human).unwrap();
    assert_eq!(view.all_windows().count(), 1);
    let window = view.window(id).unwrap();
    assert_eq!(window.workspace(), workspace_id::second);
    assert!(!window.minimized());
    assert!(window.focused());
    assert_eq!(window.surface(), Some(first.descriptor()));
    assert_eq!(window.surface_sequence(), 1);
    assert!(window.editor().is_none());
}

#[test]
fn generic_creation_close_and_mixed_batch_reject_atomically() {
    let (mut model, control, human) = setup();
    let pixels = image("Scoped editor");
    let revision = model.observe(&human).unwrap().revision();
    assert_eq!(
        model
            .prepare(
                &human,
                revision,
                desktop_command::create {
                    content: demo_content::native_editor,
                    workspace: workspace_id::first,
                    bounds: window_rect::try_new(24, 80, 320, 240).unwrap(),
                }
            )
            .err(),
        Some(desktop_error::native_editor_open_required)
    );
    assert_eq!(model.observe(&human).unwrap().revision(), revision);
    assert_eq!(model.observe(&human).unwrap().all_windows().count(), 0);
    let id = apply(
        &mut model,
        &control,
        &human,
        desktop_command::open_native_editor(pixels.descriptor()),
    )
    .created_window()
    .unwrap();
    let before = model.observe(&human).unwrap().window(id).unwrap();
    let revision = model.observe(&human).unwrap().revision();
    let transaction = model.next_transaction;
    assert_eq!(
        model
            .prepare_batch(
                &human,
                revision,
                &[desktop_command::minimize(id), desktop_command::close(id)]
            )
            .err(),
        Some(desktop_error::native_editor_close_required)
    );
    assert_eq!(model.observe(&human).unwrap().window(id), Some(before));
    assert_eq!(model.observe(&human).unwrap().revision(), revision);
    assert_eq!(model.next_transaction, transaction);
    assert_eq!(model.phase(), presentation_phase::steady);
    assert_eq!(
        model
            .prepare_batch(
                &human,
                revision,
                &[
                    desktop_command::open_native_editor(pixels.descriptor()),
                    desktop_command::open_editor
                ]
            )
            .err(),
        Some(desktop_error::invalid_batch)
    );
}

#[test]
fn native_open_requires_management_arrangement_and_presentation_grants() {
    let (mut model, control, human) = setup();
    let pixels = image("Scoped editor");
    let revision = model.observe(&human).unwrap().revision();
    for missing in [
        client_grants::manage,
        client_grants::arrange,
        client_grants::present,
    ] {
        let endpoint = model
            .bind_client(
                &control,
                client_origin::machine,
                client_grants(client_grants::all.0 & !missing.0),
            )
            .unwrap();
        assert_eq!(
            model
                .prepare(
                    &endpoint,
                    revision,
                    desktop_command::open_native_editor(pixels.descriptor())
                )
                .err(),
            Some(desktop_error::grant_denied)
        );
        assert_eq!(model.observe(&human).unwrap().all_windows().count(), 0);
        model.revoke_client(&control, &endpoint).unwrap();
    }
}

#[test]
fn private_bridge_removal_is_transactional_and_retires_window_generation() {
    let (mut model, control, human) = setup();
    let pixels = image("Scoped editor");
    let id = apply(
        &mut model,
        &control,
        &human,
        desktop_command::open_native_editor(pixels.descriptor()),
    )
    .created_window()
    .unwrap();
    let revision = model.observe(&human).unwrap().revision();
    let ticket = model
        .prepare_with(
            &human,
            revision,
            client_grants::observe.union(client_grants::manage),
            |candidate| {
                candidate.remove_native_editor(id)?;
                Ok(None)
            },
        )
        .unwrap();
    assert!(model.observe(&human).unwrap().window(id).is_some());
    assert!(model.candidate(&ticket).unwrap().window(id).is_none());
    publish(&mut model, &control, ticket);
    let next = apply(
        &mut model,
        &control,
        &human,
        desktop_command::open_native_editor(pixels.descriptor()),
    )
    .created_window()
    .unwrap();
    assert_eq!(next.slot(), id.slot());
    assert!(next.generation() > id.generation());
    assert!(model.observe(&human).unwrap().window(id).is_none());
    let revision = model.observe(&human).unwrap().revision();
    assert_eq!(
        model
            .prepare_with(
                &human,
                revision,
                client_grants::observe.union(client_grants::manage),
                |candidate| {
                    candidate.remove_native_editor(id)?;
                    Ok(None)
                }
            )
            .err(),
        Some(desktop_error::stale_window)
    );
    let scratch = apply(&mut model, &control, &human, desktop_command::open_editor)
        .created_window()
        .unwrap();
    let revision = model.observe(&human).unwrap().revision();
    assert_eq!(
        model
            .prepare_with(
                &human,
                revision,
                client_grants::observe.union(client_grants::manage),
                |candidate| {
                    candidate.remove_native_editor(scratch)?;
                    Ok(None)
                }
            )
            .err(),
        Some(desktop_error::not_native_editor_window)
    );
    assert!(model.observe(&human).unwrap().window(scratch).is_some());
}
