//! QEMU-only, privileged read-only source provisioning for ADR 0079.
//!
//! Reviewed protocol: https://www.qemu.org/docs/master/specs/fw_cfg.html
//! Selector OUT16 at 0x510, sequential data IN8 at 0x511, directory selector
//! 0x19 with big-endian fields. The trusted controller selects one exact item,
//! supplied using QEMU's `-fw_cfg ...,file=...`. The original constructor keeps
//! `opt/anomalous-linux-source` and its 512-KiB budget unchanged.
//! No DMA, data-port writes, arbitrary selector access or task upload is exposed.
//!
//! The linked controller supplies the exact length and FNV-1a-64 independently
//! of firmware bytes. FNV is an accidental-corruption/evidence check, not a
//! cryptographic signature or authentication. Firmware supplies bytes only;
//! native destination mapping and execution admission remain separate.

use core::arch::asm;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

use super::owned_source::{
    owned_source, sealed_source_reader, source_budget, source_chunk_limit, source_error,
    source_memory,
};
use super::physical_access::bootstrap_physical_access;
use crate::memory::physical::frame_allocator;

pub(super) const source_byte_limit: u64 = 512 * 1024;
const directory_capacity: u32 = 128;
const file_name: &[u8] = b"opt/anomalous-linux-source";
const fnv_offset: u64 = 0xcbf29ce484222325;
const fnv_prime: u64 = 0x100000001b3;

static scratch_claimed: AtomicBool = AtomicBool::new(false);
static mut import_scratch: [u8; source_chunk_limit] = [0; source_chunk_limit];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum source_load_error {
    invalid_input,
    invalid_state,
    interrupts_enabled,
    scratch_busy,
    scratch_not_clear,
    scratch_clear,
    signature,
    revision,
    directory_count,
    directory_entry,
    duplicate_entry,
    missing_entry,
    size,
    chunk,
    hash,
    alias_install,
    alias_remove,
    source(source_error),
}

impl source_load_error {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::invalid_input => "linux_source_input",
            Self::invalid_state => "linux_source_state",
            Self::interrupts_enabled => "linux_source_interrupts",
            Self::scratch_busy => "linux_source_scratch_busy",
            Self::scratch_not_clear => "linux_source_scratch_not_clear",
            Self::scratch_clear => "linux_source_scratch_clear",
            Self::signature => "linux_source_signature",
            Self::revision => "linux_source_revision",
            Self::directory_count => "linux_source_directory_count",
            Self::directory_entry => "linux_source_directory_entry",
            Self::duplicate_entry => "linux_source_duplicate",
            Self::missing_entry => "linux_source_missing",
            Self::size => "linux_source_size",
            Self::chunk => "linux_source_chunk",
            Self::hash => "linux_source_hash",
            Self::alias_install => "linux_source_alias_install",
            Self::alias_remove => "linux_source_alias_remove",
            Self::source(source_error::invalid_binding) => "linux_source_binding",
            Self::source(source_error::invalid_budget) => "linux_source_budget",
            Self::source(source_error::invalid_state) => "linux_source_owner_state",
            Self::source(source_error::invalid_extent) => "linux_source_extent",
            Self::source(source_error::invalid_offset) => "linux_source_offset",
            Self::source(source_error::chunk_size) => "linux_source_owner_chunk",
            Self::source(source_error::incomplete) => "linux_source_incomplete",
            Self::source(source_error::write_failed) => "linux_source_write",
            Self::source(source_error::read_failed) => "linux_source_read",
            Self::source(source_error::readback_mismatch) => "linux_source_readback",
            Self::source(source_error::frame(_)) => "linux_source_frame",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum load_phase {
    fresh,
    importing,
    failed,
    ready,
    releasing,
    released,
}

/// Keep this owner in retained supervisor storage through the consumer's full
/// lifetime. Errors never discard a partial source lease or installed alias.
/// There is no fallible Drop and no retry that resumes a failed firmware import.
#[must_use = "retain the controller until source and temporary alias cleanup succeeds"]
pub(super) struct source_controller {
    source: Option<owned_source>,
    access: bootstrap_physical_access,
    grant: u64,
    length: u64,
    expected_fnv: u64,
    name: &'static [u8],
    budget: source_budget,
    observed_fnv: u64,
    chunks: u64,
    scratch_retained: bool,
    phase: load_phase,
}

impl source_controller {
    /// Inert configuration only. Import, not these numeric fields, requires
    /// explicit privileged provisioning authority.
    pub(super) fn new(
        grant: u64,
        expected_length: u64,
        expected_fnv64: u64,
    ) -> Result<Self, source_load_error> {
        Self::new_configured(
            grant,
            expected_length,
            expected_fnv64,
            file_name,
            source_byte_limit,
        )
    }

