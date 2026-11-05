#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Experimental, allocation-free wire types shared by Anomalous components.
//!
//! This crate deliberately contains data layout only. Transport, authorization,
//! and policy live above it. The ABI is not stable while Anomalous is pre-alpha.

use core::mem::size_of;

pub mod application;
mod physical_input;
pub use physical_input::{rite_physical_input_v1, rite_physical_input_v1_size};

/// Magic bytes at the start of each semantic invocation envelope.
pub const invocation_magic: [u8; 4] = *b"ANOM";

/// Current experimental control-plane schema major version.
pub const abi_major: u16 = 0;

/// Current experimental control-plane schema minor version.
pub const abi_minor: u16 = 1;

/// A transport-independent 128-bit identifier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct id_128 {
    pub bytes: [u8; 16],
}

impl id_128 {
    pub const nil: Self = Self { bytes: [0; 16] };

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self { bytes }
    }
}

/// The semantic intent carried by an [`invocation_header`].
///
/// A transparent integer wrapper keeps unknown wire values representable and
/// avoids the invalid-discriminant hazard of decoding bytes directly into a
/// Rust enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct invocation_kind(u16);

impl invocation_kind {
    /// Discover the objects and operations visible to a principal.
    pub const discover: Self = Self(1);
    /// Read state without mutating it.
    pub const inspect: Self = Self(2);
    /// Preview a mutation and its expected consequences.
    pub const preview: Self = Self(3);
    /// Apply an authorized mutation.
    pub const apply: Self = Self(4);
    /// Request cancellation of an in-flight action.
    pub const cancel: Self = Self(5);
    /// Request a compensating action for a completed mutation.
    pub const compensate: Self = Self(6);

    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_known(self) -> bool {
        matches!(self.0, 1..=6)
    }
}

/// Flags for semantic invocation behavior.
pub mod invocation_flags {
    /// The caller requires a dry run and no externally visible mutation.
    pub const dry_run: u32 = 1 << 0;
    /// The caller permits an interactive authorization prompt.
    pub const allow_prompt: u32 = 1 << 1;
    /// The action carries a caller-selected idempotency key.
    pub const idempotent: u32 = 1 << 2;
    /// The caller requests a compensating action when one exists.
    pub const request_compensation: u32 = 1 << 3;
}

/// Rights requested by a first-wave kernel rite.
///
/// These bits describe kernel-granted surfaces rather than ambient process
/// authority. A shell vessel and a display-manager vessel should receive
/// different subsets even when both use the same transport.
pub mod rite_rights {
    /// Write bounded UTF-8 text or structured events to the visible console.
    pub const console_write: u64 = 1 << 0;
    /// Consume bounded physical-input events from the kernel input source.
    pub const input_read: u64 = 1 << 1;
    /// Terminate the calling task with an explicit status.
    pub const task_exit: u64 = 1 << 2;
    /// Yield the current task without transferring ownership or authority.
    pub const task_yield: u64 = 1 << 3;
    /// Submit bounded display updates to a framebuffer/display vessel surface.
    pub const display_update: u64 = 1 << 4;
    /// Consume bounded semantic actions from one authorized private session route.
    pub const session_input_read: u64 = 1 << 5;
    /// Submit one fixed copied semantic-surface snapshot to an authorized endpoint.
    pub const semantic_surface_submit: u64 = 1 << 6;
    /// Poll one fixed copied semantic-surface snapshot from an authorized endpoint.
    pub const semantic_surface_read: u64 = 1 << 7;

    /// The initial text-shell vessel grant: prompt/output, keyboard input, and
    /// explicit exit. Display update is deliberately separate.
    pub const shell_minimum: u64 = console_write | input_read | task_exit;

    /// The initial display-manager vessel grant: display updates plus input.
    /// Console text remains a separate diagnostic/debugging authority.
    pub const display_manager_minimum: u64 = display_update | input_read | task_exit;

    pub const known: u64 = console_write
        | input_read
        | task_exit
        | task_yield
        | display_update
        | session_input_read
        | semantic_surface_submit
        | semantic_surface_read;
}

/// First-wave kernel operations available to supervised shell/display vessels.
///
/// A transparent integer wrapper keeps unknown operation numbers ordinary data.
/// The kernel must reject unknown operations without interpreting payload bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct rite_operation(u16);

impl rite_operation {
    /// Write bounded text or structured output to a granted console surface.
    pub const console_write: Self = Self(1);
    /// Read one bounded input delivery, returning would-block when unavailable.
    pub const input_read: Self = Self(2);
    /// Exit the current task with a small status code.
    pub const task_exit: Self = Self(3);
    /// Cooperatively yield the current task.
    pub const task_yield: Self = Self(4);
    /// Submit a bounded display update to the display substrate.
    pub const display_update: Self = Self(5);
    /// Read one bounded semantic action from an authorized private session route.
    pub const session_input_read: Self = Self(6);
    /// Submit one fixed copied semantic-surface snapshot.
    pub const semantic_surface_submit: Self = Self(7);
    /// Poll one fixed copied semantic-surface snapshot.
    pub const semantic_surface_read: Self = Self(8);

    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn required_right(self) -> Option<u64> {
        match self.0 {
            1 => Some(rite_rights::console_write),
            2 => Some(rite_rights::input_read),
            3 => Some(rite_rights::task_exit),
            4 => Some(rite_rights::task_yield),
            5 => Some(rite_rights::display_update),
            6 => Some(rite_rights::session_input_read),
            7 => Some(rite_rights::semantic_surface_submit),
            8 => Some(rite_rights::semantic_surface_read),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_known(self) -> bool {
        self.required_right().is_some()
    }
}

/// Kernel rite payload classification. The actual payload bytes remain bounded
/// by [`rite_call::payload_size`] and are interpreted only after operation,
/// rights, and schema checks succeed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct rite_payload_kind(u16);

impl rite_payload_kind {
    pub const none: Self = Self(0);
    pub const utf8_text: Self = Self(1);
    pub const structured_event: Self = Self(2);
    pub const input_request: Self = Self(3);
    pub const exit_status: Self = Self(4);
    pub const display_patch: Self = Self(5);
    pub const session_input_request: Self = Self(6);
    pub const session_input_delivery: Self = Self(7);
    pub const semantic_surface_snapshot: Self = Self(8);
    pub const semantic_surface_request: Self = Self(9);
    pub const physical_input_delivery: Self = Self(10);

    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_known(self) -> bool {
        matches!(self.0, 0..=10)
    }
}

/// Version shared by the first fixed semantic-surface request and snapshot.
pub const rite_semantic_surface_version_1: u16 = 1;

/// Exact byte length of the first fixed semantic-surface snapshot body.
pub const rite_semantic_surface_snapshot_v1_size: u32 = 528;

/// Fixed poll request for one copied semantic-surface snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct rite_semantic_surface_request_v1 {
    structure_size: u16,
    version: u16,
    flags: u32,
    reply_capacity: u32,
    reserved: u32,
}

impl rite_semantic_surface_request_v1 {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            structure_size: size_of::<Self>() as u16,
            version: rite_semantic_surface_version_1,
            flags: 0,
            reply_capacity: rite_semantic_surface_snapshot_v1_size,
            reserved: 0,
        }
    }

    /// Construct an untrusted decoded body without making it valid.
    #[must_use]
    pub const fn from_raw_parts(
        structure_size: u16,
        version: u16,
        flags: u32,
        reply_capacity: u32,
        reserved: u32,
    ) -> Self {
        Self {
            structure_size,
            version,
            flags,
            reply_capacity,
            reserved,
        }
    }

    #[must_use]
    pub const fn structure_size(&self) -> u16 {
        self.structure_size
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    #[must_use]
    pub const fn reply_capacity(&self) -> u32 {
        self.reply_capacity
    }

    #[must_use]
    pub const fn reserved(&self) -> u32 {
        self.reserved
    }

    #[must_use]
    pub const fn has_supported_body(&self) -> bool {
        self.structure_size as usize == size_of::<Self>()
            && self.version == rite_semantic_surface_version_1
            && self.flags == 0
            && self.reply_capacity == rite_semantic_surface_snapshot_v1_size
            && self.reserved == 0
    }

    /// Encode the exact version-1 request without exposing Rust object layout.
    #[must_use]
    pub fn encode_le(&self) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[0..2].copy_from_slice(&self.structure_size.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.version.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.flags.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.reply_capacity.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.reserved.to_le_bytes());
        bytes
    }
}

impl Default for rite_semantic_surface_request_v1 {
    fn default() -> Self {
        Self::new()
    }
}

/// Version shared by the first fixed semantic-session request and delivery bodies.
pub const rite_session_input_version_1: u16 = 1;

/// Fixed private text capacity of a version-1 semantic-session delivery.
pub const rite_session_input_text_capacity: usize = 256;

/// Flags accepted by [`rite_session_input_request_v1`].
pub mod rite_session_input_request_flags {
    /// Block the exact caller when its authorized session route is empty.
    pub const wait_if_empty: u32 = 1 << 0;

