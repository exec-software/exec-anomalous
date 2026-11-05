use super::*;
use crate::paint::{rect, rgb, rounded_coverage, row_canvas};
use anomalous_desktop_theme::assets::{self, control_asset};
use anomalous_desktop_workspace::{
    appearance_choice, client_endpoint, client_grants, client_origin, desktop, desktop_command,
    desktop_control, layout_mode, render_view, theme_mode, update_ticket, workspace_id,
};

fn pixel(row: &[u8], x: u32) -> rgb {
    row[x as usize * 3..x as usize * 3 + 3].try_into().unwrap()
}

fn source_over(background: rgb, foreground: rgb, alpha: u8) -> rgb {
    let mut result = [0; 3];
    for channel in 0..3 {
        result[channel] = ((u32::from(background[channel]) * (255 - u32::from(alpha))
            + u32::from(foreground[channel]) * u32::from(alpha)
            + 127)
            / 255) as u8;
    }
    result
}

struct fixture {
    model: desktop,
    control: desktop_control,
    endpoint: client_endpoint,
}
impl fixture {
    fn new() -> Self {
        let (mut model, control, initial) = desktop::new_demo(frame_width, frame_height).unwrap();
        let endpoint = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        Self {
            model,
            control,
            endpoint,
        }
    }
    fn view(&self) -> render_view<'_> {
        self.model.observe(&self.endpoint).unwrap()
    }
    fn apply(&mut self, commands: &[desktop_command]) {
        let ticket = self
            .model
            .prepare_batch(&self.endpoint, self.view().revision(), commands)
            .unwrap();
        publish(&mut self.model, &self.control, ticket);
    }
    fn row(&self, y: u32) -> [u8; row_byte_count] {
        let mut row = [0; row_byte_count];
        compose_row(&self.view(), y, &mut row).unwrap();
        row
    }
}
fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    assert!(model.publish(control, ticket).is_ok());
}

#[test]
fn rounded_coverage_is_symmetric_clamped_and_empty_outside_its_bounds() {
    let mut partial = false;
    for (width, height) in [(1, 1), (3, 7), (24, 24), (100, 40)] {
        let bounds = rect::new(-12, 27, width, height);
        for radius in [-3, 0, 1, 6, 12, 40] {
            for y in -1..=height {
                for x in -1..=width {
                    let alpha = rounded_coverage(bounds, radius, bounds.x + x, bounds.y + y);
                    if x < 0 || y < 0 || x == width || y == height {
                        assert_eq!(alpha, 0);
                    } else {
                        assert_eq!(
                            alpha,
                            rounded_coverage(
                                bounds,
                                radius,
                                bounds.x + width - 1 - x,
                                bounds.y + y
                            )
                        );
                        assert_eq!(
                            alpha,
                            rounded_coverage(
                                bounds,
                                radius,
                                bounds.x + x,
                                bounds.y + height - 1 - y
                            )
                        );
                        if radius <= 0 {
                            assert_eq!(alpha, 255);
                        }
                        partial |= alpha > 0 && alpha < 255;
                    }
                }
            }
        }
    }
    assert!(partial);
    for bounds in [
        rect::new(0, 0, 0, 10),
        rect::new(0, 0, 10, 0),
        rect::new(-50, -50, -4, 10),
        rect::new(0, 0, 10, -1),
    ] {
        assert_eq!(rounded_coverage(bounds, 12, 0, 0), 0);
        let mut row = [73; row_byte_count];
        row_canvas::new(0, &mut row).rounded_frame(bounds, 12, 3, [1; 3], [2; 3]);
        assert_eq!(row, [73; row_byte_count]);
        row_canvas::new(0, &mut row).soft_shadow(bounds, 12, 16, 160, [1; 3], 12);
        assert_eq!(row, [73; row_byte_count]);
    }
}

#[test]
fn source_over_blending_retains_underlying_detail_and_clips_destination_edges() {
    let mut row = [0; row_byte_count];
    let mut canvas = row_canvas::new(10, &mut row);
    canvas.fill([120, 180, 240]);
    canvas.blend_pixel(-1, [255; 3], 255);
    canvas.blend_pixel(i64::from(frame_width), [255; 3], 255);
    canvas.blend_pixel(0, [1, 2, 3], 0);
    canvas.blend_pixel(1, [20, 30, 40], 128);
    canvas.blend_pixel(2, [20, 30, 40], 255);
    assert_eq!(pixel(&row, 0), [120, 180, 240]);
    assert_eq!(pixel(&row, 1), [70, 105, 140]);
    assert_eq!(pixel(&row, 2), [20, 30, 40]);
    assert_eq!(pixel(&row, frame_width - 1), [120, 180, 240]);
}