    /// Explicit per-source provisioning facts selected by the trusted boot
    /// controller. The item name is firmware transport metadata, not a guest
    /// pathname or authority supplied by a task. Byte/page budgets apply only
    /// to this retained source; they do not change the original constructor.
    pub(super) fn new_configured(
        grant: u64,
        expected_length: u64,
        expected_fnv64: u64,
        name: &'static [u8],
        byte_limit: u64,
    ) -> Result<Self, source_load_error> {
        let budget = validate_input(grant, expected_length, name, byte_limit)?;
        Ok(Self {
            source: None,
            access: bootstrap_physical_access::new(),
            grant,
            length: expected_length,
            expected_fnv: expected_fnv64,
            name,
            budget,
            observed_fnv: fnv_offset,
            chunks: 0,
            scratch_retained: false,
            phase: load_phase::fresh,
        })
    }

    /// # Safety
    /// The selected boot controller owns fw_cfg exclusively, on the sole BSP
    /// with IF clear/NMI masked. Length/hash/grant came from linked trusted
    /// metadata, not firmware. The restored Threshold root has no execution
    /// alias or live user tasks. Keep this controller on every result, and call
    /// finish before abandoning a failed import. No task identity is fabricated.
    #[inline(never)]
    pub(super) unsafe fn import(
        &mut self,
        allocator: &mut frame_allocator<'_>,
    ) -> Result<(), source_load_error> {
        if self.phase != load_phase::fresh {
            return Err(source_load_error::invalid_state);
        }
        if super::interrupts::interrupts_enabled() {
            return Err(source_load_error::interrupts_enabled);
        }
        if scratch_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(source_load_error::scratch_busy);
        }
        self.phase = load_phase::importing;
        self.scratch_retained = true;
        // SAFETY: Exclusive BSS acquisition above; no reference escapes import.
        let scratch = unsafe { &mut *ptr::addr_of_mut!(import_scratch) };
        let result = (|| {
            if !scratch_is_zero(scratch) {
                return Err(source_load_error::scratch_not_clear);
            }
            let mut ports = hardware_ports;
            let mut stream =
                firmware_stream::open(&mut ports, self.length, self.name, self.budget.max_bytes)?;
            // SAFETY: The explicitly privileged caller chose the origin; this
            // source is retained before allocation or any physical mutation.
            self.source = Some(
                unsafe { owned_source::new_provisioned(self.grant, self.length, self.budget) }
                    .map_err(source_load_error::source)?,
            );
            // SAFETY: Exclusive bootstrap root, no execution alias/borrowers.
            unsafe { self.access.install() }.map_err(|_| source_load_error::alias_install)?;
            let source = self
                .source
                .as_mut()
                .ok_or(source_load_error::invalid_state)?;
            // SAFETY: Both owners remain fields of self even if binding fails.
            unsafe { source.allocate(allocator, &mut self.access) }
                .map_err(source_load_error::source)?;
            // SAFETY: Controller origin, nonaliasing BSS scratch and retained
            // independent physical alias under the import exclusion interval.
            let (hash, chunks) = unsafe {
                transfer(
                    source,
                    self.grant,
                    &mut stream,
                    &mut ports,
                    &mut self.access,
                    scratch,
                    self.expected_fnv,
                )
            }?;
            self.observed_fnv = hash;
            self.chunks = chunks;
            // SAFETY: Source has no raw alias or registered root dependency;
            // only its accounted lease survives removal. No reader escaped.
            unsafe { self.access.remove() }.map_err(|_| source_load_error::alias_remove)?;
            Ok(())
        })();
        let cleared = clear_scratch(scratch);
        if cleared {
            scratch_claimed.store(false, Ordering::Release);
            self.scratch_retained = false;
        }
        if !cleared {
            self.phase = load_phase::failed;
            return Err(source_load_error::scratch_clear);
        }
        self.phase = if result.is_ok() {
            load_phase::ready
        } else {
            load_phase::failed
        };
        result
    }