    pub const known: u32 = wait_if_empty;
}

/// Semantic action carried by a versioned session-input delivery.
///
/// Unknown wire values remain ordinary data and must be rejected before use.
/// The wrapper deliberately has no formatting or hashing implementation because
/// observing the action tag is a private-content disclosure.
#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct rite_session_input_action(u16);

impl rite_session_input_action {
    pub const insert_text: Self = Self(1);
    pub const erase_previous: Self = Self(2);
    pub const submit: Self = Self(3);
    pub const cancel_submission: Self = Self(4);
    pub const source_discontinuity: Self = Self(5);

    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_known(self) -> bool {
        matches!(self.0, 1..=5)
    }
}

/// Private attribution carried by a versioned session-input delivery.
///
/// Unknown wire values remain ordinary data and must be rejected before use.
/// An origin tag authenticates and authorizes nothing and deliberately has no
/// formatting or hashing implementation.
#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct rite_session_input_origin(u16);

impl rite_session_input_origin {
    pub const human_text: Self = Self(1);
    pub const machine_text: Self = Self(2);
    pub const paste: Self = Self(3);
    pub const accessibility: Self = Self(4);
    pub const remote_text: Self = Self(5);

    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_known(self) -> bool {
        matches!(self.0, 1..=5)
    }
}

/// Fixed request body for semantic-session input version 1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct rite_session_input_request_v1 {
    structure_size: u16,
    version: u16,
    flags: u32,
    reply_capacity: u32,
    reserved: u32,
}

impl rite_session_input_request_v1 {
    pub const reply_capacity_v1: u32 = size_of::<rite_session_input_delivery_v1>() as u32;

    #[must_use]
    pub const fn new(wait_if_empty: bool) -> Self {
        Self {
            structure_size: size_of::<Self>() as u16,
            version: rite_session_input_version_1,
            flags: if wait_if_empty {
                rite_session_input_request_flags::wait_if_empty
            } else {
                0
            },
            reply_capacity: Self::reply_capacity_v1,
            reserved: 0,
        }
    }

    /// Construct an untrusted decoded body without making it valid.
    #[must_use]
    pub const fn from_raw_parts(
        structure_size: u16,
        version: u16,
        flags: u32,
        reply_capacity: u32,
        reserved: u32,
    ) -> Self {
        Self {
            structure_size,
            version,
            flags,
            reply_capacity,
            reserved,
        }
    }

    #[must_use]
    pub const fn structure_size(&self) -> u16 {
        self.structure_size
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    #[must_use]
    pub const fn reply_capacity(&self) -> u32 {
        self.reply_capacity
    }

    #[must_use]
    pub const fn reserved(&self) -> u32 {
        self.reserved
    }

    #[must_use]
    pub const fn wait_if_empty(&self) -> bool {
        self.flags & rite_session_input_request_flags::wait_if_empty != 0
    }

    #[must_use]
    pub const fn has_supported_body(&self) -> bool {
        self.structure_size as usize == size_of::<Self>()
            && self.version == rite_session_input_version_1
            && self.flags & !rite_session_input_request_flags::known == 0
            && self.reply_capacity == Self::reply_capacity_v1
            && self.reserved == 0
    }
}

/// Fixed move-only semantic-session input delivery version 1.
///
/// This private owner deliberately implements none of `Clone`, `Copy`, `Debug`,
/// `Display`, or `Hash`. Content-derived accessors are named disclosure
/// boundaries; callers decide where those values may flow.
///
/// ```compile_fail
/// use anomalous_abi::rite_session_input_delivery_v1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<rite_session_input_delivery_v1>();
/// ```
///
/// ```compile_fail
/// use anomalous_abi::rite_session_input_delivery_v1;
/// fn require_copy<T: Copy>() {}
/// require_copy::<rite_session_input_delivery_v1>();
/// ```
///
/// ```compile_fail
/// use anomalous_abi::rite_session_input_delivery_v1;
/// fn require_debug<T: core::fmt::Debug>() {}
/// require_debug::<rite_session_input_delivery_v1>();
/// ```
///
/// ```compile_fail
/// use anomalous_abi::rite_session_input_delivery_v1;
/// fn require_display<T: core::fmt::Display>() {}
/// require_display::<rite_session_input_delivery_v1>();
/// ```
///
/// ```compile_fail
/// use anomalous_abi::rite_session_input_delivery_v1;
/// fn require_hash<T: core::hash::Hash>() {}
/// require_hash::<rite_session_input_delivery_v1>();
/// ```
#[repr(C)]
pub struct rite_session_input_delivery_v1 {
    structure_size: u16,
    version: u16,
    action: rite_session_input_action,
    origin: rite_session_input_origin,
    text_byte_length: u16,
    erase_scalar_count: u16,
    reserved: u32,
    delivery_sequence: u64,
    text: [u8; rite_session_input_text_capacity],
}

impl rite_session_input_delivery_v1 {
    /// Construct an untrusted decoded body without making it valid.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn from_raw_parts(
        structure_size: u16,
        version: u16,
        action: rite_session_input_action,
        origin: rite_session_input_origin,
        text_byte_length: u16,
        erase_scalar_count: u16,
        reserved: u32,
        delivery_sequence: u64,
        text: [u8; rite_session_input_text_capacity],
    ) -> Self {
        Self {
            structure_size,
            version,
            action,
            origin,
            text_byte_length,
            erase_scalar_count,
            reserved,
            delivery_sequence,
            text,
        }
    }

    #[must_use]
    pub fn try_insert_text(
        origin: rite_session_input_origin,
        delivery_sequence: u64,
        text: &[u8],
    ) -> Option<Self> {
        if text.is_empty() || text.len() > rite_session_input_text_capacity {
            return None;
        }
        let mut owned_text = [0; rite_session_input_text_capacity];
        owned_text[..text.len()].copy_from_slice(text);
        let delivery = Self::from_raw_parts(
            size_of::<Self>() as u16,
            rite_session_input_version_1,
            rite_session_input_action::insert_text,
            origin,
            text.len() as u16,
            0,
            0,
            delivery_sequence,
            owned_text,
        );
        delivery.has_supported_body().then_some(delivery)
    }

    #[must_use]
    pub fn try_erase_previous(
        origin: rite_session_input_origin,
        delivery_sequence: u64,
        erase_scalar_count: u16,
    ) -> Option<Self> {
        Self::try_empty_action(
            rite_session_input_action::erase_previous,
            origin,
            delivery_sequence,
            erase_scalar_count,
        )
    }

    #[must_use]
    pub fn try_submit(origin: rite_session_input_origin, delivery_sequence: u64) -> Option<Self> {
        Self::try_empty_action(
            rite_session_input_action::submit,
            origin,
            delivery_sequence,
            0,
        )
    }

    #[must_use]
    pub fn try_cancel_submission(
        origin: rite_session_input_origin,
        delivery_sequence: u64,
    ) -> Option<Self> {
        Self::try_empty_action(
            rite_session_input_action::cancel_submission,
            origin,
            delivery_sequence,
            0,
        )
    }

    #[must_use]
    pub fn try_source_discontinuity(
        origin: rite_session_input_origin,
        delivery_sequence: u64,
    ) -> Option<Self> {
        Self::try_empty_action(
            rite_session_input_action::source_discontinuity,
            origin,
            delivery_sequence,
            0,
        )
    }

    fn try_empty_action(
        action: rite_session_input_action,
        origin: rite_session_input_origin,
        delivery_sequence: u64,
        erase_scalar_count: u16,
    ) -> Option<Self> {
        let delivery = Self::from_raw_parts(
            size_of::<Self>() as u16,
            rite_session_input_version_1,
            action,
            origin,
            0,
            erase_scalar_count,
            0,
            delivery_sequence,
            [0; rite_session_input_text_capacity],
        );
        delivery.has_supported_body().then_some(delivery)
    }

