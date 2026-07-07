#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod future;
mod poll;
mod std_future;

pub use future::{CtxFuture, ResumeFn, resume_fn};
pub use poll::CtxPoll;
pub use std_future::{StdFutureCtx, from_std_future};
