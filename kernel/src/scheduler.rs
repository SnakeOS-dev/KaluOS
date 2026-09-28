use core::arch::{asm, global_asm};
use core::mem::size_of;

use spin::Mutex;

use crate::{
    gdt,
    pit,
    pmm,
    serial,
    vmm,
};

const MAX_TASKS: usize = 64;
const KERNEL_STACK_PAGES: usize = 8;
const DEFAULT_QUANTUM: u8 = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Empty,
    Ready,
    Running,
    Sleeping,
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
    pub parent_pid: u64,
    pub state: TaskState,
    pub kind: TaskKind,
    pub priority: u8,
    pub rsp: u64,
    pub cr3: u64,
    pub kernel_stack_top: u64,
    pub stack_phys: u64,
    pub stack_pages: usize,
    pub exit_code: i32,
    pub wake_tick: u64,
    pub quantum: u8,
}

impl Task {
    pub const fn empty() -> Self {
        Self {
            id: 0,
            parent_pid: 0,
            state: TaskState::Empty,
            kind: TaskKind::Kernel,
            priority: 1,
            rsp: 0,
            cr3: 0,
            kernel_stack_top: 0,
            stack_phys: 0,
            stack_pages: 0,
            exit_code: 0,
            wake_tick: 0,
            quantum: DEFAULT_QUANTUM,
        }
    }
}

pub enum WaitResult {
    Exited(u64, i32),
    Running,
    NoChild,
}

struct Scheduler {
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

    fn alloc_slot(&self) -> Option<usize> {
        for i in 0..MAX_TASKS {
            if self.tasks[i].state
                == TaskState::Empty
            {
                return Some(i);
            }
        }

        None
    }

    fn highest_ready_priority(
        &self,
    ) -> Option<u8> {
        let mut highest = None;

        for task in &self.tasks {
            if task.state
                != TaskState::Ready
            {
                continue;
            }

            match highest {
                Some(value)
                    if value >= task.priority =>
                {
                }

                _ => {
                    highest =
                        Some(task.priority);
                }
            }
        }

        highest
    }

    fn select_next(
        &self,
    ) -> Option<usize> {
        for priority in (1..=10).rev() {
            for offset in 1..=MAX_TASKS {
                let index =
                    (
                        self.current
                            + offset
                    )
                        % MAX_TASKS;

                let task =
                    self.tasks[index];

                if task.state
                    == TaskState::Ready
                    && task.priority
                        == priority
                {
                    return Some(index);
                }
            }
        }

        None
    }

    fn wake_sleepers(
        &mut self,
        now: u64,
    ) {
        for i in 0..MAX_TASKS {
            if self.tasks[i].state
                == TaskState::Sleeping
                && self.tasks[i]
                    .wake_tick
                    <= now
            {
                self.tasks[i].state =
                    TaskState::Ready;

                self.tasks[i]
                    .wake_tick = 0;
            }
        }
    }
}

static SCHEDULER: Mutex<Scheduler> =
    Mutex::new(Scheduler::new());

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

