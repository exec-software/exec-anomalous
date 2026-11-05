#![allow(non_camel_case_types, non_upper_case_globals)]
//! Host-only deterministic exports of the compiled native decoration recipes.

use anomalous_desktop_theme::{
    assets::{control_asset, control_assets, recipe, source_extent},
    builtin, contrast_at_least, density_mode, rgb, theme_definition, theme_mode,
};
use std::{collections::BTreeSet, env, fmt::Write, fs, io, path::Path};

const button_states: [&str; 5] = ["normal", "hover", "pressed", "inactive", "disabled"];
const frame_states: [&str; 4] = ["active", "inactive", "maximized", "tiled"];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut check = false;
    let mut output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/desktop/frames");
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--check" => check = true,
            "--out" => output = arguments.next().ok_or("--out requires a directory")?.into(),
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    let mut files = Vec::new();
    files.push(("README.md".to_string(), readme().to_string()));
    files.push(("manifest.json".to_string(), manifest()));
    for asset in control_assets {
        files.push((format!("icons/{}.svg", asset.id()), icon(asset)));
    }
    for (id, mode) in themes() {
        let theme = builtin(mode, density_mode::comfortable);
        for state in frame_states {
            files.push((format!("{id}/{state}.svg"), frame_reference(theme, state)));
        }
        files.push((format!("{id}/controls.svg"), control_sheet(theme)));
    }
    files.push(("contact-sheet.svg".to_string(), contact_sheet()));
    let expected: BTreeSet<_> = files.iter().map(|(path, _)| path.clone()).collect();
    if check {
        let mut actual = BTreeSet::new();
        collect_files(&output, &output, &mut actual)?;
        if actual != expected {
            return Err(format!(
                "asset inventory mismatch; missing: {:?}; unexpected: {:?}",
                expected.difference(&actual).collect::<Vec<_>>(),
                actual.difference(&expected).collect::<Vec<_>>()
            )
            .into());
        }
        for (path, content) in &files {
            if fs::read(output.join(path))? != content.as_bytes() {
                return Err(
                    format!("stale generated asset: {path}; run exporter without --check").into(),
                );
            }
        }
        println!("verified {} native frame reference assets", files.len());
    } else {
        for (path, content) in &files {
            let path = output.join(path);
            fs::create_dir_all(path.parent().ok_or("asset has no parent")?)?;
            fs::write(path, content)?;
        }
        println!("exported {} native frame reference assets", files.len());
    }
    Ok(())
}

