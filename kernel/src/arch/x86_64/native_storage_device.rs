//! ADR 0063's temporary, feature-private sector mechanism. The coordinator owns
//! one device for the entire VM lifetime; task/route authority and complete
//! user-copy admission belong to the calling runtime, never to disk metadata.

use core::{
    arch::asm,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) const sector_bytes: usize = 512;
pub(super) const extent_sectors: u64 = 5;
pub(super) const poll_budget: u32 = 1_000_000;
const lba28_limit: u64 = 1 << 28;
const data_port: u16 = 0x170;
const count_port: u16 = 0x172;
const lba_low_port: u16 = 0x173;
const lba_mid_port: u16 = 0x174;
const lba_high_port: u16 = 0x175;
const select_port: u16 = 0x176;
const command_port: u16 = 0x177;
const control_port: u16 = 0x376;
const status_busy: u8 = 0x80;
const status_ready: u8 = 0x40;
const status_fault: u8 = 0x20;
const status_data: u8 = 0x08;
const status_error: u8 = 0x01;
const identify_command: u8 = 0xec;
const read_command: u8 = 0x20;
const write_command: u8 = 0x30;
const flush_command: u8 = 0xe7;

static controller_claimed: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum storage_error {
    interrupts_enabled,
    already_owned,
    invalid_extent,
    outside_extent,
    not_identified,
    already_identified,
    unsupported_device,
    unsupported_geometry,
    unsupported_flush,
    insufficient_capacity,
    absent_device,
    device_error,
    unexpected_transfer,
    poll_exhausted,
    poisoned,
    #[cfg(feature = "native_durable_failure_test")]
    observation_loss_armed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct geometry {
    pub sectors: u64,
    pub logical_sector_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct io_counters {
    pub port_reads: u64,
    pub port_writes: u64,
    pub commands_issued: u64,
    pub commands_completed: u64,
    pub sectors_read: u64,
    pub sectors_written: u64,
    pub flushes: u64,
}

pub(super) struct device {
    transport: transport<hardware_ports>,
}

impl device {
    /// Claims ownership without touching a port. There is deliberately no Drop
    /// release, reset, unpoison, or second acquisition in this kernel boot.
    ///
    /// # Safety
    /// The sole BSP must own the explicitly provisioned secondary ISA IDE
    /// channel, with only the disposable data disk at master/unit zero. No
    /// firmware, other driver, AP, interrupt, or NMI may access these ports.
    /// The proof coordinator must retain this owner across service retirement.
    pub unsafe fn acquire(extent_start: u64) -> Result<Self, storage_error> {
        claim_owner(&controller_claimed, extent_start, interrupts_clear())?;
        Ok(Self {
            transport: transport::new(hardware_ports, extent_start)?,
        })
    }

    pub fn identify(&mut self) -> Result<geometry, storage_error> {
        self.transport.identify()
    }

    pub fn read_sector(
        &mut self,
        relative: u64,
        destination: &mut [u8; sector_bytes],
    ) -> Result<(), storage_error> {
        self.transport.read_sector(relative, destination)
    }

    pub fn write_sector(
        &mut self,
        relative: u64,
        source: &[u8; sector_bytes],
    ) -> Result<(), storage_error> {
        self.transport.write_sector(relative, source)
    }

    pub fn flush(&mut self) -> Result<(), storage_error> {
        self.transport.flush()
    }

    pub fn is_poisoned(&self) -> bool {
        self.transport.poisoned
    }

    pub fn geometry(&self) -> Option<geometry> {
        self.transport.geometry
    }

    pub fn counters(&self) -> io_counters {
        self.transport.counters
    }

    /// Feature-private proof control, never a task-selected device operation.
    /// The second subsequently issued FLUSH loses completion observation.
    #[cfg(feature = "native_durable_failure_test")]
    pub fn arm_flush_observation_loss(&mut self) -> Result<(), storage_error> {
        self.transport.arm_flush_observation_loss()
    }

    /// Local grant preflight only: no status register is sampled. A pending
    /// injection or poisoned owner cannot acquire a successor service grant.
    #[cfg(feature = "native_durable_failure_test")]
    pub fn grant_available(&self) -> bool {
        self.transport.grant_available()
    }
}

fn valid_extent(start: u64) -> bool {
    start
        .checked_add(extent_sectors)
        .is_some_and(|end| end <= lba28_limit)
}

fn claim_owner(claimed: &AtomicBool, start: u64, if_clear: bool) -> Result<(), storage_error> {
    if !if_clear {
        return Err(storage_error::interrupts_enabled);
    }
    if !valid_extent(start) {
        return Err(storage_error::invalid_extent);
    }
    claimed
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| storage_error::already_owned)?;
    Ok(())
}

trait port_io {
    fn interrupts_clear(&self) -> bool;
    fn read_byte(&mut self, port: u16) -> u8;
    fn write_byte(&mut self, port: u16, value: u8);
    fn read_word(&mut self, port: u16) -> u16;
    fn write_word(&mut self, port: u16, value: u16);
}

struct transport<P> {
    ports: P,
    extent_start: u64,
    geometry: Option<geometry>,
    poisoned: bool,
    counters: io_counters,
    #[cfg(feature = "native_durable_failure_test")]
    flush_observations_remaining: u8,
}

impl<P: port_io> transport<P> {
    fn new(ports: P, extent_start: u64) -> Result<Self, storage_error> {
        if !valid_extent(extent_start) {
            return Err(storage_error::invalid_extent);
        }
        Ok(Self {
            ports,
            extent_start,
            geometry: None,
            poisoned: false,
            counters: io_counters::default(),
            #[cfg(feature = "native_durable_failure_test")]
            flush_observations_remaining: 0,
        })
    }

    #[cfg(feature = "native_durable_failure_test")]
    fn arm_flush_observation_loss(&mut self) -> Result<(), storage_error> {
        self.preflight(true)?;
        if self.flush_observations_remaining != 0 {
            return Err(storage_error::observation_loss_armed);
        }
        // All operations are synchronous under an exclusive borrow. Any path
        // that returned without observing completion has already poisoned us.
        self.flush_observations_remaining = 2;
        Ok(())
    }

    #[cfg(feature = "native_durable_failure_test")]
    fn grant_available(&self) -> bool {
        self.preflight(true).is_ok() && self.flush_observations_remaining == 0
    }

    // This is strictly local validation: even an alternate-status read would
    // violate the no-device-I/O rejection contract.
    fn preflight(&self, identified: bool) -> Result<(), storage_error> {
        if self.poisoned {
            return Err(storage_error::poisoned);
        }
        if !self.ports.interrupts_clear() {
            return Err(storage_error::interrupts_enabled);
        }
        if identified && self.geometry.is_none() {
            return Err(storage_error::not_identified);
        }
        Ok(())
    }

    fn sector_address(&self, relative: u64) -> Result<u32, storage_error> {
        self.preflight(true)?;
        if relative >= extent_sectors {
            return Err(storage_error::outside_extent);
        }
        let address = self
            .extent_start
            .checked_add(relative)
            .filter(|address| *address < lba28_limit)
            .ok_or(storage_error::outside_extent)?;
        if address >= self.geometry.ok_or(storage_error::not_identified)?.sectors {
            return Err(storage_error::outside_extent);
        }
        Ok(address as u32)
    }

    fn identify(&mut self) -> Result<geometry, storage_error> {
        self.preflight(false)?;
        if self.geometry.is_some() {
            return Err(storage_error::already_identified);
        }
        let mut bytes = [0; sector_bytes];
        let mut budget = poll_budget;
        let result = (|| {
            self.select(0xa0, &mut budget)?;
            self.write_byte(count_port, 0);
            self.write_byte(lba_low_port, 0);
            self.write_byte(lba_mid_port, 0);
            self.write_byte(lba_high_port, 0);
            self.issue(identify_command);
            self.settle(&mut budget)?;
            self.wait(true, &mut budget)?;
            if self.read_byte(lba_mid_port) != 0 || self.read_byte(lba_high_port) != 0 {
                return Err(storage_error::unsupported_device);
            }
            self.receive(&mut bytes, &mut budget)?;
            self.finish(&mut budget)?;
            decode_geometry(&bytes, self.extent_start)
        })();
        clear_sector(&mut bytes);
        match result {
            Ok(value) => {
                self.geometry = Some(value);
                Ok(value)
            }
            Err(error) => self.poison(error),
        }
    }

    fn read_sector(
        &mut self,
        relative: u64,
        destination: &mut [u8; sector_bytes],
    ) -> Result<(), storage_error> {
        let address = self.sector_address(relative)?;
        let mut bytes = [0; sector_bytes];
        let mut budget = poll_budget;
        let result = (|| {
            self.begin_sector(address, read_command, &mut budget)?;
            self.wait(true, &mut budget)?;
            self.receive(&mut bytes, &mut budget)?;
            self.finish(&mut budget)
        })();
        if result.is_ok() {
            destination.copy_from_slice(&bytes);
            self.counters.sectors_read = self.counters.sectors_read.saturating_add(1);
        }
        clear_sector(&mut bytes);
        result.or_else(|error| self.poison(error))
    }

    fn write_sector(
        &mut self,
        relative: u64,
        source: &[u8; sector_bytes],
    ) -> Result<(), storage_error> {
        let address = self.sector_address(relative)?;
        let mut budget = poll_budget;
        let result = (|| {
            self.begin_sector(address, write_command, &mut budget)?;
            self.wait(true, &mut budget)?;
            for bytes in source.chunks_exact(2) {
                self.transfer_ready(&mut budget)?;
                self.write_word(data_port, u16::from_le_bytes([bytes[0], bytes[1]]));
            }
            self.finish(&mut budget)
        })();
        result.or_else(|error| self.poison(error))?;
        self.counters.sectors_written = self.counters.sectors_written.saturating_add(1);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), storage_error> {
        self.preflight(true)?;
        let mut budget = poll_budget;
        let result = (|| {
            self.select(0xe0, &mut budget)?;
            self.issue(flush_command);
            #[cfg(feature = "native_durable_failure_test")]
            if self.flush_observations_remaining != 0 {
                self.flush_observations_remaining -= 1;
                if self.flush_observations_remaining == 0 {
                    // The final save FLUSH was actually issued. Deliberately
                    // lose its completion observation before the first status
                    // sample; normal finish -> poll_exhausted -> poison owns
                    // the failure. This is not a measured hardware timeout.
                    budget = 0;
                }
            }
            self.finish(&mut budget)
        })();
        result.or_else(|error| self.poison(error))?;
        self.counters.flushes = self.counters.flushes.saturating_add(1);
        Ok(())
    }

    fn select(&mut self, selector: u8, budget: &mut u32) -> Result<(), storage_error> {
        // nIEN; no SRST and no reset path. Both channel units are exclusively
        // reserved, so masking this channel cannot affect the AHCI boot disk.
        self.write_byte(control_port, 0x02);
        // Firmware may leave the absent slave selected. A floating/zero status
        // is therefore admissible only BEFORE explicitly selecting our master.
        loop {
            let status = self.sample(budget)?;
            if status != 0xff && status & status_busy != 0 {
                core::hint::spin_loop();
                continue;
            }
            if status != 0xff && status & status_data != 0 {
                return Err(storage_error::unexpected_transfer);
            }
            break;
        }
        self.write_byte(select_port, selector);
        self.settle(budget)?;
        self.wait(false, budget)
    }

    fn begin_sector(
        &mut self,
        address: u32,
        command: u8,
        budget: &mut u32,
    ) -> Result<(), storage_error> {
        self.select(0xe0 | ((address >> 24) as u8 & 0x0f), budget)?;
        self.write_byte(count_port, 1);
        self.write_byte(lba_low_port, address as u8);
        self.write_byte(lba_mid_port, (address >> 8) as u8);
        self.write_byte(lba_high_port, (address >> 16) as u8);
        self.issue(command);
        self.settle(budget)
    }

    fn issue(&mut self, command: u8) {
        self.write_byte(command_port, command);
        self.counters.commands_issued = self.counters.commands_issued.saturating_add(1);
    }

    fn receive(
        &mut self,
        bytes: &mut [u8; sector_bytes],
        budget: &mut u32,
    ) -> Result<(), storage_error> {
        for destination in bytes.chunks_exact_mut(2) {
            self.transfer_ready(budget)?;
            destination.copy_from_slice(&self.read_word(data_port).to_le_bytes());
        }
        Ok(())
    }

    fn finish(&mut self, budget: &mut u32) -> Result<(), storage_error> {
        self.settle(budget)?;
        self.wait(false, budget)?;
        self.counters.commands_completed = self.counters.commands_completed.saturating_add(1);
        Ok(())
    }

    fn transfer_ready(&mut self, budget: &mut u32) -> Result<(), storage_error> {
        let status = self.sample(budget)?;
        check_status(status)?;
        if status & (status_busy | status_ready | status_data) != (status_ready | status_data) {
            return Err(storage_error::unexpected_transfer);
        }
        Ok(())
    }

    fn wait(&mut self, data: bool, budget: &mut u32) -> Result<(), storage_error> {
        loop {
            let status = self.sample(budget)?;
            if status & status_busy != 0 && status != 0xff {
                core::hint::spin_loop();
                continue;
            }
            check_status(status)?;
            if status & status_ready == 0 {
                core::hint::spin_loop();
                continue;
            }
            if (status & status_data != 0) == data {
                return Ok(());
            }
            if !data {
                return Err(storage_error::unexpected_transfer);
            }
            core::hint::spin_loop();
        }
    }

    fn settle(&mut self, budget: &mut u32) -> Result<(), storage_error> {
        // Four ordered alternate-status reads are a settling convention for
        // this qualified emulated controller, not a physical wall-clock timer.
        for _ in 0..4 {
            self.sample(budget)?;
        }
        Ok(())
    }

    fn sample(&mut self, budget: &mut u32) -> Result<u8, storage_error> {
        if *budget == 0 {
            return Err(storage_error::poll_exhausted);
        }
        *budget -= 1;
        Ok(self.read_byte(control_port))
    }

    fn poison<T>(&mut self, error: storage_error) -> Result<T, storage_error> {
        self.poisoned = true;
        Err(error)
    }

    fn read_byte(&mut self, port: u16) -> u8 {
        self.counters.port_reads = self.counters.port_reads.saturating_add(1);
        self.ports.read_byte(port)
    }

    fn write_byte(&mut self, port: u16, value: u8) {
        self.counters.port_writes = self.counters.port_writes.saturating_add(1);
        self.ports.write_byte(port, value);
    }

    fn read_word(&mut self, port: u16) -> u16 {
        self.counters.port_reads = self.counters.port_reads.saturating_add(1);
        self.ports.read_word(port)
    }

    fn write_word(&mut self, port: u16, value: u16) {
        self.counters.port_writes = self.counters.port_writes.saturating_add(1);
        self.ports.write_word(port, value);
    }
}

fn check_status(status: u8) -> Result<(), storage_error> {
    if status == 0 || status == 0xff {
        return Err(storage_error::absent_device);
    }
    if status & (status_error | status_fault) != 0 {
        return Err(storage_error::device_error);
    }
    Ok(())
}

fn decode_geometry(bytes: &[u8; sector_bytes], start: u64) -> Result<geometry, storage_error> {
    let word = |index: usize| u16::from_le_bytes([bytes[index * 2], bytes[index * 2 + 1]]);
    if word(0) & 0x8000 != 0 || word(49) & (1 << 9) == 0 {
        return Err(storage_error::unsupported_device);
    }
    // Strict proof profile: require an explicitly valid word 106 and a
    // 256-word logical sector. No legacy inference or 4Kn transfer is admitted.
    if word(106) & 0xd000 != 0x4000 {
        return Err(storage_error::unsupported_geometry);
    }
    if word(83) & 0xd000 != 0x5000 || word(86) & (1 << 12) == 0 {
        return Err(storage_error::unsupported_flush);
    }
    let sectors = u64::from(word(60)) | (u64::from(word(61)) << 16);
    if sectors == 0
        || sectors >= lba28_limit
        || !valid_extent(start)
        || start + extent_sectors > sectors
    {
        return Err(storage_error::insufficient_capacity);
    }
    Ok(geometry {
        sectors,
        logical_sector_bytes: sector_bytes as u64,
    })
}

fn clear_sector(bytes: &mut [u8; sector_bytes]) {
    for byte in bytes {
        // SAFETY: Every reference names one distinct live byte of this private
        // buffer. Volatile stores ensure clearing survives dead-store removal.
        unsafe { ptr::write_volatile(byte, 0) };
    }
}

struct hardware_ports;

fn interrupts_clear() -> bool {
    let flags: u64;
    // SAFETY: Reads this CPU's flags without changing IF or accessing a device.
    unsafe {
        asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags));
    }
    flags & (1 << 9) == 0
}

