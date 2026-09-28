extern crate alloc;

use alloc::{
    string::String,
    vec,
    vec::Vec,
};

use crate::block;

pub const EXT2_ROOT_INO: u32 = 2;

const EXT2_MAGIC: u16 = 0xEF53;

const MODE_TYPE_MASK: u16 = 0xF000;
const MODE_REG: u16 = 0x8000;
const MODE_DIR: u16 = 0x4000;

const FT_UNKNOWN: u8 = 0;
const FT_REG_FILE: u8 = 1;
const FT_DIR: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ext2Error {
    Io,
    Invalid,
    NotFound,
    Exists,
    NotDir,
    IsDir,
    NotEmpty,
    NoSpace,
    NameTooLong,
    Unsupported,
    Corrupt,
}

impl From<block::BlockError> for Ext2Error {
    fn from(
        _: block::BlockError,
    ) -> Self {
        Self::Io
    }
}

#[derive(Clone)]
pub struct Superblock {
    pub inodes_count: u32,
    pub blocks_count: u32,
    pub free_blocks_count: u32,
    pub free_inodes_count: u32,
    pub first_data_block: u32,
    pub log_block_size: u32,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub first_ino: u32,
    pub inode_size: u16,
    pub feature_compat: u32,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
}

#[derive(Clone, Copy)]
pub struct GroupDesc {
    pub block_bitmap: u32,
    pub inode_bitmap: u32,
    pub inode_table: u32,
    pub free_blocks_count: u16,
    pub free_inodes_count: u16,
    pub used_dirs_count: u16,
}

#[derive(Clone)]
pub struct Inode {
    pub mode: u16,
    pub uid: u16,
    pub size: u64,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
    pub dtime: u32,
    pub gid: u16,
    pub links_count: u16,
    pub blocks: u32,
    pub flags: u32,
    pub block: [u32; 15],
}

impl Inode {
    pub fn is_dir(&self) -> bool {
        self.mode & MODE_TYPE_MASK == MODE_DIR
    }

    pub fn is_file(&self) -> bool {
        self.mode & MODE_TYPE_MASK == MODE_REG
    }
}

#[derive(Clone)]
pub struct DirEntry {
    pub inode: u32,
    pub file_type: u8,
    pub name: String,
}

pub struct Ext2 {
    pub superblock: Superblock,
    groups: Vec<GroupDesc>,
    block_size: u32,
    groups_count: u32,
}

impl Ext2 {
    pub fn mount() -> Result<Self, Ext2Error> {
        let mut raw = [0u8; 1024];

        block::read_bytes(
            1024,
            &mut raw,
        )?;

        if le16(&raw, 56) != EXT2_MAGIC {
            return Err(Ext2Error::Invalid);
        }

        let sb = Superblock {
            inodes_count: le32(&raw, 0),
            blocks_count: le32(&raw, 4),
            free_blocks_count: le32(&raw, 12),
            free_inodes_count: le32(&raw, 16),
            first_data_block: le32(&raw, 20),
            log_block_size: le32(&raw, 24),
            blocks_per_group: le32(&raw, 32),
            inodes_per_group: le32(&raw, 40),
            first_ino: {
                let rev = le32(&raw, 76);

                if rev == 0 {
                    11
                } else {
                    le32(&raw, 84)
                }
            },
            inode_size: {
                let rev = le32(&raw, 76);

                if rev == 0 {
                    128
                } else {
                    le16(&raw, 88)
                }
            },
            feature_compat: le32(&raw, 92),
            feature_incompat: le32(&raw, 96),
            feature_ro_compat: le32(&raw, 100),
        };

        if sb.blocks_per_group == 0
            || sb.inodes_per_group == 0
            || sb.inode_size < 128
        {
            return Err(Ext2Error::Corrupt);
        }

        if sb.feature_incompat
            & !(0x0002 | 0x0010)
            != 0
        {
            return Err(Ext2Error::Unsupported);
        }

        let block_size =
            1024u32
                .checked_shl(
                    sb.log_block_size,
                )
                .ok_or(
                    Ext2Error::Unsupported
                )?;

        if block_size < 1024
            || block_size > 4096
        {
            return Err(Ext2Error::Unsupported);
        }

        let groups_count =
            div_ceil_u32(
                sb.blocks_count
                    - sb.first_data_block,
                sb.blocks_per_group,
            );

        let gdt_block =
            if block_size == 1024 {
                2
            } else {
                1
            };

        let mut groups =
            Vec::with_capacity(
                groups_count as usize,
            );

        for group in 0..groups_count {
            let offset =
                gdt_block as u64
                    * block_size as u64
                + group as u64 * 32;

            let mut buf = [0u8; 32];

            block::read_bytes(
                offset,
                &mut buf,
            )?;

            groups.push(GroupDesc {
                block_bitmap: le32(
                    &buf,
                    0,
                ),
                inode_bitmap: le32(
                    &buf,
                    4,
                ),
                inode_table: le32(
                    &buf,
                    8,
                ),
                free_blocks_count: le16(
                    &buf,
                    12,
                ),
                free_inodes_count: le16(
                    &buf,
                    14,
                ),
                used_dirs_count: le16(
                    &buf,
                    16,
                ),
            });
        }

        let fs = Self {
            superblock: sb,
            groups,
            block_size,
            groups_count,
        };

        let root =
            fs.read_inode(
                EXT2_ROOT_INO,
            )?;

        if !root.is_dir() {
            return Err(Ext2Error::Corrupt);
        }

        Ok(fs)
    }

