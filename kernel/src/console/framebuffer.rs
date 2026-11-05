use crate::boot_omen::{color_channel, framebuffer_info};
use core::cell::UnsafeCell;
use core::fmt;
use core::ptr;

#[derive(Clone, Copy)]
struct color {
    red: u8,
    green: u8,
    blue: u8,
}

impl color {
    const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

const background: color = color::rgb(222, 230, 237);
const highlight: color = color::rgb(250, 252, 254);
const shadow: color = color::rgb(170, 184, 196);
const surface: color = color::rgb(224, 232, 239);
const border: color = color::rgb(78, 91, 106);
const ink: color = color::rgb(22, 31, 43);
const muted_ink: color = color::rgb(61, 76, 92);
const accent: color = color::rgb(91, 70, 246);
const terminal_background: color = color::rgb(22, 31, 43);
const terminal_ink: color = color::rgb(235, 241, 247);
const failure_background: color = color::rgb(48, 20, 29);
const failure_surface: color = color::rgb(237, 222, 226);
const failure_border: color = color::rgb(116, 37, 58);
const failure_ink: color = color::rgb(73, 22, 36);

struct interrupt_failure_surface {
    framebuffer: UnsafeCell<Option<framebuffer_info>>,
}

impl interrupt_failure_surface {
    const fn empty() -> Self {
        Self {
            framebuffer: UnsafeCell::new(None),
        }
    }

    /// Replace the retained metadata under the caller's exclusive BSP access.
    ///
    /// # Safety
    ///
    /// No other execution context may read, write, or withdraw this cell.
    unsafe fn register(&self, info: Option<framebuffer_info>) {
        // SAFETY: The caller grants exclusive access to the cell.
        unsafe { *self.framebuffer.get() = info };
    }

    /// Consume the retained metadata under the caller's exclusive BSP access.
    ///
    /// # Safety
    ///
    /// No other execution context may read, write, or withdraw this cell.
    #[cfg(any(
        test,
        feature = "supervised_renderer_test",
        feature = "workspace_dev",
        feature = "native_application_test"
    ))]
    unsafe fn withdraw(&self) -> Option<framebuffer_info> {
        // SAFETY: The caller grants exclusive access to the cell.
        unsafe { (&mut *self.framebuffer.get()).take() }
    }

    /// Copy the retained metadata for a terminal draw.
    ///
    /// # Safety
    ///
    /// No other execution context may write or withdraw this cell.
    unsafe fn retained(&self) -> Option<framebuffer_info> {
        // SAFETY: The caller excludes concurrent mutation of the cell.
        unsafe { *self.framebuffer.get() }
    }
}

// SAFETY: Registration occurs on the BSP before IF is enabled. The development
// bootstrap may restore an explicitly withdrawn registration only after the
// temporary presenter is destroyed, still before IF is enabled. The only later
// access is a terminal IF-clear failure path with no other running processor.
unsafe impl Sync for interrupt_failure_surface {}

static registered_interrupt_failure_surface: interrupt_failure_surface =
    interrupt_failure_surface::empty();

/// Retain only the validated aperture description needed by a later terminal
/// interrupt failure. This does not create a live framebuffer writer.
///
/// # Safety
///
/// Call exactly once on the BSP before enabling maskable interrupts. A present
/// framebuffer must already satisfy the complete diagnostic-surface bounds and
/// ownership checks.
pub unsafe fn register_interrupt_failure_surface(info: Option<framebuffer_info>) {
    // SAFETY: The caller guarantees one-time exclusive initialization.
    unsafe { registered_interrupt_failure_surface.register(info) };
}

/// Begin a new diagnostic lifetime after the pre-interrupt desktop presenter
/// has consumed and released the original aperture ownership.
///
/// # Safety
/// IF must remain clear on the sole BSP, no framebuffer writer may remain,
/// and `info` must be the same validated aperture returned by that presenter.
/// The interrupt backend must never have been initialized.
#[cfg(all(feature = "desktop_dev", not(test)))]
pub unsafe fn restore_bootstrap_interrupt_failure_surface(info: framebuffer_info) -> bool {
    // SAFETY: The caller excludes every reader and writer of this cell.
    if unsafe { registered_interrupt_failure_surface.retained() }.is_some() {
        return false;
    }
    // SAFETY: The previous owner is destroyed and this is a new, serialized
    // lifetime before ordinary interrupt initialization.
    unsafe { registered_interrupt_failure_surface.register(Some(info)) };
    true
}

