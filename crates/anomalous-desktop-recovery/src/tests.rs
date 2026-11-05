use anomalous_desktop_scene::{
    action_identity, declared_public_text, declared_scene_state, node_identity,
    scene_action_definition, scene_action_kind, scene_depth, scene_disclosure, scene_focus,
    scene_identity, scene_label, scene_motion, scene_node_definition, scene_rect, scene_role,
    scene_slot,
};

use super::*;

fn must<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("fixture operation failed"),
    }
}

fn candidate() -> scene_candidate {
    let slots = [
        scene_slot::root,
        scene_slot::masthead,
        scene_slot::navigation,
        scene_slot::primary_action,
        scene_slot::secondary_action,
        scene_slot::content,
        scene_slot::heading,
        scene_slot::status,
    ];
    let roles = [
        scene_role::desktop,
        scene_role::region,
        scene_role::region,
        scene_role::control,
        scene_role::control,
        scene_role::region,
        scene_role::heading,
        scene_role::status,
    ];
    let parents = [
        None,
        Some(0),
        Some(0),
        Some(2),
        Some(2),
        Some(0),
        Some(5),
        Some(5),
    ];
    let bounds = [
        (0, 0, 1024, 768),
        (32, 24, 960, 88),
        (32, 136, 256, 600),
        (56, 184, 208, 64),
        (56, 272, 208, 64),
        (312, 136, 680, 600),
        (352, 176, 600, 48),
        (352, 248, 600, 96),
    ];
    let labels = [
        "Anomalous desktop",
        "Anomalous",
        "Navigation",
        "Open shell",
        "Inspect system",
        "Workspace",
        "System workspace",
        "System ready",
    ];
    let nodes = core::array::from_fn(|index| {
        let slot = slots[index];
        let (x, y, width, height) = bounds[index];
        let action = match slot {
            scene_slot::primary_action | scene_slot::secondary_action => {
                let action_slot = if slot == scene_slot::primary_action {
                    1
                } else {
                    2
                };
                Some(scene_action_definition::new(
                    must(action_identity::try_new(action_slot, 7)),
                    scene_action_kind::activate,
                ))
            }
            _ => None,
        };
        scene_node_definition::new(
            slot,
            must(node_identity::try_new(slot.identity_slot(), 7)),
            roles[index],
            parents[index]
                .map(|parent| must(node_identity::try_new(slots[parent].identity_slot(), 7))),
            must(scene_rect::try_new(x, y, width, height)),
            scene_label::declared_public(must(declared_public_text::try_from_str(labels[index]))),
            action,
        )
    });
    scene_candidate::new(
        must(scene_identity::try_new(41, 7)),
        nodes,
        declared_scene_state::new(
            true,
            true,
            scene_focus::primary,
            scene_disclosure::declared_public,
            scene_depth::standard,
            scene_motion::reduced,
        ),
    )
}

fn fixture() -> (recovery_surface, recovery_control, presenter_endpoint) {
    must(recovery_surface::new_presented(
        must(surface_identity::try_new(51, 3)),
        must(renderer_identity::try_new(61, 1)),
        candidate(),
    ))
}

fn prepare(surface: &mut recovery_surface) {
    let revision = surface.status().committed_revision();
    must(surface.submit_revision(revision, candidate()));
    must(surface.acquire_render());
    assert_eq!(
        must(surface.render_candidate()).revision().get(),
        revision.get() + 1
    );
    must(surface.complete_render());
}

#[test]
fn gate_spans_copy_and_commit_until_exact_finish() {
    let (mut surface, control, presenter) = fixture();
    prepare(&mut surface);
    assert_eq!(must(surface.presented()).revision().get(), 1);
    let ticket = must(surface.begin(&presenter));
    assert!(surface.presented().is_err());
    assert_eq!(
        must(surface.redraw_candidate(&presenter, &ticket))
            .binding()
            .revision(),
        2
    );
    must(surface.attest_copied(&presenter, &ticket));
    assert!(surface.presented().is_err());
    must(surface.publish_semantics(&presenter, &ticket));
    assert_eq!(surface.status().committed_revision().get(), 2);
    assert!(surface.presented().is_err());
    must(surface.finish(&presenter, ticket));
    assert_eq!(must(surface.presented()).revision().get(), 2);
    assert_eq!(surface.status().publication_sequence().get(), 2);
    assert_eq!(must(surface.retire(control)).final_revision().get(), 2);
}

