extern crate std;

use super::super::mapping_plan::{mapping_budget, mapping_request, reserved_request};
use super::super::owned_mapping::{mapping_error, mapping_memory};
use super::super::owned_source::{
    owned_source, sealed_source_reader, source_budget, source_memory,
};
use super::*;
use crate::boot_omen::memory_region;
use crate::memory::physical::{frame_accounting, frame_allocator, frame_error, physical_range};
use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::{vec, vec::Vec};

const page: usize = 4096;
const base: u64 = 0x2000_0000;
const read_only: page_permissions = page_permissions {
    user: true,
    writable: false,
    executable: false,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct access_counts {
    clears: usize,
    table_reads: usize,
    table_writes: usize,
    data_reads: usize,
    data_writes: usize,
}

struct model_memory {
    bytes: Vec<u8>,
    active: u64,
    calls: access_counts,
    fail: access_counts,
    corrupt_data_write: usize,
    read_extents: Vec<physical_range>,
}

impl model_memory {
    fn new(frames: usize) -> Self {
        Self {
            bytes: vec![0xa5; frames * page],
            active: 0,
            calls: access_counts::default(),
            fail: access_counts::default(),
            corrupt_data_write: 0,
            read_extents: Vec::new(),
        }
    }

    fn bounds(&self, range: physical_range) -> Option<core::ops::Range<usize>> {
        let start = usize::try_from(range.base).ok()?;
        let end = usize::try_from(range.end()?).ok()?;
        (start < end && end <= self.bytes.len()).then_some(start..end)
    }

    fn word(&self, table: u64, index: usize) -> Option<core::ops::Range<usize>> {
        if !table.is_multiple_of(4096) || index >= 512 {
            return None;
        }
        self.bounds(physical_range::new(
            table.checked_add((index * 8) as u64)?,
            8,
        ))
    }

    fn clear(&mut self, range: physical_range) -> bool {
        self.calls.clears += 1;
        let Some(bounds) = self.bounds(range) else {
            return false;
        };
        if self.fail.clears == self.calls.clears {
            let middle = bounds.start + bounds.len().div_ceil(2);
            self.bytes[bounds.start..middle].fill(0);
            return false;
        }
        self.bytes[bounds.clone()].fill(0);
        self.bytes[bounds].iter().all(|byte| *byte == 0)
    }

    fn reset_faults(&mut self) {
        self.fail = access_counts::default();
        self.corrupt_data_write = 0;
    }

    fn snapshot(&self, range: physical_range) -> Vec<u8> {
        self.bytes[self.bounds(range).unwrap()].to_vec()
    }

    fn raw_entry(&self, table: u64, index: usize) -> u64 {
        u64::from_le_bytes(
            self.bytes[self.word(table, index).unwrap()]
                .try_into()
                .unwrap(),
        )
    }
}

// SAFETY: All physical addresses are offsets in one isolated byte vector.
// Complete bounds are checked before access, clears verify the whole extent,
// and injected failures never claim a successful partial operation.
unsafe impl mapping_memory for model_memory {
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        self.clear(range)
    }

    unsafe fn read_entry(&mut self, table: u64, index: usize) -> Option<u64> {
        self.calls.table_reads += 1;
        if self.fail.table_reads == self.calls.table_reads {
            return None;
        }
        let bounds = self.word(table, index)?;
        Some(u64::from_le_bytes(self.bytes[bounds].try_into().ok()?))
    }

    unsafe fn write_entry(&mut self, table: u64, index: usize, value: u64) -> bool {
        self.calls.table_writes += 1;
        let Some(bounds) = self.word(table, index) else {
            return false;
        };
        self.bytes[bounds].copy_from_slice(&value.to_le_bytes());
        self.fail.table_writes != self.calls.table_writes
    }

    fn active_root(&self) -> u64 {
        self.active
    }
}

// SAFETY: Source and destination ranges use the same isolated, checked RAM.
// Failed reads/writes affect only a prefix of the supplied destination.
unsafe impl source_memory for model_memory {
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        self.clear(range)
    }

    unsafe fn write(&mut self, range: physical_range, source: &[u8]) -> bool {
        self.calls.data_writes += 1;
        let Some(bounds) = self.bounds(range) else {
            return false;
        };
        if bounds.len() != source.len() {
            return false;
        }
        if self.fail.data_writes == self.calls.data_writes {
            let count = source.len().div_ceil(2);
            self.bytes[bounds.start..bounds.start + count].copy_from_slice(&source[..count]);
            return false;
        }
        self.bytes[bounds.clone()].copy_from_slice(source);
        if self.corrupt_data_write == self.calls.data_writes {
            self.bytes[bounds.end - 1] ^= 1;
        }
        true
    }

    unsafe fn read(&mut self, range: physical_range, destination: &mut [u8]) -> bool {
        self.calls.data_reads += 1;
        self.read_extents.push(range);
        let Some(bounds) = self.bounds(range) else {
            return false;
        };
        if bounds.len() != destination.len() {
            return false;
        }
        if self.fail.data_reads == self.calls.data_reads {
            let count = destination.len().div_ceil(2);
            destination[..count].copy_from_slice(&self.bytes[bounds.start..bounds.start + count]);
            return false;
        }
        destination.copy_from_slice(&self.bytes[bounds]);
        true
    }
}

