//! Local `mpi` timeout server.
//!
//! The crate baseline is documented in `se-design-baseline.md`.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mpi::{
    MessagePlacement, QueueSpaceWakeupMessage, SendError, SessionId, TaskHandle, TaskMessage,
    TaskQueue,
};

/// Crate-owned monotonic time source.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Time;

impl Time {
    /// Return the current timeout-server monotonic instant.
    #[must_use]
    pub fn now() -> TimeoutInstant {
        TimeoutInstant(Instant::now())
    }
}

/// Absolute monotonic deadline used by timeout requests.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TimeoutInstant(Instant);

impl TimeoutInstant {
    /// Return the duration from this instant until `deadline`.
    #[must_use]
    pub fn duration_until(self, deadline: Self) -> Duration {
        deadline.0.saturating_duration_since(self.0)
    }

    /// Return whether this instant has reached or passed `deadline`.
    #[must_use]
    pub fn has_reached(self, deadline: Self) -> bool {
        self >= deadline
    }
}

impl std::ops::Add<Duration> for TimeoutInstant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl std::ops::AddAssign<Duration> for TimeoutInstant {
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs;
    }
}

impl std::ops::Sub<Duration> for TimeoutInstant {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self(self.0 - rhs)
    }
}

/// Opaque operation that delivers an expired timeout message.
///
/// The timeout server owns this object while the timeout is pending. It calls
/// `try_deliver` until delivery succeeds, a non-retryable error occurs, or the
/// server's local delivery wait bound expires.
pub trait TimeoutDelivery: Send + 'static {
    /// Attempt one delivery of the already-typed timeout message.
    fn try_deliver(&mut self) -> Result<(), SendError>;
}

impl<F> TimeoutDelivery for F
where
    F: FnMut() -> Result<(), SendError> + Send + 'static,
{
    fn try_deliver(&mut self) -> Result<(), SendError> {
        self()
    }
}

/// Timeout request accepted by the server.
pub struct TimeoutRequest {
    session_id: SessionId,
    deadline: TimeoutInstant,
    delivery: Box<dyn TimeoutDelivery>,
}

impl TimeoutRequest {
    /// Construct a timeout request with an opaque delivery operation.
    #[must_use]
    pub fn new<D>(session_id: SessionId, deadline: TimeoutInstant, delivery: D) -> Self
    where
        D: TimeoutDelivery,
    {
        Self {
            session_id,
            deadline,
            delivery: Box::new(delivery),
        }
    }

    /// Return the timeout request session.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Return the absolute monotonic timeout deadline.
    #[must_use]
    pub const fn deadline(&self) -> TimeoutInstant {
        self.deadline
    }
}

/// Best-effort timeout cancel message.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TimeoutCancel {
    session_id: SessionId,
}

impl TimeoutCancel {
    /// Construct a timeout cancel for a session.
    #[must_use]
    pub const fn new(session_id: SessionId) -> Self {
        Self { session_id }
    }

    /// Return the session to cancel.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }
}

enum TimeoutServerMessageKind {
    Request(TimeoutRequest),
    Cancel(TimeoutCancel),
    QueueSpaceWakeup,
}

/// Messages received by the timeout server task.
///
/// This is the timeout server's public task declaration. Clients construct
/// values with `TimeoutServerMessage::request` and
/// `TimeoutServerMessage::cancel`, then send them through the `mpi` task handle
/// returned by `spawn_timeout_server`.
pub struct TimeoutServerMessage {
    kind: TimeoutServerMessageKind,
}

impl TimeoutServerMessage {
    /// Construct a timeout request message.
    #[must_use]
    pub const fn request(request: TimeoutRequest) -> Self {
        Self {
            kind: TimeoutServerMessageKind::Request(request),
        }
    }

    /// Construct a priority timeout cancel message.
    #[must_use]
    pub const fn cancel(cancel: TimeoutCancel) -> Self {
        Self {
            kind: TimeoutServerMessageKind::Cancel(cancel),
        }
    }

    fn queue_space_wakeup_message() -> Self {
        Self {
            kind: TimeoutServerMessageKind::QueueSpaceWakeup,
        }
    }
}

