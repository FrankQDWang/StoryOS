---
status: accepted
---

# Open the Model Gateway Seam Between Committed Dispatch Boundaries

On 2026-10-03, before Stage 4 execution, the author accepted this decision. StoryOS adds one Model Gateway module in `storyos-application`. That module owns the complete dispatch sequence for each model destination request, and the Worker calls it. StoryOS reaches each destination through a Model Provider Adapter at one seam. The first two adapters serve the Contract-Faithful Fake Destination and the Volcengine Agent Plan Responses route of [ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md). No transaction is open during destination I/O. Thus Stage 4 gets one place for the real model path, and that path stays equal to the fake path that Stage 3 proved.

## Context

At `main` `4ddf9381`, StoryOS has no model, destination, or Provider interface. The application function `complete_agent_run` only sends the claim to the `AgentRunWorkStore`. The complete AgentRun phase sequence is in `storyos-adapter-postgres/src/agent_run_work.rs` (`settle_one_phase`). It runs inside serializable transactions. The fake model result comes from a pure `storyos-core` function. The adapter calls that function inside each phase transaction. When the adapter creates a Model Attempt, it also writes the retrieval result, the successor conditions, and the late result into the Attempt payload. Only tests can supply a confirmed reference expiry, and they use SQL patches. The Contract Fault Points are environment-variable file holds in the production adapter.

A real HTTP request cannot occur at that location. [ADR 0005](0005-require-ordered-context-assembly-before-destination-disclosure.md) requires a committed manifest and a committed dispatch claim before I/O. [ADR 0007](0007-preserve-process-separable-server-worker-boundary.md) permits external I/O only in the Worker. If Stage 4 keeps this shape, the real path must add HTTP, TLS, and credential code to the PostgreSQL adapter. Or it must copy the phase sequence. Both results break S4-REQ-002, which requires the same path that the fake stage proved.

## Decision

### Terms in this record

These are design terms of this decision, not GLOSSARY terms:

- A **prepared value** is the single-use result of an adapter preparation. It holds what the exchange needs, including the resolved credential.
- An **observation** is the complete result of one exchange, as the adapter reports it. It is different from a Model Stream Event, which is one ordered event inside the exchange.
- The **sink** is the store-supplied receiver of the Model Stream Events of one exchange.

### One Model Gateway module owns the sequence

The Model Gateway module in `storyos-application` does these steps for each destination request, in this order:

1. It gets the next work for one claimed AgentRun from the store. The store settles the phases that need no destination I/O. It returns a destination request only after gate six of ADR 0005 committed the Context Assembly Manifest.
2. It tells the Model Provider Adapter to prepare the request, as the seam section below describes.
3. It commits the dispatch claim through the store. One transaction records the Destination Attempt Admission Decision, the Wire Payload Projection, the Destination Attempt, and the Outbound Disclosure Event. For a Create request, the Destination Attempt is a Model Attempt. The Event starts as OutcomeUnknown. The Wire Payload Projection and the Event commit together, before any I/O.
4. It tells the adapter to do the exchange. No transaction is open. The adapter sends ordered Model Stream Events to the sink.
5. It commits the observation through the store, after a check of the Run Lease fence.

A pre-dispatch refusal at step 2 is a proven failure before the dispatch claim. It creates no Destination Attempt and no Outbound Disclosure Event. The record operation of the store records the refusal on the AgentRun, as other refusals before dispatch are recorded.

The order of these steps is written in this module only. A new adapter cannot change it.

### The store port

The dispatch store port has four operations. Each operation is one committed transaction with a check of the Run Lease fence.

- **Get the next work** returns the next destination request, or the settled AgentRun.
- **Commit the dispatch claim** does step 3.
- **Append Model Stream Events** commits a batch of events. When a Model Attempt Cancellation is durable, it also returns the abort ticket.
- **Record** commits the observation of an exchange. It also records a pre-dispatch refusal on the AgentRun.

### The Model Provider Adapter seam

The seam has two operations:

- `prepare` takes one closed destination request: Create, Retrieve, or Abort. It returns the non-secret Wire Payload Projection and a prepared value, or a pre-dispatch refusal.
- `exchange` consumes the prepared value one time and returns one observation.

These rules apply:

- An observation has no error channel. A dropped stream, a timeout, a lost lease, or a stop after cancellation becomes an observation. The adapter does not decide retryability, fallback, or ToolCall execution.
- A Create observation is one of these:
  - proven not submitted;
  - a destination rejection with its native reason;
  - a terminal response;
  - OutcomeUnknown with any known response reference.
- A confirmed continuation expiry is a destination rejection with that reason.
- The adapter returns native items, usage, and references only. It does not report that a result is selected or that it advances the continuation. Core validation of the complete Agent Decision decides both.
- Usage is reported, estimated, or unknown. Unknown usage is never zero usage.
- The sink commits Model Stream Events in short fenced transactions. It commits at each terminal item event and after a bounded number of provisional events. S4-06 selects that bound under its existing owner. The sink tells the adapter to stop when the Run Lease is stale or a Model Attempt Cancellation is durable.

### Retrieve and Abort are destination requests

Original-result retrieval and Provider abort each send a reference and a credential to the destination. Each gets its own Destination Attempt, dispatch claim, and Outbound Disclosure Event through the same sequence. This applies the rule in [ADR 0034](0034-bound-provider-hosted-tool-operations.md) that retrieval for reconciliation has its own admitted Destination Attempt. Retrieval never sends the original request again.

