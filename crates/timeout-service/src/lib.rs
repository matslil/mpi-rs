//! Local `mpi` timeout service.
//!
//! The crate baseline is documented in `se-design-baseline.md`.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mpi::{
    MessagePlacement, QueueSpaceWakeupMessage, SendError, SessionId, TaskHandle, TaskMessage,
    TaskQueue, protocol,
};

/// Crate-owned monotonic time source.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Time;

impl Time {
    /// Return the current timeout-service monotonic instant.
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

/// Operation that delivers a payload-free timeout-occurred event.
///
/// The timeout service owns this object while the timeout is pending. It calls
/// `try_deliver` until delivery succeeds, a non-retryable error occurs, or the
/// service's local delivery wait bound expires.
pub trait TimeoutDelivery: Send + 'static {
    /// Attempt one delivery of the timeout-occurred event for `session_id`.
    fn try_deliver(&mut self, session_id: SessionId) -> Result<(), SendError>;
}

impl<F> TimeoutDelivery for F
where
    F: FnMut(SessionId) -> Result<(), SendError> + Send + 'static,
{
    fn try_deliver(&mut self, session_id: SessionId) -> Result<(), SendError> {
        self(session_id)
    }
}

/// Timeout request accepted by the service.
pub struct TimeoutRequest {
    session_id: SessionId,
    deadline: TimeoutInstant,
    delivery: Box<dyn TimeoutDelivery>,
}

impl TimeoutRequest {
    /// Construct a timeout request with a payload-free timeout-event delivery operation.
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

// Message protocol accepted by the timeout service task.
protocol! {
    pub protocol TimeoutServiceProtocolV1 {
        event request(TimeoutRequest);
        event cancel(TimeoutCancel);
    }
}

/// Messages received by the timeout service task.
enum TimeoutServiceMessage {
    /// Schedule a timeout.
    Request(TimeoutRequest),

    /// Cancel a timeout. This is priority by receiver declaration.
    Cancel(TimeoutCancel),

    /// Framework-only queue-space wakeup.
    QueueSpaceWakeup,

    /// Stop the timeout service.
    Stop,
}

impl TaskMessage for TimeoutServiceMessage {
    fn placement(&self) -> MessagePlacement {
        match self {
            Self::Cancel(_) | Self::QueueSpaceWakeup | Self::Stop => MessagePlacement::Priority,
            Self::Request(_) => MessagePlacement::Normal,
        }
    }
}

impl QueueSpaceWakeupMessage for TimeoutServiceMessage {
    fn queue_space_wakeup() -> Self {
        Self::QueueSpaceWakeup
    }

    fn into_queue_space_wakeup(self) -> Result<mpi::QueueSpaceWakeup, Self> {
        match self {
            Self::QueueSpaceWakeup => Ok(mpi::QueueSpaceWakeup),
            other => Err(other),
        }
    }
}

/// Error returned by timeout-service operations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TimeoutServiceError {
    /// A request used a `SessionId` that already has an active timeout.
    DuplicateSession(SessionId),

    /// The timeout-service queue rejected a message.
    Send(SendError),

    /// Expiry delivery did not complete before the local delivery wait bound.
    DeliveryTimedOut(SessionId),

    /// The delivery target stopped before the timeout could be delivered.
    DeliveryTargetStopped(SessionId),

    /// The timeout service task thread panicked before shutdown completed.
    ThreadPanicked,
}

impl fmt::Display for TimeoutServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSession(session_id) => {
                write!(f, "timeout session {session_id} is already active")
            }
            Self::Send(error) => write!(f, "timeout service message could not be sent: {error}"),
            Self::DeliveryTimedOut(session_id) => {
                write!(f, "timeout delivery for {session_id} timed out")
            }
            Self::DeliveryTargetStopped(session_id) => {
                write!(f, "timeout delivery target for {session_id} stopped")
            }
            Self::ThreadPanicked => f.write_str("timeout service thread panicked"),
        }
    }
}

impl std::error::Error for TimeoutServiceError {}

impl From<SendError> for TimeoutServiceError {
    fn from(error: SendError) -> Self {
        Self::Send(error)
    }
}