    pub(super) fn reader(&self) -> Result<sealed_source_reader<'_>, source_load_error> {
        if !self.is_ready() {
            return Err(source_load_error::invalid_state);
        }
        self.source
            .as_ref()
            .ok_or(source_load_error::invalid_state)?
            .reader()
            .map_err(source_load_error::source)
    }

    /// # Safety
    /// Every consumer has fully retired, including execution aliases. Restore
    /// the exclusive Threshold root under sole-BSP IF-clear/NMI-masked control.
    /// Retain self and the allocator on failure, and retry this same operation.
    /// No source reader, raw pointer or hardware access may survive cleanup.
    #[inline(never)]
    pub(super) unsafe fn finish(
        &mut self,
        allocator: &mut frame_allocator<'_>,
    ) -> Result<(), source_load_error> {
        if super::interrupts::interrupts_enabled() {
            return Err(source_load_error::interrupts_enabled);
        }
        if self.phase == load_phase::released {
            return Ok(());
        }
        if self.phase == load_phase::importing {
            return Err(source_load_error::invalid_state);
        }
        self.phase = load_phase::releasing;
        if self.scratch_retained {
            // SAFETY: A failed import retained the exclusive acquisition;
            // no other controller can borrow this BSS until successful clear.
            let scratch = unsafe { &mut *ptr::addr_of_mut!(import_scratch) };
            if !clear_scratch(scratch) {
                return Err(source_load_error::scratch_clear);
            }
            scratch_claimed.store(false, Ordering::Release);
            self.scratch_retained = false;
        }
        if let Some(source) = self.source.as_mut() {
            if source.retained_range().is_some() && self.access.alias_entry().is_none() {
                self.access = bootstrap_physical_access::new();
                // SAFETY: Fresh independent cleanup alias after consumer exit.
                unsafe { self.access.install() }.map_err(|_| source_load_error::alias_install)?;
            }
            // SAFETY: No consumer survives. On partial clear the same source
            // lease and live access stay retained for another finish attempt.
            unsafe { source.cleanup(allocator, &mut self.access) }
                .map_err(source_load_error::source)?;
        }
        if self.access.alias_entry().is_some() {
            // SAFETY: Every source lease has been released and no borrower exists.
            unsafe { self.access.remove() }.map_err(|_| source_load_error::alias_remove)?;
        }
        self.phase = load_phase::released;
        Ok(())
    }

    pub(super) fn is_ready(&self) -> bool {
        self.phase == load_phase::ready
            && !self.scratch_retained
            && self.access.alias_entry().is_none()
            && self.source.as_ref().is_some_and(owned_source::is_sealed)
    }

    pub(super) fn is_released(&self) -> bool {
        self.phase == load_phase::released
            && !self.scratch_retained
            && self.access.alias_entry().is_none()
            && self.source.as_ref().is_none_or(owned_source::is_released)
    }

    pub(super) const fn length(&self) -> u64 {
        self.length
    }
    pub(super) const fn observed_fnv64(&self) -> u64 {
        self.observed_fnv
    }
    pub(super) const fn chunks(&self) -> u64 {
        self.chunks
    }
}

fn validate_input(
    grant: u64,
    length: u64,
    name: &[u8],
    byte_limit: u64,
) -> Result<source_budget, source_load_error> {
    // A directory entry has a u32 byte length and a 56-byte NUL-terminated
    // transport name. These wire geometry checks impose no guest path limit.
    if grant == 0
        || length == 0
        || length > byte_limit
        || byte_limit == 0
        || byte_limit > u64::from(u32::MAX)
        || name.is_empty()
        || name.len() >= 56
        || name.contains(&0)
    {
        return Err(source_load_error::invalid_input);
    }
    let max_pages =
        usize::try_from(byte_limit.div_ceil(4096)).map_err(|_| source_load_error::invalid_input)?;
    Ok(source_budget {
        max_bytes: byte_limit,
        max_pages,
    })
}

fn scratch_is_zero(scratch: &[u8]) -> bool {
    scratch.iter().all(|byte| {
        // SAFETY: Caller lends exclusively owned initialized RAM.
        unsafe { ptr::read_volatile(byte) == 0 }
    })
}

fn clear_scratch(scratch: &mut [u8]) -> bool {
    for byte in scratch.iter_mut() {
        // SAFETY: Caller lends exclusively owned initialized RAM.
        unsafe { ptr::write_volatile(byte, 0) };
    }
    scratch_is_zero(scratch)
}

fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(fnv_prime);
    }
    hash
}

