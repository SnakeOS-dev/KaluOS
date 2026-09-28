extern crate alloc;

use alloc::{
    string::String,
    vec,
    vec::Vec,
};
use crate::{
    pmm,
    vfs,
    vmm,
};

const ELF_MAGIC: &[u8; 4] =
    b"\x7FELF";

const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;

const PF_X: u32 = 1;
const PF_W: u32 = 2;

pub const USER_STACK_TOP: u64 =
    0x0000_7FFF_FFFF_F000;

const USER_STACK_PAGES: usize = 16;

#[derive(Debug)]
pub enum ElfError {
    File,
    Invalid,
    Memory,
    Mapping,
}

pub struct LoadedElf {
    pub cr3: u64,
    pub entry: u64,
    pub stack: u64,
}

fn le16(
    b: &[u8],
    offset: usize,
) -> u16 {
    u16::from_le_bytes([
        b[offset],
        b[offset + 1],
    ])
}

fn le32(
    b: &[u8],
    offset: usize,
) -> u32 {
    u32::from_le_bytes([
        b[offset],
        b[offset + 1],
        b[offset + 2],
        b[offset + 3],
    ])
}

fn le64(
    b: &[u8],
    offset: usize,
) -> u64 {
    u64::from_le_bytes([
        b[offset],
        b[offset + 1],
        b[offset + 2],
        b[offset + 3],
        b[offset + 4],
        b[offset + 5],
        b[offset + 6],
        b[offset + 7],
    ])
}

