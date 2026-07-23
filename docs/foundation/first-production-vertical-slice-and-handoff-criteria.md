# First Production Vertical Slice and Handoff Criteria

- Status: accepted
- Wayfinder resolution: [Lock the First Production Vertical Slice and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Workspace boundary: [Fixed Three-Column Writing Workspace](../design/storyos-three-column-writing-workspace.md)
- Verification boundary: [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md)
- Repository boundary: [Modular-Monolith and Repository Governance Boundaries](modular-monolith-and-repository-governance-boundaries.md)

## 1. Purpose and authority

This specification selects the smallest real, end-to-end StoryOS production
slice and the evidence required before implementation planning begins. It
composes accepted author-authority, manuscript/Proposal, AgentRun, Context
Assembly, Model Gateway, protocol, PostgreSQL, recovery, retention, trust, and
verification contracts; it does not reopen their semantics.

The selected slice proves author value, not the breadth of infrastructure. Its
first successful path is a real author advancing one current passage with the
adjacent Agent, reviewing a bounded editable Proposal, and retaining the final
creative decision. A Provider response, browser state, worker process, cache,
or external API is never the authoritative creative state.

This decision does not implement Rust, a Web Client, Tiptap, PostgreSQL
migrations, a Provider Adapter, a deterministic fake, CI, deployment, or a
real Provider call. Those are the next planning and implementation work under
the acceptance gate in this document.

## 2. Confirmed author-visible value loop

The first production loop is the **First Production Writing Loop** defined in
[CONTEXT.md](../../CONTEXT.md). It begins with a real existing Project and one
selected current passage; project provisioning is a controlled prerequisite,
not an author-facing success criterion for this loop.

### 2.1 Entry and journey

The author opens the approved fixed three-column writing workspace for one
exact Project Scope. The center column is the authoritative manuscript editor
and Proposal review surface; the right column is the ordinary project Agent
conversation; the left column remains fixed project and manuscript navigation.
The slice does not add a Run bar, an App catalog, an Eval workspace, or another
creative control surface.

One successful loop has this exact shape:

1. The author may directly write or revise the selected current passage.
2. The author sends a bounded natural-language request about that passage, for
   example: “让这段雨夜更压迫，但不要替我规划下一章。”
3. The adjacent Agent works from the current passage and accepted bounded
   context, then produces an editable Proposal anchored to the current
   manuscript Revision. A transcript explanation may accompany it, but is not
   an authoritative change and is not a substitute for the Proposal.
4. The author reviews and may edit the Proposal in the editor, then explicitly
   accepts or rejects it. Acceptance creates the resulting Authoritative
   Revision and Acceptance Receipt atomically; rejection is non-destructive.

The loop is incomplete when the Agent merely replies in chat, auto-replaces
prose, produces an unreviewable draft, or turns the request into a story
outline. Author-provided outlines remain Data-only Context when explicitly
eligible; no Agent-authored outline, Author Plan, or preplanned story structure
enters this path.

### 2.2 Product defaults

The ordinary author has no required role table, PIN, Provider choice, routing
policy, retention profile, Skill installation, context Pin, or Eval setup. A
controlled deployment/bootstrap operation establishes the already-required
Project Destination Grant and non-secret Credential Reference for the Project;
those facts remain inspectable, but their controls are not a prerequisite on
the ordinary writing path. That operation does not create a global current
User, Project, or Provider assumption.

The Foundation Validation Deployment runs StoryOS Server and PostgreSQL locally
for one bootstrapped User. Every persisted fact and operation still binds the
exact `{ owner_user_id, project_id }` Project Scope so the same architecture
can serve many mutually isolated Users later. Models and embeddings remain
external APIs; Bailian may be a current test Provider but is not a selected
kernel dependency, route, or author-facing setting.

## 3. Selected coherent contract closure

The first slice contains only the following closure. Every selected boundary
is production-shaped; no in-memory, test-adapter, cache, session-continuity,
or local-model shortcut may substitute for it.

| Required capability | Selected scope | Explicit non-claim |
| --- | --- | --- |
| Trusted author entry and Project isolation | A trusted Client Session Binding, exact Project Scope on every request and record, server-derived authority, PostgreSQL composite-scope constraints, and forced RLS under a non-owner runtime role | Account-management UX, teams, ownership transfer, shared projects, or multi-author editing |
| Fixed writing workspace | One existing Project's approved three-column workspace, including the editor, normal Agent transcript, selected working target, and the established composer pause behavior | A new visual direction, app catalog, settings journey, full project-management product, or Eval navigation |
| Authoritative manuscript and Proposal review | Direct Author Edit, current manuscript Revisions/Heads, editable anchored Proposal content, validation, Acceptance, rejection, Receipts, and exact conflict/refusal behavior | Bulk/cross-location edits, document-wide transformations, automatic merge/rebase, or accepting a chat message/draft directly |
| Ordinary Agent execution | One root, author-started durable AgentRun with its RunStep, Run Lane, bounded author steering, pause, finalization, and recovery evidence | Subruns, Mailboxes, proactive triggers, background Agents, permanent agents, or a separate task-specific runtime |
| Current-passage context and disclosure | Working Target, exact author request, applicable local manuscript structure, an optional existing Project Instruction, all seven Context Assembly gates, destination manifests, and author-inspectable non-secret evidence | Default whole-manuscript injection, automatic outline use, retrieval ranking as authority, Memory recall, research, or ambient context |
| External model operation | One provider-neutral Model Gateway path through an exact Processing Destination Identity, Project Destination Grant, Project Model Use Binding, External Contract Compatibility Decision, route decision, fenced Model Attempt, non-secret wire projection, and external API dispatch | Local-model inference, Provider-hosted Tools, hidden Provider/SDK retry or fallback, Provider session/cache continuity, broad Provider catalog, or price/quality routing |
| Durable recovery and activity | Atomic commits, Receipt/event/outbox intent, Worker lease/fence, scoped canonical queries, replayable Project Activity SSE, recovery from durable facts, and honest `OutcomeUnknown` after possible dispatch | Browser/process/SDK state as truth, blind resubmission, a private Worker store, a broker, or whole-system Event Sourcing |
| Lifecycle safety evidence | The selected prose, Proposal, transcript, context, manifest, Attempt, and Receipt payloads obey the retention/redaction boundary, including a tested visible availability gap and non-revival after redaction | A retention-profile UI, general archive browser, Project import/export, deletion UX, or a claim that all retention features ship |

Tool, MCP, and Skill execution are absent from this author path. The selected
Run has no Tool Exposure, MCP server or MCP App action route, or Skill
Requirement, and its Model Capability Profile has no Tool-request mode. A
model response that attempts one is rejected before it can create a ToolCall,
destination operation, effect, or authority change. This is an explicit
fail-closed absence, not a second execution path. A later Tool, MCP, Skill,
App, research, or embedding operation must enter through its own accepted
contracts and all seven Context Assembly gates.

Embedding is also absent from the selected loop. No vector index, retrieval
cache, semantic search, or embedding call is required merely because the Model
Gateway and Context contracts support them.

## 4. Minimum durable facts and boundaries

The implementation plan must preserve these facts as distinct records or
canonical relations. Physical table layout remains owned by the PostgreSQL
contract, but combining, omitting, or replacing any row below with browser,
cache, Provider, or process state is not permitted.

| Fact family | Minimum durable fact for the selected loop |
| --- | --- |
| Identity and scope | User, Project, Project Scope, Client Session Binding admission evidence, command identity/digest/idempotency, and non-oracular refusal outcome |
| Authoritative writing | Manuscript identity, current passage/block identity, expected and resulting Authoritative Revisions/Heads, Authoritative Commit when authority changes, and Domain/Acceptance Receipts |
| Author conversation and Proposal | Exact author Message, Workspace Context, Working Target, Proposal identity/Revision/Anchor/state axes, Validation Receipt, selected Operations, Acceptance or Rejection fact, and safe conflict/refusal record |
| Run and recovery | AgentRun, RunStep/Step Snapshot, ordered Run events, author steering/pause when used, lease/fence generation, outbox/wakeup intent, terminal outcome, and recovery decision where needed |
| Context and egress | Operation Requirement, exact Context Source Versions, eligibility/selection/projection evidence, Context Assembly Manifest, Processing Destination Identity, Project Destination Grant, Project Model Use Binding, External Contract Compatibility Decision, Wire Payload Projection, Destination/Model Attempt, dispatch claim, Outbound Disclosure Event, and settlement certainty |
| Model result | Route Request/Decision, registration/capability evidence, immutable Attempt request and stream/result evidence, host validation disposition, and the resulting Proposal provenance rather than a direct manuscript write |
| Public activity and lifecycle | Application Wire Record where the protocol requires it, scoped Project Activity event/cursor/Snapshot facts, redaction/tombstone/availability-gap fact, and recovery-visibility evidence for restore |

The Web Client consumes generated public client/types only. It cannot attest
scope, author intent, acceptance, context eligibility, delivery certainty, or
recovery. Server admits public commands and queries; Worker claims only durable
intent with a current fence; PostgreSQL remains the sole canonical authority;
the external model Adapter receives only the admitted, minimum-necessary wire
projection with an ephemeral resolved credential.

## 5. External-processing and recovery rules

Every author-requested model operation follows the complete ordered Context
Assembly contract:

1. establish the exact Purpose, requester, Project Scope, Workspace Context,
   Working Target, and Operation Input Snapshot;
2. discover only permitted source candidates;
3. apply eligibility before ranking;
4. select only the bounded context needed for the current-passage request;
5. create destination-specific bounded projections;
6. commit the immutable Context Assembly Manifest; and
7. create and admit the destination Attempt before dispatch.

At durable dispatch claim, StoryOS records the non-secret wire projection and
an Outbound Disclosure Event before external I/O. A crash, disconnect, timeout,
or Worker loss after that claim remains `OutcomeUnknown` until ordinary,
fenced reconciliation records new evidence. It is never silently retried,
treated as zero usage, converted into success or failure, or replaced by a
Provider cache/session assertion.

The selected slice starts with no Provider cache, prior-response/session
continuity, cross-Project batching, embedding index, Tool/MCP server, or test
Adapter escape hatch. A deterministic fake may be used only inside the
verification harness and must traverse the same Host admission, manifest,
Attempt, and recovery path as the real external Adapter. A real author
acceptance smoke uses one separately admitted external API operation and is
not evidence of Provider internals, data retention, literary quality, or a
deterministic test pass.

## 6. Explicit exclusions and retained owners

| Excluded from the first production loop | Retained boundary or owner |
| --- | --- |
| Project-creation and project-settings UX | A real scoped Project must be provisioned through an explicit controlled path; broader project management remains later product work |
| Account/login UX, billing, teams, collaboration, ownership transfer, and multi-author editing | The existing User/Project Scope and future controlled-cloud contracts |
| Full manuscript planning, Agent-authored outline, Author Plan, character-sheet setup, and required instruction setup | Discovery Writing and the Data-only Context boundary |
| Tool execution, MCP servers, MCP Apps, Skills, research fetching, arbitrary filesystem/shell execution, and external writes | Tool/MCP/Skill and trust contracts; no selected Tool/MCP route may exist in this slice |
| Memory extraction/admission, research provenance, retrieval indexes, embeddings, semantic search, or cache continuity | Memory/Research and Context contracts; they remain eligible for a later independently bounded path |
| Eval Cases, corpora, assessment, judges, metrics, baseline/comparison, feedback, and Eval UI | [Foundation Evidence for the Standalone Eval Surface](eval-evidence-foundation.md); opening Eval is not part of this slice |
| Subruns, Mailboxes, proactive triggers, agent delegation, and Run planning beyond the one needed current-passage step | Persistent Run and Subrun/Mailbox contracts |
| Broad Provider catalog, hidden fallback, provider-hosted Tools, local models, model quality tuning, or benchmark-based routing | Model Gateway contract; one exact external Adapter path is enough for this slice |
| Author-facing retention configuration, archive/export/import/deletion UX, or complete recovery administration | Retention/archival and PostgreSQL contracts; selected redaction/non-revival and physical restore proof remain required evidence |
| Performance targets, throughput/latency claims, token-cost targets, benchmark corpus, creative-quality score, or CI/deployment implementation | Existing Protocol limits and recovery commitments remain applicable; no new numerical claim is made here |

## 7. Deterministic evidence gate

The implementation may be handed to Superpowers planning only with a concrete
gate plan for the following selected portions of the accepted Deterministic
Verification Gate catalogue. Every selected proof uses the independent oracle,
contract-faithful fake destinations, virtual monotonic scheduler, named fault
points, multi-Scope adversarial world, and replayable non-secret evidence
bundles required by [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md).

| Gate | Required selected proof |
| --- | --- |
| `DVG-01` | Release-1 contracts crate, generated OpenAPI/JSON Schema/TypeScript client, selected command/query/SSE wire corpus, limit profile, and applicable closed-input/schema-drift fixtures match their source; unsupported surface is absent rather than represented by permissive placeholders. |
| `DVG-02` | Session/Origin/Host/nonce/idempotency substitutions, foreign User/Project/object/cursor/Snapshot references, stale scope settings, and direct forced-RLS runtime probes fail closed with no authority, disclosure, cache shortcut, or existence oracle. |
| `DVG-03` | Browser proof for one physical Chinese or English author input and Core proof for whole-command Author Edit classification; direct edit, editable Proposal, conflict/refusal, validation, acceptance, rejection, idempotent retry, and acknowledgement-loss recovery settle with exact Heads, Revisions, Receipts, and events. |
| `DVG-04` | The current author request, Working Target, bounded local manuscript context, optional Project Instruction, and resulting Proposal provenance cross all seven gates. Ineligible, stale, redacted, foreign, injected, cached, or widened sources cannot reach the Model Attempt. No retrieval, memory, or Tool result is silently invented to satisfy the operation. |
| `DVG-05` | A contract-faithful external-model fake and the real Adapter mapping both use the selected Processing Destination Identity, Project Destination Grant, Project Model Use Binding, External Contract Compatibility Decision, Credential Reference, manifests, Attempt admission, non-secret wire capture, and validated response boundary. Tool/MCP/App request modes are absent or refused before any Tool exposure or egress. |
| `DVG-06` | The root AgentRun/RunStep/Run Lane portion proves ordered author steering, pause, finalization, and terminal outcome from durable facts. No Subrun or Mailbox capability is claimed delivered. |
| `DVG-07` | Fault injection at selected Core commit, outbox, Worker claim, dispatch claim, lease expiry, stale settlement, and client-acknowledgement boundaries yields only oracle-permitted idempotent state and recovery decisions. |
| `DVG-08` | A post-claim crash/timeout/disconnect creates `OutcomeUnknown`, preserves conservative disclosure and budget evidence, fences late results, and refuses blind resend. A successor or reconciliation is a separately admitted operation. |
| `DVG-09` | Selected historical activity replay and a redaction/tombstone of selected loop payload demonstrate an explicit safe availability gap and non-revival through canonical query, editor/transcript projection, context selection, cache, replay, export-facing representation, or Provider continuity. |
| `DVG-10` | The existing physical PostgreSQL recovery contract is demonstrated at the selected slice scope: a verified restore starts isolated, preserves roles/RLS and exact scope facts, rebuilds the selected projection, and withholds ordinary visibility or execution until Recovery Visibility Proof is complete. Author-facing archive/import/deletion features are not claimed. |
| `DVG-11` | The selected hostile corpus covers forged client admission, scope/RLS, prompt/content injection, Provider disclosure widening, Credential/log/telemetry leakage, stale Worker, replay, bounded input, and denied Tool/MCP/App attempts. Each expected denial supplies Negative Evidence Closure over authority, context, egress, Attempt, budget, secret, and foreign-identity effects. |
| `DVG-13` | Foundation Contract Walks for current-passage author edit/Proposal acceptance and Agent request/external dispatch/unknown recovery are executed as synthetic witnesses. They supplement, and do not replace, this product's browser and real-author acceptance evidence. |

`DVG-12` is not selected because no Eval surface ships. The Subrun/Mailbox,
archive-import/deletion, Tool/MCP execution, research-fetch, embedding, and
full retention parts of other gate families remain unimplemented and must not
be described as passed by this slice.

Any selected gate that is failed, unrun, unreplayable, evidence-incomplete, or
`unverified` blocks the handoff. An expected refusal or
`expected_outcome_unknown` passes only with its required durable evidence; a
real Provider observation, retry-until-green outcome, benchmark, or manual
assertion cannot substitute for a deterministic gate.

## 8. User acceptance evidence

The first real-author acceptance session occurs on a local StoryOS Server and
PostgreSQL database using a separately admitted external model API. It must
show, without exposing credentials or raw secret-bearing wire data, that:

1. the author opens the one Project's fixed writing workspace and can directly
   revise the current passage;
2. the author sends a current-passage request and the Agent produces an
   editable, anchored Proposal rather than an automatic prose write or outline;
3. the author can modify, accept, and reject Proposals, with Acceptance alone
   changing Authoritative State and producing its inspectable Receipt;
4. the ordinary path requires no author Provider, role, policy, Skill, Pin,
   retention, or Eval configuration;
5. the run's safe inspection view distinguishes selected context, manifest
   preparation, dispatch certainty, and Provider opacity without claiming
   Provider-internal use; and
6. interruption and restart leave the run, proposal, disclosure, and
   authoritative facts recoverable and truthful, including a visible
   `OutcomeUnknown` when that is the only honest state.

This evidence establishes that a real author can use the loop for a novel. It
does not assert response quality, provider reliability, benchmark performance,
provider retention, or a release-scale capacity number.

## 9. Superpowers implementation-planning handoff

Superpowers planning receives this specification as a fixed scope boundary,
not permission to broaden it. The first plan must preserve all accepted
contracts and produce a small sequence of implementation stages, each with
red tests before the smallest corresponding implementation. It must neither
start a second product design nor treat a prototype or `.reference/**` source
as production code.

### 9.1 Initial ownership roots and process entries

The initial implementation plan may create only these selected ownership roots
from the accepted monorepo topology:

- `storyos-contracts`;
- `storyos-core`;
- `storyos-agent-kernel`;
- `storyos-context`, limited to selected current-passage Context Assembly and
  disclosure;
- `storyos-model-gateway`, limited to the selected external model path;
- `storyos-transcript`, limited to ordinary transcript Messages and no MCP App
  host behavior;
- `storyos-application`;
- `storyos-adapter-postgres` and one provider-neutral
  `storyos-adapter-model-api`;
- `storyos-server` and `storyos-worker` as independently startable binaries;
- `apps/web`, plus checked-in `generated/` artifacts and
  `packages/storyos-client`.

`storyos-tooling` and `storyos-eval` do not enter the first workspace. Core,
the Agent Kernel, Context, and Model Gateway remain free of HTTP, PostgreSQL,
Provider SDK, browser, generated-client, Tool/MCP, and prototype/reference
dependencies. Server and Worker may co-locate locally but remain
process-separable and share only canonical PostgreSQL truth.

### 9.2 First public surface

The release-1 public contract contains only these resource-specific routes and
their generated client/types:

| Surface | Selected public operation |
| --- | --- |
| Web route | `/projects/:projectId/write` renders the fixed writing workspace and no Eval, App-catalog, or settings route is required for loop completion. |
| Workspace query | `GET /v1/projects/{project_id}/writing-workspace` returns a scoped canonical workspace projection and safe current activity reference. |
| Author edit | `POST /v1/projects/{project_id}/author-edits` accepts one normalized Author Edit; Core alone classifies the whole command as authoritative, Proposal-owned, refused, conflicted, or no effect. |
| Agent request | `POST /v1/projects/{project_id}/agent-runs` creates one bounded root AgentRun from the exact current passage and author Message. |
| Run control and inspection | `POST /v1/projects/{project_id}/agent-runs/{run_id}:pause` and `GET /v1/projects/{project_id}/agent-runs/{run_id}` expose only accepted run-control and safe inspectable facts. |
| Proposal settlement | `POST /v1/projects/{project_id}/proposals/{proposal_id}:accept` and `POST /v1/projects/{project_id}/proposals/{proposal_id}:reject` invoke the accepted Proposal state machine and never accept a Message or Draft. |
| Activity delivery | `GET /v1/projects/{project_id}/activity` is the one scoped replayable Project Activity SSE surface. |

The exact public DTOs, errors, idempotency headers, Session Binding inputs,
Snapshot/cursor representation, redaction projection, and generated files
remain owned by the Protocol and contracts crate. The paths above are the
complete first-slice inventory; adding a new public route requires a new
planning decision rather than an incidental implementation convenience.

### 9.3 Initial persistence and generated-contract package

The migration plan begins with these four named, ordered Foundation migrations:

1. `0001_scope_roles_and_sessions` establishes the `storyos` authority
   boundary, exact Project Scope relations, Client Session Binding support,
   non-owner runtime posture, and forced RLS checks.
2. `0002_manuscript_proposals_and_receipts` establishes selected Authoritative
   manuscript, Proposal, validation, Acceptance/rejection, immutable payload,
   Revision/Head, and Receipt facts.
3. `0003_runs_context_and_model_attempts` establishes root AgentRun, RunStep,
   transcript Message, Context Assembly, destination/use binding, Model
   Attempt, disclosure, lease/fence, and outbox facts.
4. `0004_activity_lifecycle_and_recovery` establishes Project Activity,
   canonical query/Snapshot support, selected redaction/tombstone facts, and
   recovery-visibility evidence.

Release 1 also checks in generated OpenAPI 3.1, JSON Schema 2020-12,
TypeScript client/types, schema catalog, Protocol Limit Profile, selected
positive and adversarial fixtures, and golden wire corpus. Rust source in
`storyos-contracts` is the only editable public-contract source; generated
output is never hand-edited or used as domain truth.

### 9.4 Planning exit gate

An implementation plan is ready to begin only when it names, for each selected
route and migration, its owning zone, Core command/query, transaction/outbox
boundary, exact Project Scope and RLS posture, context/destination boundary,
recovery/fence behavior, generated-contract change, deterministic gate,
real-author acceptance evidence, and explicit excluded capability. The plan
must also provide a non-mutating final `verify` entrypoint design and exact
checks that exclude `prototypes/**` and `.reference/**` from every production
workspace, dependency, build, test, package, release, and runtime path.

## 10. Map disposition

No additional Wayfinder ticket is created from this decision. The parent map's
remaining performance-budget and benchmark-corpus fog does not block the
Foundation Design Package or this handoff: accepted Protocol limit profiles and
the Recovery Service Profile continue to constrain safety, while no performance
or quality claim is required for the selected first loop. A future measurement
or benchmark question remains separate advisory planning rather than a hidden
acceptance gate.

With this specification, the Foundation Design Package has a selected first
production vertical slice, explicit exclusions, selected deterministic and
user-acceptance evidence, and a bounded implementation-planning handoff.

## 11. Accepted inputs

This specification consumes and does not redefine:

- [Artifact and Authoritative-State Domain Model](artifact-domain-model.md);
- [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md);
- [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md);
- [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md);
- [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md);
- [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md);
- [Foundation Evidence for the Standalone Eval Surface](eval-evidence-foundation.md);
- [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md);
- [Modular-Monolith and Repository Governance Boundaries](modular-monolith-and-repository-governance-boundaries.md);
- [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md); and
- [ADR 0012: Adopt Deterministic Contract Verification](../adr/0012-adopt-deterministic-contract-verification.md).

It creates no new ADR because it selects a bounded delivery closure inside
already accepted architectural and trust boundaries; it does not reverse a
hard premise or introduce a new cross-zone ownership model.
