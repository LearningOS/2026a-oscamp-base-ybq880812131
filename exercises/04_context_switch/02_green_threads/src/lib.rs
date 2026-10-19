//! # Green Thread Scheduler (x86_64)
//!
//! Build a simple cooperative (green) thread scheduler on top of context switching.

#![cfg(target_arch = "x86_64")]

use core::arch::naked_asm;

const STACK_SIZE: usize = 1024 * 128;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct TaskContext {
    rsp: u64,
    rbp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThreadState {
    Ready,
    Running,
    Finished,
}

struct GreenThread {
    ctx: TaskContext,
    state: ThreadState,
    _stack: Option<Vec<u8>>,
    entry: Option<extern "C" fn()>,
}

static mut CURRENT_THREAD_ENTRY: Option<extern "C" fn()> = None;

extern "C" fn thread_wrapper() {
    unsafe {
        let entry = CURRENT_THREAD_ENTRY;
        if let Some(f) = entry {
            CURRENT_THREAD_ENTRY = None;
            f();
        }
    }
    thread_finished();
}

#[unsafe(naked)]
unsafe extern "C" fn switch_context(_old: &mut TaskContext, _new: &TaskContext) {
    naked_asm!(
        "mov [rcx + 0], rsp",
        "mov [rcx + 8], rbp",
        "mov rsp, [rdx + 0]",
        "mov rbp, [rdx + 8]",
        "ret",
    );
}

pub struct Scheduler {
    threads: Vec<GreenThread>,
    current: usize,
}

impl Scheduler {
    pub fn new() -> Self {
        let main_thread = GreenThread {
            ctx: TaskContext::default(),
            state: ThreadState::Running,
            _stack: None,
            entry: None,
        };

        Self {
            threads: vec![main_thread],
            current: 0,
        }
    }

    pub fn spawn(&mut self, entry: extern "C" fn()) {
        let stack = vec![0u8; STACK_SIZE];
        let stack_top = stack.as_ptr() as usize + STACK_SIZE;

        unsafe {
            let p = stack_top as *mut usize;
            *p.sub(1) = thread_finished as *const () as usize;
            *p.sub(2) = thread_wrapper as *const () as usize;
        }

        let mut ctx = TaskContext::default();
        ctx.rsp = (stack_top - 16) as u64;

        self.threads.push(GreenThread {
            ctx,
            state: ThreadState::Ready,
            _stack: Some(stack),
            entry: Some(entry),
        });
    }

    pub fn run(&mut self) {
        unsafe {
            SCHEDULER = self as *mut Scheduler;
        }

        loop {
            let all_finished = self.threads[1..]
                .iter()
                .all(|t| t.state == ThreadState::Finished);
            if all_finished {
                break;
            }
            self.schedule_next();
        }

        unsafe {
            SCHEDULER = std::ptr::null_mut();
        }
    }

    fn schedule_next(&mut self) {
        let total = self.threads.len();

        if self.threads[self.current].state != ThreadState::Finished {
            self.threads[self.current].state = ThreadState::Ready;
        }

        let mut next = (self.current + 1) % total;
        let mut found = false;
        for _ in 0..total {
            if self.threads[next].state == ThreadState::Ready {
                found = true;
                break;
            }
            next = (next + 1) % total;
        }
        if !found {
            return;
        }

        self.threads[next].state = ThreadState::Running;

        if let Some(entry) = self.threads[next].entry {
            unsafe {
                CURRENT_THREAD_ENTRY = Some(entry);
            }
        }

        let old_idx = self.current;
        self.current = next;

        unsafe {
            let old_ctx: *mut TaskContext = &mut self.threads[old_idx].ctx;
            let new_ctx: *const TaskContext = &self.threads[next].ctx;
            switch_context(&mut *old_ctx, &*new_ctx);
        }
    }
}

static mut SCHEDULER: *mut Scheduler = std::ptr::null_mut();

pub fn yield_now() {
    unsafe {
        if !SCHEDULER.is_null() {
            (*SCHEDULER).schedule_next();
        }
    }
}

fn thread_finished() {
    unsafe {
        if !SCHEDULER.is_null() {
            let sched = &mut *SCHEDULER;
            sched.threads[sched.current].state = ThreadState::Finished;
            sched.schedule_next();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    static EXEC_ORDER: AtomicU32 = AtomicU32::new(0);

    extern "C" fn task_a() {
        EXEC_ORDER.fetch_add(1, Ordering::SeqCst);
        yield_now();
        EXEC_ORDER.fetch_add(10, Ordering::SeqCst);
        yield_now();
        EXEC_ORDER.fetch_add(100, Ordering::SeqCst);
    }

    extern "C" fn task_b() {
        EXEC_ORDER.fetch_add(1, Ordering::SeqCst);
        yield_now();
        EXEC_ORDER.fetch_add(10, Ordering::SeqCst);
    }

    #[test]
    fn test_scheduler_runs_all() {
        let _guard = TEST_LOCK.lock().unwrap();
        EXEC_ORDER.store(0, Ordering::SeqCst);

        let mut sched = Scheduler::new();
        sched.spawn(task_a);
        sched.spawn(task_b);
        sched.run();

        let got = EXEC_ORDER.load(Ordering::SeqCst);
        if got != 122 {
            panic!(
                "EXEC_ORDER: expected 122, got {} (run with --nocapture to see stderr)",
                got
            );
        }
    }

    static SIMPLE_FLAG: AtomicU32 = AtomicU32::new(0);

    extern "C" fn simple_task() {
        SIMPLE_FLAG.store(42, Ordering::SeqCst);
    }

    #[test]
    fn test_single_thread() {
        let _guard = TEST_LOCK.lock().unwrap();
        SIMPLE_FLAG.store(0, Ordering::SeqCst);

        let mut sched = Scheduler::new();
        sched.spawn(simple_task);
        sched.run();

        assert_eq!(SIMPLE_FLAG.load(Ordering::SeqCst), 42);
    }
}
