#![cfg_attr(not(test), allow(dead_code))]

//! Native FS-base ownership. Linux TLS layout and syscall policy are separate.
//! The sole BSP excludes IRQ/NMI while manipulating this non-copyable owner.
//! Assembly captures before selector loads, clears all bases before Rust, and
//! restores a staged base only after the final user-selector loads.

use crate::identity::task_identity;
#[cfg(any(test, feature = "linux_tls_test"))]
use crate::{
    scheduler::{scheduler_call_authorization, scheduler_table},
    task::task_table,
};
use core::{
    arch::asm,
    ptr,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

const fs_msr: u32 = 0xc000_0100;
const gs_msr: u32 = 0xc000_0101;
const kernel_gs_msr: u32 = 0xc000_0102;
const fsgsbase: u64 = 1 << 16;
const kernel_selector: u16 = 0x10;
const user_limit: u64 = 0x0000_8000_0000_0000;
const inactive: u64 = 0;
const clean_kernel: u64 = 1;
const staged: u64 = 2;
const user_active: u64 = 3;
const captured: u64 = 4;
const failed: u64 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum tls_error {
    state,
    identity,
    controls,
    frame,
    observation,
    base,
    restore,
    authorization,
    exhausted,
}
impl tls_error {
    pub(super) const fn reason(self) -> &'static str {
        match self {
            Self::state => "task_tls_state",
            Self::identity => "task_tls_identity",
            Self::controls => "task_tls_controls",
            Self::frame => "task_tls_frame",
            Self::observation => "task_tls_observation",
            Self::base => "task_tls_base",
            Self::restore => "task_tls_restore",
            Self::authorization => "task_tls_authorization",
            Self::exhausted => "task_tls_exhausted",
        }
    }
}

// All words are inert observations or a single scoped assembly handoff. No
// caller can construct task authority from a copied capsule. Offsets are passed
// to assembly below rather than duplicated as unexplained literals.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct capsule {
    phase: u64,
    frame: u64,
    slot: u64,
    return_fs: u64,
    fs_selector: u64,
    fs: u64,
    gs_selector: u64,
    gs: u64,
    kernel_gs: u64,
}
impl capsule {
    const fn zero() -> Self {
        Self {
            phase: inactive,
            frame: 0,
            slot: 0,
            return_fs: 0,
            fs_selector: 0,
            fs: 0,
            gs_selector: 0,
            gs: 0,
            kernel_gs: 0,
        }
    }
    const fn clean() -> Self {
        Self {
            phase: clean_kernel,
            ..Self::zero()
        }
    }
}
const _: () = assert!(core::mem::size_of::<capsule>() == 72);

static owner_taken: AtomicBool = AtomicBool::new(false);
static next_owner: AtomicU64 = AtomicU64::new(1);
static active_owner: AtomicU64 = AtomicU64::new(0);
// This word is zero while inactive, and exactly the admitted slot count while
// active. Assembly cannot accept a third slot from a two-slot session.
#[unsafe(no_mangle)]
static mut x86_64_task_tls_active_slots: u64 = 0;
// SAFETY: Sole BSP with IRQ/NMI excluded. The session acquires the singleton
// before publishing phase1; only the inspected assembly and that session write
// it. The entry/return capsule is never a userspace mapping or escaped slice.
#[unsafe(no_mangle)]
static mut x86_64_task_tls_capsule: capsule = capsule::zero();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct controls {
    cr4: u64,
    flags: u64,
    fs_selector: u16,
    gs_selector: u16,
    fs: u64,
    gs: u64,
    kernel_gs: u64,
}
impl controls {
    fn clean(self) -> bool {
        self.cr4 & fsgsbase == 0
            && self.flags & (1 << 9) == 0
            && self.fs_selector == kernel_selector
            && self.gs_selector == kernel_selector
            && self.fs == 0
            && self.gs == 0
            && self.kernel_gs == 0
    }
}

// Padding-free words make retirement and final storage wiping literal, rather
// than only assigning an Option discriminant. Identity components remain
// private comparison metadata; they never construct or grant task authority.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct task_state {
    task_slot: u64,
    task_generation: u64,
    circle_slot: u64,
    circle_generation: u64,
    fs: u64,
}
impl task_state {
    const fn empty() -> Self {
        Self {
            task_slot: 0,
            task_generation: 0,
            circle_slot: 0,
            circle_generation: 0,
            fs: 0,
        }
    }
    fn bound(identity: task_identity) -> Self {
        Self {
            task_slot: identity.task().slot() as u64,
            task_generation: identity.task().generation_value(),
            circle_slot: identity.circle().slot() as u64,
            circle_generation: identity.circle().generation_value(),
            fs: 0,
        }
    }
    #[cfg(any(test, feature = "linux_tls_test"))]
    fn identity_words(&self) -> [u64; 4] {
        [
            self.task_slot,
            self.task_generation,
            self.circle_slot,
            self.circle_generation,
        ]
    }
    fn matches(&self, identity: task_identity) -> bool {
        self.task_slot == identity.task().slot() as u64
            && self.task_generation == identity.task().generation_value()
            && self.circle_slot == identity.circle().slot() as u64
            && self.circle_generation == identity.circle().generation_value()
    }
}
const _: () = assert!(core::mem::size_of::<task_state>() == 5 * 8);

/// One inert proposal for an exact sealed suspended call. This retained slot
/// is move-only and padding-free; discard and successful commit wipe it in
/// place, rather than leaving an Option's inactive payload behind.
#[cfg(any(test, feature = "linux_tls_test"))]
#[repr(C)]
#[must_use = "retain the pending FS proposal until commit or explicit discard"]
pub(super) struct suspended_fs_update {
    owner: u64,
    caller: [u64; 4],
    veil: [u64; 4],
    correlation: [u64; 4],
    previous: u64,
    proposed: u64,
}
#[cfg(any(test, feature = "linux_tls_test"))]
impl suspended_fs_update {
    pub(super) const fn new() -> Self {
        Self {
            owner: 0,
            caller: [0; 4],
            veil: [0; 4],
            correlation: [0; 4],
            previous: 0,
            proposed: 0,
        }
    }
    pub(super) fn is_pending(&self) -> bool {
        self.owner != 0
    }
    pub(super) fn storage_is_cleared(&self) -> bool {
        // SAFETY: repr(C) fifteen contiguous u64 words; no padding or pointers.
        let words = ptr::from_ref(self).cast::<u64>();
        (0..15).all(|index| unsafe { ptr::read_volatile(words.add(index)) } == 0)
    }
    pub(super) fn discard(&mut self) {
        // SAFETY: Exclusively borrowed exact private proposal storage. It owns
        // no scheduler authority and has never changed a caller's FS state.
        unsafe { ptr::write_volatile(self, Self::new()) };
    }
}
#[cfg(any(test, feature = "linux_tls_test"))]
const _: () = assert!(core::mem::size_of::<suspended_fs_update>() == 15 * 8);

