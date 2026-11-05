use super::*;
use crate::paint::{rect, rgb, row_canvas};
use anomalous_desktop_workspace::{density_mode, layout_mode, theme_mode};
fn colors(theme: theme_mode) -> anomalous_desktop_theme::semantic_colors {
    anomalous_desktop_theme::builtin(theme, density_mode::comfortable).colors
}
use anomalous_desktop_workspace::desktop;
use std::vec;

fn pixel(row: &[u8], x: usize) -> rgb {
    row[x * 3..x * 3 + 3].try_into().expect("one RGB pixel")
}

#[test]
fn malformed_rows_preserve_the_entire_destination() {
    let (desktop, _control, ticket) = desktop::new_demo(frame_width, frame_height).expect("demo");
    let view = desktop.candidate(&ticket).expect("candidate");
    for length in [0, 1, row_byte_count - 1, row_byte_count + 1] {
        let mut output = vec![0xa7; length];
        assert_eq!(
            compose_row(&view, 0, &mut output),
            Err(composition_error::row_size_mismatch)
        );
        assert!(output.iter().all(|byte| *byte == 0xa7));
    }
    for row in [frame_height, u32::MAX] {
        let mut output = [0x5b; row_byte_count];
        assert_eq!(
            compose_row(&view, row, &mut output),
            Err(composition_error::row_out_of_bounds)
        );
        assert!(output.iter().all(|byte| *byte == 0x5b));
    }
}

#[test]
fn clipping_uses_complete_rect_intersection_and_preserves_other_pixels() {
    let mut output = [0; row_byte_count];
    let mut canvas = row_canvas::new(10, &mut output);
    canvas.fill([7, 8, 9]);
    canvas.rectangle(rect::new(-50, 8, 60, 5), [40, 50, 60]);
    canvas.rectangle(rect::new(1020, 0, 100, 768), [70, 80, 90]);
    canvas.rectangle(rect::new(100, 11, 30, 20), [255; 3]);
    canvas.rectangle(rect::new(100, 0, 30, 10), [255; 3]);
    canvas.rectangle(rect::new(100, 0, -1, 100), [255; 3]);
    for x in 0..frame_width as usize {
        let expected = match x {
            0..10 => [40, 50, 60],
            1020..1024 => [70, 80, 90],
            _ => [7, 8, 9],
        };
        assert_eq!(pixel(&output, x), expected, "pixel {x}");
    }
}

#[test]
fn later_opaque_surface_covers_earlier_text_and_clips_its_own_text() {
    let mut output = [0; row_byte_count];
    let viewport = rect::new(0, 0, 1024, 768);
    let mut canvas = row_canvas::new(20, &mut output);
    canvas.fill([0; 3]);
    canvas.rectangle(rect::new(0, 0, 200, 100), [10, 20, 30]);
    canvas.text(4, 20, 2, "AAAAAAAAAAAAAAAA", [255; 3], viewport);
    canvas.rectangle(rect::new(20, 10, 50, 50), [60, 70, 80]);
    canvas.text(
        20,
        20,
        2,
        "XXXXXXXXXXXXXXX",
        [200; 3],
        rect::new(20, 10, 12, 50),
    );
    for x in 32..70 {
        assert_eq!(pixel(&output, x), [60, 70, 80]);
    }
    assert_eq!(pixel(&output, 20), [200; 3]);
    assert_eq!(pixel(&output, 24), [60, 70, 80]);
}

#[test]
fn visible_window_order_controls_overlap_in_the_composed_view() {
    let (desktop, _control, ticket) = desktop::new_demo(frame_width, frame_height).expect("demo");
    let view = desktop.candidate(&ticket).expect("candidate");
    let windows: std::vec::Vec<_> = view.windows().collect();
    assert_eq!(
        windows.len(),
        2,
        "initial public demo is two overlapping windows"
    );
    let top = windows.last().expect("topmost window");
    assert!(top.focused());
    let bounds = top.bounds();
    let below = windows[0].bounds();
    let y = bounds.y();
    let overlap_x = bounds.x().max(below.x()) + 8;
    assert!(y >= below.y() && y < below.y() + below.height());
    assert!(overlap_x < bounds.x() + bounds.width() && overlap_x < below.x() + below.width());
    let mut output = [0; row_byte_count];
    compose_row(&view, y, &mut output).expect("row");
    assert_eq!(
        pixel(&output, overlap_x as usize),
        colors(view.theme()).focus_outer
    );
    compose_row(&view, y + 2, &mut output).expect("row");
    assert_eq!(
        pixel(&output, overlap_x as usize),
        colors(view.theme()).focus_inner
    );
}