fn collect_files(root: &Path, directory: &Path, files: &mut BTreeSet<String>) -> io::Result<()> {
    for item in fs::read_dir(directory)? {
        let item = item?;
        let kind = item.file_type()?;
        if kind.is_dir() {
            collect_files(root, &item.path(), files)?;
        } else if kind.is_file() {
            files.insert(
                item.path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        } else {
            return Err(io::Error::other("asset tree contains a non-regular file"));
        }
    }
    Ok(())
}

fn themes() -> [(&'static str, theme_mode); 3] {
    [
        ("dark", theme_mode::dark),
        ("light", theme_mode::light),
        ("high-contrast", theme_mode::high_contrast),
    ]
}

fn hex(color: rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
}

fn svg(width: u32, height: u32, title: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" role=\"img\" aria-labelledby=\"title\">\n<title id=\"title\">{title}</title>\n{body}</svg>\n"
    )
}

fn strokes(asset: control_asset) -> String {
    let mut result = String::new();
    for segment in recipe(asset) {
        writeln!(result, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"currentColor\" stroke-width=\"{}\" stroke-linecap=\"round\"/>", segment.x1, segment.y1, segment.x2, segment.y2, segment.stroke_width).unwrap();
    }
    result
}

fn icon(asset: control_asset) -> String {
    svg(source_extent, source_extent, asset.id(), &strokes(asset))
}

fn symbols() -> String {
    let mut result = String::from("<defs>\n");
    for asset in control_assets {
        writeln!(
            result,
            "<symbol id=\"{}\" viewBox=\"0 0 {source_extent} {source_extent}\">\n{}</symbol>",
            asset.id(),
            strokes(asset)
        )
        .unwrap();
    }
    result.push_str("</defs>\n");
    result
}

fn control_colors(theme: theme_definition, asset: control_asset, state: &str) -> (rgb, rgb) {
    let d = theme.decorations;
    match (asset == control_asset::close, state) {
        (true, "hover") => (d.danger_hover, d.danger_ink),
        (true, "pressed") => (d.danger_pressed, d.danger_ink),
        (_, "hover") => (d.control_hover, d.control_ink),
        (_, "pressed") => (d.control_pressed, d.control_ink),
        (_, "inactive") => (theme.colors.inactive_title, theme.colors.secondary_text),
        (_, "disabled") => (
            theme.colors.active_title,
            if contrast_at_least(theme.colors.secondary_text, theme.colors.active_title, 30) {
                theme.colors.secondary_text
            } else {
                theme.colors.active_text
            },
        ),
        _ => (theme.colors.active_title, theme.colors.active_text),
    }
}

fn control(theme: theme_definition, asset: control_asset, state: &str, x: u32, y: u32) -> String {
    let (background, foreground) = control_colors(theme, asset, state);
    let size = theme.metrics.control_size;
    let icon_size = 16;
    let inset = (size - icon_size) / 2;
    let mut result = format!(
        "<rect x=\"{x}\" y=\"{y}\" width=\"{size}\" height=\"{size}\" rx=\"{}\" fill=\"{}\"/>\n<use href=\"#{}\" x=\"{}\" y=\"{}\" width=\"{icon_size}\" height=\"{icon_size}\" color=\"{}\"/>\n",
        theme.frame.control_radius,
        hex(background),
        asset.id(),
        x + inset,
        y + inset,
        hex(foreground)
    );
    if state == "disabled" {
        writeln!(
            result,
            "<rect x=\"{}\" y=\"{}\" width=\"4\" height=\"1\" fill=\"{}\"/>",
            x + size / 2 - 2,
            y + size - 3,
            hex(foreground)
        )
        .unwrap();
    }
    result
}

fn frame_body(theme: theme_definition, state: &str) -> String {
    let c = theme.colors;
    let f = theme.frame;
    let inactive = state == "inactive";
    let snapped = state == "maximized" || state == "tiled";
    let radius = if snapped { 0 } else { f.radius };
    let x = 16;
    let y = 16;
    let width = 388;
    let height = 224;
    let title = theme.metrics.title_height;
    let title_x = x + 4;
    let title_y = y + 4;
    let title_width = width - 8;
    let title_background = if inactive {
        c.inactive_title
    } else {
        c.active_title
    };
    let mut body = format!(
        "<rect width=\"420\" height=\"256\" fill=\"{}\"/>\n",
        hex(c.background)
    );
    if theme.depth && !snapped {
        // A declarative depth reference, not a claim of pixel equality with the
        // native compositor's bounded integer shadow rasterization.
        let spread = u32::from(f.shadow_spread);
        writeln!(body, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" fill-opacity=\"{:.6}\"/>", x - spread / 2, y + theme.metrics.shadow_offset - spread / 2, width + spread, height + spread, u32::from(radius) + spread / 2, hex(c.shadow), f64::from(f.shadow_opacity) / 255.0).unwrap();
    }
    writeln!(body, "<defs><clipPath id=\"frame_clip\"><rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" rx=\"{radius}\"/></clipPath></defs>\n<g clip-path=\"url(#frame_clip)\">\n<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" fill=\"{}\"/>\n<rect x=\"{title_x}\" y=\"{title_y}\" width=\"{title_width}\" height=\"{title}\" fill=\"{}\"/>", hex(c.surface), hex(title_background)).unwrap();
    if f.title_separator > 0 {
        writeln!(
            body,
            "<rect x=\"{title_x}\" y=\"{}\" width=\"{title_width}\" height=\"{}\" fill=\"{}\"/>",
            title_y + title,
            f.title_separator,
            hex(c.border)
        )
        .unwrap();
    }
    writeln!(body, "<text x=\"{}\" y=\"{}\" font-family=\"sans-serif\" font-size=\"12\" fill=\"{}\">Native workspace</text>", title_x + 12, title_y + title / 2 + 4, hex(if inactive { c.text } else { c.active_text })).unwrap();
    let size = theme.metrics.control_size;
    let cy = title_y + (title - size) / 2;
    let cx = x + width - 4 - 3 * (size + 2);
    let label_width = title_width - 3 * (size + 2);
    if !inactive && label_width > 260 {
        let mark = if contrast_at_least(theme.decorations.primary, title_background, 30) {
            theme.decorations.primary
        } else {
            theme.decorations.primary_ink
        };
        writeln!(body, "<rect x=\"{}\" y=\"{}\" width=\"5\" height=\"5\" rx=\"2\" fill=\"{}\"/>\n<text x=\"{}\" y=\"{}\" font-family=\"sans-serif\" font-size=\"7\" fill=\"{}\">FOCUS</text>", title_x + label_width - 49, title_y + (title - 5) / 2, hex(mark), title_x + label_width - 38, title_y + title / 2 + 3, hex(c.active_text)).unwrap();
    }
    for (index, asset) in [
        control_asset::minimize,
        if state == "maximized" {
            control_asset::restore
        } else {
            control_asset::maximize
        },
        control_asset::close,
    ]
    .into_iter()
    .enumerate()
    {
        body.push_str(&control(
            theme,
            asset,
            if state == "tiled" && asset == control_asset::maximize {
                "disabled"
            } else if inactive {
                "inactive"
            } else {
                "normal"
            },
            cx + index as u32 * (size + 2),
            cy,
        ));
    }
    writeln!(body, "<text x=\"{}\" y=\"{}\" font-family=\"sans-serif\" font-size=\"18\" fill=\"{}\">A shared frame language</text>\n<text x=\"{}\" y=\"{}\" font-family=\"sans-serif\" font-size=\"12\" fill=\"{}\">{} / {state}</text>\n<rect x=\"{}\" y=\"{}\" width=\"110\" height=\"3\" fill=\"{}\"/>", x + 24, y + title + 48, hex(c.text), x + 24, y + title + 72, hex(c.secondary_text), theme.name, x + 24, y + title + 92, hex(c.accent)).unwrap();
    body.push_str("</g>\n");
    // Insets keep the complete border inside the declared frame bounds.
    let border = f.border_width + u8::from(!inactive);
    let inset = f64::from(border) / 2.0;
    writeln!(body, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>", f64::from(x) + inset, f64::from(y) + inset, f64::from(width) - f64::from(border), f64::from(height) - f64::from(border), (f64::from(radius) - inset).max(0.0), hex(if inactive { theme.decorations.inactive_border } else { c.focus_outer }), border).unwrap();
    if !inactive {
        let keyline_inset = f64::from(border) + 0.5;
        writeln!(body, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1\"/>", f64::from(x) + keyline_inset, f64::from(y) + keyline_inset, f64::from(width) - 2.0 * keyline_inset, f64::from(height) - 2.0 * keyline_inset, (f64::from(radius) - keyline_inset).max(0.0), hex(c.focus_inner)).unwrap();
    }
    body
}

fn frame_reference(theme: theme_definition, state: &str) -> String {
    svg(
        420,
        256,
        &format!("{} {state} frame reference", theme.name),
        &(symbols() + &frame_body(theme, state)),
    )
}

fn control_sheet(theme: theme_definition) -> String {
    let mut body = symbols();
    writeln!(body, "<rect width=\"420\" height=\"300\" fill=\"{}\"/>\n<text x=\"20\" y=\"28\" font-family=\"sans-serif\" font-size=\"16\" fill=\"{}\">{} / controls</text>", hex(theme.colors.surface), hex(theme.colors.text), theme.name).unwrap();
    for (row, state) in button_states.into_iter().enumerate() {
        let y = 56 + row as u32 * 44;
        writeln!(body, "<text x=\"20\" y=\"{}\" font-family=\"sans-serif\" font-size=\"12\" fill=\"{}\">{state}</text>", y + 18, hex(theme.colors.secondary_text)).unwrap();
        for (column, asset) in control_assets.into_iter().enumerate() {
            body.push_str(&control(theme, asset, state, 150 + column as u32 * 60, y));
        }
    }
    svg(420, 300, &format!("{} control states", theme.name), &body)
}

fn contact_sheet() -> String {
    // Nested SVG images keep local clip/symbol identifiers scoped separately.
    let mut body = String::from(
        "<rect width=\"1320\" height=\"1560\" fill=\"#141414\"/>\n<text x=\"24\" y=\"38\" font-family=\"sans-serif\" font-size=\"24\" fill=\"#ece9e5\">Anomalous / native frame system</text>\n<text x=\"24\" y=\"64\" font-family=\"sans-serif\" font-size=\"14\" fill=\"#bcb5ae\">Clay-inspired dark, warm light, and high contrast. Shared vector control recipes.</text>\n",
    );
    for (column, (id, _)) in themes().into_iter().enumerate() {
        for (row, state) in frame_states.into_iter().chain(["controls"]).enumerate() {
            let x = 12 + column as u32 * 436;
            let y = 112 + row as u32 * 278;
            writeln!(body, "<text x=\"{}\" y=\"{}\" font-family=\"sans-serif\" font-size=\"12\" fill=\"#ece9e5\">{id} / {state}</text>\n<image href=\"{id}/{state}.svg\" x=\"{x}\" y=\"{y}\" width=\"420\" height=\"{}\"/>", x + 16, y - 8, if state == "controls" { 300 } else { 256 }).unwrap();
        }
    }
    svg(
        1320,
        1560,
        "Anomalous native frame asset contact sheet",
        &body,
    )
}

fn manifest() -> String {
    let mut out = String::from(
        "{\n  \"schema\": 1,\n  \"id\": \"anomalous-native-frames\",\n  \"role\": \"compiled-recipe-reference-export\",\n  \"generator\": \"cargo run -p anomalous-desktop-theme --example export_assets --target x86_64-unknown-linux-gnu\",\n  \"source\": \"crates/anomalous-desktop-theme/src/assets.rs\",\n  \"source_view_box\": [0, 0, 24, 24],\n  \"raster_size\": {\"minimum\": 8, \"maximum\": 32, \"samples_per_pixel\": 16},\n  \"control_ids\": [\"minimize\", \"maximize\", \"restore\", \"close\"],\n  \"control_states\": [\"normal\", \"hover\", \"pressed\", \"inactive\", \"disabled\"],\n  \"frame_states\": [\"active\", \"inactive\", \"maximized\", \"tiled\"],\n  \"runtime_external_asset_loading\": false,\n  \"themes\": [\n",
    );
    for (index, (id, mode)) in themes().into_iter().enumerate() {
        let theme = builtin(mode, density_mode::comfortable);
        writeln!(out, "    {{\"id\": \"{id}\", \"name\": \"{}\", \"theme_schema\": {}, \"primary\": \"{}\", \"accent\": \"{}\", \"surface\": \"{}\", \"radius\": {}, \"border_width\": {}, \"title_separator\": {}, \"control_radius\": {}, \"shadow_spread\": {}, \"shadow_opacity\": {}}}{}", theme.name, theme.schema, hex(theme.decorations.primary), hex(theme.colors.accent), hex(theme.colors.surface), theme.frame.radius, theme.frame.border_width, theme.frame.title_separator, theme.frame.control_radius, theme.frame.shadow_spread, theme.frame.shadow_opacity, if index < 2 { "," } else { "" }).unwrap();
    }
    out.push_str("  ]\n}\n");
    out
}

fn readme() -> &'static str {
    "# Anomalous native frame assets\n\n\
Original Anomalous native frame recipes inspired by Antimatter's Clay palette:\n\
neutral charcoal and warm light surfaces, orange focus, and teal secondary\n\
accents. Light is an Anomalous interpretation. No Antimatter glyphs, fonts, or\n\
frame assets are copied.\n\n\
The original visual study used Antimatter's Clay palette and frame composition\n\
as design references. The public token snapshot is available in\n\
<https://github.com/exec-software/exec-antimatter/tree/master/vendor/displacer>.\n\
It is background design context, not a build dependency or an exact public\n\
reproduction of the historical reference. Native recipes in this repository\n\
are the authoritative source for these generated assets.\n\n\
## Source and regeneration\n\n\
Do not edit exported SVGs or the manifest by hand. The authoritative window\n\
controls are the bounded vectors in\n\
`crates/anomalous-desktop-theme/src/assets.rs`. Native composition samples\n\
those same recipes with integer 4 x 4 coverage; SVG exports retain the original\n\
round-cap segments. The coverage approximation is native renderer behavior,\n\
not a promise that a browser's SVG rasterizer returns identical pixels.\n\n\
From the repository root:\n\n\
```sh\n\
cargo run -p anomalous-desktop-theme --example export_assets --target x86_64-unknown-linux-gnu\n\
cargo run -p anomalous-desktop-theme --example export_assets --target x86_64-unknown-linux-gnu -- --check\n\
```\n\n\
`--check` compares every generated byte and rejects missing or extra files.\n\
An optional `--out DIRECTORY` exports to another host directory.\n\n\
## Scope\n\n\
`icons/` contains symbolic 24 x 24 SVGs using `currentColor`. The dark, light,\n\
and high-contrast directories contain four frame-state references and a\n\
normal/hover/pressed/inactive/disabled control sheet. `contact-sheet.svg` uses relative\n\
references, so keep the package together when viewing it. The four controls\n\
are minimize, maximize, restore, and close.\n\n\
Frame references derive colors, metrics, border width, corner radius, and\n\
depth from the compiled theme definition. Floating active and inactive frames\n\
have rounded corners; maximized and tiled references have square corners and\n\
no shadow. Active frames pair the outer focus border with a one-pixel inner\n\
keyline in every theme. Disabled controls include a short baseline mark and do\n\
not acquire a hover or pressed background. Reference text uses the viewer's\n\
sans-serif font. Native text remains\n\
the embedded bitmap font, and the native compositor owns its bounded shadow\n\
implementation and pixel output. These SVG references are not framebuffer\n\
goldens or a runtime package format.\n\n\
There is no runtime SVG parser, external asset loader, font dependency, or\n\
executable theme code. Native control recipes remain allocation-free and\n\
unsafe-free. Theme and frame state remain available in the canonical semantic\n\
model independently of these pixels.\n"
}
