#[cfg(feature = "workspace_dev")]
#[path = "ps2_pointer.rs"]
pub(super) mod pointer;

use core::arch::asm;
use core::fmt::Write;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use crate::input::{
    ps2_discontinuity, ps2_queue_capacity, ps2_rearm_status, ps2_sample_publication,
};

use super::{descriptors, idt};

const pic_master_command_port: u16 = 0x20;
const pic_master_data_port: u16 = 0x21;
const pic_slave_command_port: u16 = 0xa0;
const pic_slave_data_port: u16 = 0xa1;
const pic_master_vector: u8 = 0x20;
const pic_slave_vector: u8 = 0x28;
const pic_vector_count: usize = 16;
const pic_end_of_interrupt: u8 = 0x20;
const pic_read_in_service: u8 = 0x0b;
const pic_all_masked: u8 = 0xff;
const pic_timer_mask: u8 = 1 << 0;
const pic_keyboard_mask: u8 = 1 << 1;

const pit_channel_0_port: u16 = 0x40;
const pit_command_port: u16 = 0x43;
const pit_channel_0_mode_2: u8 = 0x34;
pub(super) const pit_input_hz: u32 = 1_193_182;
pub(super) const pit_divisor: u16 = 11_932;

const ps2_data_port: u16 = 0x60;
const ps2_status_command_port: u16 = 0x64;
const ps2_status_output_full: u8 = 1 << 0;
const ps2_status_input_full: u8 = 1 << 1;
const ps2_status_auxiliary: u8 = 1 << 5;
const ps2_status_timeout: u8 = 1 << 6;
const ps2_status_parity: u8 = 1 << 7;
const ps2_status_error: u8 = ps2_status_timeout | ps2_status_parity;
const ps2_command_read_configuration: u8 = 0x20;
const ps2_command_write_configuration: u8 = 0x60;
const ps2_command_disable_second_port: u8 = 0xa7;
const ps2_command_self_test: u8 = 0xaa;
const ps2_command_test_first_port: u8 = 0xab;
const ps2_command_disable_first_port: u8 = 0xad;
const ps2_command_enable_first_port: u8 = 0xae;
const ps2_configuration_first_interrupt: u8 = 1 << 0;
const ps2_configuration_second_interrupt: u8 = 1 << 1;
const ps2_configuration_system: u8 = 1 << 2;
const ps2_configuration_first_disabled: u8 = 1 << 4;
const ps2_configuration_second_disabled: u8 = 1 << 5;
const ps2_configuration_translation: u8 = 1 << 6;
const ps2_controller_self_test_passed: u8 = 0x55;
const ps2_first_port_test_passed: u8 = 0x00;
const ps2_device_acknowledge: u8 = 0xfa;
const ps2_device_resend: u8 = 0xfe;
const ps2_device_self_test_passed: u8 = 0xaa;
const ps2_device_reset: u8 = 0xff;
const ps2_device_disable_scanning: u8 = 0xf5;
const ps2_device_select_scan_set: u8 = 0xf0;
const ps2_device_scan_set_2: u8 = 0x02;
const ps2_device_enable_scanning: u8 = 0xf4;
const ps2_poll_budget: usize = 4_000_000;
const ps2_flush_budget: usize = 64;
const ps2_irq_drain_budget: usize = 16;
const ps2_resend_budget: usize = 3;
const ps2_empty_irq_streak_limit: u8 = 8;

const initialization_uninitialized: u8 = 0;
const initialization_in_progress: u8 = 1;
const initialization_ready: u8 = 2;
const initialization_failed: u8 = 3;

const _: () = assert!(pit_divisor >= 2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum external_interrupt_error {
    already_initialized,
    initialization_failed,
    gate_installation_failed,
}

impl external_interrupt_error {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::already_initialized => "already_initialized",
            Self::initialization_failed => "initialization_failed",
            Self::gate_installation_failed => "gate_installation_failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ps2_setup_status {
    ready,
    controller_timeout,
    controller_output_stuck,
    controller_error,
    controller_self_test_failed,
    first_port_test_failed,
    device_rejected_command,
    device_self_test_failed,
    configuration_mismatch,
}

impl ps2_setup_status {
    pub const fn available(self) -> bool {
        matches!(self, Self::ready)
    }

    pub const fn reason(self) -> &'static str {
        match self {
            Self::ready => "ready",
            Self::controller_timeout => "controller_timeout",
            Self::controller_output_stuck => "controller_output_stuck",
            Self::controller_error => "controller_error",
            Self::controller_self_test_failed => "controller_self_test_failed",
            Self::first_port_test_failed => "first_port_test_failed",
            Self::device_rejected_command => "device_rejected_command",
            Self::device_self_test_failed => "device_self_test_failed",
            Self::configuration_mismatch => "configuration_mismatch",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct external_interrupt_state {
    pub vector_base: u8,
    pub vector_count: usize,
    pub pit_input_hz: u32,
    pub pit_divisor: u16,
    pub initial_master_mask: u8,
    pub initial_slave_mask: u8,
    pub master_mask: u8,
    pub slave_mask: u8,
    pub input_status: ps2_setup_status,
    pub input_queue_capacity: usize,
    pub nmi_masked: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub struct external_interrupt_health {
    pub master_spurious: u64,
    pub slave_spurious: u64,
    pub empty_keyboard: u64,
    pub input_available: bool,
    pub master_mask: u8,
    pub slave_mask: u8,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(super) struct interrupt_frame {
    pub(super) accumulator: u64,
    pub(super) base: u64,
    pub(super) counter: u64,
    pub(super) data: u64,
    pub(super) frame_pointer: u64,
    pub(super) source_index: u64,
    pub(super) destination_index: u64,
    pub(super) register_8: u64,
    pub(super) register_9: u64,
    pub(super) register_10: u64,
    pub(super) register_11: u64,
    pub(super) register_12: u64,
    pub(super) register_13: u64,
    pub(super) register_14: u64,
    pub(super) register_15: u64,
    pub(super) vector: u64,
    pub(super) error_code: u64,
    pub(super) instruction_pointer: u64,
    pub(super) code_segment: u64,
    pub(super) flags: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum interrupt_origin {
    kernel,
    #[cfg(all(
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ),
        not(test)
    ))]
    user,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum interrupt_acknowledgement {
    none,
    master,
    slave_then_master,
}

#[derive(Clone, Copy)]
enum interrupt_fatal_reason {
    invalid_frame,
    invalid_vector,
    source_not_armed,
    unexpected_source,
    time_exhausted,
    event_identity_exhausted,
}

impl interrupt_fatal_reason {
    const fn record(self) -> &'static str {
        match self {
            Self::invalid_frame => {
                "\n{\"schema\":1,\"event\":\"kernel.interrupt.fatal\",\"level\":\"fatal\",\"reason\":\"invalid_frame\"}\n"
            }
            Self::invalid_vector => {
                "\n{\"schema\":1,\"event\":\"kernel.interrupt.fatal\",\"level\":\"fatal\",\"reason\":\"invalid_vector\"}\n"
            }
            Self::source_not_armed => {
                "\n{\"schema\":1,\"event\":\"kernel.interrupt.fatal\",\"level\":\"fatal\",\"reason\":\"source_not_armed\"}\n"
            }
            Self::unexpected_source => {
                "\n{\"schema\":1,\"event\":\"kernel.interrupt.fatal\",\"level\":\"fatal\",\"reason\":\"unexpected_source\"}\n"
            }
            Self::time_exhausted => {
                "\n{\"schema\":1,\"event\":\"kernel.interrupt.fatal\",\"level\":\"fatal\",\"reason\":\"time_exhausted\"}\n"
            }
            Self::event_identity_exhausted => {
                "\n{\"schema\":1,\"event\":\"kernel.interrupt.fatal\",\"level\":\"fatal\",\"reason\":\"event_identity_exhausted\"}\n"
            }
        }
    }
}

trait ps2_ports {
    fn read_status(&mut self) -> u8;
    fn read_data(&mut self) -> u8;
    fn write_command(&mut self, command: u8);
    fn write_data(&mut self, data: u8);
}

trait pic_ports {
    fn read(&mut self, port: u16) -> u8;
    fn write(&mut self, port: u16, value: u8);
}

trait pit_ports {
    fn write(&mut self, port: u16, value: u8);
}

struct hardware_pic_ports;

struct hardware_pit_ports;

impl pic_ports for hardware_pic_ports {
    fn read(&mut self, port: u16) -> u8 {
        read_port(port)
    }

    fn write(&mut self, port: u16, value: u8) {
        write_port(port, value);
    }
}

impl pit_ports for hardware_pit_ports {
    fn write(&mut self, port: u16, value: u8) {
        write_port(port, value);
    }
}

struct hardware_ps2_ports;

impl ps2_ports for hardware_ps2_ports {
    fn read_status(&mut self) -> u8 {
        read_port(ps2_status_command_port)
    }

    fn read_data(&mut self) -> u8 {
        read_port(ps2_data_port)
    }

    fn write_command(&mut self, command: u8) {
        write_port(ps2_status_command_port, command);
    }

    fn write_data(&mut self, data: u8) {
        write_port(ps2_data_port, data);
    }
}

static initialization: AtomicU8 = AtomicU8::new(initialization_uninitialized);
static master_mask: AtomicU8 = AtomicU8::new(pic_all_masked);
static slave_mask: AtomicU8 = AtomicU8::new(pic_all_masked);
static ps2_configured: AtomicBool = AtomicBool::new(false);
static input_available: AtomicBool = AtomicBool::new(false);
static master_spurious: AtomicU64 = AtomicU64::new(0);
static slave_spurious: AtomicU64 = AtomicU64::new(0);
static empty_keyboard_interrupts: AtomicU64 = AtomicU64::new(0);
static empty_keyboard_streak: AtomicU8 = AtomicU8::new(0);
#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
#[unsafe(no_mangle)]
static x86_64_timer_interrupt_count: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" {
    fn x86_64_interrupt_stub_32();
    fn x86_64_interrupt_stub_33();
    fn x86_64_interrupt_stub_34();
    fn x86_64_interrupt_stub_35();
    fn x86_64_interrupt_stub_36();
    fn x86_64_interrupt_stub_37();
    fn x86_64_interrupt_stub_38();
    fn x86_64_interrupt_stub_39();
    fn x86_64_interrupt_stub_40();
    fn x86_64_interrupt_stub_41();
    fn x86_64_interrupt_stub_42();
    fn x86_64_interrupt_stub_43();
    fn x86_64_interrupt_stub_44();
    fn x86_64_interrupt_stub_45();
    fn x86_64_interrupt_stub_46();
    fn x86_64_interrupt_stub_47();
    #[cfg(all(
        feature = "interrupt_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    fn x86_64_interrupt_test_register_probe() -> u64;
}

const interrupt_stubs: [unsafe extern "C" fn(); pic_vector_count] = [
    x86_64_interrupt_stub_32,
    x86_64_interrupt_stub_33,
    x86_64_interrupt_stub_34,
    x86_64_interrupt_stub_35,
    x86_64_interrupt_stub_36,
    x86_64_interrupt_stub_37,
    x86_64_interrupt_stub_38,
    x86_64_interrupt_stub_39,
    x86_64_interrupt_stub_40,
    x86_64_interrupt_stub_41,
    x86_64_interrupt_stub_42,
    x86_64_interrupt_stub_43,
    x86_64_interrupt_stub_44,
    x86_64_interrupt_stub_45,
    x86_64_interrupt_stub_46,
    x86_64_interrupt_stub_47,
];

/// Prepare the legacy-PC external interrupt backend without setting IF.
///
/// The operation is one-shot. It takes ownership of both 8259-compatible PICs,
/// programs PIT channel 0, and attempts to initialize the first PS/2 port. A
/// PS/2 failure is degraded and leaves IRQ1 masked; all other successful state
/// remains available. The caller decides when it is safe to execute STI.
pub fn initialize_external_interrupts() -> Result<external_interrupt_state, external_interrupt_error>
{
    disable_interrupts();
    match initialization.compare_exchange(
        initialization_uninitialized,
        initialization_in_progress,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(initialization_ready) => {
            return Err(external_interrupt_error::already_initialized);
        }
        Err(_) => {
            return Err(external_interrupt_error::initialization_failed);
        }
    }

    for (index, stub) in interrupt_stubs.iter().enumerate() {
        let vector = usize::from(pic_master_vector) + index;
        // SAFETY: IF is clear, NMI remains masked by Threshold, the BSP is the
        // sole IDT writer, and each assembly entry is resident for this image.
        let installed = unsafe {
            idt::install_gate(
                vector,
                *stub as usize as u64,
                descriptors::kernel_code_selector,
                0,
                idt::interrupt_gate_attributes,
            )
        };
        if !installed {
            mask_all_pic_lines();
            initialization.store(initialization_failed, Ordering::Release);
            return Err(external_interrupt_error::gate_installation_failed);
        }
    }

    initialize_pic();
    initialize_pit();
    let input_status = initialize_ps2();
    if input_status.available() {
        reset_empty_keyboard_streak(&empty_keyboard_streak);
    }
    let selected_master_mask = if input_status.available() {
        pic_all_masked & !pic_timer_mask & !pic_keyboard_mask
    } else {
        pic_all_masked & !pic_timer_mask
    };
    write_master_mask(selected_master_mask);
    write_slave_mask(pic_all_masked);
    ps2_configured.store(input_status.available(), Ordering::Release);
    input_available.store(input_status.available(), Ordering::Release);
    initialization.store(initialization_ready, Ordering::Release);

    Ok(external_interrupt_state {
        vector_base: pic_master_vector,
        vector_count: pic_vector_count,
        pit_input_hz,
        pit_divisor,
        initial_master_mask: pic_all_masked,
        initial_slave_mask: pic_all_masked,
        master_mask: selected_master_mask,
        slave_mask: pic_all_masked,
        input_status,
        input_queue_capacity: ps2_queue_capacity,
        nmi_masked: true,
    })
}

/// Enable maskable interrupts after successful one-shot preparation.
pub fn enable_external_interrupts() -> bool {
    if initialization.load(Ordering::Acquire) != initialization_ready {
        disable_interrupts();
        return false;
    }
    // SAFETY: The initialized IDT covers every unmasked PIC line, source state
    // is published, and the caller has finished non-interruptible setup.
    unsafe { asm!("sti", options(nostack)) };
    true
}

/// Mask the legacy sources for the desktop's one pre-initialization render.
/// Unlike terminal shutdown this preserves the virgin backend, which the
/// ordinary one-shot initialization will prepare after every user task exits.
#[cfg(all(feature = "desktop_dev", not(test)))]
pub(super) fn prepare_bootstrap_presentation() -> bool {
    if !bootstrap_presentation_backend_is_virgin() {
        return false;
    }
    mask_all_pic_lines();
    bootstrap_presentation_backend_is_virgin()
}

#[cfg(all(feature = "desktop_dev", not(test)))]
pub(super) fn bootstrap_presentation_backend_is_virgin() -> bool {
    bootstrap_presentation_state_is_virgin(
        initialization.load(Ordering::Acquire),
        master_mask.load(Ordering::Acquire),
        slave_mask.load(Ordering::Acquire),
        ps2_configured.load(Ordering::Acquire),
        input_available.load(Ordering::Acquire),
        interrupts_enabled(),
    )
}

#[cfg(feature = "desktop_dev")]
const fn bootstrap_presentation_state_is_virgin(
    state: u8,
    master: u8,
    slave: u8,
    configured: bool,
    available: bool,
    enabled: bool,
) -> bool {
    state == initialization_uninitialized
        && master == pic_all_masked
        && slave == pic_all_masked
        && !configured
        && !available
        && !enabled
}

/// Restrict the prepared legacy interrupt backend to the real PIT timer.
///
/// This feature-only transition requires active delivery on entry and leaves IF
/// clear. It masks IRQ1 and every other PIC line except IRQ0 without replacing
/// or reprogramming the already initialized PIT.
#[cfg(any(
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
pub(super) fn restrict_to_scheduler_timer() -> bool {
    let was_enabled = disable_interrupts_and_read_previous();
    if !critical_section_available(initialization.load(Ordering::Acquire), was_enabled) {
        return false;
    }

    write_master_mask(scheduler_timer_master_mask());
    write_slave_mask(pic_all_masked);
    input_available.store(false, Ordering::Release);
    master_mask.load(Ordering::Relaxed) == scheduler_timer_master_mask()
        && slave_mask.load(Ordering::Relaxed) == pic_all_masked
        && !interrupts_enabled()
}

#[cfg(feature = "owned_multi_region_test")]
static multi_region_timer_rearmed: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "owned_source_test")]
static source_timer_phase: AtomicU8 = AtomicU8::new(0);