fn allocator(ledger: &mut [u8], frames: usize) -> frame_allocator<'_> {
    frame_allocator::initialize(
        ledger,
        &[memory_region {
            base: 0,
            length: (frames * page) as u64,
            kind: 1,
            attributes: 1,
        }],
        &[physical_range::new(0, page as u64)],
    )
    .unwrap()
}

fn existing() -> mapping_plan {
    existing_at(0x40_0000, 2, &[])
}

fn existing_at(address: u64, pages: u64, reservations: &[reserved_request]) -> mapping_plan {
    mapping_plan::new(
        &[mapping_request {
            virtual_address: address,
            page_count: pages,
            permissions: read_only,
        }],
        reservations,
        mapping_budget {
            data_pages: 1024,
            table_frames: 32,
        },
    )
    .unwrap()
}

fn request(bytes: u64, populated: u64) -> runtime_request {
    runtime_request {
        virtual_address: base,
        byte_length: bytes,
        populated_byte_length: populated,
        permissions: read_only,
    }
}

fn budget() -> runtime_budget {
    runtime_budget {
        virtual_pages: 1024,
        data_pages: 1024,
        table_frames: 32,
    }
}

fn small_plan() -> runtime_plan {
    runtime_plan::new(&existing(), request(3 * 4096 - 27, 4097), budget()).unwrap()
}

fn source_byte(offset: usize) -> u8 {
    ((offset.wrapping_mul(37) ^ (offset >> 7) ^ (offset >> 15)) & 255) as u8
}

fn sealed_source(
    allocator: &mut frame_allocator<'_>,
    memory: &mut model_memory,
    length: usize,
) -> owned_source {
    // SAFETY: This host fixture is the only provisioner of its isolated RAM.
    let mut source = unsafe {
        owned_source::new_provisioned(
            73,
            length as u64,
            source_budget {
                max_bytes: length as u64,
                max_pages: length.div_ceil(page),
            },
        )
    }
    .unwrap();
    unsafe { source.allocate(allocator, memory) }.unwrap();
    let mut chunk = [0; page];
    for start in (0..length).step_by(page) {
        let count = (length - start).min(page);
        for (index, byte) in chunk[..count].iter_mut().enumerate() {
            *byte = source_byte(start + index);
        }
        unsafe { source.append_provisioned(73, start as u64, &chunk[..count], memory) }.unwrap();
    }
    unsafe { source.seal_provisioned(73) }.unwrap();
    source
}

// The runtime owner is source-agnostic. This trusted host population adapter
// bounds each read using the retained reader, then verifies the copied bytes
// and zero page slack separately. It is not a Linux fd or mmap implementation.
fn populate(
    mapping: &staged_mapping,
    reader: &sealed_source_reader<'_>,
    memory: &mut model_memory,
) -> bool {
    let Some(destination) = mapping.data_range(0) else {
        return false;
    };
    if reader.length() > destination.length {
        return false;
    }
    let mut scratch = [0; page];
    let mut observed = [0; page];
    let length = reader.length() as usize;
    for offset in (0..length).step_by(page) {
        let count = (length - offset).min(page);
        // SAFETY: Sealed source and retained destination leases are disjoint;
        // the model is exclusively borrowed for the synchronous operation.
        if unsafe { reader.read(offset as u64, &mut scratch[..count], memory) }.is_err()
            || !unsafe {
                source_memory::write(
                    memory,
                    physical_range::new(destination.base + offset as u64, count as u64),
                    &scratch[..count],
                )
            }
            || !unsafe {
                source_memory::read(
                    memory,
                    physical_range::new(destination.base + offset as u64, count as u64),
                    &mut observed[..count],
                )
            }
            || observed[..count] != scratch[..count]
        {
            return false;
        }
    }
    let tail = physical_range::new(
        destination.base + reader.length(),
        destination.length - reader.length(),
    );
    tail.length == 0
        || memory.bytes[memory.bounds(tail).unwrap()]
            .iter()
            .all(|byte| *byte == 0)
}

// Walk actual bytes using numeric hardware indexes, independently of the
// production planner, entry generator, and permission helper. Every parent is
// checked against the retained table arena before following it.
fn walk(
    mapping: &staged_mapping,
    memory: &model_memory,
    address: u64,
) -> Option<(u64, bool, bool, bool)> {
    let tables = mapping.table_range().unwrap();
    let mut table = mapping.root_address().unwrap();
    let mut user = true;
    let mut writable = true;
    let mut executable = true;
    for shift in [39, 30, 21, 12] {
        assert!(table >= tables.base && table + 4096 <= tables.end().unwrap());
        let entry = memory.raw_entry(table, ((address >> shift) & 511) as usize);
        if entry & 1 == 0 {
            return None;
        }
        assert_eq!(entry & (1 << 7), 0, "no huge page substitutes");
        user &= entry & 4 != 0;
        writable &= entry & 2 != 0;
        executable &= entry & (1 << 63) == 0;
        table = entry & 0x000f_ffff_ffff_f000;
    }
    Some((table + (address & 4095), user, writable, executable))
}

fn mapping_snapshot(
    mapping: &staged_mapping,
    memory: &model_memory,
) -> Vec<(physical_range, Vec<u8>)> {
    let mut ranges = vec![mapping.table_range().unwrap()];
    for index in 0..mapping.plan().regions().count() {
        ranges.push(mapping.data_range(index).unwrap());
    }
    ranges
        .into_iter()
        .map(|range| (range, memory.snapshot(range)))
        .collect()
}

