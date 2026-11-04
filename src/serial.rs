//! 16550 UART Serial Port Driver for Kernel Console Logging (COM1: 0x3F8)

use core::arch::asm;
use core::fmt;

const COM1: u16 = 0x3F8;

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") val,
        options(nomem, nostack, preserves_flags)
    );
}

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") val,
        options(nomem, nostack, preserves_flags)
    );
    val
}

pub struct SerialPort;

impl SerialPort {
    pub fn init() {
        unsafe {
            outb(COM1 + 1, 0x00); // Disable interrupts
            outb(COM1 + 3, 0x80); // Enable DLAB (set baud rate divisor)
            outb(COM1 + 0, 0x03); // Set divisor to 3 (lo byte) 38400 baud
            outb(COM1 + 1, 0x00); //                  (hi byte)
            outb(COM1 + 3, 0x03); // 8 bits, no parity, one stop bit
            outb(COM1 + 2, 0xC7); // Enable FIFO, clear them, with 14-byte threshold
            outb(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set
        }
    }

    fn is_transmit_empty() -> bool {
        unsafe { inb(COM1 + 5) & 0x20 != 0 }
    }

    pub fn write_byte(b: u8) {
        while !Self::is_transmit_empty() {
            core::hint::spin_loop();
        }
        unsafe {
            outb(COM1, b);
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                SerialPort::write_byte(b'\r');
            }
            SerialPort::write_byte(b);
        }
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    let _ = SerialPort.write_fmt(args);
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => {{
        $crate::serial::_print(format_args!($($arg)*));
        $crate::serial_print!("\n");
    }};
}
