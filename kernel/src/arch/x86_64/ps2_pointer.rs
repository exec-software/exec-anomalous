//! Feature-local legacy pointer transport. All content stays in copied input;
//! controller setup and queue access require IF clear on the single BSP.
use super::*;
use anomalous_abi::rite_physical_input_v1;
use core::cell::UnsafeCell;

const queue_capacity: usize = 64;
const command_enable_auxiliary: u8 = 0xa8;
const command_write_auxiliary: u8 = 0xd4;
const device_defaults: u8 = 0xf6;

static empty_pointer_streak: AtomicU8 = AtomicU8::new(0);
static session: AtomicBool = AtomicBool::new(false);
static available: AtomicBool = AtomicBool::new(false);
static pointer_state: source_cell = source_cell(UnsafeCell::new(pointer_source::new()));

#[derive(Clone, Copy)]
struct motion {
    generation: u32,
    sequence: u64,
    dx: i16,
    dy: i16,
    buttons: u8,
}

struct pointer_source {
    queue: [Option<motion>; queue_capacity],
    head: usize,
    length: usize,
    generation: u32,
    sequence: u64,
    packet: [u8; 3],
    packet_length: usize,
    lost: Option<u64>,
    loss_delivered: bool,
    stopped: bool,
}

impl pointer_source {
    const fn new() -> Self {
        Self {
            queue: [None; queue_capacity],
            head: 0,
            length: 0,
            generation: 0,
            sequence: 0,
            packet: [0; 3],
            packet_length: 0,
            lost: None,
            loss_delivered: false,
            stopped: true,
        }
    }

    fn start(&mut self) -> Option<u32> {
        let generation = self.generation.checked_add(1)?;
        *self = Self::new();
        self.generation = generation;
        self.stopped = false;
        Some(generation)
    }

    fn stop(&mut self) {
        let generation = self.generation;
        *self = Self::new();
        self.generation = generation;
    }

    fn lose(&mut self) {
        if !self.stopped {
            self.lost = Some(self.sequence);
            self.stopped = true;
            self.packet = [0; 3];
            self.packet_length = 0;
        }
    }

    /// A malformed header, overflow, controller loss, or full queue closes the
    /// generation. We never guess alignment by treating packet data as headers.
    fn push(&mut self, byte: u8, controller_error: bool) -> bool {
        if self.stopped {
            return false;
        }
        if controller_error || (self.packet_length == 0 && (byte & 8 == 0 || byte & 0xc0 != 0)) {
            self.lose();
            return false;
        }
        self.packet[self.packet_length] = byte;
        self.packet_length += 1;
        if self.packet_length != 3 {
            return true;
        }
        self.packet_length = 0;
        if self.length == queue_capacity || self.sequence == u64::MAX {
            self.lose();
            return false;
        }
        let flags = self.packet[0];
        // PS/2 motion is nine-bit sign/magnitude-in-two's-complement; the
        // sign bit comes from the header, not data bit seven.
        let dx = i16::from(self.packet[1]) - if flags & 0x10 != 0 { 256 } else { 0 };
        let dy = -(i16::from(self.packet[2]) - if flags & 0x20 != 0 { 256 } else { 0 });
        self.queue[(self.head + self.length) % queue_capacity] = Some(motion {
            generation: self.generation,
            sequence: self.sequence,
            dx,
            dy,
            buttons: flags & 7,
        });
        self.sequence += 1;
        self.length += 1;
        true
    }

    fn ready(&self) -> bool {
        self.length != 0 || (self.lost.is_some() && !self.loss_delivered)
    }

    fn pop(&mut self) -> Result<Option<rite_physical_input_v1>, &'static str> {
        if self.length != 0 {
            let motion = self.queue[self.head]
                .take()
                .ok_or("workspace_pointer_queue")?;
            self.head = (self.head + 1) % queue_capacity;
            self.length -= 1;
            return rite_physical_input_v1::pointer(
                motion.generation,
                motion.sequence,
                motion.dx,
                motion.dy,
                motion.buttons,
            )
            .map(Some)
            .ok_or("workspace_pointer_encoding");
        }
        if let Some(sequence) = self.lost.filter(|_| !self.loss_delivered) {
            let next = self
                .generation
                .checked_add(1)
                .ok_or("workspace_pointer_generation")?;
            let event =
                rite_physical_input_v1::pointer_loss(self.generation, next, sequence, sequence, 1)
                    .ok_or("workspace_pointer_loss_encoding")?;
            self.loss_delivered = true;
            return Ok(Some(event));
        }
        Ok(None)
    }
}

