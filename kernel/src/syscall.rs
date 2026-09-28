extern crate alloc;

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::arch::asm;

use crate::{
    abi::*,
    elf,
    keyboard,
    pit,
    pmm,
    process,
    scheduler,
    serial,
    vfs,
    vmm,
};

const EINVAL: i64 = -1;
const ENOENT: i64 = -2;
const EBADF: i64 = -3;
const ENOMEM: i64 = -4;
const EIO: i64 = -5;
const ENOTDIR: i64 = -6;
const EISDIR: i64 = -7;
const EEXIST: i64 = -8;
const EAGAIN: i64 = -9;

#[repr(C)]
pub struct SyscallFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}
fn copy_arguments(
    argc: usize,
    argv_ptr: u64,
) -> Option<Vec<String>> {
    if argc > 128 {
        return None;
    }

    let mut result =
        Vec::with_capacity(argc);

    for index in 0..argc {
        let base =
            argv_ptr
                + (
                    index
                    * core::mem::size_of::<UserArg>()
                ) as u64;

        let raw =
            copy_from_user(
                base,
                core::mem::size_of::<UserArg>(),
            )?;

        let ptr =
            u64::from_le_bytes(
                raw[0..8]
                    .try_into()
                    .ok()?,
            );

        let len =
            u64::from_le_bytes(
                raw[8..16]
                    .try_into()
                    .ok()?,
            ) as usize;

        if len > 4096 {
            return None;
        }

        let string =
            read_string(
                ptr,
                len,
            )?;

        result.push(string);
    }

    Some(result)
}
fn err(value: i64) -> u64 {
    value as u64
}

fn current_cr3() -> u64 {
    scheduler::current_cr3()
}

fn read_user_byte(
    address: u64,
) -> Option<u8> {
    let phys =
        unsafe {
            vmm::translate(
                current_cr3(),
                address,
            )
        }?;

    let ptr =
        vmm::phys_to_virt_addr(
            phys,
        )
        as *const u8;

    Some(
        unsafe {
            ptr.read_volatile()
        },
    )
}

fn write_user_byte(
    address: u64,
    value: u8,
) -> bool {
    let Some(phys) =
        (unsafe {
            vmm::translate(
                current_cr3(),
                address,
            )
        })
    else {
        return false;
    };

    let ptr =
        vmm::phys_to_virt_addr(
            phys,
        )
        as *mut u8;

    unsafe {
        ptr.write_volatile(
            value,
        );
    }

    true
}

fn copy_from_user(
    address: u64,
    length: usize,
) -> Option<Vec<u8>> {
    let mut result =
        Vec::with_capacity(length);

    for i in 0..length {
        result.push(
            read_user_byte(
                address
                    + i as u64,
            )?,
        );
    }

    Some(result)
}

fn copy_to_user(
    address: u64,
    data: &[u8],
) -> bool {
    for (
        i,
        byte,
    ) in data.iter().enumerate()
    {
        if !write_user_byte(
            address + i as u64,
            *byte,
        ) {
            return false;
        }
    }

    true
}

fn read_string(
    ptr: u64,
    len: usize,
) -> Option<String> {
    if len == 0 || len > 4096 {
        return None;
    }

    let bytes =
        copy_from_user(
            ptr,
            len,
        )?;

    let string =
        core::str::from_utf8(
            &bytes,
        )
        .ok()?;

    Some(
        string.to_string(),
    )
}

fn canonical_path(
    path: &str,
) -> Option<String> {
    if path.starts_with('/') {
        return Some(
            normalize(path),
        );
    }

    let pid =
        scheduler::current_pid();

    let cwd =
        process::cwd(pid)?;

    if cwd == "/" {
        Some(
            normalize(
                &alloc::format!(
                    "/{}",
                    path
                ),
            ),
        )
    } else {
        Some(
            normalize(
                &alloc::format!(
                    "{}/{}",
                    cwd,
                    path
                ),
            ),
        )
    }
}

fn normalize(
    path: &str,
) -> String {
    let mut parts:
        Vec<&str> = Vec::new();

    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => {
                parts.push(part);
            }
        }
    }

    if parts.is_empty() {
        return "/".to_string();
    }

    let mut result =
        String::from("/");

    for (
        i,
        part,
    ) in parts.iter().enumerate()
    {
        if i != 0 {
            result.push('/');
        }

        result.push_str(part);
    }

    result
}

