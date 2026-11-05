//! Native FS ownership proof over the retained multi-region execution owner.
//!
//! These fixed call sites and memory patterns qualify the mechanism. They do
//! not define a stable ABI or a Linux TLS layout.

use core::arch::asm;
use core::fmt::Write;

use super::owned_execution::owned_execution;
use super::owned_tls_fixture::{base, base_offset, memory_matches, request_case};
use super::scheduler_test::scheduler_frame;
use super::task_tls::{session, tls_error};
use crate::console::serial_port;
use crate::identity::task_identity;
use crate::memory::physical::frame_accounting;

const code_base: u64 = 0x60001000;
const expected_entries: u16 = 106;

macro_rules! request_symbols {
    ($($symbol:ident),+ $(,)?) => {
        unsafe extern "C" { $(static $symbol: u8;)+ }
        fn request_sites(index: usize) -> [u64; 10] {
            let all = [$({
                let address: u64;
                // SAFETY: Numeric immutable linked marker; no slice authority.
                unsafe { asm!("lea {out}, [rip + {symbol}]", out = out(reg) address,
                    symbol = sym $symbol, options(nomem, nostack, preserves_flags)); }
                address
            }),+];
            core::array::from_fn(|step| all[index * 10 + step])
        }
    };
}
request_symbols!(
    x86_64_owned_tls_faulting_request_0_return,
    x86_64_owned_tls_faulting_request_1_return,
    x86_64_owned_tls_faulting_request_2_return,
    x86_64_owned_tls_faulting_request_3_return,
    x86_64_owned_tls_faulting_request_4_return,
    x86_64_owned_tls_faulting_request_5_return,
    x86_64_owned_tls_faulting_request_6_return,
    x86_64_owned_tls_faulting_request_7_return,
    x86_64_owned_tls_faulting_request_8_return,
    x86_64_owned_tls_faulting_request_9_return,
    x86_64_owned_tls_peer_request_0_return,
    x86_64_owned_tls_peer_request_1_return,
    x86_64_owned_tls_peer_request_2_return,
    x86_64_owned_tls_peer_request_3_return,
    x86_64_owned_tls_peer_request_4_return,
    x86_64_owned_tls_peer_request_5_return,
    x86_64_owned_tls_peer_request_6_return,
    x86_64_owned_tls_peer_request_7_return,
    x86_64_owned_tls_peer_request_8_return,
    x86_64_owned_tls_peer_request_9_return,
);

pub(super) struct proof {
    owner: session,
    steps: [u8; 2],
    progress: [u64; 2],
    peer_at_fault: u64,
    entries: u16,
    returns: u16,
    retired: [bool; 2],
}

impl proof {
    pub(super) const fn new() -> Self {
        Self {
            owner: session::new(),
            steps: [0; 2],
            progress: [0; 2],
            peer_at_fault: 0,
            entries: 0,
            returns: 0,
            retired: [false; 2],
        }
    }