struct source_cell(UnsafeCell<pointer_source>);
// SAFETY: Every access below requires IF clear, BSP-only execution and masked
// NMI. No reference escapes a call or survives a blocking/interruptible region.
unsafe impl Sync for source_cell {}
impl source_cell {
    unsafe fn with<T>(&self, operation: impl FnOnce(&mut pointer_source) -> T) -> T {
        // SAFETY: The caller supplies exclusive interrupt/CPU exclusion.
        operation(unsafe { &mut *self.0.get() })
    }
}

pub(super) fn session_active() -> bool {
    session.load(Ordering::Acquire)
}

/// Caller holds IF clear. Startup is degraded on unavailable AUX hardware.
pub(in super::super) fn initialize() -> Result<(bool, u32), &'static str> {
    if interrupts_enabled() || initialization.load(Ordering::Acquire) != initialization_ready {
        return Err("workspace_pointer_entry_state");
    }
    // AUX and keyboard share one controller. Never revive a failed first
    // port and route unconfigured bytes as translated set-1 observations.
    if !first_port_ready(
        ps2_configured.load(Ordering::Acquire),
        input_available.load(Ordering::Acquire),
    ) {
        return Ok((false, 0));
    }
    mask_pointer();
    session.store(true, Ordering::Release);
    let mut ports = hardware_ps2_ports;
    let setup = initialize_with(&mut ports, route_keyboard);
    if setup.is_err() {
        stop_with(&mut ports, route_keyboard).map_err(|_| "workspace_pointer_restore")?;
        return Ok((false, 0));
    }
    // SAFETY: Startup is excluded from IRQ/other CPU access.
    let Some(generation) = (unsafe { pointer_state.with(pointer_source::start) }) else {
        stop_with(&mut ports, route_keyboard).map_err(|_| "workspace_pointer_restore")?;
        return Ok((false, 0));
    };
    empty_pointer_streak.store(0, Ordering::Relaxed);
    available.store(true, Ordering::Release);
    write_slave_mask(slave_mask.load(Ordering::Relaxed) & !(1 << 4));
    write_master_mask(master_mask.load(Ordering::Relaxed) & !(1 << 2));
    Ok((true, generation))
}

pub(in super::super) fn stop() -> bool {
    if !session_active() {
        return true;
    }
    mask_pointer();
    let mut ports = hardware_ps2_ports;
    let restored = stop_with(&mut ports, route_keyboard).is_ok();
    // SAFETY: The runtime has returned to its saved kernel continuation with
    // IF clear, and the AUX source is masked before clearing private storage.
    unsafe { pointer_state.with(pointer_source::stop) };
    session.store(false, Ordering::Release);
    restored
}

const fn first_port_ready(configured: bool, ready: bool) -> bool {
    configured && ready
}

fn mask_pointer() {
    available.store(false, Ordering::Release);
    write_slave_mask(slave_mask.load(Ordering::Relaxed) | (1 << 4));
    // No other slave IRQ is enabled by this bounded backend.
    write_master_mask(master_mask.load(Ordering::Relaxed) | (1 << 2));
}

pub(super) fn before_keyboard_reset() {
    if session_active() {
        mask_pointer();
        // SAFETY: The keyboard consumer holds IF clear for resynchronization.
        unsafe { pointer_state.with(pointer_source::lose) };
    }
}

pub(in super::super) fn ready() -> bool {
    // SAFETY: Called by the native runtime only with IF clear.
    unsafe { pointer_state.with(|state| state.ready()) }
}

