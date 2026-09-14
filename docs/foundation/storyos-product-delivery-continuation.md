# StoryOS Product Delivery Continuation

- Status: current release contract; product implementation is not claimed.
- Contract revision: `release-responses-memory-2026-09-14-v1`.
- Release owner: [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).
- Entry contract: [AI-Independent Editor-First Release Baseline and Handoff Criteria](ai-independent-editor-first-release-baseline-and-handoff-criteria.md).
- Planning baseline: `main@38552dd3ecaf76dace80ff8d7f80caa381972ac0`, tree `29753b24816dcfd3b778309836531bc25f998ed1`.

This file defines the production destination for retained capabilities after
the first four stages. It also records the source-to-delivery coverage needed
by `REL-007`. It does not redefine domain semantics or choose proof mechanics.
The requirement and evidence tables are the release owner's structured inputs
to the proof owner. A reference to a semantic owner requires that owner's full
applicable contract, not only the short description in this file.

## 1. Entry and completion rules

Stages 1 through 4 retain their identities and order. Stage 3 already requires
the real Host, Context Assembly, durable AgentRun, and recovery foundations;
later stages do not postpone those requirements. Stage 4 proves one real
model path. It is not completion of research, Tools, Skills, Memory, MCP Apps,
or the full retained StoryOS product.