fn syscall_text(
    ptr: u64,
    len: usize,
) -> u64 {
    let Some(data) =
        copy_from_user(
            ptr,
            len,
        )
    else {
        return err(EINVAL);
    };

    for byte in &data {
        serial::write(
            format_args!(
                "{}",
                *byte as char
            ),
        );
    }

    len as u64
}

fn syscall_read(
    fd: usize,
    ptr: u64,
    len: usize,
) -> u64 {
    let pid =
        scheduler::current_pid();

    if fd == 0 {
        let Some(byte) =
            keyboard::getc()
        else {
            return err(EAGAIN);
        };

        if len == 0 {
            return 0;
        }

        if !write_user_byte(
            ptr,
            byte,
        ) {
            return err(EINVAL);
        }

        return 1;
    }

    let Some(mut descriptor) =
        process::fd(pid, fd)
    else {
        return err(EBADF);
    };

    let stat =
        match vfs::stat(
            &descriptor.path,
        ) {
            Ok(value) => value,
            Err(_) =>
                return err(ENOENT),
        };

    if stat.is_dir {
        return err(EISDIR);
    }

    let mut data =
        vec![0u8; len];

    let count =
        match vfs::read(
            &descriptor.path,
            descriptor.offset,
            &mut data,
        ) {
            Ok(value) => value,
            Err(_) =>
                return err(EIO),
        };

    if !copy_to_user(
        ptr,
        &data[..count],
    ) {
        return err(EINVAL);
    }

    descriptor.offset +=
        count as u64;

    process::update_fd(
        pid,
        fd,
        descriptor,
    );

    count as u64
}

fn syscall_write(
    fd: usize,
    ptr: u64,
    len: usize,
) -> u64 {
    let Some(data) =
        copy_from_user(
            ptr,
            len,
        )
    else {
        return err(EINVAL);
    };

    if fd == 1 || fd == 2 {
        for byte in &data {
            serial::write(
                format_args!(
                    "{}",
                    *byte as char
                ),
            );
        }

        return len as u64;
    }

    let pid =
        scheduler::current_pid();

    let Some(mut descriptor) =
        process::fd(pid, fd)
    else {
        return err(EBADF);
    };

    if descriptor.flags
        & O_WRITE
        == 0
    {
        return err(EBADF);
    }

    let count =
        match vfs::write(
            &descriptor.path,
            descriptor.offset,
            &data,
        ) {
            Ok(value) => value,
            Err(_) =>
                return err(EIO),
        };

    descriptor.offset +=
        count as u64;

    process::update_fd(
        pid,
        fd,
        descriptor,
    );

    count as u64
}

fn syscall_readdir(
    fd: usize,
    user_ptr: u64,
) -> u64 {
    let pid =
        scheduler::current_pid();

    let Some(mut descriptor) =
        process::fd(pid, fd)
    else {
        return err(EBADF);
    };

    let entries =
        match vfs::readdir(
            &descriptor.path,
        ) {
            Ok(value) => value,
            Err(_) =>
                return err(ENOTDIR),
        };

    if descriptor.directory_index
        >= entries.len()
    {
        return 0;
    }

    let entry =
        &entries[
            descriptor.directory_index
        ];

    descriptor.directory_index += 1;

    process::update_fd(
        pid,
        fd,
        descriptor,
    );

    let mut dirent =
        Dirent {
            inode: entry.inode,
            kind: entry.file_type,
            name_len:
                entry.name
                    .len()
                    .min(255)
                    as u8,
            reserved: 0,
            name: [0; 256],
        };

    let len =
        dirent.name_len as usize;

    dirent.name[..len]
        .copy_from_slice(
            &entry
                .name
                .as_bytes()[..len],
        );

    let bytes =
        unsafe {
            core::slice::from_raw_parts(
                &dirent
                    as *const Dirent
                    as *const u8,
                core::mem::size_of::<Dirent>(),
            )
        };

    if !copy_to_user(
        user_ptr,
        bytes,
    ) {
        return err(EINVAL);
    }

    1
}

