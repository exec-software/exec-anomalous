use super::*;
use crate::paint::{rect, row_canvas};
use anomalous_desktop_workspace::{
    client_endpoint, client_grants, client_origin, demo_content, density_mode, desktop,
    desktop_command, desktop_control, layout_mode, render_view, theme_mode, update_ticket,
    window_id, window_rect, workspace_id,
};
use anomalous_window_surface::{frame, surface_descriptor};

fn pixel(row: &[u8], x: u32) -> [u8; 3] {
    row[x as usize * 3..x as usize * 3 + 3].try_into().unwrap()
}

fn publish(
    model: &mut desktop,
    control: &desktop_control,
    ticket: update_ticket,
) -> Option<window_id> {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    model
        .publish(control, ticket)
        .unwrap_or_else(|_| panic!("validated publication"))
        .created_window()
}

struct fixture {
    model: desktop,
    control: desktop_control,
    endpoint: client_endpoint,
    window: window_id,
}
impl fixture {
    fn new(descriptor: surface_descriptor) -> Self {
        let (mut model, control, initial) = desktop::new(frame_width, frame_height).unwrap();
        let endpoint = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        let revision = model.observe(&endpoint).unwrap().revision();
        let ticket = model
            .prepare(
                &endpoint,
                revision,
                desktop_command::open_graphics(descriptor),
            )
            .unwrap();
        let window = publish(&mut model, &control, ticket).unwrap();
        Self {
            model,
            control,
            endpoint,
            window,
        }
    }

    fn view(&self) -> render_view<'_> {
        self.model.observe(&self.endpoint).unwrap()
    }

    fn apply(&mut self, command: desktop_command) -> Option<window_id> {
        let revision = self.view().revision();
        let ticket = self
            .model
            .prepare(&self.endpoint, revision, command)
            .unwrap();
        publish(&mut self.model, &self.control, ticket)
    }

    fn attach(&mut self, window: window_id, descriptor: surface_descriptor) {
        let owner = self
            .model
            .bind_surface(&self.control, &self.endpoint, window)
            .unwrap();
        let revision = self.view().revision();
        let ticket = self
            .model
            .prepare_surface(&owner, revision, descriptor)
            .unwrap();
        publish(&mut self.model, &self.control, ticket);
    }
}

#[test]
fn raster_stretch_clips_every_edge_and_never_samples_stride_padding() {
    let pixels = [
        1, 2, 3, 4, 5, 6, 211, 211, 211, 7, 8, 9, 10, 11, 12, 212, 212, 212,
    ];
    let source = frame::try_new(2, 2, 9, "Four public pixels", &pixels).unwrap();
    for bounds in [
        rect::new(-4, 10, 12, 8),
        rect::new(1020, 10, 12, 8),
        rect::new(20, 10, 1, 1),
        rect::new(20, 10, 0, 5),
        rect::new(20, 10, 5, 0),
    ] {
        for y in 9..=18 {
            let mut row = [17; row_byte_count];
            row_canvas::new(y, &mut row).surface(bounds, &source);
            for x in 0..frame_width {
                let inside = bounds.width > 0
                    && bounds.height > 0
                    && i64::from(x) >= bounds.x
                    && i64::from(x) < bounds.x + bounds.width
                    && i64::from(y) >= bounds.y
                    && i64::from(y) < bounds.y + bounds.height;
                let expected = if inside {
                    let sx = (i64::from(x) - bounds.x) * 2 / bounds.width;
                    let sy = (i64::from(y) - bounds.y) * 2 / bounds.height;
                    let offset = (sy * 9 + sx * 3) as usize;
                    pixels[offset..offset + 3].try_into().unwrap()
                } else {
                    [17; 3]
                };
                assert_eq!(pixel(&row, x), expected, "x={x} y={y}");
            }
        }
    }
}

