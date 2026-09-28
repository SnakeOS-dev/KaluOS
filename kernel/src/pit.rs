use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

const PIT_FREQUENCY: u32 = 1_193_182;
const FREQUENCY: u32 = 100;

static TICKS: AtomicU64 = AtomicU64::new(0);

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

pub unsafe fn init() {
    let divisor = PIT_FREQUENCY / FREQUENCY;

    unsafe {
        outb(0x43, 0x36);
        outb(0x40, (divisor & 0xFF) as u8);
        outb(0x40, ((divisor >> 8) & 0xFF) as u8);
    }
}

pub fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