/// Restore the diagnostic path after the persistent workspace presenter ends.
///
/// # Safety
/// IF must be clear on the sole BSP, NMI masked, and every previous framebuffer
/// writer must have been destroyed. `info` is the same validated aperture
/// consumed by `withdraw_interrupt_failure_surface`. Unlike the bootstrap-only
/// helper, this transition may follow an initialized external interrupt backend.
#[cfg(all(feature = "workspace_dev", not(test)))]
pub unsafe fn restore_retired_interrupt_failure_surface(info: framebuffer_info) -> bool {
    // SAFETY: The caller excludes every reader and writer of this cell.
    if unsafe { registered_interrupt_failure_surface.retained() }.is_some() {
        return false;
    }
    // SAFETY: The prior aperture owner is destroyed before this new lifetime.
    unsafe { registered_interrupt_failure_surface.register(Some(info)) };
    true
}

/// Withdraw the retained interrupt-failure aperture before another owner is
/// constructed.
///
/// The returned metadata carries no write authority by itself. A caller must
/// still validate the complete aperture and establish unique ownership before
/// following its address. Withdrawal is consuming: every later registered
/// failure draw falls back to VGA text rather than retaining a second path to
/// the packed-pixel surface.
///
/// # Safety
///
/// IF must be clear, NMI must remain masked, and no other processor, terminal
/// failure path, or display owner may access the registration concurrently.
/// The caller must not re-register the aperture while that transferred owner
/// exists. The desktop's separate pre-interrupt restoration operation requires
/// complete destruction of the presenter before starting a new lifetime.
#[cfg(all(
    any(
        feature = "supervised_renderer_test",
        feature = "workspace_dev",
        feature = "native_application_test"
    ),
    not(test)
))]
pub unsafe fn withdraw_interrupt_failure_surface() -> Option<framebuffer_info> {
    // SAFETY: The caller provides exclusive access to the single BSP-owned
    // registration cell for this consuming transition.
    unsafe { registered_interrupt_failure_surface.withdraw() }
}

/// Replace any interrupted human surface with the terminal interrupt failure.
///
/// # Safety
///
/// IF must be clear, NMI must remain masked, and no other processor or writer
/// may resume. This function takes terminal ownership of a registered aperture.
pub unsafe fn draw_registered_interrupt_failure() {
    // SAFETY: The caller provides terminal exclusion from registration and all
    // live display writers. Reading the Copy metadata creates no aliasing
    // reference to the aperture.
    let framebuffer = unsafe { registered_interrupt_failure_surface.retained() };
    if let Some(framebuffer) = framebuffer {
        // SAFETY: Registration retained a validated aperture and the caller
        // transferred terminal ownership of its display writes.
        unsafe { draw_interrupt_failure_surface(framebuffer) };
    } else {
        draw_interrupt_failure();
    }
}

