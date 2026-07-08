# Interfaces

This document describes the intended public and internal interfaces for `mpi-rs`.

It is part of the authoritative systems-engineering baseline and should guide implementation, tests, review, and validation.

## Interface principles

INT-001: User-facing APIs should be idiomatic Rust.

INT-002: Users should send messages by calling generated methods on task handles.

INT-003: Users should not manually construct low-level runtime protocol messages for normal use.

INT-004: Task-internal synchronous and streaming APIs should require an explicit task context argument.

INT-005: External blocking APIs should be explicit and visually distinct from task-internal APIs.

## Protocol declaration interface

A protocol declaration defines a namespace-qualified message contract. The exact
macro syntax remains subject to implementation design, but the conceptual shape
is:

```rust
protocol! {
    pub protocol InventoryV1 {
        event Reindex(ReindexRequest);
        call GetItem(GetItemRequest) -> GetItemReply;
        stream WatchStock(WatchStockRequest) -> StockEvent error WatchStockError;
    }
}
```

Interface rules:

INT-006: Protocol declarations shall produce namespace-qualified protocol message identities.

INT-007: Protocol declarations shall explicitly identify payload, reply, stream item, and stream error types as applicable.

INT-008: Generated send, call, and stream APIs shall be typed from protocol message declarations.

INT-009: The interface baseline shall not require users to provide or compare generated protocol fingerprints.

INT-009A: A receive declaration shall identify a protocol-qualified reply or stream event and shall use the Rust type declared by that protocol item.

INT-009B: Generated send, call, and stream methods shall be produced from a protocol declaration or from a protocol-instance binding that identifies the concrete implementing task handle.

Conceptual protocol-instance binding:

```rust
let inventory = InventoryV1::bind(inventory_task);
let reply = inventory.get_item(ctx, request).await?;
```

In this shape, `InventoryV1::GetItem` defines the message identity and Rust
types, while `inventory_task` identifies the concrete task instance that
implements that protocol message.

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
    fn start(ctx: &mut ServerTaskContext, config: ServerConfig) {
        ctx.with_state(|state| {
            state.state = ServerState::new(config);
        });
    }

    #[event]
    fn set(ctx: &mut ServerTaskContext, key: String, value: Vec<u8>) {
        ctx.with_state(|state| {
            state.state.insert(key, value);
        });
    }

    #[call(reply = GetReply)]
    fn get(ctx: &mut ServerTaskContext, key: String) -> GetReply {
        ctx.with_state(|state| GetReply {
            value: state.state.get(&key).cloned(),
        })
    }

    #[late_reply]
    fn late_reply(
        ctx: &mut ServerTaskContext,
        reply: mpi::LateReplyRef<'_>,
    ) -> mpi::LateReplyAction {
        mpi::LateReplyAction::Ignore
    }

    #[event(priority)]
    fn shutdown(ctx: &mut ServerTaskContext) {
        ctx.stop();
    }
}
```

Handler declarations use ordinary Rust `fn` syntax. The `#[task]` macro owns the
lowering into task-local suspended execution, so users do not need to spell the
current implementation strategy in the handler signature. Handler bodies may
still use `.await` with task-internal call and stream APIs when they need to
suspend.

Handlers are associated functions without `self`. Handlers access user state
through the generated context's `with_state(|state| ...)` operation. The
`with_state` closure receives only a short mutable borrow of the task's user
state; it does not receive a task context, and task operations that require the
context must be performed outside the closure.

Interface rules:

INT-010: The task declaration macro shall be named `task`.

INT-011: The task declaration interface shall support static queue-size configuration on the `#[task]` impl attribute.

INT-012: `#[start]` shall identify the start handler.

INT-013: `#[event]` shall identify an asynchronous message with no reply.

INT-014: `#[call(reply = T)]` shall identify a synchronous request handler with reply payload type `T`.

INT-015: `#[stream(item = T, error = E)]` shall identify a streaming handler with item type `T` and error type `E`.

INT-016: `priority` shall be declared on the receiver's message declaration, not at each send site.

INT-019: Call and stream declarations may use `late_reply = "ignore"` to opt out of reporting unknown-session replies for that interaction; the default policy is equivalent to `late_reply = "report"`.

INT-019A: `#[late_reply]` shall identify an optional task handler for reported late replies.

