//! Marker traits and task-local API hooks for generated task scopes.

use crate::call::CallSession;

/// Marker trait implemented by generated task contexts.
pub trait TaskScope {
    /// Allocate one task-local call session and return its reply sender and
    /// owned suspended-call future.
    fn begin_call<T: Send + 'static>(&mut self) -> CallSession<T>;
}
