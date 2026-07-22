# Foundation Evidence for the Standalone Eval Surface

- Status: accepted
- Wayfinder resolution: [Define Foundation Evidence for the Standalone Eval Surface](https://github.com/FrankQDWang/StoryOS/issues/61)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Durable-domain boundary: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Context and disclosure boundary: [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- Run boundary: [Persistent Agent Run and Orchestration Semantics](https://github.com/FrankQDWang/StoryOS/issues/47)
- Model boundary: [ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50)
- Storage and isolation boundary: [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md)
- Protocol boundary: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)
- Trust boundary: [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md)
- Repository and delivery boundary: [Modular-Monolith and Repository Governance Boundaries](modular-monolith-and-repository-governance-boundaries.md)

## 1. Purpose and authority

This specification defines the foundation evidence contract for StoryOS Eval.
Eval is a standalone, author-facing product surface for observing and
reviewing selected project evidence and advisory assessments. It is not the
main writing surface, a Transcript MCP App, an administration dashboard, a
background monitoring service, an ambient telemetry channel, or a replacement
for the author-facing editor and Agent loop.

The contract makes an Eval Case explainable, historically inspectable, and
auditable without constructing a second truth store. It identifies which
existing durable facts may be referenced, what an Eval-facing projection may
say about them, and what it must never imply. It applies equally to a
bootstrapped local User and to a future controlled cloud deployment with
multiple isolated Users.

This specification is normative for later Eval-facing contracts and
implementation. It composes accepted authoritative-state, Artifact,
operational-record, run, context, disclosure, protocol, storage, and trust
boundaries; it does not reopen them.

It deliberately does not select:

- a database schema, table, index, or migration;
- a Rust, TypeScript, HTTP, SSE, or UI implementation;
- a complete Eval interaction design or page layout;
- an evaluation corpus, rubric, metric formula, or judge Provider;
- a retention duration, redaction algorithm, archival format, export format,
  restore procedure, or deletion implementation; or
- deterministic test implementation, CI runner, or first production slice.