#[test]
fn shadow_shades_each_underlying_surface_without_replacing_its_color() {
    let bounds = rect::new(100, 100, 120, 80);
    let mut row = [0; row_byte_count];
    let mut canvas = row_canvas::new(185, &mut row);
    canvas.fill([200, 100, 50]);
    canvas.rectangle(rect::new(160, 0, 100, 768), [100, 200, 150]);
    canvas.soft_shadow(bounds, 6, 8, 96, [0; 3], 0);
    let left = pixel(&row, 130);
    let right = pixel(&row, 190);
    for (actual, before) in [(left, [200, 100, 50]), (right, [100, 200, 150])] {
        for channel in 0..3 {
            assert!(actual[channel] > 0 && actual[channel] < before[channel]);
        }
    }
    assert_ne!(left, right);
    assert_eq!(pixel(&row, 80), [200, 100, 50]);
    assert_eq!(pixel(&row, 240), [100, 200, 150]);
}

#[test]
fn transparent_window_corner_preserves_lower_pixels_and_lower_hit_target() {
    let mut f = fixture::new();
    f.apply(&[desktop_command::set_depth(false)]);
    let view = f.view();
    let top = view.windows().last().unwrap();
    let lower = view.windows().next().unwrap();
    let (x, y) = (top.bounds().x(), top.bounds().y());
    let id = top.id();
    assert!(lower.bounds().contains(x, y));
    assert_eq!(
        hit_test_chrome(&view, x, y),
        chrome_target::window(lower.id())
    );
    let before = pixel(&f.row(y), x);
    f.apply(&[desktop_command::minimize(id)]);
    assert_eq!(pixel(&f.row(y), x), before);
}

#[test]
fn legal_largest_window_shadow_cannot_overpaint_header_chrome() {
    let mut f = fixture::new();
    let view = f.view();
    let id = view.focused_window().unwrap().id();
    let mut settings = view.theme_settings();
    let mut metrics = view.theme_definition().metrics;
    metrics.workarea_top = metrics.header_height + 8;
    metrics.shadow_offset = 0;
    let mut frame = view.theme_definition().frame;
    frame.shadow_spread = 16;
    frame.shadow_opacity = 160;
    settings.overrides.metrics = Some(metrics);
    settings.overrides.frame = Some(frame);
    f.apply(&[
        desktop_command::set_theme_settings(settings),
        desktop_command::move_window {
            window: id,
            x: 32,
            y: metrics.workarea_top,
        },
    ]);
    let before = f.row(metrics.header_height - 1);
    f.apply(&[desktop_command::set_depth(false)]);
    assert_eq!(f.row(metrics.header_height - 1), before);
}

