//! Local `mpi` timeout service implemented as a single-result stream.

use std::time::{Duration, Instant};

use mpi::{CanReceive, SendError, StreamEvent, SuspendedMessageStream, task};

/// Queue capacity used by the local timeout task.
pub const TIMEOUT_QUEUE_SIZE: usize = 32;

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

/// Absolute monotonic deadline used by timeout stream requests.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TimeoutInstant(Instant);

impl TimeoutInstant {
    /// Return the underlying standard-library monotonic instant.
    #[must_use]
    pub const fn into_inner(self) -> Instant {
        self.0
    }
}

impl std::ops::Add<Duration> for TimeoutInstant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs)
    }
}

/// Single stream item emitted when the requested deadline is reached.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TimeoutOccurred;

/// Timeout stream application error.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TimeoutError {
    /// The timeout item could not be delivered through the stream sink.
    Delivery(String),
}

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Delivery(message) => write!(f, "timeout delivery failed: {message}"),
        }
    }
}

impl std::error::Error for TimeoutError {}

#[derive(Default)]
struct TimeoutTask;

#[task(queue_size = 32)]
impl TimeoutTask {
    fn new() -> Self {
        Self
    }

    #[stream(item = TimeoutOccurred, error = TimeoutError, batch_size = 1)]
    fn timeout(
        ctx: &mut TimeoutTaskContext,
        out: &mut mpi::BoxStreamSink<TimeoutOccurred, TimeoutError>,
        deadline: TimeoutInstant,
    ) -> Result<(), TimeoutError> {
        ctx.sleep_until(deadline.into_inner()).await;
        out.push(TimeoutOccurred)
            .map_err(|error| TimeoutError::Delivery(error.to_string()))?;
        Ok(())
    }
}

impl TimeoutTaskServiceInstance {
    /// Start a single-result timeout stream from task scope.
    pub fn timeout<C>(
        &self,
        ctx: &mut C,
        deadline: TimeoutInstant,
    ) -> Result<SuspendedMessageStream<TimeoutOccurred, TimeoutError>, SendError>
    where
        C: mpi::TaskScope,
        C::Message: CanReceive<StreamEvent<TimeoutOccurred, TimeoutError>>,
    {
        self.binding().timeout(ctx, deadline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tos_req_003_time_now_uses_absolute_monotonic_instant() {
        let now = Time::now();
        assert!((now + Duration::from_millis(1)).into_inner() > now.into_inner());
    }
}