    pub fn block_size(&self) -> u32 {
        self.block_size
    }

    fn block_offset(
        &self,
        block_no: u32,
    ) -> u64 {
        block_no as u64
            * self.block_size as u64
    }

    fn read_block(
        &self,
        block_no: u32,
        buffer: &mut [u8],
    ) -> Result<(), Ext2Error> {
        if buffer.len()
            != self.block_size as usize
        {
            return Err(Ext2Error::Invalid);
        }

        block::read_bytes(
            self.block_offset(
                block_no,
            ),
            buffer,
        )?;

        Ok(())
    }

    fn write_block(
        &self,
        block_no: u32,
        buffer: &[u8],
    ) -> Result<(), Ext2Error> {
        if buffer.len()
            != self.block_size as usize
        {
            return Err(Ext2Error::Invalid);
        }

        block::write_bytes(
            self.block_offset(
                block_no,
            ),
            buffer,
        )?;

        Ok(())
    }

    fn zero_block(
        &self,
        block_no: u32,
    ) -> Result<(), Ext2Error> {
        let buffer =
            vec![
                0u8;
                self.block_size
                    as usize
            ];

        self.write_block(
            block_no,
            &buffer,
        )
    }

    fn sync_superblock(
        &self,
    ) -> Result<(), Ext2Error> {
        let mut raw =
            [0u8; 1024];

        block::read_bytes(
            1024,
            &mut raw,
        )?;

        put32(
            &mut raw,
            12,
            self.superblock
                .free_blocks_count,
        );

        put32(
            &mut raw,
            16,
            self.superblock
                .free_inodes_count,
        );

        block::write_bytes(
            1024,
            &raw,
        )?;

        Ok(())
    }

    fn sync_group(
        &self,
        group: u32,
    ) -> Result<(), Ext2Error> {
        let gdt_block =
            if self.block_size == 1024 {
                2
            } else {
                1
            };

        let offset =
            gdt_block as u64
                * self.block_size as u64
            + group as u64 * 32;

        let mut buf = [0u8; 32];

        block::read_bytes(
            offset,
            &mut buf,
        )?;

        let gd =
            self.groups[
                group as usize
            ];

        put16(
            &mut buf,
            12,
            gd.free_blocks_count,
        );

        put16(
            &mut buf,
            14,
            gd.free_inodes_count,
        );

        put16(
            &mut buf,
            16,
            gd.used_dirs_count,
        );

        block::write_bytes(
            offset,
            &buf,
        )?;

        Ok(())
    }

    pub fn read_inode(
        &self,
        ino: u32,
    ) -> Result<Inode, Ext2Error> {
        if ino == 0
            || ino
                > self
                    .superblock
                    .inodes_count
        {
            return Err(
                Ext2Error::NotFound,
            );
        }

        let index = ino - 1;

        let group =
            index
                / self
                    .superblock
                    .inodes_per_group;

        let local =
            index
                % self
                    .superblock
                    .inodes_per_group;

        let gd =
            self.groups[
                group as usize
            ];

        let offset =
            self.block_offset(
                gd.inode_table,
            )
            + local as u64
                * self
                    .superblock
                    .inode_size
                    as u64;

        let mut raw =
            vec![
                0u8;
                self.superblock
                    .inode_size
                    as usize
            ];

        block::read_bytes(
            offset,
            &mut raw,
        )?;

        let mut pointers =
            [0u32; 15];

        for i in 0..15 {
            pointers[i] =
                le32(
                    &raw,
                    40 + i * 4,
                );
        }

        let size_low =
            le32(&raw, 4) as u64;

        let size_high =
            if le16(&raw, 0)
                & MODE_TYPE_MASK
                == MODE_REG
                && raw.len() >= 112
            {
                le32(
                    &raw,
                    108,
                ) as u64
            } else {
                0
            };

        Ok(Inode {
            mode: le16(&raw, 0),
            uid: le16(&raw, 2),
            size:
                size_low
                | (size_high << 32),
            atime: le32(&raw, 8),
            ctime: le32(&raw, 12),
            mtime: le32(&raw, 16),
            dtime: le32(&raw, 20),
            gid: le16(&raw, 24),
            links_count: le16(
                &raw,
                26,
            ),
            blocks: le32(&raw, 28),
            flags: le32(&raw, 32),
            block: pointers,
        })
    }