    #[must_use]
    pub const fn structure_size(&self) -> u16 {
        self.structure_size
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Reveal the private semantic-action discriminator locally.
    #[must_use]
    pub const fn expose_action(&self) -> rite_session_input_action {
        self.action
    }

    /// Reveal the private origin-attribution discriminator locally.
    #[must_use]
    pub const fn expose_origin(&self) -> rite_session_input_origin {
        self.origin
    }

    /// Reveal the exact private text length locally.
    #[must_use]
    pub const fn expose_text_byte_length(&self) -> u16 {
        self.text_byte_length
    }

    /// Reveal the exact private erase count locally.
    #[must_use]
    pub const fn expose_erase_scalar_count(&self) -> u16 {
        self.erase_scalar_count
    }

    #[must_use]
    pub const fn reserved(&self) -> u32 {
        self.reserved
    }

    #[must_use]
    pub const fn delivery_sequence(&self) -> u64 {
        self.delivery_sequence
    }

    /// Reveal the complete fixed private text area locally for exact encoding.
    #[must_use]
    pub const fn expose_text_area(&self) -> &[u8; rite_session_input_text_capacity] {
        &self.text
    }

    /// Reveal valid inserted text locally.
    #[must_use]
    pub fn expose_text(&self) -> Option<&str> {
        if self.action != rite_session_input_action::insert_text || !self.has_supported_body() {
            return None;
        }
        core::str::from_utf8(&self.text[..usize::from(self.text_byte_length)]).ok()
    }

    #[must_use]
    pub fn has_supported_body(&self) -> bool {
        if self.structure_size as usize != size_of::<Self>()
            || self.version != rite_session_input_version_1
            || !self.action.is_known()
            || !self.origin.is_known()
            || self.reserved != 0
            || self.delivery_sequence == 0
            || (self.origin == rite_session_input_origin::paste
                && self.action != rite_session_input_action::insert_text)
        {
            return false;
        }

        let text_length = usize::from(self.text_byte_length);
        match self.action.raw() {
            value if value == rite_session_input_action::insert_text.raw() => {
                text_length != 0
                    && text_length <= rite_session_input_text_capacity
                    && self.erase_scalar_count == 0
                    && self.text[text_length..].iter().all(|byte| *byte == 0)
                    && session_input_text_is_supported(&self.text[..text_length])
            }
            value if value == rite_session_input_action::erase_previous.raw() => {
                self.text_byte_length == 0
                    && (1..=rite_session_input_text_capacity as u16)
                        .contains(&self.erase_scalar_count)
                    && self.text.iter().all(|byte| *byte == 0)
            }
            value
                if value == rite_session_input_action::submit.raw()
                    || value == rite_session_input_action::cancel_submission.raw()
                    || value == rite_session_input_action::source_discontinuity.raw() =>
            {
                self.text_byte_length == 0
                    && self.erase_scalar_count == 0
                    && self.text.iter().all(|byte| *byte == 0)
            }
            _ => false,
        }
    }

    pub fn clear(&mut self) {
        self.structure_size = 0;
        self.version = 0;
        self.action = rite_session_input_action::from_raw(0);
        self.origin = rite_session_input_origin::from_raw(0);
        self.text_byte_length = 0;
        self.erase_scalar_count = 0;
        self.reserved = 0;
        self.delivery_sequence = 0;
        self.text.fill(0);
    }

    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.structure_size == 0
            && self.version == 0
            && self.action.raw() == 0
            && self.origin.raw() == 0
            && self.text_byte_length == 0
            && self.erase_scalar_count == 0
            && self.reserved == 0
            && self.delivery_sequence == 0
            && self.text.iter().all(|byte| *byte == 0)
    }
}

impl Drop for rite_session_input_delivery_v1 {
    fn drop(&mut self) {
        self.clear();
    }
}

fn session_input_text_is_supported(bytes: &[u8]) -> bool {
    let Ok(text) = core::str::from_utf8(bytes) else {
        return false;
    };
    !text.chars().any(session_input_forbidden_scalar)
}

fn session_input_forbidden_scalar(value: char) -> bool {
    matches!(
        value,
        '\u{0000}'..='\u{0008}'
            | '\u{000b}'..='\u{001f}'
            | '\u{007f}'..='\u{009f}'
            | '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{feff}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
    )
}

/// Version of the first fixed display-patch body.
pub const rite_display_patch_version_1: u16 = 1;

/// Exact byte size of a version-1 display-patch header.
pub const rite_display_patch_header_size: u32 = size_of::<rite_display_patch_v1>() as u32;

/// Smallest complete version-1 display-patch payload: header plus one RGB8 pixel.
pub const rite_display_patch_min_payload_size: u32 = rite_display_patch_header_size + 3;

/// Largest display-patch payload admitted by the one-page first-wave Rite mailbox.
pub const rite_display_patch_max_payload_size: u32 = 4_056;

/// Largest tightly packed RGB8 pixel body carried after a version-1 header.
pub const rite_display_patch_pixel_capacity: u32 =
    rite_display_patch_max_payload_size - rite_display_patch_header_size;

/// Canonical pixel encoding carried by a [`rite_display_patch_v1`].
///
/// Unknown wire values remain ordinary data and must be rejected before use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct rite_display_pixel_format(u16);

impl rite_display_pixel_format {
    /// Tightly packed red, green, and blue bytes with no row padding.
    pub const rgb8: Self = Self(1);

    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_known(self) -> bool {
        self.0 == Self::rgb8.0
    }
}

/// Fixed header for one bounded, tightly packed RGB8 display patch.
///
/// Canonical pixel bytes immediately follow this header. This structure
/// validates only architecture-independent body relationships; the kernel
/// presenter must additionally compare the rectangle with its actual output.
/// Geometry deliberately has no ambient formatting implementation because it
/// can reveal private surface layout when this proof grows beyond fixtures.
///
/// ```compile_fail
/// use anomalous_abi::rite_display_patch_v1;
/// fn require_debug<T: core::fmt::Debug>() {}
/// require_debug::<rite_display_patch_v1>();
/// ```
#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct rite_display_patch_v1 {
    structure_size: u16,
    version: u16,
    flags: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    row_bytes: u32,
    pixel_format: rite_display_pixel_format,
    reserved: u16,
    pixel_byte_length: u32,
    reserved_2: u32,
}

impl rite_display_patch_v1 {
    /// Construct a canonical patch header when every size is representable and
    /// the complete pixel body fits the fixed first-wave mailbox capacity.
    #[must_use]
    pub const fn try_new(x: u32, y: u32, width: u32, height: u32) -> Option<Self> {
        if width == 0
            || height == 0
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
        {
            return None;
        }
        let Some(row_bytes) = width.checked_mul(3) else {
            return None;
        };
        let Some(pixel_byte_length) = row_bytes.checked_mul(height) else {
            return None;
        };
        if pixel_byte_length > rite_display_patch_pixel_capacity {
            return None;
        }
        Some(Self {
            structure_size: size_of::<Self>() as u16,
            version: rite_display_patch_version_1,
            flags: 0,
            x,
            y,
            width,
            height,
            row_bytes,
            pixel_format: rite_display_pixel_format::rgb8,
            reserved: 0,
            pixel_byte_length,
            reserved_2: 0,
        })
    }

    /// Construct an untrusted decoded header without making it valid.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn from_raw_parts(
        structure_size: u16,
        version: u16,
        flags: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        row_bytes: u32,
        pixel_format: rite_display_pixel_format,
        reserved: u16,
        pixel_byte_length: u32,
        reserved_2: u32,
    ) -> Self {
        Self {
            structure_size,
            version,
            flags,
            x,
            y,
            width,
            height,
            row_bytes,
            pixel_format,
            reserved,
            pixel_byte_length,
            reserved_2,
        }
    }

    #[must_use]
    pub const fn structure_size(&self) -> u16 {
        self.structure_size
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    #[must_use]
    pub const fn x(&self) -> u32 {
        self.x
    }

    #[must_use]
    pub const fn y(&self) -> u32 {
        self.y
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub const fn row_bytes(&self) -> u32 {
        self.row_bytes
    }

    #[must_use]
    pub const fn pixel_format(&self) -> rite_display_pixel_format {
        self.pixel_format
    }

    #[must_use]
    pub const fn reserved(&self) -> u16 {
        self.reserved
    }

    #[must_use]
    pub const fn pixel_byte_length(&self) -> u32 {
        self.pixel_byte_length
    }

    #[must_use]
    pub const fn reserved_2(&self) -> u32 {
        self.reserved_2
    }

    /// Validate the complete version-1 header and its internal size relations.
    #[must_use]
    pub const fn has_supported_body(&self) -> bool {
        if self.structure_size as usize != size_of::<Self>()
            || self.version != rite_display_patch_version_1
            || self.flags != 0
            || self.pixel_format.0 != rite_display_pixel_format::rgb8.0
            || self.reserved != 0
            || self.reserved_2 != 0
            || self.width == 0
            || self.height == 0
            || self.x.checked_add(self.width).is_none()
            || self.y.checked_add(self.height).is_none()
        {
            return false;
        }
        let Some(row_bytes) = self.width.checked_mul(3) else {
            return false;
        };
        let Some(pixel_byte_length) = row_bytes.checked_mul(self.height) else {
            return false;
        };
        self.row_bytes == row_bytes
            && self.pixel_byte_length == pixel_byte_length
            && pixel_byte_length <= rite_display_patch_pixel_capacity
    }
}

/// Fixed payload for an explicit task-exit rite.
///
/// The reserved field gives this pre-alpha payload an exact, extensible shape.
/// It must remain zero until a later ABI revision assigns it meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct rite_exit_payload {
    pub code: i32,
    pub reserved: u32,
}

impl rite_exit_payload {
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self { code, reserved: 0 }
    }

    #[must_use]
    pub const fn has_supported_body(&self) -> bool {
        self.reserved == 0
    }

    /// Encode the fixed exit body without exposing Rust object layout.
    #[must_use]
    pub fn encode_le(&self) -> [u8; 8] {
        let mut bytes = [0; 8];
        bytes[0..4].copy_from_slice(&self.code.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.reserved.to_le_bytes());
        bytes
    }
}

