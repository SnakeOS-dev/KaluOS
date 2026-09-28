#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use core::arch::asm;

use limine::request::{
    HhdmRequest,
    MemoryMapRequest,
    RequestsEndMarker,
    RequestsStartMarker,
};
use limine::BaseRevision;

mod abi;
mod ata;
mod block;
mod elf;
mod ext2;
mod gdt;
mod heap;
mod idt;
mod keyboard;
mod pic;
mod pit;
mod pmm;
mod process;
mod scheduler;
mod serial;
mod syscall;
mod vfs;
mod vmm;

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    assert!(BASE_REVISION.is_supported());

    unsafe {
        serial::init();
    }

    serial::write(format_args!(
        "KaluOS v0.02 booting...!\n"
    ));

    unsafe {
        gdt::init();
    }

    pmm::init(&MEMORY_MAP_REQUEST);

    serial::write(format_args!(
        "pmm: total={} free={}\n",
        pmm::total_pages(),
        pmm::free_pages()
    ));

    let hhdm = HHDM_REQUEST
        .get_response()
        .expect("HHDM unavailable")
        .offset();

    vmm::init(hhdm);

    serial::write(format_args!(
        "vmm: cr3={:#x}\n",
        vmm::current_pml4()
    ));

    unsafe {
        if !heap::init() {
            panic!("heap init failed");
        }
    }

    serial::write(format_args!(
        "heap: {} KiB\n",
        heap::size() / 1024
    ));

    unsafe {
        pic::init();
        idt::init();
        pit::init();
    }

    keyboard::init();

    let ata = unsafe {
        ata::AtaPio::probe_primary_master()
    }
    .expect("ATA drive not found");

    let sectors =
        crate::block::BlockDevice::sector_count(
            &ata,
        );

    serial::write(format_args!(
        "ata: {} sectors {} MiB\n",
        sectors,
        sectors / 2048
    ));

    block::register(
        Box::new(ata),
    );

    vfs::mount_root()
        .expect("EXT2 mount failed");

    serial::write(format_args!(
        "ext2: mounted rw\n"
    ));

    let root =
        vfs::readdir("/")
            .expect("root readdir failed");

    for entry in root {
        serial::write(format_args!(
            "root: ino={} name={}\n",
            entry.inode,
            entry.name
        ));
    }

    let init_stat =
        vfs::stat("/bin/init")
            .expect("/bin/init not found");

    if !init_stat.is_file {
        panic!("/bin/init is not a file");
    }

    serial::write(format_args!(
        "init: inode={} size={}\n",
        init_stat.inode,
        init_stat.size
    ));

    use alloc::{
    string::String,
    vec,
};

    let init_args =
    vec![
        String::from("/bin/init"),
    ];

    let init =
    elf::load(
        "/bin/init",
        &init_args,
    )
    .expect(
        "failed to load /bin/init"
    );
    serial::write(format_args!(
        "init: entry={:#x} cr3={:#x} stack={:#x}\n",
        init.entry,
        init.cr3,
        init.stack
    ));

    scheduler::init(5);

    process::register(
        scheduler::current_pid(),
    );

    let init_pid =
        scheduler::spawn_user(
            init.entry,
            init.stack,
            init.cr3,
            5,
            0,
        )
        .expect("failed to spawn init");

    process::register(init_pid);

    serial::write(format_args!(
        "init: pid={}\n",
        init_pid
    ));

    unsafe {
        asm!("sti");
    }

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(
    info: &core::panic::PanicInfo,
) -> ! {
    serial::write(format_args!(
        "\nKERNEL PANIC\n{}\n",
        info
    ));

    loop {
        unsafe {
            asm!("cli; hlt");
        }
    }
}
