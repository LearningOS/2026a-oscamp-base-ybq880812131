//! # Async Channel

use tokio::sync::mpsc;

pub async fn send_recv_pair() -> String {
    let (tx, mut rx) = mpsc::channel::<String>(1);

    let producer = tokio::spawn(async move {
        tx.send("hello".to_string()).await.unwrap();
    });

    let consumer = tokio::spawn(async move {
        rx.recv().await.unwrap()
    });

    producer.await.unwrap();
    consumer.await.unwrap()
}

pub async fn multiple_receivers() -> Vec<u32> {
    let (tx, mut rx) = mpsc::channel::<u32>(10);
    let tx1 = tx.clone();
    let tx2 = tx.clone();
    drop(tx);

    let t1 = tokio::spawn(async move {
        for i in 0..5 {
            tx1.send(i).await.unwrap();
        }
    });

    let t2 = tokio::spawn(async move {
        for i in 5..10 {
            tx2.send(i).await.unwrap();
        }
    });

    let mut received = Vec::new();
    while let Some(v) = rx.recv().await {
        received.push(v);
    }

    t1.await.unwrap();
    t2.await.unwrap();

    received.sort();
    received
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_recv_pair() {
        assert_eq!(send_recv_pair().await, "hello");
    }

    #[tokio::test]
    async fn test_multiple_receivers() {
        let result = multiple_receivers().await;
        assert_eq!(result, (0..10).collect::<Vec<_>>());
    }
}
