use super::*;
use anomalous_desktop_theme::{contrast_at_least, cursor_style, rgb};
use anomalous_desktop_workspace::{
    client_endpoint, client_grants, client_origin, demo_content, density_mode, desktop,
    desktop_command as command, desktop_control, launcher_choice, render_view, theme_mode,
    theme_settings, update_ticket, window_rect, workspace_id,
};

const choices: [(launcher_choice, &str); 6] = [
    (launcher_choice::welcome, "Welcome"),
    (launcher_choice::inspector, "System Inspector"),
    (launcher_choice::activity, "Activity"),
    (launcher_choice::appearance, "Appearance"),
    (launcher_choice::graphics, "Graphics Lab"),
    (launcher_choice::editor, "Editor Lab"),
];

struct fixture {
    model: desktop,
    control: desktop_control,
    endpoint: client_endpoint,
}

impl fixture {
    fn new(settings: theme_settings) -> Self {
        let (mut model, control, initial) = desktop::new_demo(frame_width, frame_height).unwrap();
        let endpoint = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, initial);
        let mut result = Self {
            model,
            control,
            endpoint,
        };
        result.apply(&[command::set_theme_settings(settings)]);
        result
    }

    fn view(&self) -> render_view<'_> {
        self.model.observe(&self.endpoint).unwrap()
    }

    fn apply(&mut self, commands: &[command]) {
        let ticket = self
            .model
            .prepare_batch(&self.endpoint, self.view().revision(), commands)
            .unwrap();
        publish(&mut self.model, &self.control, ticket);
    }

    fn bounds(&self, target: chrome_target) -> chrome_rect {
        chrome_bounds(&self.view(), target).unwrap()
    }

    fn pixel(&self, x: u32, y: u32) -> rgb {
        let mut row = [0; row_byte_count];
        compose_row(&self.view(), y, &mut row).unwrap();
        row[x as usize * 3..x as usize * 3 + 3].try_into().unwrap()
    }

    fn fill_capacity(&mut self) {
        for _ in 0..2 {
            self.apply(&[command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds: window_rect::try_new(120, 140, 480, 320).unwrap(),
            }]);
        }
    }
}

fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    assert!(model.publish(control, ticket).is_ok());
}

fn overlap(left: chrome_rect, right: chrome_rect) -> bool {
    left.x < right.x + right.width
        && right.x < left.x + left.width
        && left.y < right.y + right.height
        && right.y < left.y + left.height
}

