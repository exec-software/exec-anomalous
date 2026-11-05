#![allow(dead_code)]

//! Feature-private eager legacy floating-point ownership for the application
//! proof. The Rust kernel target is soft-float: only these explicit instructions
//! touch FP state between user capture and the final user restore. No lazy #NM,
//! XSAVE/AVX state, TLS, or stable userspace context format is admitted here.

use core::{arch::asm, ptr};

const image_size: usize = 512;
const default_control: u16 = 0x037f;
const default_mxcsr: u32 = 0x1f80;
const fallback_mxcsr_mask: u32 = 0xffbf;
const cr0_monitor_coprocessor: u64 = 1 << 1;
const cr0_emulation: u64 = 1 << 2;
const cr0_task_switched: u64 = 1 << 3;
const cr0_numeric_error: u64 = 1 << 5;
const cr4_osfxsr: u64 = 1 << 9;
const cr4_osxmmexcpt: u64 = 1 << 10;
const cr4_fsgsbase: u64 = 1 << 16;
const cr4_osxsave: u64 = 1 << 18;
const efer_msr: u32 = 0xc000_0080;
const sysenter_cs_msr: u32 = 0x174;
const efer_syscall_enable: u64 = 1;
const efer_ffxsr: u64 = 1 << 14;

#[repr(C, align(16))]
pub(super) struct fp_image {
    bytes: [u8; image_size],
}

impl fp_image {
    #[cfg(any(
        test,
        feature = "owned_workspace_cpu_test",
        feature = "mixed_workspace_test"
    ))]
    pub(super) const fn workspace_observation() -> Self {
        Self::zero()
    }

    #[cfg(any(
        test,
        feature = "owned_workspace_cpu_test",
        feature = "mixed_workspace_test"
    ))]
    pub(super) fn clear_workspace_observation(&mut self) -> bool {
        self.clear();
        self.is_cleared()
    }

    const fn zero() -> Self {
        Self {
            bytes: [0; image_size],
        }
    }

    fn initialize(&mut self) {
        self.clear();
        self.bytes[0..2].copy_from_slice(&default_control.to_le_bytes());
        self.bytes[24..28].copy_from_slice(&default_mxcsr.to_le_bytes());
    }

    pub(super) fn bytes(&self) -> &[u8; image_size] {
        &self.bytes
    }

    pub(super) fn is_initial_clean(&self) -> bool {
        self.control_word() == default_control
            && self.bytes[2..5].iter().all(|byte| *byte == 0)
            && self.bytes[6..24].iter().all(|byte| *byte == 0)
            && self.mxcsr() == default_mxcsr
            && (0..8).all(|index| {
                self.bytes[32 + 16 * index..42 + 16 * index]
                    .iter()
                    .all(|byte| *byte == 0)
            })
            && self.bytes[160..416].iter().all(|byte| *byte == 0)
    }

    pub(super) fn control_word(&self) -> u16 {
        u16::from_le_bytes([self.bytes[0], self.bytes[1]])
    }

    pub(super) fn mxcsr(&self) -> u32 {
        u32::from_le_bytes(self.bytes[24..28].try_into().unwrap())
    }

    fn mxcsr_mask(&self) -> u32 {
        let reported = u32::from_le_bytes(self.bytes[28..32].try_into().unwrap());
        if reported == 0 {
            fallback_mxcsr_mask
        } else {
            reported
        }
    }

    fn architectural_eq(&self, other: &Self) -> bool {
        // Reserved bytes, MXCSR_MASK and software bytes are not restored state.
        // Each x87 slot has ten meaningful bytes and six padding bytes.
        self.bytes[0..5] == other.bytes[0..5]
            && self.bytes[6..28] == other.bytes[6..28]
            && (0..8).all(|index| {
                self.bytes[32 + 16 * index..42 + 16 * index]
                    == other.bytes[32 + 16 * index..42 + 16 * index]
            })
            && self.bytes[160..416] == other.bytes[160..416]
    }

    fn clear(&mut self) {
        for byte in &mut self.bytes {
            // SAFETY: Every byte belongs to this unique retained image. Volatile
            // stores keep the terminal wipe observable before ownership ends.
            unsafe { ptr::write_volatile(byte, 0) };
        }
    }

    fn is_cleared(&self) -> bool {
        self.bytes.iter().all(|byte| {
            // SAFETY: The owner keeps all image bytes initialized and resident.
            unsafe { ptr::read_volatile(byte) == 0 }
        })
    }

    unsafe fn save(&mut self) {
        // SAFETY: The caller owns the live CPU and has admitted/enabled FXSAVE.
        // repr(align(16)) and the complete resident 512-byte owner satisfy the
        // instruction's memory requirements. REX.W preserves 64-bit FIP/FDP.
        unsafe {
            asm!("fxsave64 [{}]", in(reg) self.bytes.as_mut_ptr(), options(nostack, preserves_flags));
        }
    }

    unsafe fn restore(&self) {
        // SAFETY: Only this owner creates images or captures them from the CPU;
        // the caller validated MXCSR and admitted exact FIP/FDP restoration.
        unsafe {
            asm!("fxrstor64 [{}]", in(reg) self.bytes.as_ptr(), options(nostack, preserves_flags));
        }
    }
}

