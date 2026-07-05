# ctx-future

`ctx-future` provides future-like resumable computations that borrow caller-owned context only while being resumed.

The core abstraction is `CtxFuture<Cx, Input>`. A pending future stores its own continuation state, but it does not keep a mutable borrow of `Cx` after `resume` returns.

```rust
use ctx_future::{CtxFuture, CtxPoll};

struct WaitOnce(bool);

impl CtxFuture<Vec<&'static str>> for WaitOnce {
    type Output = usize;

    fn resume(&mut self, cx: &mut Vec<&'static str>, (): ()) -> CtxPoll<Self::Output> {
        cx.push("resumed");

        if self.0 {
            CtxPoll::Ready(cx.len())
        } else {
            self.0 = true;
            CtxPoll::Pending
        }
    }
}
```

This crate intentionally does not depend on `mpi-rs` or expose message-passing concepts.
