//! Marker traits and task-local API hooks for generated task scopes.

use std::sync::Arc;

use crate::call::CallSession;
use crate::stream::{StreamControl, StreamSession};

/// Marker trait implemented by generated task contexts.
pub trait TaskScope {
    /// Allocate one task-local call session and return its reply sender and
    /// owned suspended-call future.
    fn begin_call<T: Send + 'static>(&mut self) -> CallSession<T>;

    /// Allocate one task-local stream session and return its event sender and
    /// owned suspended-stream consumer.
    fn begin_stream<T: Send + 'static, E: Send + 'static>(
        &mut self,
        control: Arc<dyn StreamControl>,
    ) -> StreamSession<T, E>;
}
