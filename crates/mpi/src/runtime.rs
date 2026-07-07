//! Minimal task-local future execution support.

use ctx_future::{CtxFuture, CtxPoll};
use std::collections::VecDeque;
use std::sync::Arc;

use crate::call::{CallReleaseMessage, CallResponseMessage};
use crate::message::TaskMessage;
use crate::queue::TaskQueue;
use crate::stream::{StreamCancelMessage, StreamEventMessage, StreamPullMessage};
use crate::task::TaskContext;

fn route_task_message<M, const N: usize>(
    message: M,
    ctx: &TaskContext<M, N>,
    deferred: &mut VecDeque<M>,
) where
    M: TaskMessage
        + CallResponseMessage
        + CallReleaseMessage
        + StreamPullMessage
        + StreamCancelMessage
        + StreamEventMessage,
{
    match message.into_call_response() {
        Ok(response) => {
            let _ = ctx.deliver_call_response(response);
        }
        Err(message) => match message.into_call_release() {
            Ok(release) => {
                ctx.record_call_release(release);
            }
            Err(message) => match message.into_stream_pull() {
                Ok(pull) => {
                    ctx.record_stream_pull(pull);
                }
                Err(message) => match message.into_stream_cancel() {
                    Ok(cancel) => {
                        ctx.record_stream_cancel(cancel);
                    }
                    Err(message) => match message.into_stream_event() {
                        Ok(event) => {
                            let _ = ctx.deliver_stream_event(event);
                        }
                        Err(message) => deferred.push_back(message),
                    },
                },
            },
        },
    }
}

/// Run a context-returning task-local computation while routing queued protocol
/// messages to the task context between resume steps.
///
/// The suspended computation receives mutable task context only while
/// `CtxFuture::resume` is executing. When it returns `Pending`, this driver owns
/// the context again and can route replies, stream events, and control messages
/// before resuming the computation later.
pub fn block_on_ctx_task<M, F, const N: usize>(
    mut future: F,
    queue: &Arc<TaskQueue<M, N>>,
    ctx: &mut TaskContext<M, N>,
    deferred: &mut VecDeque<M>,
) -> F::Output
where
    M: TaskMessage
        + CallResponseMessage
        + CallReleaseMessage
        + StreamPullMessage
        + StreamCancelMessage
        + StreamEventMessage,
    F: CtxFuture<TaskContext<M, N>>,
{
    loop {
        match future.resume(ctx, ()) {
            CtxPoll::Ready(value) => return value,
            CtxPoll::Pending => match queue.recv() {
                Ok(message) => route_task_message(message, ctx, deferred),
                Err(_) => std::thread::yield_now(),
            },
        }
    }
}

/// Run a handler continuation while routing queued call responses, call lifecycle
/// messages, stream pull control, and stream events to registered task-local
/// state.
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
    M: TaskMessage
        + CallResponseMessage
        + CallReleaseMessage
        + StreamPullMessage
        + StreamCancelMessage
        + StreamEventMessage,
    F: CtxFuture<TaskContext<M, N>>,
{
    let mut ctx = ctx.clone();
    block_on_ctx_task(future, queue, &mut ctx, deferred)
}
