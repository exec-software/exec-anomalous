#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! OS-neutral, bounded RGB8 content for native windows. Frames borrow immutable
//! pixels or own immutable arrays; a canvas borrows mutable storage. None owns
//! allocation, transport, display authority, or publication state. IDs are
//! process-local immutable-content identities, never grants or wire handles.

mod drawing;
mod graphics_demo;

use core::sync::atomic::{AtomicU64, Ordering};

pub use drawing::canvas;
pub use graphics_demo::{
    demo_byte_count, demo_height, demo_stride, demo_width, draw_graphics_demo, graphics_demo_label,
};

pub type rgb = [u8; 3];
pub const maximum_width: u32 = 1024;
pub const maximum_height: u32 = 768;
pub const maximum_stride: u32 = 4096;
pub const maximum_label_bytes: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum surface_error {
    invalid_dimensions,
    invalid_stride,
    invalid_length,
    invalid_label,
    identity_exhausted,
}
impl surface_error {
    pub const fn code(self) -> &'static str {
        match self {
            Self::invalid_dimensions => "invalid_dimensions",
            Self::invalid_stride => "invalid_stride",
            Self::invalid_length => "invalid_length",
            Self::invalid_label => "invalid_label",
            Self::identity_exhausted => "identity_exhausted",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct geometry {
    width: u32,
    height: u32,
    stride: u32,
}
impl geometry {
    fn checked(width: u32, height: u32, stride: u32, length: usize) -> Result<Self, surface_error> {
        if width == 0 || height == 0 || width > maximum_width || height > maximum_height {
            return Err(surface_error::invalid_dimensions);
        }
        if stride < width * 3 || stride > maximum_stride {
            return Err(surface_error::invalid_stride);
        }
        if (stride as usize).checked_mul(height as usize) != Some(length) {
            return Err(surface_error::invalid_length);
        }
        Ok(Self {
            width,
            height,
            stride,
        })
    }

    fn row_range(self, row: u32) -> Option<core::ops::Range<usize>> {
        if row >= self.height {
            return None;
        }
        let start = row as usize * self.stride as usize;
        Some(start..start + self.width as usize * 3)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct content_label {
    bytes: [u8; maximum_label_bytes],
    length: u8,
}
impl content_label {
    fn checked(label: &str) -> Result<Self, surface_error> {
        if label.is_empty()
            || label.len() > maximum_label_bytes
            || !label.bytes().all(|byte| (b' '..=b'~').contains(&byte))
        {
            return Err(surface_error::invalid_label);
        }
        let mut result = Self {
            bytes: [0; maximum_label_bytes],
            length: label.len() as u8,
        };
        result.bytes[..label.len()].copy_from_slice(label.as_bytes());
        Ok(result)
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..usize::from(self.length)])
            .expect("validated printable ASCII label")
    }
}

/// Metadata for exactly one immutable borrowed frame. The private identity is
/// minted locally and cannot be used to authorize cross-process submissions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct surface_descriptor {
    id: u64,
    geometry: geometry,
    label: content_label,
}
impl surface_descriptor {
    pub const fn id(self) -> u64 {
        self.id
    }
    pub const fn width(self) -> u32 {
        self.geometry.width
    }
    pub const fn height(self) -> u32 {
        self.geometry.height
    }
    pub const fn stride(self) -> u32 {
        self.geometry.stride
    }
    pub fn label(&self) -> &str {
        self.label.as_str()
    }
}

static next_frame_id: AtomicU64 = AtomicU64::new(1);

fn identity_successor(current: u64) -> Option<u64> {
    // Zero is a terminal exhausted sentinel. The maximum identity is issued
    // once, after which every future mint fails instead of reusing a value.
    if current == 0 {
        None
    } else {
        Some(current.checked_add(1).unwrap_or(0))
    }
}

fn mint_identity(counter: &AtomicU64) -> Result<u64, surface_error> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, identity_successor)
        .map_err(|_| surface_error::identity_exhausted)
}

/// Validated RGB8 pixels whose source cannot be changed while they are used.
/// Every row is top-to-bottom and every pixel is red, green, then blue. Padding
/// belongs to the caller and is never exposed as visible content.
///
/// ```compile_fail
/// use anomalous_window_surface::frame;
/// let mut pixels = [0u8; 3];
/// let content = frame::try_new(1, 1, 3, "Public sample", &pixels).unwrap();
/// pixels[0] = 255; // Cannot mutate the backing storage before its last use.
/// assert_eq!(content.row(0).unwrap()[0], 0);
/// ```
pub struct frame<'a> {
    descriptor: surface_descriptor,
    pixels: &'a [u8],
}
impl<'a> frame<'a> {
    pub fn try_new(
        width: u32,
        height: u32,
        stride: u32,
        label: &str,
        pixels: &'a [u8],
    ) -> Result<Self, surface_error> {
        let geometry = geometry::checked(width, height, stride, pixels.len())?;
        let label = content_label::checked(label)?;
        let id = mint_identity(&next_frame_id)?;
        Ok(Self {
            descriptor: surface_descriptor {
                id,
                geometry,
                label,
            },
            pixels,
        })
    }

