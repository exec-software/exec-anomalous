#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! One supervised userspace personality for the already qualified producer.
//! The native supervisor owns caller identity, memory, channels and scheduling.
//! This vessel alone selects Linux operations, descriptors and errno replies.

use anomalous_veil_protocol::{
    caller_read_operation, channel_snapshot, complete_operation, completion, invocation,
    linux_policy::{self, backend},
    native_status, query_operation, request_operation, request_size, stage_kind, stage_operation,
    stage_receipt,
};
use core::{arch::global_asm, panic::PanicInfo, ptr};

global_asm!(include_str!("entry.S"), options(att_syntax));

#[unsafe(no_mangle)]
#[unsafe(link_section = ".data.elf_canary")]
static anomalous_linux_veil_data_canary: u64 = 0x454c_465f_4441_5441;

#[repr(C)]
struct call_record {
    operation: u64,
    arguments: [u64; 6],
    status: u64,
    value: u64,
    auxiliary: u64,
}

unsafe extern "C" {
    fn anomalous_linux_veil_call(record: *mut call_record);
    fn anomalous_linux_veil_fault_instruction() -> !;
    fn anomalous_linux_veil_failure_instruction() -> !;
}

fn call(operation: u64, arguments: [u64; 6]) -> call_record {
    let mut record = call_record {
        operation,
        arguments,
        status: u64::MAX,
        value: u64::MAX,
        auxiliary: u64::MAX,
    };
    // SAFETY: The assembly stub preserves SysV callee-saved registers and
    // writes only the three result fields of this live private stack record.
    // Every passed user address remains opaque until checked by its owner.
    unsafe { anomalous_linux_veil_call(&mut record) };
    record
}

fn success(record: &call_record) -> Result<(), native_status> {
    match native_status::from_raw(record.status).expect("unknown native result") {
        native_status::ok => Ok(()),
        error => {
            assert_eq!((record.value, record.auxiliary), (0, 0));
            Err(error)
        }
    }
}

fn rejected(operation: u64, arguments: [u64; 6], status: native_status) {
    let record = call(operation, arguments);
    assert_eq!(record.status, status as u64);
    assert_eq!((record.value, record.auxiliary), (0, 0));
}

struct invocation_backend {
    sequence: u64,
}

impl backend for invocation_backend {
    fn query(&mut self, capability: u64) -> Result<channel_snapshot, native_status> {
        let record = call(query_operation, [self.sequence, capability, 0, 0, 0, 0]);
        success(&record)?;
        assert_eq!(record.auxiliary, 0);
        Ok(channel_snapshot::decode(record.value).expect("invalid channel snapshot"))
    }

    fn caller_read(&mut self, address: u64, destination: &mut [u8]) -> Result<(), native_status> {
        let record = call(
            caller_read_operation,
            [
                self.sequence,
                address,
                destination.as_mut_ptr() as u64,
                destination.len() as u64,
                0,
                0,
            ],
        );
        success(&record)?;
        assert_eq!(
            (record.value, record.auxiliary),
            (destination.len() as u64, 0)
        );
        Ok(())
    }

    fn stage(
        &mut self,
        kind: stage_kind,
        address: u64,
        length: u64,
        source: &[u8],
    ) -> Result<stage_receipt, native_status> {
        assert!(
            (kind == stage_kind::copyout && source.len() as u64 == length)
                || (kind != stage_kind::copyout && source.is_empty())
        );
        let source_address = if source.is_empty() {
            0
        } else {
            source.as_ptr() as u64
        };
        let record = call(
            stage_operation,
            [
                self.sequence,
                kind as u64,
                address,
                length,
                source_address,
                0,
            ],
        );
        success(&record)?;
        assert_eq!(record.value, self.sequence);
        Ok(stage_receipt {
            token: record.value,
            length: record.auxiliary,
        })
    }
}

fn reply_arguments(request: invocation, reply: completion) -> [u64; 6] {
    [
        request.sequence(),
        reply.result_bits,
        reply.action as u64,
        reply.stage_token,
        0,
        0,
    ]
}

fn rejected_stage_replies(request: invocation, reply: completion) {
    assert_eq!(reply.stage_token, request.sequence());
    let mut changed = reply_arguments(request, reply);
    changed[3] = reply.stage_token.checked_add(1).expect("request exhausted");
    rejected(complete_operation, changed, native_status::invalid);
    changed = reply_arguments(request, reply);
    changed[2] = 3;
    rejected(complete_operation, changed, native_status::invalid);
}

