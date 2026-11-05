#![cfg_attr(not(test), allow(dead_code))]

use core::arch::{asm, x86_64::__cpuid};

pub(super) const page_size: u64 = 4096;
pub(super) const entries_per_table: usize = 512;
pub(super) const identity_map_limit: u64 = 4 * 1024 * 1024 * 1024;

pub(super) const present: u64 = 1 << 0;
pub(super) const writable: u64 = 1 << 1;
pub(super) const user_accessible: u64 = 1 << 2;
pub(super) const huge_page: u64 = 1 << 7;
pub(super) const ownership_marker: u64 = 1 << 9;
pub(super) const no_execute: u64 = 1 << 63;
pub(super) const page_address_mask: u64 = 0x000f_ffff_ffff_f000;

const efer_msr: u32 = 0xc000_0080;
const efer_nxe: u64 = 1 << 11;
const cr0_write_protect: u64 = 1 << 16;
const nx_cpuid_leaf: u32 = 0x8000_0001;
const nx_cpuid_bit: u32 = 1 << 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct page_permissions {
    pub user: bool,
    pub writable: bool,
    pub executable: bool,
}

impl page_permissions {
    pub(super) const supervisor_read_write: Self = Self {
        user: false,
        writable: true,
        executable: false,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct effective_permissions {
    pub present: bool,
    pub user: bool,
    pub writable: bool,
    pub executable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct page_indexes {
    pub pml4: usize,
    pub directory_pointer: usize,
    pub directory: usize,
    pub table: usize,
}

/// Enable the architectural controls required by managed NX and read-only
/// supervisor mappings. Both controls remain enabled for the kernel lifetime.
pub(super) fn enable_and_verify_protections() -> bool {
    let extended_maximum = __cpuid(0x8000_0000).eax;
    if extended_maximum < nx_cpuid_leaf || __cpuid(nx_cpuid_leaf).edx & nx_cpuid_bit == 0 {
        return false;
    }

    let efer = read_msr(efer_msr);
    // SAFETY: CPUID established NX support and EFER is an architectural MSR.
    unsafe { write_msr(efer_msr, efer | efer_nxe) };
    let cr0 = read_cr0();
    // SAFETY: Setting CR0.WP is architectural and preserves every other bit.
    unsafe { write_cr0(cr0 | cr0_write_protect) };
    protections_are_enabled()
}

/// Check the controls which make managed read-only and NX mappings effective.
#[must_use]
pub(super) fn protections_are_enabled() -> bool {
    read_msr(efer_msr) & efer_nxe != 0 && read_cr0() & cr0_write_protect != 0
}

#[must_use]
pub(super) const fn indexes(address: u64) -> page_indexes {
    page_indexes {
        pml4: ((address >> 39) & 0x1ff) as usize,
        directory_pointer: ((address >> 30) & 0x1ff) as usize,
        directory: ((address >> 21) & 0x1ff) as usize,
        table: ((address >> 12) & 0x1ff) as usize,
    }
}

#[must_use]
pub(super) const fn is_canonical(address: u64) -> bool {
    address <= 0x0000_7fff_ffff_ffff || address >= 0xffff_8000_0000_0000
}

#[must_use]
pub(super) const fn leaf_flags(permissions: page_permissions) -> Option<u64> {
    if permissions.writable && permissions.executable {
        return None;
    }
    let mut flags = present;
    if permissions.user {
        flags |= user_accessible;
    }
    if permissions.writable {
        flags |= writable;
    }
    if !permissions.executable {
        flags |= no_execute;
    }
    Some(flags)
}

#[must_use]
pub(super) fn effective_permissions_from_path(path: &[u64]) -> effective_permissions {
    let is_present = path.iter().all(|entry| entry & present != 0) && !path.is_empty();
    effective_permissions {
        present: is_present,
        user: is_present && path.iter().all(|entry| entry & user_accessible != 0),
        writable: is_present && path.iter().all(|entry| entry & writable != 0),
        executable: is_present && path.iter().all(|entry| entry & no_execute == 0),
    }
}

#[must_use]
pub(super) const fn entry_address(entry: u64) -> u64 {
    entry & page_address_mask
}

#[must_use]
pub(super) fn read_cr0() -> u64 {
    let value: u64;
    // SAFETY: The kernel invokes this at CPL 0 on x86-64.
    unsafe {
        asm!(
            "mov {}, cr0",
            out(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Replace CR0 while preserving every fixed and unrelated bit selected by the
/// caller.
///
/// # Safety
///
/// The caller must supply an architecturally valid value and account for any
/// paging, cache, or task-state effects caused by changed bits.
pub(super) unsafe fn write_cr0(value: u64) {
    // SAFETY: The caller owns the complete control-register transition.
    unsafe {
        asm!(
            "mov cr0, {}",
            in(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[must_use]
pub(super) fn read_cr3() -> u64 {
    let value: u64;
    // SAFETY: The kernel invokes this at CPL 0 on x86-64.
    unsafe {
        asm!(
            "mov {}, cr3",
            out(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Select an already initialized page-table root and flush non-global
/// translations for the current processor.
///
/// # Safety
///
/// `value` must name an aligned, live x86-64 root which maps the executing
/// instruction, stack, and every subsequently accessed object with compatible
/// permissions. Its complete hierarchy must remain owned until another root is
/// selected.
pub(super) unsafe fn write_cr3(value: u64) {
    // SAFETY: The caller establishes the live-root and mapping invariant.
    unsafe {
        asm!(
            "mov cr3, {}",
            in(reg) value,
            options(nostack, preserves_flags)
        );
    }
}

fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: Callers use an architectural MSR after checking its feature.
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }
    u64::from(low) | (u64::from(high) << 32)
}

unsafe fn write_msr(msr: u32, value: u64) {
    // SAFETY: The caller establishes support and preserves unrelated bits.
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_addresses_and_indexes_are_exact() {
        assert!(is_canonical(0));
        assert!(is_canonical(0x0000_7fff_ffff_ffff));
        assert!(!is_canonical(0x0000_8000_0000_0000));
        assert!(!is_canonical(0xffff_7fff_ffff_ffff));
        assert!(is_canonical(0xffff_8000_0000_0000));

        assert_eq!(
            indexes(0x0000_4001_2345_6789),
            page_indexes {
                pml4: 128,
                directory_pointer: 4,
                directory: 282,
                table: 86,
            }
        );
    }

    #[test]
    fn leaf_flags_reject_write_execute() {
        assert_eq!(
            leaf_flags(page_permissions::supervisor_read_write),
            Some(present | writable | no_execute)
        );
        assert_eq!(
            leaf_flags(page_permissions {
                user: true,
                writable: false,
                executable: true,
            }),
            Some(present | user_accessible)
        );
        assert_eq!(
            leaf_flags(page_permissions {
                user: false,
                writable: true,
                executable: true,
            }),
            None
        );
    }

    #[test]
    fn effective_permissions_reduce_every_level() {
        let parent = present | writable | user_accessible;
        let leaf = present | writable | user_accessible | no_execute;
        assert_eq!(
            effective_permissions_from_path(&[parent, parent, parent, leaf]),
            effective_permissions {
                present: true,
                user: true,
                writable: true,
                executable: false,
            }
        );

        let supervisor_parent = parent & !user_accessible;
        assert!(!effective_permissions_from_path(&[supervisor_parent, parent, parent, leaf,]).user);

        assert_eq!(
            effective_permissions_from_path(&[parent, 0, parent, leaf]),
            effective_permissions {
                present: false,
                user: false,
                writable: false,
                executable: false,
            }
        );
        assert_eq!(
            effective_permissions_from_path(&[]),
            effective_permissions {
                present: false,
                user: false,
                writable: false,
                executable: false,
            }
        );
    }

    #[test]
    fn entry_address_discards_only_flags() {
        let address = 0x0000_0001_2345_6000;
        assert_eq!(
            entry_address(address | present | writable | ownership_marker | no_execute),
            address
        );
    }
}