fn admits_cpu(vendor: [u8; 12], feature_edx: u32, extended_max: u32, extended_ebx: u32) -> bool {
    const required: u32 = (1 << 0) | (1 << 24) | (1 << 25) | (1 << 26);
    if feature_edx & required != required {
        return false;
    }
    match &vendor {
        b"GenuineIntel" => true,
        // AMD's XSaveErPtr bit guarantees unconditional save AND restore of
        // x87 error pointers. The legacy Linux FXSAVE_LEAK scrub prevents a
        // leak but does not restore arbitrary original kernel pointers exactly.
        b"AuthenticAMD" | b"HygonGenuine" => {
            extended_max >= 0x8000_0008 && extended_ebx & (1 << 2) != 0
        }
        _ => false,
    }
}

pub(super) type cpu_session = cpu_session_group<2>;

/// One retained eager legacy CPU image per task. Admission is feature-private:
/// two/three normally, shared-veil five, and eager-workspace or program-bearing
/// one. The legacy alias preserves its layout.
#[must_use = "retain the CPU owner and call finish before releasing its resident storage"]
pub(super) struct cpu_session_group<const task_count: usize> {
    kernel: fp_image,
    tasks: [fp_image; task_count],
    clean: fp_image,
    original_cr0: u64,
    original_cr4: u64,
    original_efer: u64,
    mxcsr_mask: u32,
    active: bool,
    user_owner: Option<usize>,
    vendor: [u8; 12],
    #[cfg(feature = "linux_application_test")]
    linux_entry: super::linux_syscall::syscall_session,
}

impl<const task_count: usize> cpu_session_group<task_count> {
    pub(super) const fn new() -> Self {
        Self {
            kernel: fp_image::zero(),
            tasks: [const { fp_image::zero() }; task_count],
            clean: fp_image::zero(),
            original_cr0: 0,
            original_cr4: 0,
            original_efer: 0,
            mxcsr_mask: 0,
            active: false,
            user_owner: None,
            vendor: [0; 12],
            #[cfg(feature = "linux_application_test")]
            linux_entry: super::linux_syscall::syscall_session::new(),
        }
    }

    /// # Safety
    /// Sole BSP at CPL0, IF clear and NMI masked. Keep this owner supervisor-only
    /// and resident in every selected root until finish. The caller must retain
    /// ownership on all later failures; moving or dropping an active owner is
    /// forbidden. The entire kernel continuation uses the soft-float target.
    pub(super) unsafe fn prepare(&mut self) -> Result<(), &'static str> {
        use core::arch::x86_64::__cpuid;