#[cfg(any(test, feature = "owned_source_test"))]
const source_timer_producer: u8 = 0;
#[cfg(any(test, feature = "owned_source_test"))]
const source_timer_quiesced: u8 = 1;
#[cfg(any(test, feature = "owned_source_test"))]
const source_timer_restored: u8 = 2;

#[cfg(any(test, feature = "owned_source_test"))]
#[derive(Clone, Copy)]
struct source_timer_observation {
    state: u8,
    if_enabled: bool,
    master: u8,
    slave: u8,
    input: bool,
    first_rearmed: bool,
}

#[cfg(any(test, feature = "owned_source_test"))]
#[derive(Clone, Copy)]
enum source_timer_transition {
    quiesce_producer,
    restore_consumer,
}

#[cfg(feature = "owned_source_test")]
fn observe_source_timer() -> source_timer_observation {
    source_timer_observation {
        state: initialization.load(Ordering::Acquire),
        if_enabled: interrupts_enabled(),
        master: master_mask.load(Ordering::Acquire),
        slave: slave_mask.load(Ordering::Acquire),
        input: input_available.load(Ordering::Acquire),
        first_rearmed: multi_region_timer_rearmed.load(Ordering::Acquire),
    }
}

/// Mask the completed source producer's timer without marking the initialized
/// backend failed. This single transition cannot revive a disabled backend.
///
/// # Safety
/// Sole BSP, IF clear/NMI masked; both producer tasks are terminal and the caller
/// is retaining their mappings until the bootstrap continuation can reclaim them.
#[cfg(feature = "owned_source_test")]
#[allow(dead_code)] // Higher-priority combined profiles may suppress this proof.
pub(super) unsafe fn quiesce_source_producer_timer_before_retirement() -> bool {
    apply_source_timer_transition(source_timer_transition::quiesce_producer)
}

/// Restore IRQ0 alone once, after complete producer retirement. IF stays clear;
/// the separate consumer rearm remains required immediately before first entry.
///
/// # Safety
/// Sole BSP, NMI masked, original bootstrap root/stack restored and all producer
/// contexts, mappings and aliases retired. No admitted task may execute here.
#[cfg(feature = "owned_source_test")]
#[allow(dead_code)] // Higher-priority combined profiles may suppress this proof.
pub(super) unsafe fn restore_source_consumer_timer_after_retirement() -> bool {
    apply_source_timer_transition(source_timer_transition::restore_consumer)
}

#[cfg(feature = "owned_source_test")]
fn apply_source_timer_transition(transition: source_timer_transition) -> bool {
    if !source_timer_transition_with(
        &mut hardware_pic_ports,
        &source_timer_phase,
        observe_source_timer(),
        transition,
    ) {
        return false;
    }
    let master = match transition {
        source_timer_transition::quiesce_producer => pic_all_masked,
        source_timer_transition::restore_consumer => scheduler_timer_master_mask(),
    };
    master_mask.store(master, Ordering::Release);
    slave_mask.store(pic_all_masked, Ordering::Release);
    initialization.load(Ordering::Acquire) == initialization_ready
        && !interrupts_enabled()
        && !input_available.load(Ordering::Acquire)
}

#[cfg(any(test, feature = "owned_source_test"))]
fn source_timer_transition_with(
    ports: &mut impl pic_ports,
    phase: &AtomicU8,
    observed: source_timer_observation,
    transition: source_timer_transition,
) -> bool {
    let (previous, next, required_master, next_master) = match transition {
        source_timer_transition::quiesce_producer => (
            source_timer_producer,
            source_timer_quiesced,
            scheduler_timer_master_mask(),
            pic_all_masked,
        ),
        source_timer_transition::restore_consumer => (
            source_timer_quiesced,
            source_timer_restored,
            pic_all_masked,
            scheduler_timer_master_mask(),
        ),
    };
    if observed.state != initialization_ready
        || observed.if_enabled
        || observed.master != required_master
        || observed.slave != pic_all_masked
        || observed.input
        || !observed.first_rearmed
        || phase
            .compare_exchange(previous, next, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return false;
    }
    // IF remains clear. This changes masks only: no STI, PIC reset/EOI, PIT
    // programming, timebase reset or failed-initialization resurrection.
    ports.write(pic_master_data_port, next_master);
    ports.write(pic_slave_data_port, pic_all_masked);
    true
}

