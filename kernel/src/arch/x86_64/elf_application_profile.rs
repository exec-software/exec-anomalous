//! Experimental controller for a userspace ELF adapter. The kernel accepts only
//! an inert plan matching the independently inspected, immutable host manifest.
//! File metadata never creates identities, executable permissions, or grants.

use super::*;
use anomalous_elf_adapter::load_plan;

unsafe extern "C" {
    #[cfg(all(feature = "elf_relaunch_test", not(feature = "linux_application_test")))]
    static x86_64_elf_relaunch_plan_start: u8;
    #[cfg(all(feature = "elf_relaunch_test", not(feature = "linux_application_test")))]
    static x86_64_elf_relaunch_plan_end: u8;
    #[cfg(any(
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test")),
        feature = "linux_application_test"
    ))]
    static x86_64_elf_application_data_start: u8;
    #[cfg(any(
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test")),
        feature = "linux_application_test"
    ))]
    static x86_64_elf_application_data_end: u8;
    #[cfg(any(
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test")),
        feature = "linux_application_test"
    ))]
    static x86_64_elf_application_elf_start: u8;
    #[cfg(any(
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test")),
        feature = "linux_application_test"
    ))]
    static x86_64_elf_application_elf_end: u8;
    #[cfg(any(
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test")),
        feature = "linux_application_test"
    ))]
    static x86_64_elf_application_plan_start: u8;
    #[cfg(any(
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test")),
        feature = "linux_application_test"
    ))]
    static x86_64_elf_application_plan_end: u8;
    #[cfg(all(
        feature = "elf_document_test",
        not(feature = "elf_relaunch_test"),
        not(feature = "linux_application_test")
    ))]
    static x86_64_elf_document_plan_start: u8;
    #[cfg(all(
        feature = "elf_document_test",
        not(feature = "elf_relaunch_test"),
        not(feature = "linux_application_test")
    ))]
    static x86_64_elf_document_plan_end: u8;
    #[cfg(all(
        not(feature = "linux_application_test"),
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test"))
    ))]
    static x86_64_elf_application_loader_start: u8;
    #[cfg(all(
        not(feature = "linux_application_test"),
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test"))
    ))]
    static x86_64_elf_application_loader_end: u8;
    #[cfg(all(
        not(feature = "linux_application_test"),
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test"))
    ))]
    static x86_64_elf_application_loader_fault_instruction: u8;
}

pub(super) fn span(start: u64, end: u64, maximum: u64) -> &'static [u8] {
    let length = end.checked_sub(start).unwrap_or_else(|| fail("elf_span"));
    if start < address!(kernel_rodata_start)
        || end > address!(kernel_rodata_end)
        || length == 0
        || length > maximum
    {
        fail("elf_span");
    }
    // SAFETY: Complete nonempty interval lies in immutable linked kernel rodata.
    unsafe { core::slice::from_raw_parts(start as *const u8, length as usize) }
}

pub(super) fn expected_plan() -> load_plan {
    #[cfg(all(feature = "elf_relaunch_test", not(feature = "linux_application_test")))]
    let bytes = span(
        address!(x86_64_elf_relaunch_plan_start),
        address!(x86_64_elf_relaunch_plan_end),
        96,
    );
    #[cfg(all(
        feature = "elf_document_test",
        not(feature = "elf_relaunch_test"),
        not(feature = "linux_application_test")
    ))]
    let bytes = span(
        address!(x86_64_elf_document_plan_start),
        address!(x86_64_elf_document_plan_end),
        96,
    );
    #[cfg(any(
        not(any(feature = "elf_document_test", feature = "elf_relaunch_test")),
        feature = "linux_application_test"
    ))]
    let bytes = span(
        address!(x86_64_elf_application_plan_start),
        address!(x86_64_elf_application_plan_end),
        96,
    );
    load_plan::decode(bytes).unwrap_or_else(|_| fail("elf_manifest"))
}

fn manifest_for(image: artifact) -> load_plan {
    #[cfg(feature = "linux_application_test")]
    if image.linux {
        return linux_profile::expected_plan();
    }
    let _ = image;
    expected_plan()
}

