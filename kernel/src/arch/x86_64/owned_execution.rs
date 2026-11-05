//! Feature-private aggregate owner for bounded scheduled, prepared low-root launches.
//! No fallible Drop: callers retain this aggregate on every error until finish.

#[cfg(not(feature = "application_profile_test"))]
use core::fmt::Write;
use core::{arch::asm, mem, ptr};

// The arena module carries the same gate these paths do, so the imports
// need no separate condition and nothing is left dangling in a default or
// non-arena strict lint build.
use super::arena_profile::user_stack_base;
#[cfg(any(feature = "native_runtime_test", feature = "mixed_workspace_test"))]
use super::arena_profile::{arena_mapping_plan, arena_profile};
pub(super) use super::arena_profile::{user_code_base, user_data_base, user_stack_top};
use super::descriptors;
use super::mapping_plan::{mapping_budget, mapping_plan, mapping_request, reserved_request};
use super::owned_mapping::{mapping_memory, staged_mapping};
use super::paging::{self, page_permissions, page_size};
use super::physical_access::{bootstrap_physical_access_group, proof_window_base};
use super::physical_window::planned_physical_window;
use super::prepared_launch::prepared_launch;
use super::resident_plan::{resident_plan, resident_request};
#[cfg(feature = "veil_application_test")]
use super::scheduler_test::user_return_fields_are_valid_within_stack;
use super::scheduler_test::{frame_address_matches_stack, scheduler_frame};
#[cfg(any(feature = "native_runtime_test", feature = "mixed_workspace_test"))]
use super::user_copy::validate_arena_copy_binding;
#[cfg(feature = "veil_application_test")]
use super::user_copy::validate_suspended_copy_binding;
#[cfg(any(feature = "application_profile_test", feature = "owned_source_test"))]
use super::user_copy::{copy_direction, user_copy_error, validate_copy_binding};
use super::user_entry_plan::user_entry_plan;
use crate::circle::circle_table;
#[cfg(not(feature = "application_profile_test"))]
use crate::console::serial_port;
use crate::identity::{fixture_principal_id, task_identity};
use crate::memory::physical::{frame_allocator, frame_lease, frame_purpose, physical_range};
#[cfg(feature = "veil_application_test")]
use crate::scheduler::scheduler_call_authorization;
use crate::scheduler::scheduler_table;
#[cfg(not(feature = "application_profile_test"))]
use crate::scheduler::{scheduler_context, scheduler_error};
use crate::task::{synthetic_origin_metadata, task_table};

#[cfg(feature = "linux_loader_entry_test")]
pub(super) mod loader_mapping_lifetime;
#[cfg(feature = "linux_loader_entry_test")]
mod loader_mapping_probe;

#[cfg(feature = "linux_loader_entry_test")]
pub(super) struct loader_mapping_context<'a> {
    pub tasks: &'a task_table,
    pub circles: &'a circle_table,
    pub scheduler: &'a scheduler_table,
    pub authorization: &'a scheduler_call_authorization,
    pub binding: anomalous_veil_protocol::source_read::source_binding,
}

#[cfg(feature = "linux_loader_entry_test")]
#[derive(Clone, Copy, Default)]
pub(super) struct loader_mapping_observation {
    pub population: super::runtime_source_mapping::population_observation,
    pub virtual_address: u64,
    pub byte_length: u64,
    pub virtual_pages: u64,
    pub table_fingerprint: u64,
    pub cleanup_retries: u64,
}

#[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
const transition_pages: usize = 4;

unsafe extern "C" {
    static kernel_image_start: u8;
    static kernel_text_start: u8;
    static kernel_text_end: u8;
    static kernel_rodata_start: u8;
    static kernel_rodata_end: u8;
    static kernel_data_start: u8;
    static kernel_data_end: u8;
    static kernel_bss_start: u8;
    static kernel_memory_end: u8;
}

macro_rules! linker_address {
    ($symbol:ident) => {{
        let address: u64;
        // SAFETY: Materialize the numeric marker, including adjacent symbols.
        unsafe { asm!("lea {out}, [rip + {symbol}]", out = out(reg) address,
            symbol = sym $symbol, options(nomem, nostack, preserves_flags)); }
        address
    }};
}

struct transition_stack {
    lease: frame_lease,
    bound: bool,
}

#[cfg(all(
    feature = "native_relaunch_test",
    not(feature = "linux_application_test")
))]
struct native_maintenance {
    service: task_identity,
    service_root: u64,
    service_stack: u64,
    editor: Option<task_identity>,
}

#[cfg_attr(feature = "linux_loader_entry_test", allow(dead_code))]
pub(super) type owned_execution = owned_execution_group<2>;

/// Bounded task-group ownership; every root, lease and transition stack
/// remains retained until explicit teardown. The legacy alias keeps two slots.
#[must_use = "retain every acquired resource through explicit finish, including errors"]
pub(super) struct owned_execution_group<const task_count: usize> {
    #[cfg(feature = "linux_loader_entry_test")]
    loader_mapping: Option<loader_mapping_lifetime::retained_mapping>,
    launches: [Option<prepared_launch>; task_count],
    stacks: [Option<transition_stack>; task_count],
    access: bootstrap_physical_access_group<task_count>,
    registered: [bool; task_count],
    activated: [bool; task_count],
    revalidated: [bool; task_count],
    bootstrap_stack: physical_range,
    original_rsp0: u64,
    prepared: bool,
    selecting: bool,
    finishing: bool,
    released: bool,
    #[cfg(all(
        feature = "native_relaunch_test",
        not(feature = "linux_application_test")
    ))]
    maintenance: Option<native_maintenance>,
}