fn syscall_chdir(
    ptr: u64,
    len: usize,
) -> u64 {
    let Some(raw) =
        read_string(
            ptr,
            len,
        )
    else {
        return err(EINVAL);
    };

    let Some(path) =
        canonical_path(
            &raw,
        )
    else {
        return err(EINVAL);
    };

    let stat =
        match vfs::stat(&path) {
            Ok(value) => value,
            Err(_) =>
                return err(ENOENT),
        };

    if !stat.is_dir {
        return err(ENOTDIR);
    }

    if !process::set_cwd(
        scheduler::current_pid(),
        path,
    ) {
        return err(EIO);
    }

    0
}

fn syscall_remove(
    ptr: u64,
    len: usize,
) -> u64 {
    let Some(raw) =
        read_string(ptr, len)
    else {
        return err(EINVAL);
    };

    let Some(path) =
        canonical_path(&raw)
    else {
        return err(EINVAL);
    };

    let stat =
        match vfs::stat(&path) {
            Ok(value) => value,
            Err(_) =>
                return err(ENOENT),
        };

    if stat.is_dir {
        return err(EISDIR);
    }

    match vfs::unlink(&path) {
        Ok(()) => 0,
        Err(_) => err(EIO),
    }
}

fn syscall_spawn(
    ptr: u64,
    len: usize,
    argc: usize,
    argv_ptr: u64,
    priority: u8,
) -> u64 {
    let Some(raw_path) =
        read_string(
            ptr,
            len,
        )
    else {
        return err(EINVAL);
    };

    let Some(path) =
        canonical_path(
            &raw_path,
        )
    else {
        return err(EINVAL);
    };

    let Some(mut argv) =
        copy_arguments(
            argc,
            argv_ptr,
        )
    else {
        return err(EINVAL);
    };

    if argv.is_empty() {
        argv.push(
            path.clone(),
        );
    }

    let program =
        match elf::load(
            &path,
            &argv,
        ) {
            Ok(value) => value,

            Err(_) =>
                return err(ENOENT),
        };

	let parent_pid =
   	    scheduler::current_pid();

	let Some(pid) =
		scheduler::spawn_user(
        	program.entry,
	        program.stack,
		program.cr3,
	        priority.clamp(1, 10),
        	parent_pid,
	    )
	else {
	    return err(ENOMEM);
	};

	process::register(pid);

	pid
}
fn syscall_getc() -> u64 {
    match keyboard::getc() {
        Some(value) =>
            value as u64,

        None =>
            err(EAGAIN),
    }
}

fn syscall_map(
    address: u64,
    length: u64,
    flags: u64,
) -> u64 {
    if length == 0 {
        return err(EINVAL);
    }

    let pid =
        scheduler::current_pid();

    let base =
        if address == 0 {
            match process::alloc_map(
                pid,
                length,
            ) {
                Some(value) => value,
                None =>
                    return err(ENOMEM),
            }
        } else {
            address & !0xFFF
        };

    let pages =
        (length + 4095) / 4096;

    let cr3 =
        scheduler::current_cr3();

    for i in 0..pages {
        let phys =
            match pmm::alloc() {
                Some(value) => value,
                None =>
                    return err(ENOMEM),
            };

        vmm::zero_phys_page(
            phys,
        );

        let mut map_flags =
            vmm::USER;

        if flags
            & MAP_WRITE
            != 0
        {
            map_flags |=
                vmm::WRITABLE;
        }

        if flags
            & MAP_EXEC
            == 0
        {
            map_flags |=
                vmm::NO_EXECUTE;
        }

        if unsafe {
            vmm::map_page(
                cr3,
                base + i * 4096,
                phys,
                map_flags,
            )
        }
        .is_err()
        {
            pmm::free(phys);

            return err(EIO);
        }
    }

    base
}

fn syscall_unmap(
    address: u64,
    length: u64,
) -> u64 {
    if address & 0xFFF != 0
        || length == 0
    {
        return err(EINVAL);
    }

    let pages =
        (length + 4095) / 4096;

    let cr3 =
        scheduler::current_cr3();

    for i in 0..pages {
        let virt =
            address + i * 4096;

        if let Ok(phys) =
            unsafe {
                vmm::unmap_page(
                    cr3,
                    virt,
                )
            }
        {
            pmm::free(phys);
        }
    }

    0
}

fn syscall_sysname(
    ptr: u64,
    length: usize,
) -> u64 {
    let name =
        b"KaluOS";

    let count =
        name.len().min(length);

    if !copy_to_user(
        ptr,
        &name[..count],
    ) {
        return err(EINVAL);
    }

    count as u64
}