// The caller owns the controller, source, physical access and scratch. A failed
// transfer remains unsealed; cleanup retains/reclaims every acquired lease.
#[allow(clippy::too_many_arguments)]
unsafe fn transfer(
    source: &mut owned_source,
    grant: u64,
    stream: &mut firmware_stream,
    ports: &mut impl firmware_ports,
    memory: &mut impl source_memory,
    scratch: &mut [u8; source_chunk_limit],
    expected_hash: u64,
) -> Result<(u64, u64), source_load_error> {
    let mut hash = fnv_offset;
    let mut chunks = 0;
    let result = (|| {
        if !scratch_is_zero(scratch) {
            return Err(source_load_error::scratch_not_clear);
        }
        while stream.remaining != 0 {
            let length = stream.remaining.min(source_chunk_limit as u64) as usize;
            let offset = source.received();
            stream.read(ports, &mut scratch[..length])?;
            // SAFETY: Privileged origin and exact sequential source extent;
            // retained independent access and nonaliasing initialized scratch.
            unsafe { source.append_provisioned(grant, offset, &scratch[..length], memory) }
                .map_err(source_load_error::source)?;
            hash = hash_bytes(hash, &scratch[..length]);
            chunks += 1;
            if !clear_scratch(scratch) {
                return Err(source_load_error::scratch_clear);
            }
        }
        if source.received() != source.length() {
            return Err(source_load_error::size);
        }
        if hash != expected_hash {
            return Err(source_load_error::hash);
        }
        // SAFETY: Exact expected length/hash and all accepted chunks read back;
        // this is controller provenance, not task or executable authority.
        unsafe { source.seal_provisioned(grant) }.map_err(source_load_error::source)?;
        Ok((hash, chunks))
    })();
    if !clear_scratch(scratch) {
        return Err(source_load_error::scratch_clear);
    }
    result
}

trait firmware_ports {
    fn select(&mut self, selector: u16);
    fn read_byte(&mut self) -> u8;
}

struct hardware_ports;
impl firmware_ports for hardware_ports {
    fn select(&mut self, selector: u16) {
        // SAFETY: Private adapter exists only inside the privileged import's
        // exclusive fw_cfg contract. No write-mode selectors are admitted.
        unsafe {
            asm!("out dx, ax", in("dx") 0x510u16, in("ax") selector,
            options(nomem, nostack, preserves_flags));
        }
    }
    fn read_byte(&mut self) -> u8 {
        let byte: u8;
        // SAFETY: Same exclusive read-only interface; no data OUT or DMA.
        unsafe {
            asm!("in al, dx", in("dx") 0x511u16, out("al") byte,
            options(nomem, nostack, preserves_flags));
        }
        byte
    }
}

fn read_array<const N: usize>(ports: &mut impl firmware_ports) -> [u8; N] {
    let mut bytes = [0; N];
    for byte in &mut bytes {
        *byte = ports.read_byte();
    }
    bytes
}

struct directory_scan {
    remaining: u32,
    expected_length: u64,
    selected: Option<u16>,
    name: &'static [u8],
}

impl directory_scan {
    fn new(
        count: u32,
        expected_length: u64,
        name: &'static [u8],
    ) -> Result<Self, source_load_error> {
        if count > directory_capacity {
            return Err(source_load_error::directory_count);
        }
        Ok(Self {
            remaining: count,
            expected_length,
            selected: None,
            name,
        })
    }

    fn observe(&mut self, entry: &[u8; 64]) -> Result<(), source_load_error> {
        if self.remaining == 0 {
            return Err(source_load_error::directory_count);
        }
        let name = &entry[8..];
        let end = name
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(source_load_error::directory_entry)?;
        if end == 0 || entry[6..8] != [0; 2] {
            return Err(source_load_error::directory_entry);
        }
        if &name[..end] == self.name {
            if self.selected.is_some() {
                return Err(source_load_error::duplicate_entry);
            }
            let length = u64::from(u32::from_be_bytes(entry[..4].try_into().unwrap()));
            let selector = u16::from_be_bytes(entry[4..6].try_into().unwrap());
            if length != self.expected_length {
                return Err(source_load_error::size);
            }
            if !(0x20..0x4000).contains(&selector) || name[end..].iter().any(|byte| *byte != 0) {
                return Err(source_load_error::directory_entry);
            }
            self.selected = Some(selector);
        }
        self.remaining -= 1;
        Ok(())
    }

    fn finish(self) -> Result<u16, source_load_error> {
        if self.remaining != 0 {
            return Err(source_load_error::directory_count);
        }
        self.selected.ok_or(source_load_error::missing_entry)
    }
}

