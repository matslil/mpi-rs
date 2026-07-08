//! Unix signal forwarding into task messages.

use std::fmt;
use std::thread::{self, JoinHandle};

use signal_hook::iterator::{Handle as SignalsHandle, Signals};

use crate::error::SendError;
use crate::message::TaskMessage;
use crate::task::TaskHandle;

/// Error returned while creating or joining a Unix signal bridge.
#[derive(Debug)]
pub enum SignalBridgeError {
    /// Signal registration failed.
    Register(std::io::Error),

    /// The background bridge thread panicked.
    ThreadPanicked,
}

impl fmt::Display for SignalBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Register(error) => write!(f, "could not register Unix signal bridge: {error}"),
            Self::ThreadPanicked => f.write_str("Unix signal bridge thread panicked"),
        }
    }
}

impl std::error::Error for SignalBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Register(error) => Some(error),
            Self::ThreadPanicked => None,
        }
    }
}

impl From<std::io::Error> for SignalBridgeError {
    fn from(error: std::io::Error) -> Self {
        Self::Register(error)
    }
}

/// Running bridge that owns Unix signal registrations and the forwarding thread.
///
/// Dropping the bridge closes the signal iterator. Call [`SignalBridge::join`]
/// when the caller needs to wait until the bridge thread has actually exited.
pub struct SignalBridge {
    handle: SignalsHandle,
    join: Option<JoinHandle<()>>,
}

impl SignalBridge {
    /// Stop accepting further signal notifications for this bridge.
    pub fn close(&self) {
        self.handle.close();
    }

    /// Stop the bridge and wait for the forwarding thread to exit.
    pub fn join(mut self) -> Result<(), SignalBridgeError> {
        self.close();
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| SignalBridgeError::ThreadPanicked)?;
        }
        Ok(())
    }
}

impl Drop for SignalBridge {
    fn drop(&mut self) {
        self.close();
    }
}

/// Register Unix signals and forward each observed signal into a task.
///
/// The POSIX signal handler portion is delegated to `signal-hook`; this bridge
/// consumes the resulting signal iterator on a normal Rust thread and only then
/// constructs and enqueues application messages.
pub fn forward_signals<H, M, F, I, const N: usize>(
    task: H,
    signals: I,
    make_message: F,
) -> Result<SignalBridge, SignalBridgeError>
where
    H: Into<TaskHandle<M, N>>,
    M: TaskMessage + Send + 'static,
    F: Fn(i32) -> M + Send + 'static,
    I: IntoIterator<Item = i32>,
{
    let task = task.into();
    let mut signals = Signals::new(signals)?;
    let handle = signals.handle();
    let join = thread::spawn(move || {
        for signal in &mut signals {
            match task.send_message(make_message(signal)) {
                Ok(()) => {}
                Err(SendError::QueueFull | SendError::TaskStopped) => break,
                Err(SendError::StreamFlowLimited | SendError::StreamCancelled) => {
                    unreachable!("signal bridge only sends ordinary task messages")
                }
            }
        }
    });

    Ok(SignalBridge {
        handle,
        join: Some(join),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use signal_hook::consts::signal::SIGUSR1;

    use crate::message::MessagePlacement;
    use crate::queue::TaskQueue;

    #[derive(Debug, Eq, PartialEq)]
    enum SignalMessage {
        Signal(i32),
    }

    impl TaskMessage for SignalMessage {
        fn placement(&self) -> MessagePlacement {
            MessagePlacement::Normal
        }
    }

    #[test]
    fn req_130_req_131_signal_bridge_forwards_signal_outside_handler() {
        let queue = Arc::new(TaskQueue::<SignalMessage, 4>::new());
        let task = TaskHandle::new(Arc::clone(&queue));
        let bridge =
            forward_signals(task, [SIGUSR1], SignalMessage::Signal).expect("bridge starts");

        signal_hook::low_level::raise(SIGUSR1).expect("test signal can be raised");

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut received = None;
        while Instant::now() < deadline {
            if let Some(message) = queue.try_recv() {
                received = Some(message);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        bridge.join().expect("bridge joins");
        assert_eq!(received, Some(SignalMessage::Signal(SIGUSR1)));
    }
}