pub(in super::super) fn poll() -> Result<Option<rite_physical_input_v1>, &'static str> {
    // SAFETY: The rite gate excludes all interrupt/CPU producers.
    let event = unsafe { pointer_state.with(pointer_source::pop) }?;
    if event.as_ref().is_some_and(|event| event.is_pointer_loss()) {
        mask_pointer();
        let mut ports = hardware_ps2_ports;
        // The loss is copied after this finite transport reset. No event from
        // the fresh generation may overtake its explicit cancellation boundary.
        initialize_with(&mut ports, route_keyboard).map_err(|_| "workspace_pointer_rearm")?;
        let next = unsafe { pointer_state.with(pointer_source::start) }
            .ok_or("workspace_pointer_generation")?;
        if event.as_ref().unwrap().new_generation() != next {
            return Err("workspace_pointer_generation");
        }
        empty_pointer_streak.store(0, Ordering::Relaxed);
        available.store(true, Ordering::Release);
        write_slave_mask(slave_mask.load(Ordering::Relaxed) & !(1 << 4));
        write_master_mask(master_mask.load(Ordering::Relaxed) & !(1 << 2));
    }
    Ok(event)
}

fn route_keyboard(byte: u8, error: bool) {
    // SAFETY: Controller helpers and shared ISR execute under IF-clear BSP
    // exclusion. Preserve first-port bytes while handling AUX commands.
    let instant = unsafe { crate::time::now_if_disabled() };
    let publication = if error {
        unsafe {
            crate::input::record_ps2_discontinuity(instant, ps2_discontinuity::controller_error)
        }
    } else {
        unsafe { crate::input::publish_ps2_sample(byte, instant) }
    };
    contain_input_publication(publication);
}

/// Both IRQ1 and IRQ12 enter this bounded demultiplexer while AUX is owned.
/// Drain budget exhaustion is an explicit discontinuity, never silent loss.
pub(super) fn interrupt(irq: u8) {
    let mut drained = 0;
    while drained < ps2_irq_drain_budget {
        let status = read_port(ps2_status_command_port);
        if status & ps2_status_output_full == 0 {
            break;
        }
        let byte = read_port(ps2_data_port);
        drained += 1;
        // Shared drain can service either device on either IRQ. Any byte is
        // real controller progress and breaks both consecutive-empty streaks.
        reset_empty_keyboard_streak(&empty_keyboard_streak);
        empty_pointer_streak.store(0, Ordering::Relaxed);
        if status & ps2_status_auxiliary == 0 {
            reset_empty_keyboard_streak(&empty_keyboard_streak);
            route_keyboard(byte, status & ps2_status_error != 0);
        } else if available.load(Ordering::Acquire) {
            // SAFETY: IRQ gate holds IF clear on the sole BSP.
            if !unsafe {
                pointer_state.with(|state| state.push(byte, status & ps2_status_error != 0))
            } {
                mask_pointer();
            }
        }
    }
    if drained == 0 {
        if irq == 1 {
            increment_saturating(&empty_keyboard_interrupts);
            if record_empty_keyboard_interrupt(&empty_keyboard_streak) {
                route_keyboard(0, true);
            }
        } else if irq == 12 && record_empty_keyboard_interrupt(&empty_pointer_streak) {
            // SAFETY: The IRQ gate still supplies exclusive BSP access.
            unsafe { pointer_state.with(pointer_source::lose) };
            mask_pointer();
        }
        return;
    }
    if drained == ps2_irq_drain_budget
        && read_port(ps2_status_command_port) & ps2_status_output_full != 0
    {
        // One unresolved controller stream makes either source uncertain.
        unsafe { pointer_state.with(pointer_source::lose) };
        mask_pointer();
        route_keyboard(0, true);
    }
}

fn drain_with(
    ports: &mut impl ps2_ports,
    key: &mut impl FnMut(u8, bool),
) -> Result<(), ps2_setup_status> {
    for _ in 0..ps2_flush_budget {
        let status = ports.read_status();
        if status & ps2_status_output_full == 0 {
            return Ok(());
        }
        let byte = ports.read_data();
        if status & ps2_status_auxiliary == 0 {
            key(byte, status & ps2_status_error != 0);
        }
    }
    Err(ps2_setup_status::controller_output_stuck)
}