The [release allocation and conversation defaults](ai-independent-editor-first-release-baseline-and-handoff-criteria.md#21-conversation-memory-defaults-and-stage-admission)
apply from Stage 3. Active context compaction and conversation-bound Provider
continuation arrive in Stages 3 and 4. Stage 7 adds background Project Memory;
it does not postpone those earlier foundations or change their identities.

The serial release order is Stage 5 through Stage 9 below. Each stage starts
from the preceding released main. This order is a release gate, not a claim
that all preceding features are code dependencies. Child tickets name only
the actual contract, interface, data, or acceptance inputs they need.

Every stage must preserve the complete AI-disabled editor journey and the
existing author-authority, Scope, Admission, Proposal, privacy, recovery, and
non-revival contracts. Each new record or execution family must enter its
owning migration, archive, export, restore, retention, replay, and disclosure
checks. A Stage 2 restore result does not certify a later data family.

For each stage, all its requirements, journey steps, and mandatory evidence
must pass on one exact main before release. Unrun, stale, missing, or
unreplayable evidence blocks that stage. Future-stage evidence does not block
an earlier stage, and an earlier pass does not certify a future feature.
Fake destinations can prove controlled contracts; they cannot substitute for
an explicitly required real integration or author-visible production journey.

Existing scope exclusions remain: one author per Project, no mandatory
outline or character-sheet setup, no separate workflow runtime, no automatic
creative authority, and no reference or prototype runtime dependency. The
approved browser, language, deployment, and safety scope is unchanged.

## 2. Stage 5: Governed Tools, MCP servers, and research

| ID | Production obligation | Semantic owner |
| --- | --- | --- |
| S5-REQ-001 | The ordinary Agent conversation can perform bounded research through StoryOS ToolCalls and separately admitted Provider-hosted Operations. StoryOS dispatch uses the one Tool Gateway; hosted search, reading, and bounded temporary computation use ADR 0034 whole-operation admission. ToolSpec, Registration, Enablement, Exposure, Capability, and Approval stay distinct. | Tool/MCP owner and Context owner |
| S5-REQ-002 | A third-party MCP server is an untrusted registered integration. Bind its exact contract and Project use, reject incompatible drift, mediate each effect, and keep credentials outside project records and output. | Tool/MCP owner and trust owner |
| S5-REQ-003 | Preserve native function-call/result correlation and validate the complete model-requested business-tool batch before deriving StoryOS ToolCalls. Hosted items remain evidence of the separately admitted whole operation, not invented Host ToolCalls. Returned content passes Context Assembly before a later StoryOS-controlled submission; invisible Provider-internal steps do not claim Host gates. Results grant no instruction, permission, or authority. | Model Gateway and Context owners |
| S5-REQ-004 | The author can inspect a research synthesis, its claims, exact sources, and supporting, conflicting, or limiting evidence. A missing or unavailable source remains visible as a gap, not an invented citation. | Research/Memory owner |
| S5-REQ-005 | Tool and hosted-operation cancellation, interruption, unknown effects, and unknown usage stay durable and inspectable. Enforce registered intake, actual outward-processing bounds, and finite worst-case reservations before submission. Resume/reconciliation obey existing fences; a timeout proves neither remote stop nor retry permission. Incomplete, rejected, cancelled, or fenced hosted output cannot advance normal continuation. | AgentRun, Tool/MCP, and retention owners |
| S5-REQ-006 | Research, Tool, and MCP results can supply bounded assistance or an editable Proposal. They cannot directly change prose, fiction facts, preferences, or manuscript structure. | Artifact, Core/Proposal, and Admission owners |

**Author journey S5-JRN-001.** Ask a research question in the normal Agent
panel. Use a real StoryOS Tool/MCP path and a qualified real Provider-hosted
operation; inspect each operation's scope and any required Approval. Continue
with the exact native function/result correlation. Inspect returned sources,
reports, known gaps, and a validated partial result with its limitations. Use
a result as discussion or a Proposal and reject one creative change. Interrupt
an external operation, recover its exact disposition, then disable the
integrations and continue manual writing.

The hosted request admits its complete enabled Tool set before submission,
even when the Provider ultimately uses none. Existing grants cover unchanged
in-scope work without another prompt. Expanded authority uses the exact Tool
Approval target; disclosure Approval remains distinct. No Ambient Context,
unbounded remote cost, external business write, message, publication, or direct
StoryOS write enters the hosted scope. A separate StoryOS ToolCall retains its
own permitted effects and Approval. Unobserved internal steps stay unknown;
one physical submission is not charged or disclosed twice. If Agent Plan
cannot prove a required hosted boundary, Stage 5 remains blocked rather than
claiming it from a fake, a prompt, or general Ark documentation.

| ID | Mandatory evidence |
| --- | --- |
| S5-EVD-001 | Real Tool/MCP and exact hosted-mode qualification, registration, use, complete-set intake, bounds, grants, distinct Approval targets, and contract-drift evidence. Unauthorized or unbounded operations refuse before dispatch. |
| S5-EVD-002 | Native function/result correlation, complete batch validation, research claims and exact available evidence, truthful gaps/partial results, and re-entry at StoryOS-controlled submissions. Provider reports remain distinct from Host observations and opaque internal work. |
| S5-EVD-003 | Durable interruption, cancellation, unknown-effect/usage recovery, single accounting, selected-result continuation, Proposal-only creative change, and AI-independent regression evidence. |

## 3. Stage 6: Production Skill packages and composition

| ID | Production obligation | Semantic owner |
| --- | --- | --- |
| S6-REQ-001 | The author or Agent can select an eligible standard Skill without a task-mode menu. Resolve its source, installation scope, name conflicts, exact package snapshot, and explicit selection reason. Standard Skills work without mandatory StoryOS extensions. | Skill owner |
| S6-REQ-002 | Load instructions and resources progressively under the existing instruction precedence and Context rules. A package grants no permission, disclosure right, or authority. | Skill and Context owners |
| S6-REQ-003 | Resolve declared Tool roles, optional extensions, scripts, and outcomes through the existing Gateway, execution, and budget boundaries. Missing or conflicting prerequisites produce the declared blocked or degraded outcome. | Skill, Tool/MCP, and AgentRun owners |
| S6-REQ-004 | Inspect the effective Skill selection, composed instructions, conflicts, and outcome obligations. Creating, installing, updating, or revoking a package follows its explicit lifecycle and does not silently replace a snapshot bound to active work. | Skill owner |

**Author journey S6-JRN-001.** Select and run a standard instruction-only
Skill; run an eligible Skill with a Tool role or script; inspect the exact
package and outcome; exercise a conflict or missing prerequisite; revoke or
update a package while preserving prior Run evidence; continue writing when
the Skill is unavailable.

| ID | Mandatory evidence |
| --- | --- |
| S6-EVD-001 | Standard-package compatibility, exact selection/snapshot, progressive loading, composition, and instruction-authority evidence. |
| S6-EVD-002 | Tool-role/script admission, outcome obligations, failure, revocation, active-run binding, and editor-regression evidence. |

## 4. Stage 7: Structured project continuity and Memory

| ID | Production obligation | Semantic owner |
| --- | --- | --- |
| S7-REQ-001 | Preserve one Project main Agent across distinct Project Conversations using their existing identities. Cross-conversation continuity uses eligible Project sources and generated Memory; each conversation retains its own Provider continuation. Do not load the entire transcript or Memory collection by default. | Project Agent and Context owners |
| S7-REQ-002 | Represent author-owned world facts, characters, relationships, and timeline with the accepted fiction assertion, Story Scope, and Epistemic Scope semantics. Conflicting claims remain distinguishable; generated claims need explicit author-authorized settlement before authority. | Artifact and Research/Memory owners |
| S7-REQ-003 | Keep current feedback, explicit future-facing Author Preferences, and Inferred Preferences distinct. Inference is never an automatic lasting rule. The author can inspect and change explicit preferences through their owning commands. | Research/Memory and Admission owners |
| S7-REQ-004 | Support optional author-edited Project Instruction, immutable revisions, and exact top-level Run binding. Existing bindings stay fixed across Subruns and context compaction; absence does not block ordinary assistance. This does not defer any instruction binding already required in an earlier stage. | Context and AgentRun owners |
| S7-REQ-005 | Extract from bounded eligible idle conversation snapshots and consolidate generated Memory Documents through fenced background work. Publish one complete set of exact revisions and a bounded navigation summary atomically. Keep source references, model/prompt/input identities, pending Notes, and maintenance outcomes inspectable. Search/read tools use currently permitted publications; indexes rebuild from retained documents without model calls. Generation is separate and need not reproduce identical text. | Research/Memory, storage, trust, and retention owners |
| S7-REQ-006 | Apply the release's independent conversation Memory defaults and atomic idle-only settings contract. Offer author inspection of settings, publication, documents, Notes, sources, and maintenance outcomes even with Agent use disabled. Ordinary corrections remain Messages; an explicit lasting Memory request may record a Note for later consolidation. Note-recorded, published, no-op, failed, and unavailable are distinct. Retire semantic Include/Pin/Exclude and per-claim Admission/Suppression controls without aliases. | Context, Memory, and Protocol owners |
| S7-REQ-007 | Deliver Memory source restrictions, lifecycle, qualified generated-payload cleanup, archive/export/restore, and recovery under exact source/destination bindings. Preserve current publication/Artifact Head/author/active-work/shared-payload protections and identity/use/Decision/gap evidence. Real covered-copy restrictions fence stale work and retained copies; ordinary corrections do not reset continuation. Existing active compaction and Provider continuation stay separate. Independently enabled embedding keeps its own capability, permission, budget, and disclosure gates; basic Memory recall does not require embedding. | Context, Model Gateway, persistence, and retention owners |

**Author journey S7-JRN-001.**

1. Write and explicitly establish a fiction fact or Author Preference. Let an
   eligible settled conversation contribute to background extraction and
   consolidation, then inspect its complete publication and source references.
2. Continue in a second Project Conversation with the release defaults. Inspect
   bounded navigation and on-demand reads, distinguishing read/sent evidence
   from model attention and generated recall from author-owned truth.
3. Correct an inference through ordinary conversation. Explicitly request a
   lasting Memory change, inspect its recorded Note, and later inspect the
   separate maintenance outcome. Neither acknowledgement promises exact forgetting.
4. Change use and contribution independently while idle. Verify refusal while
   foreground work is busy, captured Run settings, disabled new reads or
   extraction, and continued author inspection. Inspect no-publication-yet,
   valid empty publication, and unavailable publication as distinct states.
5. Edit an optional Project Instruction and verify old/new Run bindings.
   Exercise a real source/copy restriction, its narrow or conservative set-wide
   unavailability, and reconstruction from allowed inputs without lifting old
   payload fences. Ordinary source edits do not rewrite conversation history.
6. Recover interrupted or stale maintenance without partial publication; rebuild
   the index without a model call. Exercise qualified old generated-payload
   cleanup and inspect retained identity, use records, and explicit gaps.
   Export and restore the allowed documents, pending work, and lifecycle evidence;
   missing evidence keeps recovery on Hold. Continue writing with Memory unavailable.

Stage 7 adds no semantic exclusion classifier, per-claim memory confirmation,
precise-forgetting oracle, or required project policy made from generated text.
Background maintenance does not block the editor and may change only its
admitted generated-document set. It cannot change Skills, ToolSpecs, policy,
credentials, Author Preferences, or Authoritative State. The current Memory
and retention contracts own publication concurrency and cleanup conditions;
this stage supplies their implementation and measured profile qualification.

| ID | Mandatory evidence |
| --- | --- |
| S7-EVD-001 | Cross-conversation identity, structured truth, explicit preference, instruction revision/binding, generated-recall limitations, and author-authority evidence. |
| S7-EVD-002 | Independent default/settings behavior and idle/admission race, eligible extraction, complete publication, stale/duplicate work refusal, bounded recall, author inspection, Note origin and distinct maintenance outcomes. Include AP-22 hostile-input, wrong-Scope, forbidden-write, secret, oversize, disabled, and unavailable-source cases; inspect records and effects, not semantic truth. |
| S7-EVD-003 | Context and independently enabled embedding/disclosure, exact publication reads, active-compaction separation, real covered-copy restrictions, allowed-input/index rebuild, and Memory recovery/portability. Cover cleanup racing publication or byte acquisition, protected/shared payloads, pending Notes, failed maintenance, missing lifecycle evidence, and truthful gaps with the editor still usable. |

## 5. Stage 8: Transcript MCP Apps

| ID | Production obligation | Semantic owner |
| --- | --- | --- |
| S8-REQ-001 | Render the accepted dynamic character, relationship, timeline, and research views inside the Agent transcript. StoryOS owns the data; prose Proposal editing remains in the central editor. | MCP App and Artifact owners |
| S8-REQ-002 | Bind immutable resources and App View revisions to disposable sandboxed Instances. Initialization, negotiation, policy, resource limits, eligibility revocation, and instance termination follow the existing Host contract. | MCP App and trust owners |
| S8-REQ-003 | Recover or replay from exact stored resources and typed records without re-executing the original ToolCall. Prepared Receipts and terminal static fallback remain available when interactive rendering is unsafe or unavailable. | MCP App and retention owners |
| S8-REQ-004 | Persist and route each semantic App action through a fresh applicable Host admission boundary. An App cannot reuse the originating Run's authority, directly call a Tool, or bypass Proposal and Acceptance. Response delivery remains scoped to the requesting Instance. | MCP App, Tool/MCP, and Admission owners |

**Author journey S8-JRN-001.** Open each accepted domain-view family from the
normal conversation; inspect its exact source; make an allowed mediated
request; reject an unsafe action; close and reopen the transcript; exercise
resource revocation and unavailable rendering; inspect the safe fallback;
confirm that no Tool effect is repeated and the editor is unchanged.

| ID | Mandatory evidence |
| --- | --- |
| S8-EVD-001 | Production domain views, sandbox and immutable-resource binding, instance lifecycle, negotiation, limits, and revocation evidence. |
| S8-EVD-002 | Prepared/terminal fallback, safe replay without execution, persisted App action routing, instance-scoped delivery, and editor-regression evidence. |

## 6. Stage 9: Complete Run control and orchestration

| ID | Production obligation | Semantic owner |
| --- | --- | --- |
| S9-REQ-001 | Support optional RunPlans for nontrivial work, with RunPlan Revisions and PlanSteps as intended work items. Support bounded long work through durable RunSteps, leases, holds, waits, steering, cancellation, and finalization. Keep the layered Run timeline inspectable without displacing normal conversation. | AgentRun owner |
| S9-REQ-002 | Create hierarchical Subruns with explicit parent, narrowed context/capabilities, budget reservation, independent execution records, and typed results. A Subrun cannot expand its parent's authority or become an orphan runtime. | Subrun and AgentRun owners |
| S9-REQ-003 | Deliver mailbox messages, follow-up tasks, interrupts, Required or Advisory joins, backpressure, seals, and late results through their exact durable semantics. Recovery preserves parent/child disposition and does not infer success from process exit. | Subrun and retention owners |
| S9-REQ-004 | Run only explicitly enabled proactive event or schedule work within its recorded scope, grants, budget, and misfire rules. Repeated or late triggers do not create duplicate effects or broaden an author instruction. | AgentRun owner |
| S9-REQ-005 | Enforce resource and safety holds, budgets, usage classification, and exact settlement across model, Tool, Skill, and Subrun work. Do not replace the accepted guardrail contract with generic retry or escalation. | AgentRun, Tool/MCP, Skill, and Model Gateway owners |
| S9-REQ-006 | Honor the configured model policy without changing Project Agent identity. Expose supported route or fallback decisions and exact Attempts; an unconfigured destination stays unavailable. No extra Provider purchase, hidden fallback, or credential authority is inferred. | Model Gateway owner |

**Author journey S9-JRN-001.** Start bounded work that requires a Subrun;
inspect the plan and timeline; send steering; interrupt or cancel a child;
recover the parent/child result; exercise a budget or safety hold; enable one
bounded proactive trigger and inspect duplicate/misfire handling; inspect an
explicit model-policy decision; verify the editor remains independently usable.

| ID | Mandatory evidence |
| --- | --- |
| S9-EVD-001 | RunPlan, RunPlan Revision, and PlanStep planning evidence; RunStep, lease, wait/hold, steering, interruption, finalization, and layered-observability evidence. |
| S9-EVD-002 | Subrun scope, reservation, mailbox, join, seal, late-result, cancellation, and durable recovery evidence. |
| S9-EVD-003 | Authorized proactive triggers, misfire/deduplication, multidimensional guardrails, explicit model routing, and editor-regression evidence. |

## 7. Deferred scope outside MVP

[Eval](eval-evidence-foundation.md) is a future observation surface outside
MVP. It has no current stage, Requirement, author journey, evidence bundle,
implementation ticket, or release dependency. Its page, APIs, and behavior are
not being designed or implemented. Stage 9 completes this local MVP route.

## 8. Canonical semantic owners

| Surface | Owner and required source |
| --- | --- |
| Workspace and browser continuity | [Specify Web Editor Session, Local Journal, Projection, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/70): [Web contract](web-editor-session-synchronization-and-recovery-semantics.md) and [approved workspace](../design/storyos-three-column-writing-workspace.md). |
| Core/Proposal | [Specify the Manuscript Revision and Proposal State Machine](https://github.com/FrankQDWang/StoryOS/issues/46): [state machine](manuscript-revision-proposal-state-machine.md), ADR 0003, and the approved workspace. |
| Artifact and authority | [Define the Authoritative-State and Artifact Domain Vocabulary](https://github.com/FrankQDWang/StoryOS/issues/44): [domain model](artifact-domain-model.md), [CONTEXT](../../CONTEXT.md), and ADR 0001. |
| Admission | [Specify Author Command Admission](https://github.com/FrankQDWang/StoryOS/issues/68): [admission](author-command-admission.md) and ADR 0013. |
| Project Agent and natural-language intent | [Define Plain-Language Discovery-Writing Assistance Semantics](https://github.com/FrankQDWang/StoryOS/issues/75): [discovery writing](plain-language-discovery-writing-assistance-semantics.md), with the retained Project Agent decisions. |
| AgentRun | [Specify Persistent Agent Run and Orchestration Semantics](https://github.com/FrankQDWang/StoryOS/issues/47): [CONTEXT](../../CONTEXT.md), protocol, retention, and [experimental tuning register](../../EXPERIMENTAL-TUNING-REGISTER.md). |
| Subrun | [Specify Subrun Control-Plane, Mailbox, and Observability Semantics](https://github.com/FrankQDWang/StoryOS/issues/63): [CONTEXT](../../CONTEXT.md) and [Mailbox/retention](run-event-mailbox-snapshot-retention-and-archival-semantics.md). |
| Tool/MCP | [Specify ToolSpec, Capability, Approval, and MCP Trust Semantics](https://github.com/FrankQDWang/StoryOS/issues/48): [CONTEXT](../../CONTEXT.md), Context Assembly, protocol, and threat model. |
| Skill | [Specify SkillPackage and Task-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/49): [CONTEXT](../../CONTEXT.md), Context Assembly, and protocol. |
| Research/Memory | [Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51): [fiction, Memory, and research](fiction-memory-and-research-provenance-semantics.md). |
| Context | [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54): [Context contract](context-assembly-retrieval-and-outbound-disclosure-semantics.md), [CONTEXT](../../CONTEXT.md), and ADR 0005. |
| Model Gateway | [Specify ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50): [CONTEXT](../../CONTEXT.md), protocol, and Context Assembly. |
| MCP App | [Specify Transcript and MCP App Lifecycle Semantics](https://github.com/FrankQDWang/StoryOS/issues/53): [ADR 0002](../adr/0002-specify-transcript-and-mcp-app-lifecycle-semantics.md), [CONTEXT](../../CONTEXT.md), and Artifact model. |
| Deferred Eval scope | [Record the Deferred Eval Observation Boundary](https://github.com/FrankQDWang/StoryOS/issues/61): [future scope only](eval-evidence-foundation.md); no MVP delivery obligation. |
| Persistence and retention | [Specify the PostgreSQL Project Storage, Isolation, and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56) and [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64): their [storage](postgresql-project-storage-isolation-and-migration-contract.md) and [retention](run-event-mailbox-snapshot-retention-and-archival-semantics.md) contracts, with ADR 0004 and ADRs 0008–0011. |
| Trust, protocol, and architecture | [Threat-Model the StoryOS Service, Client, and External Trust Boundaries](https://github.com/FrankQDWang/StoryOS/issues/57), [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58), and [Define the Modular-Monolith and Repository Governance Boundaries](https://github.com/FrankQDWang/StoryOS/issues/59): their [threat](storyos-service-client-external-trust-boundaries-threat-model.md), [protocol](versioned-command-query-artifact-event-protocol.md), and [governance](modular-monolith-and-repository-governance-boundaries.md) contracts. |

## 9. Retained-source coverage

The following source inventory accounts for every original Wayfinder design
child. A source can constrain more than one stage. The named production
requirements consume the exact current semantic owner; an old short Issue
description does not override a later accepted ADR or canonical definition.
Rows marked evidence remain evidence, not independently delivered features.
Rows marked deferred remain outside MVP and need no MVP Requirement anchor.
Recording a source does not approve its former design or add delivery scope.
Requirement anchors identify the minimum release path for each source; they
do not narrow its full applicable semantic contract. Each anchored stage must
pass all of its journey and evidence obligations. Cross-stage invariants are
rechecked whenever a new record family, caller, or destination enters. The
proof owner maps these obligations to exact executable evidence.

| Original source | Current disposition and delivery coverage | Requirement anchors |
| --- | --- | --- |
| [Adopt a Domain-General Agent Loop](https://github.com/FrankQDWang/StoryOS/issues/2) | Project Agent; S3, S4, S5, S6, S9. | `S3-REQ-001`; `S9-REQ-001` |
| [Set Autonomy and Human Approval Boundaries](https://github.com/FrankQDWang/StoryOS/issues/3) | AgentRun/Tool admission invariant; S3 onward. | `S3-REQ-003`; `S5-REQ-001`; `S9-REQ-005` |
| [Bound the General Agent to the Novel Project Domain](https://github.com/FrankQDWang/StoryOS/issues/4) | Project Agent and Scope invariant; S3 onward. | `S3-REQ-001`; `REL-003` |
| [Give Each Project One Persistent Main Agent](https://github.com/FrankQDWang/StoryOS/issues/5) | Project Agent identity; S3 foundation and S7 continuity. | `S3-REQ-001`; `S7-REQ-001` |
| [Use Natural Language as the Default Task Entry](https://github.com/FrankQDWang/StoryOS/issues/6) | Discovery-writing entry; S3 onward. | `S3-REQ-002` |
| [Protect All Authoritative Creative State](https://github.com/FrankQDWang/StoryOS/issues/7) | Authority invariant in all stages; structured truth in S7. | `REL-002`; `S7-REQ-002` |
| [Support Controlled Proactive Runs](https://github.com/FrankQDWang/StoryOS/issues/8) | AgentRun; S9. | `S9-REQ-004` |
| [Expose a Layered Interruptible Run Timeline](https://github.com/FrankQDWang/StoryOS/issues/9) | AgentRun; S3 foundation, S5 activity, S9 complete control. | `S3-REQ-003`; `S5-REQ-005`; `S9-REQ-001` |
| [Use Adaptive Plan-and-Act Execution](https://github.com/FrankQDWang/StoryOS/issues/10) | AgentRun; S3 foundation and S9 long work. | `S3-REQ-003`; `S9-REQ-001` |
| [Enforce Multidimensional Run Guardrails](https://github.com/FrankQDWang/StoryOS/issues/11) | Exact AgentRun guardrails as each execution family enters; S3–S9. | `S3-REQ-003`; `S9-REQ-005` |
| [Unify Capabilities in a Tool Registry](https://github.com/FrankQDWang/StoryOS/issues/12) | Tool/MCP; S5. Static ToolSpec and dynamic authority remain distinct. | `S5-REQ-001` |
| [Separate Composable Read Tools from Domain Write Commands](https://github.com/FrankQDWang/StoryOS/issues/13) | Tool/MCP and Core boundaries; S5 onward. | `S5-REQ-001`; `S5-REQ-006` |
| [Support Hierarchical Ephemeral Subruns](https://github.com/FrankQDWang/StoryOS/issues/14) | Durable Subrun contract; S9. Ephemeral scope is not disposable execution history. | `S9-REQ-002`; `S9-REQ-003` |
| [Combine Mandatory Context with Dynamic Retrieval](https://github.com/FrankQDWang/StoryOS/issues/15) | Context; S3/S4 foundations and S7 complete retrieval. | `S3-REQ-003`; `S7-REQ-006` |
| [Forbid Hidden Persistent Agent Memory](https://github.com/FrankQDWang/StoryOS/issues/16) | Memory authority invariant; S7. | `S7-REQ-001`; `S7-REQ-005` |
| [Standardize Outputs as Typed Artifacts](https://github.com/FrankQDWang/StoryOS/issues/17) | Artifact invariant in every applicable stage. | `REL-002`; `S5-REQ-006`; `S8-REQ-001` |
| [Persist Every Agent Run by Default](https://github.com/FrankQDWang/StoryOS/issues/18) | AgentRun durability from S3; each later execution family. | `S3-REQ-003`; `S9-REQ-001` |
| [Decouple Agent Identity from Model Choice](https://github.com/FrankQDWang/StoryOS/issues/19) | Model Gateway; S3/S4 and explicit S9 policy behavior. | `S3-REQ-003`; `S4-REQ-001`; `S9-REQ-006` |
| [Keep Local Authority and Minimize External Disclosure](https://github.com/FrankQDWang/StoryOS/issues/20) | Scope/Context invariant; S1 authority and every later destination. | `REL-002`; `REL-003`; `S4-REQ-003` |
| [Package Skills as Versioned Contracts](https://github.com/FrankQDWang/StoryOS/issues/21) | Skill; S6. Standard packages do not require optional extensions. | `S6-REQ-001`; `S6-REQ-004` |
| [Pin Codex as a Shallow Reference Submodule](https://github.com/FrankQDWang/StoryOS/issues/22) | Historical reference evidence; current AGENTS reference policy governs all stages. | `REL-003`; `REL-007` |
| [Build an Independent StoryOS Agent Kernel](https://github.com/FrankQDWang/StoryOS/issues/23) | Architecture invariant; no upstream runtime dependency. | `REL-003`; `S3-REQ-001` |
| [Use HTTP Commands with Replayable SSE Events](https://github.com/FrankQDWang/StoryOS/issues/24) | Protocol invariant; S1 onward. | `REL-003`; `S2-REQ-005` |
| [Own External Contracts in a Rust Contracts Crate](https://github.com/FrankQDWang/StoryOS/issues/25) | Protocol/generated-source ownership in all stages. | `REL-003`; `REL-007` |
| [Bind Every Novel to One Durable PostgreSQL Project Scope](https://github.com/FrankQDWang/StoryOS/issues/26) | Persistence/Scope invariant; S1 onward. | `REL-003`; `S2-REQ-002` |
| [Treat Third-Party MCP Servers as Untrusted](https://github.com/FrankQDWang/StoryOS/issues/27) | Tool/MCP and trust; S5 onward. | `S5-REQ-002` |
| [Render Dynamic Domain UI as Transcript MCP Apps](https://github.com/FrankQDWang/StoryOS/issues/28) | MCP App placement and domain views; S8. | `S8-REQ-001` |
| [Keep Domain Data in StoryOS, Not MCP Apps](https://github.com/FrankQDWang/StoryOS/issues/29) | Artifact/MCP App authority; S8. | `REL-002`; `S8-REQ-001` |
| [Move Evaluation to a Dedicated Page](https://github.com/FrankQDWang/StoryOS/issues/30) | Deferred outside MVP; the Eval boundary owner retains only the future observation concept. | None; outside MVP. |
| [Keep Proposal Review and Editing Inside the Editor](https://github.com/FrankQDWang/StoryOS/issues/31) | Core/Proposal and workspace; S3 onward. | `S3-REQ-004` |
| [Use an Editable In-Context Proposal](https://github.com/FrankQDWang/StoryOS/issues/32) | Core/Proposal; S3 onward. | `S3-REQ-004` |
| [Derive Optional Comparison from Exact Proposal Revisions](https://github.com/FrankQDWang/StoryOS/issues/33) | Core-derived comparison semantics; S3. No default diff UI is added. | `S3-REQ-008` |
| [Normalize Derived Replacement Spans](https://github.com/FrankQDWang/StoryOS/issues/34) | Core-derived comparison/diagnostic span normalization, with stable Operation identity and exact versions; S3. | `S3-REQ-008` |
| [Accept or Reject by Stable Top-Level Blocks](https://github.com/FrankQDWang/StoryOS/issues/35) | Core stable Operations; S2 Block foundation, S3 Acceptance/Rejection. | `S2-REQ-004`; `S2-REQ-009`; `S3-REQ-008` |
| [Allow Multiple Non-Overlapping Proposals per Chapter](https://github.com/FrankQDWang/StoryOS/issues/36) | Core reservations and non-overlap; S3. | `S3-REQ-008` |
| [Make Undo Accept Reopen the Proposal](https://github.com/FrankQDWang/StoryOS/issues/37) | Exact safe compensation and Proposal restoration; S3. | `S3-REQ-008` |
| [Pause Streaming When the Author Starts Editing](https://github.com/FrankQDWang/StoryOS/issues/38) | Input fence and Proposal stream semantics; S3. | `S3-REQ-008` |
| [Split Selections into Inline and Block Proposals](https://github.com/FrankQDWang/StoryOS/issues/39) | Core/Proposal anchors and operations; S3. | `S3-REQ-008` |
| [Use GitHub Issues as the Wayfinder Tracker](https://github.com/FrankQDWang/StoryOS/issues/40) | Current issue-tracker rules in all stages. | `REL-006`; `HND-002` |
| [Extract Adaptable Agent-Kernel Patterns from the Pinned Codex Source](https://github.com/FrankQDWang/StoryOS/issues/41) | Retained research evidence; independent architecture in all stages. | `REL-003`; `REL-007` |
| [Establish Durable Proposal Mechanics in Tiptap and ProseMirror](https://github.com/FrankQDWang/StoryOS/issues/42) | Retained research; S2 production editor adoption and S3 Proposal proof. | `S2-REQ-009`; `S3-REQ-008` |
| [Establish StoryOS MCP Apps Host Obligations](https://github.com/FrankQDWang/StoryOS/issues/43) | Retained research; MCP App owner and S8 production proof. | `S8-REQ-002`; `S8-REQ-004` |
| [Define the Authoritative-State and Artifact Domain Vocabulary](https://github.com/FrankQDWang/StoryOS/issues/44) | Canonical Artifact/authority owner; all applicable stages. | `REL-002`; `S7-REQ-002` |
| [Validate Production Proposal Refusal, Conflict, and Recovery UX](https://github.com/FrankQDWang/StoryOS/issues/45) | Retained prototype evidence; S2 workspace and S3 production recovery. | `S2-REQ-009`; `S3-REQ-004`; `S3-REQ-008` |
| [Specify the Manuscript Revision and Proposal State Machine](https://github.com/FrankQDWang/StoryOS/issues/46) | Canonical Core/Proposal owner; S1–S4 and later creative changes. | `S2-REQ-004`; `S3-REQ-004`; `S3-REQ-008` |
| [Specify Persistent Agent Run and Orchestration Semantics](https://github.com/FrankQDWang/StoryOS/issues/47) | Canonical AgentRun owner; S3 foundations and S9 complete modes. | `S3-REQ-003`; `S9-REQ-001`; `S9-REQ-005` |
| [Specify ToolSpec, Capability, Approval, and MCP Trust Semantics](https://github.com/FrankQDWang/StoryOS/issues/48) | Canonical Tool/MCP and ADR 0034 hosted-operation owner; S5 and later callers. | `S5-REQ-001`; `S5-REQ-002`; `S5-REQ-003`; `S5-REQ-005` |
| [Specify SkillPackage and Task-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/49) | Canonical Skill owner; S6. | `S6-REQ-001`; `S6-REQ-002`; `S6-REQ-003`; `S6-REQ-004` |
| [Specify ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50) | Canonical Model Gateway and ADR 0033 owner; S3 fake continuation/compaction, S4 Agent Plan, S5 functions/hosted work, S9 policy. | `S3-REQ-003`; `S3-REQ-006`; `S4-REQ-001`; `S4-REQ-004`; `S5-REQ-003`; `S9-REQ-006` |
| [Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51) | Canonical Memory and ADR 0035 owner; S3 conversation defaults, S5 research evidence, S7 background Memory. | `S3-REQ-003`; `S5-REQ-004`; `S7-REQ-002`; `S7-REQ-003`; `S7-REQ-005`; `S7-REQ-006` |
| [Validate a Transcript-Embedded MCP Apps Host](https://github.com/FrankQDWang/StoryOS/issues/52) | Retained prototype evidence; S8 production Host. | `S8-REQ-002`; `S8-REQ-003`; `S8-REQ-004` |
| [Specify Transcript and MCP App Lifecycle Semantics](https://github.com/FrankQDWang/StoryOS/issues/53) | Canonical MCP App owner; S8. | `S8-REQ-001`; `S8-REQ-002`; `S8-REQ-003`; `S8-REQ-004` |
| [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54) | Canonical Context owner; S3 onward, with Host-controlled submissions distinct from opaque hosted steps and Memory distinct from active history. | `S3-REQ-003`; `S3-REQ-006`; `S4-REQ-003`; `S5-REQ-003`; `S7-REQ-004`; `S7-REQ-006`; `S7-REQ-007` |
| [Prototype the Fixed Workspace Shell and Dynamic Surface Boundary](https://github.com/FrankQDWang/StoryOS/issues/55) | Approved visual/surface evidence; S2 production workspace, S8 Apps. | `S2-REQ-009`; `S8-REQ-001` |
| [Specify the PostgreSQL Project Storage, Isolation, and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56) | Canonical persistence owner; each new conversation, continuation, hosted-operation, and Memory family includes Scope, lifecycle, export/restore, and separate deployed-upgrade gates. | `REL-003`; `S2-REQ-005`; `S2-REQ-007`; `S3-REQ-003`; `S7-REQ-005`; `S7-REQ-007` |
| [Threat-Model the StoryOS Service, Client, and External Trust Boundaries](https://github.com/FrankQDWang/StoryOS/issues/57) | Trust invariant at each crossing, including bounded recovery, hosted opacity, and AP-22 Memory maintenance. | `REL-002`; `REL-003`; `S3-REQ-006`; `S4-REQ-004`; `S5-REQ-002`; `S7-REQ-005`; `S7-REQ-006`; `S8-REQ-002` |
| [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58) | Canonical protocol and compatibility-matrix owner; S3 conversation/Run v2, S5 Approval targets, S7 Memory behavior and retired semantic controls. | `REL-003`; `S2-REQ-005`; `S3-REQ-003`; `S5-REQ-001`; `S7-REQ-006` |
| [Define the Modular-Monolith and Repository Governance Boundaries](https://github.com/FrankQDWang/StoryOS/issues/59) | Architecture/governance invariant; all stages. | `REL-003`; `REL-006` |
| [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) | Proof owner; consume every current release requirement and stage. | `REL-005`; `REL-007`; `HND-006` |
| [Record the Deferred Eval Observation Boundary](https://github.com/FrankQDWang/StoryOS/issues/61) | Deferred outside MVP; the Eval boundary owner retains only the future observation concept. | None; outside MVP. |
| [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62) | Release owner for this complete route and its coverage. | `REL-004`; `REL-007` |
| [Specify Subrun Control-Plane, Mailbox, and Observability Semantics](https://github.com/FrankQDWang/StoryOS/issues/63) | Canonical Subrun owner; S9. | `S9-REQ-002`; `S9-REQ-003` |
| [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64) | Canonical retention owner; existing invariants and RET-015 through RET-018 cover generated cleanup, active compaction, covered-copy restrictions, and Memory portability in their allocated stages. | `S2-REQ-005`; `S2-REQ-007`; `S3-REQ-006`; `S4-REQ-004`; `S7-REQ-005`; `S7-REQ-007`; `S8-REQ-003`; `S9-REQ-003` |
| [Research Durable, Inspectable Agent Memory Architecture](https://github.com/FrankQDWang/StoryOS/issues/65) | Retained research evidence; S7 implementation and proof. | `S7-REQ-005`; `S7-REQ-007` |
| [Research Trustworthy Browser Author-Intent Attestation Boundaries](https://github.com/FrankQDWang/StoryOS/issues/67) | Retained research evidence; Admission/trust invariant from S1. | `REL-002`; `S2-REQ-003`; `S2-REQ-004` |
| [Specify Author Command Admission](https://github.com/FrankQDWang/StoryOS/issues/68) | Canonical Admission owner; every author-command surface. | `REL-002`; `S2-REQ-003`; `S2-REQ-004`; `S8-REQ-004` |
| [Validate Production Editor Session, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/69) | Retained experimental evidence; S1/S2 production continuity. | `S2-REQ-005`; `S2-REQ-008`; `S2-REQ-009` |
| [Specify Web Editor Session, Local Journal, Projection, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/70) | Canonical Web owner; S1/S2 and later editor interaction. | `S2-REQ-003`; `S2-REQ-004`; `S2-REQ-005`; `S2-REQ-009` |
| [Define Plain-Language Discovery-Writing Assistance Semantics](https://github.com/FrankQDWang/StoryOS/issues/75) | Canonical intent owner; S3 and every later assistance mode. | `S3-REQ-002`; `S5-REQ-004`; `S6-REQ-001` |
| [Measure the Representative Writing-Path Performance and Storage-Growth Envelope](https://github.com/FrankQDWang/StoryOS/issues/76) | Retained measurement evidence; accepting owners select stage obligations without inventing an SLA. | `S2-REQ-008` |
| [Create and Lock the First Editor-First Implementation Issue](https://github.com/FrankQDWang/StoryOS/issues/77) | Historical first Stage 1 handoff; later stages use current parent specifications and approved child graphs. | `REL-006`; `HND-002`; `HND-006` |

Current production governance also consumes the completed
[Migrate the Protected Web Client Fully to TypeScript and Vitest Browser Mode](https://github.com/FrankQDWang/StoryOS/issues/209),
[Move repository verification to a local-first gate](https://github.com/FrankQDWang/StoryOS/issues/219),
and [[Automated Architecture Review] Own the Production Protected Web Client Host](https://github.com/FrankQDWang/StoryOS/issues/243).
Their current ADR 0015, ADR 0016, repository, and verification boundaries
apply to every added stage; their completed implementation is reusable.

## 10. Planning handoff and decision boundaries

The release correction delivers these requirements to the existing proof
owner. That owner must cover every current requirement, journey, evidence,
stage, and successor without treating the historical Stage 1 crosswalk as
current route verification. It also owns the mechanism that detects a missing
source disposition, duplicate owner, unknown identifier, or stale ticket graph.

After the proof owner consumes this release revision, `/to-spec` refreshes the
existing [Deliver Stage 3: The Complete Fake-Model Proposal Loop](https://github.com/FrankQDWang/StoryOS/issues/361),
[Deliver Stage 4: One Authorized Real-Model Journey](https://github.com/FrankQDWang/StoryOS/issues/362),
[Deliver Stage 5: Governed Research Tools and MCP](https://github.com/FrankQDWang/StoryOS/issues/363), and
[Deliver Stage 7: Project Continuity and Inspectable Memory](https://github.com/FrankQDWang/StoryOS/issues/365)
parent specifications.
Then `/to-tickets` presents any changed child breakdown and actual native
blocking edges for user approval before publication. Reuse current child
owners and stable requirements, preserve Stage 1/2 historical evidence and
unaffected Stage 6/8/9 promises, and audit forward/reverse coverage. Existing
old bodies do not certify these changes. Stage 3 and later implementation
remains EXECUTION HOLD until alignment passes and the author explicitly
resumes execution; this decision creates no implementation ticket or stage release.

The two conversation defaults are recorded in the release baseline. Concrete
external services, accounts, spending, and destination authorization remain
explicit prerequisites of the affected real-integration ticket. No Eval
definition, evaluator, rubric, or API decision blocks MVP planning or delivery.
The user's implementation-ticket publication and execution pause remains active.
