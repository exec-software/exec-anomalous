use crate::capsule::{
    capsule_color, capsule_command_kind, capsule_rect, capsule_shape, capsule_slot,
    fixture_command_count, label_selector, render_capsule,
};
use crate::font::{label_pixel, layout};
use crate::renderer_error;

pub const frame_width: u32 = 1024;
pub const frame_height: u32 = 768;
pub const row_byte_count: usize = frame_width as usize * 3;

pub fn render_row(
    capsule: &render_capsule,
    row: u32,
    output: &mut [u8],
) -> Result<(), renderer_error> {
    begin_row(row, output)?;
    for index in 0..fixture_command_count {
        let command = capsule.command(index);
        paint_command_on_row(
            output,
            row,
            paint_command {
                kind: command.kind,
                shape: command.shape,
                color: command.color,
                secondary_color: command.secondary_color,
                slot: command.slot,
                label: command.label,
                bounds: command.bounds,
                state_active: command.state_active,
            },
        );
    }
    Ok(())
}

pub(crate) struct paint_command {
    pub(crate) kind: capsule_command_kind,
    pub(crate) shape: capsule_shape,
    pub(crate) color: capsule_color,
    pub(crate) secondary_color: Option<capsule_color>,
    pub(crate) slot: capsule_slot,
    pub(crate) label: Option<label_selector>,
    pub(crate) bounds: capsule_rect,
    pub(crate) state_active: bool,
}

pub(crate) fn begin_row(row: u32, output: &mut [u8]) -> Result<(), renderer_error> {
    if row >= frame_height {
        return Err(renderer_error::wrong_row);
    }
    if output.len() != row_byte_count {
        return Err(renderer_error::wrong_row_buffer_length);
    }
    output.fill(0);
    Ok(())
}

pub(crate) fn paint_command_on_row(output: &mut [u8], row: u32, command: paint_command) {
    match command.kind {
        capsule_command_kind::fill => {
            paint_filled_rect(output, row, command.bounds, command.color);
        }
        capsule_command_kind::outline => {
            paint_outline(output, row, command.bounds, 2, command.color);
        }
        capsule_command_kind::label => {
            if let Some(selector) = command.label {
                paint_label(
                    output,
                    row,
                    command.bounds,
                    command.slot,
                    selector,
                    command.color,
                );
            }
        }
        capsule_command_kind::enabled_marker => {
            paint_marker(output, row, command.bounds, command.shape, command.color);
        }
        capsule_command_kind::focus_outline => {
            if command.state_active
                && command.shape == capsule_shape::focus_double_outline
                && let Some(inner) = command.secondary_color
            {
                paint_focus(output, row, command.bounds, command.color, inner);
            }
        }
        capsule_command_kind::shadow => {
            paint_offset_rect(output, row, command.bounds, 8, 8, command.color);
        }
        capsule_command_kind::highlight => {
            paint_highlight(output, row, command.bounds, command.color);
        }
    }
}

fn paint_filled_rect(output: &mut [u8], row: u32, bounds: capsule_rect, color: capsule_color) {
    if row < bounds.y() || row >= bounds.y() + bounds.height() {
        return;
    }
    paint_span(output, bounds.x(), bounds.x() + bounds.width(), color);
}

fn paint_offset_rect(
    output: &mut [u8],
    row: u32,
    bounds: capsule_rect,
    offset_x: u32,
    offset_y: u32,
    color: capsule_color,
) {
    let x = bounds.x().saturating_add(offset_x);
    let y = bounds.y().saturating_add(offset_y);
    let right = (x.saturating_add(bounds.width())).min(frame_width);
    let bottom = (y.saturating_add(bounds.height())).min(frame_height);
    if row >= y && row < bottom {
        paint_span(output, x.min(frame_width), right, color);
    }
}

fn paint_outline(
    output: &mut [u8],
    row: u32,
    bounds: capsule_rect,
    thickness: u32,
    color: capsule_color,
) {
    if row < bounds.y() || row >= bounds.y() + bounds.height() {
        return;
    }
    let right = bounds.x() + bounds.width();
    let bottom = bounds.y() + bounds.height();
    if row < bounds.y() + thickness || row >= bottom.saturating_sub(thickness) {
        paint_span(output, bounds.x(), right, color);
    } else {
        paint_span(output, bounds.x(), bounds.x() + thickness, color);
        paint_span(output, right.saturating_sub(thickness), right, color);
    }
}

