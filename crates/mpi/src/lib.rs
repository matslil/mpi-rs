//! Message-passing runtime primitives for `mpi-rs`.
//!
//! This crate contains the initial verifiable runtime slice for the `mpi-rs`
//! systems-engineering baseline. It deliberately focuses on stable primitives:
//! message placement, bounded task queues, task handles, start-message spawning,
//! session identifiers, typed responses, stream protocol events, and stream
//! cancellation. Full task macro code generation is a later implementation
//! phase.

pub mod error;
pub mod message;
pub mod queue;
pub mod session;
pub mod stream;
pub mod task;

pub use error::{RecvError, SendError};
pub use message::{CanReceive, HasSessionId, MessagePlacement, TaskMessage};
pub use mpi_macros::{call, event, start, stream, task};
pub use queue::TaskQueue;
pub use session::{EndpointId, Response, SessionId, SessionIdAllocator};
pub use stream::{MessageStream, StreamCancel, StreamControl, StreamEvent, StreamPull};
pub use task::{TaskHandle, TaskJoinError, TaskRuntime, spawn_task};
