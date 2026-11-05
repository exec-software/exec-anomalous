use super::*;
use anomalous_window_surface::frame;
use std::vec::Vec;

struct fixture {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
}

impl fixture {
    fn new() -> Self {
        Self::sized(1024, 768)
    }

    fn sized(width: u32, height: u32) -> Self {
        let (mut model, control, ticket) = desktop::new(width, height).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        model.begin(&control, &ticket).unwrap();
        model.attest_presented(&control, &ticket).unwrap();
        model.publish(&control, ticket).ok().unwrap();
        Self {
            model,
            control,
            human,
        }
    }

    fn view(&self) -> render_view<'_> {
        self.model.observe(&self.human).unwrap()
    }

    fn commit(&mut self, ticket: update_ticket) -> publication {
        self.model.begin(&self.control, &ticket).unwrap();
        self.model.attest_presented(&self.control, &ticket).unwrap();
        self.model.publish(&self.control, ticket).ok().unwrap()
    }

    fn apply(&mut self, command: desktop_command) -> publication {
        self.batch(&[command])
    }

    fn batch(&mut self, commands: &[desktop_command]) -> publication {
        let ticket = self
            .model
            .prepare_batch(&self.human, self.view().revision(), commands)
            .unwrap();
        self.commit(ticket)
    }

    fn open(&mut self, choice: launcher_choice) -> window_id {
        assert!(!self.view().launcher_open());
        self.apply(desktop_command::toggle_launcher);
        self.apply(desktop_command::activate_launcher {
            choice,
            graphics: (choice == launcher_choice::graphics).then_some(graphics()),
        });
        self.view().focused_window().unwrap().id()
    }
}

fn graphics() -> surface_descriptor {
    frame::try_new(2, 2, 6, "Public graphics", &[20; 12])
        .unwrap()
        .descriptor()
}

fn assert_rejection<T>(result: Result<T, desktop_error>, expected: desktop_error) {
    assert_eq!(result.err(), Some(expected));
}

#[test]
fn catalogue_is_exact_and_navigation_wraps_in_both_directions() {
    let contents = [
        demo_content::welcome,
        demo_content::inspector,
        demo_content::activity,
        demo_content::appearance,
        demo_content::graphics,
        demo_content::editor,
    ];
    for (index, choice) in launcher_choice::all.into_iter().enumerate() {
        assert_eq!(choice.content(), contents[index]);
        assert_eq!(choice.title(), contents[index].title());
        assert_eq!(
            choice.next(),
            launcher_choice::all[(index + 1) % contents.len()]
        );
        assert_eq!(
            choice.previous(),
            launcher_choice::all[(index + contents.len() - 1) % contents.len()]
        );
        assert_eq!(choice.next().previous(), choice);
    }
}

#[test]
fn launcher_state_is_published_once_and_shared_by_every_projection() {
    let mut f = fixture::new();
    let id = f.open(launcher_choice::welcome);
    let windows = f.view().all_windows().collect::<Vec<_>>();
    let revision = f.view().revision();
    let ticket = f
        .model
        .prepare(&f.human, revision, desktop_command::toggle_launcher)
        .unwrap();
    assert!(!f.view().launcher_open());
    let candidate = f.model.candidate(&ticket).unwrap();
    for view in [candidate, candidate.machine(), candidate.accessibility()] {
        assert!(view.launcher_open());
        assert_eq!(view.launcher_selection(), launcher_choice::welcome);
        assert_eq!(view.all_windows().collect::<Vec<_>>(), windows);
        assert_eq!(view.focused_window().unwrap().id(), id);
    }
    f.commit(ticket);
    f.apply(desktop_command::select_launcher(launcher_choice::activity));
    for view in [f.view(), f.view().machine(), f.view().accessibility()] {
        assert_eq!(view.launcher_selection(), launcher_choice::activity);
    }
    f.apply(desktop_command::dismiss_launcher);
    f.apply(desktop_command::toggle_launcher);
    for view in [f.view(), f.view().machine(), f.view().accessibility()] {
        assert!(view.launcher_open());
        assert_eq!(view.launcher_selection(), launcher_choice::welcome);
        assert_eq!(view.all_windows().collect::<Vec<_>>(), windows);
        assert_eq!(view.focused_window().unwrap().id(), id);
    }
}