impl<const task_count: usize> owned_execution_group<task_count> {
    pub(super) fn new() -> Self {
        Self {
            #[cfg(feature = "linux_loader_entry_test")]
            loader_mapping: None,
            launches: core::array::from_fn(|_| None),
            stacks: core::array::from_fn(|_| None),
            access: bootstrap_physical_access_group::new(),
            registered: [false; task_count],
            activated: [false; task_count],
            revalidated: [false; task_count],
            bootstrap_stack: physical_range::new(0, 0),
            original_rsp0: 0,
            prepared: false,
            selecting: false,
            finishing: false,
            released: false,
            #[cfg(all(
                feature = "native_relaunch_test",
                not(feature = "linux_application_test")
            ))]
            maintenance: None,
        }
    }

    /// # Safety
    /// Sole BSP, IF clear, NMI masked, exact reserved Threshold bootstrap tree.
    /// Payloads are immutable initialized linked rodata. Retain self on all errors.
    #[cfg_attr(feature = "native_application_test", allow(dead_code))]
    #[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
    pub(super) unsafe fn prepare(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        payloads: [(u64, u64); task_count],
    ) -> Result<(), &'static str> {
        // SAFETY: Preserve the original one-page profile and caller contract.
        unsafe { self.prepare_with_code_pages(allocator, circles, payloads, 1) }
    }

    /// Bounded extension for the native application fixture. All three user
    /// regions and retained teardown obligations are unchanged; only RX length
    /// grows within the same page-table branch.
    ///
    /// # Safety
    /// Same exclusive bootstrap and immutable-source contract as `prepare`.
    #[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
    pub(super) unsafe fn prepare_with_code_pages(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        payloads: [(u64, u64); task_count],
        code_pages: u64,
    ) -> Result<(), &'static str> {
        // SAFETY: The default profile has no initialized writable bytes.
        unsafe {
            self.prepare_segments(
                allocator,
                circles,
                payloads,
                [(0, 0); task_count],
                code_pages,
            )
        }
    }

    /// Populate immutable admitted source spans before committing any task.
    /// Initialized RW data never supplies task identity, roots, or service grants.
    ///
    /// # Safety
    /// Same exclusive bootstrap and immutable-source contract as `prepare`.
    #[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
    pub(super) unsafe fn prepare_segments(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        payloads: [(u64, u64); task_count],
        data: [(u64, u64); task_count],
        code_pages: u64,
    ) -> Result<(), &'static str> {
        // SAFETY: The wrapper forwards the same exclusive preparation contract.
        unsafe {
            self.prepare_segments_with_initial_stack(
                allocator,
                circles,
                payloads,
                data,
                code_pages,
                [user_stack_top; task_count],
            )
        }
    }

    /// Admit each initial stack pointer through the same guarded-stack bounds as
    /// the native top-of-stack entry. The stack remains zeroed until privileged
    /// initialization, and no task is published by preparation.
    ///
    /// # Safety
    /// Same exclusive bootstrap and immutable-source contract as `prepare`.
    // Keep transient mapping plans out of the suspended dispatcher frame.
    #[cfg_attr(feature = "elf_application_test", inline(never))]
    #[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
    pub(super) unsafe fn prepare_segments_with_initial_stack(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        payloads: [(u64, u64); task_count],
        data: [(u64, u64); task_count],
        code_pages: u64,
        initial_stacks: [u64; task_count],
    ) -> Result<(), &'static str> {
        // SAFETY: Explicitly retain the legacy three-region profile.
        unsafe {
            self.prepare_profile(
                allocator,
                circles,
                payloads,
                data,
                (code_pages, initial_stacks),
                #[cfg(feature = "native_runtime_test")]
                None,
            )
        }
    }

    /// Prepare two independently owned, guarded, zeroed native arenas. This
    /// profile is chosen by the controller, never by startup/artifact bytes.
    ///
    /// # Safety
    /// Same exclusive bootstrap and immutable-source contract as `prepare`.
    #[cfg(all(
        feature = "native_runtime_test",
        any(
            not(feature = "elf_application_test"),
            all(
                any(feature = "elf_document_test", feature = "native_relaunch_test"),
                not(feature = "linux_application_test")
            )
        )
    ))]
    // Unused in a program build, which prepares one task against the program
    // profile instead. Kept rather than gated: the two entry points differ by
    // their profile and task count, and this one stays the native proof's.
    #[cfg_attr(feature = "native_program_test", allow(dead_code))]
    pub(super) unsafe fn prepare_native_arenas(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        payloads: [(u64, u64); task_count],
        data: [(u64, u64); task_count],
        arena_bases: [u64; task_count],
    ) -> Result<(), &'static str> {
        if task_count != 2
            || arena_bases
                .iter()
                .any(|base| !matches!(*base, 0x804000 | 0x814000))
        {
            return Err("owned_arena_profile");
        }
        // SAFETY: Exact trusted four-region selection; no live-root mutation.
        unsafe {
            self.prepare_profile(
                allocator,
                circles,
                payloads,
                data,
                (8, [user_stack_top; task_count]),
                Some((arena_bases, arena_profile::native_application)),
            )
        }
    }

    /// Prepare the single task of a program-bearing launch (ADR 0094).
    ///
    /// A separate entry point rather than a widened `prepare_native_arenas`.
    /// The native application's two-task shape and its two fixed bases are a
    /// contract its callers rely on; a program launch is a different geometry
    /// rather than a relaxation of that one, so admitting it here cannot widen
    /// what the native path accepts.
    #[cfg(feature = "native_program_test")]
    pub(super) unsafe fn prepare_program_arena(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        payload: (u64, u64),
        arena_base: u64,
    ) -> Result<(), &'static str> {
        if task_count != 1 || !arena_profile::program.admits_base(arena_base) {
            return Err("owned_program_profile");
        }
        // SAFETY: Same contract as the native arena path -- the caller
        // excludes IRQ/NMI and retains every alias dependent. The profile is
        // selected here, by the controller, and never by launch bytes.
        unsafe {
            self.prepare_profile(
                allocator,
                circles,
                [payload; task_count],
                [(0, 0); task_count],
                (
                    arena_profile::program.code_pages(),
                    [user_stack_top; task_count],
                ),
                Some(([arena_base; task_count], arena_profile::program)),
            )
        }
    }

    #[cfg_attr(feature = "elf_application_test", inline(never))]
    #[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
    unsafe fn prepare_profile(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        payloads: [(u64, u64); task_count],
        data: [(u64, u64); task_count],
        entry_profile: (u64, [u64; task_count]),
        #[cfg(feature = "native_runtime_test")] arena: Option<([u64; task_count], arena_profile)>,
    ) -> Result<(), &'static str> {
        let (code_pages, initial_stacks) = entry_profile;
        // A program-bearing launch is a single task, and honestly so: ADR 0094
        // widens this gate rather than padding the launch with a peer that
        // exists only to satisfy a legacy count.
        //
        // The exception is bound to the controller-selected program profile,
        // not merely to the feature being compiled in. Gating on the feature
        // alone would also admit a single task through `prepare` and
        // `prepare_segments`, which pass no arena at all -- widening two
        // legacy paths as a side effect of enabling a third.
        #[cfg(feature = "native_program_test")]
        let single_task_program = matches!(arena, Some((_, arena_profile::program)));
        #[cfg(not(feature = "native_program_test"))]
        let single_task_program = false;
        if !(matches!(task_count, 2 | 3)
            || (cfg!(feature = "shared_veil_test") && task_count == 5)
            || (cfg!(feature = "native_program_test") && task_count == 1 && single_task_program))
            || !matches!(code_pages, 1 | 8)
        {
            return Err("owned_code_profile");
        }
        if self.access.alias_entry().is_some() || self.prepared || self.finishing || self.released {
            return Err("owned_prepare_state");
        }
        // SAFETY: The caller excludes IRQ/NMI and retains all alias dependents.
        unsafe { self.access.install() }.map_err(|_| "owned_alias_install")?;
        self.bootstrap_stack = self
            .access
            .bootstrap_stack()
            .ok_or("owned_bootstrap_stack")?;
        // SAFETY: Descriptors are initialized and exclusively owned by this BSP.
        self.original_rsp0 = unsafe { descriptors::selected_privilege_stack_top() };
        if !stack_contains_rsp(self.bootstrap_stack, 16 * 1024) || self.original_rsp0 == 0 {
            return Err("owned_bootstrap_state");
        }
        for (index, (source, length)) in payloads.into_iter().enumerate() {
            #[cfg(feature = "native_runtime_test")]
            let mapping = if let Some((bases, profile)) = arena {
                make_native_arena_stage(&self.access, self.bootstrap_stack, bases[index], profile)?
            } else {
                make_stage(&self.access, self.bootstrap_stack, code_pages)?
            };
            #[cfg(not(feature = "native_runtime_test"))]
            let mapping = make_stage(&self.access, self.bootstrap_stack, code_pages)?;
            if source < linker_address!(kernel_rodata_start)
                || length == 0
                || length > code_pages * page_size
                || source
                    .checked_add(length)
                    .is_none_or(|end| end > linker_address!(kernel_rodata_end))
            {
                return Err("owned_payload_extent");
            }
            let (data_source, data_length) = data[index];
            if data_length > 2 * page_size
                || (data_length == 0 && data_source != 0)
                || (data_length != 0
                    && (data_source < linker_address!(kernel_rodata_start)
                        || data_source
                            .checked_add(data_length)
                            .is_none_or(|end| end > linker_address!(kernel_rodata_end))))
            {
                return Err("owned_data_extent");
            }
            let stack_region = mapping.plan().regions().count() - 1;
            let admission = user_entry_plan::new(
                mapping.plan(),
                0,
                length,
                user_code_base,
                stack_region,
                initial_stacks[index],
            )
            .map_err(|_| "owned_entry_plan")?;
            #[cfg(feature = "native_runtime_test")]
            let launch = if let Some((bases, profile)) = arena {
                prepared_launch::new_with_native_arena(mapping, admission, bases[index], profile)
            } else {
                prepared_launch::new_with_code_pages(mapping, admission, code_pages)
            };
            #[cfg(not(feature = "native_runtime_test"))]
            let launch = prepared_launch::new_with_code_pages(mapping, admission, code_pages);
            self.launches[index] = Some(launch);
            let launch = self.launches[index]
                .as_mut()
                .ok_or("owned_launch_missing")?;
            // SAFETY: Retained before any allocation. Copy/readback uses only its
            // owned code lease through the independent checked physical alias.
            unsafe {
                launch.prepare(allocator, &mut self.access, circles, |mapping, access| {
                    populate(mapping, access, 0, source, length)
                        && populate(mapping, access, 1, data_source, data_length)
                })
            }
            .map_err(|_| "owned_launch_prepare")?;
            // SAFETY: Allocate exclusive RAM and clear/readback through our alias.
            let lease = unsafe {
                allocator.allocate_with(transition_pages, 1, frame_purpose::data, |range| {
                    self.access.clear_and_verify(range)
                })
            }
            .map_err(|_| "owned_stack_allocate")?;
            self.stacks[index] = Some(transition_stack {
                lease,
                bound: false,
            });
            let stack = self.stacks[index].as_mut().ok_or("owned_stack_missing")?;
            allocator
                .bind(&stack.lease)
                .map_err(|_| "owned_stack_bind")?;
            stack.bound = true;
            let frame = self.frame(index)?;
            let mut initial = scheduler_frame::initial(admission.bounds().entry());
            initial.stack_pointer = admission.bounds().initial_stack_pointer();
            // SAFETY: Exact aligned tail of this retained private 16KiB stack;
            // no scheduler consumer exists and the alias is supervisor-only NX.
            unsafe {
                ptr::write(frame, initial);
            }
        }
        self.prepared = true;
        Ok(())
    }

    pub(super) fn commit(
        &mut self,
        index: usize,
        tasks: &mut task_table,
        circles: &mut circle_table,
        principal: u64,
    ) -> Result<task_identity, &'static str> {
        if !self.prepared || self.finishing {
            return Err("owned_commit_state");
        }
        self.launches
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or("owned_launch_missing")?
            .commit(
                tasks,
                circles,
                fixture_principal_id::new(principal).ok_or("owned_principal")?,
                synthetic_origin_metadata::machine_role_fixture,
            )
            .map_err(|_| "owned_launch_commit")
    }

    /// # Safety
    /// All code, stacks, descriptors and bootstrap/alias mappings stay retained.
    /// Sole BSP and inactive private roots; retain self on partial registration.
    pub(super) unsafe fn activate(
        &mut self,
        allocator: &frame_allocator<'_>,
    ) -> Result<(), &'static str> {
        if !self.prepared
            || self.finishing
            || self.selecting
            || self.registered != [false; task_count]
            || paging::read_cr3() != self.access.bootstrap_root()
            || !stack_contains_rsp(self.bootstrap_stack, 16 * 1024)
        {
            return Err("owned_activation_state");
        }
        for index in 0..task_count {
            let launch = self.launches[index]
                .as_mut()
                .ok_or("owned_launch_missing")?;
            // SAFETY: Exclusive retained committed owner; the root is inactive.
            let mapping =
                unsafe { launch.committed_mapping_mut() }.map_err(|_| "owned_activation_commit")?;
            // SAFETY: Complete resident profile and alias were admitted above.
            unsafe { mapping.prepare_activation(allocator, &mut self.access) }
                .map_err(|_| "owned_activation_validate")?;
            self.activated[index] = true;
        }
        for index in 0..task_count {
            // SAFETY: All fully admitted mappings and the borrowed alias remain
            // owned until finish unregisters them on the restored bootstrap root.
            unsafe { self.access.register_root(self.root(index)?) }
                .map_err(|_| "owned_root_register")?;
            self.registered[index] = true;
        }
        self.selecting = true;
        Ok(())
    }

    pub(super) fn admission(&self, index: usize) -> Result<user_entry_plan, &'static str> {
        Ok(self
            .launches
            .get(index)
            .and_then(Option::as_ref)
            .ok_or("owned_launch_missing")?
            .admission())
    }
    pub(super) fn root(&self, index: usize) -> Result<u64, &'static str> {
        self.launches
            .get(index)
            .and_then(Option::as_ref)
            .ok_or("owned_launch_missing")?
            .mapping()
            .root_address()
            .ok_or("owned_root_missing")
    }
    fn stack_extent(&self, index: usize) -> Result<physical_range, &'static str> {
        let stack = self
            .stacks
            .get(index)
            .and_then(Option::as_ref)
            .ok_or("owned_stack_missing")?;
        if !stack.bound || !self.access.is_current() {
            return Err("owned_stack_unavailable");
        }
        let range = stack.lease.range().ok_or("owned_stack_range")?;
        let extent = planned_physical_window::new(
            physical_range::new(0, paging::identity_map_limit),
            proof_window_base,
        )
        .and_then(|window| window.plan_virtual_extent(range))
        .map_err(|_| "owned_stack_alias")?;
        Ok(physical_range::new(extent.base(), extent.length()))
    }
    pub(super) fn transition_stack_top(&self, index: usize) -> Result<u64, &'static str> {
        self.stack_extent(index)?.end().ok_or("owned_stack_top")
    }
    pub(super) fn frame(&self, index: usize) -> Result<*mut scheduler_frame, &'static str> {
        Ok(
            (self.transition_stack_top(index)? - mem::size_of::<scheduler_frame>() as u64)
                as *mut scheduler_frame,
        )
    }
    pub(super) fn frame_owned(&self, index: usize, frame: *const scheduler_frame) -> bool {
        self.stack_extent(index)
            .is_ok_and(|stack| frame_address_matches_stack(frame, stack))
    }
    fn word_address(&self, index: usize, region: usize, offset: u64) -> Option<u64> {
        if !offset.is_multiple_of(8) || self.finishing {
            return None;
        }
        let range = self
            .launches
            .get(index)?
            .as_ref()?
            .mapping()
            .data_range(region)?;
        if offset.checked_add(8)? > range.length {
            return None;
        }
        range.base.checked_add(offset)
    }
    #[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
    pub(super) fn read_data(&self, index: usize, offset: u64) -> Option<u64> {
        let address = self.word_address(index, 1, offset)?;
        // SAFETY: Complete aligned word inside this retained private data lease.
        unsafe { self.access.read_word(address) }
    }

    #[cfg(any(
        feature = "mixed_workspace_test",
        all(
            feature = "native_runtime_test",
            any(
                not(feature = "elf_application_test"),
                all(
                    any(feature = "elf_document_test", feature = "native_relaunch_test"),
                    not(feature = "linux_application_test")
                )
            )
        )
    ))]
    pub(super) fn arena_extent(&self, index: usize) -> Option<(u64, u64)> {
        if !self.prepared || self.finishing || self.released {
            return None;
        }
        self.launches.get(index)?.as_ref()?.arena_extent()
    }

    /// Controller observation only; no user identity or pointer grants access.
    #[cfg(any(
        feature = "mixed_workspace_test",
        all(
            feature = "native_runtime_test",
            any(
                not(feature = "elf_application_test"),
                all(
                    any(feature = "elf_document_test", feature = "native_relaunch_test"),
                    not(feature = "linux_application_test")
                )
            )
        )
    ))]
    pub(super) fn read_arena(&self, index: usize, offset: u64) -> Option<u64> {
        self.arena_extent(index)?;
        let address = self.word_address(index, 2, offset)?;
        // SAFETY: The whole aligned word belongs to this retained arena lease;
        // physical access independently verifies the live supervisor alias.
        unsafe { self.access.read_word(address) }
    }

    /// Whole-region observation for the shared-service isolation proof. This is
    /// privileged controller evidence, not a user-copy or cross-task capability.
    #[cfg(all(feature = "shared_veil_test", not(feature = "linux_tls_test")))]
    pub(super) fn proof_private_hashes(&self, index: usize) -> Option<[u64; 2]> {
        if !self.prepared || !self.selecting || self.finishing || self.released {
            return None;
        }
        let launch = self.launches.get(index)?.as_ref()?;
        let mapping = launch.mapping();
        let data = mapping.data_range(1)?;
        let stack = mapping.data_range(launch.stack_region())?;
        // SAFETY: Both complete private ranges belong to this retained owner.
        // The physical-access preflight checks IF-clear and the selected owned
        // alias; sole-BSP/NMI exclusion keeps mappings and storage live while the
        // nonescaping read hashes every word. No task can run inside this call.
        unsafe {
            Some([
                self.access.proof_hash_range(data)?,
                self.access.proof_hash_range(stack)?,
            ])
        }
    }

    #[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
    pub(super) fn write_data(
        &mut self,
        index: usize,
        offset: u64,
        value: u64,
    ) -> Result<(), &'static str> {
        let address = self
            .word_address(index, 1, offset)
            .ok_or("owned_data_extent")?;
        // SAFETY: Sole BSP, IF clear, no simultaneous user access to this lease.
        if unsafe { self.access.write_word(address, value) } {
            Ok(())
        } else {
            Err("owned_data_write")
        }
    }
    #[cfg_attr(feature = "linux_loader_entry_test", allow(dead_code))]
    pub(super) fn read_stack(&self, index: usize, offset: u64) -> Option<u64> {
        let region = self.launches.get(index)?.as_ref()?.stack_region();
        let address = self.word_address(index, region, offset)?;
        // SAFETY: Complete aligned word inside this retained private user stack.
        unsafe { self.access.read_word(address) }
    }

    /// Initialize a retained stack before its task is committed. Runtime user
    /// writes instead use the current-task-bound checked copy operation.
    #[cfg(feature = "linux_application_test")]
    #[cfg_attr(feature = "linux_loader_entry_test", allow(dead_code))]
    pub(super) fn write_stack(
        &mut self,
        index: usize,
        offset: u64,
        value: u64,
    ) -> Result<(), &'static str> {
        if !self.prepared
            || self.selecting
            || self
                .launches
                .get(index)
                .and_then(Option::as_ref)
                .is_none_or(|launch| launch.is_committed())
        {
            return Err("owned_stack_write_state");
        }
        let region = self.launches[index]
            .as_ref()
            .ok_or("owned_launch_missing")?
            .stack_region();
        let address = self
            .word_address(index, region, offset)
            .ok_or("owned_stack_extent")?;
        // SAFETY: Exact aligned word in this retained, unpublished user stack;
        // preparation is exclusive to the sole BSP with IRQ/NMI excluded.
        if unsafe { self.access.write_word(address, value) } {
            Ok(())
        } else {
            Err("owned_stack_write")
        }
    }

    #[cfg(any(feature = "application_profile_test", feature = "owned_source_test"))]
    fn copy_task_index(
        &self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        identity: task_identity,
    ) -> Result<usize, user_copy_error> {
        if !self.prepared || !self.selecting || self.finishing || self.released {
            return Err(user_copy_error::owner_unavailable);
        }
        let index = self
            .launches
            .iter()
            .position(|launch| {
                launch
                    .as_ref()
                    .is_some_and(|launch| launch.task() == Some(identity))
            })
            .ok_or(user_copy_error::task_binding)?;
        let launch = self.launches[index]
            .as_ref()
            .ok_or(user_copy_error::owner_unavailable)?;
        if !launch.is_committed()
            || !launch.mapping().is_ready()
            || !self.registered[index]
            || !self.activated[index]
        {
            return Err(user_copy_error::owner_unavailable);
        }
        let validate_binding = validate_copy_binding;
        #[cfg(any(feature = "native_runtime_test", feature = "mixed_workspace_test"))]
        let validate_binding = if launch.arena_extent().is_some() {
            validate_arena_copy_binding
        } else {
            validate_binding
        };
        #[cfg(feature = "owned_source_test")]
        let validate_binding = if launch.has_multi_region_profile() {
            super::user_copy::validate_multi_region_copy_binding
        } else {
            validate_binding
        };
        #[cfg(feature = "linux_loader_entry_test")]
        let validate_binding = if let Some(slot) = self
            .loader_mapping
            .as_ref()
            .filter(|slot| slot.belongs_to(identity))
        {
            if !launch.has_multi_region_profile()
                || !slot.attached_to(identity, index, self.root(index).unwrap_or(0))
                || !slot.resume_committed()
            {
                return Err(user_copy_error::owner_unavailable);
            }
            super::user_copy::validate_loader_mapping_copy_binding
        } else {
            validate_binding
        };
        validate_binding(
            identity,
            launch.admission().bounds(),
            tasks,
            circles,
            scheduler,
            identity,
        )?;
        if !self.access.is_current()
            || !self.current_stack_is_owned()
            || self.root(index).ok() != Some(paging::read_cr3())
            || self.transition_stack_top(index).ok()
                != Some(unsafe { descriptors::selected_privilege_stack_top() })
        {
            return Err(user_copy_error::owner_unavailable);
        }
        Ok(index)
    }

    /// Both retained owners determine composition; current task/circle counts
    /// cannot opt a caller into the published runtime subtree.
    #[cfg(any(feature = "application_profile_test", feature = "owned_source_test"))]
    unsafe fn preflight_current_mapping<'a>(
        launch: &'a prepared_launch,
        #[cfg(feature = "linux_loader_entry_test")] retained: Option<
            &'a loader_mapping_lifetime::retained_mapping,
        >,
        access: &mut bootstrap_physical_access_group<task_count>,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<super::owned_mapping::checked_copy_spans<'a>, user_copy_error> {
        let base = launch.mapping();
        #[cfg(feature = "linux_loader_entry_test")]
        if let Some(slot) = retained.filter(|slot| {
            launch
                .task()
                .is_some_and(|identity| slot.belongs_to(identity))
        }) {
            if !slot.circle_attached || !slot.resume_committed() {
                return Err(user_copy_error::owner_unavailable);
            }
            let graft = slot.graft().ok_or(user_copy_error::owner_unavailable)?;
            // SAFETY: Exact current-task/root admission preceded this call;
            // both owners and the committed graft remain borrowed by the spans.
            return unsafe {
                graft.preflight_user_copy(base, slot.mapping(), access, direction, address, length)
            };
        }
        // SAFETY: Exact current owner and its complete original tree remain live.
        unsafe { base.preflight_user_copy(access, direction, address, length) }
    }

    /// Copy an exact current task's bounded user range into trusted kernel RAM.
    /// Every returned error precedes the first destination byte. The retained
    /// sole-BSP activation interval excludes concurrent users and owner changes.
    #[cfg(any(feature = "application_profile_test", feature = "owned_source_test"))]
    pub(super) fn copy_from_user(
        &mut self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        identity: task_identity,
        user_address: u64,
        destination: &mut [u8],
    ) -> Result<(), user_copy_error> {
        let index = self.copy_task_index(tasks, circles, scheduler, identity)?;
        let launch = self.launches[index]
            .as_ref()
            .ok_or(user_copy_error::owner_unavailable)?;
        // SAFETY: Exact live task/root/stack ownership was checked above. All
        // leases and the independent alias remain retained under this borrow.
        let spans = unsafe {
            Self::preflight_current_mapping(
                launch,
                #[cfg(feature = "linux_loader_entry_test")]
                self.loader_mapping.as_ref(),
                &mut self.access,
                copy_direction::from_user,
                user_address,
                destination.len(),
            )
        }?;
        // SAFETY: Every physical span was checked against the retained user
        // mapping's actual path. Trusted kernel destination does not alias it.
        let access = unsafe {
            self.access
                .prepare_byte_spans(spans.ranges(), copy_direction::from_user)
        }
        .map_err(|_| user_copy_error::access_unavailable)?;
        access.read_into(destination);
        Ok(())
    }

    /// Validate a complete current-task output range without publishing bytes.
    /// The caller must retain the IF-clear ownership interval through I/O and
    /// copy_to_user; this check is not a transferable permission token.
    #[cfg(any(
        feature = "native_durable_test",
        feature = "native_editor_workspace_test",
        feature = "linux_loader_entry_test"
    ))]
    #[allow(dead_code)] // Combined-feature proofs may select a different runtime.
    pub(super) fn preflight_to_user(
        &mut self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        identity: task_identity,
        user_address: u64,
        length: usize,
    ) -> Result<(), user_copy_error> {
        let index = self.copy_task_index(tasks, circles, scheduler, identity)?;
        let launch = self.launches[index]
            .as_ref()
            .ok_or(user_copy_error::owner_unavailable)?;
        // SAFETY: Exact current root, task, mappings and alias are retained.
        let spans = unsafe {
            Self::preflight_current_mapping(
                launch,
                #[cfg(feature = "linux_loader_entry_test")]
                self.loader_mapping.as_ref(),
                &mut self.access,
                copy_direction::to_user,
                user_address,
                length,
            )
        }?;
        // SAFETY: Validate the same complete physical alias used by copy_to_user.
        let _access = unsafe {
            self.access
                .prepare_byte_spans(spans.ranges(), copy_direction::to_user)
        }
        .map_err(|_| user_copy_error::access_unavailable)?;
        Ok(())
    }

    /// Copy trusted kernel bytes only into the exact current task's writable
    /// user pages. Full permission/extent/alias preflight precedes every write.
    /// A privileged hardware fault is fatal, not a recoverable partial copy.
    #[cfg(feature = "application_profile_test")]
    #[cfg_attr(feature = "ny_object_test", allow(dead_code))]
    pub(super) fn copy_to_user(
        &mut self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        identity: task_identity,
        user_address: u64,
        source: &[u8],
    ) -> Result<(), user_copy_error> {
        let index = self.copy_task_index(tasks, circles, scheduler, identity)?;
        let launch = self.launches[index]
            .as_ref()
            .ok_or(user_copy_error::owner_unavailable)?;
        // SAFETY: The validated current task cannot execute concurrently, and
        // no allocator/table/root mutation can occur during the exclusive copy.
        let spans = unsafe {
            Self::preflight_current_mapping(
                launch,
                #[cfg(feature = "linux_loader_entry_test")]
                self.loader_mapping.as_ref(),
                &mut self.access,
                copy_direction::to_user,
                user_address,
                source.len(),
            )
        }?;
        // SAFETY: Every span is owned writable user RAM, separate from trusted
        // kernel source storage, with the alias retained through the commit.
        let access = unsafe {
            self.access
                .prepare_byte_spans(spans.ranges(), copy_direction::to_user)
        }
        .map_err(|_| user_copy_error::access_unavailable)?;
        access.write_from(source);
        Ok(())
    }

    #[cfg(feature = "veil_application_test")]
    fn suspended_caller_index(
        &self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        authorization: &scheduler_call_authorization,
    ) -> Result<(usize, u64), user_copy_error> {
        scheduler
            .validate_call_authorization(tasks, authorization)
            .map_err(|_| user_copy_error::task_binding)?;
        // Reuse the unchanged current-task path for the running veil. It checks
        // its exact retained generation, CR3, TSS.RSP0 and live privileged stack.
        let veil = self.copy_task_index(tasks, circles, scheduler, authorization.veil())?;
        let caller = self
            .launches
            .iter()
            .position(|launch| {
                launch
                    .as_ref()
                    .is_some_and(|launch| launch.task() == Some(authorization.caller()))
            })
            .ok_or(user_copy_error::task_binding)?;
        let launch = self.launches[caller]
            .as_ref()
            .ok_or(user_copy_error::owner_unavailable)?;
        let veil_launch = self.launches[veil]
            .as_ref()
            .ok_or(user_copy_error::owner_unavailable)?;
        if caller == veil
            || !launch.is_committed()
            || !launch.mapping().is_ready()
            || !self.registered[caller]
            || !self.activated[caller]
        {
            return Err(user_copy_error::owner_unavailable);
        }
        let validate_binding = validate_suspended_copy_binding;
        #[cfg(feature = "linux_loader_entry_test")]
        let validate_binding = if launch.has_multi_region_profile() {
            if self.loader_mapping.as_ref().is_some_and(|slot| {
                slot.attached_to(
                    authorization.caller(),
                    caller,
                    self.root(caller).unwrap_or(0),
                )
            }) {
                super::user_copy::validate_loader_mapping_suspended_copy_binding
            } else {
                super::user_copy::validate_loader_suspended_copy_binding
            }
        } else {
            validate_binding
        };
        validate_binding(
            (authorization.caller(), launch.admission().bounds()),
            (authorization.veil(), veil_launch.admission().bounds()),
            tasks,
            circles,
            scheduler,
            authorization,
        )?;
        let root = self
            .root(caller)
            .map_err(|_| user_copy_error::owner_unavailable)?;
        let current_root = self
            .root(veil)
            .map_err(|_| user_copy_error::owner_unavailable)?;
        if root == current_root || !self.access.accepts_root(root) {
            return Err(user_copy_error::owner_unavailable);
        }
        let frame = self
            .frame(caller)
            .map_err(|_| user_copy_error::owner_unavailable)?;
        if !self.frame_owned(caller, frame) {
            return Err(user_copy_error::owner_unavailable);
        }
        // SAFETY: The sealed authority proves the caller is blocked with saved
        // context. Its exact frame lies in our retained supervisor stack alias,
        // which the current veil cannot access or change. IF/NMI are excluded.
        let saved = unsafe { ptr::read(frame) };
        if !user_return_fields_are_valid_within_stack(saved, launch.admission().bounds())
            || !launch
                .admission()
                .accepts_return(saved.instruction_pointer, saved.stack_pointer)
        {
            return Err(user_copy_error::task_binding);
        }
        Ok((caller, current_root))
    }

    /// Authenticate a raw-call query without giving the veil a selectable task
    /// identity or any memory authority beyond its sealed suspended caller.
    #[cfg(feature = "veil_application_test")]
    pub(super) fn validate_suspended_caller(
        &self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        authorization: &scheduler_call_authorization,
    ) -> Result<(), user_copy_error> {
        self.suspended_caller_index(tasks, circles, scheduler, authorization)
            .map(|_| ())
    }

    /// Select composition from retained ownership, never an observed count.
    #[cfg(feature = "veil_application_test")]
    #[allow(clippy::too_many_arguments)]
    unsafe fn preflight_suspended_mapping<'a>(
        launch: &'a prepared_launch,
        #[cfg(feature = "linux_loader_entry_test")] retained: Option<
            &'a loader_mapping_lifetime::retained_mapping,
        >,
        access: &mut bootstrap_physical_access_group<task_count>,
        current_root: u64,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<super::owned_mapping::checked_copy_spans<'a>, user_copy_error> {
        let base = launch.mapping();
        #[cfg(feature = "linux_loader_entry_test")]
        if let Some(slot) = retained.filter(|slot| slot.circle_attached) {
            if launch.task().is_none_or(|id| !slot.belongs_to(id)) {
                return Err(user_copy_error::task_binding);
            }
            let graft = slot.graft().ok_or(user_copy_error::owner_unavailable)?;
            // SAFETY: Both actual owners and the publication receipt remain
            // borrowed by the returned spans until the alias copy completes.
            return unsafe {
                graft.preflight_inactive_user_copy(
                    base,
                    slot.mapping(),
                    access,
                    current_root,
                    direction,
                    address,
                    length,
                )
            };
        }
        // SAFETY: The authenticated caller and independent current root remain
        // retained; the complete ordinary translation path is still checked.
        unsafe {
            base.preflight_inactive_user_copy(access, current_root, direction, address, length)
        }
    }

    /// Validate a complete future write without touching a destination byte.
    /// This grants no durable copy authority: completion repeats every check
    /// while the scheduler's prepared reply still retains the blocked caller.
    #[cfg(feature = "veil_application_test")]
    #[cfg_attr(feature = "linux_loader_entry_test", allow(dead_code))]
    pub(super) fn preflight_to_suspended_caller(
        &mut self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        authorization: &scheduler_call_authorization,
        user_address: u64,
        length: usize,
    ) -> Result<(), user_copy_error> {
        let (caller, current_root) =
            self.suspended_caller_index(tasks, circles, scheduler, authorization)?;
        // SAFETY: Both roots, the current veil and blocked caller were validated
        // under the retained sole-BSP interval. No owner mutation can intervene.
        let spans = unsafe {
            Self::preflight_suspended_mapping(
                self.launches[caller]
                    .as_ref()
                    .ok_or(user_copy_error::owner_unavailable)?,
                #[cfg(feature = "linux_loader_entry_test")]
                self.loader_mapping.as_ref(),
                &mut self.access,
                current_root,
                copy_direction::to_user,
                user_address,
                length,
            )
        }?;
        // SAFETY: All spans are owned writable caller RAM. This only checks the
        // retained alias and discards the admission without reading or writing.
        unsafe {
            self.access
                .prepare_byte_spans(spans.ranges(), copy_direction::to_user)
        }
        .map_err(|_| user_copy_error::access_unavailable)?;
        Ok(())
    }

    /// Read only the blocked caller selected by a sealed live call authority.
    /// Every returned error precedes any trusted destination modification.
    #[cfg(feature = "veil_application_test")]
    #[cfg_attr(feature = "linux_loader_entry_test", allow(dead_code))]
    pub(super) fn copy_from_suspended_caller(
        &mut self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        authorization: &scheduler_call_authorization,
        user_address: u64,
        destination: &mut [u8],
    ) -> Result<(), user_copy_error> {
        let (caller, current_root) =
            self.suspended_caller_index(tasks, circles, scheduler, authorization)?;
        // SAFETY: Exact sealed task/frame/root bindings were checked above;
        // both roots and every lease remain retained throughout this borrow.
        let spans = unsafe {
            Self::preflight_suspended_mapping(
                self.launches[caller]
                    .as_ref()
                    .ok_or(user_copy_error::owner_unavailable)?,
                #[cfg(feature = "linux_loader_entry_test")]
                self.loader_mapping.as_ref(),
                &mut self.access,
                current_root,
                copy_direction::from_user,
                user_address,
                destination.len(),
            )
        }?;
        // SAFETY: The complete physical spans belong to the blocked caller and
        // the trusted destination cannot alias user storage.
        let access = unsafe {
            self.access
                .prepare_byte_spans(spans.ranges(), copy_direction::from_user)
        }
        .map_err(|_| user_copy_error::access_unavailable)?;
        access.read_into(destination);
        Ok(())
    }

    /// Write a fully preflighted range of the sealed blocked caller's writable
    /// RAM before an infallible scheduler reply commit. No user VA is touched.
    #[cfg(feature = "veil_application_test")]
    #[cfg_attr(feature = "linux_loader_entry_test", allow(dead_code))]
    pub(super) fn copy_to_suspended_caller(
        &mut self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        authorization: &scheduler_call_authorization,
        user_address: u64,
        source: &[u8],
    ) -> Result<(), user_copy_error> {
        let (caller, current_root) =
            self.suspended_caller_index(tasks, circles, scheduler, authorization)?;
        // SAFETY: Exact sealed task/frame/root bindings were checked above and
        // the caller remains blocked until the surrounding prepared reply ends.
        let spans = unsafe {
            Self::preflight_suspended_mapping(
                self.launches[caller]
                    .as_ref()
                    .ok_or(user_copy_error::owner_unavailable)?,
                #[cfg(feature = "linux_loader_entry_test")]
                self.loader_mapping.as_ref(),
                &mut self.access,
                current_root,
                copy_direction::to_user,
                user_address,
                source.len(),
            )
        }?;
        // SAFETY: Every byte is writable owned caller RAM, disjoint from the
        // trusted source, with both roots and the alias retained through commit.
        let access = unsafe {
            self.access
                .prepare_byte_spans(spans.ranges(), copy_direction::to_user)
        }
        .map_err(|_| user_copy_error::access_unavailable)?;
        access.write_from(source);
        Ok(())
    }

    /// Copy and independently read back actual caller RAM under one admission.
    /// All returned failures precede the first caller byte modification.
    #[cfg(feature = "linux_loader_entry_test")]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn copy_to_suspended_caller_observed(
        &mut self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        authorization: &scheduler_call_authorization,
        user_address: u64,
        source: &[u8],
        observed: &mut [u8],
    ) -> Result<(), user_copy_error> {
        if source.len() != observed.len() {
            return Err(user_copy_error::access_unavailable);
        }
        let (caller, current_root) =
            self.suspended_caller_index(tasks, circles, scheduler, authorization)?;
        // SAFETY: Exact sealed task/root/frame, sole BSP, IF/NMI excluded.
        let spans = unsafe {
            Self::preflight_suspended_mapping(
                self.launches[caller]
                    .as_ref()
                    .ok_or(user_copy_error::owner_unavailable)?,
                #[cfg(feature = "linux_loader_entry_test")]
                self.loader_mapping.as_ref(),
                &mut self.access,
                current_root,
                copy_direction::to_user,
                user_address,
                source.len(),
            )
        }?;
        // SAFETY: Complete writable owned caller RAM and disjoint trusted
        // source/observation. No mapping or alias can change before consumption.
        let access = unsafe {
            self.access
                .prepare_byte_spans(spans.ranges(), copy_direction::to_user)
        }
        .map_err(|_| user_copy_error::access_unavailable)?;
        access.write_from_observed(source, observed);
        Ok(())
    }

    /// # Safety
    /// IRQ gate or bootstrap caller owns execution, with IF clear and NMI masked.
    /// All retained stacks share an identical supervisor alias in every root.
    pub(super) unsafe fn select(&mut self, index: usize) -> Result<(), &'static str> {
        if index >= task_count
            || !self.selecting
            || self.finishing
            || !self.registered[index]
            || !self.access.is_current()
            || !self.current_stack_is_owned()
        {
            return Err("owned_select_state");
        }
        let root = self.root(index)?;
        let top = self.transition_stack_top(index)?;
        #[cfg(feature = "linux_loader_entry_test")]
        if let Some(slot) = self.loader_mapping.as_ref().filter(|slot| {
            slot.controls_selection(
                index,
                self.launches[index]
                    .as_ref()
                    .and_then(prepared_launch::task),
                root,
            )
        }) {
            let launch = self.launches[index]
                .as_ref()
                .ok_or("owned_runtime_caller_owner")?;
            let identity = launch.task().ok_or("owned_runtime_caller_identity")?;
            if !launch.is_committed()
                || !launch.mapping().is_ready()
                || !self.activated[index]
                || !slot.attached_to(identity, index, root)
                || !slot.resume_committed()
            {
                return Err("owned_runtime_caller_blocked");
            }
            let graft = slot.graft().ok_or("owned_runtime_caller_graft")?;
            // SAFETY: The single-use creation reply committed after allocator-
            // checked admission. All bindings remain retained in this aggregate;
            // neither owner can be cleaned up while selectable. This bounded
            // runtime selects the caller only from its independent veil root;
            // timer returns keep their current frame without selecting again.
            // A future scheduler that reselects the current caller needs its own
            // active-root admission. Keep this full check strictly inactive.
            if unsafe { graft.verify_selection(launch.mapping(), slot.mapping(), &mut self.access) }
                .map_err(|_| "owned_runtime_caller_tree")?
                != root
            {
                return Err("owned_runtime_caller_root");
            }
        }
        // SAFETY: Validated resident kernel/alias keeps the current handler stack
        // accessible across CR3. No interrupt can observe an intermediate RSP0.
        unsafe {
            paging::write_cr3(root);
            if !descriptors::select_privilege_stack_top(top) {
                return Err("owned_select_rsp0");
            }
        }
        if paging::read_cr3() != root || !self.access.is_current() {
            return Err("owned_select_root");
        }
        Ok(())
    }

    fn current_stack_is_owned(&self) -> bool {
        stack_contains_rsp(self.bootstrap_stack, 16 * 1024)
            || (0..task_count).any(|index| {
                self.stack_extent(index)
                    .is_ok_and(|stack| stack_contains_rsp(stack, 4096))
            })
    }

    /// Temporarily expose the retained boot framebuffer to this profile's
    /// integer-only presenter without extending a userspace root's mappings.
    /// This is not retirement: all owner, context and root registrations remain.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked, exact current task already captured into
    /// its eager CPU image. The callback must not enable interrupts, dispatch,
    /// copy userspace memory, alter page tables/TSS/CPU controls, or escape this
    /// scope except by fail-stop. Its code/data and current supervisor stack
    /// remain mapped by the shared resident region and retained high alias.
    /// Every returned `Err` requires immediate fail-stop: an invariant failure
    /// after the switch can retain the bootstrap root, so retry, task resume or
    /// owner cleanup is forbidden. Ordinary callback results restore the exact
    /// owner root before returning, including a callback result denoting failure.
    #[cfg(all(
        feature = "native_interactive_editor_test",
        not(feature = "native_durable_failure_test"),
        not(feature = "native_relaunch_test"),
        not(feature = "elf_application_test")
    ))]
    pub(super) unsafe fn with_interactive_bootstrap<T>(
        &mut self,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        identity: task_identity,
        callback: impl FnOnce() -> T,
    ) -> Result<T, &'static str> {
        if super::interrupts::interrupts_enabled() {
            return Err("interactive_present_interrupts");
        }
        let index = self
            .copy_task_index(tasks, circles, scheduler, identity)
            .map_err(|_| "interactive_present_owner")?;
        let root = self.root(index)?;
        let top = self.transition_stack_top(index)?;
        let stack = self.stack_extent(index)?;
        if !stack_contains_rsp(stack, 4096) {
            return Err("interactive_present_stack");
        }
        // SAFETY: Admission retains the same resident and high-alias mappings
        // in both roots. RSP0 is deliberately unchanged and IF stays clear.
        unsafe { paging::write_cr3(self.access.bootstrap_root()) };
        if paging::read_cr3() != self.access.bootstrap_root() || !self.access.is_current() {
            return Err("interactive_present_bootstrap");
        }
        let result = callback();
        if super::interrupts::interrupts_enabled()
            || paging::read_cr3() != self.access.bootstrap_root()
            || !self.access.is_current()
            || unsafe { descriptors::selected_privilege_stack_top() } != top
            || !stack_contains_rsp(stack, 4096)
        {
            return Err("interactive_present_callback");
        }
        // SAFETY: The unchanged admitted owner retains this exact root and
        // supervisor transition stack; no user context has run in this scope.
        unsafe { paging::write_cr3(root) };
        if self
            .copy_task_index(tasks, circles, scheduler, identity)
            .ok()
            != Some(index)
        {
            return Err("interactive_present_restore");
        }
        Ok(result)
    }

    /// # Safety
    /// All user consumers are terminal. Caller resumes the separate bootstrap
    /// continuation before finish can clear any transition-stack storage.
    pub(super) unsafe fn restore_bootstrap(&mut self) -> Result<(), &'static str> {
        if !self.selecting
            || self.finishing
            || !self.access.is_current()
            || !self.current_stack_is_owned()
        {
            return Err("owned_restore_state");
        }
        self.selecting = false;
        // SAFETY: Original bootstrap covers the live shared alias and resident
        // return path; all mappings remain owned until the nonlocal return.
        unsafe {
            paging::write_cr3(self.access.bootstrap_root());
            if !descriptors::select_privilege_stack_top(self.original_rsp0) {
                return Err("owned_restore_rsp0");
            }
        }
        if !self.access.is_current() {
            return Err("owned_restore_root");
        }
        Ok(())
    }

    /// # Safety
    /// Every scheduler context has been withdrawn. Run only after returning on
    /// the original bootstrap stack, with IF clear/NMI masked and no other CPU.
    /// Clear/release failures retain progress and permit retry. A failed mapping
    /// revalidation remains retained and fail-closed; it is not a recovery path.
    pub(super) unsafe fn finish(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        tasks: &mut task_table,
        circles: &mut circle_table,
    ) -> Result<(), &'static str> {
        // SAFETY: This is the same exclusive teardown with no injected failure.
        unsafe { self.finish_with_failure(allocator, tasks, circles, None) }
    }

    unsafe fn finish_with_failure(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        tasks: &mut task_table,
        circles: &mut circle_table,
        fail_stack: Option<usize>,
    ) -> Result<(), &'static str> {
        #[cfg(feature = "linux_loader_entry_test")]
        if self.loader_mapping.is_some() {
            return Err("owned_finish_runtime_mapping");
        }
        if self.released {
            return Ok(());
        }
        if self.selecting
            || !self.access.is_current()
            || paging::read_cr3() != self.access.bootstrap_root()
            || !stack_contains_rsp(self.bootstrap_stack, 16 * 1024)
            || tasks.attached_context_count() != 0
            || unsafe { descriptors::selected_privilege_stack_top() } != self.original_rsp0
        {
            return Err("owned_finish_state");
        }
        self.finishing = true;
        for index in 0..task_count {
            if self.registered[index] {
                // SAFETY: Bootstrap is active and selecting is irrevocably off.
                unsafe { self.access.unregister_root(self.root(index)?) }
                    .map_err(|_| "owned_root_unregister")?;
                self.registered[index] = false;
            }
        }
        for index in 0..task_count {
            if let Some(launch) = self.launches[index].as_mut() {
                if self.activated[index] && !self.revalidated[index] {
                    // SAFETY: Inactive and unregistered, after all contexts left.
                    let mapping = unsafe { launch.committed_mapping_mut() }
                        .map_err(|_| "owned_finish_commit")?;
                    unsafe { mapping.validate(allocator, &mut self.access) }
                        .map_err(|_| "owned_finish_validate")?;
                    self.revalidated[index] = true;
                }
                // SAFETY: No hardware/software consumer remains. Alias outlives
                // all releases; prepared_launch retains partial teardown state.
                unsafe { launch.cancel(allocator, &mut self.access, tasks, circles) }
                    .map_err(|_| "owned_launch_cancel")?;
            }
            if let Some(stack) = self.stacks[index].as_mut() {
                if stack.bound {
                    allocator
                        .unbind(&stack.lease)
                        .map_err(|_| "owned_stack_unbind")?;
                    stack.bound = false;
                }
                // SAFETY: Neither CPU, TSS nor scheduler refers to this stack.
                // Failure leaves the unbound lease retained for a later retry.
                unsafe {
                    allocator.release_with(&mut stack.lease, |range| {
                        if fail_stack == Some(index) {
                            false
                        } else {
                            self.access.clear_and_verify(range)
                        }
                    })
                }
                .map_err(|_| "owned_stack_clear")?;
                self.stacks[index] = None;
            }
        }
        // SAFETY: All private resources and root registrations are gone; the
        // current bootstrap stack and descriptor stack are outside this alias.
        unsafe { self.access.remove() }.map_err(|_| "owned_alias_remove")?;
        self.launches = core::array::from_fn(|_| None);
        self.access = bootstrap_physical_access_group::new();
        self.bootstrap_stack = physical_range::new(0, 0);
        self.original_rsp0 = 0;
        self.activated = [false; task_count];
        self.revalidated = [false; task_count];
        self.prepared = false;
        self.released = true;
        Ok(())
    }
}