fn assert_unchanged(snapshot: &[(physical_range, Vec<u8>)], memory: &model_memory) {
    for (range, expected) in snapshot {
        assert_eq!(&memory.snapshot(*range), expected);
    }
}

fn assert_owned(
    mapping: &staged_mapping,
    allocator: &frame_allocator<'_>,
    baseline: frame_accounting,
) {
    assert_eq!(
        allocator.accounting().active_leases - baseline.active_leases,
        mapping.retained_leases()
    );
}

fn with_small_case(
    test: impl FnOnce(
        &mut runtime_mapping<sealed_source_reader<'_>>,
        &mut frame_allocator<'_>,
        &mut model_memory,
        frame_accounting,
    ),
) {
    let mut ledger = [0; 64 / 4];
    let mut allocator = allocator(&mut ledger, 64);
    let initial = allocator.accounting();
    let mut memory = model_memory::new(64);
    let mut prior = staged_mapping::new(existing());
    unsafe { prior.build(&mut allocator, &mut memory) }.unwrap();
    let prior_data = prior.data_range(0).unwrap();
    let prior_bounds = memory.bounds(prior_data).unwrap();
    memory.bytes[prior_bounds].fill(0x79);
    let prior_before = mapping_snapshot(&prior, &memory);
    let mut source = sealed_source(&mut allocator, &mut memory, 4097);
    let source_range = source.retained_range().unwrap();
    let source_before = memory.snapshot(source_range);
    let baseline = allocator.accounting();
    let reader = source.reader().unwrap();
    let mut owner = runtime_mapping::new(small_plan(), reader);
    memory.calls = access_counts::default();
    memory.read_extents.clear();
    test(&mut owner, &mut allocator, &mut memory, baseline);
    assert_eq!(owner.phase(), runtime_phase::released);
    assert_eq!(owner.mapping().retained_leases(), 0);
    assert_eq!(allocator.accounting(), baseline);
    assert_unchanged(&prior_before, &memory);
    assert_eq!(memory.snapshot(source_range), source_before);
    memory.reset_faults();
    unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
    unsafe { prior.cleanup(&mut allocator, &mut memory) }.unwrap();
    assert_eq!(allocator.accounting(), initial);
}

fn reject_reprepare_and_publish(
    owner: &mut runtime_mapping<sealed_source_reader<'_>>,
    allocator: &mut frame_allocator<'_>,
    memory: &mut model_memory,
) {
    let before = (allocator.accounting(), memory.calls);
    assert_eq!(
        unsafe { owner.prepare(allocator, memory, |_, _, _| panic!("unexpected population")) },
        Err(runtime_error::invalid_phase)
    );
    assert_eq!(
        unsafe { owner.publish(|_, _, _| panic!("unexpected publication")) },
        Err(runtime_error::invalid_phase)
    );
    assert_eq!((allocator.accounting(), memory.calls), before);
}

#[test]
fn full_extent_budget_is_independent_of_backing_and_exact_tail_rounding() {
    let mut exact = runtime_budget {
        virtual_pages: 545,
        data_pages: 535,
        table_frames: 5,
    };
    let plan = runtime_plan::new(&existing(), request(2_231_696, 2_190_608), exact).unwrap();
    let geometry = plan.geometry();
    assert_eq!(geometry.start(), base);
    assert_eq!(geometry.end(), base + 2_232_320);
    assert_eq!(geometry.populated_end(), base + 2_191_360);
    assert_eq!(geometry.byte_length(), 2_231_696);
    assert_eq!(geometry.populated_byte_length(), 2_190_608);
    assert_eq!(geometry.page_count(), 545);
    assert_eq!(geometry.populated_page_count(), 535);
    assert_eq!(geometry.permissions(), read_only);
    assert_eq!(plan.mapping().data_pages(), 535);
    assert_eq!(plan.mapping().regions().count(), 1);
    assert_eq!(plan.mapping().tables().len(), 5);
    let tail = plan.mapping().reservations().next().unwrap();
    assert_eq!(
        (tail.start(), tail.end()),
        (base + 535 * 4096, base + 545 * 4096)
    );
    exact.virtual_pages = 544;
    assert!(runtime_plan::new(&existing(), request(2_231_696, 2_190_608), exact).is_err());
    exact.virtual_pages = 545;
    exact.data_pages = 534;
    assert!(runtime_plan::new(&existing(), request(2_231_696, 2_190_608), exact).is_err());
    exact.data_pages = 535;
    exact.table_frames = 4;
    assert!(runtime_plan::new(&existing(), request(2_231_696, 2_190_608), exact).is_err());
}

#[test]
fn all_absent_tail_pages_remain_owned_against_mappings_and_reservations() {
    let request = request(2_231_696, 2_190_608);
    for page_index in 0..545 {
        let address = base + page_index * 4096;
        assert_eq!(
            runtime_plan::new(&existing_at(address, 1, &[]), request, budget()).err(),
            Some(runtime_plan_error::existing_mapping_collision)
        );
        let existing = existing_at(
            0x40_0000,
            1,
            &[reserved_request {
                virtual_address: address,
                page_count: 1,
            }],
        );
        assert_eq!(
            runtime_plan::new(&existing, request, budget()).err(),
            Some(runtime_plan_error::existing_reservation_collision)
        );
    }
    for address in [base - 4096, base + 545 * 4096] {
        assert!(runtime_plan::new(&existing_at(address, 1, &[]), request, budget()).is_ok());
        let existing = existing_at(
            0x40_0000,
            1,
            &[reserved_request {
                virtual_address: address,
                page_count: 1,
            }],
        );
        assert!(runtime_plan::new(&existing, request, budget()).is_ok());
    }
    for (address, pages) in [(base - 4096, 2), (base - 4096, 547), (base + 544 * 4096, 2)] {
        assert!(runtime_plan::new(&existing_at(address, pages, &[]), request, budget()).is_err());
    }
}

