#![allow(non_camel_case_types, non_upper_case_globals)]
// The semantic-surface presentation terminal proof has deliberate highest
// precedence in combined-feature lint builds, followed by the transport proof.
// Dispatcher/workspace checks also retain dormant workspace helpers for type
// checking while selecting the dispatcher. Single-feature checks remain strict.
#![cfg_attr(
    all(
        any(
            feature = "input_wait_test",
            feature = "display_vessel_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "semantic_surface_presentation_test",
            all(feature = "dispatcher_test", feature = "workspace_dev")
        ),
        not(test)
    ),
    allow(dead_code)
)]
#![cfg_attr(
    all(
        any(
            feature = "display_vessel_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "semantic_surface_presentation_test",
            all(feature = "dispatcher_test", feature = "workspace_dev")
        ),
        not(test)
    ),
    allow(unused_imports)
)]
#![no_std]

#[cfg(any(
    test,
    feature = "native_application_test",
    feature = "mixed_workspace_test"
))]
mod application_channel;
mod arch;
mod boot_omen;
mod circle;
mod console;
#[cfg(feature = "desktop_dev")]
mod desktop_dev;
#[cfg(any(
    test,
    feature = "workspace_dev",
    all(
        feature = "display_vessel_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ),
    all(
        feature = "supervised_renderer_test",
        not(feature = "semantic_surface_transport_test")
    ),
    feature = "semantic_surface_presentation_test",
    feature = "native_application_test"
))]
mod display;
mod event;
mod identity;
mod input;
#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
mod interrupt_test;
mod ipc;
mod memory;
mod rite;
mod scheduler;
#[cfg(any(test, feature = "semantic_surface_transport_test"))]
mod semantic_surface;
#[cfg(any(test, feature = "input_wait_test"))]
mod session_input;
mod shell;
mod sigil;
mod supervision;
mod task;
mod time;

use anomalous_abi::{abi_major, abi_minor};
use boot_omen::boot_omen as boot_omen_record;
use core::fmt::Write;
#[cfg(not(test))]
use core::panic::PanicInfo;
use core::ptr;

// Make the automated smoke image cross stage 2's 32 KiB disk-read boundary so
// every CI boot exercises multi-chunk kernel loading. Normal images omit it.
#[cfg(feature = "smoke_test")]
#[used]
#[unsafe(link_section = ".rodata.loader_chunk_probe")]
static loader_chunk_probe: [u8; 40 * 1024] = [0xa5; 40 * 1024];

