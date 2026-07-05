//! Task-internal suspended call futures.

use ctx_future::{CtxFuture, CtxPoll};
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::task::{Context, Poll};

use crate::error::CallError;
use crate::session::{Response, SessionId, SyncReplySender, sync_reply_channel};

/// Type-erased queued call response carried by a caller task message.
pub struct QueuedCallResponse {
    /// Completed session ID.
    pub session_id: SessionId,

    /// Type-erased reply value.
    pub value: Box<dyn Any + Send>,
}

impl QueuedCallResponse {
    /// Construct a queued call response.
    #[must_use]
    pub fn new(session_id: SessionId, value: Box<dyn Any + Send>) -> Self {
        Self { session_id, value }
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
    fn call_response(session_id: SessionId, value: Box<dyn Any + Send>) -> Self;

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

/// Future returned by a task-internal call.
///
/// The task context allocates the `SessionId` and constructs this owned future
/// before the request is enqueued. The future does not borrow the task context,
/// so the generated task runtime can keep using the main context while the call
/// is suspended.
pub struct SuspendedCall<T> {
    session_id: Option<SessionId>,
    receiver: Option<Receiver<Response<T>>>,
    failed: Option<CallError>,
    on_drop: Option<CallOnDrop>,
}

impl<T> SuspendedCall<T> {
    /// Create a suspended call future for an active session.
    #[must_use]
    pub fn pending(session_id: SessionId, receiver: Receiver<Response<T>>) -> Self {
        Self {
            session_id: Some(session_id),
            receiver: Some(receiver),
            failed: None,
            on_drop: None,
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
            receiver: Some(receiver),
            failed: None,
            on_drop: Some(Box::new(on_drop)),
        }
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
        }
    }

    /// Return the session ID for an active suspended call.
    #[must_use]
    pub const fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }

    fn disarm_on_drop(&mut self) {
        self.on_drop = None;
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

        match receiver.try_recv() {
            Ok(response) => {
                assert_eq!(
                    response.session_id, session_id,
                    "suspended call received response for wrong session"
                );
                self.session_id = None;
                self.receiver = None;
                self.disarm_on_drop();
                CtxPoll::Ready(Ok(response.value))
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

impl<T> Future for SuspendedCall<T> {
    type Output = Result<T, CallError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.get_mut().try_resume() {
            CtxPoll::Ready(value) => Poll::Ready(value),
            CtxPoll::Pending => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
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
pub fn suspended_call_waiter<T>(session_id: SessionId) -> (Sender<Response<T>>, SuspendedCall<T>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    (sender, SuspendedCall::pending(session_id, receiver))
}

/// Create the waiter sender and suspended future for one queued task-internal
/// call with a hook for dropped futures.
#[must_use]
pub(crate) fn suspended_call_waiter_with_on_drop<T, F>(
    session_id: SessionId,
    on_drop: F,
) -> (Sender<Response<T>>, SuspendedCall<T>)
where
    F: FnOnce(SessionId) + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    (
        sender,
        SuspendedCall::pending_with_on_drop(session_id, receiver, on_drop),
    )
}
