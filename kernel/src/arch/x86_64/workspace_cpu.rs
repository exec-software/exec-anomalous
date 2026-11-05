//! Feature-private observation of one eager workspace CPU owner. No wire grant
//! or user-supplied restore image is accepted. Pattern checks belong only to
//! the inspected probe vessel; ordinary legacy workspace selection is unchanged.
#![allow(dead_code)]

use super::application_cpu::{cpu_session_group, fp_image};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

pub(super) const probe_magic: u64 = 0x6370_755f_7072_6f62;
pub(super) const tick_offset: u64 = 0x2200;

#[derive(Clone, Copy)]
pub(super) enum capture_kind {
    gate,
    timer,
    input,
    other_irq,
    fault,
}

#[derive(Clone, Copy, Default)]
pub(super) struct evidence {
    pub(super) gates: u64,
    pub(super) timers: u64,
    pub(super) inputs: u64,
    pub(super) other_irqs: u64,
    pub(super) faults: u64,
    pub(super) probes: u64,
    pub(super) clean_checks: u64,
    pub(super) user_checks: u64,
    pub(super) x87_timers: u64,
    pub(super) mmx_timers: u64,
}

pub(super) struct session {
    cpu: cpu_session_group<1>,
    observed: fp_image,
    phase: u64,
    evidence: evidence,
    finished: bool,
}

struct resident(UnsafeCell<session>);
// SAFETY: Sole BSP; acquisition is serialized under IF-clear, NMI-masked
// execution. No handler accesses this storage except through its runtime's
// unique borrow. The latch stays held through complete withdrawal and wiping.
unsafe impl Sync for resident {}
static claimed: AtomicBool = AtomicBool::new(false);
static storage: resident = resident(UnsafeCell::new(session {
    cpu: cpu_session_group::new(),
    observed: fp_image::workspace_observation(),
    phase: 0,
    evidence: evidence {
        gates: 0,
        timers: 0,
        inputs: 0,
        other_irqs: 0,
        faults: 0,
        probes: 0,
        clean_checks: 0,
        user_checks: 0,
        x87_timers: 0,
        mmx_timers: 0,
    },
    finished: true,
}));

fn increment(value: &mut u64) -> bool {
    let Some(next) = value.checked_add(1) else {
        return false;
    };
    *value = next;
    true
}

impl session {
    /// # Safety
    /// Sole BSP, IF clear/NMI masked. Keep the returned borrow in the final
    /// runtime; no session may begin until that runtime fully withdraws.
    pub(super) unsafe fn acquire() -> Result<&'static mut Self, &'static str> {
        if claimed.swap(true, Ordering::AcqRel) {
            return Err("workspace_cpu_claimed");
        }
        // SAFETY: The latch grants the only live borrow, never moved afterward.
        let value = unsafe { &mut *storage.0.get() };
        if !value.finished || !value.cpu.storage_is_cleared() {
            return Err("workspace_cpu_uncleared");
        }
        value.phase = 0;
        value.evidence = evidence::default();
        value.finished = false;
        unsafe { value.cpu.prepare()? };
        if !value
            .cpu
            .task_image(0)
            .is_some_and(fp_image::is_initial_clean)
            || !unsafe { value.cpu.workspace_clean_observed(&mut value.observed) }
        {
            return Err("workspace_cpu_initial_clean");
        }
        Ok(value)
    }

    pub(super) fn vendor(&self) -> &'static str {
        self.cpu.cpu_vendor()
    }
    pub(super) fn evidence(&self) -> evidence {
        self.evidence
    }
    pub(super) fn complete(&self) -> bool {
        self.phase == 4
    }

    /// # Safety
    /// Exact current owner, IF clear; every user-origin entry captures once.
    pub(super) unsafe fn capture(&mut self, kind: capture_kind) -> bool {
        if !unsafe { self.cpu.save_current(0) }
            || !unsafe { self.cpu.workspace_clean_observed(&mut self.observed) }
        {
            return false;
        }
        let count = match kind {
            capture_kind::gate => &mut self.evidence.gates,
            capture_kind::timer => &mut self.evidence.timers,
            capture_kind::input => &mut self.evidence.inputs,
            capture_kind::other_irq => &mut self.evidence.other_irqs,
            capture_kind::fault => &mut self.evidence.faults,
        };
        increment(count) && increment(&mut self.evidence.clean_checks)
    }

    fn check(&mut self, mmx: bool) -> bool {
        self.cpu
            .task_image(0)
            .is_some_and(|image| image_matches(image.bytes(), mmx))
            && increment(&mut self.evidence.user_checks)
    }

    /// Initial and explicit transition phases may be interrupted during seeding.
    /// Their full pattern is checked only at the next sealed probe return.
    pub(super) fn check_stable(&mut self) -> bool {
        match self.phase {
            0 | 2 => true,
            1 => self.check(false),
            3 | 4 => self.check(true),
            _ => false,
        }
    }

    pub(super) fn probe(&mut self, phase: u64) -> bool {
        if phase != self.phase + 1 || phase > 4 {
            return false;
        }
        let valid = match phase {
            1 => self.check(false),
            2 => self.evidence.x87_timers >= 2 && self.check(false),
            3 => self.check(true),
            4 => self.evidence.mmx_timers >= 2 && self.check(true),
            _ => false,
        };
        if !valid || !increment(&mut self.evidence.probes) {
            return false;
        }
        self.phase = phase;
        true
    }

    pub(super) fn timer_tick(&mut self) -> Result<Option<u64>, &'static str> {
        let count = match self.phase {
            1 => &mut self.evidence.x87_timers,
            3 => &mut self.evidence.mmx_timers,
            _ => return Ok(None),
        };
        if !increment(count) {
            return Err("workspace_cpu_tick_exhausted");
        }
        Ok(Some(*count))
    }

    /// # Safety
    /// Last operation before integer-only assembly returns to this user.
    pub(super) unsafe fn restore(&mut self) -> bool {
        unsafe { self.cpu.restore_task(0) }
    }

    /// # Safety
    /// Last user captured, root/TSS restored, no user can resume, all storage
    /// remains resident through exact original state restoration and wiping.
    pub(super) unsafe fn finish(&mut self) -> bool {
        if !unsafe { self.cpu.finish() } || !self.observed.clear_workspace_observation() {
            return false;
        }
        self.finished = true;
        true
    }

    /// # Safety
    /// Caller has completed all runtime cleanup; the borrow is not used after
    /// this call. No new acquisition occurs until the runtime frame returns.
    pub(super) unsafe fn release(&mut self) -> bool {
        if !self.finished || !self.cpu.storage_is_cleared() {
            return false;
        }
        claimed.store(false, Ordering::Release);
        true
    }
}