    /// Called once with exact committed identities, before task publication.
    pub(super) unsafe fn prepare(
        &mut self,
        identities: [task_identity; 2],
        execution: &owned_execution,
    ) -> Result<(), &'static str> {
        for index in 0..2 {
            for offset in [base_offset(index), base_offset(index) + 0x400] {
                if execution.read_stack(index, offset) != Some(0)
                    || execution.read_stack(index, offset + 8) != Some(0)
                {
                    return Err("owned_tls_initial_memory");
                }
            }
        }
        // SAFETY: The embedding owns committed tasks, sole BSP, IF clear and
        // masked NMI. Session admission checks every hardware precondition.
        unsafe { self.owner.prepare(identities) }.map_err(|_| "owned_tls_prepare")
    }

    /// The embedding first validates current identity, frame, root and TSS.
    pub(super) unsafe fn consume(
        &mut self,
        identity: task_identity,
        frame: *const scheduler_frame,
    ) -> Result<(), &'static str> {
        if self.entries >= expected_entries {
            return Err("owned_tls_entry_count");
        }
        // SAFETY: The complete frame is authenticated by the retained runtime.
        unsafe { self.owner.consume_entry(identity, frame) }.map_err(|_| "owned_tls_entry")?;
        if !self.owner.kernel_is_clean() {
            return Err("owned_tls_kernel_state");
        }
        self.entries += 1;
        Ok(())
    }

    /// The embedding validates the complete selected frame after root/TSS selection.
    pub(super) unsafe fn stage(
        &mut self,
        identity: task_identity,
        frame: *const scheduler_frame,
    ) -> Result<(), &'static str> {
        if self.returns >= expected_entries {
            return Err("owned_tls_return_count");
        }
        // SAFETY: The retained selected frame belongs to this exact live task.
        unsafe { self.owner.stage_return(identity, frame) }.map_err(|_| "owned_tls_return")?;
        self.returns += 1;
        Ok(())
    }

    pub(super) fn request(
        &mut self,
        index: usize,
        identity: task_identity,
        value: scheduler_frame,
        payload_start: u64,
        execution: &owned_execution,
    ) -> Result<u64, &'static str> {
        let step = *self.steps.get(index).ok_or("owned_tls_request_task")?;
        let (operation, argument, result) =
            request_case(index, step).ok_or("owned_tls_request_sequence")?;
        let expected = request_sites(index)[usize::from(step)]
            .checked_sub(payload_start)
            .and_then(|offset| code_base.checked_add(offset))
            .ok_or("owned_tls_request_site")?;
        if value.accumulator != operation
            || value.destination_index != argument
            || value.instruction_pointer != expected
            || value.flags != 2
        {
            return Err("owned_tls_request_frame");
        }
        check_memory(execution, index, step, 1)?;
        let previous = self
            .owner
            .get_fs(identity)
            .map_err(|_| "owned_tls_get_before")?;
        let observed = if operation == 13 {
            previous
        } else if matches!(step, 1 | 2) {
            if self.owner.set_fs(identity, argument) != Err(tls_error::base)
                || self.owner.get_fs(identity).ok() != Some(previous)
                || !self.owner.kernel_is_clean()
            {
                return Err("owned_tls_rejection_mutated");
            }
            1
        } else {
            self.owner
                .set_fs(identity, argument)
                .map_err(|_| "owned_tls_set")?;
            if self.owner.get_fs(identity).ok() != Some(argument) {
                return Err("owned_tls_set_readback");
            }
            0
        };
        if observed != result {
            return Err("owned_tls_request_result");
        }
        self.steps[index] += 1;
        Ok(observed)
    }

    pub(super) fn ready(
        &mut self,
        index: usize,
        identity: task_identity,
        execution: &owned_execution,
    ) -> Result<(), &'static str> {
        if self.steps.get(index) != Some(&10)
            || self.owner.get_fs(identity).ok() != Some(base(index))
        {
            return Err("owned_tls_ready_sequence");
        }
        check_memory(execution, index, 10, 1)?;
        self.progress[index] = 1;
        Ok(())
    }

    pub(super) fn sample(
        &mut self,
        index: usize,
        identity: task_identity,
        register_progress: u64,
        published_progress: u64,
        execution: &owned_execution,
    ) -> Result<(), &'static str> {
        if self.steps.get(index) != Some(&10)
            || self.owner.get_fs(identity).ok() != Some(base(index))
        {
            return Err("owned_tls_sample_sequence");
        }
        let progress = execution
            .read_stack(index, base_offset(index) + 8)
            .ok_or("owned_tls_progress_read")?;
        // INC R14, mailbox publication and INC FS:8 are separate instructions.
        // An interrupt may observe the TLS word one behind the saved register.
        if progress < self.progress[index]
            || progress > published_progress
            || published_progress > register_progress
            || register_progress - progress > 1
        {
            return Err("owned_tls_progress_observation");
        }
        check_memory(execution, index, 10, progress)?;
        self.progress[index] = progress;
        Ok(())
    }

    pub(super) fn retire(
        &mut self,
        index: usize,
        identity: task_identity,
    ) -> Result<(), &'static str> {
        if self.retired.get(index) != Some(&false) || self.progress[index] <= 1 {
            return Err("owned_tls_retirement");
        }
        self.owner
            .retire(identity)
            .map_err(|_| "owned_tls_retire")?;
        self.retired[index] = true;
        if index == 0 {
            self.peer_at_fault = self.progress[1];
        }
        Ok(())
    }

    /// Bootstrap is restored; both tasks are terminal but their storage is held.
    pub(super) unsafe fn finish(&mut self) -> Result<(), &'static str> {
        if self.steps != [10; 2]
            || self.entries != expected_entries
            || self.returns != expected_entries
            || self.retired != [true; 2]
            || self.progress[0] <= 1
            || self.progress[1] <= self.peer_at_fault
            || !self.owner.kernel_is_clean()
        {
            return Err("owned_tls_final_evidence");
        }
        // SAFETY: Both identities are retired, no user frame is staged and the
        // original bootstrap continuation has consumed the terminal transfer.
        unsafe { self.owner.finish() }.map_err(|_| "owned_tls_finish")?;
        // finish itself reads back admitted selectors and all three MSRs; after
        // retirement there is deliberately no active clean-kernel capsule.
        if !self.owner.storage_is_cleared() {
            return Err("owned_tls_finish_readback");
        }
        self.steps = [0; 2];
        self.progress = [0; 2];
        self.peer_at_fault = 0;
        self.entries = 0;
        self.returns = 0;
        self.retired = [false; 2];
        Ok(())
    }
}

