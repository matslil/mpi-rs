use ctx_future::sleep_until;
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Wake, Waker};
use std::time::{Duration, Instant};

struct CountWake(AtomicUsize);

impl Wake for CountWake {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn ctx_req_006_ctx_req_007_sleep_wakes_at_deadline() {
    let wake = Arc::new(CountWake(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    let mut sleep = pin!(sleep_until(Instant::now() + Duration::from_millis(10)));

    assert!(sleep.as_mut().poll(&mut context).is_pending());
    let deadline = Instant::now() + Duration::from_secs(1);
    while wake.0.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(sleep.as_mut().poll(&mut context).is_ready());
}

#[test]
fn ctx_req_008_dropped_sleep_cancels_wakeup() {
    let wake = Arc::new(CountWake(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    let mut sleep = Box::pin(sleep_until(Instant::now() + Duration::from_millis(20)));

    assert!(sleep.as_mut().poll(&mut context).is_pending());
    drop(sleep);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
}

#[test]
fn ctx_req_009_sleeps_wake_in_deadline_order() {
    let first = Arc::new(CountWake(AtomicUsize::new(0)));
    let second = Arc::new(CountWake(AtomicUsize::new(0)));
    let first_waker = Waker::from(Arc::clone(&first));
    let second_waker = Waker::from(Arc::clone(&second));
    let mut first_context = Context::from_waker(&first_waker);
    let mut second_context = Context::from_waker(&second_waker);
    let now = Instant::now();
    let mut later = Box::pin(sleep_until(now + Duration::from_millis(30)));
    let mut sooner = Box::pin(sleep_until(now + Duration::from_millis(10)));

    assert!(later.as_mut().poll(&mut first_context).is_pending());
    assert!(sooner.as_mut().poll(&mut second_context).is_pending());
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(second.0.load(Ordering::SeqCst), 1);
    assert_eq!(first.0.load(Ordering::SeqCst), 0);
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(first.0.load(Ordering::SeqCst), 1);
}
