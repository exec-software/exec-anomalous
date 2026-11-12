//! Native manifest proof policy atop the qualified single-task delivery lifecycle.
//! The kernel provisions opaque bytes and bootstrap grants; it never parses NIL
//! or converts diagnostic reports into authority (ADR 0103).
use super::*;
use anomalous_native_runtime::manifest_report::{self, report, report_bytes};

pub(super) const arena_bytes: u64 = 131072;
pub(super) const program_code_pages: u64 = 5;
pub(super) const expected_data_frames: usize = 51;
pub(super) const scenarios_count: usize = 10;
const arena_bases: [u64; 2] = [0x804000, 0x828000];
// Exact provisioning pins are independent of manifest identity and policy.
// Each malformed package has its own valid transport pin, so it reaches ring 3.
const sources: [(&[u8], u64, u64, u64); scenarios_count] = [
    (b"opt/anomalous-manifest-0", 856, 0x991225a06ba96b59, 0),
    (b"opt/anomalous-manifest-1", 856, 0x991225a06ba96b59, 1),
    (b"opt/anomalous-manifest-2", 596, 0x771a45936b84d907, 1),
    (b"opt/anomalous-manifest-3", 596, 0x771a45936b84d907, 0),
    (b"opt/anomalous-manifest-4", 8481, 0x350c6643497d1c52, 0),
    (b"opt/anomalous-manifest-5", 596, 0x7716e3936b81fcaa, 1),
    (b"opt/anomalous-manifest-6", 596, 0xa6f696dbe63f7866, 1),
    (b"opt/anomalous-manifest-7", 596, 0xe5724f97d60c1f3f, 1),
    (b"opt/anomalous-manifest-8", 596, 0xf45d095b5821617e, 1),
    (b"opt/anomalous-manifest-9", 1229, 0x880a5f195f25bec8, 1),
];
pub(super) fn scenario(index: usize) -> scenario_profile {
    let &(source_name, edition_length, source_fnv, ceiling) = sources
        .get(index)
        .unwrap_or_else(|| fail("manifest_scenario"));
    if !(1..=66_208).contains(&edition_length) || ceiling > 1 {
        fail("manifest_provisioning_catalog");
    }
    scenario_profile {
        base: arena_bases[index % 2],
        console_handle: 0x62,
        granted_services: service::console_write | service::file_read,
        edition_handle: 0x41,
        edition_length,
        arguments: if ceiling == 0 {
            b"ceiling=0"
        } else {
            b"ceiling=1"
        },
        source_fnv,
        source_name,
    }
}

pub(super) fn validate_report(state: &program) -> [u8; report_bytes] {
    let mut bytes = [0; report_bytes];
    for (word, chunk) in bytes.chunks_exact_mut(8).enumerate() {
        chunk.copy_from_slice(
            &state
                .execution
                .read_arena(0, word as u64 * 8)
                .unwrap_or_else(|| fail("manifest_report_readback"))
                .to_le_bytes(),
        );
    }
    let value = report::decode(&bytes).unwrap_or_else(|_| fail("manifest_report_encoding"));
    let (_, length, _, ceiling) = sources[state.scenario];
    let reads = length.div_ceil(4096);
    let writes = value.console_calls();
    // Only transport and provisioning facts are checked here. The host oracle
    // independently checks semantic outcomes and exact admitted identities.
    if value.ceiling() != ceiling
        || value.image_length() != length
        || value.read_calls() != reads
        || value.read_bytes() != length
        || state.calls != reads + writes + 2
        || state.exits != 1
        || state.invalid != 0
        || state.console_attempts != writes
        || state.console_rejected != 0
        || state.console.written() != writes
        || state.console.bytes_written() != writes * manifest_report::console_marker.len() as u64
        || !state.console.is_closed()
        || state.console_scratch.iter().any(|byte| *byte != 0)
        || state.edition_attempts != reads
        || state.edition_rejected != 0
        || state.edition.reads() != reads
        || state.edition.bytes_read() != length
        || state.edition_source_reads != reads
        || !state.edition.is_closed()
        || state.edition_scratch.iter().any(|byte| *byte != 0)
        || state.allocator_service_borrows != 2
        || state.allocator_nested_denials != 1
    {
        fail("manifest_transport_evidence");
    }
    bytes
}