/// Draw the first human-facing surface from the same boot milestone represented
/// by the machine-readable serial events.
///
/// # Safety
///
/// `info` must name the unique writable framebuffer supplied by the bootloader.
pub unsafe fn draw_boot_surface(info: framebuffer_info) {
    let Some(byte_len) = info.pitch.checked_mul(info.height) else {
        return;
    };
    // SAFETY: The caller guarantees exclusive access to the validated aperture
    // for its full pitch-by-height extent.
    let mut canvas = unsafe { canvas::new(info.address as *mut u8, byte_len, info) };
    canvas.clear(background);

    if info.width < 160 || info.height < 100 {
        return;
    }

    let panel_width = (info.width * 3 / 4).clamp(140, 840);
    let panel_height = (info.height / 2).clamp(90, 420);
    let panel_x = (info.width - panel_width) / 2;
    let panel_y = (info.height - panel_height) / 2;
    let depth = (info.width.min(info.height) / 90).clamp(3, 10);
    let radius = (panel_height / 12).clamp(8, 28);

    canvas.fill_rounded_rect(
        panel_x.saturating_sub(depth),
        panel_y.saturating_sub(depth),
        panel_width,
        panel_height,
        radius,
        highlight,
    );
    canvas.fill_rounded_rect(
        panel_x + depth,
        panel_y + depth,
        panel_width,
        panel_height,
        radius,
        shadow,
    );
    canvas.fill_rounded_rect(panel_x, panel_y, panel_width, panel_height, radius, border);
    canvas.fill_rounded_rect(
        panel_x + 2,
        panel_y + 2,
        panel_width.saturating_sub(4),
        panel_height.saturating_sub(4),
        radius.saturating_sub(2),
        surface,
    );

    let badge_size = (panel_height / 5).clamp(26, 72);
    let badge_x = panel_x + (panel_width - badge_size) / 2;
    let badge_y = panel_y + panel_height / 8;
    canvas.fill_rounded_rect(
        badge_x.saturating_sub(depth / 2),
        badge_y.saturating_sub(depth / 2),
        badge_size,
        badge_size,
        badge_size / 3,
        highlight,
    );
    canvas.fill_rounded_rect(
        badge_x + depth / 2,
        badge_y + depth / 2,
        badge_size,
        badge_size,
        badge_size / 3,
        shadow,
    );
    canvas.fill_rounded_rect(
        badge_x,
        badge_y,
        badge_size,
        badge_size,
        badge_size / 3,
        accent,
    );

    let badge_scale = (badge_size / 10).max(2);
    canvas.draw_text_centered(
        "A",
        badge_y + (badge_size - 7 * badge_scale) / 2,
        badge_scale,
        highlight,
    );

    let title_scale = (panel_width / 100).clamp(2, 7);
    let title_y = badge_y + badge_size + panel_height / 12;
    canvas.draw_text_centered("ANOMALOUS", title_y, title_scale, ink);

    let subtitle_scale = title_scale.saturating_sub(2).max(1);
    let subtitle_y = title_y + 7 * title_scale + panel_height / 12;
    canvas.draw_text_centered("PEOPLE + MACHINES", subtitle_y, subtitle_scale, muted_ink);

    let status_scale = subtitle_scale.max(1);
    let status_y = panel_y + panel_height.saturating_sub(7 * status_scale + panel_height / 10);
    canvas.draw_text_centered("MEMORY + BOOT READY", status_y, status_scale, accent);
}

/// Present a legible status when no packed-pixel framebuffer is available.
pub fn draw_text_fallback() {
    const vga_text: *mut u16 = 0xb8000 as *mut u16;
    for cell in 0..(80 * 25) {
        // SAFETY: The bootstrap identity maps VGA text memory, and each loop
        // iteration writes a distinct cell inside the 80x25 aperture.
        unsafe { ptr::write_volatile(vga_text.add(cell), 0x0720) };
    }
    write_vga_line(vga_text, 9, 31, b"ANOMALOUS", 0x0f);
    write_vga_line(vga_text, 11, 27, b"PEOPLE + MACHINES", 0x07);
    write_vga_line(vga_text, 14, 32, b"MEMORY READY", 0x0a);
    write_vga_line(vga_text, 16, 35, b"BOOT READY", 0x0d);
}

/// Present a fail-closed memory-handoff error without trusting a framebuffer.
pub fn draw_memory_failure() {
    draw_vga_failure(b"MEMORY HANDOFF REJECTED", 27);
}

/// Present a fail-closed interrupt-path error without trusting graphics state.
pub fn draw_interrupt_failure() {
    draw_vga_failure(b"INTERRUPT PATH REJECTED", 28);
}

/// Present the same fail-closed diagnostic through a validated packed-pixel
/// aperture when Threshold has already entered graphics mode.
///
/// # Safety
///
/// `info` must name the unique writable framebuffer supplied by Threshold. Its
/// complete pitch-by-height extent must be disjoint from low and kernel memory.
pub unsafe fn draw_memory_failure_surface(info: framebuffer_info) {
    // SAFETY: The caller provides the complete unique framebuffer contract.
    unsafe { draw_failure_surface(info, "MEMORY HANDOFF REJECTED") };
}