fn syscall_exec(
    frame: &mut SyscallFrame,
    ptr: u64,
    len: usize,
    argc: usize,
    argv_ptr: u64,
) -> u64 {
    let Some(raw) =
        read_string(
            ptr,
            len,
        )
    else {
        return err(EINVAL);
    };

    let Some(path) =
        canonical_path(
            &raw,
        )
    else {
        return err(EINVAL);
    };

    let Some(mut argv) =
        copy_arguments(
            argc,
            argv_ptr,
        )
    else {
        return err(EINVAL);
    };

    if argv.is_empty() {
        argv.push(
            path.clone(),
        );
    }

    let program =
        match elf::load(
            &path,
            &argv,
        ) {
            Ok(value) => value,

            Err(_) =>
                return err(ENOENT),
        };

    scheduler::set_current_cr3(
        program.cr3,
    );

    unsafe {
        vmm::switch_address_space(
            program.cr3,
        );
    }

    frame.rip =
        program.entry;

    frame.rsp =
        program.stack;

    frame.cs =
        crate::gdt::USER_CODE
            as u64;

    frame.ss =
        crate::gdt::USER_DATA
            as u64;

    frame.rflags = 0x202;

    frame.rax = 0;
    frame.rbx = 0;
    frame.rcx = 0;
    frame.rdx = 0;
    frame.rsi = 0;
    frame.rdi = 0;
    frame.rbp = 0;
    frame.r8 = 0;
    frame.r9 = 0;
    frame.r10 = 0;
    frame.r11 = 0;
    frame.r12 = 0;
    frame.r13 = 0;
    frame.r14 = 0;
    frame.r15 = 0;

    0
}
fn syscall_open(
    ptr: u64,
    len: usize,
    flags: u64,
) -> u64 {
    let Some(raw) =
        read_string(
            ptr,
            len,
        )
    else {
        return err(EINVAL);
    };

    let Some(path) =
        canonical_path(
            &raw,
        )
    else {
        return err(EINVAL);
    };

    match vfs::stat(&path) {
        Ok(stat) => {
            if flags & O_TRUNC != 0
                && stat.is_file
            {
                if vfs::truncate(
                    &path,
                )
                .is_err()
                {
                    return err(EIO);
                }
            }
        }

        Err(_) => {
            if flags
                & O_CREATE
                == 0
            {
                return err(ENOENT);
            }

            if vfs::create(
                &path,
                0o644,
            )
            .is_err()
            {
                return err(EIO);
            }
        }
    }

    match process::open(
        scheduler::current_pid(),
        path,
        flags,
    ) {
        Some(fd) => fd,
        None => err(EBADF),
    }
}

fn syscall_mkdir(
    ptr: u64,
    len: usize,
) -> u64 {
    let Some(raw) =
        read_string(
            ptr,
            len,
        )
    else {
        return err(EINVAL);
    };

    let Some(path) =
        canonical_path(
            &raw,
        )
    else {
        return err(EINVAL);
    };

    match vfs::mkdir(
        &path,
        0o755,
    ) {
        Ok(_) => 0,
        Err(
            vfs::VfsError::Exists
        ) => err(EEXIST),
        Err(_) => err(EIO),
    }
}

fn syscall_rmdir(
    ptr: u64,
    len: usize,
) -> u64 {
    let Some(raw) =
        read_string(
            ptr,
            len,
        )
    else {
        return err(EINVAL);
    };

    let Some(path) =
        canonical_path(
            &raw,
        )
    else {
        return err(EINVAL);
    };

    let stat =
        match vfs::stat(&path) {
            Ok(value) => value,
            Err(_) =>
                return err(ENOENT),
        };

    if !stat.is_dir {
        return err(ENOTDIR);
    }

    match vfs::unlink(&path) {
        Ok(()) => 0,
        Err(
            vfs::VfsError::NotEmpty
        ) => err(EEXIST),
        Err(_) => err(EIO),
    }
}

fn syscall_test_file(
    ptr: u64,
    len: usize,
) -> u64 {
    let Some(raw) =
        read_string(
            ptr,
            len,
        )
    else {
        return 0;
    };

    let Some(path) =
        canonical_path(
            &raw,
        )
    else {
        return 0;
    };

    match vfs::stat(&path) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}