/// A distinct, controller-selected single workspace owner. The existing
/// application preparation paths retain their original geometry and counts.
#[cfg(feature = "owned_workspace_test")]
impl owned_execution_group<1> {
    /// Prepare the low-linked workspace without publishing a task. The linked
    /// data starts after the zeroed one-page mailbox, at user_data_base + 4096.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked, exact reserved Threshold bootstrap tree.
    /// Both sources are immutable initialized kernel rodata. Retain this owner
    /// on every error until explicit finish succeeds; no callback may escape.
    pub(super) unsafe fn prepare_workspace(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        payload: (u64, u64),
        linked_data: (u64, u64),
    ) -> Result<(), &'static str> {
        if self.access.alias_entry().is_some()
            || self.prepared
            || self.finishing
            || self.released
            || super::interrupts::interrupts_enabled()
        {
            return Err("workspace_owned_prepare_state");
        }
        let ro_start = linker_address!(kernel_rodata_start);
        let ro_end = linker_address!(kernel_rodata_end);
        let (source, length) = payload;
        let (data_source, data_length) = linked_data;
        if source < ro_start
            || length == 0
            || length > 32 * page_size
            || source.checked_add(length).is_none_or(|end| end > ro_end)
            || data_length > 7 * page_size
            || (data_length == 0 && data_source != 0)
            || (data_length != 0
                && (data_source < ro_start
                    || data_source
                        .checked_add(data_length)
                        .is_none_or(|end| end > ro_end)))
        {
            return Err("workspace_owned_payload_extent");
        }
        // SAFETY: The exclusive bootstrap contract retains any partial install.
        unsafe { self.access.install() }.map_err(|_| "workspace_owned_alias_install")?;
        self.bootstrap_stack = self
            .access
            .bootstrap_stack()
            .ok_or("owned_bootstrap_stack")?;
        // SAFETY: Exclusively owned initialized descriptors on this BSP.
        self.original_rsp0 = unsafe { descriptors::selected_privilege_stack_top() };
        if !stack_contains_rsp(self.bootstrap_stack, 16 * 1024) || self.original_rsp0 == 0 {
            return Err("owned_bootstrap_state");
        }
        let mapping = make_workspace_stage(&self.access, self.bootstrap_stack)?;
        let admission =
            user_entry_plan::new(mapping.plan(), 0, length, user_code_base, 2, user_stack_top)
                .map_err(|_| "workspace_owned_entry_plan")?;
        self.launches[0] = Some(prepared_launch::new_workspace(mapping, admission));
        // SAFETY: Retained before the first allocation. Both callbacks populate
        // only owned data leases, with full-region readback and no live root.
        unsafe {
            self.launches[0]
                .as_mut()
                .ok_or("owned_launch_missing")?
                .prepare(allocator, &mut self.access, circles, |mapping, access| {
                    populate(mapping, access, 0, source, length)
                        && populate_workspace_data(mapping, access, data_source, data_length)
                })
        }
        .map_err(|_| "workspace_owned_launch_prepare")?;
        // SAFETY: Exclusive RAM, cleared and independently read back.
        let lease = unsafe {
            allocator.allocate_with(transition_pages, 1, frame_purpose::data, |range| {
                self.access.clear_and_verify(range)
            })
        }
        .map_err(|_| "owned_stack_allocate")?;
        self.stacks[0] = Some(transition_stack {
            lease,
            bound: false,
        });
        let stack = self.stacks[0].as_mut().ok_or("owned_stack_missing")?;
        allocator
            .bind(&stack.lease)
            .map_err(|_| "owned_stack_bind")?;
        stack.bound = true;
        let frame = self.frame(0)?;
        let mut initial = scheduler_frame::initial(admission.bounds().entry());
        initial.stack_pointer = admission.bounds().initial_stack_pointer();
        // SAFETY: Exact aligned frame tail of the retained private 16KiB stack.
        unsafe { ptr::write(frame, initial) };
        self.prepared = true;
        Ok(())
    }

    /// The sole retained mailbox's supervisor alias, not an arbitrary user
    /// pointer. The caller must never retain or use it beyond this owner's
    /// lifetime, and must separately validate its current task before access.
    pub(super) fn workspace_mailbox_alias(
        &self,
        index: usize,
    ) -> Result<physical_range, &'static str> {
        if index != 0
            || !self.prepared
            || self.finishing
            || self.released
            || !self.access.is_current()
        {
            return Err("workspace_owned_mailbox_state");
        }
        let launch = self.launches[0].as_ref().ok_or("owned_launch_missing")?;
        if !launch.is_workspace_profile() || !launch.mapping().is_ready() {
            return Err("workspace_owned_mailbox_profile");
        }
        let data = launch
            .mapping()
            .data_range(1)
            .ok_or("workspace_owned_mailbox_range")?;
        if data.length != 8 * page_size {
            return Err("workspace_owned_mailbox_range");
        }
        let extent = planned_physical_window::new(
            physical_range::new(0, paging::identity_map_limit),
            proof_window_base,
        )
        .and_then(|window| window.plan_virtual_extent(physical_range::new(data.base, page_size)))
        .map_err(|_| "workspace_owned_mailbox_alias")?;
        Ok(physical_range::new(extent.base(), extent.length()))
    }

    fn workspace_current(&self, index: usize) -> bool {
        index == 0
            && self.prepared
            && self.selecting
            && !self.finishing
            && !self.released
            && self.registered[0]
            && self.activated[0]
            && self.access.is_current()
            && self.launches[0].as_ref().is_some_and(|launch| {
                launch.is_workspace_profile()
                    && launch.is_committed()
                    && launch.mapping().is_ready()
            })
            && self.root(0).ok() == Some(paging::read_cr3())
            && self.transition_stack_top(0).ok()
                == Some(unsafe { descriptors::selected_privilege_stack_top() })
            && self
                .stack_extent(0)
                .is_ok_and(|stack| stack_contains_rsp(stack, 4096))
            && !super::interrupts::interrupts_enabled()
    }

    /// Temporarily expose boot-only MMIO without mapping it into the workspace
    /// root. This preserves the retained owner and its exact privilege stack.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked; caller has validated the sole unscheduled
    /// running workspace task and captured its GPR-only/TS fault profile. The
    /// callback is integer-only, cannot enable IRQ, dispatch, copy user memory,
    /// alter paging/TSS/CPU controls, or escape except by fail-stop. Any Err is
    /// fail-stop with ownership retained: never resume, retry, or clean up.
    pub(super) unsafe fn with_workspace_bootstrap<T>(
        &mut self,
        index: usize,
        callback: impl FnOnce() -> T,
    ) -> Result<T, &'static str> {
        if !self.workspace_current(index) {
            return Err("workspace_present_owner");
        }
        let root = self.root(index)?;
        let top = self.transition_stack_top(index)?;
        let stack = self.stack_extent(index)?;
        // SAFETY: Both roots retain the resident return path and high alias;
        // the callback never schedules and RSP0 remains unchanged.
        unsafe { paging::write_cr3(self.access.bootstrap_root()) };
        if paging::read_cr3() != self.access.bootstrap_root() || !self.access.is_current() {
            return Err("workspace_present_bootstrap");
        }
        let result = callback();
        if super::interrupts::interrupts_enabled()
            || paging::read_cr3() != self.access.bootstrap_root()
            || !self.access.is_current()
            || unsafe { descriptors::selected_privilege_stack_top() } != top
            || !stack_contains_rsp(stack, 4096)
        {
            return Err("workspace_present_callback");
        }
        // SAFETY: Exact admitted root, unchanged retained alias and task frame.
        unsafe { paging::write_cr3(root) };
        if !self.workspace_current(index) {
            return Err("workspace_present_restore");
        }
        Ok(result)
    }
}

