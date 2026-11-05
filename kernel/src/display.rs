#![cfg_attr(not(test), allow(dead_code))]

//! Bounded packed-pixel presentation beneath a future user-space compositor.
//!
//! This module owns the validated boot framebuffer aperture. It accepts only
//! complete RGB8 patches that have already been copied into kernel storage;
//! no user pointer, physical address, or caller-selected stride crosses this
//! boundary. Preparation performs every fallible check without writing. The
//! resulting single-use commit writes exactly the accepted rectangle.

use anomalous_abi::rite_display_patch_v1;
use core::ptr::{self, NonNull};

use crate::boot_omen::{color_channel, framebuffer_info};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum display_error {
    invalid_framebuffer,
    invalid_patch,
    out_of_bounds,
}

/// Fixed proof geometry for one complete canonical RGB8 desktop frame.
///
/// These values are an internal feature contract, not a public display ABI.
pub(crate) const rgb8_frame_width: usize = 1024;
pub(crate) const rgb8_frame_height: usize = 768;
pub(crate) const rgb8_frame_row_bytes: usize = rgb8_frame_width * 3;
pub(crate) const rgb8_frame_byte_length: usize = rgb8_frame_row_bytes * rgb8_frame_height;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum frame_stage_error {
    invalid_storage,
    invalid_row,
    incomplete,
    completion_already_issued,
}

/// Exclusive, move-only owner of one bounded kernel RGB8 staging frame.
///
/// The caller supplies and continues to own the backing allocation. While the
/// stage is live, its unique borrow prevents any other safe reader or writer
/// from observing or mutating a partially admitted frame. Rows are accepted in
/// strict top-to-bottom order so completion has one unambiguous meaning.
#[must_use = "a live frame stage must be completed or securely cleared"]
pub(crate) struct rgb8_frame_stage<'storage> {
    storage: &'storage mut [u8],
    next_row: usize,
    completion_issued: bool,
}

impl<'storage> rgb8_frame_stage<'storage> {
    /// Borrow exactly one zeroed canonical-frame allocation.
    pub(crate) fn try_new(storage: &'storage mut [u8]) -> Result<Self, frame_stage_error> {
        if storage.len() != rgb8_frame_byte_length || storage.iter().any(|byte| *byte != 0) {
            return Err(frame_stage_error::invalid_storage);
        }

        Ok(Self {
            storage,
            next_row: 0,
            completion_issued: false,
        })
    }

    /// Admit exactly the next complete RGB8 row.
    ///
    /// Every condition is validated before the first copy. A rejected row
    /// therefore changes neither storage nor completion state.
    pub(crate) fn stage_row(&mut self, y: usize, pixels: &[u8]) -> Result<(), frame_stage_error> {
        if self.next_row >= rgb8_frame_height
            || y >= rgb8_frame_height
            || y != self.next_row
            || pixels.len() != rgb8_frame_row_bytes
        {
            return Err(frame_stage_error::invalid_row);
        }

        let start = y * rgb8_frame_row_bytes;
        let end = start + rgb8_frame_row_bytes;
        self.storage[start..end].copy_from_slice(pixels);
        self.next_row += 1;
        Ok(())
    }

    #[must_use]
    pub(crate) const fn rows_staged(&self) -> usize {
        self.next_row
    }

    #[must_use]
    pub(crate) const fn is_complete(&self) -> bool {
        self.next_row == rgb8_frame_height
    }

    /// Produce unforgeable, borrow-scoped evidence of complete admission.
    pub(crate) fn completion(&mut self) -> Result<complete_rgb8_frame<'_>, frame_stage_error> {
        if !self.is_complete() {
            return Err(frame_stage_error::incomplete);
        }
        if self.completion_issued {
            return Err(frame_stage_error::completion_already_issued);
        }
        self.completion_issued = true;

        Ok(complete_rgb8_frame {
            pixels: self.storage,
        })
    }

    /// Zero the complete allocation, verify it, and end stage ownership.
    ///
    /// Consuming `self` prevents a completed generation from being published
    /// or reused after teardown. The backing allocation becomes available to
    /// its external lease owner only after this method returns.
    #[must_use]
    pub(crate) fn clear_and_verify(self) -> bool {
        clear_and_verify_bytes(self.storage)
    }

    /// Consume one completed generation and create a cleared next generation.
    /// The caller binds its own monotonic frame sequence to each returned owner.
    #[cfg(any(test, feature = "workspace_dev", feature = "native_application_test"))]
    pub(crate) fn recycle(mut self) -> Result<Self, frame_stage_error> {
        if !self.is_complete() || !self.completion_issued {
            return Err(frame_stage_error::incomplete);
        }
        if !clear_and_verify_bytes(self.storage) {
            return Err(frame_stage_error::invalid_storage);
        }
        self.next_row = 0;
        self.completion_issued = false;
        Ok(self)
    }
}

/// Clear and read back the exact owned RAM span. Word accesses reduce the
/// interrupt-excluded work for a full stage without skipping any byte; the
/// unaligned prefix and trailing bytes retain byte access.
fn clear_and_verify_bytes(storage: &mut [u8]) -> bool {
    // SAFETY: All bit patterns are valid usize values. align_to_mut supplies
    // disjoint slices covering only this exclusive live allocation, with the
    // middle slice correctly aligned for every volatile machine-word access.
    let (prefix, words, suffix) = unsafe { storage.align_to_mut::<usize>() };
    for byte in prefix.iter_mut().chain(suffix.iter_mut()) {
        // SAFETY: This byte is inside one exclusively borrowed edge slice.
        unsafe { ptr::write_volatile(byte, 0) };
    }
    for word in words.iter_mut() {
        // SAFETY: The aligned middle slice exclusively owns this complete word.
        unsafe { ptr::write_volatile(word, 0) };
    }
    prefix.iter().chain(suffix.iter()).all(|byte| {
        // SAFETY: The cleared edge slices remain exclusively borrowed and live.
        unsafe { ptr::read_volatile(byte) == 0 }
    }) && words.iter().all(|word| {
        // SAFETY: The cleared aligned middle slice remains live and exclusive.
        unsafe { ptr::read_volatile(word) == 0 }
    })
}

/// Borrow-scoped proof that every canonical frame row was admitted once.
///
/// Its field is private, so only `rgb8_frame_stage::completion` can construct
/// it. It intentionally exposes neither the pixel slice nor an address.
#[must_use = "completion evidence is required to preflight publication"]
pub(crate) struct complete_rgb8_frame<'stage> {
    pixels: &'stage [u8],
}

/// Unique kernel owner of one validated packed-pixel framebuffer aperture.
///
/// The owner deliberately implements neither `Copy` nor `Clone` and exposes no
/// framebuffer address, pitch, channel masks, or raw mutable storage.
pub(crate) struct framebuffer_presenter {
    aperture: NonNull<u8>,
    aperture_len: usize,
    pitch: usize,
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
    aligned_word_pixels: bool,
    red: color_channel,
    green: color_channel,
    blue: color_channel,
}

