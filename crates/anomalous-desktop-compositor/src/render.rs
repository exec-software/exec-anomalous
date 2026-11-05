use crate::chrome::{
    chrome_bounds, chrome_target, content_bounds, hit_test_chrome, panel_bounds, panel_targets,
    window_bounds, window_radius,
};
use crate::paint::{rect, rgb, row_canvas};
use anomalous_desktop_theme::{
    assets::{self, control_asset},
    contrast_at_least, cursor_style, theme_definition,
};
use anomalous_desktop_workspace::{
    demo_content, density_mode, layout_mode, render_view, theme_mode, window_view, workspace_id,
};

pub(crate) fn compose(frame: &crate::composition_frame<'_, '_>, canvas: &mut row_canvas<'_>) {
    compose_with_row_culling::<true>(frame, canvas);
}

#[cfg(test)]
pub(crate) fn compose_unculled(
    frame: &crate::composition_frame<'_, '_>,
    canvas: &mut row_canvas<'_>,
) {
    compose_with_row_culling::<false>(frame, canvas);
}

fn compose_with_row_culling<const cull: bool>(
    frame: &crate::composition_frame<'_, '_>,
    canvas: &mut row_canvas<'_>,
) {
    let view = frame.view();
    let theme = view.theme_definition();
    canvas.fill(theme.colors.background);
    desktop::<cull>(canvas, view, theme);
    for window in view.windows() {
        window_surface::<cull>(canvas, view, window, theme, frame.surface(window.id()));
    }
    status::<cull>(canvas, view, theme);
    panel::<cull>(canvas, view, theme);
    crate::launcher::paint::<cull>(canvas, view);
    pointer(canvas, view, theme);
}

fn desktop<const cull: bool>(
    canvas: &mut row_canvas<'_>,
    view: &render_view<'_>,
    theme: theme_definition,
) {
    let c = theme.colors;
    let viewport = viewport();
    if !cull || canvas.row < i64::from(theme.metrics.header_height) {
        let header_offset = i64::from(theme.metrics.header_height.saturating_sub(32)) / 2;
        canvas.rectangle(
            rect::new(0, 0, 1024, i64::from(theme.metrics.header_height)),
            c.panel,
        );
        canvas.rectangle(
            rect::new(0, i64::from(theme.metrics.header_height - 1), 1024, 1),
            c.border,
        );
        canvas.text(24, header_offset + 7, 2, "ANOMALOUS", c.text, viewport);
        canvas.text(
            154,
            header_offset + 11,
            1,
            "NATIVE WORKSPACE",
            c.secondary_text,
            viewport,
        );
        canvas.text(498, header_offset + 6, 1, theme.name, c.text, viewport);
        canvas.text(
            498,
            header_offset + 20,
            1,
            "Session appearance / public demos",
            c.secondary_text,
            viewport,
        );
        canvas.text(
            758,
            header_offset + 6,
            1,
            "WORKSPACE",
            c.secondary_text,
            viewport,
        );
        canvas.number(
            824,
            header_offset + 6,
            1,
            view.active_workspace().index() as u64 + 1,
            c.text,
            viewport,
        );
        canvas.text(
            758,
            header_offset + 20,
            1,
            layout_name(view.layout()),
            c.text,
            viewport,
        );
        canvas.text(918, header_offset + 6, 1, "REV", c.secondary_text, viewport);
        canvas.number(948, header_offset + 6, 1, view.revision(), c.text, viewport);
        canvas.text(
            918,
            header_offset + 20,
            1,
            if view.launcher_open() {
                "Esc close"
            } else if view
                .focused_window()
                .is_some_and(|window| window.editor().is_some())
            {
                "Esc caret"
            } else {
                "Esc shell"
            },
            c.secondary_text,
            viewport,
        );
    }
    if view.windows().next().is_none() {
        canvas.text(302, 274, 3, "Space for your next task", c.text, viewport);
        canvas.text(
            326,
            324,
            2,
            "Open Applications below.",
            c.secondary_text,
            viewport,
        );
        canvas.text(
            332,
            356,
            1,
            "Minimized windows remain available in the panel.",
            c.secondary_text,
            viewport,
        );
    }
}

