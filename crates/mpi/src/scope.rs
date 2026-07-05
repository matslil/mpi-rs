//! Marker traits and task-local API hooks for generated task scopes.

use std::sync::Arc;

use crate::call::CallSession;
use crate::message::LateReplyPolicy;
use crate::stream::{StreamControl, StreamSession};

/// Marker trait implemented by generated task contexts.
pub trait TaskScope {
    /// Allocate one task-local call session and return its reply sender and
    /// owned suspended-call future.
    fn begin_call<T: Send + 'static>(&mut self) -> CallSession<T>;

    /// Allocate one task-local call session with an explicit late-reply policy.
    fn begin_call_with_late_reply_policy<T: Send + 'static>(
        &mut self,
        _late_reply_policy: LateReplyPolicy,
    ) -> CallSession<T> {
        self.begin_call::<T>()
    }

    /// Allocate one task-local stream session and return its event sender and
    /// owned suspended-stream consumer.
    fn begin_stream<T: Send + 'static, E: Send + 'static>(
        &mut self,
        control: Arc<dyn StreamControl>,
    ) -> StreamSession<T, E>;

    /// Allocate one task-local stream session with an explicit late-reply policy.
    fn begin_stream_with_late_reply_policy<T: Send + 'static, E: Send + 'static>(
        &mut self,
        control: Arc<dyn StreamControl>,
        _late_reply_policy: LateReplyPolicy,
    ) -> StreamSession<T, E> {
        self.begin_stream::<T, E>(control)
    }
}