#[test]
fn invalid_geometry_permissions_and_budgets_cannot_create_a_candidate() {
    for (address, length, populated) in [
        (0, 4096, 4096),
        (base + 1, 4096, 4096),
        (base, 0, 1),
        (base, 4096, 0),
        (base, 4096, 4097),
        (base, u64::MAX, 1),
        (u64::MAX - 4095, 4096, 4096),
        ((1 << 47) - 4096, 4097, 1),
        (1 << 47, 4096, 1),
    ] {
        let mut request = request(length, populated);
        request.virtual_address = address;
        assert!(runtime_plan::new(&existing(), request, budget()).is_err());
    }
    for permissions in [
        page_permissions {
            user: false,
            ..read_only
        },
        page_permissions {
            user: true,
            writable: true,
            executable: true,
        },
    ] {
        let mut request = request(4096, 4096);
        request.permissions = permissions;
        assert!(runtime_plan::new(&existing(), request, budget()).is_err());
    }
    for bad in [
        runtime_budget {
            virtual_pages: 0,
            ..budget()
        },
        runtime_budget {
            data_pages: 0,
            ..budget()
        },
        runtime_budget {
            table_frames: 0,
            ..budget()
        },
        runtime_budget {
            table_frames: 33,
            ..budget()
        },
    ] {
        assert!(runtime_plan::new(&existing(), request(4096, 4096), bad).is_err());
    }
}

#[test]
fn populated_prefix_may_cover_valid_bytes_after_the_raw_requested_length() {
    for (length, populated, pages) in [(1, 4096, 1), (4097, 8192, 2)] {
        let plan = runtime_plan::new(&existing(), request(length, populated), budget()).unwrap();
        assert_eq!(plan.geometry().byte_length(), length);
        assert_eq!(plan.geometry().populated_byte_length(), populated);
        assert_eq!(plan.geometry().page_count(), pages);
        assert_eq!(plan.geometry().populated_page_count(), pages);
        assert_eq!(plan.mapping().reservations().count(), 0);
        assert!(runtime_plan::new(&existing(), request(length, populated + 1), budget()).is_err());
    }
}

#[test]
fn full_real_source_population_walks_535_pages_and_keeps_ten_absent() {
    let mut ledger = vec![0; 1152 / 4];
    let mut allocator = allocator(&mut ledger, 1152);
    let initial = allocator.accounting();
    let mut memory = model_memory::new(1152);
    let mut prior = staged_mapping::new(existing());
    unsafe { prior.build(&mut allocator, &mut memory) }.unwrap();
    let prior_bytes = mapping_snapshot(&prior, &memory);
    let mut source = sealed_source(&mut allocator, &mut memory, 2_190_608);
    let source_range = source.retained_range().unwrap();
    let source_before = memory.snapshot(source_range);
    let baseline = allocator.accounting();
    let reader = source.reader().unwrap();
    let plan =
        runtime_plan::new(prior.plan(), request(2_231_696, reader.length()), budget()).unwrap();
    let mut owner = runtime_mapping::new(plan, &reader);
    let before_reads = memory.read_extents.len();
    unsafe {
        owner.prepare(&mut allocator, &mut memory, |mapping, reader, memory| {
            populate(mapping, reader, memory)
        })
    }
    .unwrap();
    assert_eq!(owner.phase(), runtime_phase::ready);
    assert!(core::ptr::eq(*owner.backing(), &reader));
    assert_eq!(owner.backing().grant(), 73);
    assert_eq!(owner.backing().length(), 2_190_608);
    assert_owned(owner.mapping(), &allocator, baseline);
    assert_eq!(allocator.accounting().data_owned - baseline.data_owned, 535);
    assert_eq!(allocator.accounting().table_owned - baseline.table_owned, 5);
    let data = owner.mapping().data_range(0).unwrap();
    let mut physical_pages = BTreeSet::new();
    for index in 0..545 {
        let translated = walk(owner.mapping(), &memory, base + index * 4096);
        if index < 535 {
            assert_eq!(
                translated,
                Some((data.base + index * 4096, true, false, false))
            );
            assert!(physical_pages.insert(translated.unwrap().0));
        } else {
            assert_eq!(translated, None);
        }
    }
    assert_eq!(walk(owner.mapping(), &memory, base - 4096), None);
    assert_eq!(walk(owner.mapping(), &memory, base + 545 * 4096), None);
    let bytes = memory.snapshot(data);
    for (offset, byte) in bytes[..2_190_608].iter().enumerate() {
        assert_eq!(*byte, source_byte(offset), "source byte {offset}");
    }
    assert_eq!(bytes.len() - 2_190_608, 752);
    assert!(bytes[2_190_608..].iter().all(|byte| *byte == 0));
    let source_reads: Vec<_> = memory.read_extents[before_reads..]
        .iter()
        .filter(|range| range.base >= source_range.base && range.base < source_range.end().unwrap())
        .collect();
    assert_eq!(source_reads.len(), 535);
    assert_eq!(
        source_reads.iter().map(|range| range.length).sum::<u64>(),
        2_190_608
    );
    assert_eq!(source_reads.last().unwrap().length, 3344);
    assert!(
        source_reads
            .iter()
            .all(|range| range.end().unwrap() <= source_range.base + 2_190_608)
    );
    assert_unchanged(&prior_bytes, &memory);
    assert_eq!(memory.snapshot(source_range), source_before);
    unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
    assert_eq!(allocator.accounting(), baseline);
    assert_unchanged(&prior_bytes, &memory);
    assert_eq!(memory.snapshot(source_range), source_before);
    unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
    unsafe { prior.cleanup(&mut allocator, &mut memory) }.unwrap();
    assert_eq!(allocator.accounting(), initial);
}