#[cfg(feature = "owned_workspace_test")]
fn make_workspace_stage(
    access: &bootstrap_physical_access_group<1>,
    stack: physical_range,
) -> Result<staged_mapping, &'static str> {
    let stack_base = user_stack_top - 16 * page_size;
    let requests = [
        (user_code_base, 32, false, true),
        (user_data_base, 8, true, false),
        (stack_base, 16, true, false),
    ]
    .map(
        |(virtual_address, page_count, writable, executable)| mapping_request {
            virtual_address,
            page_count,
            permissions: page_permissions {
                user: true,
                writable,
                executable,
            },
        },
    );
    let reserved = [
        (0, 512),
        (user_code_base - page_size, 1),
        (user_code_base + 32 * page_size, 1),
        (user_data_base - page_size, 1),
        (user_data_base + 8 * page_size, 1),
        (stack_base - page_size, 1),
        (user_stack_top, 1),
    ]
    .map(|(virtual_address, page_count)| reserved_request {
        virtual_address,
        page_count,
    });
    let plan = mapping_plan::new(
        &requests,
        &reserved,
        mapping_budget {
            data_pages: 56,
            table_frames: 8,
        },
    )
    .map_err(|_| "workspace_owned_mapping_plan")?;
    make_resident_stage(
        access,
        stack,
        plan,
        #[cfg(feature = "owned_multi_region_test")]
        false,
    )
}