        if !(matches!(task_count, 2 | 3)
            || (cfg!(feature = "shared_veil_test") && task_count == 5)
            || (cfg!(feature = "owned_workspace_cpu_test") && task_count == 1)
            || (cfg!(feature = "native_program_test") && task_count == 1))
            || self.active
            || self.user_owner.is_some()
            || !interrupts_are_clear()
        {
            return Err("application_cpu_prepare_state");
        }
        let basic = __cpuid(0);
        let mut vendor = [0; 12];
        vendor[0..4].copy_from_slice(&basic.ebx.to_le_bytes());
        vendor[4..8].copy_from_slice(&basic.edx.to_le_bytes());
        vendor[8..12].copy_from_slice(&basic.ecx.to_le_bytes());
        let extended_max = __cpuid(0x8000_0000).eax;
        let extended_ebx = if extended_max >= 0x8000_0008 {
            __cpuid(0x8000_0008).ebx
        } else {
            0
        };
        if basic.eax < 1 {
            return Err("application_cpu_capabilities");
        }
        let feature_edx = __cpuid(1).edx;
        if !admits_cpu(vendor, feature_edx, extended_max, extended_ebx) {
            if matches!(&vendor, b"AuthenticAMD" | b"HygonGenuine")
                && (extended_max < 0x8000_0008 || extended_ebx & 4 == 0)
            {
                return Err("application_cpu_amd_error_pointers");
            }
            return Err("application_cpu_capabilities");
        }
        self.vendor = vendor;
        self.original_cr0 = read_cr0();
        self.original_cr4 = read_cr4();
        self.original_efer = read_efer();
        // FXSAVE cannot retain extended XSTATE. Reject an already-enabled
        // XSAVE environment instead of disabling it and silently losing state.
        // FSGSBASE would let users mutate TLS bases which this profile excludes.
        if self.original_cr4 & (cr4_osxsave | cr4_fsgsbase) != 0 {
            return Err("application_cpu_extended_state");
        }
        // There is no owned SYSCALL entry boundary in this feature. Keep that
        // instruction disabled rather than accepting boot-provided MSR targets.
        if self.original_efer & efer_syscall_enable != 0 {
            return Err("application_cpu_syscall_enabled");
        }
        // Intel SYSENTER is independent of EFER.SCE. A zero selector forces
        // #GP rather than entering an unowned bootstrap MSR target. Check only
        // when CPUID.SEP advertises this MSR interface (AMD long mode is #UD).
        if feature_edx & (1 << 11) != 0 && read_msr(sysenter_cs_msr) & 0xffff != 0 {
            return Err("application_cpu_sysenter_enabled");
        }
        self.kernel.clear();
        for image in &mut self.tasks {
            image.initialize();
        }
        self.clean.initialize();
        // SAFETY: Unrelated and fixed control bits are preserved. FFXSR must
        // be clear at CPL0 or AMD can skip XMM save/restore despite OSFXSR.
        // No FP instruction occurs until all required controls are enabled.
        unsafe {
            write_efer(self.original_efer & !efer_ffxsr);
            write_cr4(self.operating_cr4());
            write_cr0(self.operating_cr0());
            self.kernel.save();
        }
        self.mxcsr_mask = self.kernel.mxcsr_mask();
        self.active = true;
        if default_mxcsr & !self.mxcsr_mask != 0 || self.kernel.mxcsr() & !self.mxcsr_mask != 0 {
            // Nothing has changed the original FP state yet. Always restore
            // controls and clear retained storage even on this admission error.
            unsafe { self.restore_controls() };
            self.clear_storage();
            self.active = false;
            return Err("application_cpu_mxcsr_mask");
        }
        // A full canonical image clears XMM and the underlying empty x87
        // register payloads. FNINIT alone would leave those payloads behind.
        unsafe { self.clean.restore() };
        Ok(())
    }

    pub(super) fn task_image(&self, index: usize) -> Option<&fp_image> {
        self.tasks.get(index)
    }

    /// Observe actual handler state in separate retained scratch after capture.
    /// Never overwrite the canonical clean image or accept a user restore image.
    ///
    /// # Safety
    /// Same sole-BSP resident-image contract as save_current; IF clear and no
    /// user image live. The observation remains supervisor-only and is wiped.
    #[cfg(any(
        test,
        feature = "owned_workspace_cpu_test",
        feature = "mixed_workspace_test"
    ))]
    pub(super) unsafe fn workspace_clean_observed(&self, observed: &mut fp_image) -> bool {
        if !self.controls_are_active() || self.user_owner.is_some() {
            return false;
        }
        unsafe { observed.save() };
        observed.is_initial_clean() && self.clean.architectural_eq(observed)
    }

    /// A captured user is quiescent while the clean kernel image remains live.
    /// Maintenance retains this session and never restores bootstrap controls.
    #[cfg(feature = "native_relaunch_test")]
    pub(super) fn maintenance_is_ready(&self) -> bool {
        task_count == 2 && self.controls_are_active() && self.user_owner.is_none()
    }

    /// Reset only the retired editor's image before another task occupies slot
    /// zero. The controller has withdrawn that task and retains service slot one.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked, on the bootstrap maintenance continuation.
    /// No old editor context can resume, and the service image stays retained.
    #[cfg(feature = "native_relaunch_test")]
    pub(super) unsafe fn reset_native_editor(&mut self) -> bool {
        if !self.maintenance_is_ready() {
            return false;
        }
        self.tasks[0].initialize();
        self.tasks[0].is_initial_clean()
    }

    /// Establish the same legacy CPU profile plus an explicitly owned SYSCALL
    /// boundary. Initial SCE remains required clear; `prepare` and all native
    /// profiles keep their original rejection of boot-provided entry targets.
    ///
    /// # Safety
    /// Same stable resident, sole-BSP and failure-retention contract as prepare.
    /// IRQ/NMI exclusion continues through MSR installation and every selection.
    #[cfg(feature = "linux_application_test")]
    pub(super) unsafe fn prepare_linux(&mut self) -> Result<(), &'static str> {
        unsafe {
            self.prepare()?;
            self.linux_entry.prepare(self.original_efer & !efer_ffxsr)
        }
    }

    /// # Safety
    /// Previous user state captured, IF clear/NMI masked. The execution owner
    /// has already selected this exact retained supervisor stack in TSS.RSP0.
    /// Enable only for the admitted Linux producer, before restore_task/IRETQ.
    #[cfg(feature = "linux_application_test")]
    pub(super) unsafe fn select_linux_entry(&mut self, stack_top: u64, enabled: bool) -> bool {
        if !self.controls_are_active() || self.user_owner.is_some() {
            return false;
        }
        unsafe { self.linux_entry.select(stack_top, enabled) }
    }

    #[cfg(feature = "linux_application_test")]
    pub(super) fn linux_entry_matches(&self, stack_top: u64, enabled: bool) -> bool {
        self.linux_entry.selection_matches(stack_top, enabled)
    }

    #[cfg(feature = "linux_application_test")]
    pub(super) fn linux_entry_is_restored(&self) -> bool {
        !self.active
            && read_efer() == self.original_efer
            && self.linux_entry.restoration_is_verified()
    }

    #[cfg(feature = "linux_application_test")]
    pub(super) fn linux_entry_storage_is_cleared(&self) -> bool {
        !self.linux_entry.is_active() && self.linux_entry.storage_is_cleared()
    }

    pub(super) fn cpu_vendor(&self) -> &'static str {
        match &self.vendor {
            b"GenuineIntel" => "GenuineIntel",
            b"AuthenticAMD" => "AuthenticAMD",
            b"HygonGenuine" => "HygonGenuine",
            _ => "unadmitted",
        }
    }

    pub(super) fn controls_are_active(&self) -> bool {
        if !self.active
            || !interrupts_are_clear()
            || read_cr0() != self.operating_cr0()
            || read_cr4() != self.operating_cr4()
        {
            return false;
        }
        #[cfg(feature = "linux_application_test")]
        let entry_matches = if self.linux_entry.is_active() {
            self.linux_entry
                .controls_are_active(self.original_efer & !efer_ffxsr)
        } else {
            read_efer() == self.original_efer & !efer_ffxsr
        };
        #[cfg(not(feature = "linux_application_test"))]
        let entry_matches = read_efer() == self.original_efer & !efer_ffxsr;
        entry_matches
    }

    /// # Safety
    /// The current nonnested user entry belongs to index. Call before any
    /// kernel FP use or task selection, with every image still resident.
    pub(super) unsafe fn save_current(&mut self, index: usize) -> bool {
        if !self.controls_are_active()
            || self.user_owner != Some(index)
            || index >= self.tasks.len()
        {
            return false;
        }
        // FXSAVE/FXRSTOR do not wait on a pending unmasked x87 exception.
        // Loading the clean kernel image prevents user pending status from
        // affecting the kernel or the selected peer while Rust dispatch runs.
        unsafe {
            self.tasks[index].save();
            self.clean.restore();
        }
        self.user_owner = None;
        self.tasks[index].mxcsr() & !self.mxcsr_mask == 0
    }

    /// # Safety
    /// Index identifies the validated selected task. No FP instruction may
    /// occur after this restore and before IRET. Same-task returns also restore.
    pub(super) unsafe fn restore_task(&mut self, index: usize) -> bool {
        if !self.controls_are_active() || self.user_owner.is_some() {
            return false;
        }
        let Some(image) = self.tasks.get(index) else {
            return false;
        };
        if image.mxcsr() & !self.mxcsr_mask != 0 {
            return false;
        }
        unsafe { image.restore() };
        self.user_owner = Some(index);
        true
    }

    /// # Safety
    /// No user can resume, IF clear/NMI masked, last user state already saved,
    /// and all images remain supervisor-resident through the verified wipe.
    /// Restore the bootstrap root/stack before releasing any memory owner.
    pub(super) unsafe fn finish(&mut self) -> bool {
        if !self.controls_are_active() || self.user_owner.is_some() {
            return false;
        }
        // Revoke SYSCALL and restore its three entry MSRs while all stacks are
        // still retained. This restores our operating EFER; restore_controls
        // below then restores the caller's complete original EFER, including
        // FFXSR. A failed readback retains both owners and every dependency.
        #[cfg(feature = "linux_application_test")]
        if self.linux_entry.is_active() && !unsafe { self.linux_entry.finish() } {
            return false;
        }
        // Restore under the operating controls, then observe only defined
        // fields with a nonwaiting FXSAVE. Never FNINIT/FWAIT after restoration.
        unsafe {
            self.kernel.restore();
            self.clean.clear();
            self.clean.save();
        }
        let restored = self.kernel.architectural_eq(&self.clean);
        unsafe { self.restore_controls() };
        let controls_restored = read_cr0() == self.original_cr0
            && read_cr4() == self.original_cr4
            && read_efer() == self.original_efer;
        self.clear_storage();
        self.active = false;
        self.user_owner = None;
        restored && controls_restored && self.storage_is_cleared()
    }

    pub(super) fn storage_is_cleared(&self) -> bool {
        #[cfg(feature = "linux_application_test")]
        let entry_cleared = self.linux_entry_storage_is_cleared();
        #[cfg(not(feature = "linux_application_test"))]
        let entry_cleared = true;
        self.kernel.is_cleared()
            && self.tasks.iter().all(fp_image::is_cleared)
            && self.clean.is_cleared()
            && entry_cleared
    }

    fn clear_storage(&mut self) {
        self.kernel.clear();
        for image in &mut self.tasks {
            image.clear();
        }
        self.clean.clear();
        self.mxcsr_mask = 0;
    }

    fn operating_cr0(&self) -> u64 {
        (self.original_cr0 | cr0_monitor_coprocessor | cr0_numeric_error)
            & !(cr0_emulation | cr0_task_switched)
    }

    fn operating_cr4(&self) -> u64 {
        self.original_cr4 | cr4_osfxsr | cr4_osxmmexcpt
    }

    unsafe fn restore_controls(&self) {
        // No FP instruction follows: the original controls may disable FXSAVE
        // or set TS again. Preserve every original bit, including FFXSR.
        unsafe {
            write_efer(self.original_efer);
            write_cr4(self.original_cr4);
            write_cr0(self.original_cr0);
        }
    }
}

