//! Task-internal suspended call futures.

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

/// Message enums that can carry queued task-internal call responses.
pub trait CallResponseMessage: Sized {
    /// Wrap a typed response value into this task's message enum.
    fn call_response(session_id: SessionId, value: Box<dyn Any + Send>) -> Self;

    /// Extract a queued call response from this message, if it is one.
    fn into_call_response(self) -> Result<QueuedCallResponse, Self>;
}

/// Owned task-local call session state returned by a task context.
pub type CallSession<T> = (SessionId, SyncReplySender<T>, SuspendedCall<T>);

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
}

impl<T> SuspendedCall<T> {
    /// Create a suspended call future for an active session.
    #[must_use]
    pub fn pending(session_id: SessionId, receiver: Receiver<Response<T>>) -> Self {
        Self {
            session_id: Some(session_id),
            receiver: Some(receiver),
            failed: None,
        }
    }

    /// Create a suspended call future that immediately resolves to an error.
    #[must_use]
    pub fn failed(error: CallError) -> Self {
        Self {
            session_id: None,
            receiver: None,
            failed: Some(error),
        }
    }

    /// Return the session ID for an active suspended call.
    #[must_use]
    pub const fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }
}

impl<T> Future for SuspendedCall<T> {
    type Output = Result<T, CallError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        if let Some(error) = this.failed.take() {
            return Poll::Ready(Err(error));
        }

        let session_id = this
            .session_id
            .expect("suspended call polled after completion");
        let receiver = this
            .receiver
            .as_ref()
            .expect("suspended call receiver missing");

        match receiver.try_recv() {
            Ok(response) => {
                assert_eq!(
                    response.session_id, session_id,
                    "suspended call received response for wrong session"
                );
                this.session_id = None;
                this.receiver = None;
                Poll::Ready(Ok(response.value))
            }
            Err(TryRecvError::Empty) => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(TryRecvError::Disconnected) => {
                this.session_id = None;
                this.receiver = None;
                Poll::Ready(Err(CallError::ReplyDisconnected))
            }
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
