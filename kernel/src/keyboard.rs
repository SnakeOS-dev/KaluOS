use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

static SHIFT: AtomicBool = AtomicBool::new(false);
pub fn init() {}

pub fn handle(scancode: u8) {
    let byte = match scancode {
        0x2A | 0x36 => { SHIFT.store(true, Ordering::Relaxed); return; }
        0xAA | 0xB6 => { SHIFT.store(false, Ordering::Relaxed); return; }
        0x1C => b'\n',
        code if code < 0x80 => {
            match translate(code, SHIFT.load(Ordering::Relaxed)) {
                Some(c) => c as u8,
                None => return,
            }
        }
        _ => return,
    };

    KEY_BUFFER.lock().push(byte);
}
fn translate(code: u8, shift: bool) -> Option<char> {
    let normal = b"1234567890-=qwertyuiop[]asdfghjkl;'`\\zxcvbnm,./ ";
    let shifted = b"!@#$%^&*()_+QWERTYUIOP{}ASDFGHJKL:\"~|ZXCVBNM<>? ";

    let index = match code {
        0x02..=0x0D => code - 0x02,
        0x10..=0x1B => code - 0x10 + 12,
        0x1E..=0x28 => code - 0x1E + 24,
        0x29 => 33,
        0x2B => 34,
        0x2C..=0x35 => code - 0x2C + 35,
        0x39 => 45,
        _ => return None,
    };

    let table = if shift { shifted } else { normal };

    table.get(index as usize).copied().map(|c| c as char)
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;

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

pub unsafe fn read_scancode() -> u8 {
    unsafe { inb(0x60) }
}

const KEY_BUFFER_SIZE: usize = 256;

struct KeyBuffer {
    data: [u8; KEY_BUFFER_SIZE],
    read: usize,
    write: usize,
}

impl KeyBuffer {
    const fn new() -> Self {
        Self {
            data: [0; KEY_BUFFER_SIZE],
            read: 0,
            write: 0,
        }
    }

    fn push(&mut self, byte: u8) {
        let next =
            (self.write + 1)
                % KEY_BUFFER_SIZE;

        if next == self.read {
            return;
        }

        self.data[self.write] =
            byte;

        self.write = next;
    }

    fn pop(&mut self) -> Option<u8> {
        if self.read == self.write {
            return None;
        }

        let byte =
            self.data[self.read];

        self.read =
            (self.read + 1)
                % KEY_BUFFER_SIZE;

        Some(byte)
    }
}

static KEY_BUFFER: Mutex<KeyBuffer> =
    Mutex::new(KeyBuffer::new());

pub fn getc() -> Option<u8> {
    KEY_BUFFER.lock().pop()
}
pub fn push(byte: u8) {
    KEY_BUFFER.lock().push(byte);
}