/// Present a fail-closed interrupt-path error through a validated packed-pixel
/// aperture.
///
/// # Safety
///
/// `info` must name the unique writable framebuffer supplied by Threshold. The
/// caller must have terminally stopped any prior console owner before drawing.
pub unsafe fn draw_interrupt_failure_surface(info: framebuffer_info) {
    // SAFETY: The caller provides the complete terminal framebuffer contract.
    unsafe { draw_failure_surface(info, "INTERRUPT PATH REJECTED") };
}

unsafe fn draw_failure_surface(info: framebuffer_info, status: &str) {
    let Some(byte_len) = info.pitch.checked_mul(info.height) else {
        return;
    };
    // SAFETY: The caller guarantees unique access to the complete validated
    // aperture for the duration of this final diagnostic draw.
    let mut canvas = unsafe { canvas::new(info.address as *mut u8, byte_len, info) };
    canvas.clear(failure_background);

    if info.width < 160 || info.height < 100 {
        return;
    }

    let panel_width = (info.width * 4 / 5).clamp(150, 900);
    let panel_height = (info.height / 2).clamp(90, 360);
    let panel_x = (info.width - panel_width) / 2;
    let panel_y = (info.height - panel_height) / 2;
    let radius = (panel_height / 12).clamp(8, 28);
    canvas.fill_rounded_rect(
        panel_x,
        panel_y,
        panel_width,
        panel_height,
        radius,
        failure_border,
    );
    canvas.fill_rounded_rect(
        panel_x + 3,
        panel_y + 3,
        panel_width.saturating_sub(6),
        panel_height.saturating_sub(6),
        radius.saturating_sub(2),
        failure_surface,
    );

    let title_scale = (panel_width / 110).clamp(2, 7);
    let detail_scale = title_scale.saturating_sub(2).max(1);
    canvas.draw_text_centered(
        "ANOMALOUS",
        panel_y + panel_height / 5,
        title_scale,
        failure_ink,
    );
    canvas.draw_text_centered(
        status,
        panel_y + panel_height / 2,
        detail_scale,
        failure_border,
    );
    canvas.draw_text_centered(
        "SEE SERIAL DIAGNOSTICS",
        panel_y + panel_height * 3 / 4,
        detail_scale,
        failure_ink,
    );
}

fn draw_vga_failure(status: &[u8], column: usize) {
    const vga_text: *mut u16 = 0xb8000 as *mut u16;
    for cell in 0..(80 * 25) {
        // SAFETY: Threshold's low identity mapping includes the VGA aperture.
        unsafe { ptr::write_volatile(vga_text.add(cell), 0x0720) };
    }
    write_vga_line(vga_text, 10, 31, b"ANOMALOUS", 0x0f);
    write_vga_line(vga_text, 13, column, status, 0x4f);
    write_vga_line(vga_text, 15, 24, b"SEE SERIAL DIAGNOSTICS", 0x07);
}

/// Allocation-free text console for the milestone-zero shell.
pub struct screen_console {
    backend: screen_backend,
    column: usize,
    row: usize,
    columns: usize,
    rows: usize,
}

enum screen_backend {
    framebuffer(framebuffer_terminal),
    vga_text,
}

struct framebuffer_terminal {
    canvas: canvas,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    scale: usize,
}