#[test]
fn independent_walk_handles_each_parent_boundary_without_mapping_tail_pages() {
    for boundary in [1 << 21, 1 << 30, 1 << 39] {
        let mut ledger = [0; 64 / 4];
        let mut allocator = allocator(&mut ledger, 64);
        let initial = allocator.accounting();
        let mut memory = model_memory::new(64);
        let mut source = sealed_source(&mut allocator, &mut memory, 4097);
        let reader = source.reader().unwrap();
        let mut request = request(3 * 4096, 4097);
        request.virtual_address = boundary - 4096;
        let plan = runtime_plan::new(&existing(), request, budget()).unwrap();
        let mut owner = runtime_mapping::new(plan, reader);
        unsafe { owner.prepare(&mut allocator, &mut memory, populate) }.unwrap();
        let data = owner.mapping().data_range(0).unwrap();
        assert_eq!(
            walk(owner.mapping(), &memory, boundary - 4096),
            Some((data.base, true, false, false))
        );
        assert_eq!(
            walk(owner.mapping(), &memory, boundary),
            Some((data.base + 4096, true, false, false))
        );
        assert_eq!(walk(owner.mapping(), &memory, boundary + 4096), None);
        assert_eq!(walk(owner.mapping(), &memory, boundary - 8192), None);
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(allocator.accounting(), initial);
    }
}

#[test]
fn each_allocation_clear_failure_retains_partial_ownership_for_cleanup_only() {
    for failure in 1..=2 {
        with_small_case(|owner, allocator, memory, baseline| {
            memory.fail.clears = failure;
            assert_eq!(
                unsafe { owner.prepare(allocator, memory, populate) },
                Err(runtime_error::mapping(mapping_error::frame(
                    frame_error::clearing_failed
                )))
            );
            assert_eq!(owner.phase(), runtime_phase::preparing);
            assert_eq!(owner.mapping().retained_leases(), failure - 1);
            assert_owned(owner.mapping(), allocator, baseline);
            reject_reprepare_and_publish(owner, allocator, memory);
            memory.reset_faults();
            unsafe { owner.cleanup(allocator, memory) }.unwrap();
        });
    }
}

#[test]
fn real_allocator_exhaustion_before_tables_and_before_data_loses_no_lease() {
    for frames in [4, 5] {
        let mut ledger = [0; 2];
        let mut allocator = allocator(&mut ledger, frames);
        let initial = allocator.accounting();
        let mut memory = model_memory::new(frames);
        let backing = [0u8; 1];
        let mut owner = runtime_mapping::new(small_plan(), &backing);
        assert_eq!(
            unsafe {
                owner.prepare(&mut allocator, &mut memory, |_, _, _| {
                    panic!("OOM must precede population")
                })
            },
            Err(runtime_error::mapping(mapping_error::frame(
                frame_error::out_of_memory
            )))
        );
        assert_eq!(owner.phase(), runtime_phase::preparing);
        assert_eq!(owner.mapping().retained_leases(), usize::from(frames == 5));
        assert_owned(owner.mapping(), &allocator, initial);
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(allocator.accounting(), initial);
    }
}

#[test]
fn each_table_write_and_each_table_read_boundary_preserve_unpublished_ownership() {
    let mut successful = access_counts::default();
    with_small_case(|owner, allocator, memory, _| {
        unsafe { owner.prepare(allocator, memory, populate) }.unwrap();
        successful = memory.calls;
        unsafe { owner.cleanup(allocator, memory) }.unwrap();
    });
    assert_eq!(successful.table_reads, 2 * 4 * 512);
    for failure in 1..=successful.table_writes {
        with_small_case(|owner, allocator, memory, baseline| {
            memory.fail.table_writes = failure;
            assert_eq!(
                unsafe { owner.prepare(allocator, memory, populate) },
                Err(runtime_error::mapping(mapping_error::write_failed))
            );
            assert_eq!(owner.phase(), runtime_phase::preparing);
            assert_owned(owner.mapping(), allocator, baseline);
            reject_reprepare_and_publish(owner, allocator, memory);
            memory.reset_faults();
            unsafe { owner.cleanup(allocator, memory) }.unwrap();
        });
    }
    // Existing staged_mapping tests exhaust every readback position. Here the
    // independent transaction test covers the first/last entry of every table
    // in both pre-population and post-population validation passes.
    for failure in (0..8).flat_map(|table| [table * 512 + 1, (table + 1) * 512]) {
        with_small_case(|owner, allocator, memory, baseline| {
            memory.fail.table_reads = failure;
            assert_eq!(
                unsafe { owner.prepare(allocator, memory, populate) },
                Err(runtime_error::mapping(mapping_error::read_failed))
            );
            assert_eq!(owner.phase(), runtime_phase::preparing);
            assert_owned(owner.mapping(), allocator, baseline);
            reject_reprepare_and_publish(owner, allocator, memory);
            memory.reset_faults();
            unsafe { owner.cleanup(allocator, memory) }.unwrap();
        });
    }
}