fn syscall_wait(
    wanted_pid: u64,
    status_ptr: u64,
) -> u64 {
    let parent =
        scheduler::current_pid();

    match scheduler::wait_child(
        parent,
        wanted_pid,
    ) {
        scheduler::WaitResult::Exited(
            pid,
            code,
        ) => {
            if status_ptr != 0 {
                let bytes =
                    code.to_le_bytes();

                if !copy_to_user(
                    status_ptr,
                    &bytes,
                ) {
                    return err(EINVAL);
                }
            }

            process::remove(pid);

            pid
        }

        scheduler::WaitResult::Running =>
            err(EAGAIN),

        scheduler::WaitResult::NoChild =>
            err(ENOENT),
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn syscall_interrupt_entry(
    frame_rsp: u64,
) -> u64 {
    let frame =
        unsafe {
            &mut *(
                frame_rsp
                    as *mut SyscallFrame
            )
        };

    frame.rax =
        match frame.rax {
            SYS_TEXT =>
                syscall_text(
                    frame.rdi,
                    frame.rsi
                        as usize,
                ),

            SYS_READ =>
                syscall_read(
                    frame.rdi
                        as usize,
                    frame.rsi,
                    frame.rdx
                        as usize,
                ),

            SYS_WRITE =>
                syscall_write(
                    frame.rdi
                        as usize,
                    frame.rsi,
                    frame.rdx
                        as usize,
                ),

            SYS_READDIR =>
                syscall_readdir(
                    frame.rdi
                        as usize,
                    frame.rsi,
                ),

            SYS_CHDIR =>
                syscall_chdir(
                    frame.rdi,
                    frame.rsi
                        as usize,
                ),

            SYS_REMOVE =>
                syscall_remove(
                    frame.rdi,
                    frame.rsi
                        as usize,
                ),

            SYS_SPAWN =>
	    syscall_spawn(
        	frame.rdi,
	        frame.rsi as usize,
        	frame.rdx as usize,
	        frame.r10,
        	frame.r8 as u8,
	    ),
            SYS_GETC =>
                syscall_getc(),

            SYS_MAP =>
                syscall_map(
                    frame.rdi,
                    frame.rsi,
                    frame.rdx,
                ),

            SYS_UNMAP =>
                syscall_unmap(
                    frame.rdi,
                    frame.rsi,
                ),

            SYS_YIELD => {
                frame.rax = 0;

                return scheduler
                    ::yield_from_interrupt(
                        frame_rsp,
                    );
            }

            SYS_TICKS =>
                pit::ticks(),

            SYS_SYSNAME =>
                syscall_sysname(
                    frame.rdi,
                    frame.rsi
                        as usize,
                ),

            SYS_EXEC =>
	    syscall_exec(
	        frame,
		frame.rdi,
        	frame.rsi as usize,
	        frame.rdx as usize,
	        frame.r10,
	    ),
            SYS_OPEN =>
                syscall_open(
                    frame.rdi,
                    frame.rsi
                        as usize,
                    frame.rdx,
                ),

            SYS_CLOSE => {
                if process::close(
                    scheduler
                        ::current_pid(),
                    frame.rdi
                        as usize,
                ) {
                    0
                } else {
                    err(EBADF)
                }
            }

            SYS_MKDIR =>
                syscall_mkdir(
                    frame.rdi,
                    frame.rsi
                        as usize,
                ),

            SYS_RMDIR =>
                syscall_rmdir(
                    frame.rdi,
                    frame.rsi
                        as usize,
                ),

            SYS_TEST_FILE =>
                syscall_test_file(
                    frame.rdi,
                    frame.rsi
                        as usize,
                ),

            SYS_EXIT => {
                let pid =
                    scheduler
                        ::current_pid();

		return scheduler::exit_from_interrupt(
		    frame_rsp,
		    frame.rdi as i32,
		);
            }

	    SYS_WAIT =>
	    syscall_wait(
        	frame.rdi,
	        frame.rsi,
	    ),

	SYS_SLEEP => {
	    frame.rax = 0;

	    return scheduler::sleep_from_interrupt(
        	frame_rsp,
	        frame.rdi,
	    );
	}
            _ =>
                err(EINVAL),
        };

    frame_rsp
}
