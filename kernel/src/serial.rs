use core::fmt;
use core::fmt::Write;
use spin::Mutex;

const COM1: u16 = 0x3F8;

pub struct Serial;

impl Serial {
    pub const fn new() -> Self {
        Self
    }

    pub unsafe fn init(&mut self) {
        unsafe {
            outb(COM1 + 1, 0x00);
            outb(COM1 + 3, 0x80);
            outb(COM1 + 0, 0x03);
            outb(COM1 + 1, 0x00);
            outb(COM1 + 3, 0x03);
            outb(COM1 + 2, 0xC7);
            outb(COM1 + 4, 0x0B);
        }
    }

    unsafe fn write_byte(&mut self, byte: u8) {
        unsafe {
            while inb(COM1 + 5) & 0x20 == 0 {}
            outb(COM1, byte);
        }
    }
}

impl Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            unsafe {
                self.write_byte(byte);
            }
        }

        Ok(())
    }
}

pub static SERIAL: Mutex<Serial> = Mutex::new(Serial::new());

pub unsafe fn init() {
    unsafe {
        SERIAL.lock().init();
    }
}

pub fn write(args: fmt::Arguments) {
    SERIAL.lock().write_fmt(args).ok();
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;

    unsafe {
        core::arch::asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }

    value
}
