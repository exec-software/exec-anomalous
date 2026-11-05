//! ELF framing comparison over the unchanged native document personality.
//! Only an inert, exactly bound plan survives the separate loader lifetime.

use super::*;
use anomalous_elf_adapter::load_plan;

unsafe extern "C" {
    static x86_64_elf_document_data_start: u8;
    static x86_64_elf_document_data_end: u8;
    static x86_64_elf_document_elf_start: u8;
    static x86_64_elf_document_elf_end: u8;
    static x86_64_elf_document_loader_start: u8;
    static x86_64_elf_document_loader_end: u8;
    static x86_64_elf_document_loader_fault_instruction: u8;
}

fn loader_image() -> artifact {
    let bytes = elf_profile::span(
        address!(x86_64_elf_document_loader_start),
        address!(x86_64_elf_document_loader_end),
        code_pages * 4096,
    );
    let fault = address!(x86_64_elf_document_loader_fault_instruction);
    let offset = fault
        .checked_sub(user_code_base)
        .unwrap_or_else(|| fail("elf_document_loader_fault")) as usize;
    if bytes.get(offset..offset.saturating_add(2)) != Some(&[0x0f, 0x0b]) {
        fail("elf_document_loader_fault");
    }
    artifact {
        source: bytes.as_ptr() as u64,
        length: bytes.len() as u64,
        fault,
        data: (0, 0),
        loader: true,
    }
}

fn application_images(plan: load_plan) -> [runtime_image; 2] {
    let mut native = runtime_image::inspect();
    let rw = elf_profile::span(
        address!(x86_64_elf_document_data_start),
        address!(x86_64_elf_document_data_end),
        8192,
    );
    let elf = elf_profile::span(
        address!(x86_64_elf_document_elf_start),
        address!(x86_64_elf_document_elf_end),
        65536,
    );
    if plan != elf_profile::expected_plan()
        || plan.image_length() != elf.len() as u64
        || plan.entry() != user_code_base
        || plan.rx().virtual_address() != user_code_base
        || plan.rx().file_size() != native.length
        || plan.rx().memory_size() != native.length
        || plan.rw().virtual_address() != user_data_base
        || plan.rw().file_size() != 128
        || plan.rw().memory_size() != descriptor_offset + startup_size as u64
        || rw.len() as u64 != plan.rw().memory_size()
        || rw.iter().any(|byte| *byte != 0)
    {
        fail("elf_document_geometry");
    }
    let part = |offset: u64, length: u64| {
        let end = offset
            .checked_add(length)
            .unwrap_or_else(|| fail("elf_document_part"));
        elf.get(offset as usize..end as usize)
            .unwrap_or_else(|| fail("elf_document_part"))
    };
    let rx = part(plan.rx().file_offset(), plan.rx().file_size());
    let data = part(plan.rw().file_offset(), plan.rw().file_size());
    let native_rx = elf_profile::span(
        native.source,
        native.source + native.length,
        code_pages * 4096,
    );
    // The compact fixture elides only the original all-zero RW suffix. Compare
    // the whole owned data extent, including declared zero-fill and page slack,
    // before either route receives its separately authorized startup bytes.
    if rx != native_rx
        || (0..8192).any(|offset| {
            rw.get(offset).copied().unwrap_or(0) != data.get(offset).copied().unwrap_or(0)
        })
    {
        fail("elf_document_segment_comparison");
    }
    native.data = (rw.as_ptr() as u64, rw.len() as u64);
    [
        native,
        runtime_image {
            source: rx.as_ptr() as u64,
            length: rx.len() as u64,
            data: (data.as_ptr() as u64, data.len() as u64),
            ..native
        },
    ]
}