fn staged_fault_probes(request: invocation, reply: completion) -> ! {
    let sequence = request.sequence();
    let wrong_sequence = sequence.checked_add(1).expect("request exhausted");
    assert!(sequence > 1 && reply.stage_token == sequence);
    // Rejected proposals must leave the sole prepared publication retained.
    // Root independently checks the caller, queue and destination snapshots.
    rejected(
        query_operation,
        [wrong_sequence, 0, 0, 0, 0, 0],
        native_status::invalid,
    );
    rejected(
        caller_read_operation,
        [wrong_sequence, 0, 0, 1, 0, 0],
        native_status::invalid,
    );
    let args = request.arguments();
    rejected(
        stage_operation,
        [
            wrong_sequence,
            stage_kind::submit as u64,
            args[1],
            args[2],
            0,
            0,
        ],
        native_status::invalid,
    );
    let mut changed = reply_arguments(request, reply);
    changed[0] = wrong_sequence;
    rejected(complete_operation, changed, native_status::invalid);
    rejected(
        stage_operation,
        [sequence, stage_kind::submit as u64, args[1], args[2], 0, 0],
        native_status::invalid,
    );
    changed = reply_arguments(request, reply);
    changed[2] = 3;
    rejected(complete_operation, changed, native_status::invalid);
    changed = reply_arguments(request, reply);
    changed[0] = sequence - 1;
    rejected(complete_operation, changed, native_status::invalid);
    changed = reply_arguments(request, reply);
    changed[3] = wrong_sequence;
    rejected(complete_operation, changed, native_status::invalid);
    // SAFETY: This exact marker is the dedicated supervisor-containment probe.
    // No completion has published the prepared record or resumed the caller.
    unsafe { anomalous_linux_veil_fault_instruction() }
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_linux_veil_run(role: u64, scenario: u64) -> ! {
    assert!(role == 2 && scenario <= 2);
    // SAFETY: The immutable static is backed by admitted initialized RW data;
    // volatile access proves its copied file bytes rather than constant folding.
    assert_eq!(
        unsafe { ptr::read_volatile(ptr::addr_of!(anomalous_linux_veil_data_canary)) },
        0x454c_465f_4441_5441
    );
    let mut previous_sequence = 0;
    let mut successful_stage_probed = [false; 3];
    loop {
        let mut bytes = [0; request_size];
        if previous_sequence == 0 {
            rejected(
                request_operation,
                [
                    bytes.as_mut_ptr() as u64,
                    (request_size - 1) as u64,
                    0,
                    0,
                    0,
                    0,
                ],
                native_status::invalid,
            );
            rejected(
                request_operation,
                [0, request_size as u64, 0, 0, 0, 0],
                native_status::bad_copy,
            );
            assert!(bytes.iter().all(|byte| *byte == 0));
        }
        let record = call(
            request_operation,
            [bytes.as_mut_ptr() as u64, request_size as u64, 0, 0, 0, 0],
        );
        success(&record).expect("invocation retrieval failed");
        let request = invocation::decode(&bytes).expect("invalid invocation");
        assert_eq!(
            (record.value, record.auxiliary),
            (request.sequence(), request_size as u64)
        );
        assert!(request.sequence() > previous_sequence);
        if previous_sequence == 0 {
            rejected(
                query_operation,
                [request.sequence().checked_add(1).unwrap(), 0, 0, 0, 0, 0],
                native_status::invalid,
            );
        }
        previous_sequence = request.sequence();
        let mut backend = invocation_backend {
            sequence: request.sequence(),
        };
        let reply =
            linux_policy::evaluate(request, &mut backend).expect("native stage contract failed");
        if scenario == 2 && request.number() as u32 as i32 == 1 && reply.stage_token != 0 {
            staged_fault_probes(request, reply);
        }
        if scenario == 0 && reply.stage_token != 0 {
            let kind = match request.number() as u32 as i32 {
                1 => 0,
                0 => 1,
                7 => 2,
                _ => panic!("unexpected staged operation"),
            };
            if !successful_stage_probed[kind] {
                rejected_stage_replies(request, reply);
                successful_stage_probed[kind] = true;
            }
        }
        bytes.fill(0);
        let completed = call(complete_operation, reply_arguments(request, reply));
        // Completion parks this vessel. It resumes only after a new caller
        // invocation is retained, never while the previous reply is pending.
        success(&completed).expect("completion rejected");
        assert_eq!((completed.value, completed.auxiliary), (0, 0));
    }
}

#[panic_handler]
fn panic(_information: &PanicInfo<'_>) -> ! {
    // SAFETY: A distinct unexpected UD2 is contained as proof failure rather
    // than being mistaken for the intentional staged-publication abort.
    unsafe { anomalous_linux_veil_failure_instruction() }
}