/// Timeout-service runtime configuration.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TimeoutServiceConfig {
    /// Maximum time the timeout service waits while retrying queue-full expiry delivery.
    pub delivery_timeout: Duration,

    /// Local sleep duration used between queue polling and delivery retries.
    pub poll_interval: Duration,

    /// Number of priority slots reserved in the timeout-service queue.
    pub priority_reserved: usize,
}

impl Default for TimeoutServiceConfig {
    fn default() -> Self {
        Self {
            delivery_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_millis(1),
            priority_reserved: 1,
        }
    }
}

struct TimeoutServiceHandle<const N: usize> {
    handle: TaskHandle<TimeoutServiceMessage, N>,
}

impl<const N: usize> Clone for TimeoutServiceHandle<N> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
        }
    }
}

impl<const N: usize> TimeoutServiceHandle<N> {
    fn request(&self, request: TimeoutRequest) -> Result<(), SendError> {
        self.handle
            .send_message(TimeoutServiceMessage::Request(request))
    }

    fn cancel(&self, cancel: TimeoutCancel) -> Result<(), SendError> {
        self.handle
            .send_message(TimeoutServiceMessage::Cancel(cancel))
    }

    fn stop(&self) -> Result<(), TimeoutServiceError> {
        self.handle
            .send_message(TimeoutServiceMessage::Stop)
            .map_err(TimeoutServiceError::Send)
    }

    const fn task_handle(&self) -> &TaskHandle<TimeoutServiceMessage, N> {
        &self.handle
    }
}

/// Timeout service message endpoint.
pub struct TimeoutServiceEndpoint<const N: usize> {
    handle: TimeoutServiceHandle<N>,
}

impl<const N: usize> TimeoutServiceEndpoint<N> {
    fn new(handle: TimeoutServiceHandle<N>) -> Self {
        Self { handle }
    }
}

impl<const N: usize> TimeoutServiceProtocolV1::request::Target for &TimeoutServiceEndpoint<N> {
    fn request(
        &self,
        _ctx: &mut impl mpi::TaskScope,
        request: TimeoutRequest,
    ) -> Result<(), SendError> {
        self.handle.request(request)
    }

    fn request_blocking(&self, request: TimeoutRequest) -> Result<(), SendError> {
        self.handle.request(request)
    }
}

impl<const N: usize> TimeoutServiceProtocolV1::cancel::Target for &TimeoutServiceEndpoint<N> {
    fn cancel(
        &self,
        _ctx: &mut impl mpi::TaskScope,
        cancel: TimeoutCancel,
    ) -> Result<(), SendError> {
        self.handle.cancel(cancel)
    }

    fn cancel_blocking(&self, cancel: TimeoutCancel) -> Result<(), SendError> {
        self.handle.cancel(cancel)
    }
}

struct TimeoutServiceInner<const N: usize> {
    endpoint: TimeoutServiceEndpoint<N>,
    join: Mutex<Option<std::thread::JoinHandle<Result<(), TimeoutServiceError>>>>,
}

impl<const N: usize> TimeoutServiceInner<N> {
    fn stop_and_join(&self) -> Result<(), TimeoutServiceError> {
        let Some(join) = self
            .join
            .lock()
            .expect("timeout service join lock poisoned")
            .take()
        else {
            return Ok(());
        };
        match self.endpoint.handle.stop() {
            Ok(()) | Err(TimeoutServiceError::Send(SendError::TaskStopped)) => {}
            Err(error) => return Err(error),
        }
        join.join()
            .map_err(|_| TimeoutServiceError::ThreadPanicked)?
    }
}

impl<const N: usize> Drop for TimeoutServiceInner<N> {
    fn drop(&mut self) {
        if let Err(error) = self.stop_and_join() {
            panic!("timeout service failed to stop cleanly: {error}");
        }
    }
}

/// Owning timeout service instance.
///
/// Clones share ownership of the same running timeout task. Dropping the final
/// clone stops the task and waits for clean termination.
pub struct TimeoutServiceInstance<const N: usize> {
    inner: Arc<TimeoutServiceInner<N>>,
}

