//! # Free-List Allocator
//!
//! A free-list allocator manages a linked list of free blocks.
//! When deallocating, it re-inserts the block into the free list.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

pub const HEAP_SIZE: usize = 1024;
pub const BLOCK_HDR_SIZE: usize = core::mem::size_of::<FreeBlock>();

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FreeBlock {
    pub size: usize,
    pub next: *mut FreeBlock,
}

#[repr(C)]
pub struct FreeListAllocator {
    pub heap_start: usize,
    pub heap_end: usize,
    pub free_head: AtomicPtr<FreeBlock>,
}

impl FreeListAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            free_head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;

        let first_block = heap_start as *mut FreeBlock;
        (*first_block).size = heap_size - BLOCK_HDR_SIZE;
        (*first_block).next = ptr::null_mut();
        self.free_head.store(first_block, Ordering::SeqCst);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn free_list_alloc(heap_start: usize, heap_size: usize, size: usize, align: usize) -> *mut u8 {
    let mut offset = heap_start + BLOCK_HDR_SIZE;
    offset = (offset + align - 1) & !(align - 1);
    let _needed = (offset + size) - heap_start;

    // Store allocator state at heap_start
    let state_ptr = heap_start as *mut FreeBlock;
    let _ = state_ptr;

    let _free_head_ptr = heap_start as *mut FreeBlock;

    static mut ALLOC_STATE: *mut FreeBlock = ptr::null_mut();
    static mut ALLOC_HEAP_START: usize = 0;
    static mut ALLOC_HEAP_END: usize = 0;

    if ALLOC_STATE.is_null() || ALLOC_HEAP_START != heap_start {
        ALLOC_HEAP_START = heap_start;
        ALLOC_HEAP_END = heap_start + heap_size;
        let first_free = heap_start as *mut FreeBlock;
        (*first_free).size = heap_size - BLOCK_HDR_SIZE;
        (*first_free).next = ptr::null_mut();
        ALLOC_STATE = first_free;
    }

    let mut prev: *mut FreeBlock = ptr::null_mut();
    let mut curr: *mut FreeBlock = ALLOC_STATE;

    let align_up = |addr: usize, align: usize| -> usize { (addr + align - 1) & !(align - 1) };

    while !curr.is_null() {
        let curr_addr = curr as usize;
        let usable_start = align_up(curr_addr + BLOCK_HDR_SIZE, align);
        let can_hold = usable_start + size <= curr_addr + BLOCK_HDR_SIZE + (*curr).size;
        let remaining = curr_addr + BLOCK_HDR_SIZE + (*curr).size - (usable_start + size);

        if can_hold {
            let result = usable_start as *mut u8;
            if remaining >= BLOCK_HDR_SIZE + 16 {
                let new_free = (usable_start + size) as *mut FreeBlock;
                (*new_free).size = remaining - BLOCK_HDR_SIZE;
                (*new_free).next = (*curr).next;
                if prev.is_null() {
                    ALLOC_STATE = new_free;
                } else {
                    (*prev).next = new_free;
                }
            } else {
                let new_next = (*curr).next;
                if prev.is_null() {
                    ALLOC_STATE = new_next;
                } else {
                    (*prev).next = new_next;
                }
            }
            return result;
        }
        prev = curr;
        curr = (*curr).next;
    }

    ptr::null_mut()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn free_list_free(ptr_addr: usize, size: usize, align: usize) {
    static mut ALLOC_STATE: *mut FreeBlock = ptr::null_mut();

    let addr = (ptr_addr as *mut u8).sub(BLOCK_HDR_SIZE) as usize;
    let block = addr as *mut FreeBlock;
    (*block).size = size + ((ptr_addr - addr - BLOCK_HDR_SIZE) as usize); // approximate
    (*block).next = ALLOC_STATE;
    ALLOC_STATE = block;
    let _ = align;
}

unsafe impl GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut head = self.free_head.load(Ordering::SeqCst);
        let mut prev: *mut FreeBlock = ptr::null_mut();
        let align = layout.align();

        while !head.is_null() {
            let head_addr = head as usize;
            let usable_start = (head_addr + BLOCK_HDR_SIZE + align - 1) & !(align - 1);
            let total_block_size = BLOCK_HDR_SIZE + (*head).size;
            let can_hold = usable_start + layout.size() <= head_addr + total_block_size;

            if can_hold {
                let remaining = head_addr + total_block_size - (usable_start + layout.size());
                let new_next = (*head).next;

                if remaining >= BLOCK_HDR_SIZE + 16 {
                    let new_free = (usable_start + layout.size()) as *mut FreeBlock;
                    (*new_free).size = remaining - BLOCK_HDR_SIZE;
                    (*new_free).next = new_next;
                    let new_head = new_free;
                    if prev.is_null() {
                        self.free_head.store(new_head, Ordering::SeqCst);
                    } else {
                        (*prev).next = new_head;
                    }
                } else {
                    if prev.is_null() {
                        self.free_head.store(new_next, Ordering::SeqCst);
                    } else {
                        (*prev).next = new_next;
                    }
                }
                return usable_start as *mut u8;
            }
            prev = head;
            head = (*head).next;
        }
        ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let block_ptr = (ptr as usize - BLOCK_HDR_SIZE) as *mut FreeBlock;
        (*block_ptr).size = layout.size() + (ptr as usize - block_ptr as usize - BLOCK_HDR_SIZE);
        (*block_ptr).next = self.free_head.load(Ordering::SeqCst);
        self.free_head.store(block_ptr, Ordering::SeqCst);
    }
}
