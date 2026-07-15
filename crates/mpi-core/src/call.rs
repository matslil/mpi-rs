//! Task-internal suspended call futures.

use ctx_future::{CtxFuture, CtxPoll};
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::channel::{Receiver, Sender, TryRecvError, channel};
use crate::error::CallError;
use crate::lifecycle::{TaskMonitor, TaskTermination};
use crate::message::LateReplyPolicy;
use crate::session::{Response, SessionId, SyncReplySender, sync_reply_channel};

/// Type-erased queued call response carried by a caller task message.
pub struct QueuedCallResponse {
    /// Completed session ID.
    pub session_id: SessionId,

    /// Type-erased reply value.
    pub value: Box<dyn Any + Send>,

    /// Policy for this reply if the session no longer has an active waiter.
    pub late_reply_policy: LateReplyPolicy,
}

impl QueuedCallResponse {
    /// Construct a queued call response.
    #[must_use]
    pub fn new(session_id: SessionId, value: Box<dyn Any + Send>) -> Self {
        Self::with_late_reply_policy(session_id, value, LateReplyPolicy::Report)
    }

    /// Construct a queued call response with an explicit late-reply policy.
    #[must_use]
    pub fn with_late_reply_policy(
        session_id: SessionId,
        value: Box<dyn Any + Send>,
        late_reply_policy: LateReplyPolicy,
    ) -> Self {
        Self {
            session_id,
            value,
            late_reply_policy,
        }
    }
}

/// Hidden call-release control message carried by a callee task queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuedCallRelease {
    /// Call session released by the caller.
    pub session_id: SessionId,
}

impl QueuedCallRelease {
    /// Construct a queued call-release control message.
    #[must_use]
    pub const fn new(session_id: SessionId) -> Self {
        Self { session_id }
    }
}

/// Message enums that can carry queued task-internal call responses.
pub trait CallResponseMessage: Sized {
    /// Wrap a typed response value into this task's message enum.
    fn call_response(session_id: SessionId, value: Box<dyn Any + Send>) -> Self {
        Self::call_response_with_late_reply_policy(session_id, value, LateReplyPolicy::Report)
    }

    /// Wrap a typed response value with an explicit late-reply policy.
    fn call_response_with_late_reply_policy(
        session_id: SessionId,
        value: Box<dyn Any + Send>,
        late_reply_policy: LateReplyPolicy,
    ) -> Self;

    /// Extract a queued call response from this message, if it is one.
    fn into_call_response(self) -> Result<QueuedCallResponse, Self>;
}

/// Message enums that can carry queued task-internal call-release control.
pub trait CallReleaseMessage: Sized {
    /// Wrap a call-release control value into this task's message enum.
    fn call_release(session_id: SessionId) -> Self;

    /// Extract a queued call-release control value from this message, if it is one.
    fn into_call_release(self) -> Result<QueuedCallRelease, Self>;
}

/// Owned task-local call session state returned by a task context.
pub type CallSession<T> = (SessionId, SyncReplySender<T>, SuspendedCall<T>);

type CallOnDrop = Box<dyn FnOnce(SessionId) + 'static>;

/// Context-returning computation returned by a task-internal call.
///
/// The task context allocates the `SessionId` and constructs this owned future
/// before the request is enqueued. The future does not borrow the task context,
/// so the generated task runtime can keep using the main context while the call
/// is suspended.
pub struct SuspendedCall<T> {
    session_id: Option<SessionId>,
    receiver: Option<CallReceiver<T>>,
    failed: Option<CallError>,
    on_drop: Option<CallOnDrop>,
    target_monitor: Option<TaskMonitor>,
}

enum CallReceiver<T> {
    Direct(Receiver<Response<T>>),
    Queued(Receiver<Result<Response<T>, TaskTermination>>),
}

impl<T> SuspendedCall<T> {
    fn register_waker(&self, waker: &std::task::Waker) {
        if let Some(receiver) = &self.receiver {
            match receiver {
                CallReceiver::Direct(receiver) => receiver.register_waker(waker),
                CallReceiver::Queued(receiver) => receiver.register_waker(waker),
            }
        }
    }

    /// Create a suspended call future for an active session.
    #[must_use]
    pub fn pending(session_id: SessionId, receiver: Receiver<Response<T>>) -> Self {
        Self {
            session_id: Some(session_id),
            receiver: Some(CallReceiver::Direct(receiver)),
            failed: None,
            on_drop: None,
            target_monitor: None,
        }
    }

    /// Create a suspended call future for an active session with a drop hook.
    #[must_use]
    pub fn pending_with_on_drop<F>(
        session_id: SessionId,
        receiver: Receiver<Response<T>>,
        on_drop: F,
    ) -> Self
    where
        F: FnOnce(SessionId) + 'static,
    {
        Self {
            session_id: Some(session_id),
            receiver: Some(CallReceiver::Direct(receiver)),
            failed: None,
            on_drop: Some(Box::new(on_drop)),
            target_monitor: None,
        }
    }