#[test]
fn every_builtin_creates_once_then_reuses_its_live_incarnation() {
    for choice in launcher_choice::all {
        let mut f = fixture::new();
        let surface = (choice == launcher_choice::graphics).then(graphics);
        f.apply(desktop_command::toggle_launcher);
        f.apply(desktop_command::activate_launcher {
            choice,
            graphics: surface,
        });
        let id = f.view().focused_window().unwrap().id();
        let first = f.view().window(id).unwrap();
        assert_eq!(first.content(), choice.content());
        assert_eq!(first.surface(), surface);
        assert_eq!(f.view().all_windows().count(), 1);
        assert!(!f.view().launcher_open());
        f.apply(desktop_command::toggle_launcher);
        let publication = f.apply(desktop_command::activate_launcher {
            choice,
            graphics: None,
        });
        assert_eq!(publication.created_window(), None);
        assert_eq!(f.view().window(id), Some(first));
        assert_eq!(f.view().focused_window().unwrap().id(), id);
        assert_eq!(f.view().all_windows().count(), 1);
        assert!(!f.view().launcher_open());
    }
}

#[test]
fn activation_restores_and_moves_existing_app_without_losing_its_surface() {
    let mut f = fixture::new();
    let id = f.open(launcher_choice::graphics);
    let replacement = frame::try_new(2, 2, 6, "Replacement scene", &[90; 12]).unwrap();
    let owner = f.model.bind_surface(&f.control, &f.human, id).unwrap();
    let ticket = f
        .model
        .prepare_surface(&owner, f.view().revision(), replacement.descriptor())
        .unwrap();
    f.commit(ticket);
    f.batch(&[
        desktop_command::move_to_workspace {
            window: id,
            workspace: workspace_id::second,
        },
        desktop_command::minimize(id),
    ]);
    assert_eq!(f.view().focused_window(), None);
    f.apply(desktop_command::toggle_launcher);
    let publication = f.apply(desktop_command::activate_launcher {
        choice: launcher_choice::graphics,
        graphics: Some(graphics()),
    });
    assert_eq!(publication.created_window(), None);
    let window = f.view().window(id).unwrap();
    assert_eq!(window.workspace(), workspace_id::first);
    assert!(!window.minimized());
    assert!(window.focused());
    assert_eq!(window.surface(), Some(replacement.descriptor()));
    assert_eq!(window.surface_sequence(), 2);
}

#[test]
fn full_capacity_keeps_launcher_and_existing_apps_available() {
    let mut f = fixture::new();
    for choice in launcher_choice::all.into_iter().take(4) {
        f.open(choice);
    }
    let ids = f
        .view()
        .all_windows()
        .map(|window| window.id())
        .collect::<Vec<_>>();
    f.apply(desktop_command::toggle_launcher);
    assert_eq!(f.view().all_windows().count(), window_capacity);
    for choice in launcher_choice::all.into_iter().take(4) {
        assert!(f.view().launcher_enabled(choice));
    }
    assert!(!f.view().launcher_enabled(launcher_choice::graphics));
    f.apply(desktop_command::select_launcher(launcher_choice::graphics));
    let revision = f.view().revision();
    let transaction = f.model.next_transaction;
    assert_rejection(
        f.model.prepare(
            &f.human,
            revision,
            desktop_command::activate_launcher {
                choice: launcher_choice::graphics,
                graphics: Some(graphics()),
            },
        ),
        desktop_error::window_capacity_exhausted,
    );
    assert_eq!(f.model.next_transaction, transaction);
    assert_eq!(f.view().revision(), revision);
    assert!(f.view().launcher_open());
    assert_eq!(f.view().launcher_selection(), launcher_choice::graphics);
    f.batch(&[
        desktop_command::set_pointer { x: 50, y: 50 },
        desktop_command::set_pointer_buttons(1),
    ]);
    assert_rejection(
        f.model.prepare(
            &f.human,
            f.view().revision(),
            desktop_command::arm_pointer(desktop_target::launcher_item(launcher_choice::graphics)),
        ),
        desktop_error::invalid_pointer_target,
    );
    f.apply(desktop_command::activate_launcher {
        choice: launcher_choice::welcome,
        graphics: None,
    });
    assert_eq!(f.view().all_windows().count(), window_capacity);
    assert!(
        f.view()
            .all_windows()
            .all(|window| ids.contains(&window.id()))
    );
    assert_eq!(
        f.view().focused_window().unwrap().content(),
        demo_content::welcome
    );
}