/// Status returned by a kernel rite boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct rite_status(u16);

impl rite_status {
    pub const ok: Self = Self(0);
    pub const would_block: Self = Self(1);
    pub const denied: Self = Self(2);
    pub const bad_operation: Self = Self(3);
    pub const bad_payload: Self = Self(4);
    pub const limit_exceeded: Self = Self(5);
    pub const interrupted: Self = Self(6);
    pub const faulted: Self = Self(7);

    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_known(self) -> bool {
        matches!(self.0, 0..=7)
    }
}

/// Fixed call prefix for the first supervised-vessel kernel boundary.
///
/// The prefix is deliberately smaller than the semantic-control invocation
/// envelope. It identifies one low-level kernel rite after a vessel has already
/// been launched with explicit sigils/rights. Variable payload bytes, when any,
/// immediately follow this header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct rite_call {
    pub header_size: u16,
    pub operation: rite_operation,
    pub payload_kind: rite_payload_kind,
    pub reserved: u16,
    /// Caller-declared rights used to narrow a request. The kernel must compare
    /// these bits with a trusted vessel grant; this field confers no authority.
    pub rights_present: u64,
    pub payload_size: u32,
    /// Caller-declared request bound. A kernel-owned per-operation cap remains
    /// authoritative even when this value is larger.
    pub payload_limit: u32,
    pub trace_id: id_128,
}

impl rite_call {
    #[must_use]
    pub const fn new(
        operation: rite_operation,
        payload_kind: rite_payload_kind,
        rights_present: u64,
        payload_size: u32,
        payload_limit: u32,
        trace_id: id_128,
    ) -> Self {
        Self {
            header_size: size_of::<Self>() as u16,
            operation,
            payload_kind,
            reserved: 0,
            rights_present,
            payload_size,
            payload_limit,
            trace_id,
        }
    }

    /// Validate the fixed caller declaration only.
    ///
    /// Success does not authorize execution. The kernel must still match the
    /// caller's exact incarnation and compare rights_present with trusted state.
    #[must_use]
    pub const fn validate_prefix(&self) -> rite_status {
        if self.header_size as usize != size_of::<Self>() || self.reserved != 0 {
            return rite_status::bad_payload;
        }
        if !self.operation.is_known() {
            return rite_status::bad_operation;
        }
        if !self.payload_kind.is_known() || self.rights_present & !rite_rights::known != 0 {
            return rite_status::bad_payload;
        }
        let Some(required) = self.operation.required_right() else {
            return rite_status::bad_operation;
        };
        if self.rights_present & required == 0 {
            return rite_status::denied;
        }
        if self.payload_size > self.payload_limit {
            return rite_status::limit_exceeded;
        }
        rite_status::ok
    }

    /// Encode the fixed call prefix without exposing Rust object layout.
    #[must_use]
    pub fn encode_le(&self) -> [u8; 40] {
        let mut bytes = [0; 40];
        bytes[0..2].copy_from_slice(&self.header_size.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.operation.raw().to_le_bytes());
        bytes[4..6].copy_from_slice(&self.payload_kind.raw().to_le_bytes());
        bytes[6..8].copy_from_slice(&self.reserved.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.rights_present.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.payload_size.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.payload_limit.to_le_bytes());
        bytes[24..40].copy_from_slice(&self.trace_id.bytes);
        bytes
    }
}

/// Fixed reply prefix for a kernel rite call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct rite_reply {
    pub header_size: u16,
    pub status: rite_status,
    pub payload_kind: rite_payload_kind,
    pub reserved: u16,
    pub payload_size: u32,
    /// Explicit wire-layout reservation. This closes the native x86-64
    /// alignment gap before the sequence field; it must remain zero on both
    /// sides of the boundary.
    pub reserved_2: u32,
    pub sequence: u64,
    pub trace_id: id_128,
}

impl rite_reply {
    #[must_use]
    pub const fn new(
        status: rite_status,
        payload_kind: rite_payload_kind,
        payload_size: u32,
        sequence: u64,
        trace_id: id_128,
    ) -> Self {
        Self {
            header_size: size_of::<Self>() as u16,
            status,
            payload_kind,
            reserved: 0,
            payload_size,
            reserved_2: 0,
            sequence,
            trace_id,
        }
    }

    #[must_use]
    pub const fn has_supported_header(&self) -> bool {
        self.header_size as usize == size_of::<Self>()
            && self.status.is_known()
            && self.payload_kind.is_known()
            && self.reserved == 0
            && self.reserved_2 == 0
    }

    /// Decode one fixed little-endian reply prefix as ordinary integer data.
    ///
    /// This does not validate the returned prefix. Unknown status and payload
    /// values remain representable and reject through `has_supported_header`.
    #[must_use]
    pub fn decode_le(bytes: &[u8; 40]) -> Self {
        let mut trace = [0; 16];
        trace.copy_from_slice(&bytes[24..40]);
        Self {
            header_size: u16::from_le_bytes([bytes[0], bytes[1]]),
            status: rite_status::from_raw(u16::from_le_bytes([bytes[2], bytes[3]])),
            payload_kind: rite_payload_kind::from_raw(u16::from_le_bytes([bytes[4], bytes[5]])),
            reserved: u16::from_le_bytes([bytes[6], bytes[7]]),
            payload_size: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            reserved_2: u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
            sequence: u64::from_le_bytes([
                bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22],
                bytes[23],
            ]),
            trace_id: id_128::from_bytes(trace),
        }
    }
}

const _: () = {
    assert!(size_of::<rite_semantic_surface_request_v1>() == 16);
    assert!(core::mem::offset_of!(rite_semantic_surface_request_v1, structure_size) == 0);
    assert!(core::mem::offset_of!(rite_semantic_surface_request_v1, version) == 2);
    assert!(core::mem::offset_of!(rite_semantic_surface_request_v1, flags) == 4);
    assert!(core::mem::offset_of!(rite_semantic_surface_request_v1, reply_capacity) == 8);
    assert!(core::mem::offset_of!(rite_semantic_surface_request_v1, reserved) == 12);

    assert!(size_of::<rite_session_input_action>() == 2);
    assert!(size_of::<rite_session_input_origin>() == 2);

    assert!(size_of::<rite_session_input_request_v1>() == 16);
    assert!(core::mem::offset_of!(rite_session_input_request_v1, structure_size) == 0);
    assert!(core::mem::offset_of!(rite_session_input_request_v1, version) == 2);
    assert!(core::mem::offset_of!(rite_session_input_request_v1, flags) == 4);
    assert!(core::mem::offset_of!(rite_session_input_request_v1, reply_capacity) == 8);
    assert!(core::mem::offset_of!(rite_session_input_request_v1, reserved) == 12);

    assert!(size_of::<rite_session_input_delivery_v1>() == 280);
    assert!(core::mem::offset_of!(rite_session_input_delivery_v1, structure_size) == 0);
    assert!(core::mem::offset_of!(rite_session_input_delivery_v1, version) == 2);
    assert!(core::mem::offset_of!(rite_session_input_delivery_v1, action) == 4);
    assert!(core::mem::offset_of!(rite_session_input_delivery_v1, origin) == 6);
    assert!(core::mem::offset_of!(rite_session_input_delivery_v1, text_byte_length) == 8);
    assert!(core::mem::offset_of!(rite_session_input_delivery_v1, erase_scalar_count) == 10);
    assert!(core::mem::offset_of!(rite_session_input_delivery_v1, reserved) == 12);
    assert!(core::mem::offset_of!(rite_session_input_delivery_v1, delivery_sequence) == 16);
    assert!(core::mem::offset_of!(rite_session_input_delivery_v1, text) == 24);

    assert!(size_of::<rite_display_pixel_format>() == 2);
    assert!(size_of::<rite_display_patch_v1>() == 40);
    assert!(core::mem::offset_of!(rite_display_patch_v1, structure_size) == 0);
    assert!(core::mem::offset_of!(rite_display_patch_v1, version) == 2);
    assert!(core::mem::offset_of!(rite_display_patch_v1, flags) == 4);
    assert!(core::mem::offset_of!(rite_display_patch_v1, x) == 8);
    assert!(core::mem::offset_of!(rite_display_patch_v1, y) == 12);
    assert!(core::mem::offset_of!(rite_display_patch_v1, width) == 16);
    assert!(core::mem::offset_of!(rite_display_patch_v1, height) == 20);
    assert!(core::mem::offset_of!(rite_display_patch_v1, row_bytes) == 24);
    assert!(core::mem::offset_of!(rite_display_patch_v1, pixel_format) == 28);
    assert!(core::mem::offset_of!(rite_display_patch_v1, reserved) == 30);
    assert!(core::mem::offset_of!(rite_display_patch_v1, pixel_byte_length) == 32);
    assert!(core::mem::offset_of!(rite_display_patch_v1, reserved_2) == 36);

    assert!(size_of::<rite_exit_payload>() == 8);
    assert!(core::mem::offset_of!(rite_exit_payload, code) == 0);
    assert!(core::mem::offset_of!(rite_exit_payload, reserved) == 4);

    assert!(size_of::<rite_call>() == 40);
    assert!(core::mem::offset_of!(rite_call, rights_present) == 8);
    assert!(core::mem::offset_of!(rite_call, payload_size) == 16);
    assert!(core::mem::offset_of!(rite_call, payload_limit) == 20);
    assert!(core::mem::offset_of!(rite_call, trace_id) == 24);

    assert!(size_of::<rite_reply>() == 40);
    assert!(core::mem::offset_of!(rite_reply, payload_size) == 8);
    assert!(core::mem::offset_of!(rite_reply, reserved_2) == 12);
    assert!(core::mem::offset_of!(rite_reply, sequence) == 16);
    assert!(core::mem::offset_of!(rite_reply, trace_id) == 24);
};