impl TaskMessage for TimeoutServerMessage {
    fn placement(&self) -> MessagePlacement {
        match &self.kind {
            TimeoutServerMessageKind::Cancel(_) | TimeoutServerMessageKind::QueueSpaceWakeup => {
                MessagePlacement::Priority
            }
            TimeoutServerMessageKind::Request(_) => MessagePlacement::Normal,
        }
    }
}

impl QueueSpaceWakeupMessage for TimeoutServerMessage {
    fn queue_space_wakeup() -> Self {
        Self::queue_space_wakeup_message()
    }

    fn into_queue_space_wakeup(self) -> Result<mpi::QueueSpaceWakeup, Self> {
        match self.kind {
            TimeoutServerMessageKind::QueueSpaceWakeup => Ok(mpi::QueueSpaceWakeup),
            other => Err(Self { kind: other }),
        }
    }
}

/// Error returned by timeout-server operations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TimeoutServerError {
    /// A request used a `SessionId` that already has an active timeout.
    DuplicateSession(SessionId),

    /// The timeout-server queue rejected a message.
    Send(SendError),

    /// Expiry delivery did not complete before the local delivery wait bound.
    DeliveryTimedOut(SessionId),

    /// The delivery target stopped before the timeout could be delivered.
    DeliveryTargetStopped(SessionId),
}

impl fmt::Display for TimeoutServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSession(session_id) => {
                write!(f, "timeout session {session_id} is already active")
            }
            Self::Send(error) => write!(f, "timeout server message could not be sent: {error}"),
            Self::DeliveryTimedOut(session_id) => {
                write!(f, "timeout delivery for {session_id} timed out")
            }
            Self::DeliveryTargetStopped(session_id) => {
                write!(f, "timeout delivery target for {session_id} stopped")
            }
        }
    }
}

impl std::error::Error for TimeoutServerError {}

impl From<SendError> for TimeoutServerError {
    fn from(error: SendError) -> Self {
        Self::Send(error)
    }
}

/// Timeout-server runtime configuration.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TimeoutServerConfig {
    /// Maximum time the timeout server waits while retrying queue-full expiry delivery.
    pub delivery_timeout: Duration,

    /// Local sleep duration used between queue polling and delivery retries.
    pub poll_interval: Duration,

    /// Number of priority slots reserved in the timeout-server queue.
    pub priority_reserved: usize,
}

impl Default for TimeoutServerConfig {
    fn default() -> Self {
        Self {
            delivery_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_millis(1),
            priority_reserved: 1,
        }
    }
}

/// Join handle for the timeout server thread.
pub struct TimeoutServerRuntime {
    join: std::thread::JoinHandle<Result<(), TimeoutServerError>>,
}

impl TimeoutServerRuntime {
    /// Wait for the timeout server to stop.
    pub fn join(self) -> std::thread::Result<Result<(), TimeoutServerError>> {
        self.join.join()
    }
}

struct ActiveTimeout {
    deadline: TimeoutInstant,
    delivery: Box<dyn TimeoutDelivery>,
}

/// Spawn a timeout server with default configuration.
pub fn spawn_timeout_server<const N: usize>()
-> (TaskHandle<TimeoutServerMessage, N>, TimeoutServerRuntime) {
    spawn_timeout_server_with_config(TimeoutServerConfig::default())
}

/// Spawn a timeout server with explicit configuration.
pub fn spawn_timeout_server_with_config<const N: usize>(
    config: TimeoutServerConfig,
) -> (TaskHandle<TimeoutServerMessage, N>, TimeoutServerRuntime) {
    let queue = Arc::new(
        TaskQueue::<TimeoutServerMessage, N>::with_priority_reserved(config.priority_reserved),
    );
    let handle = TaskHandle::new(queue);
    let worker_handle = handle.clone();
    let join = std::thread::spawn(move || run_server(worker_handle, config));
    (handle, TimeoutServerRuntime { join })
}