pub fn load(
    path: &str,
    argv: &[String],
) -> Result<LoadedElf, ElfError> {
    let stat =
        vfs::stat(path)
            .map_err(|_| ElfError::File)?;

    if !stat.is_file
        || stat.size < 64
    {
        return Err(
            ElfError::Invalid,
        );
    }

    let mut image =
        vec![0u8; stat.size as usize];

    let read =
        vfs::read(
            path,
            0,
            &mut image,
        )
        .map_err(|_| ElfError::File)?;

    if read != image.len() {
        return Err(
            ElfError::File,
        );
    }

    if &image[0..4] != ELF_MAGIC
        || image[4] != 2
        || image[5] != 1
        || le16(&image, 16) != ET_EXEC
        || le16(&image, 18) != EM_X86_64
    {
        return Err(
            ElfError::Invalid,
        );
    }

    let entry =
        le64(&image, 24);

    let phoff =
        le64(&image, 32) as usize;

    let phentsize =
        le16(&image, 54) as usize;

    let phnum =
        le16(&image, 56) as usize;

    if phentsize < 56 {
        return Err(
            ElfError::Invalid,
        );
    }

    let cr3 =
        vmm::new_address_space()
            .map_err(
                |_| ElfError::Memory
            )?;

    for index in 0..phnum {
        let offset =
            phoff
                + index
                    * phentsize;

        if offset + 56 > image.len() {
            return Err(
                ElfError::Invalid,
            );
        }

        let p_type =
            le32(
                &image,
                offset,
            );

        if p_type != PT_LOAD {
            continue;
        }

        let flags =
            le32(
                &image,
                offset + 4,
            );

        let file_offset =
            le64(
                &image,
                offset + 8,
            );

        let virtual_address =
            le64(
                &image,
                offset + 16,
            );

        let file_size =
            le64(
                &image,
                offset + 32,
            );

        let memory_size =
            le64(
                &image,
                offset + 40,
            );

        if file_size > memory_size {
            return Err(
                ElfError::Invalid,
            );
        }

        if file_offset
            .checked_add(file_size)
            .ok_or(ElfError::Invalid)?
            > image.len() as u64
        {
            return Err(
                ElfError::Invalid,
            );
        }

        let start =
            virtual_address & !0xFFF;

        let end =
            (
                virtual_address
                    + memory_size
                    + 0xFFF
            ) & !0xFFF;

        let mut page = start;

        while page < end {
            if unsafe {
                vmm::translate(
                    cr3,
                    page,
                )
            }
            .is_none()
            {
                let phys =
                    pmm::alloc()
                        .ok_or(
                            ElfError::Memory
                        )?;

                vmm::zero_phys_page(
                    phys,
                );

                let mut map_flags =
                    vmm::USER;

                if flags & PF_W != 0 {
                    map_flags |=
                        vmm::WRITABLE;
                }

                if flags & PF_X == 0 {
                    map_flags |=
                        vmm::NO_EXECUTE;
                }

                unsafe {
                    vmm::map_page(
                        cr3,
                        page,
                        phys,
                        map_flags,
                    )
                }
                .map_err(
                    |_| ElfError::Mapping
                )?;
            }

            page += 4096;
        }

        for i in 0..file_size {
            let target =
                virtual_address + i;

            let phys =
                unsafe {
                    vmm::translate(
                        cr3,
                        target,
                    )
                }
                .ok_or(
                    ElfError::Mapping
                )?;

            let dst =
                vmm::phys_to_virt_addr(
                    phys,
                )
                as *mut u8;

            unsafe {
                dst.write(
                    image[
                        (
                            file_offset
                                + i
                        ) as usize
                    ],
                );
            }
        }
    }

    for page in 0..USER_STACK_PAGES {
        let virt =
            USER_STACK_TOP
                - (
                    page as u64
                    + 1
                ) * 4096;

        let phys =
            pmm::alloc()
                .ok_or(
                    ElfError::Memory
                )?;

        vmm::zero_phys_page(
            phys,
        );

        unsafe {
            vmm::map_page(
                cr3,
                virt,
                phys,
                vmm::USER
                    | vmm::WRITABLE
                    | vmm::NO_EXECUTE,
            )
        }
        .map_err(
            |_| ElfError::Mapping
        )?;
    }

    let stack =
    build_initial_stack(
        cr3,
        argv,
    )?;

    Ok(LoadedElf {
	cr3,
	entry,
	stack,
    })
}
fn write_user_bytes(
    cr3: u64,
    address: u64,
    data: &[u8],
) -> Result<(), ElfError> {
    for (i, byte) in data.iter().enumerate() {
        let virt =
            address + i as u64;

        let phys =
            unsafe {
                vmm::translate(
                    cr3,
                    virt,
                )
            }
            .ok_or(
                ElfError::Mapping
            )?;

        let ptr =
            vmm::phys_to_virt_addr(
                phys,
            ) as *mut u8;

        unsafe {
            ptr.write(*byte);
        }
    }

    Ok(())
}

fn write_user_u64(
    cr3: u64,
    address: u64,
    value: u64,
) -> Result<(), ElfError> {
    write_user_bytes(
        cr3,
        address,
        &value.to_le_bytes(),
    )
}

fn build_initial_stack(
    cr3: u64,
    argv: &[String],
) -> Result<u64, ElfError> {
    let mut rsp =
        USER_STACK_TOP;

    let mut pointers:
        Vec<u64> =
        Vec::with_capacity(
            argv.len(),
        );

    for arg in argv.iter().rev() {
        let bytes =
            arg.as_bytes();

        rsp = rsp
            .checked_sub(
                bytes.len() as u64 + 1,
            )
            .ok_or(
                ElfError::Memory
            )?;

        write_user_bytes(
            cr3,
            rsp,
            bytes,
        )?;

        write_user_bytes(
            cr3,
            rsp + bytes.len() as u64,
            &[0],
        )?;

        pointers.push(rsp);
    }

    pointers.reverse();

    rsp &= !0xF;

    rsp -= 8;

    write_user_u64(
        cr3,
        rsp,
        0,
    )?;

    for pointer in
        pointers.iter().rev()
    {
        rsp -= 8;

        write_user_u64(
            cr3,
            rsp,
            *pointer,
        )?;
    }

    rsp -= 8;

    write_user_u64(
        cr3,
        rsp,
        argv.len() as u64,
    )?;

    Ok(rsp)
}
