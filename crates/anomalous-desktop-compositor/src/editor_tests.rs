use super::*;
use crate::paint::{rect, row_canvas};
use anomalous_desktop_theme::contrast_at_least;
use anomalous_desktop_workspace::{
    caret_motion, client_endpoint, client_grants, client_origin, desktop, desktop_command,
    desktop_control, editor_action, editor_feedback, render_view, theme_mode, theme_settings,
    update_ticket, window_id,
};

struct fixture {
    model: desktop,
    control: desktop_control,
    endpoint: client_endpoint,
    id: window_id,
}
fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    model.publish(control, ticket).ok().unwrap();
}
impl fixture {
    fn new(theme: theme_mode, font_scale: u8) -> Self {
        let (mut model, control, initial) = desktop::new(frame_width, frame_height).unwrap();
        let endpoint = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        for command in [
            desktop_command::set_theme_settings(theme_settings {
                theme,
                font_scale,
                ..theme_settings::initial
            }),
            desktop_command::open_editor,
        ] {
            let revision = model.observe(&endpoint).unwrap().revision();
            let ticket = model.prepare(&endpoint, revision, command).unwrap();
            publish(&mut model, &control, ticket);
        }
        let id = model
            .observe(&endpoint)
            .unwrap()
            .focused_window()
            .unwrap()
            .id();
        Self {
            model,
            control,
            endpoint,
            id,
        }
    }
    fn view(&self) -> render_view<'_> {
        self.model.observe(&self.endpoint).unwrap()
    }
    fn apply(&mut self, command: desktop_command) {
        let ticket = self
            .model
            .prepare(&self.endpoint, self.view().revision(), command)
            .unwrap();
        publish(&mut self.model, &self.control, ticket);
    }
    fn edit(&mut self, action: editor_action) {
        self.apply(desktop_command::edit_editor {
            window: self.id,
            action,
        });
    }
    fn layout(&self) -> crate::editor::editor_layout {
        let view = self.view();
        crate::editor::layout(
            crate::chrome::content_bounds(&view, view.window(self.id).unwrap()).paint(),
            i64::from(view.theme_definition().font_scale),
        )
        .unwrap()
    }
    fn row(&self, y: u32) -> [u8; row_byte_count] {
        let mut row = [0; row_byte_count];
        compose_row(&self.view(), y, &mut row).unwrap();
        row
    }
}

#[test]
fn editor_glyphs_selection_and_caret_match_literal_cells_in_every_theme_and_scale() {
    let glyphs = [[14u8, 17, 17, 31, 17, 17, 17], [16, 16, 22, 25, 17, 17, 30]]; // A, b
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        for font in [1, 2] {
            let mut f = fixture::new(theme, font);
            f.edit(editor_action::insert(b'A'));
            f.edit(editor_action::insert(b'b'));
            f.edit(editor_action::move_caret {
                motion: caret_motion::left,
                extend: true,
            });
            let view = f.view();
            let e = view.window(f.id).unwrap().editor().unwrap();
            assert_eq!(
                (e.text(), e.selection(), e.revision()),
                (b"Ab".as_slice(), (2, 1), 3)
            );
            let l = f.layout();
            let c = view.theme_definition().colors;
            let caret = if contrast_at_least(c.accent, c.active_title, 30) {
                c.accent
            } else {
                c.active_text
            };
            assert!(contrast_at_least(c.text, c.panel, 45));
            assert!(contrast_at_least(c.active_text, c.active_title, 45));
            assert!(contrast_at_least(caret, c.active_title, 30));
            for y in 0..l.line.height {
                let row = f.row((l.line.y + y) as u32);
                for x in 0..18 * l.scale {
                    let cell = (x / (6 * l.scale)) as usize;
                    let column = (x / l.scale % 6) as usize;
                    let selected = cell == 1;
                    let mut expected = if selected { c.active_title } else { c.panel };
                    if selected && y >= 7 * l.scale {
                        expected = c.active_text;
                    }
                    if cell < 2
                        && y < 7 * l.scale
                        && column < 5
                        && glyphs[cell][(y / l.scale) as usize] & (1 << (4 - column)) != 0
                    {
                        expected = if selected { c.active_text } else { c.text };
                    }
                    if x >= 6 * l.scale && x < 7 * l.scale {
                        expected = caret;
                    }
                    let start = (l.line.x + x) as usize * 3;
                    assert_eq!(
                        &row[start..start + 3],
                        &expected,
                        "theme {theme:?}, font {font}, ({x},{y})"
                    );
                }
            }
        }
    }
}

#[test]
fn editor_scroll_keeps_the_last_caret_visible_and_returns_home_without_mutation() {
    let mut f = fixture::new(theme_mode::dark, 1);
    f.apply(desktop_command::resize_window {
        window: f.id,
        width: 280,
        height: 260,
    });
    for _ in 0..31 {
        f.edit(editor_action::insert(b'x'));
    }
    let e = f.view().window(f.id).unwrap().editor().unwrap();
    let l = f.layout();
    let cells = ((l.line.width - l.scale) / (6 * l.scale)) as usize;
    assert!(cells < 31);
    let x = l.line.x + cells as i64 * 6 * l.scale;
    let c = f.view().theme_definition().colors;
    let ink = if contrast_at_least(c.accent, c.panel, 30) {
        c.accent
    } else {
        c.text
    };
    let row = f.row(l.line.y as u32);
    assert_eq!(&row[x as usize * 3..x as usize * 3 + 3], &ink);
    assert!(x + l.scale <= l.line.x + l.line.width);
    f.edit(editor_action::move_caret {
        motion: caret_motion::start,
        extend: false,
    });
    let current = f.view().window(f.id).unwrap().editor().unwrap();
    assert_eq!(current.text(), e.text());
    assert_eq!(current.revision(), e.revision());
    let row = f.row(l.line.y as u32);
    assert_eq!(&row[l.line.x as usize * 3..l.line.x as usize * 3 + 3], &ink);
}

