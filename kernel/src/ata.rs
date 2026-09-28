use core::arch::asm;

use crate::block::{
    BlockDevice,
    BlockError,
    SECTOR_SIZE,
};

const DATA: u16 = 0;
const ERROR: u16 = 1;
const FEATURES: u16 = 1;
const SECTOR_COUNT: u16 = 2;
const LBA_LOW: u16 = 3;
const LBA_MID: u16 = 4;
const LBA_HIGH: u16 = 5;
const DRIVE: u16 = 6;
const STATUS: u16 = 7;
const COMMAND: u16 = 7;

const STATUS_ERR: u8 = 1;
const STATUS_DRQ: u8 = 1 << 3;
const STATUS_DF: u8 = 1 << 5;
const STATUS_BSY: u8 = 1 << 7;

pub struct AtaPio {
    io: u16,
    ctrl: u16,
    slave: bool,
    sectors: u64,
}

impl AtaPio {
    pub unsafe fn probe_primary_master()
        -> Result<Self, BlockError>
    {
        unsafe {
            Self::probe(0x1F0, 0x3F6, false)
        }
    }

    pub unsafe fn probe(
        io: u16,
        ctrl: u16,
        slave: bool,
    ) -> Result<Self, BlockError> {
        let mut ata = Self {
            io,
            ctrl,
            slave,
            sectors: 0,
        };

        unsafe {
            ata.select();
            outb(io + SECTOR_COUNT, 0);
            outb(io + LBA_LOW, 0);
            outb(io + LBA_MID, 0);
            outb(io + LBA_HIGH, 0);
            outb(io + COMMAND, 0xEC);
        }

        let status = unsafe {
            inb(io + STATUS)
        };

        if status == 0 {
            return Err(BlockError::NoDevice);
        }

        ata.wait_not_busy()?;

        let mid = unsafe {
            inb(io + LBA_MID)
        };

        let high = unsafe {
            inb(io + LBA_HIGH)
        };

        if mid != 0 || high != 0 {
            return Err(BlockError::Invalid);
        }

        ata.wait_drq()?;

        let mut identify = [0u16; 256];

        for word in &mut identify {
            *word = unsafe {
                inw(io + DATA)
            };
        }

        if identify[49] & (1 << 9) == 0 {
            return Err(BlockError::Invalid);
        }

        let lba28 =
            identify[60] as u64
            | ((identify[61] as u64) << 16);

        let lba48_supported =
            identify[83] & (1 << 10) != 0;

        let lba48 =
            identify[100] as u64
            | ((identify[101] as u64) << 16)
            | ((identify[102] as u64) << 32)
            | ((identify[103] as u64) << 48);

        ata.sectors =
            if lba48_supported && lba48 != 0 {
                lba48
            } else {
                lba28
            };

        if ata.sectors == 0 {
            return Err(BlockError::Invalid);
        }

        Ok(ata)
    }

    unsafe fn select(&self) {
        unsafe {
            outb(
                self.io + DRIVE,
                0xE0
                    | if self.slave {
                        0x10
                    } else {
                        0
                    },
            );

            self.delay400();
        }
    }

    unsafe fn delay400(&self) {
        unsafe {
            inb(self.ctrl);
            inb(self.ctrl);
            inb(self.ctrl);
            inb(self.ctrl);
        }
    }

    fn wait_not_busy(
        &self,
    ) -> Result<u8, BlockError> {
        for _ in 0..1_000_000 {
            let status = unsafe {
                inb(self.io + STATUS)
            };

            if status & STATUS_BSY == 0 {
                if status
                    & (STATUS_ERR | STATUS_DF)
                    != 0
                {
                    return Err(BlockError::Io);
                }

                return Ok(status);
            }
        }

        Err(BlockError::Io)
    }

    fn wait_drq(
        &self,
    ) -> Result<(), BlockError> {
        for _ in 0..1_000_000 {
            let status = unsafe {
                inb(self.io + STATUS)
            };

            if status
                & (STATUS_ERR | STATUS_DF)
                != 0
            {
                return Err(BlockError::Io);
            }

            if status & STATUS_BSY == 0
                && status & STATUS_DRQ != 0
            {
                return Ok(());
            }
        }

        Err(BlockError::Io)
    }

    fn setup_lba28(
        &self,
        lba: u64,
        command: u8,
    ) -> Result<(), BlockError> {
        if lba > 0x0FFF_FFFF {
            return Err(BlockError::OutOfBounds);
        }

        unsafe {
            self.select();

            outb(self.io + FEATURES, 0);
            outb(self.io + SECTOR_COUNT, 1);

            outb(
                self.io + LBA_LOW,
                lba as u8,
            );

            outb(
                self.io + LBA_MID,
                (lba >> 8) as u8,
            );

            outb(
                self.io + LBA_HIGH,
                (lba >> 16) as u8,
            );

            outb(
                self.io + DRIVE,
                0xE0
                    | if self.slave {
                        0x10
                    } else {
                        0
                    }
                    | ((lba >> 24) as u8 & 0x0F),
            );

            outb(
                self.io + COMMAND,
                command,
            );
        }

        Ok(())
    }
}

impl BlockDevice for AtaPio {
    fn sector_count(&self) -> u64 {
        self.sectors
    }

    fn read_sector(
        &mut self,
        lba: u64,
        buffer: &mut [u8; SECTOR_SIZE],
    ) -> Result<(), BlockError> {
        if lba >= self.sectors {
            return Err(BlockError::OutOfBounds);
        }

        self.setup_lba28(lba, 0x20)?;
        self.wait_drq()?;

        for i in 0..256 {
            let value = unsafe {
                inw(self.io + DATA)
            };

            buffer[i * 2] =
                value as u8;

            buffer[i * 2 + 1] =
                (value >> 8) as u8;
        }

        self.wait_not_busy()?;

        Ok(())
    }

    fn write_sector(
        &mut self,
        lba: u64,
        buffer: &[u8; SECTOR_SIZE],
    ) -> Result<(), BlockError> {
        if lba >= self.sectors {
            return Err(BlockError::OutOfBounds);
        }

        self.setup_lba28(lba, 0x30)?;
        self.wait_drq()?;

        for i in 0..256 {
            let value =
                buffer[i * 2] as u16
                | ((buffer[i * 2 + 1]
                    as u16)
                    << 8);

            unsafe {
                outw(self.io + DATA, value);
            }
        }

        self.wait_not_busy()?;

        Ok(())
    }

    fn flush(
        &mut self,
    ) -> Result<(), BlockError> {
        unsafe {
            self.select();

            outb(
                self.io + COMMAND,
                0xE7,
            );
        }

        self.wait_not_busy()?;

        Ok(())
    }
}

unsafe fn outb(
    port: u16,
    value: u8,
) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(
                nomem,
                nostack,
                preserves_flags
            )
        );
    }
}

unsafe fn inb(
    port: u16,
) -> u8 {
    let value: u8;

    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(
                nomem,
                nostack,
                preserves_flags
            )
        );
    }

    value
}

unsafe fn inw(
    port: u16,
) -> u16 {
    let value: u16;

    unsafe {
        asm!(
            "in ax, dx",
            in("dx") port,
            out("ax") value,
            options(
                nomem,
                nostack,
                preserves_flags
            )
        );
    }

    value
}

unsafe fn outw(
    port: u16,
    value: u16,
) {
    unsafe {
        asm!(
            "out dx, ax",
            in("dx") port,
            in("ax") value,
            options(
                nomem,
                nostack,
                preserves_flags
            )
        );
    }
}
