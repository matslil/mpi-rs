# Full Implementation Attempt Report

## Summary

This report records a second implementation attempt after the initial runtime baseline was merged.

The implementation was extended beyond the previous runtime substrate with:

- external blocking synchronous calls through `TaskHandle::call_blocking`;
- typed reply channels using `SyncReplySender<T>` and `SyncReplyReceiver<T>`;
- task-local `TaskContext` with self handle, stop handling, and session allocation;
- producer-side `StreamSink<T, E, S>` with batching, end events, and error events;
- additional requirement-referenced tests for external calls, task context session allocation, and stream sink behavior.

## Requirements addressed by this attempt

This attempt adds or improves implementation evidence for:

- REQ-015: generated task context concept, partially through runtime `TaskContext`;
- REQ-084: task-local session allocation;
- REQ-091: exactly one external blocking call response path;
- REQ-100: stream event variants;
- REQ-102: stream batching;
- REQ-105: stream error handling;
- REQ-111: producer-side end/error event emission;
- REQ-120: explicit external blocking API;
- REQ-121: separation between external blocking API and future task-internal APIs.

## Concrete design blocker found

A full implementation of the documented macro interface cannot be completed exactly as currently described without changing the macro interface.

The interface document shows this shape:

```rust
#[task(queue_size = 32)]
struct ServerTask {
    state: ServerState,
}

#[task]
impl ServerTask {
    #[start]
    async fn start(&mut self, ctx: &mut ServerContext, config: ServerConfig) {
        // ...
    }

    #[event]
    async fn set(&mut self, ctx: &mut ServerContext, key: String, value: Vec<u8>) {
        // ...
    }
}
```

The task declaration data needed to generate a complete task surface is split across two independent attribute macro invocations:

- the struct attribute contains the queue size and task state type;
- the impl attribute contains the handlers, message names, arguments, reply types, and placement attributes.

A Rust procedural macro expansion should not rely on global mutable state shared between independent attribute invocations. Expansion order is not a stable user-facing contract, and such state would be fragile under incremental compilation, parallel expansion, and rust-analyzer/proc-macro server behavior.

Because of this, a full and robust implementation of generated message enums, contexts, handles, send methods, and dispatch cannot be generated from the documented split form without one of these design changes:

1. Put the whole task declaration in one macro input, for example a module-level `task! { ... }` macro.
2. Put all required generation information on the `impl` attribute, including queue size and state construction policy.
3. Keep `#[task]` on the struct as a marker only, and use a separate explicit macro invocation that names the task type and handler impl.
4. Accept a weaker implementation where the macro requires repeated queue size/configuration on the impl block.

## Recommendation

Use option 1 or 2.

The most Rust-like incremental path is probably option 2:

```rust
#[task(queue_size = 32)]
impl ServerTask {
    #[start]
    async fn start(&mut self, ctx: &mut ServerContext, config: ServerConfig) {
        // ...
    }
}
```

The struct remains a normal Rust struct, and the `#[task]` impl macro has all handler and queue-size data needed to generate the message enum, context, handle, spawn helper, send methods, and dispatch.

## Remaining implementation gaps

The following remain incomplete after this attempt:

- full proc-macro generation for message enums, context, handles, send methods, and dispatch;
- task-local async executor with suspended continuations;
- selective receive waiter registry;
- task-internal synchronous calls that suspend rather than block;
- full stream consumer `next(ctx).await` with task-local receive state;
- credit-based stream flow control and producer suspension;
- late one-shot response fallback policy;
- Unix signal bridge;
- diagnostics, timeouts, tracing, and deadlock/debug support;
- compile-fail tests for receive declarations.

## Conclusion

The runtime design still appears implementable, and this attempt adds more of the runtime needed for calls and streams.

The documented split macro interface is the current blocker for a complete macro-generated implementation. The next system-engineering action should be to revise `docs/se/interfaces.md` and related requirements to choose a macro declaration shape that can be implemented robustly.
