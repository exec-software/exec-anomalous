use anomalous_desktop_workspace::{
    appearance_choice, density_mode, launcher_choice, layout_mode, render_view, window_view,
    workspace_id,
};

/// Logical pixel bounds shared by composition and human input routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct chrome_rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl chrome_rect {
    pub const fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x && y >= self.y && x - self.x < self.width && y - self.y < self.height
    }
    pub(crate) const fn paint(self) -> crate::paint::rect {
        crate::paint::rect::new(
            self.x as i64,
            self.y as i64,
            self.width as i64,
            self.height as i64,
        )
    }
}

/// Shared semantic regions remain correlation, never authority. Keep the
/// compositor name as an alias so existing geometry callers use one identity.
pub use anomalous_desktop_workspace::desktop_target as chrome_target;

pub(crate) const panel_targets: [chrome_target; 4] = [
    chrome_target::launcher,
    chrome_target::workspace(workspace_id::first),
    chrome_target::workspace(workspace_id::second),
    chrome_target::appearance_launcher,
];

pub(crate) fn panel_bounds(view: &render_view<'_>) -> chrome_rect {
    let metrics = view.theme_definition().metrics;
    chrome_rect {
        x: 16,
        y: view.height().saturating_sub(metrics.panel_height + 8),
        width: view.width().saturating_sub(32),
        height: metrics.panel_height,
    }
}

pub(crate) fn window_bounds(window: window_view) -> chrome_rect {
    let bounds = window.bounds();
    chrome_rect {
        x: bounds.x(),
        y: bounds.y(),
        width: bounds.width(),
        height: bounds.height(),
    }
}

pub(crate) fn window_radius(view: &render_view<'_>, window: window_view) -> i64 {
    if window.maximized() || view.layout() != layout_mode::floating {
        0
    } else {
        i64::from(view.theme_definition().frame.radius)
    }
}

pub(crate) fn content_bounds(view: &render_view<'_>, window: window_view) -> chrome_rect {
    let bounds = window.bounds();
    let padding = if view.density() == density_mode::compact {
        14
    } else {
        20
    };
    let title = view
        .theme_definition()
        .metrics
        .title_height
        .min(bounds.height().saturating_sub(8));
    chrome_rect {
        x: bounds.x() + 4 + padding,
        y: bounds.y() + 4 + title + padding,
        width: bounds.width().saturating_sub(8 + 2 * padding),
        height: bounds.height().saturating_sub(8 + title + 2 * padding),
    }
}