    pub fn write_inode(
        &self,
        ino: u32,
        inode: &Inode,
    ) -> Result<(), Ext2Error> {
        let index = ino - 1;

        let group =
            index
                / self
                    .superblock
                    .inodes_per_group;

        let local =
            index
                % self
                    .superblock
                    .inodes_per_group;

        let gd =
            self.groups[
                group as usize
            ];

        let offset =
            self.block_offset(
                gd.inode_table,
            )
            + local as u64
                * self
                    .superblock
                    .inode_size
                    as u64;

        let mut raw =
            vec![
                0u8;
                self.superblock
                    .inode_size
                    as usize
            ];

        block::read_bytes(
            offset,
            &mut raw,
        )?;

        put16(
            &mut raw,
            0,
            inode.mode,
        );

        put16(
            &mut raw,
            2,
            inode.uid,
        );

        put32(
            &mut raw,
            4,
            inode.size as u32,
        );

        put32(
            &mut raw,
            8,
            inode.atime,
        );

        put32(
            &mut raw,
            12,
            inode.ctime,
        );

        put32(
            &mut raw,
            16,
            inode.mtime,
        );

        put32(
            &mut raw,
            20,
            inode.dtime,
        );

        put16(
            &mut raw,
            24,
            inode.gid,
        );

        put16(
            &mut raw,
            26,
            inode.links_count,
        );

        put32(
            &mut raw,
            28,
            inode.blocks,
        );

        put32(
            &mut raw,
            32,
            inode.flags,
        );

        for i in 0..15 {
            put32(
                &mut raw,
                40 + i * 4,
                inode.block[i],
            );
        }

        if inode.is_file()
            && raw.len() >= 112
        {
            put32(
                &mut raw,
                108,
                (inode.size >> 32)
                    as u32,
            );
        }

        block::write_bytes(
            offset,
            &raw,
        )?;

        Ok(())
    }

    fn alloc_block(
        &mut self,
    ) -> Result<u32, Ext2Error> {
        for group in 0..self.groups_count {
            let gd =
                self.groups[
                    group as usize
                ];

            if gd.free_blocks_count == 0 {
                continue;
            }

            let mut bitmap =
                vec![
                    0u8;
                    self.block_size
                        as usize
                ];

            self.read_block(
                gd.block_bitmap,
                &mut bitmap,
            )?;

            let group_start =
                self
                    .superblock
                    .first_data_block
                + group
                    * self
                        .superblock
                        .blocks_per_group;

            let max =
                self
                    .superblock
                    .blocks_per_group
                    .min(
                        self
                            .superblock
                            .blocks_count
                            .saturating_sub(
                                group_start,
                            ),
                    );

            for bit in 0..max {
                if !bitmap_test(
                    &bitmap,
                    bit as usize,
                ) {
                    bitmap_set(
                        &mut bitmap,
                        bit as usize,
                    );

                    self.write_block(
                        gd.block_bitmap,
                        &bitmap,
                    )?;

                    self.groups[
                        group as usize
                    ]
                        .free_blocks_count -= 1;

                    self.superblock
                        .free_blocks_count -= 1;

                    self.sync_group(group)?;
                    self.sync_superblock()?;

                    let block_no =
                        group_start + bit;

                    self.zero_block(
                        block_no,
                    )?;

                    return Ok(block_no);
                }
            }
        }

        Err(Ext2Error::NoSpace)
    }

    fn free_block(
        &mut self,
        block_no: u32,
    ) -> Result<(), Ext2Error> {
        if block_no
            < self
                .superblock
                .first_data_block
            || block_no
                >= self
                    .superblock
                    .blocks_count
        {
            return Err(Ext2Error::Invalid);
        }

        let rel =
            block_no
                - self
                    .superblock
                    .first_data_block;

        let group =
            rel
                / self
                    .superblock
                    .blocks_per_group;

        let bit =
            rel
                % self
                    .superblock
                    .blocks_per_group;

        let gd =
            self.groups[
                group as usize
            ];

        let mut bitmap =
            vec![
                0u8;
                self.block_size
                    as usize
            ];

        self.read_block(
            gd.block_bitmap,
            &mut bitmap,
        )?;

        if !bitmap_test(
            &bitmap,
            bit as usize,
        ) {
            return Err(Ext2Error::Corrupt);
        }

        bitmap_clear(
            &mut bitmap,
            bit as usize,
        );

        self.write_block(
            gd.block_bitmap,
            &bitmap,
        )?;

        self.groups[
            group as usize
        ]
            .free_blocks_count += 1;

        self.superblock
            .free_blocks_count += 1;

        self.sync_group(group)?;
        self.sync_superblock()?;

        Ok(())
    }