#[test]
fn source_read_destination_write_and_readback_failures_do_not_publish_a_partial_prefix() {
    for failure in 1..=4 {
        with_small_case(|owner, allocator, memory, baseline| {
            memory.fail.data_reads = failure;
            assert_eq!(
                unsafe { owner.prepare(allocator, memory, populate) },
                Err(runtime_error::population_failed)
            );
            assert_eq!(owner.phase(), runtime_phase::preparing);
            assert_owned(owner.mapping(), allocator, baseline);
            reject_reprepare_and_publish(owner, allocator, memory);
            memory.reset_faults();
            unsafe { owner.cleanup(allocator, memory) }.unwrap();
        });
    }
    for failure in 1..=2 {
        for corrupt in [false, true] {
            with_small_case(|owner, allocator, memory, baseline| {
                if corrupt {
                    memory.corrupt_data_write = failure;
                } else {
                    memory.fail.data_writes = failure;
                }
                assert_eq!(
                    unsafe { owner.prepare(allocator, memory, populate) },
                    Err(runtime_error::population_failed)
                );
                assert_eq!(owner.phase(), runtime_phase::preparing);
                assert_owned(owner.mapping(), allocator, baseline);
                reject_reprepare_and_publish(owner, allocator, memory);
                memory.reset_faults();
                unsafe { owner.cleanup(allocator, memory) }.unwrap();
            });
        }
    }
}

#[test]
fn successful_callback_cannot_hide_table_corruption_from_post_population_validation() {
    with_small_case(|owner, allocator, memory, baseline| {
        let result = unsafe {
            owner.prepare(allocator, memory, |mapping, reader, memory| {
                assert!(populate(mapping, reader, memory));
                // Deliberate violation of the trusted callback contract: a
                // previously absent root entry becomes present. The owner
                // must still catch this during its independent revalidation.
                let root = mapping.root_address().unwrap();
                assert!(mapping_memory::write_entry(memory, root, 511, 0x1003));
                true
            })
        };
        assert_eq!(
            result,
            Err(runtime_error::mapping(mapping_error::entry_mismatch))
        );
        assert_eq!(owner.phase(), runtime_phase::preparing);
        assert_owned(owner.mapping(), allocator, baseline);
        reject_reprepare_and_publish(owner, allocator, memory);
        unsafe { owner.cleanup(allocator, memory) }.unwrap();
    });
}

#[test]
fn population_unwind_retains_ownership_and_allows_only_cleanup() {
    with_small_case(|owner, allocator, memory, baseline| {
        let result = catch_unwind(AssertUnwindSafe(|| unsafe {
            owner.prepare(allocator, memory, |mapping, reader, memory| {
                assert!(populate(mapping, reader, memory));
                panic!("population interrupted after data was written");
            })
        }));
        assert!(result.is_err());
        assert_eq!(owner.phase(), runtime_phase::preparing);
        assert_owned(owner.mapping(), allocator, baseline);
        reject_reprepare_and_publish(owner, allocator, memory);
        unsafe { owner.cleanup(allocator, memory) }.unwrap();
    });
}

#[test]
fn known_publication_rejection_preserves_exact_candidate_and_can_retry_once() {
    with_small_case(|owner, allocator, memory, baseline| {
        assert_eq!(
            unsafe { owner.publish(|_, _, _| panic!("empty candidate")) },
            Err(runtime_error::invalid_phase)
        );
        assert_eq!(
            unsafe { owner.confirm_withdrawal() },
            Err(runtime_error::invalid_phase)
        );
        unsafe { owner.prepare(allocator, memory, populate) }.unwrap();
        let before = mapping_snapshot(owner.mapping(), memory);
        let accounting = allocator.accounting();
        let calls = memory.calls;
        let geometry = owner.geometry();
        let backing_address = core::ptr::from_ref(owner.backing());
        assert_eq!(
            unsafe { owner.confirm_withdrawal() },
            Err(runtime_error::invalid_phase)
        );
        assert_eq!(
            unsafe {
                owner.publish(|mapping, backing, offered| {
                    assert_eq!(offered, geometry);
                    assert_eq!(core::ptr::from_ref(backing), backing_address);
                    assert!(mapping.is_ready());
                    publication_result::rejected
                })
            },
            Err(runtime_error::publication_rejected)
        );
        assert_eq!(owner.phase(), runtime_phase::ready);
        assert_eq!(allocator.accounting(), accounting);
        assert_eq!(memory.calls, calls);
        assert_unchanged(&before, memory);
        assert_owned(owner.mapping(), allocator, baseline);
        unsafe { owner.publish(|_, _, _| publication_result::published) }.unwrap();
        assert_eq!(owner.phase(), runtime_phase::published);
        reject_reprepare_and_publish(owner, allocator, memory);
        assert_eq!(
            unsafe { owner.cleanup(allocator, memory) },
            Err(runtime_error::invalid_phase)
        );
        assert_eq!((allocator.accounting(), memory.calls), (accounting, calls));
        assert_unchanged(&before, memory);
        // No external references were installed by this host adapter. The
        // unsafe confirmation models that separate adapter's withdrawal proof.
        unsafe { owner.confirm_withdrawal() }.unwrap();
        assert_eq!(owner.phase(), runtime_phase::withdrawn);
        reject_reprepare_and_publish(owner, allocator, memory);
        assert_eq!(
            unsafe { owner.confirm_withdrawal() },
            Err(runtime_error::invalid_phase)
        );
        unsafe { owner.cleanup(allocator, memory) }.unwrap();
    });
}

