//! Task endpoint termination state and infrastructure message subscriptions.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use crate::error::SendError;
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

/// Infrastructure-generated task termination message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskTerminated {
    /// Session associated with a call, stream, or supervision subscription.
    pub session_id: SessionId,
    /// Endpoint that terminated.
    pub target: EndpointId,
    /// Recorded reason for termination.
    pub termination: TaskTermination,
    /// Whether this event belongs to an explicit supervision subscription.
    pub supervised: bool,
}

/// Generated task message enums capable of carrying termination messages.
pub trait TaskTerminationMessage: Sized {
    /// Wrap an infrastructure termination event.
    fn task_terminated(event: TaskTerminated) -> Self;
}

/// Type-erased target for an infrastructure termination message.
pub trait TaskTerminationTarget: Send + Sync {
    /// Enqueue one termination message at the receiving task.
    fn send_task_terminated(&self, event: TaskTerminated) -> Result<(), SendError>;
}

type TerminationDelivery = Box<dyn FnOnce(TaskTermination) + Send + 'static>;

struct LifecycleState {
    termination: Option<TaskTermination>,
    monitors: HashMap<u64, TerminationDelivery>,
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
        for deliver in monitors.into_values() {
            deliver(termination.clone());
        }
        true
    }

    pub(crate) fn monitor(self: &Arc<Self>, deliver: TerminationDelivery) -> TaskMonitor {
        let id = self.next_monitor.fetch_add(1, Ordering::Relaxed);
        let mut deliver = Some(deliver);
        let immediate = {
            let mut state = self
                .state
                .lock()
                .expect("endpoint lifecycle mutex poisoned");
            match state.termination.clone() {
                Some(termination) => Some(termination),
                None => {
                    state.monitors.insert(id, deliver.take().unwrap());
                    None
                }
            }
        };
        if let Some(termination) = immediate {
            deliver.take().unwrap()(termination);
            TaskMonitor {
                target: Weak::new(),
                id: None,
            }
        } else {
            TaskMonitor {
                target: Arc::downgrade(self),
                id: Some(id),
            }
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
}

impl Drop for TaskMonitor {
    fn drop(&mut self) {
        if let (Some(target), Some(id)) = (self.target.upgrade(), self.id.take()) {
            target.cancel(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn mpi_req_140_141_registration_observes_exactly_one_termination() {
        let lifecycle = EndpointLifecycle::new();
        let deliveries = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&deliveries);
        let _monitor = lifecycle.monitor(Box::new(move |_| {
            observed.fetch_add(1, Ordering::Relaxed);
        }));

        assert!(lifecycle.terminate(TaskTermination::Completed));
        assert!(!lifecycle.terminate(TaskTermination::Stopped));
        assert_eq!(deliveries.load(Ordering::Relaxed), 1);

        let observed = Arc::clone(&deliveries);
        let _late_monitor = lifecycle.monitor(Box::new(move |_| {
            observed.fetch_add(1, Ordering::Relaxed);
        }));
        assert_eq!(deliveries.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn mpi_req_142_dropping_monitor_cancels_delivery() {
        let lifecycle = EndpointLifecycle::new();
        let deliveries = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&deliveries);
        let monitor = lifecycle.monitor(Box::new(move |_| {
            observed.fetch_add(1, Ordering::Relaxed);
        }));
        drop(monitor);

        lifecycle.terminate(TaskTermination::Stopped);
        assert_eq!(deliveries.load(Ordering::Relaxed), 0);
    }
}
