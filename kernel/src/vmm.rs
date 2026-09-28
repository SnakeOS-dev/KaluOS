use core::arch::asm;
use spin::Mutex;

use crate::pmm;

pub const PAGE_SIZE: u64 = 4096;
pub const HUGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;

pub const PRESENT: u64 = 1 << 0;
pub const WRITABLE: u64 = 1 << 1;
pub const USER: u64 = 1 << 2;
pub const WRITE_THROUGH: u64 = 1 << 3;
pub const NO_CACHE: u64 = 1 << 4;
pub const ACCESSED: u64 = 1 << 5;
pub const DIRTY: u64 = 1 << 6;
pub const HUGE: u64 = 1 << 7;
pub const GLOBAL: u64 = 1 << 8;
pub const NO_EXECUTE: u64 = 1 << 63;

const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const HUGE_ADDR_MASK: u64 = 0x000F_FFFF_FFE0_0000;

static HHDM_OFFSET: Mutex<u64> = Mutex::new(0);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    const fn empty() -> Self {
        Self(0)
    }

    fn present(self) -> bool {
        self.0 & PRESENT != 0
    }

    fn huge(self) -> bool {
        self.0 & HUGE != 0
    }

    fn address(self) -> u64 {
        self.0 & ADDR_MASK
    }

    fn huge_address(self) -> u64 {
        self.0 & HUGE_ADDR_MASK
    }

    fn set(&mut self, address: u64, flags: u64) {
        self.0 = (address & ADDR_MASK) | flags;
    }

    fn set_huge(&mut self, address: u64, flags: u64) {
        self.0 = (address & HUGE_ADDR_MASK) | flags | HUGE;
    }

    fn clear(&mut self) {
        self.0 = 0;
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    fn clear(&mut self) {
        for entry in &mut self.entries {
            entry.clear();
        }
    }
}
pub fn phys_to_virt_addr(phys: u64) -> u64 {
    phys + hhdm_offset()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    OutOfMemory,
    AlreadyMapped,
    NotMapped,
    HugePageConflict,
    InvalidAlignment,
}

pub fn init(hhdm_offset: u64) {
    *HHDM_OFFSET.lock() = hhdm_offset;
}

fn hhdm_offset() -> u64 {
    *HHDM_OFFSET.lock()
}

fn phys_to_virt(phys: u64) -> *mut PageTable {
    (phys + hhdm_offset()) as *mut PageTable
}

fn pml4_index(address: u64) -> usize {
    ((address >> 39) & 0x1FF) as usize
}

fn pdpt_index(address: u64) -> usize {
    ((address >> 30) & 0x1FF) as usize
}

fn pd_index(address: u64) -> usize {
    ((address >> 21) & 0x1FF) as usize
}

fn pt_index(address: u64) -> usize {
    ((address >> 12) & 0x1FF) as usize
}

fn alloc_table() -> Result<u64, MapError> {
    let phys = pmm::alloc().ok_or(MapError::OutOfMemory)?;

    unsafe {
        let table = &mut *phys_to_virt(phys);
        table.clear();
    }

    Ok(phys)
}

unsafe fn table_from_entry(
    entry: &mut PageTableEntry,
    flags: u64,
) -> Result<&'static mut PageTable, MapError> {
    if !entry.present() {
        let phys = alloc_table()?;
        entry.set(phys, PRESENT | WRITABLE | (flags & USER));
    }

    if flags & USER != 0 {
        entry.0 |= USER;
    }

    if entry.huge() {
        return Err(MapError::HugePageConflict);
    }

    unsafe {
        Ok(&mut *phys_to_virt(entry.address()))
    }
}

