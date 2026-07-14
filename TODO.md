Todo list
=========

NOTE: Any AI agent: Ignore this file!

- timeout-service: Copy too heavy, use SessionID only for timeout requests?

- persistent-log-service: Cleanup of discarded log entries from disk?

- Make it a compiler error to access state without "with_state" clause

- mpi-macro: Generate task creation method automatically?

- Implement os-event bridges for other supported platforms (Windows, MacOS, iOS, Android)

- mpi: Are tasks dynamic or static right now?

- os-event-bridges: Add handling of command line arguments sent as message?

- subscription-service: Works like a mailing list

- broker-service: Service registry

- mpi: Dead task handling:
  - Special message, similar to start, telling of dead tasks, received instead of expected reply
  - Send method call should now be able to return error for task dead

- Protocol bridge service: For each protocol, implement a message <-> protocol bridge
  - AMQP (requires transactions)
  - XMPP (for messaging support)
  - Kafka (for cloud solutions)
  - STOMP (for good browser integration)
  - Google trace (to publish trace information)

- Tracking support (optional)
  - Be able to hook up on LTTNG when using Linux
  - Be able to produce CTF trace logs, regardless of LTTNG
  - Negible overhead when disabled, low-overhead when enabled

- Log support (optional)
  - Log to system default place
  - Gives a platform independent messaging interface for logging

- Design documentation together with source, using Mermaid for illustrations