    fn alloc_inode(
        &mut self,
        is_dir: bool,
    ) -> Result<u32, Ext2Error> {
        for group in 0..self.groups_count {
            let gd =
                self.groups[
                    group as usize
                ];

            if gd.free_inodes_count == 0 {
                continue;
            }

            let mut bitmap =
                vec![
                    0u8;
                    self.block_size
                        as usize
                ];

            self.read_block(
                gd.inode_bitmap,
                &mut bitmap,
            )?;

            let max =
                self
                    .superblock
                    .inodes_per_group
                    .min(
                        self
                            .superblock
                            .inodes_count
                            .saturating_sub(
                                group
                                    * self
                                        .superblock
                                        .inodes_per_group,
                            ),
                    );

            for bit in 0..max {
                let ino =
                    group
                        * self
                            .superblock
                            .inodes_per_group
                    + bit
                    + 1;

                if ino
                    < self
                        .superblock
                        .first_ino
                    && ino != EXT2_ROOT_INO
                {
                    continue;
                }

                if !bitmap_test(
                    &bitmap,
                    bit as usize,
                ) {
                    bitmap_set(
                        &mut bitmap,
                        bit as usize,
                    );

                    self.write_block(
                        gd.inode_bitmap,
                        &bitmap,
                    )?;

                    self.groups[
                        group as usize
                    ]
                        .free_inodes_count -= 1;

                    self.superblock
                        .free_inodes_count -= 1;

                    if is_dir {
                        self.groups[
                            group as usize
                        ]
                            .used_dirs_count += 1;
                    }

                    self.sync_group(group)?;
                    self.sync_superblock()?;

                    return Ok(ino);
                }
            }
        }

        Err(Ext2Error::NoSpace)
    }

    fn free_inode(
        &mut self,
        ino: u32,
        was_dir: bool,
    ) -> Result<(), Ext2Error> {
        let index = ino - 1;

        let group =
            index
                / self
                    .superblock
                    .inodes_per_group;

        let bit =
            index
                % self
                    .superblock
                    .inodes_per_group;

        let gd =
            self.groups[
                group as usize
            ];

        let mut bitmap =
            vec![
                0u8;
                self.block_size
                    as usize
            ];

        self.read_block(
            gd.inode_bitmap,
            &mut bitmap,
        )?;

        bitmap_clear(
            &mut bitmap,
            bit as usize,
        );

        self.write_block(
            gd.inode_bitmap,
            &bitmap,
        )?;

        self.groups[
            group as usize
        ]
            .free_inodes_count += 1;

        self.superblock
            .free_inodes_count += 1;

        if was_dir {
            self.groups[
                group as usize
            ]
                .used_dirs_count -= 1;
        }

        self.sync_group(group)?;
        self.sync_superblock()?;

        Ok(())
    }

    fn alloc_inode_block(
        &mut self,
        inode: &mut Inode,
    ) -> Result<u32, Ext2Error> {
        let block =
            self.alloc_block()?;

        inode.blocks +=
            self.block_size / 512;

        Ok(block)
    }

    fn pointer_read(
        &self,
        block_no: u32,
        index: usize,
    ) -> Result<u32, Ext2Error> {
        let ppb =
            self.block_size as usize / 4;

        if index >= ppb {
            return Err(Ext2Error::Invalid);
        }

        let mut buffer =
            vec![
                0u8;
                self.block_size
                    as usize
            ];

        self.read_block(
            block_no,
            &mut buffer,
        )?;

        Ok(le32(
            &buffer,
            index * 4,
        ))
    }

    fn pointer_write(
        &self,
        block_no: u32,
        index: usize,
        value: u32,
    ) -> Result<(), Ext2Error> {
        let mut buffer =
            vec![
                0u8;
                self.block_size
                    as usize
            ];

        self.read_block(
            block_no,
            &mut buffer,
        )?;

        put32(
            &mut buffer,
            index * 4,
            value,
        );

        self.write_block(
            block_no,
            &buffer,
        )
    }

    fn map_indirect(
        &mut self,
        inode: &mut Inode,
        root_slot: usize,
        indexes: &[usize],
        allocate: bool,
    ) -> Result<u32, Ext2Error> {
        let mut current =
            inode.block[root_slot];

        if current == 0 {
            if !allocate {
                return Ok(0);
            }

            current =
                self.alloc_inode_block(
                    inode,
                )?;

            inode.block[root_slot] =
                current;
        }

        for (
            level,
            &index,
        ) in indexes.iter().enumerate()
        {
            let mut next =
                self.pointer_read(
                    current,
                    index,
                )?;

            let last =
                level + 1
                    == indexes.len();

            if next == 0 && allocate {
                next =
                    self.alloc_inode_block(
                        inode,
                    )?;

                self.pointer_write(
                    current,
                    index,
                    next,
                )?;
            }

            if next == 0 {
                return Ok(0);
            }

            if last {
                return Ok(next);
            }

            current = next;
        }

        Err(Ext2Error::Corrupt)
    }

