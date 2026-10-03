---
status: accepted
---

# Open the Model Gateway Seam Between Committed Dispatch Boundaries

On 2026-10-03, before Stage 4 execution, the author accepted this decision. StoryOS adds one Model Gateway module in `storyos-application`. That module owns the complete dispatch sequence for each model destination request. The Worker calls it. Each destination is a Model Provider Adapter at one seam. The first two adapters are the Contract-Faithful Fake Destination and the Volcengine Agent Plan Responses route of [ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md). No transaction is open during destination I/O. This decision gives Stage 4 one place for the real model path, and it keeps that path equal to the fake path that Stage 3 proved.

## Context

At `main` `4ddf9381`, StoryOS has no model, destination, or Provider interface. `AgentRunWorkStore::complete_agent_run` in `storyos-application` only passes the claim to the store. The complete AgentRun phase sequence is in `storyos-adapter-postgres/src/agent_run_work.rs` (`settle_one_phase`). It runs inside serializable transactions. The fake model result comes from a pure `storyos-core` function, and the adapter calls that function inside each phase transaction. The adapter writes the original-result retrieval result, the successor conditions, and the late result into the Model Attempt JSON payload when it creates the Attempt. Only tests can supply a confirmed reference expiry, and they use SQL patches. The Contract Fault Points for dispatch and stream are environment-variable file holds in the production adapter.

A real HTTP request cannot occur at that location. ADR 0005 requires a committed manifest and a committed dispatch claim before I/O. ADR 0007 permits external I/O only in the Worker. If Stage 4 does not change this shape, the real path must add HTTP, TLS, and credential code to the PostgreSQL adapter, or it must copy the phase sequence. Both results break S4-REQ-002, which requires the same path that the fake stage proved.

## Decision

### One Model Gateway module owns the sequence

The Model Gateway module in `storyos-application` does these steps for each destination request, in this order:

1. It gets the next work for one claimed AgentRun from the store. The store settles the phases that need no destination I/O.
2. It tells the selected Model Provider Adapter to prepare the request. The adapter returns the non-secret Wire Payload Projection and a prepared value. A refusal at this step is a proven failure before the dispatch claim. It creates no Outbound Disclosure Event.
3. It commits the dispatch claim through the store. One transaction records the Destination Attempt Admission Decision, the Wire Payload Projection, the Model Attempt or Destination Attempt, and the Outbound Disclosure Event as OutcomeUnknown.
4. It tells the adapter to do the exchange. No transaction is open. The adapter sends ordered Model Stream Events to a sink that the store supplies.
5. It commits the observation through the store, after a check of the Run Lease fence.

The order of these steps is written in this module only. A new destination cannot change it.

### The Model Provider Adapter seam

The seam has two operations:

- `prepare` takes one closed destination request: Create, Retrieve, or Abort. It returns the Wire Payload Projection and a prepared value, or a pre-dispatch refusal.
- `exchange` consumes the prepared value once and returns one observation.

These rules apply:

- An observation has no error channel. A dropped stream, a timeout, a lost lease, or a stop after cancellation becomes an observation. The adapter does not decide retryability, fallback, or ToolCall execution.
- A Create observation can be: proven not submitted, a Provider rejection with its native reason, a terminal response, or OutcomeUnknown with any known response reference. A confirmed continuation expiry is a Provider rejection with that reason.
- The adapter returns native items, usage, and references only. It does not report that a result is selected or that it advances the continuation. Core validation of the complete Agent Decision decides both.
- Usage is reported, estimated, or unknown. Unknown usage is never zero usage.
- The sink commits Model Stream Events in short fenced transactions: at each terminal item event, and after a bounded number of provisional events. S4-06 selects that bound under its existing owner. The sink tells the adapter to stop when the Run Lease is stale or a Model Attempt Cancellation is durable.
- Only the store can make an Abort request, and only after the Model Attempt Cancellation is committed.

### Retrieve and Abort are destination requests

Original-result retrieval and Provider abort each send a reference and a credential to the destination. Each gets its own Destination Attempt, dispatch claim, and Outbound Disclosure Event through the same sequence. This applies the rule in [ADR 0034](0034-bound-provider-hosted-tool-operations.md) that retrieval for reconciliation has its own admitted Destination Attempt. Retrieval never sends the original request again.

### Credential resolution

