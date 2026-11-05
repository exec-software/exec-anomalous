#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Bounded Linux x86-64 syscall producer for the native application contract.
//! Descriptors 3/4 are preopened nonblocking Anomalous record devices. They do
//! not claim pipe, filesystem, libc, TLS, signal, or Electron compatibility.

use anomalous_abi::application::{
    application_input_action, application_input_v1, application_input_v1_size,
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_editor_contract::{editor_state, frame_byte_count, render, theme};
use core::{
    arch::{asm, global_asm},
    panic::PanicInfo,
    ptr,
};

global_asm!(include_str!("entry.S"), options(att_syntax));

#[unsafe(no_mangle)]
#[unsafe(link_section = ".data.elf_canary")]
static anomalous_linux_client_data_canary: u64 = 0x454c_465f_4441_5441;

const surface_fd: u64 = 3;
const input_fd: u64 = 4;
const wait_limit: usize = 256;
const poll_in: i16 = 1;
const poll_out: i16 = 4;
const bad_descriptor: i64 = -9;
const would_block: i64 = -11;
const bad_address: i64 = -14;
const not_implemented: i64 = -38;
const invalid_argument: i64 = -22;
const message_size: i64 = -90;

static readonly_poll: [u8; 8] = [3, 0, 0, 0, 4, 0, 0xaa, 0x55];

unsafe extern "C" {
    fn anomalous_linux_client_syscall3(number: u64, first: u64, second: u64, third: u64) -> i64;
    fn anomalous_linux_client_fault_instruction() -> !;
    fn anomalous_linux_client_entry_probe() -> u64;
    static mut anomalous_linux_client_poll_tail: u64;
}

struct client_buffers {
    pixels: [u8; frame_byte_count],
    packet: [u8; application_surface_v1_size],
    input: [u8; application_input_v1_size],
}

// The single producer entry takes the sole borrow for its complete lifetime.
// These buffers occupy private BSS and keep surface packets off the user stack.
static mut buffer_storage: client_buffers = client_buffers {
    pixels: [0; frame_byte_count],
    packet: [0; application_surface_v1_size],
    input: [0; application_input_v1_size],
};

fn syscall(number: u64, first: u64, second: u64, third: u64) -> i64 {
    // SAFETY: The assembly stub preserves the SysV callee-saved registers. Any
    // passed pointer is opaque to it and is checked by the syscall personality.
    unsafe { selected_syscall(number, first, second, third) }
}

fn yield_peer() {
    assert_eq!(syscall(24, 0, 0, 0), 0);
}

fn ready(fd: u64, events: i16) -> bool {
    // Linux pollfd is exactly i32 fd, i16 events, i16 revents on x86-64. Encode
    // its little-endian bytes explicitly; no Rust aggregate layout crosses ABI.
    let mut record = [0_u8; 8];
    record[..4].copy_from_slice(&(fd as i32).to_le_bytes());
    record[4..6].copy_from_slice(&events.to_le_bytes());
    let result = syscall(7, record.as_mut_ptr() as u64, 1, 0);
    assert_eq!(&record[..4], &(fd as i32).to_le_bytes());
    assert_eq!(&record[4..6], &events.to_le_bytes());
    let revents = i16::from_le_bytes(record[6..8].try_into().unwrap());
    match result {
        0 => {
            assert_eq!(revents, 0);
            false
        }
        1 => {
            assert_eq!(revents, events);
            true
        }
        _ => panic!("poll rejected"),
    }
}

fn wait_surface_ack() {
    // These declared one-record devices become writable only after the native
    // service has acknowledged the current record. This is transport receipt,
    // not proof of semantic completion or presentation.
    for _ in 0..wait_limit {
        if ready(surface_fd, poll_out) {
            return;
        }
        yield_peer();
    }
    panic!("surface acknowledgement wait exhausted");
}

fn receive_input(bytes: &mut [u8; application_input_v1_size], probe: bool) {
    for _ in 0..wait_limit {
        if !ready(input_fd, poll_in) {
            yield_peer();
            continue;
        }
        if probe {
            input_and_poll_probes(bytes);
        }
        match syscall(0, input_fd, bytes.as_mut_ptr() as u64, bytes.len() as u64) {
            value if value == bytes.len() as i64 => {
                if probe {
                    assert!(!ready(input_fd, poll_in));
                    let received = *bytes;
                    assert_eq!(
                        syscall(0, input_fd, bytes.as_mut_ptr() as u64, bytes.len() as u64),
                        would_block
                    );
                    assert_eq!(bytes, &received);
                }
                return;
            }
            would_block => yield_peer(),
            _ => panic!("input read rejected"),
        }
    }
    panic!("input wait exhausted");
}

fn poll_record(fd: i32, events: i16) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&fd.to_le_bytes());
    bytes[4..6].copy_from_slice(&events.to_le_bytes());
    bytes[6..].copy_from_slice(&0x55aa_i16.to_le_bytes());
    bytes
}

