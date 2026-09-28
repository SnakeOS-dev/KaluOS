# KaluOS

A small x86_64 operating system written in Rust, with its own libc and userland.

Built on top of the [limine-rust-template](https://github.com/limine-bootloader/limine-rust-template).

## Status

Early development (v0.01). Boots on x86_64, mounts ext2 read-write, loads and runs a userland `init` written in C.

Boot log:

```

KaluOS v0.01 booting...!
pmm: total=1048576 free=523227
vmm: cr3=0x7ff93000
heap: 1024 KiB
ata: 131072 sectors 64 MiB
ext2: mounted rw
root: ino=2 name=.
root: ino=2 name=..
root: ino=11 name=lost+found
root: ino=12 name=bin
root: ino=13 name=etc
root: ino=14 name=home
root: ino=15 name=tmp
root: ino=16 name=dev
init: inode=17 size=19080
init: entry=0x400000 cr3=0x1c4000 stack=0x7fffffffeff0
task: spawned id=1 entry=0xffffffff80022578 stack=0xffff8000001e8000 rsp=0x0
init: pid=2
KaluOS userland initialized
ticks: 6
14 .
2 ..
18 userland.txt
malloc: 0x400000001018
init terminated

```

## What works

- Physical memory manager (PMM)
- Virtual memory manager (VMM) with paging
- Kernel heap
- ATA driver
- ext2 filesystem (read-write)
- VFS layer
- ELF loader
- GDT, IDT, PIC, PIT
- Keyboard and serial input
- Scheduler and process abstraction
- Syscalls (kernel <-> userland ABI)
- Userland libc (own headers, `crt0.S`, malloc, stdio, string, unistd, errno)
- Userland `init` program in C

## Layout

```

kernel/       Rust kernel (x86_64)
userland/
libc/       libc implementation (C + asm)
libc/programs/   userland programs
limine.conf   bootloader config

```

## Building

Requirements:

- Rust nightly toolchain (see `rust-toolchain.toml`)
- `nasm`
- `make`
- `qemu-system-x86_64`
- `xorriso`

```sh
make
make run
```

## Roadmap

· v0.02: shell, more syscalls
· v0.03: more shell utils
· ...

Credits

Started from the limine-rust-template. Bootloader by Limine.

## License

MIT See LICENSE.
