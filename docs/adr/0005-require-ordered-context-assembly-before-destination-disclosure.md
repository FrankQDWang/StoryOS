---
status: accepted
---

# Require Ordered Context Assembly Before Destination Disclosure

StoryOS requires every destination-bound context item to advance through seven
ordered gates: Operation Requirement Determination, Candidate Discovery,
Source Eligibility Gate, Selection and Ranking, Bounded Projection, Context Assembly
Manifest Commit, then Destination-specific Disclosure and Attempt. The gates
cannot be skipped, merged, inverted, or reconstructed after the fact. An
invalid requirement or Blocked sufficiency decision terminates before gate
seven with no destination I/O; a Complete or explicitly Degraded assembly may
reach a Destination Attempt only after gate six commits. Host-internal assembly
is not represented as a fake Destination Attempt. Failure to commit the
immutable Context Assembly Manifest prevents destination I/O.

Eligibility precedes ranking; selection grants no truth, authority, evidence
status, or disclosure permission. Every destination receives its own
minimum-necessary projection. A lossy projection is a new source-bearing item,
and any model-, Tool-, or externally generated projection is a separate
operation that recursively crosses the same gates. Tool and MCP results cross
the complete boundary again before later model use.

Context Assembly, destination, wire, disclosure, and Destination Attempt
records establish different facts. They preserve exact Project Scope and historical evidence but
never claim that a model internally attended to or used supplied content.
Cache, compaction, prior-response, and provider continuity optimize processing
only after current source, permission, applicable Memory Suppression for a
memory-derived or ordinary-recall path, lifecycle, Project Scope, and
destination eligibility are revalidated.

The full normative contract is [Context Assembly, Retrieval, and Outbound
Disclosure Semantics](../foundation/context-assembly-retrieval-and-outbound-disclosure-semantics.md).
This decision resolves [Wayfinder issue 54](https://github.com/FrankQDWang/StoryOS/issues/54)
and operates under the ownership and deployment boundary in [ADR
0004](0004-adopt-postgresql-service-and-project-isolation-boundary.md).

## Considered options

- Building a Provider prompt directly from transcript and retrieved items was
  rejected because it collapses eligibility, ranking, projection,
  authorization, and transport into one opaque action.
- Ranking all discovered candidates and lowering ineligible sources was
  rejected because similarity and score cannot safely represent applicable
  owning-domain Admission and Memory Suppression, permission, authority, or
  destination disclosure policy.
- Persisting a request or trace after outbound submission was rejected because
  a crash can then disclose project data without durable pre-submission
  evidence or a safe recovery boundary.
- Allowing a worker to perform external I/O before a durable dispatch claim was
  rejected because a crash could leave an actual disclosure with no Event. The
  claim persists the exact wire evidence and an OutcomeUnknown Disclosure Event
  before destination I/O may cross the External Processing Boundary; later
  confirmation settles the Destination Attempt without rewriting that Event.
- Treating one Provider request, transcript reconstruction, or prompt cache as
  the complete context receipt was rejected because logical context, wire
  delta, opaque continuity, and actual destination submission are different
  facts.
- Trusting Tool or MCP output as instructions because its implementation was
  authorized was rejected because execution trust does not establish content
  truth, Instruction Authority, or disclosure eligibility.
- Requiring author confirmation before every already-authorized model or
  embedding call was rejected because it would make the editor Agent unusable.
  An explicit bounded Project Destination Grant permits ordinary calls, while
  new destinations or expanded disclosure boundaries still require exact
  author authorization.

## Consequences

- PostgreSQL persistence must support an immutable manifest-before-egress
  boundary and exact User plus Project isolation from the first local validation
  deployment onward.
- Request builders, Model Provider Adapters, Tool gateways, background jobs, caches,
  and recovery workers cannot own independent prompt or disclosure shortcuts.
- Every physical resend, retry, fallback, and destination change creates new
  Destination Attempt and disclosure evidence.
- Exact non-secret provider-specific application payload material may be
  retained or referenced separately from provider-neutral logical context.
  Credential values, their digests, and credential-bearing transport envelopes
  remain ephemeral; provider-opaque state remains explicitly unknown.
- Historical context and disclosure evidence is append-only; later correction,
  applicable Memory Suppression, permission changes, and retention changes
  govern future use without rewriting the past.
- Authors receive a zero-configuration editor experience plus on-demand
  inspection and simple controls, rather than mandatory context-management
  ceremony.
- Physical schemas, API representations, redaction and retention, quantitative
  ranking and budget tuning, and verification mechanics remain with their
  downstream owner tickets.
