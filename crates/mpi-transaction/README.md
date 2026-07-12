# mpi-transaction

`mpi-transaction` contains transaction coordination support that depends on
message-based workspace services. The currently implemented surface is
`TransactionDecisionLog`, which records commit or abort decisions through
`persistent-log-storage-service`.

## What It Solves

After a transaction reaches a durable commit or abort decision, recovery must
continue that recorded outcome. This crate stores decision records in the
persistent log service and decodes them during restart recovery.

## How It Interacts With Other Crates

- Uses `mpi-core` transaction identifiers, transaction paths, and
  `TransactionDecision`.
- Depends on `persistent-log-storage-service` for durable storage.
- Is re-exported by the `mpi` facade with `enable-mpi-transaction`.
- Does not decide business success or failure. It records decisions already
  made by transaction coordination code.

## Usage

Examples are in `examples/`:

- [record_and_recover.rs](examples/record_and_recover.rs): record commit and
  abort decisions, restart the storage service, and recover the records.

## Best Practices

Record only durable transaction decisions, not arbitrary progress notes.

Use `TransactionPath` to preserve whether the decision belongs to a root or
nested transaction. `SessionId` matches individual message interactions;
`TransactionPath` identifies transaction membership.

Handle `TransactionLogError` explicitly. It distinguishes call failures,
storage-service errors, and invalid records found during recovery.

Ignore non-transaction records in the same persistent log only if they use a
different record prefix. `TransactionDecisionLog` filters for its own record
magic and validates matching records strictly.