#[test]
fn launcher_choices_fit_and_hit_the_visible_rows_in_every_supported_text_profile() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        for density in [density_mode::comfortable, density_mode::compact] {
            for font_scale in [1, 2] {
                let mut f = fixture::new(theme_settings {
                    theme,
                    density,
                    font_scale,
                    ..theme_settings::initial
                });
                assert_eq!(
                    chrome_bounds(&f.view(), chrome_target::launcher_background),
                    None
                );
                for (choice, _) in choices {
                    assert_eq!(
                        chrome_bounds(&f.view(), chrome_target::launcher_item(choice)),
                        None
                    );
                }
                f.apply(&[command::toggle_launcher]);
                let popup = f.bounds(chrome_target::launcher_background);
                let panel = crate::chrome::panel_bounds(&f.view());
                assert_eq!((popup.x, popup.width), (24, 456));
                assert!(popup.y > f.view().theme_definition().metrics.header_height);
                assert_eq!(popup.y + popup.height + 8, panel.y);
                assert!(popup.x + popup.width < frame_width);
                let scale = 2 * u32::from(font_scale);
                for (index, (choice, label)) in choices.into_iter().enumerate() {
                    let target = chrome_target::launcher_item(choice);
                    let item = f.bounds(target);
                    assert_eq!(choice.title(), label);
                    assert_eq!((item.x, item.width), (40, 424));
                    assert!(item.y >= popup.y + 64);
                    assert!(item.y + item.height < popup.y + popup.height - 32);
                    assert!(28 + label.len() as u32 * 6 * scale - scale <= item.width - 4);
                    assert!(7 * scale + 8 <= item.height);
                    for (previous, _) in &choices[..index] {
                        assert!(!overlap(
                            item,
                            f.bounds(chrome_target::launcher_item(*previous))
                        ));
                    }
                    for (x, y) in [
                        (item.x + item.width / 2, item.y + item.height / 2),
                        (item.x, item.y + item.height / 2),
                        (item.x + item.width - 1, item.y + item.height / 2),
                    ] {
                        assert_eq!(hit_test_chrome(&f.view(), x, y), target);
                    }
                    assert_eq!(
                        hit_test_chrome(&f.view(), item.x + item.width, item.y + 8),
                        chrome_target::launcher_background
                    );
                    assert_eq!(
                        hit_test_chrome(&f.view(), item.x + 8, item.y + item.height),
                        chrome_target::launcher_background
                    );
                    assert_eq!(
                        f.pixel(item.x + item.width - 8, item.y + item.height / 2),
                        f.view().theme_definition().colors.panel
                    );
                }
                // Inspect the final glyph of the longest label independently
                // of the text painter, including the largest allowed scale.
                let item = f.bounds(chrome_target::launcher_item(launcher_choice::inspector));
                let last_x = item.x + 28 + 15 * 6 * scale;
                let text_y = item.y + (item.height - 7 * scale) / 2;
                assert!((text_y..text_y + 7 * scale).any(|y| {
                    (last_x..last_x + 5 * scale)
                        .any(|x| f.pixel(x, y) == f.view().theme_definition().colors.text)
                }));
                f.apply(&[command::dismiss_launcher]);
                assert_eq!(
                    chrome_bounds(&f.view(), chrome_target::launcher_background),
                    None
                );
                for (choice, _) in choices {
                    assert_eq!(
                        chrome_bounds(&f.view(), chrome_target::launcher_item(choice)),
                        None
                    );
                }
            }
        }
    }
}

#[test]
fn cleaned_panel_keeps_four_tasks_disjoint_and_exposes_no_retired_quick_controls() {
    for panel_height in [40, 56, 64] {
        let mut settings = theme_settings::initial;
        let mut metrics = settings.validate().unwrap().metrics;
        metrics.panel_height = panel_height;
        metrics.workarea_bottom_inset = panel_height + 24;
        settings.overrides.metrics = Some(metrics);
        let mut f = fixture::new(settings);
        f.fill_capacity();
        let panel = crate::chrome::panel_bounds(&f.view());
        let mut targets = std::vec::Vec::from(crate::chrome::panel_targets);
        targets.extend(
            f.view()
                .all_windows()
                .map(|window| chrome_target::task(window.id())),
        );
        assert_eq!(targets.len(), 8);
        for (index, target) in targets.iter().enumerate() {
            let item = f.bounds(*target);
            assert!(item.x >= panel.x && item.x + item.width <= panel.x + panel.width);
            assert!(item.y > panel.y && item.y + item.height < panel.y + panel.height);
            assert_eq!(
                hit_test_chrome(&f.view(), item.x + item.width / 2, item.y + item.height / 2),
                *target
            );
            for previous in &targets[..index] {
                assert!(!overlap(item, f.bounds(*previous)));
            }
        }
        assert_eq!(hit_test_chrome(&f.view(), 396, 24), chrome_target::desktop);
        for removed in [
            chrome_target::theme,
            chrome_target::density,
            chrome_target::font,
            chrome_target::cursor,
            chrome_target::layout,
        ] {
            assert_eq!(chrome_bounds(&f.view(), removed), None);
        }
    }
}

#[test]
fn launcher_blocks_opaque_windows_and_paints_below_the_pointer() {
    let mut f = fixture::new(theme_settings {
        cursor: cursor_style::crosshair,
        ..theme_settings::initial
    });
    let window = f.view().focused_window().unwrap().id();
    f.apply(&[command::maximize(window)]);
    // This point lies in the fixed popup header, over the maximized app.
    let (x, y) = (60, 290);
    assert_eq!(
        hit_test_chrome(&f.view(), x, y),
        chrome_target::window(window)
    );
    let before = f.pixel(x, y);
    f.apply(&[command::toggle_launcher]);
    assert_eq!(
        hit_test_chrome(&f.view(), x, y),
        chrome_target::launcher_background
    );
    assert_eq!(f.pixel(x, y), f.view().theme_definition().colors.panel);
    f.apply(&[command::set_pointer { x, y }]);
    assert_eq!(
        f.pixel(x, y),
        f.view().theme_definition().colors.cursor_fill
    );
    f.apply(&[
        command::set_pointer { x: 1000, y: 680 },
        command::dismiss_launcher,
    ]);
    assert_eq!(
        hit_test_chrome(&f.view(), x, y),
        chrome_target::window(window)
    );
    assert_eq!(f.pixel(x, y), before);
}