impl screen_console {
    /// Create a console over the bootloader's packed-pixel framebuffer.
    ///
    /// # Safety
    ///
    /// `info` must describe the uniquely owned, writable framebuffer validated
    /// from `boot_omen`. No other framebuffer writer may remain live.
    #[must_use]
    pub unsafe fn with_framebuffer(info: framebuffer_info) -> Self {
        let byte_len = info.pitch.saturating_mul(info.height);
        // SAFETY: The caller transfers unique ownership of the complete
        // pitch-by-height framebuffer aperture to this console.
        let canvas = unsafe { canvas::new(info.address as *mut u8, byte_len, info) };
        let scale = (info.width.min(info.height) / 320).clamp(1, 3);
        let cell_width = 6 * scale;
        let cell_height = 8 * scale;
        let horizontal_margin = (2 * cell_width).min(info.width / 4);
        let top = (4 * cell_height).min(info.height / 3);
        let bottom_margin = cell_height.min(info.height.saturating_sub(top));
        let width = info.width.saturating_sub(2 * horizontal_margin);
        let height = info
            .height
            .saturating_sub(top)
            .saturating_sub(bottom_margin);
        Self {
            backend: screen_backend::framebuffer(framebuffer_terminal {
                canvas,
                left: horizontal_margin,
                top,
                width,
                height,
                scale,
            }),
            column: 0,
            row: 0,
            columns: (width / cell_width).max(1),
            rows: (height / cell_height).max(1),
        }
    }

    /// Create a console over the identity-mapped VGA text aperture.
    #[must_use]
    pub const fn with_vga_text() -> Self {
        Self {
            backend: screen_backend::vga_text,
            column: 0,
            row: 0,
            columns: 80,
            rows: 25,
        }
    }

    pub fn initialize(&mut self) {
        if let screen_backend::framebuffer(terminal) = &mut self.backend {
            terminal.canvas.clear(background);
            let title_scale = terminal.scale.max(1);
            terminal.canvas.draw_text(
                "ANOMALOUS // BOOTSTRAP SHELL",
                terminal.left,
                terminal.scale * 8,
                title_scale,
                ink,
            );
            let depth = terminal.scale * 3;
            terminal.canvas.fill_rounded_rect(
                terminal.left.saturating_sub(depth),
                terminal.top.saturating_sub(depth),
                terminal.width,
                terminal.height,
                terminal.scale * 4,
                highlight,
            );
            terminal.canvas.fill_rounded_rect(
                terminal.left + depth,
                terminal.top + depth,
                terminal.width,
                terminal.height,
                terminal.scale * 4,
                shadow,
            );
        }
        self.clear();
    }

    pub fn clear(&mut self) {
        self.column = 0;
        self.row = 0;
        match &mut self.backend {
            screen_backend::framebuffer(terminal) => terminal.canvas.fill_rect(
                terminal.left,
                terminal.top,
                terminal.width,
                terminal.height,
                terminal_background,
            ),
            screen_backend::vga_text => {
                for cell in 0..(80 * 25) {
                    // SAFETY: VGA text memory contains exactly 80x25 u16 cells.
                    unsafe { ptr::write_volatile((0xb8000 as *mut u16).add(cell), 0x0720) };
                }
            }
        }
    }

    pub fn write_input(&mut self, byte: u8) {
        if (b' '..=b'~').contains(&byte) {
            self.write_byte(byte);
        }
    }

