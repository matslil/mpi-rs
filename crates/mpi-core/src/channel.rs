//! Internal wake-aware channels used by runtime waiters.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::task::Waker;

struct State<T> {
    values: VecDeque<T>,
    senders: usize,
    receiver_alive: bool,
    waker: Option<Waker>,
}

struct Shared<T> {
    state: Mutex<State<T>>,
    available: Condvar,
}

/// Error returned when the receiving endpoint has been dropped.
pub struct SendError<T>(pub T);

/// Non-blocking receive failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TryRecvError {
    Empty,
    Disconnected,
}

/// Blocking receive failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecvError;

/// Sending endpoint for an internal runtime channel.
pub struct Sender<T> {
    shared: Arc<Shared<T>>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.shared
            .state
            .lock()
            .expect("channel mutex poisoned")
            .senders += 1;
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T> Sender<T> {
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        let waker = {
            let mut state = self.shared.state.lock().expect("channel mutex poisoned");
            if !state.receiver_alive {
                return Err(SendError(value));
            }
            state.values.push_back(value);
            state.waker.take()
        };
        self.shared.available.notify_one();
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(())
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let waker = {
            let mut state = self.shared.state.lock().expect("channel mutex poisoned");
            state.senders -= 1;
            (state.senders == 0).then(|| state.waker.take()).flatten()
        };
        self.shared.available.notify_all();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

/// Receiving endpoint for an internal runtime channel.
pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
}

impl<T> Receiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut state = self.shared.state.lock().expect("channel mutex poisoned");
        if let Some(value) = state.values.pop_front() {
            Ok(value)
        } else if state.senders == 0 {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        let mut state = self.shared.state.lock().expect("channel mutex poisoned");
        loop {
            if let Some(value) = state.values.pop_front() {
                return Ok(value);
            }
            if state.senders == 0 {
                return Err(RecvError);
            }
            state = self
                .shared
                .available
                .wait(state)
                .expect("channel mutex poisoned");
        }
    }

    pub fn register_waker(&self, waker: &Waker) {
        let mut state = self.shared.state.lock().expect("channel mutex poisoned");
        if state.values.is_empty() && state.senders != 0 {
            if state
                .waker
                .as_ref()
                .is_none_or(|current| !current.will_wake(waker))
            {
                state.waker = Some(waker.clone());
            }
        } else {
            waker.wake_by_ref();
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().expect("channel mutex poisoned");
        state.receiver_alive = false;
        state.values.clear();
        state.waker = None;
    }
}

#[must_use]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            values: VecDeque::new(),
            senders: 1,
            receiver_alive: true,
            waker: None,
        }),
        available: Condvar::new(),
    });
    (
        Sender {
            shared: Arc::clone(&shared),
        },
        Receiver { shared },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Wake, Waker};

    struct CountWake(AtomicUsize);

    impl Wake for CountWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn mpi_req_150_delivery_before_receive_is_retained() {
        let (sender, receiver) = channel();
        sender.send(42).unwrap_or_else(|_| panic!("send failed"));
        assert_eq!(receiver.try_recv(), Ok(42));
    }

    #[test]
    fn mpi_req_150_registered_waiter_is_woken_after_delivery() {
        let (sender, receiver) = channel();
        let wake = Arc::new(CountWake(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&wake));
        receiver.register_waker(&waker);

        sender.send(7).unwrap_or_else(|_| panic!("send failed"));

        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert_eq!(receiver.try_recv(), Ok(7));
    }

    #[test]
    fn mpi_req_150_blocking_receive_uses_condition_notification() {
        let (sender, receiver) = channel();
        let thread = std::thread::spawn(move || receiver.recv());
        sender.send(9).unwrap_or_else(|_| panic!("send failed"));
        assert_eq!(thread.join().unwrap(), Ok(9));
    }

    #[test]
    fn mpi_req_150_disconnection_is_explicit() {
        let (sender, receiver) = channel::<()>();
        drop(sender);
        assert_eq!(receiver.try_recv(), Err(TryRecvError::Disconnected));

        let (sender, receiver) = channel();
        drop(receiver);
        assert!(sender.send(()).is_err());
    }
}
