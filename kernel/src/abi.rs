pub const SYS_TEXT: u64 = 1;
pub const SYS_READ: u64 = 2;
pub const SYS_WRITE: u64 = 3;
pub const SYS_READDIR: u64 = 4;
pub const SYS_CHDIR: u64 = 5;
pub const SYS_REMOVE: u64 = 6;
pub const SYS_SPAWN: u64 = 7;
pub const SYS_GETC: u64 = 8;
pub const SYS_MAP: u64 = 9;
pub const SYS_UNMAP: u64 = 10;
pub const SYS_YIELD: u64 = 11;
pub const SYS_TICKS: u64 = 12;
pub const SYS_SYSNAME: u64 = 13;
pub const SYS_EXEC: u64 = 14;
pub const SYS_OPEN: u64 = 15;
pub const SYS_CLOSE: u64 = 16;
pub const SYS_MKDIR: u64 = 17;
pub const SYS_RMDIR: u64 = 18;
pub const SYS_TEST_FILE: u64 = 19;
pub const SYS_EXIT: u64 = 20;
pub const SYS_WAIT: u64 = 21;
pub const SYS_SLEEP: u64 = 22;

pub const O_READ: u64 = 1;
pub const O_WRITE: u64 = 2;
pub const O_CREATE: u64 = 4;
pub const O_TRUNC: u64 = 8;

pub const MAP_WRITE: u64 = 1;
pub const MAP_EXEC: u64 = 2;

#[repr(C)]
pub struct Dirent {
    pub inode: u32,
    pub kind: u8,
    pub name_len: u8,
    pub reserved: u16,
    pub name: [u8; 256],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct UserArg {
    pub ptr: u64,
    pub len: u64,
}
