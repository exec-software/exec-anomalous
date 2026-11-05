//! Export only the actual compositor's public built-in desktop for visual QA.
use anomalous_desktop_compositor::{compose_row, frame_height, frame_width, row_byte_count};
use anomalous_desktop_workspace::{
    client_grants, client_origin, desktop, desktop_command, theme_mode,
};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let usage = "usage: public_preview OUTPUT.ppm [light|dark|contrast] [regular|large] [desktop|appearance]";
    let path = arguments.next().ok_or(usage)?;
    let theme = match arguments
        .next()
        .as_deref()
        .and_then(|arg| arg.to_str())
        .unwrap_or("light")
    {
        "light" => theme_mode::light,
        "dark" => theme_mode::dark,
        "contrast" => theme_mode::high_contrast,
        _ => return Err(usage.into()),
    };
    let font_scale = match arguments
        .next()
        .as_deref()
        .and_then(|arg| arg.to_str())
        .unwrap_or("regular")
    {
        "regular" => 1,
        "large" => 2,
        _ => return Err(usage.into()),
    };
    let appearance = match arguments
        .next()
        .as_deref()
        .and_then(|arg| arg.to_str())
        .unwrap_or("desktop")
    {
        "desktop" => false,
        "appearance" => true,
        _ => return Err(usage.into()),
    };
    if arguments.next().is_some() {
        return Err(usage.into());
    }
    let (mut desktop, control, initial) = desktop::new_demo(frame_width, frame_height)
        .map_err(|_| "public desktop construction failed")?;
    desktop
        .begin(&control, &initial)
        .map_err(|_| "begin initial")?;
    desktop
        .attest_presented(&control, &initial)
        .map_err(|_| "preview initial assertion")?;
    desktop
        .publish(&control, initial)
        .map_err(|_| "publish initial")?;
    let endpoint = desktop
        .bind_client(&control, client_origin::human, client_grants::all)
        .map_err(|_| "preview endpoint")?;
    let view = desktop
        .observe(&endpoint)
        .map_err(|_| "preview observation")?;
    let mut settings = view.theme_settings();
    settings.theme = theme;
    settings.font_scale = font_scale;
    let mut commands = vec![
        desktop_command::set_theme_settings(settings),
        desktop_command::set_pointer { x: 946, y: 636 },
    ];
    if appearance {
        commands.push(desktop_command::open_appearance);
    }
    let ticket = desktop
        .prepare_batch(&endpoint, view.revision(), &commands)
        .map_err(|_| "preview settings")?;
    let view = desktop
        .candidate(&ticket)
        .map_err(|_| "public view unavailable")?;
    let mut file = std::fs::File::create(path)?;
    write!(file, "P6\n{frame_width} {frame_height}\n255\n")?;
    let mut row = [0; row_byte_count];
    for y in 0..frame_height {
        compose_row(&view, y, &mut row).map_err(|error| error.code())?;
        file.write_all(&row)?;
    }
    file.flush()?;
    Ok(())
}