#[test]
fn controls_use_authored_vector_coverage_for_normal_hover_pressed_and_reset_states() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        for asset in [
            control_asset::minimize,
            control_asset::maximize,
            control_asset::close,
        ] {
            let mut f = fixture::new();
            f.apply(&[desktop_command::set_theme(theme)]);
            let view = f.view();
            let id = view.focused_window().unwrap().id();
            let target = match asset {
                control_asset::minimize => chrome_target::minimize(id),
                control_asset::maximize => chrome_target::maximize(id),
                _ => chrome_target::close(id),
            };
            let bounds = chrome_bounds(&view, target).unwrap();
            let available = bounds.width.min(bounds.height).saturating_sub(4);
            let size = if available >= 24 {
                24
            } else if available >= 20 {
                20
            } else {
                16
            };
            let origin_x = bounds.x + (bounds.width - size) / 2;
            let origin_y = bounds.y + (bounds.height - size) / 2;
            // Place the cursor below the icon raster, inside its control. This
            // tests the icon's entire native mask without covering it with cursor pixels.
            for state in 0..4 {
                if state != 0 {
                    f.apply(&[
                        desktop_command::set_pointer {
                            x: bounds.x + bounds.width / 2,
                            y: bounds.y + bounds.height - 2,
                        },
                        desktop_command::set_pointer_buttons(if state == 2 { 1 } else { 0 }),
                        if state == 2 {
                            desktop_command::arm_pointer(target)
                        } else {
                            desktop_command::cancel_pointer
                        },
                    ]);
                }
                let view = f.view();
                let definition = view.theme_definition();
                let (fill, ink) = if state == 0 {
                    (
                        definition.colors.active_title,
                        definition.colors.active_text,
                    )
                } else if asset == control_asset::close {
                    (
                        if state == 2 {
                            definition.decorations.danger_pressed
                        } else {
                            definition.decorations.danger_hover
                        },
                        definition.decorations.danger_ink,
                    )
                } else {
                    (
                        if state == 2 {
                            definition.decorations.control_pressed
                        } else {
                            definition.decorations.control_hover
                        },
                        definition.decorations.control_ink,
                    )
                };
                if state != 0 {
                    assert_eq!(
                        hit_test_chrome(&view, view.pointer().x(), view.pointer().y()),
                        target
                    );
                }
                for y in 0..size {
                    let row = f.row(origin_y + y);
                    for x in 0..size {
                        assert_eq!(
                            pixel(&row, origin_x + x),
                            source_over(fill, ink, assets::coverage(asset, x, y, size)),
                            "theme {theme:?} asset {asset:?} state {state} pixel {x},{y}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn every_control_requires_its_own_arm_and_current_hover_for_pressed_pixels() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        let mut f = fixture::new();
        f.apply(&[
            desktop_command::set_theme(theme),
            desktop_command::open_appearance,
        ]);
        let id = f.view().focused_window().unwrap().id();
        let targets = [
            chrome_target::minimize(id),
            chrome_target::maximize(id),
            chrome_target::close(id),
            chrome_target::task(id),
            chrome_target::launcher,
            chrome_target::appearance_launcher,
            chrome_target::workspace(workspace_id::first),
            chrome_target::workspace(workspace_id::second),
        ];
        for target in targets.into_iter().chain(
            appearance_choice::all
                .into_iter()
                .map(|choice| chrome_target::appearance_choice(id, choice)),
        ) {
            let bounds = chrome_bounds(&f.view(), target).unwrap();
            let x = bounds.x + bounds.width / 2;
            let pointer_y = bounds.y + bounds.height - 2;
            let sample_y = bounds.y + 3;
            assert_eq!(hit_test_chrome(&f.view(), x, pointer_y), target);
            let sample = |f: &fixture| pixel(&f.row(sample_y), x);
            let at = desktop_command::set_pointer { x, y: pointer_y };
            let away = desktop_command::set_pointer { x: 0, y: 100 };

            f.apply(&[away, desktop_command::set_pointer_buttons(0)]);
            let normal = sample(&f);
            f.apply(&[at, desktop_command::set_pointer_buttons(0)]);
            let hovered = sample(&f);

            // Holding from outside, or arming another control, cannot acquire
            // this control's pressed appearance just by moving over its pixels.
            f.apply(&[at, desktop_command::set_pointer_buttons(1)]);
            assert_eq!(sample(&f), hovered, "unarmed {theme:?} {target:?}");
            let other = if target == chrome_target::appearance_launcher {
                chrome_target::workspace(workspace_id::first)
            } else {
                chrome_target::appearance_launcher
            };
            f.apply(&[
                at,
                desktop_command::set_pointer_buttons(1),
                desktop_command::arm_pointer(other),
            ]);
            assert_eq!(sample(&f), hovered, "other arm {theme:?} {target:?}");

            f.apply(&[
                at,
                desktop_command::set_pointer_buttons(1),
                desktop_command::arm_pointer(target),
            ]);
            let definition = f.view().theme_definition();
            let pressed = match target {
                chrome_target::close(_) => definition.decorations.danger_pressed,
                chrome_target::workspace(workspace_id::first) => {
                    definition.decorations.primary_pressed
                }
                _ => definition.decorations.control_pressed,
            };
            assert_eq!(sample(&f), pressed, "armed {theme:?} {target:?}");

            // The arm survives an excursion, while the visible depression
            // follows the current hit target and reappears only on reentry.
            f.apply(&[
                away,
                desktop_command::set_pointer_buttons(1),
                desktop_command::arm_pointer(target),
            ]);
            assert_eq!(sample(&f), normal, "left {theme:?} {target:?}");
            f.apply(&[
                at,
                desktop_command::set_pointer_buttons(1),
                desktop_command::arm_pointer(target),
            ]);
            assert_eq!(sample(&f), pressed, "reentered {theme:?} {target:?}");

            // Any semantic transaction without explicit rearming cancels the
            // visual arm, even while physical left-button state stays down.
            f.apply(&[desktop_command::cancel_pointer]);
            assert_eq!(sample(&f), hovered, "canceled {theme:?} {target:?}");
            for buttons in [2, 3, 4, 5, 6, 7, 0] {
                f.apply(&[at, desktop_command::set_pointer_buttons(buttons)]);
                assert_eq!(
                    sample(&f),
                    hovered,
                    "buttons={buttons} {theme:?} {target:?}"
                );
            }
        }
    }
}

#[test]
fn disabled_maximize_remains_visible_and_never_acquires_a_hover_target() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        let mut f = fixture::new();
        f.apply(&[
            desktop_command::set_theme(theme),
            desktop_command::set_layout(layout_mode::horizontal),
        ]);
        let view = f.view();
        let id = view.focused_window().unwrap().id();
        let bounds = chrome_bounds(&view, chrome_target::maximize(id)).unwrap();
        let x = bounds.x + bounds.width / 2;
        let y = bounds.y + bounds.height / 2;
        assert_eq!(hit_test_chrome(&view, x, y), chrome_target::window(id));
        let title = view.theme_definition().colors.active_title;
        let mut different = 0;
        for row_y in bounds.y..bounds.y + bounds.height {
            let row = f.row(row_y);
            for col in bounds.x..bounds.x + bounds.width {
                different += usize::from(pixel(&row, col) != title);
            }
        }
        assert!(different > 4, "disabled control vanished for {theme:?}");
    }
}

#[test]
fn minimum_controls_keep_opaque_vector_cores_with_accessible_contrast() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        let mut f = fixture::new();
        f.apply(&[desktop_command::set_theme(theme)]);
        let view = f.view();
        let mut settings = view.theme_settings();
        let mut metrics = view.theme_definition().metrics;
        metrics.title_height = 28;
        metrics.control_size = 20;
        settings.overrides.metrics = Some(metrics);
        f.apply(&[desktop_command::set_theme_settings(settings)]);
        let view = f.view();
        let id = view.focused_window().unwrap().id();
        let definition = view.theme_definition();
        assert!(anomalous_desktop_theme::contrast_at_least(
            definition.colors.active_text,
            definition.colors.active_title,
            45
        ));
        for (target, asset) in [
            (chrome_target::minimize(id), control_asset::minimize),
            (chrome_target::maximize(id), control_asset::maximize),
            (chrome_target::close(id), control_asset::close),
        ] {
            let bounds = chrome_bounds(&view, target).unwrap();
            assert_eq!(bounds.width, 20);
            assert_eq!(bounds.height, 20);
            let mut opaque = 0;
            for y in 0..16 {
                let row = f.row(bounds.y + 2 + y);
                for x in 0..16 {
                    if assets::coverage(asset, x, y, 16) == 255 {
                        assert_eq!(pixel(&row, bounds.x + 2 + x), definition.colors.active_text);
                        opaque += 1;
                    }
                }
            }
            assert!(opaque >= 2, "no opaque core for {asset:?} in {theme:?}");
        }
    }
}

