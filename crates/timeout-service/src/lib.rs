//! Local `mpi` timeout service.
//!
//! The crate baseline is documented in `se-design-baseline.md`.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mpi::{
    CanReceive, MessagePlacement, MessageTarget, QueueSpaceWakeupMessage, SendError, SessionId,
    TaskHandle, TaskMessage, TaskRuntime, spawn_task,
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

/// Payload-free event emitted when a requested timeout occurs.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TimeoutOccurred {
    session_id: SessionId,
}

impl TimeoutOccurred {
    /// Return the timeout session that expired.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }
}

/// Timeout request accepted by the service.
pub struct TimeoutRequest {
    session_id: SessionId,
    deadline: TimeoutInstant,
    sender: Arc<dyn MessageTarget<TimeoutOccurred>>,
}

impl TimeoutRequest {
    /// Construct a timeout request using the requesting task as its return endpoint.
    #[must_use]
    pub fn new<C>(ctx: &C, session_id: SessionId, deadline: TimeoutInstant) -> Self
    where
        C: mpi::TaskScope,
        C::Message: CanReceive<TimeoutOccurred>,
    {
        Self {
            session_id,
            deadline,
            sender: ctx.message_target::<TimeoutOccurred>(),
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

    #[cfg(test)]
    fn with_target(
        session_id: SessionId,
        deadline: TimeoutInstant,
        sender: Arc<dyn MessageTarget<TimeoutOccurred>>,
    ) -> Self {
        Self {
            session_id,
            deadline,
            sender,
        }
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

/// Task-scoped timeout service protocol.
pub struct TimeoutServiceProtocolV1;

/// Protocol instance bound to one timeout service endpoint.
pub struct TimeoutServiceBinding<'a, const N: usize> {
    endpoint: &'a TimeoutServiceEndpoint<N>,
}

impl<const N: usize> TimeoutServiceBinding<'_, N> {
    /// Schedule a timeout from task scope.
    pub fn request(
        &self,
        _ctx: &mut impl mpi::TaskScope,
        request: TimeoutRequest,
    ) -> Result<(), SendError> {
        self.endpoint.handle.request(request)
    }

    /// Cancel a timeout from task scope.
    pub fn cancel(
        &self,
        _ctx: &mut impl mpi::TaskScope,
        cancel: TimeoutCancel,
    ) -> Result<(), SendError> {
        self.endpoint.handle.cancel(cancel)
    }
}

/// Messages received by the timeout service task.
enum TimeoutServiceMessage {
    /// Start the timeout service task.
    Start,

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
            Self::Start | Self::Cancel(_) | Self::QueueSpaceWakeup | Self::Stop => {
                MessagePlacement::Priority
            }
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

struct TimeoutServiceInner<const N: usize> {
    endpoint: TimeoutServiceEndpoint<N>,
    join: Mutex<Option<TaskRuntime<Result<(), TimeoutServiceError>>>>,
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
    pub fn protocol(&self) -> TimeoutServiceBinding<'_, N> {
        TimeoutServiceBinding {
            endpoint: &self.inner.endpoint,
        }
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

    #[cfg(test)]
    fn request_for_test(&self, request: TimeoutRequest) -> Result<(), SendError> {
        self.inner.endpoint.handle.request(request)
    }

    #[cfg(test)]
    fn cancel_for_test(&self, cancel: TimeoutCancel) -> Result<(), SendError> {
        self.inner.endpoint.handle.cancel(cancel)
    }
}

struct ActiveTimeout {
    deadline: TimeoutInstant,
    sender: Arc<dyn MessageTarget<TimeoutOccurred>>,
}

/// Start a timeout service with default configuration.
pub fn start_timeout_service<const N: usize>() -> TimeoutServiceInstance<N> {
    start_timeout_service_with_config(TimeoutServiceConfig::default())
}

/// Start a timeout service with explicit configuration.
pub fn start_timeout_service_with_config<const N: usize>(
    config: TimeoutServiceConfig,
) -> TimeoutServiceInstance<N> {
    let (task_handle, runtime) = spawn_task::<TimeoutServiceMessage, _, _, N>(
        TimeoutServiceMessage::Start,
        config.priority_reserved,
        move |task_handle| {
            let worker_handle = TimeoutServiceHandle {
                handle: task_handle,
            };
            run_service(worker_handle, config)
        },
    )
    .expect("timeout service queue must accept its start message");
    let handle = TimeoutServiceHandle {
        handle: task_handle,
    };
    TimeoutServiceInstance {
        inner: Arc::new(TimeoutServiceInner {
            endpoint: TimeoutServiceEndpoint::new(handle),
            join: Mutex::new(Some(runtime)),
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
                TimeoutServiceMessage::Start => {}
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
                            sender: request.sender,
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
            deliver_with_local_timeout(session_id, timeout.sender, config)?;
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
    sender: Arc<dyn MessageTarget<TimeoutOccurred>>,
    config: TimeoutServiceConfig,
) -> Result<(), TimeoutServiceError> {
    let delivery_deadline = Time::now() + config.delivery_timeout;
    loop {
        match sender.send(TimeoutOccurred { session_id }) {
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
    use std::sync::{Arc, Mutex};

    use mpi::EndpointId;

    use super::*;

    fn session(sequence: u64) -> SessionId {
        SessionId::new(EndpointId(7), sequence)
    }

    struct RecordingTarget {
        observed: Arc<Mutex<Vec<SessionId>>>,
    }

    impl MessageTarget<TimeoutOccurred> for RecordingTarget {
        fn send(&self, occurred: TimeoutOccurred) -> Result<(), SendError> {
            self.observed.lock().unwrap().push(occurred.session_id());
            Ok(())
        }
    }

    struct RetryTarget {
        attempts: Arc<AtomicUsize>,
        observed: Arc<Mutex<Vec<SessionId>>>,
    }

    impl MessageTarget<TimeoutOccurred> for RetryTarget {
        fn send(&self, occurred: TimeoutOccurred) -> Result<(), SendError> {
            if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(SendError::QueueFull);
            }
            self.observed.lock().unwrap().push(occurred.session_id());
            Ok(())
        }
    }

    struct FullTarget;

    impl MessageTarget<TimeoutOccurred> for FullTarget {
        fn send(&self, _occurred: TimeoutOccurred) -> Result<(), SendError> {
            Err(SendError::QueueFull)
        }
    }

    fn request_with_target(
        session_id: SessionId,
        delay: Duration,
        observed: Arc<Mutex<Vec<SessionId>>>,
    ) -> TimeoutRequest {
        TimeoutRequest::with_target(
            session_id,
            Time::now() + delay,
            Arc::new(RecordingTarget { observed }),
        )
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
        let observed = Arc::new(Mutex::new(Vec::new()));
        let session_id = session(1);

        service
            .request_for_test(request_with_target(
                session_id,
                Duration::from_millis(5),
                Arc::clone(&observed),
            ))
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(1);
        while observed.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(*observed.lock().unwrap(), vec![session_id]);
    }

    #[test]
    fn tos_req_009_cancel_discards_pending_timeout() {
        let service = start_timeout_service::<8>();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let session_id = session(2);

        service
            .request_for_test(request_with_target(
                session_id,
                Duration::from_millis(100),
                Arc::clone(&observed),
            ))
            .unwrap();
        service
            .cancel_for_test(TimeoutCancel::new(session_id))
            .unwrap();

        std::thread::sleep(Duration::from_millis(150));
        assert!(observed.lock().unwrap().is_empty());
    }

    #[test]
    fn tos_req_008_duplicate_active_request_is_rejected() {
        let config = TimeoutServiceConfig {
            delivery_timeout: Duration::from_millis(20),
            ..TimeoutServiceConfig::default()
        };
        let service = start_timeout_service_with_config::<8>(config);
        let observed = Arc::new(Mutex::new(Vec::new()));
        let session_id = session(3);

        service
            .request_for_test(request_with_target(
                session_id,
                Duration::from_secs(10),
                Arc::clone(&observed),
            ))
            .unwrap();
        service
            .request_for_test(request_with_target(
                session_id,
                Duration::from_secs(10),
                observed,
            ))
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
        let observed = Arc::new(Mutex::new(Vec::new()));
        let session_id = session(5);

        service
            .request_for_test(TimeoutRequest::with_target(
                session_id,
                Time::now() + Duration::from_millis(5),
                Arc::new(RetryTarget {
                    attempts: attempts_for_delivery,
                    observed: Arc::clone(&observed),
                }),
            ))
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(1);
        while observed.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(*observed.lock().unwrap(), vec![session_id]);
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
            .request_for_test(TimeoutRequest::with_target(
                session_id,
                Time::now(),
                Arc::new(FullTarget),
            ))
            .unwrap();

        assert_eq!(
            service.join_for_test(),
            Err(TimeoutServiceError::DeliveryTimedOut(session_id))
        );
    }

    #[test]
    fn tos_req_006_timeout_occurred_event_contains_only_session_id() {
        let service = start_timeout_service::<8>();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let session_id = session(7);

        service
            .request_for_test(TimeoutRequest::with_target(
                session_id,
                Time::now(),
                Arc::new(RecordingTarget {
                    observed: Arc::clone(&observed),
                }),
            ))
            .unwrap();

        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(*observed.lock().unwrap(), vec![session_id]);
    }

    #[test]
    fn tos_req_016_service_instance_final_drop_stops_task() {
        let service = start_timeout_service::<8>();
        let clone = service.clone();
        drop(service);

        let observed = Arc::new(Mutex::new(Vec::new()));
        let session_id = session(8);
        clone
            .request_for_test(request_with_target(
                session_id,
                Duration::from_millis(5),
                Arc::clone(&observed),
            ))
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(1);
        while observed.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(*observed.lock().unwrap(), vec![session_id]);
        drop(clone);
    }
}
