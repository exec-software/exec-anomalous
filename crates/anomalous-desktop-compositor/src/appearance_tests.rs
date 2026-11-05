use super::*;
use anomalous_desktop_theme::{contrast_at_least, rgb};
use anomalous_desktop_workspace::{
    appearance_choice, client_endpoint, client_grants, client_origin, demo_content, density_mode,
    desktop, desktop_command, desktop_control, minimum_window_height, minimum_window_width,
    render_view, theme_mode, theme_settings, update_ticket, window_id, workspace_id,
};

struct fixture {
    model: desktop,
    control: desktop_control,
    endpoint: client_endpoint,
    appearance: window_id,
}

impl fixture {
    fn new(settings: theme_settings) -> Self {
        let (mut model, control, ticket) = desktop::new(frame_width, frame_height).unwrap();
        let endpoint = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        publish(&mut model, &control, ticket);
        for command in [
            desktop_command::set_theme_settings(settings),
            desktop_command::open_appearance,
        ] {
            let revision = model.observe(&endpoint).unwrap().revision();
            let ticket = model.prepare(&endpoint, revision, command).unwrap();
            publish(&mut model, &control, ticket);
        }
        let appearance = model
            .observe(&endpoint)
            .unwrap()
            .focused_window()
            .unwrap()
            .id();
        Self {
            model,
            control,
            endpoint,
            appearance,
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

    fn bounds(&self, choice: appearance_choice) -> chrome_rect {
        chrome_bounds(
            &self.view(),
            chrome_target::appearance_choice(self.appearance, choice),
        )
        .unwrap()
    }

    fn pixel(&self, x: u32, y: u32) -> rgb {
        let mut row = [0; row_byte_count];
        compose_row(&self.view(), y, &mut row).unwrap();
        row[x as usize * 3..x as usize * 3 + 3].try_into().unwrap()
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
fn every_appearance_option_has_disjoint_painted_and_pickable_bounds_in_each_preset() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        for density in [density_mode::comfortable, density_mode::compact] {
            for font_scale in [1, 2] {
                let f = fixture::new(theme_settings {
                    theme,
                    density,
                    font_scale,
                    ..theme_settings::initial
                });
                assert!(appearance_controls_visible(&f.view(), f.appearance));
                let window = f.view().window(f.appearance).unwrap().bounds();
                for (index, choice) in appearance_choice::all.into_iter().enumerate() {
                    let bounds = f.bounds(choice);
                    assert!(bounds.width >= 132 && bounds.height >= 28);
                    assert!(bounds.x > window.x());
                    assert!(
                        bounds.y > window.y() + f.view().theme_definition().metrics.title_height
                    );
                    assert!(bounds.x + bounds.width < window.x() + window.width());
                    assert!(bounds.y + bounds.height < window.y() + window.height());
                    for previous in &appearance_choice::all[..index] {
                        assert!(!overlap(bounds, f.bounds(*previous)));
                    }
                    for (x, y) in [
                        (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2),
                        (bounds.x + 4, bounds.y + bounds.height / 2),
                        (bounds.x + bounds.width - 5, bounds.y + bounds.height / 2),
                    ] {
                        assert_eq!(
                            hit_test_chrome(&f.view(), x, y),
                            chrome_target::appearance_choice(f.appearance, choice),
                        );
                    }
                    // This interior point lies beyond the longest label and
                    // outside both selection marker and focus outlines.
                    assert_eq!(
                        f.pixel(bounds.x + bounds.width - 5, bounds.y + bounds.height / 2),
                        f.view().theme_definition().colors.panel,
                    );
                }
            }
        }
    }
}

#[test]
fn unavailable_appearance_windows_expose_no_choice_bounds() {
    let mut f = fixture::new(theme_settings::initial);
    let id = f.appearance;
    for command in [
        desktop_command::minimize(id),
        desktop_command::move_to_workspace {
            window: id,
            workspace: workspace_id::second,
        },
        desktop_command::resize_window {
            window: id,
            width: minimum_window_width,
            height: minimum_window_height,
        },
    ] {
        f.apply(command);
        assert!(!appearance_controls_visible(&f.view(), id));
        for choice in appearance_choice::all {
            assert_eq!(
                chrome_bounds(&f.view(), chrome_target::appearance_choice(id, choice)),
                None
            );
        }
        f.apply(desktop_command::open_appearance);
    }
    f.apply(desktop_command::close(id));
    assert!(!appearance_controls_visible(&f.view(), id));
    for choice in appearance_choice::all {
        assert_eq!(
            chrome_bounds(&f.view(), chrome_target::appearance_choice(id, choice)),
            None
        );
    }
    f.apply(desktop_command::create {
        content: demo_content::activity,
        workspace: workspace_id::first,
        bounds: anomalous_desktop_workspace::window_rect::try_new(100, 100, 840, 568).unwrap(),
    });
    let ordinary = f.view().focused_window().unwrap().id();
    assert!(!appearance_controls_visible(&f.view(), ordinary));
    for choice in appearance_choice::all {
        assert_eq!(
            chrome_bounds(
                &f.view(),
                chrome_target::appearance_choice(ordinary, choice)
            ),
            None
        );
    }
}

#[test]
fn a_higher_window_occludes_appearance_pixels_and_choices() {
    let mut f = fixture::new(theme_settings::initial);
    let bounds = f.bounds(appearance_choice::dark);
    let (x, y) = (bounds.x + bounds.width - 5, bounds.y + bounds.height / 2);
    assert_eq!(f.pixel(x, y), f.view().theme_definition().colors.panel);
    let window = f.view().window(f.appearance).unwrap().bounds();
    f.apply(desktop_command::create {
        content: demo_content::activity,
        workspace: workspace_id::first,
        bounds: window,
    });
    let cover = f.view().focused_window().unwrap().id();
    assert_eq!(
        hit_test_chrome(&f.view(), x, y),
        chrome_target::window(cover)
    );
    assert_eq!(f.pixel(x, y), f.view().theme_definition().colors.surface);
    assert_ne!(f.pixel(x, y), f.view().theme_definition().colors.panel);
}

#[test]
fn disabled_choice_paints_a_literal_marker_and_has_no_pointer_action() {
    let mut f = fixture::new(theme_settings::initial);
    let bounds = f.bounds(appearance_choice::dark);
    let marker = (bounds.x + 8, bounds.y + 15);
    assert_eq!(
        f.pixel(marker.0, marker.1),
        f.view().theme_definition().colors.panel
    );
    let mut settings = f.view().theme_settings();
    settings.overrides.decorations = Some(f.view().theme_definition().decorations);
    f.apply(desktop_command::set_theme_settings(settings));
    assert!(!f.view().appearance_enabled(appearance_choice::dark));
    assert_eq!(
        f.pixel(marker.0, marker.1),
        f.view().theme_definition().colors.secondary_text
    );
    assert_eq!(
        hit_test_chrome(
            &f.view(),
            bounds.x + bounds.width / 2,
            bounds.y + bounds.height / 2
        ),
        chrome_target::window(f.appearance),
    );
}

#[test]
fn keyboard_focus_and_chosen_value_are_distinct_canonical_visible_states() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        let mut f = fixture::new(theme_settings {
            theme,
            ..theme_settings::initial
        });
        let choice = appearance_choice::compact;
        let bounds = f.bounds(choice);
        let border = (bounds.x + bounds.width / 2, bounds.y);
        let before = f.pixel(border.0, border.1);
        let before_inner = f.pixel(border.0, border.1 + 1);
        let settings = f.view().theme_settings();
        f.apply(desktop_command::select_appearance {
            window: f.appearance,
            choice,
        });
        assert_eq!(f.view().theme_settings(), settings);
        assert_eq!(
            f.view()
                .window(f.appearance)
                .unwrap()
                .appearance_selection(),
            Some(choice)
        );
        assert!(!f.view().appearance_selected(choice));
        let outer = f.pixel(border.0, border.1);
        let inner = f.pixel(border.0, border.1 + 1);
        let fill = f.pixel(border.0, border.1 + 3);
        assert_ne!((outer, inner), (before, before_inner));
        assert!(contrast_at_least(outer, fill, 30) || contrast_at_least(inner, fill, 30));
        let comfortable = f.bounds(appearance_choice::comfortable);
        assert_eq!(
            f.pixel(comfortable.x + 8, comfortable.y + 13),
            f.view().theme_definition().colors.text,
        );
        assert_eq!(
            f.pixel(bounds.x + 8, bounds.y + 13),
            f.view().theme_definition().colors.panel
        );
        f.apply(desktop_command::activate_appearance {
            window: f.appearance,
            choice,
        });
        assert!(f.view().appearance_selected(choice));
        let bounds = f.bounds(choice);
        assert_eq!(
            f.pixel(bounds.x + 8, bounds.y + 13),
            f.view().theme_definition().colors.text
        );
    }
}

