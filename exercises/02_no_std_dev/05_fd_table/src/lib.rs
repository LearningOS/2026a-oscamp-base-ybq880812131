//! # File Descriptor Table
//!
//! Implement a simple file descriptor (fd) table — the core data structure
//! for managing open files in an OS kernel.

use std::sync::Arc;

pub trait File: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> isize;
    fn write(&self, buf: &[u8]) -> isize;
}

pub struct FdTable {
    entries: Vec<Option<Arc<dyn File>>>,
}

impl FdTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn alloc(&mut self, file: Arc<dyn File>) -> usize {
        for (i, slot) in self.entries.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(file);
                return i;
            }
        }
        let fd = self.entries.len();
        self.entries.push(Some(file));
        fd
    }

    pub fn get(&self, fd: usize) -> Option<Arc<dyn File>> {
        self.entries.get(fd)?.clone()
    }

    pub fn close(&mut self, fd: usize) -> bool {
        match self.entries.get_mut(fd) {
            Some(slot) => {
                if slot.is_some() {
                    *slot = None;
                    true
                } else {
                    false
                }
            }
            None => false,
        }
    }

    pub fn count(&self) -> usize {
        self.entries.iter().filter(|s| s.is_some()).count()
    }
}

impl Default for FdTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockFile {
        id: usize,
        write_log: Mutex<Vec<Vec<u8>>>,
    }

    impl MockFile {
        fn new(id: usize) -> Arc<Self> {
            Arc::new(Self {
                id,
                write_log: Mutex::new(vec![]),
            })
        }
    }

    impl File for MockFile {
        fn read(&self, buf: &mut [u8]) -> isize {
            buf[0] = self.id as u8;
            1
        }
        fn write(&self, buf: &[u8]) -> isize {
            self.write_log.lock().unwrap().push(buf.to_vec());
            buf.len() as isize
        }
    }

    #[test]
    fn test_alloc_basic() {
        let mut table = FdTable::new();
        let fd = table.alloc(MockFile::new(0));
        assert_eq!(fd, 0, "first fd should be 0");
        let fd2 = table.alloc(MockFile::new(1));
        assert_eq!(fd2, 1, "second fd should be 1");
    }

    #[test]
    fn test_get() {
        let mut table = FdTable::new();
        let file = MockFile::new(42);
        let fd = table.alloc(file);
        let got = table.get(fd);
        assert!(got.is_some(), "get should return Some");
        let mut buf = [0u8; 1];
        got.unwrap().read(&mut buf);
        assert_eq!(buf[0], 42);
    }

    #[test]
    fn test_get_invalid() {
        let table = FdTable::new();
        assert!(table.get(0).is_none());
        assert!(table.get(999).is_none());
    }

    #[test]
    fn test_close_and_reuse() {
        let mut table = FdTable::new();
        let fd0 = table.alloc(MockFile::new(0));
        let fd1 = table.alloc(MockFile::new(1));
        let fd2 = table.alloc(MockFile::new(2));

        assert!(table.close(fd1), "closing fd=1 should succeed");
        assert!(
            table.get(fd1).is_none(),
            "get should return None after close"
        );

        let fd_new = table.alloc(MockFile::new(99));
        assert_eq!(fd_new, fd1, "should reuse the smallest closed fd");

        let _ = (fd0, fd2);
    }

    #[test]
    fn test_close_invalid() {
        let mut table = FdTable::new();
        assert!(
            !table.close(0),
            "closing non-existent fd should return false"
        );
    }

    #[test]
    fn test_count() {
        let mut table = FdTable::new();
        assert_eq!(table.count(), 0);
        let fd0 = table.alloc(MockFile::new(0));
        let fd1 = table.alloc(MockFile::new(1));
        assert_eq!(table.count(), 2);
        table.close(fd0);
        assert_eq!(table.count(), 1);
        table.close(fd1);
        assert_eq!(table.count(), 0);
    }

    #[test]
    fn test_write_through_fd() {
        let mut table = FdTable::new();
        let file = MockFile::new(0);
        let fd = table.alloc(file);
        let f = table.get(fd).unwrap();
        let n = f.write(b"hello");
        assert_eq!(n, 5);
    }
}