    fn map_file_block(
        &mut self,
        inode: &mut Inode,
        logical: u32,
        allocate: bool,
    ) -> Result<u32, Ext2Error> {
        let ppb =
            self.block_size / 4;

        if logical < 12 {
            let slot =
                logical as usize;

            if inode.block[slot] == 0
                && allocate
            {
                inode.block[slot] =
                    self.alloc_inode_block(
                        inode,
                    )?;
            }

            return Ok(
                inode.block[slot],
            );
        }

        let mut n =
            logical - 12;

        if n < ppb {
            return self.map_indirect(
                inode,
                12,
                &[n as usize],
                allocate,
            );
        }

        n -= ppb;

        let double_capacity =
            ppb
                .checked_mul(ppb)
                .ok_or(
                    Ext2Error::Unsupported
                )?;

        if n < double_capacity {
            let a =
                n / ppb;

            let b =
                n % ppb;

            return self.map_indirect(
                inode,
                13,
                &[
                    a as usize,
                    b as usize,
                ],
                allocate,
            );
        }

        n -= double_capacity;

        let triple_capacity =
            double_capacity
                .checked_mul(ppb)
                .ok_or(
                    Ext2Error::Unsupported
                )?;

        if n >= triple_capacity {
            return Err(
                Ext2Error::Unsupported,
            );
        }

        let a =
            n / double_capacity;

        let rem =
            n % double_capacity;

        let b =
            rem / ppb;

        let c =
            rem % ppb;

        self.map_indirect(
            inode,
            14,
            &[
                a as usize,
                b as usize,
                c as usize,
            ],
            allocate,
        )
    }

    fn free_indirect_tree(
        &mut self,
        block_no: u32,
        depth: u8,
    ) -> Result<(), Ext2Error> {
        if block_no == 0 {
            return Ok(());
        }

        let ppb =
            self.block_size as usize
                / 4;

        let mut buffer =
            vec![
                0u8;
                self.block_size
                    as usize
            ];

        self.read_block(
            block_no,
            &mut buffer,
        )?;

        for i in 0..ppb {
            let child =
                le32(
                    &buffer,
                    i * 4,
                );

            if child == 0 {
                continue;
            }

            if depth == 1 {
                self.free_block(
                    child,
                )?;
            } else {
                self.free_indirect_tree(
                    child,
                    depth - 1,
                )?;
            }
        }

        self.free_block(block_no)?;

        Ok(())
    }

    pub fn truncate_zero(
        &mut self,
        ino: u32,
    ) -> Result<(), Ext2Error> {
        let mut inode =
            self.read_inode(ino)?;

        for i in 0..12 {
            if inode.block[i] != 0 {
                self.free_block(
                    inode.block[i],
                )?;

                inode.block[i] = 0;
            }
        }

        if inode.block[12] != 0 {
            self.free_indirect_tree(
                inode.block[12],
                1,
            )?;

            inode.block[12] = 0;
        }

        if inode.block[13] != 0 {
            self.free_indirect_tree(
                inode.block[13],
                2,
            )?;

            inode.block[13] = 0;
        }

        if inode.block[14] != 0 {
            self.free_indirect_tree(
                inode.block[14],
                3,
            )?;

            inode.block[14] = 0;
        }

        inode.size = 0;
        inode.blocks = 0;

        self.write_inode(
            ino,
            &inode,
        )
    }

    pub fn read_file(
        &mut self,
        ino: u32,
        offset: u64,
        buffer: &mut [u8],
    ) -> Result<usize, Ext2Error> {
        let mut inode =
            self.read_inode(ino)?;

        if inode.is_dir() {
            return Err(Ext2Error::IsDir);
        }

        if offset >= inode.size {
            return Ok(0);
        }

        let count =
            buffer.len().min(
                (inode.size - offset)
                    as usize,
            );

        let mut done = 0usize;

        while done < count {
            let pos =
                offset
                    + done as u64;

            let logical =
                (pos
                    / self.block_size
                        as u64)
                    as u32;

            let inside =
                (pos
                    % self.block_size
                        as u64)
                    as usize;

            let take =
                (count - done).min(
                    self.block_size
                        as usize
                        - inside,
                );

            let block_no =
                self.map_file_block(
                    &mut inode,
                    logical,
                    false,
                )?;

            if block_no == 0 {
                buffer[
                    done..
                    done + take
                ]
                    .fill(0);
            } else {
                let mut block_buf =
                    vec![
                        0u8;
                        self.block_size
                            as usize
                    ];

                self.read_block(
                    block_no,
                    &mut block_buf,
                )?;

                buffer[
                    done..
                    done + take
                ]
                    .copy_from_slice(
                        &block_buf[
                            inside..
                            inside + take
                        ],
                    );
            }

            done += take;
        }

        Ok(done)
    }