// Pure state transitions are exercised without executing privileged operations.
#[repr(C)]
#[derive(Debug, Eq, PartialEq)]
struct state_group<const task_count: usize> {
    tasks: [task_state; task_count],
    // Zero means none/inactive; live indices are stored as one through N.
    current: u64,
    active: u64,
}
impl<const task_count: usize> state_group<task_count> {
    const fn new() -> Self {
        Self {
            tasks: [task_state::empty(); task_count],
            current: 0,
            active: 0,
        }
    }
    fn prepare(
        &mut self,
        identities: [task_identity; task_count],
        hardware: controls,
    ) -> Result<(), tls_error> {
        if !(task_count == 2 || (cfg!(any(test, feature = "linux_tls_test")) && task_count == 3))
            || self.active != 0
            || self.tasks != [task_state::empty(); task_count]
            || self.current != 0
        {
            return Err(tls_error::state);
        }
        if identities
            .iter()
            .enumerate()
            .any(|(index, identity)| identities[..index].contains(identity))
        {
            return Err(tls_error::identity);
        }
        if !hardware.clean() {
            return Err(tls_error::controls);
        }
        self.tasks = identities.map(task_state::bound);
        self.active = 1;
        Ok(())
    }
    fn index(&self, identity: task_identity) -> Result<usize, tls_error> {
        if self.active == 0 {
            return Err(tls_error::state);
        }
        self.tasks
            .iter()
            .position(|task| task.matches(identity))
            .ok_or(tls_error::identity)
    }
    fn check(&self, entry: &capsule, hardware: controls, phase: u64) -> Result<(), tls_error> {
        if self.active == 0 || entry.phase != phase {
            return Err(tls_error::state);
        }
        if !hardware.clean() {
            return Err(tls_error::controls);
        }
        Ok(())
    }
    fn consume(
        &mut self,
        entry: &mut capsule,
        hardware: controls,
        identity: task_identity,
        frame: u64,
    ) -> Result<(), tls_error> {
        self.check(entry, hardware, captured)?;
        let index = self.index(identity)?;
        if !valid_frame(frame) || entry.frame != frame || entry.slot != index as u64 + 1 {
            return Err(tls_error::frame);
        }
        if entry.fs_selector != 0
            || entry.gs_selector != 0
            || entry.gs != 0
            || entry.kernel_gs != 0
            || entry.fs != self.tasks[index].fs
            || entry.return_fs != self.tasks[index].fs
        {
            return Err(tls_error::observation);
        }
        *entry = capsule::clean();
        self.current = index as u64 + 1;
        Ok(())
    }
    fn stage(
        &mut self,
        entry: &mut capsule,
        hardware: controls,
        identity: task_identity,
        frame: u64,
    ) -> Result<(), tls_error> {
        self.check(entry, hardware, clean_kernel)?;
        let index = self.index(identity)?;
        if !valid_frame(frame) {
            return Err(tls_error::frame);
        }
        if *entry != capsule::clean() {
            return Err(tls_error::state);
        }
        *entry = capsule {
            phase: staged,
            frame,
            slot: index as u64 + 1,
            return_fs: self.tasks[index].fs,
            ..capsule::zero()
        };
        self.current = 0;
        Ok(())
    }
    fn current_index(
        &self,
        entry: &capsule,
        hardware: controls,
        identity: task_identity,
    ) -> Result<usize, tls_error> {
        self.check(entry, hardware, clean_kernel)?;
        let index = self.index(identity)?;
        if self.current != index as u64 + 1 || *entry != capsule::clean() {
            return Err(tls_error::identity);
        }
        Ok(index)
    }
    fn set(
        &mut self,
        entry: &capsule,
        hardware: controls,
        identity: task_identity,
        base: u64,
    ) -> Result<(), tls_error> {
        let index = self.current_index(entry, hardware, identity)?;
        if base >= user_limit {
            return Err(tls_error::base);
        }
        self.tasks[index].fs = base;
        Ok(())
    }
    fn get(
        &self,
        entry: &capsule,
        hardware: controls,
        identity: task_identity,
    ) -> Result<u64, tls_error> {
        Ok(self.tasks[self.current_index(entry, hardware, identity)?].fs)
    }
    #[cfg(any(test, feature = "linux_tls_test"))]
    fn suspended_indices(
        &self,
        entry: &capsule,
        hardware: controls,
        tasks: &task_table,
        scheduler: &scheduler_table,
        authority: &scheduler_call_authorization,
    ) -> Result<(usize, usize), tls_error> {
        scheduler
            .validate_call_authorization(tasks, authority)
            .map_err(|_| tls_error::authorization)?;
        let veil = self.current_index(entry, hardware, authority.veil())?;
        let caller = self.index(authority.caller())?;
        if caller == veil {
            return Err(tls_error::authorization);
        }
        Ok((caller, veil))
    }
    #[cfg(any(test, feature = "linux_tls_test"))]
    fn query_suspended(
        &self,
        entry: &capsule,
        hardware: controls,
        tasks: &task_table,
        scheduler: &scheduler_table,
        authority: &scheduler_call_authorization,
    ) -> Result<u64, tls_error> {
        let (caller, _) = self.suspended_indices(entry, hardware, tasks, scheduler, authority)?;
        Ok(self.tasks[caller].fs)
    }
    #[cfg(any(test, feature = "linux_tls_test"))]
    #[allow(clippy::too_many_arguments)] // Explicit independent ownership observations.
    fn stage_suspended(
        &self,
        entry: &capsule,
        hardware: controls,
        tasks: &task_table,
        scheduler: &scheduler_table,
        authority: &scheduler_call_authorization,
        update: &mut suspended_fs_update,
        value: u64,
    ) -> Result<(), tls_error> {
        let (caller, veil) =
            self.suspended_indices(entry, hardware, tasks, scheduler, authority)?;
        if !update.storage_is_cleared() {
            return Err(tls_error::state);
        }
        if value >= user_limit {
            return Err(tls_error::base);
        }
        *update = suspended_fs_update {
            owner: self.active,
            caller: self.tasks[caller].identity_words(),
            veil: self.tasks[veil].identity_words(),
            correlation: authority.correlation_words(),
            previous: self.tasks[caller].fs,
            proposed: value,
        };
        Ok(())
    }
    #[cfg(any(test, feature = "linux_tls_test"))]
    #[allow(clippy::too_many_arguments)] // Explicit independent ownership observations.
    fn commit_suspended(
        &mut self,
        entry: &capsule,
        hardware: controls,
        tasks: &task_table,
        scheduler: &scheduler_table,
        authority: &scheduler_call_authorization,
        update: &mut suspended_fs_update,
    ) -> Result<(), tls_error> {
        let (caller, veil) =
            self.suspended_indices(entry, hardware, tasks, scheduler, authority)?;
        if update.owner != self.active
            || update.caller != self.tasks[caller].identity_words()
            || update.veil != self.tasks[veil].identity_words()
            || update.correlation != authority.correlation_words()
            || update.previous != self.tasks[caller].fs
            || update.proposed >= user_limit
        {
            return Err(tls_error::authorization);
        }
        // No fallible operation follows the effect. Hardware stays zero until
        // this exact caller is subsequently selected for the final return.
        self.tasks[caller].fs = update.proposed;
        update.discard();
        Ok(())
    }
    fn retire(
        &mut self,
        entry: &capsule,
        hardware: controls,
        identity: task_identity,
    ) -> Result<(), tls_error> {
        self.check(entry, hardware, clean_kernel)?;
        if *entry != capsule::clean() {
            return Err(tls_error::state);
        }
        let index = self.index(identity)?;
        // SAFETY: Exclusive access to this private padding-free five-word slot.
        // Volatile storage writes cannot leave an inactive identity payload.
        unsafe { ptr::write_volatile(&mut self.tasks[index], task_state::empty()) };
        if self.current == index as u64 + 1 {
            self.current = 0;
        }
        Ok(())
    }
    fn finish_ready(&self, entry: &capsule, hardware: controls) -> Result<(), tls_error> {
        self.check(entry, hardware, clean_kernel)?;
        if *entry != capsule::clean()
            || self.current != 0
            || self.tasks != [task_state::empty(); task_count]
        {
            return Err(tls_error::state);
        }
        Ok(())
    }
    fn is_cleared(&self) -> bool {
        // SAFETY: repr(C), a contiguous array of five-u64 records plus two
        // u64 words. Every component is eight-byte aligned with no padding.
        let words = ptr::from_ref(self).cast::<u64>();
        (0..task_count * 5 + 2).all(|index| unsafe { ptr::read_volatile(words.add(index)) } == 0)
    }
    fn clear(&mut self) {
        // SAFETY: This exact private, padding-free owner is exclusively borrowed.
        // The caller has completed the hardware readback before final release.
        unsafe { ptr::write_volatile(self, Self::new()) };
    }
}
const _: () = assert!(core::mem::size_of::<state_group<2>>() == 12 * 8);
const _: () = assert!(core::mem::size_of::<state_group<3>>() == 17 * 8);
#[cfg(test)]
type state = state_group<2>;