struct validated_framebuffer {
    aperture: NonNull<u8>,
    aperture_len: usize,
    pitch: usize,
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
    red: color_channel,
    green: color_channel,
    blue: color_channel,
}

impl framebuffer_presenter {
    /// Take unique ownership of a previously validated boot framebuffer.
    ///
    /// This repeats the geometry and channel checks so synthetic or stale
    /// metadata cannot turn a later patch commit into an out-of-aperture write.
    ///
    /// # Safety
    ///
    /// `info.address..info.address + info.pitch * info.height` must remain a
    /// live, initialized, uniquely writable framebuffer aperture for the
    /// returned presenter's lifetime. No other writer may retain ownership.
    pub(crate) unsafe fn try_new(info: framebuffer_info) -> Result<Self, display_error> {
        let validated = validate_framebuffer(info)?;

        Ok(Self {
            aperture: validated.aperture,
            aperture_len: validated.aperture_len,
            pitch: validated.pitch,
            width: validated.width,
            height: validated.height,
            bytes_per_pixel: validated.bytes_per_pixel,
            aligned_word_pixels: validated.bytes_per_pixel == 4
                && validated
                    .aperture
                    .as_ptr()
                    .addr()
                    .is_multiple_of(align_of::<u32>())
                && validated.pitch.is_multiple_of(align_of::<u32>()),
            red: validated.red,
            green: validated.green,
            blue: validated.blue,
        })
    }

    #[must_use]
    pub(crate) const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub(crate) const fn height(&self) -> usize {
        self.height
    }

    /// Seed the complete aperture with a deterministic proof-only canary.
    ///
    /// The display vessel uses this before entering ring 3 so it can inspect
    /// every byte after each rejected call, rather than inferring rejection
    /// atomicity from status and counters alone.
    #[cfg(all(feature = "display_vessel_test", not(test)))]
    pub(crate) fn initialize_proof_canary(&mut self) {
        for index in 0..self.aperture_len {
            // SAFETY: Construction validated the complete aperture, and the
            // unique presenter remains its only software writer.
            unsafe {
                ptr::write_volatile(self.aperture.as_ptr().add(index), proof_canary_byte(index))
            };
        }
    }

    /// Inspect every aperture byte against the deterministic proof canary.
    #[cfg(all(feature = "display_vessel_test", not(test)))]
    #[must_use]
    pub(crate) fn proof_canary_is_intact(&self) -> bool {
        (0..self.aperture_len).all(|index| {
            // SAFETY: The presenter's live unique aperture covers `index`.
            (unsafe { ptr::read_volatile(self.aperture.as_ptr().add(index)) })
                == proof_canary_byte(index)
        })
    }

    /// Validate one copied patch completely without touching the framebuffer.
    ///
    /// `pixels` must be the exact copied suffix following `patch` in the
    /// admitted Rite body. The returned owner keeps the unique presenter borrow
    /// and can only be consumed by one infallible commit.
    pub(crate) fn prepare_patch<'presenter, 'pixels>(
        &'presenter mut self,
        patch: &rite_display_patch_v1,
        pixels: &'pixels [u8],
    ) -> Result<prepared_framebuffer_patch<'presenter, 'pixels>, display_error> {
        if !patch.has_supported_body()
            || usize::try_from(patch.pixel_byte_length()).ok() != Some(pixels.len())
        {
            return Err(display_error::invalid_patch);
        }

        let x = usize::try_from(patch.x()).map_err(|_| display_error::invalid_patch)?;
        let y = usize::try_from(patch.y()).map_err(|_| display_error::invalid_patch)?;
        let width = usize::try_from(patch.width()).map_err(|_| display_error::invalid_patch)?;
        let height = usize::try_from(patch.height()).map_err(|_| display_error::invalid_patch)?;
        self.prepare_rgb8_region(x, y, width, height, pixels)
    }

    /// Preflight one complete immutable kernel-owned RGB8 region. Unlike the
    /// per-Rite wire prefix, a retained stage can exceed one copied packet.
    /// The caller supplies its already completed stage; no user mapping is
    /// borrowed and every size/extent is checked before the first pixel write.
    pub(crate) fn prepare_rgb8_region<'presenter, 'pixels>(
        &'presenter mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        pixels: &'pixels [u8],
    ) -> Result<prepared_framebuffer_patch<'presenter, 'pixels>, display_error> {
        if width == 0 || height == 0 {
            return Err(display_error::invalid_patch);
        }
        let row_bytes = width.checked_mul(3).ok_or(display_error::invalid_patch)?;
        let end_x = x.checked_add(width).ok_or(display_error::out_of_bounds)?;
        let end_y = y.checked_add(height).ok_or(display_error::out_of_bounds)?;
        if end_x > self.width || end_y > self.height {
            return Err(display_error::out_of_bounds);
        }

        let source_len = row_bytes
            .checked_mul(height)
            .ok_or(display_error::invalid_patch)?;
        let destination_last_row = end_y
            .checked_sub(1)
            .and_then(|row| row.checked_mul(self.pitch))
            .ok_or(display_error::out_of_bounds)?;
        let destination_row_end = end_x
            .checked_mul(self.bytes_per_pixel)
            .and_then(|span| destination_last_row.checked_add(span))
            .ok_or(display_error::out_of_bounds)?;
        if source_len != pixels.len() || destination_row_end > self.aperture_len {
            return Err(display_error::out_of_bounds);
        }

        Ok(prepared_framebuffer_patch {
            presenter: self,
            pixels,
            x,
            y,
            width,
            height,
            row_bytes,
        })
    }

    /// Preflight one complete canonical RGB8 frame without writing a pixel.
    ///
    /// Completion evidence can only originate from a fully admitted stage.
    /// The existing prepared-patch owner performs the eventual conversion and
    /// commit; constructing it directly here avoids pretending that a complete
    /// frame fits in the deliberately bounded per-Rite patch payload.
    pub(crate) fn prepare_complete_rgb8_frame<'presenter, 'frame>(
        &'presenter mut self,
        frame: complete_rgb8_frame<'frame>,
    ) -> Result<prepared_framebuffer_patch<'presenter, 'frame>, display_error> {
        if self.width != rgb8_frame_width || self.height != rgb8_frame_height {
            return Err(display_error::invalid_framebuffer);
        }

        let destination_last_row = (rgb8_frame_height - 1)
            .checked_mul(self.pitch)
            .ok_or(display_error::invalid_framebuffer)?;
        let destination_row_end = rgb8_frame_width
            .checked_mul(self.bytes_per_pixel)
            .and_then(|span| destination_last_row.checked_add(span))
            .ok_or(display_error::invalid_framebuffer)?;
        if frame.pixels.len() != rgb8_frame_byte_length || destination_row_end > self.aperture_len {
            return Err(display_error::invalid_framebuffer);
        }

        Ok(prepared_framebuffer_patch {
            presenter: self,
            pixels: frame.pixels,
            x: 0,
            y: 0,
            width: rgb8_frame_width,
            height: rgb8_frame_height,
            row_bytes: rgb8_frame_row_bytes,
        })
    }

