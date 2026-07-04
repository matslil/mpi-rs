//! Minimal task-local future execution support.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use crate::call::CallResponseMessage;
use crate::message::TaskMessage;
use crate::queue::TaskQueue;
use crate::stream::StreamEventMessage;
use crate::task::TaskContext;

/// Run a future to completion on the current task thread.
///
/// This executor is intentionally minimal. It is sufficient for generated
/// handlers that complete without waiting on external async runtimes.
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

/// Run a handler future while routing queued call responses and stream events to
/// registered waiters.
///
/// Ordinary messages received while the current handler is suspended are deferred
/// and processed by the outer task loop after the current handler completes.
pub fn block_on_task<M, F, const N: usize>(
    future: F,
    queue: &Arc<TaskQueue<M, N>>,
    ctx: &TaskContext<M, N>,
    deferred: &mut VecDeque<M>,
) -> F::Output
where
    M: TaskMessage + CallResponseMessage + StreamEventMessage,
    F: Future,
{
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = pin!(future);

    loop {
        match Future::poll(future.as_mut(), &mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => match queue.recv() {
                Ok(message) => match message.into_call_response() {
                    Ok(response) => {
                        let _ = ctx.deliver_call_response(response);
                    }
                    Err(message) => match message.into_stream_event() {
                        Ok(event) => {
                            let _ = ctx.deliver_stream_event(event);
                        }
                        Err(message) => deferred.push_back(message),
                    },
                },
                Err(_) => std::thread::yield_now(),
            },
        }
    }
}
