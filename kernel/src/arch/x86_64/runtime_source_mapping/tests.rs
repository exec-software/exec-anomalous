extern crate std;

use super::super::mapping_plan::{mapping_budget, mapping_plan, mapping_request};
use super::super::owned_source::{owned_source, source_budget};
use super::super::paging::page_permissions;
use super::super::runtime_mapping::{
    runtime_budget, runtime_mapping, runtime_phase, runtime_plan, runtime_request,
};
use super::*;
use crate::boot_omen::memory_region;
use crate::memory::physical::frame_allocator;
use std::{vec, vec::Vec};

const page: usize = 4096;
const base: u64 = 0x2000_0000;
const read_only: page_permissions = page_permissions {
    user: true,
    writable: false,
    executable: false,
};

#[derive(Default)]
struct access_counts {
    clears: usize,
    table_reads: usize,
    table_writes: usize,
    reads: usize,
    writes: usize,
}

struct model_memory {
    bytes: Vec<u8>,
    active: u64,
    calls: access_counts,
    fail_read: usize,
    fail_write: usize,
    fail_table_read: usize,
    corrupt_write: bool,
    tables_read: Vec<u64>,
}

impl model_memory {
    fn new(frames: usize) -> Self {
        Self {
            bytes: vec![0xa5; frames * page],
            active: 0,
            calls: access_counts::default(),
            fail_read: 0,
            fail_write: 0,
            fail_table_read: 0,
            corrupt_write: false,
            tables_read: Vec::new(),
        }
    }

    fn bounds(&self, range: physical_range) -> Option<core::ops::Range<usize>> {
        let start = usize::try_from(range.base).ok()?;
        let end = usize::try_from(range.end()?).ok()?;
        (start < end && end <= self.bytes.len()).then_some(start..end)
    }

    fn word(&self, table: u64, index: usize) -> Option<core::ops::Range<usize>> {
        if !table.is_multiple_of(page_bytes) || index >= 512 {
            return None;
        }
        self.bounds(physical_range::new(
            table.checked_add((index * 8) as u64)?,
            8,
        ))
    }

    fn raw_entry(&self, table: u64, index: usize) -> u64 {
        u64::from_le_bytes(
            self.bytes[self.word(table, index).unwrap()]
                .try_into()
                .unwrap(),
        )
    }

    fn set_entry(&mut self, table: u64, index: usize, value: u64) {
        let bounds = self.word(table, index).unwrap();
        self.bytes[bounds].copy_from_slice(&value.to_le_bytes());
    }

    fn leaf(&self, root: u64, address: u64) -> (u64, usize) {
        let mut table = root;
        for shift in [39, 30, 21] {
            table = self.raw_entry(table, ((address >> shift) & 511) as usize) & physical_mask;
        }
        (table, ((address >> 12) & 511) as usize)
    }

    fn snapshot(&self, range: physical_range) -> Vec<u8> {
        self.bytes[self.bounds(range).unwrap()].to_vec()
    }

    fn clear(&mut self, range: physical_range) -> bool {
        self.calls.clears += 1;
        let Some(bounds) = self.bounds(range) else {
            return false;
        };
        self.bytes[bounds.clone()].fill(0);
        self.bytes[bounds].iter().all(|byte| *byte == 0)
    }

    fn reset(&mut self) {
        self.calls = access_counts::default();
        self.fail_read = 0;
        self.fail_write = 0;
        self.fail_table_read = 0;
        self.corrupt_write = false;
        self.tables_read.clear();
    }
}

// SAFETY: Physical extents are offsets in isolated modeled RAM. Every access
// checks its complete bounds; the model never executes or publishes a tree.
unsafe impl mapping_memory for model_memory {
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        self.clear(range)
    }

    unsafe fn read_entry(&mut self, table: u64, index: usize) -> Option<u64> {
        self.calls.table_reads += 1;
        self.tables_read.push(table);
        if self.calls.table_reads == self.fail_table_read {
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
        true
    }

    fn active_root(&self) -> u64 {
        self.active
    }
}