fn status<const cull: bool>(
    canvas: &mut row_canvas<'_>,
    view: &render_view<'_>,
    theme: theme_definition,
) {
    let c = theme.colors;
    let viewport = viewport();
    let panel_y = panel_bounds(view).y;
    let area = view.work_area();
    let band_height = panel_y.saturating_sub(area.y() + area.height()).min(14);
    let band_y = panel_y - band_height;
    // Valid metrics leave at least eight rows here, so the seven-row status
    // text is entirely within this band, including the smallest custom inset.
    if cull && (canvas.row < i64::from(band_y) || canvas.row >= i64::from(panel_y)) {
        return;
    }
    canvas.rectangle(
        rect::new(0, i64::from(band_y), 1024, i64::from(band_height)),
        c.background,
    );
    let status_y = i64::from(band_y + (band_height.saturating_sub(7)) / 2);
    let pointer = view.pointer();
    let hint = if pointer.visible() {
        target_hint(hit_test_chrome(view, pointer.x(), pointer.y()))
    } else {
        "Drag a titlebar to move / drag the lower-right corner to resize"
    };
    canvas.text(24, status_y, 1, hint, c.secondary_text, viewport);
    canvas.text(
        770,
        status_y,
        1,
        "Applications / P Appearance",
        c.secondary_text,
        viewport,
    );
}

fn target_hint(target: chrome_target) -> &'static str {
    use chrome_target::*;
    match target {
        titlebar(_) => "Drag to move this window",
        resize(_) => "Drag to resize this window",
        minimize(_) => "Minimize / restore from its panel button",
        maximize(_) => "Maximize or restore this window",
        close(_) => "Close this public demo window",
        task(_) => "Focus this window / restore if minimized",
        launcher => "Choose an application / open or close the launcher",
        launcher_item(_) => "Open this application / existing windows are restored",
        launcher_background => "Choose an application / Esc closes the launcher",
        appearance_launcher => "Open Appearance / P / maximum four windows",
        appearance_choice(_, _) => "Apply this appearance choice to the current session",
        theme => "Cycle official Light, Dark and High Contrast themes",
        density => "Change between comfortable and compact spacing",
        font => "Change the size of the embedded bitmap font",
        cursor => "Change between arrow and crosshair cursors",
        workspace(_) => "Switch workspace / each workspace retains its windows",
        layout => "Cycle floating, horizontal and vertical window layouts",
        _ => "Drag a titlebar to move / drag the lower-right corner to resize",
    }
}