fn valid_frame(frame: u64) -> bool {
    frame >= 0xffff_8000_0000_0000 && frame.is_multiple_of(8) && frame.checked_add(22 * 8).is_some()
}

/// Caller retains this owner through all errors, assembly transitions and the
/// bootstrap continuation. Hardware ownership is never inferred from an index.
#[must_use = "retain TLS ownership until explicit finish after bootstrap resume"]
#[repr(transparent)]
pub(super) struct session_group<const task_count: usize> {
    state: state_group<task_count>,
}
pub(super) type session = session_group<2>;
impl<const task_count: usize> session_group<task_count> {
    pub(super) const fn new() -> Self {
        Self {
            state: state_group::new(),
        }
    }
    /// # Safety
    /// Exact admitted committed identities, sole BSP, IF clear/NMI masked. All task
    /// storage and the supervisor capsule remain resident until explicit finish.
    pub(super) unsafe fn prepare(
        &mut self,
        identities: [task_identity; task_count],
    ) -> Result<(), tls_error> {
        let hardware = observe_controls();
        if read_capsule() != capsule::zero()
            || read_active_slots() != 0
            || active_owner.load(Ordering::Acquire) != 0
        {
            return Err(tls_error::state);
        }
        // Validate without publishing ownership or changing this session.
        let mut candidate = state_group::new();
        candidate.prepare(identities, hardware)?;
        if self.state != state_group::new() {
            return Err(tls_error::state);
        }
        owner_taken
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| tls_error::state)?;
        let nonce = match issue_owner(&next_owner) {
            Ok(nonce) => nonce,
            Err(error) => {
                owner_taken.store(false, Ordering::Release);
                return Err(error);
            }
        };
        candidate.active = nonce;
        self.state = candidate;
        active_owner.store(nonce, Ordering::Release);
        write_active_slots(task_count as u64);
        write_capsule(capsule::clean());
        Ok(())
    }
    /// # Safety
    /// Caller has authenticated the entire exact frame and current task/root/
    /// TSS/stack before this call. No other dispatch or alias mutation can occur.
    pub(super) unsafe fn consume_entry<F>(
        &mut self,
        identity: task_identity,
        frame: *const F,
    ) -> Result<(), tls_error> {
        self.require_owner()?;
        let mut entry = read_capsule();
        self.state
            .consume(&mut entry, observe_controls(), identity, frame as u64)?;
        write_capsule(entry);
        Ok(())
    }
    /// # Safety
    /// Exact live selected owner/root/TSS and complete return frame were checked.
    /// No task/CPU change may follow except the inspected final return helper.
    pub(super) unsafe fn stage_return<F>(
        &mut self,
        identity: task_identity,
        frame: *const F,
    ) -> Result<(), tls_error> {
        self.require_owner()?;
        let mut entry = read_capsule();
        self.state
            .stage(&mut entry, observe_controls(), identity, frame as u64)?;
        write_capsule(entry);
        Ok(())
    }
    pub(super) fn set_fs(&mut self, identity: task_identity, value: u64) -> Result<(), tls_error> {
        self.require_owner()?;
        self.state
            .set(&read_capsule(), observe_controls(), identity, value)
    }
    pub(super) fn get_fs(&self, identity: task_identity) -> Result<u64, tls_error> {
        self.require_owner()?;
        self.state
            .get(&read_capsule(), observe_controls(), identity)
    }
    /// # Safety
    /// The execution owner has just validated this exact sealed suspended
    /// caller and the current veil's root/TSS/frame/stack under IF/NMI exclusion.
    /// The owner, scheduler and task records remain stable throughout the call.
    #[cfg(any(test, feature = "linux_tls_test"))]
    pub(super) unsafe fn query_suspended_fs(
        &self,
        tasks: &task_table,
        scheduler: &scheduler_table,
        authority: &scheduler_call_authorization,
    ) -> Result<u64, tls_error> {
        self.require_owner()?;
        self.state.query_suspended(
            &read_capsule(),
            observe_controls(),
            tasks,
            scheduler,
            authority,
        )
    }
    /// Prepare an inert FS effect for the sealed caller, never the running veil.
    /// Rejections leave the owner and retained update slot unchanged.
    ///
    /// # Safety
    /// Same exact execution-owner validation and exclusion as query_suspended_fs.
    #[cfg(any(test, feature = "linux_tls_test"))]
    pub(super) unsafe fn stage_suspended_fs(
        &self,
        tasks: &task_table,
        scheduler: &scheduler_table,
        authority: &scheduler_call_authorization,
        update: &mut suspended_fs_update,
        value: u64,
    ) -> Result<(), tls_error> {
        self.require_owner()?;
        self.state.stage_suspended(
            &read_capsule(),
            observe_controls(),
            tasks,
            scheduler,
            authority,
            update,
            value,
        )
    }
    /// # Safety
    /// Repeat the execution-owner validation for this exact blocked caller.
    /// These three immutable inputs come from the held prepared scheduler reply.
    /// All other fallible effect/frame checks precede this call; immediately
    /// commit that reply after success, without a fallible or scheduling step.
    /// Rejections retain both owner and proposal for a valid retry or discard.
    #[cfg(any(test, feature = "linux_tls_test"))]
    pub(super) unsafe fn commit_suspended_fs(
        &mut self,
        tasks: &task_table,
        scheduler: &scheduler_table,
        authority: &scheduler_call_authorization,
        update: &mut suspended_fs_update,
    ) -> Result<(), tls_error> {
        self.require_owner()?;
        self.state.commit_suspended(
            &read_capsule(),
            observe_controls(),
            tasks,
            scheduler,
            authority,
            update,
        )
    }
    /// Retire only after the runtime has made this exact task unresumable.
    /// Kernel-only API; an identity value does not grant callers retirement.
    pub(super) fn retire(&mut self, identity: task_identity) -> Result<(), tls_error> {
        self.require_owner()?;
        self.state
            .retire(&read_capsule(), observe_controls(), identity)
    }
    /// # Safety
    /// All admitted tasks are retired, bootstrap CR3/RSP/TSS restored, IF clear/NMI
    /// masked. Retain owner on any readback error; no task may ever resume.
    pub(super) unsafe fn finish(&mut self) -> Result<(), tls_error> {
        self.require_owner()?;
        self.state
            .finish_ready(&read_capsule(), observe_controls())?;
        // The admitted originals are exactly selectors10 and all-zero bases.
        // Clear the capsule before restoring those selectors/MSRs. Any failure
        // retains the session latch and prevents reuse, including after zeroing.
        write_capsule(capsule::zero());
        unsafe {
            asm!("mov fs, ax", "mov gs, ax", in("ax") kernel_selector, options(nostack, preserves_flags));
            write_msr(fs_msr, 0);
            write_msr(gs_msr, 0);
            write_msr(kernel_gs_msr, 0);
        }
        if read_capsule() != capsule::zero() || !observe_controls().clean() {
            return Err(tls_error::restore);
        }
        // Explicit volatile overwrite removes per-task identities and bases.
        self.state.clear();
        write_active_slots(0);
        active_owner.store(0, Ordering::Release);
        if !self.state.is_cleared()
            || read_active_slots() != 0
            || active_owner.load(Ordering::Acquire) != 0
        {
            return Err(tls_error::restore);
        }
        owner_taken.store(false, Ordering::Release);
        Ok(())
    }
    pub(super) fn kernel_is_clean(&self) -> bool {
        self.require_owner().is_ok()
            && observe_controls().clean()
            && read_capsule() == capsule::clean()
    }
    pub(super) fn storage_is_cleared(&self) -> bool {
        self.state.is_cleared()
            && !owner_taken.load(Ordering::Acquire)
            && active_owner.load(Ordering::Acquire) == 0
            && read_active_slots() == 0
            && read_capsule() == capsule::zero()
    }
    fn require_owner(&self) -> Result<(), tls_error> {
        if self.state.active != 0
            && self.state.active == active_owner.load(Ordering::Acquire)
            && owner_taken.load(Ordering::Acquire)
            && read_active_slots() == task_count as u64
        {
            Ok(())
        } else {
            Err(tls_error::state)
        }
    }
}

