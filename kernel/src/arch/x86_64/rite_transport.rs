//! Fixed, allocation-free transport for native x86-64 rite mailboxes.
//!
//! Wire values are decoded and encoded explicitly as little-endian bytes. Raw
//! mailbox access remains unsafe because numeric range checks cannot establish
//! that a physical alias is live, mapped, allocator-owned, or exclusive.

use anomalous_abi::{
    id_128, rite_call, rite_display_patch_min_payload_size, rite_display_patch_pixel_capacity,
    rite_display_patch_v1, rite_display_pixel_format, rite_exit_payload, rite_operation,
    rite_payload_kind, rite_reply, rite_semantic_surface_request_v1,
    rite_semantic_surface_snapshot_v1_size, rite_session_input_delivery_v1,
    rite_session_input_request_v1, rite_status,
};
use core::mem;
use core::ptr;
use core::sync::atomic::{Ordering, compiler_fence};

use super::paging;
use crate::{memory, rite::copy_bounded_body_bytes};

const page_size: usize = 4096;

pub(super) const call_size: usize = mem::size_of::<rite_call>();
pub(super) const reply_size: usize = mem::size_of::<rite_reply>();
pub(super) const transport_payload_limit: u32 = page_size as u32 - call_size as u32;
pub(super) const session_input_request_size: usize =
    mem::size_of::<rite_session_input_request_v1>();
pub(super) const session_input_delivery_size: usize =
    mem::size_of::<rite_session_input_delivery_v1>();
pub(super) const session_input_reply_span_size: usize = call_size + session_input_delivery_size;
pub(super) const semantic_surface_request_size: usize =
    mem::size_of::<rite_semantic_surface_request_v1>();
pub(super) const semantic_surface_snapshot_size: usize =
    rite_semantic_surface_snapshot_v1_size as usize;
pub(super) const semantic_surface_reply_span_size: usize =
    call_size + semantic_surface_snapshot_size;
pub(super) const display_patch_header_size: usize = mem::size_of::<rite_display_patch_v1>();
pub(super) const display_patch_pixel_capacity: usize =
    transport_payload_limit as usize - display_patch_header_size;

pub(super) type rite_scratch = [u8; page_size];

const _: () = {
    assert!(mem::size_of::<rite_exit_payload>() == 8);
    assert!(mem::offset_of!(rite_exit_payload, code) == 0);
    assert!(mem::offset_of!(rite_exit_payload, reserved) == 4);

    assert!(call_size == 40);
    assert!(mem::offset_of!(rite_call, header_size) == 0);
    assert!(mem::offset_of!(rite_call, operation) == 2);
    assert!(mem::offset_of!(rite_call, payload_kind) == 4);
    assert!(mem::offset_of!(rite_call, reserved) == 6);
    assert!(mem::offset_of!(rite_call, rights_present) == 8);
    assert!(mem::offset_of!(rite_call, payload_size) == 16);
    assert!(mem::offset_of!(rite_call, payload_limit) == 20);
    assert!(mem::offset_of!(rite_call, trace_id) == 24);

    assert!(reply_size == 40);
    assert!(mem::offset_of!(rite_reply, header_size) == 0);
    assert!(mem::offset_of!(rite_reply, status) == 2);
    assert!(mem::offset_of!(rite_reply, payload_kind) == 4);
    assert!(mem::offset_of!(rite_reply, reserved) == 6);
    assert!(mem::offset_of!(rite_reply, payload_size) == 8);
    assert!(mem::offset_of!(rite_reply, reserved_2) == 12);
    assert!(mem::offset_of!(rite_reply, sequence) == 16);
    assert!(mem::offset_of!(rite_reply, trace_id) == 24);

    assert!(transport_payload_limit == 4056);
    assert!(call_size + transport_payload_limit as usize == page_size);
    assert!(session_input_request_size == 16);
    assert!(session_input_delivery_size == 280);
    assert!(session_input_reply_span_size == 320);
    assert!(semantic_surface_request_size == 16);
    assert!(semantic_surface_snapshot_size == 528);
    assert!(semantic_surface_reply_span_size == 568);
    assert!(display_patch_header_size == 40);
    assert!(display_patch_pixel_capacity == 4_016);
    assert!(display_patch_pixel_capacity == rite_display_patch_pixel_capacity as usize);
};

/// Prepared bytes for one semantic-session reply.
///
/// The complete body is encoded before any mailbox mutation. This owner is
/// deliberately move-only and clears its private scratch when it is dropped.
#[must_use = "a prepared session-input reply must be staged or discarded"]
pub(super) struct prepared_session_input_reply {
    prefix: [u8; reply_size],
    body: [u8; session_input_delivery_size],
    body_present: bool,
}

/// A reply whose body and non-header prefix fields are complete while its
/// two-byte validity marker remains zero.
///
/// Dropping this value deliberately leaves the mailbox invalid. Only the
/// selected-consumer transaction may publish the saved header after its source
/// and pending-channel commits have completed.
#[must_use = "publish the header only after the selected-consumer commit"]
pub(super) struct staged_session_input_reply {
    mailbox_alias: u64,
    header: [u8; 2],
}

/// Prepared bytes for one copied semantic-surface reply.
#[must_use = "a prepared semantic-surface reply must be staged or discarded"]
pub(super) struct prepared_semantic_surface_reply {
    prefix: [u8; reply_size],
    body: [u8; semantic_surface_snapshot_size],
    body_present: bool,
}

/// A semantic-surface reply staged with its validity header still zero.
#[must_use = "publish the header only after endpoint consumption commits"]
pub(super) struct staged_semantic_surface_reply {
    mailbox_alias: u64,
    header: [u8; 2],
}

impl staged_semantic_surface_reply {
    /// Publish the two-byte reply header last.
    ///
    /// # Safety
    ///
    /// The caller must retain the exact live mailbox mapping and serialized
    /// gate interval used to stage this reply, and must already have committed
    /// the correlated endpoint consumption and Rite-channel settlement.
    pub(super) unsafe fn publish(self) {
        // SAFETY: Staging proved this exact span writable and the caller
        // retains the mapping and exclusion contract above.
        unsafe { write_mailbox_bytes_unchecked(self.mailbox_alias, 0, &self.header) };
    }
}

impl staged_session_input_reply {
    /// Publish the two-byte reply header last.
    ///
    /// # Safety
    ///
    /// The caller must retain the exact live mailbox mapping and exclusive
    /// single-BSP gate interval used to stage this reply. It must also have
    /// committed the correlated scheduler selection, endpoint consumption (or
    /// terminal acknowledgement), and pending-channel settlement.
    pub(super) unsafe fn publish(self) {
        // SAFETY: The caller retains the mapping and exclusion contract above;
        // staging already proved this exact two-byte span writable.
        unsafe { write_mailbox_bytes_unchecked(self.mailbox_alias, 0, &self.header) };
    }
}

impl Drop for prepared_session_input_reply {
    fn drop(&mut self) {
        let _ = clear_and_verify_bytes(&mut self.prefix);
        let _ = clear_and_verify_bytes(&mut self.body);
        self.body_present = false;
    }
}

impl Drop for prepared_semantic_surface_reply {
    fn drop(&mut self) {
        let _ = clear_and_verify_bytes(&mut self.prefix);
        let _ = clear_and_verify_bytes(&mut self.body);
        self.body_present = false;
    }
}

