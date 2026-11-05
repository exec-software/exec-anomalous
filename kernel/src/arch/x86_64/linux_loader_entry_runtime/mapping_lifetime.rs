//! Native lifetime controls; all foreign request interpretation stays in the veil.

use super::*;
use crate::arch::x86_64::owned_execution::loader_mapping_lifetime::lifetime_context;
use anomalous_veil_protocol::source_mapping_lifetime::{
    source_mapping_control_request, source_mapping_handle,
};

#[derive(Clone, Copy, Default)]
pub(super) struct lifetime_observation {
    sequence: u64,
    generation: u64,
    retentions: u64,
    publishes: u64,
    inspections: u64,
    releases: u64,
    rejections: u64,
    hardware_admitted: bool,
    caller_resumed: bool,
    next_sequence: u64,
    resume_rejections: u64,
    terminal_release: bool,
    mapping: loader_mapping_observation,
}

impl lifetime_observation {
    pub(super) fn qualified(&self, scenario: u64) -> bool {
        if matches!(scenario, 0 | 7) {
            self.sequence == 11
                && self.generation == 1
                && self.retentions == 1
                && self.publishes == 1
                && self.inspections == 1
                && self.releases == 0
                && self.rejections == 11
                && self.hardware_admitted
                && self.caller_resumed
                && self.next_sequence == 12
                && self.resume_rejections == 7
                && !self.terminal_release
                && self.mapping.cleanup_retries == 0
        } else {
            self.sequence == 0
                && self.generation == 0
                && self.retentions == 0
                && self.publishes == 0
                && self.inspections == 0
                && self.releases == 0
                && self.rejections == 0
                && !self.hardware_admitted
                && !self.caller_resumed
                && self.next_sequence == 0
                && self.resume_rejections == 0
                && !self.terminal_release
        }
    }

    pub(super) fn reject_resume(&mut self) {
        self.resume_rejections = self
            .resume_rejections
            .checked_add(1)
            .unwrap_or_else(|| fail("loader_resume_rejections"));
    }

    pub(super) fn committed_resume(&mut self) {
        if self.hardware_admitted || self.caller_resumed || self.releases != 0 {
            fail("loader_resume_observation_replay");
        }
        self.hardware_admitted = true;
    }

    pub(super) fn observe_next(&mut self, sequence: u64) {
        if !self.hardware_admitted
            || self.caller_resumed
            || self.next_sequence != 0
            || self.sequence.checked_add(1) != Some(sequence)
        {
            fail("loader_resume_next_sequence");
        }
        // Only a fresh hardware-authenticated Linux entry proves that the
        // committed reply actually resumed the interpreter.
        self.caller_resumed = true;
        self.next_sequence = sequence;
    }

    pub(super) fn terminal_release(&mut self, observation: loader_mapping_observation) {
        if self.releases != 0 || self.terminal_release {
            fail("loader_resume_release_replay");
        }
        self.mapping = observation;
        self.releases = 1;
        self.terminal_release = true;
    }
}

