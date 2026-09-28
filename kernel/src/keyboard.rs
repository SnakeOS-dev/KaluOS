use core::arch::asm;
use core::sync::atomic::{
    AtomicBool,
    Ordering,
};

use spin::Mutex;

const DATA_PORT: u16 = 0x60;
const STATUS_PORT: u16 = 0x64;

const BUFFER_SIZE: usize = 256;

static SHIFT: AtomicBool =
    AtomicBool::new(false);

static CAPS_LOCK: AtomicBool =
    AtomicBool::new(false);

static EXTENDED: AtomicBool =
    AtomicBool::new(false);

struct KeyboardBuffer {
    data: [u8; BUFFER_SIZE],
    read: usize,
    write: usize,
}

impl KeyboardBuffer {
    const fn new() -> Self {
        Self {
            data: [0; BUFFER_SIZE],
            read: 0,
            write: 0,
        }
    }

    fn push(
        &mut self,
        value: u8,
    ) {
        let next =
            (self.write + 1)
                % BUFFER_SIZE;

        if next == self.read {
            return;
        }

        self.data[self.write] =
            value;

        self.write = next;
    }

    fn pop(
        &mut self,
    ) -> Option<u8> {
        if self.read == self.write {
            return None;
        }

        let value =
            self.data[self.read];

        self.read =
            (self.read + 1)
                % BUFFER_SIZE;

        Some(value)
    }

    fn available(
        &self,
    ) -> usize {
        if self.write >= self.read {
            self.write - self.read
        } else {
            BUFFER_SIZE
                - self.read
                + self.write
        }
    }
}

static KEY_BUFFER:
    Mutex<KeyboardBuffer> =
    Mutex::new(
        KeyboardBuffer::new()
    );

pub fn init() {
    SHIFT.store(
        false,
        Ordering::Relaxed,
    );

    CAPS_LOCK.store(
        false,
        Ordering::Relaxed,
    );

    EXTENDED.store(
        false,
        Ordering::Relaxed,
    );
}

pub fn getc() -> Option<u8> {
    KEY_BUFFER
        .lock()
        .pop()
}

pub fn available() -> usize {
    KEY_BUFFER
        .lock()
        .available()
}

pub fn handle() {
    let code =
        unsafe {
            inb(DATA_PORT)
        };

    if code == 0xE0 {
        EXTENDED.store(
            true,
            Ordering::Relaxed,
        );

        return;
    }

    if EXTENDED.swap(
        false,
        Ordering::Relaxed,
    ) {
        handle_extended(code);
        return;
    }

    match code {
        0x2A | 0x36 => {
            SHIFT.store(
                true,
                Ordering::Relaxed,
            );
        }

        0xAA | 0xB6 => {
            SHIFT.store(
                false,
                Ordering::Relaxed,
            );
        }

        0x3A => {
            let current =
                CAPS_LOCK.load(
                    Ordering::Relaxed,
                );

            CAPS_LOCK.store(
                !current,
                Ordering::Relaxed,
            );
        }

        0x1C => {
            push_byte(b'\n');
        }

        0x0E => {
            push_byte(8);
        }

        0x01 => {
            push_byte(27);
        }

        code if code & 0x80 == 0 => {
            let shift =
                SHIFT.load(
                    Ordering::Relaxed,
                );

            let caps =
                CAPS_LOCK.load(
                    Ordering::Relaxed,
                );

            if let Some(c) =
                translate(
                    code,
                    shift,
                    caps,
                )
            {
                push_byte(c);
            }
        }

        _ => {}
    }
}

fn handle_extended(
    code: u8,
) {
    if code & 0x80 != 0 {
        return;
    }

    match code {
        0x48 => {
            push_escape_sequence(
                b"\x1b[A"
            );
        }

        0x50 => {
            push_escape_sequence(
                b"\x1b[B"
            );
        }

        0x4D => {
            push_escape_sequence(
                b"\x1b[C"
            );
        }

        0x4B => {
            push_escape_sequence(
                b"\x1b[D"
            );
        }

        0x47 => {
            push_escape_sequence(
                b"\x1b[H"
            );
        }

        0x4F => {
            push_escape_sequence(
                b"\x1b[F"
            );
        }

        0x53 => {
            push_escape_sequence(
                b"\x1b[3~"
            );
        }

        0x49 => {
            push_escape_sequence(
                b"\x1b[5~"
            );
        }

        0x51 => {
            push_escape_sequence(
                b"\x1b[6~"
            );
        }

        _ => {}
    }
}

fn push_byte(
    value: u8,
) {
    KEY_BUFFER
        .lock()
        .push(value);
}

fn push_escape_sequence(
    sequence: &[u8],
) {
    let mut buffer =
        KEY_BUFFER.lock();

    for byte in sequence {
        buffer.push(*byte);
    }
}