#[test]
fn editor_paint_never_escapes_content_even_when_too_small_or_reporting_rejection() {
    let mut f = fixture::new(theme_mode::light, 1);
    for (width, height) in [(120, 96), (180, 160), (520, 340)] {
        f.apply(desktop_command::resize_window {
            window: f.id,
            width,
            height,
        });
        for action in [
            editor_action::insert(b'A'),
            editor_action::undo,
            editor_action::undo,
            editor_action::save_unavailable,
            editor_action::unsupported_key,
        ] {
            f.edit(action);
            let view = f.view();
            let window = view.window(f.id).unwrap();
            let content = crate::chrome::content_bounds(&view, window).paint();
            for y in (content.y - 1)..=(content.y + content.height) {
                let mut row = [173; row_byte_count];
                crate::editor::paint(&mut row_canvas::new(y as u32, &mut row), &view, window);
                for (x, pixel) in row.chunks_exact(3).enumerate() {
                    if y < content.y
                        || y >= content.y + content.height
                        || (x as i64) < content.x
                        || (x as i64) >= content.x + content.width
                    {
                        assert_eq!(pixel, [173; 3], "paint escaped {width}x{height}");
                    }
                }
            }
        }
    }
}

#[test]
fn editor_rejection_and_save_unavailable_have_distinct_visible_feedback_without_text_changes() {
    let mut f = fixture::new(theme_mode::dark, 1);
    for _ in 0..31 {
        f.edit(editor_action::insert(b'x'));
    }
    let e = f.view().window(f.id).unwrap().editor().unwrap();
    let l = f.layout();
    let status_rows = |f: &fixture| {
        (l.status_y..l.status_y + 14)
            .map(|y| f.row(y as u32))
            .collect::<std::vec::Vec<_>>()
    };
    let edited = status_rows(&f);
    f.edit(editor_action::insert(b'y'));
    let full = f.view().window(f.id).unwrap().editor().unwrap();
    assert_eq!(
        full.feedback(),
        editor_feedback::rejected(anomalous_desktop_workspace::editor_error::text_capacity)
    );
    assert_eq!(
        (
            full.text(),
            full.revision(),
            full.selection(),
            full.undo_depth()
        ),
        (e.text(), e.revision(), e.selection(), e.undo_depth())
    );
    let rejected = status_rows(&f);
    assert_ne!(edited, rejected);
    f.edit(editor_action::save_unavailable);
    assert_eq!(
        f.view().window(f.id).unwrap().editor().unwrap().text(),
        e.text()
    );
    assert_ne!(rejected, status_rows(&f));
}

#[test]
fn editor_projection_respects_focus_and_modal_caret_and_preserves_the_document() {
    let mut f = fixture::new(theme_mode::dark, 1);
    f.edit(editor_action::insert(b'A'));
    let l = f.layout();
    let y = l.line.y as u32;
    let before = f.row(y);
    let saved = f.view().window(f.id).unwrap().editor().unwrap();
    f.apply(desktop_command::toggle_launcher);
    // Paint only this canonical content to isolate caret policy from popup occlusion.
    let mut modal = [0; row_byte_count];
    let view = f.view();
    crate::editor::paint(
        &mut row_canvas::new(y, &mut modal),
        &view,
        view.window(f.id).unwrap(),
    );
    let x = (l.line.x + 6 * l.scale) as usize * 3;
    assert_eq!(&modal[x..x + 3], &view.theme_definition().colors.panel);
    assert_ne!(&before[x..x + 3], &modal[x..x + 3]);
    assert_eq!(view.window(f.id).unwrap().editor().unwrap(), saved);
    f.apply(desktop_command::dismiss_launcher);
    assert_eq!(f.row(y), before);
}

#[test]
fn editor_layout_rejects_unreadable_geometry_without_negative_cell_counts() {
    for font in [1, 2] {
        for width in [0, 1, 23 * font] {
            assert!(crate::editor::layout(rect::new(0, 0, width, 1000), font).is_none());
        }
        for height in [0, 1, 43 * font] {
            assert!(crate::editor::layout(rect::new(0, 0, 1000, height), font).is_none());
        }
    }
}

#[test]
fn selected_spaces_have_an_independent_high_contrast_underline() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        for font in [1, 2] {
            let mut f = fixture::new(theme, font);
            f.edit(editor_action::insert(b' '));
            f.edit(editor_action::insert(b' '));
            f.edit(editor_action::select_all);
            let l = f.layout();
            let c = f.view().theme_definition().colors;
            assert!(contrast_at_least(c.active_text, c.active_title, 45));
            assert!(
                contrast_at_least(c.active_title, c.panel, 30)
                    || contrast_at_least(c.active_text, c.panel, 30)
            );
            for y in 7 * l.scale..8 * l.scale {
                let row = f.row((l.line.y + y) as u32);
                for x in 0..12 * l.scale {
                    let start = (l.line.x + x) as usize * 3;
                    assert_eq!(&row[start..start + 3], &c.active_text);
                }
            }
        }
    }
}
