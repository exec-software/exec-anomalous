//! Feature-private raw Linux TLS fixture. Linux policy remains in the veil.
use core::{arch::global_asm, ptr};

global_asm!(include_str!("tls.S"), options(att_syntax));

unsafe extern "C" {
    fn anomalous_linux_client_tls_syscall(number: u64, first: u64, second: u64, third: u64) -> i64;
    fn anomalous_linux_client_tls_spin();
    fn anomalous_linux_client_tls_probe() -> u64;
    fn anomalous_linux_client_tls_canary(value: u64);
    fn anomalous_linux_client_tls_fault() -> !;
}

pub(super) unsafe fn syscall(number: u64, first: u64, second: u64, third: u64) -> i64 {
    // SAFETY: Retained SysV wrapper, with explicit FP witnesses around the raw call.
    let _retained_raw_symbol = super::anomalous_linux_client_syscall3;
    unsafe { anomalous_linux_client_tls_syscall(number, first, second, third) }
}
pub(super) unsafe fn fault() -> ! {
    // SAFETY: Loads the same FP witness then reaches the admitted UD2 marker.
    let _retained_fault_symbol = super::anomalous_linux_client_fault_instruction;
    unsafe { anomalous_linux_client_tls_fault() }
}

pub(super) unsafe fn entry_probe() -> u64 {
    let _retained_probe_symbol = super::anomalous_linux_client_entry_probe;
    // SAFETY: The wrapper establishes the FP witness around the unchanged
    // red-zone/GPR probe, then verifies its preserved state before returning.
    unsafe { anomalous_linux_client_tls_probe() }
}

pub(super) fn probe() {
    const set: u64 = 0x1002;
    const get: u64 = 0x1003;
    const a: u64 = 0x800020;
    const b: u64 = 0x800040;
    const high: u64 = 0x7fff12345000;
    let call = |operation, value| super::syscall(158, operation, value, 0);
    let read = |expected| {
        let mut observed = u64::MAX;
        assert_eq!(call(get, ptr::addr_of_mut!(observed) as u64), 0);
        assert_eq!(observed, expected);
    };
    read(0);
    assert_eq!(call(set, 0x0000800000000000), -1);
    assert_eq!(call(set, 0xffff800000001000), -1);
    assert_eq!(call(set, a), 0);
    // SAFETY: The fixture's private mapped reserved words are initially zero;
    // the assembly uses the real selected FS base, not a flat-pointer shortcut.
    unsafe { anomalous_linux_client_tls_canary(0x745153aaf00d0001) };
    read(a);
    assert_eq!(call(set, b), 0);
    unsafe { anomalous_linux_client_tls_canary(0x745153bbface0001) };
    read(b);
    assert_eq!(call(set, 0), 0);
    read(0);
    assert_eq!(call(set, high), 0);
    read(high);
    assert_eq!(call(set, a), 0);
    read(a);
    // The timer sets this retained private word only after observing the actual
    // FS canary/progress and the independently saved FP pattern together.
    unsafe { anomalous_linux_client_tls_spin() };
    let tail = 0x801ff8 as *mut u64;
    let sentinel = 0xa55a112233445566;
    unsafe { ptr::write_volatile(tail, sentinel) };
    assert_eq!(call(get, tail as u64 + 4), -14);
    assert_eq!(unsafe { ptr::read_volatile(tail) }, sentinel);
    unsafe { ptr::write_volatile(tail, 0) };
    assert_eq!(call(get, super::readonly_poll.as_ptr() as u64), -14);
    assert_eq!(
        unsafe { ptr::read_volatile(&super::readonly_poll) },
        [3, 0, 0, 0, 4, 0, 0xaa, 0x55]
    );
    assert_eq!(call(get, 0), -14);
    assert_eq!(call(0x1001, 0), -22);
    assert_eq!(call(0x1004, 0), -22);
}