/// Restart the existing cadence once, after slow unpublished preparation. A
/// latched PIC edge still follows normal delivery/EOI; no timebase is reset.
///
/// # Safety
/// Sole BSP, IF clear/NMI masked, immediately before the first user entry of
/// the multi-region proof. Never call during admitted task execution.
#[cfg(feature = "owned_multi_region_test")]
#[allow(dead_code)]
pub(super) unsafe fn rearm_multi_region_timer_before_entry() -> bool {
    rearm_multi_region_timer_with(
        &mut hardware_pit_ports,
        &multi_region_timer_rearmed,
        initialization.load(Ordering::Acquire),
        interrupts_enabled(),
        master_mask.load(Ordering::Acquire),
        slave_mask.load(Ordering::Acquire),
        input_available.load(Ordering::Acquire),
    )
}

/// Second, distinct one-shot entry after source producers have fully retired.
/// Producer quiescence and consumer mask restoration must already be consumed.
///
/// # Safety
/// Same sole-BSP exclusion as first entry, after producer teardown and fresh
/// consumer preparation. No admitted userspace execution is live during this.
#[cfg(feature = "owned_source_test")]
#[allow(dead_code)]
pub(super) unsafe fn rearm_source_consumer_timer_before_entry() -> bool {
    static rearmed: AtomicBool = AtomicBool::new(false);
    rearm_source_consumer_timer_with(
        &mut hardware_pit_ports,
        &rearmed,
        &source_timer_phase,
        observe_source_timer(),
    )
}

#[cfg(any(test, feature = "owned_source_test"))]
fn rearm_source_consumer_timer_with(
    ports: &mut impl pit_ports,
    rearmed: &AtomicBool,
    phase: &AtomicU8,
    observed: source_timer_observation,
) -> bool {
    if phase.load(Ordering::Acquire) != source_timer_restored || !observed.first_rearmed {
        return false;
    }
    rearm_multi_region_timer_with(
        ports,
        rearmed,
        observed.state,
        observed.if_enabled,
        observed.master,
        observed.slave,
        observed.input,
    )
}

#[cfg(any(test, feature = "owned_multi_region_test"))]
fn rearm_multi_region_timer_with(
    ports: &mut impl pit_ports,
    rearmed: &AtomicBool,
    state: u8,
    if_enabled: bool,
    master: u8,
    slave: u8,
    input: bool,
) -> bool {
    if state != initialization_ready
        || if_enabled
        || master != scheduler_timer_master_mask()
        || slave != pic_all_masked
        || input
        || rearmed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return false;
    }
    initialize_pit_with(ports);
    true
}

#[cfg(any(
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
pub(super) fn scheduler_interrupts_are_masked() -> bool {
    master_mask.load(Ordering::Acquire) == pic_all_masked
        && slave_mask.load(Ordering::Acquire) == pic_all_masked
        && !interrupts_enabled()
}

/// Stop external delivery and mask every legacy PIC source.
///
/// This does not make the one-shot controller state reusable. It is a terminal
/// shutdown operation for tests and failure containment.
pub fn disable_external_interrupts() {
    disable_interrupts();
    mask_all_pic_lines();
    input_available.store(false, Ordering::Release);
    initialization.store(initialization_failed, Ordering::Release);
}

#[must_use]
pub fn interrupts_enabled() -> bool {
    let flags: u64;
    // SAFETY: PUSHFQ/POP only inspect the current processor's flags and balance
    // the current stack exactly.
    unsafe {
        asm!(
            "pushfq",
            "pop {flags}",
            flags = out(reg) flags,
            options(preserves_flags)
        );
    }
    flags & (1 << 9) != 0
}

/// Run one bounded operation under the bootstrap interrupt exclusion contract.
///
/// This is the safe bridge for consumers of the kernel's interrupt-owned
/// `UnsafeCell` state. It returns `None` with IF clear unless the backend was
/// prepared and delivery was already active. A successful operation always
/// restores IF before returning. The closure must not block or attempt to enter
/// another architecture interrupt critical section.
pub fn with_external_interrupts_disabled<T>(operation: impl FnOnce() -> T) -> Option<T> {
    let was_enabled = disable_interrupts_and_read_previous();
    if !critical_section_available(initialization.load(Ordering::Acquire), was_enabled) {
        return None;
    }

    let result = operation();
    if initialization.load(Ordering::Acquire) != initialization_ready || interrupts_enabled() {
        disable_interrupts();
        return None;
    }
    // SAFETY: The backend was ready and IF was active on entry. The closure ran
    // under the BSP/NMI exclusion contract and completed before restoration.
    unsafe { asm!("sti", options(nostack)) };
    Some(result)
}

/// Re-enable IRQ1 after the consumer has observed a loss and reset all decoder
/// and keymap state.
///
/// This may only succeed inside [`with_external_interrupts_disabled`]. It does
/// a bounded controller/device reset while IRQ1 remains masked, then installs
/// a fresh source generation with sequence zero before unmasking delivery.
/// Terminal event-identity exhaustion is never rearmed.
pub fn rearm_keyboard_input_after_loss() -> Option<u32> {
    if interrupts_enabled()
        || initialization.load(Ordering::Acquire) != initialization_ready
        || input_available.load(Ordering::Acquire)
        || master_mask.load(Ordering::Relaxed) & pic_keyboard_mask == 0
    {
        return None;
    }

    // SAFETY: IF is observed clear, Threshold keeps NMI masked, and this slice
    // starts no application processors. The caller reset its semantic decoder
    // before requesting transport delivery again.
    if !unsafe { crate::input::ps2_resynchronization_required_if_disabled() } {
        return None;
    }

    #[cfg(feature = "workspace_dev")]
    pointer::before_keyboard_reset();
    ps2_configured.store(false, Ordering::Release);
    let input_status = initialize_ps2();
    if !input_status.available() {
        return None;
    }

    // SAFETY: The caller consumed the loss and reset its semantic keymap; the
    // bounded controller/device reset above discarded all transport suffixes
    // while IRQ1 and IF remained masked.
    match unsafe { crate::input::complete_ps2_resynchronization_if_disabled() } {
        ps2_rearm_status::ready(generation) => {
            reset_empty_keyboard_streak(&empty_keyboard_streak);
            ps2_configured.store(true, Ordering::Release);
            input_available.store(true, Ordering::Release);
            unmask_keyboard_line();
            Some(generation)
        }
        ps2_rearm_status::pending_delivery => None,
        ps2_rearm_status::terminal(_) => {
            interrupt_fatal(interrupt_fatal_reason::event_identity_exhausted)
        }
    }
}

/// Force one feature-only capacity loss through the same containment path used
/// by IRQ1. The caller must hold the architecture interrupt critical section.
#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub fn interrupt_test_force_loss() -> bool {
    if interrupts_enabled()
        || initialization.load(Ordering::Acquire) != initialization_ready
        || !input_available.load(Ordering::Acquire)
    {
        return false;
    }
    // SAFETY: The feature caller holds IF clear on the sole BSP, exactly as the
    // real IRQ1 producer does, and the private queue is known to be empty.
    let publication =
        unsafe { crate::input::force_ps2_capacity_loss_for_test(crate::time::now_if_disabled()) };
    let lost = publication.is_lost();
    contain_input_publication(publication);
    lost && !input_available.load(Ordering::Acquire)
        && master_mask.load(Ordering::Relaxed) & pic_keyboard_mask != 0
}

/// Sleep without losing work published between the caller's check and HLT.
///
/// Returns false with IF clear if the external backend is not ready. Otherwise
/// the caller's predicate runs with IF clear and must not block or mutate shared
/// state. STI's interrupt shadow makes the adjacent HLT atomic with respect to
/// delivery of an already-pending interrupt.
pub fn wait_for_interrupt_if_idle(work_ready_if_disabled: impl FnOnce() -> bool) -> bool {
    let was_enabled = disable_interrupts_and_read_previous();
    if !critical_section_available(initialization.load(Ordering::Acquire), was_enabled) {
        return false;
    }
    if work_ready_if_disabled() {
        // SAFETY: Preparation is complete and the predicate found work, so the
        // caller should resume with ordinary external delivery enabled.
        unsafe { asm!("sti", options(nostack)) };
    } else {
        // SAFETY: The STI shadow delays recognition through the immediately
        // following HLT. A pending or subsequent interrupt wakes this BSP.
        unsafe { asm!("sti", "hlt", options(nostack)) };
    }
    true
}

#[must_use]
#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub fn read_external_interrupt_health() -> external_interrupt_health {
    external_interrupt_health {
        master_spurious: master_spurious.load(Ordering::Relaxed),
        slave_spurious: slave_spurious.load(Ordering::Relaxed),
        empty_keyboard: empty_keyboard_interrupts.load(Ordering::Relaxed),
        input_available: input_available.load(Ordering::Acquire),
        master_mask: master_mask.load(Ordering::Relaxed),
        slave_mask: slave_mask.load(Ordering::Relaxed),
    }
}

pub fn disable_interrupts() {
    // SAFETY: This changes only the current processor's maskable-interrupt flag.
    // Retaining the default memory effects prevents protected operations from
    // moving across this exclusion boundary.
    unsafe { asm!("cli", options(nostack)) };
}

/// Report the live source state after a native workspace retires; a failed
/// loss rearm must not advertise an available keyboard to the fallback shell.
#[cfg(all(feature = "workspace_dev", not(test)))]
pub(super) fn workspace_input_available() -> bool {
    input_available.load(Ordering::Acquire)
}

fn disable_interrupts_and_read_previous() -> bool {
    let flags: u64;
    // SAFETY: PUSHFQ/POP balance the current stack and CLI closes the interrupt
    // window before this instruction sequence returns to Rust.
    unsafe {
        asm!(
            "pushfq",
            "pop {flags}",
            "cli",
            flags = out(reg) flags,
        );
    }
    flags & (1 << 9) != 0
}