pub(super) fn run() -> ! {
    if super::super::is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("elf_document_initial_state");
    }
    let baseline_image = artifact::inspect();
    let loader_image = loader_image();
    // SAFETY: Sole bootstrap owner consumes the unique framebuffer aperture.
    let info = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("framebuffer_unavailable"));
    // SAFETY: Unique withdrawn aperture remains owned through both routes.
    let mut presenter = unsafe { framebuffer_presenter::try_new(info) }
        .unwrap_or_else(|_| fail("framebuffer_geometry"));
    if presenter.width() != 1024 || presenter.height() != 768 {
        fail("display_profile");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Initialized sole-BSP descriptor state is exclusively owned.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let outside = framebuffer_hash(info, true);
    let zeroed_bytes = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let (loader_vendor, loader) = run_owned(
            allocator,
            loader_image,
            kernel_cr3,
            bootstrap_stack,
            0,
            initial,
        );
        // run_owned restores bootstrap state, withdraws both contexts and
        // reclaims every loader lease before any document task is allocated.
        let images = application_images(
            loader
                .accepted_plan
                .unwrap_or_else(|| fail("elf_document_no_plan")),
        );
        let (vendor, normal) = run_owned(
            allocator,
            baseline_image,
            kernel_cr3,
            bootstrap_stack,
            0,
            initial,
        );
        // SAFETY: Both baseline tasks retired; only copied kernel rows remain.
        let patch = presenter
            .prepare_rgb8_region(32, 32, stage_width, stage_height, unsafe {
                &*display_stage.0.get()
            })
            .unwrap_or_else(|_| fail("elf_document_baseline_patch"));
        if vendor != loader_vendor
            || !patch.commit_and_verify()
            || framebuffer_hash(info, true) != outside
        {
            fail("elf_document_baseline_pixels");
        }
        clear_stage();
        let mut presented = framebuffer_hash(info, false);
        let (fault_vendor, faulted) = run_owned(
            allocator,
            baseline_image,
            kernel_cr3,
            bootstrap_stack,
            1,
            initial,
        );
        if fault_vendor != vendor || framebuffer_hash(info, false) != presented {
            fail("elf_document_baseline_fault");
        }
        clear_stage();
        emit_evidence(initial, vendor, [normal, faulted]);
        emit_ready(loader_image, loader);
        let mut zeroed_bytes = 0;
        let mut first_surfaces = [0; packet_limit];
        for scenario in execution_order {
            let mut baseline = runtime_record::default();
            for (route, image) in images.into_iter().enumerate() {
                let (current_vendor, record) = run_arena_pair(
                    allocator,
                    image,
                    scenario,
                    kernel_cr3,
                    bootstrap_stack,
                    initial,
                );
                if current_vendor != vendor {
                    fail("elf_document_cpu_comparison");
                }
                if matches!(scenario, 0 | 3) {
                    if route == 0
                        && scenario == 3
                        && record.application.packet_hashes[0] == first_surfaces
                    {
                        fail("elf_document_distinct_surfaces");
                    }
                    // SAFETY: Every task is retired before presentation; no
                    // application can mutate the copied staging pixels now.
                    let patch = presenter
                        .prepare_rgb8_region(32, 32, stage_width, stage_height, unsafe {
                            &*display_stage.0.get()
                        })
                        .unwrap_or_else(|_| fail("elf_document_patch"));
                    if (route == 1 && !patch.matches_pixels())
                        || (route == 0 && scenario == 3 && patch.matches_pixels())
                    {
                        fail("elf_document_pixel_comparison");
                    }
                    if !patch.commit_and_verify() {
                        fail("elf_document_pixel_readback");
                    }
                    presented = framebuffer_hash(info, false);
                }
                clear_stage();
                if framebuffer_hash(info, true) != outside
                    || framebuffer_hash(info, false) != presented
                {
                    fail("elf_document_preserved_pixels");
                }
                let mut comparable = record;
                comparable.application.timer_ticks = 0;
                if route == 0 {
                    baseline = comparable;
                    if scenario == 0 {
                        first_surfaces = record.application.packet_hashes[0];
                    }
                } else if baseline != comparable {
                    fail("elf_document_route_comparison");
                }
                zeroed_bytes += record.zeroed_bytes;
                emit_record(route, scenario, record);
            }
        }
        zeroed_bytes
    })
    .unwrap_or_else(|error| fail(error.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() || zeroed_bytes != 524288 {
        fail("elf_document_final_state");
    }
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.elf_document_test.passed","level":"info","routes":2,"application_tasks":16,"loader_tasks":2,"service_lifetimes":8,"normal_runs":4,"contained_faults":4,"zeroed_bytes":{},"hashes_equal":true,"pixels_equal":true,"resources_restored":true,"authority_revoked":true,"source_language":"rust","store_scope":"service_lifetime","personality":"native_int_f4"}}"#,
        zeroed_bytes
    );
    super::super::super::qemu_exit_success();
    super::super::super::halt_forever()
}

