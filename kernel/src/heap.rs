use core::alloc::{GlobalAlloc, Layout};
use core::mem::{align_of, size_of};
use core::ptr::null_mut;

use spin::Mutex;

use crate::{pmm, vmm};

const HEAP_START: u64 = 0xFFFF_A000_0000_0000;
const HEAP_INITIAL_PAGES: usize = 256;
const HEAP_MAX_PAGES: usize = 4096;

#[repr(C)]
struct Block {
    size: usize,
    free: bool,
    next: *mut Block,
}

unsafe impl Send for Block {}

pub struct Heap {
    head: *mut Block,
    mapped_pages: usize,
    initialized: bool,
}

unsafe impl Send for Heap {}

impl Heap {
    pub const fn new() -> Self {
        Self {
            head: null_mut(),
            mapped_pages: 0,
            initialized: false,
        }
    }

    unsafe fn init(&mut self) -> bool {
        if self.initialized {
            return true;
        }

        if !unsafe { self.map_pages(HEAP_INITIAL_PAGES) } {
            return false;
        }

        let head = HEAP_START as *mut Block;

        unsafe {
            head.write(Block {
                size: HEAP_INITIAL_PAGES * pmm::PAGE_SIZE as usize
                    - size_of::<Block>(),
                free: true,
                next: null_mut(),
            });
        }

        self.head = head;
        self.initialized = true;

        true
    }

    unsafe fn map_pages(&mut self, count: usize) -> bool {
        if self.mapped_pages + count > HEAP_MAX_PAGES {
            return false;
        }

        let pml4 = vmm::current_pml4();
        let start = self.mapped_pages;

        for i in 0..count {
            let virt =
                HEAP_START + ((start + i) as u64 * pmm::PAGE_SIZE);

            let Some(phys) = pmm::alloc() else {
                return false;
            };

            let result = unsafe {
                vmm::map_page(
                    pml4,
                    virt,
                    phys,
                    vmm::WRITABLE | vmm::NO_EXECUTE,
                )
            };

            if result.is_err() {
                pmm::free(phys);
                return false;
            }
        }

        self.mapped_pages += count;

        true
    }

    unsafe fn grow(&mut self, minimum: usize) -> bool {
        let needed =
            minimum
                .saturating_add(size_of::<Block>())
                .saturating_add(pmm::PAGE_SIZE as usize - 1)
                / pmm::PAGE_SIZE as usize;

        let pages = needed.max(16);

        if self.mapped_pages + pages > HEAP_MAX_PAGES {
            return false;
        }

        let old_end =
            HEAP_START as usize
                + self.mapped_pages
                    * pmm::PAGE_SIZE as usize;

        if !unsafe { self.map_pages(pages) } {
            return false;
        }

        let new_block =
            align_up(
                old_end,
                align_of::<Block>(),
            ) as *mut Block;

        let mapped_end =
            HEAP_START as usize
                + self.mapped_pages
                    * pmm::PAGE_SIZE as usize;

        let available =
            mapped_end
                .saturating_sub(new_block as usize)
                .saturating_sub(size_of::<Block>());

        unsafe {
            new_block.write(Block {
                size: available,
                free: true,
                next: null_mut(),
            });
        }

        let mut current = self.head;

        if current.is_null() {
            self.head = new_block;
            return true;
        }

        unsafe {
            while !(*current).next.is_null() {
                current = (*current).next;
            }

            (*current).next = new_block;
        }

        unsafe {
            self.merge();
        }

        true
    }

    unsafe fn alloc_inner(&mut self, layout: Layout) -> *mut u8 {
        if !self.initialized {
            if !unsafe { self.init() } {
                return null_mut();
            }
        }

        let requested = layout.size().max(1);
        let alignment =
            layout
                .align()
                .max(align_of::<Block>());

        loop {
            let mut current = self.head;

            while !current.is_null() {
                unsafe {
                    if (*current).free {
                        let payload_start =
                            current as usize
                                + size_of::<Block>();

                        let data_start =
                            align_up(
                                payload_start,
                                alignment,
                            );

                        let data_end =
                            match data_start
                                .checked_add(requested)
                            {
                                Some(value) => value,
                                None => return null_mut(),
                            };

                        let block_end =
                            payload_start
                                .saturating_add(
                                    (*current).size
                                );

                        if data_end <= block_end {
                            let next_header =
                                align_up(
                                    data_end,
                                    align_of::<Block>(),
                                );

                            let can_split =
                                next_header
                                    .checked_add(
                                        size_of::<Block>() + 16
                                    )
                                    .is_some_and(
                                        |v| v <= block_end
                                    );

                            if can_split {
                                let next =
                                    next_header
                                        as *mut Block;

                                let next_payload =
                                    next_header
                                        + size_of::<Block>();

                                next.write(Block {
                                    size:
                                        block_end
                                            - next_payload,
                                    free: true,
                                    next:
                                        (*current).next,
                                });

                                (*current).next = next;
                                (*current).size =
                                    next_header
                                        - payload_start;
                            }

                            (*current).free = false;

                            return data_start
                                as *mut u8;
                        }
                    }

                    current = (*current).next;
                }
            }

            let minimum =
                requested
                    .saturating_add(alignment)
                    .saturating_add(
                        size_of::<Block>(),
                    );

            if !unsafe { self.grow(minimum) } {
                return null_mut();
            }
        }
    }

    unsafe fn free_inner(&mut self, ptr: *mut u8) {
        if ptr.is_null() || !self.initialized {
            return;
        }

        let address = ptr as usize;

        let heap_start = HEAP_START as usize;
        let heap_end =
            heap_start
                + self.mapped_pages
                    * pmm::PAGE_SIZE as usize;

        if address < heap_start
            || address >= heap_end
        {
            return;
        }

        let mut current = self.head;

        while !current.is_null() {
            unsafe {
                let payload =
                    current as usize
                        + size_of::<Block>();

                let end =
                    payload
                        .saturating_add(
                            (*current).size
                        );

                if address >= payload
                    && address < end
                {
                    (*current).free = true;
                    self.merge();
                    return;
                }

                current = (*current).next;
            }
        }
    }

    unsafe fn merge(&mut self) {
        let mut current = self.head;

        while !current.is_null() {
            unsafe {
                let next = (*current).next;

                if next.is_null() {
                    break;
                }

                let current_end =
                    current as usize
                        + size_of::<Block>()
                        + (*current).size;

                if (*current).free
                    && (*next).free
                    && current_end
                        == next as usize
                {
                    (*current).size +=
                        size_of::<Block>()
                            + (*next).size;

                    (*current).next =
                        (*next).next;

                    continue;
                }

                current = next;
            }
        }
    }
}

pub struct KernelAllocator;

static HEAP: Mutex<Heap> =
    Mutex::new(Heap::new());

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        unsafe {
            HEAP
                .lock()
                .alloc_inner(layout)
        }
    }

    unsafe fn dealloc(
        &self,
        ptr: *mut u8,
        _layout: Layout,
    ) {
        unsafe {
            HEAP
                .lock()
                .free_inner(ptr);
        }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: KernelAllocator =
    KernelAllocator;

pub unsafe fn init() -> bool {
    unsafe {
        HEAP.lock().init()
    }
}

pub fn mapped_pages() -> usize {
    HEAP.lock().mapped_pages
}

pub fn size() -> usize {
    mapped_pages()
        * pmm::PAGE_SIZE as usize
}

const fn align_up(
    value: usize,
    alignment: usize,
) -> usize {
    (value + alignment - 1)
        & !(alignment - 1)
}
