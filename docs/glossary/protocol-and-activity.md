# Glossary: Commands, protocol, and Activity Stream

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Command Acknowledgement**:
The idempotently replayable public result of submitting one exact command, returned as Committed with its immutable Receipt only after the complete Core Transition commits, or as Accepted with a durable operation reference when later asynchronous settlement remains. Accepted proves only durable admission and never success; its settlement is observed through a bounded query or the Project Activity Stream rather than a delayed HTTP result.
_Avoid_: HTTP success as domain success, in-memory job acknowledgement, long-poll completion, duplicate execution after lost response

**Command Idempotency Fence**:
The compact Project Scope-bound continuation of one settled command's idempotency arbiter, preserving its command kind, key, digest, Command ID, immutable acknowledgement or replayable acknowledgement reference, final Receipt or operation reference, and retention provenance after larger execution payloads leave hot storage. A matching retry replays the same logical acknowledgement through current authorized redaction without re-executing; a different digest conflicts, and a known key never becomes a new command through expiry or compaction.
_Avoid_: Expired key reuse, response-cache entry, new command with old key, best-effort duplicate filter

**Protocol Compatibility Profile**:
The pre-1.0 same-release contract that binds one deployed Web Client, Server, Worker, generated client, public schemas, Event catalog, and Protocol Limit Profile. A release mismatch produces `upgrade_required` before domain admission or cursor advancement, while stored historical facts retain their own schema identity and project through the active release.
_Avoid_: Mixed-release runtime, ambient compatibility window, client-guessed safety semantics

**External Contract Compatibility Decision**:
The immutable Host result created only after one exact Project Scope-bound external-use binding exists, admitting or rejecting that binding against its global Registration and Adapter revision, protocol, schema or Tool digest, capability snapshot, wire mapping, exact Processing Destination Identity, Credential binding generation when applicable, and effect ceiling. It references but never creates, contains, or mutates the use binding; the binding never points forward to a Decision. Changing any binding field creates a new binding and then a new Decision, while changed observed protocol, schema, capability, or wire evidence that leaves the pinned binding and Registration/Adapter tuple unchanged creates only a new Decision. A global contract or Adapter observation contains no Project data, Credential Reference, actual account, or disclosure destination and cannot itself admit use. Every external use pins one exact observed contract and Adapter mapping; drift quarantines new use, and any widening of destination, disclosure, Credential binding, effect, or capability requires new authorization as applicable plus the corresponding new binding and Decision, while historical work remains bound to both original records.
_Avoid_: Semver-range trust, Provider alias compatibility, handshake as authorization, silent SDK upgrade, permanent external-version support

**Protocol Limit Profile**:
An immutable versioned contract fixing public validity ceilings and counting meaning for byte, item, depth, time, token, attempt, replay, expansion, rate, and concurrency at every public and external protocol crossing. Every numeric or semantic change creates a new Profile Revision activated with its matching StoryOS release, and dynamic resource pressure may only produce temporary rate or concurrency admission. Each Receipt, Attempt, Snapshot, and limit outcome binds both the Profile Revision and the actual effective bounds frozen from exact policy, grant, destination, and counting-profile inputs, while authors receive no routine limit configuration burden.
_Avoid_: Scattered magic limit, client-requested expansion, same-revision narrowing, unversioned token counting, author-facing protocol tuning

**Application Wire Record**:
The immutable non-secret evidence retaining the exact schema-valid message-content bytes, wire schema and profile, typed-record reference, and digest for an authorized durable command or admission and for each supported public Event representation. It excludes transport headers, cookies, authorization and anti-forgery material, credential-bearing envelopes, unauthorized or malformed request bodies, repeated SSE deliveries, and rebuildable Query response bytes; canonical semantic bytes never stand in for separately claimed original wire bytes.
_Avoid_: HTTP traffic capture, request log, Query-response archive, JCS digest as original wire evidence

**Project Activity Stream**:
The one canonical public replay stream for an exact Project Scope, assigning every committed client-visible activity event a strictly increasing project-local position while preserving its typed identity, cause, and any owning Run or aggregate sequence. Run-, Artifact-, and other filtered streams are cursor-bound derived views rather than separate truth streams; every cursor is bounded by its Replay Generation and resumes only through that generation or a fresh Activity Stream Resync, while Mailbox, Worker, Provider, MCP, and Adapter protocols remain outside this public envelope.
_Avoid_: Global event bus, per-Run truth stream, internal event log, universal protocol envelope

**Replay Generation**:
A project-local bounded replay epoch of the Project Activity Stream, published with one authorized replay floor and fresh Snapshot at a compaction or archival boundary. A cursor never crosses generations: a below-floor cursor fails explicitly rather than being translated, guessed, or silently advanced.
_Avoid_: Infinite cursor migration, guessed offset, stream fork

**Authority History Floor**:
The Project Scope Activity position after which every successful Manuscript Structure Transition has a complete Authoritative Commit and Author Action. Earlier structural Activity stays immutable and is not rewritten into those records.
_Avoid_: Replay Floor, Replay Generation, invented backfill, Barrier as history rewrite

**Activity Stream Resync**:
The authorized recovery from an expired Project Activity cursor that loads a fresh canonical Snapshot and resumes strictly after its recorded Activity position. It exposes the replay-generation boundary and never treats a cursor-too-old failure as an empty stream, a successful replay, or permission to skip historical facts.
_Avoid_: Cursor translation, silent reset, empty history

**Canonical Query Snapshot**:
An authorized, time-bounded stable reading boundary over Project Scope-bound durable facts, binding its Activity position, query/view inputs, redaction, schema, and replay generation. It may expire and be reissued, but is neither a Run Checkpoint, a backup, nor a permanent second copy of history.
_Avoid_: Run Checkpoint, backup, permanent query result, live process view, Pinned Export Source

**Canonical Query**:
A public read of exact Authoritative State, Artifact, Receipt, Approval, Run, or other canonical facts at one committed Project Scope-bound Snapshot. It supports read-your-acknowledgement against a required Project Activity Stream position, and every page remains bound to the same Snapshot and stable order or fails with an explicit resync outcome.
_Avoid_: Eventually consistent authority read, mixed-Snapshot pagination, cache result as current truth

**Projection Query**:
A public read of one bounded rebuildable search, embedding, retrieval, history, or other projection, returning its exact source Snapshot, projection watermark, completeness, and lag. It never presents lag as an empty canonical result; an unmet required watermark returns an explicit projection-not-ready outcome, while Snapshot lifetime remains a retention policy.
_Avoid_: Canonical Query, hidden eventual consistency, empty result for stale projection, unbounded projection dump

**Command-response Project**:
The three-field Project projection captured with a command outcome: Project identity, title, and open state including Current Chapter when present. First delivery and exact retry use this stored projection. An independent Project query continues to return current state.
_Avoid_: Live Project read in a command acknowledgement, manuscript text, whole Snapshot

**Historical acknowledgement unavailable**:
The explicit Problem for a known pre-capture command whose complete original acknowledgement cannot be proved. It is not command failure, and it is not a license to present live state as the old reply.
_Avoid_: Generic network error, fabricated success, live-state backfill