#[cfg(feature = "owned_workspace_test")]
fn populate_workspace_data(
    mapping: &staged_mapping,
    access: &mut bootstrap_physical_access_group<1>,
    source: u64,
    length: u64,
) -> bool {
    let Some(data) = mapping.data_range(1) else {
        return false;
    };
    if data.length != 8 * page_size || length > 7 * page_size {
        return false;
    }
    for offset in (0..data.length).step_by(8) {
        let mut bytes = [0; 8];
        for (index, byte) in bytes.iter_mut().enumerate() {
            if let Some(input) = (offset + index as u64)
                .checked_sub(page_size)
                .filter(|at| *at < length)
            {
                // SAFETY: Immutable linked rodata validated before allocation.
                *byte = unsafe { ptr::read_volatile((source + input) as *const u8) };
            }
        }
        let word = u64::from_le_bytes(bytes);
        // SAFETY: Whole word of inactive exclusively owned data; all mailbox,
        // BSS and page-slack bytes are explicitly zeroed and verified as well.
        if !unsafe { access.write_word(data.base + offset, word) }
            || unsafe { access.read_word(data.base + offset) } != Some(word)
        {
            return false;
        }
    }
    true
}

// Feature-selected immutable image input; the qualified flat-native method
// retains its exact argument geometry when ELF comparison is not selected.
#[cfg(all(
    feature = "native_relaunch_test",
    not(feature = "linux_application_test"),
    not(feature = "elf_relaunch_test")
))]
type native_editor_image = ((u64, u64), u64);
#[cfg(all(feature = "elf_relaunch_test", not(feature = "linux_application_test")))]
type native_editor_image = ((u64, u64), (u64, u64), u64);

#[cfg(all(
    feature = "native_relaunch_test",
    not(feature = "linux_application_test")
))]
impl owned_execution_group<2> {
    fn native_service_matches(
        &self,
        tasks: &task_table,
        scheduler: &scheduler_table,
        service: task_identity,
    ) -> bool {
        self.launches[1].as_ref().and_then(prepared_launch::task) == Some(service)
            && self.registered[1]
            && self.activated[1]
            && tasks.snapshot(service).is_ok_and(|s| {
                s.state() == crate::task::task_state::running && s.context_attached()
            })
            && scheduler
                .snapshot(tasks)
                .is_ok_and(|s| s.current() == Some(service) && s.reservation().is_none())
    }

    /// Preserve the running service's logical gate invocation while moving to
    /// the bootstrap root. The caller must subsequently abandon this handler
    /// stack through the separate integer-only maintenance continuation.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked; service CPU state has been captured, all
    /// editor routes closed, and the exact terminal editor can never resume.
    pub(super) unsafe fn native_maintenance_bootstrap(
        &mut self,
        tasks: &task_table,
        scheduler: &scheduler_table,
        editor: task_identity,
        service: task_identity,
    ) -> Result<(), &'static str> {
        if self.maintenance.is_some()
            || !self.prepared
            || !self.selecting
            || self.finishing
            || self.released
            || !self.access.is_current()
            || !self.current_stack_is_owned()
            || !self.native_service_matches(tasks, scheduler, service)
            || self.launches[0].as_ref().and_then(prepared_launch::task) != Some(editor)
            || !tasks
                .snapshot(editor)
                .is_ok_and(|s| s.state().is_terminal())
            || !scheduler.snapshot(tasks).is_ok_and(|s| s.runnable() == 0)
        {
            return Err("native_maintenance_admission");
        }
        let service_root = self.root(1)?;
        let service_stack = self.transition_stack_top(1)?;
        if paging::read_cr3() != service_root
            || unsafe { descriptors::selected_privilege_stack_top() } != service_stack
        {
            return Err("native_maintenance_current");
        }
        self.maintenance = Some(native_maintenance {
            service,
            service_root,
            service_stack,
            editor: Some(editor),
        });
        self.selecting = false;
        // SAFETY: The independent bootstrap root retains the same live alias;
        // the old handler stack remains owned until after the nonlocal return.
        unsafe {
            paging::write_cr3(self.access.bootstrap_root());
            if !descriptors::select_privilege_stack_top(self.original_rsp0) {
                return Err("native_maintenance_rsp0");
            }
        }
        if !self.access.is_current() {
            return Err("native_maintenance_root");
        }
        Ok(())
    }

    fn native_maintenance_valid(&self, tasks: &task_table, scheduler: &scheduler_table) -> bool {
        let Some(state) = &self.maintenance else {
            return false;
        };
        !self.selecting
            && !self.finishing
            && !self.released
            && self.access.is_current()
            && paging::read_cr3() == self.access.bootstrap_root()
            && stack_contains_rsp(self.bootstrap_stack, 16 * 1024)
            && unsafe { descriptors::selected_privilege_stack_top() } == self.original_rsp0
            && self.native_service_matches(tasks, scheduler, state.service)
            && self.root(1).ok() == Some(state.service_root)
            && self.transition_stack_top(1).ok() == Some(state.service_stack)
    }

    /// Release only slot zero. Every successful step is recorded before the
    /// next fallible clear; a retained stack-clear failure may be retried.
    /// `expected` is the exact retained identity, or None for a failed prepare.
    ///
    /// # Safety
    /// On the original bootstrap stack, IF clear/NMI masked. The editor's
    /// scheduler context is already withdrawn; service ownership stays live.
    pub(super) unsafe fn native_release_editor(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        tasks: &mut task_table,
        circles: &mut circle_table,
        scheduler: &scheduler_table,
        expected: Option<task_identity>,
        fail_stack_clear: bool,
    ) -> Result<(), &'static str> {
        if !self.native_maintenance_valid(tasks, scheduler)
            || self.maintenance.as_ref().and_then(|state| state.editor) != expected
        {
            return Err("native_editor_release_state");
        }
        let launch = self.launches[0]
            .as_ref()
            .ok_or("native_editor_release_missing")?;
        if let Some(identity) = launch.task() {
            if Some(identity) != expected
                || !tasks.snapshot(identity).is_ok_and(|s| {
                    !s.context_attached()
                        && (s.state().is_terminal() || s.state() == crate::task::task_state::ready)
                })
            {
                return Err("native_editor_release_context");
            }
        } else if expected.is_some() && !launch.is_released() {
            return Err("native_editor_release_identity");
        }
        if self.registered[0] {
            // SAFETY: Bootstrap is selected; the exact editor is unresumable.
            unsafe { self.access.unregister_root(self.root(0)?) }
                .map_err(|_| "native_editor_unregister")?;
            self.registered[0] = false;
        }
        let launch = self.launches[0]
            .as_mut()
            .ok_or("native_editor_release_missing")?;
        if self.activated[0] && !self.revalidated[0] {
            let mapping = unsafe { launch.committed_mapping_mut() }
                .map_err(|_| "native_editor_release_commit")?;
            unsafe { mapping.validate(allocator, &mut self.access) }
                .map_err(|_| "native_editor_release_validate")?;
            self.revalidated[0] = true;
        }
        // SAFETY: Only this editor's mappings/attachments are passed to cancel.
        // It retains partial clear/release progress and rejects live contexts.
        unsafe { launch.cancel(allocator, &mut self.access, tasks, circles) }
            .map_err(|_| "native_editor_release_launch")?;
        if let Some(stack) = self.stacks[0].as_mut() {
            if stack.bound {
                allocator
                    .unbind(&stack.lease)
                    .map_err(|_| "native_editor_stack_unbind")?;
                stack.bound = false;
            }
            unsafe {
                allocator.release_with(&mut stack.lease, |range| {
                    !fail_stack_clear && self.access.clear_and_verify(range)
                })
            }
            .map_err(|_| "native_editor_stack_clear")?;
            self.stacks[0] = None;
        }
        self.launches[0] = None;
        self.activated[0] = false;
        self.revalidated[0] = false;
        self.maintenance
            .as_mut()
            .ok_or("native_maintenance_missing")?
            .editor = None;
        Ok(())
    }

    /// Construct a fresh four-region editor without publishing a task. A false
    /// population probe deliberately leaves the partial owner for cancellation.
    ///
    /// # Safety
    /// Same immutable-source and exclusive bootstrap contracts as preparation;
    /// the retained service cannot execute until maintenance explicitly ends.
    #[inline(never)]
    pub(super) unsafe fn native_prepare_editor(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        tasks: &task_table,
        scheduler: &scheduler_table,
        image: native_editor_image,
        fail_population: bool,
    ) -> Result<(), &'static str> {
        if !self.native_maintenance_valid(tasks, scheduler)
            || self.launches[0].is_some()
            || self.stacks[0].is_some()
            || self.registered[0]
            || self
                .maintenance
                .as_ref()
                .and_then(|state| state.editor)
                .is_some()
        {
            return Err("native_editor_prepare_state");
        }
        #[cfg(not(feature = "elf_relaunch_test"))]
        let ((source, length), arena_base) = image;
        #[cfg(feature = "elf_relaunch_test")]
        let ((source, length), data, arena_base) = image;
        if source < linker_address!(kernel_rodata_start)
            || length == 0
            || length > 8 * page_size
            || source
                .checked_add(length)
                .is_none_or(|end| end > linker_address!(kernel_rodata_end))
        {
            return Err("native_editor_payload_extent");
        }
        #[cfg(feature = "elf_relaunch_test")]
        if data.1 == 0
            || data.1 > 2 * page_size
            || data.0 < linker_address!(kernel_rodata_start)
            || data
                .0
                .checked_add(data.1)
                .is_none_or(|end| end > linker_address!(kernel_rodata_end))
        {
            return Err("native_editor_data_extent");
        }
        let mapping = make_native_arena_stage(
            &self.access,
            self.bootstrap_stack,
            arena_base,
            arena_profile::native_application,
        )?;
        let admission =
            user_entry_plan::new(mapping.plan(), 0, length, user_code_base, 3, user_stack_top)
                .map_err(|_| "native_editor_entry_plan")?;
        self.launches[0] = Some(prepared_launch::new_with_native_arena(
            mapping,
            admission,
            arena_base,
            arena_profile::native_application,
        ));
        let launch = self.launches[0]
            .as_mut()
            .ok_or("native_editor_prepare_missing")?;
        unsafe {
            launch.prepare(allocator, &mut self.access, circles, |mapping, access| {
                #[cfg(not(feature = "elf_relaunch_test"))]
                let data = (0, 0);
                // Populate verifies every initialized byte and the whole zero
                // tail before the intentional failure can retain this owner.
                populate(mapping, access, 0, source, length)
                    && populate(mapping, access, 1, data.0, data.1)
                    && !fail_population
            })
        }
        .map_err(|_| "native_editor_prepare_population")?;
        let lease = unsafe {
            allocator.allocate_with(transition_pages, 1, frame_purpose::data, |range| {
                self.access.clear_and_verify(range)
            })
        }
        .map_err(|_| "native_editor_stack_allocate")?;
        self.stacks[0] = Some(transition_stack {
            lease,
            bound: false,
        });
        let stack = self.stacks[0]
            .as_mut()
            .ok_or("native_editor_stack_missing")?;
        allocator
            .bind(&stack.lease)
            .map_err(|_| "native_editor_stack_bind")?;
        stack.bound = true;
        let mut initial = scheduler_frame::initial(admission.bounds().entry());
        initial.stack_pointer = admission.bounds().initial_stack_pointer();
        unsafe { ptr::write(self.frame(0)?, initial) };
        Ok(())
    }

    pub(super) fn native_commit_editor(
        &mut self,
        tasks: &mut task_table,
        circles: &mut circle_table,
        scheduler: &scheduler_table,
        principal: u64,
    ) -> Result<task_identity, &'static str> {
        if !self.native_maintenance_valid(tasks, scheduler)
            || self
                .maintenance
                .as_ref()
                .and_then(|state| state.editor)
                .is_some()
        {
            return Err("native_editor_commit_state");
        }
        let identity = self.commit(0, tasks, circles, principal)?;
        // No fallible operation follows task publication before retaining it.
        self.maintenance
            .as_mut()
            .expect("validated native maintenance")
            .editor = Some(identity);
        Ok(identity)
    }

    /// # Safety
    /// The fresh committed editor remains unpublished to the scheduler; its
    /// immutable mappings and transition stack stay owned through final entry.
    pub(super) unsafe fn native_activate_editor(
        &mut self,
        allocator: &frame_allocator<'_>,
        tasks: &task_table,
        scheduler: &scheduler_table,
        identity: task_identity,
    ) -> Result<(), &'static str> {
        if !self.native_maintenance_valid(tasks, scheduler)
            || self.maintenance.as_ref().and_then(|state| state.editor) != Some(identity)
            || self.registered[0]
            || !tasks
                .snapshot(identity)
                .is_ok_and(|s| s.state() == crate::task::task_state::ready && !s.context_attached())
        {
            return Err("native_editor_activate_state");
        }
        let launch = self.launches[0]
            .as_mut()
            .ok_or("native_editor_activate_missing")?;
        let mapping = unsafe { launch.committed_mapping_mut() }
            .map_err(|_| "native_editor_activate_commit")?;
        unsafe { mapping.prepare_activation(allocator, &mut self.access) }
            .map_err(|_| "native_editor_activate_validate")?;
        self.activated[0] = true;
        unsafe { self.access.register_root(self.root(0)?) }
            .map_err(|_| "native_editor_register")?;
        self.registered[0] = true;
        Ok(())
    }

    /// End the unpublished interval only after both exact contexts are admitted.
    /// The caller next selects the unchanged service and restores its saved CPU.
    pub(super) fn native_maintenance_resume(
        &mut self,
        tasks: &task_table,
        scheduler: &scheduler_table,
        editor: task_identity,
    ) -> Result<(), &'static str> {
        if !self.native_maintenance_valid(tasks, scheduler)
            || self.maintenance.as_ref().and_then(|state| state.editor) != Some(editor)
            || !self.registered[0]
            || !self.activated[0]
            || self.launches[0].as_ref().and_then(prepared_launch::task) != Some(editor)
            || !tasks
                .snapshot(editor)
                .is_ok_and(|s| s.state() == crate::task::task_state::ready && s.context_attached())
            || !scheduler
                .snapshot(tasks)
                .is_ok_and(|s| s.admitted() == 2 && s.runnable() == 1)
            || scheduler.context(tasks, editor).ok()
                != Some(crate::scheduler::scheduler_context::initial)
        {
            return Err("native_maintenance_resume_state");
        }
        self.maintenance = None;
        self.selecting = true;
        Ok(())
    }

    /// Full RW, arena, user-stack and transition-stack observation while the
    /// service is quiescent on the independent bootstrap continuation.
    pub(super) fn native_service_hashes(
        &self,
        tasks: &task_table,
        scheduler: &scheduler_table,
    ) -> Option<[u64; 4]> {
        if !self.native_maintenance_valid(tasks, scheduler) {
            return None;
        }
        let launch = self.launches[1].as_ref()?;
        let stack = self.stacks[1].as_ref()?.lease.range()?;
        unsafe {
            Some([
                self.access
                    .proof_hash_range(launch.mapping().data_range(1)?)?,
                self.access
                    .proof_hash_range(launch.mapping().data_range(2)?)?,
                self.access
                    .proof_hash_range(launch.mapping().data_range(launch.stack_region())?)?,
                self.access.proof_hash_range(stack)?,
            ])
        }
    }
}

