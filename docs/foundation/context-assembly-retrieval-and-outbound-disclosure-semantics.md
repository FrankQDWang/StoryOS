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
- Accepted inputs: [Model continuation](../adr/0033-use-volcengine-responses-for-the-first-real-model-path.md), [hosted operations](../adr/0034-bound-provider-hosted-tool-operations.md), and [Project Memory](../adr/0035-use-background-generated-project-memory.md)
- Research evidence: [Context Assembly, Retrieval, and Outbound Disclosure Source Audit](../research/context-assembly-retrieval-outbound-disclosure-source-audit.md)

## 1. Purpose and authority

This specification defines the logical contract by which StoryOS determines,
discovers, qualifies, selects, projects, records, and discloses context for one
operation. It governs each StoryOS-controlled submission to a model, Tool,
MCP server, embedding service, telemetry sink, or other processing destination.
ADR 0033 owns model continuation, ADR 0034 owns bounded Provider-hosted work,
and ADR 0035 owns background Project Memory.

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

Every StoryOS-controlled destination submission advances through the following
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
commits. These are Host decisions about the submitted operation, not a claim
that every Provider-internal step has a StoryOS gate record.

If generating a Projection requires a model, Tool, MCP server, embedding
service, or other processing destination, that generation is a separate
operation that crosses all seven gates before StoryOS submits it. A received
Tool, MCP, or hosted result crosses the contract before StoryOS supplies or
references it in a later model request. A separately authorized Provider-hosted
Operation may consume internal results before StoryOS receives them. Its
complete admitted intake, Tool set, outward processing, and effect bounds use
ADR 0034; do not invent a Host-controlled dispatch for each invisible step.

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
Manifests and Events, Project Destination Grants,
and Destination Attempts are Operational Records or immutable payloads
owned by those records.

They never become Artifacts or Authoritative State, never use Artifact
lifecycle as execution state, and never become eligible for Acceptance. If an
operation also produces author-facing reusable content, that content is created
as a separate typed Artifact with explicit provenance back to these Operational
Records. Retrieval indexes and Context Cache Entries remain disposable
projections rather than a fourth durable space.

Every generated Artifact Revision records the exact Context Assembly Manifest
through an available_as_context relation. The Manifest records known supplied
source references and input provenance, not a graph of model influence.
A direct item-level relation is added
only when that source is one of the provenance target kinds already admitted by
the Artifact domain model and the relation's own semantics are established;
the Host never widens the target union or invents a Revision merely because an
item was context. available_as_context proves availability only. derived_from,
supported_by, opposed_by, qualified_by, responds_to, and other closed
provenance relations are added separately only when their own semantics are
established; none is inferred from inclusion, disclosure, or model output. The
Artifact remains independently explainable without replaying the Run.