Those choices remain with their existing owners identified in
[section 12](#12-downstream-ownership-and-handoff).

## 2. Fixed product position

StoryOS is a general project-scoped discovery-writing Agent Loop. The author
discovers the novel in the editor, may tell the adjacent Agent what they want,
and retains every authoritative creative decision. Eval helps the author
observe evidence about selected work; it does not introduce an Agent-authored
outline, Author Plan, fixed story workflow, hidden quality authority, or a new
ordinary author configuration burden.

The following product boundaries are fixed:

1. Opening Eval is a read of existing Project Scope-bound evidence. It does
   not itself dispatch a model request, call a judge, upload project content,
   change the manuscript, create an Eval Case, add a corpus row, or emit
   background-monitoring data.
2. An Eval Case is explicitly selected. Ordinary writing and ordinary
   AgentRuns retain their existing durable facts, but do not automatically
   become evaluation data.
3. An optional Evaluation Corpus contains only explicitly selected,
   Project Scope-bound Eval Cases. It is neither a shadow archive of all Runs
   nor a prerequisite for using the writing Agent.
4. Eval conclusions, metrics, comparisons, and feedback are advisory. They
   never become Authoritative State, an Author Preference, an Agent
   instruction, a route-selection signal, a capability grant, a disclosure
   grant, a score-derived creative target, or an Acceptance substitute.
5. An external assessment is an explicit new operation. It crosses the
   ordinary authorization, minimum-disclosure, Attempt, Receipt, and
   OutcomeUnknown boundaries before any project-derived content can leave
   StoryOS.
6. The Eval page is a product for the author, not an opaque system for judging
   the author. Its limits, evidence gaps, and redactions must be legible to
   the authorized author rather than silently hidden behind a complete-looking
   dashboard.

If an observation inspires a creative change, the author returns to the
editor or makes a new ordinary Agent request. Any non-deterministic,
bulk, cross-location, Agent-, Tool-, MCP-, or extension-produced change still
requires an inspectable Core Proposal and explicit Author Acceptance. Eval
cannot create a shortcut around that boundary.

## 3. One evidence model, not a fourth truth space

The Artifact domain model already has three disjoint durable spaces:
Authoritative State, Artifacts, and Operational Records. Eval introduces no
fourth space and cannot relabel a projection, metric, or score as truth.

An Eval Case is a frozen selection of exact settled references into that
existing model. An Eval Evidence View is a read-only, minimum-necessary
projection over those references. An assessment may add a new advisory
Analysis Report Artifact and its supporting Operational Records, but neither
rewrites the selected evidence nor owns author state.

> Explicit case selection leads to exact settled evidence references and a
> read-only Eval Evidence View. An optional explicit assessment attempt records
> ordinary authorization, disclosure, Attempt, and Receipt facts, then may
> produce an advisory judgment or result. An optional explicit
> baseline/comparison and case-scoped author feedback can reference that result.
> None of these arrows changes Authoritative State.

### 3.1 Evidence closure

An Eval Case has an evidence closure: the minimum exact set of retained
references needed to explain the case selection and any later assessment.
The closure is a reference graph, not a copied prompt transcript or a new
canonical dataset.

For every referenced item, the closure records or can resolve:

| Requirement | Meaning |
|---|---|
| Exact Project Scope | One trusted pair of owner_user_id and project_id. Every reference, query, cache, projection, and authorization check remains inside it. |
| Stable identity | The original logical identity appropriate to the source: Artifact, revision, Run, RunStep, manifest, Attempt, receipt, event, or immutable protocol record. |
| Exact revision or snapshot | A historical reference pins the specific Artifact Revision, Context Source Version, manifest revision, protocol record, or committed Snapshot rather than a mutable latest view. |
| Provenance | The closed relation, creation record, or operation boundary that explains why the item is present. Selection is not silently promoted into evidentiary support. |
| Time and causality | Creation and settlement time, relevant Project Activity position or Snapshot, and available cause/correlation relation. An item created later is never presented as earlier evidence. |
| Availability and redaction | Current lifecycle, authorization, redaction/disclosure profile, and an explicit explanation when the item cannot be shown in full. |
| Stability | Whether the fact is immutable, append-only operational history, a rebuildable projection, host-observed, provider-reported, or provider-opaque. |
| Explainability limit | What the item proves and the next fact it does not prove. |

The case selection itself may reference only settled facts. A terminal
OutcomeUnknown Attempt may be included only with its exact uncertainty status;
a pending operation is not eligible for case selection. Neither is converted
to a completed or successful assessment for display convenience.

### 3.2 Case time is not current time

An Eval Evidence View may be opened long after its case was selected. It must
distinguish:

- the selected historical evidence closure;
- the view's committed read Snapshot, projection watermark, and generation
  time, when applicable;
- the current visibility or redaction state of each referenced item; and
- a later assessment or comparison that references the case.

Later correction, suppression, archival, tombstoning, permission change,
provider drift, or projection rebuild cannot rewrite the historical selection.
It may make some payload unavailable; the view then describes the gap rather
than presenting a new current version as if it were the original evidence.

## 4. Classification of Eval-facing records

The following classification is semantic. It deliberately does not prescribe
tables, wire shapes, aggregate ownership, or UI components.

| Eval-facing concept | Existing durable classification | Required durable relation | It must not become |
|---|---|---|---|
| Eval Case selection | Operational Record | Explicit selector, Project Scope, selection time/cause, and exact settled evidence closure | An automatic Run archive, benchmark row, or Authoritative State |
| Evaluation Corpus membership | Operational Record | Explicit case membership and its selection/revision identity, all in one Project Scope | A cross-Project dataset or required author setup |
| Eval Evidence View | Read-only projection | Source boundary, authorized view/redaction profile, freshness or Snapshot information, and exact case references | A canonical truth store, hidden telemetry, or prompt dump |
| Metric or rubric application | Operational input record | Exact metric/rubric definition identity, version or digest, evaluator identity when applicable, and the case/result it qualifies | An implicit global quality target or author instruction |
| Eval Assessment Attempt | Operational Record using the normal run/attempt boundary | Explicit initiator, purpose, case closure, authorization, disclosure, destination Attempt, certainty, and Receipts as applicable | A page-load action or ambient external-processing grant |
| Usage, cost, latency, route, and cache observations | Operational Record or rebuildable operational projection | Observation source, measured interval or provider report, associated operation/Attempt, and certainty | A semantic quality fact or proof of model attention |
| Eval Judgment and Eval Result | Analysis Report Artifact Revision | Exact case, metric/rubric input, assessment Attempt or human origin, evidence closure, and provenance to any cited facts | Authoritative State, a Proposal, Acceptance, or a hidden score authority |
| Eval Baseline selection | Operational Record | Explicit selected case/result revisions and exact metric/rubric definition | A default project score or global benchmark |
| Eval Comparison conclusion | Analysis Report Artifact Revision | Explicit baseline selection, compared exact revisions, metric compatibility statement, and limitations | A routing rule, author goal, or verdict on unselected work |
| Eval Author Feedback | Message Artifact when durable text is saved, plus its case-scoped association record | Author identity, exact case/result target, time, and advisory relationship | An automatic Author Preference, instruction, acceptance, or state change |

An Analysis Report remains advisory even if it contains a numerical metric, a
human opinion, or output from a trusted Skill. A score is content inside an
advisory Artifact; it never gains product authority merely because it is
stored, repeated, compared, or visually prominent.

### 4.1 Metric definitions and evaluator identity

A metric can be qualitative, quantitative, or mixed. Its application must
pin an exact definition. A globally reusable metric definition may be
unscoped only when it contains no project-derived content or project
authority; its application to an Eval Case is always Project Scope-bound.

An evaluator identity identifies the human, StoryOS-owned deterministic
operation, Skill, Tool, or external Provider role that produced an assessment.
It is provenance, not a grant of truth or authority. A Provider name,
model-label, cache claim, or output digest does not make a result objective,
stable, or comparable to a result made under a different declared metric or
evaluator context.

### 4.2 Results, corrections, and supersession

Eval Judgments, Results, and Comparison conclusions are immutable Artifact
Revisions. A correction, reassessment, or changed interpretation creates a
new derived or superseding advisory Artifact with an explicit relationship to
the earlier result. It never overwrites the original observation.

The original result remains inspectable subject to lifecycle, redaction, and
retention rules. If its source is tombstoned or redacted later, the result
continues to identify that limitation and does not silently claim complete
reproduction.

## 5. Evidence families and their explainability limits

The Eval page can expose only claims supported by the following existing
evidence families. Each family must be rendered with its own certainty and
limit rather than merged into a generic "context used" or "model history"
story.

### 5.1 Run, operation, and author-action evidence

| Evidence | What Eval may say | What Eval must not say |
|---|---|---|
| AgentRun, RunStep, RunPlan, RunEvent, mailbox or lifecycle record | Which StoryOS operation was admitted, its declared purpose, state transition, and causal position | That the author agreed with the result, that a model followed the plan, or that a later manuscript state was caused by it without exact provenance |
| Direct Author Action, Proposal, Validation Receipt, Acceptance Receipt, Reject/Undo record | What exact author action or Proposal settlement occurred, against which revisions, and when | That an Eval judgment authorized, caused, or replaced that action |
| Operation and protocol receipts | What StoryOS durably settled or refused | A result beyond the receipt's stated certainty, especially an external outcome after OutcomeUnknown |
| Application Wire Record and Project Activity position | The exact public boundary, committed Snapshot, sequence, or replayable historical record where permitted | A copy of all runtime memory, secrets, raw request bodies, or an external Provider's internal trace |

An Eval Case can cite a manuscript Artifact Revision, a Proposal Revision, or
an author action to explain the before/after state. It must not infer
authorship, creative intent, quality, or causation from proximity in time
alone.

### 5.2 Context, retrieval, and projection evidence

Context Assembly establishes a ladder of distinct facts. Eval preserves that
ladder rather than flattening it:

| Recorded fact | Eval may explain | Eval must not infer |
|---|---|---|
| Stored | StoryOS retained the source or record | That it was considered, selected, sent, or used |
| Discovered | Candidate discovery located the exact source identity and Context Source Version | Eligibility, relevance, support, or disclosure permission |
| Eligible | Current qualification allowed the candidate to proceed | Selection, projection, or destination use |
| Selected | Ranking and budget chose the eligible candidate | That the selected content was fully represented or sent |
| Projected | A destination-oriented representation was created, including declared loss where applicable | That the original source, full text, or projection was used internally |
| Context Assembly Manifest committed | The complete assembled source closure was durably prepared before disclosure | That any destination received it, that the exact secret-bearing wire was identical, or that a model used it |
| Destination disclosure and Attempt evidence | What was authorized, what crossed StoryOS's dispatch boundary, and the best-known submission certainty | Provider retention, cache behavior, model attention, or semantic reliance |

An Eval Evidence View may show selection reasons, ranking profile identity,
budget outcome, compaction lineage, projection loss, and insufficiency or
degradation reasons when the viewer is authorized. These are explanations of
StoryOS decisions, not assertions that the source was correct, authoritative,
or effective.

### 5.3 Source, Artifact, and provenance evidence

Artifact evidence uses the Artifact domain model's stable logical identity,
immutable Revision identity, content digest, source snapshot, and typed
provenance edges. The Eval view must preserve their different meanings:

- A digest helps identify exact bytes; it is not an Artifact identity and does
  not authorize cross-Project lookup or deduplication.
- A selected context item establishes availability to an operation, not
  evidentiary support for an Eval Judgment.
- A provenance edge means only its closed relation type. For example,
  available_as_context does not become derived_from, supported_by, or proof
  that a model used a source.
- A Source Snapshot or Research Artifact remains source evidence with its own
  origin and lifecycle. A later Eval result is a separate derived Analysis
  Report, not a rewrite of the source.

### 5.4 Usage, latency, cost, route, and cache evidence

Usage and latency are operational observability facts. Every Eval rendering of
them identifies whether it is:

- host-observed at a named StoryOS boundary;
- Provider-reported;
- Host-estimated from declared accounting rules; or
- unknown, unavailable, or incomplete.

The view may show an exact Attempt, measured interval, declared route,
provider-report reference, cost settlement, or cache/continuity observation.
It must not transform these into claims about literary quality, author
behavior, provider-side processing duration, token attention, data retention,
or a model's actual use of a source.

Provider cache, continuity, prior-response, and opaque provider-session
mechanisms are external state. A Provider-reported cache hit or a
StoryOS-observed continuity reference is useful operational evidence only. It
does not establish what was retained, recalled, attended to, or shared inside
the Provider, and it never bridges Project Scopes.

### 5.5 External judge output

An external judge output is untrusted external content until StoryOS records
it through the ordinary result boundary. Its retained advisory Result must
identify the Assessment Attempt, declared evaluator identity, exact metric or
rubric definition, available output provenance, and all known limitations.

The Result may say that a named evaluator returned an opinion. It cannot say
that the opinion is canonical, that the evaluator actually considered every
supplied input, or that the opinion is an authoritative fact about the novel.

## 6. Eval Evidence Views are minimum-necessary product projections

An Eval Evidence View is a user-facing read projection for one explicit Eval
Case. It exists to let an authorized author understand an observation, not to
collect telemetry about the author or expose a backstage control panel.

### 6.1 Required view boundary

Each view is bound to:

- the requester's current authorized Project Scope;
- the exact Eval Case selection;
- an authorized redaction/disclosure profile;
- its source Snapshot, projection watermark, and generation time when the
  data is projection-backed; and
- the current availability state of every material referenced source.

The view contains only the minimum evidence necessary to explain the current
case or result. It does not automatically expose full transcripts, all
project prose, raw prompts, credential-bearing transport details, unrelated
Run history, raw logs, or another Project's matching content.

Opening or refreshing the view is not an Eval Assessment Attempt and produces
no ambient external request. A product implementation may create ordinary
access auditing where independently required by the established platform, but
such auditing is not Eval evidence, does not alter the case, and must not
become hidden author monitoring.

### 6.2 Availability, redaction, and evidence gaps

Availability is visible evidence. If a material reference is redacted,
unavailable, archived, expired, permission-limited, tombstoned, suppressed,
or otherwise incomplete, the authorized view must show:

1. that the referenced evidence exists or historically existed, where
   disclosure policy permits that fact;
2. the applicable non-sensitive reason category and view limitation;
3. the exact historical identity or a safe non-oracular substitute when the
   identity itself cannot be disclosed; and
4. the resulting limit on the assessment or comparison.

The view must not reveal protected payload merely to make an assessment
look reproducible, silently drop a material gap while claiming complete
coverage, or fill the gap with an unproven substitute. It follows the
established non-oracle query behavior: a viewer without access receives no
extra existence or cross-scope information through Eval.

For a retained tombstone, the established PurgedSourceRef-style explanation is
the evidence of the gap. It is not permission to reconstruct erased content.

### 6.3 Projection is not canonical truth

An Eval Evidence View may be rebuilt after a read-model rebuild, schema
projection change, or authorized redaction change. Its presentation can differ
while its pinned case selection and any historical Analysis Report references
remain the same.

Every projection-backed display distinguishes its source boundary and
freshness. A stale or incomplete projection is not silently rendered as a
complete canonical account. A projection cannot mutate its sources, create
authority, grant disclosure, or become a retained substitute for an
unavailable Artifact Revision.

## 7. Explicit assessments and reproducibility

### 7.1 Assessment initiation

An author may explicitly initiate an Eval Assessment Attempt. A purely
read-only human observation needs no Provider call. A deterministic local
assessment and an external judge assessment are separate operation kinds;
neither is caused by merely opening the Eval page.

When an Assessment Attempt uses a Tool, MCP server, external judge, model,
embedding service, or other destination, it must obey the accepted context and
disclosure contract:

1. establish an operation purpose and exact Project Scope;
2. assemble only the minimum authorized case evidence through all seven
   context gates;
3. commit the Context Assembly Manifest before egress;
4. establish the destination-specific grant, disclosure, compatibility, and
   Attempt evidence;
5. preserve dispatch/submission certainty, including OutcomeUnknown; and
6. record the resulting external content as untrusted, advisory output.

An external assessment cannot reuse an earlier main-writing disclosure,
Provider session, cache, or authorization as an ambient Eval grant. A new
purpose, destination, wider category, or altered evaluator requires the
normal fresh boundary.

### 7.2 Reproducibility means reopening evidence, not rerunning a Provider

Eval reproducibility is evidence reproducibility. It means an authorized
viewer can reopen the same selected historical evidence closure and inspect
its known provenance, metric definition, assessment record, and limitations.
It does not promise that a Provider, judge, cache, route, or opaque mechanism
can be replayed to produce the same answer.

An Eval Result must state an Eval Reproducibility Status that distinguishes at
least:

- an inspectable closure whose material retained references can be reopened
  under the current authorized view;
- a limited closure with disclosed redaction, lifecycle, or evidence gaps; and
- an unavailable or unverifiable closure whose exact material evidence cannot
  now be inspected.

These are evidence-coverage statements, not determinism grades. An external
Provider may change its model, endpoint behavior, cache, policy, or hidden
state; a human evaluator may also change their opinion. A later reassessment
is therefore a new Assessment Attempt and new advisory Result, connected to
the older one for comparison but never replacing it.

### 7.3 What a result can claim

An Eval Result may truthfully say:

- which exact case evidence StoryOS selected and could disclose for the
  assessment;
- which evaluator and metric/rubric identity produced the observation;
- what StoryOS knew about dispatch or confirmed submission;
- what the evaluator returned, as attributed advisory content; and
- what evidence/redaction/opacity limits qualify the result.

It may not say:

- that a model internally read, attended to, remembered, or relied on a source;
- that a Provider retained, cached, or shared project content beyond recorded
  evidence;
- that the evaluation is objectively correct, canonical, or binding on the
  author;
- that a score represents a project-wide quality fact; or
- that a later manuscript revision is caused by the evaluation without an
  explicit, independently recorded author action or provenance relation.

## 8. Baselines, comparisons, and author feedback

### 8.1 Explicit Baselines only

There is no automatic project-wide score, default baseline, invisible
benchmark, or hidden creative target. A Baseline exists only after an explicit
selection of exact Eval Case or Result revisions together with the exact
metric/rubric definition used for the comparison.

A Comparison must state:

- the chosen Baseline and compared exact revisions;
- whether their metrics, evaluator identities, evidence coverage, and
  Reproducibility Status are compatible enough for the stated comparison;
- the relevant temporal order; and
- any redaction, provider opacity, or lifecycle limitation.

An incompatible comparison is an observation of incompatibility, not a reason
to normalize scores or fabricate a trend. Neither a Baseline nor a Comparison
becomes an Agent instruction, a manuscript goal, model-routing criterion, or
author configuration default.

### 8.2 Case-scoped author feedback

The author may save feedback about a case, judgment, or comparison. It is
case-scoped advisory context: for example, the author may explain that a
judge's criticism of a deliberately ambiguous scene was not useful for that
scene.

Saved feedback identifies its author and exact target. It may qualify how an
authorized reader interprets that particular evaluation. It does not
automatically update Author Preferences, Project Instructions, corpus
membership, future metrics, a baseline, model routing, an Agent request, or
Authoritative State.

If the author wants a feedback statement to become an actual preference,
instruction, or creative change, they use that domain's separate, explicit
operation. The Eval record remains evidence of the original observation, not
an implicit promotion mechanism.

## 9. Default evidence and optional evaluation data

Normal StoryOS use already retains the project-scoped durable facts needed to
explain an ordinary Agent operation: source and Artifact revisions, run and
operation records, context and disclosure manifests, Attempts, receipts,
usage, and author actions as established by the accepted contracts. This
foundation does not add another default capture pipeline.

The default author experience is therefore:

- write in the editor and use the adjacent Agent without creating an Eval Case;
- retain only the normal durable evidence required by that work;
- open the standalone Eval page only when the author wants observation; and
- incur no automatic external judge call, corpus configuration, benchmark,
  score, or recurring evaluation setup.

Optional evaluation data begins only when an author explicitly selects a case,
adds it to a Project Scope-bound corpus, or starts an Assessment Attempt.
Corpus membership is not a disclosure grant. An experimental corpus cannot
silently copy ordinary Runs, cause background judging, or make unrelated
project content visible.

This distinction preserves a useful future Eval product without turning the
discovery-writing Agent into a test harness or requiring authors to manage
evaluation configuration to write a novel.

## 10. Privacy, permission, isolation, and lifecycle boundaries

### 10.1 Exact Project Scope and future multi-User operation

Every Eval Case, selection, query, reference, result, baseline, comparison,
feedback association, cache, index, and evidence projection binds the exact
trusted pair { owner_user_id, project_id }. This applies from the initial
single bootstrapped local User onward; no global single-user shortcut is
permitted.

Matching digests, shared Provider accounts, similarly named manuscripts,
global metrics, caches, or opaque IDs never authorize a cross-Project join,
lookup, view, corpus membership, comparison, export, or disclosure. A global
metric definition contains no project-derived data. A Project-specific
application of it remains scoped and access-controlled.

### 10.2 Minimum necessary and permissions

Eval follows the same minimum-necessary rule as every other StoryOS
destination and read surface. The view is authorized per requester and current
Project Scope, and its redaction profile applies before presentation.

An Eval Result may cite a protected source without turning the citation into
permission to reveal it. The view exposes only what the requester may inspect,
plus safe evidence-gap information allowed by non-oracle behavior. A read
does not widen a Provider destination grant, author capability, Tool
capability, export right, or authority to accept a Proposal.

### 10.3 External processing, logs, and telemetry

Only an explicit Assessment Attempt may send project-derived evidence to an
external evaluator. It uses a destination-specific disclosure manifest and
receipts under the ordinary contract. The Eval page itself is not a
monitoring or telemetry sink.

Logs, telemetry, support evidence, and error surfaces receive only the
sanitized operational projection permitted by the trust model. They do not
inherit an Eval Evidence View, raw prose, raw prompt, source payload, judge
output, or credential-bearing wire data merely because they can name an
evaluation case.

### 10.4 Retention, archival, export, and restore

This specification fixes only the evidence obligation: a retained Eval
selection or Result must preserve its exact historical references and disclose
when any of them is unavailable, redacted, archived, suppressed, or
tombstoned. It does not choose how long payloads are retained or how archive,
purge, export, and restore work.

Any future Eval export or restore is a Project export or restore operation,
not a separate bypass. It must preserve Project Scope, provenance,
redaction/lifecycle status, and the distinction between canonical evidence and
derived Eval projections. It must never treat an exported projection as a
replacement canonical source or revive erased protected payload.

## 11. Required foundation invariants

An Eval-facing implementation conforms to this specification only if all of
the following remain true:

1. Eval creates no fourth durable truth space.
2. Every project-bearing Eval fact and read projection binds and validates one
   exact Project Scope.
3. An Eval Case and Evaluation Corpus are explicit selections, never default
   capture of ordinary writing or Agent Runs.
4. An Eval Evidence View is author-facing, read-only, minimum-necessary, and
   non-authoritative; opening it causes no external processing.
5. Every case reference pins an exact settled identity, revision or Snapshot,
   provenance, time/causal relation where available, availability state, and
   explainability limit.
6. Eval preserves the Stored-to-ConfirmedSubmitted distinctions and never
   claims model-internal use from StoryOS evidence.
7. Provider cache, continuity, prior-response, and opaque mechanisms remain
   explicitly opaque and cannot create cross-Project context, provenance, or
   reproducibility claims.
8. An external assessment has an explicit purpose and crosses ordinary
   authorization, disclosure, Attempt, Receipt, and uncertainty boundaries.
9. Usage, cost, latency, and cache observations identify their observation
   source and never become quality, authority, or provider-internal-use facts.
10. Redaction, absence, lifecycle, and projection gaps are visible to an
    authorized viewer without disclosing protected payload or becoming an
    oracle.
11. Reproducibility means reopening the declared evidence closure; a later
    evaluation is a new observation, not an overwrite or promised Provider
    replay.
12. A metric, score, baseline, comparison, judgment, or result remains
    advisory and explicitly scoped; there is no automatic project score or
    hidden creative target.
13. Author feedback is case-scoped and does not automatically mutate
    preferences, instructions, routing, corpus membership, evaluation
    configuration, or Authoritative State.
14. Any creative effect prompted by Eval follows the ordinary editor command,
    Proposal, validation, and Author Acceptance boundaries.
15. Retention, redaction, export, restore, verification implementation, and
    production-slice choices are consumed from their designated downstream
    owners rather than redefined here.

## 12. Downstream ownership and handoff

This specification closes the Eval evidence-model decision and hands off
physical realization without creating a new implementation ticket.

| Existing owner | Owns next | Must preserve from this specification |
|---|---|---|
| [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64) | Event/snapshot lifecycle, retention, archival, redaction execution, export, restore, deletion, and historical availability handling | Exact historical references, visible availability gaps, lifecycle/provenance distinctions, and no resurrection of erased payload |
| [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) | Deterministic verification design, adversarial proof gates, recovery behavior, and test implementation | Explicit-case-only capture, no page-load egress, Scope isolation, redaction/non-oracle behavior, uncertainty preservation, provider-opacity limits, and no authority crossing |
| [Lock the First Production Vertical Slice and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62) | The first shippable slice, user-facing sequencing, implementation priority, and acceptance criteria | A standalone author-facing advisory Eval product, normal author flow without evaluation setup, and no implementation that turns observation into hidden monitoring or control |

The later owners may choose concrete schemas, commands, queries, screens,
providers, archives, verification fixtures, and rollout order only if they
preserve these semantic boundaries. This ticket creates no evaluation corpus,
judge integration, UI design, or test implementation.

### 12.1 Verification scenarios handed off

Later verification gates must be able to demonstrate at least:

1. ordinary writing and ordinary Agent Runs retain their normal evidence but
   create neither an Eval Case nor external judge traffic;
2. an explicitly selected case resolves only exact same-Project settled
   revisions and shows its historical source boundary;
3. an Eval view does not create a Provider or judge call, disclosure, or
   corpus membership merely by opening or refreshing;
4. a redacted, archived, tombstoned, permission-limited, or missing material
   source remains a visible limitation without payload disclosure or a
   fabricated complete account;
5. cross-User and cross-Project references, caches, corpus rows, comparison
   targets, and projections fail closed;
6. a Context Assembly Manifest and a dispatch/confirmation record are shown
   with their distinct certainty without claiming model-internal use;
7. an explicit external Assessment Attempt produces separate authorization,
   disclosure, Attempt, Receipt, usage/latency, and OutcomeUnknown evidence as
   applicable;
8. a later Provider or human reassessment creates a distinct Result and does
   not overwrite an earlier one;
9. a comparison without an explicit compatible Baseline is refused or rendered
   as incompatible rather than converted into a global trend; and
10. Eval feedback, judgment, score, or comparison cannot change
    Authoritative State, author preferences, Agent instructions, routing, or
    disclosure authority without an independently authorized operation.

## 13. Accepted inputs and evidence

This specification consumes and does not reopen:

- the author-owned Authoritative State and Artifact/Operational Record
  separation in [Artifact and Authoritative-State Domain Model](artifact-domain-model.md);
- the accepted persistent Agent Run, Tool/MCP, and ModelGateway boundaries
  recorded in [CONTEXT.md](../../CONTEXT.md) and their accepted Wayfinder
  issue history;
- the seven context gates, immutable Context Assembly Manifest,
  destination-specific disclosure, Attempt, cache-opacity, and author-inspect
  boundaries in [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md);
- the exact Project Scope, PostgreSQL authority, cache/index, archival,
  export, restore, and future multi-User boundaries in [PostgreSQL Project
  Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md);
- the immutable Application Wire Record, replayable Project Activity,
  Snapshot, OutcomeUnknown, outbox/lease/fence, query-redaction, and
  compatibility boundaries in [Versioned Command, Query, Artifact, and Event
  Protocol](versioned-command-query-artifact-event-protocol.md);
- the minimum-necessary disclosure, log/telemetry, external-provider, and
  cross-Project threat boundaries in [StoryOS Service, Client, and External
  Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md);
- the foundation monorepo, contracts/generated, process-separable
  Server/Worker, prototype-evidence, and reference-isolation boundaries in
  [Modular-Monolith and Repository Governance Boundaries](modular-monolith-and-repository-governance-boundaries.md); and
- the user-confirmed product decisions that Eval is a standalone
  author-facing observation surface; cases and corpora are explicit; feedback
  is case-scoped; reproducibility means reopening evidence rather than
  deterministic external rerun; baselines are explicit; and external
  assessments require a separate explicit operation.

No new ADR is required. This specification applies existing accepted
architecture to one bounded product surface. A future decision that changes
the authority boundary, Project Scope isolation, or external-disclosure model
would require its own ADR rather than silently changing Eval.
