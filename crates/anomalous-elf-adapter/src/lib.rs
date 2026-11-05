#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Bounded ELF64 parsing for a userspace adapter, with no allocation or authority.
//!
//! This accepts a sectionless deployment subset, not arbitrary ELF or a Linux
//! execution personality. The immutable result describes requested file and
//! virtual extents only. It owns no pages, authenticates no source, grants no
//! rights, and does not establish instruction safety or runtime compatibility.
//! The kernel may decode and independently admit the inert plan; ELF parsing
//! and executable/personality policy remain in the userspace adapter.

/// Separate Linux image inspection; does not widen the qualified ANEL profile.
#[cfg(feature = "linux_image")]
pub mod linux_image;
#[cfg(feature = "linux_image")]
pub mod linux_startup;

pub const encoded_plan_size: usize = 96;
const header_end: u64 = 64 + 2 * 56;
const page_size: u64 = 4096;

/// The bounds a deployment image is measured against.
///
/// These were module constants, which was right while there was one kind of
/// image. ADR 0092 introduces a second: an interpreter is a far larger program
/// than a native application and carries an edition besides, so it cannot fit
/// geometry chosen for a 32 KiB task.
///
/// The answer is a named profile rather than wider constants. Widening the
/// qualified one would re-measure every image already admitted under it
/// against bounds nobody qualified, and the value of that profile is precisely
/// that it is small enough to have been reasoned about. A second profile
/// leaves the first untouched.
///
/// A profile is not a way to opt out of bounds. Every field is still a bound,
/// `fits` still refuses anything outside it, and a profile whose limits are
/// not page-aligned or whose regions overlap is rejected before it can admit
/// anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct deployment_profile {
    pub image_limit: u64,
    pub rx_base: u64,
    pub rw_base: u64,
    pub rx_limit: u64,
    pub rw_limit: u64,
}

/// The qualified profile: a first-party native application, unchanged.
///
/// Every existing image is admitted by exactly the bounds it always was.
pub const anel_v1: deployment_profile = deployment_profile {
    image_limit: 65536,
    rx_base: 0x400000,
    rw_base: 0x800000,
    rx_limit: 8 * page_size,
    rw_limit: 2 * page_size,
};

impl deployment_profile {
    /// Is this profile itself coherent?
    ///
    /// Asked before a profile is used to admit anything, because a profile is
    /// the thing doing the bounding: one whose regions overlap or whose limits
    /// are unaligned would admit images nobody could reason about, and the
    /// failure would look like the image's fault.
    pub const fn is_coherent(&self) -> bool {
        self.image_limit >= header_end
            && self.rx_base.is_multiple_of(page_size)
            && self.rw_base.is_multiple_of(page_size)
            && self.rx_limit > 0
            && self.rw_limit > 0
            && self.rx_limit.is_multiple_of(page_size)
            && self.rw_limit.is_multiple_of(page_size)
            && self.rx_base < self.rw_base
            && match self.rx_base.checked_add(self.rx_limit) {
                Some(rx_end) => rx_end <= self.rw_base,
                None => false,
            }
            && self.rw_base.checked_add(self.rw_limit).is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum elf_error {
    truncated,
    image_size,
    elf_header,
    program_header,
    load_geometry,
    file_extent,
    nonzero_padding,
    plan_encoding,
}

/// Inert extent, not a dereferenceable address or a grant to allocate/map it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct load_segment {
    file_offset: u64,
    file_size: u64,
    virtual_address: u64,
    memory_size: u64,
}

impl load_segment {
    pub const fn file_offset(self) -> u64 {
        self.file_offset
    }

    pub const fn file_size(self) -> u64 {
        self.file_size
    }

    pub const fn virtual_address(self) -> u64 {
        self.virtual_address
    }

    pub const fn memory_size(self) -> u64 {
        self.memory_size
    }
}

/// Fixed native profile: RX then RW, plus independently owned/guarded stack.
/// Permissions are implied by these two roles and cannot be widened by wire
/// fields. Copies still require a retained immutable source of image_length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct load_plan {
    entry: u64,
    rx: load_segment,
    rw: load_segment,
    image_length: u64,
}