impl runtime {
    #[inline(never)]
    pub(super) fn mapping_lifetime_call(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if !self.authenticated_request(self.sequence)
            || self.waiting.is_some()
            || interrupts::interrupts_enabled()
            || !self.file_proof_complete
            || !self.scratch_is_clear()
            || self.mapping_readback.iter().any(|b| *b != 0)
        {
            fail("loader_mapping_lifetime_phase");
        }
        let frame = self.execution.frame(0).unwrap_or_else(|e| fail(e));
        if !self.execution.frame_owned(0, frame) {
            fail("loader_lifetime_frame");
        }
        // SAFETY: Exact owned saved frame of the sealed suspended caller.
        let before = frame_words(unsafe { ptr::read(frame) });
        if before != self.reply_frames[self.raw_calls as usize - 1][0] {
            fail("loader_lifetime_pending_frame");
        }
        let sequence = self.sequence;
        let correlation = self.pending.as_ref().unwrap().correlation_words();
        let invocation = self.request;
        let arguments = [
            value.destination_index,
            value.source_index,
            value.data,
            value.register_10,
            value.register_8,
            value.register_9,
        ];
        let context = lifetime_context {
            tasks: &self.tasks,
            scheduler: &self.scheduler,
            authorization: self.pending.as_ref().unwrap(),
            binding: self.file_binding,
            sequence,
            incarnation: self.incarnation,
        };
        let result = (|| -> Result<(u64, u64), native_status> {
            if value.accumulator == 26 {
                let request =
                    source_mapping_request::decode(sequence, self.file_binding, arguments)?;
                if self.mapping_lifetime.retentions != 0 || self.execution.has_loader_mapping() {
                    return Err(native_status::invalid);
                }
                let observation = memory::with_frame_allocator(|allocator| {
                    if allocator.accounting() != self.allocator_live_accounting {
                        fail("loader_lifetime_initial_accounting");
                    }
                    // SAFETY: Actual source is moved into retained BSS before
                    // allocation; no access/allocator reference crosses return.
                    unsafe {
                        self.execution.retain_loader_mapping(
                            allocator,
                            context,
                            &mut self.circles,
                            request,
                            &mut self.file_source,
                            (&mut self.copy_scratch, &mut self.mapping_readback),
                        )
                    }
                })
                .unwrap_or_else(|e| fail(e.reason()))?;
                self.mapping_lifetime.sequence = sequence;
                self.mapping_lifetime.generation = 1;
                self.mapping_lifetime.retentions = 1;
                self.mapping_lifetime.mapping = observation;
                return Ok((1, self.incarnation));
            }
            if !self.execution.has_loader_mapping() {
                return Err(native_status::invalid);
            }
            let handle =
                source_mapping_handle::new(self.mapping_lifetime.generation, self.incarnation)?;
            source_mapping_control_request::decode(sequence, handle, arguments)?;
            // Lifetime control belongs to the creation invocation. A fresh
            // scheduler call does not revive the old handle, even if the native
            // caller substitutes its new sequence into otherwise valid words.
            if sequence != self.mapping_lifetime.sequence {
                return Err(native_status::invalid);
            }
            match value.accumulator {
                27 if self.mapping_lifetime.publishes == 0 => {
                    let observation = memory::with_frame_allocator(|allocator| {
                        // SAFETY: Caller remains sealed/blocked; both ownership
                        // components persist in BSS before any external store.
                        unsafe {
                            self.execution.publish_loader_mapping(
                                allocator,
                                context,
                                &mut self.circles,
                            )
                        }
                    })
                    .unwrap_or_else(|e| fail(e.reason()))?;
                    self.mapping_lifetime.publishes = 1;
                    Ok((
                        observation.population.populated_pages,
                        observation.virtual_pages,
                    ))
                }
                28 if self.mapping_lifetime.publishes == 1
                    && self.mapping_lifetime.inspections == 0 =>
                {
                    // SAFETY: Normal composed user-copy validates both retained
                    // owners. Scratch is disjoint BSS and wiped before return.
                    let observation = unsafe {
                        self.execution.inspect_loader_mapping(
                            context,
                            &self.circles,
                            (&mut self.copy_scratch, &mut self.mapping_readback),
                        )
                    }?;
                    self.mapping_lifetime.inspections = 1;
                    Ok((
                        observation.population.source_bytes,
                        observation.population.destination_fnv64,
                    ))
                }
                29 if self.mapping_lifetime.inspections == 1
                    && self.mapping_lifetime.releases == 0 =>
                {
                    self.execution
                        .authenticate_loader_mapping(context, &self.circles)?;
                    if !self.execution.loader_mapping_inspected() {
                        fail("loader_release_uninspected");
                    }
                    let observation = memory::with_frame_allocator(|allocator| {
                        // SAFETY: Authenticated caller cannot resume; withdrawal
                        // and the no-PCID/global flush precede explicit cleanup.
                        let observation = unsafe {
                            self.execution.release_loader_mapping_storage(
                                allocator,
                                &mut self.circles,
                                &mut self.file_source,
                            )
                        };
                        if allocator.accounting() != self.allocator_live_accounting {
                            fail("loader_lifetime_final_accounting");
                        }
                        observation
                    })
                    .unwrap_or_else(|e| fail(e.reason()));
                    self.mapping_lifetime.mapping = observation;
                    self.mapping_lifetime.releases = 1;
                    Ok((
                        observation.population.populated_pages,
                        observation.virtual_pages,
                    ))
                }
                _ => Err(native_status::invalid),
            }
        })();
        if !self.authenticated_request(sequence)
            || self.sequence != sequence
            || self.pending.as_ref().unwrap().correlation_words() != correlation
            || self.request != invocation
            || self.waiting.is_some()
            || frame_words(unsafe { ptr::read(frame) }) != before
            || !self.scratch_is_clear()
            || self.mapping_readback.iter().any(|b| *b != 0)
        {
            fail("loader_mapping_lifetime_effect");
        }
        match result {
            Ok((value, auxiliary)) => (0, value, auxiliary),
            Err(status) => {
                self.mapping_lifetime.rejections = self
                    .mapping_lifetime
                    .rejections
                    .checked_add(1)
                    .unwrap_or_else(|| fail("loader_lifetime_rejections"));
                (status as u64, 0, 0)
            }
        }
    }
}

pub(super) fn emit(state: &runtime, serial: &mut serial_port) {
    let value = state.mapping_lifetime;
    let mapping = value.mapping;
    let population = mapping.population;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.mapping_lifetime","level":"info","scenario":{},"sequence":{},"generation":{},"retentions":{},"publishes":{},"inspections":{},"releases":{},"rejections":{},"base":{},"byte_length":{},"virtual_pages":{},"populated_pages":{},"table_frames":{},"source_bytes":{},"source_fnv64":{},"destination_fnv64":{},"zero_bytes":{},"absent_pages":{},"published":{},"withdrawn":{},"released":{},"hardware_admitted":{},"caller_resumed":{},"next_sequence":{},"resume_rejections":{},"terminal_release":{},"original_copy_preserved":true,"writes_refused":true,"tail_refused":true,"neighbors_refused":true,"accounting_restored":true,"circle_restored":true,"caller_preserved":true,"scratch_cleared":true}}"#,
        state.scenario,
        value.sequence,
        value.generation,
        value.retentions,
        value.publishes,
        value.inspections,
        value.releases,
        value.rejections,
        mapping.virtual_address,
        mapping.byte_length,
        mapping.virtual_pages,
        population.populated_pages,
        population.table_frames,
        population.source_bytes,
        population.source_fnv64,
        population.destination_fnv64,
        population.zero_bytes,
        population.absent_pages,
        value.publishes == 1,
        value.releases == 1,
        value.releases == 1,
        value.hardware_admitted,
        value.caller_resumed,
        value.next_sequence,
        value.resume_rejections,
        value.terminal_release
    );
}
