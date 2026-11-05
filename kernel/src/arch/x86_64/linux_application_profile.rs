//! Two real entry conventions over the same retained owners and native service.
//! Linux syscall policy is intentionally a feature-private proof controller;
//! this does not qualify a production userspace veil or stock Linux executable.
use super::*;
use anomalous_elf_adapter::load_plan;
use elf_profile::span;

unsafe extern "C" {
    #[cfg(not(feature = "linux_tls_test"))]
    static x86_64_linux_application_payload_start: u8;
    #[cfg(not(feature = "linux_tls_test"))]
    static x86_64_linux_application_payload_end: u8;
    #[cfg(not(feature = "linux_tls_test"))]
    static x86_64_linux_application_user_fault_instruction: u8;
    #[cfg(not(feature = "linux_tls_test"))]
    static x86_64_linux_application_data_start: u8;
    #[cfg(not(feature = "linux_tls_test"))]
    static x86_64_linux_application_data_end: u8;
    #[cfg(not(feature = "linux_tls_test"))]
    static x86_64_linux_application_elf_start: u8;
    #[cfg(not(feature = "linux_tls_test"))]
    static x86_64_linux_application_elf_end: u8;
    static x86_64_linux_application_plan_start: u8;
    static x86_64_linux_application_plan_end: u8;
    #[cfg(not(feature = "veil_application_test"))]
    static x86_64_linux_application_loader_start: u8;
    #[cfg(not(feature = "veil_application_test"))]
    static x86_64_linux_application_loader_end: u8;
    #[cfg(not(feature = "veil_application_test"))]
    static x86_64_linux_application_loader_fault_instruction: u8;
}

pub(super) fn expected_plan() -> load_plan {
    load_plan::decode(span(
        address!(x86_64_linux_application_plan_start),
        address!(x86_64_linux_application_plan_end),
        96,
    ))
    .unwrap_or_else(|_| fail("linux_manifest"))
}

#[cfg(not(feature = "veil_application_test"))]
fn loader_image() -> artifact {
    let bytes = span(
        address!(x86_64_linux_application_loader_start),
        address!(x86_64_linux_application_loader_end),
        code_pages * 4096,
    );
    let fault = address!(x86_64_linux_application_loader_fault_instruction);
    check_fault(bytes, fault);
    artifact {
        source: bytes.as_ptr() as u64,
        length: bytes.len() as u64,
        fault,
        data: (0, 0),
        loader: true,
        linux: true,
    }
}
#[cfg(not(feature = "linux_tls_test"))]
fn check_fault(bytes: &[u8], fault: u64) {
    let offset = fault
        .checked_sub(user_code_base)
        .unwrap_or_else(|| fail("linux_fault_marker")) as usize;
    if bytes.get(offset..offset.saturating_add(2)) != Some(&[0x0f, 0x0b]) {
        fail("linux_fault_marker");
    }
}
#[cfg(not(feature = "linux_tls_test"))]
pub(super) fn application_image(plan: load_plan) -> artifact {
    let elf = span(
        address!(x86_64_linux_application_elf_start),
        address!(x86_64_linux_application_elf_end),
        65536,
    );
    let rx = span(
        address!(x86_64_linux_application_payload_start),
        address!(x86_64_linux_application_payload_end),
        code_pages * 4096,
    );
    let rw = span(
        address!(x86_64_linux_application_data_start),
        address!(x86_64_linux_application_data_end),
        8192,
    );
    if plan != expected_plan()
        || plan.entry() != user_code_base
        || plan.image_length() != elf.len() as u64
        || plan.rx().file_size() != rx.len() as u64
        || plan.rw().file_size() != rw.len() as u64
        || plan.rw().memory_size() != 8192
        || rw.get(..128).is_none_or(|s| s.iter().any(|b| *b != 0))
    {
        fail("linux_image_geometry");
    }
    let part = |offset: u64, length: u64| {
        let end = offset
            .checked_add(length)
            .unwrap_or_else(|| fail("linux_segment"));
        elf.get(offset as usize..end as usize)
            .unwrap_or_else(|| fail("linux_segment"))
    };
    let rx_loaded = part(plan.rx().file_offset(), plan.rx().file_size());
    let rw_loaded = part(plan.rw().file_offset(), plan.rw().file_size());
    if rx_loaded != rx || rw_loaded != rw {
        fail("linux_segment_comparison");
    }
    let fault = address!(x86_64_linux_application_user_fault_instruction);
    check_fault(rx_loaded, fault);
    artifact {
        source: rx_loaded.as_ptr() as u64,
        length: rx_loaded.len() as u64,
        fault,
        data: (rw_loaded.as_ptr() as u64, rw_loaded.len() as u64),
        loader: false,
        linux: true,
    }
}