impl load_plan {
    pub const fn entry(self) -> u64 {
        self.entry
    }

    pub const fn image_length(self) -> u64 {
        self.image_length
    }

    pub const fn rx(self) -> load_segment {
        self.rx
    }

    pub const fn rw(self) -> load_segment {
        self.rw
    }

    /// Explicit little-endian representation; never serialize Rust layout.
    pub fn encode(self) -> [u8; encoded_plan_size] {
        let mut bytes = [0; encoded_plan_size];
        bytes[..4].copy_from_slice(b"ANEL");
        bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
        bytes[6..8].copy_from_slice(&(encoded_plan_size as u16).to_le_bytes());
        for (offset, value) in [
            (8, self.entry),
            (16, self.rx.file_offset),
            (24, self.rx.file_size),
            (32, self.rx.virtual_address),
            (40, self.rx.memory_size),
            (48, self.rw.file_offset),
            (56, self.rw.file_size),
            (64, self.rw.virtual_address),
            (72, self.rw.memory_size),
            (80, self.image_length),
        ] {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    /// Validate only the canonical wire and bounded load geometry. This does
    /// not parse ELF, inspect segment contents/gaps, or authenticate the source.
    /// The consumer must bind the complete plan to its independently retained
    /// source and enforce native authority, population, and lifetime rules.
    /// Decode a plan admitted by the qualified profile.
    ///
    /// Unchanged behaviour for every caller: `anel_v1` is the geometry this
    /// has always enforced.
    pub fn decode(bytes: &[u8]) -> Result<Self, elf_error> {
        Self::decode_for(bytes, anel_v1)
    }

    /// Decode a plan admitted by a named profile.
    pub fn decode_for(bytes: &[u8], profile: deployment_profile) -> Result<Self, elf_error> {
        if !profile.is_coherent() {
            return Err(elf_error::load_geometry);
        }
        if bytes.len() != encoded_plan_size
            || bytes.get(..4) != Some(b"ANEL")
            || read_u16(bytes, 4)? != 1
            || read_u16(bytes, 6)? != encoded_plan_size as u16
            || read_u64(bytes, 88)? != 0
        {
            return Err(elf_error::plan_encoding);
        }
        let plan = Self {
            entry: read_u64(bytes, 8)?,
            rx: read_segment(bytes, 16)?,
            rw: read_segment(bytes, 48)?,
            image_length: read_u64(bytes, 80)?,
        };
        plan.validate(profile)?;
        Ok(plan)
    }

    fn validate(self, profile: deployment_profile) -> Result<(), elf_error> {
        if !profile.is_coherent() {
            return Err(elf_error::load_geometry);
        }
        if self.image_length < header_end || self.image_length > profile.image_limit {
            return Err(elf_error::image_size);
        }
        if self.entry != profile.rx_base
            || self.rx.virtual_address != profile.rx_base
            || self.rx.file_size == 0
            || self.rx.file_size != self.rx.memory_size
            || self.rx.memory_size > profile.rx_limit
            || self.rw.virtual_address != profile.rw_base
            || self.rw.file_size < 128
            || self.rw.file_size > self.rw.memory_size
            || self.rw.memory_size > profile.rw_limit
        {
            return Err(elf_error::load_geometry);
        }
        for segment in [self.rx, self.rw] {
            if !segment.file_offset.is_multiple_of(page_size)
                || segment.file_offset < header_end
                || segment
                    .virtual_address
                    .checked_add(segment.memory_size)
                    .is_none()
            {
                return Err(elf_error::load_geometry);
            }
        }
        let rx_end = self.rx.file_offset.checked_add(self.rx.file_size);
        let rw_end = self.rw.file_offset.checked_add(self.rw.file_size);
        // Fixed bases and bounded sizes also make their complete rounded page
        // extents disjoint, below the canonical boundary and away from page0.
        if rx_end.is_none_or(|end| end > self.rw.file_offset) || rw_end != Some(self.image_length) {
            return Err(elf_error::file_extent);
        }
        Ok(())
    }
}

/// Parse the complete immutable deployment ELF in the unprivileged adapter.
/// Exactly two ascending PT_LOADs are supported; all gaps outside the ELF/PHT
/// and file-backed segments must be zero, and no trailing bytes are admitted.
/// p_paddr is deliberately ignored: it does not select physical memory.
/// Parse an image admitted by the qualified profile.
///
/// Unchanged behaviour for every caller.
pub fn parse(bytes: &[u8]) -> Result<load_plan, elf_error> {
    parse_for(bytes, anel_v1)
}

/// Parse an image admitted by a named profile.
///
/// The profile bounds the image; it does not relax what an image must look
/// like. Sectionless, fixed entry, zero padding, no implied authority: all of
/// that is the same whatever the profile says about size.
pub fn parse_for(bytes: &[u8], profile: deployment_profile) -> Result<load_plan, elf_error> {
    // The profile is judged before the image is. An incoherent profile cannot
    // bound anything, and reporting the image's size first would name the
    // wrong fault -- the image is fine; the thing measuring it is not.
    if !profile.is_coherent() {
        return Err(elf_error::load_geometry);
    }
    if bytes.len() as u64 > profile.image_limit {
        return Err(elf_error::image_size);
    }
    if bytes.len() < header_end as usize {
        return Err(elf_error::truncated);
    }
    if bytes.get(..7) != Some(b"\x7fELF\x02\x01\x01")
        || bytes[7..16].iter().any(|byte| *byte != 0)
        || read_u16(bytes, 16)? != 2
        || read_u16(bytes, 18)? != 62
        || read_u32(bytes, 20)? != 1
        || read_u64(bytes, 32)? != 64
        || read_u64(bytes, 40)? != 0
        || read_u32(bytes, 48)? != 0
        || read_u16(bytes, 52)? != 64
        || read_u16(bytes, 54)? != 56
        || read_u16(bytes, 56)? != 2
        || read_u16(bytes, 58)? != 0
        || read_u16(bytes, 60)? != 0
        || read_u16(bytes, 62)? != 0
    {
        return Err(elf_error::elf_header);
    }
    let mut segments = [load_segment {
        file_offset: 0,
        file_size: 0,
        virtual_address: 0,
        memory_size: 0,
    }; 2];
    for (index, segment) in segments.iter_mut().enumerate() {
        let offset = 64 + index * 56;
        if read_u32(bytes, offset)? != 1
            || read_u32(bytes, offset + 4)? != if index == 0 { 5 } else { 6 }
            || read_u64(bytes, offset + 48)? != page_size
        {
            return Err(elf_error::program_header);
        }
        *segment = load_segment {
            file_offset: read_u64(bytes, offset + 8)?,
            file_size: read_u64(bytes, offset + 32)?,
            virtual_address: read_u64(bytes, offset + 16)?,
            memory_size: read_u64(bytes, offset + 40)?,
        };
    }
    let plan = load_plan {
        entry: read_u64(bytes, 24)?,
        rx: segments[0],
        rw: segments[1],
        image_length: bytes.len() as u64,
    };
    plan.validate(profile)?;
    // All four offsets are now bounded by image_length == bytes.len(), and
    // every addition was checked. No file-provided address is dereferenced.
    let rx_start = plan.rx.file_offset as usize;
    let rx_end = rx_start + plan.rx.file_size as usize;
    let rw_start = plan.rw.file_offset as usize;
    if bytes[header_end as usize..rx_start]
        .iter()
        .chain(bytes[rx_end..rw_start].iter())
        .any(|byte| *byte != 0)
    {
        return Err(elf_error::nonzero_padding);
    }
    Ok(plan)
}

fn read_segment(bytes: &[u8], offset: usize) -> Result<load_segment, elf_error> {
    Ok(load_segment {
        file_offset: read_u64(bytes, offset)?,
        file_size: read_u64(bytes, offset + 8)?,
        virtual_address: read_u64(bytes, offset + 16)?,
        memory_size: read_u64(bytes, offset + 24)?,
    })
}

fn read_array<const size: usize>(bytes: &[u8], offset: usize) -> Result<[u8; size], elf_error> {
    let end = offset.checked_add(size).ok_or(elf_error::truncated)?;
    bytes
        .get(offset..end)
        .and_then(|value| value.try_into().ok())
        .ok_or(elf_error::truncated)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, elf_error> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, elf_error> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, elf_error> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::{vec, vec::Vec};

    fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn fixture_at(
        rx_offset: usize,
        rx_size: usize,
        rw_offset: usize,
        rw_size: usize,
        rw_memory: usize,
    ) -> Vec<u8> {
        let mut bytes = vec![0; rw_offset + rw_size];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        put_u16(&mut bytes, 16, 2);
        put_u16(&mut bytes, 18, 62);
        put_u32(&mut bytes, 20, 1);
        put_u64(&mut bytes, 24, 0x400000);
        put_u64(&mut bytes, 32, 64);
        put_u16(&mut bytes, 52, 64);
        put_u16(&mut bytes, 54, 56);
        put_u16(&mut bytes, 56, 2);
        for (offset, flags, file_offset, file_size, address, memory_size) in [
            (64, 5, rx_offset, rx_size, 0x400000, rx_size),
            (120, 6, rw_offset, rw_size, 0x800000, rw_memory),
        ] {
            put_u32(&mut bytes, offset, 1);
            put_u32(&mut bytes, offset + 4, flags);
            put_u64(&mut bytes, offset + 8, file_offset as u64);
            put_u64(&mut bytes, offset + 16, address);
            put_u64(&mut bytes, offset + 24, address);
            put_u64(&mut bytes, offset + 32, file_size as u64);
            put_u64(&mut bytes, offset + 40, memory_size as u64);
            put_u64(&mut bytes, offset + 48, 4096);
        }
        bytes[rx_offset..rx_offset + rx_size].fill(0x90);
        // Nonzero initialized RW bytes are data to copy, not padding to reject.
        bytes[rw_offset..rw_offset + rw_size].fill(0x5a);
        bytes
    }

    fn fixture() -> Vec<u8> {
        fixture_at(0x1000, 0x2130, 0x4000, 128, 0x1da0)
    }

    fn rejected_without_mutation(bytes: &[u8]) {
        let before = bytes.to_vec();
        assert!(parse(bytes).is_err());
        assert_eq!(bytes, before);
    }

    #[test]
    fn deployment_plan_has_independent_golden_wire_and_round_trips() {
        let bytes = fixture();
        let before = bytes.clone();
        let plan = parse(&bytes).unwrap();
        assert_eq!(plan.entry(), 0x400000);
        assert_eq!(plan.image_length(), 0x4080);
        assert_eq!(plan.rx().file_offset(), 0x1000);
        assert_eq!(plan.rx().file_size(), 0x2130);
        assert_eq!(plan.rx().virtual_address(), 0x400000);
        assert_eq!(plan.rx().memory_size(), 0x2130);
        assert_eq!(plan.rw().file_offset(), 0x4000);
        assert_eq!(plan.rw().file_size(), 128);
        assert_eq!(plan.rw().virtual_address(), 0x800000);
        assert_eq!(plan.rw().memory_size(), 0x1da0);
        let golden = [
            0x41, 0x4e, 0x45, 0x4c, 0x01, 0x00, 0x60, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x21, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x21,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xa0, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x40, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(plan.encode(), golden);
        assert_eq!(load_plan::decode(&golden), Ok(plan));
        assert_eq!(bytes, before);
    }

    #[test]
    fn physical_address_and_payload_contents_are_not_authority_or_instruction_admission() {
        let mut bytes = fixture();
        let plan = parse(&bytes).unwrap();
        for physical in [0, 0x100000, 0xffff_ffff_ffff_f000, u64::MAX] {
            put_u64(&mut bytes, 88, physical);
            put_u64(&mut bytes, 144, physical);
            assert_eq!(parse(&bytes), Ok(plan));
        }
        bytes[0x1000] = 0xff;
        bytes[0x407f] = 0xab;
        assert_eq!(parse(&bytes), Ok(plan));
        // Decode admits bounded intent without proving that the declared BSS
        // matches a particular source. The consumer must bind its own manifest.
        let mut wire = plan.encode();
        put_u64(&mut wire, 72, 8192);
        let changed = load_plan::decode(&wire).unwrap();
        assert_ne!(changed, plan);
        assert_eq!(changed.rw().memory_size(), 8192);
        // At the maximum bound the loader can still offer a different valid
        // geometry; only the independently retained source manifest rejects it.
        let mut smaller = changed.encode();
        put_u64(&mut smaller, 72, 8191);
        let smaller = load_plan::decode(&smaller).unwrap();
        assert_ne!(smaller, changed);
        assert_eq!(smaller.rw().memory_size(), 8191);
    }

    #[test]
    fn exact_profile_limits_accept_non_page_sized_code_and_full_capacity() {
        for (rx_size, rw_size, rw_memory) in [(1, 128, 128), (4097, 129, 8192), (32768, 8192, 8192)]
        {
            let rw_offset = (4096 + rx_size + 4095) & !4095;
            let bytes = fixture_at(4096, rx_size, rw_offset, rw_size, rw_memory);
            let plan = parse(&bytes).unwrap();
            assert_eq!(plan.rx().file_size(), rx_size as u64);
            assert_eq!(plan.rw().memory_size(), rw_memory as u64);
            assert_eq!(load_plan::decode(&plan.encode()), Ok(plan));
        }
        let largest_file = fixture_at(0x1000, 1, 0xf000, 4096, 8192);
        assert_eq!(largest_file.len(), 65536);
        assert_eq!(parse(&largest_file).unwrap().image_length(), 65536);
        let mut oversized = largest_file;
        oversized.push(0);
        assert_eq!(parse(&oversized), Err(elf_error::image_size));
    }

    #[test]
    fn every_truncation_and_any_trailing_byte_are_rejected_without_changing_input() {
        let bytes = fixture();
        let accepted = parse(&bytes).unwrap();
        for length in 0..bytes.len() {
            assert!(parse(&bytes[..length]).is_err(), "accepted length {length}");
        }
        for byte in [0, 1, 0xff] {
            let mut changed = bytes.clone();
            changed.push(byte);
            rejected_without_mutation(&changed);
        }
        assert_eq!(parse(&bytes), Ok(accepted));
    }

    #[test]
    fn unsupported_elf_headers_and_extended_tables_are_rejected() {
        let bytes = fixture();
        for offset in 0..16 {
            let mut changed = bytes.clone();
            changed[offset] ^= 1;
            rejected_without_mutation(&changed);
        }
        for (offset, value) in [
            (16, 1),
            (16, 3),
            (18, 3),
            (52, 0),
            (52, 63),
            (52, 65),
            (54, 0),
            (54, 55),
            (54, 64),
            (56, 0),
            (56, 1),
            (56, 3),
            (56, 0xffff),
            (58, 64),
            (60, 1),
            (62, 1),
            (62, 0xffff),
        ] {
            let mut changed = bytes.clone();
            put_u16(&mut changed, offset, value);
            rejected_without_mutation(&changed);
        }
        for (offset, value) in [(20, 0), (20, 2), (48, 1), (48, u32::MAX)] {
            let mut changed = bytes.clone();
            put_u32(&mut changed, offset, value);
            rejected_without_mutation(&changed);
        }
        for (offset, value) in [(32, 0), (32, 63), (32, u64::MAX), (40, 64), (40, u64::MAX)] {
            let mut changed = bytes.clone();
            put_u64(&mut changed, offset, value);
            rejected_without_mutation(&changed);
        }
    }

    #[test]
    fn program_types_permissions_alignment_and_order_cannot_widen_the_profile() {
        let bytes = fixture();
        for offset in [64, 120] {
            for kind in [0, 2, 3, 4, 5, 6, 7, 0x6474_e551, u32::MAX] {
                let mut changed = bytes.clone();
                put_u32(&mut changed, offset, kind);
                rejected_without_mutation(&changed);
            }
            for flags in [0, 1, 2, 3, 4, 7, 0x1000_0005, u32::MAX] {
                let mut changed = bytes.clone();
                put_u32(&mut changed, offset + 4, flags);
                rejected_without_mutation(&changed);
            }
            for alignment in [0, 1, 2048, 4095, 8192, u64::MAX] {
                let mut changed = bytes.clone();
                put_u64(&mut changed, offset + 48, alignment);
                rejected_without_mutation(&changed);
            }
        }
        let mut changed = bytes.clone();
        changed[64..120].copy_from_slice(&bytes[120..176]);
        changed[120..176].copy_from_slice(&bytes[64..120]);
        rejected_without_mutation(&changed);
        changed[120..176].copy_from_slice(&bytes[120..176]);
        rejected_without_mutation(&changed); // Duplicated RW header.
    }

    #[test]
    fn segment_geometry_overflow_overlap_and_entry_outside_initialized_code_are_rejected() {
        let bytes = fixture();
        for (offset, value) in [
            (24, 0),
            (24, 0x400001),
            (24, 0x402130),
            (24, 0x403000),
            (24, 0x800000),
            (24, u64::MAX),
            (72, 0),
            (72, 64),
            (72, 176),
            (72, 4095),
            (72, 4097),
            (72, 0xffff_ffff_ffff_f000),
            (80, 0),
            (80, 0x100000),
            (80, 0x400001),
            (80, 0x800000),
            (80, 1 << 47),
            (80, u64::MAX),
            (96, 0),
            (96, 32769),
            (96, u64::MAX),
            (104, 0),
            (104, 0x212f),
            (104, 0x2131),
            (104, 32769),
            (104, u64::MAX),
            (128, 0),
            (128, 0x1000),
            (128, 0x2000),
            (128, 0x4001),
            (128, 0xffff_ffff_ffff_f000),
            (136, 0),
            (136, 0x100000),
            (136, 0x400000),
            (136, 0x800001),
            (136, u64::MAX),
            (152, 0),
            (152, 127),
            (152, 0x1da1),
            (152, 8193),
            (152, u64::MAX),
            (160, 0),
            (160, 127),
            (160, 8193),
            (160, u64::MAX),
        ] {
            let mut changed = bytes.clone();
            put_u64(&mut changed, offset, value);
            assert!(parse(&changed).is_err(), "accepted field {offset}={value}");
        }
        // RX's full capacity is valid, but one additional byte is not even
        // when file and memory sizes agree and all other geometry is coherent.
        rejected_without_mutation(&fixture_at(4096, 32769, 40960, 128, 8192));
    }

    #[test]
    fn only_declared_load_bytes_may_be_nonzero_outside_header_tables() {
        let bytes = fixture();
        for offset in [176, 0x1000 - 1, 0x3130, 0x4000 - 1] {
            let mut changed = bytes.clone();
            changed[offset] = 1;
            assert_eq!(parse(&changed), Err(elf_error::nonzero_padding));
        }
        let plan = parse(&bytes).unwrap();
        for offset in [0x1000, 0x3130 - 1, 0x4000, 0x4080 - 1] {
            let mut changed = bytes.clone();
            changed[offset] ^= 0xff;
            assert_eq!(parse(&changed), Ok(plan));
        }
    }

    #[test]
    fn wire_rejects_corrupt_dimensions_reserved_bytes_and_unbounded_or_overlapping_plans() {
        let plan = parse(&fixture()).unwrap();
        let wire = plan.encode();
        for length in 0..wire.len() {
            assert_eq!(
                load_plan::decode(&wire[..length]),
                Err(elf_error::plan_encoding)
            );
        }
        let mut extra = wire.to_vec();
        extra.push(0);
        assert_eq!(load_plan::decode(&extra), Err(elf_error::plan_encoding));
        for offset in (0..8).chain(88..96) {
            let mut changed = wire;
            changed[offset] ^= 1;
            let before = changed;
            assert!(load_plan::decode(&changed).is_err());
            assert_eq!(changed, before);
        }
        for offset in (8..88).step_by(8) {
            let mut changed = wire;
            put_u64(&mut changed, offset, u64::MAX);
            assert!(load_plan::decode(&changed).is_err());
        }
        for (offset, value) in [
            (8, 0x400001),
            (16, 0),
            (16, 4097),
            (16, 0xffff_ffff_ffff_f000),
            (24, 0),
            (24, 32769),
            (32, 0x800000),
            (40, 0x2131),
            (48, 0x2000),
            (48, 0xffff_ffff_ffff_f000),
            (56, 127),
            (56, 8193),
            (64, 0x400000),
            (72, 127),
            (72, 8193),
            (80, 0x407f),
            (80, 0x4081),
            (80, 65537),
        ] {
            let mut changed = wire;
            put_u64(&mut changed, offset, value);
            assert!(
                load_plan::decode(&changed).is_err(),
                "accepted field {offset}={value}"
            );
        }
        assert_eq!(load_plan::decode(&wire), Ok(plan));
    }

    #[test]
    fn a_profile_that_bounds_nothing_coherently_admits_nothing() {
        // A profile is the thing doing the bounding, so an incoherent one must
        // be refused before it can admit an image. Otherwise the failure looks
        // like the image's fault, and an overlapping or unaligned profile
        // would quietly let two regions share memory.
        let overlapping = deployment_profile {
            rw_base: anel_v1.rx_base,
            ..anel_v1
        };
        let touching = deployment_profile {
            rw_base: anel_v1.rx_base + anel_v1.rx_limit - page_size,
            ..anel_v1
        };
        let unaligned = deployment_profile {
            rx_base: anel_v1.rx_base + 1,
            ..anel_v1
        };
        let unaligned_limit = deployment_profile {
            rx_limit: anel_v1.rx_limit + 1,
            ..anel_v1
        };
        let empty = deployment_profile {
            rx_limit: 0,
            ..anel_v1
        };
        let tiny = deployment_profile {
            image_limit: 8,
            ..anel_v1
        };
        let inverted = deployment_profile {
            rx_base: anel_v1.rw_base,
            rw_base: anel_v1.rx_base,
            ..anel_v1
        };
        for bad in [
            overlapping,
            touching,
            unaligned,
            unaligned_limit,
            empty,
            tiny,
            inverted,
        ] {
            assert!(
                !bad.is_coherent(),
                "{bad:?} is not a profile anything should be measured against"
            );
            let image = fixture_at(4096, 64, 8192, 128, 128);
            assert_eq!(parse_for(&image, bad), Err(elf_error::load_geometry));
        }
        assert!(anel_v1.is_coherent());
    }

    #[test]
    fn a_wider_profile_admits_what_the_qualified_one_refuses_and_no_more() {
        // The qualified profile is 8 RX pages. An image needing nine is refused
        // by it and admitted by a profile that says so -- which is the whole
        // point: the bound moves because someone named a new one, not because
        // the old one grew.
        let wide = deployment_profile {
            image_limit: 1 << 20,
            rx_limit: 64 * page_size,
            rw_limit: 16 * page_size,
            ..anel_v1
        };
        assert!(wide.is_coherent());

        let big = fixture_at(
            4096,
            9 * page_size as usize,
            4096 + 9 * page_size as usize,
            128,
            128,
        );
        assert_eq!(
            parse(&big),
            Err(elf_error::load_geometry),
            "the qualified profile still refuses it"
        );
        assert!(parse_for(&big, wide).is_ok(), "the wider profile admits it");

        // And the wider profile is still a bound, not an opening.
        let bigger = fixture_at(
            4096,
            65 * page_size as usize,
            4096 + 65 * page_size as usize,
            128,
            128,
        );
        assert_eq!(parse_for(&bigger, wide), Err(elf_error::load_geometry));
    }

    #[test]
    fn the_qualified_profile_is_what_parse_has_always_enforced() {
        // parse() and parse_for(anel_v1) must be the same function, or every
        // image admitted before this change is being measured against
        // something nobody qualified.
        for (rx_offset, rx_size, rw_offset, rw_size, rw_memory) in [
            (4096usize, 64usize, 8192usize, 128usize, 128usize),
            (
                4096,
                8 * page_size as usize,
                4096 + 8 * page_size as usize,
                128,
                2 * page_size as usize,
            ),
            (
                4096,
                9 * page_size as usize,
                4096 + 9 * page_size as usize,
                128,
                128,
            ),
            (4096, 64, 8192, 64, 64),
        ] {
            let image = fixture_at(rx_offset, rx_size, rw_offset, rw_size, rw_memory);
            assert_eq!(parse(&image), parse_for(&image, anel_v1));
        }
    }
}