/// Copy and decode one fixed rite-call prefix from a physical mailbox alias.
///
/// # Safety
///
/// `mailbox_alias` must name a live, identity-mapped, allocator-owned page that
/// remains readable and mutation-excluded for the complete volatile copy.
pub(super) unsafe fn read_call(mailbox_alias: u64) -> Result<rite_call, &'static str> {
    let mut bytes = [0_u8; call_size];
    // SAFETY: The caller provides the complete alias lifetime, mapping, and
    // exclusivity contract required by the raw copy.
    unsafe { read_mailbox_bytes(mailbox_alias, 0, &mut bytes) }?;
    Ok(decode_call_bytes(&bytes))
}

pub(super) fn decode_call_bytes(bytes: &[u8; call_size]) -> rite_call {
    let mut trace = [0_u8; 16];
    trace.copy_from_slice(&bytes[24..40]);
    rite_call {
        header_size: u16::from_le_bytes([bytes[0], bytes[1]]),
        operation: rite_operation::from_raw(u16::from_le_bytes([bytes[2], bytes[3]])),
        payload_kind: rite_payload_kind::from_raw(u16::from_le_bytes([bytes[4], bytes[5]])),
        reserved: u16::from_le_bytes([bytes[6], bytes[7]]),
        rights_present: u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]),
        payload_size: u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
        payload_limit: u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
        trace_id: id_128::from_bytes(trace),
    }
}

/// Decode the exact version-1 semantic-session request from staged body bytes.
///
/// This function does not decide whether the outer call shape or the decoded
/// body is authorized. The architecture gate must complete ordinary Rite
/// admission first and then call `has_supported_body` on the returned value.
pub(super) fn decode_session_input_request(
    scratch: &rite_scratch,
) -> rite_session_input_request_v1 {
    rite_session_input_request_v1::from_raw_parts(
        u16::from_le_bytes([scratch[0], scratch[1]]),
        u16::from_le_bytes([scratch[2], scratch[3]]),
        u32::from_le_bytes([scratch[4], scratch[5], scratch[6], scratch[7]]),
        u32::from_le_bytes([scratch[8], scratch[9], scratch[10], scratch[11]]),
        u32::from_le_bytes([scratch[12], scratch[13], scratch[14], scratch[15]]),
    )
}

/// Decode one fixed semantic-surface poll request from copied scratch bytes.
pub(super) fn decode_semantic_surface_request(
    scratch: &rite_scratch,
) -> rite_semantic_surface_request_v1 {
    rite_semantic_surface_request_v1::from_raw_parts(
        u16::from_le_bytes([scratch[0], scratch[1]]),
        u16::from_le_bytes([scratch[2], scratch[3]]),
        u32::from_le_bytes([scratch[4], scratch[5], scratch[6], scratch[7]]),
        u32::from_le_bytes([scratch[8], scratch[9], scratch[10], scratch[11]]),
        u32::from_le_bytes([scratch[12], scratch[13], scratch[14], scratch[15]]),
    )
}

/// Decode the fixed version-1 display header from a staged Rite body.
///
/// This preserves unknown tags and malformed fields as ordinary data. The
/// architecture gate must complete admission first; the presenter validates
/// the decoded relationships and actual output bounds before any write.
pub(super) fn decode_display_patch(scratch: &rite_scratch) -> rite_display_patch_v1 {
    rite_display_patch_v1::from_raw_parts(
        u16::from_le_bytes([scratch[0], scratch[1]]),
        u16::from_le_bytes([scratch[2], scratch[3]]),
        u32::from_le_bytes([scratch[4], scratch[5], scratch[6], scratch[7]]),
        u32::from_le_bytes([scratch[8], scratch[9], scratch[10], scratch[11]]),
        u32::from_le_bytes([scratch[12], scratch[13], scratch[14], scratch[15]]),
        u32::from_le_bytes([scratch[16], scratch[17], scratch[18], scratch[19]]),
        u32::from_le_bytes([scratch[20], scratch[21], scratch[22], scratch[23]]),
        u32::from_le_bytes([scratch[24], scratch[25], scratch[26], scratch[27]]),
        rite_display_pixel_format::from_raw(u16::from_le_bytes([scratch[28], scratch[29]])),
        u16::from_le_bytes([scratch[30], scratch[31]]),
        u32::from_le_bytes([scratch[32], scratch[33], scratch[34], scratch[35]]),
        u32::from_le_bytes([scratch[36], scratch[37], scratch[38], scratch[39]]),
    )
}

/// Borrow only the copied pixel suffix declared by an admitted outer payload.
pub(super) fn display_patch_pixels(
    scratch: &rite_scratch,
    payload_size: u32,
) -> Result<&[u8], &'static str> {
    let payload_size = usize::try_from(payload_size).map_err(|_| "display_patch_size_invalid")?;
    if !((rite_display_patch_min_payload_size as usize)..=transport_payload_limit as usize)
        .contains(&payload_size)
    {
        return Err("display_patch_span_invalid");
    }
    Ok(&scratch[display_patch_header_size..payload_size])
}

/// Encode and validate one complete private semantic-session delivery before
/// any user mailbox can be mutated.
pub(super) fn prepare_session_input_delivery_reply(
    reply: rite_reply,
    delivery: &rite_session_input_delivery_v1,
) -> Result<prepared_session_input_reply, &'static str> {
    if !reply.has_supported_header()
        || reply.status != rite_status::ok
        || reply.payload_kind != rite_payload_kind::session_input_delivery
        || reply.payload_size as usize != session_input_delivery_size
        || !delivery.has_supported_body()
    {
        return Err("rite_session_input_delivery_reply_invalid");
    }

    Ok(prepared_session_input_reply {
        prefix: encode_reply_bytes(reply),
        body: encode_session_input_delivery(delivery),
        body_present: true,
    })
}

/// Prepare one payload-free semantic-session outcome while retaining the same
/// fixed body-clear contract as a nonempty delivery.
pub(super) fn prepare_session_input_empty_reply(
    reply: rite_reply,
) -> Result<prepared_session_input_reply, &'static str> {
    if !reply.has_supported_header()
        || !matches!(
            reply.status,
            rite_status::would_block | rite_status::interrupted
        )
        || reply.payload_kind != rite_payload_kind::none
        || reply.payload_size != 0
    {
        return Err("rite_session_input_empty_reply_invalid");
    }

    Ok(prepared_session_input_reply {
        prefix: encode_reply_bytes(reply),
        body: [0; session_input_delivery_size],
        body_present: false,
    })
}

impl prepared_session_input_reply {
    /// Invalidate the header, clear the complete fixed reply-body area, copy an
    /// optional delivery body, and write every other prefix field.
    ///
    /// Every fallible span check occurs before the first mailbox write. Once
    /// those checks pass, the volatile stores are bounded and infallible under
    /// the proof's fixed mapping contract. A hardware mapping fault is fail-stop.
    ///
    /// # Safety
    ///
    /// `mailbox_alias` must name the live, page-aligned, allocator-owned
    /// supervisor alias of the exact consumer's one RW+NX mailbox. The caller
    /// must retain exclusive access until the returned staged reply is either
    /// published or deliberately abandoned with an invalid header.
    pub(super) unsafe fn stage(
        mut self,
        mailbox_alias: u64,
    ) -> Result<staged_session_input_reply, &'static str> {
        if !mailbox_span_fits(mailbox_alias, 0, session_input_reply_span_size) {
            return Err("rite_session_input_reply_span_invalid");
        }