#[test]
fn unknown_and_unwound_publication_cannot_rollback_or_replay_before_explicit_withdrawal() {
    for unwind in [false, true] {
        with_small_case(|owner, allocator, memory, baseline| {
            unsafe { owner.prepare(allocator, memory, populate) }.unwrap();
            let before = mapping_snapshot(owner.mapping(), memory);
            let accounting = allocator.accounting();
            let calls = memory.calls;
            if unwind {
                let result = catch_unwind(AssertUnwindSafe(|| unsafe {
                    owner.publish(|_, _, _| panic!("publication outcome lost"))
                }));
                assert!(result.is_err());
            } else {
                assert_eq!(
                    unsafe { owner.publish(|_, _, _| publication_result::unknown) },
                    Err(runtime_error::publication_unknown)
                );
            }
            assert_eq!(owner.phase(), runtime_phase::uncertain);
            reject_reprepare_and_publish(owner, allocator, memory);
            assert_eq!(
                unsafe { owner.cleanup(allocator, memory) },
                Err(runtime_error::invalid_phase)
            );
            assert_eq!((allocator.accounting(), memory.calls), (accounting, calls));
            assert_unchanged(&before, memory);
            assert_owned(owner.mapping(), allocator, baseline);
            unsafe { owner.confirm_withdrawal() }.unwrap();
            reject_reprepare_and_publish(owner, allocator, memory);
            unsafe { owner.cleanup(allocator, memory) }.unwrap();
        });
    }
}

#[test]
fn every_cleanup_clear_failure_preserves_remaining_handles_and_never_reopens_publication() {
    for published in [false, true] {
        // Table detachment, data release, table release are separate verified
        // clears. Failing after unbind still retains an unbound live lease.
        for failure in 1..=3 {
            with_small_case(|owner, allocator, memory, baseline| {
                unsafe { owner.prepare(allocator, memory, populate) }.unwrap();
                if published {
                    unsafe { owner.publish(|_, _, _| publication_result::published) }.unwrap();
                    unsafe { owner.confirm_withdrawal() }.unwrap();
                }
                memory.fail.clears = memory.calls.clears + failure;
                let error = unsafe { owner.cleanup(allocator, memory) }.unwrap_err();
                assert_eq!(
                    error,
                    if failure == 1 {
                        runtime_error::mapping(mapping_error::clear_failed)
                    } else {
                        runtime_error::mapping(mapping_error::frame(frame_error::clearing_failed))
                    }
                );
                assert_eq!(owner.phase(), runtime_phase::cleaning);
                assert_eq!(
                    owner.mapping().retained_leases(),
                    if failure == 3 { 1 } else { 2 }
                );
                assert_owned(owner.mapping(), allocator, baseline);
                assert_eq!(
                    allocator.accounting().bound_leases - baseline.bound_leases,
                    3 - failure
                );
                reject_reprepare_and_publish(owner, allocator, memory);
                assert_eq!(
                    unsafe { owner.confirm_withdrawal() },
                    Err(runtime_error::invalid_phase)
                );
                memory.reset_faults();
                unsafe { owner.cleanup(allocator, memory) }.unwrap();
                let complete = (allocator.accounting(), memory.calls);
                unsafe { owner.cleanup(allocator, memory) }.unwrap();
                assert_eq!((allocator.accounting(), memory.calls), complete);
                reject_reprepare_and_publish(owner, allocator, memory);
            });
        }
    }
}

fn reject_backing_recovery<backing_type>(
    owner: runtime_mapping<backing_type>,
) -> runtime_mapping<backing_type> {
    let phase = owner.phase();
    let geometry = owner.geometry();
    let leases = owner.mapping().retained_leases();
    match owner.into_backing() {
        Ok(_) => panic!("backing escaped before complete cleanup"),
        Err(retained) => {
            assert_eq!(retained.phase(), phase);
            assert_eq!(retained.geometry(), geometry);
            assert_eq!(retained.mapping().retained_leases(), leases);
            retained
        }
    }
}

// The returned owner has no lifetime tied to this call, its allocator borrow,
// the population callback, or a sealed reader. Source leases move with it.
fn prepare_owned_source_mapping(
    allocator: &mut frame_allocator<'_>,
    memory: &mut model_memory,
) -> runtime_mapping<owned_source> {
    let source = sealed_source(allocator, memory, 4097);
    let mut owner = runtime_mapping::new(small_plan(), source);
    unsafe {
        owner.prepare(allocator, memory, |mapping, source, memory| {
            let reader = source.reader().unwrap();
            populate(mapping, &reader, memory)
        })
    }
    .unwrap();
    owner
}