pub(super) fn offer_plan(state: &mut runtime, value: scheduler_frame) -> u64 {
    // Role is a controller-owned identity assignment, never decoded from ELF.
    if !state.image.loader || state.selected != 0 {
        return 2;
    }
    if state.evidence.accepted_plan.is_some() || value.data != 96 {
        return 4;
    }
    let mut bytes = [0; 96];
    if state
        .execution
        .copy_from_user(
            &state.tasks,
            &state.circles,
            &state.scheduler,
            state.identities[state.selected],
            value.source_index,
            &mut bytes,
        )
        .is_err()
    {
        return 3;
    }
    let Ok(plan) = load_plan::decode(&bytes) else {
        return 4;
    };
    if plan != manifest_for(state.image) {
        return 4;
    }
    state.evidence.accepted_plan = Some(plan);
    0
}

pub(super) fn verify_loader(image: artifact, record: evidence) {
    if record.calls != 15
        || record.denials != 1
        || record.bad_copies != 1
        || record.invalid != 8
        || record.exits != 2
        || record.faults != 0
        || record.switches != 0
        || record.sends != [0; 2]
        || record.deliveries != [0; 2]
        || record.acknowledgements != [0; 2]
        || record.closed != 0
        || record.discarded != 0
        || record.rows != 0
        || record.surface_hashes != [0; 7]
        || record.accepted_plan != Some(manifest_for(image))
    {
        fail("elf_loader_evidence");
    }
}

pub(super) fn data_word(image: artifact, offset: u64, default: u64) -> u64 {
    if image.data.1 == 0 {
        return default;
    }
    let bytes = span(
        image.data.0,
        image
            .data
            .0
            .checked_add(image.data.1)
            .unwrap_or_else(|| fail("elf_data_span")),
        8192,
    );
    let mut word = [0; 8];
    for (index, byte) in word.iter_mut().enumerate() {
        *byte = bytes.get(offset as usize + index).copied().unwrap_or(0);
    }
    u64::from_le_bytes(word)
}

#[cfg(all(
    not(feature = "linux_application_test"),
    not(any(feature = "elf_document_test", feature = "elf_relaunch_test"))
))]
fn loader_image() -> artifact {
    let bytes = span(
        address!(x86_64_elf_application_loader_start),
        address!(x86_64_elf_application_loader_end),
        code_pages * 4096,
    );
    let fault = address!(x86_64_elf_application_loader_fault_instruction);
    let offset = fault
        .checked_sub(user_code_base)
        .unwrap_or_else(|| fail("elf_loader_fault")) as usize;
    if bytes.get(offset..offset.saturating_add(2)) != Some(&[0x0f, 0x0b]) {
        fail("elf_loader_fault");
    }
    artifact {
        source: bytes.as_ptr() as u64,
        length: bytes.len() as u64,
        fault,
        data: (0, 0),
        loader: true,
        #[cfg(feature = "linux_application_test")]
        linux: false,
    }
}

#[cfg(any(
    not(any(feature = "elf_document_test", feature = "elf_relaunch_test")),
    feature = "linux_application_test"
))]
pub(super) fn application_images(plan: load_plan) -> [artifact; 2] {
    let mut native = artifact::inspect();
    let rw = span(
        address!(x86_64_elf_application_data_start),
        address!(x86_64_elf_application_data_end),
        8192,
    );
    let elf = span(
        address!(x86_64_elf_application_elf_start),
        address!(x86_64_elf_application_elf_end),
        65536,
    );
    if plan != expected_plan()
        || plan.image_length() != elf.len() as u64
        || plan.entry() != user_code_base
        || plan.rx().file_size() != native.length
        || plan.rw().file_size() != rw.len() as u64
        || rw.get(..128).is_none_or(|s| s.iter().any(|b| *b != 0))
    {
        fail("elf_application_geometry");
    }
    let part = |offset: u64, length: u64| {
        let end = offset
            .checked_add(length)
            .unwrap_or_else(|| fail("elf_part"));
        elf.get(offset as usize..end as usize)
            .unwrap_or_else(|| fail("elf_part"))
    };
    let rx = part(plan.rx().file_offset(), plan.rx().file_size());
    let data = part(plan.rw().file_offset(), plan.rw().file_size());
    let native_rx = span(
        native.source,
        native.source + native.length,
        code_pages * 4096,
    );
    if rx != native_rx || data != rw {
        fail("elf_segment_comparison");
    }
    native.data = (rw.as_ptr() as u64, rw.len() as u64);
    [
        native,
        artifact {
            source: rx.as_ptr() as u64,
            length: rx.len() as u64,
            data: (data.as_ptr() as u64, data.len() as u64),
            ..native
        },
    ]
}

