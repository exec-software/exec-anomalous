//! Public built-in settings content. Layout and picking consume the same
//! canonical choices; no independent widget values or hidden controls exist.
use crate::chrome::{chrome_rect, content_bounds};
use crate::paint::{rect, row_canvas};
use crate::{chrome_target, hit_test_chrome};
use anomalous_desktop_theme::contrast_at_least;
use anomalous_desktop_workspace::{
    appearance_choice, demo_content, layout_mode, render_view, theme_mode, window_id, window_view,
};

const row_height: u32 = 40;
const label_width: u32 = 136;
const button_height: u32 = 32;
const rows: [(&str, &[appearance_choice]); 7] = [
    (
        "Theme",
        &[
            appearance_choice::light,
            appearance_choice::dark,
            appearance_choice::high_contrast,
        ],
    ),
    (
        "Spacing",
        &[appearance_choice::comfortable, appearance_choice::compact],
    ),
    (
        "Text size",
        &[
            appearance_choice::regular_text,
            appearance_choice::large_text,
        ],
    ),
    (
        "Cursor",
        &[
            appearance_choice::arrow_cursor,
            appearance_choice::crosshair_cursor,
        ],
    ),
    (
        "Frames",
        &[
            appearance_choice::rounded_frames,
            appearance_choice::square_frames,
        ],
    ),
    (
        "Shadows",
        &[appearance_choice::depth_on, appearance_choice::depth_off],
    ),
    (
        "Motion",
        &[
            appearance_choice::reduced_motion_on,
            appearance_choice::reduced_motion_off,
        ],
    ),
];

/// Small windows retain their title controls but expose no invisible settings.
/// Native keyboard navigation uses this same predicate before dispatching.
pub fn appearance_controls_visible(view: &render_view<'_>, id: window_id) -> bool {
    let Some(window) = view.window(id) else {
        return false;
    };
    let client = content_bounds(view, window);
    window.content() == demo_content::appearance
        && !window.minimized()
        && window.workspace() == view.active_workspace()
        && client.width >= 620
        && client.height >= 422
}

pub(crate) fn choice_bounds(
    view: &render_view<'_>,
    id: window_id,
    choice: appearance_choice,
) -> Option<chrome_rect> {
    if !appearance_controls_visible(view, id) {
        return None;
    }
    let content = content_bounds(view, view.window(id)?);
    if choice == appearance_choice::reset {
        return Some(chrome_rect {
            x: content.x,
            y: content.y + 346,
            width: 164,
            height: button_height,
        });
    }
    for (row, (_, choices)) in rows.iter().enumerate() {
        if let Some(column) = choices.iter().position(|item| *item == choice) {
            let width = (content.width - label_width - (choices.len() as u32 - 1) * 8)
                / choices.len() as u32;
            return Some(chrome_rect {
                x: content.x + label_width + column as u32 * (width + 8),
                y: content.y + 48 + row as u32 * row_height,
                width,
                height: button_height,
            });
        }
    }
    None
}

fn label(choice: appearance_choice) -> &'static str {
    use appearance_choice::*;
    match choice {
        light => "Light",
        dark => "Dark",
        high_contrast => "Hi Contrast",
        comfortable => "Comfortable",
        compact => "Compact",
        regular_text => "Regular",
        large_text => "Large",
        arrow_cursor => "Arrow",
        crosshair_cursor => "Crosshair",
        rounded_frames => "Rounded",
        square_frames => "Square",
        depth_on => "On",
        depth_off => "Off",
        reduced_motion_on => "Reduced",
        reduced_motion_off => "Standard",
        reset => "Reset defaults",
    }
}

