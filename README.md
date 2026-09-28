# KaluOS

A small x86_64 operating system written in Rust, with its own libc and userland.

Built on top of the [limine-rust-template](https://github.com/limine-bootloader/limine-rust-template).

## Status

Early development (v0.02). Boots on x86_64, mounts ext2 read-write, loads and runs a userland shell

Boot log:

```
KaluOS v0.02 booting...!
pmm: total=1048576 free=523186
vmm: cr3=0x7ff89000
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
init: inode=17 size=19208
init: entry=0x400000 cr3=0x1c4000 stack=0x7fffffffefd8
task: spawned id=1 entry=0xffffffff800256f0 stack=0xffff8000001e8000 0
init: pid=2
KaluOS init
init: KaluSH pid=3
KaluSH
kalu@localhost# ls
lost+found
bin
etc
home
tmp
dev
kalu@localhost# echo gato
gato
kalu@localhost# help
KaluBox
ls [DIR]
cat FILE
echo TEXT...
mkdir DIR
rm FILE
rmdir DIR
touch FILE
ticks
sysname
sleep MS
clear
help
kalu@localhost#
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
· v0.03: more shell utils
· ...

Credits

Started from the limine-rust-template. Bootloader by Limine.

## License

MIT See LICENSE.
