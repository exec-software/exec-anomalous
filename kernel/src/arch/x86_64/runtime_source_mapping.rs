#![cfg_attr(not(test), allow(dead_code))]

//! Populate and independently inspect a detached, read-only source prefix.
//!
//! This adapter grants no authority, publishes no mapping and releases no
//! ownership. Authentication and lifetime management belong to its caller.

use super::mapping_plan::{table_frame_capacity, user_address_limit};
use super::owned_mapping::{mapping_memory, staged_mapping};
use super::owned_source::{sealed_source_reader, source_memory};
use super::runtime_mapping::runtime_geometry;
use crate::memory::physical::physical_range;

const page_bytes: u64 = 4096;
const physical_mask: u64 = 0x000f_ffff_ffff_f000;
const parent_flags: u64 = 0x207;
const readonly_leaf_flags: u64 = 0x8000_0000_0000_0205;
const fnv_offset: u64 = 0xcbf2_9ce4_8422_2325;
const fnv_prime: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct population_observation {
    pub source_bytes: u64,
    /// Hashes cover the logical prefix only; zero slack is checked separately.
    pub source_fnv64: u64,
    pub destination_fnv64: u64,
    pub zero_bytes: u64,
    pub populated_pages: u64,
    /// Owned absent tail pages; both adjacent pages are also checked.
    pub absent_pages: u64,
    pub table_frames: u64,
}

/// Copy the genuine sealed prefix, read back every copied byte and all final
/// page slack, then independently walk every backed, absent and adjacent page.
/// Only this detached owner's single user/read-only/NX region is supported.
/// The hashes supplement exact comparison; they do not authenticate a source.
/// Both scratch buffers are volatile-wiped on every ordinary return, including
/// failure. Failure leaves a zero observation and may leave populated bytes.
///
/// # Safety
/// The caller must independently authenticate the operation and source, retain
/// the source and detached mapping owners, and hold exclusive physical access
/// to their disjoint extents throughout this call. The candidate must never
/// have been published and must remain inactive. No concurrent table, source,
/// destination or scratch mutation is permitted. The memory implementation's
/// access requirements must hold for the entire call. Scratch buffers must be
/// separate from all physical extents and from one another. Retain the owner
/// on every outcome until explicit cleanup succeeds; this helper cannot clean
/// up, publish, switch roots, or turn an observation into an access grant.
#[inline(never)]
pub(super) unsafe fn populate_and_verify(
    mapping: &staged_mapping,
    reader: &sealed_source_reader<'_>,
    geometry: runtime_geometry,
    memory: &mut (impl mapping_memory + source_memory),
    source_scratch: &mut [u8; 4096],
    readback_scratch: &mut [u8; 4096],
    observation: &mut population_observation,
) -> bool {
    *observation = population_observation::default();
    // SAFETY: The caller retains exclusive access and both owners until return.
    let result = unsafe {
        populate_inner(
            mapping,
            reader,
            geometry,
            memory,
            source_scratch,
            readback_scratch,
        )
    };
    wipe(source_scratch);
    wipe(readback_scratch);
    if let Some(result) = result {
        *observation = result;
        true
    } else {
        false
    }
}