pub(super) fn emit(
    vendor: &str,
    initial: frame_accounting,
    records: &[program_record; scenarios_count],
) {
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_manifest_test.ready","level":"info","vendor":"{}","tasks":1,"scenarios":10,"code_pages":{},"descriptor_bytes":256,"arena_bytes":131072,"stack_bytes":32768,"mappings":4,"data_frames":{},"table_frames":9,"leases":6,"free_frames":{},"edition_mapped":false}}"#,
        vendor, program_code_pages, expected_data_frames, initial.free
    );
    let mut total_zeroed = 0;
    let mut total_writes = 0;
    for (index, record) in records.iter().enumerate() {
        total_zeroed += record.zeroed_bytes;
        total_writes += record.console_writes;
        if record.granted != record.stated || record.granted[2] != 3 {
            fail("manifest_bootstrap_record");
        }
        let value = report::decode(&record.report).unwrap_or_else(|_| fail("manifest_record"));
        let status = if value.status() == manifest_report::status::admitted {
            "native_policy_admitted"
        } else {
            "native_policy_refused"
        };
        let reason = manifest_report::reason::as_str(value.reason())
            .unwrap_or_else(|| fail("manifest_reason"));
        let _ = write!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_manifest_test.scenario","level":"info","scenario":{},"arena_base":{},"zeroed_bytes":{},"calls":{},"timer_ticks":{},"invalid":{},"exits":{},"bootstrap_services":3,"console_attempts":{},"console_writes":{},"console_rejected":{},"console_bytes":{},"edition_attempts":{},"edition_reads":{},"edition_rejected":{},"edition_read_bytes":{},"edition_source_reads":{},"allocator_service_borrows":{},"allocator_nested_denials":{},"status":"{}","reason":"{}","executed":false,"workload_effects":0,"report_hex":""#,
            index,
            scenario(index).base,
            record.zeroed_bytes,
            record.calls,
            record.timer_ticks,
            record.invalid,
            record.exits,
            record.console_attempts,
            record.console_writes,
            record.console_rejected,
            record.console_bytes,
            record.edition_attempts,
            record.edition_reads,
            record.edition_rejected,
            record.edition_read_bytes,
            record.edition_source_reads,
            record.allocator_service_borrows,
            record.allocator_nested_denials,
            status,
            reason
        );
        for byte in record.report {
            let _ = write!(serial, "{byte:02x}");
        }
        let _ = writeln!(serial, "\"}}");
    }
    if total_writes != 1 || total_zeroed != 1310720 {
        fail("manifest_final_evidence");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_manifest_test.passed","level":"info","program_tasks":10,"descriptor_bytes":256,"arena_bytes":131072,"zeroed_bytes":1310720,"editions_executed":0,"loader_console_writes":1,"source_released":true,"allocator_available":true,"resources_restored":true,"authority_revoked":true,"edition_frame_preserved":true,"source_language":"rust"}}"#
    );
}

// Diagnostic records own no resources. Retain them in BSS so ten full reports
// never become a large bootstrap-stack array or a by-value array temporary.
struct retained_records(UnsafeCell<[core::mem::MaybeUninit<program_record>; scenarios_count]>);
// SAFETY: Sole BSP executes this terminal proof once with interrupts serialized.
unsafe impl Sync for retained_records {}
static record_storage: retained_records = retained_records(UnsafeCell::new(
    [const { core::mem::MaybeUninit::uninit() }; scenarios_count],
));

pub(super) fn run(
    image: program_image,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    initial: frame_accounting,
) -> ! {
    let mut vendor = "";
    let pointer = record_storage.0.get().cast::<program_record>();
    for index in 0..scenarios_count {
        let (current_vendor, record) =
            run_program(image, index, kernel_cr3, bootstrap_stack, initial);
        if index == 0 {
            vendor = current_vendor;
        } else if current_vendor != vendor {
            fail("manifest_cpu_comparison");
        }
        // SAFETY: Each BSS element is written once after its task is fully
        // retired. No reference to this storage spans a guest transfer.
        unsafe {
            pointer.add(index).write(record);
        }
    }
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("manifest_final_interrupts");
    }
    // SAFETY: All elements initialized above, no writers or live tasks remain.
    let initialized = unsafe { &*pointer.cast::<[program_record; scenarios_count]>() };
    emit(vendor, initial, initialized);
    super::super::super::qemu_exit_success();
    super::super::super::halt_forever()
}
