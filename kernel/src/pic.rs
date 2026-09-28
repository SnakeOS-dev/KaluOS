use core::arch::asm;

const PIC1: u16 = 0x20;
const PIC2: u16 = 0xA0;

const PIC1_COMMAND: u16 = PIC1;
const PIC1_DATA: u16 = PIC1 + 1;
const PIC2_COMMAND: u16 = PIC2;
const PIC2_DATA: u16 = PIC2 + 1;

const ICW1_INIT: u8 = 0x10;
const ICW1_ICW4: u8 = 0x01;
const ICW4_8086: u8 = 0x01;

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
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
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }

    value
}

fn io_wait() {
    unsafe {
        outb(0x80, 0);
    }
}

pub unsafe fn init() {
    unsafe {
        let mask1 = inb(0x21);
        let mask2 = inb(0xA1);

        outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();

        outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();

        outb(PIC1_DATA, 32);
        io_wait();

        outb(PIC2_DATA, 40);
        io_wait();

        outb(PIC1_DATA, 4);
        io_wait();

        outb(PIC2_DATA, 2);
        io_wait();

        outb(PIC1_DATA, ICW4_8086);
        io_wait();

        outb(PIC2_DATA, ICW4_8086);
        io_wait();

        outb(PIC1_DATA, mask1 & !3);
        outb(PIC2_DATA, mask2);
    }
}

pub fn eoi(irq: u8) {
    unsafe {
        if irq >= 8 {
            outb(PIC2_COMMAND, 0x20);
        }

        outb(PIC1_COMMAND, 0x20);
    }
}