unsafe fn populate_inner(
    mapping: &staged_mapping,
    reader: &sealed_source_reader<'_>,
    geometry: runtime_geometry,
    memory: &mut (impl mapping_memory + source_memory),
    source_scratch: &mut [u8; 4096],
    readback_scratch: &mut [u8; 4096],
) -> Option<population_observation> {
    let permissions = geometry.permissions();
    if !mapping.is_ready()
        || mapping.residency().is_some()
        || !permissions.user
        || permissions.writable
        || permissions.executable
        || geometry.start() < page_bytes
        || !geometry.start().is_multiple_of(page_bytes)
        || geometry.end() > user_address_limit
        || geometry.populated_byte_length() == 0
        || geometry.populated_byte_length() > reader.length()
    {
        return None;
    }
    let mut regions = mapping.plan().regions();
    let region = regions.next()?;
    if regions.next().is_some()
        || region.extent().start() != geometry.start()
        || region.extent().end() != geometry.populated_end()
        || region.permissions() != permissions
    {
        return None;
    }
    let mut reservations = mapping.plan().reservations();
    if geometry.populated_end() < geometry.end() {
        let tail = reservations.next()?;
        if tail.start() != geometry.populated_end() || tail.end() != geometry.end() {
            return None;
        }
    }
    if reservations.next().is_some() {
        return None;
    }
    let data = mapping.data_range(0)?;
    let tables = mapping.table_range()?;
    let root = mapping.root_address()?;
    if !valid_extent(data)
        || !valid_extent(tables)
        || tables.length / page_bytes > table_frame_capacity as u64
        || tables.length / page_bytes != mapping.plan().tables().len() as u64
        || root != tables.base
        || memory.active_root() & physical_mask == root
        || data.length != geometry.populated_end().checked_sub(geometry.start())?
        || data.length < geometry.populated_byte_length()
        || data.length - geometry.populated_byte_length() >= page_bytes
        || (data.base < tables.end()? && tables.base < data.end()?)
    {
        return None;
    }
    let mut result = population_observation {
        source_bytes: geometry.populated_byte_length(),
        source_fnv64: fnv_offset,
        destination_fnv64: fnv_offset,
        zero_bytes: data.length - geometry.populated_byte_length(),
        populated_pages: data.length / page_bytes,
        absent_pages: geometry.end().checked_sub(geometry.populated_end())? / page_bytes,
        table_frames: tables.length / page_bytes,
    };
    let mut offset = 0;
    while offset < result.source_bytes {
        let count = (result.source_bytes - offset).min(page_bytes) as usize;
        let destination = physical_range::new(data.base.checked_add(offset)?, count as u64);
        // SAFETY: The reader checks its sealed logical bounds. The destination
        // is within this owner's data lease and both buffers are exclusive.
        unsafe {
            reader
                .read(offset, &mut source_scratch[..count], memory)
                .ok()?;
            if !memory.write(destination, &source_scratch[..count])
                || !memory.read(destination, &mut readback_scratch[..count])
            {
                return None;
            }
        }
        if source_scratch[..count] != readback_scratch[..count] {
            return None;
        }
        result.source_fnv64 = fnv(result.source_fnv64, &source_scratch[..count]);
        result.destination_fnv64 = fnv(result.destination_fnv64, &readback_scratch[..count]);
        offset += count as u64;
    }
    if result.zero_bytes != 0 {
        let count = result.zero_bytes as usize;
        let slack = physical_range::new(data.base.checked_add(offset)?, result.zero_bytes);
        // SAFETY: All slack belongs to the final retained data page. Inspect it
        // without repairing it, so a failed initial clear remains observable.
        if !unsafe { memory.read(slack, &mut readback_scratch[..count]) }
            || readback_scratch[..count].iter().any(|byte| *byte != 0)
        {
            return None;
        }
    }
    let mut address = geometry.start();
    while address < geometry.end() {
        // SAFETY: Every followed table is checked against the owned arena
        // before access, without trusting the plan's table layout or indexes.
        let actual = unsafe { walk(memory, tables, root, address) }?;
        let expected = if address < geometry.populated_end() {
            Some(data.base.checked_add(address - geometry.start())?)
        } else {
            None
        };
        if actual != expected {
            return None;
        }
        address = address.checked_add(page_bytes)?;
    }
    // The upper boundary slot is also checked when end is the first address
    // outside the supported lower half. This is an absence check only; that
    // address is never treated as a valid user address or dereferenced.
    for address in [geometry.start() - page_bytes, geometry.end()] {
        // SAFETY: Only candidate-owned tables are read, including at boundaries.
        if unsafe { walk(memory, tables, root, address) }?.is_some() {
            return None;
        }
    }
    if memory.active_root() & physical_mask != root {
        Some(result)
    } else {
        None
    }
}

fn valid_extent(range: physical_range) -> bool {
    range.base != 0
        && range.base.is_multiple_of(page_bytes)
        && range.length != 0
        && range.length.is_multiple_of(page_bytes)
        && range
            .end()
            .is_some_and(|end| end <= physical_mask + page_bytes)
}

/// None is a failed inspection; Some(None) is a fully absent translation.
/// Exact fresh-tree flags reject huge/PAT pages, writable or executable leaves,
/// missing user access, and unexpected software/reserved/accessed/dirty bits.
unsafe fn walk(
    memory: &mut impl mapping_memory,
    arena: physical_range,
    root: u64,
    address: u64,
) -> Option<Option<u64>> {
    let arena_end = arena.end()?;
    let mut table = root;
    for shift in [39, 30, 21, 12] {
        if !table.is_multiple_of(page_bytes)
            || table < arena.base
            || table.checked_add(page_bytes)? > arena_end
        {
            return None;
        }
        let index = ((address >> shift) & 511) as usize;
        // SAFETY: The whole table page is inside the retained table arena and
        // the independently decoded index is within its 512 entries.
        let entry = unsafe { memory.read_entry(table, index) }?;
        if entry == 0 {
            return Some(None);
        }
        let flags = entry & !physical_mask;
        if shift == 12 {
            return (flags == readonly_leaf_flags).then_some(Some(entry & physical_mask));
        }
        if flags != parent_flags {
            return None;
        }
        table = entry & physical_mask;
    }
    None
}

fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(fnv_prime);
    }
    hash
}

fn wipe(buffer: &mut [u8; 4096]) {
    for byte in buffer {
        // SAFETY: Each byte is exclusively borrowed, initialized and writable.
        // Volatile stores retain the clearing operation after the final read.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}

#[cfg(test)]
mod tests;