pub(crate) fn paint(canvas: &mut row_canvas<'_>, view: &render_view<'_>, window: window_view) {
    let theme = view.theme_definition();
    let c = theme.colors;
    let content = content_bounds(view, window).paint();
    if canvas.row < content.y || canvas.row >= content.y + content.height {
        return;
    }
    if !appearance_controls_visible(view, window.id()) {
        canvas.text(content.x, content.y, 1, "Appearance", c.text, content);
        canvas.text(
            content.x,
            content.y + 18,
            1,
            "Maximize for all settings",
            c.secondary_text,
            content,
        );
        canvas.text(
            content.x,
            content.y + 36,
            1,
            if view.layout() == layout_mode::floating {
                "X maximizes this window"
            } else {
                "T to floating, then X"
            },
            c.secondary_text,
            content,
        );
        return;
    }
    canvas.text(
        content.x,
        content.y,
        2,
        "Make this space yours.",
        c.text,
        content,
    );
    canvas.text(
        content.x,
        content.y + 24,
        1,
        "Changes apply immediately to this session.",
        c.secondary_text,
        content,
    );
    if window.focused()
        && let Some(choice) = window.appearance_selection()
    {
        canvas.text(
            content.x + content.width - 206,
            content.y + 24,
            1,
            "Key:",
            c.secondary_text,
            content,
        );
        canvas.text(
            content.x + content.width - 176,
            content.y + 24,
            1,
            label(choice),
            c.text,
            content,
        );
    }
    for (row, (name, _)) in rows.iter().enumerate() {
        canvas.text(
            content.x,
            content.y + 57 + row as i64 * i64::from(row_height),
            2,
            name,
            c.text,
            content,
        );
    }
    let pointer = view.pointer();
    let hovered = if pointer.visible() {
        hit_test_chrome(view, pointer.x(), pointer.y())
    } else {
        chrome_target::desktop
    };
    for choice in appearance_choice::all {
        let bounds = choice_bounds(view, window.id(), choice)
            .expect("complete settings layout")
            .paint();
        if canvas.row < bounds.y || canvas.row >= bounds.y + bounds.height {
            continue;
        }
        let enabled = view.appearance_enabled(choice);
        let selected = view.appearance_selected(choice);
        let keyboard_focus = window.focused() && window.appearance_selection() == Some(choice);
        let hover = enabled && hovered == chrome_target::appearance_choice(window.id(), choice);
        let pressed = hover
            && view.pointer_buttons() == 1
            && view.pointer_armed() == Some(chrome_target::appearance_choice(window.id(), choice));
        let (fill, ink) = if pressed {
            (
                theme.decorations.control_pressed,
                theme.decorations.control_ink,
            )
        } else if hover {
            (
                theme.decorations.control_hover,
                theme.decorations.control_ink,
            )
        } else {
            (c.panel, if enabled { c.text } else { c.secondary_text })
        };
        let focus_ink = if contrast_at_least(c.focus_outer, fill, 30)
            || contrast_at_least(c.focus_inner, fill, 30)
        {
            c.focus_outer
        } else {
            ink
        };
        canvas.rounded_frame(
            bounds,
            i64::from(theme.frame.control_radius),
            if keyboard_focus { 2 } else { 1 },
            fill,
            if keyboard_focus {
                focus_ink
            } else {
                theme.decorations.inactive_border
            },
        );
        if keyboard_focus {
            canvas.rounded_outline(
                bounds.inset(2),
                i64::from(theme.frame.control_radius).saturating_sub(2),
                1,
                c.focus_inner,
            );
        }
        // A literal marker makes chosen and disabled states independent of hue.
        if selected || !enabled {
            canvas.text(
                bounds.x + 8,
                bounds.y + 12,
                1,
                if !enabled { "-" } else { "*" },
                ink,
                bounds.inset(3),
            );
        }
        // Control typography stays bounded like the task panel. The chosen
        // content text scale is demonstrated separately below these controls.
        let text = label(choice);
        let scale = if choice == appearance_choice::reset {
            1
        } else {
            2
        };
        let width = text.len() as i64 * 6 * scale - scale;
        canvas.text(
            bounds.x + (bounds.width - width) / 2,
            bounds.y + (bounds.height - 7 * scale) / 2,
            scale,
            text,
            ink,
            bounds.inset(4),
        );
    }
    let sample_x = content.x + 184;
    let sample_scale = i64::from(theme.font_scale);
    canvas.text(
        sample_x,
        content.y + 350,
        sample_scale,
        "Aa / Window text preview",
        c.text,
        rect::new(sample_x, content.y + 346, content.width - 184, 32),
    );
    canvas.text(
        content.x,
        content.y + 390,
        1,
        if view.theme() == theme_mode::high_contrast {
            "High Contrast uses flat frames. Shadows preference is retained."
        } else {
            "Motion stores a preference; this compositor uses static effects."
        },
        c.secondary_text,
        content,
    );
    canvas.text(
        content.x,
        content.y + 408,
        1,
        "F6 / Shift+F6 selects | Enter applies | Tab switches windows",
        c.secondary_text,
        content,
    );
}