#[test]
fn closed_or_invalid_graphics_launch_rejects_the_entire_candidate() {
    let mut f = fixture::new();
    let revision = f.view().revision();
    for command in [
        desktop_command::select_launcher(launcher_choice::activity),
        desktop_command::activate_launcher {
            choice: launcher_choice::activity,
            graphics: None,
        },
    ] {
        assert_rejection(
            f.model.prepare(&f.human, revision, command),
            desktop_error::launcher_closed,
        );
    }
    f.apply(desktop_command::toggle_launcher);
    let revision = f.view().revision();
    let transaction = f.model.next_transaction;
    for (choice, descriptor) in [
        (launcher_choice::graphics, None),
        (launcher_choice::welcome, Some(graphics())),
    ] {
        assert_rejection(
            f.model.prepare_batch(
                &f.human,
                revision,
                &[
                    desktop_command::select_launcher(choice),
                    desktop_command::activate_launcher {
                        choice,
                        graphics: descriptor,
                    },
                ],
            ),
            desktop_error::invalid_launcher_graphics,
        );
        assert_eq!(f.view().revision(), revision);
        assert_eq!(f.view().launcher_selection(), launcher_choice::welcome);
        assert!(f.view().launcher_open());
        assert_eq!(f.view().all_windows().count(), 0);
        assert_eq!(f.model.next_transaction, transaction);
    }
}

#[test]
fn launcher_observation_and_arrangement_never_grant_launch_authority() {
    for origin in [client_origin::human, client_origin::machine] {
        let mut f = fixture::new();
        let arranger = f
            .model
            .bind_client(
                &f.control,
                origin,
                client_grants::observe.union(client_grants::arrange),
            )
            .unwrap();
        let observer = f
            .model
            .bind_client(&f.control, origin, client_grants::observe)
            .unwrap();
        let revision = f.view().revision();
        assert_rejection(
            f.model
                .prepare(&observer, revision, desktop_command::toggle_launcher),
            desktop_error::grant_denied,
        );
        let ticket = f
            .model
            .prepare(&arranger, revision, desktop_command::toggle_launcher)
            .unwrap();
        f.commit(ticket);
        let revision = f.view().revision();
        for choice in launcher_choice::all {
            assert_rejection(
                f.model.prepare(
                    &arranger,
                    revision,
                    desktop_command::activate_launcher {
                        choice,
                        graphics: (choice == launcher_choice::graphics).then_some(graphics()),
                    },
                ),
                desktop_error::grant_denied,
            );
        }
        let manager = f
            .model
            .bind_client(
                &f.control,
                origin,
                client_grants::observe
                    .union(client_grants::manage)
                    .union(client_grants::arrange),
            )
            .unwrap();
        assert_rejection(
            f.model.prepare(
                &manager,
                revision,
                desktop_command::activate_launcher {
                    choice: launcher_choice::graphics,
                    graphics: Some(graphics()),
                },
            ),
            desktop_error::grant_denied,
        );
        let ticket = f
            .model
            .prepare(
                &manager,
                revision,
                desktop_command::activate_launcher {
                    choice: launcher_choice::welcome,
                    graphics: None,
                },
            )
            .unwrap();
        assert_eq!(f.commit(ticket).origin(), Some(origin));
        assert_eq!(f.view().all_windows().count(), 1);
    }
}

