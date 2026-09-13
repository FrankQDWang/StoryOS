---
status: accepted
---

# Require Ordered Context Assembly Before Destination Disclosure

StoryOS requires every Host-controlled destination submission to advance through
seven ordered gates: Operation Requirement Determination, Candidate Discovery,
Source Eligibility Gate, Selection and Ranking, Bounded Projection, Context Assembly
Manifest Commit, then Destination-specific Disclosure and Attempt. The gates
cannot be skipped, merged, inverted, or reconstructed after the fact. An
invalid requirement or Blocked sufficiency decision terminates before gate
seven with no destination I/O; a Complete or explicitly Degraded assembly may
reach a Destination Attempt only after gate six commits. Host-internal assembly
is not represented as a fake Destination Attempt. Failure to commit the
immutable Context Assembly Manifest prevents destination I/O.

Eligibility precedes ranking; selection grants no truth, authority, evidence
status, or disclosure permission. Each submitted operation has a
minimum-necessary projection. A lossy projection is a new source-bearing item,
and each StoryOS-submitted generation request is a separate operation through
the same gates. Received Tool, MCP, and hosted results cross the boundary before
later StoryOS submission. ADR 0034 permits bounded Provider-hosted work after
admission of its complete intake, enabled Tool set, outward processing, and
effects. Invisible internal steps do not have invented Host gate records.

Context Assembly, destination, wire, disclosure, and Destination Attempt
records establish different facts. They preserve exact Project Scope and historical evidence but
never claim that a model internally attended to or used supplied content.
New reads check current source access and applicable Memory settings. Recorded
conversation items retain their own identities and copy/retention boundaries.
Ordinary author corrections append Messages for the Agent to interpret; source
edits and ordinary deletion do not rewrite history or automatically reset a
Provider chain. No Include/Pin/Exclude registry, semantic suppression classifier,
or derived-influence removal graph is introduced.

General compaction may prepare a later request within one active Run and turn.
It preserves known input and prior projection references, output, and loss or
unknown-state evidence, without mutating submitted Attempts or claiming precise
forgetting. ADR 0033 owns compatible delta/reference use, full-input submission,
current instructions and target, and recovery fences. ADR 0035 owns background
Memory. Real Project Isolation, access revocation, deletion/redaction of retained
copies, and destination authorization remain Host-enforced controls.

The full normative contract is [Context Assembly, Retrieval, and Outbound
Disclosure Semantics](../foundation/context-assembly-retrieval-and-outbound-disclosure-semantics.md).
This decision resolves [Wayfinder issue 54](https://github.com/FrankQDWang/StoryOS/issues/54)
and operates under the ownership and deployment boundary in [ADR
0004](0004-adopt-postgresql-service-and-project-isolation-boundary.md).

## Considered options

- Letting request construction bypass source access, authorization, or durable
  submission evidence was rejected. A general Agent loop still owns ordinary
  language interpretation and requests sources through bounded tools.
- Ranking all discovered candidates and lowering ineligible sources was
  rejected because similarity and score cannot safely represent applicable
  source access, current Memory settings, authority, or
  destination disclosure policy.
- Persisting a request or trace after outbound submission was rejected because
  a crash can then disclose project data without durable pre-submission
  evidence or a safe recovery boundary.
- Allowing a worker to perform external I/O before a durable dispatch claim was
  rejected because a crash could leave an actual disclosure with no Event. The
  claim persists the exact wire evidence and an OutcomeUnknown Disclosure Event
  before destination I/O may cross the StoryOS Controlled Processing
  Boundary; later
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
- A bounded hosted operation shares its owning physical Model/Destination
  Attempt; internal Provider events are reports or unknown facts, not duplicate
  local dispatches. A StoryOS-controlled processor's own nested external call
  still requires separate admission and dispatch evidence.
- Exact non-secret provider-specific application payload material may be
  retained or referenced separately from provider-neutral logical context.
  Credential values, their digests, and credential-bearing transport envelopes
  remain ephemeral; provider-opaque state remains explicitly unknown.
- Historical context and disclosure evidence is immutable under its retention
  contract. Current-source reads and access to retained copies are distinct;
  real permission or retention restrictions still block the uses they govern.
- PostgreSQL retains project content and recoverable context records under the
  existing storage contract. Markdown is a readable content format, not a
  second persistent file store. Active process memory and external Provider
  state cannot replace StoryOS durability.
- Authors receive a zero-configuration editor experience plus on-demand
  inspection and simple controls, rather than mandatory context-management
  ceremony.
- Physical schemas, API representations, redaction and retention, quantitative
  ranking and budget tuning, and verification mechanics remain with their
  downstream owner tickets.
- These owners must revise their existing contracts and stage specifications.
  This decision does not claim their completion or resume Stage 3 and later
  product implementation.