fn paint_highlight(output: &mut [u8], row: u32, bounds: capsule_rect, color: capsule_color) {
    if row < bounds.y() || row >= bounds.y() + bounds.height() {
        return;
    }
    if row < bounds.y() + 2 {
        paint_span(
            output,
            bounds.x() + 2,
            bounds.x() + bounds.width().saturating_sub(2),
            color,
        );
    } else if row < bounds.y() + bounds.height().saturating_sub(2) {
        paint_span(output, bounds.x() + 2, bounds.x() + 4, color);
    }
}

fn paint_marker(
    output: &mut [u8],
    row: u32,
    bounds: capsule_rect,
    shape: capsule_shape,
    color: capsule_color,
) {
    let marker = pixel_rect {
        x: bounds.x() + bounds.width() - 28,
        y: bounds.y() + (bounds.height() - 12) / 2,
        width: 12,
        height: 12,
    };
    match shape {
        capsule_shape::enabled => paint_pixel_rect(output, row, marker, color),
        capsule_shape::disabled => {
            paint_pixel_outline(output, row, marker, 2, color);
            if row >= marker.y + 2 && row < marker.y + marker.height - 2 {
                let local_y = row - marker.y;
                let first = marker.x + local_y;
                let second = marker.x + marker.width - 1 - local_y;
                paint_span(output, first, first + 1, color);
                paint_span(output, second, second + 1, color);
            }
        }
        _ => {}
    }
}

fn paint_focus(
    output: &mut [u8],
    row: u32,
    bounds: capsule_rect,
    outer: capsule_color,
    inner: capsule_color,
) {
    let outer_rect = expanded(bounds, 6);
    let inner_rect = expanded(bounds, 3);
    paint_pixel_outline(output, row, outer_rect, 2, outer);
    paint_pixel_outline(output, row, inner_rect, 1, inner);
}

fn paint_label(
    output: &mut [u8],
    row: u32,
    bounds: capsule_rect,
    slot: crate::capsule::capsule_slot,
    selector: crate::capsule::label_selector,
    color: capsule_color,
) {
    let layout = layout(slot, bounds);
    if row < layout.y || row >= layout.y + crate::font::glyph_height * layout.scale {
        return;
    }
    let mut x = bounds.x();
    let right = (bounds.x() + bounds.width()).min(frame_width);
    while x < right {
        if label_pixel(selector, layout, x, row) {
            paint_span(output, x, x + 1, color);
        }
        x += 1;
    }
}

#[derive(Clone, Copy)]
struct pixel_rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn expanded(bounds: capsule_rect, amount: u32) -> pixel_rect {
    let x = bounds.x().saturating_sub(amount);
    let y = bounds.y().saturating_sub(amount);
    let right = (bounds.x() + bounds.width())
        .saturating_add(amount)
        .min(frame_width);
    let bottom = (bounds.y() + bounds.height())
        .saturating_add(amount)
        .min(frame_height);
    pixel_rect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}

fn paint_pixel_rect(output: &mut [u8], row: u32, bounds: pixel_rect, color: capsule_color) {
    if row >= bounds.y && row < bounds.y + bounds.height {
        paint_span(output, bounds.x, bounds.x + bounds.width, color);
    }
}

fn paint_pixel_outline(
    output: &mut [u8],
    row: u32,
    bounds: pixel_rect,
    thickness: u32,
    color: capsule_color,
) {
    if row < bounds.y || row >= bounds.y + bounds.height {
        return;
    }
    let right = bounds.x + bounds.width;
    let bottom = bounds.y + bounds.height;
    if row < bounds.y + thickness || row >= bottom.saturating_sub(thickness) {
        paint_span(output, bounds.x, right, color);
    } else {
        paint_span(output, bounds.x, bounds.x + thickness, color);
        paint_span(output, right.saturating_sub(thickness), right, color);
    }
}

fn paint_span(output: &mut [u8], start: u32, end: u32, color: capsule_color) {
    let start = start.min(frame_width) as usize;
    let end = end.min(frame_width) as usize;
    let rgb = color.rgb();
    for x in start..end {
        let offset = x * 3;
        output[offset..offset + 3].copy_from_slice(&rgb);
    }
}