#[test]
fn smallest_resize_grip_paints_only_within_its_shared_action_region() {
    let mut f = fixture::new();
    let view = f.view();
    let id = view.focused_window().unwrap().id();
    let mut settings = view.theme_settings();
    let mut metrics = view.theme_definition().metrics;
    metrics.resize_grip = 8;
    let mut frame = view.theme_definition().frame;
    frame.radius = 12;
    settings.overrides.metrics = Some(metrics);
    settings.overrides.frame = Some(frame);
    f.apply(&[desktop_command::set_theme_settings(settings)]);
    let view = f.view();
    let window = view.window(id).unwrap().bounds();
    let grip = chrome_bounds(&view, chrome_target::resize(id)).unwrap();
    assert_eq!((grip.width, grip.height), (8, 8));
    let ink = view.theme_definition().colors.secondary_text;
    let mut marks = 0;
    // The content padding excludes text from this corner. Every secondary-ink
    // pixel here belongs to the grip, including dots outside an undersized clip.
    for y in window.y() + window.height() - 20..window.y() + window.height() {
        let row = f.row(y);
        for x in window.x() + window.width() - 20..window.x() + window.width() {
            if pixel(&row, x) == ink {
                assert!(grip.contains(x, y), "grip ink outside target at {x},{y}");
                assert_eq!(
                    hit_test_chrome(&view, x, y),
                    chrome_target::resize(id),
                    "grip ink has no resize target at {x},{y}; bottom-right={},{}",
                    window.x() + window.width(),
                    window.y() + window.height()
                );
                marks += 1;
            }
        }
    }
    assert!(marks > 0, "smallest legal grip must remain visible");
}