#[unsafe(no_mangle)]
/// Enter the Rust kernel using the handoff created by Threshold.
///
/// # Safety
///
/// `omen_address` must point to a live, naturally aligned `boot_omen_record`
/// owned by the boot loader. The pointed-to record and referenced buffers must
/// remain readable until the kernel establishes its own memory manager. Any
/// advertised framebuffer must name its complete writable aperture, and the
/// loader must transfer exclusive software write access for the kernel's use.
pub unsafe extern "C" fn kernel_main(omen_address: *const boot_omen_record) -> ! {
    let mut serial = console::serial_port::com1();
    serial.initialize();

    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"boot.kernel.enter","level":"info","arch":"x86_64"}}"#
    );

    let fault_path = arch::initialize_fault_path();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.fault_path.ready","level":"info","vectors":{},"tss":true,"double_fault_ist":{},"nmi_ist":{},"nmi_masked":{}}}"#,
        fault_path.vector_count,
        fault_path.double_fault_ist,
        fault_path.nmi_ist,
        fault_path.nmi_masked
    );

    if omen_address.is_null() {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"boot.omen.invalid","level":"fatal","reason":"null"}}"#
        );
        console::draw_memory_failure();
        arch::qemu_exit_failure();
        arch::halt_forever();
    }

    // SAFETY: Threshold passes a naturally aligned pointer to its statically
    // allocated handoff record and transfers read-only ownership to the kernel.
    let omen = unsafe { ptr::read(omen_address) };
    if !omen.has_valid_header() {
        reject_memory(&mut serial, "handoff_schema", None);
    }
    let framebuffer = match omen.framebuffer() {
        Ok(framebuffer) => framebuffer,
        Err(error) => reject_memory(&mut serial, error.reason(), None),
    };
    let diagnostic_framebuffer =
        framebuffer.filter(|framebuffer| memory::framebuffer_is_safe_for_diagnostics(*framebuffer));
    if !omen.is_valid() {
        reject_memory(&mut serial, "handoff_schema", diagnostic_framebuffer);
    }

    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"boot.omen.accepted","level":"info","version":{},"boot_drive":{}}}"#,
        omen.version, omen.boot_drive
    );

    // SAFETY: The validated Threshold handoff owns the bounded descriptor table
    // until this one-time copy completes, and the bootstrap identity map keeps
    // it readable throughout the copy.
    let memory_map = match unsafe { omen.memory_map() } {
        Ok(memory_map) => memory_map,
        Err(error) => reject_memory(&mut serial, error.reason(), diagnostic_framebuffer),
    };
    if !arch::initialize_memory_protections() {
        reject_memory(
            &mut serial,
            "paging_protection_unavailable",
            diagnostic_framebuffer,
        );
    }
    let memory = match memory::initialize(memory_map.regions(), framebuffer) {
        Ok(memory) => memory,
        Err(error) => reject_memory(&mut serial, error.reason(), diagnostic_framebuffer),
    };
    let usable_bytes = memory.accounting.free as u64 * memory.frame_size;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"boot.memory.map","level":"info","regions":{},"usable_bytes":{}}}"#,
        memory.region_count, usable_bytes
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory.ready","level":"info","frame_size":{},"address_limit":{},"total_frames":{},"free_frames":{},"unavailable_frames":{},"data_owned_frames":{},"table_owned_frames":{}}}"#,
        memory.frame_size,
        memory.address_limit,
        memory.total_frames,
        memory.accounting.free,
        memory.accounting.unavailable,
        memory.accounting.data_owned,
        memory.accounting.table_owned
    );

    // SAFETY: Memory initialization accepted the optional diagnostic aperture,
    // IF remains clear, NMI remains masked, and this is the only BSP. The
    // retained value is inert metadata until a terminal interrupt failure.
    unsafe { console::register_interrupt_failure_surface(diagnostic_framebuffer) };

    let framebuffer_available = if let Some(framebuffer) = framebuffer {
        // SAFETY: Threshold's VBE handoff describes a unique physical aperture,
        // and the bootstrap identity maps the first four GiB.
        unsafe { console::draw_boot_surface(framebuffer) };
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"boot.display.ready","level":"info","width":{},"height":{},"pitch":{},"bpp":{}}}"#,
            framebuffer.width, framebuffer.height, framebuffer.pitch, framebuffer.bits_per_pixel
        );
        true
    } else {
        console::draw_text_fallback();
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"boot.display.fallback","level":"warn","mode":"vga_text"}}"#
        );
        false
    };

    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"boot.ready","level":"info","control_abi":"{abi_major}.{abi_minor}","framebuffer":{framebuffer_available}}}"#
    );

    #[cfg(all(
        feature = "display_vessel_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    match framebuffer {
        Some(framebuffer) => arch::run_display_vessel_test(framebuffer),
        None => {
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"kernel.display_vessel_test.failed","level":"fatal","reason":"framebuffer_unavailable"}}"#
            );
            arch::qemu_exit_failure();
            arch::halt_forever();
        }
    }

    #[cfg(any(
        all(
            feature = "supervised_renderer_test",
            not(feature = "semantic_surface_transport_test")
        ),
        feature = "semantic_surface_transport_test",
        not(feature = "display_vessel_test"),
        test
    ))]
    {
        #[cfg(all(
            feature = "fault_test",
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test")
        ))]
        arch::run_fault_test();
        #[cfg(all(
            feature = "circle_test",
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test")
        ))]
        arch::run_circle_test();
        #[cfg(all(
            feature = "control_test",
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test"),
            not(test)
        ))]
        arch::run_control_test();
        #[cfg(all(
            feature = "rite_test",
            not(feature = "shell_vessel_test"),
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test"),
            not(test)
        ))]
        {
            let screen = if let Some(framebuffer) = framebuffer {
                // SAFETY: Boot drawing released its temporary canvas, and this
                // terminal feature takes the only live screen ownership.
                unsafe { console::screen_console::with_framebuffer(framebuffer) }
            } else {
                console::screen_console::with_vga_text()
            };
            arch::run_rite_test(screen);
        }
        #[cfg(all(
            feature = "memory_test",
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test"),
            not(test)
        ))]
        arch::run_memory_test();

        #[cfg(all(feature = "desktop_dev", not(test)))]
        let desktop = if desktop_dev::selected() {
            let framebuffer = framebuffer
                .unwrap_or_else(|| desktop_dev::fail(&mut serial, "framebuffer_unavailable"));
            arch::run_bootstrap_presentation(framebuffer);
            // SAFETY: The returning native path has destroyed every task and
            // framebuffer writer; ordinary IRQ initialization has not begun.
            Some(
                unsafe { desktop_dev::desktop_frame::capture(framebuffer) }
                    .unwrap_or_else(|reason| desktop_dev::fail(&mut serial, reason)),
            )
        } else {
            None
        };

        let interrupts = match arch::initialize_external_interrupts() {
            Ok(interrupts) => interrupts,
            Err(error) => reject_interrupts(&mut serial, error.reason(), diagnostic_framebuffer),
        };
        if !arch::enable_external_interrupts() || !arch::external_interrupts_enabled() {
            reject_interrupts(&mut serial, "enablement_failed", diagnostic_framebuffer);
        }
        let input_available = interrupts.input_status.available();
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.interrupts.ready","level":"info","backend":"legacy_pc","vector_base":{},"vector_count":{},"pit_input_hz":{},"pit_divisor":{},"timebase_generation":{},"initial_master_mask":{},"initial_slave_mask":{},"master_mask":{},"slave_mask":{},"input_available":{input_available},"input_source_generation":{},"input_queue_capacity":{},"nmi_masked":{},"if_enabled":true}}"#,
            interrupts.vector_base,
            interrupts.vector_count,
            interrupts.pit_input_hz,
            interrupts.pit_divisor,
            time::boot_timebase_generation,
            interrupts.initial_master_mask,
            interrupts.initial_slave_mask,
            interrupts.master_mask,
            interrupts.slave_mask,
            input::ps2_source_generation,
            interrupts.input_queue_capacity,
            interrupts.nmi_masked
        );
        if !input_available {
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"kernel.input.unavailable","level":"warn","source":"physical_keyboard","reason":"{}","timer_available":true}}"#,
                interrupts.input_status.reason()
            );
        }

        #[cfg(all(
            feature = "workspace_dev",
            not(feature = "dispatcher_test"),
            not(test),
            not(any(
                feature = "fault_test",
                feature = "circle_test",
                feature = "control_test",
                feature = "rite_test",
                feature = "memory_test",
                feature = "interrupt_test",
                feature = "supervision_test",
                feature = "shell_vessel_test",
                feature = "input_wait_test",
                feature = "display_vessel_test",
                feature = "supervised_renderer_test",
                feature = "semantic_surface_transport_test",
                feature = "semantic_surface_presentation_test",
                feature = "desktop_dev"
            ))
        ))]
        {
            // Keep this development branch terminal even when its required
            // hardware is unavailable; scheduler_test supplies entry helpers
            // and must not accidentally become the fallback environment.
            fn enter_workspace_or_shell(
                framebuffer: Option<boot_omen::framebuffer_info>,
                input_available: bool,
                serial: &mut console::serial_port,
            ) {
                if let Some(framebuffer) = framebuffer.filter(|_| input_available) {
                    arch::run_workspace(framebuffer);
                }
                let screen = if let Some(framebuffer) = framebuffer {
                    // SAFETY: No workspace presenter was constructed, and boot
                    // drawing released the aperture before this console.
                    unsafe { console::screen_console::with_framebuffer(framebuffer) }
                } else {
                    console::screen_console::with_vga_text()
                };
                shell::run(screen, serial, input_available)
            }
            enter_workspace_or_shell(framebuffer, input_available, &mut serial);
        }

        #[cfg(all(feature = "desktop_dev", not(test)))]
        if let Some(desktop) = desktop {
            desktop.run(&mut serial, input_available);
        }

        #[cfg(all(
            feature = "semantic_surface_transport_test",
            not(feature = "semantic_surface_presentation_test"),
            not(test)
        ))]
        arch::run_semantic_surface_transport_test();

        #[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
        match framebuffer {
            Some(framebuffer) => arch::run_semantic_surface_presentation_test(framebuffer),
            None => {
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"kernel.semantic_surface_presentation_test.failed","level":"fatal","reason":"framebuffer_unavailable"}}"#
                );
                arch::disable_external_interrupts();
                arch::qemu_exit_failure();
                arch::halt_forever();
            }
        }

        #[cfg(all(
            feature = "supervised_renderer_test",
            not(feature = "semantic_surface_transport_test"),
            not(test)
        ))]
        match framebuffer {
            Some(framebuffer) => arch::run_supervised_renderer_test(framebuffer),
            None => {
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"kernel.supervised_renderer_test.failed","level":"fatal","reason":"framebuffer_unavailable"}}"#
                );
                arch::disable_external_interrupts();
                arch::qemu_exit_failure();
                arch::halt_forever();
            }
        }

        #[cfg(all(
            feature = "dispatcher_test",
            not(test),
            not(any(
                feature = "fault_test",
                feature = "circle_test",
                feature = "control_test",
                feature = "rite_test",
                feature = "memory_test",
                feature = "interrupt_test",
                feature = "supervision_test",
                feature = "shell_vessel_test",
                feature = "input_wait_test",
                feature = "display_vessel_test",
                feature = "supervised_renderer_test",
                feature = "semantic_surface_transport_test",
                feature = "semantic_surface_presentation_test",
                feature = "desktop_dev"
            ))
        ))]
        arch::run_dispatcher_test();

        #[cfg(all(
            feature = "scheduler_test",
            not(feature = "dispatcher_test"),
            not(feature = "supervision_test"),
            not(feature = "shell_vessel_test"),
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test"),
            not(test)
        ))]
        arch::run_scheduler_test();
        #[cfg(all(
            feature = "supervision_test",
            not(feature = "shell_vessel_test"),
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test"),
            not(test)
        ))]
        arch::run_supervision_test();
        #[cfg(all(
            feature = "shell_vessel_test",
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test"),
            not(test)
        ))]
        {
            let screen = if let Some(framebuffer) = framebuffer {
                // SAFETY: Boot drawing released its temporary canvas, and this
                // terminal feature takes the only live screen ownership.
                unsafe { console::screen_console::with_framebuffer(framebuffer) }
            } else {
                console::screen_console::with_vga_text()
            };
            arch::run_shell_vessel_test(screen);
        }

        #[cfg(all(
            feature = "interrupt_test",
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test")
        ))]
        interrupt_test::run(&mut serial);

        // QEMU terminates here for smoke images. In normal images this is a no-op,
        // and the same built kernel enters the interactive environment.
        #[cfg(all(
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test")
        ))]
        arch::qemu_exit_success();
        #[cfg(all(
            not(feature = "input_wait_test"),
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test")
        ))]
        let screen = if let Some(framebuffer) = framebuffer {
            // SAFETY: Boot drawing released its temporary canvas, leaving the
            // validated framebuffer uniquely owned by this console.
            unsafe { console::screen_console::with_framebuffer(framebuffer) }
        } else {
            console::screen_console::with_vga_text()
        };
        #[cfg(all(
            not(feature = "supervised_renderer_test"),
            not(feature = "semantic_surface_transport_test")
        ))]
        match () {
            #[cfg(not(feature = "input_wait_test"))]
            () => shell::run(screen, &mut serial, input_available),
            #[cfg(all(feature = "input_wait_test", not(test)))]
            () => arch::run_input_wait_test(),
            #[cfg(all(feature = "input_wait_test", test))]
            () => arch::halt_forever(),
        }
        #[cfg(all(
            any(
                feature = "supervised_renderer_test",
                feature = "semantic_surface_transport_test"
            ),
            test
        ))]
        arch::halt_forever()
    }
}

