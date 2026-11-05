use crate::console::serial_port;
use crate::input::{key_state, physical_input_delivery};
use crate::time::monotonic_instant;
use core::fmt::Write;

const minimum_timer_interrupts: u64 = 8;
const input_wait_tick_budget: u64 = 1_000;
const private_probe_make_codes: [u8; 12] = [
    0x19, 0x13, 0x17, 0x2f, 0x1e, 0x14, 0x12, 0x19, 0x13, 0x18, 0x30, 0x12,
];
const post_rearm_make_codes: [u8; 1] = [0x2d];

#[derive(Clone, Copy)]
struct input_proof {
    source_generation: u32,
    first_sequence: u64,
    last_sequence: u64,
    transitions: u64,
}

#[derive(Clone, Copy)]
struct loss_proof {
    source_generation: u32,
    first_sequence: u64,
    last_sequence: u64,
    lost_count: u64,
    delivered_before: bool,
    resynchronization_required: bool,
    new_source_generation: u32,
    contained_master_mask: u8,
    rearmed_master_mask: u8,
}

pub(crate) fn run(serial: &mut serial_port) {
    if !crate::arch::read_external_interrupt_health().input_available {
        fail(serial, "physical_input_unavailable");
    }

    let start = read_time(serial);
    if !crate::arch::run_interrupt_return_probe() {
        fail(serial, "register_return_probe");
    }
    let end = wait_for_timer_progress(serial, start);
    let interrupts = end
        .ticks_since(start)
        .unwrap_or_else(|| fail(serial, "timebase_discontinuity"));
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.interrupt_test.timer.verified","level":"info","timebase_generation":{},"start_tick":{},"end_tick":{},"interrupts":{interrupts}}}"#,
        end.timebase().generation(),
        start.tick(),
        end.tick()
    );

    let input = wait_for_exact_physical_input(
        serial,
        end,
        crate::input::ps2_source_generation,
        0,
        &private_probe_make_codes,
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.interrupt_test.input.verified","level":"info","source_generation":{},"first_sequence":{},"last_sequence":{},"transitions":{},"ordered":true,"monotonic":true}}"#,
        input.source_generation, input.first_sequence, input.last_sequence, input.transitions
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.interrupt_test.return.verified","level":"info","timer":true,"keyboard":true,"state_preserved":true}}"#
    );

    let loss = prove_loss_contract(serial, input);
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.interrupt_test.loss.rearmed","level":"info","old_source_generation":{},"new_source_generation":{},"contained_master_mask":{},"rearmed_master_mask":{},"input_available":true}}"#,
        loss.source_generation,
        loss.new_source_generation,
        loss.contained_master_mask,
        loss.rearmed_master_mask
    );

    let post_rearm_began_at = read_time(serial);
    let post_rearm = wait_for_exact_physical_input(
        serial,
        post_rearm_began_at,
        loss.new_source_generation,
        0,
        &post_rearm_make_codes,
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.interrupt_test.loss.verified","level":"info","source_generation":{},"first_sequence":{},"last_sequence":{},"lost_count":{},"delivered_before":{},"delivered_after":true,"resynchronization_required":{},"new_source_generation":{},"post_rearm_first_sequence":{},"post_rearm_last_sequence":{},"post_rearm_transitions":{}}}"#,
        loss.source_generation,
        loss.first_sequence,
        loss.last_sequence,
        loss.lost_count,
        loss.delivered_before,
        loss.resynchronization_required,
        loss.new_source_generation,
        post_rearm.first_sequence,
        post_rearm.last_sequence,
        post_rearm.transitions
    );

    crate::arch::disable_external_interrupts();
    let health = crate::arch::read_external_interrupt_health();
    if crate::arch::external_interrupts_enabled()
        || health.master_mask != u8::MAX
        || health.slave_mask != u8::MAX
    {
        fail(serial, "final_masking");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.interrupt_test.masked","level":"info","master_mask":{},"slave_mask":{},"if_enabled":false}}"#,
        health.master_mask, health.slave_mask
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.interrupt_test.passed","level":"info","ticks":{},"transitions":{},"lost":{}}}"#,
        end.tick(),
        input.transitions,
        loss.lost_count
    );
    crate::arch::qemu_exit_success();
    crate::arch::halt_forever()
}