fn panel<const cull: bool>(
    canvas: &mut row_canvas<'_>,
    view: &render_view<'_>,
    theme: theme_definition,
) {
    let c = theme.colors;
    let bounds = panel_bounds(view).paint();
    let radius = i64::from(theme.frame.radius);
    if theme.depth {
        canvas.soft_shadow(
            bounds,
            radius,
            theme.frame.shadow_spread.min(6),
            theme.frame.shadow_opacity / 2,
            c.shadow,
            2,
        );
    }
    if cull && (canvas.row < bounds.y || canvas.row >= bounds.y + bounds.height) {
        return;
    }
    canvas.rounded_frame(
        bounds,
        radius,
        1,
        c.panel,
        theme.decorations.inactive_border,
    );
    let pointer = view.pointer();
    let hover = if pointer.visible() {
        hit_test_chrome(view, pointer.x(), pointer.y())
    } else {
        chrome_target::desktop
    };
    for target in panel_targets {
        let Some(bounds) = chrome_bounds(view, target) else {
            continue;
        };
        let active = matches!(target, chrome_target::workspace(workspace) if workspace == view.active_workspace())
            || (target == chrome_target::launcher && view.launcher_open());
        let ink = button_background(
            canvas,
            bounds.paint(),
            active,
            hover == target,
            hover == target && view.pointer_buttons() == 1 && view.pointer_armed() == Some(target),
            theme,
        );
        let label = match target {
            chrome_target::launcher => "Applications",
            chrome_target::workspace(workspace_id::first) => "1",
            chrome_target::workspace(workspace_id::second) => "2",
            chrome_target::appearance_launcher => "Appearance",
            _ => "",
        };
        centered_text(canvas, bounds.paint(), 2, label, ink);
    }
    for window in view
        .all_windows()
        .filter(|window| window.workspace() == view.active_workspace())
    {
        let target = chrome_target::task(window.id());
        let Some(bounds) = chrome_bounds(view, target) else {
            continue;
        };
        let bounds = bounds.paint();
        let ink = button_background(
            canvas,
            bounds,
            false,
            hover == target,
            hover == target && view.pointer_buttons() == 1 && view.pointer_armed() == Some(target),
            theme,
        );
        // A sibling can enable the workspace extension through Cargo feature
        // unification without enabling this crate's forwarding feature.
        #[allow(unreachable_patterns)]
        let title = match window.content() {
            demo_content::welcome => "Welcome",
            demo_content::inspector => "Inspect",
            demo_content::activity => "Activity",
            demo_content::appearance => "Style",
            demo_content::graphics => "Graphics",
            demo_content::editor => "Editor",
            content => content.title(),
        };
        centered_text(canvas, bounds, 2, title, ink);
        if window.minimized() {
            canvas.rectangle(
                rect::new(
                    bounds.x + bounds.width / 2 - 7,
                    bounds.y + bounds.height - 7,
                    14,
                    2,
                ),
                ink,
            );
        } else {
            canvas.rectangle(
                rect::new(
                    bounds.x + 12,
                    bounds.y + bounds.height - 4,
                    bounds.width - 24,
                    2,
                ),
                {
                    let mark = if window.focused() {
                        theme.decorations.primary
                    } else {
                        c.accent
                    };
                    let fill = if hover == target
                        && view.pointer_buttons() == 1
                        && view.pointer_armed() == Some(target)
                    {
                        theme.decorations.control_pressed
                    } else if hover == target {
                        theme.decorations.control_hover
                    } else {
                        c.panel
                    };
                    if contrast_at_least(mark, fill, 30) {
                        mark
                    } else {
                        ink
                    }
                },
            );
        }
    }
}

fn button_background(
    canvas: &mut row_canvas<'_>,
    bounds: rect,
    selected: bool,
    hover: bool,
    pressed: bool,
    theme: theme_definition,
) -> rgb {
    let c = theme.colors;
    let d = theme.decorations;
    let (fill, ink) = if selected {
        (
            if pressed {
                d.primary_pressed
            } else if hover {
                d.primary_hover
            } else {
                d.primary
            },
            d.primary_ink,
        )
    } else if pressed {
        (d.control_pressed, d.control_ink)
    } else if hover {
        (d.control_hover, d.control_ink)
    } else {
        (c.panel, c.text)
    };
    canvas.rounded_frame(
        bounds,
        i64::from(theme.frame.control_radius),
        1,
        fill,
        if hover { d.primary } else { d.inactive_border },
    );
    ink
}