fn stack_contains_rsp(stack: physical_range, headroom: u64) -> bool {
    let rsp: u64;
    // SAFETY: Observe the live stack only; all operands remain numeric.
    unsafe {
        asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags));
    }
    stack
        .base
        .checked_add(headroom)
        .is_some_and(|start| rsp >= start)
        && stack.end().is_some_and(|end| rsp < end)
}

#[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
fn make_stage<const task_count: usize>(
    access: &bootstrap_physical_access_group<task_count>,
    stack: physical_range,
    code_pages: u64,
) -> Result<staged_mapping, &'static str> {
    let requests = [
        (user_code_base, code_pages, false, true),
        (user_data_base, 2, true, false),
        (user_stack_base, 2, true, false),
    ]
    .map(
        |(virtual_address, page_count, writable, executable)| mapping_request {
            virtual_address,
            page_count,
            permissions: page_permissions {
                user: true,
                writable,
                executable,
            },
        },
    );
    let reserved = [
        (0, 512),
        (user_stack_base - page_size, 1),
        (user_stack_top, 1),
    ]
    .map(|(virtual_address, page_count)| reserved_request {
        virtual_address,
        page_count,
    });
    let plan = mapping_plan::new(
        &requests,
        &reserved,
        mapping_budget {
            data_pages: code_pages + 4,
            table_frames: 8,
        },
    )
    .map_err(|_| "owned_mapping_plan")?;
    make_resident_stage(
        access,
        stack,
        plan,
        #[cfg(feature = "owned_multi_region_test")]
        false,
    )
}

#[cfg(any(
    test,
    feature = "native_runtime_test",
    feature = "mixed_workspace_test"
))]
fn make_native_arena_stage<const task_count: usize>(
    access: &bootstrap_physical_access_group<task_count>,
    stack: physical_range,
    arena_base: u64,
    profile: arena_profile,
) -> Result<staged_mapping, &'static str> {
    let plan = arena_mapping_plan(arena_base, profile)?;
    make_resident_stage(
        access,
        stack,
        plan,
        #[cfg(feature = "owned_multi_region_test")]
        false,
    )
}

fn make_resident_stage<const task_count: usize>(
    access: &bootstrap_physical_access_group<task_count>,
    stack: physical_range,
    plan: mapping_plan,
    #[cfg(feature = "owned_multi_region_test")] complete_ancestry: bool,
) -> Result<staged_mapping, &'static str> {
    let text = (
        linker_address!(kernel_text_start),
        linker_address!(kernel_text_end),
    );
    let rodata = (
        linker_address!(kernel_rodata_start),
        linker_address!(kernel_rodata_end),
    );
    let data = (
        linker_address!(kernel_data_start),
        linker_address!(kernel_memory_end),
    );
    if linker_address!(kernel_image_start) != 0x100000
        || text.0 != 0x100000
        || text.1 != rodata.0
        || rodata.1 != data.0
        || data.1 > 0x200000
        || linker_address!(kernel_data_end) > linker_address!(kernel_bss_start)
        || linker_address!(kernel_bss_start) > data.1
        || stack.end().is_none_or(|end| end > text.0)
        || !stack_contains_rsp(stack, 16 * 1024)
        || !unsafe { descriptors::resident_state_is_valid(data.0, data.1) }
    {
        return Err("owned_resident_geometry");
    }
    let mut resident = [resident_request {
        virtual_address: 0,
        page_count: 0,
        permissions: page_permissions {
            user: false,
            writable: false,
            executable: false,
        },
    }; 4];
    for (index, (start, end, writable, executable)) in [
        (text.0, text.1, false, true),
        (rodata.0, rodata.1, false, false),
        (data.0, data.1, true, false),
        (
            stack.base,
            stack.end().ok_or("owned_stack_end")?,
            true,
            false,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if end <= start || !start.is_multiple_of(page_size) || !end.is_multiple_of(page_size) {
            return Err("owned_resident_alignment");
        }
        resident[index] = resident_request {
            virtual_address: start,
            page_count: (end - start) / page_size,
            permissions: page_permissions {
                user: false,
                writable,
                executable,
            },
        };
    }
    #[cfg(feature = "owned_multi_region_test")]
    if complete_ancestry {
        let residency = resident_plan::new_with_ancestry(
            &plan,
            &resident,
            access.alias_entry().ok_or("owned_alias_missing")?,
            32,
        )
        .map_err(|_| "owned_multi_resident_plan")?;
        return staged_mapping::new_resident(plan, residency)
            .map_err(|_| "owned_multi_resident_stage");
    }
    let residency = resident_plan::new(
        &plan,
        &resident,
        access.alias_entry().ok_or("owned_alias_missing")?,
        plan.tables().len() + 1,
    )
    .map_err(|_| "owned_resident_plan")?;
    staged_mapping::new_resident(plan, residency).map_err(|_| "owned_resident_stage")
}

#[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
fn populate<const task_count: usize>(
    mapping: &staged_mapping,
    access: &mut bootstrap_physical_access_group<task_count>,
    region: usize,
    source: u64,
    length: u64,
) -> bool {
    let Some(code) = mapping.data_range(region) else {
        return false;
    };
    // Read back the complete region, including zero-filled BSS and page slack.
    for offset in (0..code.length).step_by(8) {
        let mut bytes = [0; 8];
        for (index, byte) in bytes.iter_mut().enumerate() {
            if offset + (index as u64) < length {
                // SAFETY: Caller validated the immutable linked rodata interval.
                *byte =
                    unsafe { ptr::read_volatile((source + offset + index as u64) as *const u8) };
            }
        }
        let word = u64::from_le_bytes(bytes);
        // SAFETY: Complete aligned word inside the retained exclusive user lease;
        // root is inactive and the independent supervisor alias is writable.
        if !unsafe { access.write_word(code.base + offset, word) }
            || unsafe { access.read_word(code.base + offset) } != Some(word)
        {
            return false;
        }
    }
    true
}

pub(super) fn tls_bases_are_zero() -> bool {
    for register in [0xc000_0100_u32, 0xc000_0101, 0xc000_0102] {
        let low: u32;
        let high: u32;
        // SAFETY: Architectural FS/GS MSRs in the admitted CPL0 environment.
        unsafe {
            asm!("rdmsr", in("ecx") register, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
        }
        if low != 0 || high != 0 {
            return false;
        }
    }
    true
}

/// Explicit role assignment for the mixed native admission proof. Neither
/// executable metadata nor a user-supplied role selects these memory budgets.
#[cfg(feature = "mixed_workspace_test")]
impl owned_execution_group<3> {
    /// Workspace 0 has a zero mailbox, linked data, a separate nine-page frame
    /// arena and a 64KiB user stack. Editor 1 and service 2 retain the bounded
    /// eight-page arena profile. Preparation publishes no runnable task.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked, reserved Threshold bootstrap tree. Sources
    /// are immutable linked rodata. Retain self on every error, including partial
    /// alias installation, population or transition-stack allocation.
    #[inline(never)]
    pub(super) unsafe fn prepare_workspace_editor_service(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        payloads: [(u64, u64); 3],
        linked_data: [(u64, u64); 3],
    ) -> Result<(), &'static str> {
        if self.access.alias_entry().is_some()
            || self.prepared
            || self.finishing
            || self.released
            || super::interrupts::interrupts_enabled()
        {
            return Err("mixed_owned_prepare_state");
        }
        let ro_start = linker_address!(kernel_rodata_start);
        let ro_end = linker_address!(kernel_rodata_end);
        // Validate every source before acquiring any resources for any role.
        for index in 0..3 {
            let (source, length) = payloads[index];
            let (data_source, data_length) = linked_data[index];
            let code_pages = if index == 0 { 32 } else { 8 };
            let data_pages = if index == 0 { 7 } else { 2 };
            if source < ro_start
                || length == 0
                || length > code_pages * page_size
                || source.checked_add(length).is_none_or(|end| end > ro_end)
                || data_length > data_pages * page_size
                || (data_length == 0 && data_source != 0)
                || (data_length != 0
                    && (data_source < ro_start
                        || data_source
                            .checked_add(data_length)
                            .is_none_or(|end| end > ro_end)))
            {
                return Err("mixed_owned_payload_extent");
            }
        }
        // SAFETY: The caller retains the owner across any partial installation.
        unsafe { self.access.install() }.map_err(|_| "mixed_owned_alias_install")?;
        self.bootstrap_stack = self
            .access
            .bootstrap_stack()
            .ok_or("owned_bootstrap_stack")?;
        // SAFETY: The initialized descriptors are exclusively owned by this BSP.
        self.original_rsp0 = unsafe { descriptors::selected_privilege_stack_top() };
        if !stack_contains_rsp(self.bootstrap_stack, 16 * 1024) || self.original_rsp0 == 0 {
            return Err("owned_bootstrap_state");
        }
        for index in 0..3 {
            // SAFETY: Exact prevalidated role and sources; each helper retains
            // its launch before the first allocation and cannot publish tasks.
            unsafe {
                self.prepare_mixed_role(
                    allocator,
                    circles,
                    index,
                    payloads[index],
                    linked_data[index],
                )
            }?;
        }
        self.prepared = true;
        Ok(())
    }

    // Retire per-role mapping-plan temporaries before preparing the next role;
    // the suspended controller must retain ownership, not constructor copies.
    #[inline(never)]
    unsafe fn prepare_mixed_role(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        index: usize,
        payload: (u64, u64),
        linked_data: (u64, u64),
    ) -> Result<(), &'static str> {
        let mapping = if index == 0 {
            make_mixed_workspace_stage(&self.access, self.bootstrap_stack)?
        } else {
            make_native_arena_stage(
                &self.access,
                self.bootstrap_stack,
                if index == 1 { 0x804000 } else { 0x814000 },
                arena_profile::native_application,
            )?
        };
        let admission = user_entry_plan::new(
            mapping.plan(),
            0,
            payload.1,
            user_code_base,
            3,
            user_stack_top,
        )
        .map_err(|_| "mixed_owned_entry_plan")?;
        self.launches[index] = Some(if index == 0 {
            prepared_launch::new_mixed_workspace(mapping, admission)
        } else {
            prepared_launch::new_with_native_arena(
                mapping,
                admission,
                if index == 1 { 0x804000 } else { 0x814000 },
                arena_profile::native_application,
            )
        });
        // SAFETY: Inactive retained mapping; all source bytes were checked in
        // the outer method. Read back every initialized and zero-filled byte.
        unsafe {
            self.launches[index]
                .as_mut()
                .ok_or("owned_launch_missing")?
                .prepare(allocator, &mut self.access, circles, |mapping, access| {
                    populate(mapping, access, 0, payload.0, payload.1)
                        && if index == 0 {
                            populate_mixed_workspace_data(
                                mapping,
                                access,
                                linked_data.0,
                                linked_data.1,
                            )
                        } else {
                            populate(mapping, access, 1, linked_data.0, linked_data.1)
                        }
                        && populate(mapping, access, 2, 0, 0)
                        && populate(mapping, access, 3, 0, 0)
                })
        }
        .map_err(|_| "mixed_owned_launch_prepare")?;
        // SAFETY: Fresh exclusive RAM, cleared and verified through our alias.
        let lease = unsafe {
            allocator.allocate_with(transition_pages, 1, frame_purpose::data, |range| {
                self.access.clear_and_verify(range)
            })
        }
        .map_err(|_| "owned_stack_allocate")?;
        self.stacks[index] = Some(transition_stack {
            lease,
            bound: false,
        });
        let stack = self.stacks[index].as_mut().ok_or("owned_stack_missing")?;
        allocator
            .bind(&stack.lease)
            .map_err(|_| "owned_stack_bind")?;
        stack.bound = true;
        let mut initial = scheduler_frame::initial(admission.bounds().entry());
        initial.stack_pointer = admission.bounds().initial_stack_pointer();
        // SAFETY: Exact aligned tail of this retained, unpublished private stack.
        unsafe { ptr::write(self.frame(index)?, initial) };
        Ok(())
    }
}

#[cfg(feature = "mixed_workspace_test")]
fn make_mixed_workspace_stage(
    access: &bootstrap_physical_access_group<3>,
    stack: physical_range,
) -> Result<staged_mapping, &'static str> {
    let stack_base = user_stack_top - 16 * page_size;
    let requests = [
        (user_code_base, 32, false, true),
        (user_data_base, 8, true, false),
        (0x810000, 9, true, false),
        (stack_base, 16, true, false),
    ]
    .map(
        |(virtual_address, page_count, writable, executable)| mapping_request {
            virtual_address,
            page_count,
            permissions: page_permissions {
                user: true,
                writable,
                executable,
            },
        },
    );
    // One contiguous absent gap covers both the RW upper guard and the frame
    // arena lower guard without exceeding the fixed reservation capacity.
    let reserved = [
        (0, 512),
        (user_code_base - page_size, 1),
        (user_code_base + 32 * page_size, 1),
        (user_data_base - page_size, 1),
        (0x808000, 8),
        (0x819000, 1),
        (stack_base - page_size, 1),
        (user_stack_top, 1),
    ]
    .map(|(virtual_address, page_count)| reserved_request {
        virtual_address,
        page_count,
    });
    let plan = mapping_plan::new(
        &requests,
        &reserved,
        mapping_budget {
            data_pages: 65,
            table_frames: 8,
        },
    )
    .map_err(|_| "mixed_owned_mapping_plan")?;
    make_resident_stage(
        access,
        stack,
        plan,
        #[cfg(feature = "owned_multi_region_test")]
        false,
    )
}

