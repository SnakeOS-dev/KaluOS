use core::arch::{asm,global_asm};

use spin::Mutex;

use crate::{gdt, pmm, vmm};

const MAX_TASKS: usize = 64;
const KERNEL_STACK_PAGES: usize = 8;
const QUANTUM: u8 = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Empty,
    Ready,
    Running,
    Zombie,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Kernel,
    User,
}

#[derive(Clone, Copy)]
pub struct Task {
    pub id: u64,
    pub state: TaskState,
    pub kind: TaskKind,
    pub priority: u8,
    pub rsp: u64,
    pub cr3: u64,
    pub kernel_stack_top: u64,
    pub stack_phys: u64,
    pub stack_pages: usize,
    pub exit_code: i32,
    quantum: u8,
}
global_asm!(
    r#"
.global kernel_task_bootstrap
kernel_task_bootstrap:
    mov rsp, rsi
    and rsp, -16
    sub rsp, 8
    sti
    jmp rdi
"#
);

unsafe extern "C" {
    fn kernel_task_bootstrap();
}
impl Task {
    const fn empty() -> Self {
        Self {
            id: 0,
            state: TaskState::Empty,
            kind: TaskKind::Kernel,
            priority: 1,
            rsp: 0,
            cr3: 0,
            kernel_stack_top: 0,
            stack_phys: 0,
            stack_pages: 0,
            exit_code: 0,
            quantum: QUANTUM,
        }
    }
}

pub struct Scheduler {
    tasks: [Task; MAX_TASKS],
    current: usize,
    next_id: u64,
    initialized: bool,
}

impl Scheduler {
    const fn new() -> Self {
        Self {
            tasks: [Task::empty(); MAX_TASKS],
            current: 0,
            next_id: 1,
            initialized: false,
        }
    }

    fn find_slot(&self) -> Option<usize> {
        for i in 1..MAX_TASKS {
            if self.tasks[i].state == TaskState::Empty {
                return Some(i);
            }
        }

        None
    }

    fn highest_ready_priority(&self) -> u8 {
        let mut priority = 0;

        for task in &self.tasks {
            if task.state == TaskState::Ready && task.priority > priority {
                priority = task.priority;
            }
        }

        priority
    }

    fn select_next(&self) -> Option<usize> {
        for priority in (1..=10).rev() {
            for offset in 1..=MAX_TASKS {
                let index = (self.current + offset) % MAX_TASKS;
                let task = self.tasks[index];

                if task.state == TaskState::Ready
                    && task.priority == priority
                {
                    return Some(index);
                }
            }
        }

        None
    }

    fn reap(&mut self) {
        for i in 0..MAX_TASKS {
            if i == self.current {
                continue;
            }

            if self.tasks[i].state != TaskState::Zombie {
                continue;
            }

            let phys = self.tasks[i].stack_phys;
            let pages = self.tasks[i].stack_pages;

            if phys != 0 && pages != 0 {
                pmm::free_many(phys, pages);
            }

            self.tasks[i] = Task::empty();
        }
    }

    unsafe fn spawn_kernel_inner(
	    &mut self,
	    entry: extern "C" fn() -> !,
	    priority: u8,
	) -> Option<u64> {
	    let slot = self.find_slot()?;
	
	    let stack_phys =
		    pmm::alloc_many(KERNEL_STACK_PAGES)?;

		let stack_base =
		    vmm::phys_to_virt_addr(stack_phys);

		let stack_top =
		    (stack_base
		        + KERNEL_STACK_PAGES as u64 * pmm::PAGE_SIZE)
		        & !0xF;
		
		let rsp = unsafe {
		    build_kernel_frame(
		        stack_top,
		        entry,
		    )
		};
	    if rsp == 0 {
        	return None;
	    }

	    let id = self.next_id;
	    self.next_id += 1;

	    self.tasks[slot] = Task {
        	id,
	        state: TaskState::Ready,
	        kind: TaskKind::Kernel,
        	priority: priority.clamp(1, 10),
	        rsp,
        	cr3: vmm::current_pml4(),
	        kernel_stack_top: stack_top,
        	stack_phys,
	        stack_pages: KERNEL_STACK_PAGES,
        	exit_code: 0,
	        quantum: QUANTUM,
	    };

	    crate::serial::write(format_args!(
        	"task: spawned id={} entry={:#x} stack={:#x} rsp={:#x}\n",
	        id,
        	entry as usize,
	        stack_top,
        	rsp
	    ));

        	Some(id)
    }
    unsafe fn spawn_user_inner(
        &mut self,
        entry: u64,
        user_stack: u64,
        cr3: u64,
        priority: u8,
        arg0: u64,
    ) -> Option<u64> {
        let slot = self.find_slot()?;

        let stack_phys =
            pmm::alloc_many(KERNEL_STACK_PAGES)?;

        let stack_base =
            vmm::phys_to_virt_addr(stack_phys);

        let stack_top =
            stack_base
                + (KERNEL_STACK_PAGES as u64 * pmm::PAGE_SIZE);

        let rsp = unsafe {
            build_user_frame(
                stack_top,
                entry,
                user_stack,
                arg0,
            )
        };

        let id = self.next_id;
        self.next_id += 1;

        self.tasks[slot] = Task {
            id,
            state: TaskState::Ready,
            kind: TaskKind::User,
            priority: priority.clamp(1, 10),
            rsp,
            cr3,
            kernel_stack_top: stack_top & !0xF,
            stack_phys,
            stack_pages: KERNEL_STACK_PAGES,
            exit_code: 0,
            quantum: QUANTUM,
        };

        Some(id)
    }
}