    pub fn backspace(&mut self) {
        if self.column > 0 {
            self.column -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.column = self.columns.saturating_sub(1);
        } else {
            return;
        }
        self.clear_cell(self.column, self.row);
    }

    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.newline();
                return;
            }
            b'\r' => {
                self.column = 0;
                return;
            }
            b' '..=b'~' => {}
            _ => return,
        }

        self.draw_cell(self.column, self.row, byte);
        self.column += 1;
        if self.column >= self.columns {
            self.newline();
        }
    }

    fn newline(&mut self) {
        self.column = 0;
        if self.row + 1 < self.rows {
            self.row += 1;
        } else {
            self.scroll();
            self.row = self.rows.saturating_sub(1);
        }
    }

    fn draw_cell(&mut self, column: usize, row: usize, byte: u8) {
        match &mut self.backend {
            screen_backend::framebuffer(terminal) => {
                let cell_width = terminal.scale * 6;
                let cell_height = terminal.scale * 8;
                let x = terminal.left + column * cell_width;
                let y = terminal.top + row * cell_height;
                terminal
                    .canvas
                    .fill_rect(x, y, cell_width, cell_height, terminal_background);
                let character = char::from(byte);
                terminal
                    .canvas
                    .draw_character(character, x, y, terminal.scale, terminal_ink);
            }
            screen_backend::vga_text => {
                let offset = row * 80 + column;
                let cell = u16::from(byte) | (u16::from(0x0f_u8) << 8);
                // SAFETY: Row and column are bounded by the 80x25 console.
                unsafe { ptr::write_volatile((0xb8000 as *mut u16).add(offset), cell) };
            }
        }
    }

    fn clear_cell(&mut self, column: usize, row: usize) {
        match &mut self.backend {
            screen_backend::framebuffer(terminal) => {
                let cell_width = terminal.scale * 6;
                let cell_height = terminal.scale * 8;
                terminal.canvas.fill_rect(
                    terminal.left + column * cell_width,
                    terminal.top + row * cell_height,
                    cell_width,
                    cell_height,
                    terminal_background,
                );
            }
            screen_backend::vga_text => {
                let offset = row * 80 + column;
                // SAFETY: Row and column are bounded by the 80x25 console.
                unsafe { ptr::write_volatile((0xb8000 as *mut u16).add(offset), 0x0720) };
            }
        }
    }

    fn scroll(&mut self) {
        match &mut self.backend {
            screen_backend::framebuffer(terminal) => {
                let cell_height = terminal.scale * 8;
                terminal.canvas.scroll_rect_up(
                    terminal.left,
                    terminal.top,
                    terminal.width,
                    terminal.height,
                    cell_height,
                    terminal_background,
                );
            }
            screen_backend::vga_text => {
                let target = 0xb8000 as *mut u16;
                for row in 1..25 {
                    for column in 0..80 {
                        // SAFETY: Both addresses are distinct cells inside the
                        // VGA aperture; top-to-bottom copying handles overlap.
                        let cell = unsafe { ptr::read_volatile(target.add(row * 80 + column)) };
                        unsafe { ptr::write_volatile(target.add((row - 1) * 80 + column), cell) };
                    }
                }
                for column in 0..80 {
                    // SAFETY: The final row lies inside the VGA aperture.
                    unsafe { ptr::write_volatile(target.add(24 * 80 + column), 0x0720) };
                }
            }
        }
    }
}

impl fmt::Write for screen_console {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        for byte in value.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

fn write_vga_line(target: *mut u16, row: usize, column: usize, text: &[u8], attribute: u8) {
    for (offset, byte) in text.iter().copied().enumerate() {
        let cell = u16::from(byte) | (u16::from(attribute) << 8);
        // SAFETY: Callers use fixed text that remains inside the 80x25 aperture.
        unsafe { ptr::write_volatile(target.add(row * 80 + column + offset), cell) };
    }
}

struct canvas {
    buffer: *mut u8,
    buffer_len: usize,
    info: framebuffer_info,
}

impl canvas {
    /// Create a drawing surface over a memory-mapped framebuffer.
    ///
    /// # Safety
    ///
    /// `buffer` must remain writable and exclusively owned for `buffer_len`
    /// bytes for the lifetime of this value.
    unsafe fn new(buffer: *mut u8, buffer_len: usize, info: framebuffer_info) -> Self {
        Self {
            buffer,
            buffer_len,
            info,
        }
    }

    fn clear(&mut self, color: color) {
        self.fill_rect(0, 0, self.info.width, self.info.height, color);
    }

    fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: color) {
        let end_x = x.saturating_add(width).min(self.info.width);
        let end_y = y.saturating_add(height).min(self.info.height);
        for pixel_y in y..end_y {
            for pixel_x in x..end_x {
                self.set_pixel(pixel_x, pixel_y, color);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_rounded_rect(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        radius: usize,
        color: color,
    ) {
        let end_x = x.saturating_add(width).min(self.info.width);
        let end_y = y.saturating_add(height).min(self.info.height);
        let radius = radius.min(width / 2).min(height / 2);
        let radius_squared = radius.saturating_mul(radius);

        for pixel_y in y..end_y {
            for pixel_x in x..end_x {
                let local_x = pixel_x - x;
                let local_y = pixel_y - y;
                let corner_x = if local_x < radius {
                    radius - local_x
                } else if local_x >= width.saturating_sub(radius) {
                    local_x - width.saturating_sub(radius - 1)
                } else {
                    0
                };
                let corner_y = if local_y < radius {
                    radius - local_y
                } else if local_y >= height.saturating_sub(radius) {
                    local_y - height.saturating_sub(radius - 1)
                } else {
                    0
                };

                if corner_x == 0
                    || corner_y == 0
                    || corner_x.saturating_mul(corner_x) + corner_y.saturating_mul(corner_y)
                        <= radius_squared
                {
                    self.set_pixel(pixel_x, pixel_y, color);
                }
            }
        }
    }

    fn draw_text_centered(&mut self, text: &str, y: usize, scale: usize, color: color) {
        let width = text_width(text, scale);
        let x = self.info.width.saturating_sub(width) / 2;
        self.draw_text(text, x, y, scale, color);
    }

    fn draw_text(&mut self, text: &str, mut x: usize, y: usize, scale: usize, color: color) {
        for character in text.chars() {
            self.draw_character(character, x, y, scale, color);
            x = x.saturating_add(6 * scale);
        }
    }

    fn draw_character(&mut self, character: char, x: usize, y: usize, scale: usize, color: color) {
        let rows = glyph(character);
        for (row_index, row) in rows.into_iter().enumerate() {
            for column in 0..5 {
                if row & (1 << (4 - column)) != 0 {
                    self.fill_rect(
                        x + column * scale,
                        y + row_index * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: color) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        let bytes_per_pixel = usize::from(self.info.bits_per_pixel).div_ceil(8);
        let index = y
            .saturating_mul(self.info.pitch)
            .saturating_add(x.saturating_mul(bytes_per_pixel));
        let end = index.saturating_add(bytes_per_pixel);
        if end > self.buffer_len || bytes_per_pixel > 4 {
            return;
        }

        let packed = encode_channel(color.red, self.info.red)
            | encode_channel(color.green, self.info.green)
            | encode_channel(color.blue, self.info.blue);
        for offset in 0..bytes_per_pixel {
            let byte = (packed >> (offset * 8)) as u8;
            // SAFETY: `index..end` was checked against the unique framebuffer
            // aperture supplied to `canvas::new`. Volatile stores preserve MMIO
            // writes until the VM subsystem establishes explicit cache policy.
            unsafe { ptr::write_volatile(self.buffer.add(index + offset), byte) };
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn scroll_rect_up(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        amount: usize,
        clear_color: color,
    ) {
        let end_x = x.saturating_add(width).min(self.info.width);
        let end_y = y.saturating_add(height).min(self.info.height);
        let amount = amount.min(end_y.saturating_sub(y));
        let source_y = y.saturating_add(amount).min(end_y);
        let bytes_per_pixel = usize::from(self.info.bits_per_pixel).div_ceil(8);
        for pixel_y in source_y..end_y {
            for pixel_x in x..end_x {
                let source = pixel_y
                    .saturating_mul(self.info.pitch)
                    .saturating_add(pixel_x.saturating_mul(bytes_per_pixel));
                let destination = (pixel_y - amount)
                    .saturating_mul(self.info.pitch)
                    .saturating_add(pixel_x.saturating_mul(bytes_per_pixel));
                if source.saturating_add(bytes_per_pixel) > self.buffer_len
                    || destination.saturating_add(bytes_per_pixel) > self.buffer_len
                {
                    continue;
                }
                for offset in 0..bytes_per_pixel {
                    // SAFETY: Both byte ranges were checked against the unique
                    // framebuffer. Forward copying is valid for an upward move.
                    let byte = unsafe { ptr::read_volatile(self.buffer.add(source + offset)) };
                    unsafe { ptr::write_volatile(self.buffer.add(destination + offset), byte) };
                }
            }
        }
        self.fill_rect(
            x,
            end_y.saturating_sub(amount),
            width,
            amount.min(height),
            clear_color,
        );
    }
}

fn encode_channel(value: u8, channel: color_channel) -> u32 {
    let bits = channel.size.min(16);
    if bits == 0 || channel.position >= 32 {
        return 0;
    }
    let maximum = (1u32 << bits) - 1;
    let scaled = (u32::from(value) * maximum + 127) / 255;
    scaled.checked_shl(u32::from(channel.position)).unwrap_or(0)
}

fn text_width(text: &str, scale: usize) -> usize {
    text.chars()
        .count()
        .saturating_mul(6 * scale)
        .saturating_sub(scale)
}

fn glyph(character: char) -> [u8; 7] {
    match character {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        'a' => [
            0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111,
        ],
        'b' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110,
        ],
        'c' => [
            0b00000, 0b00000, 0b01111, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'd' => [
            0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111,
        ],
        'e' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01111,
        ],
        'f' => [
            0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000,
        ],
        'g' => [
            0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110,
        ],
        'h' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001,
        ],
        'i' => [
            0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'j' => [
            0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'k' => [
            0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010,
        ],
        'l' => [
            0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'm' => [
            0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101,
        ],
        'n' => [
            0b00000, 0b00000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001,
        ],
        'o' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'p' => [
            0b00000, 0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000,
        ],
        'q' => [
            0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001,
        ],
        'r' => [
            0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000,
        ],
        's' => [
            0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110,
        ],
        't' => [
            0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110,
        ],
        'u' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101,
        ],
        'v' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'w' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010,
        ],
        'x' => [
            0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001,
        ],
        'y' => [
            0b00000, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110,
        ],
        'z' => [
            0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
        '"' => [
            0b01010, 0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '#' => [
            0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0b00000,
        ],
        '$' => [
            0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100,
        ],
        '%' => [
            0b11001, 0b11010, 0b00100, 0b01000, 0b10110, 0b00110, 0b00000,
        ],
        '&' => [
            0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101,
        ],
        '\'' => [
            0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '*' => [
            0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        ':' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ],
        ';' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b00100, 0b01000,
        ],
        '<' => [
            0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
        ],
        '=' => [
            0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
        ],
        '>' => [
            0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
        ],
        '?' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
        '@' => [
            0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01111,
        ],
        '[' => [
            0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
        ],
        '\\' => [
            0b10000, 0b01000, 0b01000, 0b00100, 0b00010, 0b00010, 0b00001,
        ],
        ']' => [
            0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
        ],
        '^' => [
            0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        '`' => [
            0b01000, 0b00100, 0b00010, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '{' => [
            0b00010, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00010,
        ],
        '|' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        '}' => [
            0b01000, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01000,
        ],
        '~' => [
            0b00000, 0b00000, 0b01001, 0b10110, 0b00000, 0b00000, 0b00000,
        ],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_framebuffer() -> framebuffer_info {
        framebuffer_info {
            address: 0x1234_5000,
            pitch: 16,
            width: 4,
            height: 3,
            bits_per_pixel: 24,
            red: color_channel {
                position: 16,
                size: 8,
            },
            green: color_channel {
                position: 8,
                size: 8,
            },
            blue: color_channel {
                position: 0,
                size: 8,
            },
        }
    }

    #[test]
    fn interrupt_failure_surface_withdrawal_is_single_consumption() {
        let failure_registration = interrupt_failure_surface::empty();
        let expected = test_framebuffer();

        // SAFETY: This test owns the local cell and performs no concurrent
        // access to it.
        unsafe { failure_registration.register(Some(expected)) };
        // SAFETY: The local cell remains exclusively test-owned.
        let withdrawn = unsafe { failure_registration.withdraw() }.expect("registered surface");
        assert_eq!(withdrawn.address, expected.address);
        assert_eq!(withdrawn.pitch, expected.pitch);
        assert_eq!(withdrawn.width, expected.width);
        assert_eq!(withdrawn.height, expected.height);

        // SAFETY: The local cell remains exclusively test-owned. A consuming
        // withdrawal must leave no retained metadata for any later path.
        assert!(unsafe { failure_registration.withdraw() }.is_none());
        // SAFETY: The local cell remains exclusively test-owned.
        assert!(unsafe { failure_registration.retained() }.is_none());
    }
}