#[test]
fn complete_frame_preflight_rejects_missing_duplicate_extraneous_and_wrong_content() {
    let source = frame::try_new(1, 1, 3, "First pixel", &[1, 2, 3]).unwrap();
    let other = frame::try_new(1, 1, 3, "First pixel", &[1, 2, 3]).unwrap();
    let mut scene = fixture::new(source.descriptor());
    let binding = surface_binding::new(scene.window, &source);
    let view = scene.view();
    assert!(matches!(
        composition_frame::try_new(&view, &[]),
        Err(composition_error::missing_surface_binding)
    ));
    assert!(matches!(
        composition_frame::try_new(&view, &[binding, binding]),
        Err(composition_error::duplicate_surface_binding)
    ));
    assert!(matches!(
        composition_frame::try_new(&view, &[binding; 5]),
        Err(composition_error::surface_capacity_exceeded)
    ));
    assert!(matches!(
        composition_frame::try_new(&view, &[surface_binding::new(scene.window, &other)]),
        Err(composition_error::surface_descriptor_mismatch)
    ));
    // A fresh identity is distinct even when every pixel and other field agrees.
    assert_ne!(source.descriptor().id(), other.descriptor().id());
    let mut row = [93; row_byte_count];
    assert_eq!(
        compose_row(&view, 300, &mut row),
        Err(composition_error::missing_surface_binding)
    );
    assert!(row.iter().all(|byte| *byte == 93));

    let unattached = scene
        .apply(desktop_command::create {
            content: demo_content::activity,
            workspace: workspace_id::first,
            bounds: window_rect::try_new(40, 90, 200, 180).unwrap(),
        })
        .unwrap();
    assert!(matches!(
        composition_frame::try_new(
            &scene.view(),
            &[binding, surface_binding::new(unattached, &other)]
        ),
        Err(composition_error::extraneous_surface_binding)
    ));
}

#[test]
fn attached_hidden_windows_still_require_their_exact_frames() {
    let source = frame::try_new(1, 1, 3, "Opaque magenta", &[251, 2, 199]).unwrap();
    let mut scene = fixture::new(source.descriptor());
    let binding = surface_binding::new(scene.window, &source);
    let window = scene.view().window(scene.window).unwrap();
    let viewport = client_viewport(&scene.view(), window);
    let x = viewport.x + viewport.width / 2;
    let y = viewport.y + viewport.height / 2;
    for command in [
        desktop_command::minimize(scene.window),
        desktop_command::restore(scene.window),
        desktop_command::switch_workspace(workspace_id::second),
    ] {
        scene.apply(command);
        let view = scene.view();
        assert!(matches!(
            composition_frame::try_new(&view, &[]),
            Err(composition_error::missing_surface_binding)
        ));
        let mut row = [0; row_byte_count];
        composition_frame::try_new(&view, &[binding])
            .unwrap()
            .compose_row(y, &mut row)
            .unwrap();
        if !matches!(command, desktop_command::restore(_)) {
            assert_ne!(pixel(&row, x), [251, 2, 199]);
        }
    }
}

#[test]
fn window_reuse_does_not_admit_the_closed_incarnations_binding() {
    let source = frame::try_new(1, 1, 3, "Public pixel", &[1, 2, 3]).unwrap();
    let mut scene = fixture::new(source.descriptor());
    let old = scene.window;
    scene.apply(desktop_command::close(old));
    let new = scene
        .apply(desktop_command::open_graphics(source.descriptor()))
        .unwrap();
    assert_eq!(old.slot(), new.slot());
    assert_ne!(old.generation(), new.generation());
    assert!(matches!(
        composition_frame::try_new(&scene.view(), &[surface_binding::new(old, &source)]),
        Err(composition_error::extraneous_surface_binding)
    ));
    assert!(
        composition_frame::try_new(&scene.view(), &[surface_binding::new(new, &source)]).is_ok()
    );
}

#[test]
fn surfaces_obey_the_shared_client_viewport_across_theme_size_and_layout_changes() {
    let source = frame::try_new(1, 1, 3, "Opaque magenta", &[251, 2, 199]).unwrap();
    let mut scene = fixture::new(source.descriptor());
    let binding = surface_binding::new(scene.window, &source);
    for command in [
        desktop_command::set_theme(theme_mode::dark),
        desktop_command::set_theme(theme_mode::high_contrast),
        desktop_command::set_theme(theme_mode::light),
        desktop_command::set_density(density_mode::compact),
        desktop_command::resize_window {
            window: scene.window,
            width: 120,
            height: 96,
        },
        desktop_command::maximize(scene.window),
        desktop_command::set_layout(layout_mode::horizontal),
    ] {
        scene.apply(command);
        let view = scene.view();
        let window = view.window(scene.window).unwrap();
        let viewport = client_viewport(&view, window);
        assert!(viewport.width > 0 && viewport.height > 0);
        let y = viewport.y + viewport.height / 2;
        let mut row = [0; row_byte_count];
        composition_frame::try_new(&view, &[binding])
            .unwrap()
            .compose_row(y, &mut row)
            .unwrap();
        assert_ne!(pixel(&row, viewport.x - 1), [251, 2, 199]);
        assert_ne!(pixel(&row, viewport.x + viewport.width), [251, 2, 199]);
        for x in viewport.x..viewport.x + viewport.width {
            assert_eq!(pixel(&row, x), [251, 2, 199]);
        }
        composition_frame::try_new(&view, &[binding])
            .unwrap()
            .compose_row(viewport.y - 1, &mut row)
            .unwrap();
        assert_ne!(pixel(&row, viewport.x), [251, 2, 199]);
        composition_frame::try_new(&view, &[binding])
            .unwrap()
            .compose_row(viewport.y + viewport.height, &mut row)
            .unwrap();
        assert_ne!(pixel(&row, viewport.x), [251, 2, 199]);
    }
}

