# Message model design

This document describes the intended message model for `mpi-rs`.

The goal is to provide an OSE-inspired message passing model for Rust applications while keeping the user-facing API idiomatic for Rust. OSE uses the term signal; `mpi-rs` uses the term message.

## Goals

- A task is an operating system thread with an associated bounded message queue.
- A task declares, at compile time, which messages it can receive.
- Sending a message is done by calling a generated method on the receiving task handle.
- The generated send method constructs the message and enqueues it directly.
- A full queue causes the send method to return a queue-full error.
- Queue size is static for each task, but configurable at compile time.
- Message handlers should be able to wait for specific replies without blocking the task thread.
- While one handler waits, the task should continue receiving and handling other messages.
- Synchronous calls and streaming calls should be type checked so a task can only wait for replies or stream events that it declares it can receive.

## Task model

A task consists of:

- a task state object;
- a task handle used by other code to send messages;
- a bounded message queue;
- a task context passed to handlers;
- a dispatch loop running on one OS thread.

Conceptually:

```rust
pub struct TaskHandle<M>
where
    M: TaskMessage,
{
    queue: Arc<TaskQueue<M>>,
}

pub struct TaskContext<M>
where
    M: TaskMessage,
{
    self_handle: TaskHandle<M>,
    next_session_sequence: u64,
    // task-local runtime state for suspended receives/continuations
}
```

Each task has one generated message enum. For example:

```rust
pub enum ServerMessage {
    Start(ServerStart),
    Get(Get),
    Set(Set),
    Shutdown(Shutdown),
}
```

The receiving task owns the meaning and placement of its messages.

## Queue placement

Each declared message has a statically declared placement:

```rust
pub enum MessagePlacement {
    Normal,
    Priority,
}
```

Normal messages are inserted at the tail of the queue.

Priority messages are inserted at the head of the queue.

A single `VecDeque` is sufficient:

```rust
match msg.placement() {
    MessagePlacement::Normal => queue.push_back(msg),
    MessagePlacement::Priority => queue.push_front(msg),
}
```

This means priority messages are LIFO relative to each other. That is intentional for the initial design because priority messages are expected to be exceptional, for example shutdown, cancellation, or urgent control operations.

The sender does not choose the priority. The receiver's message declaration decides it.

For example, if `ServerTask` declares `Shutdown` as priority, then all sends of `Shutdown` to `ServerTask` are priority sends.

## Message categories

There are three main message categories:

```text
message       asynchronous, no reply
call          synchronous, exactly one reply
stream        synchronous/generator-style, zero or more replies followed by end or error
```

Each category can be normal or priority depending on the receiver's declaration.

The matrix is:

```text
              normal              priority
message       async message        priority async message
call          sync call            priority sync call
stream        streaming call       priority streaming call
```

For calls and streams, request priority and reply/event priority are independent:

- the request placement is declared by the callee task, because the callee receives the request;
- the response or stream event placement is declared by the caller task, because the caller receives the response or stream event.

## Task message trait

Every generated task message enum implements `TaskMessage`:

```rust
pub trait TaskMessage: Sized {
    fn placement(&self) -> MessagePlacement;
}
```

Example generated implementation:

```rust
impl TaskMessage for ServerMessage {
    fn placement(&self) -> MessagePlacement {
        match self {
            ServerMessage::Start(_) => MessagePlacement::Priority,
            ServerMessage::Get(_) => MessagePlacement::Normal,
            ServerMessage::Set(_) => MessagePlacement::Normal,
            ServerMessage::Shutdown(_) => MessagePlacement::Priority,
        }
    }
}
```

The queue only needs `M: TaskMessage` to decide where to insert the message.

## Declaring messages

A possible user-facing declaration style:

```rust
#[task(queue_size = 32)]
struct ServerTask {
    state: ServerState,
}

#[messages]
impl ServerTask {
    #[start(priority)]
    async fn start(&mut self, ctx: &mut ServerContext, config: ServerConfig) {
        self.state = ServerState::new(config);
    }

    #[message]
    async fn set(&mut self, ctx: &mut ServerContext, key: String, value: Vec<u8>) {
        self.state.insert(key, value);
    }

    #[call(reply = GetReply)]
    async fn get(&mut self, ctx: &mut ServerContext, key: String) -> GetReply {
        GetReply {
            value: self.state.get(&key).cloned(),
        }
    }

    #[message(priority)]
    async fn shutdown(&mut self, ctx: &mut ServerContext) {
        ctx.stop();
    }
}
```

The macro would generate:

- the task message enum;
- the task handle;
- one handle method per sendable message;
- the queue placement implementation;
- dispatch from enum variant to handler;
- reply and stream plumbing for `call` and `stream` messages.