fn window_surface<const cull: bool>(
    canvas: &mut row_canvas<'_>,
    view: &render_view<'_>,
    window: window_view,
    theme: theme_definition,
    surface: Option<&anomalous_window_surface::frame<'_>>,
) {
    let c = theme.colors;
    let bounds = window_bounds(window).paint();
    let radius = window_radius(view, window);
    if theme.depth
        && view.layout() == layout_mode::floating
        && !window.maximized()
        && canvas.row >= i64::from(theme.metrics.header_height)
    {
        canvas.soft_shadow(
            bounds,
            radius,
            theme.frame.shadow_spread,
            theme.frame.shadow_opacity,
            c.shadow,
            i64::from(theme.metrics.shadow_offset),
        );
    }
    if cull && (canvas.row < bounds.y || canvas.row >= bounds.y + bounds.height) {
        return;
    }
    let border_width = i64::from(theme.frame.border_width) + i64::from(window.focused());
    canvas.rounded_frame(
        bounds,
        radius,
        border_width,
        c.surface,
        if window.focused() {
            c.focus_outer
        } else {
            theme.decorations.inactive_border
        },
    );
    let inner = bounds.inset(4);
    let title_height = i64::from(theme.metrics.title_height).min(inner.height);
    let title = rect::new(inner.x, inner.y, inner.width, title_height);
    let ink = if window.focused() {
        c.active_text
    } else {
        c.text
    };
    let title_background = if window.focused() {
        c.active_title
    } else {
        c.inactive_title
    };
    canvas.clipped_rectangle(title, inner, title_background);
    canvas.clipped_rectangle(
        rect::new(
            inner.x,
            title.y + title.height,
            inner.width,
            i64::from(theme.frame.title_separator),
        ),
        inner,
        c.border,
    );
    let title_text = chrome_bounds(view, chrome_target::titlebar(window.id()))
        .expect("visible title")
        .paint();
    let scale = 2 * i64::from(theme.font_scale);
    if window.focused() && title_text.width > 260 {
        let mark = if contrast_at_least(theme.decorations.primary, title_background, 30) {
            theme.decorations.primary
        } else {
            theme.decorations.primary_ink
        };
        canvas.rounded_rectangle(
            rect::new(
                title_text.x + title_text.width - 49,
                title_text.y + (title_text.height - 5) / 2,
                5,
                5,
            ),
            2,
            mark,
        );
    }
    canvas.text(
        title_text.x + 12,
        title_text.y + (title_text.height - 7 * scale) / 2,
        scale,
        window.title(),
        ink,
        rect::new(
            title_text.x + 4,
            title_text.y,
            title_text.width
                - 8
                - if window.focused() && title_text.width > 260 {
                    44
                } else {
                    0
                },
            title_text.height,
        ),
    );
    if window.focused() && title_text.width > 260 {
        canvas.text(
            title_text.x + title_text.width - 38,
            title_text.y + (title_text.height - 7) / 2,
            1,
            "FOCUS",
            ink,
            title_text,
        );
    }
    let pointer = view.pointer();
    let hover = if pointer.visible() {
        hit_test_chrome(view, pointer.x(), pointer.y())
    } else {
        chrome_target::desktop
    };
    for target in [
        chrome_target::minimize(window.id()),
        chrome_target::maximize(window.id()),
        chrome_target::close(window.id()),
    ] {
        let region = chrome_bounds(view, target)
            .expect("visible title button")
            .paint();
        let enabled =
            !matches!(target, chrome_target::maximize(_)) || view.layout() == layout_mode::floating;
        let highlighted = enabled && hover == target;
        let pressed =
            highlighted && view.pointer_buttons() == 1 && view.pointer_armed() == Some(target);
        let d = theme.decorations;
        let glyph_ink = if !enabled || (!highlighted && !window.focused()) {
            if contrast_at_least(c.secondary_text, title_background, 30) {
                c.secondary_text
            } else {
                ink
            }
        } else if highlighted && matches!(target, chrome_target::close(_)) {
            d.danger_ink
        } else if highlighted {
            d.control_ink
        } else {
            ink
        };
        if highlighted {
            let fill = if matches!(target, chrome_target::close(_)) {
                if pressed {
                    d.danger_pressed
                } else {
                    d.danger_hover
                }
            } else if pressed {
                d.control_pressed
            } else {
                d.control_hover
            };
            canvas.rounded_rectangle(region, i64::from(theme.frame.control_radius), fill);
        }
        let asset = match target {
            chrome_target::minimize(_) => control_asset::minimize,
            chrome_target::maximize(_) if window.maximized() => control_asset::restore,
            chrome_target::maximize(_) => control_asset::maximize,
            _ => control_asset::close,
        };
        window_icon(canvas, region, asset, glyph_ink);
        if !enabled {
            // A small baseline mark supplements subdued glyphs; disabled is
            // visible beyond color and no corresponding hit target is offered.
            canvas.rectangle(
                rect::new(
                    region.x + region.width / 2 - 2,
                    region.y + region.height - 3,
                    4,
                    1,
                ),
                glyph_ink,
            );
        }
    }
    let content = content_bounds(view, window).paint();
    if let Some(surface) = surface {
        canvas.surface(content, surface);
    } else {
        #[allow(unreachable_patterns)]
        match window.content() {
            demo_content::welcome => welcome(canvas, content, theme),
            demo_content::inspector => inspector(canvas, content, view, theme),
            demo_content::activity => activity(canvas, content, view, theme),
            demo_content::appearance => crate::appearance::paint(canvas, view, window),
            demo_content::editor => crate::editor::paint(canvas, view, window),
            demo_content::graphics => {
                paragraph(
                    canvas,
                    content,
                    content.y,
                    i64::from(theme.font_scale),
                    "Awaiting surface",
                    c.secondary_text,
                );
            }
            // External content can arrive through a dependency's feature.
            // Complete binding validation precedes painting; this fallback
            // neither fetches arbitrary pixels nor invents application state.
            _ => {
                paragraph(
                    canvas,
                    content,
                    content.y,
                    i64::from(theme.font_scale),
                    "Awaiting application surface",
                    c.secondary_text,
                );
            }
        }
    }
    if let Some(grip) = chrome_bounds(view, chrome_target::resize(window.id())) {
        let grip = grip.paint();
        for step in 0..3 {
            let offset = 4 + step * 4;
            for mark in [
                rect::new(
                    grip.x + grip.width - offset - 2,
                    grip.y + grip.height - 4,
                    2,
                    2,
                ),
                rect::new(
                    grip.x + grip.width - 4,
                    grip.y + grip.height - offset - 2,
                    2,
                    2,
                ),
            ] {
                if canvas.row < mark.y.max(grip.y)
                    || canvas.row >= (mark.y + mark.height).min(grip.y + grip.height)
                {
                    continue;
                }
                for x in mark.x.max(grip.x)..(mark.x + mark.width).min(grip.x + grip.width) {
                    // The largest legal corner radius cuts into a small grip.
                    // Use the same silhouette as picking, not just its box.
                    canvas.blend_pixel(
                        x,
                        c.secondary_text,
                        crate::paint::rounded_coverage(bounds, radius, x, canvas.row),
                    );
                }
            }
        }
    }
    // Paint after the title and grip so the widest legal border cannot have
    // its contrasting inner keyline overwritten by the fixed content inset.
    if window.focused() {
        canvas.rounded_outline(
            bounds.inset(border_width),
            (radius - border_width).max(0),
            1,
            c.focus_inner,
        );
    }
}

