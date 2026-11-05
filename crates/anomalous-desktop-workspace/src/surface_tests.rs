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
    let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
    let human = model
        .bind_client(&control, client_origin::human, client_grants::all)
        .unwrap();
    publish(&mut model, &control, initial);
    (model, control, human)
}
fn open(
    model: &mut desktop,
    control: &desktop_control,
    client: &client_endpoint,
    frame: &frame<'_>,
) -> window_id {
    let revision = model.observe(client).unwrap().revision();
    let ticket = model
        .prepare(
            client,
            revision,
            desktop_command::open_graphics(frame.descriptor()),
        )
        .unwrap();
    publish(model, control, ticket).created_window().unwrap()
}

#[test]
fn replacement_is_scoped_revisioned_and_invisible_until_acknowledged() {
    let (mut model, control, human) = setup();
    let first = image("First public scene");
    let second = image("Second public scene");
    let id = open(&mut model, &control, &human, &first);
    let owner = model.bind_surface(&control, &human, id).unwrap();
    let revision = model.observe(&human).unwrap().revision();
    let ticket = model
        .prepare_surface(&owner, revision, second.descriptor())
        .unwrap();
    assert_eq!(
        model.observe(&human).unwrap().window(id).unwrap().surface(),
        Some(first.descriptor())
    );
    let candidate = model.candidate(&ticket).unwrap().window(id).unwrap();
    assert_eq!(candidate.surface(), Some(second.descriptor()));
    assert_eq!(candidate.surface_sequence(), 2);
    assert_eq!(
        model
            .prepare_surface(&owner, revision, first.descriptor())
            .err(),
        Some(desktop_error::backpressure)
    );
    model.begin(&control, &ticket).unwrap();
    assert_eq!(
        model.observe(&human).err(),
        Some(desktop_error::unavailable)
    );
    let rejection = model.publish(&control, ticket).err().unwrap();
    assert_eq!(rejection.error(), desktop_error::wrong_phase);
    let ticket = rejection.into_ticket();
    model.attest_presented(&control, &ticket).unwrap();
    assert!(model.publish(&control, ticket).is_ok());
    assert_eq!(
        model.observe(&human).unwrap().window(id).unwrap().surface(),
        Some(second.descriptor())
    );
    assert_eq!(
        model
            .prepare_surface(&owner, revision, first.descriptor())
            .err(),
        Some(desktop_error::stale_revision)
    );
}

#[test]
fn content_authority_is_explicit_and_identical_for_human_and_machine() {
    for origin in [client_origin::human, client_origin::machine] {
        let (mut model, control, manager) = setup();
        let first = image("Public image");
        let id = open(&mut model, &control, &manager, &first);
        let denied = model
            .bind_client(
                &control,
                origin,
                client_grants::observe.union(client_grants::manage),
            )
            .unwrap();
        assert_eq!(
            model.bind_surface(&control, &denied, id).err(),
            Some(desktop_error::grant_denied)
        );
        let revision = model.observe(&denied).unwrap().revision();
        assert_eq!(
            model
                .prepare(
                    &denied,
                    revision,
                    desktop_command::open_graphics(first.descriptor())
                )
                .err(),
            Some(desktop_error::grant_denied)
        );
        let producer = model
            .bind_client(
                &control,
                origin,
                client_grants::observe.union(client_grants::present),
            )
            .unwrap();
        let owner = model.bind_surface(&control, &producer, id).unwrap();
        let second = image("Replacement");
        let ticket = model
            .prepare_surface(&owner, revision, second.descriptor())
            .unwrap();
        assert_eq!(publish(&mut model, &control, ticket).origin(), Some(origin));
        let a = model.observe(&producer).unwrap().window(id).unwrap();
        let b = model.observe(&manager).unwrap().window(id).unwrap();
        assert_eq!(a.surface(), b.surface());
        assert_eq!(a.surface_sequence(), b.surface_sequence());
    }
}

#[test]
fn revoked_foreign_and_closed_window_owners_cannot_publish() {
    let (mut model, control, human) = setup();
    let image = image("Scoped content");
    let id = open(&mut model, &control, &human, &image);
    let producer = model
        .bind_client(
            &control,
            client_origin::machine,
            client_grants::observe.union(client_grants::present),
        )
        .unwrap();
    let owner = model.bind_surface(&control, &producer, id).unwrap();
    let revision = model.observe(&human).unwrap().revision();
    let (mut other, _, _) = setup();
    assert_eq!(
        other.prepare_surface(&owner, 1, image.descriptor()).err(),
        Some(desktop_error::client_denied)
    );
    let ticket = model
        .prepare_surface(&owner, revision, image.descriptor())
        .unwrap();
    model.revoke_client(&control, &producer).unwrap();
    assert_eq!(
        model.begin(&control, &ticket),
        Err(desktop_error::client_denied)
    );
    assert!(model.cancel(&control, ticket).is_ok());
    assert_eq!(
        model
            .prepare_surface(&owner, revision, image.descriptor())
            .err(),
        Some(desktop_error::client_denied)
    );
    let owner = model.bind_surface(&control, &human, id).unwrap();
    let ticket = model
        .prepare(&human, revision, desktop_command::close(id))
        .unwrap();
    publish(&mut model, &control, ticket);
    let replacement = open(&mut model, &control, &human, &image);
    assert_ne!(id, replacement);
    let revision = model.observe(&human).unwrap().revision();
    assert_eq!(
        model
            .prepare_surface(&owner, revision, image.descriptor())
            .err(),
        Some(desktop_error::stale_window)
    );
}

#[test]
fn reopening_preserves_content_and_sequence_exhaustion_rolls_back() {
    let (mut model, control, human) = setup();
    let first = image("Initial");
    let second = image("Retained");
    let id = open(&mut model, &control, &human, &first);
    let owner = model.bind_surface(&control, &human, id).unwrap();
    let revision = model.observe(&human).unwrap().revision();
    let ticket = model
        .prepare_surface(&owner, revision, second.descriptor())
        .unwrap();
    publish(&mut model, &control, ticket);
    let revision = model.observe(&human).unwrap().revision();
    let ticket = model
        .prepare(
            &human,
            revision,
            desktop_command::open_graphics(first.descriptor()),
        )
        .unwrap();
    assert!(
        publish(&mut model, &control, ticket)
            .created_window()
            .is_none()
    );
    assert_eq!(
        model.observe(&human).unwrap().window(id).unwrap().surface(),
        Some(second.descriptor())
    );
    model.committed.window_mut(id).unwrap().surface_sequence = u64::MAX;
    let revision = model.observe(&human).unwrap().revision();
    assert_eq!(
        model
            .prepare_surface(&owner, revision, first.descriptor())
            .err(),
        Some(desktop_error::sequence_exhausted)
    );
    assert!(model.pending.is_none());
    assert_eq!(
        model.observe(&human).unwrap().window(id).unwrap().surface(),
        Some(second.descriptor())
    );
    assert_eq!(model.observe(&human).unwrap().revision(), revision);
}