## Start message

Task creation creates and enqueues a special start message.

The task thread is started first, then the start message is sent to its queue.

This keeps initialization in-band:

```text
create task queue
spawn OS thread running task loop
enqueue Start message
Start handler initializes task state
```

There is no separate out-of-band task initialization path.

## Handler execution and selective receive

Handlers are intended to be `async` functions executed by a task-local runtime.

When a handler awaits a specific reply or stream event, it should not block the task thread. Instead:

```text
handler sends request
handler awaits reply/event
handler continuation is suspended
task returns to receive loop
task handles other messages
matching reply/event arrives
suspended handler resumes
```

The receive loop conceptually does:

```text
pop next message according to queue placement
check suspended waiters
if a waiter matches, resume that waiter
otherwise dispatch the message to its normal handler
```

Matching is by message kind and, for protocol messages, by `SessionId`.

## Compile-time receive checks

A task must declare the reply and stream event messages it can receive.

This can be checked using a generated trait:

```rust
pub trait CanReceive<T>: TaskMessage {
    fn wrap(value: T) -> Self;
}
```

If `ClientTask` declares that it can receive `Response<GetReply>`, the macro generates:

```rust
impl CanReceive<Response<GetReply>> for ClientMessage {
    fn wrap(value: Response<GetReply>) -> Self {
        ClientMessage::GetReply(value)
    }
}
```

A generated synchronous call method then requires:

```rust
CallerMessage: CanReceive<Response<GetReply>>
```

If a handler tries to call a synchronous method but its task has not declared support for the reply message, the code does not compile.

The same model applies to streams:

```rust
CallerMessage: CanReceive<StreamEvent<Row, QueryError>>
```

## Session ID

`mpi-rs` should use a universal `SessionId` for logical interactions that expect future messages.

A `SessionId` identifies a logical exchange, not a single physical queued message.

It is used for:

- synchronous calls;
- streaming calls;
- cancellation;
- matching replies to the correct suspended handler;
- discarding late stream events;
- tracing and debugging.

Recommended type:

```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SessionId {
    origin: EndpointId,
    sequence: u64,
}
```

`origin` identifies the task or external endpoint that created the session. `sequence` is local to that origin. This avoids global synchronization while still producing practically unique session IDs.

Protocol messages implement:

```rust
pub trait HasSessionId {
    fn session_id(&self) -> SessionId;
}
```

## Synchronous calls

A synchronous call is a request message followed by exactly one response message.

User-facing code:

```rust
let reply = server.get(ctx, key).await?;
```

Generated expansion is conceptually:

```rust
let session_id = ctx.next_session_id();

let reply_to = ctx.reply_address::<Response<GetReply>>(session_id);

server.enqueue(ServerMessage::Get(Get {
    session_id,
    reply_to,
    key,
}))?;

let response: Response<GetReply> =
    ctx.receive_session(session_id).await?;

Ok(response.value)
```

The response wrapper is:

```rust
pub struct Response<T> {
    pub session_id: SessionId,
    pub value: T,
}
```

The callee handler can still look like a normal Rust function returning the reply payload:

```rust
#[call(reply = GetReply)]
async fn get(&mut self, ctx: &mut ServerContext, key: String) -> GetReply {
    GetReply { value: self.state.get(&key).cloned() }
}
```

The macro/runtime turns the returned value into `Response<GetReply>` and sends it to the caller.

## Multiple handlers sending the same call

`SessionId` solves the ambiguity where two suspended handlers in the same task send the same synchronous request type to another task.

Example:

```text
handler A sends Get with session 100
handler B sends Get with session 101
server replies to session 101 first
server replies to session 100 second
```

Each suspended handler is waiting for both:

```text
message kind = Response<GetReply>
session_id = its own session
```

So the correct handler resumes even when replies arrive out of order.

## Streaming calls

A streaming call is a request message followed by zero or more stream events and then either end or error.

The stream uses the same `SessionId` model as synchronous calls.

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

A caller should see a Rust-like generator interface:

```rust
let mut rows = database.query(ctx, sql).await?;

while let Some(row) = rows.next(ctx).await? {
    println!("{row:?}");
}
```

The explicit `ctx` argument keeps the design compatible with the task-local runtime. Later, a `futures_core::Stream` implementation may be possible if the stream object can safely capture task-local receive state.

## Stream batching

Streams should support batching from the beginning to avoid one message per item.

Internally, `StreamEvent::Batch` may carry many values. The public `next(ctx).await` API still returns one item at a time by draining a local buffer before awaiting another batch.

Conceptual stream object:

```rust
pub struct MessageStream<T, E> {
    session_id: SessionId,
    control: StreamControlHandle,
    buffer: VecDeque<T>,
    finished: bool,
}
```