#[cfg(feature = "mixed_workspace_test")]
fn populate_mixed_workspace_data(
    mapping: &staged_mapping,
    access: &mut bootstrap_physical_access_group<3>,
    source: u64,
    length: u64,
) -> bool {
    let Some(data) = mapping.data_range(1) else {
        return false;
    };
    if data.length != 8 * page_size || length > 7 * page_size {
        return false;
    }
    for offset in (0..data.length).step_by(8) {
        let mut bytes = [0; 8];
        for (index, byte) in bytes.iter_mut().enumerate() {
            if let Some(input) = (offset + index as u64)
                .checked_sub(page_size)
                .filter(|at| *at < length)
            {
                // SAFETY: The outer method validated immutable linked rodata.
                *byte = unsafe { ptr::read_volatile((source + input) as *const u8) };
            }
        }
        let word = u64::from_le_bytes(bytes);
        // SAFETY: Complete word in the retained inactive workspace RW lease.
        if !unsafe { access.write_word(data.base + offset, word) }
            || unsafe { access.read_word(data.base + offset) } != Some(word)
        {
            return false;
        }
    }
    true
}

/// Exercise real retained resources with one scheduler slot and failed clears.
/// Unexpected errors halt in the owning scope; only fully reclaimed state returns.
#[cfg(not(feature = "application_profile_test"))]
#[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
pub(super) fn prove_rollback(
    allocator: &mut frame_allocator<'_>,
    payloads: [(u64, u64); 2],
) -> Result<(), &'static str> {
    let initial = allocator.accounting();
    let root = paging::read_cr3();
    let mut execution = owned_execution::new();
    let mut tasks = task_table::new();
    let mut circles = circle_table::new();
    // SAFETY: Invoked under the same exclusive pre-dispatch bootstrap contract.
    unsafe { execution.prepare(allocator, &mut circles, payloads) }
        .unwrap_or_else(|reason| fail(reason));
    let ids = core::array::from_fn::<_, 2, _>(|index| {
        execution
            .commit(index, &mut tasks, &mut circles, index as u64 + 1)
            .unwrap_or_else(|reason| fail(reason))
    });
    let mut scheduler =
        scheduler_table::new(1).unwrap_or_else(|_| fail("owned_rollback_scheduler"));
    scheduler
        .admit(&mut circles, &mut tasks, ids[0])
        .unwrap_or_else(|_| fail("owned_rollback_admit"));
    if scheduler.admit(&mut circles, &mut tasks, ids[1]) != Err(scheduler_error::capacity_exhausted)
        || scheduler.withdraw_unstarted_context(&mut circles, &mut tasks, ids[0])
            != Ok(scheduler_context::initial)
    {
        fail("owned_rollback_withdraw");
    }
    let snapshot = scheduler
        .snapshot(&tasks)
        .unwrap_or_else(|_| fail("owned_rollback_snapshot"));
    if snapshot.admitted() != 0
        || snapshot.runnable() != 0
        || snapshot.current().is_some()
        || snapshot.next_dispatch_sequence() != Some(0)
        || snapshot.next_preemption_sequence() != Some(0)
    {
        fail("owned_rollback_scheduler_changed");
    }
    for index in 0..2 {
        // SAFETY: Never selected, contexts withdrawn, bootstrap unchanged. Each
        // injected failure must retain its unbound lease and the physical alias.
        if unsafe {
            execution.finish_with_failure(allocator, &mut tasks, &mut circles, Some(index))
        } != Err("owned_stack_clear")
            || !execution.access.is_current()
            || execution.stacks[index]
                .as_ref()
                .is_none_or(|stack| stack.bound || stack.lease.range().is_none())
        {
            fail("owned_rollback_retention");
        }
    }
    // SAFETY: Retry finishes the retained partial teardown in the same scope.
    unsafe { execution.finish(allocator, &mut tasks, &mut circles) }
        .unwrap_or_else(|reason| fail(reason));
    if allocator.accounting() != initial
        || tasks.active_count() != 0
        || tasks.attached_context_count() != 0
        || circles.accounting() != circle_table::new().accounting()
        || paging::read_cr3() != root
        || ids
            .iter()
            .any(|id| tasks.validate(*id).is_ok() || circles.validate(id.circle()).is_ok())
    {
        fail("owned_rollback_resources");
    }
    Ok(())
}

#[cfg(not(feature = "application_profile_test"))]
#[cfg_attr(feature = "owned_multi_region_test", allow(dead_code))]
fn fail(reason: &'static str) -> ! {
    super::interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_dispatcher_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}

#[cfg(feature = "native_editor_workspace_test")]
impl owned_execution_group<3> {
    /// The sole retained mailbox's supervisor alias, not an arbitrary user
    /// pointer. The caller must never retain or use it beyond this owner's
    /// lifetime, and must separately validate its current task before access.
    pub(super) fn mixed_workspace_mailbox_alias(&self) -> Result<physical_range, &'static str> {
        if !self.prepared || self.finishing || self.released || !self.access.is_current() {
            return Err("workspace_owned_mailbox_state");
        }
        let launch = self.launches[0].as_ref().ok_or("owned_launch_missing")?;
        if !launch.is_mixed_workspace_profile() || !launch.mapping().is_ready() {
            return Err("workspace_owned_mailbox_profile");
        }
        let data = launch
            .mapping()
            .data_range(1)
            .ok_or("workspace_owned_mailbox_range")?;
        if data.length != 8 * page_size {
            return Err("workspace_owned_mailbox_range");
        }
        let extent = planned_physical_window::new(
            physical_range::new(0, paging::identity_map_limit),
            proof_window_base,
        )
        .and_then(|window| window.plan_virtual_extent(physical_range::new(data.base, page_size)))
        .map_err(|_| "workspace_owned_mailbox_alias")?;
        Ok(physical_range::new(extent.base(), extent.length()))
    }

    fn mixed_workspace_current(&self, index: usize) -> bool {
        index == 0
            && self.prepared
            && self.selecting
            && !self.finishing
            && !self.released
            && self.registered[0]
            && self.activated[0]
            && self.access.is_current()
            && self.launches[0].as_ref().is_some_and(|launch| {
                launch.is_mixed_workspace_profile()
                    && launch.is_committed()
                    && launch.mapping().is_ready()
            })
            && self.root(0).ok() == Some(paging::read_cr3())
            && self.transition_stack_top(0).ok()
                == Some(unsafe { descriptors::selected_privilege_stack_top() })
            && self
                .stack_extent(0)
                .is_ok_and(|stack| stack_contains_rsp(stack, 4096))
            && !super::interrupts::interrupts_enabled()
    }

    /// Temporarily expose boot-only MMIO without mapping it into the workspace
    /// root. This preserves the retained owner and its exact privilege stack.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked; caller has validated the sole unscheduled
    /// running workspace task, eagerly captured its CPU image, and selected the
    /// clean kernel image. The callback cannot enable IRQ, dispatch, copy user memory,
    /// alter paging/TSS/CPU controls, or escape except by fail-stop. Any Err is
    /// fail-stop with ownership retained: never resume, retry, or clean up.
    pub(super) unsafe fn with_mixed_workspace_bootstrap<T>(
        &mut self,
        callback: impl FnOnce() -> T,
    ) -> Result<T, &'static str> {
        let index = 0;
        if !self.mixed_workspace_current(index) {
            return Err("workspace_present_owner");
        }
        let root = self.root(index)?;
        let top = self.transition_stack_top(index)?;
        let stack = self.stack_extent(index)?;
        // SAFETY: Both roots retain the resident return path and high alias;
        // the callback never schedules and RSP0 remains unchanged.
        unsafe { paging::write_cr3(self.access.bootstrap_root()) };
        if paging::read_cr3() != self.access.bootstrap_root() || !self.access.is_current() {
            return Err("workspace_present_bootstrap");
        }
        let result = callback();
        if super::interrupts::interrupts_enabled()
            || paging::read_cr3() != self.access.bootstrap_root()
            || !self.access.is_current()
            || unsafe { descriptors::selected_privilege_stack_top() } != top
            || !stack_contains_rsp(stack, 4096)
        {
            return Err("workspace_present_callback");
        }
        // SAFETY: Exact admitted root, unchanged retained alias and task frame.
        unsafe { paging::write_cr3(root) };
        if !self.mixed_workspace_current(index) {
            return Err("workspace_present_restore");
        }
        Ok(result)
    }
}

#[cfg(all(
    feature = "owned_multi_region_test",
    not(feature = "application_profile_test")
))]
impl<const task_count: usize> owned_execution_group<task_count> {
    /// Native region/source descriptions only. ELF and startup policy remain
    /// outside this mechanism; no task is published before explicit commit.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked, reserved Threshold root, immutable linked
    /// rodata sources and retained self on every error until explicit finish.
    #[allow(dead_code)]
    pub(super) unsafe fn prepare_multi_region(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        images: [super::multi_region_plan::multi_region_image<'_>; task_count],
    ) -> Result<(), &'static str> {
        if task_count != 2
            || self.access.alias_entry().is_some()
            || self.prepared
            || self.finishing
            || self.released
        {
            return Err("multi_prepare_state");
        }
        // SAFETY: Exclusive bootstrap ownership; retain alias through cleanup.
        unsafe { self.access.install() }.map_err(|_| "multi_alias_install")?;
        self.bootstrap_stack = self
            .access
            .bootstrap_stack()
            .ok_or("multi_bootstrap_stack")?;
        // SAFETY: Descriptor selection is exclusively owned by this BSP.
        self.original_rsp0 = unsafe { descriptors::selected_privilege_stack_top() };
        if !stack_contains_rsp(self.bootstrap_stack, 16 * 1024) || self.original_rsp0 == 0 {
            return Err("multi_bootstrap_state");
        }
        for (index, image) in images.into_iter().enumerate() {
            let (plan, admission, sources, stack_region) = image.into_parts();
            for source in sources {
                let start = source.bytes.as_ptr() as u64;
                if start < linker_address!(kernel_rodata_start)
                    || start
                        .checked_add(source.bytes.len() as u64)
                        .is_none_or(|end| end > linker_address!(kernel_rodata_end))
                {
                    return Err("multi_source_extent");
                }
            }
            let mapping = make_resident_stage(&self.access, self.bootstrap_stack, plan, true)?;
            self.launches[index] = Some(prepared_launch::new_multi_region(
                mapping,
                admission,
                stack_region,
            ));
            let launch = self.launches[index]
                .as_mut()
                .ok_or("multi_launch_missing")?;
            // SAFETY: Store the owner before allocation; populate and read back
            // only its complete data leases via the independent physical alias.
            unsafe {
                launch.prepare(allocator, &mut self.access, circles, |mapping, access| {
                    for (region_index, region) in mapping.plan().regions().enumerate() {
                        let Some(range) = mapping.data_range(region_index) else {
                            return false;
                        };
                        for offset in (0..range.length).step_by(8) {
                            let mut bytes = [0; 8];
                            for (byte_index, byte) in bytes.iter_mut().enumerate() {
                                *byte = super::multi_region_plan::initialized_byte(
                                    sources,
                                    region.extent().start() + offset + byte_index as u64,
                                );
                            }
                            let word = u64::from_le_bytes(bytes);
                            if !access.write_word(range.base + offset, word)
                                || access.read_word(range.base + offset) != Some(word)
                            {
                                return false;
                            }
                        }
                    }
                    true
                })
            }
            .map_err(|_| "multi_launch_prepare")?;
            // SAFETY: Owned transition storage is independently cleared/read back.
            let lease = unsafe {
                allocator.allocate_with(transition_pages, 1, frame_purpose::data, |range| {
                    self.access.clear_and_verify(range)
                })
            }
            .map_err(|_| "multi_stack_allocate")?;
            self.stacks[index] = Some(transition_stack {
                lease,
                bound: false,
            });
            let stack = self.stacks[index].as_mut().ok_or("multi_stack_missing")?;
            allocator
                .bind(&stack.lease)
                .map_err(|_| "multi_stack_bind")?;
            stack.bound = true;
            let mut initial = scheduler_frame::initial(admission.bounds().entry());
            initial.stack_pointer = admission.bounds().initial_stack_pointer();
            // SAFETY: Complete aligned retained stack tail, unpublished task.
            unsafe {
                ptr::write(self.frame(index)?, initial);
            }
        }
        self.prepared = true;
        Ok(())
    }

    /// # Safety
    /// Same exclusive bootstrap/cancel contract as finish; no live consumers.
    pub(super) unsafe fn multi_finish_with_stack_failure(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        tasks: &mut task_table,
        circles: &mut circle_table,
        index: usize,
    ) -> Result<(), &'static str> {
        if index >= task_count {
            return Err("multi_failure_index");
        }
        // SAFETY: Forward retained teardown with a single clear failure.
        unsafe { self.finish_with_failure(allocator, tasks, circles, Some(index)) }
    }

    /// Privileged proof observation, never a task-supplied pointer grant.
    pub(super) fn multi_read(&self, index: usize, offset: u64) -> Option<u64> {
        let address = self.word_address(index, 3, offset.checked_add(256)?)?;
        // SAFETY: Exact word in this owner's known private mailbox region.
        unsafe { self.access.read_word(address) }
    }

    pub(super) fn multi_write(
        &mut self,
        index: usize,
        offset: u64,
        value: u64,
    ) -> Result<(), &'static str> {
        let launch = self
            .launches
            .get(index)
            .and_then(Option::as_ref)
            .ok_or("multi_owner")?;
        let permissions = launch
            .mapping()
            .plan()
            .regions()
            .nth(3)
            .ok_or("multi_region")?
            .permissions();
        if !self.prepared
            || self.finishing
            || self.released
            || !launch.is_committed()
            || !permissions.user
            || !permissions.writable
            || permissions.executable
        {
            return Err("multi_mailbox_permissions");
        }
        let address = self
            .word_address(index, 3, offset.checked_add(256).ok_or("multi_offset")?)
            .ok_or("multi_mailbox")?;
        // SAFETY: IF clear/NMI masked, no simultaneous user access, owned RW word.
        if unsafe { self.access.write_word(address, value) } {
            Ok(())
        } else {
            Err("multi_write")
        }
    }
}

// One explicitly acquired, supervisor-only copy scratch. Bulk source bytes live
// in accounted leases; this transient scratch is never retained in a source.
// The atomic guard rejects nested/overlapping imports even within this BSP proof.
#[cfg(all(
    feature = "owned_source_test",
    not(feature = "application_profile_test")
))]
static source_scratch_in_use: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
#[cfg(all(
    feature = "owned_source_test",
    not(feature = "application_profile_test")
))]
static mut source_scratch: [u8; super::owned_source::source_chunk_limit] =
    [0; super::owned_source::source_chunk_limit];