        let header = [self.prefix[0], self.prefix[1]];
        // SAFETY: The complete fixed span was checked above and the caller
        // supplies its lifetime, ownership, and exclusion guarantees.
        unsafe {
            write_mailbox_bytes_unchecked(mailbox_alias, 0, &[0, 0]);
            clear_mailbox_bytes_unchecked(mailbox_alias, call_size, session_input_delivery_size);
            if self.body_present {
                write_mailbox_bytes_unchecked(mailbox_alias, call_size, &self.body);
            }
            write_mailbox_bytes_unchecked(mailbox_alias, 2, &self.prefix[2..]);
        }
        compiler_fence(Ordering::SeqCst);
        let _ = clear_and_verify_bytes(&mut self.prefix);
        let _ = clear_and_verify_bytes(&mut self.body);
        self.body_present = false;

        Ok(staged_session_input_reply {
            mailbox_alias,
            header,
        })
    }
}

/// Encode and validate one fixed semantic-surface delivery before mailbox mutation.
pub(super) fn prepare_semantic_surface_delivery_reply(
    reply: rite_reply,
    body: &[u8; semantic_surface_snapshot_size],
) -> Result<prepared_semantic_surface_reply, &'static str> {
    if !reply.has_supported_header()
        || reply.status != rite_status::ok
        || reply.payload_kind != rite_payload_kind::semantic_surface_snapshot
        || reply.payload_size as usize != semantic_surface_snapshot_size
    {
        return Err("rite_semantic_surface_delivery_reply_invalid");
    }

    Ok(prepared_semantic_surface_reply {
        prefix: encode_reply_bytes(reply),
        body: *body,
        body_present: true,
    })
}

/// Prepare a payload-free semantic-surface outcome while clearing the fixed body area.
///
/// A successful submit returns the same fixed empty shape as a rejection or
/// poll outcome: `ok`, no payload kind, and a zero payload size.  Delivery is
/// the only semantic-surface success that carries a body.
pub(super) fn prepare_semantic_surface_empty_reply(
    reply: rite_reply,
) -> Result<prepared_semantic_surface_reply, &'static str> {
    if !reply.has_supported_header()
        || !matches!(
            reply.status,
            rite_status::ok
                | rite_status::would_block
                | rite_status::denied
                | rite_status::bad_payload
        )
        || reply.payload_kind != rite_payload_kind::none
        || reply.payload_size != 0
    {
        return Err("rite_semantic_surface_empty_reply_invalid");
    }

    Ok(prepared_semantic_surface_reply {
        prefix: encode_reply_bytes(reply),
        body: [0; semantic_surface_snapshot_size],
        body_present: false,
    })
}

impl prepared_semantic_surface_reply {
    /// Clear and stage the complete fixed reply span with the header invalid.
    ///
    /// # Safety
    ///
    /// `mailbox_alias` must be the live page-aligned supervisor alias of the
    /// exact consumer mailbox and remain exclusively owned through publication.
    pub(super) unsafe fn stage(
        mut self,
        mailbox_alias: u64,
    ) -> Result<staged_semantic_surface_reply, &'static str> {
        if !mailbox_span_fits(mailbox_alias, 0, semantic_surface_reply_span_size) {
            return Err("rite_semantic_surface_reply_span_invalid");
        }

        let header = [self.prefix[0], self.prefix[1]];
        // SAFETY: The complete span was checked above and the caller supplies
        // the mapping lifetime, ownership, and serialized gate interval.
        unsafe {
            write_mailbox_bytes_unchecked(mailbox_alias, 0, &[0, 0]);
            clear_mailbox_bytes_unchecked(mailbox_alias, call_size, semantic_surface_snapshot_size);
            if self.body_present {
                write_mailbox_bytes_unchecked(mailbox_alias, call_size, &self.body);
            }
            write_mailbox_bytes_unchecked(mailbox_alias, 2, &self.prefix[2..]);
        }
        compiler_fence(Ordering::SeqCst);
        let _ = clear_and_verify_bytes(&mut self.prefix);
        let _ = clear_and_verify_bytes(&mut self.body);
        self.body_present = false;

        Ok(staged_semantic_surface_reply {
            mailbox_alias,
            header,
        })
    }
}

fn encode_session_input_delivery(
    delivery: &rite_session_input_delivery_v1,
) -> [u8; session_input_delivery_size] {
    let mut bytes = [0_u8; session_input_delivery_size];
    bytes[0..2].copy_from_slice(&delivery.structure_size().to_le_bytes());
    bytes[2..4].copy_from_slice(&delivery.version().to_le_bytes());
    bytes[4..6].copy_from_slice(&delivery.expose_action().raw().to_le_bytes());
    bytes[6..8].copy_from_slice(&delivery.expose_origin().raw().to_le_bytes());
    bytes[8..10].copy_from_slice(&delivery.expose_text_byte_length().to_le_bytes());
    bytes[10..12].copy_from_slice(&delivery.expose_erase_scalar_count().to_le_bytes());
    bytes[12..16].copy_from_slice(&delivery.reserved().to_le_bytes());
    bytes[16..24].copy_from_slice(&delivery.delivery_sequence().to_le_bytes());
    bytes[24..].copy_from_slice(delivery.expose_text_area());
    bytes
}

/// Encode and copy one fixed rite reply to a physical mailbox alias.
///
/// # Safety
///
/// `mailbox_alias` must name a live, identity-mapped, allocator-owned page that
/// remains writable and mutation-excluded for the complete volatile copy.
pub(super) unsafe fn write_reply(
    mailbox_alias: u64,
    reply: rite_reply,
) -> Result<(), &'static str> {
    if !reply.has_supported_header() {
        return Err("rite_reply_header_invalid");
    }
    let bytes = encode_reply_bytes(reply);
    // SAFETY: The caller provides the complete alias lifetime, mapping, and
    // exclusivity contract required by the raw copy.
    unsafe { write_mailbox_bytes(mailbox_alias, 0, &bytes) }
}

pub(super) fn encode_reply_bytes(reply: rite_reply) -> [u8; reply_size] {
    let mut bytes = [0_u8; reply_size];
    bytes[0..2].copy_from_slice(&reply.header_size.to_le_bytes());
    bytes[2..4].copy_from_slice(&reply.status.raw().to_le_bytes());
    bytes[4..6].copy_from_slice(&reply.payload_kind.raw().to_le_bytes());
    bytes[6..8].copy_from_slice(&reply.reserved.to_le_bytes());
    bytes[8..12].copy_from_slice(&reply.payload_size.to_le_bytes());
    bytes[12..16].copy_from_slice(&reply.reserved_2.to_le_bytes());
    bytes[16..24].copy_from_slice(&reply.sequence.to_le_bytes());
    bytes[24..40].copy_from_slice(&reply.trace_id.bytes);
    bytes
}

/// Copy an admitted rite body into fixed kernel-owned scratch storage.
///
/// # Safety
///
/// `mailbox_alias` must name a live, identity-mapped, allocator-owned page that
/// remains readable and mutation-excluded for the complete volatile copy. It
/// must be the physical alias of the mapping at `user_mailbox_address`.
pub(super) unsafe fn copy_admitted_body(
    mailbox_alias: u64,
    user_mailbox_address: u64,
    payload_size: u32,
    scratch: &mut rite_scratch,
    copy_count: &mut usize,
    bytes_copied: &mut usize,
) -> Result<(), &'static str> {
    if !scratch_is_zero(scratch) || !payload_span_fits(user_mailbox_address, payload_size) {
        return Err("rite_body_copy_preflight_failed");
    }
    let length = usize::try_from(payload_size).map_err(|_| "rite_body_size_invalid")?;
    let next_copy_count = copy_count
        .checked_add(1)
        .ok_or("rite_body_copy_count_overflow")?;
    let next_bytes_copied = bytes_copied
        .checked_add(length)
        .ok_or("rite_body_byte_count_overflow")?;
    // SAFETY: The caller provides the complete alias lifetime, mapping, and
    // exclusivity contract required by the raw copy.
    if let Err(reason) =
        unsafe { read_mailbox_bytes(mailbox_alias, call_size, &mut scratch[..length]) }
    {
        if !clear_and_verify_bytes(scratch) {
            return Err("rite_body_copy_scratch_clear_failed");
        }
        return Err(reason);
    }
    *copy_count = next_copy_count;
    *bytes_copied = next_bytes_copied;
    Ok(())
}