struct firmware_stream {
    remaining: u64,
}
impl firmware_stream {
    fn open(
        ports: &mut impl firmware_ports,
        expected_length: u64,
        name: &'static [u8],
        byte_limit: u64,
    ) -> Result<Self, source_load_error> {
        validate_input(1, expected_length, name, byte_limit)?;
        ports.select(0);
        if read_array::<4>(ports) != *b"QEMU" {
            return Err(source_load_error::signature);
        }
        ports.select(1);
        if u32::from_le_bytes(read_array(ports)) & 1 == 0 {
            return Err(source_load_error::revision);
        }
        ports.select(0x19);
        let count = u32::from_be_bytes(read_array(ports));
        let mut scan = directory_scan::new(count, expected_length, name)?;
        for _ in 0..count {
            scan.observe(&read_array(ports))?;
        }
        ports.select(scan.finish()?);
        Ok(Self {
            remaining: expected_length,
        })
    }

    fn read(
        &mut self,
        ports: &mut impl firmware_ports,
        output: &mut [u8],
    ) -> Result<(), source_load_error> {
        if output.is_empty()
            || output.len() > source_chunk_limit
            || output.len() as u64 > self.remaining
        {
            return Err(source_load_error::chunk);
        }
        for byte in output.iter_mut() {
            *byte = ports.read_byte();
        }
        self.remaining -= output.len() as u64;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::boot_omen::memory_region;
    use crate::memory::physical::{frame_error, physical_range};
    use std::{vec, vec::Vec};

    fn entry(name: &[u8], selector: u16, length: u32) -> [u8; 64] {
        let mut entry = [0; 64];
        entry[..4].copy_from_slice(&length.to_be_bytes());
        entry[4..6].copy_from_slice(&selector.to_be_bytes());
        entry[8..8 + name.len()].copy_from_slice(name);
        entry
    }

    struct ports {
        files: Vec<(u16, Vec<u8>)>,
        selected: u16,
        offset: usize,
        selections: Vec<u16>,
        reads: usize,
    }

    impl ports {
        fn new(entries: &[[u8; 64]], payload: &[u8]) -> Self {
            let mut directory = (entries.len() as u32).to_be_bytes().to_vec();
            for item in entries {
                directory.extend_from_slice(item);
            }
            Self {
                files: vec![
                    (0, b"QEMU".to_vec()),
                    (1, 1u32.to_le_bytes().to_vec()),
                    (0x19, directory),
                    (0x31, payload.to_vec()),
                ],
                selected: 0,
                offset: 0,
                selections: Vec::new(),
                reads: 0,
            }
        }
    }

    impl firmware_ports for ports {
        fn select(&mut self, selector: u16) {
            self.selected = selector;
            self.offset = 0;
            self.selections.push(selector);
        }
        fn read_byte(&mut self) -> u8 {
            let byte = self
                .files
                .iter()
                .find(|item| item.0 == self.selected)
                .and_then(|item| item.1.get(self.offset))
                .copied()
                .unwrap_or(0);
            self.offset += 1;
            self.reads += 1;
            byte
        }
    }

    struct ram {
        bytes: Vec<u8>,
        fail_write: bool,
        fail_clear: bool,
    }

    // SAFETY: Isolated modeled RAM with checked extents and exact slice lengths;
    // no external pointer, MMIO, privileged mapping or task identity is used.
    unsafe impl source_memory for ram {
        unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
            let Some(end) = range.end().and_then(|v| usize::try_from(v).ok()) else {
                return false;
            };
            let Some(output) = self.bytes.get_mut(range.base as usize..end) else {
                return false;
            };
            if self.fail_clear {
                return false;
            }
            output.fill(0);
            output.iter().all(|byte| *byte == 0)
        }
        unsafe fn write(&mut self, range: physical_range, source: &[u8]) -> bool {
            let Some(end) = range.end().and_then(|v| usize::try_from(v).ok()) else {
                return false;
            };
            let Some(output) = self.bytes.get_mut(range.base as usize..end) else {
                return false;
            };
            if output.len() != source.len() {
                return false;
            }
            if self.fail_write {
                output[0] = source[0];
                return false;
            }
            output.copy_from_slice(source);
            true
        }
        unsafe fn read(&mut self, range: physical_range, destination: &mut [u8]) -> bool {
            let Some(end) = range.end().and_then(|v| usize::try_from(v).ok()) else {
                return false;
            };
            let Some(input) = self.bytes.get(range.base as usize..end) else {
                return false;
            };
            if input.len() != destination.len() {
                return false;
            }
            destination.copy_from_slice(input);
            true
        }
    }

