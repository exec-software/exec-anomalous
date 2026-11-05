//! Opt-in composition evidence; no syscall-number or errno interpretation.
use super::super::super::linux_tls_fixture as fixture;
use super::*;

#[derive(Clone, Copy, Default)]
pub(super) struct tls_evidence {
    pub(super) queries: u64,
    pub(super) stages: u64,
    pub(super) commits: u64,
    pub(super) entries: u64,
    pub(super) returns: u64,
    pub(super) timer_samples: u64,
    pub(super) caller_fp_entries: u64,
    pub(super) peer_zero_entries: u64,
    pub(super) retired: u64,
    pub(super) full_width_commits: u64,
    pub(super) retired_caller_fs: u64,
    caller_fs_at_entry: u64,
}

unsafe extern "C" {
    static x86_64_linux_tls_application_payload_start: u8;
    static x86_64_linux_tls_application_payload_end: u8;
    static x86_64_linux_tls_application_data_start: u8;
    static x86_64_linux_tls_application_data_end: u8;
    static x86_64_linux_tls_application_elf_start: u8;
    static x86_64_linux_tls_application_elf_end: u8;
    static x86_64_linux_tls_application_plan_start: u8;
    static x86_64_linux_tls_application_plan_end: u8;
    static x86_64_linux_tls_application_user_fault_instruction: u8;
    static x86_64_linux_tls_veil_payload_start: u8;
    static x86_64_linux_tls_veil_payload_end: u8;
    static x86_64_linux_tls_veil_data_start: u8;
    static x86_64_linux_tls_veil_data_end: u8;
    static x86_64_linux_tls_veil_user_fault_instruction: u8;
}

pub(super) fn images() -> (artifact, artifact) {
    let span = elf_profile::span;
    let rx = span(
        address!(x86_64_linux_tls_application_payload_start),
        address!(x86_64_linux_tls_application_payload_end),
        code_pages * 4096,
    );
    let rw = span(
        address!(x86_64_linux_tls_application_data_start),
        address!(x86_64_linux_tls_application_data_end),
        8192,
    );
    let elf = span(
        address!(x86_64_linux_tls_application_elf_start),
        address!(x86_64_linux_tls_application_elf_end),
        65536,
    );
    let plan = anomalous_elf_adapter::load_plan::decode(span(
        address!(x86_64_linux_tls_application_plan_start),
        address!(x86_64_linux_tls_application_plan_end),
        96,
    ))
    .unwrap_or_else(|_| fail("linux_tls_manifest"));
    if plan.entry() != user_code_base
        || plan.image_length() != elf.len() as u64
        || plan.rx().file_size() != rx.len() as u64
        || plan.rw().file_size() != rw.len() as u64
        || plan.rw().memory_size() != 8192
        || rw.get(..128).is_none_or(|s| s.iter().any(|b| *b != 0))
    {
        fail("linux_tls_geometry");
    }
    let file_part = |offset: u64, length: u64| {
        let end = offset
            .checked_add(length)
            .unwrap_or_else(|| fail("linux_tls_segment"));
        elf.get(offset as usize..end as usize)
            .unwrap_or_else(|| fail("linux_tls_segment"))
    };
    if file_part(plan.rx().file_offset(), plan.rx().file_size()) != rx
        || file_part(plan.rw().file_offset(), plan.rw().file_size()) != rw
    {
        fail("linux_tls_segment_comparison");
    }
    let make = |rx: &[u8], rw: &[u8], fault: u64, linux| {
        let offset = fault
            .checked_sub(user_code_base)
            .unwrap_or_else(|| fail("linux_tls_fault")) as usize;
        if rx.get(offset..offset.saturating_add(2)) != Some(&[0x0f, 0x0b]) {
            fail("linux_tls_fault");
        }
        artifact {
            source: rx.as_ptr() as u64,
            length: rx.len() as u64,
            fault,
            data: (rw.as_ptr() as u64, rw.len() as u64),
            loader: false,
            linux,
        }
    };
    let client = make(
        rx,
        rw,
        address!(x86_64_linux_tls_application_user_fault_instruction),
        true,
    );
    let veil = make(
        span(
            address!(x86_64_linux_tls_veil_payload_start),
            address!(x86_64_linux_tls_veil_payload_end),
            code_pages * 4096,
        ),
        span(
            address!(x86_64_linux_tls_veil_data_start),
            address!(x86_64_linux_tls_veil_data_end),
            8192,
        ),
        address!(x86_64_linux_tls_veil_user_fault_instruction),
        false,
    );
    (client, veil)
}