pub fn current_pml4() -> u64 {
    let value: u64;

    unsafe {
        asm!(
            "mov {}, cr3",
            out(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }

    value & ADDR_MASK
}

pub unsafe fn map_page(
    pml4_phys: u64,
    virtual_address: u64,
    physical_address: u64,
    flags: u64,
) -> Result<(), MapError> {
    if virtual_address & (PAGE_SIZE - 1) != 0
        || physical_address & (PAGE_SIZE - 1) != 0
    {
        return Err(MapError::InvalidAlignment);
    }

    let pml4 = unsafe { &mut *phys_to_virt(pml4_phys) };

    let pml4e = &mut pml4.entries[pml4_index(virtual_address)];
    let pdpt = unsafe { table_from_entry(pml4e, flags)? };

    let pdpte = &mut pdpt.entries[pdpt_index(virtual_address)];

    if pdpte.huge() {
        return Err(MapError::HugePageConflict);
    }

    let pd = unsafe { table_from_entry(pdpte, flags)? };

    let pde = &mut pd.entries[pd_index(virtual_address)];

    if pde.huge() {
        return Err(MapError::HugePageConflict);
    }

    let pt = unsafe { table_from_entry(pde, flags)? };

    let pte = &mut pt.entries[pt_index(virtual_address)];

    if pte.present() {
        return Err(MapError::AlreadyMapped);
    }

    pte.set(
        physical_address,
        flags | PRESENT,
    );

    invlpg(virtual_address);

    Ok(())
}

pub unsafe fn map_huge_page(
    pml4_phys: u64,
    virtual_address: u64,
    physical_address: u64,
    flags: u64,
) -> Result<(), MapError> {
    if virtual_address & (HUGE_PAGE_SIZE - 1) != 0
        || physical_address & (HUGE_PAGE_SIZE - 1) != 0
    {
        return Err(MapError::InvalidAlignment);
    }

    let pml4 = unsafe { &mut *phys_to_virt(pml4_phys) };

    let pml4e = &mut pml4.entries[pml4_index(virtual_address)];
    let pdpt = unsafe { table_from_entry(pml4e, flags)? };

    let pdpte = &mut pdpt.entries[pdpt_index(virtual_address)];

    if pdpte.huge() {
        return Err(MapError::HugePageConflict);
    }

    let pd = unsafe { table_from_entry(pdpte, flags)? };

    let pde = &mut pd.entries[pd_index(virtual_address)];

    if pde.present() {
        return Err(MapError::AlreadyMapped);
    }

    pde.set_huge(
        physical_address,
        flags | PRESENT,
    );

    invlpg(virtual_address);

    Ok(())
}

pub unsafe fn unmap_page(
    pml4_phys: u64,
    virtual_address: u64,
) -> Result<u64, MapError> {
    if virtual_address & (PAGE_SIZE - 1) != 0 {
        return Err(MapError::InvalidAlignment);
    }

    let pml4 = unsafe { &mut *phys_to_virt(pml4_phys) };

    let pml4e = &mut pml4.entries[pml4_index(virtual_address)];

    if !pml4e.present() {
        return Err(MapError::NotMapped);
    }

    let pdpt = unsafe { &mut *phys_to_virt(pml4e.address()) };

    let pdpte = &mut pdpt.entries[pdpt_index(virtual_address)];

    if !pdpte.present() {
        return Err(MapError::NotMapped);
    }

    if pdpte.huge() {
        return Err(MapError::HugePageConflict);
    }

    let pd = unsafe { &mut *phys_to_virt(pdpte.address()) };

    let pde = &mut pd.entries[pd_index(virtual_address)];

    if !pde.present() {
        return Err(MapError::NotMapped);
    }

    if pde.huge() {
        return Err(MapError::HugePageConflict);
    }

    let pt = unsafe { &mut *phys_to_virt(pde.address()) };

    let pte = &mut pt.entries[pt_index(virtual_address)];

    if !pte.present() {
        return Err(MapError::NotMapped);
    }

    let phys = pte.address();

    pte.clear();
    invlpg(virtual_address);

    Ok(phys)
}

pub unsafe fn unmap_huge_page(
    pml4_phys: u64,
    virtual_address: u64,
) -> Result<u64, MapError> {
    if virtual_address & (HUGE_PAGE_SIZE - 1) != 0 {
        return Err(MapError::InvalidAlignment);
    }

    let pml4 = unsafe { &mut *phys_to_virt(pml4_phys) };

    let pml4e = &mut pml4.entries[pml4_index(virtual_address)];

    if !pml4e.present() {
        return Err(MapError::NotMapped);
    }

    let pdpt = unsafe { &mut *phys_to_virt(pml4e.address()) };

    let pdpte = &mut pdpt.entries[pdpt_index(virtual_address)];

    if !pdpte.present() {
        return Err(MapError::NotMapped);
    }

    let pd = unsafe { &mut *phys_to_virt(pdpte.address()) };

    let pde = &mut pd.entries[pd_index(virtual_address)];

    if !pde.present() || !pde.huge() {
        return Err(MapError::NotMapped);
    }

    let phys = pde.huge_address();

    pde.clear();
    invlpg(virtual_address);

    Ok(phys)
}

pub unsafe fn translate(
    pml4_phys: u64,
    virtual_address: u64,
) -> Option<u64> {
    let pml4 = unsafe { &*phys_to_virt(pml4_phys) };

    let pml4e = pml4.entries[pml4_index(virtual_address)];

    if !pml4e.present() {
        return None;
    }

    let pdpt = unsafe { &*phys_to_virt(pml4e.address()) };

    let pdpte = pdpt.entries[pdpt_index(virtual_address)];

    if !pdpte.present() {
        return None;
    }

    let pd = unsafe { &*phys_to_virt(pdpte.address()) };

    let pde = pd.entries[pd_index(virtual_address)];

    if !pde.present() {
        return None;
    }

    if pde.huge() {
        let offset = virtual_address & (HUGE_PAGE_SIZE - 1);
        return Some(pde.huge_address() + offset);
    }

    let pt = unsafe { &*phys_to_virt(pde.address()) };

    let pte = pt.entries[pt_index(virtual_address)];

    if !pte.present() {
        return None;
    }

    Some(pte.address() + (virtual_address & (PAGE_SIZE - 1)))
}

pub unsafe fn alloc_and_map(
    pml4_phys: u64,
    virtual_address: u64,
    flags: u64,
) -> Result<u64, MapError> {
    let phys = pmm::alloc().ok_or(MapError::OutOfMemory)?;

    match unsafe {
        map_page(
            pml4_phys,
            virtual_address,
            phys,
            flags,
        )
    } {
        Ok(()) => Ok(phys),
        Err(error) => {
            pmm::free(phys);
            Err(error)
        }
    }
}

pub unsafe fn free_and_unmap(
    pml4_phys: u64,
    virtual_address: u64,
) -> Result<(), MapError> {
    let phys = unsafe {
        unmap_page(
            pml4_phys,
            virtual_address,
        )?
    };

    pmm::free(phys);

    Ok(())
}

pub fn flush_tlb() {
    unsafe {
        let cr3 = current_pml4();

        asm!(
            "mov cr3, {}",
            in(reg) cr3,
            options(nostack, preserves_flags)
        );
    }
}

fn invlpg(address: u64) {
    unsafe {
        asm!(
            "invlpg [{}]",
            in(reg) address,
            options(nostack, preserves_flags)
        );
    }
}
pub fn new_address_space()
    -> Result<u64, MapError>
{
    let phys =
        pmm::alloc()
            .ok_or(MapError::OutOfMemory)?;

    unsafe {
        let new =
            &mut *phys_to_virt(phys);

        new.clear();

        let current =
            &*phys_to_virt(
                current_pml4()
            );

        for i in 256..512 {
            new.entries[i] =
                current.entries[i];
        }
    }

    Ok(phys)
}pub unsafe fn switch_address_space(
    cr3: u64,
) {
    unsafe {
        asm!(
            "mov cr3, {}",
            in(reg) cr3,
            options(
                nostack,
                preserves_flags
            )
        );
    }
}
pub fn zero_phys_page(
    phys: u64,
) {
    let ptr =
        phys_to_virt_addr(phys)
            as *mut u8;

    unsafe {
        core::ptr::write_bytes(
            ptr,
            0,
            PAGE_SIZE as usize,
        );
    }
}