The adapter resolves the Credential Reference in `prepare`, before the dispatch claim. Thus a missing credential is a proven failure before the claim, and it creates no Outbound Disclosure Event. The resolved value stays only in the prepared value. That value cannot be copied, printed, serialized, or used two times. No durable record, log, or digest contains the value.

### Crate placement

- `storyos-application` contains the Model Gateway module and its ports: the dispatch store, the Model Provider Adapter, the stream sink, and the credential resolver.
- `storyos-adapter-fake-destination` contains the Contract-Faithful Fake Destination. It takes the scripted fake planner from `storyos-core` and the prepared recovery observations from the PostgreSQL adapter. It derives each result from the request and the reference. It keeps no process memory that a restart can lose.
- `storyos-adapter-volcengine-responses` contains the Agent Plan Responses adapter. It is the only crate that depends on an HTTP client and TLS.
- `storyos-adapter-postgres` keeps SQL and persistence. It gets no HTTP, TLS, or credential dependency.
- `storyos-worker` is the composition root that connects these adapters.
- Pre-dispatch refusal of an unavailable capability is a Host decision. It stays in `storyos-core` and does not move into the fake adapter.

### The Server process does not hold Provider credentials

[ADR 0007](0007-preserve-process-separable-server-worker-boundary.md) permits a co-located Worker. The Worker loop that runs inside the Server process uses only the Contract-Faithful Fake Destination. The real route needs a separate `storyos-worker` process. The `storyos-server` crate must not depend on `storyos-adapter-volcengine-responses`, directly or through another crate. Thus the dependency graph enforces this rule.

### The first real route runs on the author's Mac

The Foundation-local Credential Reference resolver is macOS Keychain ([ADR 0004](0004-adopt-postgresql-service-and-project-isolation-boundary.md)). Environment variables are inputs for development and tests only. The production Worker runs on a Linux VPS ([ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md)), and no current contract names a Linux credential backend. Therefore Stage 4 qualification and the real-author journey run on the author's Mac local deployment with Keychain. The real route on the Linux VPS stays unavailable. A later ADR must select the Linux credential backend before that route can be enabled.

### Contract Fault Points

After each commit, the Model Gateway module reports the reached Contract Fault Point to an injected observer. Production uses an observer that does nothing. Rust tests use a deterministic barrier. Process tests use an observer that `storyos-worker` builds from test configuration. Adapter crates do not read environment variables for fault points. The dispatch and stream file holds leave `storyos-adapter-postgres`. The decision, compaction, and successor holds leave when their phases move.

### The rest of the phase sequence

The remaining phase logic stays in `storyos-adapter-postgres` for now. These rules apply from this decision forward:

- New Stage 4 phase logic goes into `storyos-application` or `storyos-core`, not into `storyos-adapter-postgres`.
- A ticket that changes an existing phase first moves the decisions of that phase out of the PostgreSQL adapter.

## Considered options

- An HTTP client in `storyos-adapter-postgres` was rejected. It makes a persistence crate own external I/O and credentials. It also puts I/O next to the serializable transaction code.
- A separate real-model phase sequence beside the fake sequence was rejected. The real path and the fake path would differ, and Stage 3 proof would not apply to Stage 4.
- One adapter method for all requests, without a prepare step, was rejected. A missing credential would then appear after the dispatch claim and create an unnecessary OutcomeUnknown Event. Impossible pairs, such as an Abort request with a completed response, would also be possible.
- A generic operation trait with associated event and observation types was rejected. Three known requests do not need it, and it adds type ceremony for each request.
- Moving the complete phase sequence out of the PostgreSQL adapter before Stage 4 was rejected. It moves more than 2,000 lines. The incremental rule above gives the same direction at lower risk.
- Ordinary environment variables for the production credential were rejected. They contradict the Credential Reference contract.

## Consequences

- A prerequisite specification delivers this seam before S4-02, S4-04, and S4-06. It changes no Stage 3 behavior. The existing Stage 3 tests prove it with the fake adapter, and it needs no Provider authorization.
- Recovery tests get retrieval results, late results, and reference expiry as fake adapter observations. They no longer patch the Model Attempt payload with SQL.
- S4-01 and S4-02 bind the Volcengine adapter and the Keychain resolver through these ports. They do not add another dispatch path.
- A local real-route session starts the Server with the in-process Worker turned off, and starts a separate `storyos-worker` process.
- The Linux credential backend and the production real route remain open. This decision does not authorize an external account, spend, or deployment change.