#[test]
fn raising_the_lower_canonical_window_replaces_the_previously_visible_title() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let view = desktop.observe(&endpoint).expect("committed");
    let lower = view.windows().next().expect("lower window");
    let upper = view.windows().last().expect("upper window");
    // This overlap pixel lies inside the upper title but to the right of both
    // windows' body text. The expected colors are independent of paint order.
    let x = lower.bounds().x() + lower.bounds().width() - 10;
    let y = upper.bounds().y() + 32;
    let mut output = [0; row_byte_count];
    compose_row(&view, y, &mut output).expect("before focus");
    assert_eq!(
        pixel(&output, x as usize),
        colors(view.theme()).active_title
    );
    let ticket = desktop
        .prepare(
            &endpoint,
            view.revision(),
            desktop_command::focus(lower.id()),
        )
        .expect("focus lower window");
    let view = desktop.candidate(&ticket).expect("candidate");
    assert_eq!(view.windows().last().expect("raised").id(), lower.id());
    compose_row(&view, y, &mut output).expect("after focus");
    assert_eq!(pixel(&output, x as usize), colors(view.theme()).surface);
}

#[test]
fn focus_has_nested_geometry_and_literal_text_beyond_color() {
    let (desktop, _control, ticket) = desktop::new_demo(frame_width, frame_height).expect("demo");
    let view = desktop.candidate(&ticket).expect("candidate");
    let focused = view
        .windows()
        .find(|window| window.focused())
        .expect("focus");
    let bounds = focused.bounds();
    let mut output = [0; row_byte_count];
    // Independently inspect two outer rows: different nested borders survive
    // grayscale because their lightness differs, independent of accent hue.
    compose_row(&view, bounds.y(), &mut output).expect("row");
    let outer = pixel(&output, bounds.x() as usize + 8);
    compose_row(&view, bounds.y() + 2, &mut output).expect("row");
    let inner = pixel(&output, bounds.x() as usize + 8);
    assert!(contrast(outer, inner) >= 3.0);
    let title = chrome_bounds(&view, chrome_target::titlebar(focused.id())).expect("title");
    let label_y = title.y + (title.height - 7) / 2;
    compose_row(&view, label_y, &mut output).expect("label row");
    let label_x = title.x + title.width - 38;
    // The first row of the F is five foreground pixels; this is a literal
    // focus word, not an independently invented action or status indicator.
    for x in label_x..label_x + 5 {
        assert_eq!(pixel(&output, x as usize), colors(view.theme()).active_text);
    }
}

#[test]
fn all_rows_are_deterministic_and_do_not_write_outside_the_caller_slice() {
    let (desktop, _control, ticket) = desktop::new_demo(frame_width, frame_height).expect("demo");
    let view = desktop.candidate(&ticket).expect("candidate");
    for row in 0..frame_height {
        let mut first = [0xa5; row_byte_count + 16];
        let mut second = [0x72; row_byte_count];
        compose_row(&view, row, &mut first[8..row_byte_count + 8]).expect("first row");
        compose_row(&view, row, &mut second).expect("second row");
        assert_eq!(&first[8..row_byte_count + 8], second.as_slice());
        assert!(
            first[..8]
                .iter()
                .chain(first[row_byte_count + 8..].iter())
                .all(|value| *value == 0xa5)
        );
    }
}

fn luminance(color: rgb) -> f64 {
    let channel = |value: u8| {
        let normalized = f64::from(value) / 255.0;
        if normalized <= 0.04045 {
            normalized / 12.92
        } else {
            ((normalized + 0.055) / 1.055).powf(2.4)
        }
    };
    channel(color[0]) * 0.2126 + channel(color[1]) * 0.7152 + channel(color[2]) * 0.0722
}