fn read_auxiliary(
    ports: &mut impl ps2_ports,
    key: &mut impl FnMut(u8, bool),
) -> Result<u8, ps2_setup_status> {
    for _ in 0..ps2_poll_budget {
        let status = ports.read_status();
        if status & ps2_status_output_full == 0 {
            core::hint::spin_loop();
            continue;
        }
        let byte = ports.read_data();
        if status & ps2_status_auxiliary == 0 {
            key(byte, status & ps2_status_error != 0);
            continue;
        }
        if status & ps2_status_error != 0 {
            return Err(ps2_setup_status::controller_error);
        }
        return Ok(byte);
    }
    Err(ps2_setup_status::controller_timeout)
}

fn auxiliary_command(
    ports: &mut impl ps2_ports,
    key: &mut impl FnMut(u8, bool),
    byte: u8,
) -> Result<(), ps2_setup_status> {
    for _ in 0..ps2_resend_budget {
        controller_command(ports, command_write_auxiliary)?;
        controller_data(ports, byte)?;
        match read_auxiliary(ports, key)? {
            ps2_device_acknowledge => return Ok(()),
            ps2_device_resend => {}
            _ => return Err(ps2_setup_status::device_rejected_command),
        }
    }
    Err(ps2_setup_status::device_rejected_command)
}

fn initialize_with(
    ports: &mut impl ps2_ports,
    mut key: impl FnMut(u8, bool),
) -> Result<(), ps2_setup_status> {
    // Quiesce first-port transmission before reading the untagged controller
    // configuration response; preserve already-buffered keyboard observations.
    controller_command(ports, ps2_command_disable_first_port)?;
    controller_command(ports, ps2_command_disable_second_port)?;
    drain_with(ports, &mut key)?;
    let original = read_ps2_configuration(ports)?;
    controller_command(ports, command_enable_auxiliary)?;
    auxiliary_command(ports, &mut key, ps2_device_reset)?;
    if read_auxiliary(ports, &mut key)? != ps2_device_self_test_passed
        || read_auxiliary(ports, &mut key)? != 0
    {
        return Err(ps2_setup_status::device_self_test_failed);
    }
    auxiliary_command(ports, &mut key, device_defaults)?;
    // Defaults select the standard three-byte stream; wheel/five-button
    // negotiation is deliberately absent from this profile.
    let configuration =
        (original | ps2_configuration_second_interrupt) & !ps2_configuration_second_disabled;
    write_ps2_configuration(ports, configuration)?;
    if read_ps2_configuration(ports)? != configuration {
        return Err(ps2_setup_status::configuration_mismatch);
    }
    auxiliary_command(ports, &mut key, ps2_device_enable_scanning)?;
    controller_command(ports, ps2_command_enable_first_port)?;
    Ok(())
}