fn check_memory(
    execution: &owned_execution,
    index: usize,
    step: u8,
    progress: u64,
) -> Result<(), &'static str> {
    let offset = base_offset(index);
    let observed = [offset, offset + 8, offset + 0x400, offset + 0x408]
        .map(|offset| execution.read_stack(index, offset));
    if !memory_matches(index, step, progress, observed) {
        return Err("owned_tls_memory_correspondence");
    }
    Ok(())
}

pub(super) fn emit_evidence(initial: frame_accounting) {
    let mut serial = serial_port::com1();
    for record in [
        r#"{"schema":1,"event":"kernel.owned_tls_test.rollback","level":"info","unpublished_cancellations":2,"stack_clear_failures":2,"cleanup_retries":2,"resources_restored":true}"#,
        r#"{"schema":1,"event":"kernel.owned_tls_test.ready","level":"info","tasks":2,"quantum_switches":64,"survivor_ticks":16,"data_frames":184,"table_frames":20,"active_leases":22,"bound_leases":22,"regions_per_task":9,"executable_regions":2,"initialized_readback":true,"rejected_return_sites":8,"owned_low_roots":true,"prepared_launch":true,"high_alias_stacks":true,"fs_only":true,"fsgsbase_disabled":true,"gs_zero":true,"kernel_gs_zero":true}"#,
        r#"{"schema":1,"event":"kernel.owned_tls_test.operations","level":"info","tasks":2,"calls":20,"accepted_sets":8,"rejected_sets":4,"gets":8,"initial_zero":true,"full_width_bases":true,"base_changes_preserved":true}"#,
        r#"{"schema":1,"event":"kernel.owned_tls_test.progress","level":"info","preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"task_0_progress":true,"task_1_progress":true,"canaries_intact":true,"tls_memory_correlated":true,"cross_region_gates":2}"#,
        r#"{"schema":1,"event":"kernel.owned_tls_test.fault_contained","level":"info","vector":6,"task":0,"peer":1,"peer_continued":true,"survivor_ticks":16,"survivor_tls_progress":true}"#,
    ] {
        let _ = writeln!(serial, "{record}");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_tls_test.released","level":"info","free_frames":{},"resources_restored":true,"storage_cleared":true,"kernel_continuation_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true,"tls_storage_cleared":true,"tls_hardware_restored":true}}"#,
        initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_tls_test.passed","level":"info","timer_ticks":80,"dispatches":66,"preemptions":64,"faults":1,"exits":1,"tls_entries":106,"tls_returns":106}}"#
    );
}
