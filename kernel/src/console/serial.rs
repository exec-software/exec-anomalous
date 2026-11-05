use core::arch::asm;
use core::fmt;

/// Minimal polling driver for the PC-compatible 16550 UART.
pub struct serial_port {
    base: u16,
}

impl serial_port {
    const com_1: u16 = 0x3F8;

    #[must_use]
    pub const fn com1() -> Self {
        Self { base: Self::com_1 }
    }

    pub fn initialize(&mut self) {
        // The sequence disables UART interrupts, selects a 38,400 baud divisor,
        // configures 8N1, enables FIFO, and asserts RTS/DTR.
        self.write_register(1, 0x00);
        self.write_register(3, 0x80);
        self.write_register(0, 0x03);
        self.write_register(1, 0x00);
        self.write_register(3, 0x03);
        self.write_register(2, 0xC7);
        self.write_register(4, 0x0B);
    }

    fn write_byte(&mut self, byte: u8) {
        // Bound the poll so missing emulated hardware cannot deadlock boot.
        for _ in 0..100_000 {
            if self.read_register(5) & 0x20 != 0 {
                break;
            }
            core::hint::spin_loop();
        }
        self.write_register(0, byte);
    }

    fn write_register(&self, offset: u16, value: u8) {
        let port = self.base + offset;
        // SAFETY: The kernel owns the configured legacy COM1 I/O range. `out`
        // touches only the selected port and does not access memory or the stack.
        unsafe {
            asm!(
                "out dx, al",
                in("dx") port,
                in("al") value,
                options(nomem, nostack, preserves_flags)
            );
        }
    }

    fn read_register(&self, offset: u16) -> u8 {
        let port = self.base + offset;
        let value: u8;
        // SAFETY: The kernel owns the configured legacy COM1 I/O range. `in`
        // reads only the selected port and returns the byte in AL.
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
}

impl fmt::Write for serial_port {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        for byte in value.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}