#[repr(C)]
struct UserFrame {
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
    rsp: u64,
    ss: u64,
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

extern "C" fn idle_task() -> ! {
    loop {
        unsafe {
            asm!(
                "hlt",
                options(
                    nomem,
                    nostack
                )
            );
        }
    }
}

fn irq_save() -> u64 {
    let flags: u64;

    unsafe {
        asm!(
            "pushfq",
            "pop {}",
            "cli",
            out(reg) flags,
            options(
                nomem
            )
        );
    }

    flags
}

fn irq_restore(flags: u64) {
    if flags & (1 << 9) != 0 {
        unsafe {
            asm!(
                "sti",
                options(
                    nomem,
                    nostack
                )
            );
        }
    }
}

fn allocate_kernel_stack()
    -> Option<(u64, u64)>
{
    let phys =
        pmm::alloc_many(
            KERNEL_STACK_PAGES,
        )?;

    let virt =
        vmm::phys_to_virt_addr(
            phys,
        );

    let top =
        virt
            + KERNEL_STACK_PAGES
                as u64
                * pmm::PAGE_SIZE;

    Some((phys, top))
}

unsafe fn build_kernel_frame(
    stack_top: u64,
    entry: extern "C" fn() -> !,
) -> u64 {
    let aligned_top =
        stack_top & !0xF;

    let rsp =
        aligned_top
            - size_of::<KernelFrame>()
                as u64;

    let frame =
        rsp as *mut KernelFrame;

    unsafe {
        frame.write(
            KernelFrame {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: 0,
                r11: 0,
                r10: 0,
                r9: 0,
                r8: 0,
                rdi:
                    entry
                        as usize
                        as u64,
                rsi:
                    stack_top,
                rbp: 0,
                rdx: 0,
                rcx: 0,
                rbx: 0,
                rax: 0,
                rip:
                    kernel_task_bootstrap
                        as usize
                        as u64,
                cs:
                    gdt::KERNEL_CODE
                        as u64,
                rflags: 0x2,
            },
        );
    }

    rsp
}

unsafe fn build_user_frame(
    kernel_stack_top: u64,
    entry: u64,
    user_stack: u64,
) -> u64 {
    let aligned_top =
        kernel_stack_top & !0xF;

    let rsp =
        aligned_top
            - size_of::<UserFrame>()
                as u64;

    let frame =
        rsp as *mut UserFrame;

    unsafe {
        frame.write(
            UserFrame {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: 0,
                r11: 0,
                r10: 0,
                r9: 0,
                r8: 0,
                rdi: 0,
                rsi: 0,
                rbp: 0,
                rdx: 0,
                rcx: 0,
                rbx: 0,
                rax: 0,
                rip: entry,
                cs:
                    gdt::USER_CODE
                        as u64,
                rflags: 0x202,
                rsp: user_stack,
                ss:
                    gdt::USER_DATA
                        as u64,
            },
        );
    }

    rsp
}

fn activate(task: Task) {
    let current =
        vmm::current_pml4();

    if current != task.cr3 {
        unsafe {
            vmm::switch_address_space(
                task.cr3,
            );
        }
    }

    if task.kernel_stack_top != 0 {
        gdt::set_rsp0(
            task.kernel_stack_top,
        );
    }
}

pub fn init(
    boot_priority: u8,
) {
    let flags =
        irq_save();

    {
        let mut scheduler =
            SCHEDULER.lock();

        if scheduler.initialized {
            drop(scheduler);
            irq_restore(flags);
            return;
        }

        let cr3 =
            vmm::current_pml4();

        scheduler.tasks[0] =
            Task {
                id: 0,
                parent_pid: 0,
                state:
                    TaskState::Running,
                kind:
                    TaskKind::Kernel,
                priority:
                    boot_priority
                        .clamp(1, 10),
                rsp: 0,
                cr3,
                kernel_stack_top: 0,
                stack_phys: 0,
                stack_pages: 0,
                exit_code: 0,
                wake_tick: 0,
                quantum:
                    DEFAULT_QUANTUM,
            };

        scheduler.current = 0;
        scheduler.next_id = 1;
        scheduler.initialized = true;
    }

    irq_restore(flags);

    let _ =
        spawn_kernel(
            idle_task,
            1,
            0,
        );
}

pub fn spawn_kernel(
    entry: extern "C" fn() -> !,
    priority: u8,
    parent_pid: u64,
) -> Option<u64> {
    let flags =
        irq_save();

    let (
        stack_phys,
        stack_top,
    ) =
        match allocate_kernel_stack() {
            Some(value) => value,

            None => {
                irq_restore(flags);
                return None;
            }
        };

    let rsp =
        unsafe {
            build_kernel_frame(
                stack_top,
                entry,
            )
        };

    let mut scheduler =
        SCHEDULER.lock();

    let Some(slot) =
        scheduler.alloc_slot()
    else {
        drop(scheduler);

        pmm::free_many(
            stack_phys,
            KERNEL_STACK_PAGES,
        );

        irq_restore(flags);

        return None;
    };

    let id =
        scheduler.next_id;

    scheduler.next_id =
        scheduler
            .next_id
            .wrapping_add(1);

    if scheduler.next_id == 0 {
        scheduler.next_id = 1;
    }

    scheduler.tasks[slot] =
        Task {
            id,
            parent_pid,
            state:
                TaskState::Ready,
            kind:
                TaskKind::Kernel,
            priority:
                priority.clamp(1, 10),
            rsp,
            cr3:
                vmm::current_pml4(),
            kernel_stack_top:
                stack_top,
            stack_phys,
            stack_pages:
                KERNEL_STACK_PAGES,
            exit_code: 0,
            wake_tick: 0,
            quantum:
                DEFAULT_QUANTUM,
        };

    serial::write(
        format_args!(
            "task: spawned id={} entry={:#x} stack={:#x} rsp={:#x}\n",
            id,
            entry as usize as u64,
            stack_top,
            rsp
        ),
    );

    drop(scheduler);

    irq_restore(flags);

    Some(id)
}

pub fn spawn_user(
    entry: u64,
    user_stack: u64,
    cr3: u64,
    priority: u8,
    parent_pid: u64,
) -> Option<u64> {
    let flags =
        irq_save();

    let (
        stack_phys,
        kernel_stack_top,
    ) =
        match allocate_kernel_stack() {
            Some(value) => value,

            None => {
                irq_restore(flags);
                return None;
            }
        };

    let rsp =
        unsafe {
            build_user_frame(
                kernel_stack_top,
                entry,
                user_stack,
            )
        };

    let mut scheduler =
        SCHEDULER.lock();

    let Some(slot) =
        scheduler.alloc_slot()
    else {
        drop(scheduler);

        pmm::free_many(
            stack_phys,
            KERNEL_STACK_PAGES,
        );

        irq_restore(flags);

        return None;
    };

    let id =
        scheduler.next_id;

    scheduler.next_id =
        scheduler
            .next_id
            .wrapping_add(1);

    if scheduler.next_id == 0 {
        scheduler.next_id = 1;
    }

    scheduler.tasks[slot] =
        Task {
            id,
            parent_pid,
            state:
                TaskState::Ready,
            kind:
                TaskKind::User,
            priority:
                priority.clamp(1, 10),
            rsp,
            cr3,
            kernel_stack_top,
            stack_phys,
            stack_pages:
                KERNEL_STACK_PAGES,
            exit_code: 0,
            wake_tick: 0,
            quantum:
                DEFAULT_QUANTUM,
        };

    drop(scheduler);

    irq_restore(flags);

    Some(id)
}

pub fn on_timer(
    rsp: u64,
) -> u64 {
    let flags =
        irq_save();

    let now =
        pit::ticks();

    let mut scheduler =
        SCHEDULER.lock();

    if !scheduler.initialized {
        drop(scheduler);
        irq_restore(flags);
        return rsp;
    }

    scheduler.wake_sleepers(now);

    let current =
        scheduler.current;

    if scheduler.tasks[current].state
        != TaskState::Running
    {
        drop(scheduler);
        irq_restore(flags);
        return rsp;
    }

    scheduler.tasks[current].rsp =
        rsp;

    let current_priority =
        scheduler.tasks[current]
            .priority;

    let higher_ready =
        scheduler
            .highest_ready_priority()
            .map(
                |priority| {
                    priority
                        > current_priority
                },
            )
            .unwrap_or(false);

    if !higher_ready {
        if scheduler.tasks[current]
            .quantum
            > 1
        {
            scheduler.tasks[current]
                .quantum -= 1;

            drop(scheduler);

            irq_restore(flags);

            return rsp;
        }
    }

    scheduler.tasks[current]
        .quantum =
        DEFAULT_QUANTUM;

    scheduler.tasks[current].state =
        TaskState::Ready;

    let Some(next) =
        scheduler.select_next()
    else {
        scheduler.tasks[current].state =
            TaskState::Running;

        drop(scheduler);

        irq_restore(flags);

        return rsp;
    };

    if next == current {
        scheduler.tasks[current].state =
            TaskState::Running;

        drop(scheduler);

        irq_restore(flags);

        return rsp;
    }

    scheduler.current =
        next;

    scheduler.tasks[next].state =
        TaskState::Running;

    scheduler.tasks[next].quantum =
        DEFAULT_QUANTUM;

    let task =
        scheduler.tasks[next];

    drop(scheduler);

    activate(task);

    irq_restore(flags);

    task.rsp
}

pub fn yield_from_interrupt(
    rsp: u64,
) -> u64 {
    let flags =
        irq_save();

    let mut scheduler =
        SCHEDULER.lock();

    if !scheduler.initialized {
        drop(scheduler);
        irq_restore(flags);
        return rsp;
    }

    let current =
        scheduler.current;

    scheduler.tasks[current].rsp =
        rsp;

    if scheduler.tasks[current].state
        == TaskState::Running
    {
        scheduler.tasks[current].state =
            TaskState::Ready;
    }

    scheduler.tasks[current].quantum =
        DEFAULT_QUANTUM;

    let Some(next) =
        scheduler.select_next()
    else {
        scheduler.tasks[current].state =
            TaskState::Running;

        drop(scheduler);

        irq_restore(flags);

        return rsp;
    };

    scheduler.current =
        next;

    scheduler.tasks[next].state =
        TaskState::Running;

    scheduler.tasks[next].quantum =
        DEFAULT_QUANTUM;

    let task =
        scheduler.tasks[next];

    drop(scheduler);

    activate(task);

    irq_restore(flags);

    task.rsp
}

pub fn sleep_from_interrupt(
    rsp: u64,
    ticks: u64,
) -> u64 {
    if ticks == 0 {
        return yield_from_interrupt(
            rsp,
        );
    }

    let now =
        pit::ticks();

    let flags =
        irq_save();

    let mut scheduler =
        SCHEDULER.lock();

    let current =
        scheduler.current;

    scheduler.tasks[current].rsp =
        rsp;

    scheduler.tasks[current]
        .wake_tick =
        now.saturating_add(ticks);

    scheduler.tasks[current].state =
        TaskState::Sleeping;

    scheduler.tasks[current].quantum =
        DEFAULT_QUANTUM;

    let Some(next) =
        scheduler.select_next()
    else {
        scheduler.tasks[current].state =
            TaskState::Running;

        scheduler.tasks[current]
            .wake_tick = 0;

        drop(scheduler);

        irq_restore(flags);

        return rsp;
    };

    scheduler.current =
        next;

    scheduler.tasks[next].state =
        TaskState::Running;

    scheduler.tasks[next].quantum =
        DEFAULT_QUANTUM;

    let task =
        scheduler.tasks[next];

    drop(scheduler);

    activate(task);

    irq_restore(flags);

    task.rsp
}

pub fn exit_from_interrupt(
    rsp: u64,
    code: i32,
) -> u64 {
    let flags =
        irq_save();

    let mut scheduler =
        SCHEDULER.lock();

    let current =
        scheduler.current;

    scheduler.tasks[current].rsp =
        rsp;

    scheduler.tasks[current]
        .exit_code =
        code;

    scheduler.tasks[current].state =
        TaskState::Zombie;

    scheduler.tasks[current].quantum =
        DEFAULT_QUANTUM;

    let Some(next) =
        scheduler.select_next()
    else {
        drop(scheduler);

        irq_restore(flags);

        panic!(
            "scheduler: no runnable task after exit"
        );
    };

    scheduler.current =
        next;

    scheduler.tasks[next].state =
        TaskState::Running;

    scheduler.tasks[next].quantum =
        DEFAULT_QUANTUM;

    let task =
        scheduler.tasks[next];

    drop(scheduler);

    activate(task);

    irq_restore(flags);

    task.rsp
}

pub fn wait_child(
    parent_pid: u64,
    wanted_pid: u64,
) -> WaitResult {
    let flags =
        irq_save();

    let mut scheduler =
        SCHEDULER.lock();

    let mut has_child =
        false;

    for i in 0..MAX_TASKS {
        let task =
            scheduler.tasks[i];

        if task.state
            == TaskState::Empty
        {
            continue;
        }

        if task.parent_pid
            != parent_pid
        {
            continue;
        }

        if wanted_pid != 0
            && task.id
                != wanted_pid
        {
            continue;
        }

        has_child = true;

        if task.state
            == TaskState::Zombie
        {
            let pid =
                task.id;

            let code =
                task.exit_code;

            let stack_phys =
                task.stack_phys;

            let stack_pages =
                task.stack_pages;

            scheduler.tasks[i] =
                Task::empty();

            drop(scheduler);

            if stack_phys != 0
                && stack_pages != 0
            {
                pmm::free_many(
                    stack_phys,
                    stack_pages,
                );
            }

            irq_restore(flags);

            return WaitResult::Exited(
                pid,
                code,
            );
        }
    }

    drop(scheduler);

    irq_restore(flags);

    if has_child {
        WaitResult::Running
    } else {
        WaitResult::NoChild
    }
}

pub fn current_pid() -> u64 {
    let flags =
        irq_save();

    let scheduler =
        SCHEDULER.lock();

    let pid =
        scheduler.tasks[
            scheduler.current
        ]
        .id;

    drop(scheduler);

    irq_restore(flags);

    pid
}

pub fn current_parent_pid()
    -> u64
{
    let flags =
        irq_save();

    let scheduler =
        SCHEDULER.lock();

    let pid =
        scheduler.tasks[
            scheduler.current
        ]
        .parent_pid;

    drop(scheduler);

    irq_restore(flags);

    pid
}

pub fn current_cr3() -> u64 {
    let flags =
        irq_save();

    let scheduler =
        SCHEDULER.lock();

    let cr3 =
        scheduler.tasks[
            scheduler.current
        ]
        .cr3;

    drop(scheduler);

    irq_restore(flags);

    cr3
}

pub fn current_priority() -> u8 {
    let flags =
        irq_save();

    let scheduler =
        SCHEDULER.lock();

    let priority =
        scheduler.tasks[
            scheduler.current
        ]
        .priority;

    drop(scheduler);

    irq_restore(flags);

    priority
}

pub fn set_current_cr3(
    cr3: u64,
) {
    let flags =
        irq_save();

    let mut scheduler =
        SCHEDULER.lock();

    let current =
        scheduler.current;

    scheduler.tasks[current].cr3 =
        cr3;

    drop(scheduler);

    irq_restore(flags);
}

pub fn set_priority(
    pid: u64,
    priority: u8,
) -> bool {
    let flags =
        irq_save();

    let mut scheduler =
        SCHEDULER.lock();

    for task in &mut scheduler.tasks {
        if task.state
            != TaskState::Empty
            && task.id == pid
        {
            task.priority =
                priority.clamp(1, 10);

            drop(scheduler);

            irq_restore(flags);

            return true;
        }
    }

    drop(scheduler);

    irq_restore(flags);

    false
}

pub fn task_exists(
    pid: u64,
) -> bool {
    let flags =
        irq_save();

    let scheduler =
        SCHEDULER.lock();

    let exists =
        scheduler
            .tasks
            .iter()
            .any(
                |task| {
                    task.state
                        != TaskState::Empty
                        && task.id
                            == pid
                },
            );

    drop(scheduler);

    irq_restore(flags);

    exists
}