fn contrast(first: rgb, second: rgb) -> f64 {
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

#[test]
fn every_palette_text_pair_meets_four_point_five_contrast() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        let palette = colors(theme);
        for (foreground, background) in [
            (palette.text, palette.surface),
            (palette.text, palette.panel),
            (palette.text, palette.inactive_title),
            (palette.text, palette.background),
            (palette.secondary_text, palette.surface),
            (palette.secondary_text, palette.panel),
            (palette.secondary_text, palette.background),
            (palette.active_text, palette.active_title),
            (palette.accent, palette.surface),
            (palette.accent, palette.panel),
        ] {
            assert!(
                contrast(foreground, background) >= 4.5,
                "text contrast {}",
                contrast(foreground, background)
            );
        }
        assert!(contrast(palette.focus_inner, palette.focus_outer) >= 3.0);
    }
}

#[test]
fn unsupported_canonical_viewports_leave_the_row_unchanged() {
    let (desktop, _control, ticket) = desktop::new_demo(800, 600).expect("canonical viewport");
    let view = desktop.candidate(&ticket).expect("candidate");
    let mut output = [0xe3; row_byte_count];
    assert_eq!(
        compose_row(&view, 0, &mut output),
        Err(composition_error::unsupported_viewport)
    );
    assert!(output.iter().all(|byte| *byte == 0xe3));
}

fn publish(
    desktop: &mut desktop,
    control: &anomalous_desktop_workspace::desktop_control,
    ticket: anomalous_desktop_workspace::update_ticket,
) {
    desktop.begin(control, &ticket).expect("begin");
    desktop
        .attest_presented(control, &ticket)
        .expect("host assertion");
    assert!(desktop.publish(control, ticket).is_ok());
}

#[test]
fn palette_density_and_layout_changes_render_the_same_canonical_candidate() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let mut row = [0; row_byte_count];
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        let revision = desktop.observe(&endpoint).expect("committed").revision();
        let ticket = desktop
            .prepare(&endpoint, revision, desktop_command::set_theme(theme))
            .expect("theme");
        let view = desktop.candidate(&ticket).expect("candidate");
        assert_eq!(view.theme(), theme);
        compose_row(&view, 66, &mut row).expect("background row");
        assert_eq!(pixel(&row, 0), colors(theme).background);
        publish(&mut desktop, &control, ticket);
        for density in [density_mode::compact, density_mode::comfortable] {
            let revision = desktop.observe(&endpoint).expect("committed").revision();
            let ticket = desktop
                .prepare(&endpoint, revision, desktop_command::set_density(density))
                .expect("density");
            publish(&mut desktop, &control, ticket);
            for layout in [
                layout_mode::floating,
                layout_mode::horizontal,
                layout_mode::vertical,
            ] {
                let revision = desktop.observe(&endpoint).expect("committed").revision();
                let ticket = desktop
                    .prepare(&endpoint, revision, desktop_command::set_layout(layout))
                    .expect("layout");
                let view = desktop.candidate(&ticket).expect("candidate");
                assert_eq!(view.layout(), layout);
                for y in 0..frame_height {
                    compose_row(&view, y, &mut row).expect("complete composed row");
                }
                publish(&mut desktop, &control, ticket);
            }
        }
    }
}

#[test]
fn controls_hit_their_drawn_bounds_without_punching_through_front_windows() {
    let (desktop, _control, initial) = desktop::new_demo(frame_width, frame_height).expect("demo");
    let view = desktop.candidate(&initial).expect("candidate");
    let top = view.windows().last().expect("top");
    for target in [
        chrome_target::minimize(top.id()),
        chrome_target::maximize(top.id()),
        chrome_target::close(top.id()),
        chrome_target::resize(top.id()),
        chrome_target::titlebar(top.id()),
    ] {
        let bounds = chrome_bounds(&view, target).expect("bounds");
        assert_eq!(
            hit_test_chrome(
                &view,
                bounds.x + bounds.width / 2,
                bounds.y + bounds.height / 2
            ),
            target
        );
        assert!(!bounds.contains(bounds.x + bounds.width, bounds.y));
        assert!(!bounds.contains(bounds.x, bounds.y + bounds.height));
    }
    for target in crate::chrome::panel_targets {
        let bounds = chrome_bounds(&view, target).expect("panel bounds");
        assert_eq!(
            hit_test_chrome(
                &view,
                bounds.x + bounds.width / 2,
                bounds.y + bounds.height / 2
            ),
            target
        );
    }
    let x = top.bounds().x() + 20;
    let y = top.bounds().y() + 80;
    assert_eq!(
        hit_test_chrome(&view, x, y),
        chrome_target::window(top.id())
    );
    assert_eq!(
        hit_test_chrome(&view, u32::MAX, u32::MAX),
        chrome_target::desktop
    );
}