    fn write_pixel(&mut self, x: usize, y: usize, red: u8, green: u8, blue: u8) {
        let index = y * self.pitch + x * self.bytes_per_pixel;
        let packed = encode_channel(red, self.red)
            | encode_channel(green, self.green)
            | encode_channel(blue, self.blue);
        if self.aligned_word_pixels {
            // SAFETY: Construction established four-byte pixels, aligned base
            // and pitch. Preflight bounds this complete pixel and the unique
            // presenter borrow excludes aliases. Little endian preserves the
            // byte conversion for every admitted channel layout, not just BGR.
            unsafe {
                ptr::write_volatile(
                    self.aperture.as_ptr().add(index).cast::<u32>(),
                    packed.to_le(),
                )
            };
            return;
        }
        for offset in 0..self.bytes_per_pixel {
            let byte = (packed >> (offset * 8)) as u8;
            // SAFETY: Construction validated the full pitch-by-height aperture.
            // Patch preparation proved this exact pixel range lies inside that
            // aperture, and the single-use mutable owner excludes all aliases.
            unsafe { ptr::write_volatile(self.aperture.as_ptr().add(index + offset), byte) };
        }
    }
}

/// Fully checked single-use framebuffer mutation.
#[must_use = "a prepared framebuffer patch must be committed or discarded"]
pub(crate) struct prepared_framebuffer_patch<'presenter, 'pixels> {
    presenter: &'presenter mut framebuffer_presenter,
    pixels: &'pixels [u8],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    row_bytes: usize,
}

impl prepared_framebuffer_patch<'_, '_> {
    /// Commit the already validated RGB8 rectangle.
    ///
    /// No checks that can reject caller data remain, so a successful Rite
    /// reply may be constructed immediately after this method returns.
    pub(crate) fn commit(mut self) {
        self.write();
    }

    fn write(&mut self) {
        debug_assert_eq!(self.pixels.len(), self.row_bytes * self.height);
        for (row_index, row) in self.pixels.chunks_exact(self.row_bytes).enumerate() {
            for (column_index, rgb) in row.chunks_exact(3).enumerate() {
                debug_assert!(column_index < self.width);
                self.presenter.write_pixel(
                    self.x + column_index,
                    self.y + row_index,
                    rgb[0],
                    rgb[1],
                    rgb[2],
                );
            }
        }
    }

    /// Copy and compare through the same borrow-scoped source and aperture
    /// owners. This verifies software readback, not hardware scanout timing.
    #[cfg(any(test, feature = "workspace_dev", feature = "native_application_test"))]
    pub(crate) fn commit_and_verify(mut self) -> bool {
        self.write();
        self.matches_pixels()
    }

    #[cfg(any(test, feature = "workspace_dev", feature = "native_application_test"))]
    pub(crate) fn matches_pixels(&self) -> bool {
        for (row_index, row) in self.pixels.chunks_exact(self.row_bytes).enumerate() {
            for (column_index, rgb) in row.chunks_exact(3).enumerate() {
                let index = (self.y + row_index) * self.presenter.pitch
                    + (self.x + column_index) * self.presenter.bytes_per_pixel;
                let packed = encode_channel(rgb[0], self.presenter.red)
                    | encode_channel(rgb[1], self.presenter.green)
                    | encode_channel(rgb[2], self.presenter.blue);
                if self.presenter.aligned_word_pixels {
                    // SAFETY: The same construction and preflight invariants
                    // as write_pixel prove this aligned complete pixel. Read
                    // every bit, including unused channel/padding bits.
                    let actual = unsafe {
                        ptr::read_volatile(
                            self.presenter.aperture.as_ptr().add(index).cast::<u32>(),
                        )
                    };
                    if actual != packed.to_le() {
                        return false;
                    }
                    continue;
                }
                for offset in 0..self.presenter.bytes_per_pixel {
                    // SAFETY: Preflight validated this exact aperture span;
                    // this owner retains its sole presenter borrow throughout.
                    let actual = unsafe {
                        ptr::read_volatile(self.presenter.aperture.as_ptr().add(index + offset))
                    };
                    if actual != (packed >> (offset * 8)) as u8 {
                        return false;
                    }
                }
            }
        }
        true
    }
}

fn validate_framebuffer(info: framebuffer_info) -> Result<validated_framebuffer, display_error> {
    if !(24..=32).contains(&info.bits_per_pixel)
        || info.width == 0
        || info.height == 0
        || info.address == 0
        || !channel_is_valid(info.red, info.bits_per_pixel)
        || !channel_is_valid(info.green, info.bits_per_pixel)
        || !channel_is_valid(info.blue, info.bits_per_pixel)
        || channels_overlap(info.red, info.green)
        || channels_overlap(info.red, info.blue)
        || channels_overlap(info.green, info.blue)
    {
        return Err(display_error::invalid_framebuffer);
    }

    let bytes_per_pixel = usize::from(info.bits_per_pixel).div_ceil(8);
    let minimum_pitch = info
        .width
        .checked_mul(bytes_per_pixel)
        .ok_or(display_error::invalid_framebuffer)?;
    let aperture_len = info
        .pitch
        .checked_mul(info.height)
        .ok_or(display_error::invalid_framebuffer)?;
    let aperture_len_u64 =
        u64::try_from(aperture_len).map_err(|_| display_error::invalid_framebuffer)?;
    let address = usize::try_from(info.address).map_err(|_| display_error::invalid_framebuffer)?;
    if info.pitch < minimum_pitch
        || aperture_len > isize::MAX as usize
        || address.checked_add(aperture_len).is_none()
        || info.address.checked_add(aperture_len_u64).is_none()
    {
        return Err(display_error::invalid_framebuffer);
    }
    let aperture = NonNull::new(address as *mut u8).ok_or(display_error::invalid_framebuffer)?;

    Ok(validated_framebuffer {
        aperture,
        aperture_len,
        pitch: info.pitch,
        width: info.width,
        height: info.height,
        bytes_per_pixel,
        red: info.red,
        green: info.green,
        blue: info.blue,
    })
}