fn wait_for_timer_progress(
    serial: &mut serial_port,
    start: monotonic_instant,
) -> monotonic_instant {
    loop {
        let current = read_time(serial);
        let Some(progress) = current.ticks_since(start) else {
            fail(serial, "timebase_discontinuity");
        };
        if progress >= minimum_timer_interrupts {
            return current;
        }
        if !crate::arch::wait_for_interrupt_if_idle(|| false) {
            fail(serial, "timer_wait_unavailable");
        }
    }
}

fn wait_for_exact_physical_input(
    serial: &mut serial_port,
    began_at: monotonic_instant,
    expected_generation: u32,
    expected_first_sequence: u64,
    expected_make_codes: &[u8],
) -> input_proof {
    let expected_transitions = u64::try_from(expected_make_codes.len())
        .ok()
        .and_then(|length| length.checked_mul(2))
        .unwrap_or_else(|| fail(serial, "expected_transition_count_exhausted"));
    let mut first_sequence = None;
    let mut last_observed_at = None;
    let mut transitions = 0u64;

    loop {
        let delivery = crate::arch::with_external_interrupts_disabled(|| {
            // SAFETY: The architecture critical section excludes the sole IRQ
            // producer for the complete typed-channel access.
            unsafe { crate::input::poll_physical_input_if_disabled() }
        })
        .unwrap_or_else(|| fail(serial, "input_critical_section"));

        if let Some(delivery) = delivery {
            match delivery {
                physical_input_delivery::transition(record) => {
                    let generation = record.source().generation();
                    let sequence = record.sequence().get();
                    let expected_sequence = expected_first_sequence
                        .checked_add(transitions)
                        .unwrap_or_else(|| fail(serial, "expected_sequence_exhausted"));
                    let transition = *record.payload();
                    let expected_index = usize::try_from(transitions / 2)
                        .unwrap_or_else(|_| fail(serial, "expected_transition_index"));
                    let expected_state = if transitions & 1 == 0 {
                        key_state::pressed
                    } else {
                        key_state::released
                    };
                    if generation != expected_generation
                        || sequence != expected_sequence
                        || transition.key().is_extended()
                        || transition.key().make_code() != expected_make_codes[expected_index]
                        || transition.state() != expected_state
                        || last_observed_at.is_some_and(|previous| {
                            record.observed_at().ticks_since(previous).is_none()
                        })
                    {
                        fail(serial, "input_private_or_metadata_mismatch");
                    }
                    first_sequence.get_or_insert(sequence);
                    last_observed_at = Some(record.observed_at());
                    transitions = transitions
                        .checked_add(1)
                        .unwrap_or_else(|| fail(serial, "transition_count_exhausted"));
                    if transitions == expected_transitions {
                        return input_proof {
                            source_generation: expected_generation,
                            first_sequence: first_sequence
                                .unwrap_or_else(|| fail(serial, "input_identity_missing")),
                            last_sequence: sequence,
                            transitions,
                        };
                    }
                }
                physical_input_delivery::loss(_) => {
                    fail(serial, "unexpected_input_loss");
                }
            }
            continue;
        }

        let now = read_time(serial);
        if now
            .ticks_since(began_at)
            .is_none_or(|ticks| ticks > input_wait_tick_budget)
        {
            fail(serial, "input_timeout");
        }
        if !crate::arch::wait_for_interrupt_if_idle(|| {
            // SAFETY: The architecture evaluates the bounded predicate under
            // the same IF-clear single-BSP exclusion contract.
            unsafe { crate::input::ps2_delivery_ready_if_disabled() }
        }) {
            fail(serial, "input_wait_unavailable");
        }
    }
}

fn prove_loss_contract(serial: &mut serial_port, input: input_proof) -> loss_proof {
    crate::arch::with_external_interrupts_disabled(|| {
        // SAFETY: This entire feature proof runs in one architecture-owned
        // critical section, excluding real IRQ1 publication until it ends.
        unsafe { prove_loss_contract_if_disabled(input) }
    })
    .unwrap_or_else(|| fail(serial, "loss_critical_section"))
    .unwrap_or_else(|reason| fail(serial, reason))
}

