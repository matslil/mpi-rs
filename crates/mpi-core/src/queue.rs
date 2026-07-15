//! Bounded task queue with normal and priority FIFO placement.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

use crate::channel::{Sender, channel};
use crate::error::{RecvError, SendError};
use crate::message::{MessagePlacement, TaskMessage};
use crate::session::EndpointId;

/// Type-erased target for framework queue-space wakeups.
pub trait QueueSpaceWakeupTarget: Send + Sync {
    /// Return the task endpoint waiting for receiver-owned queue capacity.
    fn endpoint(&self) -> EndpointId;

    /// Try to deliver one framework-only queue-space wakeup to the waiting task.
    fn try_wake(&self) -> Result<(), SendError>;
}

struct QueueState<M> {
    priority: VecDeque<M>,
    normal: VecDeque<M>,
    reservations: HashMap<EndpointId, MessagePlacement>,
    waiters: VecDeque<(
        EndpointId,
        MessagePlacement,
        Arc<dyn QueueSpaceWakeupTarget>,
    )>,
    closed: bool,
}

impl<M> Default for QueueState<M> {
    fn default() -> Self {
        Self {
            priority: VecDeque::new(),
            normal: VecDeque::new(),
            reservations: HashMap::new(),
            waiters: VecDeque::new(),
            closed: false,
        }
    }
}

/// A bounded queue owned by a task.
///
/// The queue has one compile-time capacity shared by the normal and priority
/// internal FIFO queues. Message placement is read from `TaskMessage`, so the
/// sender cannot override the receiver's placement declaration.
pub struct TaskQueue<M, const N: usize>
where
    M: TaskMessage,
{
    state: Mutex<QueueState<M>>,
    available: Condvar,
    priority_reserved: usize,
}

/// Read-only diagnostic snapshot of a task queue.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TaskQueueSnapshot {
    /// Compile-time total queue capacity.
    pub capacity: usize,

    /// Total number of queued messages.
    pub total_len: usize,

    /// Number of queued priority messages.
    pub priority_len: usize,

    /// Number of queued normal messages.
    pub normal_len: usize,

    /// Number of queue slots reserved for priority messages.
    pub priority_reserved: usize,

    /// Number of queue slots currently reserved for waiting senders.
    pub reserved_len: usize,

    /// Number of senders waiting for a receiver-owned reservation.
    pub waiting_senders: usize,

    /// Whether the queue has been closed.
    pub closed: bool,
}

impl<M, const N: usize> Default for TaskQueue<M, N>
where
    M: TaskMessage,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<M, const N: usize> TaskQueue<M, N>