pub(super) fn payload_span_fits(user_mailbox_address: u64, payload_size: u32) -> bool {
    let Ok(length) = usize::try_from(payload_size) else {
        return false;
    };
    let Some(end) = call_size.checked_add(length) else {
        return false;
    };
    let Some(virtual_end) = user_mailbox_address.checked_add(end as u64) else {
        return false;
    };
    let Some(mailbox_end) = user_mailbox_address.checked_add(page_size as u64) else {
        return false;
    };
    end <= page_size
        && paging::is_canonical(user_mailbox_address)
        && paging::is_canonical(virtual_end.saturating_sub(1))
        && virtual_end <= mailbox_end
}

/// Validate the one-page temporal request/reply overlap used by the semantic
/// session operation. The request must already be staged into kernel-owned
/// state before callers use the same offset for a reply body.
pub(super) fn session_input_reply_span_fits(user_mailbox_address: u64) -> bool {
    let Some(virtual_end) = user_mailbox_address.checked_add(session_input_reply_span_size as u64)
    else {
        return false;
    };
    let Some(mailbox_end) = user_mailbox_address.checked_add(page_size as u64) else {
        return false;
    };
    paging::is_canonical(user_mailbox_address)
        && paging::is_canonical(virtual_end - 1)
        && virtual_end <= mailbox_end
}

/// Validate the one-page request/reply overlap for a semantic-surface poll.
pub(super) fn semantic_surface_reply_span_fits(user_mailbox_address: u64) -> bool {
    let Some(virtual_end) =
        user_mailbox_address.checked_add(semantic_surface_reply_span_size as u64)
    else {
        return false;
    };
    let Some(mailbox_end) = user_mailbox_address.checked_add(page_size as u64) else {
        return false;
    };
    paging::is_canonical(user_mailbox_address)
        && paging::is_canonical(virtual_end - 1)
        && virtual_end <= mailbox_end
}

pub(super) fn decode_exit_payload(
    scratch: &rite_scratch,
) -> Result<rite_exit_payload, &'static str> {
    if mem::size_of::<rite_exit_payload>() != 8 {
        return Err("rite_exit_layout_invalid");
    }
    Ok(rite_exit_payload {
        code: i32::from_le_bytes([scratch[0], scratch[1], scratch[2], scratch[3]]),
        reserved: u32::from_le_bytes([scratch[4], scratch[5], scratch[6], scratch[7]]),
    })
}

pub(super) fn scratch_is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

