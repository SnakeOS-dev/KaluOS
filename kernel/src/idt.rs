use core::arch::{asm, global_asm};

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    options: u16,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const MISSING: Self = Self {
        offset_low: 0,
        selector: 0,
        options: 0,
        offset_mid: 0,
        offset_high: 0,
        reserved: 0,
    };

    fn set(&mut self, handler: u64, options: u16) {
        self.offset_low = handler as u16;
        self.selector = 0x08;
        self.options = options;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.reserved = 0;
    }
}

#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; 256] =
    [IdtEntry::MISSING; 256];

global_asm!(
    r#"
.macro PUSH_ALL
    push rax
    push rbx
    push rcx
    push rdx
    push rbp
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
.endm

.macro POP_ALL
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rbp
    pop rdx
    pop rcx
    pop rbx
    pop rax
.endm

.global timer_stub
timer_stub:
    PUSH_ALL
    mov rdi, rsp
    sub rsp, 32
    and rsp, -16
    call timer_interrupt_entry
    mov rsp, rax
    POP_ALL
    iretq

.global keyboard_stub
keyboard_stub:
    PUSH_ALL
    mov rdi, rsp
    sub rsp, 32
    and rsp, -16
    call keyboard_interrupt_entry
    mov rsp, rax
    POP_ALL
    iretq

.global syscall_stub
syscall_stub:
    PUSH_ALL
    mov rdi, rsp
    sub rsp, 32
    and rsp, -16
    call syscall_interrupt_entry
    mov rsp, rax
    POP_ALL
    iretq

.global kernel_yield_stub
kernel_yield_stub:
    PUSH_ALL
    mov rdi, rsp
    sub rsp, 32
    and rsp, -16
    call kernel_yield_interrupt_entry
    mov rsp, rax
    POP_ALL
    iretq
"#);
global_asm!(
    r#"
.global exception_gp_stub
exception_gp_stub:
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rdi
    push rsi
    push rbp
    push rdx
    push rcx
    push rbx
    push rax

    mov rdi, 13
    mov rsi, [rsp + 120]
    mov rdx, [rsp + 128]

    call exception_handler

1:
    cli
    hlt
    jmp 1b

.global exception_pf_stub
exception_pf_stub:
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rdi
    push rsi
    push rbp
    push rdx
    push rcx
    push rbx
    push rax

    mov rdi, 14
    mov rsi, [rsp + 120]
    mov rdx, [rsp + 128]

    call exception_handler

1:
    cli
    hlt
    jmp 1b

.global exception_df_stub
exception_df_stub:
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rdi
    push rsi
    push rbp
    push rdx
    push rcx
    push rbx
    push rax

    mov rdi, 8
    mov rsi, [rsp + 120]
    mov rdx, [rsp + 128]

    call exception_handler

1:
    cli
    hlt
    jmp 1b
"#
);

unsafe extern "C" {
    fn timer_stub();
    fn keyboard_stub();
    fn syscall_stub();
    fn kernel_yield_stub();

    fn exception_gp_stub();
    fn exception_pf_stub();
    fn exception_df_stub();
}
#[unsafe(no_mangle)]
extern "C" fn exception_handler(
    vector: u64,
    error: u64,
    rip: u64,
) -> ! {
    let cr2: u64;

    unsafe {
        asm!(
            "mov {}, cr2",
            out(reg) cr2,
            options(nomem, nostack, preserves_flags)
        );
    }

    crate::serial::write(format_args!(
        "\nEXCEPTION {}\nerror={:#x}\nrip={:#x}\ncr2={:#x}\n",
        vector,
        error,
        rip,
        cr2
    ));

    loop {
        unsafe {
            asm!("cli; hlt");
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn timer_interrupt_entry(
    rsp: u64,
) -> u64 {
    crate::pit::tick();
    crate::pic::eoi(0);

    crate::scheduler::on_timer(rsp)
}

#[unsafe(no_mangle)]
extern "C" fn keyboard_interrupt_entry(
    rsp: u64,
) -> u64 {
    let scancode =
        unsafe { crate::keyboard::read_scancode() };

    crate::keyboard::handle(scancode);
    crate::pic::eoi(1);

    rsp
}

#[unsafe(no_mangle)]
extern "C" fn kernel_yield_interrupt_entry(
    rsp: u64,
) -> u64 {
    crate::scheduler::yield_from_interrupt(rsp)
}

pub unsafe fn init() {
    let idt =
        core::ptr::addr_of_mut!(IDT) as *mut IdtEntry;

     unsafe {
    (*idt.add(8)).set(
        exception_df_stub as *const () as u64,
        0x8E00,
    );

    (*idt.add(13)).set(
        exception_gp_stub as *const () as u64,
        0x8E00,
    );

    (*idt.add(14)).set(
        exception_pf_stub as *const () as u64,
        0x8E00,
    );

    (*idt.add(32)).set(
        timer_stub as *const () as u64,
        0x8E00,
    );

    (*idt.add(33)).set(
        keyboard_stub as *const () as u64,
        0x8E00,
    );

    (*idt.add(0x80)).set(
        syscall_stub as *const () as u64,
        0xEE00,
    );

    (*idt.add(0x81)).set(
        kernel_yield_stub as *const () as u64,
        0x8E00,
    );
    }
     let ptr = IdtPointer {
        limit:
            (core::mem::size_of::<[IdtEntry; 256]>() - 1)
                as u16,

        base: core::ptr::addr_of!(IDT) as u64,
    };
    
    unsafe {
        asm!(
            "lidt [{}]",
            in(reg) &ptr,
            options(readonly, nostack, preserves_flags)
        );
    }
}