/// Initialize ADR 0026's deterministic pre-presenter framebuffer baseline.
///
/// Validation completes before the first write. The canary covers the complete
/// `pitch * height` aperture, including every scanline's non-visible padding,
/// so later rejection checks do not infer atomicity from visible pixels alone.
///
/// # Safety
///
/// `info` must name a live, initialized, uniquely writable framebuffer for its
/// complete pitch-by-height extent. No presenter, console, failure path, or
/// device writer may access the aperture concurrently.
#[cfg(any(test, all(feature = "supervised_renderer_test", not(test))))]
pub(crate) unsafe fn initialize_supervised_renderer_baseline(
    info: framebuffer_info,
) -> Result<(), display_error> {
    let validated = validate_framebuffer(info)?;
    for index in 0..validated.aperture_len {
        // SAFETY: The caller grants this operation unique access to the whole
        // validated aperture, and `index` is bounded by that exact extent.
        unsafe {
            ptr::write_volatile(
                validated.aperture.as_ptr().add(index),
                proof_canary_byte(index),
            )
        };
    }
    Ok(())
}

/// Verify ADR 0026's deterministic pre-presenter framebuffer baseline.
///
/// The complete metadata is revalidated before the first read, and every byte
/// in `pitch * height`, including padding, participates in the result.
///
/// # Safety
///
/// `info` must name the same live, initialized framebuffer aperture previously
/// initialized by `initialize_supervised_renderer_baseline`. No writer may
/// access it for the duration of this inspection.
#[cfg(any(test, all(feature = "supervised_renderer_test", not(test))))]
pub(crate) unsafe fn supervised_renderer_baseline_is_intact(
    info: framebuffer_info,
) -> Result<bool, display_error> {
    let validated = validate_framebuffer(info)?;
    let mut intact = true;
    for index in 0..validated.aperture_len {
        // SAFETY: The caller grants stable access to the whole validated
        // aperture, and `index` is bounded by that exact extent.
        let observed = unsafe { ptr::read_volatile(validated.aperture.as_ptr().add(index)) };
        intact &= observed == proof_canary_byte(index);
    }
    Ok(intact)
}

fn channel_is_valid(channel: color_channel, bits_per_pixel: u8) -> bool {
    channel.size != 0
        && channel.size <= 16
        && channel.position < bits_per_pixel
        && u16::from(channel.position) + u16::from(channel.size) <= u16::from(bits_per_pixel)
}

fn channels_overlap(left: color_channel, right: color_channel) -> bool {
    let left_end = u16::from(left.position) + u16::from(left.size);
    let right_end = u16::from(right.position) + u16::from(right.size);
    u16::from(left.position) < right_end && u16::from(right.position) < left_end
}

fn encode_channel(value: u8, channel: color_channel) -> u32 {
    let maximum = (1u32 << channel.size) - 1;
    let scaled = (u32::from(value) * maximum + 127) / 255;
    scaled << channel.position
}