fn input_and_poll_probes(bytes: &mut [u8; application_input_v1_size]) {
    // The first command is queued and the outgoing record already acknowledged.
    // Every rejection below must preserve that exact command for the final read.
    bytes.fill(0x5a);
    assert_eq!(
        syscall(0, input_fd, bytes.as_mut_ptr() as u64, 63),
        message_size
    );
    assert!(bytes.iter().all(|b| *b == 0x5a));
    assert_eq!(syscall(0, input_fd, 0, 64), bad_address);
    assert!(ready(input_fd, poll_in));
    assert_eq!(syscall(0, input_fd, u64::MAX, 0), 0);
    assert_eq!(syscall(1, surface_fd, u64::MAX, 0), 0);
    assert_eq!(
        syscall(0, input_fd, bytes.as_mut_ptr() as u64, 4097),
        message_size
    );
    assert_eq!(
        syscall(1, surface_fd, bytes.as_ptr() as u64, 4097),
        message_size
    );
    assert!(bytes.iter().all(|b| *b == 0x5a));
    assert_eq!(syscall(7, 0, 0, 0), 0);

    let mut ignored = poll_record(-1, poll_in | poll_out);
    assert_eq!(syscall(7, ignored.as_mut_ptr() as u64, 1, 0), 0);
    assert_eq!(&ignored[6..], &[0, 0]);
    let mut unknown = poll_record(99, 0);
    assert_eq!(syscall(7, unknown.as_mut_ptr() as u64, 1, 0), 1);
    assert_eq!(&unknown[6..], &32_i16.to_le_bytes());
    let mut duplicate = [poll_record(3, poll_out); 2];
    assert_eq!(syscall(7, duplicate.as_mut_ptr() as u64, 2, 0), 2);
    for record in duplicate {
        assert_eq!(&record[6..], &poll_out.to_le_bytes());
    }
    let mut unsupported = poll_record(3, 0x4000);
    assert_eq!(syscall(7, unsupported.as_mut_ptr() as u64, 1, 0), 0);
    assert_eq!(&unsupported[6..], &[0, 0]);
    let mut invalid = poll_record(3, poll_out);
    let original = invalid;
    assert_eq!(
        syscall(7, invalid.as_mut_ptr() as u64, 9, 0),
        invalid_argument
    );
    assert_eq!(invalid, original);
    assert_eq!(
        syscall(7, invalid.as_mut_ptr() as u64, 1, 1),
        invalid_argument
    );
    assert_eq!(invalid, original);

    // This exact8-byte NOBITS object ends at the data mapping boundary; the
    // requested second record lies in the absent guard. No partial copyback.
    let tail = ptr::addr_of_mut!(anomalous_linux_client_poll_tail);
    assert_eq!(tail as u64, 0x801ff8);
    let sentinel = u64::from_le_bytes(poll_record(3, poll_out));
    // SAFETY: The linker reserves this aligned8-byte object outside Rust BSS
    // objects, inside the retained private RW mapping, for the sole producer.
    unsafe { ptr::write_volatile(tail, sentinel) };
    assert_eq!(syscall(7, tail as u64, 2, 0), bad_address);
    assert_eq!(unsafe { ptr::read_volatile(tail) }, sentinel);
    unsafe { ptr::write_volatile(tail, 0) };

    // A readable valid poll request in RX cannot become a writable result array.
    assert_eq!(syscall(7, readonly_poll.as_ptr() as u64, 1, 0), bad_address);
    // SAFETY: Volatile observation covers the actual immutable RX bytes.
    assert_eq!(
        unsafe { ptr::read_volatile(&readonly_poll) },
        [3, 0, 0, 0, 4, 0, 0xaa, 0x55]
    );
    assert!(ready(input_fd, poll_in));
    assert!(bytes.iter().all(|b| *b == 0x5a));
}