unsafe fn prove_loss_contract_if_disabled(input: input_proof) -> Result<loss_proof, &'static str> {
    // SAFETY: The caller established uninterrupted exclusive access above.
    if unsafe { crate::input::ps2_delivery_ready_if_disabled() } {
        return Err("loss_queue_not_empty");
    }
    if !crate::arch::interrupt_test_force_loss() {
        return Err("forced_loss_not_contained");
    }
    let contained_health = crate::arch::read_external_interrupt_health();
    if contained_health.input_available
        || contained_health.master_mask != 0xfe
        || contained_health.slave_mask != u8::MAX
    {
        return Err("forced_loss_containment_state");
    }

    let first_queued_sequence = input
        .last_sequence
        .checked_add(1)
        .ok_or("forced_queue_sequence_exhausted")?;
    let mut previous_observed_at = None;
    for offset in 0..crate::input::ps2_queue_capacity {
        // SAFETY: The caller still owns the source channel exclusively.
        let Some(delivery) = (unsafe { crate::input::poll_physical_input_if_disabled() }) else {
            return Err("forced_queue_delivery_missing");
        };
        let physical_input_delivery::transition(record) = delivery else {
            return Err("forced_queue_early_loss");
        };
        let sequence = first_queued_sequence
            .checked_add(u64::try_from(offset).map_err(|_| "forced_queue_offset")?)
            .ok_or("forced_queue_sequence_exhausted")?;
        let transition = *record.payload();
        if record.source().generation() != input.source_generation
            || record.sequence().get() != sequence
            || transition.key().is_extended()
            || transition.key().make_code() != 0x1e
            || transition.state() != key_state::pressed
            || previous_observed_at
                .is_some_and(|previous| record.observed_at().ticks_since(previous).is_none())
        {
            return Err("forced_queue_delivery_mismatch");
        }
        previous_observed_at = Some(record.observed_at());
    }

    // SAFETY: The caller still owns the source channel exclusively.
    let Some(delivery) = (unsafe { crate::input::poll_physical_input_if_disabled() }) else {
        return Err("forced_loss_missing");
    };
    let physical_input_delivery::loss(loss) = delivery else {
        return Err("forced_loss_replaced");
    };
    let resynchronization_required =
        // SAFETY: This reads the same source-local latch while IF remains clear.
        unsafe { crate::input::ps2_resynchronization_required_if_disabled() };
    if loss.terminal_reason().is_some()
        || !loss.includes_capacity_loss()
        || loss.includes_source_discontinuity()
        || loss.lost_count() != 1
        || !resynchronization_required
    {
        return Err("forced_loss_contract");
    }
    let first_sequence = loss
        .first_sequence()
        .ok_or("forced_loss_identity_missing")?
        .get();
    let last_sequence = loss
        .last_sequence()
        .ok_or("forced_loss_identity_missing")?
        .get();
    let expected_loss_sequence = first_queued_sequence
        .checked_add(
            u64::try_from(crate::input::ps2_queue_capacity).map_err(|_| "forced_queue_capacity")?,
        )
        .ok_or("forced_loss_sequence_exhausted")?;
    if loss.source().generation() != input.source_generation
        || first_sequence != expected_loss_sequence
        || last_sequence != expected_loss_sequence
        || unsafe { crate::input::ps2_delivery_ready_if_disabled() }
    {
        return Err("forced_loss_range");
    }

    let Some(new_source_generation) = crate::arch::rearm_keyboard_input_after_loss() else {
        return Err("forced_loss_resynchronization");
    };
    let rearmed_health = crate::arch::read_external_interrupt_health();
    if new_source_generation != input.source_generation + 1
        || !rearmed_health.input_available
        || rearmed_health.master_mask != 0xfc
        || rearmed_health.slave_mask != u8::MAX
    {
        return Err("forced_loss_rearm_state");
    }

    Ok(loss_proof {
        source_generation: loss.source().generation(),
        first_sequence,
        last_sequence,
        lost_count: loss.lost_count(),
        delivered_before: true,
        resynchronization_required,
        new_source_generation,
        contained_master_mask: contained_health.master_mask,
        rearmed_master_mask: rearmed_health.master_mask,
    })
}

fn read_time(serial: &mut serial_port) -> monotonic_instant {
    crate::arch::with_external_interrupts_disabled(|| {
        // SAFETY: The architecture critical section excludes the sole timer
        // producer for this exact observation.
        unsafe { crate::time::now_if_disabled() }
    })
    .unwrap_or_else(|| fail(serial, "time_critical_section"))
}

fn fail(serial: &mut serial_port, reason: &str) -> ! {
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.interrupt_test.failed","level":"error","reason":"{reason}"}}"#
    );
    crate::arch::disable_external_interrupts();
    crate::arch::qemu_exit_failure();
    crate::arch::halt_forever()
}