INT-017: An explicit normal placement for a start handler shall be rejected or ignored in favor of forced priority.

INT-018: A `#[task]` attribute on a struct is non-authoritative and should not be required for code generation.

INT-018A: Handler declarations should use ordinary Rust `fn` syntax without a `self` receiver; the task macro owns any async or `CtxFuture` lowering needed to execute them.

INT-018B: Generated task contexts shall expose a scoped `with_state` operation so user-state mutation is explicit and cannot directly perform context-dependent task operations inside the state borrow.

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

INT-027: Runtime task handles may expose or wrap a shared `TaskEndpoint` that owns the target queue reference and task lifecycle state. Task-local runtime drivers may receive through this endpoint rather than accepting a raw queue reference. This endpoint is an implementation support interface; ordinary users still send through generated task-handle methods.

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
Those declarations are derived from protocol-declared response and stream event
types.

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

INT-042A: Receive declarations should refer to protocol-qualified response or stream event identities and their protocol-declared Rust types rather than unqualified message names.

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

INT-063: Late one-shot responses shall be passed to the receiving task's late-reply handler rather than silently discarded by default.

INT-064: A one-shot response generated for a call declared with `late_reply = "ignore"` shall not be exposed as an unknown-session condition when its session has no active waiter.

INT-065: A late-reply handler shall receive `LateReplyRef<'_>` and return `LateReplyAction`.

INT-066: `LateReplyRef<'_>` shall expose the late reply's `SessionId`, late-reply kind, and read-only downcast access to the reply payload.

INT-067: `LateReplyAction` shall include `Ignore` and `Terminate`.

INT-068: If no late-reply handler is declared, the default late-reply handler shall return `Ignore`.

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
fn query(
    ctx: &mut DatabaseContext,
    mut out: StreamSink<Row, QueryError>,
    sql: String,
) -> Result<(), QueryError> {
    let rows = ctx.with_state(|state| state.state.execute(sql))?;
    for row in rows {
        out.push(row)?;
    }

    Ok(())
}
```

Producer-side stream code that is lowered into a native `CtxFuture`
continuation may instead use `yield_item(value)` or `yield_batch(values)`.
Those operations send a stream reply and then return `Pending` once so the
task-local runtime can route stream cancellation, flow-control, replies, or
ordinary messages before the producer continuation resumes.

Interface rules:

INT-070: The public stream consumer API shall return one item at a time.

INT-071: The internal stream event API may batch many items per event.

INT-072: The stream object shall hide batching from ordinary consumer code.

INT-073: The stream object shall attempt asynchronous cancellation when dropped before completion.

INT-074: Stream cancellation shall not require awaiting acknowledgement during drop.

INT-075: The producer-side `StreamSink` shall hide batching, end, error, and flow-control details where possible.

INT-075A: The producer-side `StreamSink` shall provide a native `CtxFuture`
yield operation for stream producers that send a stream reply and return
context to the task-local runtime before producer continuation.

INT-075B: `StreamSink::yield_item()` and `StreamSink::yield_batch()` shall not require ordinary handlers to manually handle no-credit retry loops; when credit is unavailable they shall suspend until credit, cancellation, or another defined terminal stream-control condition is routed by the task-local runtime.

INT-076: A future `futures_core::Stream` implementation may be added only if it preserves safe access to task-local receive state.

INT-077: Stream replies shall be passed to the receiving task's late-reply handler by default when their session has no active waiter or stream object.

INT-078: Stream replies generated for a stream declared with `late_reply = "ignore"` shall not be exposed as an unknown-session condition when their session has no active waiter or stream object.

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
- protocol violation or late reply policy, when diagnostics are implemented.

Interface rules:

INT-100: Queue-full behavior shall be represented as an explicit error.

INT-101: Errors returned by public APIs should be typed and documented.

## Unix signal interface

Unix signal support should expose convenient forwarding to tasks, but the public API must not imply that normal Rust message construction happens inside the POSIX signal handler.

Interface rules:

INT-110: Any signal forwarding API shall be implemented through a signal bridge.

INT-111: The actual POSIX signal handler behavior shall be limited to async-signal-safe operations.

INT-112: Unix signal bridge APIs shall be available on Unix targets when the default-enabled `unix-signals` feature is active, and shall be removable from the public API by disabling that feature.