impl<const N: usize> Clone for TimeoutServiceInstance<N> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<const N: usize> TimeoutServiceInstance<N> {
    /// Return the timeout protocol binding for this service instance.
    #[must_use]
    pub fn protocol(&self) -> TimeoutServiceProtocolV1::Binding<&TimeoutServiceEndpoint<N>> {
        TimeoutServiceProtocolV1::bind(&self.inner.endpoint)
    }

    #[cfg(test)]
    fn join_for_test(&self) -> Result<(), TimeoutServiceError> {
        let Some(join) = self
            .inner
            .join
            .lock()
            .expect("timeout service join lock poisoned")
            .take()
        else {
            return Ok(());
        };
        join.join()
            .map_err(|_| TimeoutServiceError::ThreadPanicked)?
    }
}

struct ActiveTimeout {
    deadline: TimeoutInstant,
    delivery: Box<dyn TimeoutDelivery>,
}

/// Start a timeout service with default configuration.
pub fn start_timeout_service<const N: usize>() -> TimeoutServiceInstance<N> {
    start_timeout_service_with_config(TimeoutServiceConfig::default())
}

/// Start a timeout service with explicit configuration.
pub fn start_timeout_service_with_config<const N: usize>(
    config: TimeoutServiceConfig,
) -> TimeoutServiceInstance<N> {
    let queue = Arc::new(
        TaskQueue::<TimeoutServiceMessage, N>::with_priority_reserved(config.priority_reserved),
    );
    let handle = TimeoutServiceHandle {
        handle: TaskHandle::new(queue),
    };
    let worker_handle = handle.clone();
    let join = std::thread::spawn(move || run_service(worker_handle, config));
    TimeoutServiceInstance {
        inner: Arc::new(TimeoutServiceInner {
            endpoint: TimeoutServiceEndpoint::new(handle),
            join: Mutex::new(Some(join)),
        }),
    }
}