#[test]
fn popup_pointer_arms_only_its_button_and_enabled_visible_items() {
    let mut f = fixture::new();
    let id = f.open(launcher_choice::welcome);
    f.batch(&[
        desktop_command::set_pointer { x: 50, y: 50 },
        desktop_command::set_pointer_buttons(1),
        desktop_command::arm_pointer(desktop_target::close(id)),
    ]);
    f.apply(desktop_command::toggle_launcher);
    assert_eq!(f.view().pointer_armed(), None);
    let revision = f.view().revision();
    for target in [
        desktop_target::launcher_background,
        desktop_target::close(id),
        desktop_target::minimize(id),
        desktop_target::maximize(id),
        desktop_target::task(id),
        desktop_target::appearance_launcher,
        desktop_target::workspace(workspace_id::second),
        desktop_target::theme,
    ] {
        assert_rejection(
            f.model
                .prepare(&f.human, revision, desktop_command::arm_pointer(target)),
            desktop_error::invalid_pointer_target,
        );
    }
    for target in [
        desktop_target::launcher,
        desktop_target::launcher_item(launcher_choice::welcome),
    ] {
        f.apply(desktop_command::arm_pointer(target));
        assert_eq!(f.view().pointer_armed(), Some(target));
    }
    f.apply(desktop_command::dismiss_launcher);
    assert_eq!(f.view().pointer_armed(), None);
    assert_rejection(
        f.model.prepare(
            &f.human,
            f.view().revision(),
            desktop_command::arm_pointer(desktop_target::launcher_item(launcher_choice::welcome)),
        ),
        desktop_error::invalid_pointer_target,
    );
}

#[test]
fn enabled_state_checks_layout_and_exhaustion_without_mutating_the_view() {
    let mut f = fixture::sized(640, 480);
    for choice in [
        launcher_choice::welcome,
        launcher_choice::inspector,
        launcher_choice::activity,
    ] {
        f.open(choice);
    }
    f.apply(desktop_command::set_layout(layout_mode::vertical));
    let revision = f.view().revision();
    let windows = f.view().all_windows().collect::<Vec<_>>();
    assert!(!f.view().launcher_enabled(launcher_choice::appearance));
    assert!(!f.view().launcher_enabled(launcher_choice::graphics));
    assert!(f.view().launcher_enabled(launcher_choice::welcome));
    assert_eq!(f.view().revision(), revision);
    assert_eq!(f.view().all_windows().collect::<Vec<_>>(), windows);
    f.apply(desktop_command::set_layout(layout_mode::floating));
    f.model.committed.next_tile_order = u64::MAX;
    assert!(!f.view().launcher_enabled(launcher_choice::appearance));
    assert!(f.view().launcher_enabled(launcher_choice::welcome));
}

#[test]
fn stale_launcher_revision_and_multiple_launch_batch_cannot_change_state() {
    let mut f = fixture::new();
    f.apply(desktop_command::toggle_launcher);
    let stale = f.view().revision();
    f.apply(desktop_command::select_launcher(launcher_choice::activity));
    let command = desktop_command::activate_launcher {
        choice: launcher_choice::welcome,
        graphics: None,
    };
    assert_rejection(
        f.model.prepare(&f.human, stale, command),
        desktop_error::stale_revision,
    );
    assert_rejection(
        f.model
            .prepare_batch(&f.human, f.view().revision(), &[command, command]),
        desktop_error::invalid_batch,
    );
    assert!(f.view().launcher_open());
    assert_eq!(f.view().launcher_selection(), launcher_choice::activity);
    assert_eq!(f.view().all_windows().count(), 0);
}