fn window_icon(canvas: &mut row_canvas<'_>, bounds: rect, asset: control_asset, ink: rgb) {
    // Quantized sizes keep an opaque stroke core, including the minimum
    // 20px control. Arbitrary sizes can split a thin glyph across two pixels.
    let available = bounds.width.min(bounds.height).saturating_sub(4);
    let size = if available >= 24 {
        24
    } else if available >= 20 {
        20
    } else {
        16
    };
    let x = bounds.x + (bounds.width - i64::from(size)) / 2;
    let y = bounds.y + (bounds.height - i64::from(size)) / 2;
    if canvas.row < y || canvas.row >= y + i64::from(size) {
        return;
    }
    for column in 0..size {
        canvas.blend_pixel(
            x + i64::from(column),
            ink,
            assets::coverage(asset, column, (canvas.row - y) as u32, size),
        );
    }
}

fn welcome(canvas: &mut row_canvas<'_>, content: rect, theme: theme_definition) {
    let scale = i64::from(theme.font_scale);
    let c = theme.colors;
    let y = paragraph(
        canvas,
        content,
        content.y,
        3 * scale,
        "Make room for your work.",
        c.text,
    );
    let y = paragraph(
        canvas,
        content,
        y + 20,
        2 * scale,
        "Drag a titlebar to move. Pull the corner to resize. Your window controls are above.",
        c.secondary_text,
    );
    let y = paragraph(
        canvas,
        content,
        y + 16,
        2 * scale,
        "Open Applications below. Use Appearance to make this space yours.",
        c.secondary_text,
    );
    canvas.clipped_rectangle(
        rect::new(content.x, y + 15, content.width, 1),
        content,
        c.border,
    );
    let y = paragraph(
        canvas,
        content,
        y + 30,
        scale,
        "LIGHT AND DARK / ONE SHARED DESKTOP",
        c.accent,
    );
    paragraph(
        canvas,
        content,
        y + 12,
        scale,
        "Human and machine clients share window commands. These are public built-in demonstrations.",
        c.secondary_text,
    );
}

