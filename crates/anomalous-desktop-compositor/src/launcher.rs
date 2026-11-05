use crate::chrome::{chrome_rect, chrome_target, hit_test_chrome, panel_bounds};
use crate::paint::row_canvas;
use anomalous_desktop_theme::contrast_at_least;
use anomalous_desktop_workspace::{density_mode, launcher_choice, render_view};

/// Fixed public choices fit both supported text scales without clipping labels.
/// Only the open canonical launcher exposes overlay geometry.
pub(crate) fn bounds(view: &render_view<'_>) -> Option<chrome_rect> {
    if !view.launcher_open() {
        return None;
    }
    let height = 120 + launcher_choice::all.len() as u32 * choice_step(view);
    Some(chrome_rect {
        x: 24,
        y: panel_bounds(view).y.saturating_sub(height + 8),
        width: 456,
        height,
    })
}

pub(crate) fn choice_bounds(
    view: &render_view<'_>,
    choice: launcher_choice,
) -> Option<chrome_rect> {
    let popup = bounds(view)?;
    let index = launcher_choice::all
        .iter()
        .position(|item| *item == choice)? as u32;
    let step = choice_step(view);
    Some(chrome_rect {
        x: popup.x + 16,
        y: popup.y + 64 + index * step,
        width: popup.width - 32,
        height: step - 6,
    })
}

fn choice_step(view: &render_view<'_>) -> u32 {
    if view.density() == density_mode::compact {
        48
    } else {
        52
    }
}

pub(crate) fn paint<const cull: bool>(canvas: &mut row_canvas<'_>, view: &render_view<'_>) {
    let Some(popup) = bounds(view).map(chrome_rect::paint) else {
        return;
    };
    let theme = view.theme_definition();
    let c = theme.colors;
    let d = theme.decorations;
    if theme.depth {
        canvas.soft_shadow(
            popup,
            i64::from(theme.frame.radius),
            theme.frame.shadow_spread.min(6),
            theme.frame.shadow_opacity / 2,
            c.shadow,
            2,
        );
    }
    if cull && (canvas.row < popup.y || canvas.row >= popup.y + popup.height) {
        return;
    }
    canvas.rounded_frame(popup, i64::from(theme.frame.radius), 1, c.panel, c.border);
    let scale = 2 * i64::from(theme.font_scale);
    canvas.text(
        popup.x + 16,
        popup.y + 16,
        scale,
        "Applications",
        c.text,
        popup.inset(8),
    );
    let pointer = view.pointer();
    let hovered = if pointer.visible() {
        hit_test_chrome(view, pointer.x(), pointer.y())
    } else {
        chrome_target::desktop
    };
    for choice in launcher_choice::all {
        let item = choice_bounds(view, choice)
            .expect("open launcher choice")
            .paint();
        if cull && (canvas.row < item.y || canvas.row >= item.y + item.height) {
            continue;
        }
        let target = chrome_target::launcher_item(choice);
        let enabled = view.launcher_enabled(choice);
        let selected = view.launcher_selection() == choice;
        let hover = enabled && hovered == target;
        let pressed = hover && view.pointer_buttons() == 1 && view.pointer_armed() == Some(target);
        let (fill, ink) = if pressed {
            (d.control_pressed, d.control_ink)
        } else if hover {
            (d.control_hover, d.control_ink)
        } else {
            (c.panel, if enabled { c.text } else { c.secondary_text })
        };
        let focus_ink = if contrast_at_least(c.focus_outer, fill, 30) {
            c.focus_outer
        } else {
            ink
        };
        canvas.rounded_frame(
            item,
            i64::from(theme.frame.control_radius),
            if selected { 2 } else { 1 },
            fill,
            if selected {
                focus_ink
            } else {
                d.inactive_border
            },
        );
        if selected {
            canvas.rounded_outline(
                item.inset(2),
                i64::from(theme.frame.control_radius).saturating_sub(2),
                1,
                c.focus_inner,
            );
        }
        // Selection and unavailability also have literal, non-color markers.
        if selected || !enabled {
            canvas.text(
                item.x + 10,
                item.y + (item.height - 7) / 2,
                1,
                if enabled { ">" } else { "-" },
                ink,
                item.inset(4),
            );
        }
        canvas.text(
            item.x + 28,
            item.y + (item.height - 7 * scale) / 2,
            scale,
            choice.title(),
            ink,
            item.inset(4),
        );
    }
    canvas.text(
        popup.x + 16,
        popup.y + popup.height - 24,
        1,
        if view.launcher_enabled(view.launcher_selection()) {
            "Choose an application / Esc closes"
        } else {
            "Unavailable in the current workspace."
        },
        c.secondary_text,
        popup.inset(8),
    );
}
