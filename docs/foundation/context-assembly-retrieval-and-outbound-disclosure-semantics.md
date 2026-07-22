# Context Assembly, Retrieval, and Outbound Disclosure Semantics

- Status: accepted
- Wayfinder resolution: [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Parent domain model: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Memory and evidence boundary: [Fiction Memory and Research Provenance Semantics](fiction-memory-and-research-provenance-semantics.md)
- Run boundary: [Persistent Agent Run and Orchestration Semantics](https://github.com/FrankQDWang/StoryOS/issues/47)
- Tool and MCP boundary: [ToolSpec, Capability, Approval, and MCP Trust Semantics](https://github.com/FrankQDWang/StoryOS/issues/48)
- Model routing boundary: [ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50)
- Ownership and deployment decision: [ADR 0004](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md)
- Ordered assembly decision: [ADR 0005](../adr/0005-require-ordered-context-assembly-before-destination-disclosure.md)
- Research evidence: [Context Assembly, Retrieval, and Outbound Disclosure Source Audit](../research/context-assembly-retrieval-outbound-disclosure-source-audit.md)

## 1. Purpose and authority

This specification defines the logical contract by which StoryOS determines,
discovers, qualifies, selects, projects, records, and discloses context for one
operation. It applies before project-derived information enters a model, Tool,
MCP server, embedding service, telemetry sink, or any other processing
destination.

The contract exists to make a stable editor-integrated discovery-writing Agent
useful without turning retrieval, prompt construction, provider sessions, or
author-supplied documents into hidden authority. It is provider-neutral and
deployment-neutral. The initial StoryOS Server and PostgreSQL deployment may
run locally for one bootstrapped User, while model and embedding inference use
external APIs. The same semantics apply if the service later runs in the cloud
for many isolated Users.

This document is normative for domain behavior and downstream implementation
contracts. The cited research report is evidence, not an accepted decision.
Its options, implications, and unresolved questions have no authority except
where this specification or the canonical glossary explicitly resolves them.

This specification does not define database tables, Rust structs, API wire
formats, ranking formulas, source-class budget values, UI layout, retention
durations, or a specific Provider. Repository-wide hard model-context caps
remain normative below. This contract creates no Agent-authored outline,
Author Plan, or preplanned story structure. An author-owned outline is ordinary
Data-only Context unless some exact statement separately enters an established
authoritative or instructional domain path.

## 2. Mother contract

Every destination-bound context item may advance only through the following
seven semantic gates in order:

1. Operation Requirement Determination;
2. Candidate Discovery;
3. Source Eligibility Gate;
4. Selection and Ranking;
5. Bounded Projection;
6. Context Assembly Manifest Commit;
7. Destination-specific Disclosure and Attempt.

No implementation, Model Provider Adapter, Tool, cache, prior response, background
job, or recovery path may skip, merge, invert, or retroactively simulate these
gates. An invalid Operation Requirement stops before assembly begins. A later
Blocked decision advances only far enough to preserve the applicable refusal,
candidate, and manifest evidence and never enters gate seven. Internal
optimization may pipeline work only when durable evidence still proves the
same ordered decisions and no destination can receive content before gate six
commits. Any content that reaches a destination has completed all seven gates.

If generating a Projection requires a model, Tool, MCP server, embedding
service, or other processing destination, that generation is a separate
operation that recursively crosses all seven gates. A Tool or MCP result that
may enter later model context is also a new source at a new context boundary
and crosses the complete contract again.

### 2.1 Project ownership and isolation

Every project-bearing operation and record in this specification binds one
trusted Project Scope:

| Field | Meaning |
|---|---|
| owner_user_id | The durable User who acts as this Project's sole Project Author |
| project_id | The durable novel Project identity |

The pair is resolved from trusted StoryOS state. A client-supplied owner,
process-global current User, filesystem path, provider session, opaque object
ID, or ProjectId alone never establishes authority.

Context Candidate discovery, retrieval namespaces, source joins, cache identities,
manifests, Credential References and authorized project-use bindings,
destination grants, Destination Attempts, and disclosure evidence
must fail closed if either member is missing, ambiguous, or mismatched. No
content-derived digest, similarity score, shared Provider account, or global
cache may bridge two Project Scopes.

### 2.2 Facts that must remain distinct

StoryOS treats the following as separate facts:

| Fact | What it establishes |
|---|---|
| Stored | StoryOS retains a source or record |
| Discovered | Candidate Discovery located an exact source identity and Context Source Version |
| Eligible | Current qualification permits the Context Candidate to continue for this operation |
| Selected | Budget and ranking chose the eligible Context Candidate |
| Projected | A destination-oriented representation was created |
| Disclosable | Current policy and grants permit that exact Projection for one destination |
| Dispatched | One Destination Attempt crossed StoryOS's local outbound dispatch boundary |
| Confirmed submitted | Current evidence confirms submission beyond that boundary; OutcomeUnknown remains only potential-disclosure evidence |
| Used internally | The destination or model actually attended to or relied on the content |

No row implies a later row. StoryOS can establish the first eight only from its
own evidence. It must never claim the final fact from prompt construction,
request transmission, provider acknowledgement, or model output.

### 2.3 Durable-space ownership

This contract creates no fourth durable truth space. Operation Requirements,
Operation Input Snapshots, recorded Context Candidate and eligibility
decisions, selection and ranking results, Bounded Projections, compaction
Projections, Context Assembly and destination manifests, Outbound Disclosure
Manifests and Events, Project Destination Grants, author context-control
records, and Destination Attempts are Operational Records or immutable payloads
owned by those records.

They never become Artifacts or Authoritative State, never use Artifact
lifecycle as execution state, and never become eligible for Acceptance. If an
operation also produces author-facing reusable content, that content is created
as a separate typed Artifact with explicit provenance back to these Operational
Records. Retrieval indexes and Context Cache Entries remain disposable
projections rather than a fourth durable space.

Every generated Artifact Revision records the exact Context Assembly Manifest
through an available_as_context relation. The Manifest owns the complete
supplied Context Source Version closure. A direct item-level relation is added
only when that source is one of the provenance target kinds already admitted by
the Artifact domain model and the relation's own semantics are established;
the Host never widens the target union or invents a Revision merely because an
item was context. available_as_context proves availability only. derived_from,
supported_by, opposed_by, qualified_by, responds_to, and other closed
provenance relations are added separately only when their own semantics are
established; none is inferred from inclusion, disclosure, or model output. The
Artifact remains independently explainable without replaying the Run.

## 3. Gate one: Operation Requirement Determination

Context Assembly begins only after the Host creates one immutable Operation
Requirement. It binds at least:

- one operation identity and exact Project Scope;
- the trusted requester User and initiating author, Run, Service, or Job cause;
- one explicit Purpose;
- the applicable Workspace Context and Working Target, with a separate explicit
  not-applicable fact for each legitimately absent boundary, the exact typed
  initiating cause, and the immutable Operation Input Snapshot in every case;
- the intended result and completion criteria;
- one exact destination requirement, initially naming either a Processing
  Destination Identity or a closed allowed destination class;
- the immutable destination-resolution decision that selects one exact
  Processing Destination Identity before gate one completes;
- destination intake and disclosure policy boundaries;
- Mandatory Context obligations and their degradation policy;
- permitted dynamic source classes and retrieval modes;
- the authorization policy ceiling, applicable Project Destination and
  Capability Grants, whether a Destination Disclosure Approval is required,
  and any already-effective scoped authorization that does not depend on a
  future Destination Attempt;
- context, disclosure, and resource budgets;
- the Project Instruction Binding for an AgentRun, including an explicit
  no-instruction fact, or an explicit not-applicable fact for a non-AgentRun
  operation;
- policy, contract, and source-resolution versions required for reproducibility.

Missing Purpose, invalid Project Scope, unknown requester authority, or an
undefined destination-permission boundary prevents assembly from starting.
Changing Purpose, intended result, Project Scope, or destination boundary
creates a new Operation Requirement; it never mutates the original one.

An allowed destination class is a routing bound, not an executable
destination. For a model operation, the Host creates or binds the immutable
Model Route Request and creates the Model Route Decision from hard Purpose,
capability, context-bound,
disclosure, authorization, and budget requirements inside gate one. Other
destination kinds use their owning immutable Registration or routing decision.
The Decision selects a globally reusable, non-authorizing Model Registration
only together with one exact Project Scope-bound Project Model Use Binding that
pins project use authorization, Credential Reference binding when required,
external compatibility, destination, and hard bounds. A Registration or
project-free Provider observation cannot satisfy this binding.
No source content is disclosed during resolution. Gate one cannot complete,
and Candidate Discovery cannot begin, until one exact Processing Destination
Identity, endpoint or account boundary, and governing intake contract are
known.

If later selection or projection cannot fit that destination, StoryOS blocks or
creates a new Operation Requirement and destination-resolution decision. A
fallback or destination change performs a new Context Assembly; it does not
reuse eligibility or projection decisions made for the prior destination.

A Model Fallback remains inside the same Model Invocation and immutable Model
Route Request. Its new Operation Requirement may change only the exact
destination binding and evidence required to rerun all seven gates. Fresh
eligibility and Projection must reproduce the same semantic request digest and
Effective Model Context. If the new destination requires different selected
logical context, Purpose, authority, Tool exposure, or completion semantics,
the change is not Model Fallback and requires a new RunStep and Model
Invocation.

### 3.1 Mandatory Context has two planes

Mandatory Context means that an obligation must be considered. It does not mean
that the source bypasses qualification, receives unlimited budget, or may be
disclosed to every destination.

The Host Control Context plane contains identities, policies, grants,
Approvals, budgets, destination contracts, eligibility decisions, and other
control evidence. It governs assembly but is not automatically destination
visible.

The Mandatory Context Projection plane contains only the minimum content an
eligible destination needs to perform the current Purpose.

For a model step, the closed default obligations are:

| Obligation | Visibility and failure behavior |
|---|---|
| Purpose and exact initiating instruction or typed cause | Exact Required and non-degradable; include the current exact author instruction when one exists, otherwise preserve the authorized event, schedule, Service, or Job cause without inventing author speech |
| Project Scope, requester authority, safety, Capability, and destination policy | Host Control; only minimum operational constraints become model-visible |
| Exact Working Target and necessary local structure | Exact Required when needed to perform the requested edit or analysis |
| Bounded Run Continuity Context | Only the prior inputs, decisions, and settled results necessary to interpret this step |
| Project Instruction Binding | The exact bound Revision is mandatory for every model step when configured; absence is recorded explicitly |
| Applicable explicit Author Preferences | Included only under their exact Story and operational scope |
| Selected Skill Instruction Context and outcome contracts | Exact selected versions only; no unselected Skill body is injected |
| Actual Tool contracts exposed for this step | Exact ToolSpec and exposure projection needed for valid model requests |
| Author-required Include or Pin targets | Mandatory Context Candidates, still subject to all later gates |
| Manifest and sufficiency boundary | Host Control and non-degradable |

Whole transcripts, manuscripts, chapters, Agent Memory, Research collections,
character sheets, and author-owned outlines are dynamic by default. Their
authorship, proximity, retrieval rank, or repeated inclusion does not turn them
into mandatory instructions or Authoritative State.

For a non-model destination, Mandatory Context is restricted to fields declared
by its exact Destination Context Intake Contract. Project Instruction, Working
Target, Transcript, Agent Memory, and surrounding project content are not
ambient inputs.

### 3.2 Sufficiency and degradation

Before manifest commit, the Host records exactly one Context Sufficiency
Decision:

- Complete: every mandatory obligation is satisfied;
- Degraded: one predeclared degradation applies, with unmet needs, narrowed
  completion criteria, and prohibited claims or effects;
- Blocked: a non-degradable obligation is absent, ineligible, unverifiable, or
  cannot fit an eligible route.

A Declared Context Degradation must exist in the Operation Requirement before
Candidate Discovery and state why continuation remains valid. It cannot
silently preserve the original success criteria. An undeclared omission,
changed Purpose, or changed destination requires a new Operation Requirement.

Non-degradable context includes Purpose and exact initiating instruction or
typed cause, current author instruction when applicable, Project Scope and
isolation, the exact Working Target needed for the requested operation,
applicable authority and safety boundaries, effective Capability and
destination permission, and durable manifest availability.

## 4. Gate two: Candidate Discovery

Candidate Discovery enumerates every mandatory source and may locate optional
dynamic Context Candidates only from source classes allowed by the Operation
Requirement. Allowed source families include exact Authoritative State,
Artifacts, settled Operational Records, Agent Memory projections, Research
sources, bounded Working Context, selected SkillPackage Snapshots, and
immutable ToolSpec, schema, policy, Registration, Adapter, and capability
definitions.

Each Context Candidate identifies an exact domain object and Context Source
Version before current eligibility or selection. The owning domain may use an
Authoritative or Artifact Revision, Operational Record, typed terminal outcome,
Snapshot, ToolSpec, SkillPackage Snapshot, policy version, Registration
revision, or another immutable versioned contract. Context Assembly never
invents a fake Revision. An index hit, URI, provider cache key, transcript
position, or content digest is only a locator.

Working Context may become a Context Candidate only through an immutable item
version captured by the Operation Input Snapshot. For a RunStep, the Step
Snapshot fulfills this boundary and may capture the exact persisted author
input, Steering Input, selection snapshot, or bounded provisional result
evidence. A Service, Job, embedding, telemetry, or other non-Run operation uses
its own typed Operation Input Snapshot and never invents an AgentRun or Step
Snapshot. A live mutable editor buffer, in-flight model stream, Tool progress
stream, or process-memory object is never discovered directly. A later change
is a new captured version for a later Context Assembly and never changes the
current Context Candidate or manifest.

### 4.1 Retrieval modes

Dynamic Retrieval has three authorizable modes:

1. Deterministic Requirement Retrieval resolves exact or typed obligations
   already declared by the Operation Requirement, including the Working Target,
   applicable Author Preferences, and required Skill context roles.
2. Agent Retrieval Request is a typed request in one persisted Agent Decision,
   declaring Purpose, source classes, scope, and budget. Its results can enter
   only a later RunStep under a new Context Assembly.
3. Author-required Retrieval arises from an explicit author instruction and
   makes the target mandatory for discovery within that instruction's scope.

Author origin does not grant source authority, any owning-domain qualification
including Memory Admission, budget exemption, or Disclosure Eligibility.

Speculative Context Prefetch may warm a StoryOS-controlled Project Scope-bound
index or cache. It selects and discloses nothing. Prefetch that itself calls an
external or separately controlled processor is a full independent operation.

Similarity-based content may not be silently inserted into an in-flight Model
Attempt. Retrieval never rewrites the Step Snapshot whose Agent Decision asked
for it.

### 4.2 Retrieval indexes

Full-text, vector, graph, and other indexes are disposable projections. Their
internal IDs, scores, copied filters, and availability carry no durable
identity, truth, authority, Memory Admission or any other owning-domain
qualification, permission, or disclosure right.

Every lookup is constrained by exact Project Scope, and every returned hit is
resolved to the canonical source identity and Context Source Version before
gate three.
Index loss may reduce discovery availability but cannot erase or rewrite
canonical domain meaning.

Embedding generation uses an external API under the current product boundary.
It is therefore a separate Outbound Disclosure operation. Cross-project
batches, globally shared prose caches, or implicit background embedding are
forbidden.

### 4.3 Bounded growth

Context Assembly remains bounded when a Project, transcript, Artifact store,
Run history, or retrieval index grows without bound. Every Operation
Requirement sets finite hard budgets for discovery, Context Candidates per allowed
source class, selected items, Projections, and total destination context.

Discovery records its exact query or typed requirement, index or source-view
version, coverage boundary, stable continuation position when applicable, and
whether an optional search space was only partially enumerated. Every source
actually considered is still recorded in the manifest. A mandatory exact
source cannot be hidden by a discovery cap; inability to resolve it follows the
declared Degraded or Blocked behavior.

Every model-visible context fragment is structured, attributable, inspectable,
and hard-capped. No single injected item may exceed 10K tokens. Introducing a
new item kind that can exceed 1K tokens requires explicit design review. These
are repository invariants, not ranking-tuning defaults. Whole-project growth,
pagination, cache warming, and provider continuity cannot expand them.

## 5. Gate three: Source Eligibility Gate

Eligibility is fail-closed and completes before any relevance ranking. For
every Context Candidate the Host checks:

1. exact domain identity and Context Source Version resolution;
2. exact matching Project Scope for project-bearing content and trusted
   requester permission;
3. current Source Integrity and retained payload availability when owned by the
   source kind;
4. applicable owning-domain qualifications, including Memory Admission for a
   memory-derived or ordinary-recall Context Candidate, Lifecycle, Archive,
   Tombstone, and Retention State;
5. applicable Memory Suppression for a memory-derived or ordinary-recall path,
   plus Context Exclude controls;
6. exact Story Scope and Epistemic Scope when the source domain defines them;
7. source supersession, invalidation, correction, and current qualification;
8. source and destination data-category policy;
9. Context Trust Assessment;
10. current destination-specific Disclosure Eligibility.

Failure or inability to establish any required check excludes the Context
Candidate. The Context Candidate may not survive with a lower score, warning,
confidence penalty, or untrusted label. A mandatory Context Candidate that
fails eligibility produces the predeclared Degraded outcome or makes the
operation Blocked.

A qualification that the owning source domain does not define is recorded as
not applicable rather than fabricated. If the source domain does define it,
missing or unverifiable current evidence fails closed.

A globally or User-reusable ToolSpec, schema, policy, Adapter definition,
public capability description, or other definition may remain source-unscoped
only when it contains no project-derived data or project authority. Its
selection, exposure, and use are still recorded under the current Project
Scope. Any supposedly reusable definition containing project-derived data is
ineligible until represented through an exact Project Scope-bound source.

### 5.1 Orthogonal trust axes

Context Trust Assessment records independent facts:

| Axis | Question it answers |
|---|---|
| Source Integrity | Does this content match the claimed exact Context Source Version? |
| Instruction Authority | May this content direct Agent behavior? |
| Domain or evidentiary status | What truth or evidence meaning does its owning domain object have? |
| Execution Trust | May this exact registered implementation be invoked? |
| Disclosure Eligibility | May this source or Projection be disclosed for this Purpose to this destination? |

No axis implies another. Signed content may still be Data-only Context. A
trusted Tool may return false or malicious text. An authoritative fiction fact
may be ineligible for a destination. An author-created document may be
non-authoritative. Tool, MCP, model, Research, manuscript, imported, and outline
content is Data-only Context unless it separately enters a closed
Instruction-Authority path.

## 6. Gate four: Selection and Ranking

Only eligible Context Candidates participate. Budget is allocated in this order:

1. non-degradable Mandatory Context;
2. eligible degradable Mandatory Context under its declared policy;
3. eligible dynamic Context Candidates under the applicable Context Ranking Profile.

If non-degradable Exact Required content cannot fit a destination, the Host
chooses another eligible route with sufficient capacity, creates a newly
reframed Operation Requirement, or records Blocked. It never silently
truncates, summarizes, or drops that content.

### 6.1 Ranking profiles

Ranking is Purpose- and source-class-specific. A versioned Context Ranking
Profile may compare:

- semantic relevance to the explicit Purpose;
- exact Story and Epistemic Scope specificity;
- structural, causal, or narrative proximity;
- coverage of named entities, claims, unresolved references, or requirements;
- diversity and non-duplication;
- balanced supporting, opposing, and qualifying Research evidence;
- genuinely time-sensitive currency when the source class requires it;
- deterministic budget fit;
- a stable non-semantic final tie-break.

No profile may boost or penalize a Context Candidate based on a universal trust score,
source ownership, author identity, source authority as a relevance bonus,
prior access or retrieval frequency, popularity, repetition, model confidence,
or wall-clock decay of still-applicable fiction truth or historical evidence.

Selection, similarity, rank, and budget placement never grant truth, authority,
evidentiary status, binding force, or disclosure permission.

The exact formula, weights, thresholds, and empirical context limits remain
implementation and tuning decisions. They must be versioned inputs and cannot
alter the semantics above.

## 7. Gate five: Bounded Projection

Selected exact Context Source Versions are transformed into the minimum necessary
destination-oriented representation under one versioned Context Projection
Policy.

Each selected item uses exactly one mode:

| Projection mode | Contract |
|---|---|
| Exact Required | Preserve the complete eligible content and semantics; failure to fit blocks or reroutes |
| Deterministic Excerpt | Select complete domain units or locator-bound ranges and record every omitted boundary |
| Derived Summary | Create a new lossy, source-bearing Projection with explicit generator and loss evidence |
| Reference Only | Reveal bounded catalog information and a qualified locator without the payload |

Arbitrary token slicing, silent head-tail truncation, Provider-default
summarization, and replacement of source evidence are forbidden.

Every lossy Projection records:

- its own immutable identity;
- exact Project Scope;
- every exact Context Source Version and prior Projection;
- Purpose, exact Processing Destination Identity, and applicable destination
  intake and disclosure policy revisions;
- projection mode and transformation-policy version;
- deterministic or generated producer identity;
- input and output extents;
- omitted ranges, semantic classes, modalities, precision, and uncertainty;
- one structured Projection Loss Indicator;
- creation operation and manifest references.

A Projection never overwrites its source, changes its source's authority, or
masquerades as original evidence.

Outbound data categories follow exact provenance through every excerpt,
summary, generated query, digest, de-identification, and other transformation.
A Projection cannot silently shed a protected category or disclosure bound.
Any narrower derived classification requires an explicit versioned
transformation policy and attributable classification decision; uncertainty
retains the more restrictive source category.

### 7.1 Compaction

Context compaction is an immutable Derived Summary over one exact bounded input
range. It preserves a transitive Compaction Source Closure containing source
Context Source Versions, prior Projection lineages, compaction boundaries, and accumulated
loss evidence.

Compaction never rewrites Messages, Run Events, Tool results, Step Snapshots,
manifests, Proposals, or original context history. A summary of a summary may
be produced only when the complete source closure remains inspectable and the
new accumulated loss is recorded.

When a model, Tool, or external service generates compaction, that generation
is an independent seven-gate operation and an Outbound Disclosure where
applicable. An author may Exclude a compaction Projection or its protected
provenance closure from future unsubmitted Destination Attempts without changing history.

### 7.2 Cache and continuity

A Context Cache Entry is a disposable acceleration product keyed by exact
Project Scope, Context Source Versions, policy and transformation versions,
qualification state, destination identity, grant, and Adapter mapping.

Before reuse, a Context Cache Reuse Decision revalidates source identity and
version, every applicable owning-domain qualification such as Lifecycle,
Tombstone, and Retention plus Memory Admission and Memory Suppression for a
memory-derived or ordinary-recall dependency, permission, policy, destination,
grant, and Adapter dependencies. Changed or unverifiable dependencies make the
entry unusable immediately, even if physical invalidation is delayed.

Provider prompt caches, prior-response handles, encrypted compaction objects,
and session continuity are Opaque Provider Continuity. They may optimize a Wire
Payload Projection but cannot become StoryOS history, a Context Candidate, cached
permission, or the only evidence of Effective Destination Context. If StoryOS
cannot prove the complete local source closure remains eligible and logically
present, it retransmits the necessary context or blocks the dependent
Destination Attempt.

## 8. Gate six: Context Assembly Manifest Commit

Before any destination submission, StoryOS commits one immutable
ContextAssemblyManifest. Conceptually it records:

- manifest identity, schema version, exact Project Scope, requester, and cause;
- Operation Requirement, Purpose, intended result, destination requirements,
  and Operation Input Snapshot;
- Step Snapshot and Project Instruction Binding when the operation belongs to
  an AgentRun, otherwise explicit not-applicable facts;
- every mandatory obligation and allowed dynamic source class;
- every considered Context Candidate, exact Context Source Version, discovery reason, and
  eligibility result;
- every exclusion with typed reason and governing policy or author control;
- Context Trust Assessments and destination Disclosure Eligibility;
- Ranking Profile, ranking inputs, stable outcome, and selection reasons;
- exact selected Context Source Versions and Projections;
- projection modes, loss indicators, and source closures;
- context budgets, allocation, omissions, and unmet needs;
- Complete, Degraded, or Blocked Context Sufficiency Decision;
- authorization policy, grant, approval-requirement, cache-reuse, and
  transformation versions, plus any Approval already effective when the
  manifest was committed;
- links to any predecessor assembly cancelled and rebuilt after a prospective
  control change.

Failure to durably commit the manifest prevents destination I/O. A debug log,
trace span, reconstructed transcript, provider request object, or cache entry
is not a substitute.

The manifest proves what StoryOS considered and logically prepared. It does not
prove destination submission, exact wire bytes, Provider retention, model
attention, internal model use, evidentiary reliance, or output correctness.

Manifests are immutable historical evidence. Later correction, Memory Suppression,
permission revocation, retention expiry, Project Instruction update, or source
edit changes future eligibility and may be displayed alongside history as a
current-invalidity annotation. It never rewrites what the earlier operation
considered or prepared.

## 9. Gate seven: Destination-specific Disclosure and Attempt

Every destination receives its own minimum-necessary projection after gate six.
Sharing a Provider, connection, SDK, Run, or cache never combines destination
authority.

### 9.1 Processing boundary classifications

| Class | Meaning | Evidence contract by execution stage |
|---|---|---|
| StoryOS Host Internal Processing | Core-owned resolution, eligibility, selection, and deterministic projection with no separate processor; it is not a gate-seven destination | Applicable Host records and ContextAssemblyManifest when assembly reaches gate six |
| StoryOS Controlled Processing Destination | Separately registered processor inside the enforced StoryOS Controlled Processing Boundary | DestinationContextManifest and Destination Attempt |
| External Processing Destination | Model Provider, embedding API, external MCP server, hosted Tool, telemetry or support system, or any independently controlled processor | DestinationContextManifest, OutboundDisclosureManifest, Destination Attempt, and persisted Wire Payload Projection before dispatch; Disclosure Event at the durable local dispatch claim before external I/O |

Classification depends on exact Registration, operational control, enforced
Project Isolation, implementation, and data path. It does not depend on
localhost, hostname, first-party branding, infrastructure ownership, network
route, or whether StoryOS itself runs locally or in the cloud.

Current model and embedding APIs are External Processing Destinations.

### 9.2 Destination and disclosure manifests

One immutable DestinationContextManifest binds:

- its ContextAssemblyManifest;
- exact requester User and Project Scope;
- one exact Processing Destination Identity and its owning Registration Revision;
- the exact Project Scope-bound external-use binding, compatibility decision,
  and Credential binding generation when the destination requires one;
- one Purpose and the applicable owning intake contract: Model Capability
  Profile plus Model Attempt Request for a model, or Destination Context Intake
  Contract for a non-model destination;
- processing-boundary class;
- minimum-necessary selected Projections;
- applicable policy, Project Destination Grant, Capability Grant,
  approval requirement, and any authorization already effective before the
  manifest was committed;
- logical Effective Destination Context;
- allowed wire projection and hard bounds.

For an External Processing Destination, an OutboundDisclosureManifest
specializes that record with exact outbound data categories and disclosure
policy. Identical destination, logical payload, policy, and currently eligible
grant may reference one immutable manifest after fresh revalidation. The
manifest is not a Destination Attempt and is never proof of a later submission.

### 9.3 Destination Attempt and wire evidence

Every concrete planned destination execution, including an initial submission,
physical resend, retry, repair, fallback, or destination change, owns a distinct
Destination Attempt even when it settles before dispatch. It is durably
established before outbound I/O and binds its exact Processing Destination
Identity, Project Scope-bound external-use binding, manifests, and semantic
request. Model Attempt and the owning
destination-specific Tool or service attempt refine this common boundary.

When exact destination disclosure approval is required, the Host first creates
the unsubmitted Destination Attempt in an Awaiting Approval state and prepares
its exact Wire Payload Projection without performing I/O. The author Decision
then creates a Destination Disclosure Approval bound to that Attempt, its
immutable destination and disclosure manifests, and its logical and wire
payload closure. The later Destination Attempt Admission Decision binds and
revalidates that Approval. This order prevents either an immutable manifest or
Approval from referring to an Attempt that did not yet exist.

Immediately before destination I/O, the Host records one fail-closed
Destination Attempt Admission Decision over current Project Scope, every source
and Projection dependency, Lifecycle, applicable Memory Suppression for a
memory-derived or ordinary-recall dependency, Context Exclude, requester
permission, grants and exact Tool or Destination Disclosure Approval when
required, destination identity and Registration status, governing intake
contract, policy, and budget. Only an admitted Decision may submit.

Any changed, revoked, expired, mismatched, or unverifiable dependency preserves
the committed manifests, settles or cancels the unsubmitted Destination
Attempt, and requires new Context Assembly. No Outbound Disclosure Event is
created when failure is confirmed before the durable local dispatch claim. The
PostgreSQL, protocol, and verification
tickets own the physical fencing or atomic admission mechanism that prevents a
state change from being silently missed between this decision and I/O.

The Wire Payload Projection durably records the exact non-secret
provider-specific application payload bytes, frames, fields, or
access-controlled payload references prepared for one Destination Attempt,
plus opaque Credential References or secret-injection slots, the mapping
version, and a digest over non-secret material only. Credential values,
credential-value digests, and credential-bearing transport-envelope bytes are
ephemeral and never enter the Projection. It must commit before the egress
worker can claim dispatch. The Projection alone is wire-form evidence, not
proof of destination receipt or the complete logical Effective Destination
Context when cache or prior-response references are used.

StoryOS defines its local outbound dispatch boundary as the durable claim by an
egress worker, not as a later provider acknowledgement or an unfenced socket
write. Claiming dispatch transactionally creates one immutable Outbound
Disclosure Event binding the Destination Attempt, OutboundDisclosureManifest,
and persisted Wire Payload Projection. Only after that transaction commits may
the worker perform external I/O. The Event begins as OutcomeUnknown
conservative potential-disclosure evidence; later immutable provider or
transport confirmation evidence may settle the Destination Attempt as
ConfirmedSubmitted without rewriting the Event. A failure proven before the
claim creates no Event. A crash after claim remains OutcomeUnknown even if no
bytes ultimately left, preferring conservative extra evidence over an actual
disclosure with no durable record. A later Destination Attempt may reuse an
immutable manifest only after current revalidation; it never reuses the prior
Destination Attempt or Disclosure Event as current execution evidence.

A fallback to another Model Registration, endpoint, or account boundary always
requires a new Operation Requirement, Context Assembly, route decision,
Project Model Use Binding, destination and disclosure manifests, and
Destination Attempt. It cannot carry the prior route's Credential Reference
or compatibility binding forward. If the actual
processor, endpoint/account, control classification, or intake/disclosure
boundary changes, it also resolves a new Processing Destination Identity and
requires authority for that identity. A Registration, Adapter, serialization,
or model revision change that preserves those boundaries retains the same
Processing Destination Identity and may use its still-effective Project
Destination Grant, but it still requires fresh Registration, intake, wire,
cache, and admission evidence. Either form creates a new disclosure occurrence
and Event only if its Destination Attempt reaches the durable dispatch claim.
It may proceed only inside current destination and budget authority. An
uncertain prior Destination Attempt remains OutcomeUnknown and retains
conservative usage and disclosure evidence; a successor cannot convert it to
not-submitted.

Within one Model Invocation, fallback preserves the immutable Model Route
Request, semantic request digest, and Effective Model Context. The new assembly
revalidates those same semantics for the exact resolved Processing Destination
Identity and new route and Registration evidence. If exact semantic
equivalence cannot be preserved, StoryOS creates no fallback Attempt; changed
logical context belongs to a new RunStep and Model Invocation.

StoryOS distinguishes:

- exactly reconstructable logical and wire content;
- content known through an exact StoryOS-held reference;
- provider-opaque continuity or internal state.

Inspection may report all three but never present the provider-opaque part as
exact Effective Destination Context. Exact wire inspection is limited to the
persisted non-secret application payload and opaque secret-injection
placeholders; it never reconstructs credential-bearing transport envelopes.

### 9.4 Project Destination Grant and Approval

An author creates a Project Destination Grant through explicit project
settings. It enables one exact Processing Destination Identity for named
ordinary Purposes, outbound data categories, and hard bounds under one Project
Scope.

Destination Attempts for ordinary model and embedding operations inside the
effective Grant proceed without individual confirmation. They still require
all seven gates, current
eligibility, minimum-necessary disclosure, budgets, and complete evidence.

The following grant nothing by themselves:

- storing a credential;
- discovering or registering a Provider;
- sharing a Provider account with another Project;
- a prior disclosure;
- a cache hit or prior-response handle;
- model or Tool preference;
- successful execution history.

For a non-Tool operation, a new or changed destination, endpoint/account
boundary, Purpose, data category, or wider bound requires an explicit Project
Destination Grant setting change or an exact Destination Disclosure Approval
Wait before submission. A Tool Effect Request instead uses its owning
Capability and Tool Approval contract intersected with the governing
destination-policy ceiling. High-risk Tool disclosure, an external write, or
an irreversible Tool effect retains one-shot Tool Approval. High-risk
disclosure to a non-Tool destination requires a one-shot Destination Disclosure
Approval for one exact unsubmitted Destination Attempt.

This default preserves a stable conversational and editor Agent. It avoids
per-call consent fatigue without turning one configured Provider into blanket
permission for every service or every project payload.

### 9.5 External Provider accountability

StoryOS records what it selected and projected, the exact prepared non-secret
payload, the durable dispatch claim, its best-known submission certainty, the
named destination, Purpose, governing grant or authorization, and owning
Destination Attempt. It claims that content was sent only when immutable
confirmation evidence establishes ConfirmedSubmitted; OutcomeUnknown remains
conservative potential disclosure.
It does not claim control over or exact knowledge of a Provider's internal
retention, training, logging, subprocessors, hidden caches, or later handling.

Zero-data-retention labels, omitted telemetry fields, contracts, and Provider
promises may be external evidence for a separate policy purpose. They do not
undo a disclosure or become execution guarantees inside StoryOS durable truth.

### 9.6 Telemetry and non-model destinations

Telemetry, debug, crash-reporting, and support systems are separate
destinations. Their diagnostic Purpose does not grant access to prose, prompts,
Research, Tool results, Project Instructions, credentials, or complete Run
payloads. Default telemetry disclosure is limited to sanitized operational
categories, identifiers, timings, and digests.

Every non-model Tool, MCP server, embedding service, hosted Tool, and other
destination has an exact Destination Context Intake Contract. Only declared
fields, source classes, Purposes, and bounded data categories may be supplied.
Ambient Context is forbidden.

Provider-hosted Tools are separate External Processing Destinations. They do
not inherit the Model Attempt's context, Capability, or disclosure grant.

A StoryOS Controlled Processing Destination cannot become an unrecorded egress
proxy. If a controlled Tool, MCP adapter, or service needs to call a downstream
external endpoint, the Host treats that endpoint as its own exact External
Processing Destination and performs a separate seven-gate operation for the
minimum outbound payload. That nested call requires its own destination and
disclosure manifests, admitted Destination Attempt, Wire Payload Projection,
and Disclosure Event on dispatch. Its final admission intersects the enclosing
processor's exact owning effect and authorization contract with the governing
destination-policy and grant boundary. For a ToolCall, that owning contract is
the exact Tool Effect Request plus the applicable Capability Grant or Tool
Approval. The enclosing controlled-destination manifest and Destination
Attempt cannot replace the nested evidence, and no effect or destination
authorization extends beyond its exact bounds.

## 10. Tool and MCP result boundary

Execution Trust for a Tool, MCP server, Provider, or Adapter only permits
invocation through its governed boundary. It grants no truth, evidence status,
Instruction Authority, Authoritative State, or disclosure permission to its
output.

Tool and MCP outputs are Data-only Context. Imperative text inside a result is
quoted data and cannot direct Agent behavior. A Skill may provide Instruction
Authority only when its exact SkillPackage Snapshot is separately selected and
loaded through the Skill contract; a Tool result cannot self-promote into Skill
instructions.

When a Tool or MCP result may enter a subsequent model step:

1. its exact result or Artifact Revision becomes a new Context Candidate source;
2. a new Operation Requirement states why it is needed;
3. the result crosses identity, Project Scope, lifecycle, trust, permission, and
   disclosure eligibility again;
4. selection and Bounded Projection operate on the qualified result;
5. new manifests and Destination Attempt evidence are committed.

An MCP App remains a sandboxed view or controller over StoryOS-owned typed
records. Its Host Context, update-model-context contribution, Tool result, and
App Action never become ambient model context or an authoritative write path.

## 11. Author inspection and controls

### 11.1 Inspect

Context Inspect is read-only and available on demand. It may show current or
historical:

- Operation Requirements;
- Context Candidates and discovery reasons;
- eligibility decisions and current-invalidity annotations;
- ranking and budget choices;
- Projections, omissions, and loss;
- ContextAssemblyManifests and DestinationContextManifests;
- OutboundDisclosureManifests;
- exact non-secret wire deltas plus opaque secret-injection placeholders;
- Disclosure Events and Destination Attempts;
- exactly reconstructable, reference-known, and provider-opaque context.

Historical inspection preserves facts at operation time. Current invalidity is
shown separately and never rewrites history. Inspection obeys current
permissions, Project Isolation, redaction, and sensitive-content access rules.
Credential values are represented only by opaque Credential References plus
non-secret availability or status evidence; StoryOS never stores or displays a
credential-value digest. Other access-controlled non-secret payload evidence
may use controlled references or versioned digests only when its owning policy
permits that representation.

Ordinary already-authorized Context Assembly does not pause merely because it
is inspectable. Only an exact Capability, Approval, or destination-policy
requirement creates an Approval Wait.

### 11.2 Include

Include binds one exact Operation Requirement and makes a named source,
exact Context Source Version, fragment, or domain object a Mandatory Context
Candidate for that operation. It does not follow updates, persist into later
Runs, or grant authority, any owning-domain qualification including Memory
Admission, budget exemption, or disclosure permission.

### 11.3 Pin

Pin is a prospective Author Context Requirement scoped to Next Operation,
Current AgentRun, or Project. Its source-version strategy is Exact Context
Source Version or Follow Source Identity.

An Exact Context Source Version remains fixed but is rechecked for current
lifecycle, permission, destination eligibility, and applicable Memory
Suppression when it resolves through a memory-derived or ordinary-recall path.
An Artifact or Authoritative State source uses its exact Revision; another
source family uses its owning immutable version boundary rather than inventing
a Revision. Follow Source Identity resolves the current exact Context Source
Version through the owning domain and repeats the full eligibility process.
Ambiguous split, merge, or deletion makes the requirement unmet; the Host never
guesses a replacement. Suppression does not block direct governed use of the
unchanged raw source.

Pin requires logical consideration, not disclosure to every destination. It is
an underlying domain control, not a context-management form the author must
learn or routinely operate.

### 11.4 Exclude

Exclude applies at Operation, AgentRun, or Destination scope to a source,
Context Source Version, fragment, data category, and protected
derived-provenance closure. It affects only future unsubmitted Destination
Attempts.

If a ContextAssemblyManifest already committed but disclosure has not occurred,
StoryOS preserves it, cancels the pending Destination Attempt, and performs a
new assembly. It never edits the old manifest. A resulting mandatory gap becomes Degraded
only under a declared fallback; otherwise it is Blocked. Past disclosures
cannot be withdrawn.

### 11.5 Suppress

Suppress retains the accepted Memory Suppression semantics: it is a persistent,
auditable ban on future memory extraction, Memory Admission, and ordinary recall that
survives replay, Worker recovery, and index rebuild. It is not a one-operation
Exclude, does not alter the source, and never rewrites prior Runs. Lifting
Suppression permits fresh eligibility evaluation and does not restore old
Memory Admission.

Suppression applies when content is memory-derived or enters through ordinary
memory recall. It does not ban direct governed use of the unchanged raw source.
An author who wants to bar a source, fragment, provenance closure, data
category, or its use at a destination scope uses Context Exclude. Disabling a
Processing Destination Identity itself changes or revokes its Project
Destination Grant or governing policy. A hard policy or Tombstone continues
through its owning domain command.

### 11.6 Control precedence

Positive author controls never override hard negative boundaries:

1. Tombstone, current permission, Capability, and destination policy;
2. Memory Suppression for a memory-derived or ordinary-recall Context
   Candidate;
3. applicable Exclude;
4. Include and Pin;
5. ordinary dynamic Context Candidate ranking.

Archive, Tombstone, source editing, ownership, and other domain changes use
their own commands rather than being hidden inside context controls.

### 11.7 Default context experience

StoryOS must first be a stable conversational Agent that can operate on the
editor. It automatically receives the eligible Working Target and necessary
bounded continuity without requiring a character sheet, manual source-version
strategy, Pin configuration, Context Manifest review, or Project Instruction.

Author controls should appear as plain-language or simple direct actions such
as use this, do not use this, or show what was sent. The domain distinctions
remain exact underneath the UI, but their complexity is not pushed onto the
author.

## 12. Project Instruction

Project Instruction is optional. At creation of each top-level AgentRun,
StoryOS binds either the exact effective Project Instruction Revision or the
explicit fact that none was configured. The binding never changes in place.

The bound Revision is Mandatory Context for every model step in the AgentRun
and every descendant Subrun, which must independently reference the same
binding. Compaction, window trimming, cache, and prior-response continuity
cannot make it logically disappear. If StoryOS cannot establish continued
presence, it retransmits it or blocks the dependent step.

Updating project settings creates a new Revision for new top-level AgentRuns.
An active AgentRun continues with its bound Revision. Immediate changes use
scoped Steering Input from the next safe RunStep and do not rewrite the Project
Instruction or completed history.

Later ineligibility under current permission, safety, project-setting
revocation or content removal, or the owning lifecycle may make a bound
Revision Degraded or Blocked; frozen binding never overrides current hard
policy.

Instruction precedence is:

1. StoryOS product, domain, safety, permission, Capability, ToolSpec, Proposal,
   and Acceptance boundaries;
2. current exact author instruction, Steering, Approval, and Wait response;
3. bound Project Instruction Revision;
4. selected Skill Instruction Context;
5. Tool and MCP results, Research, prose, Artifacts, Agent Memory, outlines, and
   all other Data-only Context.

Project Instruction is mandatory for model context when configured. It is not
automatically disclosed to Tools, MCP servers, embedding services, telemetry,
or other destinations.

## 13. Failure and recovery semantics

If the ContextAssemblyManifest cannot be committed, no Destination Attempt may
start. Once it commits, any failure before submission preserves the manifest
and records or cancels the pending Destination Attempt according to its owning
execution contract.

Recovery never infers non-submission from a missing response, closed connection,
expired lease, process crash, or absent destination record. It always
reconciles the durable Destination Attempt plus wire and local dispatch
evidence; an External Processing Destination additionally uses its Outbound
Disclosure Event, and a Provider-backed destination uses available Provider
evidence. Missing type-inapplicable evidence is not fabricated.

A safe retry is a new Destination Attempt. It revalidates all current source,
policy, applicable Memory Suppression for a memory-derived or ordinary-recall
dependency, retention, grant, destination, and budget dependencies. If the
prior Destination Attempt may have submitted, the successor requires authority
and budget for an additional disclosure and cannot erase the predecessor's
uncertainty.

Source correction, permission revocation, and retention expiry invalidate the
future retrieval, Projection, and cache dependencies they govern as soon as the
canonical change is effective. Memory Suppression invalidates memory-derived
and ordinary-recall dependencies, not direct governed use of the unchanged raw
source. A Project Destination Grant change invalidates assemblies,
Projections, Destination Attempts, and caches bound to that exact destination
grant, not unrelated internal retrieval or another destination. Physical cache
cleanup may occur later, but every affected stale dependency is immediately
ineligible.

No recovery, compaction, or cache process rewrites historical Context Candidates,
manifests, Projections, Disclosure Events, Destination Attempts, or the exact context
evidence held by a prior Run.

### 13.1 Contract-change and migration impact

This is a breaking semantic contract for future persisted context, disclosure,
authorization, event, and recovery representations. At acceptance time StoryOS
has no production Rust workspace, PostgreSQL schema, or persisted production
runtime data; the existing prototypes are not production contract stores.
Therefore no live-data migration or compatibility reader is required for this
decision. The PostgreSQL and versioned-protocol tickets must implement only the
accepted forms, exact identities, sequencing, and certainty distinctions in
this specification. Any later change after those forms are persisted must
provide an explicit schema, event, replay, cache, and recovery migration path.

## 14. Downstream ownership

This specification fixes semantics and hands off physical or wire realization:

| Ticket | Owns next | Must preserve |
|---|---|---|
| [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md) | tables, constraints, transactions, indexes, payload storage, migrations, backup, restore, export, and Credential Reference integration | manifest-before-egress durability, immutable history, exact Project Scope, cache invalidation, rebuildable indexes |
| [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md), resolving [Threat-Model the StoryOS Service, Client, and External Trust Boundaries](https://github.com/FrankQDWang/StoryOS/issues/57) | credible attack paths, structural mitigations, residual risks, verification evidence, and downstream security ownership | orthogonal trust axes, fail-closed eligibility, destination-specific grants, no ambient context |
| [Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58) | DTOs, schemas, commands, event envelopes, compatibility, and errors | all exact identities, Revisions, manifests, controls, attempts, and Project Scope |
| [Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) | fake destinations, property tests, crash/retry/replay and isolation tests | seven ordered gates, no disclosure before commit, recursive boundaries, uncertainty preservation |
| [Foundation Evidence for the Standalone Eval Surface](https://github.com/FrankQDWang/StoryOS/issues/61) | Eval-facing run, context, source, Artifact, usage, and decision evidence | manifests, sufficiency and ranking reasons, Projections and loss, destination, disclosure, and Destination Attempt evidence remain derived views and never redefine truth, authority, or model use |
| [First Production Vertical Slice and Handoff](https://github.com/FrankQDWang/StoryOS/issues/62) | implementation slice and acceptance gate | zero-configuration editor Agent, external model and embedding APIs, Provider neutrality |
| [Run Event, Mailbox, Snapshot, Retention, and Archival](https://github.com/FrankQDWang/StoryOS/issues/64) | long-term retention, payload redaction, archival, export, and deletion | historical context truth, compaction lineage, manifest and disclosure evidence |

No new follow-up ticket is required. These existing owners cover every
remaining physical, security, protocol, verification, slice, and retention
decision exposed by this contract.

## 15. Required verification scenarios

Downstream implementation and verification must cover at least:

1. An ordinary editor model step automatically includes the exact Working
   Target, bounded continuity, and bound Project Instruction when configured;
   otherwise the AgentRun records its explicit absence, without requiring a
   character sheet or per-call confirmation.
2. A configured Model Attempt, refining the Destination Attempt boundary and
   operating inside its exact Project Model Use Binding and Project Destination
   Grant, passes all gates and records separate scope/use, compatibility,
   credential-binding, assembly, destination, wire, disclosure, and execution
   evidence.
3. A new Provider, endpoint, Purpose, or wider data category blocks on exact
   authorization before disclosure.
4. An ineligible mandatory source produces the declared Degraded outcome or
   Blocked and never survives as a lower-ranked warning.
5. A suppressed high-similarity memory hit is excluded before ranking.
6. Ranking changes selection but cannot change source authority, evidence
   status, or disclosure permission.
7. An Exact Required item that cannot fit blocks or reroutes and is never
   silently summarized.
8. A model-generated compaction summary runs as a separate seven-gate operation
   and retains complete source closure and loss evidence.
9. Current applicable Memory Suppression for a memory-derived or
   ordinary-recall dependency, or permission revocation, prevents reuse of an
   otherwise valid Provider cache entry.
10. A Tool or MCP result becomes Data-only Context Candidate content for a later step
    and crosses all seven gates again.
11. Exclude after assembly but before submission preserves the old manifest,
    cancels pending work, and creates new assembly evidence.
12. Updating Project Instruction does not change an active AgentRun binding;
    a new top-level AgentRun receives the new Revision.
13. A Subrun independently proves the same Project Instruction Binding and
    exact Project Scope as its root.
14. Model Fallback reruns all seven gates for the newly resolved route while
    preserving the same Model Invocation, Route Request, semantic request
    digest, and Effective Model Context; it uses the new route's exact Project
    Model Use Binding rather than inheriting the prior credential/compatibility
    binding. A dispatched fallback creates a new Destination Attempt, wire
    projection, and Disclosure Event, while semantic drift refuses fallback.
15. An OutcomeUnknown Destination Attempt remains visible and budgeted when a
    successor is admitted.
16. Inspection distinguishes the exact non-secret wire delta and opaque
    secret-injection placeholders, reference-known effective context, and
    provider-opaque state without claiming model use or exposing credential
    material.
17. Cross-User and cross-Project Context Candidate discovery, indexes, embeddings,
    caches, manifests, and Destination Attempt reuse fail closed.
18. Telemetry receives sanitized operational fields only and cannot inherit
    model, Tool, or Run context.
19. Failure to commit the ContextAssemblyManifest causes zero destination I/O.
20. Historical inspection preserves what an earlier operation considered,
    selected, projected, prepared, and durably claimed for dispatch, together
    with its best-known submission certainty, after later source correction or
    Memory Suppression. It describes content as sent only when confirmation
    evidence establishes ConfirmedSubmitted.
21. A policy, permission, grant, applicable Memory Suppression for a
    memory-derived or ordinary-recall dependency, Exclude, Lifecycle, or
    destination change after manifest commit but before I/O makes final
    Destination Attempt admission fail, causes no dispatch, and requires new
    Context Assembly.
22. A StoryOS-controlled Tool adapter that calls an external search API records
    the adapter execution and the search endpoint as separate destination
    operations; the minimum query disclosure has its own External Processing
    Destination evidence.
23. A noninteractive scheduled embedding or sanitized telemetry operation
    records explicit not-applicable Workspace Context and Working Target facts,
    its exact typed cause, and an Operation Input Snapshot rather than inventing
    an AgentRun, RunStep, editor selection, or author instruction.

## 16. Normative invariants

1. Every project-bearing fact binds one exact Project Scope.
2. No Context Assembly gate is skipped, merged, inverted, or retroactively
   simulated.
3. Purpose and destination permission exist before Candidate Discovery.
4. Mandatory means considered, not automatically eligible or disclosed.
5. Non-degradable context never disappears through ranking, budgeting,
   truncation, compaction, cache, or fallback.
6. Retrieval discovery and index hits grant no eligibility.
7. Source qualification completes before relevance ranking.
8. An unverifiable Context Candidate is excluded rather than down-ranked.
9. Similarity, rank, access frequency, repetition, and cache stability never
   create truth, authority, evidence, permission, or Instruction Authority.
10. Author ownership of a document does not make it Authoritative State or an
    Author Plan.
11. Every lossy Projection is a new immutable, source-bearing item with explicit
    loss evidence.
12. Excerpts preserve complete domain units and exact omission boundaries.
13. External generation of a Projection recursively crosses all seven gates.
14. Compaction never rewrites source or historical context evidence.
15. Cache reuse revalidates all current eligibility and destination
    dependencies.
16. A controlled processor's downstream external call is a separate External
    Processing Destination. Its final admission remains contained by the
    enclosing processor's exact owning effect and authorization contract; a
    ToolCall specializes that contract as its Tool Effect Request and
    applicable Capability Grant or Tool Approval. No authority extends beyond
    those bounds, and the enclosing manifest or Destination Attempt never
    substitutes for the nested execution and disclosure evidence.
17. Opaque Provider Continuity is never StoryOS history or cached permission.
18. A ContextAssemblyManifest commits before any destination submission.
19. A manifest proves StoryOS preparation, not submission or destination use.
20. Every destination receives a separately minimized and authorized context.
21. Every local outbound dispatch or redispatch has its own Disclosure Event
    and Destination Attempt evidence with explicit submission certainty;
    OutcomeUnknown is treated conservatively without being called confirmed
    submission.
22. A prior Destination Attempt never proves a later execution.
23. Provider promises and retention labels never undo Outbound Disclosure.
24. Tool and MCP execution trust never grants truth or Instruction Authority to
    returned content.
25. Tool and MCP results cross the entire contract again before later context
    use.
26. Positive author controls never override Tombstone, permission, Capability,
    destination policy, applicable Memory Suppression for a memory-derived or
    ordinary-recall Context Candidate, or applicable Exclude.
27. Inspect never rewrites history or presents provider-opaque inference as
    exact fact.
28. Project Instruction is optional for the author but mandatory model context
    within an AgentRun when bound.
29. Project Instruction is not ambient context for non-model destinations.
30. Ordinary use inside an effective Project Destination Grant does not require
    confirmation for each Destination Attempt.
31. A new destination or expanded Purpose, data category, or bound requires
    exact author authorization before submission.
32. Provider choice, including Bailian, is configuration rather than a kernel
    dependency.
33. Current model and embedding inference always use external APIs.
34. Historical context and disclosure evidence is never rewritten by later
    source, policy, permission, Memory Suppression, retention, or cache changes.
35. StoryOS never claims that a model internally used content merely because
    StoryOS selected, sent, or referenced it.
36. Projection, summarization, query generation, hashing, and de-identification
    never silently discard protected outbound data categories; any narrower
    classification is explicit, versioned, attributable, and fail-closed under
    uncertainty.
37. Every Destination Attempt passes a live fail-closed Admission Decision
    immediately before I/O; changed dependencies never ride on a stale
    manifest.
38. A noninteractive operation with no legitimate Workspace Context or Working
    Target records explicit not-applicable facts plus its typed cause and
    Operation Input Snapshot; it never fabricates UI or Run state.
