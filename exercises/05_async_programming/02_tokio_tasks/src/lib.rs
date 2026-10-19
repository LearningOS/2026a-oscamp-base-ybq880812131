//! # Tokio Tasks

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::Mutex;

pub async fn say_hi() -> String {
    tokio::time::sleep(Duration::from_millis(10)).await;
    "hi".to_string()
}

pub async fn concurrent_sum(n: u64) -> u64 {
    let mut handles = Vec::new();
    for i in 0..n {
        handles.push(tokio::spawn(async move { i }));
    }
    let mut sum = 0u64;
    for h in handles {
        if let Ok(v) = h.await {
            sum += v;
        }
    }
    sum
}

pub async fn race_tasks(n: u64) -> u64 {
    let winner = Arc::new(AtomicUsize::new(usize::MAX));
    let mut handles = Vec::new();
    for i in 0..n {
        let w = Arc::clone(&winner);
        handles.push(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1)).await;
            let _ = w.compare_exchange(usize::MAX, i as usize, Ordering::AcqRel, Ordering::Relaxed);
        }));
    }
    for h in handles {
        let _ = h.await;
    }
    winner.load(Ordering::Relaxed) as u64
}

static COUNTER: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(0));

pub async fn increment_counter() -> u32 {
    let mut guard = COUNTER.lock().await;
    *guard += 1;
    *guard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_say_hi() {
        assert_eq!(say_hi().await, "hi");
    }

    #[tokio::test]
    async fn test_concurrent_sum() {
        let result = concurrent_sum(10).await;
        assert_eq!(result, (0..10).sum::<u64>());
    }

    #[tokio::test]
    async fn test_increment_counter() {
        let handles: Vec<_> = (0..10)
            .map(|_| tokio::spawn(increment_counter()))
            .collect();
        for h in handles {
            assert!(h.await.is_ok());
        }
        let c = COUNTER.lock().await;
        assert!(*c >= 10);
    }
}
