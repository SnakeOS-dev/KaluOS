extern crate alloc;

use alloc::boxed::Box;
use spin::Mutex;

pub const SECTOR_SIZE: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    NoDevice,
    Io,
    OutOfBounds,
    Invalid,
}

pub trait BlockDevice: Send {
    fn sector_count(&self) -> u64;
    fn read_sector(
        &mut self,
        lba: u64,
        buffer: &mut [u8; SECTOR_SIZE],
    ) -> Result<(), BlockError>;
    fn write_sector(
        &mut self,
        lba: u64,
        buffer: &[u8; SECTOR_SIZE],
    ) -> Result<(), BlockError>;
    fn flush(&mut self) -> Result<(), BlockError>;
}

static DEVICE: Mutex<Option<Box<dyn BlockDevice>>> =
    Mutex::new(None);

pub fn register(device: Box<dyn BlockDevice>) {
    *DEVICE.lock() = Some(device);
}

pub fn sector_count() -> Result<u64, BlockError> {
    let mut device = DEVICE.lock();

    let dev = device
        .as_mut()
        .ok_or(BlockError::NoDevice)?;

    Ok(dev.sector_count())
}

pub fn read_sector(
    lba: u64,
    buffer: &mut [u8; SECTOR_SIZE],
) -> Result<(), BlockError> {
    let mut device = DEVICE.lock();

    let dev = device
        .as_mut()
        .ok_or(BlockError::NoDevice)?;

    if lba >= dev.sector_count() {
        return Err(BlockError::OutOfBounds);
    }

    dev.read_sector(lba, buffer)
}

pub fn write_sector(
    lba: u64,
    buffer: &[u8; SECTOR_SIZE],
) -> Result<(), BlockError> {
    let mut device = DEVICE.lock();

    let dev = device
        .as_mut()
        .ok_or(BlockError::NoDevice)?;

    if lba >= dev.sector_count() {
        return Err(BlockError::OutOfBounds);
    }

    dev.write_sector(lba, buffer)
}

pub fn flush() -> Result<(), BlockError> {
    let mut device = DEVICE.lock();

    let dev = device
        .as_mut()
        .ok_or(BlockError::NoDevice)?;

    dev.flush()
}

pub fn read_bytes(
    offset: u64,
    buffer: &mut [u8],
) -> Result<(), BlockError> {
    if buffer.is_empty() {
        return Ok(());
    }

    let total = sector_count()? * SECTOR_SIZE as u64;

    if offset
        .checked_add(buffer.len() as u64)
        .ok_or(BlockError::OutOfBounds)?
        > total
    {
        return Err(BlockError::OutOfBounds);
    }

    let mut remaining = buffer.len();
    let mut dst = 0usize;
    let mut pos = offset;
    let mut sector = [0u8; SECTOR_SIZE];

    while remaining > 0 {
        let lba = pos / SECTOR_SIZE as u64;
        let sector_offset =
            (pos % SECTOR_SIZE as u64) as usize;

        read_sector(lba, &mut sector)?;

        let count = remaining.min(
            SECTOR_SIZE - sector_offset,
        );

        buffer[dst..dst + count].copy_from_slice(
            &sector[
                sector_offset..
                sector_offset + count
            ],
        );

        remaining -= count;
        dst += count;
        pos += count as u64;
    }

    Ok(())
}

pub fn write_bytes(
    offset: u64,
    buffer: &[u8],
) -> Result<(), BlockError> {
    if buffer.is_empty() {
        return Ok(());
    }

    let total = sector_count()? * SECTOR_SIZE as u64;

    if offset
        .checked_add(buffer.len() as u64)
        .ok_or(BlockError::OutOfBounds)?
        > total
    {
        return Err(BlockError::OutOfBounds);
    }

    let mut remaining = buffer.len();
    let mut src = 0usize;
    let mut pos = offset;
    let mut sector = [0u8; SECTOR_SIZE];

    while remaining > 0 {
        let lba = pos / SECTOR_SIZE as u64;
        let sector_offset =
            (pos % SECTOR_SIZE as u64) as usize;

        let count = remaining.min(
            SECTOR_SIZE - sector_offset,
        );

        if sector_offset != 0
            || count != SECTOR_SIZE
        {
            read_sector(lba, &mut sector)?;
        }

        sector[
            sector_offset..
            sector_offset + count
        ]
            .copy_from_slice(
                &buffer[src..src + count],
            );

        write_sector(lba, &sector)?;

        remaining -= count;
        src += count;
        pos += count as u64;
    }

    Ok(())
}