#[cfg(all(
    not(feature = "linux_application_test"),
    not(any(feature = "elf_document_test", feature = "elf_relaunch_test"))
))]
pub(super) fn run() -> ! {
    if is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("elf_initial_state");
    }
    // SAFETY: Sole BSP consumes the console's unique aperture once.
    let info = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("framebuffer_unavailable"));
    // SAFETY: The withdrawn console registration grants this unique presenter.
    let mut presenter = unsafe { framebuffer_presenter::try_new(info) }
        .unwrap_or_else(|_| fail("framebuffer_geometry"));
    if presenter.width() != 1024 || presenter.height() != 768 {
        fail("display_profile");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Sole BSP reads the initialized descriptor state.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let outside = framebuffer_hash(info, true);
    let (initial, vendor, records, loader) = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let (loader_vendor, loader) = run_owned(
            allocator,
            loader_image(),
            kernel_cr3,
            bootstrap_stack,
            0,
            initial,
        );
        // run_owned has withdrawn all loader task contexts, roots and leases.
        // Only this inert value survives before any application allocation.
        let images =
            application_images(loader.accepted_plan.unwrap_or_else(|| fail("elf_no_plan")));
        let mut baseline = [evidence::default(); 2];
        let mut baseline_pixels = 0;
        let mut final_records = baseline;
        for (route, image) in images.into_iter().enumerate() {
            let (vendor, normal) =
                run_owned(allocator, image, kernel_cr3, bootstrap_stack, 0, initial);
            if vendor != loader_vendor
                || normal.rows != stage_height as u64
                || framebuffer_hash(info, true) != outside
            {
                fail("elf_presentation_preflight");
            }
            // SAFETY: Both tasks retired; sole bootstrap owner reads copied rows.
            let pixels = unsafe { &*display_stage.0.get() };
            let patch = presenter
                .prepare_rgb8_region(32, 32, stage_width, stage_height, pixels)
                .unwrap_or_else(|_| fail("display_patch"));
            // Compare every destination byte to the existing native frame
            // before writing the ELF route, without another large buffer.
            if route == 1 && !patch.matches_pixels() {
                fail("elf_pixel_comparison");
            }
            if !patch.commit_and_verify() || framebuffer_hash(info, true) != outside {
                fail("framebuffer_readback");
            }
            clear_stage();
            let presented = framebuffer_hash(info, false);
            let (fault_vendor, faulted) =
                run_owned(allocator, image, kernel_cr3, bootstrap_stack, 1, initial);
            if fault_vendor != vendor || framebuffer_hash(info, false) != presented {
                fail("fault_changed_presentation");
            }
            clear_stage();
            let mut comparable = [normal, faulted];
            for record in &mut comparable {
                record.timer_ticks = 0;
            }
            if route == 0 {
                baseline = comparable;
                baseline_pixels = presented;
            } else if baseline != comparable || baseline_pixels != presented {
                fail("elf_route_comparison");
            }
            final_records = [normal, faulted];
        }
        (initial, loader_vendor, final_records, loader)
    })
    .unwrap_or_else(|error| fail(error.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("final_interrupt_state");
    }
    emit_evidence(initial, vendor, records);
    verify_loader(loader_image(), loader);
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.elf_application_test.loaded","level":"info","loader_tasks":2,"calls":15,"denials":1,"bad_copies":1,"invalid":8,"accepted":1,"plan_bytes":96,"initialized_rw":true,"zero_fill_verified":true,"loader_reclaimed":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.elf_application_test.passed","level":"info","routes":2,"application_tasks":8,"loader_tasks":2,"surfaces_per_route":7,"inputs_per_route":6,"hashes_equal":true,"pixels_equal":true,"resources_restored":true,"personality":"native_int_f4"}}"#
    );
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
