use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use crate::{CtxFuture, CtxPoll};

/// [`CtxFuture`] adapter for Rust standard futures.
///
/// This is an interoperability boundary for code that is still authored as
/// `async fn`. The adapted future never receives or stores caller-owned context;
/// context-aware suspension should be represented directly as [`CtxFuture`].
pub struct StdFutureCtx<F> {
    future: Pin<Box<F>>,
}

impl<F> StdFutureCtx<F> {
    /// Wrap a standard future so a [`CtxFuture`] scheduler can drive it.
    #[must_use]
    pub fn new(future: F) -> Self {
        Self {
            future: Box::pin(future),
        }
    }
}

/// Wrap a standard future so a [`CtxFuture`] scheduler can drive it.
#[must_use]
pub fn from_std_future<F>(future: F) -> StdFutureCtx<F> {
    StdFutureCtx::new(future)
}

impl<Cx, F> CtxFuture<Cx> for StdFutureCtx<F>
where
    F: Future,
{
    type Output = F::Output;

    fn resume(&mut self, _cx: &mut Cx, (): ()) -> CtxPoll<Self::Output> {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);

        match Future::poll(self.future.as_mut(), &mut context) {
            Poll::Ready(value) => CtxPoll::Ready(value),
            Poll::Pending => CtxPoll::Pending,
        }
    }
}
