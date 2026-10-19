//! # Stackful Coroutine and Context Switch (x86_64)
//!
//! Implement minimal context save/restore using inline assembly.
//! Saves/restores callee-saved registers (rsp, rbx, r12-r15).

#![cfg(target_arch = "x86_64")]

use core::arch::naked_asm;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct TaskContext {
    pub rsp: u64,
    pub rbx: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl TaskContext {
    pub const fn empty() -> Self {
        Self {
            rsp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }

    pub fn init(&mut self, stack_top: usize, entry: usize) {
        unsafe {
            let stack_ptr = stack_top as *mut usize;
            *stack_ptr.sub(1) = entry;
        }
        self.rsp = (stack_top - 8) as u64;
    }
}

#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(_old: &mut TaskContext, _new: &TaskContext) {
    naked_asm!(
        "mov [rcx + 0x00], rsp",
        "mov [rcx + 0x08], rbx",
        "mov [rcx + 0x10], r12",
        "mov [rcx + 0x18], r13",
        "mov [rcx + 0x20], r14",
        "mov [rcx + 0x28], r15",
        "mov rsp, [rdx + 0x00]",
        "mov rbx, [rdx + 0x08]",
        "mov r12, [rdx + 0x10]",
        "mov r13, [rdx + 0x18]",
        "mov r14, [rdx + 0x20]",
        "mov r15, [rdx + 0x28]",
        "ret",
    );
}

const STACK_SIZE: usize = 1024 * 64;

pub fn alloc_stack() -> (Vec<u8>, usize) {
    let buf = vec![0u8; STACK_SIZE];
    let top = buf.as_ptr() as usize + STACK_SIZE;
    (buf, top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    extern "C" fn task_entry() {
        COUNTER.store(42, Ordering::SeqCst);
        loop {
            std::hint::spin_loop();
        }
    }

    #[test]
    fn test_alloc_stack() {
        let (buf, top) = alloc_stack();
        assert_eq!(top, buf.as_ptr() as usize + STACK_SIZE);
        assert!(top % 16 == 0);
    }

    #[test]
    fn test_context_init() {
        let (buf, top) = alloc_stack();
        let _ = buf;
        let mut ctx = TaskContext::empty();
        let entry = task_entry as *const () as usize;
        ctx.init(top, entry);
        assert_eq!(ctx.rsp, (top - 8) as u64);
        assert!(ctx.rsp != 0);
    }

    #[test]
    fn test_switch_to_task() {
        COUNTER.store(0, Ordering::SeqCst);

        static mut MAIN_CTX_PTR: *mut TaskContext = std::ptr::null_mut();
        static mut TASK_CTX_PTR: *mut TaskContext = std::ptr::null_mut();

        extern "C" fn cooperative_task() {
            COUNTER.store(99, Ordering::SeqCst);
            unsafe {
                switch_context(&mut *TASK_CTX_PTR, &*MAIN_CTX_PTR);
            }
        }

        let (_stack_buf, stack_top) = alloc_stack();
        let mut main_ctx = TaskContext::empty();
        let mut task_ctx = TaskContext::empty();
        task_ctx.init(stack_top, cooperative_task as *const () as usize);

        unsafe {
            MAIN_CTX_PTR = &mut main_ctx;
            TASK_CTX_PTR = &mut task_ctx;
            switch_context(&mut main_ctx, &task_ctx);
        }

        assert_eq!(COUNTER.load(Ordering::SeqCst), 99);
    }
}