// SAFETY: Reads and writes cover checked extents in the same isolated RAM.
// Failed operations touch only a destination prefix. The corrupt-write option
// deliberately simulates an undetected physical fault for readback tests.
unsafe impl source_memory for model_memory {
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        self.clear(range)
    }

    unsafe fn write(&mut self, range: physical_range, source: &[u8]) -> bool {
        self.calls.writes += 1;
        let Some(bounds) = self.bounds(range) else {
            return false;
        };
        if bounds.len() != source.len() {
            return false;
        }
        if self.calls.writes == self.fail_write {
            let count = source.len().div_ceil(2);
            self.bytes[bounds.start..bounds.start + count].copy_from_slice(&source[..count]);
            return false;
        }
        self.bytes[bounds.clone()].copy_from_slice(source);
        if self.corrupt_write {
            self.bytes[bounds.end - 1] ^= 1;
        }
        true
    }

    unsafe fn read(&mut self, range: physical_range, destination: &mut [u8]) -> bool {
        self.calls.reads += 1;
        let Some(bounds) = self.bounds(range) else {
            return false;
        };
        if bounds.len() != destination.len() {
            return false;
        }
        if self.calls.reads == self.fail_read {
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
        &[physical_range::new(0, page_bytes)],
    )
    .unwrap()
}

fn existing() -> mapping_plan {
    mapping_plan::new(
        &[mapping_request {
            virtual_address: 0x40_0000,
            page_count: 2,
            permissions: read_only,
        }],
        &[],
        mapping_budget {
            data_pages: 1024,
            table_frames: 32,
        },
    )
    .unwrap()
}

fn plan(start: u64, length: u64, populated: u64, permissions: page_permissions) -> runtime_plan {
    runtime_plan::new(
        &existing(),
        runtime_request {
            virtual_address: start,
            byte_length: length,
            populated_byte_length: populated,
            permissions,
        },
        runtime_budget {
            virtual_pages: 1024,
            data_pages: 1024,
            table_frames: 32,
        },
    )
    .unwrap()
}

fn source_byte(offset: usize) -> u8 {
    ((offset.wrapping_mul(37) ^ (offset >> 7) ^ (offset >> 15)) & 255) as u8
}