pub(super) fn populate_stack<const N: usize>(
    execution: &mut super::super::owned_execution::owned_execution_group<N>,
    index: usize,
    scenario: usize,
) {
    let rsp = user_stack_top - 256;
    let mut bytes = [0u8; 256];
    // Minimal declared startup: argc, argv[], empty envp[], and bounded auxv.
    let words = [
        2u64,
        rsp + 128,
        rsp + 160,
        0,
        0,
        6,
        4096,
        9,
        user_code_base,
        0,
        0,
    ];
    for (slot, word) in words.into_iter().enumerate() {
        bytes[slot * 8..slot * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }
    bytes[128..145].copy_from_slice(b"anomalous-editor\0");
    let name: &[u8] = if scenario == 0 {
        b"normal\0"
    } else {
        b"fault\0"
    };
    bytes[160..160 + name.len()].copy_from_slice(name);
    for (slot, word) in bytes.chunks_exact(8).enumerate() {
        let word = u64::from_le_bytes(word.try_into().unwrap());
        let offset = 8192 - 256 + slot as u64 * 8;
        execution
            .write_stack(index, offset, word)
            .unwrap_or_else(|e| fail(e));
        if execution.read_stack(index, offset) != Some(word) {
            fail("linux_stack_readback");
        }
    }
    let frame = execution.frame(index).unwrap_or_else(|e| fail(e));
    // SAFETY: Initial retained supervisor frame before task publication; owner
    // admission selected exactly this aligned user stack and initialized GPRs.
    unsafe {
        if (*frame).stack_pointer != rsp || (*frame).frame_pointer != 0 || (*frame).data != 0 {
            fail("linux_initial_frame");
        }
        (*frame).flags = 0x202;
    }
    bytes.fill(0);
}

#[cfg(not(feature = "veil_application_test"))]
fn comparable(mut e: evidence) -> evidence {
    e.calls = 0;
    e.timer_ticks = 0;
    e.denials = 0;
    e.bad_copies = 0;
    e.invalid = 0;
    e.linux_calls = [0; 6];
    e
}

#[cfg(not(feature = "veil_application_test"))]
pub(super) fn run() -> ! {
    if is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("linux_initial_state");
    }
    // SAFETY: Sole bootstrap owner consumes the unique console aperture.
    let info = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("framebuffer_unavailable"));
    // SAFETY: Registration transferred the only live aperture grant above.
    let mut presenter = unsafe { framebuffer_presenter::try_new(info) }
        .unwrap_or_else(|_| fail("framebuffer_geometry"));
    if presenter.width() != 1024 || presenter.height() != 768 {
        fail("display_profile");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Sole BSP, initialized TSS with IRQ/NMI excluded.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let outside = framebuffer_hash(info, true);
    let (initial, vendor, baseline, records) = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let (vendor, loader) = run_owned(
            allocator,
            loader_image(),
            kernel_cr3,
            bootstrap_stack,
            0,
            initial,
        );
        let linux = application_image(
            loader
                .accepted_plan
                .unwrap_or_else(|| fail("linux_no_plan")),
        );
        let native = elf_profile::application_images(elf_profile::expected_plan())[0];
        let mut baseline = [evidence::default(); 2];
        let mut records = baseline;
        let mut baseline_pixels = 0;
        for (route, producer) in [native, linux].into_iter().enumerate() {
            let (normal_vendor, normal) = run_pair(
                allocator,
                [producer, native],
                kernel_cr3,
                bootstrap_stack,
                0,
                initial,
            );
            if normal_vendor != vendor
                || normal.rows != stage_height as u64
                || framebuffer_hash(info, true) != outside
            {
                fail("linux_presentation_preflight");
            }
            // SAFETY: Every task retired; only bootstrap reads the copied rows.
            let pixels = unsafe { &*display_stage.0.get() };
            let patch = presenter
                .prepare_rgb8_region(32, 32, stage_width, stage_height, pixels)
                .unwrap_or_else(|_| fail("display_patch"));
            if route == 1 && !patch.matches_pixels() {
                fail("linux_pixel_comparison");
            }
            if !patch.commit_and_verify() || framebuffer_hash(info, true) != outside {
                fail("framebuffer_readback");
            }
            clear_stage();
            let presented = framebuffer_hash(info, false);
            let (fault_vendor, faulted) = run_pair(
                allocator,
                [producer, native],
                kernel_cr3,
                bootstrap_stack,
                1,
                initial,
            );
            if fault_vendor != vendor || framebuffer_hash(info, false) != presented {
                fail("fault_changed_presentation");
            }
            clear_stage();
            if route == 0 {
                baseline = [normal, faulted];
                baseline_pixels = presented;
            } else if baseline.map(comparable) != [normal, faulted].map(comparable)
                || baseline_pixels != presented
            {
                fail("linux_route_comparison");
            }
            records = [normal, faulted];
        }
        (initial, vendor, baseline, records)
    })
    .unwrap_or_else(|e| fail(e.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("final_interrupt_state");
    }
    emit_evidence(initial, vendor, baseline);
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_application_test.loaded","level":"info","loader_tasks":2,"calls":15,"denials":1,"bad_copies":1,"invalid":8,"accepted":1,"plan_bytes":96,"initialized_rw":true,"zero_fill_verified":true,"loader_reclaimed":true}}"#
    );
    for (scenario, e) in records.iter().enumerate() {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.linux_application_test.scenario","level":"info","scenario":{},"calls":{},"cooperative_switches":{},"timer_ticks":{},"surface_sends":{},"input_sends":{},"surface_deliveries":{},"input_deliveries":{},"surface_acks":{},"input_acks":{},"denials":{},"bad_copies":{},"invalid":{},"closed":{},"discarded":{},"exits":{},"faults":{},"rows":{},"sys_read":{},"sys_write":{},"sys_poll":{},"sys_yield":{},"sys_exit":{},"sys_unknown":{}}}"#,
            scenario,
            e.calls,
            e.switches,
            e.timer_ticks,
            e.sends[0],
            e.sends[1],
            e.deliveries[0],
            e.deliveries[1],
            e.acknowledgements[0],
            e.acknowledgements[1],
            e.denials,
            e.bad_copies,
            e.invalid,
            e.closed,
            e.discarded,
            e.exits,
            e.faults,
            e.rows,
            e.linux_calls[0],
            e.linux_calls[1],
            e.linux_calls[2],
            e.linux_calls[3],
            e.linux_calls[4],
            e.linux_calls[5]
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_application_test.passed","level":"info","routes":2,"application_tasks":8,"loader_tasks":2,"surfaces_per_route":7,"inputs_per_route":6,"hashes_equal":true,"pixels_equal":true,"resources_restored":true,"personality":"linux_syscall_native_int_f4"}}"#
    );
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
