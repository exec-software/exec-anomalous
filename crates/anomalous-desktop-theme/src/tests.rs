use super::*;

#[test]
fn official_catalog_has_valid_named_components_and_contrast() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        for density in [density_mode::compact, density_mode::comfortable] {
            let settings = theme_settings {
                theme,
                density,
                ..theme_settings::initial
            };
            let definition = settings.validate().unwrap();
            assert_eq!(definition.schema, 1);
            assert!(definition.name.starts_with("Anomalous "));
            assert_eq!(definition.font, font_family::embedded_5x7);
            assert_eq!(definition.login, component_support::unavailable);
            assert_eq!(definition.sounds, component_support::unavailable);
            assert!(contrast_at_least(
                definition.colors.text,
                definition.colors.surface,
                45
            ));
            assert!(contrast_at_least(
                definition.colors.active_text,
                definition.colors.active_title,
                45
            ));
        }
    }
    assert_ne!(
        builtin(theme_mode::light, density_mode::compact).colors,
        builtin(theme_mode::dark, density_mode::compact).colors
    );
}

#[test]
fn exact_wcag_endpoints_and_known_color_pair() {
    assert!(contrast_at_least([255; 3], [0; 3], 210));
    assert!(!contrast_at_least([255; 3], [0; 3], 211));
    assert!(contrast_at_least([255; 3], [118; 3], 45));
    assert!(!contrast_at_least([255; 3], [119; 3], 45));
    assert!(!contrast_at_least([45; 3], [45; 3], 30));
}

#[test]
fn bounds_reject_unsupported_font_and_cursor_sizes() {
    for font_scale in [0, 3, u8::MAX] {
        assert_eq!(
            theme_settings {
                font_scale,
                ..theme_settings::initial
            }
            .validate(),
            Err(theme_error::invalid_font_scale)
        );
    }
    for cursor_size in [0, 4, u8::MAX] {
        assert_eq!(
            theme_settings {
                cursor_size,
                ..theme_settings::initial
            }
            .validate(),
            Err(theme_error::invalid_cursor_size)
        );
    }
    let custom = theme_settings {
        font_scale: 2,
        cursor_size: 3,
        cursor: cursor_style::crosshair,
        ..theme_settings::initial
    }
    .validate()
    .unwrap();
    assert_eq!(custom.font_scale, 2);
    assert_eq!(custom.cursor_size, 3);
    assert_eq!(custom.cursor, cursor_style::crosshair);
}

#[test]
fn invalid_color_override_cannot_hide_labels_or_cursor() {
    let base = builtin(theme_mode::light, density_mode::comfortable);
    for colors in [
        semantic_colors {
            text: base.colors.surface,
            ..base.colors
        },
        semantic_colors {
            background: base.colors.text,
            ..base.colors
        },
        semantic_colors {
            background: base.colors.secondary_text,
            ..base.colors
        },
        semantic_colors {
            active_text: base.colors.active_title,
            ..base.colors
        },
        semantic_colors {
            cursor_outline: base.colors.cursor_fill,
            ..base.colors
        },
        semantic_colors {
            focus_inner: base.colors.focus_outer,
            ..base.colors
        },
    ] {
        assert_eq!(
            theme_settings {
                overrides: theme_overrides {
                    colors: Some(colors),
                    metrics: None,
                    ..Default::default()
                },
                ..theme_settings::initial
            }
            .validate(),
            Err(theme_error::insufficient_contrast)
        );
    }
}

#[test]
fn metrics_are_bounded_and_leave_control_and_panel_clearance() {
    let metrics = builtin(theme_mode::light, density_mode::comfortable).metrics;
    for invalid in [
        chrome_metrics {
            workarea_top: 48,
            ..metrics
        },
        chrome_metrics {
            panel_height: u32::MAX,
            ..metrics
        },
        chrome_metrics {
            control_size: 36,
            title_height: 32,
            ..metrics
        },
        chrome_metrics {
            shadow_offset: 13,
            ..metrics
        },
        chrome_metrics {
            workarea_margin: 0,
            ..metrics
        },
    ] {
        assert_eq!(
            theme_settings {
                overrides: theme_overrides {
                    metrics: Some(invalid),
                    colors: None,
                    ..Default::default()
                },
                ..theme_settings::initial
            }
            .validate(),
            Err(theme_error::invalid_metrics)
        );
    }
}

