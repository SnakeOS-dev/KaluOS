extern crate alloc;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use spin::Mutex;

const MAX_FDS: usize = 32;

#[derive(Clone)]
pub struct FileDescriptor {
    pub path: String,
    pub offset: u64,
    pub flags: u64,
    pub directory_index: usize,
}

pub struct Process {
    pub pid: u64,
    pub cwd: String,
    pub fds: Vec<Option<FileDescriptor>>,
    pub next_map: u64,
}

static PROCESSES: Mutex<Vec<Process>> =
    Mutex::new(Vec::new());

pub fn register(pid: u64) {
    let mut processes = PROCESSES.lock();

    if processes
        .iter()
        .any(|p| p.pid == pid)
    {
        return;
    }

    let mut fds = Vec::new();

    for _ in 0..MAX_FDS {
        fds.push(None);
    }

    processes.push(Process {
        pid,
        cwd: "/".to_string(),
        fds,
        next_map: 0x0000_4000_0000_0000,
    });
}

fn index(pid: u64) -> Option<usize> {
    PROCESSES
        .lock()
        .iter()
        .position(|p| p.pid == pid)
}

pub fn cwd(pid: u64) -> Option<String> {
    let processes = PROCESSES.lock();

    processes
        .iter()
        .find(|p| p.pid == pid)
        .map(|p| p.cwd.clone())
}

pub fn set_cwd(
    pid: u64,
    path: String,
) -> bool {
    let mut processes = PROCESSES.lock();

    let Some(process) =
        processes
            .iter_mut()
            .find(|p| p.pid == pid)
    else {
        return false;
    };

    process.cwd = path;

    true
}

pub fn open(
    pid: u64,
    path: String,
    flags: u64,
) -> Option<u64> {
    let mut processes = PROCESSES.lock();

    let process =
        processes
            .iter_mut()
            .find(|p| p.pid == pid)?;

    for fd in 3..MAX_FDS {
        if process.fds[fd].is_none() {
            process.fds[fd] =
                Some(FileDescriptor {
                    path,
                    offset: 0,
                    flags,
                    directory_index: 0,
                });

            return Some(fd as u64);
        }
    }

    None
}

pub fn close(
    pid: u64,
    fd: usize,
) -> bool {
    if fd < 3 || fd >= MAX_FDS {
        return false;
    }

    let mut processes = PROCESSES.lock();

    let Some(process) =
        processes
            .iter_mut()
            .find(|p| p.pid == pid)
    else {
        return false;
    };

    if process.fds[fd].is_none() {
        return false;
    }

    process.fds[fd] = None;

    true
}

pub fn fd(
    pid: u64,
    fd: usize,
) -> Option<FileDescriptor> {
    let processes = PROCESSES.lock();

    let process =
        processes
            .iter()
            .find(|p| p.pid == pid)?;

    process.fds
        .get(fd)?
        .clone()
}

pub fn update_fd(
    pid: u64,
    fd: usize,
    descriptor: FileDescriptor,
) -> bool {
    let mut processes = PROCESSES.lock();

    let Some(process) =
        processes
            .iter_mut()
            .find(|p| p.pid == pid)
    else {
        return false;
    };

    if fd >= process.fds.len() {
        return false;
    }

    process.fds[fd] =
        Some(descriptor);

    true
}

pub fn alloc_map(
    pid: u64,
    length: u64,
) -> Option<u64> {
    let mut processes = PROCESSES.lock();

    let process =
        processes
            .iter_mut()
            .find(|p| p.pid == pid)?;

    let pages =
        (length + 4095) & !4095;

    let address =
        process.next_map;

    process.next_map =
        process.next_map
            .checked_add(pages)?;

    Some(address)
}

pub fn remove(pid: u64) {
    let mut processes =
        PROCESSES.lock();

    if let Some(i) =
        processes
            .iter()
            .position(|p| p.pid == pid)
    {
        processes.remove(i);
    }
}
