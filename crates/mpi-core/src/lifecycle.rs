//! Task endpoint termination state and supervision subscriptions.

use ctx_future::{CtxFuture, CtxPoll};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};

use crate::session::{EndpointId, SessionId};

/// Irreversible reason recorded when a task endpoint terminates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskTermination {
    /// The task dispatch loop returned normally without an explicit stop.
    Completed,
    /// The task was explicitly stopped or its endpoint was closed.
    Stopped,
    /// The task dispatch loop unwound through the runtime panic boundary.
    Panicked {
        /// Sanitized string panic payload, when one was available.
        message: Option<String>,
    },
}

struct LifecycleState {
    termination: Option<TaskTermination>,
    monitors: HashMap<u64, Sender<TaskTermination>>,
}

pub(crate) struct EndpointLifecycle {
    state: Mutex<LifecycleState>,
    next_monitor: AtomicU64,
}

impl EndpointLifecycle {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(LifecycleState {
                termination: None,
                monitors: HashMap::new(),
            }),
            next_monitor: AtomicU64::new(0),
        })
    }

    pub(crate) fn termination(&self) -> Option<TaskTermination> {
        self.state
            .lock()
            .expect("endpoint lifecycle mutex poisoned")
            .termination
            .clone()
    }

    pub(crate) fn terminate(&self, termination: TaskTermination) -> bool {
        let monitors = {
            let mut state = self
                .state
                .lock()
                .expect("endpoint lifecycle mutex poisoned");
            if state.termination.is_some() {
                return false;
            }
            state.termination = Some(termination.clone());
            std::mem::take(&mut state.monitors)
        };
        for monitor in monitors.into_values() {
            let _ = monitor.send(termination.clone());
        }
        true
    }

    pub(crate) fn monitor(
        self: &Arc<Self>,
        subscriber: EndpointId,
        session_id: SessionId,
    ) -> TaskMonitor {
        let (sender, receiver) = mpsc::channel();
        let id = self.next_monitor.fetch_add(1, Ordering::Relaxed);
        let immediate = {
            let mut state = self
                .state
                .lock()
                .expect("endpoint lifecycle mutex poisoned");
            match state.termination.clone() {
                Some(termination) => Some(termination),
                None => {
                    state.monitors.insert(id, sender.clone());
                    None
                }
            }
        };
        if let Some(termination) = immediate {
            let _ = sender.send(termination);
        }
        TaskMonitor {
            target: Arc::downgrade(self),
            id: Some(id),
            subscriber,
            session_id,
            receiver,
            completed: false,
        }
    }

    fn cancel(&self, id: u64) {
        self.state
            .lock()
            .expect("endpoint lifecycle mutex poisoned")
            .monitors
            .remove(&id);
    }
}

/// Cancellable task-scoped subscription to another endpoint's termination.
pub struct TaskMonitor {
    target: Weak<EndpointLifecycle>,
    id: Option<u64>,
    subscriber: EndpointId,
    session_id: SessionId,
    receiver: Receiver<TaskTermination>,
    completed: bool,
}

impl TaskMonitor {
    /// Return the supervision session ID.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Return the endpoint that owns this subscription.
    #[must_use]
    pub const fn subscriber(&self) -> EndpointId {
        self.subscriber
    }

    pub(crate) fn try_termination(&mut self) -> Option<TaskTermination> {
        match self.try_resume() {
            CtxPoll::Ready(termination) => Some(termination),
            CtxPoll::Pending => None,
        }
    }

    fn try_resume(&mut self) -> CtxPoll<TaskTermination> {
        match self.receiver.try_recv() {
            Ok(termination) => {
                self.completed = true;
                self.id = None;
                CtxPoll::Ready(termination)
            }
            Err(TryRecvError::Empty) => CtxPoll::Pending,
            Err(TryRecvError::Disconnected) => {
                self.completed = true;
                self.id = None;
                CtxPoll::Ready(TaskTermination::Stopped)
            }
        }
    }
}

impl<Cx> CtxFuture<Cx> for TaskMonitor {
    type Output = TaskTermination;

    fn resume(&mut self, _cx: &mut Cx, (): ()) -> CtxPoll<Self::Output> {
        self.try_resume()
    }
}

impl Future for TaskMonitor {
    type Output = TaskTermination;

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

impl Drop for TaskMonitor {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        if let (Some(target), Some(id)) = (self.target.upgrade(), self.id.take()) {
            target.cancel(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpi_req_140_141_monitor_observes_exactly_one_recorded_termination() {
        let lifecycle = EndpointLifecycle::new();
        let session = SessionId::new(EndpointId(1), 1);
        let mut before = lifecycle.monitor(EndpointId(1), session);

        assert!(lifecycle.terminate(TaskTermination::Completed));
        assert!(!lifecycle.terminate(TaskTermination::Stopped));
        assert_eq!(before.try_termination(), Some(TaskTermination::Completed));
        assert_eq!(lifecycle.termination(), Some(TaskTermination::Completed));

        let mut after = lifecycle.monitor(EndpointId(2), SessionId::new(EndpointId(2), 1));
        assert_eq!(after.try_termination(), Some(TaskTermination::Completed));
    }

    #[test]
    fn mpi_req_142_dropping_monitor_cancels_subscription() {
        let lifecycle = EndpointLifecycle::new();
        let monitor = lifecycle.monitor(EndpointId(1), SessionId::new(EndpointId(1), 1));
        assert_eq!(lifecycle.state.lock().unwrap().monitors.len(), 1);

        drop(monitor);

        assert!(lifecycle.state.lock().unwrap().monitors.is_empty());
    }
}
