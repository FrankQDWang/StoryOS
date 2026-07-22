---
status: accepted
---

# Preserve a Process-Separable Server and Worker Boundary

StoryOS keeps the Server and Worker independently startable and deployable within one modular monolith. The Server owns public HTTP/SSE transport and trusted request admission; Core owns authoritative transitions; the Worker performs only durably claimed asynchronous or external work through Core-owned contracts. The Foundation default may co-locate them for zero-configuration local use, without treating that convenience as a recovery or authority shortcut.

## Considered options

- An HTTP-process background thread was rejected because request lifecycle, timeout, restart, lease, fence, outbox, and OutcomeUnknown recovery would not have a durable process boundary.
- A mandatory multi-service deployment was rejected because it would add operational configuration and service coordination without adding an authority boundary or validating Discovery Writing.

## Consequences

- Server and Worker share the same PostgreSQL authority and contracts; no broker, worker database, or internal HTTP API becomes a source of truth.
- This decision does not choose the Foundation process manager, container topology, worker count, queue library, or deployment provider.