fn interrupts_are_clear() -> bool {
    let flags: u64;
    unsafe { asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags)) };
    flags & (1 << 9) == 0
}

fn read_cr0() -> u64 {
    let value: u64;
    unsafe { asm!("mov {}, cr0", out(reg) value, options(nomem, nostack, preserves_flags)) };
    value
}

fn read_cr4() -> u64 {
    let value: u64;
    unsafe { asm!("mov {}, cr4", out(reg) value, options(nomem, nostack, preserves_flags)) };
    value
}

fn read_efer() -> u64 {
    read_msr(efer_msr)
}

fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high,
            options(nomem, nostack, preserves_flags));
    }
    (u64::from(high) << 32) | u64::from(low)
}

unsafe fn write_cr0(value: u64) {
    unsafe { asm!("mov cr0, {}", in(reg) value, options(nostack, preserves_flags)) };
}

unsafe fn write_cr4(value: u64) {
    unsafe { asm!("mov cr4, {}", in(reg) value, options(nostack, preserves_flags)) };
}

unsafe fn write_efer(value: u64) {
    unsafe {
        asm!("wrmsr", in("ecx") efer_msr, in("eax") value as u32, in("edx") (value >> 32) as u32,
            options(nostack, preserves_flags));
    }
}

#[cfg(not(test))]
core::arch::global_asm!(
    include_str!("application_cpu_entry.S"),
    owned_tls_enabled = const cfg!(all(not(test), any(feature = "owned_tls_test", feature = "linux_tls_test"))) as usize,
    options(att_syntax)
);