    #[test]
    fn exact_directory_and_sequential_chunks_do_not_reselect_or_overread() {
        let payload: Vec<u8> = (0..4101).map(|i| (i as u8).wrapping_mul(7)).collect();
        let entries = [
            entry(b"unrelated", 0x20, 123),
            entry(file_name, 0x31, payload.len() as u32),
        ];
        let mut ports = ports::new(&entries, &payload);
        let mut stream =
            firmware_stream::open(&mut ports, 4101, file_name, source_byte_limit).unwrap();
        assert_eq!(ports.selections, [0, 1, 0x19, 0x31]);
        let mut first = [0; source_chunk_limit];
        stream.read(&mut ports, &mut first).unwrap();
        assert_eq!(&first[..], &payload[..4096]);
        let mut last = [0; 5];
        stream.read(&mut ports, &mut last).unwrap();
        assert_eq!(&last[..], &payload[4096..]);
        let reads = ports.reads;
        let mut untouched = [0xa5; 1];
        assert_eq!(
            stream.read(&mut ports, &mut untouched),
            Err(source_load_error::chunk)
        );
        assert_eq!(untouched, [0xa5]);
        assert_eq!(ports.reads, reads);
        assert_eq!(ports.selections, [0, 1, 0x19, 0x31]);
    }

    #[test]
    fn configured_item_is_selected_independently_of_the_original_source() {
        const second_name: &[u8] = b"opt/anomalous/linux-loader-file-source";
        let payload: Vec<u8> = (0..4101).map(|i| (i as u8).wrapping_mul(23)).collect();
        let entries = [
            entry(file_name, 0x30, 3),
            entry(second_name, 0x31, payload.len() as u32),
        ];
        let mut ports = ports::new(&entries, &payload);
        ports.files.push((0x30, b"elf".to_vec()));
        let mut original =
            firmware_stream::open(&mut ports, 3, file_name, source_byte_limit).unwrap();
        let mut first = [0; 3];
        original.read(&mut ports, &mut first).unwrap();
        assert_eq!(&first, b"elf");
        let mut second = firmware_stream::open(&mut ports, 4101, second_name, 4101).unwrap();
        let mut chunk = [0; source_chunk_limit];
        second.read(&mut ports, &mut chunk).unwrap();
        assert_eq!(&chunk[..], &payload[..4096]);
        let mut tail = [0; 5];
        second.read(&mut ports, &mut tail).unwrap();
        assert_eq!(&tail[..], &payload[4096..]);
        assert_eq!(ports.selections, [0, 1, 0x19, 0x30, 0, 1, 0x19, 0x31]);

        for entries in [
            vec![entry(file_name, 0x30, 3)],
            vec![entry(
                b"opt/anomalous/linux-loader-file-source-other",
                0x31,
                4101,
            )],
        ] {
            let mut ports = ports::new(&entries, &payload);
            assert_eq!(
                firmware_stream::open(&mut ports, 4101, second_name, 4101).err(),
                Some(source_load_error::missing_entry)
            );
            assert_eq!(ports.selections, [0, 1, 0x19]);
        }
        let mut ports = ports::new(&[entries[0], entries[1], entries[1]], &payload);
        assert_eq!(
            firmware_stream::open(&mut ports, 4101, second_name, 4101).err(),
            Some(source_load_error::duplicate_entry)
        );
        assert_eq!(ports.selections, [0, 1, 0x19]);
    }

    #[test]
    fn configured_budgets_round_pages_without_widening_original_limits() {
        const second_name: &[u8] = b"opt/anomalous/linux-loader-file-source";
        let controller = source_controller::new_configured(
            0x4c450002,
            2_190_608,
            17,
            second_name,
            8 * 1024 * 1024,
        )
        .unwrap();
        assert_eq!(controller.name, second_name);
        assert_eq!(controller.length(), 2_190_608);
        assert_eq!(controller.grant, 0x4c450002);
        assert_eq!(controller.expected_fnv, 17);
        assert_eq!(
            controller.budget,
            source_budget {
                max_bytes: 8 * 1024 * 1024,
                max_pages: 2048
            }
        );
        assert!(!controller.is_ready());
        assert!(controller.reader().is_err());
        assert!(matches!(
            source_controller::new(1, 2_190_608, 17),
            Err(source_load_error::invalid_input)
        ));
        let original = source_controller::new(1, source_byte_limit, 19).unwrap();
        assert_eq!(original.name, file_name);
        assert_eq!(
            original.budget,
            source_budget {
                max_bytes: source_byte_limit,
                max_pages: 128
            }
        );
        let rounded = source_controller::new_configured(2, 4097, 0, second_name, 4097).unwrap();
        assert_eq!(
            rounded.budget,
            source_budget {
                max_bytes: 4097,
                max_pages: 2
            }
        );
    }