#[cfg(any(
    test,
    all(feature = "display_vessel_test", not(test)),
    all(feature = "supervised_renderer_test", not(test))
))]
fn proof_canary_byte(index: usize) -> u8 {
    (index as u8).rotate_left(3) ^ 0xa5
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::boot_omen::memory_region;
    use crate::memory::physical::{frame_allocator, frame_error, frame_purpose};
    use anomalous_abi::{rite_display_patch_v1, rite_display_pixel_format};
    use std::vec;

    const red_low: color_channel = color_channel {
        position: 0,
        size: 8,
    };
    const green_middle: color_channel = color_channel {
        position: 8,
        size: 8,
    };
    const blue_high: color_channel = color_channel {
        position: 16,
        size: 8,
    };

    fn info_for(
        bytes: &mut [u8],
        width: usize,
        height: usize,
        pitch: usize,
        red: color_channel,
        green: color_channel,
        blue: color_channel,
    ) -> framebuffer_info {
        framebuffer_info {
            address: bytes.as_mut_ptr() as u64,
            pitch,
            width,
            height,
            bits_per_pixel: 24,
            red,
            green,
            blue,
        }
    }

    unsafe fn presenter_for(info: framebuffer_info) -> framebuffer_presenter {
        // SAFETY: Each test passes one live uniquely borrowed byte slice whose
        // exact pitch-by-height extent outlives the returned local presenter.
        unsafe { framebuffer_presenter::try_new(info) }.expect("valid test framebuffer")
    }

    fn fill_patterned_row(row: &mut [u8], y: usize) {
        assert_eq!(row.len(), rgb8_frame_row_bytes);
        for (x, rgb) in row.chunks_exact_mut(3).enumerate() {
            let x = x as u8;
            let y = y as u8;
            rgb[0] = x.wrapping_add(y);
            rgb[1] = y.rotate_left(1) ^ 0x3c;
            rgb[2] = x.rotate_right(1) ^ y ^ 0xa7;
        }
    }

    #[test]
    fn supervised_renderer_baseline_covers_every_aperture_byte() {
        const prefix: usize = 11;
        const suffix: usize = 13;
        const width: usize = 3;
        const height: usize = 4;
        const pitch: usize = width * 3 + 5;
        const guard: u8 = 0x3d;
        let aperture_len = pitch * height;
        let mut guarded = vec![guard; prefix + aperture_len + suffix];
        let info = {
            let aperture = &mut guarded[prefix..prefix + aperture_len];
            info_for(
                aperture,
                width,
                height,
                pitch,
                red_low,
                green_middle,
                blue_high,
            )
        };

        // SAFETY: The guarded vector owns the exact live aperture described by
        // `info`, and no other framebuffer owner exists in this test.
        unsafe { initialize_supervised_renderer_baseline(info) }.expect("valid baseline");
        assert!(guarded[..prefix].iter().all(|byte| *byte == guard));
        assert!(
            guarded[prefix + aperture_len..]
                .iter()
                .all(|byte| *byte == guard)
        );
        for index in 0..aperture_len {
            assert_eq!(guarded[prefix + index], proof_canary_byte(index));
        }
        // SAFETY: The same aperture remains live and has no concurrent writer.
        assert_eq!(
            unsafe { supervised_renderer_baseline_is_intact(info) },
            Ok(true)
        );

        // Mutate every visible and padding byte independently. Each mutation
        // must be observed, proving the verifier does not sample or stop at
        // `width * bytes_per_pixel` within a pitched row.
        for index in 0..aperture_len {
            guarded[prefix + index] ^= 0xff;
            // SAFETY: The same aperture remains live and stable during each
            // complete inspection.
            assert_eq!(
                unsafe { supervised_renderer_baseline_is_intact(info) },
                Ok(false),
                "mutation at aperture byte {index} was not observed"
            );
            guarded[prefix + index] ^= 0xff;
        }
        // SAFETY: Every mutation was restored before this final inspection.
        assert_eq!(
            unsafe { supervised_renderer_baseline_is_intact(info) },
            Ok(true)
        );
    }

    #[test]
    fn supervised_renderer_baseline_rejects_before_writing() {
        let mut aperture = [0x6d_u8; 32];
        let original = aperture;
        let mut invalid = info_for(&mut aperture, 3, 2, 10, red_low, green_middle, blue_high);
        invalid.pitch = 8;

        // SAFETY: The backing array is live, but malformed pitch metadata must
        // be rejected before the helper follows it or writes any byte.
        assert_eq!(
            unsafe { initialize_supervised_renderer_baseline(invalid) },
            Err(display_error::invalid_framebuffer)
        );
        assert_eq!(aperture, original);
        // SAFETY: Verification shares the same reject-before-access boundary.
        assert_eq!(
            unsafe { supervised_renderer_baseline_is_intact(invalid) },
            Err(display_error::invalid_framebuffer)
        );
        assert_eq!(aperture, original);
    }

    #[test]
    fn retained_rgb_region_preflights_large_stages_and_preserves_borders() {
        let mut aperture = vec![0x5a; 400 * 32 * 4];
        let mut info = info_for(
            &mut aperture,
            400,
            32,
            1600,
            red_low,
            green_middle,
            blue_high,
        );
        info.bits_per_pixel = 32;
        let pixels = vec![0x23; 384 * 24 * 3];
        // SAFETY: This presenter exclusively owns the complete live host array.
        let mut presenter = unsafe { framebuffer_presenter::try_new(info) }.unwrap();
        for (x, y, width, height, bytes) in [
            (0, 0, 0, 1, pixels.as_slice()),
            (usize::MAX, 0, 384, 24, pixels.as_slice()),
            (17, 0, 384, 24, pixels.as_slice()),
            (0, 9, 384, 24, pixels.as_slice()),
            (8, 4, 384, 24, &pixels[..pixels.len() - 1]),
        ] {
            assert!(
                presenter
                    .prepare_rgb8_region(x, y, width, height, bytes)
                    .is_err()
            );
        }
        assert!(aperture.iter().all(|byte| *byte == 0x5a));
        assert!(
            presenter
                .prepare_rgb8_region(8, 4, 384, 24, &pixels)
                .unwrap()
                .commit_and_verify()
        );
        for y in 0..32 {
            for x in 0..400 {
                let start = y * 1600 + x * 4;
                if (4..28).contains(&y) && (8..392).contains(&x) {
                    assert_eq!(&aperture[start..start + 4], &[0x23, 0x23, 0x23, 0]);
                } else {
                    assert_eq!(&aperture[start..start + 4], &[0x5a; 4]);
                }
            }
        }
    }

    #[test]
    fn frame_stage_requires_exact_zeroed_heap_storage() {
        let mut short = vec![0_u8; rgb8_frame_byte_length - 1];
        let short_before = short.clone();
        assert!(matches!(
            rgb8_frame_stage::try_new(&mut short),
            Err(frame_stage_error::invalid_storage)
        ));
        assert_eq!(short, short_before);

        let mut long = vec![0_u8; rgb8_frame_byte_length + 1];
        let long_before = long.clone();
        assert!(matches!(
            rgb8_frame_stage::try_new(&mut long),
            Err(frame_stage_error::invalid_storage)
        ));
        assert_eq!(long, long_before);

        let mut nonzero = vec![0_u8; rgb8_frame_byte_length];
        nonzero[rgb8_frame_byte_length / 2] = 0x5a;
        let nonzero_before = nonzero.clone();
        assert!(matches!(
            rgb8_frame_stage::try_new(&mut nonzero),
            Err(frame_stage_error::invalid_storage)
        ));
        assert_eq!(nonzero, nonzero_before);

        let mut exact = vec![0_u8; rgb8_frame_byte_length];
        let stage = rgb8_frame_stage::try_new(&mut exact).expect("exact zeroed frame");
        assert_eq!(stage.rows_staged(), 0);
        assert!(!stage.is_complete());
        assert!(stage.clear_and_verify());
        assert!(exact.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn rejected_rows_are_stage_atomic_and_completion_is_exact() {
        const prefix: usize = 17;
        const suffix: usize = 19;
        const guard: u8 = 0xc7;
        let mut guarded = vec![guard; prefix + rgb8_frame_byte_length + suffix];
        let storage = &mut guarded[prefix..prefix + rgb8_frame_byte_length];
        storage.fill(0);
        let mut stage = rgb8_frame_stage::try_new(&mut *storage).expect("zeroed stage");
        let mut row = vec![0_u8; rgb8_frame_row_bytes];
        fill_patterned_row(&mut row, 0);
        stage.stage_row(0, &row).expect("first row");
        assert_eq!(stage.rows_staged(), 1);

        let admitted = stage.storage.to_vec();
        for (y, pixels) in [
            (0, row.as_slice()),
            (2, row.as_slice()),
            (rgb8_frame_height, row.as_slice()),
            (1, &row[..rgb8_frame_row_bytes - 1]),
        ] {
            assert_eq!(
                stage.stage_row(y, pixels),
                Err(frame_stage_error::invalid_row)
            );
            assert_eq!(stage.rows_staged(), 1);
            assert_eq!(stage.storage, admitted.as_slice());
        }
        assert_eq!(
            stage.completion().err(),
            Some(frame_stage_error::incomplete)
        );

        for y in 1..rgb8_frame_height {
            fill_patterned_row(&mut row, y);
            stage.stage_row(y, &row).expect("sequential row");
        }
        assert_eq!(stage.rows_staged(), rgb8_frame_height);
        assert!(stage.is_complete());
        assert!(stage.completion().is_ok());

        assert_eq!(
            stage.completion().err(),
            Some(frame_stage_error::completion_already_issued)
        );

        let complete = stage.storage.to_vec();
        fill_patterned_row(&mut row, rgb8_frame_height - 1);
        assert_eq!(
            stage.stage_row(rgb8_frame_height - 1, &row),
            Err(frame_stage_error::invalid_row)
        );
        assert_eq!(stage.rows_staged(), rgb8_frame_height);
        assert_eq!(stage.storage, complete.as_slice());

        assert!(stage.clear_and_verify());
        assert!(storage.iter().all(|byte| *byte == 0));
        assert!(guarded[..prefix].iter().all(|byte| *byte == guard));
        assert!(
            guarded[prefix + rgb8_frame_byte_length..]
                .iter()
                .all(|byte| *byte == guard)
        );
    }

    #[test]
    fn partial_stage_cannot_produce_publication_evidence() {
        let mut storage = vec![0_u8; rgb8_frame_byte_length];
        let mut stage = rgb8_frame_stage::try_new(&mut storage).expect("zeroed stage");
        let row = vec![0x7b_u8; rgb8_frame_row_bytes];
        stage.stage_row(0, &row).expect("first row");

        assert_eq!(
            stage.completion().err(),
            Some(frame_stage_error::incomplete)
        );
        assert_eq!(stage.rows_staged(), 1);
        assert!(
            stage.storage[..rgb8_frame_row_bytes]
                .iter()
                .all(|byte| *byte == 0x7b)
        );
        assert!(
            stage.storage[rgb8_frame_row_bytes..]
                .iter()
                .all(|byte| *byte == 0)
        );

        assert!(stage.clear_and_verify());
        assert!(storage.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn complete_frame_preflight_is_atomic_and_commit_converts_with_pitch_padding() {
        const padding: usize = 7;
        const padding_canary: u8 = 0xd3;
        let pitch = rgb8_frame_row_bytes + padding;
        let mut framebuffer = vec![padding_canary; pitch * rgb8_frame_height];
        let info = info_for(
            &mut framebuffer,
            rgb8_frame_width,
            rgb8_frame_height,
            pitch,
            blue_high,
            green_middle,
            red_low,
        );
        // SAFETY: `framebuffer` is the exact unique aperture described by
        // `info`; it remains live until the presenter is dropped below.
        let mut presenter = unsafe { presenter_for(info) };

        let mut storage = vec![0_u8; rgb8_frame_byte_length];
        let mut stage = rgb8_frame_stage::try_new(&mut storage).expect("zeroed stage");
        let mut row = vec![0_u8; rgb8_frame_row_bytes];
        for y in 0..rgb8_frame_height {
            fill_patterned_row(&mut row, y);
            stage.stage_row(y, &row).expect("sequential row");
        }

        let before_preflight = framebuffer.clone();
        let evidence = stage.completion().expect("complete frame evidence");
        let prepared = presenter
            .prepare_complete_rgb8_frame(evidence)
            .expect("exact complete-frame publication");
        assert_eq!(framebuffer, before_preflight);
        assert!(prepared.commit_and_verify());
        drop(presenter);

        for y in 0..rgb8_frame_height {
            let visible = &framebuffer[y * pitch..y * pitch + rgb8_frame_row_bytes];
            for (x, bgr) in visible.chunks_exact(3).enumerate() {
                let x = x as u8;
                let y = y as u8;
                let red = x.wrapping_add(y);
                let green = y.rotate_left(1) ^ 0x3c;
                let blue = x.rotate_right(1) ^ y ^ 0xa7;
                assert_eq!(bgr, [blue, green, red]);
            }
            assert_eq!(
                &framebuffer[y * pitch + rgb8_frame_row_bytes..(y + 1) * pitch],
                &[padding_canary; padding]
            );
        }

        assert!(stage.clear_and_verify());
        assert!(storage.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn readback_detects_changed_visible_bytes_through_the_presenter_owner() {
        let mut bytes = [0xa5; 16];
        let info = info_for(&mut bytes, 2, 2, 8, blue_high, green_middle, red_low);
        // SAFETY: This test owns the exact complete aperture throughout.
        let mut presenter = unsafe { presenter_for(info) };
        let patch = rite_display_patch_v1::try_new(0, 0, 2, 2).unwrap();
        let pixels = [255, 0, 0, 0, 255, 0, 0, 0, 255, 0x12, 0x34, 0x56];
        let mut prepared = presenter.prepare_patch(&patch, &pixels).unwrap();
        prepared.write();
        assert!(prepared.matches_pixels());
        prepared.presenter.write_pixel(1, 1, 0x12, 0x34, 0x57);
        assert!(!prepared.matches_pixels());
        drop(prepared);
        drop(presenter);
        assert_eq!(&bytes[6..8], &[0xa5; 2]);
        assert_eq!(&bytes[14..16], &[0xa5; 2]);
    }

    #[test]
    fn word_and_byte_pixels_preserve_layout_guards_and_complete_readback() {
        const width: usize = 4;
        const height: usize = 3;
        const guard: u8 = 0xa5;
        let patch = rite_display_patch_v1::try_new(1, 1, 2, 2).unwrap();
        let pixels = [
            0x12, 0x34, 0x56, 0xff, 0x00, 0x80, 0x00, 0xff, 0x01, 0xab, 0xcd, 0xef,
        ];
        let layouts = [
            [red_low, green_middle, blue_high],
            [blue_high, green_middle, red_low],
            [
                color_channel {
                    position: 13,
                    size: 11,
                },
                color_channel {
                    position: 5,
                    size: 8,
                },
                color_channel {
                    position: 0,
                    size: 5,
                },
            ],
        ];
        let mut word_cases = 0;
        let mut byte_cases = 0;
        for bits in [24, 25, 32] {
            let bytes_per_pixel = usize::from(bits).div_ceil(8);
            for prefix in 0..4 {
                for padding in 0..5 {
                    for [red, green, blue] in layouts {
                        let pitch = width * bytes_per_pixel + padding;
                        let aperture_len = pitch * height;
                        let mut guarded = vec![guard; prefix + aperture_len + 7];
                        let mut expected = guarded.clone();
                        let mut info = info_for(
                            &mut guarded[prefix..prefix + aperture_len],
                            width,
                            height,
                            pitch,
                            red,
                            green,
                            blue,
                        );
                        info.bits_per_pixel = bits;
                        // SAFETY: This test owns the exact live guarded aperture
                        // until its only presenter is dropped below.
                        let mut presenter = unsafe { presenter_for(info) };
                        if presenter.aligned_word_pixels {
                            word_cases += 1;
                        } else {
                            byte_cases += 1;
                        }
                        for (pixel, rgb) in pixels.chunks_exact(3).enumerate() {
                            // Independent reference arithmetic uses a wider
                            // integer and constructs the expected byte layout.
                            let mut packed = 0u64;
                            for (value, channel) in rgb.iter().zip([red, green, blue]) {
                                packed |= ((u64::from(*value) * ((1 << channel.size) - 1) + 127)
                                    / 255)
                                    << channel.position;
                            }
                            let start = prefix
                                + (pixel / 2 + 1) * pitch
                                + (pixel % 2 + 1) * bytes_per_pixel;
                            expected[start..start + bytes_per_pixel]
                                .copy_from_slice(&packed.to_le_bytes()[..bytes_per_pixel]);
                        }
                        let before = guarded.clone();
                        let mut prepared = presenter.prepare_patch(&patch, &pixels).unwrap();
                        assert_eq!(guarded, before, "preflight must not write");
                        prepared.write();
                        assert!(prepared.matches_pixels());
                        // Corrupt every byte, including unused channel bits,
                        // through the still-exclusive presenter owner.
                        for pixel in 0..4 {
                            let start = (pixel / 2 + 1) * pitch + (pixel % 2 + 1) * bytes_per_pixel;
                            for offset in 0..bytes_per_pixel {
                                // SAFETY: These are exact visible patch bytes
                                // inside the presenter's validated aperture.
                                let pointer = unsafe {
                                    prepared.presenter.aperture.as_ptr().add(start + offset)
                                };
                                let previous = unsafe { ptr::read_volatile(pointer) };
                                unsafe { ptr::write_volatile(pointer, previous ^ 0x80) };
                                assert!(!prepared.matches_pixels());
                                unsafe { ptr::write_volatile(pointer, previous) };
                            }
                        }
                        assert!(prepared.matches_pixels());
                        drop(prepared);
                        drop(presenter);
                        assert_eq!(guarded, expected, "exact pixels and all canary bytes");
                    }
                }
            }
        }
        assert!(word_cases > 0 && byte_cases > 0);
    }

    #[test]
    fn verified_clear_covers_unaligned_edges_and_short_spans_only() {
        let word = size_of::<usize>();
        for prefix in 0..word {
            for length in 0..=word * 3 + 1 {
                let mut guarded = vec![0xa5; prefix + length + word];
                assert!(clear_and_verify_bytes(
                    &mut guarded[prefix..prefix + length]
                ));
                assert!(guarded[..prefix].iter().all(|byte| *byte == 0xa5));
                assert!(
                    guarded[prefix..prefix + length]
                        .iter()
                        .all(|byte| *byte == 0)
                );
                assert!(guarded[prefix + length..].iter().all(|byte| *byte == 0xa5));
            }
        }
    }

    #[test]
    fn recycled_frame_clears_content_and_requires_fresh_complete_admission() {
        let mut storage = vec![0; rgb8_frame_byte_length];
        let mut stage = rgb8_frame_stage::try_new(&mut storage).unwrap();
        let row = [0x6e; rgb8_frame_row_bytes];
        for y in 0..rgb8_frame_height {
            stage.stage_row(y, &row).unwrap();
        }
        drop(stage.completion().unwrap());
        let mut next = stage.recycle().unwrap();
        assert_eq!(next.rows_staged(), 0);
        assert!(!next.is_complete());
        assert!(next.storage.iter().all(|byte| *byte == 0));
        assert!(matches!(
            next.completion(),
            Err(frame_stage_error::incomplete)
        ));
        assert_eq!(next.stage_row(1, &row), Err(frame_stage_error::invalid_row));
        assert_eq!(next.stage_row(0, &row), Ok(()));
        assert!(next.clear_and_verify());
    }

    #[test]
    fn postpublication_clear_failure_keeps_frame_and_quarantines_stage_lease() {
        const frame_size: usize = 4096;
        const frames_per_ledger_byte: usize = 4;
        const pixel: [u8; 3] = [0x12, 0x34, 0x56];
        let stage_frames = rgb8_frame_byte_length / frame_size;
        assert_eq!(rgb8_frame_byte_length % frame_size, 0);

        let represented_frames = stage_frames.div_ceil(frames_per_ledger_byte);
        let mut ledger = vec![0_u8; represented_frames];
        let usable = memory_region {
            base: 0,
            length: (represented_frames * frames_per_ledger_byte * frame_size) as u64,
            kind: 1,
            attributes: 1,
        };
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[usable], &[]).expect("test allocator");
        // SAFETY: This host test owns the isolated logical ledger and models
        // successful initialization without exposing a physical alias.
        let mut lease =
            unsafe { allocator.allocate_with(stage_frames, 1, frame_purpose::data, |_| true) }
                .expect("stage lease");
        let retained_accounting = allocator.accounting();
        let retained_range = lease.range().expect("active stage lease");

        let mut storage = vec![0_u8; rgb8_frame_byte_length];
        let mut stage = rgb8_frame_stage::try_new(&mut storage).expect("zeroed stage");
        let row = vec![pixel[0], pixel[1], pixel[2]].repeat(rgb8_frame_width);
        for y in 0..rgb8_frame_height {
            stage.stage_row(y, &row).expect("sequential row");
        }

        let mut framebuffer = vec![0xa5_u8; rgb8_frame_byte_length];
        let info = info_for(
            &mut framebuffer,
            rgb8_frame_width,
            rgb8_frame_height,
            rgb8_frame_row_bytes,
            red_low,
            green_middle,
            blue_high,
        );
        // SAFETY: `framebuffer` is the exact unique aperture described by
        // `info` and remains live for the complete prepared publication.
        let mut presenter = unsafe { presenter_for(info) };
        let completion = stage.completion().expect("complete frame evidence");
        let prepared_frame = presenter
            .prepare_complete_rgb8_frame(completion)
            .expect("complete publication preflight");
        let prepared_release = allocator
            .prepare_release(&mut lease)
            .expect("stage release preflight");
        assert_eq!(prepared_release.range(), retained_range);

        prepared_frame.commit();
        drop(presenter);
        let published = framebuffer.clone();
        let mut stage = Some(stage);
        // SAFETY: The deliberate `false` result models a failed complete
        // physical clear/readback. It authorizes neither metadata retirement
        // nor reuse, and the callback retains no physical range.
        assert_eq!(
            unsafe {
                prepared_release.clear_and_release_with(|range| {
                    assert_eq!(range, retained_range);
                    drop(stage.take());
                    false
                })
            },
            Err(frame_error::clearing_failed)
        );

        assert_eq!(framebuffer, published);
        assert!(framebuffer.chunks_exact(3).all(|rgb| rgb == pixel));
        assert_eq!(allocator.accounting(), retained_accounting);
        assert_eq!(lease.range(), Some(retained_range));
        assert!(storage.iter().any(|byte| *byte != 0));
    }

    #[test]
    fn complete_frame_requires_exact_presenter_geometry_before_writing() {
        let mut storage = vec![0_u8; rgb8_frame_byte_length];
        let mut stage = rgb8_frame_stage::try_new(&mut storage).expect("zeroed stage");
        let row = vec![0x42_u8; rgb8_frame_row_bytes];
        for y in 0..rgb8_frame_height {
            stage.stage_row(y, &row).expect("sequential row");
        }

        let width = rgb8_frame_width + 1;
        let pitch = width * 3;
        let mut framebuffer = vec![0x6d_u8; pitch * rgb8_frame_height];
        let original = framebuffer.clone();
        let info = info_for(
            &mut framebuffer,
            width,
            rgb8_frame_height,
            pitch,
            red_low,
            green_middle,
            blue_high,
        );
        // SAFETY: `framebuffer` is the exact unique aperture described by
        // `info`; rejected preflight performs no mutation.
        let mut presenter = unsafe { presenter_for(info) };
        assert!(matches!(
            presenter.prepare_complete_rgb8_frame(stage.completion().expect("complete")),
            Err(display_error::invalid_framebuffer)
        ));
        assert_eq!(framebuffer, original);
        drop(presenter);

        assert!(stage.clear_and_verify());
    }

    #[test]
    fn preparation_is_failure_atomic_and_commit_mutates_only_the_rectangle() {
        const prefix: usize = 7;
        const suffix: usize = 9;
        const width: usize = 4;
        const height: usize = 3;
        const pitch: usize = 16;
        let mut guarded = [0xa5_u8; prefix + pitch * height + suffix];
        let aperture = &mut guarded[prefix..prefix + pitch * height];
        let info = info_for(
            aperture,
            width,
            height,
            pitch,
            red_low,
            green_middle,
            blue_high,
        );
        // SAFETY: `aperture` is the unique live extent described by `info`.
        let mut presenter = unsafe { presenter_for(info) };
        assert_eq!(presenter.width(), width);
        assert_eq!(presenter.height(), height);

        let patch = rite_display_patch_v1::try_new(1, 1, 2, 2).expect("valid patch");
        let pixels = [
            0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xa0, 0xb0, 0xc0,
        ];
        let before_prepare = guarded;
        let prepared = presenter
            .prepare_patch(&patch, &pixels)
            .expect("bounded patch");
        assert_eq!(guarded, before_prepare);
        prepared.commit();

        assert_eq!(&guarded[..prefix], &[0xa5; prefix]);
        assert_eq!(&guarded[prefix + pitch * height..], &[0xa5; suffix]);
        for row in 0..height {
            let row_start = prefix + row * pitch;
            assert_eq!(
                &guarded[row_start + width * 3..row_start + pitch],
                &[0xa5; 4]
            );
        }
        assert_eq!(
            &guarded[prefix + pitch + 3..prefix + pitch + 9],
            &pixels[..6]
        );
        assert_eq!(
            &guarded[prefix + pitch * 2 + 3..prefix + pitch * 2 + 9],
            &pixels[6..]
        );
        assert_eq!(&guarded[prefix..prefix + 12], &[0xa5; 12]);
        assert_eq!(
            &guarded[prefix + pitch * 2..prefix + pitch * 2 + 3],
            &[0xa5; 3]
        );
        assert_eq!(
            &guarded[prefix + pitch * 2 + 9..prefix + pitch * 2 + 12],
            &[0xa5; 3]
        );
    }

    #[test]
    fn rgb8_is_converted_through_rgb_and_bgr_channel_masks() {
        let patch = rite_display_patch_v1::try_new(0, 0, 1, 1).expect("single pixel");
        let pixels = [0x12, 0x34, 0x56];

        let mut rgb = [0_u8; 3];
        let rgb_info = info_for(&mut rgb, 1, 1, 3, red_low, green_middle, blue_high);
        // SAFETY: `rgb` is the exact unique framebuffer extent.
        let mut rgb_presenter = unsafe { presenter_for(rgb_info) };
        rgb_presenter
            .prepare_patch(&patch, &pixels)
            .expect("RGB patch")
            .commit();
        assert_eq!(rgb, pixels);

        let mut bgr = [0_u8; 3];
        let bgr_info = info_for(&mut bgr, 1, 1, 3, blue_high, green_middle, red_low);
        // SAFETY: `bgr` is the exact unique framebuffer extent.
        let mut bgr_presenter = unsafe { presenter_for(bgr_info) };
        bgr_presenter
            .prepare_patch(&patch, &pixels)
            .expect("BGR patch")
            .commit();
        assert_eq!(bgr, [0x56, 0x34, 0x12]);
    }

    #[test]
    fn every_rejected_patch_leaves_the_complete_aperture_unchanged() {
        let mut aperture = [0x6d_u8; 4 * 3 * 3];
        let info = info_for(&mut aperture, 4, 3, 12, red_low, green_middle, blue_high);
        // SAFETY: `aperture` is the exact unique framebuffer extent.
        let mut presenter = unsafe { presenter_for(info) };
        let original = aperture;

        let valid = rite_display_patch_v1::try_new(0, 0, 1, 1).expect("valid patch");
        assert!(matches!(
            presenter.prepare_patch(&valid, &[1, 2]),
            Err(display_error::invalid_patch)
        ));
        assert_eq!(aperture, original);

        let right = rite_display_patch_v1::try_new(4, 0, 1, 1).expect("wire-valid patch");
        assert!(matches!(
            presenter.prepare_patch(&right, &[1, 2, 3]),
            Err(display_error::out_of_bounds)
        ));
        assert_eq!(aperture, original);

        let bottom = rite_display_patch_v1::try_new(0, 3, 1, 1).expect("wire-valid patch");
        assert!(matches!(
            presenter.prepare_patch(&bottom, &[1, 2, 3]),
            Err(display_error::out_of_bounds)
        ));
        assert_eq!(aperture, original);

        let across_right = rite_display_patch_v1::try_new(3, 0, 2, 1).expect("wire-valid patch");
        assert!(matches!(
            presenter.prepare_patch(&across_right, &[1, 2, 3, 4, 5, 6]),
            Err(display_error::out_of_bounds)
        ));
        assert_eq!(aperture, original);

        let across_bottom = rite_display_patch_v1::try_new(0, 2, 1, 2).expect("wire-valid patch");
        assert!(matches!(
            presenter.prepare_patch(&across_bottom, &[1, 2, 3, 4, 5, 6]),
            Err(display_error::out_of_bounds)
        ));
        assert_eq!(aperture, original);

        let crossed = rite_display_patch_v1::from_raw_parts(
            40,
            1,
            0,
            0,
            0,
            1,
            1,
            3,
            rite_display_pixel_format::from_raw(2),
            0,
            3,
            0,
        );
        assert!(matches!(
            presenter.prepare_patch(&crossed, &[1, 2, 3]),
            Err(display_error::invalid_patch)
        ));
        assert_eq!(aperture, original);

        presenter
            .prepare_patch(&valid, &[1, 2, 3])
            .expect("valid edge patch")
            .commit();
        assert_eq!(&aperture[..3], &[1, 2, 3]);
        assert!(aperture[3..].iter().all(|byte| *byte == 0x6d));

        let bottom_right =
            rite_display_patch_v1::try_new(3, 2, 1, 1).expect("exact bottom-right patch");
        presenter
            .prepare_patch(&bottom_right, &[4, 5, 6])
            .expect("bottom-right edge is in bounds")
            .commit();
        assert_eq!(&aperture[33..36], &[4, 5, 6]);
    }

    #[test]
    fn presenter_rejects_invalid_pitch_extent_and_channel_layouts() {
        let mut aperture = [0_u8; 24];
        let valid = info_for(&mut aperture, 2, 2, 6, red_low, green_middle, blue_high);

        let mut invalid = valid;
        invalid.pitch = 5;
        // SAFETY: Invalid metadata is rejected before an owner is constructed.
        assert!(unsafe { framebuffer_presenter::try_new(invalid) }.is_err());

        invalid = valid;
        invalid.height = usize::MAX;
        // SAFETY: Invalid metadata is rejected before an owner is constructed.
        assert!(unsafe { framebuffer_presenter::try_new(invalid) }.is_err());

        invalid = valid;
        invalid.red = green_middle;
        // SAFETY: Overlapping channels are rejected before construction.
        assert!(unsafe { framebuffer_presenter::try_new(invalid) }.is_err());

        invalid = valid;
        invalid.blue = color_channel {
            position: 23,
            size: 2,
        };
        // SAFETY: Out-of-format channels are rejected before construction.
        assert!(unsafe { framebuffer_presenter::try_new(invalid) }.is_err());

        invalid = valid;
        invalid.bits_per_pixel = 16;
        // SAFETY: Unsupported depths are rejected before construction.
        assert!(unsafe { framebuffer_presenter::try_new(invalid) }.is_err());
    }
}