fn issue_owner(counter: &AtomicU64) -> Result<u64, tls_error> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            if next == 0 {
                None
            } else {
                Some(next.checked_add(1).unwrap_or(0))
            }
        })
        .map_err(|_| tls_error::exhausted)
}
fn read_active_slots() -> u64 {
    // SAFETY: Sole BSP under the same exclusion contract as the capsule.
    unsafe { ptr::read_volatile(ptr::addr_of!(x86_64_task_tls_active_slots)) }
}
fn write_active_slots(value: u64) {
    // SAFETY: Only the singleton owner publishes or clears this supervisor word.
    unsafe { ptr::write_volatile(ptr::addr_of_mut!(x86_64_task_tls_active_slots), value) };
}

fn read_capsule() -> capsule {
    // SAFETY: Kernel-private BSP caller under the API exclusion contract.
    unsafe { ptr::read_volatile(ptr::addr_of!(x86_64_task_tls_capsule)) }
}
fn write_capsule(value: capsule) {
    // SAFETY: Exact singleton owner, IRQ/NMI excluded and no concurrent entry.
    unsafe { ptr::write_volatile(ptr::addr_of_mut!(x86_64_task_tls_capsule), value) };
}
fn read_msr(register: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!("rdmsr", in("ecx") register, out("eax") low, out("edx") high, options(nostack, nomem, preserves_flags))
    };
    (u64::from(high) << 32) | u64::from(low)
}
unsafe fn write_msr(register: u32, value: u64) {
    unsafe {
        asm!("wrmsr", in("ecx") register, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nostack, preserves_flags))
    };
}
fn observe_controls() -> controls {
    let cr4: u64;
    let flags: u64;
    let fs_selector: u16;
    let gs_selector: u16;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nostack, nomem, preserves_flags));
        asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags));
        asm!("mov {0:x}, fs", out(reg) fs_selector, options(nostack, nomem, preserves_flags));
        asm!("mov {0:x}, gs", out(reg) gs_selector, options(nostack, nomem, preserves_flags));
    }
    controls {
        cr4,
        flags,
        fs_selector,
        gs_selector,
        fs: read_msr(fs_msr),
        gs: read_msr(gs_msr),
        kernel_gs: read_msr(kernel_gs_msr),
    }
}