fn wire_theme(value: theme) -> application_theme {
    match value {
        theme::light => application_theme::light,
        theme::dark => application_theme::dark,
    }
}

fn apply(state: &mut editor_state, action: application_input_action<'_>) {
    match action {
        application_input_action::replace_selection(text) => state.insert(text).unwrap(),
        application_input_action::set_selection { anchor, head } => {
            state
                .set_selection(usize::from(anchor), usize::from(head))
                .unwrap();
        }
        application_input_action::set_theme(value) => state.set_theme(match value {
            application_theme::light => theme::light,
            application_theme::dark => theme::dark,
        }),
        application_input_action::undo => state.undo().unwrap(),
    }
}

fn encode_surface(state: &editor_state, sequence: u64, buffers: &mut client_buffers) {
    let metadata = render(state, &mut buffers.pixels);
    application_surface_v1::try_new(
        application_surface_state {
            request_sequence: sequence,
            document_revision: metadata.revision,
            text: state.text(),
            selection_anchor: metadata.anchor as u8,
            selection_head: metadata.head as u8,
            dirty: metadata.dirty,
            theme: wire_theme(metadata.appearance),
            undo_depth: metadata.undo_depth,
        },
        &buffers.pixels,
    )
    .unwrap()
    .encode_into(&mut buffers.packet)
    .unwrap();
}

fn source(fault: bool, buffers: &mut client_buffers) -> ! {
    #[cfg(feature = "tls")]
    tls::probe();
    assert_eq!(syscall(u64::MAX, 0, 0, 0), not_implemented);
    // SAFETY: The leaf fixture preserves SysV state, asserts its own red zone
    // and GPR/RCX/R11 results, and performs a second unsupported raw syscall.
    assert_eq!(unsafe { selected_entry_probe() }, 1);
    assert_eq!(
        syscall(
            1_u64 << 32,
            surface_fd,
            0,
            application_surface_v1_size as u64
        ),
        bad_descriptor
    );
    assert_eq!(
        syscall(1, input_fd, 0, application_input_v1_size as u64),
        bad_descriptor
    );
    assert_eq!(
        syscall(1, surface_fd, 0, application_surface_v1_size as u64),
        bad_address
    );
    let mut state = editor_state::new();
    for sequence in 1..=7 {
        encode_surface(&state, sequence, buffers);
        assert_eq!(
            syscall(
                1,
                surface_fd,
                buffers.packet.as_ptr() as u64,
                buffers.packet.len() as u64
            ),
            buffers.packet.len() as i64
        );
        for byte in &mut buffers.packet {
            // SAFETY: The sole live mutable buffer owns these mapped bytes.
            // Volatile corruption proves the queued message is a kernel copy.
            unsafe { ptr::write_volatile(byte, 0xa5) };
        }
        if fault {
            // SAFETY: This no-return symbol is the separately admitted UD2. It
            // uses no arming syscall and is reached only after packet one queued.
            unsafe { selected_fault() }
        }
        wait_surface_ack();
        if sequence == 7 {
            exit();
        }
        receive_input(&mut buffers.input, sequence == 1);
        let input = application_input_v1::decode(&buffers.input).unwrap();
        assert_eq!(input.sequence(), sequence);
        apply(&mut state, input.action());
        // read acknowledged atomically after its complete checked copy. There
        // is no private acknowledgement syscall in this Linux entry profile.
    }
    panic!("source fell through");
}