// This continuation deliberately keeps the CPU session active with its clean
// kernel image selected. It is separate from the terminal resume contract,
// which requires restoration of original controls and a complete session wipe.
#[cfg(all(not(test), feature = "native_relaunch_test"))]
core::arch::global_asm!(
    ".text",
    ".global x86_64_application_checkpoint_kernel",
    ".type x86_64_application_checkpoint_kernel, @function",
    "x86_64_application_checkpoint_kernel:",
    "cli",
    "movw $0x10, %ax",
    "movw %ax, %ds",
    "movw %ax, %es",
    "movw %ax, %ss",
    "movw %ax, %fs",
    "movw %ax, %gs",
    ".if {owned_tls_enabled}",
    "callq x86_64_task_tls_resume_kernel",
    ".endif",
    "movq %rsi, %rsp",
    "popq %r15",
    "popq %r14",
    "popq %r13",
    "popq %r12",
    "popq %rbp",
    "popq %rbx",
    "movq %rdi, %rax",
    "retq",
    ".size x86_64_application_checkpoint_kernel, . - x86_64_application_checkpoint_kernel",
    owned_tls_enabled = const cfg!(all(not(test), any(feature = "owned_tls_test", feature = "linux_tls_test"))) as usize,
    options(att_syntax)
);

#[cfg(all(not(test), feature = "native_relaunch_test"))]
unsafe extern "C" {
    /// # Safety
    /// Exact single-use suspended application-enter continuation; bootstrap
    /// CR3/TSS restored, IF clear, saved service CPU image retained, clean kernel
    /// image active. All subsequent maintenance Rust remains integer-only.
    pub(super) fn x86_64_application_checkpoint_kernel(status: u64, kernel_rsp: u64) -> !;
}