    #[test]
    fn malformed_configured_names_and_budgets_touch_no_firmware() {
        const longest_name: &[u8] = &[b'x'; 55];
        let mut ports = ports::new(&[entry(longest_name, 0x31, 1)], b"x");
        let mut stream = firmware_stream::open(&mut ports, 1, longest_name, 1).unwrap();
        let mut byte = [0];
        stream.read(&mut ports, &mut byte).unwrap();
        assert_eq!(&byte, b"x");
        for (length, name, byte_limit) in [
            (0, file_name, source_byte_limit),
            (1, file_name, 0),
            (2, file_name, 1),
            (1, file_name, u64::from(u32::MAX) + 1),
            (u64::MAX, file_name, u64::MAX),
            (1, b"".as_slice(), source_byte_limit),
            (1, b"embedded\0nul".as_slice(), source_byte_limit),
            (1, &[b'x'; 56], source_byte_limit),
        ] {
            assert!(matches!(
                source_controller::new_configured(1, length, 0, name, byte_limit),
                Err(source_load_error::invalid_input)
            ));
            let mut ports = ports::new(&[], &[]);
            assert_eq!(
                firmware_stream::open(&mut ports, length, name, byte_limit).err(),
                Some(source_load_error::invalid_input)
            );
            assert!(ports.selections.is_empty());
            assert_eq!(ports.reads, 0);
        }
        assert!(matches!(
            source_controller::new_configured(0, 1, 0, file_name, source_byte_limit),
            Err(source_load_error::invalid_input)
        ));
    }

    #[test]
    fn malformed_directory_never_selects_a_payload() {
        let valid = entry(file_name, 0x31, 3);
        let mut malformed = Vec::new();
        malformed.push((vec![], source_load_error::missing_entry));
        malformed.push((vec![valid, valid], source_load_error::duplicate_entry));
        malformed.push((vec![entry(file_name, 0x31, 2)], source_load_error::size));
        malformed.push((vec![entry(file_name, 0x31, 4)], source_load_error::size));
        for selector in [0, 0x19, 0x4000, 0x8000, 0xffff] {
            malformed.push((
                vec![entry(file_name, selector, 3)],
                source_load_error::directory_entry,
            ));
        }
        let mut padding = valid;
        padding[63] = 1;
        malformed.push((vec![padding], source_load_error::directory_entry));
        let mut reserved = valid;
        reserved[6] = 1;
        malformed.push((vec![reserved], source_load_error::directory_entry));
        let mut unterminated = valid;
        unterminated[8..].fill(b'x');
        malformed.push((vec![unterminated], source_load_error::directory_entry));
        malformed.push((
            vec![valid; directory_capacity as usize + 1],
            source_load_error::directory_count,
        ));
        for (entries, expected) in malformed {
            let mut ports = ports::new(&entries, b"elf");
            assert_eq!(
                firmware_stream::open(&mut ports, 3, file_name, source_byte_limit).err(),
                Some(expected)
            );
            assert_eq!(ports.selections, [0, 1, 0x19]);
        }
        let mut scan = directory_scan::new(2, 3, file_name).unwrap();
        scan.observe(&valid).unwrap();
        assert_eq!(scan.finish(), Err(source_load_error::directory_count));
    }