impl runtime {
    pub(super) fn consume_tls(&mut self, frame: *const scheduler_frame) {
        // SAFETY: Caller first authenticates the complete current frame/root/TSS.
        unsafe {
            self.tls
                .consume_entry(self.identities[self.selected], frame)
        }
        .unwrap_or_else(|e| fail(e.reason()));
        if !self.tls.kernel_is_clean() {
            fail("linux_tls_kernel_state");
        }
        self.veil_evidence.tls.entries += 1;
        if self.selected == 0 {
            self.veil_evidence.tls.caller_fs_at_entry = self
                .tls
                .get_fs(self.identities[0])
                .unwrap_or_else(|e| fail(e.reason()));
        } else {
            if self
                .tls
                .get_fs(self.identities[self.selected])
                .unwrap_or_else(|e| fail(e.reason()))
                != 0
            {
                fail("linux_tls_peer_base");
            }
            self.veil_evidence.tls.peer_zero_entries += 1;
        }
    }
    pub(super) fn stage_tls(&mut self, frame: *const scheduler_frame) {
        if !self.frame_valid(frame) {
            fail("linux_tls_return_frame");
        }
        // SAFETY: Selected retained root/TSS/frame verified before staging.
        unsafe { self.tls.stage_return(self.identities[self.selected], frame) }
            .unwrap_or_else(|e| fail(e.reason()));
        self.veil_evidence.tls.returns += 1;
    }
    pub(super) fn retire_tls(&mut self, index: usize) {
        if index == 0 {
            // An exit completion cannot carry a staged effect. Thus the last
            // authenticated caller capture remains its terminal FS value even
            // when a returning veil, rather than the caller, is current.
            let base = self.veil_evidence.tls.caller_fs_at_entry;
            if base
                != if self.scenario == 2 {
                    0
                } else {
                    fixture::base_a
                }
            {
                fail("linux_tls_retire_base");
            }
            self.veil_evidence.tls.retired_caller_fs = base;
        }
        self.tls
            .retire(self.identities[index])
            .unwrap_or_else(|e| fail(e.reason()));
        self.veil_evidence.tls.retired += 1;
    }
    pub(super) fn observe_caller_fp(&mut self) {
        if self.selected != 0
            || !self
                .cpu
                .task_image(0)
                .is_some_and(|image| fixture::fp_matches(image.bytes()))
        {
            fail("linux_tls_caller_fp");
        }
        let base = self
            .tls
            .get_fs(self.identities[0])
            .unwrap_or_else(|e| fail(e.reason()));
        if let Some((offset, canary)) = fixture::mapped_canary(base)
            && (self.execution.read_data(0, offset) != Some(canary)
                || !self
                    .execution
                    .read_data(0, offset + 8)
                    .is_some_and(|p| p > 0))
        {
            fail("linux_tls_caller_memory");
        }
        self.veil_evidence.tls.caller_fp_entries += 1;
    }
    pub(super) fn observe_tls_timer(&mut self) {
        if self.selected != 0 || self.execution.read_data(0, 104) != Some(1) {
            return;
        }
        let base = self
            .tls
            .get_fs(self.identities[0])
            .unwrap_or_else(|e| fail(e.reason()));
        if base != fixture::base_a
            || self.execution.read_data(0, 32) != Some(fixture::canary_a)
            || !self.execution.read_data(0, 40).is_some_and(|v| v > 0)
        {
            fail("linux_tls_timer_memory");
        }
        // Outside the dedicated spin the compiled caller may use FP registers.
        // Only the first spin observation supplies the physical timer witness.
        if self.execution.read_data(0, 40) == Some(1) {
            return;
        }
        if self.execution.read_data(0, 96) == Some(0) {
            if !self
                .cpu
                .task_image(0)
                .is_some_and(|image| fixture::fp_matches(image.bytes()))
            {
                fail("linux_tls_timer_fp");
            }
            self.execution
                .write_data(0, 96, 1)
                .unwrap_or_else(|e| fail(e));
            self.veil_evidence.tls.timer_samples += 1;
        }
    }
    pub(super) fn caller_fs(&self) -> Result<u64, u64> {
        let authority = self.pending.as_ref().ok_or(4u64)?;
        self.execution
            .validate_suspended_caller(&self.tasks, &self.circles, &self.scheduler, authority)
            .map_err(|_| 4u64)?;
        // SAFETY: Retained suspended caller/root validated above; sole IF-clear owner.
        unsafe {
            self.tls
                .query_suspended_fs(&self.tasks, &self.scheduler, authority)
        }
        .map_err(|_| 4u64)
    }
    pub(super) fn tls_operation(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        let args = [
            value.destination_index,
            value.source_index,
            value.data,
            value.register_10,
            value.register_8,
            value.register_9,
        ];
        if !fixture::operation_arguments_valid(value.accumulator, args) {
            return (4, 0, 0);
        }
        if value.accumulator == 21 {
            return match self.caller_fs() {
                Ok(base) => {
                    self.veil_evidence.tls.queries += 1;
                    (0, base, 0)
                }
                Err(status) => (status, 0, 0),
            };
        }
        if self.stage_kind != 0 || self.tls_update.is_pending() {
            return (4, 0, 0);
        }
        let authority = self.pending.as_ref().unwrap();
        if self
            .execution
            .validate_suspended_caller(&self.tasks, &self.circles, &self.scheduler, authority)
            .is_err()
            || unsafe {
                self.tls.stage_suspended_fs(
                    &self.tasks,
                    &self.scheduler,
                    authority,
                    &mut self.tls_update,
                    value.source_index,
                )
            }
            .is_err()
        {
            return (4, 0, 0);
        }
        self.stage_kind = 4;
        self.stage_address = value.source_index;
        self.veil_evidence.stages += 1;
        self.veil_evidence.tls.stages += 1;
        (0, self.sequence, 0)
    }
    pub(super) fn validate_tls_finished(&self) {
        let e = self.veil_evidence.tls;
        if !self.tls.storage_is_cleared()
            || !self.tls_update.storage_is_cleared()
            || e.entries
                != self.evidence.calls
                    + self.veil_evidence.raw_calls
                    + self.evidence.timer_ticks
                    + u64::from(self.scenario != 0)
            || e.returns != e.entries
            || e.retired != 3
            || e.caller_fp_entries != self.veil_evidence.raw_calls + u64::from(self.scenario == 1)
            || e.peer_zero_entries < 2
            || e.timer_samples != u64::from(self.scenario != 2)
            || e.full_width_commits != u64::from(self.scenario != 2)
            || e.retired_caller_fs
                != if self.scenario == 2 {
                    0
                } else {
                    fixture::base_a
                }
            || e.queries != [11, 11, 3][self.scenario]
            || e.stages != [5, 5, 1][self.scenario]
            || e.commits != [5, 5, 0][self.scenario]
        {
            fail("linux_tls_final_evidence");
        }
    }
}

