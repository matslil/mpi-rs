//! Task-internal suspended call futures.

use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::task::{Context, Poll};

use crate::error::CallError;
use crate::session::{Response, SessionId, SyncReplySender, sync_reply_channel};

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

impl<T> Unpin for SuspendedCall<T> {}

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

/// Create the reply sender and suspended future for one task-internal call.
#[must_use]
pub fn suspended_call_channel<T>(session_id: SessionId) -> (SyncReplySender<T>, SuspendedCall<T>) {
    let (sender, receiver) = sync_reply_channel();
    (sender, SuspendedCall::pending(session_id, receiver))
}