    pub const fn descriptor(&self) -> surface_descriptor {
        self.descriptor
    }

    /// Visible bytes only, excluding stride padding. An out-of-range row has
    /// no readable span.
    pub fn row(&self, row: u32) -> Option<&[u8]> {
        Some(&self.pixels[self.descriptor.geometry.row_range(row)?])
    }
}

/// One immutable RGB8 frame with inline, caller-supplied storage. Moving this
/// owner preserves its content identity; borrowing it never mints a new one.
/// It performs no allocation and grants no display or transport authority.
///
/// The pixel array can be reclaimed only by consuming the owner after its last
/// borrowed frame use. Reusing those bytes in a new owner mints a fresh identity.
///
/// ```compile_fail
/// use anomalous_window_surface::owned_frame;
/// let owner = owned_frame::try_new(1, 1, 3, "Public sample", [0; 3]).unwrap();
/// let content = owner.as_frame();
/// let mut pixels = owner.into_pixels(); // Cannot move while content borrows it.
/// pixels[0] = 255;
/// assert_eq!(content.row(0).unwrap()[0], 0);
/// ```
///
/// ```compile_fail
/// use anomalous_window_surface::owned_frame;
/// let owner = owned_frame::try_new(1, 1, 3, "Public sample", [0; 3]).unwrap();
/// let content = owner.as_frame();
/// content.row(0).unwrap()[0] = 255; // Borrowed pixels remain immutable.
/// assert_eq!(content.row(0).unwrap()[0], 0);
/// ```
pub struct owned_frame<const size: usize> {
    descriptor: surface_descriptor,
    pixels: [u8; size],
}

impl<const size: usize> owned_frame<size> {
    pub fn try_new(
        width: u32,
        height: u32,
        stride: u32,
        label: &str,
        pixels: [u8; size],
    ) -> Result<Self, surface_error> {
        let descriptor = frame::try_new(width, height, stride, label, &pixels)?.descriptor();
        Ok(Self { descriptor, pixels })
    }

    pub const fn descriptor(&self) -> surface_descriptor {
        self.descriptor
    }

    pub fn as_frame(&self) -> frame<'_> {
        frame {
            descriptor: self.descriptor,
            pixels: &self.pixels,
        }
    }

    pub fn into_pixels(self) -> [u8; size] {
        self.pixels
    }
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;

/// Immutable content retaining exclusive ownership of caller-provided storage.
/// Moving this owner moves only metadata and a reference. The caller chooses
/// and retains the backing allocation; this type grants no transport authority.
///
/// ```compile_fail
/// use anomalous_window_surface::borrowed_frame;
/// let mut pixels = [0u8; 3];
/// let owner = borrowed_frame::try_new(1, 1, 3, "Public sample", &mut pixels).ok().unwrap();
/// pixels[0] = 255;
/// assert_eq!(owner.as_frame().row(0).unwrap()[0], 0);
/// ```
/// ```compile_fail
/// use anomalous_window_surface::borrowed_frame;
/// let mut pixels = [0u8; 3];
/// let owner = borrowed_frame::try_new(1, 1, 3, "Public sample", &mut pixels).ok().unwrap();
/// let view = owner.as_frame();
/// let pixels = owner.into_pixels();
/// pixels[0] = 255;
/// assert_eq!(view.row(0).unwrap()[0], 0);
/// ```
pub struct borrowed_frame<'a, const size: usize> {
    descriptor: surface_descriptor,
    pixels: &'a mut [u8; size],
}

/// A rejected constructor returns the unchanged backing buffer.
pub struct rejected_borrowed_frame<'a, const size: usize> {
    error: surface_error,
    pixels: &'a mut [u8; size],
}
impl<'a, const size: usize> rejected_borrowed_frame<'a, size> {
    pub const fn error(&self) -> surface_error {
        self.error
    }
    pub fn into_pixels(self) -> &'a mut [u8; size] {
        self.pixels
    }
}

impl<'a, const size: usize> borrowed_frame<'a, size> {
    pub fn try_new(
        width: u32,
        height: u32,
        stride: u32,
        label: &str,
        pixels: &'a mut [u8; size],
    ) -> Result<Self, rejected_borrowed_frame<'a, size>> {
        let descriptor = match frame::try_new(width, height, stride, label, pixels) {
            Ok(view) => view.descriptor(),
            Err(error) => return Err(rejected_borrowed_frame { error, pixels }),
        };
        Ok(Self { descriptor, pixels })
    }
    pub const fn descriptor(&self) -> surface_descriptor {
        self.descriptor
    }
    pub fn as_frame(&self) -> frame<'_> {
        frame {
            descriptor: self.descriptor,
            pixels: self.pixels,
        }
    }
    /// End this content identity and reclaim the exact backing buffer. A new
    /// constructor mints a fresh identity even when its bytes are unchanged.
    pub fn into_pixels(self) -> &'a mut [u8; size] {
        self.pixels
    }
}

#[cfg(test)]
mod borrowed_tests;
