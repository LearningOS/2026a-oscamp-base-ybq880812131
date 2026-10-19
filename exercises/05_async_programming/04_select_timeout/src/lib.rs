//! # Select and Timeout

use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::error::Elapsed;

pub async fn race_primitives() -> &'static str {
    let (tx1, mut rx1) = oneshot::channel::<&'static str>();
    let (tx2, mut rx2) = oneshot::channel::<&'static str>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(5)).await;
        let _ = tx1.send("fast");
    });

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx2.send("slow");
    });

    tokio::select! {
        v = &mut rx1 => v.unwrap(),
        v = &mut rx2 => v.unwrap(),
    }
}

pub async fn with_timeout<F, T>(future: F, dur: Duration) -> Result<T, Elapsed>
where
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(dur, future).await
}

pub async fn collect_results() -> Vec<u32> {
    use tokio::sync::mpsc;
    let (tx, mut rx) = mpsc::channel::<u32>(10);

    let mut handles = Vec::new();
    for i in 0..3 {
        let tx = tx.clone();
        handles.push(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send(i).await.unwrap();
        }));
    }
    drop(tx);

    let mut results = Vec::new();
    while let Some(v) = rx.recv().await {
        results.push(v);
    }

    for h in handles {
        let _ = h.await;
    }

    results.sort();
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_race_primitives() {
        let result = race_primitives().await;
        assert_eq!(result, "fast");
    }

    #[tokio::test]
    async fn test_with_timeout_completes() {
        let fut = tokio::time::sleep(Duration::from_millis(5));
        let result = with_timeout(fut, Duration::from_millis(100)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_with_timeout_fires() {
        let fut = tokio::time::sleep(Duration::from_millis(200));
        let result = with_timeout(fut, Duration::from_millis(5)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_collect_results() {
        let results = collect_results().await;
        assert_eq!(results, vec![0, 1, 2]);
    }
}