    pub fn write_file(
        &mut self,
        ino: u32,
        offset: u64,
        data: &[u8],
    ) -> Result<usize, Ext2Error> {
        let mut inode =
            self.read_inode(ino)?;

        if inode.is_dir() {
            return Err(Ext2Error::IsDir);
        }

        let end =
            offset
                .checked_add(
                    data.len() as u64,
                )
                .ok_or(
                    Ext2Error::Unsupported
                )?;

        let mut done = 0usize;

        while done < data.len() {
            let pos =
                offset
                    + done as u64;

            let logical =
                (pos
                    / self.block_size
                        as u64)
                    as u32;

            let inside =
                (pos
                    % self.block_size
                        as u64)
                    as usize;

            let take =
                (data.len() - done).min(
                    self.block_size
                        as usize
                        - inside,
                );

            let block_no =
                self.map_file_block(
                    &mut inode,
                    logical,
                    true,
                )?;

            let mut block_buf =
                vec![
                    0u8;
                    self.block_size
                        as usize
                ];

            if inside != 0
                || take
                    != self.block_size
                        as usize
            {
                self.read_block(
                    block_no,
                    &mut block_buf,
                )?;
            }

            block_buf[
                inside..
                inside + take
            ]
                .copy_from_slice(
                    &data[
                        done..
                        done + take
                    ],
                );

            self.write_block(
                block_no,
                &block_buf,
            )?;

            done += take;
        }

        if end > inode.size {
            inode.size = end;
        }

        self.write_inode(
            ino,
            &inode,
        )?;

        block::flush()?;

        Ok(done)
    }

    pub fn read_dir(
        &mut self,
        ino: u32,
    ) -> Result<Vec<DirEntry>, Ext2Error> {
        let mut inode =
            self.read_inode(ino)?;

        if !inode.is_dir() {
            return Err(Ext2Error::NotDir);
        }

        let blocks =
            div_ceil_u64(
                inode.size,
                self.block_size as u64,
            );

        let mut result =
            Vec::new();

        for logical in 0..blocks {
            let block_no =
                self.map_file_block(
                    &mut inode,
                    logical as u32,
                    false,
                )?;

            if block_no == 0 {
                continue;
            }

            let mut buffer =
                vec![
                    0u8;
                    self.block_size
                        as usize
                ];

            self.read_block(
                block_no,
                &mut buffer,
            )?;

            let mut pos = 0usize;

            while pos + 8
                <= buffer.len()
            {
                let entry_ino =
                    le32(
                        &buffer,
                        pos,
                    );

                let rec_len =
                    le16(
                        &buffer,
                        pos + 4,
                    ) as usize;

                let name_len =
                    buffer[pos + 6]
                        as usize;

                let file_type =
                    buffer[pos + 7];

                if rec_len < 8
                    || pos + rec_len
                        > buffer.len()
                    || name_len
                        > rec_len - 8
                {
                    return Err(
                        Ext2Error::Corrupt,
                    );
                }

                if entry_ino != 0 {
                    let name =
                        core::str::from_utf8(
                            &buffer[
                                pos + 8..
                                pos
                                    + 8
                                    + name_len
                            ],
                        )
                        .map_err(
                            |_| {
                                Ext2Error::Corrupt
                            },
                        )?
                        .into();

                    result.push(
                        DirEntry {
                            inode:
                                entry_ino,
                            file_type,
                            name,
                        },
                    );
                }

                pos += rec_len;
            }
        }

        Ok(result)
    }

    pub fn lookup(
        &mut self,
        parent: u32,
        name: &str,
    ) -> Result<u32, Ext2Error> {
        for entry in
            self.read_dir(parent)?
        {
            if entry.name == name {
                return Ok(entry.inode);
            }
        }

        Err(Ext2Error::NotFound)
    }

