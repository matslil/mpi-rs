//! Procedural marker attributes for `mpi` task declarations.
//!
//! These macros are intentionally minimal in the first implementation slice. They
//! preserve the intended source-level syntax while the runtime primitives and
//! verification tests are established. Future implementation phases will replace
//! the pass-through behavior with code generation for task message enums,
//! contexts, handles, dispatch, call plumbing, and stream plumbing.

use proc_macro::TokenStream;

fn passthrough(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a task type or task implementation block.
#[proc_macro_attribute]
pub fn task(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}

/// Marks the start handler for a task.
#[proc_macro_attribute]
pub fn start(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}

/// Marks an asynchronous event handler.
#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}

/// Marks a synchronous call handler.
#[proc_macro_attribute]
pub fn call(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}

/// Marks a streaming call handler.
#[proc_macro_attribute]
pub fn stream(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}