`next(ctx)` does:

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

## Stream cancellation

Cancellation is an asynchronous message.

A stream object may send cancel when dropped:

```rust
impl<T, E> Drop for MessageStream<T, E> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.control.try_cancel(self.session_id);
        }
    }
}
```

Because cancellation is asynchronous, drop does not need to await an acknowledgement.

The cancel message uses the same session ID:

```rust
pub struct StreamCancel {
    pub session_id: SessionId,
}
```

Stream cancel should usually be priority in the producer task, because cancellation should not wait behind a long queue of normal work.

## Late stream events

After a stream is cancelled or dropped, some already-sent stream events may still arrive at the caller.

The receive logic may discard stream events whose `SessionId` no longer has an active waiter or active stream object.

This discard rule should be specific to protocol messages such as stream events. Ordinary application messages should not be silently discarded.

A reasonable rule:

```text
if StreamEvent<T, E> arrives and no active session exists:
    discard or log-and-discard

if Response<T> arrives and no active waiter exists:
    dispatch to fallback reply handler, or log according to task policy
```

Late stream events are expected after cancellation. Late one-shot responses are more likely to indicate timeout, cancellation, or a bug, so they are better surfaced to a fallback handler by default.

## Stream producer

A streaming handler should remain within the current task model. It must not create a new task merely to produce stream items.

Producer-side API example:

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

The runtime sends `StreamEvent::End` automatically when the function returns `Ok(())`, or `StreamEvent::Error` when it returns `Err(error)`.

The producer may suspend when backpressured or waiting for stream control messages. While suspended, the task continues handling other messages.

## Flow control

A stream producer must not flood the consumer queue.

The recommended design is credit-based flow control:

```text
consumer grants N credits
producer may send up to N items
consumer grants more credits as the local buffer drains
```

Hidden control messages:

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

The public API should hide this. Users consume streams with `next(ctx).await`.

## External callers

Task-internal synchronous calls must not block the OS thread. They suspend the current handler continuation and return to the task receive loop.

External callers do not have a task queue. They may use separate blocking APIs:

```rust
let reply = server.get_blocking(key)?;
```

External blocking calls can use a one-shot channel internally instead of a task reply message.

The distinction should be explicit:

- task-internal API: `server.get(ctx, key).await?`
- external API: `server.get_blocking(key)?`

This avoids accidental blocking inside task handlers.

## Unix signal context

Normal message construction may allocate memory and is not safe inside a POSIX signal handler.

Unix signal support should therefore be implemented through a bridge:

```text
signal handler writes to eventfd/self-pipe or sets atomic state
signal bridge task/thread observes it
bridge allocates normal Rust message
bridge sends message to target task
```

The public API may make signal forwarding convenient, but the actual POSIX signal handler must only perform async-signal-safe operations.

## Suggested crate structure

```text
crates/
  mpi/
    src/
      lib.rs
      task.rs
      queue.rs
      message.rs
      runtime.rs
      receive.rs
      session.rs
      stream.rs
      error.rs
      signal.rs
  mpi-macros/
    src/
      lib.rs
      task_macro.rs
      message_macro.rs
examples/
  ping_pong.rs
  synchronous_call.rs
  streaming_query.rs
  priority_shutdown.rs
```

The `mpi` crate contains the runtime primitives.

The `mpi-macros` crate contains proc macros for task and message declarations.

## Initial implementation phases

A practical implementation order:

1. Bounded task queues with normal/priority insertion.
2. Task handles and generated asynchronous message send methods.
3. Start message and task spawn API.
4. Generated message enums and dispatch loops.
5. Async handlers run by a task-local executor.
6. Selective receive by message kind.
7. `SessionId`, `Response<T>`, and synchronous calls.
8. Compile-time `CanReceive<T>` checks for replies.
9. `StreamEvent<T, E>`, `MessageStream<T, E>`, and stream cancellation.
10. Stream batching and credit-based flow control.
11. Safe Unix signal bridge.
12. Diagnostics, timeouts, tracing, and deadlock/debug support.

## Core rule summary

```text
Every task owns its message queue.
Every task declares the messages it can receive.
Every message has receiver-declared queue placement.
Normal messages go to the queue tail.
Priority messages go to the queue head.
Synchronous calls are request messages plus typed Response<T> messages.
Streaming calls are request messages plus typed StreamEvent<T, E> messages.
SessionId identifies the logical interaction for calls and streams.
A task can only await replies or stream events it declares it can receive.
Handlers suspend on receive instead of blocking the OS thread.
Late stream events after cancellation may be discarded.
Everything received by a task is still a message.
```