const fn critical_section_available(state: u8, was_enabled: bool) -> bool {
    state == initialization_ready && was_enabled
}

#[cfg(any(
    test,
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
const fn scheduler_timer_master_mask() -> u8 {
    pic_all_masked & !pic_timer_mask
}

#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub fn run_interrupt_return_probe() -> bool {
    if !interrupts_enabled() || initialization.load(Ordering::Acquire) != initialization_ready {
        return false;
    }
    let initial_count = x86_64_timer_interrupt_count.load(Ordering::Acquire);
    let mut registers_preserved = true;
    for _ in 0..32 {
        // SAFETY: This feature-only routine is called after the PIC/PIT gates
        // are live and IF is set. It halts for one external interrupt and
        // preserves the complete System V call boundary before returning.
        registers_preserved &= unsafe { x86_64_interrupt_test_register_probe() } == 1;
        if !registers_preserved {
            return false;
        }
        if x86_64_timer_interrupt_count.load(Ordering::Acquire) > initial_count {
            return true;
        }
    }
    false
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_interrupt_dispatch(
    frame_address: *mut interrupt_frame,
) -> *mut interrupt_frame {
    if !interrupt_frame_address_is_valid(frame_address) {
        interrupt_fatal(interrupt_fatal_reason::invalid_frame);
    }
    // SAFETY: Every external stub constructs the complete fixed frame before
    // passing its live address, and the dispatcher does not retain the value.
    let frame = unsafe { ptr::read(frame_address) };
    let Some(origin) = interrupt_frame_origin(&frame) else {
        interrupt_fatal(interrupt_fatal_reason::invalid_frame);
    };
    #[cfg(not(all(
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ),
        not(test)
    )))]
    let _ = origin;
    #[cfg(all(
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ),
        not(test)
    ))]
    if origin == interrupt_origin::user
        && !super::scheduler_test::user_interrupt_frame_is_valid(frame_address)
    {
        interrupt_fatal(interrupt_fatal_reason::invalid_frame);
    }
    let Ok(vector) = u8::try_from(frame.vector) else {
        interrupt_fatal(interrupt_fatal_reason::invalid_vector);
    };
    let Some(irq) = vector.checked_sub(pic_master_vector) else {
        interrupt_fatal(interrupt_fatal_reason::invalid_vector);
    };
    if usize::from(irq) >= pic_vector_count {
        interrupt_fatal(interrupt_fatal_reason::invalid_vector);
    }
    #[cfg(all(
        feature = "owned_workspace_cpu_test",
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
    if origin == interrupt_origin::user && super::workspace_runtime::is_active() {
        super::workspace_runtime::capture_interrupt(frame_address, irq);
    }
    #[cfg(all(
        feature = "native_editor_workspace_test",
        not(test),
        not(any(
            feature = "ny_object_test",
            feature = "native_application_test",
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
    if origin == interrupt_origin::user && super::dispatcher_runtime::is_active() {
        super::dispatcher_runtime::capture_interrupt(frame_address);
    }
    let _interactive_input = false;
    #[cfg(all(
        feature = "native_interactive_editor_test",
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
            feature = "desktop_dev",
            feature = "native_durable_failure_test",
            feature = "native_relaunch_test",
            feature = "elf_application_test"
        ))
    ))]
    let _interactive_input = super::dispatcher_runtime::interactive_input_active() && irq == 1;
    let _workspace_input = false;
    #[cfg(all(
        feature = "native_editor_workspace_test",
        not(test),
        not(any(
            feature = "ny_object_test",
            feature = "native_application_test",
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
    let _workspace_input = super::dispatcher_runtime::is_active() && matches!(irq, 1 | 12);
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
    let _workspace_input = super::workspace_runtime::is_active() && matches!(irq, 1 | 12);
    #[cfg(all(
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ),
        not(test)
    ))]
    if origin == interrupt_origin::user && irq != 0 && !_workspace_input && !_interactive_input {
        interrupt_fatal(interrupt_fatal_reason::unexpected_source);
    }
    if initialization.load(Ordering::Acquire) != initialization_ready {
        interrupt_fatal(interrupt_fatal_reason::source_not_armed);
    }

    match irq {
        0 => {
            if !pic_source_is_unmasked(
                irq,
                master_mask.load(Ordering::Relaxed),
                slave_mask.load(Ordering::Relaxed),
            ) {
                interrupt_fatal(interrupt_fatal_reason::source_not_armed);
            }
            // SAFETY: Interrupt gates clear IF and only the BSP advances this
            // bootstrap source during the current one-processor slice.
            let instant = unsafe { crate::time::advance_from_interrupt() };
            if instant.is_saturated() {
                interrupt_fatal(interrupt_fatal_reason::time_exhausted);
            }
            #[cfg(all(
                feature = "interrupt_test",
                not(feature = "supervised_renderer_test"),
                not(feature = "semantic_surface_transport_test")
            ))]
            increment_saturating(&x86_64_timer_interrupt_count);
            acknowledge(interrupt_acknowledgement::master);
            #[cfg(all(
                any(
                    feature = "scheduler_test",
                    feature = "supervision_test",
                    feature = "shell_vessel_test",
                    feature = "input_wait_test",
                    feature = "supervised_renderer_test",
                    feature = "semantic_surface_transport_test"
                ),
                not(test)
            ))]
            if origin == interrupt_origin::user {
                return dispatch_scheduler_timer(frame_address, instant);
            }
        }
        1 => {
            if !pic_source_is_unmasked(
                irq,
                master_mask.load(Ordering::Relaxed),
                slave_mask.load(Ordering::Relaxed),
            ) || !input_available.load(Ordering::Acquire)
            {
                interrupt_fatal(interrupt_fatal_reason::source_not_armed);
            }
            #[cfg(feature = "workspace_dev")]
            if pointer::session_active() {
                pointer::interrupt(irq);
            } else {
                handle_keyboard_interrupt();
            }
            #[cfg(not(feature = "workspace_dev"))]
            handle_keyboard_interrupt();
            acknowledge(interrupt_acknowledgement::master);
        }
        #[cfg(feature = "workspace_dev")]
        12 if pointer::session_active() => {
            // A previously latched IRQ12 can arrive after source loss masks
            // delivery. Draining/discarding its AUX suffix is still bounded.
            pointer::interrupt(irq);
            acknowledge(interrupt_acknowledgement::slave_then_master);
        }
        7 => {
            let in_service = read_pic_in_service(pic_master_command_port);
            let acknowledgement = acknowledgement_for(irq, in_service);
            if acknowledgement == interrupt_acknowledgement::none {
                increment_saturating(&master_spurious);
            } else {
                interrupt_fatal(interrupt_fatal_reason::unexpected_source);
            }
            acknowledge(acknowledgement);
        }
        15 => {
            let in_service = read_pic_in_service(pic_slave_command_port);
            let acknowledgement = acknowledgement_for(irq, in_service);
            if acknowledgement == interrupt_acknowledgement::master {
                increment_saturating(&slave_spurious);
            } else {
                interrupt_fatal(interrupt_fatal_reason::unexpected_source);
            }
            acknowledge(acknowledgement);
        }
        _ => {
            interrupt_fatal(interrupt_fatal_reason::unexpected_source);
        }
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
    if origin == interrupt_origin::user && super::workspace_runtime::is_active() {
        return super::workspace_runtime::resume_interrupt(frame_address);
    }
    #[cfg(all(
        feature = "native_interactive_editor_test",
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
            feature = "desktop_dev",
            feature = "native_durable_failure_test",
            feature = "native_relaunch_test",
            feature = "elf_application_test"
        ))
    ))]
    if origin == interrupt_origin::user && _interactive_input {
        return super::dispatcher_runtime::resume_interactive_input(frame_address);
    }
    #[cfg(all(
        feature = "native_editor_workspace_test",
        not(test),
        not(any(
            feature = "ny_object_test",
            feature = "native_application_test",
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
    if origin == interrupt_origin::user && _workspace_input {
        return super::dispatcher_runtime::resume_interrupt(frame_address);
    }
    frame_address
}

#[cfg(all(
    any(
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ),
    not(test)
))]
fn dispatch_scheduler_timer(
    frame_address: *mut interrupt_frame,
    instant: crate::time::monotonic_instant,
) -> *mut interrupt_frame {
    let Some(selected_frame_address) =
        super::scheduler_test::dispatch_timer_interrupt(frame_address, instant)
    else {
        interrupt_fatal(interrupt_fatal_reason::invalid_frame);
    };
    if !interrupt_frame_address_is_valid(selected_frame_address)
        || !super::scheduler_test::user_interrupt_frame_is_valid(selected_frame_address)
    {
        interrupt_fatal(interrupt_fatal_reason::invalid_frame);
    }
    // SAFETY: The scheduler accepted the selected pointer as a complete, live
    // frame owned by the newly active task.
    let selected_frame = unsafe { ptr::read(selected_frame_address) };
    if interrupt_frame_origin(&selected_frame) != Some(interrupt_origin::user) {
        interrupt_fatal(interrupt_fatal_reason::invalid_frame);
    }
    selected_frame_address
}

#[cfg(test)]
fn interrupt_frame_is_valid(frame: &interrupt_frame) -> bool {
    interrupt_frame_origin(frame) == Some(interrupt_origin::kernel)
}

fn interrupt_frame_origin(frame: &interrupt_frame) -> Option<interrupt_origin> {
    if frame.error_code != 0 || frame.flags & (1 << 1) == 0 {
        return None;
    }
    if frame.code_segment == u64::from(descriptors::kernel_code_selector) {
        return Some(interrupt_origin::kernel);
    }
    #[cfg(all(
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ),
        not(test)
    ))]
    if frame.code_segment == u64::from(descriptors::user_code_selector) {
        return Some(interrupt_origin::user);
    }
    None
}

fn interrupt_frame_address_is_valid(frame_address: *const interrupt_frame) -> bool {
    !frame_address.is_null()
        && (frame_address as usize) & (core::mem::align_of::<interrupt_frame>() - 1) == 0
}