static SCHEDULER: Mutex<Scheduler> =
    Mutex::new(Scheduler::new());

extern "C" fn idle() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

unsafe fn push(stack: &mut u64, value: u64) {
    *stack -= 8;

    unsafe {
        (*stack as *mut u64).write(value);
    }
}
#[repr(C)]
struct KernelFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
}

unsafe fn build_kernel_frame(
    stack_top: u64,
    entry: extern "C" fn() -> !,
) -> u64 {
    let frame_top = stack_top & !0xF;

    let rsp =
        frame_top - core::mem::size_of::<KernelFrame>() as u64;

    let frame = rsp as *mut KernelFrame;

    unsafe {
        frame.write(KernelFrame {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,

            rdi: entry as usize as u64,
            rsi: stack_top,

            rbp: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,

            rip: kernel_task_bootstrap as usize as u64,
            cs: gdt::KERNEL_CODE as u64,
            rflags: 0x2,
        });
    }

    rsp
}

unsafe fn build_user_frame(
    stack_top: u64,
    entry: u64,
    user_stack: u64,
    arg0: u64,
) -> u64 {
    let mut sp = stack_top & !0xF;

    unsafe {
        push(&mut sp, gdt::USER_DATA as u64);
        push(&mut sp, user_stack);
        push(&mut sp, 0x202);
        push(&mut sp, gdt::USER_CODE as u64);
        push(&mut sp, entry);

        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, arg0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
        push(&mut sp, 0);
    }

    sp
}

fn irq_save() -> u64 {
    let flags: u64;

    unsafe {
        asm!(
            "pushfq",
            "pop {}",
            "cli",
            out(reg) flags,
        );
    }

    flags
}

fn irq_restore(flags: u64) {
    if flags & (1 << 9) != 0 {
        unsafe {
            asm!("sti");
        }
    }
}

fn activate(task: Task) {
    let current = vmm::current_pml4();

    if current != task.cr3 {
        unsafe {
            asm!(
                "mov cr3, {}",
                in(reg) task.cr3,
                options(nostack, preserves_flags)
            );
        }
    }

    if task.kernel_stack_top != 0 {
        gdt::set_rsp0(task.kernel_stack_top);
    }
}
pub fn init(boot_priority: u8) {
    let flags = irq_save();

    let mut scheduler = SCHEDULER.lock();

    scheduler.tasks[0] = Task {
        id: 0,
        state: TaskState::Running,
        kind: TaskKind::Kernel,
        priority: boot_priority.clamp(1, 10),
        rsp: 0,
        cr3: vmm::current_pml4(),
        kernel_stack_top: 0,
        stack_phys: 0,
        stack_pages: 0,
        exit_code: 0,
        quantum: QUANTUM,
    };

    scheduler.current = 0;
    scheduler.next_id = 1;
    scheduler.initialized = true;

    unsafe {
        scheduler.spawn_kernel_inner(idle, 1);
    }

    drop(scheduler);

    irq_restore(flags);
}

pub fn spawn_kernel(
    entry: extern "C" fn() -> !,
    priority: u8,
) -> Option<u64> {
    let flags = irq_save();

    let result = unsafe {
        SCHEDULER
            .lock()
            .spawn_kernel_inner(entry, priority)
    };

    irq_restore(flags);

    result
}

