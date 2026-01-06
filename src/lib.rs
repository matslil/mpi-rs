//! MPI-inspired object runtime.
//!
//! This crate offers a minimal runtime for building message-driven objects that
//! run on dedicated threads. Methods are called by sending messages, and
//! responses are delivered through reply streams so both normal return values
//! and generator-style yields are supported.

use std::marker::PhantomData;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Responses emitted by an MPI object method.
///
/// A single call may yield zero or more [`Yield`] messages before either
/// finishing without a return value or sending a [`Return`] message.
#[derive(Debug, PartialEq, Eq)]
pub enum MpiResponse<Y, R> {
    /// A yielded value from a generator-like method.
    Yield(Y),
    /// The final return value of a method.
    Return(R),
    /// A marker for functions that do not return anything.
    Finished,
}

/// Errors that can occur when interacting with an MPI object.
#[derive(Debug, thiserror::Error)]
pub enum MpiError {
    /// The target object stopped responding.
    #[error("object has shut down")]
    ObjectTerminated,
}

struct Request<M: Send + 'static> {
    message: M,
    respond_to: Sender<MpiResponse<M::Yield, M::Return>>,
}

/// Trait implemented by message types that can be dispatched to an MPI object.
pub trait MpiMessage: Send + 'static {
    /// Concrete object type that receives the message.
    type Target: Send + 'static;
    /// Type of values yielded during execution.
    type Yield: Send + 'static;
    /// Type returned when the method finishes.
    type Return: Send + 'static;

    /// Execute the message against the object and write results to the sink.
    fn dispatch(self, object: &mut Self::Target, sink: &mut ReplySink<Self::Yield, Self::Return>);
}

/// A reply helper that ensures callers receive the proper response sequence.
pub struct ReplySink<'a, Y, R> {
    sender: &'a Sender<MpiResponse<Y, R>>,
    finished: bool,
}

impl<'a, Y, R> ReplySink<'a, Y, R> {
    fn new(sender: &'a Sender<MpiResponse<Y, R>>) -> Self {
        Self {
            sender,
            finished: false,
        }
    }

    /// Yield an intermediate value to the caller.
    pub fn yield_item(&mut self, value: Y) {
        // Ignore send errors; they indicate the caller hung up.
        let _ = self.sender.send(MpiResponse::Yield(value));
    }

    /// Send the final return value to the caller.
    pub fn return_value(&mut self, value: R) {
        if !self.finished {
            self.finished = true;
            let _ = self.sender.send(MpiResponse::Return(value));
        }
    }

    /// Mark the call as finished without a return value.
    pub fn finish(&mut self) {
        if !self.finished {
            self.finished = true;
            let _ = self.sender.send(MpiResponse::Finished);
        }
    }
}

impl<'a, Y, R> Drop for ReplySink<'a, Y, R> {
    fn drop(&mut self) {
        if !self.finished {
            self.finish();
        }
    }
}

/// Handle used by callers to communicate with an MPI object.
pub struct MpiHandle<M>
where
    M: MpiMessage,
{
    sender: Sender<Request<M>>,
    _marker: PhantomData<M::Target>,
}

impl<M> Clone for MpiHandle<M>
where
    M: MpiMessage,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _marker: PhantomData,
        }
    }
}

impl<M> MpiHandle<M>
where
    M: MpiMessage,
{
    /// Send a message and return a stream of responses.
    pub fn call_stream(&self, message: M) -> Result<ResponseStream<M::Yield, M::Return>, MpiError> {
        let (sender, receiver) = mpsc::channel();
        let request = Request {
            message,
            respond_to: sender,
        };
        self.sender
            .send(request)
            .map_err(|_| MpiError::ObjectTerminated)?;
        Ok(ResponseStream { receiver })
    }

    /// Send a message and wait for the final return value.
    pub fn call(&self, message: M) -> Result<Option<M::Return>, MpiError> {
        let mut stream = self.call_stream(message)?;
        while let Some(response) = stream.next() {
            match response {
                MpiResponse::Return(value) => return Ok(Some(value)),
                MpiResponse::Finished => return Ok(None),
                MpiResponse::Yield(_) => {}
            }
        }
        Err(MpiError::ObjectTerminated)
    }
}

/// A stream of responses produced by an MPI object call.
pub struct ResponseStream<Y, R> {
    receiver: Receiver<MpiResponse<Y, R>>,
}

impl<Y, R> ResponseStream<Y, R> {
    /// Collect all yielded values until the call returns.
    pub fn collect_yields(mut self) -> Vec<Y> {
        let mut items = Vec::new();
        while let Some(resp) = self.next() {
            match resp {
                MpiResponse::Yield(value) => items.push(value),
                MpiResponse::Return(_) | MpiResponse::Finished => break,
            }
        }
        items
    }
}

impl<Y, R> Iterator for ResponseStream<Y, R> {
    type Item = MpiResponse<Y, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}

/// Spawn an MPI object in its own thread.
pub fn spawn_object<O, M>(mut object: O) -> MpiHandle<M>
where
    O: Send + 'static,
    M: MpiMessage<Target = O>,
{
    let (sender, receiver): (Sender<Request<M>>, Receiver<Request<M>>) = mpsc::channel();

    thread::spawn(move || {
        while let Ok(Request {
            message,
            respond_to,
        }) = receiver.recv()
        {
            let mut sink = ReplySink::new(&respond_to);
            message.dispatch(&mut object, &mut sink);
            // sink drops here to ensure the call is closed.
        }
    });

    MpiHandle {
        sender,
        _marker: PhantomData,
    }
}

/// Helper macro for building trampoline methods on a handle wrapper.
#[macro_export]
macro_rules! mpi_trampoline {
    (impl $wrapper:ty { $(
        $(#[$meta:meta])* fn $name:ident ( & $self:ident $(, $arg:ident : $arg_ty:ty )* ) -> $ret:ty => $ctor:expr ;
    )* }) => {
        impl $wrapper {
            $(
                $(#[$meta])*
                pub fn $name(& $self $(, $arg : $arg_ty )* ) -> Result<$ret, $crate::MpiError> {
                    self.0.call($ctor)
                        .map(|opt| opt.expect("method returned no value"))
                }
            )*
        }
    };
}

/// Macro for creating non-blocking trampolines that expose the raw stream.
#[macro_export]
macro_rules! mpi_trampoline_stream {
    (impl $wrapper:ty { $(
        $(#[$meta:meta])* fn $name:ident ( & $self:ident $(, $arg:ident : $arg_ty:ty )* ) -> $ret:ty => $ctor:expr ;
    )* }) => {
        impl $wrapper {
            $(
                $(#[$meta])*
                pub fn $name(& $self $(, $arg : $arg_ty )* ) -> Result<$ret, $crate::MpiError> {
                    self.0.call_stream($ctor)
                }
            )*
        }
    };
}