#[cfg(not(test))]
unsafe extern "C" {
    pub(super) fn x86_64_application_enter(
        frame: *const super::scheduler_test::scheduler_frame,
        kernel_rsp: *mut u64,
    ) -> u64;
    pub(super) fn x86_64_application_resume_kernel(status: u64, kernel_rsp: u64) -> !;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_task_images_are_distinct_aligned_initialized_and_fully_wiped() {
        let mut session = cpu_session_group::<3>::new();
        assert!(session.storage_is_cleared());
        for image in &mut session.tasks {
            assert_eq!(image as *mut fp_image as usize % 16, 0);
            image.initialize();
            assert!(image.is_initial_clean());
        }
        for (index, image) in session.tasks.iter_mut().enumerate() {
            image.bytes[160..176].fill(index as u8 + 1);
        }
        for index in 0..3 {
            assert_eq!(
                &session.task_image(index).unwrap().bytes()[160..176],
                &[index as u8 + 1; 16]
            );
        }
        assert!(session.task_image(3).is_none());
        assert!(session.kernel.is_cleared());
        assert!(session.clean.is_cleared());
        session.clear_storage();
        assert!(session.storage_is_cleared());
        assert_eq!(
            core::mem::size_of::<cpu_session_group<3>>(),
            core::mem::size_of::<cpu_session>() + 512
        );
        // SAFETY: Unsupported capacity returns before any CPU state inspection.
        assert_eq!(
            unsafe { cpu_session_group::<4>::new().prepare() },
            Err("application_cpu_prepare_state")
        );
        #[cfg(not(feature = "owned_workspace_cpu_test"))]
        assert_eq!(
            unsafe { cpu_session_group::<1>::new().prepare() },
            Err("application_cpu_prepare_state")
        );
    }

    #[cfg(feature = "shared_veil_test")]
    #[test]
    fn five_cpu_images_keep_independent_state_and_clear_all_slots() {
        let mut session = cpu_session_group::<5>::new();
        for (index, image) in session.tasks.iter_mut().enumerate() {
            assert_eq!(image as *mut fp_image as usize % 16, 0);
            image.initialize();
            image.bytes[160..176].fill(index as u8 + 1);
        }
        for index in 0..5 {
            assert_eq!(
                &session.task_image(index).unwrap().bytes()[160..176],
                &[index as u8 + 1; 16]
            );
        }
        assert!(session.task_image(5).is_none());
        assert!(session.kernel.is_cleared());
        session.clear_storage();
        assert!(session.storage_is_cleared());
    }

    #[test]
    fn cpu_admission_requires_complete_legacy_state_and_exact_amd_error_pointers() {
        let required = (1 << 0) | (1 << 24) | (1 << 25) | (1 << 26);
        for vendor in [*b"GenuineIntel", *b"AuthenticAMD", *b"HygonGenuine"] {
            assert!(admits_cpu(vendor, required, 0x8000_0008, 4));
            for bit in [0, 24, 25, 26] {
                assert!(!admits_cpu(vendor, required & !(1 << bit), 0x8000_0008, 4));
            }
        }
        assert!(admits_cpu(*b"GenuineIntel", required, 0, 0));
        for vendor in [*b"AuthenticAMD", *b"HygonGenuine"] {
            assert!(!admits_cpu(vendor, required, 0x8000_0007, 4));
            assert!(!admits_cpu(vendor, required, 0x8000_0008, 0));
        }
        assert!(!admits_cpu(*b"unknown cpu!", required, 0x8000_0008, 4));
    }

    #[test]
    fn image_geometry_defaults_and_wipe_cover_all_retained_bytes() {
        assert_eq!(core::mem::size_of::<fp_image>(), 512);
        assert_eq!(core::mem::align_of::<fp_image>(), 16);
        let mut session = cpu_session::new();
        assert!(session.storage_is_cleared());
        for image in [
            &mut session.kernel,
            &mut session.tasks[0],
            &mut session.clean,
        ] {
            image.bytes.fill(0xa5);
            image.initialize();
            assert!(image.is_initial_clean());
            assert_eq!(image.mxcsr_mask(), fallback_mxcsr_mask);
            image.bytes[28..32].copy_from_slice(&0xffff_u32.to_le_bytes());
            assert_eq!(image.mxcsr_mask(), 0xffff);
        }
        session.tasks[1].bytes.fill(0x5a);
        session.clear_storage();
        assert!(session.storage_is_cleared());
    }

    #[test]
    fn exact_restore_comparison_ignores_only_nonarchitectural_bytes() {
        let mut original = fp_image::zero();
        original.initialize();
        for offset in 0..512 {
            let mut observed = fp_image::zero();
            observed.bytes.copy_from_slice(original.bytes());
            observed.bytes[offset] ^= 1;
            let defined = offset < 5
                || (6..28).contains(&offset)
                || ((32..160).contains(&offset) && (offset - 32) % 16 < 10)
                || (160..416).contains(&offset);
            assert_eq!(
                original.architectural_eq(&observed),
                !defined,
                "offset {offset}"
            );
        }
    }
}