pub fn spawn_user(
    entry: u64,
    user_stack: u64,
    cr3: u64,
    priority: u8,
    arg0: u64,
) -> Option<u64> {
    let flags = irq_save();

    let result = unsafe {
        SCHEDULER
            .lock()
            .spawn_user_inner(
                entry,
                user_stack,
                cr3,
                priority,
                arg0,
            )
    };

    irq_restore(flags);

    result
}

pub fn on_timer(rsp: u64) -> u64 {
    let mut scheduler = SCHEDULER.lock();

    if !scheduler.initialized {
        return rsp;
    }

    scheduler.reap();

    let current = scheduler.current;

    if scheduler.tasks[current].state == TaskState::Running {
        scheduler.tasks[current].rsp = rsp;
    }

    let current_priority =
        scheduler.tasks[current].priority;

    let higher =
        scheduler.highest_ready_priority() > current_priority;

    if scheduler.tasks[current].state == TaskState::Running
        && scheduler.tasks[current].quantum > 1
        && !higher
    {
        scheduler.tasks[current].quantum -= 1;
        return rsp;
    }

    if scheduler.tasks[current].state == TaskState::Running {
        scheduler.tasks[current].state = TaskState::Ready;
    }

    let Some(next) = scheduler.select_next() else {
        scheduler.tasks[current].state = TaskState::Running;
        scheduler.tasks[current].quantum = QUANTUM;
        return rsp;
    };

    scheduler.tasks[next].state = TaskState::Running;
    scheduler.tasks[next].quantum = QUANTUM;
    scheduler.current = next;

    let task = scheduler.tasks[next];
    let next_rsp = task.rsp;

    drop(scheduler);

    activate(task);

    next_rsp
}

pub fn yield_from_interrupt(rsp: u64) -> u64 {
    let mut scheduler = SCHEDULER.lock();

    scheduler.reap();

    let current = scheduler.current;

    scheduler.tasks[current].rsp = rsp;

    if scheduler.tasks[current].state == TaskState::Running {
        scheduler.tasks[current].state = TaskState::Ready;
    }

    let Some(next) = scheduler.select_next() else {
        scheduler.tasks[current].state = TaskState::Running;
        return rsp;
    };

    scheduler.tasks[next].state = TaskState::Running;
    scheduler.tasks[next].quantum = QUANTUM;
    scheduler.current = next;

    let task = scheduler.tasks[next];
    let next_rsp = task.rsp;

    drop(scheduler);

    activate(task);

    next_rsp
}

pub fn exit_from_interrupt(
    rsp: u64,
    code: i32,
) -> u64 {
    let mut scheduler = SCHEDULER.lock();

    let current = scheduler.current;

    scheduler.tasks[current].rsp = rsp;
    scheduler.tasks[current].state = TaskState::Zombie;
    scheduler.tasks[current].exit_code = code;

    let next = scheduler
        .select_next()
        .expect("scheduler: no runnable task");

    scheduler.tasks[next].state = TaskState::Running;
    scheduler.tasks[next].quantum = QUANTUM;
    scheduler.current = next;

    let task = scheduler.tasks[next];
    let next_rsp = task.rsp;

    drop(scheduler);

    activate(task);

    next_rsp
}

pub fn current_pid() -> u64 {
    let flags = irq_save();

    let scheduler = SCHEDULER.lock();
    let id = scheduler.tasks[scheduler.current].id;

    drop(scheduler);

    irq_restore(flags);

    id
}
pub fn current_cr3() -> u64 {
    let flags = irq_save();

    let scheduler = SCHEDULER.lock();
    let cr3 = scheduler.tasks[scheduler.current].cr3;

    drop(scheduler);

    irq_restore(flags);

    cr3
}

pub fn yield_now() {
    unsafe {
        asm!("int 0x81");
    }
}
pub fn set_current_cr3(
    cr3: u64,
) {
    let flags = irq_save();

    let mut scheduler =
        SCHEDULER.lock();

    let current =
        scheduler.current;

    scheduler.tasks[current].cr3 =
        cr3;

    drop(scheduler);

    irq_restore(flags);
}
pub fn current_priority() -> u8 {
    let flags = irq_save();

    let scheduler =
        SCHEDULER.lock();

    let value =
        scheduler.tasks[
            scheduler.current
        ]
        .priority;

    drop(scheduler);

    irq_restore(flags);

    value
}