/// Fixed prefix for a typed invocation sent by a human or machine client.
///
/// Variable-length schema and payload bytes immediately follow this header.
/// Multi-byte fields use the target's native representation in milestone 0;
/// a canonical serialized encoding will be selected before this ABI stabilizes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct invocation_header {
    pub magic: [u8; 4],
    pub abi_major: u16,
    pub abi_minor: u16,
    pub header_size: u16,
    pub kind: invocation_kind,
    pub flags: u32,
    pub body_size: u32,
    pub reserved: u32,
    pub trace_id: id_128,
    pub invocation_id: id_128,
    pub principal_id: id_128,
    pub required_sigil: id_128,
}

impl invocation_header {
    /// Construct a header with explicit caller, trace, and capability identity.
    #[must_use]
    pub const fn new(
        kind: invocation_kind,
        flags: u32,
        body_size: u32,
        trace_id: id_128,
        invocation_id: id_128,
        principal_id: id_128,
        required_sigil: id_128,
    ) -> Self {
        Self {
            magic: invocation_magic,
            abi_major,
            abi_minor,
            header_size: size_of::<Self>() as u16,
            kind,
            flags,
            body_size,
            reserved: 0,
            trace_id,
            invocation_id,
            principal_id,
            required_sigil,
        }
    }

