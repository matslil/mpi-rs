//! Local `mpi` timeout service.
//!
//! The crate baseline is documented in `se-design-baseline.md`.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mpi::{
    CallError, CanReceive, MessagePlacement, QueueSpaceWakeupMessage, Response, SendError,
    SessionId, SuspendedCall, SyncReplySender, TaskHandle, TaskMessage, TaskQueue, TaskScope,
    protocol,
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

/// Timeout request accepted by the service.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TimeoutRequest {
    deadline: TimeoutInstant,
}

impl TimeoutRequest {
    /// Construct a timeout request for an absolute monotonic deadline.
    #[must_use]
    pub const fn new(deadline: TimeoutInstant) -> Self {
        Self { deadline }
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
        call request(TimeoutRequest) -> ();
        event cancel(TimeoutCancel);
    }
}

/// Messages received by the timeout service task.
enum TimeoutServiceMessage {
    /// Schedule a timeout.
    Request {
        session_id: SessionId,
        reply: SyncReplySender<()>,
        request: TimeoutRequest,
    },

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
            Self::Request { .. } => MessagePlacement::Normal,
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

    /// The requester stopped before the timeout reply could be delivered.
    RequesterStopped(SessionId),

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
            Self::RequesterStopped(session_id) => {
                write!(f, "timeout requester for {session_id} stopped")
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
    /// Local sleep duration used between queue polls.
    pub poll_interval: Duration,

    /// Number of priority slots reserved in the timeout-service queue.
    pub priority_reserved: usize,
}

impl Default for TimeoutServiceConfig {
    fn default() -> Self {
        Self {
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
    fn request(
        &self,
        session_id: SessionId,
        reply: SyncReplySender<()>,
        request: TimeoutRequest,
    ) -> Result<(), SendError> {
        self.handle.send_message(TimeoutServiceMessage::Request {
            session_id,
            reply,
            request,
        })
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
    fn request<C>(&self, ctx: &mut C, request: TimeoutRequest) -> SuspendedCall<()>
    where
        C: TaskScope,
        C::Message: CanReceive<TimeoutServiceProtocolV1::request::Reply>,
    {
        let (session_id, reply, future) = ctx.begin_call::<()>();
        match self.handle.request(session_id, reply, request) {
            Ok(()) => {
                let cancel_handle = self.handle.clone();
                future.with_additional_on_drop(move |session_id| {
                    let _ = cancel_handle.cancel(TimeoutCancel::new(session_id));
                })
            }
            Err(error) => SuspendedCall::failed(error.into()),
        }
    }

    fn request_blocking(&self, request: TimeoutRequest) -> Result<(), CallError> {
        self.handle
            .task_handle()
            .call_blocking(|session_id, reply| TimeoutServiceMessage::Request {
                session_id,
                reply,
                request,
            })
            .map(|response| response.value)
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
    reply: SyncReplySender<()>,
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
                TimeoutServiceMessage::Request {
                    session_id,
                    reply,
                    request,
                } => {
                    if canceled.remove(&session_id) {
                        continue;
                    }
                    if active.contains_key(&session_id) {
                        return Err(TimeoutServiceError::DuplicateSession(session_id));
                    }
                    deadlines.push(Reverse((request.deadline, session_id)));
                    active.insert(
                        session_id,
                        ActiveTimeout {
                            deadline: request.deadline,
                            reply,
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
            timeout
                .reply
                .send_from(
                    handle.task_handle().queue_space_wakeup_target(),
                    Response::new(session_id, ()),
                )
                .map_err(|error| match error {
                    SendError::TaskStopped => TimeoutServiceError::RequesterStopped(session_id),
                    other => TimeoutServiceError::Send(other),
                })?;
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

#[cfg(test)]
mod tests {
    use mpi::{EndpointId, sync_reply_channel};

    use super::*;

    fn session(sequence: u64) -> SessionId {
        SessionId::new(EndpointId(7), sequence)
    }

    fn enqueue_request<const N: usize>(
        service: &TimeoutServiceInstance<N>,
        session_id: SessionId,
        delay: Duration,
    ) -> mpi::SyncReplyReceiver<()> {
        let (reply, receiver) = sync_reply_channel();
        service
            .inner
            .endpoint
            .handle
            .request(session_id, reply, TimeoutRequest::new(Time::now() + delay))
            .unwrap();
        receiver
    }

    #[test]
    fn tos_req_003_time_now_returns_timeout_instant() {
        let start = Time::now();
        let deadline = start + Duration::from_millis(1);

        assert!(deadline >= start);
        assert!(start.duration_until(deadline) <= Duration::from_millis(1));
    }

    #[test]
    fn tos_req_007_expiry_delivers_payload_free_reply() {
        let service = start_timeout_service::<8>();
        let start = Time::now();

        let reply: () = service
            .protocol()
            .request_blocking(TimeoutRequest::new(start + Duration::from_millis(5)))
            .unwrap();

        assert_eq!(reply, ());
        assert!(Time::now().has_reached(start + Duration::from_millis(5)));
    }

    #[test]
    fn tos_req_009_cancel_discards_pending_timeout() {
        let service = start_timeout_service::<8>();
        let session_id = session(2);
        let reply = enqueue_request(&service, session_id, Duration::from_millis(100));
        service
            .protocol()
            .cancel_blocking(TimeoutCancel::new(session_id))
            .unwrap();

        assert!(reply.recv_timeout(Duration::from_millis(150)).is_err());
    }

    #[test]
    fn tos_req_008_duplicate_active_request_is_rejected() {
        let service = start_timeout_service::<8>();
        let session_id = session(3);

        let _first = enqueue_request(&service, session_id, Duration::from_secs(10));
        let _second = enqueue_request(&service, session_id, Duration::from_secs(10));

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
    fn tos_req_016_service_instance_final_drop_stops_task() {
        let service = start_timeout_service::<8>();
        let clone = service.clone();
        drop(service);

        let reply: () = clone
            .protocol()
            .request_blocking(TimeoutRequest::new(Time::now() + Duration::from_millis(5)))
            .unwrap();

        assert_eq!(reply, ());
        drop(clone);
    }
}
