//! Minimal task-local future execution support.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// Run a future to completion on the current task thread.
///
/// This executor is intentionally minimal. It is sufficient for generated
/// handlers that complete without waiting on external async runtimes. More
/// advanced suspended-receive behavior is expected to be implemented by the
/// task-local runtime and waiter registry in a later phase.
pub fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = pin!(future);

    loop {
        match Future::poll(future.as_mut(), &mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
