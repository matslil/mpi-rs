# Protocol Design Baseline

This document defines the baseline protocol model for `mpi-rs`.

Protocols are intended to make message contracts reusable across tasks,
applications, and eventually separate binaries that may be built and released
on different roadmaps.

## Purpose

A protocol is a named, exported message contract. It defines the events, calls,
streams, replies, stream events, and stream errors that belong to that protocol
and the explicit Rust types associated with each protocol message.

The protocol model is the source of truth for compile-time receive checks.
Task declarations decide which protocol messages a task handles and which
protocol replies or stream events the task can receive while waiting. A receive
declaration is valid for a protocol reply or stream event when it matches the
protocol-qualified message identity and the Rust type declared by the protocol.

## Protocol identity

A protocol belongs to a namespace. In Rust, the namespace is represented by the
module or crate path that exports the protocol declaration.

A protocol message identity is formed from:

- protocol namespace;
- protocol name;
- message name.

Conceptually:

```text
namespace::ProtocolName::MessageName
```

Protocol and message names only need to be unique within their defining
namespace. Two protocols may use the same short message name without conflict
because the namespace-qualified identity differs.

Protocol interaction names should use Rust-style `snake_case`. Generated Rust
modules for protocol interactions use that same `snake_case` name. Generated
receive identity types inside an interaction module use Rust-style `PascalCase`
names:

- a call named `get` has a generated reply receive identity `get::Reply`;
- a stream named `list_directories` has generated stream receive identities
  `list_directories::Item`, `list_directories::Finish`, and
  `list_directories::Error`.

When rendered as external protocol message names, those derived identities are
conceptually equivalent to `get_reply`, `list_directories_item`,
`list_directories_finish`, and `list_directories_error`.

## Explicit message types

Protocol message declarations explicitly name all Rust types carried by the
protocol message interaction:

- event declarations name the event payload type;
- call declarations name the request payload type and reply payload type;
- stream declarations name the request payload type, stream item type, and
  stream error type.

Conceptual declaration shape:

```rust
protocol! {
    pub protocol InventoryV1 {
        event reindex(ReindexRequest);
        call get_item(GetItemRequest) -> GetItemReply;
        stream watch_stock(WatchStockRequest) -> StockEvent error WatchStockError;
    }
}
```

The generated Rust API should behave like a typed Rust function surface: the
selected protocol-qualified message determines the request payload type and the
reply, stream item, or stream error type.

## Protocol-derived sending

Send, call, and stream operations should be generated from the protocol
declaration or from a derivative of it.

A protocol derivative may bind a protocol declaration to a concrete task,
endpoint, or handle that implements a specific instance of the protocol. The
derivative identifies where a protocol message is sent, while the protocol
continues to define the message identity and the Rust types that make the send
operation type-correct.

## Compatibility

Published protocols are append-only.

After a protocol is published, existing protocol message declarations and their
associated payload, reply, stream item, and stream error types shall not be
changed or removed.

Compatible protocol evolution may add new message declarations to the protocol.

Incompatible protocol evolution requires a new protocol name. The recommended
convention is to include a version suffix in the protocol name, such as
`InventoryV2`.

The baseline does not require a generated protocol fingerprint. Compatibility is
defined by the namespace-qualified protocol identity, explicit message type
declarations, and the append-only process rule.

## Separate binaries

Applications in separate binaries can share a protocol by depending on the same
protocol crate or otherwise sharing the same exported protocol declaration.

Compile-time guarantees apply within each compiled binary: a caller compiled
against a protocol declaration knows the expected payload and reply types, and a
task can only await protocol replies or stream events that its message enum is
declared to receive.

Remote endpoints may still need runtime capability discovery or connection
failure behavior for cases where a peer does not implement a requested
namespace-qualified protocol message. That runtime behavior is separate from the
compile-time type relationship established by the protocol declaration.