#[test]
fn disabled_entries_remain_visible_selectable_and_block_underlying_hits() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        let mut f = fixture::new(theme_settings {
            theme,
            ..theme_settings::initial
        });
        f.fill_capacity();
        f.apply(&[
            command::toggle_launcher,
            command::select_launcher(launcher_choice::appearance),
        ]);
        assert!(f.view().launcher_enabled(launcher_choice::welcome));
        for choice in [launcher_choice::appearance, launcher_choice::graphics] {
            assert!(!f.view().launcher_enabled(choice));
            let item = f.bounds(chrome_target::launcher_item(choice));
            let (x, y) = (item.x + item.width - 12, item.y + item.height / 2);
            assert_eq!(
                hit_test_chrome(&f.view(), x, y),
                chrome_target::launcher_background
            );
            f.apply(&[
                command::set_pointer { x, y },
                command::set_pointer_buttons(1),
            ]);
            let colors = f.view().theme_definition().colors;
            assert_eq!(
                f.pixel(item.x + item.width / 2, item.y + item.height - 7),
                colors.panel
            );
            // The literal '-' is present even without relying on subdued color.
            let marker_y = item.y + (item.height - 7) / 2 + 3;
            assert_eq!(f.pixel(item.x + 10, marker_y), colors.secondary_text);
        }
        let item = f.bounds(chrome_target::launcher_item(launcher_choice::appearance));
        let colors = f.view().theme_definition().colors;
        let focus = if contrast_at_least(colors.focus_outer, colors.panel, 30) {
            colors.focus_outer
        } else {
            colors.secondary_text
        };
        assert_eq!(f.pixel(item.x + item.width / 2, item.y + 1), focus);
    }
}

#[test]
fn launcher_item_pressed_pixels_require_its_matching_arm_and_hover() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        let mut f = fixture::new(theme_settings {
            theme,
            ..theme_settings::initial
        });
        f.apply(&[command::toggle_launcher]);
        let target = chrome_target::launcher_item(launcher_choice::activity);
        let item = f.bounds(target);
        let at = command::set_pointer {
            x: item.x + item.width / 2,
            y: item.y + item.height - 2,
        };
        let away = command::set_pointer { x: 1000, y: 100 };
        let sample = |f: &fixture| f.pixel(item.x + item.width / 2, item.y + 3);
        let theme = f.view().theme_definition();
        f.apply(&[at, command::set_pointer_buttons(1)]);
        assert_eq!(sample(&f), theme.decorations.control_hover);
        f.apply(&[
            at,
            command::set_pointer_buttons(1),
            command::arm_pointer(chrome_target::launcher),
        ]);
        assert_eq!(sample(&f), theme.decorations.control_hover);
        f.apply(&[
            at,
            command::set_pointer_buttons(1),
            command::arm_pointer(target),
        ]);
        assert_eq!(sample(&f), theme.decorations.control_pressed);
        f.apply(&[
            away,
            command::set_pointer_buttons(1),
            command::arm_pointer(target),
        ]);
        assert_eq!(sample(&f), theme.colors.panel);
        f.apply(&[
            at,
            command::set_pointer_buttons(1),
            command::arm_pointer(target),
        ]);
        assert_eq!(sample(&f), theme.decorations.control_pressed);
        f.apply(&[command::cancel_pointer]);
        assert_eq!(sample(&f), theme.decorations.control_hover);
        for buttons in [0, 2, 3, 4, 5, 6, 7] {
            f.apply(&[at, command::set_pointer_buttons(buttons)]);
            assert_eq!(sample(&f), theme.decorations.control_hover);
        }
    }
}