#[cfg(not(test))]
core::arch::global_asm!(include_str!("task_tls_entry.S"),
    max_task_count = const if cfg!(feature = "linux_tls_test") { 3 } else { 2 },
    capsule_phase = const core::mem::offset_of!(capsule, phase),
    capsule_frame = const core::mem::offset_of!(capsule, frame),
    capsule_slot = const core::mem::offset_of!(capsule, slot),
    capsule_return_fs = const core::mem::offset_of!(capsule, return_fs),
    capsule_fs_selector = const core::mem::offset_of!(capsule, fs_selector),
    capsule_fs = const core::mem::offset_of!(capsule, fs),
    capsule_gs_selector = const core::mem::offset_of!(capsule, gs_selector),
    capsule_gs = const core::mem::offset_of!(capsule, gs),
    capsule_kernel_gs = const core::mem::offset_of!(capsule, kernel_gs),
    phase_inactive = const inactive, phase_clean = const clean_kernel,
    phase_staged = const staged, phase_user = const user_active,
    phase_captured = const captured, phase_failed = const failed,
    options(att_syntax));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{circle_id, task_id};
    const frame_a: u64 = 0xffff_9000_0040_0f50;
    const frame_b: u64 = 0xffff_9000_0050_0f50;
    fn identity(slot: usize, generation: u64) -> task_identity {
        task_identity::new(
            task_id::try_from_parts(slot, generation).unwrap(),
            circle_id::try_from_parts(slot, generation).unwrap(),
        )
    }
    fn clean() -> controls {
        controls {
            cr4: 0,
            flags: 2,
            fs_selector: 0x10,
            gs_selector: 0x10,
            fs: 0,
            gs: 0,
            kernel_gs: 0,
        }
    }
    fn fixture() -> (state, capsule, [task_identity; 2]) {
        let ids = [identity(0, 1), identity(1, 7)];
        let mut value = state::new();
        value.prepare(ids, clean()).unwrap();
        (value, capsule::clean(), ids)
    }
    fn snapshot(value: &state) -> ([task_state; 2], u64, u64) {
        (value.tasks, value.current, value.active)
    }
    // Model the public observations made by the assembly; no fake owner or
    // reader is exposed outside this private state-machine unit test.
    fn entered(entry: &mut capsule) {
        assert_eq!(entry.phase, staged);
        entry.phase = captured;
        entry.fs = entry.return_fs;
    }
    fn consume_initial<const count: usize>(
        value: &mut state_group<count>,
        entry: &mut capsule,
        identity: task_identity,
        frame: u64,
    ) {
        value.stage(entry, clean(), identity, frame).unwrap();
        entered(entry);
        value.consume(entry, clean(), identity, frame).unwrap();
    }

    #[test]
    fn prepare_admits_only_exact_clean_bootstrap_and_distinct_owners() {
        let ids = [identity(0, 1), identity(1, 1)];
        let invalid = [
            controls {
                cr4: fsgsbase,
                ..clean()
            },
            controls {
                flags: 0x202,
                ..clean()
            },
            controls {
                fs_selector: 0,
                ..clean()
            },
            controls {
                gs_selector: 0,
                ..clean()
            },
            controls { fs: 1, ..clean() },
            controls { gs: 1, ..clean() },
            controls {
                kernel_gs: 1,
                ..clean()
            },
        ];
        for hardware in invalid {
            let mut value = state::new();
            assert_eq!(value.prepare(ids, hardware), Err(tls_error::controls));
            assert_eq!(value, state::new());
        }
        let mut value = state::new();
        assert_eq!(
            value.prepare([ids[0]; 2], clean()),
            Err(tls_error::identity)
        );
        assert_eq!(value, state::new());
        value.prepare(ids, clean()).unwrap();
        let before = snapshot(&value);
        assert_eq!(value.prepare(ids, clean()), Err(tls_error::state));
        assert_eq!(snapshot(&value), before);
        assert_eq!(core::mem::size_of::<capsule>(), 72);
        assert!(core::mem::size_of::<session>() <= 128);
    }

    #[test]
    fn exact_capture_consumes_once_and_rejects_selector_base_frame_and_owner_changes() {
        let (mut value, mut entry, ids) = fixture();
        value.stage(&mut entry, clean(), ids[0], frame_a).unwrap();
        entered(&mut entry);
        let actual = entry;
        let before = snapshot(&value);
        for changed in [
            capsule {
                phase: user_active,
                ..actual
            },
            capsule {
                phase: failed,
                ..actual
            },
            capsule {
                frame: frame_b,
                ..actual
            },
            capsule { slot: 2, ..actual },
            capsule {
                fs_selector: 0x2b,
                ..actual
            },
            capsule {
                gs_selector: 0x2b,
                ..actual
            },
            capsule {
                fs: 0x7000,
                ..actual
            },
            capsule {
                return_fs: 0x7000,
                ..actual
            },
            capsule { gs: 1, ..actual },
            capsule {
                kernel_gs: 1,
                ..actual
            },
        ] {
            let mut rejected = changed;
            assert!(
                value
                    .consume(&mut rejected, clean(), ids[0], frame_a)
                    .is_err()
            );
            assert_eq!(rejected, changed);
            assert_eq!(snapshot(&value), before);
        }
        for wrong in [
            ids[1],
            identity(0, 2),
            task_identity::new(ids[0].task(), ids[1].circle()),
        ] {
            assert!(value.consume(&mut entry, clean(), wrong, frame_a).is_err());
            assert_eq!(entry, actual);
            assert_eq!(snapshot(&value), before);
        }
        value.consume(&mut entry, clean(), ids[0], frame_a).unwrap();
        assert_eq!(entry, capsule::clean());
        assert_eq!(
            value.consume(&mut entry, clean(), ids[0], frame_a),
            Err(tls_error::state)
        );
        assert_eq!(value.get(&entry, clean(), ids[0]), Ok(0));
    }

    #[test]
    fn setter_is_current_only_and_rejects_the_entire_upper_address_half_atomically() {
        let (mut value, mut entry, ids) = fixture();
        consume_initial(&mut value, &mut entry, ids[0], frame_a);
        value.set(&entry, clean(), ids[0], 0x6003_b003).unwrap();
        let before = snapshot(&value);
        for bad in [
            user_limit,
            0x0001_0000_0000_0000,
            0xffff_8000_0000_0000,
            u64::MAX,
        ] {
            assert_eq!(
                value.set(&entry, clean(), ids[0], bad),
                Err(tls_error::base)
            );
            assert_eq!(snapshot(&value), before);
        }
        assert_eq!(
            value.set(&entry, clean(), ids[1], 0),
            Err(tls_error::identity)
        );
        assert_eq!(value.get(&entry, clean(), ids[1]), Err(tls_error::identity));
        for valid in [user_limit - 1, 0, 0x4000_7101] {
            value.set(&entry, clean(), ids[0], valid).unwrap();
            assert_eq!(value.get(&entry, clean(), ids[0]), Ok(valid));
        }
    }

    #[test]
    fn every_active_transition_rejects_dirty_kernel_controls_without_consumption() {
        let (mut value, mut entry, ids) = fixture();
        consume_initial(&mut value, &mut entry, ids[0], frame_a);
        for hardware in [
            controls {
                cr4: fsgsbase,
                ..clean()
            },
            controls {
                flags: 0x202,
                ..clean()
            },
            controls { fs: 1, ..clean() },
            controls { gs: 1, ..clean() },
            controls {
                kernel_gs: 1,
                ..clean()
            },
            controls {
                fs_selector: 0,
                ..clean()
            },
            controls {
                gs_selector: 0,
                ..clean()
            },
        ] {
            let before = snapshot(&value);
            assert_eq!(
                value.set(&entry, hardware, ids[0], 9),
                Err(tls_error::controls)
            );
            assert_eq!(
                value.get(&entry, hardware, ids[0]),
                Err(tls_error::controls)
            );
            assert_eq!(
                value.stage(&mut entry, hardware, ids[1], frame_b),
                Err(tls_error::controls)
            );
            assert_eq!(
                value.retire(&entry, hardware, ids[0]),
                Err(tls_error::controls)
            );
            assert_eq!(
                value.finish_ready(&entry, hardware),
                Err(tls_error::controls)
            );
            assert_eq!(entry, capsule::clean());
            assert_eq!(snapshot(&value), before);
            let mut observation = capsule {
                phase: captured,
                frame: frame_a,
                slot: 1,
                ..capsule::zero()
            };
            let old = observation;
            assert_eq!(
                value.consume(&mut observation, hardware, ids[0], frame_a),
                Err(tls_error::controls)
            );
            assert_eq!(observation, old);
            assert_eq!(snapshot(&value), before);
        }
    }

    #[test]
    fn staged_return_cannot_be_replayed_replaced_retired_or_finished() {
        let (mut value, mut entry, ids) = fixture();
        for frame in [0, 0x4000_0000, frame_a + 1, u64::MAX - 7] {
            assert_eq!(
                value.stage(&mut entry, clean(), ids[0], frame),
                Err(tls_error::frame)
            );
            assert_eq!(entry, capsule::clean());
        }
        value.stage(&mut entry, clean(), ids[0], frame_a).unwrap();
        let before = snapshot(&value);
        let capsule_before = entry;
        assert_eq!(
            value.stage(&mut entry, clean(), ids[1], frame_b),
            Err(tls_error::state)
        );
        assert_eq!(value.retire(&entry, clean(), ids[0]), Err(tls_error::state));
        assert_eq!(value.set(&entry, clean(), ids[0], 0), Err(tls_error::state));
        assert_eq!(value.finish_ready(&entry, clean()), Err(tls_error::state));
        assert_eq!(entry, capsule_before);
        assert_eq!(snapshot(&value), before);
    }

    #[test]
    fn task_switch_fault_retirement_and_fresh_session_preserve_exact_fs_owners() {
        let (mut value, mut entry, ids) = fixture();
        consume_initial(&mut value, &mut entry, ids[0], frame_a);
        value.set(&entry, clean(), ids[0], 0x4000_7100).unwrap();
        consume_initial(&mut value, &mut entry, ids[1], frame_b);
        assert_eq!(value.get(&entry, clean(), ids[1]), Ok(0));
        value.set(&entry, clean(), ids[1], 0x6003_b000).unwrap();
        for _ in 0..3 {
            consume_initial(&mut value, &mut entry, ids[0], frame_a);
            assert_eq!(value.get(&entry, clean(), ids[0]), Ok(0x4000_7100));
            consume_initial(&mut value, &mut entry, ids[1], frame_b);
            assert_eq!(value.get(&entry, clean(), ids[1]), Ok(0x6003_b000));
        }
        value.retire(&entry, clean(), ids[0]).unwrap();
        assert_eq!(value.tasks[0], task_state::empty());
        assert_eq!(
            value.stage(&mut entry, clean(), ids[0], frame_a),
            Err(tls_error::identity)
        );
        assert_eq!(
            value.retire(&entry, clean(), ids[0]),
            Err(tls_error::identity)
        );
        assert_eq!(value.finish_ready(&entry, clean()), Err(tls_error::state));
        consume_initial(&mut value, &mut entry, ids[1], frame_b);
        assert_eq!(value.get(&entry, clean(), ids[1]), Ok(0x6003_b000));
        value.retire(&entry, clean(), ids[1]).unwrap();
        value.finish_ready(&entry, clean()).unwrap();
        assert_eq!(value.tasks, [task_state::empty(); 2]);
        assert!(
            value
                .finish_ready(
                    &capsule {
                        phase: failed,
                        ..entry
                    },
                    clean()
                )
                .is_err()
        );
        // Numeric identities alone cannot carry FS state into a fresh owner.
        let mut fresh = state::new();
        fresh.prepare(ids, clean()).unwrap();
        consume_initial(&mut fresh, &mut entry, ids[0], frame_a);
        assert_eq!(fresh.get(&entry, clean(), ids[0]), Ok(0));
    }

    #[test]
    fn retired_records_and_finished_owner_are_literal_zero_bytes() {
        // Use nonzero identity components and FS values in both slots so a
        // stale inactive payload cannot accidentally satisfy this byte check.
        let ids = [identity(13, 21), identity(34, 55)];
        let mut value = state::new();
        value.prepare(ids, clean()).unwrap();
        let mut entry = capsule::clean();
        consume_initial(&mut value, &mut entry, ids[0], frame_a);
        value.set(&entry, clean(), ids[0], 0x1234_5678).unwrap();
        consume_initial(&mut value, &mut entry, ids[1], frame_b);
        value.set(&entry, clean(), ids[1], 0x2345_6789).unwrap();
        let peer = value.tasks[1];
        assert!(!value.is_cleared());
        value.retire(&entry, clean(), ids[0]).unwrap();
        assert_eq!(value.tasks[1], peer);
        assert!(all_bytes_zero(&value.tasks[0]));
        value.retire(&entry, clean(), ids[1]).unwrap();
        assert!(all_bytes_zero(&value.tasks[1]));
        value.finish_ready(&entry, clean()).unwrap();
        value.clear();
        assert!(all_bytes_zero(&value));
        assert!(value.is_cleared());
        assert_eq!(core::mem::size_of::<session>(), 96);
    }

    fn all_bytes_zero<T>(value: &T) -> bool {
        // SAFETY: Called only on the repr(C) all-u64 records above, whose exact
        // sizes assert no padding. The immutable borrow keeps every byte live.
        let bytes = core::ptr::from_ref(value).cast::<u8>();
        (0..core::mem::size_of::<T>())
            .all(|index| unsafe { ptr::read_volatile(bytes.add(index)) } == 0)
    }

    #[test]
    fn two_and_three_slot_layouts_remain_bounded_and_duplicate_bindings_are_inert() {
        assert_eq!(core::mem::size_of::<session>(), 96);
        assert_eq!(core::mem::size_of::<session_group<3>>(), 136);
        assert_eq!(core::mem::size_of::<suspended_fs_update>(), 120);
        let ids = [identity(0, 1), identity(1, 2), identity(2, 3)];
        for duplicate in [[ids[0], ids[1], ids[0]], [ids[0], ids[1], ids[1]]] {
            let mut value = state_group::<3>::new();
            assert_eq!(value.prepare(duplicate, clean()), Err(tls_error::identity));
            assert!(value.is_cleared());
        }
        let mut one = state_group::<1>::new();
        assert_eq!(one.prepare([ids[0]], clean()), Err(tls_error::state));
        assert!(one.is_cleared());
        let mut four = state_group::<4>::new();
        assert_eq!(four.prepare([ids[0]; 4], clean()), Err(tls_error::state));
        assert!(four.is_cleared());
        let mut value = state_group::<3>::new();
        value.prepare(ids, clean()).unwrap();
        let mut entry = capsule::clean();
        consume_initial(&mut value, &mut entry, ids[2], frame_b);
        value.set(&entry, clean(), ids[2], user_limit - 1).unwrap();
        assert_eq!(value.get(&entry, clean(), ids[2]), Ok(user_limit - 1));
        for id in ids {
            value.retire(&entry, clean(), id).unwrap();
        }
        value.finish_ready(&entry, clean()).unwrap();
        value.clear();
        assert!(all_bytes_zero(&value));
    }

    #[test]
    fn owner_nonce_exhaustion_never_reuses_a_previous_owner() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(issue_owner(&counter), Ok(u64::MAX - 1));
        assert_eq!(issue_owner(&counter), Ok(u64::MAX));
        assert_eq!(counter.load(Ordering::Relaxed), 0);
        assert_eq!(issue_owner(&counter), Err(tls_error::exhausted));
        assert_eq!(issue_owner(&counter), Err(tls_error::exhausted));
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    struct suspended_fixture {
        _circles: crate::circle::circle_table,
        tasks: task_table,
        scheduler: scheduler_table,
        ids: [task_identity; 3],
        pending: Option<scheduler_call_authorization>,
        waiting: Option<crate::scheduler::scheduler_veil_wait_authorization>,
        value: state_group<3>,
        entry: capsule,
    }
    impl suspended_fixture {
        fn new() -> Self {
            use crate::{
                circle::circle_table,
                identity::fixture_principal_id,
                task::{synthetic_origin_metadata, task_binding, task_bounds},
            };
            let mut circles = circle_table::new();
            let mut tasks = task_table::new();
            let mut scheduler = scheduler_table::new(3).unwrap();
            let ids = core::array::from_fn(|index| {
                let circle = circles.create().unwrap();
                let bounds =
                    task_bounds::new(0x1000, 0x2000, 0x1000, 0x3000, 0x4000, 0x4000).unwrap();
                let id = tasks
                    .create(
                        &mut circles,
                        task_binding::new(
                            circle,
                            fixture_principal_id::new(index as u64 + 1).unwrap(),
                            synthetic_origin_metadata::human_role_fixture,
                            bounds,
                        ),
                    )
                    .unwrap();
                scheduler.admit(&mut circles, &mut tasks, id).unwrap();
                id
            });
            scheduler.start_next(&mut tasks).unwrap();
            let mut value = state_group::<3>::new();
            value.prepare(ids, clean()).unwrap();
            let mut entry = capsule::clean();
            consume_initial(&mut value, &mut entry, ids[0], frame_a);
            value.set(&entry, clean(), ids[0], 0x4000_1234).unwrap();
            let mut pending = None;
            let mut waiting = None;
            scheduler
                .prepare_call_handoff(&mut tasks, ids[0], ids[2], &mut waiting, &mut pending)
                .unwrap()
                .commit();
            consume_initial(&mut value, &mut entry, ids[2], frame_b);
            Self {
                _circles: circles,
                tasks,
                scheduler,
                ids,
                pending,
                waiting,
                value,
                entry,
            }
        }
        fn authority(&self) -> &scheduler_call_authorization {
            self.pending.as_ref().unwrap()
        }
        fn stage(&self, update: &mut suspended_fs_update, base: u64) -> Result<(), tls_error> {
            self.value.stage_suspended(
                &self.entry,
                clean(),
                &self.tasks,
                &self.scheduler,
                self.authority(),
                update,
                base,
            )
        }
        fn query(&self) -> Result<u64, tls_error> {
            self.value.query_suspended(
                &self.entry,
                clean(),
                &self.tasks,
                &self.scheduler,
                self.authority(),
            )
        }
        fn commit(&mut self, update: &mut suspended_fs_update) -> Result<(), tls_error> {
            self.value.commit_suspended(
                &self.entry,
                clean(),
                &self.tasks,
                &self.scheduler,
                self.pending.as_ref().unwrap(),
                update,
            )
        }
    }
    fn proposal_words(update: &suspended_fs_update) -> [u64; 15] {
        // SAFETY: Exact initialized padding-free fifteen-word inert record.
        let words = ptr::from_ref(update).cast::<u64>();
        core::array::from_fn(|index| unsafe { ptr::read_volatile(words.add(index)) })
    }

    #[test]
    fn sealed_fs_effect_is_inert_until_prepared_reply_and_never_sets_the_veil() {
        let mut f = suspended_fixture::new();
        let mut update = suspended_fs_update::new();
        assert_eq!(f.query(), Ok(0x4000_1234));
        assert_eq!(
            f.value.get(&f.entry, clean(), f.ids[0]),
            Err(tls_error::identity)
        );
        let peers = [f.value.tasks[1], f.value.tasks[2]];
        f.stage(&mut update, 0x7000_5678).unwrap();
        assert!(update.is_pending());
        assert_eq!(f.query(), Ok(0x4000_1234));
        assert_eq!(f.value.get(&f.entry, clean(), f.ids[2]), Ok(0));
        let reply = f
            .scheduler
            .prepare_call_reply(&mut f.tasks, &mut f.pending, &mut f.waiting)
            .unwrap();
        f.value
            .commit_suspended(
                &f.entry,
                clean(),
                reply.tasks(),
                reply.scheduler(),
                reply.authorization(),
                &mut update,
            )
            .unwrap();
        assert!(update.storage_is_cleared() && all_bytes_zero(&update));
        assert_eq!([f.value.tasks[1], f.value.tasks[2]], peers);
        assert_eq!(reply.commit().task(), f.ids[0]);
        consume_initial(&mut f.value, &mut f.entry, f.ids[0], frame_a);
        assert_eq!(f.value.get(&f.entry, clean(), f.ids[0]), Ok(0x7000_5678));
        assert!(f.pending.is_none());
    }

    #[test]
    fn failed_fs_proposals_and_commits_preserve_exact_owner_and_retained_stage() {
        let mut f = suspended_fixture::new();
        let foreign = suspended_fixture::new();
        assert_eq!(f.ids, foreign.ids);
        let mut update = suspended_fs_update::new();
        for bad in [user_limit, 0xffff_8000_0000_0000, u64::MAX] {
            assert_eq!(f.stage(&mut update, bad), Err(tls_error::base));
            assert!(update.storage_is_cleared());
        }
        f.stage(&mut update, 0).unwrap();
        let old_update = proposal_words(&update);
        let old_tasks = f.value.tasks;
        assert_eq!(f.stage(&mut update, 7), Err(tls_error::state));
        // A genuine authority for a different scheduler can have every public
        // identity and sequence equal. The private correlation still rejects it.
        assert_eq!(f.authority().sequence(), foreign.authority().sequence());
        assert_eq!(
            f.value.commit_suspended(
                &f.entry,
                clean(),
                &foreign.tasks,
                &foreign.scheduler,
                foreign.authority(),
                &mut update
            ),
            Err(tls_error::authorization)
        );
        assert_eq!(f.value.tasks, old_tasks);
        assert_eq!(proposal_words(&update), old_update);
        assert_eq!(
            f.value.query_suspended(
                &f.entry,
                clean(),
                &f.tasks,
                &foreign.scheduler,
                f.authority()
            ),
            Err(tls_error::authorization)
        );
        f.value.active = 99;
        assert_eq!(f.commit(&mut update), Err(tls_error::authorization));
        f.value.active = 1;
        f.value.tasks[0].fs ^= 1;
        assert_eq!(f.commit(&mut update), Err(tls_error::authorization));
        f.value.tasks = old_tasks;
        f.value.current = 1;
        assert_eq!(f.query(), Err(tls_error::identity));
        f.value.current = 3;
        let bad_controls = controls { fs: 1, ..clean() };
        assert_eq!(
            f.value.commit_suspended(
                &f.entry,
                bad_controls,
                &f.tasks,
                &f.scheduler,
                f.pending.as_ref().unwrap(),
                &mut update
            ),
            Err(tls_error::controls)
        );
        assert_eq!(f.value.tasks, old_tasks);
        assert_eq!(proposal_words(&update), old_update);
        // Discard is explicitly inert and clears the actual retained bytes.
        update.discard();
        assert!(update.storage_is_cleared() && all_bytes_zero(&update));
        assert_eq!(f.commit(&mut update), Err(tls_error::authorization));
        assert_eq!(f.value.tasks, old_tasks);
    }

    #[test]
    fn fs_stage_cannot_replay_into_a_later_call_or_survive_dependency_retirement() {
        let mut f = suspended_fixture::new();
        let mut update = suspended_fs_update::new();
        f.stage(&mut update, 0x6000_0000).unwrap();
        let staged_words = proposal_words(&update);
        f.scheduler
            .prepare_call_reply(&mut f.tasks, &mut f.pending, &mut f.waiting)
            .unwrap()
            .commit();
        consume_initial(&mut f.value, &mut f.entry, f.ids[0], frame_a);
        f.scheduler
            .prepare_call_handoff(
                &mut f.tasks,
                f.ids[0],
                f.ids[2],
                &mut f.waiting,
                &mut f.pending,
            )
            .unwrap()
            .commit();
        consume_initial(&mut f.value, &mut f.entry, f.ids[2], frame_b);
        assert_eq!(f.commit(&mut update), Err(tls_error::authorization));
        assert_eq!(proposal_words(&update), staged_words);
        assert_eq!(f.query(), Ok(0x4000_1234));
        let fault = f
            .scheduler
            .prepare_call_fault(&mut f.tasks, &mut f.pending)
            .unwrap();
        fault.commit();
        f.value.retire(&f.entry, clean(), f.ids[0]).unwrap();
        f.value.retire(&f.entry, clean(), f.ids[2]).unwrap();
        assert!(f.pending.is_none());
        assert!(all_bytes_zero(&f.value.tasks[0]));
        assert!(all_bytes_zero(&f.value.tasks[2]));
        update.discard();
        assert!(update.storage_is_cleared());
        assert_eq!(f.value.tasks[1].fs, 0);
    }
}