pub(super) fn emit(records: [(evidence, veil_evidence); 3]) {
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_tls_test.ready","level":"info","tasks":3,"scenarios":3,"data_frames":48,"table_frames":27,"leases":15,"request_bytes":80,"policy":"userspace","startup":"fixed_fixture","tls_slots":3,"fs_only":true,"eager_cpu":true}}"#
    );
    for (scenario, (e, v)) in records.into_iter().enumerate() {
        let t = v.tls;
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.linux_tls_test.scenario","level":"info","scenario":{},"raw_calls":{},"completions":{},"stages":{},"rejected":{},"aborted":{},"cooperative_switches":{},"timer_ticks":{},"surface_sends":{},"input_sends":{},"surface_deliveries":{},"input_deliveries":{},"surface_acks":{},"input_acks":{},"discarded":{},"exits":{},"faults":{},"rows":{},"native_calls":{},"tls_queries":{},"tls_stages":{},"tls_commits":{},"tls_entries":{},"tls_returns":{},"tls_timer_samples":{},"caller_fp_entries":{},"peer_zero_entries":{},"tls_retired":{},"full_width_commits":{},"retired_caller_fs":{}}}"#,
            scenario,
            v.raw_calls,
            v.completions,
            v.stages,
            v.rejected,
            v.aborted,
            e.switches,
            e.timer_ticks,
            e.sends[0],
            e.sends[1],
            e.deliveries[0],
            e.deliveries[1],
            e.acknowledgements[0],
            e.acknowledgements[1],
            e.discarded,
            e.exits,
            e.faults,
            e.rows,
            e.calls,
            t.queries,
            t.stages,
            t.commits,
            t.entries,
            t.returns,
            t.timer_samples,
            t.caller_fp_entries,
            t.peer_zero_entries,
            t.retired,
            t.full_width_commits,
            t.retired_caller_fs
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_tls_test.passed","level":"info","application_tasks":9,"hashes_equal":true,"pixels_equal":true,"abort_unpublished":true,"resources_restored":true,"authority_revoked":true,"tls_cleared":true,"cpu_restored":true,"full_width_metadata":true,"mapped_fs_bytes":true}}"#
    );
}