fn inspector(
    canvas: &mut row_canvas<'_>,
    content: rect,
    view: &render_view<'_>,
    theme: theme_definition,
) {
    let c = theme.colors;
    let scale = i64::from(theme.font_scale);
    let body_scale = 2 * scale;
    let y = paragraph(
        canvas,
        content,
        content.y,
        3 * scale,
        "Your desktop, your way.",
        c.text,
    );
    let rows = [
        (
            "Theme",
            match view.theme() {
                theme_mode::light => "Light",
                theme_mode::dark => "Dark",
                theme_mode::high_contrast => "High contrast",
            },
        ),
        ("Layout", layout_name(view.layout())),
        (
            "Spacing",
            if view.density() == density_mode::compact {
                "Compact"
            } else {
                "Comfortable"
            },
        ),
        (
            "Font",
            if theme.font_scale == 1 {
                "Bitmap / regular"
            } else {
                "Bitmap / large"
            },
        ),
        (
            "Cursor",
            if theme.cursor == cursor_style::arrow {
                "Arrow"
            } else {
                "Crosshair"
            },
        ),
        ("Effects", if theme.depth { "Static depth" } else { "Flat" }),
        ("Motion", "No animation"),
    ];
    let stacked = content.width < 192 * body_scale;
    let line_height = (7 * body_scale + 13) * if stacked { 2 } else { 1 };
    for (index, (label, value)) in rows.iter().enumerate() {
        let y = y + 20 + index as i64 * line_height;
        canvas.text(content.x, y, body_scale, label, c.secondary_text, content);
        let (x, y) = if stacked {
            (content.x, y + 7 * body_scale + 10)
        } else {
            (content.x + 88 * body_scale, y)
        };
        canvas.text(x, y, body_scale, value, c.text, content);
    }
    let y = y + 20 + rows.len() as i64 * line_height;
    canvas.clipped_rectangle(rect::new(content.x, y, content.width, 1), content, c.border);
    paragraph(
        canvas,
        content,
        y + 14,
        scale,
        "Appearance is session-local. Adjust it with the panel controls.",
        c.secondary_text,
    );
}

fn activity(
    canvas: &mut row_canvas<'_>,
    content: rect,
    view: &render_view<'_>,
    theme: theme_definition,
) {
    let c = theme.colors;
    let scale = i64::from(theme.font_scale);
    let y = paragraph(
        canvas,
        content,
        content.y,
        3 * scale,
        "Workspace activity",
        c.text,
    );
    canvas.text(
        content.x,
        y + 24,
        2 * scale,
        "Visible windows",
        c.secondary_text,
        content,
    );
    canvas.number(
        content.x + 210 * scale,
        y + 24,
        2 * scale,
        view.windows().count() as u64,
        c.text,
        content,
    );
    canvas.text(
        content.x,
        y + 58,
        2 * scale,
        "Scene revision",
        c.secondary_text,
        content,
    );
    canvas.number(
        content.x + 210 * scale,
        y + 58,
        2 * scale,
        view.revision(),
        c.text,
        content,
    );
    paragraph(
        canvas,
        content,
        y + 106,
        2 * scale,
        "These values describe this view. No external agent is connected here.",
        c.secondary_text,
    );
}