#[test]
fn appearance_shortcut_stays_inside_the_panel_and_below_work_area() {
    for panel_height in [40, 56, 64] {
        let mut settings = theme_settings::initial;
        let mut metrics = settings.validate().unwrap().metrics;
        metrics.panel_height = panel_height;
        metrics.workarea_bottom_inset = panel_height + 24;
        settings.overrides.metrics = Some(metrics);
        let f = fixture::new(settings);
        let view = f.view();
        let bounds = chrome_bounds(&view, chrome_target::appearance_launcher).unwrap();
        assert!(bounds.height >= 24);
        assert!(bounds.x >= 840);
        assert!(bounds.x + bounds.width <= 992);
        assert!(bounds.y >= frame_height - panel_height);
        assert!(bounds.y > view.work_area().y() + view.work_area().height());
        assert!(bounds.y + bounds.height <= frame_height - 16);
        let (x, y) = (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
        assert_eq!(
            hit_test_chrome(&view, x, y),
            chrome_target::appearance_launcher
        );
        let mut ink_pixels = 0;
        for y in bounds.y..bounds.y + bounds.height {
            let mut row = [0; row_byte_count];
            compose_row(&view, y, &mut row).unwrap();
            for column in bounds.x + 20..bounds.x + bounds.width - 20 {
                let offset = column as usize * 3;
                if row[offset..offset + 3] == view.theme_definition().colors.text {
                    ink_pixels += 1;
                }
            }
        }
        assert!(
            ink_pixels > 80,
            "panel shortcut must retain its visible label"
        );
    }
}