#[test]
fn minimized_windows_have_only_panel_targets_and_tasks_do_not_reorder_on_focus() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let view = desktop.observe(&endpoint).expect("view");
    let lower = view.windows().next().expect("lower").id();
    let task = chrome_bounds(&view, chrome_target::task(lower)).expect("task");
    let ticket = desktop
        .prepare(&endpoint, view.revision(), desktop_command::focus(lower))
        .expect("focus");
    assert_eq!(
        chrome_bounds(
            &desktop.candidate(&ticket).expect("candidate"),
            chrome_target::task(lower)
        ),
        Some(task)
    );
    publish(&mut desktop, &control, ticket);
    let revision = desktop.observe(&endpoint).expect("view").revision();
    let ticket = desktop
        .prepare(&endpoint, revision, desktop_command::minimize(lower))
        .expect("minimize");
    let view = desktop.candidate(&ticket).expect("candidate");
    assert_eq!(chrome_bounds(&view, chrome_target::titlebar(lower)), None);
    assert_eq!(chrome_bounds(&view, chrome_target::close(lower)), None);
    assert_eq!(chrome_bounds(&view, chrome_target::task(lower)), Some(task));
    assert_eq!(
        hit_test_chrome(&view, task.x + 5, task.y + 5),
        chrome_target::task(lower)
    );
}

#[test]
fn maximized_and_tiled_windows_have_no_resize_grip() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let view = desktop.observe(&endpoint).expect("view");
    let id = view.focused_window().expect("focused").id();
    let ticket = desktop
        .prepare(&endpoint, view.revision(), desktop_command::maximize(id))
        .expect("maximize");
    let view = desktop.candidate(&ticket).expect("candidate");
    assert_eq!(chrome_bounds(&view, chrome_target::resize(id)), None);
    assert!(chrome_bounds(&view, chrome_target::maximize(id)).is_some());
    publish(&mut desktop, &control, ticket);
    let revision = desktop.observe(&endpoint).expect("view").revision();
    let ticket = desktop
        .prepare(
            &endpoint,
            revision,
            desktop_command::set_layout(layout_mode::vertical),
        )
        .expect("tile");
    assert_eq!(
        chrome_bounds(
            &desktop.candidate(&ticket).expect("candidate"),
            chrome_target::resize(id)
        ),
        None
    );
}

#[test]
fn canonical_cursor_is_last_clipped_and_uses_both_theme_cursor_colors() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let revision = desktop.observe(&endpoint).expect("view").revision();
    let ticket = desktop
        .prepare(
            &endpoint,
            revision,
            desktop_command::set_pointer { x: 100, y: 100 },
        )
        .expect("pointer");
    let view = desktop.candidate(&ticket).expect("candidate");
    let c = view.theme_definition().colors;
    let mut row = [0; row_byte_count];
    compose_row(&view, 102, &mut row).expect("cursor row");
    assert_eq!(pixel(&row, 100), c.cursor_outline);
    assert_eq!(pixel(&row, 101), c.cursor_fill);
    assert_eq!(pixel(&row, 102), c.cursor_outline);
    publish(&mut desktop, &control, ticket);
    let revision = desktop.observe(&endpoint).expect("view").revision();
    let ticket = desktop
        .prepare(
            &endpoint,
            revision,
            desktop_command::set_pointer {
                x: frame_width - 1,
                y: frame_height - 1,
            },
        )
        .expect("edge pointer");
    compose_row(
        &desktop.candidate(&ticket).expect("candidate"),
        frame_height - 1,
        &mut row,
    )
    .expect("clipped pointer");
    assert_eq!(pixel(&row, frame_width as usize - 1), c.cursor_outline);
}