fn emit_ready(image: artifact, loader: evidence) {
    elf_profile::verify_loader(image, loader);
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.elf_document_test.ready","level":"info","tasks":2,"routes":2,"scenarios":4,"channels":4,"descriptor_bytes":128,"arena_bytes":32768,"mappings_per_task":4,"data_frames":48,"table_frames":18,"leases":12}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.elf_document_test.loaded","level":"info","loader_tasks":2,"calls":{},"denials":{},"bad_copies":{},"invalid":{},"accepted":1,"plan_bytes":96,"rw_file_bytes":128,"rw_memory_bytes":4224,"elided_zero_bytes":4096,"loader_reclaimed":true,"expanded_rw_equal":true}}"#,
        loader.calls, loader.denials, loader.bad_copies, loader.invalid
    );
}

fn emit_record(route: usize, scenario: usize, record: runtime_record) {
    let mut serial = serial_port::com1();
    let e = record.application;
    let fault_kind = match scenario {
        1 => "before_commit",
        2 => "after_commit",
        _ => "none",
    };
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.elf_document_test.scenario","level":"info","route":{},"scenario":{},"starts":{},"zeroed_bytes":{},"calls":{},"timer_ticks":{},"cooperative_switches":{},"surfaces":{},"inputs":{},"requests":{},"replies":{},"surface_deliveries":{},"input_deliveries":{},"request_deliveries":{},"reply_deliveries":{},"surface_acks":{},"input_acks":{},"request_acks":{},"reply_acks":{},"denials":{},"bad_copies":{},"invalid":{},"closed":{},"discarded":{},"rows":{},"exits":{},"faults":{},"fault_kind":"{}","arena_copies":{},"resources_restored":true}}"#,
        route,
        scenario,
        record.starts,
        record.zeroed_bytes,
        e.calls,
        e.timer_ticks,
        e.switches,
        e.sends[0],
        e.sends[1],
        e.sends[2],
        e.sends[3],
        e.deliveries[0],
        e.deliveries[1],
        e.deliveries[2],
        e.deliveries[3],
        e.acknowledgements[0],
        e.acknowledgements[1],
        e.acknowledgements[2],
        e.acknowledgements[3],
        e.denials,
        e.bad_copies,
        e.invalid,
        e.closed,
        e.discarded,
        e.rows,
        e.exits,
        e.faults,
        fault_kind,
        record.arena_copies
    );
    for (index, hash) in e.packet_hashes[0][..e.sends[0] as usize].iter().enumerate() {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.elf_document_test.surface","level":"info","route":{},"scenario":{},"sequence":{},"hash":{}}}"#,
            route,
            scenario,
            index + 1,
            hash
        );
    }
    for channel in 2..channel_count {
        for (index, hash) in e.packet_hashes[channel][..e.sends[channel] as usize]
            .iter()
            .enumerate()
        {
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"kernel.elf_document_test.packet","level":"info","route":{},"scenario":{},"channel":{},"sequence":{},"hash":{}}}"#,
                route,
                scenario,
                channel,
                index + 1,
                hash
            );
        }
    }
}