fn interrupt_fatal(reason: interrupt_fatal_reason) -> ! {
    disable_external_interrupts();
    // A static record keeps terminal IRQ failures attributable without
    // formatting, allocating, or projecting any input content into telemetry.
    let mut serial = crate::console::serial_port::com1();
    let _ = serial.write_str(reason.record());
    // SAFETY: External delivery is disabled and fully masked, NMI remains
    // masked, and the BSP will never resume an interrupted display writer.
    unsafe { crate::console::draw_registered_interrupt_failure() };
    super::qemu_exit_failure();
    super::halt_forever()
}

fn initialize_pic() {
    let mut ports = hardware_pic_ports;
    initialize_pic_with(&mut ports);
    master_mask.store(pic_all_masked, Ordering::Relaxed);
    slave_mask.store(pic_all_masked, Ordering::Relaxed);
}

fn initialize_pic_with(ports: &mut impl pic_ports) {
    ports.write(pic_master_data_port, pic_all_masked);
    ports.write(pic_slave_data_port, pic_all_masked);
    for (port, value) in [
        (pic_master_command_port, 0x11),
        (pic_slave_command_port, 0x11),
        (pic_master_data_port, pic_master_vector),
        (pic_slave_data_port, pic_slave_vector),
        (pic_master_data_port, 0x04),
        (pic_slave_data_port, 0x02),
        (pic_master_data_port, 0x01),
        (pic_slave_data_port, 0x01),
    ] {
        ports.write(port, value);
        pic_io_wait(ports);
    }
    ports.write(pic_master_data_port, pic_all_masked);
    ports.write(pic_slave_data_port, pic_all_masked);
}

fn initialize_pit() {
    let mut ports = hardware_pit_ports;
    initialize_pit_with(&mut ports);
}

fn initialize_pit_with(ports: &mut impl pit_ports) {
    let [divisor_low, divisor_high] = pit_divisor_bytes();
    ports.write(pit_command_port, pit_channel_0_mode_2);
    ports.write(pit_channel_0_port, divisor_low);
    ports.write(pit_channel_0_port, divisor_high);
}

const fn pit_divisor_bytes() -> [u8; 2] {
    [pit_divisor as u8, (pit_divisor >> 8) as u8]
}

fn initialize_ps2() -> ps2_setup_status {
    let mut ports = hardware_ps2_ports;
    initialize_ps2_with(&mut ports)
}

fn initialize_ps2_with(ports: &mut impl ps2_ports) -> ps2_setup_status {
    if controller_command(ports, ps2_command_disable_first_port).is_err()
        || controller_command(ports, ps2_command_disable_second_port).is_err()
    {
        return ps2_setup_status::controller_timeout;
    }
    if !flush_ps2_output(ports) {
        return ps2_setup_status::controller_output_stuck;
    }

    let original_configuration = match read_ps2_configuration(ports) {
        Ok(configuration) => configuration,
        Err(error) => return error,
    };
    let setup_configuration = (original_configuration & ps2_configuration_system)
        | ps2_configuration_first_disabled
        | ps2_configuration_second_disabled
        | ps2_configuration_translation;
    if write_ps2_configuration(ports, setup_configuration).is_err() {
        return ps2_setup_status::controller_timeout;
    }

    if controller_command(ports, ps2_command_self_test).is_err() {
        return ps2_setup_status::controller_timeout;
    }
    match read_ps2_output(ports) {
        Ok(ps2_controller_self_test_passed) => {}
        Ok(_) => return ps2_setup_status::controller_self_test_failed,
        Err(error) => return error,
    }
    // Controller self-test may reset its configuration byte.
    if write_ps2_configuration(ports, setup_configuration).is_err() {
        return ps2_setup_status::controller_timeout;
    }
    if controller_command(ports, ps2_command_test_first_port).is_err() {
        return ps2_setup_status::controller_timeout;
    }
    match read_ps2_output(ports) {
        Ok(ps2_first_port_test_passed) => {}
        Ok(_) => return ps2_setup_status::first_port_test_failed,
        Err(error) => return error,
    }
    if controller_command(ports, ps2_command_enable_first_port).is_err() {
        return ps2_setup_status::controller_timeout;
    }

    if send_ps2_device_byte(ports, ps2_device_reset).is_err() {
        return ps2_setup_status::device_rejected_command;
    }
    match read_ps2_output(ports) {
        Ok(ps2_device_self_test_passed) => {}
        Ok(_) => return ps2_setup_status::device_self_test_failed,
        Err(error) => return error,
    }
    for byte in [
        ps2_device_disable_scanning,
        ps2_device_select_scan_set,
        ps2_device_scan_set_2,
    ] {
        if send_ps2_device_byte(ports, byte).is_err() {
            return ps2_setup_status::device_rejected_command;
        }
    }

    let final_configuration = (original_configuration & ps2_configuration_system)
        | ps2_configuration_first_interrupt
        | ps2_configuration_second_disabled
        | ps2_configuration_translation;
    if write_ps2_configuration(ports, final_configuration).is_err() {
        return ps2_setup_status::controller_timeout;
    }
    let observed_configuration = match read_ps2_configuration(ports) {
        Ok(configuration) => configuration,
        Err(error) => return error,
    };
    let controlled_bits = ps2_configuration_first_interrupt
        | ps2_configuration_second_interrupt
        | ps2_configuration_first_disabled
        | ps2_configuration_second_disabled
        | ps2_configuration_translation;
    if observed_configuration & controlled_bits != final_configuration & controlled_bits {
        return ps2_setup_status::configuration_mismatch;
    }
    if send_ps2_device_byte(ports, ps2_device_enable_scanning).is_err() {
        return ps2_setup_status::device_rejected_command;
    }
    ps2_setup_status::ready
}

fn controller_command(ports: &mut impl ps2_ports, command: u8) -> Result<(), ps2_setup_status> {
    wait_for_ps2_input_clear(ports)?;
    ports.write_command(command);
    Ok(())
}

fn controller_data(ports: &mut impl ps2_ports, data: u8) -> Result<(), ps2_setup_status> {
    wait_for_ps2_input_clear(ports)?;
    ports.write_data(data);
    Ok(())
}

fn read_ps2_configuration(ports: &mut impl ps2_ports) -> Result<u8, ps2_setup_status> {
    controller_command(ports, ps2_command_read_configuration)?;
    read_ps2_output(ports)
}

fn write_ps2_configuration(
    ports: &mut impl ps2_ports,
    configuration: u8,
) -> Result<(), ps2_setup_status> {
    controller_command(ports, ps2_command_write_configuration)?;
    controller_data(ports, configuration)
}

fn send_ps2_device_byte(ports: &mut impl ps2_ports, byte: u8) -> Result<(), ps2_setup_status> {
    for _ in 0..ps2_resend_budget {
        controller_data(ports, byte)?;
        match read_ps2_output(ports)? {
            ps2_device_acknowledge => return Ok(()),
            ps2_device_resend => {}
            _ => return Err(ps2_setup_status::device_rejected_command),
        }
    }
    Err(ps2_setup_status::device_rejected_command)
}