#[test]
fn custom_tokens_change_pixels_and_hit_metrics_in_the_same_candidate() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let view = desktop.observe(&endpoint).expect("view");
    let id = view.focused_window().expect("focused").id();
    let old_title = chrome_bounds(&view, chrome_target::titlebar(id)).expect("title");
    let mut settings = view.theme_settings();
    let mut colors = view.theme_definition().colors;
    colors.background = [220, 234, 241];
    let mut metrics = view.theme_definition().metrics;
    metrics.title_height = 48;
    metrics.control_size = 32;
    metrics.shadow_offset = 10;
    settings.overrides.colors = Some(colors);
    settings.overrides.metrics = Some(metrics);
    let ticket = desktop
        .prepare(
            &endpoint,
            view.revision(),
            desktop_command::set_theme_settings(settings),
        )
        .expect("custom theme");
    let view = desktop.candidate(&ticket).expect("candidate");
    let title = chrome_bounds(&view, chrome_target::titlebar(id)).expect("custom title");
    assert_eq!(title.height, 48);
    assert_ne!(title.height, old_title.height);
    let mut row = [0; row_byte_count];
    compose_row(&view, 60, &mut row).expect("background");
    assert_eq!(pixel(&row, 0), [220, 234, 241]);
    let window = view.window(id).expect("window").bounds();
    compose_row(&view, window.y() + window.height() + 8, &mut row).expect("shadow");
    let shadow = pixel(&row, (window.x() + window.width() / 2) as usize);
    assert_ne!(
        shadow, colors.background,
        "shadow-offset override reaches below the frame"
    );
    assert_ne!(
        shadow, colors.shadow,
        "shadow blends instead of replacing lower pixels"
    );
    for (shaded, background) in shadow.into_iter().zip(colors.background) {
        assert!(shaded < background);
    }
    let control = chrome_bounds(&view, chrome_target::close(id)).expect("control");
    assert_eq!(control.width, 32);
    assert_eq!(
        hit_test_chrome(&view, control.x + 16, control.y + 16),
        chrome_target::close(id)
    );
}

#[test]
fn font_and_cursor_customization_change_real_pixels_without_changing_control_targets() {
    use anomalous_desktop_theme::cursor_style;
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let view = desktop.observe(&endpoint).expect("view");
    let mut settings = view.theme_settings();
    settings.font_scale = 2;
    settings.cursor = cursor_style::crosshair;
    settings.cursor_size = 3;
    let appearance_button =
        chrome_bounds(&view, chrome_target::appearance_launcher).expect("Appearance control");
    let title = view.focused_window().expect("focus");
    let y = title.bounds().y() + 14;
    let mut before = [0; row_byte_count];
    compose_row(&view, y, &mut before).expect("regular type");
    let ticket = desktop
        .prepare_batch(
            &endpoint,
            view.revision(),
            &[
                desktop_command::set_theme_settings(settings),
                desktop_command::set_pointer { x: 100, y: 70 },
            ],
        )
        .expect("font and cursor settings");
    let view = desktop.candidate(&ticket).expect("candidate");
    assert_eq!(
        chrome_bounds(&view, chrome_target::appearance_launcher),
        Some(appearance_button)
    );
    let mut after = [0; row_byte_count];
    compose_row(&view, y, &mut after).expect("large type");
    assert_ne!(before, after);
    compose_row(&view, 70, &mut after).expect("crosshair");
    assert_eq!(
        pixel(&after, 100),
        view.theme_definition().colors.cursor_fill
    );
    assert_eq!(
        pixel(&after, 76),
        view.theme_definition().colors.cursor_outline
    );
    assert_eq!(
        hit_test_chrome(&view, appearance_button.x + 5, appearance_button.y + 5),
        chrome_target::appearance_launcher
    );
}

#[test]
fn tiled_maximize_is_visibly_inactive_and_does_not_emit_an_action_target() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let view = desktop.observe(&endpoint).expect("view");
    let id = view.focused_window().expect("focus").id();
    let ticket = desktop
        .prepare(
            &endpoint,
            view.revision(),
            desktop_command::set_layout(layout_mode::horizontal),
        )
        .expect("tile");
    let view = desktop.candidate(&ticket).expect("candidate");
    let bounds = chrome_bounds(&view, chrome_target::maximize(id)).expect("reserved icon region");
    let x = bounds.x + bounds.width / 2;
    let y = bounds.y + bounds.height / 2;
    assert_eq!(hit_test_chrome(&view, x, y), chrome_target::window(id));
    let mut output = [0; row_byte_count];
    compose_row(&view, y - 5, &mut output).expect("subdued icon");
    let color = pixel(&output, x as usize);
    let theme = view.theme_definition();
    assert_ne!(color, theme.colors.active_text);
    assert_ne!(color, theme.colors.active_title);
}