fn stop_with(
    ports: &mut impl ps2_ports,
    mut key: impl FnMut(u8, bool),
) -> Result<(), ps2_setup_status> {
    controller_command(ports, ps2_command_disable_first_port)?;
    controller_command(ports, ps2_command_disable_second_port)?;
    drain_with(ports, &mut key)?;
    let configuration = (read_ps2_configuration(ports)? | ps2_configuration_second_disabled)
        & !ps2_configuration_second_interrupt;
    write_ps2_configuration(ports, configuration)?;
    if read_ps2_configuration(ports)? != configuration {
        return Err(ps2_setup_status::configuration_mismatch);
    }
    controller_command(ports, ps2_command_enable_first_port)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(source: &mut pointer_source, bytes: [u8; 3]) {
        for byte in bytes {
            assert!(source.push(byte, false));
        }
    }
    #[test]
    fn nine_bit_motion_y_inversion_and_buttons_are_preserved() {
        let mut source = pointer_source::new();
        assert_eq!(source.start(), Some(1));
        packet(&mut source, [0x0d, 255, 127]);
        packet(&mut source, [0x3a, 0, 0]);
        let first = source.pop().unwrap().unwrap();
        assert_eq!(
            (
                first.delta_x(),
                first.delta_y(),
                first.buttons(),
                first.sequence()
            ),
            (255, -127, 5, 0)
        );
        let second = source.pop().unwrap().unwrap();
        assert_eq!(
            (
                second.delta_x(),
                second.delta_y(),
                second.buttons(),
                second.sequence()
            ),
            (-256, 256, 2, 1)
        );
    }
    #[test]
    fn overflow_bad_framing_and_controller_errors_close_generation() {
        for (header, error) in [(0, false), (0x48, false), (0x88, false), (8, true)] {
            let mut source = pointer_source::new();
            source.start().unwrap();
            packet(&mut source, [9, 2, 3]);
            assert!(!source.push(header, error));
            assert!(!source.push(8, false));
            assert!(source.pop().unwrap().unwrap().is_pointer());
            let loss = source.pop().unwrap().unwrap();
            assert!(loss.is_pointer_loss());
            assert_eq!(
                (
                    loss.source_generation(),
                    loss.new_generation(),
                    loss.sequence()
                ),
                (1, 2, 1)
            );
            assert!(!source.ready());
            assert_eq!(source.start(), Some(2));
            packet(&mut source, [8, 1, 2]);
            assert_eq!(source.pop().unwrap().unwrap().sequence(), 0);
        }
    }
    #[test]
    fn full_queue_preserves_older_motion_before_one_loss() {
        let mut source = pointer_source::new();
        source.start().unwrap();
        for _ in 0..queue_capacity {
            packet(&mut source, [8, 1, 0]);
        }
        assert!(source.push(8, false));
        assert!(source.push(1, false));
        assert!(!source.push(0, false));
        for sequence in 0..queue_capacity {
            assert_eq!(source.pop().unwrap().unwrap().sequence(), sequence as u64);
        }
        assert!(source.pop().unwrap().unwrap().is_pointer_loss());
        assert!(source.pop().unwrap().is_none());
    }
    #[test]
    fn restart_clears_held_content_but_never_reuses_generation() {
        let mut source = pointer_source::new();
        source.start().unwrap();
        packet(&mut source, [15, 4, 5]);
        source.stop();
        assert!(!source.ready());
        assert_eq!(source.start(), Some(2));
        assert_eq!(source.sequence, 0);
        source.generation = u32::MAX;
        assert!(source.start().is_none());
        source.sequence = u64::MAX;
        assert!(source.push(8, false));
        assert!(source.push(0, false));
        assert!(!source.push(0, false));
        assert!(source.pop().is_err());
    }
    extern crate std;
    use std::collections::VecDeque;
    use std::vec::Vec;

    struct simulated_pointer_ports {
        output: VecDeque<(u8, u8)>,
        configuration: u8,
        configuration_write: bool,
        auxiliary_write: bool,
        device_commands: Vec<u8>,
        reject: bool,
        resend: usize,
        device_id: u8,
        corrupt_configuration: bool,
    }
    impl simulated_pointer_ports {
        fn new() -> Self {
            Self {
                output: VecDeque::new(),
                configuration: 0x61,
                configuration_write: false,
                auxiliary_write: false,
                device_commands: Vec::new(),
                reject: false,
                resend: 0,
                device_id: 0,
                corrupt_configuration: false,
            }
        }
        fn aux(&mut self, byte: u8) {
            self.output.push_back((ps2_status_auxiliary, byte));
        }
    }
    impl ps2_ports for simulated_pointer_ports {
        fn read_status(&mut self) -> u8 {
            self.output
                .front()
                .map_or(0, |(status, _)| ps2_status_output_full | status)
        }
        fn read_data(&mut self) -> u8 {
            self.output.pop_front().unwrap().1
        }
        fn write_command(&mut self, command: u8) {
            match command {
                ps2_command_disable_first_port => {
                    self.configuration |= ps2_configuration_first_disabled
                }
                ps2_command_enable_first_port => {
                    self.configuration &= !ps2_configuration_first_disabled
                }
                ps2_command_disable_second_port => {
                    self.configuration |= ps2_configuration_second_disabled
                }
                command_enable_auxiliary => {
                    self.configuration &= !ps2_configuration_second_disabled
                }
                ps2_command_read_configuration => self.output.push_back((0, self.configuration)),
                ps2_command_write_configuration => self.configuration_write = true,
                command_write_auxiliary => self.auxiliary_write = true,
                _ => panic!("unexpected controller command"),
            }
        }
        fn write_data(&mut self, byte: u8) {
            if self.configuration_write {
                self.configuration = byte ^ if self.corrupt_configuration { 2 } else { 0 };
                self.configuration_write = false;
                return;
            }
            assert!(self.auxiliary_write);
            self.auxiliary_write = false;
            self.device_commands.push(byte);
            if self.reject {
                self.aux(0xfc);
            } else if self.resend != 0 {
                self.resend -= 1;
                self.aux(ps2_device_resend);
            } else {
                self.aux(ps2_device_acknowledge);
                if byte == ps2_device_reset {
                    self.aux(ps2_device_self_test_passed);
                    self.aux(self.device_id);
                }
            }
        }
    }
    #[test]
    fn hardware_setup_preserves_keyboard_and_selects_only_standard_mouse() {
        let mut ports = simulated_pointer_ports::new();
        ports.output.push_back((0, 0x2a));
        ports.output.push_back((ps2_status_auxiliary, 0xff));
        let mut keys = Vec::new();
        assert_eq!(
            initialize_with(&mut ports, |byte, error| keys.push((byte, error))),
            Ok(())
        );
        assert_eq!(keys.as_slice(), &[(0x2a, false)]);
        assert_eq!(ports.configuration, 0x43);
        assert_eq!(ports.device_commands.as_slice(), &[0xff, 0xf6, 0xf4]);
        ports.output.push_back((0, 0xaa));
        ports.output.push_back((ps2_status_auxiliary, 0xf0));
        assert_eq!(
            stop_with(&mut ports, |byte, error| keys.push((byte, error))),
            Ok(())
        );
        assert_eq!(keys.as_slice(), &[(0x2a, false), (0xaa, false)]);
        assert_eq!(ports.configuration, 0x61);
    }
    #[test]
    fn auxiliary_responses_cannot_swallow_keyboard_bytes() {
        let mut ports = simulated_pointer_ports::new();
        ports.output.push_back((0, 0x9e));
        ports.output.push_back((ps2_status_auxiliary, 0xfa));
        let mut keys = Vec::new();
        assert_eq!(
            read_auxiliary(&mut ports, &mut |byte, error| keys.push((byte, error))),
            Ok(0xfa)
        );
        assert_eq!(keys.as_slice(), &[(0x9e, false)]);
    }
    #[test]
    fn hardware_failure_and_resend_are_finite_and_restore_keyboard_configuration() {
        let mut ports = simulated_pointer_ports::new();
        ports.resend = 1;
        assert_eq!(initialize_with(&mut ports, |_, _| {}), Ok(()));
        assert_eq!(ports.device_commands.as_slice(), &[0xff, 0xff, 0xf6, 0xf4]);
        for mode in 0..4 {
            let mut ports = simulated_pointer_ports::new();
            match mode {
                0 => ports.reject = true,
                1 => ports.resend = 10,
                2 => ports.device_id = 3,
                _ => ports.corrupt_configuration = true,
            }
            assert!(initialize_with(&mut ports, |_, _| {}).is_err());
            assert!(ports.device_commands.len() <= 3);
            ports.corrupt_configuration = false;
            assert_eq!(stop_with(&mut ports, |_, _| {}), Ok(()));
            assert_eq!(ports.configuration, 0x61);
        }
    }
    #[test]
    fn empty_irq_streak_is_finite_and_progress_resets_it() {
        let streak = AtomicU8::new(0);
        for _ in 0..ps2_empty_irq_streak_limit - 1 {
            assert!(!record_empty_keyboard_interrupt(&streak));
        }
        assert!(record_empty_keyboard_interrupt(&streak));
        reset_empty_keyboard_streak(&streak);
        assert!(!record_empty_keyboard_interrupt(&streak));
    }
    #[test]
    fn failed_first_port_never_becomes_an_auxiliary_keyboard_source() {
        assert!(first_port_ready(true, true));
        assert!(!first_port_ready(false, true));
        assert!(!first_port_ready(true, false));
        assert!(!first_port_ready(false, false));
    }
}