/// Return the same bounded rectangles that the compositor uses for controls.
/// Absent, hidden, minimized, or inapplicable window decorations have no bounds.
pub fn chrome_bounds(view: &render_view<'_>, target: chrome_target) -> Option<chrome_rect> {
    use chrome_target::*;
    let panel = panel_bounds(view);
    let button = |x, width| {
        Some(chrome_rect {
            x,
            y: panel.y + 8,
            width,
            height: panel.height.saturating_sub(16),
        })
    };
    match target {
        desktop => Some(chrome_rect {
            x: 0,
            y: 0,
            width: view.width(),
            height: view.height(),
        }),
        launcher => button(24, 168),
        launcher_background => crate::launcher::bounds(view),
        launcher_item(choice) => crate::launcher::choice_bounds(view, choice),
        appearance_launcher => button(840, 152),
        appearance_choice(id, choice) => crate::appearance::choice_bounds(view, id, choice),
        workspace(workspace_id::first) => button(752, 32),
        workspace(workspace_id::second) => button(792, 32),
        layout | theme | density | font | cursor => None,
        task(id) => {
            let item = view.window(id)?;
            if item.workspace() != view.active_workspace() {
                return None;
            }
            let index = view
                .all_windows()
                .filter(|candidate| {
                    candidate.workspace() == view.active_workspace()
                        && candidate.id().slot() < id.slot()
                })
                .count();
            button(212 + index as u32 * 112, 108)
        }
        window(id) | titlebar(id) | resize(id) | minimize(id) | maximize(id) | close(id) => {
            let item = view.window(id)?;
            if item.minimized() || item.workspace() != view.active_workspace() {
                return None;
            }
            let bounds = window_bounds(item);
            let metrics = view.theme_definition().metrics;
            let inset = 4;
            let title_height = metrics
                .title_height
                .min(bounds.height.saturating_sub(inset * 2));
            let size = metrics
                .control_size
                .min(title_height.saturating_sub(4))
                .min((bounds.width.saturating_sub(inset * 2 + 24) / 3).saturating_sub(2));
            let button_y = bounds.y + inset + (title_height - size) / 2;
            match target {
                chrome_target::window(_) => Some(bounds),
                titlebar(_) => Some(chrome_rect {
                    x: bounds.x + inset,
                    y: bounds.y + inset,
                    width: bounds.width.saturating_sub(inset * 2 + 3 * (size + 2)),
                    height: title_height,
                }),
                minimize(_) | maximize(_) | close(_) => {
                    let index = match target {
                        close(_) => 0,
                        maximize(_) => 1,
                        _ => 2,
                    };
                    Some(chrome_rect {
                        x: bounds.x + bounds.width - inset - (index + 1) * (size + 2),
                        y: button_y,
                        width: size,
                        height: size,
                    })
                }
                resize(_) if view.layout() == layout_mode::floating && !item.maximized() => {
                    let size = metrics.resize_grip.min(bounds.width).min(bounds.height);
                    Some(chrome_rect {
                        x: bounds.x + bounds.width - size,
                        y: bounds.y + bounds.height - size,
                        width: size,
                        height: size,
                    })
                }
                _ => None,
            }
        }
    }
}

/// Pick the launcher overlay, panel, then the topmost opaque window and controls.
/// A lower window can never receive a click through a higher window's content.
pub fn hit_test_chrome(view: &render_view<'_>, x: u32, y: u32) -> chrome_target {
    if x >= view.width() || y >= view.height() {
        return chrome_target::desktop;
    }
    if chrome_bounds(view, chrome_target::launcher_background)
        .is_some_and(|bounds| bounds.contains(x, y))
    {
        for choice in launcher_choice::all {
            let target = chrome_target::launcher_item(choice);
            if chrome_bounds(view, target).is_some_and(|bounds| bounds.contains(x, y))
                && view.launcher_enabled(choice)
            {
                return target;
            }
        }
        // Disabled entries, padding and header consume the overlay hit too.
        return chrome_target::launcher_background;
    }
    if panel_bounds(view).contains(x, y) {
        for target in panel_targets {
            if chrome_bounds(view, target).is_some_and(|bounds| bounds.contains(x, y)) {
                return target;
            }
        }
        for window in view
            .all_windows()
            .filter(|window| window.workspace() == view.active_workspace())
        {
            let target = chrome_target::task(window.id());
            if chrome_bounds(view, target).is_some_and(|bounds| bounds.contains(x, y)) {
                return target;
            }
        }
        return chrome_target::desktop;
    }
    let Some(window) = view
        .windows()
        .filter(|window| {
            crate::paint::rounded_coverage(
                window_bounds(*window).paint(),
                window_radius(view, *window),
                i64::from(x),
                i64::from(y),
            ) >= 128
        })
        .last()
    else {
        return chrome_target::desktop;
    };
    for target in [
        chrome_target::close(window.id()),
        chrome_target::maximize(window.id()),
        chrome_target::minimize(window.id()),
        chrome_target::resize(window.id()),
        chrome_target::titlebar(window.id()),
    ] {
        if matches!(target, chrome_target::maximize(_)) && view.layout() != layout_mode::floating {
            continue;
        }
        if chrome_bounds(view, target).is_some_and(|bounds| bounds.contains(x, y)) {
            return target;
        }
    }
    for choice in appearance_choice::all {
        let target = chrome_target::appearance_choice(window.id(), choice);
        if chrome_bounds(view, target).is_some_and(|bounds| bounds.contains(x, y))
            && view.appearance_enabled(choice)
        {
            return target;
        }
    }
    chrome_target::window(window.id())
}