    fn add_dir_entry(
        &mut self,
        parent_ino: u32,
        child_ino: u32,
        name: &str,
        file_type: u8,
    ) -> Result<(), Ext2Error> {
        if name.is_empty()
            || name.len() > 255
        {
            return Err(
                Ext2Error::NameTooLong,
            );
        }

        if self.lookup(
            parent_ino,
            name,
        ).is_ok()
        {
            return Err(
                Ext2Error::Exists,
            );
        }

        let mut parent =
            self.read_inode(
                parent_ino,
            )?;

        if !parent.is_dir() {
            return Err(
                Ext2Error::NotDir,
            );
        }

        let needed =
            align4(
                8 + name.len(),
            );

        let blocks =
            div_ceil_u64(
                parent.size,
                self.block_size
                    as u64,
            );

        for logical in 0..blocks {
            let block_no =
                self.map_file_block(
                    &mut parent,
                    logical as u32,
                    false,
                )?;

            let mut buffer =
                vec![
                    0u8;
                    self.block_size
                        as usize
                ];

            self.read_block(
                block_no,
                &mut buffer,
            )?;

            let mut pos = 0usize;

            while pos + 8
                <= buffer.len()
            {
                let inode =
                    le32(
                        &buffer,
                        pos,
                    );

                let rec_len =
                    le16(
                        &buffer,
                        pos + 4,
                    ) as usize;

                if rec_len < 8
                    || pos + rec_len
                        > buffer.len()
                {
                    return Err(
                        Ext2Error::Corrupt,
                    );
                }

                if inode == 0
                    && rec_len >= needed
                {
                    write_dir_entry(
                        &mut buffer,
                        pos,
                        child_ino,
                        rec_len,
                        name,
                        file_type,
                    );

                    self.write_block(
                        block_no,
                        &buffer,
                    )?;

                    return Ok(());
                }

                let name_len =
                    buffer[pos + 6]
                        as usize;

                let actual =
                    align4(
                        8 + name_len,
                    );

                if rec_len >= actual
                    && rec_len - actual
                        >= needed
                {
                    put16(
                        &mut buffer,
                        pos + 4,
                        actual as u16,
                    );

                    write_dir_entry(
                        &mut buffer,
                        pos + actual,
                        child_ino,
                        rec_len - actual,
                        name,
                        file_type,
                    );

                    self.write_block(
                        block_no,
                        &buffer,
                    )?;

                    return Ok(());
                }

                pos += rec_len;
            }
        }

        let logical =
            blocks as u32;

        let block_no =
            self.map_file_block(
                &mut parent,
                logical,
                true,
            )?;

        let mut buffer =
            vec![
                0u8;
                self.block_size
                    as usize
            ];

        write_dir_entry(
            &mut buffer,
            0,
            child_ino,
            self.block_size as usize,
            name,
            file_type,
        );

        self.write_block(
            block_no,
            &buffer,
        )?;

        parent.size +=
            self.block_size as u64;

        self.write_inode(
            parent_ino,
            &parent,
        )?;

        Ok(())
    }

    fn remove_dir_entry(
        &mut self,
        parent_ino: u32,
        name: &str,
    ) -> Result<u32, Ext2Error> {
        let mut parent =
            self.read_inode(
                parent_ino,
            )?;

        if !parent.is_dir() {
            return Err(
                Ext2Error::NotDir,
            );
        }

        let blocks =
            div_ceil_u64(
                parent.size,
                self.block_size
                    as u64,
            );

        for logical in 0..blocks {
            let block_no =
                self.map_file_block(
                    &mut parent,
                    logical as u32,
                    false,
                )?;

            let mut buffer =
                vec![
                    0u8;
                    self.block_size
                        as usize
                ];

            self.read_block(
                block_no,
                &mut buffer,
            )?;

            let mut pos = 0usize;
            let mut previous:
                Option<usize> = None;

            while pos + 8
                <= buffer.len()
            {
                let inode =
                    le32(
                        &buffer,
                        pos,
                    );

                let rec_len =
                    le16(
                        &buffer,
                        pos + 4,
                    ) as usize;

                let name_len =
                    buffer[pos + 6]
                        as usize;

                if rec_len < 8
                    || pos + rec_len
                        > buffer.len()
                {
                    return Err(
                        Ext2Error::Corrupt,
                    );
                }

                if inode != 0 {
                    let entry_name =
                        core::str::from_utf8(
                            &buffer[
                                pos + 8..
                                pos
                                    + 8
                                    + name_len
                            ],
                        )
                        .map_err(
                            |_| {
                                Ext2Error::Corrupt
                            },
                        )?;

                    if entry_name == name {
                        if let Some(prev) =
                            previous
                        {
                            let prev_len =
                                le16(
                                    &buffer,
                                    prev + 4,
                                ) as usize;

                            put16(
                                &mut buffer,
                                prev + 4,
                                (
                                    prev_len
                                        + rec_len
                                ) as u16,
                            );
                        } else {
                            put32(
                                &mut buffer,
                                pos,
                                0,
                            );
                        }

                        self.write_block(
                            block_no,
                            &buffer,
                        )?;

                        return Ok(inode);
                    }

                    previous =
                        Some(pos);
                }

                pos += rec_len;
            }
        }

        Err(Ext2Error::NotFound)
    }

    pub fn create_file(
        &mut self,
        parent: u32,
        name: &str,
        permissions: u16,
    ) -> Result<u32, Ext2Error> {
        let ino =
            self.alloc_inode(false)?;

        let inode = Inode {
            mode:
                MODE_REG
                | (permissions & 0x0FFF),
            uid: 0,
            size: 0,
            atime: 0,
            ctime: 0,
            mtime: 0,
            dtime: 0,
            gid: 0,
            links_count: 1,
            blocks: 0,
            flags: 0,
            block: [0; 15],
        };

        self.write_inode(
            ino,
            &inode,
        )?;

        if let Err(error) =
            self.add_dir_entry(
                parent,
                ino,
                name,
                FT_REG_FILE,
            )
        {
            self.free_inode(
                ino,
                false,
            )?;

            return Err(error);
        }

        Ok(ino)
    }