    /// Whether the envelope uses the current experimental schema and layout.
    #[must_use]
    pub fn has_supported_header(&self) -> bool {
        self.magic == invocation_magic
            && self.abi_major == abi_major
            && self.header_size as usize >= size_of::<Self>()
            && self.kind.is_known()
            && self.reserved == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invocation_header_layout_is_deliberate() {
        assert_eq!(size_of::<id_128>(), 16);
        assert_eq!(size_of::<invocation_header>(), 88);
    }

    #[test]
    fn new_invocation_identifies_the_current_schema() {
        let invocation = invocation_header::new(
            invocation_kind::preview,
            invocation_flags::dry_run,
            42,
            id_128::from_bytes([1; 16]),
            id_128::from_bytes([2; 16]),
            id_128::from_bytes([3; 16]),
            id_128::from_bytes([4; 16]),
        );

        assert!(invocation.has_supported_header());
        assert_eq!(invocation.body_size, 42);
        assert_eq!(invocation.flags, invocation_flags::dry_run);
    }

    #[test]
    fn unknown_invocation_kind_is_data_not_undefined_behavior() {
        let mut invocation = invocation_header::new(
            invocation_kind::discover,
            0,
            0,
            id_128::nil,
            id_128::nil,
            id_128::nil,
            id_128::nil,
        );
        invocation.kind = invocation_kind::from_raw(u16::MAX);

        assert_eq!(invocation.kind.raw(), u16::MAX);
        assert!(!invocation.has_supported_header());
    }

    #[test]
    fn rite_layouts_are_deliberate_and_allocation_free() {
        assert_eq!(size_of::<rite_operation>(), 2);
        assert_eq!(size_of::<rite_payload_kind>(), 2);
        assert_eq!(size_of::<rite_status>(), 2);
        assert_eq!(size_of::<rite_semantic_surface_request_v1>(), 16);
        assert_eq!(size_of::<rite_session_input_action>(), 2);
        assert_eq!(size_of::<rite_session_input_origin>(), 2);
        assert_eq!(size_of::<rite_session_input_request_v1>(), 16);
        assert_eq!(size_of::<rite_session_input_delivery_v1>(), 280);
        assert_eq!(size_of::<rite_display_pixel_format>(), 2);
        assert_eq!(size_of::<rite_display_patch_v1>(), 40);
        assert_eq!(size_of::<rite_exit_payload>(), 8);
        assert_eq!(size_of::<rite_call>(), 40);
        assert_eq!(size_of::<rite_reply>(), 40);
        assert!(core::mem::needs_drop::<rite_session_input_delivery_v1>());
    }

    #[test]
    fn display_patch_layout_offsets_and_limits_are_exact() {
        assert_eq!(rite_display_patch_header_size, 40);
        assert_eq!(rite_display_patch_min_payload_size, 43);
        assert_eq!(rite_display_patch_max_payload_size, 4_056);
        assert_eq!(rite_display_patch_pixel_capacity, 4_016);
        assert_eq!(
            core::mem::offset_of!(rite_display_patch_v1, structure_size),
            0
        );
        assert_eq!(core::mem::offset_of!(rite_display_patch_v1, version), 2);
        assert_eq!(core::mem::offset_of!(rite_display_patch_v1, flags), 4);
        assert_eq!(core::mem::offset_of!(rite_display_patch_v1, x), 8);
        assert_eq!(core::mem::offset_of!(rite_display_patch_v1, y), 12);
        assert_eq!(core::mem::offset_of!(rite_display_patch_v1, width), 16);
        assert_eq!(core::mem::offset_of!(rite_display_patch_v1, height), 20);
        assert_eq!(core::mem::offset_of!(rite_display_patch_v1, row_bytes), 24);
        assert_eq!(
            core::mem::offset_of!(rite_display_patch_v1, pixel_format),
            28
        );
        assert_eq!(core::mem::offset_of!(rite_display_patch_v1, reserved), 30);
        assert_eq!(
            core::mem::offset_of!(rite_display_patch_v1, pixel_byte_length),
            32
        );
        assert_eq!(core::mem::offset_of!(rite_display_patch_v1, reserved_2), 36);
    }

    #[test]
    fn display_pixel_format_keeps_unknown_wire_values_as_data() {
        assert_eq!(rite_display_pixel_format::rgb8.raw(), 1);
        assert!(rite_display_pixel_format::rgb8.is_known());
        for value in [0, 2, u16::MAX] {
            let format = rite_display_pixel_format::from_raw(value);
            assert_eq!(format.raw(), value);
            assert!(!format.is_known());
        }
    }

    #[test]
    fn display_patch_constructor_derives_one_canonical_shape() {
        let patch = rite_display_patch_v1::try_new(7, 11, 19, 23).expect("valid patch");
        assert!(patch.has_supported_body());
        assert_eq!(patch.structure_size(), 40);
        assert_eq!(patch.version(), rite_display_patch_version_1);
        assert_eq!(patch.flags(), 0);
        assert_eq!(patch.x(), 7);
        assert_eq!(patch.y(), 11);
        assert_eq!(patch.width(), 19);
        assert_eq!(patch.height(), 23);
        assert_eq!(patch.row_bytes(), 57);
        assert_eq!(patch.pixel_format(), rite_display_pixel_format::rgb8);
        assert_eq!(patch.reserved(), 0);
        assert_eq!(patch.pixel_byte_length(), 1_311);
        assert_eq!(patch.reserved_2(), 0);

        let edge = rite_display_patch_v1::try_new(u32::MAX - 1, u32::MAX - 1, 1, 1)
            .expect("largest nonwrapping endpoints");
        assert!(edge.has_supported_body());
        assert_eq!(edge.pixel_byte_length(), 3);

        let maximum_width =
            rite_display_patch_v1::try_new(0, 0, 1_338, 1).expect("largest width in one mailbox");
        assert_eq!(maximum_width.pixel_byte_length(), 4_014);
        let maximum_height =
            rite_display_patch_v1::try_new(0, 0, 1, 1_338).expect("largest height in one mailbox");
        assert_eq!(maximum_height.pixel_byte_length(), 4_014);
    }

    #[test]
    fn display_patch_constructor_rejects_empty_wrapped_and_oversize_shapes() {
        for shape in [
            (0, 0, 0, 1),
            (0, 0, 1, 0),
            (u32::MAX, 0, 1, 1),
            (0, u32::MAX, 1, 1),
            (0, 0, u32::MAX, 1),
            (0, 0, 1, u32::MAX),
            (0, 0, 1_339, 1),
            (0, 0, 1, 1_339),
            (0, 0, 37, 37),
        ] {
            assert!(
                rite_display_patch_v1::try_new(shape.0, shape.1, shape.2, shape.3).is_none(),
                "unexpected valid shape {shape:?}"
            );
        }
    }

    #[test]
    fn display_patch_rejects_every_noncanonical_header_field() {
        let valid = rite_display_patch_v1::try_new(3, 5, 7, 11).expect("valid patch");

        let mut mutations = [valid; 12];
        mutations[0].structure_size = 39;
        mutations[1].version = 2;
        mutations[2].flags = 1;
        mutations[3].width = 0;
        mutations[4].height = 0;
        mutations[5].row_bytes = 20;
        mutations[6].pixel_format = rite_display_pixel_format::from_raw(0);
        mutations[7].reserved = 1;
        mutations[8].pixel_byte_length = 230;
        mutations[9].reserved_2 = 1;
        mutations[10].x = u32::MAX;
        mutations[11].y = u32::MAX;
        for mutation in mutations {
            assert!(!mutation.has_supported_body());
        }

        let multiplication_overflow = rite_display_patch_v1::from_raw_parts(
            40,
            1,
            0,
            0,
            0,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            rite_display_pixel_format::rgb8,
            0,
            u32::MAX,
            0,
        );
        assert!(!multiplication_overflow.has_supported_body());

        let over_capacity = rite_display_patch_v1::from_raw_parts(
            40,
            1,
            0,
            0,
            0,
            1_339,
            1,
            4_017,
            rite_display_pixel_format::rgb8,
            0,
            4_017,
            0,
        );
        assert!(!over_capacity.has_supported_body());
    }

    #[test]
    fn session_input_numeric_values_and_outer_call_are_exact() {
        assert_eq!(rite_rights::session_input_read, 1 << 5);
        assert_eq!(rite_rights::known, 0b1111_1111);
        assert_eq!(rite_rights::shell_minimum, 0b000111);
        assert_eq!(rite_rights::display_manager_minimum, 0b010110);
        assert_eq!(rite_operation::session_input_read.raw(), 6);
        assert_eq!(
            rite_operation::session_input_read.required_right(),
            Some(rite_rights::session_input_read)
        );
        assert_eq!(rite_payload_kind::session_input_request.raw(), 6);
        assert_eq!(rite_payload_kind::session_input_delivery.raw(), 7);
        assert!(rite_payload_kind::session_input_request.is_known());
        assert!(rite_payload_kind::session_input_delivery.is_known());

        for (action, value) in [
            (rite_session_input_action::insert_text, 1),
            (rite_session_input_action::erase_previous, 2),
            (rite_session_input_action::submit, 3),
            (rite_session_input_action::cancel_submission, 4),
            (rite_session_input_action::source_discontinuity, 5),
        ] {
            assert!(action.is_known());
            assert_eq!(action.raw(), value);
        }
        for value in [0, 6, u16::MAX] {
            let unknown_action = rite_session_input_action::from_raw(value);
            assert_eq!(unknown_action.raw(), value);
            assert!(!unknown_action.is_known());
        }

        for (origin, value) in [
            (rite_session_input_origin::human_text, 1),
            (rite_session_input_origin::machine_text, 2),
            (rite_session_input_origin::paste, 3),
            (rite_session_input_origin::accessibility, 4),
            (rite_session_input_origin::remote_text, 5),
        ] {
            assert!(origin.is_known());
            assert_eq!(origin.raw(), value);
        }
        for value in [0, 6, u16::MAX] {
            let unknown_origin = rite_session_input_origin::from_raw(value);
            assert_eq!(unknown_origin.raw(), value);
            assert!(!unknown_origin.is_known());
        }

        assert_eq!(rite_session_input_request_flags::wait_if_empty, 1);
        assert_eq!(rite_session_input_request_flags::known, 1);
        let call = rite_call::new(
            rite_operation::session_input_read,
            rite_payload_kind::session_input_request,
            rite_rights::session_input_read,
            size_of::<rite_session_input_request_v1>() as u32,
            size_of::<rite_session_input_request_v1>() as u32,
            id_128::from_bytes([0x62; 16]),
        );
        assert_eq!(call.validate_prefix(), rite_status::ok);

        let reply = rite_reply::new(
            rite_status::ok,
            rite_payload_kind::session_input_delivery,
            size_of::<rite_session_input_delivery_v1>() as u32,
            7,
            call.trace_id,
        );
        assert!(reply.has_supported_header());
    }

    #[test]
    fn semantic_surface_numeric_values_and_outer_calls_are_exact() {
        assert_eq!(rite_rights::semantic_surface_submit, 1 << 6);
        assert_eq!(rite_rights::semantic_surface_read, 1 << 7);
        assert_eq!(rite_rights::shell_minimum, 0b0000_0111);
        assert_eq!(rite_rights::display_manager_minimum, 0b0001_0110);
        assert_eq!(rite_operation::semantic_surface_submit.raw(), 7);
        assert_eq!(rite_operation::semantic_surface_read.raw(), 8);
        assert_eq!(
            rite_operation::semantic_surface_submit.required_right(),
            Some(rite_rights::semantic_surface_submit)
        );
        assert_eq!(
            rite_operation::semantic_surface_read.required_right(),
            Some(rite_rights::semantic_surface_read)
        );
        assert_eq!(rite_payload_kind::semantic_surface_snapshot.raw(), 8);
        assert_eq!(rite_payload_kind::semantic_surface_request.raw(), 9);
        assert!(rite_payload_kind::semantic_surface_snapshot.is_known());
        assert!(rite_payload_kind::semantic_surface_request.is_known());

        let submit = rite_call::new(
            rite_operation::semantic_surface_submit,
            rite_payload_kind::semantic_surface_snapshot,
            rite_rights::semantic_surface_submit,
            rite_semantic_surface_snapshot_v1_size,
            rite_semantic_surface_snapshot_v1_size,
            id_128::from_bytes([0x71; 16]),
        );
        assert_eq!(submit.validate_prefix(), rite_status::ok);

        let read = rite_call::new(
            rite_operation::semantic_surface_read,
            rite_payload_kind::semantic_surface_request,
            rite_rights::semantic_surface_read,
            size_of::<rite_semantic_surface_request_v1>() as u32,
            size_of::<rite_semantic_surface_request_v1>() as u32,
            id_128::from_bytes([0x82; 16]),
        );
        assert_eq!(read.validate_prefix(), rite_status::ok);

        let reply = rite_reply::new(
            rite_status::ok,
            rite_payload_kind::semantic_surface_snapshot,
            rite_semantic_surface_snapshot_v1_size,
            3,
            read.trace_id,
        );
        assert!(reply.has_supported_header());
    }

    #[test]
    fn semantic_surface_request_accepts_only_the_exact_version_1_shape() {
        let request = rite_semantic_surface_request_v1::new();
        assert!(request.has_supported_body());
        assert_eq!(request.structure_size(), 16);
        assert_eq!(request.version(), rite_semantic_surface_version_1);
        assert_eq!(request.flags(), 0);
        assert_eq!(request.reply_capacity(), 528);
        assert_eq!(request.reserved(), 0);

        for malformed in [
            rite_semantic_surface_request_v1::from_raw_parts(15, 1, 0, 528, 0),
            rite_semantic_surface_request_v1::from_raw_parts(17, 1, 0, 528, 0),
            rite_semantic_surface_request_v1::from_raw_parts(16, 0, 0, 528, 0),
            rite_semantic_surface_request_v1::from_raw_parts(16, 2, 0, 528, 0),
            rite_semantic_surface_request_v1::from_raw_parts(16, 1, 1, 528, 0),
            rite_semantic_surface_request_v1::from_raw_parts(16, 1, u32::MAX, 528, 0),
            rite_semantic_surface_request_v1::from_raw_parts(16, 1, 0, 527, 0),
            rite_semantic_surface_request_v1::from_raw_parts(16, 1, 0, 529, 0),
            rite_semantic_surface_request_v1::from_raw_parts(16, 1, 0, 528, 1),
        ] {
            assert!(!malformed.has_supported_body());
        }
    }

    #[test]
    fn semantic_surface_native_wire_helpers_are_exact_little_endian() {
        let request = rite_semantic_surface_request_v1::from_raw_parts(
            0x0102,
            0x0304,
            0x0506_0708,
            0x1112_1314,
            0x2122_2324,
        );
        assert_eq!(
            request.encode_le(),
            [
                0x02, 0x01, 0x04, 0x03, 0x08, 0x07, 0x06, 0x05, 0x14, 0x13, 0x12, 0x11, 0x24, 0x23,
                0x22, 0x21,
            ]
        );

        let trace = id_128::from_bytes([
            0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e,
            0x3f, 0x40,
        ]);
        let call = rite_call {
            header_size: 0x0102,
            operation: rite_operation::from_raw(0x0304),
            payload_kind: rite_payload_kind::from_raw(0x0506),
            reserved: 0x0708,
            rights_present: 0x1112_1314_1516_1718,
            payload_size: 0x2122_2324,
            payload_limit: 0x2526_2728,
            trace_id: trace,
        };
        let call_bytes = call.encode_le();
        assert_eq!(&call_bytes[0..8], &[2, 1, 4, 3, 6, 5, 8, 7]);
        assert_eq!(&call_bytes[8..16], &0x1112_1314_1516_1718_u64.to_le_bytes());
        assert_eq!(&call_bytes[16..20], &0x2122_2324_u32.to_le_bytes());
        assert_eq!(&call_bytes[20..24], &0x2526_2728_u32.to_le_bytes());
        assert_eq!(&call_bytes[24..40], &trace.bytes);

        let exit = rite_exit_payload {
            code: -0x0102_0304,
            reserved: 0x1112_1314,
        };
        let exit_bytes = exit.encode_le();
        assert_eq!(&exit_bytes[0..4], &(-0x0102_0304_i32).to_le_bytes());
        assert_eq!(&exit_bytes[4..8], &0x1112_1314_u32.to_le_bytes());

        let mut reply_bytes = [0; 40];
        reply_bytes[0..2].copy_from_slice(&0x0102_u16.to_le_bytes());
        reply_bytes[2..4].copy_from_slice(&0x0304_u16.to_le_bytes());
        reply_bytes[4..6].copy_from_slice(&0x0506_u16.to_le_bytes());
        reply_bytes[6..8].copy_from_slice(&0x0708_u16.to_le_bytes());
        reply_bytes[8..12].copy_from_slice(&0x1112_1314_u32.to_le_bytes());
        reply_bytes[12..16].copy_from_slice(&0x2122_2324_u32.to_le_bytes());
        reply_bytes[16..24].copy_from_slice(&0x3132_3334_3536_3738_u64.to_le_bytes());
        reply_bytes[24..40].copy_from_slice(&trace.bytes);
        let reply = rite_reply::decode_le(&reply_bytes);
        assert_eq!(reply.header_size, 0x0102);
        assert_eq!(reply.status.raw(), 0x0304);
        assert_eq!(reply.payload_kind.raw(), 0x0506);
        assert_eq!(reply.reserved, 0x0708);
        assert_eq!(reply.payload_size, 0x1112_1314);
        assert_eq!(reply.reserved_2, 0x2122_2324);
        assert_eq!(reply.sequence, 0x3132_3334_3536_3738);
        assert_eq!(reply.trace_id, trace);
    }

    #[test]
    fn session_input_layout_offsets_are_exact() {
        assert_eq!(
            core::mem::offset_of!(rite_session_input_request_v1, structure_size),
            0
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_request_v1, version),
            2
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_request_v1, flags),
            4
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_request_v1, reply_capacity),
            8
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_request_v1, reserved),
            12
        );

        assert_eq!(
            core::mem::offset_of!(rite_session_input_delivery_v1, structure_size),
            0
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_delivery_v1, version),
            2
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_delivery_v1, action),
            4
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_delivery_v1, origin),
            6
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_delivery_v1, text_byte_length),
            8
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_delivery_v1, erase_scalar_count),
            10
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_delivery_v1, reserved),
            12
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_delivery_v1, delivery_sequence),
            16
        );
        assert_eq!(
            core::mem::offset_of!(rite_session_input_delivery_v1, text),
            24
        );
    }

    #[test]
    fn session_input_request_accepts_only_the_exact_version_1_shape() {
        let poll = rite_session_input_request_v1::new(false);
        assert!(poll.has_supported_body());
        assert_eq!(poll.structure_size(), 16);
        assert_eq!(poll.version(), rite_session_input_version_1);
        assert_eq!(poll.flags(), 0);
        assert_eq!(poll.reply_capacity(), 280);
        assert_eq!(poll.reserved(), 0);
        assert!(!poll.wait_if_empty());

        let wait = rite_session_input_request_v1::new(true);
        assert!(wait.has_supported_body());
        assert_eq!(
            wait.flags(),
            rite_session_input_request_flags::wait_if_empty
        );
        assert!(wait.wait_if_empty());

        for request in [
            rite_session_input_request_v1::from_raw_parts(15, 1, 0, 280, 0),
            rite_session_input_request_v1::from_raw_parts(17, 1, 0, 280, 0),
            rite_session_input_request_v1::from_raw_parts(16, 0, 0, 280, 0),
            rite_session_input_request_v1::from_raw_parts(16, 2, 0, 280, 0),
            rite_session_input_request_v1::from_raw_parts(16, 1, 1 << 1, 280, 0),
            rite_session_input_request_v1::from_raw_parts(16, 1, 1 << 31, 280, 0),
            rite_session_input_request_v1::from_raw_parts(16, 1, 0, 279, 0),
            rite_session_input_request_v1::from_raw_parts(16, 1, 0, 281, 0),
            rite_session_input_request_v1::from_raw_parts(16, 1, 0, 280, 1),
        ] {
            assert!(!request.has_supported_body());
        }
    }

    fn session_input_origins() -> [rite_session_input_origin; 5] {
        [
            rite_session_input_origin::human_text,
            rite_session_input_origin::machine_text,
            rite_session_input_origin::paste,
            rite_session_input_origin::accessibility,
            rite_session_input_origin::remote_text,
        ]
    }

    fn valid_insert() -> rite_session_input_delivery_v1 {
        rite_session_input_delivery_v1::try_insert_text(
            rite_session_input_origin::human_text,
            7,
            b"private text",
        )
        .expect("valid insertion")
    }

    #[test]
    fn every_valid_action_and_origin_pair_has_one_exact_shape() {
        let mut accepted = 0;
        for origin in session_input_origins() {
            let insertion = rite_session_input_delivery_v1::try_insert_text(origin, 1, b"x")
                .expect("every origin may insert text");
            assert!(insertion.has_supported_body());
            assert!(insertion.expose_action() == rite_session_input_action::insert_text);
            assert!(insertion.expose_origin() == origin);
            assert_eq!(insertion.expose_text(), Some("x"));
            accepted += 1;

            let nonpaste = origin != rite_session_input_origin::paste;
            let deliveries = [
                rite_session_input_delivery_v1::try_erase_previous(origin, 2, 1),
                rite_session_input_delivery_v1::try_submit(origin, 3),
                rite_session_input_delivery_v1::try_cancel_submission(origin, 4),
                rite_session_input_delivery_v1::try_source_discontinuity(origin, 5),
            ];
            for delivery in deliveries {
                assert_eq!(delivery.is_some(), nonpaste);
                if let Some(delivery) = delivery {
                    assert!(delivery.has_supported_body());
                    assert!(delivery.expose_origin() == origin);
                    accepted += 1;
                }
            }
        }
        assert_eq!(accepted, 21);
    }

    #[test]
    fn insert_text_is_bounded_utf8_with_an_exact_zero_tail() {
        let delivery = rite_session_input_delivery_v1::try_insert_text(
            rite_session_input_origin::paste,
            9,
            "human λ\tmachine\n".as_bytes(),
        )
        .expect("valid bounded text");
        assert!(delivery.has_supported_body());
        assert_eq!(delivery.structure_size(), 280);
        assert_eq!(delivery.version(), rite_session_input_version_1);
        assert_eq!(delivery.expose_text(), Some("human λ\tmachine\n"));
        assert_eq!(delivery.expose_text_byte_length(), 17);
        assert_eq!(delivery.expose_erase_scalar_count(), 0);
        assert_eq!(delivery.reserved(), 0);
        assert_eq!(delivery.delivery_sequence(), 9);
        assert!(
            delivery.expose_text_area()[17..]
                .iter()
                .all(|byte| *byte == 0)
        );

        let exact = [b'a'; rite_session_input_text_capacity];
        let delivery = rite_session_input_delivery_v1::try_insert_text(
            rite_session_input_origin::remote_text,
            u64::MAX,
            &exact,
        )
        .expect("exact capacity");
        assert_eq!(delivery.expose_text_byte_length(), 256);
        assert_eq!(
            delivery.expose_text().map(|text| text.as_bytes()),
            Some(exact.as_slice())
        );

        assert!(
            rite_session_input_delivery_v1::try_insert_text(
                rite_session_input_origin::human_text,
                1,
                b""
            )
            .is_none()
        );
        assert!(
            rite_session_input_delivery_v1::try_insert_text(
                rite_session_input_origin::human_text,
                1,
                &[b'x'; rite_session_input_text_capacity + 1]
            )
            .is_none()
        );
        assert!(
            rite_session_input_delivery_v1::try_insert_text(
                rite_session_input_origin::human_text,
                1,
                &[0xf0, 0x28, 0x8c, 0x28]
            )
            .is_none()
        );
        assert!(
            rite_session_input_delivery_v1::try_insert_text(
                rite_session_input_origin::human_text,
                0,
                b"x"
            )
            .is_none()
        );
        assert!(
            rite_session_input_delivery_v1::try_insert_text(
                rite_session_input_origin::from_raw(u16::MAX),
                1,
                b"x"
            )
            .is_none()
        );
    }

    fn frozen_forbidden_scalar(value: u32) -> bool {
        matches!(
            value,
            0x0000..=0x0008
                | 0x000b..=0x001f
                | 0x007f..=0x009f
                | 0x00ad
                | 0x034f
                | 0x061c
                | 0x180e
                | 0xfeff
                | 0x200b..=0x200f
                | 0x202a..=0x202e
                | 0x2060..=0x206f
        )
    }

    #[test]
    fn version_1_scalar_policy_matches_the_frozen_adr_0021_set() {
        let mut rejected = 0;
        for value in 0..=0x10ffff {
            let Some(scalar) = char::from_u32(value) else {
                continue;
            };
            assert_eq!(
                session_input_forbidden_scalar(scalar),
                frozen_forbidden_scalar(value)
            );
            if !frozen_forbidden_scalar(value) {
                continue;
            }
            let mut bytes = [0; 4];
            let text = scalar.encode_utf8(&mut bytes);
            assert!(
                rite_session_input_delivery_v1::try_insert_text(
                    rite_session_input_origin::human_text,
                    1,
                    text.as_bytes()
                )
                .is_none()
            );
            rejected += 1;
        }
        assert_eq!(rejected, 94);

        for text in ["\t", "\n", "visible text", "λ"] {
            assert!(
                rite_session_input_delivery_v1::try_insert_text(
                    rite_session_input_origin::paste,
                    1,
                    text.as_bytes()
                )
                .is_some()
            );
        }
    }

    #[test]
    fn raw_delivery_validation_rejects_every_malformed_field_class() {
        let mut mutation = valid_insert();
        mutation.structure_size = 279;
        assert!(!mutation.has_supported_body());

        let mut mutation = valid_insert();
        mutation.version = 2;
        assert!(!mutation.has_supported_body());

        let mut mutation = valid_insert();
        mutation.action = rite_session_input_action::from_raw(u16::MAX);
        assert!(!mutation.has_supported_body());

        for value in [0, 6] {
            let mut mutation = valid_insert();
            mutation.action = rite_session_input_action::from_raw(value);
            assert!(!mutation.has_supported_body());
        }

        let mut mutation = valid_insert();
        mutation.origin = rite_session_input_origin::from_raw(u16::MAX);
        assert!(!mutation.has_supported_body());

        for value in [0, 6] {
            let mut mutation = valid_insert();
            mutation.origin = rite_session_input_origin::from_raw(value);
            assert!(!mutation.has_supported_body());
        }

        let mutation = rite_session_input_delivery_v1::from_raw_parts(
            280,
            1,
            rite_session_input_action::insert_text,
            rite_session_input_origin::human_text,
            0,
            0,
            0,
            1,
            [0; rite_session_input_text_capacity],
        );
        assert!(!mutation.has_supported_body());

        let mut mutation = valid_insert();
        mutation.reserved = 1;
        assert!(!mutation.has_supported_body());

        let mut mutation = valid_insert();
        mutation.delivery_sequence = 0;
        assert!(!mutation.has_supported_body());

        let mut mutation = valid_insert();
        mutation.text_byte_length = 257;
        assert!(!mutation.has_supported_body());

        let mut mutation = valid_insert();
        mutation.erase_scalar_count = 1;
        assert!(!mutation.has_supported_body());

        let mut mutation = valid_insert();
        mutation.text[0] = 0xff;
        assert!(!mutation.has_supported_body());

        let mut mutation = valid_insert();
        mutation.text[mutation.text_byte_length as usize] = 1;
        assert!(!mutation.has_supported_body());

        let mut forbidden = [0; rite_session_input_text_capacity];
        forbidden[0] = b'\r';
        let mutation = rite_session_input_delivery_v1::from_raw_parts(
            280,
            1,
            rite_session_input_action::insert_text,
            rite_session_input_origin::human_text,
            1,
            0,
            0,
            1,
            forbidden,
        );
        assert!(!mutation.has_supported_body());

        for action in [
            rite_session_input_action::erase_previous,
            rite_session_input_action::submit,
            rite_session_input_action::cancel_submission,
            rite_session_input_action::source_discontinuity,
        ] {
            let mutation = rite_session_input_delivery_v1::from_raw_parts(
                280,
                1,
                action,
                rite_session_input_origin::paste,
                0,
                u16::from(action == rite_session_input_action::erase_previous),
                0,
                1,
                [0; rite_session_input_text_capacity],
            );
            assert!(!mutation.has_supported_body());
        }
    }

    #[test]
    fn nontext_action_shapes_are_exact() {
        for count in [1, 256] {
            let erase = rite_session_input_delivery_v1::try_erase_previous(
                rite_session_input_origin::human_text,
                1,
                count,
            )
            .expect("valid erase");
            assert!(erase.has_supported_body());
            assert_eq!(erase.expose_erase_scalar_count(), count);
            assert_eq!(erase.expose_text(), None);
        }
        for count in [0, 257, u16::MAX] {
            assert!(
                rite_session_input_delivery_v1::try_erase_previous(
                    rite_session_input_origin::human_text,
                    1,
                    count
                )
                .is_none()
            );
        }

        let mut erase = rite_session_input_delivery_v1::try_erase_previous(
            rite_session_input_origin::human_text,
            1,
            1,
        )
        .expect("valid erase");
        erase.text[0] = 1;
        assert!(!erase.has_supported_body());

        for mut delivery in [
            rite_session_input_delivery_v1::try_submit(rite_session_input_origin::human_text, 1)
                .expect("submit"),
            rite_session_input_delivery_v1::try_cancel_submission(
                rite_session_input_origin::machine_text,
                2,
            )
            .expect("cancel"),
            rite_session_input_delivery_v1::try_source_discontinuity(
                rite_session_input_origin::remote_text,
                3,
            )
            .expect("discontinuity"),
        ] {
            assert!(delivery.has_supported_body());
            assert_eq!(delivery.expose_text(), None);
            delivery.erase_scalar_count = 1;
            assert!(!delivery.has_supported_body());
        }
    }

    #[test]
    fn delivery_clear_removes_content_and_metadata() {
        let mut delivery = valid_insert();
        assert!(!delivery.is_clear());
        delivery.clear();
        assert!(delivery.is_clear());
        assert!(!delivery.has_supported_body());
        assert_eq!(delivery.expose_text(), None);
        assert!(delivery.expose_text_area().iter().all(|byte| *byte == 0));

        let mut erase = rite_session_input_delivery_v1::try_erase_previous(
            rite_session_input_origin::machine_text,
            u64::MAX,
            256,
        )
        .expect("valid erase");
        assert_eq!(erase.expose_erase_scalar_count(), 256);
        erase.clear();
        assert!(erase.is_clear());
        assert_eq!(erase.expose_erase_scalar_count(), 0);
        assert_eq!(erase.delivery_sequence(), 0);
    }

    #[test]
    fn rite_call_accepts_only_known_declared_bounded_prefixes() {
        let call = rite_call::new(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            12,
            128,
            id_128::from_bytes([7; 16]),
        );
        assert_eq!(call.validate_prefix(), rite_status::ok);

        let mut mutation = call;
        mutation.operation = rite_operation::from_raw(u16::MAX);
        assert_eq!(mutation.validate_prefix(), rite_status::bad_operation);

        mutation = call;
        mutation.rights_present = 0;
        assert_eq!(mutation.validate_prefix(), rite_status::denied);

        mutation = call;
        mutation.payload_size = 129;
        assert_eq!(mutation.validate_prefix(), rite_status::limit_exceeded);

        mutation = call;
        mutation.payload_kind = rite_payload_kind::from_raw(u16::MAX);
        assert_eq!(mutation.validate_prefix(), rite_status::bad_payload);

        mutation = call;
        mutation.rights_present = rite_rights::known | (1 << 63);
        assert_eq!(mutation.validate_prefix(), rite_status::bad_payload);
    }

    #[test]
    fn shell_and_display_manager_grants_are_separate() {
        assert_ne!(
            rite_rights::shell_minimum,
            rite_rights::display_manager_minimum
        );
        assert_eq!(rite_rights::shell_minimum & rite_rights::display_update, 0);
        assert_eq!(
            rite_rights::display_manager_minimum & rite_rights::console_write,
            0
        );
    }

    #[test]
    fn rite_exit_payload_has_an_exact_reserved_shape() {
        let payload = rite_exit_payload::new(-7);
        assert_eq!(payload.code, -7);
        assert!(payload.has_supported_body());

        let mut mutation = payload;
        mutation.reserved = 1;
        assert!(!mutation.has_supported_body());
    }

    #[test]
    fn rite_reply_rejects_unknown_status_and_reserved_bits() {
        let reply = rite_reply::new(
            rite_status::would_block,
            rite_payload_kind::none,
            0,
            9,
            id_128::from_bytes([8; 16]),
        );
        assert!(reply.has_supported_header());

        let mut mutation = reply;
        mutation.status = rite_status::from_raw(u16::MAX);
        assert!(!mutation.has_supported_header());

        mutation = reply;
        mutation.reserved = 1;
        assert!(!mutation.has_supported_header());

        mutation = reply;
        mutation.reserved_2 = 1;
        assert!(!mutation.has_supported_header());
    }
}