#[test]
fn static_effects_never_override_reduced_motion_or_high_contrast() {
    for reduced_motion in [true, false] {
        let definition = theme_settings {
            reduced_motion,
            ..theme_settings::initial
        }
        .validate()
        .unwrap();
        assert_eq!(definition.reduced_motion, reduced_motion);
        assert_eq!(definition.animation_duration_ms, 0);
    }
    assert!(
        !theme_settings {
            theme: theme_mode::high_contrast,
            depth: true,
            ..theme_settings::initial
        }
        .validate()
        .unwrap()
        .depth
    );
    assert!(
        !theme_settings {
            depth: false,
            ..theme_settings::initial
        }
        .validate()
        .unwrap()
        .depth
    );
}

#[test]
fn validated_component_overrides_are_used_without_changing_catalog_defaults() {
    let base = builtin(theme_mode::light, density_mode::comfortable);
    let colors = semantic_colors {
        background: [234, 240, 248],
        accent: [30, 60, 140],
        ..base.colors
    };
    let metrics = chrome_metrics {
        tile_gap: 16,
        shadow_offset: 4,
        ..base.metrics
    };
    let custom = theme_settings {
        overrides: theme_overrides {
            colors: Some(colors),
            metrics: Some(metrics),
            ..Default::default()
        },
        ..theme_settings::initial
    }
    .validate()
    .unwrap();
    assert_eq!(custom.colors, colors);
    assert_eq!(custom.metrics, metrics);
    assert_eq!(builtin(theme_mode::light, density_mode::comfortable), base);
}

#[test]
fn official_pair_keeps_clay_roles_and_readable_neutral_titles() {
    let dark = builtin(theme_mode::dark, density_mode::comfortable);
    assert_eq!(dark.colors.background, [0x14; 3]);
    assert_eq!(dark.colors.surface, [0x1a; 3]);
    assert_eq!(dark.colors.active_title, [0x22; 3]);
    assert_eq!(dark.colors.text, [0xec, 0xe9, 0xe5]);
    assert_eq!(dark.decorations.primary, [0xf2, 0x66, 0x1a]);
    assert_eq!(dark.colors.accent, [0x33, 0xb7, 0xa6]);
    assert_eq!(theme_settings::initial.theme, theme_mode::light);

    for mode in [theme_mode::light, theme_mode::dark] {
        let theme = builtin(mode, density_mode::comfortable);
        for surface in [theme.colors.inactive_title, theme.colors.active_title] {
            assert!(contrast_at_least(theme.decorations.primary, surface, 30));
            assert!(contrast_at_least(theme.colors.text, surface, 45));
        }
        assert_ne!(theme.colors.active_title, theme.decorations.primary);
        assert_ne!(theme.decorations.primary, theme.colors.accent);
    }
}

#[test]
fn decoration_overrides_cannot_hide_hover_pressed_or_destructive_controls() {
    let base = builtin(theme_mode::dark, density_mode::comfortable);
    let d = base.decorations;
    for decorations in [
        decoration_palette {
            control_ink: d.control_hover,
            ..d
        },
        decoration_palette {
            control_ink: d.control_pressed,
            ..d
        },
        decoration_palette {
            danger_ink: d.danger_hover,
            ..d
        },
        decoration_palette {
            danger_ink: d.danger_pressed,
            ..d
        },
        decoration_palette {
            primary_ink: d.primary,
            ..d
        },
        decoration_palette {
            primary_hover: d.primary_ink,
            ..d
        },
        decoration_palette {
            primary_pressed: d.primary_ink,
            ..d
        },
        decoration_palette {
            primary: base.colors.surface,
            primary_ink: base.colors.text,
            primary_hover: base.colors.surface,
            primary_pressed: base.colors.surface,
            ..d
        },
    ] {
        assert_eq!(
            theme_settings {
                theme: theme_mode::dark,
                overrides: theme_overrides {
                    decorations: Some(decorations),
                    ..Default::default()
                },
                ..theme_settings::initial
            }
            .validate(),
            Err(theme_error::insufficient_contrast)
        );
    }
}