where
    M: TaskMessage,
{
    /// Create an empty open queue.
    #[must_use]
    pub fn new() -> Self {
        Self::with_priority_reserved(1)
    }

    /// Create an empty open queue with an explicit priority reserve.
    #[must_use]
    pub fn with_priority_reserved(priority_reserved: usize) -> Self {
        Self {
            state: Mutex::new(QueueState::default()),
            available: Condvar::new(),
            priority_reserved: priority_reserved.min(N),
        }
    }

    /// Return the compile-time capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Return the total number of queued messages.
    #[must_use]
    pub fn len(&self) -> usize {
        let state = self.state.lock().expect("queue mutex poisoned");
        state.priority.len() + state.normal.len()
    }

    /// Return `true` when no messages are queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return `true` when the shared capacity is exhausted.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len() == N
    }

    /// Return the number of queue slots reserved for priority messages.
    #[must_use]
    pub const fn priority_reserved(&self) -> usize {
        self.priority_reserved
    }

    /// Return a read-only diagnostic snapshot of the queue state.
    #[must_use]
    pub fn snapshot(&self) -> TaskQueueSnapshot {
        let state = self.state.lock().expect("queue mutex poisoned");
        let priority_len = state.priority.len();
        let normal_len = state.normal.len();
        TaskQueueSnapshot {
            capacity: N,
            total_len: priority_len + normal_len,
            priority_len,
            normal_len,
            priority_reserved: self.priority_reserved,
            reserved_len: state.reservations.len(),
            waiting_senders: state.waiters.len(),
            closed: state.closed,
        }
    }

    /// Close the queue and wake blocked receivers.
    pub fn close(&self) {
        let mut state = self.state.lock().expect("queue mutex poisoned");
        state.closed = true;
        self.available.notify_all();
    }

    /// Try to enqueue one message from an external sender.
    ///
    /// This method never blocks. It returns `SendError::QueueFull` if no
    /// unreserved capacity is available for the message placement.
    pub fn try_send(&self, message: M) -> Result<(), SendError> {
        let mut state = self.state.lock().expect("queue mutex poisoned");

        if state.closed {
            return Err(SendError::TaskStopped);
        }

        if !self.has_unreserved_capacity(&state, message.placement()) {
            return Err(SendError::QueueFull);
        }

        Self::push_message(&mut state, message);

        drop(state);
        self.available.notify_one();
        Ok(())
    }

    /// Try to enqueue one message from a task endpoint.
    ///
    /// When capacity is exhausted, the queue records the sender for a future
    /// receiver-owned reservation and returns `SendError::QueueFull`.
    pub fn try_send_from(
        &self,
        sender: Arc<dyn QueueSpaceWakeupTarget>,
        message: M,
    ) -> Result<(), SendError> {
        let mut state = self.state.lock().expect("queue mutex poisoned");
        let sender_endpoint = sender.endpoint();

        if state.closed {
            return Err(SendError::TaskStopped);
        }

        if state.reservations.remove(&sender_endpoint).is_some()
            || self.has_unreserved_capacity(&state, message.placement())
        {
            Self::push_message(&mut state, message);
            drop(state);
            self.available.notify_one();
            return Ok(());
        }

        self.register_waiter(&mut state, sender, message.placement());
        Err(SendError::QueueFull)
    }

    /// Enqueue one message from a task endpoint, waiting until the receiving
    /// queue grants capacity by sending a framework wakeup message.
    pub fn send_waiting_from(
        &self,
        sender: Arc<dyn QueueSpaceWakeupTarget>,
        message: M,
    ) -> Result<(), SendError> {
        let mut state = self.state.lock().expect("queue mutex poisoned");
        let mut message = Some(message);
        let sender_endpoint = sender.endpoint();
        let (wakeup_tx, wakeup_rx) = channel();
        let sender = Arc::new(QueueSpaceWakeupAttempt {
            target: sender,
            wakeup: Mutex::new(Some(wakeup_tx)),
        });

        loop {
            if state.closed {
                return Err(SendError::TaskStopped);
            }

            let placement = message
                .as_ref()
                .expect("message missing before successful send")
                .placement();
            if state.reservations.remove(&sender_endpoint).is_some()
                || self.has_unreserved_capacity(&state, placement)
            {
                Self::push_message(
                    &mut state,
                    message
                        .take()
                        .expect("message missing before successful send"),
                );
                drop(state);
                self.available.notify_one();
                return Ok(());
            }

            self.register_waiter(&mut state, sender.clone(), placement);
            drop(state);

            match wakeup_rx.recv() {
                Ok(()) => {}
                Err(_) => return Err(SendError::TaskStopped),
            }

            state = self.state.lock().expect("queue mutex poisoned");
        }
    }

    /// Receive one message, blocking until a message is available or the queue closes.
    pub fn recv(&self) -> Result<M, RecvError> {
        let mut state = self.state.lock().expect("queue mutex poisoned");

        loop {
            if let Some(message) = state.priority.pop_front() {
                self.grant_reservations(&mut state);
                drop(state);
                self.available.notify_all();
                return Ok(message);
            }

            if let Some(message) = state.normal.pop_front() {
                self.grant_reservations(&mut state);
                drop(state);
                self.available.notify_all();
                return Ok(message);
            }

            if state.closed {
                return Err(RecvError::Closed);
            }

            state = self.available.wait(state).expect("queue mutex poisoned");
        }
    }

    /// Try to receive one message without blocking.
    #[must_use]
    pub fn try_recv(&self) -> Option<M> {
        let mut state = self.state.lock().expect("queue mutex poisoned");
        let message = state
            .priority
            .pop_front()
            .or_else(|| state.normal.pop_front());
        if message.is_some() {
            self.grant_reservations(&mut state);
            drop(state);
            self.available.notify_all();
        }
        message
    }

    fn push_message(state: &mut QueueState<M>, message: M) {
        match message.placement() {
            MessagePlacement::Normal => state.normal.push_back(message),
            MessagePlacement::Priority => state.priority.push_back(message),
        }
    }

    fn occupied_slots(state: &QueueState<M>) -> usize {
        state.priority.len() + state.normal.len()
    }

    fn committed_slots(state: &QueueState<M>) -> usize {
        Self::occupied_slots(state) + state.reservations.len()
    }

    fn has_unreserved_capacity(&self, state: &QueueState<M>, placement: MessagePlacement) -> bool {
        let committed = Self::committed_slots(state);
        match placement {
            MessagePlacement::Priority => committed < N,
            MessagePlacement::Normal => committed < N.saturating_sub(self.priority_reserved),
        }
    }

    fn register_waiter(
        &self,
        state: &mut QueueState<M>,
        sender: Arc<dyn QueueSpaceWakeupTarget>,
        placement: MessagePlacement,
    ) {
        let sender_endpoint = sender.endpoint();
        if state.reservations.contains_key(&sender_endpoint)
            || state
                .waiters
                .iter()
                .any(|(waiting_sender, _, _)| *waiting_sender == sender_endpoint)
        {
            return;
        }
        state
            .waiters
            .push_back((sender_endpoint, placement, sender));
        self.grant_reservations(state);
    }

    fn grant_reservations(&self, state: &mut QueueState<M>) {
        while let Some((sender, placement, target)) = state.waiters.front().cloned() {
            if !self.has_unreserved_capacity(state, placement) {
                break;
            }
            state.waiters.pop_front();
            state.reservations.insert(sender, placement);
            if target.try_wake().is_err() {
                state.reservations.remove(&sender);
                continue;
            }
        }
    }
}

struct QueueSpaceWakeupAttempt {
    target: Arc<dyn QueueSpaceWakeupTarget>,
    wakeup: Mutex<Option<Sender<()>>>,
}

impl QueueSpaceWakeupTarget for QueueSpaceWakeupAttempt {
    fn endpoint(&self) -> EndpointId {
        self.target.endpoint()
    }

    fn try_wake(&self) -> Result<(), SendError> {
        self.target.try_wake()?;
        if let Some(wakeup) = self
            .wakeup
            .lock()
            .expect("queue wakeup mutex poisoned")
            .take()
        {
            let _ = wakeup.send(());
        }
        Ok(())
    }
}