#[test]
fn thick_focused_border_keeps_its_inner_keyline_over_a_contrasting_title() {
    let mut f = fixture::new();
    let view = f.view();
    let id = view.focused_window().unwrap().id();
    let mut settings = view.theme_settings();
    let mut colors = view.theme_definition().colors;
    colors.active_title = [41, 38, 32];
    colors.active_text = colors.focus_inner;
    let mut frame = view.theme_definition().frame;
    frame.border_width = 3;
    frame.radius = 12;
    settings.overrides.colors = Some(colors);
    settings.overrides.frame = Some(frame);
    f.apply(&[desktop_command::set_theme_settings(settings)]);
    let view = f.view();
    let bounds = view.window(id).unwrap().bounds();
    let title = chrome_bounds(&view, chrome_target::titlebar(id)).unwrap();
    assert!(anomalous_desktop_theme::contrast_at_least(
        colors.focus_inner,
        colors.active_title,
        45
    ));
    // The focused outer border adds one pixel to the three-pixel profile. Its
    // inner keyline is therefore at inset four, shared with the title's bounds.
    assert_eq!(
        pixel(&f.row(bounds.y() + 4), bounds.x() + bounds.width() / 2),
        colors.focus_inner
    );
    let middle_title = title.y + title.height / 2;
    assert_eq!(
        pixel(&f.row(middle_title), bounds.x() + 4),
        colors.focus_inner
    );
    assert_eq!(
        pixel(&f.row(middle_title), bounds.x() + bounds.width() - 5),
        colors.focus_inner
    );
}

#[test]
fn row_culling_preserves_complete_frames_at_custom_metric_extremes() {
    // Compare against the complete paint path without surface-level culling,
    // retaining primitive clipping. This keeps the optimization check valid
    // when intentional UI changes supersede old whole-frame hash fixtures.
    // The original eighteen metric/palette/scene cases remain, plus a launcher
    // over a maximized window at both extremes, including a clipped 2x cursor.
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        for large in [false, true] {
            for scene in 0..4 {
                let mut f = fixture::new();
                let view = f.view();
                let mut settings = view.theme_settings();
                settings.theme = theme;
                settings.font_scale = if large { 2 } else { 1 };
                settings.cursor_size = if large { 2 } else { 1 };
                let mut metrics = view.theme_definition().metrics;
                if large {
                    metrics.header_height = 64;
                    metrics.panel_height = 64;
                    metrics.workarea_top = 96;
                    metrics.workarea_bottom_inset = 128;
                    metrics.workarea_margin = 24;
                    metrics.title_height = 56;
                    metrics.control_size = 36;
                    metrics.resize_grip = 24;
                    metrics.tile_gap = 20;
                    metrics.shadow_offset = 12;
                } else {
                    metrics.header_height = 32;
                    metrics.panel_height = 40;
                    metrics.workarea_top = 48;
                    metrics.workarea_bottom_inset = 64;
                    metrics.workarea_margin = 8;
                    metrics.title_height = 28;
                    metrics.control_size = 20;
                    metrics.resize_grip = 8;
                    metrics.tile_gap = 4;
                    metrics.shadow_offset = 0;
                }
                settings.overrides.metrics = Some(metrics);
                let mut commands: std::vec::Vec<_> = view
                    .windows()
                    .map(|window| desktop_command::close(window.id()))
                    .collect();
                commands.push(desktop_command::set_theme_settings(settings));
                commands.push(desktop_command::set_pointer { x: 1023, y: 767 });
                f.apply(&commands);
                if scene != 0 {
                    f.apply(&[if scene == 1 {
                        desktop_command::open_appearance
                    } else {
                        desktop_command::create {
                            content: anomalous_desktop_workspace::demo_content::welcome,
                            workspace: workspace_id::first,
                            bounds: f.view().work_area(),
                        }
                    }]);
                    if scene >= 2 {
                        let id = f.view().windows().next().unwrap().id();
                        f.apply(&[desktop_command::maximize(id)]);
                    }
                }
                if scene == 3 {
                    f.apply(&[desktop_command::toggle_launcher]);
                }
                let view = f.view();
                let frame = composition_frame::try_new(&view, &[]).unwrap();
                for row in 0..frame_height {
                    let mut expected = [0; row_byte_count];
                    crate::render::compose_unculled(
                        &frame,
                        &mut row_canvas::new(row, &mut expected),
                    );
                    assert_eq!(
                        f.row(row),
                        expected,
                        "theme={theme:?} large={large} scene={scene} row={row}"
                    );
                }
            }
        }
    }
}