An Abort request needs an abort ticket. Only the store can issue an abort ticket, and only after the Model Attempt Cancellation commits. The append operation returns the ticket, and the sink gives it to the module when it tells the adapter to stop. The types permit no other way to make an Abort request.

### Credential resolution

The adapter resolves the Credential Reference at step 2, before the dispatch claim. Thus a missing credential is a pre-dispatch refusal. The resolved value stays only in the prepared value. That value cannot be copied, printed, serialized, or used two times. No durable record, log, or digest contains the value.

### Crate placement

- `storyos-application` contains the Model Gateway module and its ports: the dispatch store, the Model Provider Adapter, the sink, and the credential resolver.
- `storyos-adapter-fake-destination` contains the adapter for the Contract-Faithful Fake Destination. It takes the scripted fake planner from `storyos-core`. It also takes the recovery observations that the PostgreSQL adapter now writes in advance. It derives each result from the request and the reference. It keeps no process memory that a restart can lose.
- `storyos-adapter-volcengine-responses` contains the Agent Plan Responses adapter. It is the only crate that depends on an HTTP client and TLS.
- `storyos-adapter-postgres` keeps SQL and persistence. It gets no HTTP, TLS, or credential dependency.
- The `storyos-worker` library contains the Worker loop. It does not depend on a Provider adapter.
- One binary package composes the Worker loop with the PostgreSQL store, the fake and Volcengine adapters, and the credential resolver. No other package depends on that binary package.
- The Worker binary moves into that package and keeps its name. For each request, it uses the adapter that the Model Registration binds. Thus process tests and the real route use the same Worker binary.
- The capability refusal before dispatch is a Host decision. It stays in `storyos-core` and does not move into the fake adapter.

### The Server process does not hold Provider credentials

ADR 0007 permits a co-located Worker. The Worker loop inside the Server process uses only the fake adapter. It does not claim an AgentRun whose Model Registration binds another adapter. It never uses the fake adapter in place of another adapter. The real route needs the separate Worker binary. `storyos-server` depends on the `storyos-worker` library and the fake adapter only. It must not depend on `storyos-adapter-volcengine-responses`, directly or through another package. Thus the dependency graph enforces this rule.

### The first real route runs on the author's Mac

[ADR 0004](0004-adopt-postgresql-service-and-project-isolation-boundary.md) names macOS Keychain as the Foundation-local Credential Reference resolver. It also names a managed secret service for a later controlled-cloud deployment under the same resolver contract. Environment variables are inputs for development and tests only. [ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md) puts the production Worker on a Linux VPS. That deployment is neither the Foundation-local Mac nor a controlled-cloud deployment. No contract selects its resolver.

Therefore Stage 4 qualification and the real-author journey run on the author's Mac local deployment with Keychain. The real route on the Linux VPS stays unavailable. A later ADR must select the resolver for that deployment under the ADR 0004 contract before that route can be enabled.

### Contract Fault Points

After each commit, the Model Gateway module reports the reached Contract Fault Point to an injected observer. Production uses an observer that does nothing. Rust tests use a deterministic barrier. Process tests use an observer that the composition root builds from test configuration.

The dispatch and stream file holds leave `storyos-adapter-postgres` with this change. The decision, compaction, successor, and conversation holds stay there for now. Each one leaves when its phase moves. New fault points use the observer, not environment variables in an adapter crate.

### The rest of the phase sequence

The remaining phase logic stays in `storyos-adapter-postgres` for now. These rules apply from this decision forward:

- New Stage 4 phase logic goes into `storyos-application` or `storyos-core`, not into `storyos-adapter-postgres`.
- A ticket that changes an existing phase first moves the decisions of that phase out of the PostgreSQL adapter.

## Considered options

- An HTTP client in `storyos-adapter-postgres` was rejected. It makes a persistence crate own external I/O and credentials. It also puts I/O next to the serializable transaction code.
- A separate real-model phase sequence beside the fake sequence was rejected. The real path and the fake path would differ, and the Stage 3 proof would not apply to Stage 4.
- One adapter operation for all requests, without a preparation step, was rejected. A missing credential would then appear after the dispatch claim. It would create an unnecessary Outbound Disclosure Event in OutcomeUnknown. It would also permit impossible pairs, such as an Abort request with a completed response.
- A generic operation trait with associated event and observation types was rejected. Three known requests do not need it, and it adds type ceremony for each request.
- A move of the complete phase sequence out of the PostgreSQL adapter before Stage 4 was rejected. It moves more than 2,000 lines. The incremental rule above gives the same direction at lower risk.
- Ordinary environment variables for the production credential were rejected. They contradict the Credential Reference contract.

## Consequences

- A prerequisite specification delivers this seam before S4-02, S4-04, and S4-06. It changes no Stage 3 behavior. The existing Stage 3 tests prove it with the fake adapter, and it needs no Provider authorization.
- Recovery tests get retrieval results, late results, and reference expiry as fake adapter observations. They do not patch the Model Attempt payload with SQL.
- S4-01 and S4-02 connect the Volcengine adapter and the Keychain resolver through these ports. They do not add another dispatch path.
- For a local real-route session, the Server starts with its in-process Worker disabled, and the separate Worker binary starts.
- The Linux VPS resolver and the production real route remain open. This decision does not authorize an external account, spend, or deployment change.
