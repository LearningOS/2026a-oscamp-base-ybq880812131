//! # Read-Write Lock (Writer-Priority)
//!
//! Implement a writer-priority read-write lock from scratch using atomics.

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};

const READER_MASK: u32 = (1 << 30) - 1;
const WRITER_HOLDING: u32 = 1 << 30;
const WRITER_WAITING: u32 = 1 << 31;

pub struct RwLock<T> {
    state: AtomicU32,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for RwLock<T> {}
unsafe impl<T: Send + Sync> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            data: UnsafeCell::new(data),
        }
    }

    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        loop {
            let s = self.state.load(Ordering::Acquire);
            if (s & WRITER_HOLDING) != 0 || (s & WRITER_WAITING) != 0 {
                while {
                    let s = self.state.load(Ordering::Relaxed);
                    (s & WRITER_HOLDING) != 0 || (s & WRITER_WAITING) != 0
                } {
                    core::hint::spin_loop();
                }
                continue;
            }
            let readers = s & READER_MASK;
            if readers >= READER_MASK {
                core::hint::spin_loop();
                continue;
            }
            if self
                .state
                .compare_exchange(s, s + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return RwLockReadGuard { lock: self };
            }
        }
    }

    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.state.fetch_or(WRITER_WAITING, Ordering::Release);
        loop {
            let s = self.state.load(Ordering::Acquire);
            if (s & READER_MASK) != 0 || (s & WRITER_HOLDING) != 0 {
                while {
                    let s = self.state.load(Ordering::Relaxed);
                    (s & READER_MASK) != 0 || (s & WRITER_HOLDING) != 0
                } {
                    core::hint::spin_loop();
                }
                continue;
            }
            if self
                .state
                .compare_exchange(s, WRITER_HOLDING, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return RwLockWriteGuard { lock: self };
            }
        }
    }
}

pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock
            .state
            .fetch_and(!(WRITER_HOLDING | WRITER_WAITING), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_multiple_readers() {
        let lock = Arc::new(RwLock::new(0u32));
        let mut handles = vec![];
        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                let g = l.read();
                assert_eq!(*g, 0);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_writer_excludes_readers() {
        let lock = Arc::new(RwLock::new(0u32));
        let lock_w = Arc::clone(&lock);
        let writer = thread::spawn(move || {
            let mut g = lock_w.write();
            *g = 42;
        });
        writer.join().unwrap();
        let g = lock.read();
        assert_eq!(*g, 42);
    }

    #[test]
    fn test_concurrent_reads_after_write() {
        let lock = Arc::new(RwLock::new(Vec::<i32>::new()));
        {
            let mut g = lock.write();
            g.push(1);
            g.push(2);
        }
        let mut handles = vec![];
        for _ in 0..5 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                let g = l.read();
                assert_eq!(g.len(), 2);
                assert_eq!(&*g, &[1, 2]);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_writes_serialized() {
        let lock = Arc::new(RwLock::new(0u64));
        let mut handles = vec![];
        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let mut g = l.write();
                    *g += 1;
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(*lock.read(), 1000);
    }
}
