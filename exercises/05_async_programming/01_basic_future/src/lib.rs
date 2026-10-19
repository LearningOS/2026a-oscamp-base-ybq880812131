//! # Basic Future: Implement a simple Future manually

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::sync::Arc;

pub struct CounterFuture {
    target: u32,
    current: Arc<AtomicU32>,
    waker: Option<Waker>,
}

impl CounterFuture {
    pub fn new(target: u32, current: Arc<AtomicU32>) -> Self {
        Self {
            target,
            current,
            waker: None,
        }
    }

    pub fn increment(&mut self) {
        self.current.fetch_add(1, Ordering::SeqCst);
        if self.current.load(Ordering::SeqCst) >= self.target {
            if let Some(waker) = self.waker.take() {
                waker.wake();
            }
        }
    }
}

impl Future for CounterFuture {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let cur = self.current.load(Ordering::SeqCst);
        if cur >= self.target {
            Poll::Ready(cur)
        } else {
            self.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct ManualWaker {
    woken: AtomicBool,
}

impl Wake for ManualWaker {
    fn wake(self: Arc<Self>) {
        self.woken.store(true, Ordering::SeqCst);
    }
}

pub fn create_waker() -> (Waker, Arc<ManualWaker>) {
    let inner = Arc::new(ManualWaker {
        woken: AtomicBool::new(false),
    });
    let waker = inner.clone().into();
    (waker, inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ready_future() {
        let counter = Arc::new(AtomicU32::new(10));
        let mut fut = CounterFuture::new(10, counter);
        let waker = create_waker();
        let mut cx = Context::from_waker(&waker.0);
        let result = Pin::new(&mut fut).poll(&mut cx);
        assert!(result.is_ready());
    }

    #[test]
    fn test_pending_then_ready() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut fut = CounterFuture::new(5, counter.clone());
        let (waker, handle) = create_waker();
        let mut cx = Context::from_waker(&waker);

        assert!(matches!(
            Pin::new(&mut fut).poll(&mut cx),
            Poll::Pending
        ));
        assert!(!handle.woken.load(Ordering::SeqCst));

        for _ in 0..5 {
            fut.increment();
        }

        assert!(handle.woken.load(Ordering::SeqCst));
        let mut cx2 = Context::from_waker(&waker);
        assert!(matches!(
            Pin::new(&mut fut).poll(&mut cx2),
            Poll::Ready(5)
        ));
    }

    #[test]
    fn test_multiple_polls() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut fut = CounterFuture::new(3, counter.clone());
        let (waker, _) = create_waker();

        for _ in 0..3 {
            let mut cx = Context::from_waker(&waker);
            assert!(matches!(
                Pin::new(&mut fut).poll(&mut cx),
                Poll::Pending
            ));
            fut.increment();
        }

        let mut cx = Context::from_waker(&waker);
        assert!(matches!(
            Pin::new(&mut fut).poll(&mut cx),
            Poll::Ready(3)
        ));
    }
}