#[test]
fn lower_layer_rejections_restore_prepared_and_receipt_owners() {
    let (mut surface, control, presenter) = fixture();
    let (mut foreign, foreign_control, _) = fixture();
    prepare(&mut surface);
    let ticket = must(surface.begin(&presenter));
    core::mem::swap(&mut surface.presentation, &mut foreign.presentation);
    let before = surface.status();
    assert_eq!(
        surface.attest_copied(&presenter, &ticket),
        Err(recovery_error::service(surface_error::control_denied))
    );
    assert!(surface.status() == before);
    assert!(surface.prepared.is_some());
    assert!(surface.receipt.is_none());
    assert!(surface.presented().is_err());
    core::mem::swap(&mut surface.presentation, &mut foreign.presentation);
    must(surface.attest_copied(&presenter, &ticket));
    core::mem::swap(&mut surface.control, &mut foreign.control);
    let before = surface.status();
    assert_eq!(
        surface.publish_semantics(&presenter, &ticket),
        Err(recovery_error::service(surface_error::control_denied))
    );
    assert!(surface.status() == before);
    assert!(surface.receipt.is_some());
    assert!(surface.publication.is_none());
    core::mem::swap(&mut surface.control, &mut foreign.control);
    must(surface.publish_semantics(&presenter, &ticket));
    must(surface.finish(&presenter, ticket));
    must(surface.retire(control));
    must(foreign.retire(foreign_control));
}

#[test]
fn lower_layer_retirement_rejection_preserves_complete_coordinator() {
    let (mut surface, control, presenter) = fixture();
    let (mut foreign, foreign_control, _) = fixture();
    prepare(&mut surface);
    let ticket = must(surface.begin(&presenter));
    must(surface.attest_copied(&presenter, &ticket));
    let before = surface.status();
    let bytes = *must(surface.redraw_candidate(&presenter, &ticket)).as_bytes();
    core::mem::swap(&mut surface.control, &mut foreign.control);
    let rejection = match surface.retire(control) {
        Ok(_) => panic!("foreign internal control accepted"),
        Err(rejection) => rejection,
    };
    assert_eq!(
        rejection.error(),
        recovery_error::service(surface_error::control_denied)
    );
    let (mut surface, control) = rejection.into_parts();
    assert!(surface.status() == before);
    assert_eq!(
        must(surface.redraw_candidate(&presenter, &ticket)).as_bytes(),
        &bytes
    );
    assert!(surface.presented().is_err());
    core::mem::swap(&mut surface.control, &mut foreign.control);
    must(surface.publish_semantics(&presenter, &ticket));
    must(surface.finish(&presenter, ticket));
    must(surface.retire(control));
    must(foreign.retire(foreign_control));
}

#[test]
fn counter_exhaustion_preserves_gate_and_current_owners() {
    let (mut surface, control, mut presenter) = fixture();
    prepare(&mut surface);
    surface.transaction = u64::MAX;
    let before = surface.status();
    assert!(matches!(
        surface.begin(&presenter),
        Err(recovery_error::transaction_exhausted)
    ));
    assert!(surface.status() == before);
    assert_eq!(must(surface.presented()).revision().get(), 1);
    surface.transaction = 0;
    surface.presenter_generation = u64::MAX;
    presenter.generation = u64::MAX;
    let ticket = must(surface.begin(&presenter));
    let before = surface.status();
    assert!(matches!(
        surface.replace_presenter(&control),
        Err(recovery_error::presenter_generation_exhausted)
    ));
    assert!(surface.status() == before);
    assert!(surface.presented().is_err());
    must(surface.attest_copied(&presenter, &ticket));
    must(surface.publish_semantics(&presenter, &ticket));
    must(surface.finish(&presenter, ticket));
    must(surface.retire(control));
}

#[test]
fn second_transaction_rejects_retained_old_ticket_correlation() {
    let (mut surface, control, presenter) = fixture();
    prepare(&mut surface);
    let ticket = must(surface.begin(&presenter));
    // Internal forgery models replay only. Public safe Rust cannot clone owners.
    let stale = presentation_ticket {
        instance: ticket.instance,
        presenter_generation: ticket.presenter_generation,
        transaction: ticket.transaction,
    };
    must(surface.attest_copied(&presenter, &ticket));
    must(surface.publish_semantics(&presenter, &ticket));
    must(surface.finish(&presenter, ticket));
    prepare(&mut surface);
    let ticket = must(surface.begin(&presenter));
    assert_eq!(
        surface.attest_copied(&presenter, &stale),
        Err(recovery_error::stale_ticket)
    );
    assert!(surface.presented().is_err());
    must(surface.attest_copied(&presenter, &ticket));
    must(surface.publish_semantics(&presenter, &ticket));
    must(surface.finish(&presenter, ticket));
    assert_eq!(must(surface.presented()).revision().get(), 3);
    must(surface.retire(control));
}