fn exit() -> ! {
    syscall(60, 0, 0, 0);
    panic!("terminal exit returned");
}

unsafe fn stack_word(initial: *const u64, index: usize) -> u64 {
    assert!(index < 69);
    // SAFETY: The entry ABI supplies a readable initial stack record; callers
    // inspect only its bounded argc/argv/envp/auxv fields, stopping at AT_NULL.
    unsafe { ptr::read(initial.add(index)) }
}

unsafe fn argument_is(address: u64, expected: &[u8]) -> bool {
    assert!(address != 0 && address.checked_add(expected.len() as u64).is_some());
    // SAFETY: The process entry ABI supplies NUL-terminated argument storage.
    // This fixture reads at most the expected short string and its terminator.
    for (index, expected) in expected.iter().enumerate() {
        if unsafe { ptr::read((address as *const u8).add(index)) } != *expected {
            return false;
        }
    }
    unsafe { ptr::read((address as *const u8).add(expected.len())) == 0 }
}

unsafe fn startup_scenario(initial: *const u64) -> bool {
    assert!(!initial.is_null() && (initial as usize).is_multiple_of(16));
    // SAFETY: The assembly entry passes the unmodified process-entry RSP. The
    // kernel supplies two argv pointers, their NULL, empty envp and bounded auxv.
    unsafe {
        assert_eq!(stack_word(initial, 0), 2);
        let name = stack_word(initial, 1);
        let scenario = stack_word(initial, 2);
        assert!(argument_is(name, b"anomalous-editor"));
        assert_eq!(stack_word(initial, 3), 0);
        assert_eq!(stack_word(initial, 4), 0);
        let mut pages = false;
        let mut entry = false;
        let mut terminated = false;
        for index in 0..32 {
            let kind = stack_word(initial, 5 + 2 * index);
            let value = stack_word(initial, 6 + 2 * index);
            match kind {
                0 => {
                    assert_eq!(value, 0);
                    terminated = true;
                    break;
                }
                6 => {
                    assert!(!pages && value == 4096);
                    pages = true;
                }
                9 => {
                    assert!(!entry && value == 0x400000);
                    entry = true;
                }
                _ => {}
            }
        }
        assert!(pages && entry && terminated);
        if argument_is(scenario, b"normal") {
            false
        } else {
            assert!(argument_is(scenario, b"fault"));
            true
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn anomalous_linux_client_run(initial: *const u64) -> ! {
    // SAFETY: The ELF entry establishes the documented process stack; parsing
    // here precedes all syscalls and never consults native fixed-address args.
    let fault = unsafe { startup_scenario(initial) };
    // SAFETY: Initialized private RW bytes are retained for the task lifetime.
    assert_eq!(
        unsafe { ptr::read_volatile(&anomalous_linux_client_data_canary) },
        0x454c_465f_4441_5441
    );
    // SAFETY: This sole producer entry borrows its private BSS aggregate once.
    let buffers = unsafe { &mut *ptr::addr_of_mut!(buffer_storage) };
    source(fault, buffers)
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // Only the distinct fixture marker can be the expected queued-task fault.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}

#[cfg(feature = "tls")]
mod tls;
#[cfg(feature = "tls")]
use tls::{
    entry_probe as selected_entry_probe, fault as selected_fault, syscall as selected_syscall,
};
#[cfg(not(feature = "tls"))]
use {
    anomalous_linux_client_entry_probe as selected_entry_probe,
    anomalous_linux_client_fault_instruction as selected_fault,
    anomalous_linux_client_syscall3 as selected_syscall,
};
