# Interfaces

This document describes the intended public and internal interfaces for `mpi-rs`.

It is part of the authoritative systems-engineering baseline and should guide implementation, tests, review, and validation.

## Interface principles

INT-001: User-facing APIs should be idiomatic Rust.

INT-002: Users should send messages by calling generated methods on task handles.

INT-003: Users should not manually construct low-level runtime protocol messages for normal use.

INT-004: Task-internal synchronous and streaming APIs should require an explicit task context argument.

INT-005: External blocking APIs should be explicit and visually distinct from task-internal APIs.

## Task declaration interface

The task state is a normal Rust struct. The `#[task]` attribute is applied to the `impl` block that contains the task handlers. The `impl` attribute shall contain the queue-size configuration so that one macro invocation has all information needed to generate the task message enum, context type, handle type, send methods, spawn helper, placement implementation, and dispatch plumbing.

Example declaration style:

```rust
struct ServerTask {
    state: ServerState,
}

#[task(queue_size = 32)]
impl ServerTask {
    #[start]
    async fn start(&mut self, ctx: &mut ServerTaskContext, config: ServerConfig) {
        self.state = ServerState::new(config);
    }

    #[event]
    async fn set(&mut self, ctx: &mut ServerTaskContext, key: String, value: Vec<u8>) {
        self.state.insert(key, value);
    }

    #[call(reply = GetReply)]
    async fn get(&mut self, ctx: &mut ServerTaskContext, key: String) -> GetReply {
        GetReply {
            value: self.state.get(&key).cloned(),
        }
    }

    #[event(priority)]
    async fn shutdown(&mut self, ctx: &mut ServerTaskContext) {
        ctx.stop();
    }
}
```

Interface rules:

INT-010: The task declaration macro shall be named `task`.

INT-011: The task declaration interface shall support static queue-size configuration on the `#[task]` impl attribute.

INT-012: `#[start]` shall identify the start handler.

INT-013: `#[event]` shall identify an asynchronous message with no reply.

INT-014: `#[call(reply = T)]` shall identify a synchronous request handler with reply payload type `T`.

INT-015: `#[stream(item = T, error = E)]` shall identify a streaming handler with item type `T` and error type `E`.

INT-016: `priority` shall be declared on the receiver's message declaration, not at each send site.

INT-017: An explicit normal placement for a start handler shall be rejected or ignored in favor of forced priority.

INT-018: A `#[task]` attribute on a struct is non-authoritative and should not be required for code generation.

## Generated task handle interface

Generated task handles expose send methods.

Event example:

```rust
server.set(ctx, key, value).await?;
```

Call example:

```rust
let reply = server.get(ctx, key).await?;
```

External blocking call example:

```rust
let reply = server.get_blocking(key)?;
```

Stream example:

```rust
let mut rows = database.query(ctx, sql).await?;
while let Some(row) = rows.next(ctx).await? {
    println!("{row:?}");
}
```

Interface rules:

INT-020: Each sendable message shall have a generated method on the receiving task handle.

INT-021: Generated methods shall return an explicit error when enqueueing fails.

INT-022: Event send methods shall not require the caller to manually construct a message enum variant.

INT-023: Call send methods shall hide session allocation and response matching from ordinary user code.

INT-024: Stream send methods shall hide stream request, stream event, flow-control, and cancellation plumbing from ordinary user code.

INT-025: Task-internal generated methods shall accept a task context argument when needed to suspend and resume handlers.

INT-026: Generated external blocking call methods shall be explicitly named with a blocking suffix such as `_blocking`.

## Message interface

Runtime message placement is exposed through a trait conceptually equivalent to:

```rust
pub trait TaskMessage: Sized {
    fn placement(&self) -> MessagePlacement;
}
```

Placement is conceptually:

```rust
pub enum MessagePlacement {
    Normal,
    Priority,
}
```

Interface rules:

INT-030: Every generated task message enum shall implement the task message placement interface.

INT-031: Placement shall be computed from the receiving task's message declaration.

INT-032: Start messages shall report priority placement.

## Compile-time receive check interface

A task must declare the response and stream event messages it can receive.

The mechanism may be equivalent to:

```rust
pub trait CanReceive<T>: TaskMessage {
    fn wrap(value: T) -> Self;
}
```

Interface rules:

INT-040: A caller task shall only be able to call a task-internal synchronous method when its message enum can receive the corresponding response type.

INT-041: A caller task shall only be able to call a task-internal streaming method when its message enum can receive the corresponding stream event type.

INT-042: Missing receive support should fail at compile time rather than runtime.

## Context future interface

The reusable `ctx-future` crate exposes a context-returning suspension interface conceptually equivalent to:

```rust
pub trait CtxFuture<Cx, Input = ()> {
    type Output;

    fn resume(&mut self, cx: &mut Cx, input: Input) -> CtxPoll<Self::Output>;
}

pub enum CtxPoll<T> {
    Pending,
    Ready(T),
}
```