fn reject_interrupts(
    serial: &mut console::serial_port,
    reason: &str,
    framebuffer: Option<boot_omen::framebuffer_info>,
) -> ! {
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.interrupts.invalid","level":"fatal","reason":"{reason}"}}"#
    );
    arch::disable_external_interrupts();
    if let Some(framebuffer) = framebuffer {
        // SAFETY: Memory initialization accepted this complete framebuffer
        // aperture, and no interactive screen owner has been constructed yet.
        unsafe { console::draw_interrupt_failure_surface(framebuffer) };
    } else {
        console::draw_interrupt_failure();
    }
    arch::qemu_exit_failure();
    arch::halt_forever()
}

fn reject_memory(
    serial: &mut console::serial_port,
    reason: &str,
    framebuffer: Option<boot_omen::framebuffer_info>,
) -> ! {
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"boot.memory.invalid","level":"fatal","reason":"{reason}"}}"#
    );
    if let Some(framebuffer) = framebuffer {
        // SAFETY: The handoff parser validated the complete pixel layout, and
        // the diagnostic layout check proved the aperture disjoint from low
        // and linked kernel memory before allocator publication.
        unsafe { console::draw_memory_failure_surface(framebuffer) };
    } else {
        console::draw_memory_failure();
    }
    arch::qemu_exit_failure();
    arch::halt_forever()
}

#[panic_handler]
#[cfg(not(test))]
fn panic(info: &PanicInfo<'_>) -> ! {
    let mut serial = console::serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.panic","level":"fatal"}}"#
    );
    let _ = writeln!(serial, "panic detail: {info}");
    arch::qemu_exit_failure();
    arch::halt_forever()
}