#[test]
fn frame_geometry_and_shadow_coverage_have_finite_safe_bounds() {
    let frame = builtin(theme_mode::dark, density_mode::comfortable).frame;
    for invalid in [
        frame_style {
            radius: 13,
            ..frame
        },
        frame_style {
            border_width: 0,
            ..frame
        },
        frame_style {
            border_width: 4,
            ..frame
        },
        frame_style {
            title_separator: 3,
            ..frame
        },
        frame_style {
            control_radius: 11,
            ..frame
        },
        frame_style {
            shadow_spread: 17,
            ..frame
        },
        frame_style {
            shadow_opacity: 161,
            ..frame
        },
    ] {
        assert_eq!(
            theme_settings {
                overrides: theme_overrides {
                    frame: Some(invalid),
                    ..Default::default()
                },
                ..theme_settings::initial
            }
            .validate(),
            Err(theme_error::invalid_frame)
        );
    }
}

#[test]
fn custom_decoration_and_frame_components_reach_the_validated_definition() {
    let base = builtin(theme_mode::light, density_mode::comfortable);
    let decorations = decoration_palette {
        control_hover: [220, 209, 195],
        inactive_border: [150, 142, 130],
        ..base.decorations
    };
    let frame = frame_style {
        radius: 10,
        shadow_spread: 12,
        shadow_opacity: 120,
        ..base.frame
    };
    let settings = theme_settings {
        overrides: theme_overrides {
            decorations: Some(decorations),
            frame: Some(frame),
            ..Default::default()
        },
        ..theme_settings::initial
    };
    let custom = settings.validate().unwrap();
    assert_eq!(custom.decorations, decorations);
    assert_eq!(custom.frame, frame);
    assert_eq!(custom.colors, base.colors);
    assert_eq!(builtin(theme_mode::light, density_mode::comfortable), base);

    // A dark color component combined with this light decoration component is
    // rejected unless the caller also supplies a compatible decoration palette.
    assert_eq!(
        theme_settings {
            overrides: theme_overrides {
                colors: Some(builtin(theme_mode::dark, density_mode::comfortable).colors),
                ..settings.overrides
            },
            ..settings
        }
        .validate(),
        Err(theme_error::insufficient_contrast)
    );
}

#[test]
fn corner_radius_override_preserves_each_themes_other_frame_defaults() {
    for theme in [
        theme_mode::light,
        theme_mode::dark,
        theme_mode::high_contrast,
    ] {
        let base = builtin(theme, density_mode::comfortable);
        for radius in [0, 6, 12] {
            let settings = theme_settings {
                theme,
                overrides: theme_overrides {
                    corner_radius: Some(radius),
                    ..Default::default()
                },
                ..theme_settings::initial
            };
            let mut expected = base;
            expected.frame.radius = radius;
            assert_eq!(settings.validate().unwrap(), expected);
            assert_eq!(settings.overrides.frame, None);
        }
    }
}

#[test]
fn corner_radius_layers_over_an_explicit_profile_without_rewriting_it() {
    let base = builtin(theme_mode::dark, density_mode::comfortable);
    let frame = frame_style {
        radius: 10,
        control_radius: 7,
        shadow_spread: 12,
        shadow_opacity: 123,
        ..base.frame
    };
    let mut settings = theme_settings {
        theme: theme_mode::dark,
        overrides: theme_overrides {
            frame: Some(frame),
            corner_radius: Some(0),
            ..Default::default()
        },
        ..theme_settings::initial
    };
    assert_eq!(
        settings.validate().unwrap().frame,
        frame_style { radius: 0, ..frame }
    );
    assert_eq!(settings.overrides.frame, Some(frame));
    settings.overrides.corner_radius = None;
    assert_eq!(settings.validate().unwrap().frame, frame);
    for invalid in [13, u8::MAX] {
        settings.overrides.corner_radius = Some(invalid);
        assert_eq!(settings.validate(), Err(theme_error::invalid_frame));
        assert_eq!(settings.overrides.frame, Some(frame));
    }
    assert_eq!(theme_settings::initial.overrides.corner_radius, None);
}
