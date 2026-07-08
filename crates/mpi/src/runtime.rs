//! Minimal task-local future execution support.

use ctx_future::{CtxFuture, CtxPoll, from_std_future};
use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;

use crate::call::{CallReleaseMessage, CallResponseMessage};
use crate::message::TaskMessage;
use crate::queue::TaskQueue;
use crate::stream::{StreamCancelMessage, StreamEventMessage, StreamPullMessage};
use crate::task::TaskContext;

fn route_task_message_with_dispatch<M, D, const N: usize>(
    message: M,
    ctx: &mut TaskContext<M, N>,
    dispatch: &mut D,
) where
    M: TaskMessage
        + CallResponseMessage
        + CallReleaseMessage
        + StreamPullMessage
        + StreamCancelMessage
        + StreamEventMessage,
    D: FnMut(M, &mut TaskContext<M, N>),
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
                        Err(message) => dispatch(message, ctx),
                    },
                },
            },
        },
    }
}

fn route_task_message<M, const N: usize>(
    message: M,
    ctx: &mut TaskContext<M, N>,
    deferred: &mut VecDeque<M>,
) where
    M: TaskMessage
        + CallResponseMessage
        + CallReleaseMessage
        + StreamPullMessage
        + StreamCancelMessage
        + StreamEventMessage,
{
    route_task_message_with_dispatch(message, ctx, &mut |message, _ctx| {
        deferred.push_back(message);
    });
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

/// Run a context-returning task-local computation while dispatching ordinary
/// messages between pending resume steps.
///
/// Protocol messages are still routed to task-local waiters and control state
/// before ordinary dispatch. Messages that are not protocol replies or control
/// messages are passed to `dispatch` while the suspended computation is pending.
/// This is the runtime building block for task loops whose handlers have been
/// lowered into native `CtxFuture` continuations.
pub fn block_on_ctx_task_with_dispatch<M, F, D, const N: usize>(
    mut future: F,
    queue: &Arc<TaskQueue<M, N>>,
    ctx: &mut TaskContext<M, N>,
    mut dispatch: D,
) -> F::Output
where
    M: TaskMessage
        + CallResponseMessage
        + CallReleaseMessage
        + StreamPullMessage
        + StreamCancelMessage
        + StreamEventMessage,
    F: CtxFuture<TaskContext<M, N>>,
    D: FnMut(M, &mut TaskContext<M, N>),
{
    loop {
        match future.resume(ctx, ()) {
            CtxPoll::Ready(value) => return value,
            CtxPoll::Pending => match queue.recv() {
                Ok(message) => route_task_message_with_dispatch(message, ctx, &mut dispatch),
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

/// Run a compiler-generated handler future through the task-local
/// context-returning scheduler.
///
/// This is the compatibility boundary for handler bodies that still rely on
/// Rust `.await` syntax. Generated dispatch calls this helper so the task
/// runtime remains responsible for adapting those continuations into
/// [`CtxFuture`] execution.
pub fn block_on_handler<M, F, const N: usize>(
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
    F: Future,
{
    block_on_task(from_std_future(future), queue, ctx, deferred)
}