fn wait_for_ps2_input_clear(ports: &mut impl ps2_ports) -> Result<(), ps2_setup_status> {
    for _ in 0..ps2_poll_budget {
        if ports.read_status() & ps2_status_input_full == 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(ps2_setup_status::controller_timeout)
}

fn read_ps2_output(ports: &mut impl ps2_ports) -> Result<u8, ps2_setup_status> {
    for _ in 0..ps2_poll_budget {
        let status = ports.read_status();
        if status & ps2_status_output_full == 0 {
            core::hint::spin_loop();
            continue;
        }
        let value = ports.read_data();
        if status & (ps2_status_error | ps2_status_auxiliary) != 0 {
            return Err(ps2_setup_status::controller_error);
        }
        return Ok(value);
    }
    Err(ps2_setup_status::controller_timeout)
}

fn flush_ps2_output(ports: &mut impl ps2_ports) -> bool {
    for _ in 0..ps2_flush_budget {
        if ports.read_status() & ps2_status_output_full == 0 {
            return true;
        }
        let _ = ports.read_data();
    }
    ports.read_status() & ps2_status_output_full == 0
}

fn handle_keyboard_interrupt() {
    if !input_available.load(Ordering::Acquire) {
        mask_pic_line(1);
        return;
    }

    let mut drained = 0usize;
    while drained < ps2_irq_drain_budget {
        let status = read_port(ps2_status_command_port);
        if status & ps2_status_output_full == 0 {
            break;
        }
        let scan_byte = read_port(ps2_data_port);
        drained += 1;
        if drained == 1 {
            empty_keyboard_streak.store(0, Ordering::Relaxed);
        }
        // SAFETY: The interrupt gate keeps IF clear and the BSP is the only
        // producer for both the clock snapshot and PS/2 event queue.
        let observed_at = unsafe { crate::time::now_if_disabled() };
        if status & ps2_status_error != 0 {
            // SAFETY: This is the same IF-clear, single-producer context.
            let publication = unsafe {
                crate::input::record_ps2_discontinuity(
                    observed_at,
                    ps2_discontinuity::controller_error,
                )
            };
            contain_input_publication(publication);
            return;
        }
        if status & ps2_status_auxiliary != 0 {
            // SAFETY: This is the same IF-clear, single-producer context.
            let publication = unsafe {
                crate::input::record_ps2_discontinuity(
                    observed_at,
                    ps2_discontinuity::auxiliary_data,
                )
            };
            contain_input_publication(publication);
            return;
        }
        // SAFETY: This is the sole interrupt producer, and the queue storage is
        // initialized before IRQ1 can be unmasked.
        let publication = unsafe { crate::input::publish_ps2_sample(scan_byte, observed_at) };
        if publication.is_lost() {
            contain_input_publication(publication);
            return;
        }
    }

    if drained == 0 {
        increment_saturating(&empty_keyboard_interrupts);
        if record_empty_keyboard_interrupt(&empty_keyboard_streak) {
            // SAFETY: IF remains clear and this is the sole PS/2 producer.
            let observed_at = unsafe { crate::time::now_if_disabled() };
            // SAFETY: The queue and sequence state are exclusively produced here.
            let publication = unsafe {
                crate::input::record_ps2_discontinuity(
                    observed_at,
                    ps2_discontinuity::controller_error,
                )
            };
            contain_input_publication(publication);
        }
        return;
    }
    if drained == ps2_irq_drain_budget
        && read_port(ps2_status_command_port) & ps2_status_output_full != 0
    {
        // SAFETY: IF remains clear and this is the sole PS/2 producer.
        let observed_at = unsafe { crate::time::now_if_disabled() };
        // SAFETY: The queue and sequence state are exclusively produced here.
        let publication = unsafe {
            crate::input::record_ps2_discontinuity(observed_at, ps2_discontinuity::controller_error)
        };
        contain_input_publication(publication);
    }
}

fn contain_input_publication(publication: ps2_sample_publication) {
    if publication.terminal_reason().is_some() {
        interrupt_fatal(interrupt_fatal_reason::event_identity_exhausted);
    }
    if publication.is_lost() {
        disable_keyboard_input();
    }
}

fn disable_keyboard_input() {
    input_available.store(false, Ordering::Release);
    mask_pic_line(1);
}

fn acknowledgement_for(irq: u8, in_service: u8) -> interrupt_acknowledgement {
    match irq {
        0..=6 => interrupt_acknowledgement::master,
        7 if in_service & (1 << 7) == 0 => interrupt_acknowledgement::none,
        7 => interrupt_acknowledgement::master,
        8..=14 => interrupt_acknowledgement::slave_then_master,
        15 if in_service & (1 << 7) == 0 => interrupt_acknowledgement::master,
        15 => interrupt_acknowledgement::slave_then_master,
        _ => interrupt_acknowledgement::none,
    }
}

const fn pic_source_is_unmasked(irq: u8, selected_master: u8, selected_slave: u8) -> bool {
    if irq < 8 {
        selected_master & (1 << irq) == 0
    } else if irq < 16 {
        selected_master & (1 << 2) == 0 && selected_slave & (1 << (irq - 8)) == 0
    } else {
        false
    }
}

fn acknowledge(acknowledgement: interrupt_acknowledgement) {
    let mut ports = hardware_pic_ports;
    acknowledge_with(&mut ports, acknowledgement);
}

fn acknowledge_with(ports: &mut impl pic_ports, acknowledgement: interrupt_acknowledgement) {
    match acknowledgement {
        interrupt_acknowledgement::none => {}
        interrupt_acknowledgement::master => {
            ports.write(pic_master_command_port, pic_end_of_interrupt);
        }
        interrupt_acknowledgement::slave_then_master => {
            ports.write(pic_slave_command_port, pic_end_of_interrupt);
            ports.write(pic_master_command_port, pic_end_of_interrupt);
        }
    }
}

fn read_pic_in_service(command_port: u16) -> u8 {
    let mut ports = hardware_pic_ports;
    read_pic_in_service_with(&mut ports, command_port)
}

fn read_pic_in_service_with(ports: &mut impl pic_ports, command_port: u16) -> u8 {
    ports.write(command_port, pic_read_in_service);
    ports.read(command_port)
}

fn mask_pic_line(irq: u8) {
    if irq < 8 {
        let selected = master_mask.load(Ordering::Relaxed) | (1 << irq);
        write_master_mask(selected);
    } else if irq < 16 {
        let selected = slave_mask.load(Ordering::Relaxed) | (1 << (irq - 8));
        write_slave_mask(selected);
    }
}

fn unmask_keyboard_line() {
    let selected = master_mask.load(Ordering::Relaxed) & !pic_keyboard_mask;
    write_master_mask(selected);
}

fn mask_all_pic_lines() {
    write_master_mask(pic_all_masked);
    write_slave_mask(pic_all_masked);
}

fn write_master_mask(mask: u8) {
    master_mask.store(mask, Ordering::Relaxed);
    write_port(pic_master_data_port, mask);
}

fn write_slave_mask(mask: u8) {
    slave_mask.store(mask, Ordering::Relaxed);
    write_port(pic_slave_data_port, mask);
}

fn increment_saturating(counter: &AtomicU64) {
    // Interrupt gates exclude nested writers, NMI remains masked, and no AP is
    // started in this slice. One atomic load and at most one store therefore
    // provide a fixed instruction bound while readers may still sample safely.
    let current = counter.load(Ordering::Relaxed);
    if let Some(next) = current.checked_add(1) {
        counter.store(next, Ordering::Relaxed);
    }
}

fn reset_empty_keyboard_streak(streak: &AtomicU8) {
    streak.store(0, Ordering::Relaxed);
}

fn record_empty_keyboard_interrupt(streak: &AtomicU8) -> bool {
    let next = streak.load(Ordering::Relaxed).saturating_add(1);
    streak.store(next, Ordering::Relaxed);
    next >= ps2_empty_irq_streak_limit
}

fn read_port(port: u16) -> u8 {
    let value: u8;
    // SAFETY: The legacy-PC backend owns the fixed controller port selected by
    // its caller and only exists on the current x86-64 platform path.
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

fn write_port(port: u16, value: u8) {
    // SAFETY: The legacy-PC backend owns the fixed controller port selected by
    // its caller and serializes protocol-dependent accesses itself.
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

fn pic_io_wait(ports: &mut impl pic_ports) {
    ports.write(0x80, 0);
}

#[cfg(test)]
mod tests {
    extern crate std;

    use core::mem;
    use std::collections::VecDeque;
    use std::vec::Vec;

    use super::*;

    #[derive(Default)]
    struct simulated_pic_ports {
        reads: VecDeque<u8>,
        writes: Vec<(u16, u8)>,
    }

    impl pic_ports for simulated_pic_ports {
        fn read(&mut self, _port: u16) -> u8 {
            self.reads.pop_front().unwrap_or(0)
        }

        fn write(&mut self, port: u16, value: u8) {
            self.writes.push((port, value));
        }
    }

    struct simulated_ps2_ports {
        configuration: u8,
        output: VecDeque<u8>,
        writes: Vec<(u16, u8)>,
        expecting_configuration: bool,
        input_stuck: bool,
        reject_device_commands: bool,
        resend_once: bool,
        resent: bool,
        controller_self_test: u8,
        first_port_test: u8,
    }

    #[derive(Default)]
    struct simulated_pit_ports {
        writes: Vec<(u16, u8)>,
    }

    impl pit_ports for simulated_pit_ports {
        fn write(&mut self, port: u16, value: u8) {
            self.writes.push((port, value));
        }
    }

    impl simulated_ps2_ports {
        fn working() -> Self {
            Self {
                configuration: ps2_configuration_system
                    | ps2_configuration_first_interrupt
                    | ps2_configuration_second_interrupt
                    | ps2_configuration_translation,
                output: VecDeque::new(),
                writes: Vec::new(),
                expecting_configuration: false,
                input_stuck: false,
                reject_device_commands: false,
                resend_once: false,
                resent: false,
                controller_self_test: ps2_controller_self_test_passed,
                first_port_test: ps2_first_port_test_passed,
            }
        }

        fn device_writes(&self) -> Vec<u8> {
            self.writes
                .iter()
                .filter_map(|(port, value)| (*port == ps2_data_port).then_some(*value))
                .collect()
        }
    }

    impl ps2_ports for simulated_ps2_ports {
        fn read_status(&mut self) -> u8 {
            let input = if self.input_stuck {
                ps2_status_input_full
            } else {
                0
            };
            let output = if self.output.is_empty() {
                0
            } else {
                ps2_status_output_full
            };
            input | output
        }

        fn read_data(&mut self) -> u8 {
            self.output.pop_front().unwrap_or(0)
        }

        fn write_command(&mut self, command: u8) {
            self.writes.push((ps2_status_command_port, command));
            match command {
                ps2_command_read_configuration => self.output.push_back(self.configuration),
                ps2_command_write_configuration => self.expecting_configuration = true,
                ps2_command_disable_first_port => {
                    self.configuration |= ps2_configuration_first_disabled;
                }
                ps2_command_disable_second_port => {
                    self.configuration |= ps2_configuration_second_disabled;
                }
                ps2_command_self_test => {
                    self.configuration = 0;
                    self.output.push_back(self.controller_self_test);
                }
                ps2_command_test_first_port => self.output.push_back(self.first_port_test),
                ps2_command_enable_first_port => {
                    self.configuration &= !ps2_configuration_first_disabled;
                }
                _ => {}
            }
        }

        fn write_data(&mut self, data: u8) {
            self.writes.push((ps2_data_port, data));
            if self.expecting_configuration {
                self.configuration = data;
                self.expecting_configuration = false;
                return;
            }
            if self.reject_device_commands {
                self.output.push_back(0xfc);
            } else if self.resend_once && !self.resent {
                self.resent = true;
                self.output.push_back(ps2_device_resend);
            } else {
                self.output.push_back(ps2_device_acknowledge);
                if data == ps2_device_reset {
                    self.output.push_back(ps2_device_self_test_passed);
                }
            }
        }
    }

    #[test]
    fn interrupt_frame_matches_the_returning_assembly_prefix() {
        assert_eq!(mem::size_of::<interrupt_frame>(), 20 * 8);
        assert_eq!(mem::align_of::<interrupt_frame>(), 8);
        assert_eq!(mem::offset_of!(interrupt_frame, accumulator), 0);
        assert_eq!(mem::offset_of!(interrupt_frame, register_15), 14 * 8);
        assert_eq!(mem::offset_of!(interrupt_frame, vector), 15 * 8);
        assert_eq!(mem::offset_of!(interrupt_frame, error_code), 16 * 8);
        assert_eq!(
            mem::offset_of!(interrupt_frame, instruction_pointer),
            17 * 8
        );
        assert_eq!(mem::offset_of!(interrupt_frame, code_segment), 18 * 8);
        assert_eq!(mem::offset_of!(interrupt_frame, flags), 19 * 8);
    }

    #[test]
    fn interrupt_frame_validation_rejects_impossible_return_state() {
        let mut frame = interrupt_frame {
            accumulator: 0,
            base: 0,
            counter: 0,
            data: 0,
            frame_pointer: 0,
            source_index: 0,
            destination_index: 0,
            register_8: 0,
            register_9: 0,
            register_10: 0,
            register_11: 0,
            register_12: 0,
            register_13: 0,
            register_14: 0,
            register_15: 0,
            vector: u64::from(pic_master_vector),
            error_code: 0,
            instruction_pointer: 0x10_0000,
            code_segment: u64::from(descriptors::kernel_code_selector),
            flags: 1 << 1,
        };
        assert!(interrupt_frame_is_valid(&frame));

        frame.error_code = 1;
        assert!(!interrupt_frame_is_valid(&frame));
        frame.error_code = 0;
        frame.flags = 0;
        assert!(!interrupt_frame_is_valid(&frame));
        frame.flags = 1 << 1;
        frame.code_segment = 3;
        assert!(!interrupt_frame_is_valid(&frame));
        frame.code_segment = (1_u64 << 32) | u64::from(descriptors::kernel_code_selector);
        assert!(!interrupt_frame_is_valid(&frame));
    }

    #[test]
    fn interrupt_frame_address_validation_is_exact() {
        let frame = core::mem::MaybeUninit::<interrupt_frame>::uninit();
        let address = frame.as_ptr();

        assert!(interrupt_frame_address_is_valid(address));
        assert!(!interrupt_frame_address_is_valid(ptr::null()));
        assert!(!interrupt_frame_address_is_valid(
            address.cast::<u8>().wrapping_add(1).cast()
        ));
    }

    #[test]
    fn scheduler_timer_mask_keeps_only_the_initialized_pit_line() {
        assert_eq!(scheduler_timer_master_mask(), 0xfe);
        assert_eq!(scheduler_timer_master_mask() & pic_timer_mask, 0);
        assert_ne!(scheduler_timer_master_mask() & pic_keyboard_mask, 0);
    }

    #[test]
    fn critical_sections_require_prepared_and_active_delivery() {
        assert!(critical_section_available(initialization_ready, true));
        assert!(!critical_section_available(initialization_ready, false));
        assert!(!critical_section_available(
            initialization_uninitialized,
            true
        ));
        assert!(!critical_section_available(initialization_failed, true));
    }

    #[cfg(feature = "desktop_dev")]
    #[test]
    fn bootstrap_presentation_cannot_revive_or_borrow_an_initialized_backend() {
        assert!(bootstrap_presentation_state_is_virgin(
            initialization_uninitialized,
            255,
            255,
            false,
            false,
            false,
        ));
        for state in 1..=u8::MAX {
            assert!(!bootstrap_presentation_state_is_virgin(
                state, 255, 255, false, false, false,
            ));
        }
        for mask in 0..255 {
            assert!(!bootstrap_presentation_state_is_virgin(
                initialization_uninitialized,
                mask,
                255,
                false,
                false,
                false,
            ));
            assert!(!bootstrap_presentation_state_is_virgin(
                initialization_uninitialized,
                255,
                mask,
                false,
                false,
                false,
            ));
        }
        for flags in 1..8 {
            assert!(!bootstrap_presentation_state_is_virgin(
                initialization_uninitialized,
                255,
                255,
                flags & 1 != 0,
                flags & 2 != 0,
                flags & 4 != 0,
            ));
        }
    }

    #[test]
    fn pic_vectors_do_not_overlap_architectural_exceptions() {
        assert_eq!(pic_master_vector, 0x20);
        assert_eq!(pic_slave_vector, 0x28);
        assert_eq!(usize::from(pic_master_vector) + pic_vector_count - 1, 0x2f);
    }

    #[test]
    fn pic_spurious_acknowledgements_are_exact() {
        assert_eq!(acknowledgement_for(7, 0), interrupt_acknowledgement::none);
        assert_eq!(
            acknowledgement_for(7, 1 << 7),
            interrupt_acknowledgement::master
        );
        assert_eq!(
            acknowledgement_for(15, 0),
            interrupt_acknowledgement::master
        );
        assert_eq!(
            acknowledgement_for(15, 1 << 7),
            interrupt_acknowledgement::slave_then_master
        );
    }

    #[test]
    fn pic_in_service_query_selects_the_isr_register() {
        let mut ports = simulated_pic_ports::default();
        ports.reads.push_back(0x80);
        assert_eq!(
            read_pic_in_service_with(&mut ports, pic_slave_command_port),
            0x80
        );
        assert_eq!(
            ports.writes.as_slice(),
            &[(pic_slave_command_port, pic_read_in_service)]
        );
    }

    #[test]
    fn ordinary_pic_acknowledgements_follow_the_cascade() {
        for irq in 0..7 {
            assert_eq!(
                acknowledgement_for(irq, 0),
                interrupt_acknowledgement::master
            );
        }
        for irq in 8..15 {
            assert_eq!(
                acknowledgement_for(irq, 0),
                interrupt_acknowledgement::slave_then_master
            );
        }
    }

    #[test]
    fn pic_initialization_masks_and_programs_each_controller_in_order() {
        let mut ports = simulated_pic_ports::default();
        initialize_pic_with(&mut ports);
        assert_eq!(
            ports.writes.as_slice(),
            &[
                (pic_master_data_port, pic_all_masked),
                (pic_slave_data_port, pic_all_masked),
                (pic_master_command_port, 0x11),
                (0x80, 0),
                (pic_slave_command_port, 0x11),
                (0x80, 0),
                (pic_master_data_port, pic_master_vector),
                (0x80, 0),
                (pic_slave_data_port, pic_slave_vector),
                (0x80, 0),
                (pic_master_data_port, 0x04),
                (0x80, 0),
                (pic_slave_data_port, 0x02),
                (0x80, 0),
                (pic_master_data_port, 0x01),
                (0x80, 0),
                (pic_slave_data_port, 0x01),
                (0x80, 0),
                (pic_master_data_port, pic_all_masked),
                (pic_slave_data_port, pic_all_masked),
            ]
        );
    }

    #[test]
    fn pic_end_of_interrupt_writes_follow_controller_order() {
        let mut ports = simulated_pic_ports::default();
        acknowledge_with(&mut ports, interrupt_acknowledgement::none);
        assert!(ports.writes.is_empty());

        acknowledge_with(&mut ports, interrupt_acknowledgement::master);
        assert_eq!(
            ports.writes.as_slice(),
            &[(pic_master_command_port, pic_end_of_interrupt)]
        );

        ports.writes.clear();
        acknowledge_with(&mut ports, interrupt_acknowledgement::slave_then_master);
        assert_eq!(
            ports.writes.as_slice(),
            &[
                (pic_slave_command_port, pic_end_of_interrupt),
                (pic_master_command_port, pic_end_of_interrupt),
            ]
        );
    }

    #[test]
    fn source_arming_follows_both_pic_masks() {
        assert!(pic_source_is_unmasked(0, 0xfc, 0xff));
        assert!(pic_source_is_unmasked(1, 0xfc, 0xff));
        assert!(!pic_source_is_unmasked(2, 0xfc, 0xff));
        assert!(!pic_source_is_unmasked(8, 0xfb, 0xff));
        assert!(pic_source_is_unmasked(8, 0xfb, 0xfe));
        assert!(!pic_source_is_unmasked(16, 0x00, 0x00));
    }

    #[test]
    fn interrupt_counters_increment_once_and_saturate_without_retry() {
        let counter = AtomicU64::new(41);
        increment_saturating(&counter);
        assert_eq!(counter.load(Ordering::Relaxed), 42);
        counter.store(u64::MAX, Ordering::Relaxed);
        increment_saturating(&counter);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn pit_programming_and_divisor_are_exact() {
        let target_hz = 100u32;
        let rounded = (pit_input_hz + target_hz / 2) / target_hz;
        assert_eq!(u32::from(pit_divisor), rounded);
        assert!(pit_divisor >= 2);
        assert_eq!(pit_divisor_bytes(), [0x9c, 0x2e]);

        let mut ports = simulated_pit_ports::default();
        initialize_pit_with(&mut ports);
        assert_eq!(
            ports.writes.as_slice(),
            &[
                (pit_command_port, pit_channel_0_mode_2),
                (pit_channel_0_port, 0x9c),
                (pit_channel_0_port, 0x2e),
            ]
        );
    }

    #[test]
    fn multi_region_timer_rearm_requires_unpublished_exclusion_and_is_one_shot() {
        let rearmed = AtomicBool::new(false);
        let mut ports = simulated_pit_ports::default();
        for (state, enabled, master, slave, input) in [
            (0, false, 0xfe, 0xff, false),
            (1, false, 0xfe, 0xff, false),
            (2, true, 0xfe, 0xff, false),
            (2, false, 0xff, 0xff, false),
            (2, false, 0xfc, 0xff, false),
            (2, false, 0xfe, 0xfe, false),
            (2, false, 0xfe, 0xff, true),
        ] {
            assert!(!rearm_multi_region_timer_with(
                &mut ports, &rearmed, state, enabled, master, slave, input,
            ));
            assert!(ports.writes.is_empty());
            assert!(!rearmed.load(Ordering::Acquire));
        }
        assert!(rearm_multi_region_timer_with(
            &mut ports, &rearmed, 2, false, 0xfe, 0xff, false,
        ));
        assert_eq!(
            ports.writes.as_slice(),
            &[
                (pit_command_port, 0x34),
                (pit_channel_0_port, 0x9c),
                (pit_channel_0_port, 0x2e)
            ]
        );
        ports.writes.clear();
        assert!(!rearm_multi_region_timer_with(
            &mut ports, &rearmed, 2, false, 0xfe, 0xff, false,
        ));
        assert!(ports.writes.is_empty());
        assert!(rearmed.load(Ordering::Acquire));
    }

    #[test]
    fn source_timer_mask_transitions_require_exclusion_and_preserve_terminal_failure() {
        for transition in [
            source_timer_transition::quiesce_producer,
            source_timer_transition::restore_consumer,
        ] {
            let (phase_value, required_master) = match transition {
                source_timer_transition::quiesce_producer => (source_timer_producer, 0xfe),
                source_timer_transition::restore_consumer => (source_timer_quiesced, 0xff),
            };
            let valid = source_timer_observation {
                state: initialization_ready,
                if_enabled: false,
                master: required_master,
                slave: 0xff,
                input: false,
                first_rearmed: true,
            };
            for observed in [
                source_timer_observation {
                    state: initialization_uninitialized,
                    ..valid
                },
                source_timer_observation {
                    state: initialization_in_progress,
                    ..valid
                },
                source_timer_observation {
                    state: initialization_failed,
                    ..valid
                },
                source_timer_observation {
                    if_enabled: true,
                    ..valid
                },
                source_timer_observation {
                    master: 0xfc,
                    ..valid
                },
                source_timer_observation {
                    master: required_master ^ 1,
                    ..valid
                },
                source_timer_observation {
                    slave: 0xfe,
                    ..valid
                },
                source_timer_observation {
                    input: true,
                    ..valid
                },
                source_timer_observation {
                    first_rearmed: false,
                    ..valid
                },
            ] {
                let phase = AtomicU8::new(phase_value);
                let mut ports = simulated_pic_ports::default();
                assert!(!source_timer_transition_with(
                    &mut ports, &phase, observed, transition
                ));
                assert!(ports.writes.is_empty());
                assert_eq!(phase.load(Ordering::Acquire), phase_value);
            }
        }
    }

    #[test]
    fn source_timer_restore_and_rearm_follow_one_complete_retirement_transition() {
        let phase = AtomicU8::new(source_timer_producer);
        let rearmed = AtomicBool::new(false);
        let mut pic = simulated_pic_ports::default();
        let mut pit = simulated_pit_ports::default();
        let running = source_timer_observation {
            state: initialization_ready,
            if_enabled: false,
            master: 0xfe,
            slave: 0xff,
            input: false,
            first_rearmed: true,
        };
        let masked = source_timer_observation {
            master: 0xff,
            ..running
        };
        // A masked observation alone does not prove producer quiescence.
        assert!(!source_timer_transition_with(
            &mut pic,
            &phase,
            masked,
            source_timer_transition::restore_consumer,
        ));
        assert!(!rearm_source_consumer_timer_with(
            &mut pit, &rearmed, &phase, running
        ));
        assert!(pic.writes.is_empty());
        assert!(pit.writes.is_empty());
        assert!(source_timer_transition_with(
            &mut pic,
            &phase,
            running,
            source_timer_transition::quiesce_producer,
        ));
        assert_eq!(
            pic.writes,
            [(pic_master_data_port, 0xff), (pic_slave_data_port, 0xff)]
        );
        assert_eq!(phase.load(Ordering::Acquire), source_timer_quiesced);
        pic.writes.clear();
        // Even an otherwise valid timer-only observation cannot skip restore.
        assert!(!rearm_source_consumer_timer_with(
            &mut pit, &rearmed, &phase, running
        ));
        assert!(!source_timer_transition_with(
            &mut pic,
            &phase,
            running,
            source_timer_transition::quiesce_producer,
        ));
        assert!(pic.writes.is_empty());
        assert!(source_timer_transition_with(
            &mut pic,
            &phase,
            masked,
            source_timer_transition::restore_consumer,
        ));
        assert_eq!(
            pic.writes,
            [(pic_master_data_port, 0xfe), (pic_slave_data_port, 0xff)]
        );
        assert_eq!(phase.load(Ordering::Acquire), source_timer_restored);
        pic.writes.clear();
        assert!(!source_timer_transition_with(
            &mut pic,
            &phase,
            masked,
            source_timer_transition::restore_consumer,
        ));
        for observed in [
            source_timer_observation {
                state: initialization_failed,
                ..running
            },
            source_timer_observation {
                first_rearmed: false,
                ..running
            },
            source_timer_observation {
                if_enabled: true,
                ..running
            },
            masked,
        ] {
            assert!(!rearm_source_consumer_timer_with(
                &mut pit, &rearmed, &phase, observed
            ));
            assert!(!rearmed.load(Ordering::Acquire));
            assert!(pit.writes.is_empty());
        }
        assert!(rearm_source_consumer_timer_with(
            &mut pit, &rearmed, &phase, running
        ));
        assert_eq!(
            pit.writes,
            [
                (pit_command_port, 0x34),
                (pit_channel_0_port, 0x9c),
                (pit_channel_0_port, 0x2e),
            ]
        );
        pit.writes.clear();
        assert!(!rearm_source_consumer_timer_with(
            &mut pit, &rearmed, &phase, running
        ));
        assert!(pit.writes.is_empty());
        assert!(pic.writes.is_empty());
    }

    #[test]
    fn fatal_records_are_static_structured_and_content_free() {
        for record in [
            interrupt_fatal_reason::invalid_frame.record(),
            interrupt_fatal_reason::invalid_vector.record(),
            interrupt_fatal_reason::source_not_armed.record(),
            interrupt_fatal_reason::unexpected_source.record(),
            interrupt_fatal_reason::time_exhausted.record(),
            interrupt_fatal_reason::event_identity_exhausted.record(),
        ] {
            assert!(record.starts_with("\n{\"schema\":1,"));
            assert!(record.contains("\"event\":\"kernel.interrupt.fatal\""));
            assert!(record.ends_with('\n'));
            assert!(!record.contains("scan"));
            assert!(!record.contains("key"));

            let interrupted = std::format!("partial_foreground_record{record}");
            let mut lines = interrupted.lines();
            assert_eq!(lines.next(), Some("partial_foreground_record"));
            assert!(
                lines
                    .next()
                    .is_some_and(|line| line.starts_with("{\"schema\":1,"))
            );
            assert!(lines.next().is_none());
        }
    }

    #[test]
    fn successful_ps2_rearm_starts_a_fresh_empty_irq_streak() {
        let streak = AtomicU8::new(ps2_empty_irq_streak_limit);
        reset_empty_keyboard_streak(&streak);
        for _ in 1..ps2_empty_irq_streak_limit {
            assert!(!record_empty_keyboard_interrupt(&streak));
        }
        assert!(record_empty_keyboard_interrupt(&streak));
    }

    #[test]
    fn controlled_ps2_configuration_is_deterministic() {
        let original = 0xff;
        let setup = (original & ps2_configuration_system)
            | ps2_configuration_first_disabled
            | ps2_configuration_second_disabled
            | ps2_configuration_translation;
        let final_configuration = (original & ps2_configuration_system)
            | ps2_configuration_first_interrupt
            | ps2_configuration_second_disabled
            | ps2_configuration_translation;

        assert_eq!(setup, 0x74);
        assert_eq!(final_configuration, 0x65);
        assert_eq!(final_configuration & ps2_configuration_second_interrupt, 0);
        assert_eq!(final_configuration & ps2_configuration_first_disabled, 0);
    }

    #[test]
    fn selected_masks_enable_only_live_sources() {
        let with_keyboard = pic_all_masked & !pic_timer_mask & !pic_keyboard_mask;
        let without_keyboard = pic_all_masked & !pic_timer_mask;

        assert_eq!(with_keyboard, 0xfc);
        assert_eq!(without_keyboard, 0xfe);
        assert_eq!(pic_all_masked, 0xff);
    }

    #[test]
    fn ps2_setup_owns_configuration_and_device_scan_set() {
        let mut ports = simulated_ps2_ports::working();

        assert_eq!(initialize_ps2_with(&mut ports), ps2_setup_status::ready);
        assert_eq!(ports.configuration, 0x65);
        assert_eq!(
            ports.device_writes(),
            [
                0x74,
                0x74,
                ps2_device_reset,
                ps2_device_disable_scanning,
                ps2_device_select_scan_set,
                ps2_device_scan_set_2,
                0x65,
                ps2_device_enable_scanning,
            ]
        );
    }

    #[test]
    fn ps2_setup_can_reestablish_a_controlled_transport_after_loss() {
        let mut ports = simulated_ps2_ports::working();

        assert_eq!(initialize_ps2_with(&mut ports), ps2_setup_status::ready);
        assert_eq!(initialize_ps2_with(&mut ports), ps2_setup_status::ready);
        assert_eq!(ports.configuration, 0x65);
        assert_eq!(
            ports
                .device_writes()
                .iter()
                .filter(|byte| **byte == ps2_device_reset)
                .count(),
            2
        );
        assert_eq!(
            ports
                .device_writes()
                .iter()
                .filter(|byte| **byte == ps2_device_enable_scanning)
                .count(),
            2
        );
    }

    #[test]
    fn ps2_setup_retries_one_resend_without_unbounded_recovery() {
        let mut ports = simulated_ps2_ports::working();
        ports.resend_once = true;

        assert_eq!(initialize_ps2_with(&mut ports), ps2_setup_status::ready);
        assert_eq!(
            ports
                .device_writes()
                .iter()
                .filter(|byte| **byte == ps2_device_reset)
                .count(),
            2
        );
    }

    #[test]
    fn ps2_setup_degrades_on_controller_or_device_failure() {
        let mut stuck = simulated_ps2_ports::working();
        stuck.input_stuck = true;
        assert_eq!(
            initialize_ps2_with(&mut stuck),
            ps2_setup_status::controller_timeout
        );

        let mut bad_controller = simulated_ps2_ports::working();
        bad_controller.controller_self_test = 0xfc;
        assert_eq!(
            initialize_ps2_with(&mut bad_controller),
            ps2_setup_status::controller_self_test_failed
        );

        let mut bad_port = simulated_ps2_ports::working();
        bad_port.first_port_test = 1;
        assert_eq!(
            initialize_ps2_with(&mut bad_port),
            ps2_setup_status::first_port_test_failed
        );

        let mut rejected = simulated_ps2_ports::working();
        rejected.reject_device_commands = true;
        assert_eq!(
            initialize_ps2_with(&mut rejected),
            ps2_setup_status::device_rejected_command
        );
    }
}

/// Select only PIT and the already initialized keyboard for this isolated
/// native profile. Storage remains polled with IF clear and no disk IRQ grant.
#[cfg(all(
    feature = "native_interactive_editor_test",
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
        feature = "desktop_dev",
        feature = "native_durable_failure_test",
        feature = "native_relaunch_test",
        feature = "elf_application_test"
    ))
))]
pub(super) fn restrict_to_interactive_input() -> bool {
    let was_enabled = disable_interrupts_and_read_previous();
    if !critical_section_available(initialization.load(Ordering::Acquire), was_enabled)
        || !input_available.load(Ordering::Acquire)
        || !ps2_configured.load(Ordering::Acquire)
    {
        return false;
    }
    let mask = pic_all_masked & !pic_timer_mask & !pic_keyboard_mask;
    write_master_mask(mask);
    write_slave_mask(pic_all_masked);
    master_mask.load(Ordering::Acquire) == mask
        && slave_mask.load(Ordering::Acquire) == pic_all_masked
        && !interrupts_enabled()
}