    pub fn mkdir(
        &mut self,
        parent: u32,
        name: &str,
        permissions: u16,
    ) -> Result<u32, Ext2Error> {
        let ino =
            self.alloc_inode(true)?;

        let mut inode = Inode {
            mode:
                MODE_DIR
                | (permissions & 0x0FFF),
            uid: 0,
            size:
                self.block_size
                    as u64,
            atime: 0,
            ctime: 0,
            mtime: 0,
            dtime: 0,
            gid: 0,
            links_count: 2,
            blocks: 0,
            flags: 0,
            block: [0; 15],
        };

        let block_no =
            self.map_file_block(
                &mut inode,
                0,
                true,
            )?;

        let mut buffer =
            vec![
                0u8;
                self.block_size
                    as usize
            ];

        let dot_len =
            align4(9);

        write_dir_entry(
            &mut buffer,
            0,
            ino,
            dot_len,
            ".",
            FT_DIR,
        );

        write_dir_entry(
            &mut buffer,
            dot_len,
            parent,
            self.block_size as usize
                - dot_len,
            "..",
            FT_DIR,
        );

        self.write_block(
            block_no,
            &buffer,
        )?;

        self.write_inode(
            ino,
            &inode,
        )?;

        if let Err(error) =
            self.add_dir_entry(
                parent,
                ino,
                name,
                FT_DIR,
            )
        {
            self.truncate_zero(ino)?;

            self.free_inode(
                ino,
                true,
            )?;

            return Err(error);
        }

        let mut parent_inode =
            self.read_inode(parent)?;

        parent_inode.links_count =
            parent_inode
                .links_count
                .saturating_add(1);

        self.write_inode(
            parent,
            &parent_inode,
        )?;

        Ok(ino)
    }

    pub fn unlink(
        &mut self,
        parent: u32,
        name: &str,
    ) -> Result<(), Ext2Error> {
        if name == "."
            || name == ".."
        {
            return Err(Ext2Error::Invalid);
        }

        let ino =
            self.lookup(
                parent,
                name,
            )?;

        let inode =
            self.read_inode(ino)?;

        if inode.is_dir() {
            let entries =
                self.read_dir(ino)?;

            for entry in entries {
                if entry.name != "."
                    && entry.name != ".."
                {
                    return Err(
                        Ext2Error::NotEmpty,
                    );
                }
            }
        }

        self.remove_dir_entry(
            parent,
            name,
        )?;

        if inode.is_dir() {
            let mut parent_inode =
                self.read_inode(parent)?;

            parent_inode.links_count =
                parent_inode
                    .links_count
                    .saturating_sub(1);

            self.write_inode(
                parent,
                &parent_inode,
            )?;
        }

        self.truncate_zero(ino)?;

        self.free_inode(
            ino,
            inode.is_dir(),
        )?;

        Ok(())
    }

    pub fn sync(
        &self,
    ) -> Result<(), Ext2Error> {
        self.sync_superblock()?;

        for group in 0..self.groups_count {
            self.sync_group(group)?;
        }

        block::flush()?;

        Ok(())
    }
}

fn write_dir_entry(
    buffer: &mut [u8],
    offset: usize,
    inode: u32,
    rec_len: usize,
    name: &str,
    file_type: u8,
) {
    put32(
        buffer,
        offset,
        inode,
    );

    put16(
        buffer,
        offset + 4,
        rec_len as u16,
    );

    buffer[offset + 6] =
        name.len() as u8;

    buffer[offset + 7] =
        file_type;

    buffer[
        offset + 8..
        offset + 8 + name.len()
    ]
        .copy_from_slice(
            name.as_bytes(),
        );
}

fn bitmap_test(
    bitmap: &[u8],
    bit: usize,
) -> bool {
    bitmap[bit / 8]
        & (1 << (bit % 8))
        != 0
}

fn bitmap_set(
    bitmap: &mut [u8],
    bit: usize,
) {
    bitmap[bit / 8]
        |= 1 << (bit % 8);
}

fn bitmap_clear(
    bitmap: &mut [u8],
    bit: usize,
) {
    bitmap[bit / 8]
        &= !(1 << (bit % 8));
}

fn align4(
    value: usize,
) -> usize {
    (value + 3) & !3
}

fn div_ceil_u32(
    a: u32,
    b: u32,
) -> u32 {
    (a + b - 1) / b
}

fn div_ceil_u64(
    a: u64,
    b: u64,
) -> u64 {
    (a + b - 1) / b
}

fn le16(
    data: &[u8],
    offset: usize,
) -> u16 {
    u16::from_le_bytes([
        data[offset],
        data[offset + 1],
    ])
}

fn le32(
    data: &[u8],
    offset: usize,
) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn put16(
    data: &mut [u8],
    offset: usize,
    value: u16,
) {
    data[offset..offset + 2]
        .copy_from_slice(
            &value.to_le_bytes(),
        );
}

fn put32(
    data: &mut [u8],
    offset: usize,
    value: u32,
) {
    data[offset..offset + 4]
        .copy_from_slice(
            &value.to_le_bytes(),
        );
}