#[test]
fn owned_backing_survives_call_and_storage_moves_until_withdrawal_and_cleanup() {
    for outcome in [publication_result::published, publication_result::unknown] {
        let mut ledger = [0; 64 / 4];
        let mut allocator = allocator(&mut ledger, 64);
        let initial = allocator.accounting();
        let mut memory = model_memory::new(64);
        let mut retained = Some(prepare_owned_source_mapping(&mut allocator, &mut memory));
        let mut owner = retained.take().unwrap();
        let source_range = owner.backing().retained_range().unwrap();
        let source_before = memory.snapshot(source_range);
        let mapping_before = mapping_snapshot(owner.mapping(), &memory);
        let before = (allocator.accounting(), memory.calls);
        owner = reject_backing_recovery(owner);
        assert_eq!((allocator.accounting(), memory.calls), before);
        assert_eq!(owner.backing().reader().unwrap().grant(), 73);
        assert_eq!(
            unsafe {
                owner.publish(|_, source, geometry| {
                    assert_eq!(source.reader().unwrap().length(), 4097);
                    assert_eq!(geometry.populated_byte_length(), 4097);
                    outcome
                })
            },
            match outcome {
                publication_result::published => Ok(()),
                publication_result::unknown => Err(runtime_error::publication_unknown),
                publication_result::rejected => unreachable!(),
            }
        );
        assert_eq!(
            unsafe { owner.cleanup(&mut allocator, &mut memory) },
            Err(runtime_error::invalid_phase)
        );
        retained = Some(reject_backing_recovery(owner));
        owner = retained.take().unwrap();
        assert_eq!((allocator.accounting(), memory.calls), before);
        assert_unchanged(&mapping_before, &memory);
        assert_eq!(memory.snapshot(source_range), source_before);

        // The host publication adapter installed no external references. Only
        // this explicit confirmation permits cleanup; confirmation alone must
        // still retain the source until every mapping lease has been released.
        unsafe { owner.confirm_withdrawal() }.unwrap();
        owner = reject_backing_recovery(owner);
        assert_eq!((allocator.accounting(), memory.calls), before);
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        let mut source = match owner.into_backing() {
            Ok(source) => source,
            Err(_) => panic!("released mapping retained the backing"),
        };
        assert_eq!(
            allocator.accounting().active_leases,
            initial.active_leases + 1
        );
        assert_eq!(memory.snapshot(source_range), source_before);
        let mut last = [0; 2];
        unsafe { source.reader().unwrap().read(4095, &mut last, &mut memory) }.unwrap();
        assert_eq!(last, [source_byte(4095), source_byte(4096)]);
        unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(allocator.accounting(), initial);
    }
}

#[test]
fn failed_preparation_and_partial_cleanup_do_not_release_owned_backing() {
    let mut ledger = [0; 64 / 4];
    let mut allocator = allocator(&mut ledger, 64);
    let initial = allocator.accounting();
    let mut memory = model_memory::new(64);
    let source = sealed_source(&mut allocator, &mut memory, 4097);
    let source_range = source.retained_range().unwrap();
    let source_before = memory.snapshot(source_range);
    let mut owner = runtime_mapping::new(small_plan(), source);
    memory.calls = access_counts::default();
    memory.fail.data_reads = 1;
    assert_eq!(
        unsafe {
            owner.prepare(&mut allocator, &mut memory, |mapping, source, memory| {
                populate(mapping, &source.reader().unwrap(), memory)
            })
        },
        Err(runtime_error::population_failed)
    );
    assert_eq!(owner.phase(), runtime_phase::preparing);
    assert_eq!(owner.mapping().retained_leases(), 2);
    let before = (allocator.accounting(), memory.calls);
    owner = reject_backing_recovery(owner);
    assert_eq!((allocator.accounting(), memory.calls), before);
    memory.reset_faults();
    memory.fail.clears = memory.calls.clears + 2;
    assert_eq!(
        unsafe { owner.cleanup(&mut allocator, &mut memory) },
        Err(runtime_error::mapping(mapping_error::frame(
            frame_error::clearing_failed
        )))
    );
    assert_eq!(owner.phase(), runtime_phase::cleaning);
    assert_eq!(owner.mapping().retained_leases(), 2);
    let before = (allocator.accounting(), memory.calls);
    owner = reject_backing_recovery(owner);
    assert_eq!((allocator.accounting(), memory.calls), before);
    assert_eq!(memory.snapshot(source_range), source_before);
    assert_eq!(owner.backing().reader().unwrap().grant(), 73);
    memory.reset_faults();
    unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
    let mut source = match owner.into_backing() {
        Ok(source) => source,
        Err(_) => panic!("released mapping retained the backing"),
    };
    assert_eq!(memory.snapshot(source_range), source_before);
    unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
    assert_eq!(allocator.accounting(), initial);
}

#[test]
fn borrowed_unsized_backing_requires_explicit_cleanup_even_without_leases() {
    let mut ledger = [0; 64 / 4];
    let mut allocator = allocator(&mut ledger, 64);
    let initial = allocator.accounting();
    let mut memory = model_memory::new(64);
    let bytes = [0xa5; 9];
    let owner = runtime_mapping::new(small_plan(), bytes.as_slice());
    let mut owner = reject_backing_recovery(owner);
    assert_eq!(owner.phase(), runtime_phase::empty);
    assert_eq!(owner.mapping().retained_leases(), 0);
    assert_eq!(allocator.accounting(), initial);
    unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
    let recovered = match owner.into_backing() {
        Ok(backing) => backing,
        Err(_) => panic!("released empty mapping retained the backing"),
    };
    assert!(core::ptr::eq(recovered, bytes.as_slice()));
    assert_eq!(allocator.accounting(), initial);
    assert_eq!(memory.calls, access_counts::default());
}