impl port_io for hardware_ports {
    fn interrupts_clear(&self) -> bool {
        interrupts_clear()
    }

    fn read_byte(&mut self, port: u16) -> u8 {
        let value: u8;
        // SAFETY: Only the retained unique owner constructs this backend; all
        // accesses use the provisioned channel while the sole BSP has IF clear.
        unsafe {
            asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
        }
        value
    }

    fn write_byte(&mut self, port: u16, value: u8) {
        // SAFETY: Same fixed-port, exclusive-owner, IF-clear contract as above.
        unsafe {
            asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
        }
    }

    fn read_word(&mut self, port: u16) -> u16 {
        let value: u16;
        // SAFETY: Only the fixed data register is used during admitted DRQ.
        unsafe {
            asm!("in ax, dx", in("dx") port, out("ax") value, options(nomem, nostack, preserves_flags));
        }
        value
    }

    fn write_word(&mut self, port: u16, value: u16) {
        // SAFETY: Only the fixed data register is used during admitted DRQ.
        unsafe {
            asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    struct mock_ports {
        if_clear: bool,
        selected: bool,
        absent_before_select: bool,
        command: u8,
        transferred: usize,
        short_after: Option<usize>,
        busy_command: Option<u8>,
        completion_error: bool,
        identify: [u8; sector_bytes],
        sector: [u8; sector_bytes],
        words_written: Vec<u16>,
        byte_writes: Vec<(u16, u8)>,
        reads: u64,
        writes: u64,
        #[cfg(feature = "native_durable_failure_test")]
        reads_at_last_issue: u64,
        #[cfg(feature = "native_durable_failure_test")]
        writes_at_last_issue: u64,
    }

    impl mock_ports {
        fn new() -> Self {
            let mut identify = [0; sector_bytes];
            for (word, value) in [
                (0, 0x40u16),
                (49, 0x200),
                (60, 16384),
                (83, 0x5000),
                (86, 0x1000),
                (106, 0x6000),
            ] {
                identify[word * 2..word * 2 + 2].copy_from_slice(&value.to_le_bytes());
            }
            Self {
                if_clear: true,
                selected: false,
                absent_before_select: false,
                command: 0,
                transferred: 0,
                short_after: None,
                busy_command: None,
                completion_error: false,
                identify,
                sector: core::array::from_fn(|index| (index as u8).wrapping_mul(31)),
                words_written: Vec::new(),
                byte_writes: Vec::new(),
                reads: 0,
                writes: 0,
                #[cfg(feature = "native_durable_failure_test")]
                reads_at_last_issue: 0,
                #[cfg(feature = "native_durable_failure_test")]
                writes_at_last_issue: 0,
            }
        }

        fn status(&self) -> u8 {
            if self.absent_before_select && !self.selected {
                return 0xff;
            }
            if self.busy_command == Some(self.command) {
                return status_busy;
            }
            if matches!(
                self.command,
                identify_command | read_command | write_command
            ) {
                if self
                    .short_after
                    .is_some_and(|limit| self.transferred >= limit)
                    && self.transferred < sector_bytes / 2
                {
                    return status_ready;
                }
                if self.transferred < sector_bytes / 2 {
                    return status_ready | status_data;
                }
            }
            if self.command != 0 && self.completion_error {
                return status_ready | status_error;
            }
            status_ready
        }
    }

    impl port_io for mock_ports {
        fn interrupts_clear(&self) -> bool {
            self.if_clear
        }

        fn read_byte(&mut self, port: u16) -> u8 {
            self.reads += 1;
            match port {
                control_port => self.status(),
                lba_mid_port | lba_high_port => 0,
                _ => panic!("unexpected byte read"),
            }
        }

        fn write_byte(&mut self, port: u16, value: u8) {
            self.writes += 1;
            self.byte_writes.push((port, value));
            if port == command_port {
                self.command = value;
                self.transferred = 0;
                #[cfg(feature = "native_durable_failure_test")]
                {
                    self.reads_at_last_issue = self.reads;
                    self.writes_at_last_issue = self.writes;
                }
            } else if port == select_port {
                self.selected = true;
            }
        }

        fn read_word(&mut self, port: u16) -> u16 {
            assert_eq!(port, data_port);
            assert!(matches!(self.command, identify_command | read_command));
            self.reads += 1;
            let source = if self.command == identify_command {
                &self.identify
            } else {
                &self.sector
            };
            let offset = self.transferred * 2;
            self.transferred += 1;
            u16::from_le_bytes([source[offset], source[offset + 1]])
        }

        fn write_word(&mut self, port: u16, value: u16) {
            assert_eq!(port, data_port);
            assert_eq!(self.command, write_command);
            self.writes += 1;
            self.transferred += 1;
            self.words_written.push(value);
        }
    }

    fn ready() -> transport<mock_ports> {
        let mut result = transport::new(mock_ports::new(), 8).unwrap();
        assert_eq!(
            result.identify().unwrap(),
            geometry {
                sectors: 16384,
                logical_sector_bytes: 512
            }
        );
        result
    }

    fn assert_port_accounting(transport: &transport<mock_ports>) {
        assert_eq!(transport.counters.port_reads, transport.ports.reads);
        assert_eq!(transport.counters.port_writes, transport.ports.writes);
    }

    #[test]
    fn local_claim_is_single_use_and_invalid_attempts_do_not_consume_it() {
        let claimed = AtomicBool::new(false);
        assert_eq!(
            claim_owner(&claimed, 8, false),
            Err(storage_error::interrupts_enabled)
        );
        for start in [u64::MAX, lba28_limit - 4, lba28_limit] {
            assert_eq!(
                claim_owner(&claimed, start, true),
                Err(storage_error::invalid_extent)
            );
        }
        assert!(!claimed.load(Ordering::Relaxed));
        assert_eq!(claim_owner(&claimed, 8, true), Ok(()));
        assert_eq!(
            claim_owner(&claimed, 8, true),
            Err(storage_error::already_owned)
        );
    }

    #[test]
    fn unqualified_and_outside_extent_requests_touch_no_ports_or_destination() {
        let mut device = transport::new(mock_ports::new(), 8).unwrap();
        let mut bytes = [0xa5; sector_bytes];
        assert_eq!(
            device.read_sector(0, &mut bytes),
            Err(storage_error::not_identified)
        );
        assert_eq!(
            device.write_sector(0, &bytes),
            Err(storage_error::not_identified)
        );
        assert_eq!(device.flush(), Err(storage_error::not_identified));
        assert_eq!(device.counters, io_counters::default());
        device.identify().unwrap();
        let counters = device.counters;
        for sector in [5, 8, lba28_limit, u64::MAX] {
            assert_eq!(
                device.read_sector(sector, &mut bytes),
                Err(storage_error::outside_extent)
            );
            assert_eq!(
                device.write_sector(sector, &bytes),
                Err(storage_error::outside_extent)
            );
        }
        assert_eq!(device.identify(), Err(storage_error::already_identified));
        assert_eq!(device.counters, counters);
        assert_eq!(bytes, [0xa5; sector_bytes]);
        assert_port_accounting(&device);
    }

    #[test]
    fn if_enabled_rejects_every_operation_without_ports_or_poison() {
        let mut device = ready();
        device.ports.if_clear = false;
        let counters = device.counters;
        let mut bytes = [0xa5; sector_bytes];
        assert_eq!(
            device.read_sector(0, &mut bytes),
            Err(storage_error::interrupts_enabled)
        );
        assert_eq!(
            device.write_sector(0, &bytes),
            Err(storage_error::interrupts_enabled)
        );
        assert_eq!(device.flush(), Err(storage_error::interrupts_enabled));
        assert_eq!(device.identify(), Err(storage_error::interrupts_enabled));
        assert_eq!(device.counters, counters);
        assert_eq!(bytes, [0xa5; sector_bytes]);
        assert!(!device.poisoned);
        assert_port_accounting(&device);
    }

    #[test]
    fn identify_selects_master_after_firmware_left_absent_slave() {
        let mut ports = mock_ports::new();
        ports.absent_before_select = true;
        let mut device = transport::new(ports, 8).unwrap();
        device.identify().unwrap();
        assert_eq!(device.ports.byte_writes[0], (control_port, 2));
        assert_eq!(device.ports.byte_writes[1], (select_port, 0xa0));
        assert_eq!(device.counters.commands_issued, 1);
        assert_eq!(device.counters.commands_completed, 1);
        assert_eq!(device.ports.transferred, 256);
        assert_port_accounting(&device);
    }

    #[test]
    fn qualification_rejects_wrong_device_sector_flush_and_capacity_before_writes() {
        for (word, value, expected) in [
            (0, 0x8000, storage_error::unsupported_device),
            (49, 0, storage_error::unsupported_device),
            (106, 0, storage_error::unsupported_geometry),
            (106, 0x5000, storage_error::unsupported_geometry),
            (83, 0x4000, storage_error::unsupported_flush),
            (83, 0xd000, storage_error::unsupported_flush),
            (86, 0, storage_error::unsupported_flush),
            (60, 12, storage_error::insufficient_capacity),
        ] {
            let mut ports = mock_ports::new();
            ports.identify[word * 2..word * 2 + 2].copy_from_slice(&(value as u16).to_le_bytes());
            let mut device = transport::new(ports, 8).unwrap();
            assert_eq!(device.identify(), Err(expected));
            assert!(device.poisoned);
            let counters = device.counters;
            assert_eq!(
                device.write_sector(0, &[0; sector_bytes]),
                Err(storage_error::poisoned)
            );
            assert_eq!(device.flush(), Err(storage_error::poisoned));
            assert_eq!(device.identify(), Err(storage_error::poisoned));
            assert_eq!(device.counters, counters);
            assert_eq!(device.counters.sectors_written, 0);
            assert_port_accounting(&device);
        }
    }

    #[test]
    fn successful_sector_commands_are_exact_and_flush_is_separate() {
        let mut device = ready();
        let mut destination = [0; sector_bytes];
        device.read_sector(4, &mut destination).unwrap();
        assert_eq!(destination, device.ports.sector);
        assert!(device.ports.byte_writes.ends_with(&[
            (control_port, 2),
            (select_port, 0xe0),
            (count_port, 1),
            (lba_low_port, 12),
            (lba_mid_port, 0),
            (lba_high_port, 0),
            (command_port, read_command),
        ]));
        let source = core::array::from_fn(|index| (index as u8).wrapping_add(19));
        device.write_sector(0, &source).unwrap();
        assert_eq!(device.ports.words_written.len(), 256);
        for (index, value) in device.ports.words_written.iter().enumerate() {
            assert_eq!(
                *value,
                u16::from_le_bytes([source[index * 2], source[index * 2 + 1]])
            );
        }
        assert_eq!(device.counters.flushes, 0);
        device.flush().unwrap();
        assert_eq!(device.counters.commands_issued, 4);
        assert_eq!(device.counters.commands_completed, 4);
        assert_eq!(device.counters.sectors_read, 1);
        assert_eq!(device.counters.sectors_written, 1);
        assert_eq!(device.counters.flushes, 1);
        assert_port_accounting(&device);
    }

    #[test]
    fn short_read_never_publishes_partial_bytes_and_poisons_without_retry() {
        let mut device = ready();
        device.ports.short_after = Some(19);
        let mut destination = [0xa5; sector_bytes];
        assert_eq!(
            device.read_sector(0, &mut destination),
            Err(storage_error::unexpected_transfer)
        );
        assert_eq!(destination, [0xa5; sector_bytes]);
        assert_eq!(device.ports.transferred, 19);
        assert!(device.poisoned);
        let counters = device.counters;
        assert_eq!(
            device.read_sector(0, &mut destination),
            Err(storage_error::poisoned)
        );
        assert_eq!(
            device.write_sector(0, &destination),
            Err(storage_error::poisoned)
        );
        assert_eq!(device.flush(), Err(storage_error::poisoned));
        assert_eq!(device.counters, counters);
        assert_port_accounting(&device);
    }

    #[test]
    fn read_completion_failure_keeps_destination_after_full_transfer() {
        let mut device = ready();
        device.ports.completion_error = true;
        // Clear the mock's previous IDENTIFY command; the injected failure is
        // at the next command completion, not the pre-command idle observation.
        device.ports.command = 0;
        let mut destination = [0xa5; sector_bytes];
        assert_eq!(
            device.read_sector(0, &mut destination),
            Err(storage_error::device_error)
        );
        assert_eq!(device.ports.transferred, 256);
        assert_eq!(destination, [0xa5; sector_bytes]);
        assert_eq!(device.counters.sectors_read, 0);
        assert_eq!(device.counters.commands_completed, 1);
        assert!(device.poisoned);
        assert_port_accounting(&device);
    }

    #[test]
    fn short_write_and_flush_failure_leave_whole_device_poisoned() {
        let mut device = ready();
        device.ports.short_after = Some(17);
        assert_eq!(
            device.write_sector(0, &[0x33; sector_bytes]),
            Err(storage_error::unexpected_transfer)
        );
        assert_eq!(device.ports.words_written.len(), 17);
        assert_eq!(device.counters.sectors_written, 0);
        assert!(device.poisoned);
        let mut device = ready();
        device.ports.command = 0;
        device.ports.completion_error = true;
        assert_eq!(device.flush(), Err(storage_error::device_error));
        assert_eq!(device.counters.flushes, 0);
        assert!(device.poisoned);
        assert_port_accounting(&device);
    }

    #[test]
    fn post_issue_timeout_uses_one_finite_budget_and_denies_all_further_io() {
        let mut device = ready();
        device.ports.busy_command = Some(write_command);
        let before = device.counters;
        assert_eq!(
            device.write_sector(0, &[0; sector_bytes]),
            Err(storage_error::poll_exhausted)
        );
        assert_eq!(
            device.counters.port_reads - before.port_reads,
            u64::from(poll_budget)
        );
        assert_eq!(device.counters.commands_issued - before.commands_issued, 1);
        assert_eq!(
            device.counters.commands_completed,
            before.commands_completed
        );
        assert_eq!(device.ports.words_written.len(), 0);
        let after = device.counters;
        assert_eq!(device.flush(), Err(storage_error::poisoned));
        assert_eq!(device.counters, after);
        assert_port_accounting(&device);
    }

    #[test]
    fn private_sector_clear_covers_every_byte() {
        let mut bytes = [0xa5; sector_bytes];
        clear_sector(&mut bytes);
        assert_eq!(bytes, [0; sector_bytes]);
    }

    #[cfg(feature = "native_durable_failure_test")]
    #[test]
    fn observation_loss_arm_and_grant_preflight_do_not_touch_ports() {
        let mut device = transport::new(mock_ports::new(), 8).unwrap();
        assert!(!device.grant_available());
        assert_eq!(
            device.arm_flush_observation_loss(),
            Err(storage_error::not_identified)
        );
        assert_eq!(device.counters, io_counters::default());
        device.identify().unwrap();
        let before = device.counters;
        device.ports.if_clear = false;
        assert!(!device.grant_available());
        assert_eq!(
            device.arm_flush_observation_loss(),
            Err(storage_error::interrupts_enabled)
        );
        device.ports.if_clear = true;
        assert!(device.grant_available());
        assert_eq!(device.arm_flush_observation_loss(), Ok(()));
        assert!(!device.grant_available());
        assert_eq!(
            device.arm_flush_observation_loss(),
            Err(storage_error::observation_loss_armed)
        );
        assert_eq!(device.flush_observations_remaining, 2);
        assert_eq!(device.counters, before);
        assert_port_accounting(&device);
    }

    #[cfg(feature = "native_durable_failure_test")]
    #[test]
    fn only_second_flush_is_issued_without_any_completion_observation() {
        let mut device = ready();
        // Earlier format/mount flushes are outside the target save's arm.
        device.flush().unwrap();
        let before = device.counters;
        device.arm_flush_observation_loss().unwrap();
        device.write_sector(1, &[0x31; sector_bytes]).unwrap();
        device.flush().unwrap();
        assert_eq!(device.flush_observations_remaining, 1);
        assert!(!device.poisoned);
        assert_eq!(device.counters.flushes, before.flushes + 1);
        assert_eq!(
            device.counters.commands_completed,
            before.commands_completed + 2
        );
        let after_body = device.counters;
        assert_eq!(
            device.arm_flush_observation_loss(),
            Err(storage_error::observation_loss_armed)
        );
        assert!(!device.grant_available());
        assert_eq!(device.counters, after_body);
        // Ordinary reads/writes do not advance the flush-only countdown.
        let mut readback = [0; sector_bytes];
        device.read_sector(0, &mut readback).unwrap();
        device.write_sector(2, &[0x73; sector_bytes]).unwrap();
        assert_eq!(device.flush_observations_remaining, 1);
        let before_final = device.counters;
        assert_eq!(device.flush(), Err(storage_error::poll_exhausted));
        assert_eq!(
            device.ports.byte_writes.last(),
            Some(&(command_port, flush_command))
        );
        assert_eq!(device.ports.reads, device.ports.reads_at_last_issue);
        assert_eq!(device.ports.writes, device.ports.writes_at_last_issue);
        assert_eq!(
            device.counters.commands_issued,
            before_final.commands_issued + 1
        );
        assert_eq!(
            device.counters.commands_completed,
            before_final.commands_completed
        );
        assert_eq!(device.counters.flushes, before_final.flushes);
        assert_eq!(device.counters.sectors_written, before.sectors_written + 2);
        assert_eq!(device.flush_observations_remaining, 0);
        assert!(device.poisoned);
        assert!(!device.grant_available());
        assert_port_accounting(&device);
    }

    #[cfg(feature = "native_durable_failure_test")]
    #[test]
    fn observation_loss_poison_survives_service_borrow_end_and_denies_regrant() {
        let mut device = ready();
        {
            let service = &mut device;
            service.arm_flush_observation_loss().unwrap();
            service.flush().unwrap();
            assert_eq!(service.flush(), Err(storage_error::poll_exhausted));
        }
        // Ending a service borrow is not a controller fence. This checks the
        // retained transport, while the target proof checks actual pair teardown.
        let poisoned = device.counters;
        let mut destination = [0xa5; sector_bytes];
        assert!(!device.grant_available());
        assert_eq!(
            device.read_sector(0, &mut destination),
            Err(storage_error::poisoned)
        );
        assert_eq!(
            device.write_sector(0, &[0x73; sector_bytes]),
            Err(storage_error::poisoned)
        );
        assert_eq!(device.flush(), Err(storage_error::poisoned));
        assert_eq!(device.identify(), Err(storage_error::poisoned));
        assert_eq!(
            device.arm_flush_observation_loss(),
            Err(storage_error::poisoned)
        );
        assert_eq!(destination, [0xa5; sector_bytes]);
        assert_eq!(device.counters, poisoned);
        assert_port_accounting(&device);
    }

    #[cfg(feature = "native_durable_failure_test")]
    #[test]
    fn earlier_real_failure_cannot_be_cleared_by_injection_control() {
        let mut device = ready();
        device.arm_flush_observation_loss().unwrap();
        device.ports.command = 0;
        device.ports.completion_error = true;
        assert_eq!(device.flush(), Err(storage_error::device_error));
        assert_eq!(device.flush_observations_remaining, 1);
        let before = device.counters;
        assert!(!device.grant_available());
        assert_eq!(
            device.arm_flush_observation_loss(),
            Err(storage_error::poisoned)
        );
        assert_eq!(device.flush(), Err(storage_error::poisoned));
        assert_eq!(device.counters, before);
        assert_port_accounting(&device);
    }
}
