# mpi-rs

`mpi-rs` is intended to be a Rust message passing infrastructure inspired by the task and signal model of Enea OSE, but using Rust terminology and Rust-friendly APIs.

The core abstraction is a **task**: an operating system thread with an associated bounded message queue. Each task declares the messages it can receive. Generated task handle methods allocate/build the corresponding message and enqueue it directly into the receiving task's queue.

The current design direction is documented in:

- [Message model design](docs/message-model.md)

The design covers:

- task-local bounded message queues;
- statically declared message sets;
- normal and priority FIFO queue placement;
- events, meaning asynchronous messages with no reply;
- synchronous calls with typed replies;
- generator-style streaming calls;
- a universal `SessionId` used for both single replies and streams;
- compile-time checks that a caller task can receive the reply or stream events it awaits;
- Rust-like handler APIs using async continuations rather than blocking task threads.

This repository is currently in the design phase.
