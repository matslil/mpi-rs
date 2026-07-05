#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod future;
mod poll;

pub use future::{CtxFuture, ResumeFn, resume_fn};
pub use poll::CtxPoll;
