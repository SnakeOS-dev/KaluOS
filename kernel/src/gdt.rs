use core::arch::asm;
use core::mem::size_of;

pub const KERNEL_CODE: u16 = 0x08;
pub const KERNEL_DATA: u16 = 0x10;
pub const USER_DATA: u16 = 0x1B;
pub const USER_CODE: u16 = 0x23;
const TSS_SELECTOR: u16 = 0x28;

#[repr(C, packed)]
struct Tss {
    reserved0: u32,
    rsp: [u64; 3],
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iomap_base: u16,
}

impl Tss {
    const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp: [0; 3],
            reserved1: 0,
            ist: [0; 7],
            reserved2: 0,
            reserved3: 0,
            iomap_base: size_of::<Tss>() as u16,
        }
    }
}

#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

static mut TSS: Tss = Tss::new();

static mut GDT: [u64; 7] = [
    0,
    0x00AF9A000000FFFF,
    0x00CF92000000FFFF,
    0x00CFF2000000FFFF,
    0x00AFFA000000FFFF,
    0,
    0,
];

pub unsafe fn init() {
    let base = core::ptr::addr_of!(TSS) as u64;
    let limit = (size_of::<Tss>() - 1) as u64;

    let low =
        (limit & 0xFFFF)
        | ((base & 0xFFFFFF) << 16)
        | (0x89 << 40)
        | (((limit >> 16) & 0xF) << 48)
        | (((base >> 24) & 0xFF) << 56);

    let high = base >> 32;

    let gdt = core::ptr::addr_of_mut!(GDT) as *mut u64;

    unsafe {
        gdt.add(5).write(low);
        gdt.add(6).write(high);
    }

    let ptr = GdtPointer {
        limit: (size_of::<[u64; 7]>() - 1) as u16,
        base: core::ptr::addr_of!(GDT) as u64,
    };

    unsafe {
        asm!(
            "lgdt [{}]",
            in(reg) &ptr,
            options(readonly, nostack, preserves_flags)
        );

        asm!(
            "push 0x08",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov ss, ax",
            "mov fs, ax",
            "mov gs, ax",
            out("rax") _,
            options(preserves_flags)
        );

        asm!(
            "ltr ax",
            in("ax") TSS_SELECTOR,
            options(nostack, preserves_flags)
        );
    }
}

pub fn set_rsp0(rsp: u64) {
    unsafe {
        let ptr =
            (core::ptr::addr_of_mut!(TSS) as *mut u8)
                .add(4) as *mut u64;

        ptr.write_unaligned(rsp);
    }
}
