use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Instant;

struct SleepState {
    cancelled: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

/// Owned future that becomes ready at an absolute monotonic deadline.
pub struct SleepUntil {
    deadline: Instant,
    state: Arc<SleepState>,
    started: bool,
}

/// Create an owned future that wakes at `deadline`.
#[must_use]
pub fn sleep_until(deadline: Instant) -> SleepUntil {
    SleepUntil {
        deadline,
        state: Arc::new(SleepState {
            cancelled: AtomicBool::new(false),
            waker: Mutex::new(None),
        }),
        started: false,
    }
}

impl Future for SleepUntil {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.deadline {
            return Poll::Ready(());
        }

        *self.state.waker.lock().expect("sleep waker lock poisoned") = Some(cx.waker().clone());

        if !self.started {
            self.started = true;
            let deadline = self.deadline;
            let state = Arc::clone(&self.state);
            std::thread::spawn(move || {
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
                if state.cancelled.load(Ordering::Acquire) {
                    return;
                }
                if let Some(waker) = state
                    .waker
                    .lock()
                    .expect("sleep waker lock poisoned")
                    .take()
                {
                    waker.wake();
                }
            });
        }

        Poll::Pending
    }
}

impl Drop for SleepUntil {
    fn drop(&mut self) {
        self.state.cancelled.store(true, Ordering::Release);
        self.state
            .waker
            .lock()
            .expect("sleep waker lock poisoned")
            .take();
    }
}
