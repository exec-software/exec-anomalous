//! Public scratch text is part of the immutable canonical candidate. Paint one
//! row at a time; no third Graphics bitmap or mutable content sidecar exists.
use crate::chrome::content_bounds;
use crate::paint::{rect, row_canvas};
use anomalous_desktop_theme::contrast_at_least;
use anomalous_desktop_workspace::{editor_feedback, render_view, window_view};

#[derive(Clone, Copy)]
pub(crate) struct editor_layout {
    pub(crate) line: rect,
    pub(crate) scale: i64,
    pub(crate) status_y: i64,
}

pub(crate) fn layout(content: rect, font: i64) -> Option<editor_layout> {
    // Keep the user's selected text scale. A smaller window explicitly asks
    // to be enlarged, rather than exposing clipped/invisible editing controls.
    if content.width < 24 * font || content.height < 44 * font {
        return None;
    }
    Some(editor_layout {
        line: rect::new(
            content.x + 2 * font,
            content.y + 14 * font,
            content.width - 4 * font,
            16 * font,
        ),
        scale: 2 * font,
        status_y: content.y + 34 * font,
    })
}

pub(crate) fn paint(canvas: &mut row_canvas<'_>, view: &render_view<'_>, window: window_view) {
    let Some(editor) = window.editor() else {
        return;
    };
    let content = content_bounds(view, window).paint();
    let theme = view.theme_definition();
    let c = theme.colors;
    let font = i64::from(theme.font_scale);
    let Some(layout) = layout(content, font) else {
        let label = if matches!(
            editor.feedback(),
            editor_feedback::rejected(_)
                | editor_feedback::save_unavailable
                | editor_feedback::unsupported_key
                | editor_feedback::input_reset
                | editor_feedback::display_unavailable
        ) {
            editor.feedback().label()
        } else {
            "Enlarge editor"
        };
        crate::render::paragraph(canvas, content, content.y, font, label, c.text);
        return;
    };
    canvas.text(
        content.x,
        content.y,
        font,
        "PUBLIC SCRATCH / MEMORY ONLY",
        c.secondary_text,
        content,
    );
    let line = layout.line;
    canvas.clipped_rectangle(line.inset(-2 * font), content, c.border);
    canvas.clipped_rectangle(line, content, c.panel);
    let (anchor, head) = editor.selection();
    // Reserve one caret stroke after the last whole cell. Horizontal scrolling
    // is a deterministic projection of the caret, never hidden editing state.
    let cells = ((line.width - layout.scale) / (6 * layout.scale)).max(0) as usize;
    let first = head.saturating_sub(cells);
    let from = anchor.min(head);
    let to = anchor.max(head);
    for index in first..editor.text().len() {
        let x = line.x + (index - first) as i64 * 6 * layout.scale;
        if x >= line.x + line.width {
            break;
        }
        let selected = index >= from && index < to;
        if selected {
            canvas.clipped_rectangle(
                rect::new(x, line.y, 6 * layout.scale, line.height),
                line,
                c.active_title,
            );
            // Selection must remain evident even for spaces and palettes whose
            // title/panel fills are close. The ink underline has text contrast.
            canvas.clipped_rectangle(
                rect::new(x, line.y + 7 * layout.scale, 6 * layout.scale, layout.scale),
                line,
                c.active_text,
            );
        }
        // Canonical local text is validated printable ASCII; this borrowed
        // one-byte string does not allocate or duplicate an editor snapshot.
        if let Ok(text) = core::str::from_utf8(&editor.text()[index..index + 1]) {
            canvas.text(
                x,
                line.y,
                layout.scale,
                text,
                if selected { c.active_text } else { c.text },
                line,
            );
        }
    }
    if window.focused() && !view.launcher_open() {
        let selected = head >= from && head < to;
        let background = if selected { c.active_title } else { c.panel };
        let fallback = if selected { c.active_text } else { c.text };
        let caret = if contrast_at_least(c.accent, background, 30) {
            c.accent
        } else {
            fallback
        };
        canvas.clipped_rectangle(
            rect::new(
                line.x + (head - first) as i64 * 6 * layout.scale,
                line.y,
                layout.scale,
                line.height,
            ),
            line,
            caret,
        );
    }
    let y = crate::render::paragraph(
        canvas,
        content,
        layout.status_y,
        font,
        editor.feedback().label(),
        c.text,
    );
    let y = crate::render::paragraph(
        canvas,
        content,
        y + 10 * font,
        font,
        "Closing this window discards its text.",
        c.secondary_text,
    );
    let y = crate::render::paragraph(
        canvas,
        content,
        y + 10 * font,
        font,
        "Arrows: caret / Shift: select / Ctrl+A: all / Ctrl+Z: undo",
        c.secondary_text,
    );
    crate::render::paragraph(
        canvas,
        content,
        y + 10 * font,
        font,
        "F1: Applications / Tab: focus / single ASCII line",
        c.secondary_text,
    );
}