fn pointer(canvas: &mut row_canvas<'_>, view: &render_view<'_>, theme: theme_definition) {
    let pointer = view.pointer();
    if !pointer.visible() {
        return;
    }
    let x = i64::from(pointer.x());
    let y = i64::from(pointer.y());
    let size = i64::from(theme.cursor_size);
    let c = theme.colors;
    if theme.cursor == cursor_style::crosshair {
        canvas.rectangle(
            rect::new(x - 8 * size, y - size, 17 * size, 3 * size),
            c.cursor_outline,
        );
        canvas.rectangle(
            rect::new(x - size, y - 8 * size, 3 * size, 17 * size),
            c.cursor_outline,
        );
        canvas.rectangle(rect::new(x - 7 * size, y, 15 * size, size), c.cursor_fill);
        canvas.rectangle(rect::new(x, y - 7 * size, size, 15 * size), c.cursor_fill);
        return;
    }
    if canvas.row < y || canvas.row >= y + 18 * size {
        return;
    }
    // Two-color authored bitmap; hotspot is its upper-left pixel. Both colors
    // remain legible over the light/dark themes and any overlapping content.
    const arrow: [&str; 18] = [
        "X           ",
        "XX          ",
        "XOX         ",
        "XOOX        ",
        "XOOOX       ",
        "XOOOOX      ",
        "XOOOOOX     ",
        "XOOOOOOX    ",
        "XOOOOOOOX   ",
        "XOOOOOOOOX  ",
        "XOOOOOXXXXX ",
        "XOOXOOX     ",
        "XOX XOOX    ",
        "XX  XOOX    ",
        "X    XOOX   ",
        "     XOOX   ",
        "      XX    ",
        "            ",
    ];
    for (row, line) in arrow.iter().enumerate() {
        if canvas.row < y + row as i64 * size || canvas.row >= y + (row as i64 + 1) * size {
            continue;
        }
        for (column, pixel) in line.bytes().enumerate() {
            let color = match pixel {
                b'X' => c.cursor_outline,
                b'O' => c.cursor_fill,
                _ => continue,
            };
            canvas.rectangle(
                rect::new(x + column as i64 * size, y + row as i64 * size, size, size),
                color,
            );
        }
    }
}

pub(crate) fn paragraph(
    canvas: &mut row_canvas<'_>,
    content: rect,
    mut y: i64,
    scale: i64,
    mut text: &str,
    ink: rgb,
) -> i64 {
    if content.width <= 0 || content.height <= 0 {
        return y;
    }
    let columns = (content.width / (6 * scale)).max(1) as usize;
    while !text.is_empty() {
        let mut length = text.len().min(columns);
        if length < text.len()
            && let Some(space) = text.as_bytes()[..length]
                .iter()
                .rposition(|byte| *byte == b' ')
            && space > 0
        {
            length = space;
        }
        // All compositor text is fixed printable ASCII, so byte slicing is
        // character aligned. No external or application text enters this path.
        canvas.text(content.x, y, scale, &text[..length], ink, content);
        text = text[length..].trim_start();
        y += 7 * scale + 8;
        if y >= content.y + content.height {
            break;
        }
    }
    y
}

fn centered_text(canvas: &mut row_canvas<'_>, bounds: rect, scale: i64, label: &str, ink: rgb) {
    let width = label.len() as i64 * 6 * scale - scale;
    canvas.text(
        bounds.x + (bounds.width - width).max(4) / 2,
        bounds.y + (bounds.height - 7 * scale) / 2,
        scale,
        label,
        ink,
        bounds.inset(3),
    );
}
const fn viewport() -> rect {
    rect::new(0, 0, 1024, 768)
}
const fn layout_name(layout: layout_mode) -> &'static str {
    match layout {
        layout_mode::floating => "Floating",
        layout_mode::horizontal => "Horizontal",
        layout_mode::vertical => "Vertical",
    }
}