fn sealed_source(
    allocator: &mut frame_allocator<'_>,
    memory: &mut model_memory,
    length: usize,
) -> owned_source {
    // SAFETY: This fixture is the only provisioner and retains its allocation.
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

fn expected_hash(length: usize) -> u64 {
    (0..length).fold(0xcbf2_9ce4_8422_2325, |hash, offset| {
        (hash ^ u64::from(source_byte(offset))).wrapping_mul(0x100_0000_01b3)
    })
}

#[test]
fn full_prefix_shape_is_copied_checked_and_cancelled_without_changing_prior_owners() {
    const frames: usize = 1152;
    const length: usize = 2_190_608;
    let mut ledger = vec![0; frames / 4];
    let mut allocator = allocator(&mut ledger, frames);
    let initial = allocator.accounting();
    let mut memory = model_memory::new(frames);
    let mut prior = staged_mapping::new(existing());
    unsafe { prior.build(&mut allocator, &mut memory) }.unwrap();
    let prior_tables = memory.snapshot(prior.table_range().unwrap());
    let prior_data = memory.snapshot(prior.data_range(0).unwrap());
    let mut source = sealed_source(&mut allocator, &mut memory, length);
    let source_bytes = memory.snapshot(source.retained_range().unwrap());
    let baseline = allocator.accounting();
    let reader = source.reader().unwrap();
    let plan = plan(base, 2_231_696, length as u64, read_only);
    let geometry = plan.geometry();
    let mut owner = runtime_mapping::new(plan, &reader);
    let mut scratch = [0xa5; page];
    let mut readback = [0x5a; page];
    let mut observation = population_observation::default();
    unsafe {
        owner.prepare(&mut allocator, &mut memory, |mapping, reader, memory| {
            memory.reset();
            let tables_before = memory.snapshot(mapping.table_range().unwrap());
            let success = populate_and_verify(
                mapping,
                reader,
                geometry,
                memory,
                &mut scratch,
                &mut readback,
                &mut observation,
            );
            assert_eq!(
                memory.snapshot(mapping.table_range().unwrap()),
                tables_before
            );
            assert_eq!(memory.calls.clears, 0);
            assert_eq!(memory.calls.table_writes, 0);
            assert_eq!(memory.calls.writes, 535);
            assert_eq!(memory.calls.reads, 1071);
            success
        })
    }
    .unwrap();
    assert_eq!(owner.phase(), runtime_phase::ready);
    assert_eq!(
        observation,
        population_observation {
            source_bytes: length as u64,
            source_fnv64: expected_hash(length),
            destination_fnv64: expected_hash(length),
            zero_bytes: 752,
            populated_pages: 535,
            absent_pages: 10,
            table_frames: 5,
        }
    );
    assert!(scratch.iter().chain(&readback).all(|byte| *byte == 0));
    let data = memory.snapshot(owner.mapping().data_range(0).unwrap());
    assert!(
        data[..length]
            .iter()
            .enumerate()
            .all(|(offset, byte)| *byte == source_byte(offset))
    );
    assert!(data[length..].iter().all(|byte| *byte == 0));
    assert_eq!(memory.snapshot(prior.table_range().unwrap()), prior_tables);
    assert_eq!(memory.snapshot(prior.data_range(0).unwrap()), prior_data);
    assert_eq!(
        memory.snapshot(source.retained_range().unwrap()),
        source_bytes
    );
    assert_eq!(allocator.accounting().data_owned - baseline.data_owned, 535);
    assert_eq!(allocator.accounting().table_owned - baseline.table_owned, 5);
    unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
    assert_eq!(allocator.accounting(), baseline);
    unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
    unsafe { prior.cleanup(&mut allocator, &mut memory) }.unwrap();
    assert_eq!(allocator.accounting(), initial);
}

#[derive(Clone, Copy, Debug)]
enum fault {
    read(usize),
    write(usize),
    readback_corruption,
    dirty_slack,
    table_read,
    escaped_table,
    writable_leaf,
    executable_leaf,
    supervisor_leaf,
    huge_leaf,
    aliased_leaf,
    tail,
    lower_neighbor,
    upper_neighbor,
    wrong_geometry,
    active_root,
    short_source,
}

#[test]
fn failures_preserve_ownership_wipe_both_buffers_and_leave_no_success_observation() {
    for fault in [
        fault::read(1),
        fault::read(2),
        fault::read(3),
        fault::read(4),
        fault::read(5),
        fault::write(1),
        fault::write(2),
        fault::readback_corruption,
        fault::dirty_slack,
        fault::table_read,
        fault::escaped_table,
        fault::writable_leaf,
        fault::executable_leaf,
        fault::supervisor_leaf,
        fault::huge_leaf,
        fault::aliased_leaf,
        fault::tail,
        fault::lower_neighbor,
        fault::upper_neighbor,
        fault::wrong_geometry,
        fault::active_root,
        fault::short_source,
    ] {
        let mut ledger = [0; 16];
        let mut allocator = allocator(&mut ledger, 64);
        let initial = allocator.accounting();
        let mut memory = model_memory::new(64);
        let length = if matches!(fault, fault::short_source) {
            4096
        } else {
            4097
        };
        let mut source = sealed_source(&mut allocator, &mut memory, length);
        let source_before = memory.snapshot(source.retained_range().unwrap());
        let baseline = allocator.accounting();
        let reader = source.reader().unwrap();
        let runtime = plan(base + page_bytes, 3 * page_bytes - 27, 4097, read_only);
        let mut geometry = runtime.geometry();
        let mut owner = runtime_mapping::new(runtime, &reader);
        let mut scratch = [0xa5; page];
        let mut readback = [0x5a; page];
        let mut observation = population_observation {
            source_bytes: 99,
            ..Default::default()
        };
        let result = unsafe {
            owner.prepare(&mut allocator, &mut memory, |mapping, reader, memory| {
                memory.reset();
                let arena = mapping.table_range().unwrap();
                let root = mapping.root_address().unwrap();
                let data = mapping.data_range(0).unwrap();
                let (leaf, index) = memory.leaf(root, geometry.start());
                let entry = memory.raw_entry(leaf, index);
                match fault {
                    fault::read(call) => memory.fail_read = call,
                    fault::write(call) => memory.fail_write = call,
                    fault::readback_corruption => memory.corrupt_write = true,
                    fault::dirty_slack => memory.bytes[(data.base + 4097) as usize] = 1,
                    fault::table_read => memory.fail_table_read = 2,
                    fault::escaped_table => memory.set_entry(root, 0, arena.end().unwrap() | 0x207),
                    fault::writable_leaf => memory.set_entry(leaf, index, entry | 2),
                    fault::executable_leaf => memory.set_entry(leaf, index, entry & !(1 << 63)),
                    fault::supervisor_leaf => memory.set_entry(leaf, index, entry & !4),
                    fault::huge_leaf => memory.set_entry(leaf, index, entry | 128),
                    fault::aliased_leaf => memory.set_entry(leaf, index + 1, entry),
                    fault::tail => memory.set_entry(leaf, index + 2, entry),
                    fault::lower_neighbor => memory.set_entry(leaf, index - 1, entry),
                    fault::upper_neighbor => memory.set_entry(leaf, index + 3, entry),
                    fault::wrong_geometry => {
                        geometry =
                            plan(base + 2 * page_bytes, 3 * page_bytes - 27, 4097, read_only)
                                .geometry();
                    }
                    fault::active_root => memory.active = root,
                    fault::short_source => {}
                }
                let table_before = memory.snapshot(arena);
                let success = populate_and_verify(
                    mapping,
                    reader,
                    geometry,
                    memory,
                    &mut scratch,
                    &mut readback,
                    &mut observation,
                );
                assert!(!success, "{fault:?}");
                assert_eq!(memory.snapshot(arena), table_before, "{fault:?}");
                assert_eq!(memory.calls.clears, 0, "{fault:?}");
                assert_eq!(memory.calls.table_writes, 0, "{fault:?}");
                assert!(
                    memory.tables_read.iter().all(|table| {
                        *table >= arena.base && *table + page_bytes <= arena.end().unwrap()
                    }),
                    "{fault:?}"
                );
                if matches!(fault, fault::escaped_table) {
                    assert_eq!(memory.tables_read, [root]);
                }
                success
            })
        };
        assert!(result.is_err(), "{fault:?}");
        assert_eq!(owner.phase(), runtime_phase::preparing, "{fault:?}");
        assert_eq!(owner.mapping().retained_leases(), 2, "{fault:?}");
        assert_eq!(observation, population_observation::default(), "{fault:?}");
        assert!(
            scratch.iter().chain(&readback).all(|byte| *byte == 0),
            "{fault:?}"
        );
        assert_eq!(
            memory.snapshot(source.retained_range().unwrap()),
            source_before,
            "{fault:?}"
        );
        assert_eq!(
            allocator.accounting().active_leases - baseline.active_leases,
            2
        );
        memory.active = 0;
        memory.reset();
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(allocator.accounting(), baseline, "{fault:?}");
        unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(allocator.accounting(), initial, "{fault:?}");
    }
}

#[test]
fn exact_page_prefix_reads_no_source_suffix_and_has_no_slack_or_tail() {
    let mut ledger = [0; 16];
    let mut allocator = allocator(&mut ledger, 64);
    let mut memory = model_memory::new(64);
    let mut source = sealed_source(&mut allocator, &mut memory, 4097);
    let reader = source.reader().unwrap();
    let plan = plan(base, page_bytes, page_bytes, read_only);
    let geometry = plan.geometry();
    let mut owner = runtime_mapping::new(plan, &reader);
    let mut scratch = [0xa5; page];
    let mut readback = [0x5a; page];
    let mut observation = population_observation::default();
    unsafe {
        owner.prepare(&mut allocator, &mut memory, |mapping, reader, memory| {
            memory.reset();
            let result = populate_and_verify(
                mapping,
                reader,
                geometry,
                memory,
                &mut scratch,
                &mut readback,
                &mut observation,
            );
            assert_eq!(memory.calls.reads, 2);
            assert_eq!(memory.calls.writes, 1);
            result
        })
    }
    .unwrap();
    assert_eq!(observation.source_bytes, page_bytes);
    assert_eq!(observation.source_fnv64, expected_hash(page));
    assert_eq!(observation.destination_fnv64, expected_hash(page));
    assert_eq!(observation.zero_bytes, 0);
    assert_eq!(observation.absent_pages, 0);
    assert!(scratch.iter().chain(&readback).all(|byte| *byte == 0));
    unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
    unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
}

#[test]
fn unsupported_permissions_are_rejected_before_physical_access() {
    for permissions in [
        page_permissions {
            writable: true,
            ..read_only
        },
        page_permissions {
            executable: true,
            ..read_only
        },
    ] {
        let mut ledger = [0; 16];
        let mut allocator = allocator(&mut ledger, 64);
        let mut memory = model_memory::new(64);
        let mut source = sealed_source(&mut allocator, &mut memory, 1);
        let reader = source.reader().unwrap();
        let plan = plan(base, 1, 1, permissions);
        let geometry = plan.geometry();
        let mut owner = runtime_mapping::new(plan, &reader);
        let mut scratch = [0xa5; page];
        let mut readback = [0x5a; page];
        let mut observation = population_observation::default();
        let result = unsafe {
            owner.prepare(&mut allocator, &mut memory, |mapping, reader, memory| {
                memory.reset();
                let result = populate_and_verify(
                    mapping,
                    reader,
                    geometry,
                    memory,
                    &mut scratch,
                    &mut readback,
                    &mut observation,
                );
                assert_eq!(memory.calls.reads, 0);
                assert_eq!(memory.calls.writes, 0);
                assert_eq!(memory.calls.table_reads, 0);
                result
            })
        };
        assert!(result.is_err());
        assert!(scratch.iter().chain(&readback).all(|byte| *byte == 0));
        assert_eq!(observation, population_observation::default());
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
    }
}