fn run_server<const N: usize>(
    handle: TaskHandle<TimeoutServerMessage, N>,
    config: TimeoutServerConfig,
) -> Result<(), TimeoutServerError> {
    let mut active = HashMap::<SessionId, ActiveTimeout>::new();
    let mut canceled = HashSet::<SessionId>::new();
    let mut deadlines = BinaryHeap::<Reverse<(TimeoutInstant, SessionId)>>::new();

    loop {
        while let Some(message) = handle.try_recv_message() {
            match message.kind {
                TimeoutServerMessageKind::Request(request) => {
                    if canceled.remove(&request.session_id) {
                        continue;
                    }
                    if active.contains_key(&request.session_id) {
                        return Err(TimeoutServerError::DuplicateSession(request.session_id));
                    }
                    deadlines.push(Reverse((request.deadline, request.session_id)));
                    active.insert(
                        request.session_id,
                        ActiveTimeout {
                            deadline: request.deadline,
                            delivery: request.delivery,
                        },
                    );
                }
                TimeoutServerMessageKind::Cancel(cancel) => {
                    if active.remove(&cancel.session_id).is_none() {
                        canceled.insert(cancel.session_id);
                    }
                }
                TimeoutServerMessageKind::QueueSpaceWakeup => {}
            }
        }

        if !handle.is_accepting() {
            return Ok(());
        }

        let now = Time::now();
        let mut delivered = false;
        while let Some(Reverse((deadline, session_id))) = deadlines.peek().copied() {
            if !now.has_reached(deadline) {
                break;
            }
            deadlines.pop();
            let Some(timeout) = active.remove(&session_id) else {
                continue;
            };
            if timeout.deadline != deadline {
                continue;
            }
            deliver_with_local_timeout(session_id, timeout.delivery, config)?;
            delivered = true;
        }

        if delivered {
            continue;
        }

        let sleep_for = deadlines
            .peek()
            .map(|Reverse((deadline, _))| now.duration_until(*deadline))
            .unwrap_or(config.poll_interval)
            .min(config.poll_interval);

        if sleep_for.is_zero() {
            std::thread::yield_now();
        } else {
            std::thread::sleep(sleep_for);
        }
    }
}