fn run_service<const N: usize>(
    handle: TimeoutServiceHandle<N>,
    config: TimeoutServiceConfig,
) -> Result<(), TimeoutServiceError> {
    let mut active = HashMap::<SessionId, ActiveTimeout>::new();
    let mut canceled = HashSet::<SessionId>::new();
    let mut deadlines = BinaryHeap::<Reverse<(TimeoutInstant, SessionId)>>::new();

    loop {
        while let Some(message) = handle.task_handle().try_recv_message() {
            match message {
                TimeoutServiceMessage::Request(request) => {
                    if canceled.remove(&request.session_id) {
                        continue;
                    }
                    if active.contains_key(&request.session_id) {
                        return Err(TimeoutServiceError::DuplicateSession(request.session_id));
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
                TimeoutServiceMessage::Cancel(cancel) => {
                    if active.remove(&cancel.session_id).is_none() {
                        canceled.insert(cancel.session_id);
                    }
                }
                TimeoutServiceMessage::QueueSpaceWakeup => {}
                TimeoutServiceMessage::Stop => {
                    handle.task_handle().close();
                    return Ok(());
                }
            }
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
    config: TimeoutServiceConfig,
) -> Result<(), TimeoutServiceError> {
    let delivery_deadline = Time::now() + config.delivery_timeout;
    loop {
        match delivery.try_deliver(session_id) {
            Ok(()) => return Ok(()),
            Err(SendError::QueueFull) => {
                if Time::now().has_reached(delivery_deadline) {
                    return Err(TimeoutServiceError::DeliveryTimedOut(session_id));
                }
                std::thread::sleep(config.poll_interval);
            }
            Err(SendError::TaskStopped) => {
                return Err(TimeoutServiceError::DeliveryTargetStopped(session_id));
            }
            Err(error) => return Err(TimeoutServiceError::Send(error)),
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
        TimeoutRequest::new(session_id, Time::now() + delay, move |delivered| {
            tx.send(delivered).map_err(|_| SendError::TaskStopped)
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
    fn tos_req_007_expiry_delivers_timeout_occurred_event() {
        let service = start_timeout_service::<8>();
        let (tx, rx) = mpsc::channel();
        let session_id = session(1);

        service
            .protocol()
            .request_blocking(request_with_sender(
                session_id,
                Duration::from_millis(5),
                tx,
            ))
            .unwrap();

        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), session_id);
    }

    #[test]
    fn tos_req_009_cancel_discards_pending_timeout() {
        let service = start_timeout_service::<8>();
        let (tx, rx) = mpsc::channel();
        let session_id = session(2);

        service
            .protocol()
            .request_blocking(request_with_sender(
                session_id,
                Duration::from_millis(100),
                tx,
            ))
            .unwrap();
        service
            .protocol()
            .cancel_blocking(TimeoutCancel::new(session_id))
            .unwrap();

        assert!(rx.recv_timeout(Duration::from_millis(150)).is_err());
    }

    #[test]
    fn tos_req_008_duplicate_active_request_is_rejected() {
        let config = TimeoutServiceConfig {
            delivery_timeout: Duration::from_millis(20),
            ..TimeoutServiceConfig::default()
        };
        let service = start_timeout_service_with_config::<8>(config);
        let (tx, _rx) = mpsc::channel();
        let session_id = session(3);

        service
            .protocol()
            .request_blocking(request_with_sender(
                session_id,
                Duration::from_secs(10),
                tx.clone(),
            ))
            .unwrap();
        service
            .protocol()
            .request_blocking(request_with_sender(session_id, Duration::from_secs(10), tx))
            .unwrap();

        assert_eq!(
            service.join_for_test(),
            Err(TimeoutServiceError::DuplicateSession(session_id))
        );
    }

    #[test]
    fn tos_req_011_cancel_message_is_priority() {
        assert_eq!(
            TimeoutServiceMessage::Cancel(TimeoutCancel::new(session(4))).placement(),
            MessagePlacement::Priority
        );
    }

    #[test]
    fn tos_req_013_queue_full_delivery_retries_until_success() {
        let config = TimeoutServiceConfig {
            delivery_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_millis(1),
            priority_reserved: 1,
        };
        let service = start_timeout_service_with_config::<8>(config);
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_delivery = Arc::clone(&attempts);
        let (tx, rx) = mpsc::channel();
        let session_id = session(5);

        service
            .protocol()
            .request_blocking(TimeoutRequest::new(
                session_id,
                Time::now() + Duration::from_millis(5),
                move |delivered| {
                    let attempt = attempts_for_delivery.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        return Err(SendError::QueueFull);
                    }
                    tx.send(delivered).map_err(|_| SendError::TaskStopped)
                },
            ))
            .unwrap();

        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), session_id);
        assert!(attempts.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn tos_req_014_delivery_wait_is_locally_bounded() {
        let config = TimeoutServiceConfig {
            delivery_timeout: Duration::from_millis(20),
            poll_interval: Duration::from_millis(1),
            priority_reserved: 1,
        };
        let service = start_timeout_service_with_config::<8>(config);
        let session_id = session(6);

        service
            .protocol()
            .request_blocking(TimeoutRequest::new(session_id, Time::now(), |_| {
                Err(SendError::QueueFull)
            }))
            .unwrap();

        assert_eq!(
            service.join_for_test(),
            Err(TimeoutServiceError::DeliveryTimedOut(session_id))
        );
    }

    #[test]
    fn tos_req_006_timeout_occurred_event_contains_only_session_id() {
        let service = start_timeout_service::<8>();
        let observed = Arc::new(Mutex::new(None));
        let observed_for_delivery = Arc::clone(&observed);
        let session_id = session(7);

        service
            .protocol()
            .request_blocking(TimeoutRequest::new(
                session_id,
                Time::now(),
                move |delivered| {
                    *observed_for_delivery.lock().unwrap() = Some(delivered);
                    Ok(())
                },
            ))
            .unwrap();

        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(*observed.lock().unwrap(), Some(session_id));
    }

    #[test]
    fn tos_req_016_service_instance_final_drop_stops_task() {
        let service = start_timeout_service::<8>();
        let clone = service.clone();
        drop(service);

        let (tx, rx) = mpsc::channel();
        let session_id = session(8);
        clone
            .protocol()
            .request_blocking(request_with_sender(
                session_id,
                Duration::from_millis(5),
                tx,
            ))
            .unwrap();

        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), session_id);
        drop(clone);
    }
}