#[test]
fn compact_large_titles_keep_their_first_and_last_glyph_rows() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let view = desktop.observe(&endpoint).expect("view");
    let id = view.focused_window().expect("focus").id();
    let mut settings = view.theme_settings();
    settings.font_scale = 2;
    settings.density = density_mode::compact;
    let ticket = desktop
        .prepare(
            &endpoint,
            view.revision(),
            desktop_command::set_theme_settings(settings),
        )
        .expect("large compact");
    let view = desktop.candidate(&ticket).expect("candidate");
    let title = chrome_bounds(&view, chrome_target::titlebar(id)).expect("title");
    let mut row = [0; row_byte_count];
    // The known S glyph has top ink in column1 and bottom ink in column0.
    compose_row(&view, title.y + 2, &mut row).expect("top glyph row");
    assert_eq!(
        pixel(&row, (title.x + 16) as usize),
        view.theme_definition().colors.active_text
    );
    compose_row(&view, title.y + 29, &mut row).expect("bottom glyph row");
    assert_eq!(
        pixel(&row, (title.x + 12) as usize),
        view.theme_definition().colors.active_text
    );
}

#[test]
fn minimum_windows_reserve_a_drag_handle_even_with_largest_custom_controls() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let view = desktop.observe(&endpoint).expect("view");
    let id = view.focused_window().expect("focus").id();
    let mut settings = view.theme_settings();
    let mut metrics = view.theme_definition().metrics;
    metrics.title_height = 48;
    metrics.control_size = 36;
    settings.overrides.metrics = Some(metrics);
    let ticket = desktop
        .prepare_batch(
            &endpoint,
            view.revision(),
            &[
                desktop_command::resize_window {
                    window: id,
                    width: 120,
                    height: 96,
                },
                desktop_command::set_theme_settings(settings),
            ],
        )
        .expect("minimum window");
    let view = desktop.candidate(&ticket).expect("candidate");
    let title = chrome_bounds(&view, chrome_target::titlebar(id)).expect("title");
    assert!(title.width >= 24);
    assert_eq!(
        hit_test_chrome(&view, title.x + 12, title.y + 12),
        chrome_target::titlebar(id)
    );
    let window = view.window(id).expect("window").bounds();
    for target in [
        chrome_target::minimize(id),
        chrome_target::maximize(id),
        chrome_target::close(id),
    ] {
        let bounds = chrome_bounds(&view, target).expect("control");
        assert!(window.contains(bounds.x, bounds.y));
        assert!(window.contains(bounds.x + bounds.width - 1, bounds.y + bounds.height - 1));
        assert_eq!(
            hit_test_chrome(
                &view,
                bounds.x + bounds.width / 2,
                bounds.y + bounds.height / 2
            ),
            target
        );
    }
}

#[test]
fn compact_header_and_maximized_shadow_cannot_overpaint_other_desktop_regions() {
    use anomalous_desktop_workspace::{client_grants, client_origin, desktop_command};
    let (mut desktop, control, initial) =
        desktop::new_demo(frame_width, frame_height).expect("demo");
    publish(&mut desktop, &control, initial);
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .expect("client");
    let view = desktop.observe(&endpoint).expect("view");
    let id = view.focused_window().expect("focus").id();
    let mut settings = view.theme_settings();
    let mut metrics = view.theme_definition().metrics;
    metrics.header_height = 32;
    metrics.shadow_offset = 12;
    settings.overrides.metrics = Some(metrics);
    let ticket = desktop
        .prepare_batch(
            &endpoint,
            view.revision(),
            &[
                desktop_command::set_theme_settings(settings),
                desktop_command::maximize(id),
            ],
        )
        .expect("compact header and maximum shadow");
    let view = desktop.candidate(&ticket).expect("candidate");
    let mut row = [0; row_byte_count];
    compose_row(&view, 33, &mut row).expect("below header");
    assert!(
        row.chunks_exact(3)
            .all(|pixel| pixel == view.theme_definition().colors.background)
    );
    // The shadow reaches this reserved hint band, whose clear background and
    // known D glyph are painted afterwards, outside the maximized work area.
    compose_row(&view, 690, &mut row).expect("hint band background");
    assert_eq!(pixel(&row, 100), view.theme_definition().colors.background);
    compose_row(&view, 693, &mut row).expect("hint glyph");
    assert_eq!(
        pixel(&row, 24),
        view.theme_definition().colors.secondary_text
    );
}