fn translate(
    code: u8,
    shift: bool,
    caps: bool,
) -> Option<u8> {
    let letter_upper =
        shift ^ caps;

    let value =
        match code {
            0x02 => {
                if shift {
                    b'!'
                } else {
                    b'1'
                }
            }

            0x03 => {
                if shift {
                    b'@'
                } else {
                    b'2'
                }
            }

            0x04 => {
                if shift {
                    b'#'
                } else {
                    b'3'
                }
            }

            0x05 => {
                if shift {
                    b'$'
                } else {
                    b'4'
                }
            }

            0x06 => {
                if shift {
                    b'%'
                } else {
                    b'5'
                }
            }

            0x07 => {
                if shift {
                    b'^'
                } else {
                    b'6'
                }
            }

            0x08 => {
                if shift {
                    b'&'
                } else {
                    b'7'
                }
            }

            0x09 => {
                if shift {
                    b'*'
                } else {
                    b'8'
                }
            }

            0x0A => {
                if shift {
                    b'('
                } else {
                    b'9'
                }
            }

            0x0B => {
                if shift {
                    b')'
                } else {
                    b'0'
                }
            }

            0x0C => {
                if shift {
                    b'_'
                } else {
                    b'-'
                }
            }

            0x0D => {
                if shift {
                    b'+'
                } else {
                    b'='
                }
            }

            0x10 =>
                letter(
                    b'q',
                    letter_upper,
                ),

            0x11 =>
                letter(
                    b'w',
                    letter_upper,
                ),

            0x12 =>
                letter(
                    b'e',
                    letter_upper,
                ),

            0x13 =>
                letter(
                    b'r',
                    letter_upper,
                ),

            0x14 =>
                letter(
                    b't',
                    letter_upper,
                ),

            0x15 =>
                letter(
                    b'y',
                    letter_upper,
                ),

            0x16 =>
                letter(
                    b'u',
                    letter_upper,
                ),

            0x17 =>
                letter(
                    b'i',
                    letter_upper,
                ),

            0x18 =>
                letter(
                    b'o',
                    letter_upper,
                ),

            0x19 =>
                letter(
                    b'p',
                    letter_upper,
                ),

            0x1A => {
                if shift {
                    b'{'
                } else {
                    b'['
                }
            }

            0x1B => {
                if shift {
                    b'}'
                } else {
                    b']'
                }
            }

            0x1E =>
                letter(
                    b'a',
                    letter_upper,
                ),

            0x1F =>
                letter(
                    b's',
                    letter_upper,
                ),

            0x20 =>
                letter(
                    b'd',
                    letter_upper,
                ),

            0x21 =>
                letter(
                    b'f',
                    letter_upper,
                ),

            0x22 =>
                letter(
                    b'g',
                    letter_upper,
                ),

            0x23 =>
                letter(
                    b'h',
                    letter_upper,
                ),

            0x24 =>
                letter(
                    b'j',
                    letter_upper,
                ),

            0x25 =>
                letter(
                    b'k',
                    letter_upper,
                ),

            0x26 =>
                letter(
                    b'l',
                    letter_upper,
                ),

            0x27 => {
                if shift {
                    b':'
                } else {
                    b';'
                }
            }

            0x28 => {
                if shift {
                    b'"'
                } else {
                    b'\''
                }
            }

            0x29 => {
                if shift {
                    b'~'
                } else {
                    b'`'
                }
            }

            0x2B => {
                if shift {
                    b'|'
                } else {
                    b'\\'
                }
            }

            0x2C =>
                letter(
                    b'z',
                    letter_upper,
                ),

            0x2D =>
                letter(
                    b'x',
                    letter_upper,
                ),

            0x2E =>
                letter(
                    b'c',
                    letter_upper,
                ),

            0x2F =>
                letter(
                    b'v',
                    letter_upper,
                ),

            0x30 =>
                letter(
                    b'b',
                    letter_upper,
                ),

            0x31 =>
                letter(
                    b'n',
                    letter_upper,
                ),

            0x32 =>
                letter(
                    b'm',
                    letter_upper,
                ),

            0x33 => {
                if shift {
                    b'<'
                } else {
                    b','
                }
            }

            0x34 => {
                if shift {
                    b'>'
                } else {
                    b'.'
                }
            }

            0x35 => {
                if shift {
                    b'?'
                } else {
                    b'/'
                }
            }

            0x39 => b' ',

            _ => {
                return None;
            }
        };

    Some(value)
}

fn letter(
    value: u8,
    uppercase: bool,
) -> u8 {
    if uppercase {
        value - b'a' + b'A'
    } else {
        value
    }
}

unsafe fn inb(
    port: u16,
) -> u8 {
    let value: u8;

    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(
                nomem,
                nostack,
                preserves_flags
            )
        );
    }

    value
}
