use limine::memory_map::EntryType;
use limine::request::MemoryMapRequest;
use spin::Mutex;

pub const PAGE_SIZE: u64 = 4096;
pub const MAX_MEMORY: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_PAGES: usize = (MAX_MEMORY / PAGE_SIZE) as usize;
const BITMAP_WORDS: usize = MAX_PAGES.div_ceil(64);

pub struct Pmm {
    bitmap: [u64; BITMAP_WORDS],
    total_pages: usize,
    free_pages: usize,
    next_hint: usize,
    initialized: bool,
}

impl Pmm {
    pub const fn new() -> Self {
        Self {
            bitmap: [u64::MAX; BITMAP_WORDS],
            total_pages: 0,
            free_pages: 0,
            next_hint: 0,
            initialized: false,
        }
    }

    fn set_used(&mut self, page: usize) {
        if page >= MAX_PAGES {
            return;
        }

        let word = page / 64;
        let bit = page % 64;

        self.bitmap[word] |= 1u64 << bit;
    }

    fn set_free(&mut self, page: usize) {
        if page >= MAX_PAGES {
            return;
        }

        let word = page / 64;
        let bit = page % 64;

        self.bitmap[word] &= !(1u64 << bit);
    }

    fn is_used(&self, page: usize) -> bool {
        if page >= MAX_PAGES {
            return true;
        }

        let word = page / 64;
        let bit = page % 64;

        self.bitmap[word] & (1u64 << bit) != 0
    }

    fn mark_region_free(&mut self, base: u64, length: u64) {
        let start = base.div_ceil(PAGE_SIZE);
        let end = (base + length) / PAGE_SIZE;

        for page in start..end {
            let page = page as usize;

            if page >= MAX_PAGES {
                break;
            }

            if self.is_used(page) {
                self.set_free(page);
                self.free_pages += 1;
            }
        }
    }

    fn mark_region_used(&mut self, base: u64, length: u64) {
        let start = base / PAGE_SIZE;
        let end = (base + length).div_ceil(PAGE_SIZE);

        for page in start..end {
            let page = page as usize;

            if page >= MAX_PAGES {
                break;
            }

            if !self.is_used(page) {
                self.set_used(page);

                if self.free_pages > 0 {
                    self.free_pages -= 1;
                }
            }
        }
    }

    fn alloc_page(&mut self) -> Option<u64> {
        if !self.initialized || self.free_pages == 0 {
            return None;
        }

        let start = self.next_hint;

        for page in start..self.total_pages {
            if !self.is_used(page) {
                self.set_used(page);
                self.free_pages -= 1;
                self.next_hint = page + 1;

                return Some(page as u64 * PAGE_SIZE);
            }
        }

        for page in 1..start {
            if !self.is_used(page) {
                self.set_used(page);
                self.free_pages -= 1;
                self.next_hint = page + 1;

                return Some(page as u64 * PAGE_SIZE);
            }
        }

        None
    }

    fn alloc_pages(&mut self, count: usize) -> Option<u64> {
        if !self.initialized || count == 0 || count > self.free_pages {
            return None;
        }

        let mut run_start = 0usize;
        let mut run_length = 0usize;

        for page in 1..self.total_pages {
            if !self.is_used(page) {
                if run_length == 0 {
                    run_start = page;
                }

                run_length += 1;

                if run_length == count {
                    for p in run_start..run_start + count {
                        self.set_used(p);
                    }

                    self.free_pages -= count;
                    self.next_hint = run_start + count;

                    return Some(run_start as u64 * PAGE_SIZE);
                }
            } else {
                run_length = 0;
            }
        }

        None
    }

    fn free_page(&mut self, address: u64) -> bool {
        if !self.initialized {
            return false;
        }

        if address % PAGE_SIZE != 0 {
            return false;
        }

        let page = (address / PAGE_SIZE) as usize;

        if page == 0 || page >= self.total_pages || page >= MAX_PAGES {
            return false;
        }

        if !self.is_used(page) {
            return false;
        }

        self.set_free(page);
        self.free_pages += 1;

        if page < self.next_hint {
            self.next_hint = page;
        }

        true
    }

    fn free_pages_range(&mut self, address: u64, count: usize) -> bool {
        if count == 0 || address % PAGE_SIZE != 0 {
            return false;
        }

        let start = (address / PAGE_SIZE) as usize;

        if start == 0 {
            return false;
        }

        let Some(end) = start.checked_add(count) else {
            return false;
        };

        if end > self.total_pages || end > MAX_PAGES {
            return false;
        }

        for page in start..end {
            if !self.is_used(page) {
                return false;
            }
        }

        for page in start..end {
            self.set_free(page);
        }

        self.free_pages += count;

        if start < self.next_hint {
            self.next_hint = start;
        }

        true
    }
}

static PMM: Mutex<Pmm> = Mutex::new(Pmm::new());

pub fn init(request: &'static MemoryMapRequest) {
    let response = request
        .get_response()
        .expect("memory map unavailable");

    let mut pmm = PMM.lock();

    let mut highest = 0u64;

    for entry in response.entries() {
        let end = entry.base.saturating_add(entry.length);

        if end > highest {
            highest = end;
        }
    }

    highest = highest.min(MAX_MEMORY);

    pmm.total_pages = (highest / PAGE_SIZE) as usize;

    for entry in response.entries() {
        if entry.entry_type == EntryType::USABLE {
            pmm.mark_region_free(entry.base, entry.length);
        }
    }

    pmm.mark_region_used(0, PAGE_SIZE);

    pmm.next_hint = 1;
    pmm.initialized = true;
}

pub fn alloc() -> Option<u64> {
    PMM.lock().alloc_page()
}

pub fn alloc_many(count: usize) -> Option<u64> {
    PMM.lock().alloc_pages(count)
}

pub fn free(address: u64) -> bool {
    PMM.lock().free_page(address)
}

pub fn free_many(address: u64, count: usize) -> bool {
    PMM.lock().free_pages_range(address, count)
}

pub fn total_pages() -> usize {
    PMM.lock().total_pages
}

pub fn free_pages() -> usize {
    PMM.lock().free_pages
}

pub fn used_pages() -> usize {
    let pmm = PMM.lock();
    pmm.total_pages.saturating_sub(pmm.free_pages)
}

pub fn total_memory() -> u64 {
    total_pages() as u64 * PAGE_SIZE
}

pub fn free_memory() -> u64 {
    free_pages() as u64 * PAGE_SIZE
}

pub fn used_memory() -> u64 {
    used_pages() as u64 * PAGE_SIZE
}