The [storage contract, section 6](postgresql-project-storage-isolation-and-migration-contract.md#6-canonical-payload-placement)
owns physical persistence. Messages, Research snapshots, Memory Documents,
conversation records, and recoverable context/operation evidence use PostgreSQL
under their existing domain ownership. Markdown, text, and JSON describe
content representation, not a separate persistent directory on the Server.
Active request assembly may use process memory; durable inputs and recovery
records cannot exist only there. Temporary files, exports, and Recovery Copies
retain their separate purposes. Provider-held state is external and cannot
replace StoryOS records. This contract adds no object store or file database.

### 2.4 Current sources, conversation history, and active context

New retrieval reads the current available source under its owning access and
lifecycle rules. Recorded conversation items have their own identities and
retention boundaries. Editing or ordinarily deleting a source does not rewrite
an earlier Message, captured result, or submitted request. Do not re-resolve
historical bytes through a mutable latest source or require proof that their
earlier semantic influence has disappeared.

Append ordinary author corrections and incomplete requests as Messages or
Steering Input for the next safe decision boundary. The Agent interprets them
with its available context. Earlier material may remain in active context while
later instructions guide the work. General compaction can reduce that context;
ordinary language does not create a semantic exclusion registry or automatic
Provider reset. Actual access revocation, Project deletion, redaction, and
restrictions on retained copies remain enforceable business controls.

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
- one independently established, immutable, non-authorizing Processing
  Destination Identity record under the same Project Scope, selected before
  gate one completes;
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
Model Route Request and derives the Model Route Decision from hard Purpose,
capability, context-bound, disclosure, authorization, and budget requirements
inside gate one after the candidate sequence below. Other destination kinds use
their owning immutable Registration or routing decision.
For each candidate, the Host starts with one global project-free Registration
and service surface. It then establishes or reuses one independent, exact
Project Scope-bound Processing Destination Identity; a Project Credential
Binding may supply non-authorizing account-boundary evidence when required, but
credential configuration or locator similarity grants nothing and cannot by
itself prove identity continuity. Establishment and every later
re-verification append an immutable same-Scope Identity evidence revision. The
Host next resolves the Project Destination Grant or other owning use
authorization for that already-existing
Identity, creates one Project Model Use Binding that pins the Registration,
authorization, Credential binding when required, Identity, its exact current
evidence revision, and hard bounds, and only then creates or resolves the
separate immutable External Contract
Compatibility Decision over that binding plus the Registration and Adapter.
The binding does not create the Identity and contains no compatibility
Decision. A Model Operational Snapshot then repeats the Identity, binding, and
Decision under complete Scope; a Model Route Decision may select only an
admitted Registration/Identity/authorization/binding/compatibility-Decision
tuple. A Registration or project-free Provider observation cannot satisfy any
scoped record in that chain.
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
| Explicitly attached or requested sources | Resolve available references under current access; ordinary wording is interpreted by the Agent, not compiled into a persistent context-control policy |
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
3. Author-requested Retrieval starts from an explicit source reference or an
   Agent interpretation of the author's request. It resolves available sources
   without requiring a formal source/version strategy from the author.

Author origin does not grant source authority, access, budget exemption, or
Disclosure Eligibility. A request with an unresolved reference may be clarified
through the ordinary Agent loop; no exclusion or suppression record is inferred.

When Memory use is enabled, a bounded navigation summary can point to currently
published Memory Documents. The Agent searches and reads selected documents
through ordinary retrieval tools. Do not preload all Memory or require an
embedding index. Apply the Memory owner's separate use and generation settings;
a Memory Note is guidance for consolidation, not an access-control rule.

Speculative Context Prefetch may warm a StoryOS-controlled Project Scope-bound
index or cache. It selects and discloses nothing. Prefetch that itself calls an
external or separately controlled processor is a full independent operation.

Similarity-based content may not be silently inserted into an in-flight Model
Attempt. Retrieval never rewrites the Step Snapshot whose Agent Decision asked
for it.

### 4.2 Retrieval indexes

Full-text, vector, graph, and other indexes are disposable projections. Their
internal IDs, scores, copied filters, and availability carry no durable
identity, truth, authority, owning-domain qualification, permission, or
disclosure right.

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

Every StoryOS-injected context fragment is structured, attributable, inspectable,
and hard-capped. No single injected item may exceed 10K tokens. Introducing a
new item kind that can exceed 1K tokens requires explicit design review. These
are repository invariants, not ranking-tuning defaults. Whole-project growth,
pagination, cache warming, and provider continuity cannot expand the permitted
Host input. These caps do not assert a count of opaque Provider-internal items.

## 5. Gate three: Source Eligibility Gate

Eligibility is fail-closed and completes before any relevance ranking. For
every Context Candidate the Host checks:

1. exact domain identity and Context Source Version resolution;
2. exact matching Project Scope for project-bearing content and trusted
   requester permission;
3. current Source Integrity and retained payload availability when owned by the
   source kind;
4. applicable owning-domain Lifecycle, Archive, Tombstone, and Retention State;
5. current memory-use settings when newly reading a Memory Document or summary;
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

Apply these checks to the source actually read or referenced. A retained
Message or result is not a fresh read of every source mentioned inside it.
Ordinary source changes do not recursively disqualify conversation history,
summaries, or Provider references. A real restriction that also covers retained
copies still applies. When that restriction cannot be enforced for an opaque
reference, stop its reuse and admit a permitted reconstruction or record a Hold.

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

Unrecorded truncation, unqualified Provider-default summarization, and
replacement of original source evidence are forbidden. Optional Tool output
may use bounded excerpts with explicit cut boundaries; Exact Required content
retains its complete-content requirement.

Every lossy Projection records:

- its own immutable identity;
- exact Project Scope;
- known exact input Context Source Versions and prior Projections;
- Purpose, exact Processing Destination Identity, and applicable destination
  intake and disclosure policy revisions;
- projection mode and transformation-policy version;
- deterministic or generated producer identity;
- input and output extents;
- known omitted ranges or modalities, expected loss, and explicit uncertainty;
- one structured Projection Loss Indicator;
- creation operation and manifest references.

A Projection never overwrites its source, changes its source's authority, or
masquerades as original evidence.

Deterministic excerpts record their exact cuts. Generated summaries record
their actual input, generator, output, and known loss; they cannot prove an
exhaustive list of omitted meanings. Unknown semantic loss remains unknown.

Outbound data categories follow exact provenance through every excerpt,
summary, generated query, digest, de-identification, and other transformation.
A Projection cannot silently shed a protected category or disclosure bound.
Any narrower derived classification requires an explicit versioned
transformation policy and attributable classification decision; uncertainty
retains the more restrictive source category.

### 7.1 Compaction

General compaction prepares bounded active context for later model calls,
including calls within the same conversation turn and active AgentRun. It uses
general task, progress, decision, author-constraint, and pending-work guidance.
It is not triggered by a special class of creative instruction and does not
promise precise forgetting or exact semantic preservation.

Record the known bounded input or prior compaction reference, producer and
generation contract, output, reported usage, and loss or unknown-state evidence.
Re-compaction keeps available prior references and any known gaps. It requires
neither a complete semantic influence graph nor reconstruction of all earlier
raw content. Retained-copy access restrictions still apply to the actual input.

Install the result only for a later request. A changed Effective Model Context
uses a new RunStep and Model Invocation under ADR 0033. Preserve Messages, Run
Events, Tool results, Step Snapshots, manifests, and original request evidence
under their owning retention rules. Active compaction is not Operational
History Compaction, deletion, or background Memory consolidation.

Each additional StoryOS-controlled model, Tool, or external compaction request
is an admitted seven-gate operation. A returned summary or native compaction
object must satisfy the selected route's validated result contract before
later use. Record opaque Provider output under its original mapping, with
unknown internals clearly marked. Native compaction, context editing, cache,
and Responses transport are separate capabilities; unknown required behavior
blocks that path. A Host-managed summary can use ordinary admitted model work
without claiming native support. Preserve required current instructions,
Working Target, native Tool correlation, and pending work at the next boundary.

### 7.2 Cache and continuity

A Context Cache Entry is a disposable acceleration product keyed by exact
Project Scope, Context Source Versions, policy and transformation versions,
qualification state, destination identity, grant, and Adapter mapping.

Before reusing cached new-source reads, revalidate current source identity,
version, Lifecycle, Retention, memory-use settings where applicable, permission,
policy, destination, grant, and Adapter dependencies. A stale read cannot be
served as current content. Cached historical input uses its recorded identity
and current retained-copy access rules, not the latest version of every source
it mentions. Physical cache cleanup may follow logical invalidation.

Provider prompt caches, prior-response handles, encrypted compaction objects,
and session continuity are Opaque Provider Continuity. They may optimize a Wire
Payload Projection but cannot become StoryOS history, a Context Candidate, cached
permission, or the only evidence of Effective Destination Context. Use ADR 0033's
current binding, authorization, and validated request mapping. A delta/reference
can avoid resending prior input; it does not prove internal content or attention.
If the mapping cannot represent the new request, use admitted full input or a
new transport continuation without replacing Project Conversation identity.
Ordinary source changes do not automatically reset the chain. Actual revocation,
retained-copy restrictions, explicit reset, and unusable handles retain their
own operational boundaries.

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
- every rejected candidate with its applicable access or policy reason;
- Context Trust Assessments and destination Disclosure Eligibility;
- Ranking Profile, ranking inputs, stable outcome, and selection reasons;
- exact selected Context Source Versions and Projections;
- projection modes, loss indicators, known input references, and evidence gaps;
- context budgets, allocation, omissions, and unmet needs;
- Complete, Degraded, or Blocked Context Sufficiency Decision;
- authorization policy, grant, approval-requirement, cache-reuse, and
  transformation versions, plus any Approval already effective when the
  manifest was committed;
- links to any predecessor assembly replaced at a later request boundary.

Failure to durably commit the manifest prevents destination I/O. A debug log,
trace span, reconstructed transcript, provider request object, or cache entry
is not a substitute.

The manifest proves what StoryOS considered and logically prepared. It does not
prove destination submission, exact wire bytes, Provider retention, model
attention, internal model use, evidentiary reliance, or output correctness.

Manifests are immutable historical evidence. Later source changes affect new
reads; real permission, retention, and copy restrictions affect the records
they govern. Show current availability separately without rewriting earlier
preparation. Known references and Provider reports do not supply exact unknown
internal content. In this contract, request or payload closure means the
declared input and reference dependencies, never a semantic influence graph.

## 9. Gate seven: Destination-specific Disclosure and Attempt

Every StoryOS-controlled submission has its minimum-necessary projection after gate six.
Sharing a Provider, connection, SDK, Run, or cache never combines destination
authority.

### 9.1 Processing boundary classifications

This table describes actual StoryOS-controlled submissions. A bounded hosted
operation uses its owning submission evidence under section 9.6, not a separate
Host Attempt for every Provider-internal destination or query.

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
- one exact, independently established Processing Destination Identity under
  the same Project Scope, pinned by the scoped use binding together with that
  binding's exact current Identity evidence revision and referenced project-free
  Registration Revision;
- the exact Project Scope-bound external-use binding, the separate subsequent
  compatibility Decision over that binding, and Credential binding generation
  when the destination requires one;
- one Purpose and the applicable owning intake contract: Model Capability
  Profile plus Model Attempt Request for a model, or Destination Context Intake
  Contract for a non-model destination;
- when hosted work is enabled, the complete admitted operation and its explicit
  intake, prior-state access, Tool set, and outward-processing bounds;
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
Identity and current evidence revision, Project Scope-bound external-use
binding, separate compatibility Decision, manifests, and semantic request.
Model Attempt and the owning
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
Destination Attempt Admission Decision over current Project Scope, the actual
source and Projection dependencies, Lifecycle, retained-copy restrictions,
memory-use settings for newly read Memory, requester
permission, grants and exact Tool or Destination Disclosure Approval when
required, destination identity and current evidence revision, Registration
status, governing intake contract, policy, and budget. Only an admitted
Decision may submit.

Any invalid required dependency preserves
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
Project Model Use Binding, subsequent compatibility Decision, destination and
disclosure manifests, and Destination Attempt. It cannot carry the prior
route's Credential Reference binding or compatibility Decision forward. If the
actual processor, endpoint/account, control classification, or
intake/disclosure boundary changes, the Host first establishes a new Processing
Destination Identity and then requires authority for that identity. A
Registration, Adapter, serialization, or model revision change that preserves
those
boundaries retains the same Processing Destination Identity and may use its
still-effective Project Destination Grant, but it requires a new Project Model
Use Binding when its pinned contract changes, then a separate compatibility
Decision plus fresh intake, wire, cache, and admission evidence. Either form
creates a new disclosure occurrence and Event only if its Destination Attempt
reaches the durable dispatch claim. Credential rotation may retain the same
Identity only when fresh evidence is appended immutably under the same Scope
and verifies the same actual account
boundary; the new use binding pins that evidence revision, while a matching
Credential Reference, locator, or Provider alias is insufficient.
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
- Provider-reported content, processing, or usage with its original association;
- provider-opaque continuity or internal state.

Inspection may report all four but never present the provider-opaque part as
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

Provider-hosted Tools do not inherit the Model Attempt's context, Capability,
or disclosure grant. ADR 0034 binds their complete enabled set, explicit intake
including intentionally referenced state, permitted processors and outward
destinations, and enforceable bounds before the owning Model Attempt submits.
When this is one physical submission, use that Model/Destination Attempt and
its disclosure evidence; do not double-count it or invent internal dispatches.
If the Provider cannot limit access to admitted intake and outward processing,
the mode is ineligible. A prompt instruction alone is not enforcement.
Internal queries, consumption, and transfers that StoryOS cannot observe stay
unknown; returned events are Provider reports, not Host gate decisions.

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

For Provider-hosted work, these steps govern returned-result intake and later
StoryOS submissions. They do not assert a Host check before internal Provider
consumption. Preserve native item/call correlation, reported source identities,
limits, and uncertainty. Validate the returned shape, size, and source binding.
Only the selected complete Model Attempt whose entire Agent Decision validates
and becomes durable can enter ordinary continuation. Incomplete, rejected,
cancelled, unselected, or fenced output remains evidence only. A validated
partial research deliverable retains its sources and unmet objective under
ADR 0034. Provider-reported sources are not StoryOS-captured Research snapshots
unless StoryOS actually captures and validates them through that domain path.

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
- exact content, known references, Provider reports, and opaque internal state.

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

### 11.2 Ordinary steering and source requests

Use the general Agent loop for use this, do not use this, corrections, and
incomplete creative requests. Record the author's Message and any explicit
source reference. The Agent decides what to read or how to continue under the
existing instruction hierarchy; the Host does not compile that language into
Include, Pin, Exclude, or Suppress records, persistent negative rules, or a
semantic removal graph. Ordinary steering takes effect at the next safe
RunStep and cannot mutate an already submitted Attempt.

A selected attachment identifies a source for ordinary retrieval. It grants
no access, authority, or disclosure permission and does not require a permanent
follow-version strategy. When the meaning matters and remains unclear, the
Agent can ask a normal question. Context compaction remains general capacity
management, not deterministic compliance with a semantic forgetting request.

### 11.3 Settings, Memory Notes, and business operations

Use the Memory contract's separate use and generation settings. An explicit
memory-change request can produce a plain-language Memory Note for later
consolidation; it is not a per-claim suppression or admission decision.

Project Instruction, source edits, Archive, Tombstone, Redaction, Project
Deletion, access permissions, and destination grants retain their owning
settings or commands. Disabling a destination changes its use authority while
preserving its immutable identity. The Agent may help the author use an
available business tool; the tool applies its existing validation and approval
contract. No contextual preference grants new authority or overrides a real
restriction on source or retained-copy access. No action recalls prior
external disclosure or guarantees deletion of unknown Provider-internal state.

### 11.4 Default context experience

The editor Agent automatically receives the eligible current Working Target,
current instructions, and bounded conversation continuity. Memory can provide
summary navigation and on-demand reads when enabled. The author need not
configure context scopes, source versions, a character sheet, pins, manifests,
or Project Instruction before ordinary help works. Inspection stays on demand;
ordinary already-authorized work has no additional confirmation ceremony.

## 12. Project Instruction

Project Instruction is optional. At creation of each top-level AgentRun,
StoryOS binds either the exact effective Project Instruction Revision or the
explicit fact that none was configured. The binding never changes in place.

The bound Revision is Mandatory Context for every model step in the AgentRun
and every descendant Subrun, which must independently reference the same
binding. Each request supplies the required bound instruction under the
validated profile. Compaction, cache, and continuation cannot replace it with
stale summary text; an unsupported mapping requires retransmission or blocks
the step. This proves request preparation, not model attention.

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

A safe retry is a new Destination Attempt. It revalidates its actual input,
retained-copy restrictions, current source-read permissions, policy, retention,
grant, destination, and budget dependencies. If the
prior Destination Attempt may have submitted, the successor requires authority
and budget for an additional disclosure and cannot erase the predecessor's
uncertainty.

Source changes affect future reads and caches of those reads, without rewriting
captured conversation history or automatically invalidating its continuation.
Real permission, redaction, deletion, or retention restrictions apply to the
exact sources and retained copies they govern as soon as effective. A Project
Destination Grant change affects operations bound to that grant, not unrelated
internal retrieval or another destination. Physical cleanup may follow logical
ineligibility. When permitted continuation is unusable, apply ADR 0033's bounded
recovery; missing responses do not prove expiry or authorize unknown effects
to be repeated. Cancellation and predecessor fences remain in force.

No recovery, compaction, or cache process rewrites historical Context Candidates,
manifests, Projections, Disclosure Events, Destination Attempts, or the exact context
evidence held by a prior Run.

### 13.1 Contract-change and migration impact

This revision changes planned context controls, compaction evidence, continuation,
and hosted-result boundaries. StoryOS already has a Rust workspace, PostgreSQL
schema, editor runtime, and generated contracts. This documentation revision
changes none of those physical formats and is not a migration.
The protocol, storage, and retention owners must compare their current forms
and published tickets with this contract, remove obsolete semantic controls,
and identify any affected persisted forms before implementation. A required
schema, event, replay, cache, export, or recovery migration must be explicit;
do not infer compatibility from this planning PR.

## 14. Downstream ownership

This specification fixes semantics and hands off physical or wire realization:

| Ticket | Owns next | Must preserve |
|---|---|---|
| [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md) | tables, constraints, transactions, indexes, payload storage, migrations, backup, restore, export, and Credential Reference integration | manifest-before-egress durability, immutable history, exact Project Scope, cache invalidation, rebuildable indexes |
| [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md), resolving [Threat-Model the StoryOS Service, Client, and External Trust Boundaries](https://github.com/FrankQDWang/StoryOS/issues/57) | credible attack paths, structural mitigations, residual risks, verification evidence, and downstream security ownership | orthogonal trust axes, fail-closed eligibility, destination-specific grants, no ambient context |
| [Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58) | DTOs, schemas, commands, event envelopes, compatibility, and errors | exact identities, current settings, known/opaque context evidence, hosted operation, attempts, and Project Scope; remove obsolete semantic controls |
| [Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) | fake destinations, crash/retry/replay and isolation tests | seven gates at Host-controlled boundaries, bounded hosted intake, ordinary steering, compaction transitions, and uncertainty; no semantic-correctness oracle |
| [Record the Deferred Eval Observation Boundary](https://github.com/FrankQDWang/StoryOS/issues/61) | Future observation surface outside MVP; no current design or implementation handoff | Existing Context and disclosure evidence keeps its independent runtime contract |
| [AI-Independent Editor-First Release Baseline and Handoff](https://github.com/FrankQDWang/StoryOS/issues/62) | implementation slice and acceptance gate | zero-configuration manual editor, later external model and embedding APIs, Provider neutrality |
| [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md) | long-term retention, payload redaction, archival, export, and deletion | historical context truth, compaction lineage, manifest and disclosure evidence |

No new follow-up ticket is required. These existing owners cover every
remaining physical, security, protocol, verification, slice, and retention
decision exposed by this contract.
Their older contracts and stage tickets are not declared aligned by this
revision. Release and proof revisions precede /to-spec and the user-approved
/to-tickets refresh. Stage 3 and later product implementation remains on
EXECUTION HOLD; exact Provider account/model qualification remains separate.

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
5. Disabled Memory use prevents a new summary/document read even when its
   retrieval score is high; an index hit grants no access.
6. Ranking changes selection but cannot change source authority, evidence
   status, or disclosure permission.
7. An Exact Required item that cannot fit blocks or reroutes and is never
   silently summarized.
8. Compaction between calls in one turn records its admitted input, output,
   known references, and loss/unknown facts without changing prior requests.
9. Real permission or retained-copy restrictions prevent an otherwise valid
   cache or opaque reference from bypassing current admission.
10. A Tool or MCP result becomes Data-only Context Candidate content for a later step
    and crosses all seven gates again.
11. Ordinary non-use guidance appends a Message without creating an exclusion
    registry, mutating past input, or automatically resetting continuation.
12. Updating Project Instruction does not change an active AgentRun binding;
    a new top-level AgentRun receives the new Revision.
13. A Subrun independently proves the same Project Instruction Binding and
    exact Project Scope as its root.
14. Model Fallback reruns all seven gates for the newly resolved route while
    preserving the same Model Invocation, Route Request, semantic request
    digest, and Effective Model Context; it uses the new route's exact Project
    Model Use Binding and subsequent compatibility Decision rather than
    inheriting the prior Credential binding or Decision. A dispatched fallback
    creates a new Destination Attempt, wire projection, and Disclosure Event,
    while semantic drift refuses fallback.
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
    Memory update. It describes content as sent only when confirmation
    evidence establishes ConfirmedSubmitted.
21. An invalid required policy, permission, grant, retained-copy, Lifecycle, or
    destination dependency after manifest commit but before I/O makes final
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
24. New source reads reflect source edits or deletion while retained conversation
    items keep their identities and applicable access/retention boundaries.
25. A bounded Memory summary navigates to on-demand document reads. Memory
    updates and Notes do not rewrite history or become instruction authority.
26. A hosted operation binds its complete intake and outward-processing bounds
    before submission. Its actual submitted Attempt is counted once; invisible
    internal queries remain unknown. Returned reports are not Host observations.
27. Rejected, cancelled, incomplete, or fenced hosted output cannot advance
    ordinary continuation; a complete validated partial result retains limits.
28. An incompatible delta/reference mapping uses admitted full input or a new
    transport continuation without changing conversation identity. Unknown
    native compaction capability cannot be inferred from Responses support.

These cases prove Host transitions and evidence, not that a model understood
every instruction, forgot a source, or produced an exact semantic summary.

## 16. Normative invariants

1. Every project-bearing fact binds one exact Project Scope.
2. No gate is skipped, merged, inverted, or retroactively simulated for a
   StoryOS-controlled submission; hosted internal steps use ADR 0034's boundary.
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
12. Excerpts identify exact cut boundaries; required complete content is not cut.
13. Each StoryOS-submitted Projection generation crosses all seven gates.
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
20. Each submitted operation has minimized input and authorized destination
    bounds, including its permitted hosted processors and outward processing.
21. Every local outbound dispatch or redispatch has its own Disclosure Event
    and Destination Attempt evidence with explicit submission certainty;
    OutcomeUnknown is treated conservatively without being called confirmed
    submission.
22. A prior Destination Attempt never proves a later execution.
23. Provider promises and retention labels never undo Outbound Disclosure.
24. Tool and MCP execution trust never grants truth or Instruction Authority to
    returned content.
25. Received Tool, MCP, and hosted results cross the contract before a later
    StoryOS submission; invisible internal consumption is not Host admission.
26. Author guidance never overrides source/retained-copy access, Capability,
    destination policy, or actual Memory settings.
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
    source, policy, permission, Memory, retention, or cache changes.
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