#[cfg(all(
    feature = "owned_source_test",
    not(feature = "application_profile_test")
))]
impl<const task_count: usize> owned_execution_group<task_count> {
    /// # Safety
    /// Retain source independently of this execution owner on every result.
    /// Sole BSP/IF clear/NMI masked; bootstrap selected before user entry.
    pub(super) unsafe fn source_allocate(
        &mut self,
        source: &mut super::owned_source::owned_source,
        allocator: &mut frame_allocator<'_>,
    ) -> Result<(), &'static str> {
        if !self.prepared
            || self.finishing
            || self.released
            || super::interrupts::interrupts_enabled()
            || paging::read_cr3() != self.access.bootstrap_root()
            || !self.access.is_current()
        {
            return Err("source_allocate_state");
        }
        // SAFETY: Independent lease retained by source, live writable alias.
        unsafe { source.allocate(allocator, &mut self.access) }.map_err(|_| "source_allocate")
    }

    /// Current-task authentication is independent of the snapshot grant. Every
    /// request rejection precedes destination mutation. A backend failure may
    /// dirty unaccepted source storage, as documented by owned_source::append.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn source_append(
        &mut self,
        source: &mut super::owned_source::owned_source,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        identity: task_identity,
        grant: u64,
        offset: u64,
        user_address: u64,
        length: usize,
    ) -> Result<(), &'static str> {
        use core::sync::atomic::Ordering;
        self.copy_task_index(tasks, circles, scheduler, identity)
            .map_err(|_| "source_uploader_binding")?;
        source
            .validate_append(identity, grant, offset, length)
            .map_err(|_| "source_append_request")?;
        if source_scratch_in_use
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("source_scratch_busy");
        }
        // SAFETY: Exclusive scratch grant above, IF clear and NMI masked. This
        // kernel BSS is resident in every selected root; source/user leases are
        // separate allocator-owned RAM. No slice leaves this synchronous call.
        let scratch = unsafe { &mut *ptr::addr_of_mut!(source_scratch) };
        let result = if scratch
            .iter()
            .any(|byte| unsafe { ptr::read_volatile(byte) } != 0)
        {
            Err("source_scratch_not_clear")
        } else {
            self.copy_from_user(
                tasks,
                circles,
                scheduler,
                identity,
                user_address,
                &mut scratch[..length],
            )
            .map_err(|_| "source_user_copy")
            .and_then(|()| {
                // SAFETY: Exact authenticated current task, copied bounded
                // nonaliasing bytes; source retains its exclusive lease.
                unsafe {
                    source.append(
                        identity,
                        grant,
                        offset,
                        &scratch[..length],
                        &mut self.access,
                    )
                }
                .map_err(|_| "source_append_storage")
            })
        };
        for byte in scratch.iter_mut() {
            // SAFETY: Complete exclusive initialized scratch, retained above.
            unsafe { ptr::write_volatile(byte, 0) };
        }
        let cleared = scratch
            .iter()
            .all(|byte| unsafe { ptr::read_volatile(byte) } == 0);
        source_scratch_in_use.store(false, Ordering::Release);
        if !cleared {
            return Err("source_scratch_clear");
        }
        result
    }

    pub(super) fn source_seal(
        &self,
        source: &mut super::owned_source::owned_source,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        identity: task_identity,
        grant: u64,
    ) -> Result<(), &'static str> {
        self.copy_task_index(tasks, circles, scheduler, identity)
            .map_err(|_| "source_seal_binding")?;
        source.seal(identity, grant).map_err(|_| "source_seal")
    }

    /// Privileged fixture observation, never an uploader-selected read endpoint.
    pub(super) fn source_observe(
        &mut self,
        source: &super::owned_source::owned_source,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), &'static str> {
        if super::interrupts::interrupts_enabled() || !self.access.is_current() {
            return Err("source_observe_state");
        }
        // SAFETY: Reader borrows sealed storage; alias and nonaliasing kernel
        // destination remain retained throughout this bounded synchronous read.
        unsafe {
            source.reader().map_err(|_| "source_observe_reader")?.read(
                offset,
                destination,
                &mut self.access,
            )
        }
        .map_err(|_| "source_observe_read")
    }
}

#[cfg(all(
    feature = "owned_source_test",
    not(feature = "application_profile_test")
))]
impl<const task_count: usize> owned_execution_group<task_count> {
    /// Populate native mappings from a sealed source, never uploader pointers.
    /// Source possession alone supplies neither destination nor execution grants.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked, bootstrap root and supervisor stack live.
    /// Retain self on every error until finish, and sources through population.
    // Share metadata temporaries with the caller's separately scoped consumer
    // preparation frame; a second frame exceeds the bootstrap headroom guard.
    #[inline(always)]
    pub(super) unsafe fn prepare_owned_sources(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        images: [super::owned_source_plan::owned_source_image<'_>; task_count],
    ) -> Result<(), &'static str> {
        if task_count != 2
            || self.access.alias_entry().is_some()
            || self.prepared
            || self.finishing
            || self.released
            || super::interrupts::interrupts_enabled()
        {
            return Err("source_prepare_state");
        }
        // SAFETY: Fresh alias, independent of the already retired producer.
        unsafe { self.access.install() }.map_err(|_| "source_alias_install")?;
        self.bootstrap_stack = self
            .access
            .bootstrap_stack()
            .ok_or("source_bootstrap_stack")?;
        // SAFETY: Exclusive descriptor selection before any published task.
        self.original_rsp0 = unsafe { descriptors::selected_privilege_stack_top() };
        if !stack_contains_rsp(self.bootstrap_stack, 16 * 1024) || self.original_rsp0 == 0 {
            return Err("source_bootstrap_state");
        }
        for (index, image) in images.into_iter().enumerate() {
            let (plan, admission, reader, sources, stack_region) = image.into_parts();
            let mapping = make_resident_stage(&self.access, self.bootstrap_stack, plan, true)?;
            self.launches[index] = Some(prepared_launch::new_multi_region(
                mapping,
                admission,
                stack_region,
            ));
            let launch = self.launches[index]
                .as_mut()
                .ok_or("source_launch_missing")?;
            // SAFETY: Store destination owner before allocations. Reader keeps
            // source sealed and retained; only a fresh scoped alias reads it.
            unsafe {
                launch.prepare(allocator, &mut self.access, circles, |mapping, access| {
                    for (region_index, region) in mapping.plan().regions().enumerate() {
                        let Some(range) = mapping.data_range(region_index) else {
                            return false;
                        };
                        for offset in (0..range.length).step_by(8) {
                            let mut bytes = [0; 8];
                            let destination = region.extent().start() + offset;
                            for span in sources {
                                let start = destination.max(span.destination);
                                let end = (destination + 8).min(span.destination + span.length);
                                if start < end
                                    && reader
                                        .read(
                                            span.source_offset + (start - span.destination),
                                            &mut bytes[(start - destination) as usize
                                                ..(end - destination) as usize],
                                            access,
                                        )
                                        .is_err()
                                {
                                    return false;
                                }
                            }
                            let word = u64::from_le_bytes(bytes);
                            if !access.write_word(range.base + offset, word)
                                || access.read_word(range.base + offset) != Some(word)
                            {
                                return false;
                            }
                        }
                    }
                    true
                })
            }
            .map_err(|_| "source_launch_prepare")?;
            // SAFETY: Independent retained transition stack, cleared/read back.
            let lease = unsafe {
                allocator.allocate_with(transition_pages, 1, frame_purpose::data, |range| {
                    self.access.clear_and_verify(range)
                })
            }
            .map_err(|_| "source_stack_allocate")?;
            self.stacks[index] = Some(transition_stack {
                lease,
                bound: false,
            });
            let stack = self.stacks[index].as_mut().ok_or("source_stack_missing")?;
            allocator
                .bind(&stack.lease)
                .map_err(|_| "source_stack_bind")?;
            stack.bound = true;
            let mut initial = scheduler_frame::initial(admission.bounds().entry());
            initial.stack_pointer = admission.bounds().initial_stack_pointer();
            // SAFETY: Full retained unpublished frame at exact owned stack tail.
            unsafe { ptr::write(self.frame(index)?, initial) };
        }
        self.prepared = true;
        Ok(())
    }
}

#[cfg(feature = "linux_loader_entry_test")]
impl owned_execution_group<3> {
    /// Read a bounded sealed-source chunk through this execution owner's
    /// already-live physical alias. This does not select a source grant,
    /// authenticate a task, or publish any bytes to userspace. Metadata errors
    /// leave the destination untouched; a backend failure may dirty a prefix,
    /// so the embedding must discard and clear its scratch on every error.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked. The embedding retains the sealed source
    /// owner and this execution group throughout the read, separately validates
    /// the authenticated request and any eventual userspace output, and lends
    /// initialized kernel scratch that cannot alias the retained source. No
    /// source slice, physical pointer or scratch reference may escape the call.
    pub(super) unsafe fn read_loader_source(
        &mut self,
        reader: &super::owned_source::sealed_source_reader<'_>,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), &'static str> {
        if !self.prepared
            || !self.selecting
            || self.finishing
            || self.released
            || super::interrupts::interrupts_enabled()
            || !self.access.is_current()
            || !self.current_stack_is_owned()
        {
            return Err("loader_source_read_state");
        }
        // SAFETY: The borrowing reader retains the sealed lease; this borrow
        // retains the current alias. The caller supplies nonaliasing kernel
        // scratch under the same excluded ownership interval. The reader
        // checks the complete nonempty <=4096-byte logical extent first.
        unsafe { reader.read(offset, destination, &mut self.access) }
            .map_err(|_| "loader_source_read")
    }

    /// Prepare one nine-region sealed-source caller and two ordinary native
    /// roles. The controller selects this mixed profile; source bytes do not.
    /// No task is published until the separate commit calls succeed.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked, reserved bootstrap root and stack live.
    /// Native spans are immutable linked rodata. Retain this owner after every
    /// error until finish, and keep the sealed source alive through population.
    pub(super) unsafe fn prepare_loader_entry(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        caller: super::owned_source_plan::owned_source_image<'_>,
        native_images: [((u64, u64), (u64, u64)); 2],
    ) -> Result<(), &'static str> {
        if self.access.alias_entry().is_some()
            || self.prepared
            || self.finishing
            || self.released
            || super::interrupts::interrupts_enabled()
        {
            return Err("loader_prepare_state");
        }
        // Reject native source extents before acquiring any destination owner.
        for ((code, length), (data, data_length)) in native_images {
            if code < linker_address!(kernel_rodata_start)
                || length == 0
                || length > 8 * page_size
                || code
                    .checked_add(length)
                    .is_none_or(|end| end > linker_address!(kernel_rodata_end))
                || data_length > 2 * page_size
                || (data_length == 0 && data != 0)
                || (data_length != 0
                    && (data < linker_address!(kernel_rodata_start)
                        || data
                            .checked_add(data_length)
                            .is_none_or(|end| end > linker_address!(kernel_rodata_end))))
            {
                return Err("loader_native_extent");
            }
        }
        // SAFETY: Exclusive bootstrap ownership; alias remains through cleanup.
        unsafe { self.access.install() }.map_err(|_| "loader_alias_install")?;
        self.bootstrap_stack = self
            .access
            .bootstrap_stack()
            .ok_or("loader_bootstrap_stack")?;
        // SAFETY: Descriptor state belongs exclusively to this BSP.
        self.original_rsp0 = unsafe { descriptors::selected_privilege_stack_top() };
        if !stack_contains_rsp(self.bootstrap_stack, 16 * 1024) || self.original_rsp0 == 0 {
            return Err("loader_bootstrap_state");
        }
        // Separate non-inlined role construction avoids reserving every plan's
        // large temporaries concurrently on the 64-KiB bootstrap stack.
        // SAFETY: Exclusive installed alias, sealed caller source still borrowed.
        unsafe { self.prepare_loader_caller(allocator, circles, caller) }?;
        for (offset, image) in native_images.into_iter().enumerate() {
            // SAFETY: Index is exactly one or two; linked spans were checked.
            unsafe { self.prepare_loader_native(allocator, circles, offset + 1, image) }?;
        }
        self.prepared = true;
        Ok(())
    }

    #[inline(never)]
    unsafe fn prepare_loader_caller(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        image: super::owned_source_plan::owned_source_image<'_>,
    ) -> Result<(), &'static str> {
        let (plan, admission, reader, sources, stack_region) = image.into_parts();
        if plan
            .regions()
            .nth(stack_region)
            .is_none_or(|region| region.extent().end() - region.extent().start() != 16 * page_size)
        {
            return Err("loader_caller_stack");
        }
        let mapping = make_resident_stage(&self.access, self.bootstrap_stack, plan, true)?;
        self.launches[0] = Some(prepared_launch::new_multi_region(
            mapping,
            admission,
            stack_region,
        ));
        let launch = self.launches[0].as_mut().ok_or("loader_caller_missing")?;
        // SAFETY: Store the owner before allocation. The sealed reader borrows
        // live source ownership; each scoped read uses our independent alias.
        unsafe {
            launch.prepare(allocator, &mut self.access, circles, |mapping, access| {
                for (region_index, region) in mapping.plan().regions().enumerate() {
                    let Some(range) = mapping.data_range(region_index) else {
                        return false;
                    };
                    for offset in (0..range.length).step_by(8) {
                        let mut bytes = [0; 8];
                        let destination = region.extent().start() + offset;
                        for span in sources {
                            let start = destination.max(span.destination);
                            let end = (destination + 8).min(span.destination + span.length);
                            if start < end
                                && reader
                                    .read(
                                        span.source_offset + (start - span.destination),
                                        &mut bytes[(start - destination) as usize
                                            ..(end - destination) as usize],
                                        access,
                                    )
                                    .is_err()
                            {
                                return false;
                            }
                        }
                        let word = u64::from_le_bytes(bytes);
                        if !access.write_word(range.base + offset, word)
                            || access.read_word(range.base + offset) != Some(word)
                        {
                            return false;
                        }
                    }
                }
                true
            })
        }
        .map_err(|_| "loader_caller_prepare")?;
        // SAFETY: The caller role is unpublished and retains its complete plan.
        unsafe { self.prepare_loader_transition(allocator, 0, admission) }
    }

    #[inline(never)]
    unsafe fn prepare_loader_native(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        index: usize,
        ((source, length), (data, data_length)): ((u64, u64), (u64, u64)),
    ) -> Result<(), &'static str> {
        let mapping = make_stage(&self.access, self.bootstrap_stack, 8)?;
        let admission =
            user_entry_plan::new(mapping.plan(), 0, length, user_code_base, 2, user_stack_top)
                .map_err(|_| "loader_native_entry")?;
        self.launches[index] = Some(prepared_launch::new_with_code_pages(mapping, admission, 8));
        let launch = self.launches[index]
            .as_mut()
            .ok_or("loader_native_missing")?;
        // SAFETY: Linked sources were checked before alias installation. Store
        // each destination owner before any fallible allocation/population.
        unsafe {
            launch.prepare(allocator, &mut self.access, circles, |mapping, access| {
                populate(mapping, access, 0, source, length)
                    && populate(mapping, access, 1, data, data_length)
            })
        }
        .map_err(|_| "loader_native_prepare")?;
        // SAFETY: Exact unpublished native role, same retained alias lifetime.
        unsafe { self.prepare_loader_transition(allocator, index, admission) }
    }

    unsafe fn prepare_loader_transition(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        index: usize,
        admission: user_entry_plan,
    ) -> Result<(), &'static str> {
        // SAFETY: Independent exclusive supervisor storage is zeroed/read back.
        let lease = unsafe {
            allocator.allocate_with(transition_pages, 1, frame_purpose::data, |range| {
                self.access.clear_and_verify(range)
            })
        }
        .map_err(|_| "loader_stack_allocate")?;
        self.stacks[index] = Some(transition_stack {
            lease,
            bound: false,
        });
        let stack = self.stacks[index].as_mut().ok_or("loader_stack_missing")?;
        allocator
            .bind(&stack.lease)
            .map_err(|_| "loader_stack_bind")?;
        stack.bound = true;
        let mut initial = scheduler_frame::initial(admission.bounds().entry());
        initial.stack_pointer = admission.bounds().initial_stack_pointer();
        // SAFETY: Full retained aligned frame, unpublished until explicit commit.
        unsafe { ptr::write(self.frame(index)?, initial) };
        Ok(())
    }
}