    fn pending_queued(
        session_id: SessionId,
        receiver: Receiver<Result<Response<T>, TaskTermination>>,
    ) -> Self {
        Self {
            session_id: Some(session_id),
            receiver: Some(CallReceiver::Queued(receiver)),
            failed: None,
            on_drop: None,
            target_monitor: None,
        }
    }

    /// Retain the target lifecycle registration for this active call.
    #[must_use]
    pub fn with_target_monitor(mut self, monitor: TaskMonitor) -> Self {
        self.target_monitor = Some(monitor);
        self
    }

    /// Chain another drop hook onto this suspended call.
    ///
    /// Hooks run in registration order when the active future is dropped before
    /// completion. Normal completion disarms all hooks.
    #[must_use]
    pub fn with_additional_on_drop<F>(mut self, on_drop: F) -> Self
    where
        F: FnOnce(SessionId) + 'static,
    {
        let previous = self.on_drop.take();
        self.on_drop = Some(Box::new(move |session_id| {
            if let Some(previous) = previous {
                previous(session_id);
            }
            on_drop(session_id);
        }));
        self
    }

    /// Create a suspended call future that immediately resolves to an error.
    #[must_use]
    pub fn failed(error: CallError) -> Self {
        Self {
            session_id: None,
            receiver: None,
            failed: Some(error),
            on_drop: None,
            target_monitor: None,
        }
    }

    /// Return the session ID for an active suspended call.
    #[must_use]
    pub const fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }

    fn disarm_on_drop(&mut self) {
        self.on_drop = None;
        self.target_monitor = None;
    }

    fn try_resume(&mut self) -> CtxPoll<Result<T, CallError>> {
        if let Some(error) = self.failed.take() {
            return CtxPoll::Ready(Err(error));
        }

        let session_id = self
            .session_id
            .expect("suspended call polled after completion");
        let receiver = self
            .receiver
            .as_ref()
            .expect("suspended call receiver missing");

        let result = match receiver {
            CallReceiver::Direct(receiver) => receiver.try_recv().map(Ok),
            CallReceiver::Queued(receiver) => receiver.try_recv(),
        };
        match result {
            Ok(Ok(response)) => {
                assert_eq!(
                    response.session_id, session_id,
                    "suspended call received response for wrong session"
                );
                self.session_id = None;
                self.receiver = None;
                self.disarm_on_drop();
                CtxPoll::Ready(Ok(response.value))
            }
            Ok(Err(termination)) => {
                self.session_id = None;
                self.receiver = None;
                self.disarm_on_drop();
                CtxPoll::Ready(Err(CallError::TargetTerminated(termination)))
            }
            Err(TryRecvError::Empty) => CtxPoll::Pending,
            Err(TryRecvError::Disconnected) => {
                self.session_id = None;
                self.receiver = None;
                self.disarm_on_drop();
                CtxPoll::Ready(Err(CallError::ReplyDisconnected))
            }
        }
    }
}

impl<Cx, T> CtxFuture<Cx> for SuspendedCall<T> {
    type Output = Result<T, CallError>;

    fn resume(&mut self, _cx: &mut Cx, (): ()) -> CtxPoll<Self::Output> {
        self.try_resume()
    }
}

/// Compatibility bridge for Rust `.await` syntax in user-authored async
/// handlers. The task-local runtime drives the same state through `CtxFuture`.
impl<T> Future for SuspendedCall<T> {
    type Output = Result<T, CallError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.register_waker(cx.waker());
        match this.try_resume() {
            CtxPoll::Ready(value) => Poll::Ready(value),
            CtxPoll::Pending => Poll::Pending,
        }
    }
}

impl<T> Drop for SuspendedCall<T> {
    fn drop(&mut self) {
        if let (Some(session_id), Some(on_drop)) = (self.session_id.take(), self.on_drop.take()) {
            on_drop(session_id);
        }
    }
}

/// Create the reply sender and suspended future for one direct in-memory call.
#[must_use]
pub fn suspended_call_channel<T: Send + 'static>(session_id: SessionId) -> CallSession<T> {
    let (sender, receiver) = sync_reply_channel();
    (
        session_id,
        sender,
        SuspendedCall::pending(session_id, receiver),
    )
}

/// Create the waiter sender and suspended future for one queued task-internal call.
#[must_use]
pub fn suspended_call_waiter<T>(
    session_id: SessionId,
) -> (
    Sender<Result<Response<T>, TaskTermination>>,
    SuspendedCall<T>,
) {
    let (sender, receiver) = channel();
    (sender, SuspendedCall::pending_queued(session_id, receiver))
}

/// Create the waiter sender and suspended future for one queued task-internal
/// call with a hook for dropped futures.
#[must_use]
pub(crate) fn suspended_call_waiter_with_on_drop<T, F>(
    session_id: SessionId,
    on_drop: F,
) -> (
    Sender<Result<Response<T>, TaskTermination>>,
    SuspendedCall<T>,
)
where
    F: FnOnce(SessionId) + 'static,
{
    let (sender, receiver) = channel();
    let future =
        SuspendedCall::pending_queued(session_id, receiver).with_additional_on_drop(on_drop);
    (sender, future)
}