#[test]
fn generic_surface_pixels_follow_window_stacking_and_never_cover_chrome() {
    let red = frame::try_new(1, 1, 3, "Red content", &[241, 1, 2]).unwrap();
    let blue = frame::try_new(1, 1, 3, "Blue content", &[3, 4, 242]).unwrap();
    let mut scene = fixture::new(red.descriptor());
    scene.apply(desktop_command::move_window {
        window: scene.window,
        x: 100,
        y: 100,
    });
    scene.apply(desktop_command::resize_window {
        window: scene.window,
        width: 360,
        height: 280,
    });
    let second = scene
        .apply(desktop_command::create {
            content: demo_content::graphics,
            workspace: workspace_id::first,
            bounds: window_rect::try_new(160, 150, 300, 230).unwrap(),
        })
        .unwrap();
    scene.attach(second, blue.descriptor());
    let bindings = [
        surface_binding::new(scene.window, &red),
        surface_binding::new(second, &blue),
    ];
    let mut row = [0; row_byte_count];
    composition_frame::try_new(&scene.view(), &bindings)
        .unwrap()
        .compose_row(260, &mut row)
        .unwrap();
    assert_eq!(pixel(&row, 250), [3, 4, 242]);
    scene.apply(desktop_command::raise(scene.window));
    composition_frame::try_new(&scene.view(), &bindings)
        .unwrap()
        .compose_row(260, &mut row)
        .unwrap();
    assert_eq!(pixel(&row, 250), [241, 1, 2]);
    composition_frame::try_new(&scene.view(), &bindings)
        .unwrap()
        .compose_row(110, &mut row)
        .unwrap();
    assert_ne!(pixel(&row, 250), [241, 1, 2]);
}

#[test]
fn malformed_output_rows_leave_all_bytes_unchanged_with_attached_content() {
    let source = frame::try_new(1, 1, 3, "Public pixel", &[1, 2, 3]).unwrap();
    let scene = fixture::new(source.descriptor());
    let view = scene.view();
    let content =
        composition_frame::try_new(&view, &[surface_binding::new(scene.window, &source)]).unwrap();
    for y in [frame_height, u32::MAX] {
        let mut row = [29; row_byte_count];
        assert_eq!(
            content.compose_row(y, &mut row),
            Err(composition_error::row_out_of_bounds)
        );
        assert!(row.iter().all(|byte| *byte == 29));
    }
    for length in [0, 1, row_byte_count - 1, row_byte_count + 1] {
        let mut row = std::vec![37; length];
        assert_eq!(
            content.compose_row(300, &mut row),
            Err(composition_error::row_size_mismatch)
        );
        assert!(row.iter().all(|byte| *byte == 37));
    }
}

#[test]
fn canonical_cursor_composes_after_opaque_client_pixels() {
    let source = frame::try_new(1, 1, 3, "Opaque magenta", &[251, 2, 199]).unwrap();
    let mut scene = fixture::new(source.descriptor());
    let viewport = client_viewport(&scene.view(), scene.view().window(scene.window).unwrap());
    let x = viewport.x + 10;
    let y = viewport.y + 10;
    scene.apply(desktop_command::set_pointer { x, y });
    let view = scene.view();
    let mut row = [0; row_byte_count];
    composition_frame::try_new(&view, &[surface_binding::new(scene.window, &source)])
        .unwrap()
        .compose_row(y + 2, &mut row)
        .unwrap();
    let colors = view.theme_definition().colors;
    assert_eq!(pixel(&row, x), colors.cursor_outline);
    assert_eq!(pixel(&row, x + 1), colors.cursor_fill);
    assert_eq!(pixel(&row, x + 2), colors.cursor_outline);
    assert_eq!(pixel(&row, x + 3), [251, 2, 199]);
}