fn deliver_with_local_timeout(
    session_id: SessionId,
    mut delivery: Box<dyn TimeoutDelivery>,
    config: TimeoutServerConfig,
) -> Result<(), TimeoutServerError> {
    let delivery_deadline = Time::now() + config.delivery_timeout;
    loop {
        match delivery.try_deliver() {
            Ok(()) => return Ok(()),
            Err(SendError::QueueFull) => {
                if Time::now().has_reached(delivery_deadline) {
                    return Err(TimeoutServerError::DeliveryTimedOut(session_id));
                }
                std::thread::sleep(config.poll_interval);
            }
            Err(SendError::TaskStopped) => {
                return Err(TimeoutServerError::DeliveryTargetStopped(session_id));
            }
            Err(error) => return Err(TimeoutServerError::Send(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};

    use mpi::EndpointId;

    use super::*;

    fn session(sequence: u64) -> SessionId {
        SessionId::new(EndpointId(7), sequence)
    }

    fn request_with_sender(
        session_id: SessionId,
        delay: Duration,
        tx: mpsc::Sender<SessionId>,
    ) -> TimeoutRequest {
        TimeoutRequest::new(session_id, Time::now() + delay, move || {
            tx.send(session_id).map_err(|_| SendError::TaskStopped)
        })
    }

    #[test]
    fn tos_req_003_time_now_returns_timeout_instant() {
        let start = Time::now();
        let deadline = start + Duration::from_millis(1);

        assert!(deadline >= start);
        assert!(start.duration_until(deadline) <= Duration::from_millis(1));
    }

    #[test]
    fn tos_req_007_expiry_delivers_opaque_message() {
        let (server, runtime) = spawn_timeout_server::<8>();
        let (tx, rx) = mpsc::channel();
        let session_id = session(1);

        server
            .send_message(TimeoutServerMessage::request(request_with_sender(
                session_id,
                Duration::from_millis(5),
                tx,
            )))
            .unwrap();

        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), session_id);
        server.close();
        assert_eq!(runtime.join().unwrap(), Ok(()));
    }

    #[test]
    fn tos_req_009_cancel_discards_pending_timeout() {
        let (server, runtime) = spawn_timeout_server::<8>();
        let (tx, rx) = mpsc::channel();
        let session_id = session(2);

        server
            .send_message(TimeoutServerMessage::request(request_with_sender(
                session_id,
                Duration::from_millis(100),
                tx,
            )))
            .unwrap();
        server
            .send_message(TimeoutServerMessage::cancel(TimeoutCancel::new(session_id)))
            .unwrap();

        assert!(rx.recv_timeout(Duration::from_millis(150)).is_err());
        server.close();
        assert_eq!(runtime.join().unwrap(), Ok(()));
    }

    #[test]
    fn tos_req_008_duplicate_active_request_is_rejected() {
        let config = TimeoutServerConfig {
            delivery_timeout: Duration::from_millis(20),
            ..TimeoutServerConfig::default()
        };
        let (server, runtime) = spawn_timeout_server_with_config::<8>(config);
        let (tx, _rx) = mpsc::channel();
        let session_id = session(3);

        server
            .send_message(TimeoutServerMessage::request(request_with_sender(
                session_id,
                Duration::from_secs(10),
                tx.clone(),
            )))
            .unwrap();
        server
            .send_message(TimeoutServerMessage::request(request_with_sender(
                session_id,
                Duration::from_secs(10),
                tx,
            )))
            .unwrap();

        assert_eq!(
            runtime.join().unwrap(),
            Err(TimeoutServerError::DuplicateSession(session_id))
        );
    }

    #[test]
    fn tos_req_011_cancel_message_is_priority() {
        assert_eq!(
            TimeoutServerMessage::cancel(TimeoutCancel::new(session(4))).placement(),
            MessagePlacement::Priority
        );
    }

    #[test]
    fn tos_req_016_clients_send_protocol_messages_to_server() {
        let (server, runtime) = spawn_timeout_server::<8>();
        let (tx, rx) = mpsc::channel();
        let session_id = session(8);

        server
            .send_message(TimeoutServerMessage::request(request_with_sender(
                session_id,
                Duration::from_millis(5),
                tx,
            )))
            .unwrap();

        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), session_id);
        server.close();
        assert_eq!(runtime.join().unwrap(), Ok(()));
    }

    #[test]
    fn tos_req_013_queue_full_delivery_retries_until_success() {
        let config = TimeoutServerConfig {
            delivery_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_millis(1),
            priority_reserved: 1,
        };
        let (server, runtime) = spawn_timeout_server_with_config::<8>(config);
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_delivery = Arc::clone(&attempts);
        let (tx, rx) = mpsc::channel();
        let session_id = session(5);

        server
            .send_message(TimeoutServerMessage::request(TimeoutRequest::new(
                session_id,
                Time::now() + Duration::from_millis(5),
                move || {
                    let attempt = attempts_for_delivery.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        return Err(SendError::QueueFull);
                    }
                    tx.send(session_id).map_err(|_| SendError::TaskStopped)
                },
            )))
            .unwrap();

        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), session_id);
        assert!(attempts.load(Ordering::SeqCst) >= 2);
        server.close();
        assert_eq!(runtime.join().unwrap(), Ok(()));
    }

    #[test]
    fn tos_req_014_delivery_wait_is_locally_bounded() {
        let config = TimeoutServerConfig {
            delivery_timeout: Duration::from_millis(20),
            poll_interval: Duration::from_millis(1),
            priority_reserved: 1,
        };
        let (server, runtime) = spawn_timeout_server_with_config::<8>(config);
        let session_id = session(6);

        server
            .send_message(TimeoutServerMessage::request(TimeoutRequest::new(
                session_id,
                Time::now(),
                || Err(SendError::QueueFull),
            )))
            .unwrap();

        assert_eq!(
            runtime.join().unwrap(),
            Err(TimeoutServerError::DeliveryTimedOut(session_id))
        );
    }

    #[test]
    fn tos_req_006_delivery_payload_remains_opaque_to_server() {
        #[derive(Debug, Eq, PartialEq)]
        struct SenderSpecificPayload {
            value: u32,
        }

        let (server, runtime) = spawn_timeout_server::<8>();
        let observed = Arc::new(Mutex::new(None));
        let observed_for_delivery = Arc::clone(&observed);
        let payload = SenderSpecificPayload { value: 42 };

        server
            .send_message(TimeoutServerMessage::request(TimeoutRequest::new(
                session(7),
                Time::now(),
                move || {
                    *observed_for_delivery.lock().unwrap() = Some(payload.value);
                    Ok(())
                },
            )))
            .unwrap();

        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(*observed.lock().unwrap(), Some(42));
        server.close();
        assert_eq!(runtime.join().unwrap(), Ok(()));
    }
}