Interface rules:

INT-043: The `ctx-future` crate shall not depend on `mpi-rs`.

INT-044: `mpi-rs` generated task handlers may be lowered into `CtxFuture` implementations or equivalent context-returning state machines.

INT-045: A pending `CtxFuture` used by `mpi-rs` shall not retain mutable borrows of task state or task context after `resume` returns.

## Session interface

Recommended session type:

```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SessionId {
    origin: EndpointId,
    sequence: u64,
}
```

Protocol messages may expose:

```rust
pub trait HasSessionId {
    fn session_id(&self) -> SessionId;
}
```

Interface rules:

INT-050: `SessionId` shall be available to runtime protocol messages for calls, streams, cancellation, matching, tracing, and debugging.

INT-051: Session allocation shall normally be hidden from user-facing call and stream APIs.

INT-052: Debugging and diagnostics may expose session IDs.

## Synchronous response interface

Conceptual response wrapper:

```rust
pub struct Response<T> {
    pub session_id: SessionId,
    pub value: T,
}
```

Interface rules:

INT-060: Responses shall carry the `SessionId` of the call they complete.

INT-061: User call handlers may return the reply payload rather than manually constructing `Response<T>`.

INT-062: The macro or runtime shall convert the returned payload into the response message.

INT-063: Late one-shot responses shall be exposed to fallback reply handling or task policy rather than silently discarded by default.

## Stream interface

Conceptual stream event:

```rust
pub enum StreamEvent<T, E> {
    Batch {
        session_id: SessionId,
        values: Vec<T>,
    },
    End {
        session_id: SessionId,
    },
    Error {
        session_id: SessionId,
        error: E,
    },
}
```

Conceptual stream object:

```rust
pub struct MessageStream<T, E> {
    session_id: SessionId,
    control: StreamControlHandle,
    buffer: VecDeque<T>,
    finished: bool,
}
```

Consumer API:

```rust
let mut rows = database.query(ctx, sql).await?;

while let Some(row) = rows.next(ctx).await? {
    println!("{row:?}");
}
```

`next(ctx)` should behave conceptually as follows:

```text
if local buffer has item:
    return item
if stream is finished:
    return None
receive matching StreamEvent<T, E> for this SessionId
if Batch:
    buffer values and return first item
if End:
    mark finished and return None
if Error:
    mark finished and return error
```

Producer API:

```rust
#[stream(item = Row, error = QueryError, batch_size = 64)]
async fn query(
    &mut self,
    ctx: &mut DatabaseContext,
    mut out: StreamSink<Row, QueryError>,
    sql: String,
) -> Result<(), QueryError> {
    for row in self.state.execute(sql)? {
        out.push(row).await?;
    }

    Ok(())
}
```

Interface rules:

INT-070: The public stream consumer API shall return one item at a time.

INT-071: The internal stream event API may batch many items per event.

INT-072: The stream object shall hide batching from ordinary consumer code.

INT-073: The stream object shall attempt asynchronous cancellation when dropped before completion.

INT-074: Stream cancellation shall not require awaiting acknowledgement during drop.

INT-075: The producer-side `StreamSink` shall hide batching, end, error, and flow-control details where possible.

INT-076: A future `futures_core::Stream` implementation may be added only if it preserves safe access to task-local receive state.

## Stream flow-control interface

Hidden control messages are conceptually:

```rust
pub struct StreamPull {
    pub session_id: SessionId,
    pub credit: u32,
}

pub struct StreamCancel {
    pub session_id: SessionId,
}
```

Suggested defaults:

```text
batch_size = 64
window = 128
low_watermark = 32
```

Interface rules:

INT-080: Flow-control messages should be hidden from ordinary user code.

INT-081: Default stream batching and window values may exist but should be configurable by task or stream declaration when needed.

## External caller interface

External callers do not have a task queue and may use explicit blocking APIs.

Example:

```rust
let reply = server.get_blocking(key)?;
```

Interface rules:

INT-090: External blocking APIs shall be explicitly named as blocking or otherwise clearly distinct.

INT-091: External blocking APIs may use one-shot channels internally.

INT-092: Task-internal code should use context-aware APIs rather than external blocking APIs.

## Error interface

Expected error categories include:

- queue full;
- task stopped;
- send failed;
- receive cancelled;
- stream cancelled;
- stream error;
- timeout, when timeouts are implemented;
- protocol violation or late response policy, when diagnostics are implemented.

Interface rules:

INT-100: Queue-full behavior shall be represented as an explicit error.

INT-101: Errors returned by public APIs should be typed and documented.

## Unix signal interface

Unix signal support should expose convenient forwarding to tasks, but the public API must not imply that normal Rust message construction happens inside the POSIX signal handler.

Interface rules:

INT-110: Any signal forwarding API shall be implemented through a signal bridge.

INT-111: The actual POSIX signal handler behavior shall be limited to async-signal-safe operations.