fn word(bytes: &[u8; 512], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

/// Defined architectural bytes only; FIP/FDP vary with the exact FLD location,
/// padding/MXCSR_MASK/software bytes are not compared as register contents.
fn image_matches(bytes: &[u8; 512], mmx: bool) -> bool {
    let control = u16::from_le_bytes([bytes[0], bytes[1]]);
    let status = u16::from_le_bytes([bytes[2], bytes[3]]);
    let mxcsr = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    if control != if mmx { 0x0b7f } else { 0x077f }
        || status != if mmx { 0 } else { 0x3800 }
        || bytes[4] != if mmx { 0xff } else { 0x80 }
        || mxcsr != if mmx { 0x5f80 } else { 0x3f80 }
    {
        return false;
    }
    if mmx {
        for index in 0..8 {
            if word(bytes, 32 + index * 16) != 0x6d6d_7800_0000_0100 + index as u64
                || bytes[40 + index * 16..42 + index * 16] != [0xff, 0xff]
            {
                return false;
            }
        }
    } else if word(bytes, 32) != 0x8000_0000_0000_0000 || bytes[40..42] != [0xff, 0x3f] {
        return false;
    }
    (0..16).all(|index| {
        word(bytes, 160 + index * 16) == 0x31a7_c0de_0000_0000 + index as u64
            && word(bytes, 168 + index * 16) == 0x91ee_c0de_0000_0000 + index as u64
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(mmx: bool) -> [u8; 512] {
        let mut bytes = [0; 512];
        bytes[0..2].copy_from_slice(&(if mmx { 0x0b7f_u16 } else { 0x077f_u16 }).to_le_bytes());
        bytes[2..4].copy_from_slice(&(if mmx { 0_u16 } else { 0x3800_u16 }).to_le_bytes());
        bytes[4] = if mmx { 0xff } else { 0x80 };
        bytes[24..28].copy_from_slice(&(if mmx { 0x5f80_u32 } else { 0x3f80_u32 }).to_le_bytes());
        if mmx {
            for i in 0..8 {
                bytes[32 + i * 16..40 + i * 16]
                    .copy_from_slice(&(0x6d6d_7800_0000_0100_u64 + i as u64).to_le_bytes());
                bytes[40 + i * 16..42 + i * 16].fill(0xff);
            }
        } else {
            bytes[32..40].copy_from_slice(&0x8000_0000_0000_0000_u64.to_le_bytes());
            bytes[40..42].copy_from_slice(&0x3fff_u16.to_le_bytes());
        }
        for i in 0..16 {
            bytes[160 + i * 16..168 + i * 16]
                .copy_from_slice(&(0x31a7_c0de_0000_0000_u64 + i as u64).to_le_bytes());
            bytes[168 + i * 16..176 + i * 16]
                .copy_from_slice(&(0x91ee_c0de_0000_0000_u64 + i as u64).to_le_bytes());
        }
        bytes
    }
    #[test]
    fn cpu_probe_requires_both_architectural_patterns_and_every_xmm_lane() {
        for mmx in [false, true] {
            let bytes = fixture(mmx);
            assert!(image_matches(&bytes, mmx));
            assert!(!image_matches(&bytes, !mmx));
            for offset in [0, 2, 4, 24, 32, 40].into_iter().chain(160..416) {
                let mut bad = bytes;
                bad[offset] ^= 1;
                assert!(!image_matches(&bad, mmx), "offset {offset}");
            }
            if mmx {
                for i in 0..8 {
                    let mut bad = bytes;
                    bad[32 + i * 16] ^= 1;
                    assert!(!image_matches(&bad, true));
                }
            }
        }
    }
    #[test]
    fn separate_observation_is_aligned_and_clear_without_privileged_instructions() {
        let mut image = fp_image::workspace_observation();
        assert_eq!((&image as *const fp_image as usize) % 16, 0);
        assert!(image.clear_workspace_observation());
        assert_eq!(core::mem::size_of::<fp_image>(), 512);
    }
}