    #[test]
    fn absent_firmware_bad_revision_and_invalid_budget_fail_before_payload() {
        for length in [0, source_byte_limit + 1, u64::MAX] {
            assert!(matches!(
                source_controller::new(1, length, 0),
                Err(source_load_error::invalid_input)
            ));
            let mut ports = ports::new(&[], &[]);
            assert_eq!(
                firmware_stream::open(&mut ports, length, file_name, source_byte_limit).err(),
                Some(source_load_error::invalid_input)
            );
            assert!(ports.selections.is_empty());
        }
        assert!(matches!(
            source_controller::new(0, 1, 0),
            Err(source_load_error::invalid_input)
        ));
        for (index, bytes, error, selections) in [
            (0, *b"NONE", source_load_error::signature, 1),
            (1, 2u32.to_le_bytes(), source_load_error::revision, 2),
        ] {
            let mut ports = ports::new(&[entry(file_name, 0x31, 3)], b"elf");
            ports.files[index].1 = bytes.to_vec();
            assert_eq!(
                firmware_stream::open(&mut ports, 3, file_name, source_byte_limit).err(),
                Some(error)
            );
            assert_eq!(ports.selections.len(), selections);
        }
        assert_eq!(hash_bytes(fnv_offset, b"hello"), 0xa430d84680aabd0b);
    }

    #[test]
    fn real_provisioned_lease_transfer_is_sealed_only_after_exact_hash_and_cleans_on_failure() {
        // Success, wrong hash, short physical payload returning firmware zeros,
        // and a partial storage failure all use the actual retained owner.
        for failure in 0..4 {
            let mut ledger = [0; 4];
            let mut allocator = frame_allocator::initialize(
                &mut ledger,
                &[memory_region {
                    base: 0,
                    length: 16 * 4096,
                    kind: 1,
                    attributes: 1,
                }],
                &[physical_range::new(0, 4096)],
            )
            .unwrap();
            let before = allocator.accounting();
            let payload: Vec<u8> = (0..4101)
                .map(|i| (i as u8).wrapping_mul(17).wrapping_add(3))
                .collect();
            let physical = if failure == 2 {
                &payload[..4097]
            } else {
                &payload[..]
            };
            let mut ports = ports::new(&[entry(file_name, 0x31, 4101)], physical);
            let mut stream =
                firmware_stream::open(&mut ports, 4101, file_name, source_byte_limit).unwrap();
            let mut memory = ram {
                bytes: vec![0xa5; 16 * 4096],
                fail_write: failure == 3,
                fail_clear: false,
            };
            // SAFETY: Isolated modeled controller, allocator and storage.
            let mut source = unsafe {
                owned_source::new_provisioned(
                    7,
                    4101,
                    source_budget {
                        max_bytes: source_byte_limit,
                        max_pages: 128,
                    },
                )
            }
            .unwrap();
            unsafe { source.allocate(&mut allocator, &mut memory) }.unwrap();
            let range = source.retained_range().unwrap();
            let mut scratch = [0; source_chunk_limit];
            let expected = hash_bytes(fnv_offset, &payload) ^ u64::from(failure == 1);
            let result = unsafe {
                transfer(
                    &mut source,
                    7,
                    &mut stream,
                    &mut ports,
                    &mut memory,
                    &mut scratch,
                    expected,
                )
            };
            assert!(scratch.iter().all(|byte| *byte == 0));
            if failure == 0 {
                assert_eq!(result, Ok((expected, 2)));
                let reader = source.reader().unwrap();
                assert_eq!(reader.uploader(), None);
                let mut observed = [0; 5];
                unsafe { reader.read(4096, &mut observed, &mut memory) }.unwrap();
                assert_eq!(&observed[..], &payload[4096..]);
                assert!(
                    memory.bytes[(range.base + 4101) as usize..range.end().unwrap() as usize]
                        .iter()
                        .all(|byte| *byte == 0)
                );
            } else {
                assert_eq!(
                    result,
                    Err(if failure == 3 {
                        source_load_error::source(source_error::write_failed)
                    } else {
                        source_load_error::hash
                    })
                );
                assert!(!source.is_sealed());
                assert!(source.reader().is_err());
                assert_eq!(source.received(), if failure == 3 { 0 } else { 4101 });
            }
            memory.fail_clear = true;
            assert_eq!(
                unsafe { source.cleanup(&mut allocator, &mut memory) },
                Err(source_error::frame(frame_error::clearing_failed))
            );
            assert_eq!(source.retained_range(), Some(range));
            assert_eq!(
                allocator.accounting().active_leases,
                before.active_leases + 1
            );
            assert_eq!(allocator.accounting().bound_leases, before.bound_leases);
            memory.fail_clear = false;
            unsafe { source.cleanup(&mut allocator, &mut memory) }.unwrap();
            assert_eq!(allocator.accounting(), before);
            assert!(
                memory.bytes[range.base as usize..range.end().unwrap() as usize]
                    .iter()
                    .all(|byte| *byte == 0)
            );
        }
    }
}