pub(super) fn clear_and_verify_bytes(bytes: &mut [u8]) -> bool {
    for byte in bytes.iter_mut() {
        // SAFETY: The caller exclusively owns the complete mutable slice.
        unsafe { ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
    bytes.iter().all(|byte| {
        // SAFETY: The same exclusive slice remains live through verification.
        unsafe { ptr::read_volatile(byte) == 0 }
    })
}

unsafe fn read_mailbox_bytes(
    mailbox_alias: u64,
    offset: usize,
    destination: &mut [u8],
) -> Result<(), &'static str> {
    if !mailbox_span_fits(mailbox_alias, offset, destination.len()) {
        return Err("rite_mailbox_read_out_of_bounds");
    }
    let base = usize::try_from(mailbox_alias).map_err(|_| "rite_mailbox_alias_invalid")?;
    copy_bounded_body_bytes(destination, |index| {
        // SAFETY: The kernel caller must retain exclusive ownership of this
        // checked one-page physical alias and keep it identity-mapped and
        // readable for the complete volatile copy.
        Ok(unsafe { ptr::read_volatile((base as *const u8).add(offset + index)) })
    })?;
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

unsafe fn write_mailbox_bytes(
    mailbox_alias: u64,
    offset: usize,
    source: &[u8],
) -> Result<(), &'static str> {
    if !mailbox_span_fits(mailbox_alias, offset, source.len()) {
        return Err("rite_mailbox_write_out_of_bounds");
    }
    let base = usize::try_from(mailbox_alias).map_err(|_| "rite_mailbox_alias_invalid")?;
    for (index, byte) in source.iter().copied().enumerate() {
        // SAFETY: The kernel caller must retain exclusive ownership of this
        // checked one-page physical alias and keep it identity-mapped and
        // writable for the complete volatile copy.
        unsafe { ptr::write_volatile((base as *mut u8).add(offset + index), byte) };
    }
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

unsafe fn write_mailbox_bytes_unchecked(mailbox_alias: u64, offset: usize, source: &[u8]) {
    debug_assert!(mailbox_span_fits(mailbox_alias, offset, source.len()));
    let base = mailbox_alias as usize;
    for (index, byte) in source.iter().copied().enumerate() {
        // SAFETY: The caller already proved the complete destination span and
        // retains exclusive ownership for this bounded commit interval.
        unsafe { ptr::write_volatile((base as *mut u8).add(offset + index), byte) };
    }
    compiler_fence(Ordering::SeqCst);
}

unsafe fn clear_mailbox_bytes_unchecked(mailbox_alias: u64, offset: usize, length: usize) {
    debug_assert!(mailbox_span_fits(mailbox_alias, offset, length));
    let base = mailbox_alias as usize;
    for index in 0..length {
        // SAFETY: The caller already proved the complete destination span and
        // retains exclusive ownership for this bounded commit interval.
        unsafe { ptr::write_volatile((base as *mut u8).add(offset + index), 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

fn mailbox_span_fits(mailbox_alias: u64, offset: usize, length: usize) -> bool {
    let Some(end) = offset.checked_add(length) else {
        return false;
    };
    mailbox_alias & (page_size as u64 - 1) == 0
        && end <= page_size
        && usize::try_from(mailbox_alias).is_ok()
        && mailbox_alias
            .checked_add(page_size as u64)
            .is_some_and(|limit| limit <= memory::address_limit)
}

#[cfg(test)]
mod tests {
    use anomalous_abi::{
        rite_call, rite_operation, rite_payload_kind, rite_rights, rite_session_input_action,
        rite_session_input_delivery_v1, rite_session_input_origin, rite_session_input_request_v1,
        rite_status,
    };

    use crate::circle::circle_table;
    use crate::identity::{fixture_principal_id, task_identity};
    use crate::rite::{
        rite_channel, rite_channel_test_snapshot, rite_decision, rite_mailbox_mapping_identity,
        rite_session_input_completion_outcome,
    };
    use crate::scheduler::{
        compose_endpoint_wake_authorization, scheduler_selection_authorization,
        scheduler_selection_test_snapshot, scheduler_table, scheduler_test_snapshot,
    };
    use crate::session_input::{
        session_input_action, session_input_action_kind, session_input_binding,
        session_input_consumer_identity, session_input_endpoint, session_input_endpoint_identity,
        session_input_endpoint_test_snapshot, session_input_origin,
        session_input_receive_preparation, session_input_route_epoch,
        session_input_session_identity, session_input_wait_commit,
    };
    use crate::task::{
        synthetic_origin_metadata, task_binding, task_bounds, task_table, task_table_test_snapshot,
    };

    use super::*;

    const user_mailbox: u64 = 0x0000_7000_0000_3000;

    #[derive(Eq, PartialEq)]
    struct composite_copy_observation {
        endpoint: session_input_endpoint_test_snapshot,
        channel: rite_channel_test_snapshot,
        tasks: task_table_test_snapshot,
        scheduler: scheduler_test_snapshot,
        selection: Option<scheduler_selection_test_snapshot>,
    }

    fn composite_task_bounds() -> task_bounds {
        task_bounds::new(0x1000, 0x2000, 0x1000, 0x3000, 0x4000, 0x4000).expect("task bounds")
    }

    fn create_composite_task(
        circles: &mut circle_table,
        tasks: &mut task_table,
        principal: u64,
    ) -> task_identity {
        let circle = circles.create().expect("circle");
        tasks
            .create(
                circles,
                task_binding::new(
                    circle,
                    fixture_principal_id::new(principal).expect("principal"),
                    synthetic_origin_metadata::machine_role_fixture,
                    composite_task_bounds(),
                ),
            )
            .expect("task")
    }

    fn observe_composite_copy_state(
        endpoint: &session_input_endpoint,
        channel: &rite_channel,
        tasks: &task_table,
        scheduler: &scheduler_table,
        selection: &Option<scheduler_selection_authorization>,
    ) -> composite_copy_observation {
        composite_copy_observation {
            endpoint: endpoint.test_snapshot(),
            channel: channel.test_snapshot(),
            tasks: tasks.test_snapshot(),
            scheduler: scheduler.test_snapshot(),
            selection: selection
                .as_ref()
                .map(scheduler_selection_authorization::test_snapshot),
        }
    }

    #[test]
    fn call_prefix_decodes_explicit_little_endian_wire_bytes() {
        let mut bytes = [0_u8; call_size];
        bytes[0..2].copy_from_slice(&(call_size as u16).to_le_bytes());
        bytes[2..4].copy_from_slice(&rite_operation::console_write.raw().to_le_bytes());
        bytes[4..6].copy_from_slice(&rite_payload_kind::utf8_text.raw().to_le_bytes());
        bytes[8..16].copy_from_slice(&rite_rights::console_write.to_le_bytes());
        bytes[16..20].copy_from_slice(&17_u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&31_u32.to_le_bytes());
        let trace = [
            0x01, 0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x9a, 0xab, 0xbc, 0xcd, 0xde,
            0xef, 0xf0,
        ];
        bytes[24..40].copy_from_slice(&trace);

        let call = decode_call_bytes(&bytes);

        assert_eq!(call.header_size as usize, call_size);
        assert_eq!(call.operation, rite_operation::console_write);
        assert_eq!(call.payload_kind, rite_payload_kind::utf8_text);
        assert_eq!(call.reserved, 0);
        assert_eq!(call.rights_present, rite_rights::console_write);
        assert_eq!(call.payload_size, 17);
        assert_eq!(call.payload_limit, 31);
        assert_eq!(call.trace_id, id_128::from_bytes(trace));
    }

    #[test]
    fn reply_prefix_encodes_explicit_little_endian_wire_bytes() {
        let trace = id_128::from_bytes([0xa5; 16]);
        let reply = rite_reply::new(
            rite_status::would_block,
            rite_payload_kind::none,
            0,
            0x1020_3040_5060_7080,
            trace,
        );

        let bytes = encode_reply_bytes(reply);

        assert_eq!(&bytes[0..2], &(reply_size as u16).to_le_bytes());
        assert_eq!(&bytes[2..4], &rite_status::would_block.raw().to_le_bytes());
        assert_eq!(&bytes[4..6], &rite_payload_kind::none.raw().to_le_bytes());
        assert_eq!(&bytes[6..8], &[0; 2]);
        assert_eq!(&bytes[8..16], &[0; 8]);
        assert_eq!(&bytes[16..24], &0x1020_3040_5060_7080_u64.to_le_bytes());
        assert_eq!(&bytes[24..40], &[0xa5; 16]);
    }

    #[test]
    fn session_input_request_decodes_exact_little_endian_fields() {
        let mut scratch = [0_u8; page_size];
        scratch[0..2].copy_from_slice(&16_u16.to_le_bytes());
        scratch[2..4].copy_from_slice(&1_u16.to_le_bytes());
        scratch[4..8].copy_from_slice(&1_u32.to_le_bytes());
        scratch[8..12].copy_from_slice(&280_u32.to_le_bytes());
        scratch[12..16].copy_from_slice(&0_u32.to_le_bytes());

        let request = decode_session_input_request(&scratch);

        assert_eq!(request.structure_size(), 16);
        assert_eq!(request.version(), 1);
        assert_eq!(request.flags(), 1);
        assert_eq!(request.reply_capacity(), 280);
        assert_eq!(request.reserved(), 0);
        assert!(request.wait_if_empty());
        assert!(request.has_supported_body());
    }

    #[test]
    fn semantic_surface_request_decodes_exact_little_endian_fields() {
        let mut scratch = [0_u8; page_size];
        scratch[0..2].copy_from_slice(&16_u16.to_le_bytes());
        scratch[2..4].copy_from_slice(&1_u16.to_le_bytes());
        scratch[4..8].copy_from_slice(&0_u32.to_le_bytes());
        scratch[8..12].copy_from_slice(&528_u32.to_le_bytes());
        scratch[12..16].copy_from_slice(&0_u32.to_le_bytes());

        let request = decode_semantic_surface_request(&scratch);

        assert_eq!(request.structure_size(), 16);
        assert_eq!(request.version(), 1);
        assert_eq!(request.flags(), 0);
        assert_eq!(request.reply_capacity(), 528);
        assert_eq!(request.reserved(), 0);
        assert!(request.has_supported_body());
    }

    #[test]
    fn display_patch_decodes_every_little_endian_field_without_trusting_it() {
        let mut scratch = [0_u8; page_size];
        scratch[0..2].copy_from_slice(&0x0102_u16.to_le_bytes());
        scratch[2..4].copy_from_slice(&0x0304_u16.to_le_bytes());
        scratch[4..8].copy_from_slice(&0x0506_0708_u32.to_le_bytes());
        scratch[8..12].copy_from_slice(&0x1112_1314_u32.to_le_bytes());
        scratch[12..16].copy_from_slice(&0x2122_2324_u32.to_le_bytes());
        scratch[16..20].copy_from_slice(&0x3132_3334_u32.to_le_bytes());
        scratch[20..24].copy_from_slice(&0x4142_4344_u32.to_le_bytes());
        scratch[24..28].copy_from_slice(&0x5152_5354_u32.to_le_bytes());
        scratch[28..30].copy_from_slice(&0x6162_u16.to_le_bytes());
        scratch[30..32].copy_from_slice(&0x7172_u16.to_le_bytes());
        scratch[32..36].copy_from_slice(&0x8182_8384_u32.to_le_bytes());
        scratch[36..40].copy_from_slice(&0x9192_9394_u32.to_le_bytes());

        let patch = decode_display_patch(&scratch);

        assert_eq!(patch.structure_size(), 0x0102);
        assert_eq!(patch.version(), 0x0304);
        assert_eq!(patch.flags(), 0x0506_0708);
        assert_eq!(patch.x(), 0x1112_1314);
        assert_eq!(patch.y(), 0x2122_2324);
        assert_eq!(patch.width(), 0x3132_3334);
        assert_eq!(patch.height(), 0x4142_4344);
        assert_eq!(patch.row_bytes(), 0x5152_5354);
        assert_eq!(patch.pixel_format().raw(), 0x6162);
        assert_eq!(patch.reserved(), 0x7172);
        assert_eq!(patch.pixel_byte_length(), 0x8182_8384);
        assert_eq!(patch.reserved_2(), 0x9192_9394);
        assert!(!patch.has_supported_body());
    }

    #[test]
    fn display_patch_pixel_suffix_is_exactly_bounded_by_the_copied_payload() {
        let mut scratch = [0_u8; page_size];
        for (index, byte) in scratch.iter_mut().enumerate() {
            *byte = index as u8;
        }

        let minimum = display_patch_pixels(&scratch, 43).expect("one RGB8 pixel");
        assert_eq!(minimum, &[40, 41, 42]);

        let maximum = display_patch_pixels(&scratch, transport_payload_limit)
            .expect("complete mailbox payload capacity");
        assert_eq!(maximum.len(), display_patch_pixel_capacity);
        assert_eq!(maximum.first(), Some(&40));
        assert_eq!(maximum.last(), Some(&215));

        for payload_size in [0, 39, 40, 42, transport_payload_limit + 1, u32::MAX] {
            assert!(display_patch_pixels(&scratch, payload_size).is_err());
        }
    }

    #[test]
    fn session_input_delivery_encodes_the_complete_fixed_body() {
        let delivery = rite_session_input_delivery_v1::try_insert_text(
            rite_session_input_origin::machine_text,
            0x1020_3040_5060_7080,
            "A\n☃".as_bytes(),
        )
        .expect("valid delivery");
        let reply = rite_reply::new(
            rite_status::ok,
            rite_payload_kind::session_input_delivery,
            session_input_delivery_size as u32,
            7,
            id_128::from_bytes([0x6d; 16]),
        );

        let transaction =
            prepare_session_input_delivery_reply(reply, &delivery).expect("valid transaction");

        assert_eq!(&transaction.body[0..2], &280_u16.to_le_bytes());
        assert_eq!(&transaction.body[2..4], &1_u16.to_le_bytes());
        assert_eq!(
            &transaction.body[4..6],
            &rite_session_input_action::insert_text.raw().to_le_bytes()
        );
        assert_eq!(
            &transaction.body[6..8],
            &rite_session_input_origin::machine_text.raw().to_le_bytes()
        );
        assert_eq!(&transaction.body[8..10], &5_u16.to_le_bytes());
        assert_eq!(&transaction.body[10..16], &[0; 6]);
        assert_eq!(
            &transaction.body[16..24],
            &0x1020_3040_5060_7080_u64.to_le_bytes()
        );
        assert_eq!(&transaction.body[24..29], "A\n☃".as_bytes());
        assert!(transaction.body[29..].iter().all(|byte| *byte == 0));
        assert!(transaction.body_present);
    }

    #[test]
    fn session_input_reply_preparation_rejects_crossed_shapes() {
        let delivery =
            rite_session_input_delivery_v1::try_submit(rite_session_input_origin::human_text, 1)
                .expect("valid delivery");
        let trace = id_128::from_bytes([0x81; 16]);
        for reply in [
            rite_reply::new(
                rite_status::would_block,
                rite_payload_kind::session_input_delivery,
                280,
                1,
                trace,
            ),
            rite_reply::new(rite_status::ok, rite_payload_kind::none, 280, 1, trace),
            rite_reply::new(
                rite_status::ok,
                rite_payload_kind::session_input_delivery,
                279,
                1,
                trace,
            ),
        ] {
            assert!(prepare_session_input_delivery_reply(reply, &delivery).is_err());
        }

        for reply in [
            rite_reply::new(rite_status::ok, rite_payload_kind::none, 0, 1, trace),
            rite_reply::new(
                rite_status::interrupted,
                rite_payload_kind::session_input_delivery,
                0,
                1,
                trace,
            ),
            rite_reply::new(
                rite_status::would_block,
                rite_payload_kind::none,
                1,
                1,
                trace,
            ),
        ] {
            assert!(prepare_session_input_empty_reply(reply).is_err());
        }

        assert!(
            prepare_session_input_empty_reply(rite_reply::new(
                rite_status::would_block,
                rite_payload_kind::none,
                0,
                1,
                trace,
            ))
            .is_ok()
        );
        assert!(
            prepare_session_input_empty_reply(rite_reply::new(
                rite_status::interrupted,
                rite_payload_kind::none,
                0,
                1,
                trace,
            ))
            .is_ok()
        );
    }

    #[test]
    fn host_copy_failures_never_publish_a_valid_header() {
        let delivery = rite_session_input_delivery_v1::try_insert_text(
            rite_session_input_origin::human_text,
            1,
            &[b'A'; 256],
        )
        .expect("valid delivery");
        let reply = rite_reply::new(
            rite_status::ok,
            rite_payload_kind::session_input_delivery,
            session_input_delivery_size as u32,
            1,
            id_128::from_bytes([0x31; 16]),
        );
        let transaction =
            prepare_session_input_delivery_reply(reply, &delivery).expect("valid transaction");

        for failure_at in 0..session_input_delivery_size {
            let mut mailbox = [0x7b; session_input_reply_span_size];
            assert_eq!(
                model_stage(&transaction, &mut mailbox, Some(failure_at)),
                Err("injected_session_input_body_copy_failure")
            );
            assert_eq!(&mailbox[0..2], &[0, 0]);
            assert!(
                mailbox[call_size + failure_at..]
                    .iter()
                    .all(|byte| *byte == 0)
            );
        }

        let mut mailbox = [0x7b; session_input_reply_span_size];
        model_stage(&transaction, &mut mailbox, None).expect("complete staging");
        assert_eq!(&mailbox[0..2], &[0, 0]);
        assert_eq!(
            &mailbox[call_size..],
            transaction.body.as_slice(),
            "complete body precedes header publication"
        );
        model_publish(&transaction, &mut mailbox);
        assert_eq!(&mailbox[0..2], &(reply_size as u16).to_le_bytes());
    }

    #[test]
    fn semantic_surface_copy_failures_preserve_an_invalid_header_and_complete_body() {
        let mut body = [0_u8; semantic_surface_snapshot_size];
        for (index, byte) in body.iter_mut().enumerate() {
            *byte = index.wrapping_mul(37) as u8;
        }
        let reply = rite_reply::new(
            rite_status::ok,
            rite_payload_kind::semantic_surface_snapshot,
            semantic_surface_snapshot_size as u32,
            7,
            id_128::from_bytes([0xa7; 16]),
        );
        let transaction =
            prepare_semantic_surface_delivery_reply(reply, &body).expect("valid transaction");

        for failure_at in 0..semantic_surface_snapshot_size {
            let mut mailbox = [0x7b; semantic_surface_reply_span_size];
            assert_eq!(
                model_semantic_surface_stage(&transaction, &mut mailbox, Some(failure_at)),
                Err("injected_semantic_surface_body_copy_failure")
            );
            assert_eq!(&mailbox[0..2], &[0, 0]);
            assert!(
                mailbox[call_size + failure_at..]
                    .iter()
                    .all(|byte| *byte == 0)
            );
        }

        let mut mailbox = [0x7b; semantic_surface_reply_span_size];
        model_semantic_surface_stage(&transaction, &mut mailbox, None).expect("complete staging");
        assert_eq!(&mailbox[0..2], &[0, 0]);
        assert_eq!(&mailbox[call_size..], body.as_slice());
        model_semantic_surface_publish(&transaction, &mut mailbox);
        assert_eq!(&mailbox[0..2], &(reply_size as u16).to_le_bytes());
    }

    #[test]
    fn semantic_surface_empty_poll_clears_a_prior_fixed_body() {
        let trace = id_128::from_bytes([0x82; 16]);
        let body = [0x5a; semantic_surface_snapshot_size];
        let full = prepare_semantic_surface_delivery_reply(
            rite_reply::new(
                rite_status::ok,
                rite_payload_kind::semantic_surface_snapshot,
                semantic_surface_snapshot_size as u32,
                1,
                trace,
            ),
            &body,
        )
        .expect("full transaction");
        let empty = prepare_semantic_surface_empty_reply(rite_reply::new(
            rite_status::would_block,
            rite_payload_kind::none,
            0,
            2,
            trace,
        ))
        .expect("empty transaction");
        let mut mailbox = [0x77; semantic_surface_reply_span_size];

        model_semantic_surface_stage(&full, &mut mailbox, None).expect("full stage");
        model_semantic_surface_publish(&full, &mut mailbox);
        assert!(mailbox[call_size..].iter().all(|byte| *byte == 0x5a));
        model_semantic_surface_stage(&empty, &mut mailbox, None).expect("empty stage");
        model_semantic_surface_publish(&empty, &mut mailbox);

        assert_eq!(&mailbox[0..2], &(reply_size as u16).to_le_bytes());
        assert!(mailbox[call_size..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn semantic_surface_submit_ok_prepares_the_exact_empty_success_reply() {
        let trace = id_128::from_bytes([0x41; 16]);
        let reply = rite_reply::new(rite_status::ok, rite_payload_kind::none, 0, 3, trace);
        let transaction =
            prepare_semantic_surface_empty_reply(reply).expect("submit success reply shape");
        let mut mailbox = [0x77; semantic_surface_reply_span_size];

        model_semantic_surface_stage(&transaction, &mut mailbox, None)
            .expect("stage submit success reply");
        assert_eq!(&mailbox[0..2], &[0, 0]);
        assert!(mailbox[call_size..].iter().all(|byte| *byte == 0));
        model_semantic_surface_publish(&transaction, &mut mailbox);

        assert_eq!(&mailbox[0..2], &(reply_size as u16).to_le_bytes());
        assert_eq!(
            u16::from_le_bytes([mailbox[2], mailbox[3]]),
            rite_status::ok.raw()
        );
        assert_eq!(
            u16::from_le_bytes([mailbox[4], mailbox[5]]),
            rite_payload_kind::none.raw()
        );
        assert_eq!(
            u32::from_le_bytes([mailbox[8], mailbox[9], mailbox[10], mailbox[11]]),
            0
        );
    }

    #[test]
    fn every_body_copy_failure_preserves_the_full_selected_delivery_transaction() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let consumer = create_composite_task(&mut circles, &mut tasks, 1);
        let peer = create_composite_task(&mut circles, &mut tasks, 2);
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, consumer)
            .expect("admit consumer");
        scheduler
            .admit(&mut circles, &mut tasks, peer)
            .expect("admit peer");
        let initial = scheduler.start_next(&mut tasks).expect("start consumer");
        assert!(initial.task() == consumer);

        let binding = session_input_binding::new(
            session_input_endpoint_identity::try_new(0, 1).expect("endpoint"),
            session_input_session_identity::try_new(1, 1).expect("session"),
            session_input_consumer_identity::try_new(1, 1).expect("consumer"),
            session_input_route_epoch::try_new(1).expect("route"),
            consumer,
        );
        let mapping = rite_mailbox_mapping_identity::try_new(consumer, 1).expect("mapping");
        let request_body = rite_session_input_request_v1::new(true);
        let request = rite_call::new(
            rite_operation::session_input_read,
            rite_payload_kind::session_input_request,
            rite_rights::session_input_read,
            session_input_request_size as u32,
            session_input_request_size as u32,
            id_128::from_bytes([0x31; 16]),
        );
        let mut endpoint = session_input_endpoint::new(binding);
        let mut channel =
            rite_channel::try_new(consumer, rite_rights::session_input_read).expect("channel");

        let prepared_wait = match endpoint.prepare_receive(binding).expect("prepare wait") {
            session_input_receive_preparation::wait(prepared) => prepared,
            session_input_receive_preparation::ready => panic!("empty endpoint reported ready"),
        };
        let wait = prepared_wait.proposed_wait();
        let readiness = prepared_wait.proposed_readiness();
        let admission = match channel
            .assess(&tasks, consumer, request)
            .expect("Rite assessment")
        {
            rite_decision::admitted(admission) => admission,
            rite_decision::rejected(_) => panic!("session input Rite was rejected"),
        };
        let pending_preparation = admission
            .prepare_session_input_pending(&request_body, binding, &prepared_wait, mapping)
            .expect("prepare pending Rite");
        let mut scheduler_wake = None;
        let prepared_block = scheduler
            .prepare_block_current_and_reserve(
                &mut tasks,
                consumer,
                peer,
                wait,
                readiness,
                &mut scheduler_wake,
            )
            .expect("prepare reserved block");
        let blocked = match endpoint.commit_wait(prepared_wait).expect("commit wait") {
            session_input_wait_commit::blocked(blocked) => blocked,
            session_input_wait_commit::ready => panic!("empty endpoint avoided block"),
        };
        pending_preparation.commit(blocked);
        let blocked_transition = prepared_block.commit();
        assert!(blocked_transition.task() == consumer);
        assert!(blocked_transition.dispatch().task() == peer);
        assert!(scheduler_wake.is_some());

        let private_text = [b'A'; 256];
        let publication = endpoint
            .prepare_publication(
                binding,
                session_input_origin::machine_text,
                session_input_action::try_insert_text(&private_text).expect("private input action"),
            )
            .unwrap_or_else(|rejection| {
                panic!("publication preparation: {}", rejection.error().code())
            });
        let correlation = publication
            .wake_correlation()
            .expect("publication wake correlation");
        let mut composed_wake = None;
        let mut selection = None;
        let wake_preflight = scheduler
            .preflight_endpoint_wake(
                &tasks,
                &scheduler_wake,
                correlation,
                &composed_wake,
                &selection,
            )
            .expect("wake preflight");
        let publication = endpoint
            .commit_publication(publication)
            .unwrap_or_else(|rejection| panic!("publication commit: {}", rejection.error().code()));
        let endpoint_wake = publication.into_wake().expect("endpoint wake");
        if let Err(rejection) = compose_endpoint_wake_authorization(
            &mut scheduler_wake,
            endpoint_wake,
            wake_preflight,
            &mut composed_wake,
        ) {
            panic!("wake composition: {:?}", rejection.error());
        }
        let prepared_wake = scheduler
            .prepare_endpoint_wake(&mut tasks, &mut composed_wake, &mut selection)
            .expect("prepare scheduler wake");
        assert!(prepared_wake.commit().task() == consumer);
        assert!(scheduler_wake.is_none());
        assert!(composed_wake.is_none());
        assert!(selection.is_some());

        let baseline =
            observe_composite_copy_state(&endpoint, &channel, &tasks, &scheduler, &selection);
        for failure_at in 0..session_input_delivery_size {
            let pending = channel
                .session_input_pending()
                .expect("pending Rite before injected failure");
            let prepared_delivery = endpoint
                .prepare_delivery(binding, Some(correlation))
                .expect("prepare source delivery");
            let mut wire = {
                let delivery = prepared_delivery
                    .delivery(&endpoint)
                    .expect("borrow source delivery");
                assert!(delivery.binding() == binding);
                assert!(delivery.expose_origin() == session_input_origin::machine_text);
                assert!(delivery.expose_action_kind() == session_input_action_kind::insert_text);
                assert!(delivery.expose_sequence().get() == 1);
                assert!(usize::from(delivery.expose_byte_len()) == private_text.len());
                assert!(delivery.expose_text().map(str::as_bytes) == Some(private_text.as_slice()));
                rite_session_input_delivery_v1::try_insert_text(
                    rite_session_input_origin::machine_text,
                    delivery.expose_sequence().get(),
                    delivery.expose_text().expect("delivery text").as_bytes(),
                )
                .expect("wire delivery")
            };
            let completion = channel
                .prepare_session_input_completion(
                    pending,
                    rite_session_input_completion_outcome::delivery,
                )
                .expect("prepare pending completion");
            let transaction = prepare_session_input_delivery_reply(completion.reply(), &wire)
                .expect("prepare reply transaction");
            wire.clear();
            assert!(wire.is_clear());
            let prepared_selection = scheduler
                .prepare_reserved_preemption(&mut tasks, peer, &mut selection)
                .expect("prepare selected consumer");
            let predicted = prepared_selection.transition();
            assert!(predicted.task() == peer);
            assert!(predicted.dispatch().task() == consumer);

            let mut mailbox = [0x7b; session_input_reply_span_size];
            assert_eq!(
                model_stage(&transaction, &mut mailbox, Some(failure_at)),
                Err("injected_session_input_body_copy_failure")
            );
            assert_eq!(&mailbox[0..2], &[0, 0]);
            assert_eq!(&mailbox[2..call_size], &[0x7b; call_size - 2]);
            assert_eq!(
                &mailbox[call_size..call_size + failure_at],
                &transaction.body[..failure_at]
            );
            assert!(
                mailbox[call_size + failure_at..]
                    .iter()
                    .all(|byte| *byte == 0)
            );
            mailbox.fill(0);

            drop(prepared_selection);
            drop(completion);
            drop(prepared_delivery);
            drop(transaction);
            let after =
                observe_composite_copy_state(&endpoint, &channel, &tasks, &scheduler, &selection);
            assert!(
                after == baseline,
                "injected body-copy failure changed cross-layer state"
            );
        }
    }

    #[test]
    fn empty_outcome_clears_a_prior_long_delivery_body() {
        let trace = id_128::from_bytes([0x42; 16]);
        let delivery = rite_session_input_delivery_v1::try_insert_text(
            rite_session_input_origin::remote_text,
            1,
            &[b'A'; 256],
        )
        .expect("valid delivery");
        let full = prepare_session_input_delivery_reply(
            rite_reply::new(
                rite_status::ok,
                rite_payload_kind::session_input_delivery,
                280,
                1,
                trace,
            ),
            &delivery,
        )
        .expect("delivery transaction");
        let empty = prepare_session_input_empty_reply(rite_reply::new(
            rite_status::interrupted,
            rite_payload_kind::none,
            0,
            2,
            trace,
        ))
        .expect("interruption transaction");
        let mut mailbox = [0x77; session_input_reply_span_size];

        model_stage(&full, &mut mailbox, None).expect("full stage");
        model_publish(&full, &mut mailbox);
        assert!(mailbox[call_size..].iter().any(|byte| *byte != 0));
        model_stage(&empty, &mut mailbox, None).expect("empty stage");
        model_publish(&empty, &mut mailbox);

        assert_eq!(&mailbox[0..2], &(reply_size as u16).to_le_bytes());
        assert!(mailbox[call_size..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn payload_span_accepts_only_the_fixed_canonical_page() {
        assert!(payload_span_fits(user_mailbox, 0));
        assert!(payload_span_fits(user_mailbox, transport_payload_limit));
        assert!(!payload_span_fits(
            user_mailbox,
            transport_payload_limit + 1
        ));
        assert!(!payload_span_fits(0x0000_8000_0000_0000, 0));
        assert!(!payload_span_fits(u64::MAX - 32, 0));
        assert!(semantic_surface_reply_span_fits(user_mailbox));
        assert!(!semantic_surface_reply_span_fits(0x0000_8000_0000_0000));
        assert!(!semantic_surface_reply_span_fits(u64::MAX - 32));
    }

    #[test]
    fn mailbox_span_rejects_unaligned_overflowing_and_out_of_range_aliases() {
        assert!(mailbox_span_fits(0x2000, 0, page_size));
        assert!(mailbox_span_fits(0x2000, page_size, 0));
        assert!(!mailbox_span_fits(0x2001, 0, 1));
        assert!(!mailbox_span_fits(0x2000, page_size, 1));
        assert!(!mailbox_span_fits(u64::MAX & !(page_size as u64 - 1), 0, 1));
        assert!(!mailbox_span_fits(memory::address_limit, 0, 1));
    }

    #[test]
    fn exit_payload_decode_and_scratch_clear_are_exact() {
        let mut scratch = [0_u8; page_size];
        scratch[0..4].copy_from_slice(&(-42_i32).to_le_bytes());
        scratch[4..8].copy_from_slice(&0_u32.to_le_bytes());

        assert_eq!(
            decode_exit_payload(&scratch),
            Ok(rite_exit_payload::new(-42))
        );
        assert!(!scratch_is_zero(&scratch));
        assert!(clear_and_verify_bytes(&mut scratch));
        assert!(scratch_is_zero(&scratch));
    }

    fn model_stage(
        transaction: &prepared_session_input_reply,
        mailbox: &mut [u8; session_input_reply_span_size],
        failure_at: Option<usize>,
    ) -> Result<(), &'static str> {
        mailbox[0..2].fill(0);
        mailbox[call_size..].fill(0);
        if transaction.body_present {
            for (index, byte) in transaction.body.iter().copied().enumerate() {
                if failure_at == Some(index) {
                    return Err("injected_session_input_body_copy_failure");
                }
                mailbox[call_size + index] = byte;
            }
        }
        mailbox[2..call_size].copy_from_slice(&transaction.prefix[2..]);
        Ok(())
    }

    fn model_publish(
        transaction: &prepared_session_input_reply,
        mailbox: &mut [u8; session_input_reply_span_size],
    ) {
        mailbox[0..2].copy_from_slice(&transaction.prefix[0..2]);
    }

    fn model_semantic_surface_stage(
        transaction: &prepared_semantic_surface_reply,
        mailbox: &mut [u8; semantic_surface_reply_span_size],
        failure_at: Option<usize>,
    ) -> Result<(), &'static str> {
        mailbox[0..2].fill(0);
        mailbox[call_size..].fill(0);
        if transaction.body_present {
            for (index, byte) in transaction.body.iter().copied().enumerate() {
                if failure_at == Some(index) {
                    return Err("injected_semantic_surface_body_copy_failure");
                }
                mailbox[call_size + index] = byte;
            }
        }
        mailbox[2..call_size].copy_from_slice(&transaction.prefix[2..]);
        Ok(())
    }

    fn model_semantic_surface_publish(
        transaction: &prepared_semantic_surface_reply,
        mailbox: &mut [u8; semantic_surface_reply_span_size],
    ) {
        mailbox[0..2].copy_from_slice(&transaction.prefix[0..2]);
    }
}
