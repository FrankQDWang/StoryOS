# Glossary: Instructions, context, and memory

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Author Preference**:
An explicit, future-facing, scope-bounded author-owned creative or working constraint within Authoritative State. When an unambiguous author instruction maps deterministically to its domain action, that instruction is its authorization; local or ambiguous feedback cannot create one and may only source an Inferred Preference.
_Avoid_: Inferred Preference, hidden rule, globalized feedback, memory confirmation

**Project Instruction**:
An optional, author-edited, project-scoped instruction whose immutable Revisions are created through project settings and may define language, expression, author-Agent working style, and long-lived working requirements. Its absence never prevents the Default Context Experience, while its presence has Instruction Authority for model context but grants no Capability or disclosure permission, bypasses no Proposal or Acceptance boundary, and does not turn asserted story facts or an outline into Authoritative State or an Author Plan.
_Avoid_: Required project setup, system prompt, Author Plan, project document as canon

**Project Instruction Binding**:
The immutable top-level AgentRun fact that binds either the exact effective Project Instruction Revision at Run creation or the explicit fact that none was configured; every descendant Subrun and applicable Context Assembly Manifest independently references the same binding. Ordinary setting updates affect only new top-level AgentRuns, current-run changes require scoped Steering Input, and later ineligibility of the bound Revision makes dependent work explicitly Degraded or Blocked rather than switching, continuing, or rewriting prior Steps.
_Avoid_: Mutable active instruction, latest settings lookup, implicit Subrun inheritance, session prompt copy

**Effective Model Context**:
The Effective Destination Context for one Model Attempt, including the AgentRun's bound Project Instruction Revision whenever one exists. Its evidence distinguishes StoryOS-held input, sent content, known continuation references, Provider reports, and opaque internal state; required current instructions must be supplied under the validated profile without claiming knowledge of model attention or all Provider-internal content.
_Avoid_: Request delta, initial prompt only, provider session assumption

**Instruction Precedence**:
The fixed order in which StoryOS product, domain, safety, permission, Capability, ToolSpec protocol, Proposal, and Acceptance boundaries outrank current exact author instructions, Steering Input, Approval, and Wait answers; those outrank the Project Instruction Binding; it outranks selected Skill Instruction Context; and all outrank Data-only Context. Within one authority layer, a current applicable instruction with more specific scope prevails, while no lower layer can waive a higher boundary.
_Avoid_: Prompt order, last text wins, Skill override, model-chosen priority

**Agent Memory**:
Project Scope-bound, inspectable, generated reference material that helps the Agent carry useful context across Project Conversations and AgentRuns. It can be corrected and read selectively, but grants no authority, instruction priority, execution permission, or guarantee of current truth.
_Avoid_: Authoritative State, Provider continuation, active context compaction, hidden persistent memory

**Conversation Memory Settings**:
The independent author settings on one exact Project Conversation that control new Agent Memory reads and that conversation's eligibility for future Memory extraction. They may change only when every associated foreground root AgentRun is Terminal, or none exists; independent background Memory work does not prevent the change, which neither deletes published Memory nor rewrites recorded context or earlier disclosure.
_Avoid_: Project-wide memory switch, semantic suppression, deletion request, active-loop steering

**Working Context**:
The operation-bounded view of live author input, in-progress model or Tool activity, short-term plans, and other unsettled material needed to continue active work. Only an immutable item version captured by the applicable Operation Input Snapshot may become a Context Candidate; for a RunStep, its Step Snapshot fulfills that boundary. A live mutable buffer, stream, or process object cannot be injected directly. Working Context evidence may remain in Operational Records for recovery; Memory extraction later reads eligible durable conversation snapshots, never the live buffer itself.
_Avoid_: Agent Memory, durable project knowledge, hidden cross-Run memory

**Workspace Context**:
The StoryOS-owned interaction surface and exact view state from which an interactive operation begins, such as the active editor, Project, chapter, selection, or transcript location, bound to the same Project Scope and applicable Working Target. It supplies attributable UI origin and deterministic locators but grants no source authority, permission, Capability, or cross-project access. A noninteractive Service, Job, embedding, telemetry, or other operation with no interaction surface records an explicit not-applicable fact plus its exact typed cause and Operation Input Snapshot rather than fabricating UI state.
_Avoid_: Project identity, browser tab as authority, process-global current workspace, untrusted client selection, fabricated background UI state

**Context Assembly**:
The Host-owned seven-gate pipeline for each StoryOS-controlled destination submission: Operation Requirement Determination, Candidate Discovery, Source Eligibility Gate, Selection and Ranking, Bounded Projection, Context Assembly Manifest Commit, then Destination-specific Disclosure and Attempt. Received results cross it before later StoryOS submission; a bounded Provider-hosted Operation has prior admission for its whole scope without invented Host gates for invisible internal steps.
_Avoid_: Prompt construction, retrieval query, provider request builder, internal Provider trace

**Purpose**:
The explicit, bounded reason one operation processes context and the exact class of result it is allowed to produce for its named destination. Purpose constrains discovery, projection, disclosure, and completion but grants no source access, Capability, authority, or permission and cannot be broadened in place after assembly begins.
_Avoid_: Vague task label, destination identity, capability, post-hoc justification

**Working Target**:
The exact Project Scope-bound domain object, immutable input version, selection, question, or bounded object set on which one operation is allowed to work, together with any expected current Revisions needed to detect drift. It identifies the focus of work but grants no authority, permission, source eligibility, or right to expand to surrounding Project content. A legitimately untargeted noninteractive operation records an explicit not-applicable fact and remains bounded by its Purpose, typed cause, Project Scope, Operation Input Snapshot, and allowed source classes rather than inventing a target.
_Avoid_: Working Target Context, whole Project, editor tab, inferred story scope, fabricated Job target

**Operation Input Snapshot**:
The immutable, Project Scope-bound Operational Record capturing the exact inputs and source versions from which one operation begins. For a RunStep, its Step Snapshot fulfills this boundary; a Service, Job, embedding, telemetry, or other non-Run operation records an equivalent typed snapshot without inventing an AgentRun or Step Snapshot. Later input changes create a new Snapshot and never mutate one already used by Context Assembly.
_Avoid_: Live buffer, process memory, Step Snapshot for every operation, mutable request object

**Operation Requirement**:
The immutable Operational Record produced by Operation Requirement Determination, binding one Purpose, requester, Project Scope, applicable Workspace Context and Working Target or explicit not-applicable facts, Operation Input Snapshot, intended result, destination boundary, exact Processing Destination Identity when destination-bound, Mandatory Context, permitted dynamic source classes, budgets, applicable grants, authorization policy and approval requirement, and declared degradation behavior. A material boundary change creates a new Operation Requirement rather than widening the existing one.
_Avoid_: RunPlan, prompt, mutable request options, destination grant

**Operation Requirement Determination**:
The first Context Assembly gate, which binds one operation's Purpose, exact requester User, Project Scope, applicable Workspace Context and Working Target or explicit not-applicable facts with an exact typed cause and Operation Input Snapshot, intended result, destination requirement, required context, and permitted dynamic source classes before Candidate Discovery. An initial exact destination class is only a routing bound: before this gate completes, the Host must select one independently established Processing Destination Identity under the same Project Scope, then validate its applicable Project Destination Grant or other owning use-authorization boundary before any scoped external-use binding or compatibility Decision can enter routing. Context Assembly cannot begin without an explicit Purpose, valid Project Scope, exact destination identity for destination-bound work, and destination-permission boundary.
_Avoid_: RunPlan, Model Route Request, implicit prompt goal

**Mandatory Context**:
The closed, operation-specific context obligations that must participate in Context Assembly, divided into Host Control Context and a Mandatory Context Projection for each destination. Mandatory means required for a complete operation, not exempt from the Source Eligibility Gate, budget, projection, or disclosure gates.
_Avoid_: Entire prompt, full project context, eligibility bypass, always disclose

**Non-degradable Context Requirement**:
A Mandatory Context obligation whose absence, ineligibility, or unverifiability makes the current operation Blocked, including its Purpose and exact initiating instruction or typed cause, Project Isolation, exact Working Target when applicable or its explicit not-applicable fact and Operation Input Snapshot, applicable authority and safety constraints, effective capability and destination permission, and durable manifest boundary. A current author instruction is required when one initiated or steered the operation; an authorized event, schedule, Service, or Job cause records its exact identity without inventing author speech. The Requirement cannot be waived by a model, ranking policy, warning, or runtime fallback.
_Avoid_: High-priority context, soft requirement, best-effort prerequisite

**Declared Context Degradation**:
A named fallback fixed by the Operation Requirement before Candidate Discovery, specifying which otherwise-required context may be absent, why continuation remains valid, how the result and completion criteria narrow, which claims or effects become forbidden, and how the limitation remains inspectable. An undeclared fallback or one that changes the Purpose, destination, or intended result requires a new Operation Requirement rather than mutating the current operation.
_Avoid_: Silent omission, ad hoc model fallback, lower confidence, unchanged success

**Optional Dynamic Context**:
An allowed Context Candidate source whose absence or non-selection is an ordinary bounded-selection result rather than a degradation. Optionality grants no exemption from Source Eligibility or destination disclosure rules when the content is selected.
_Avoid_: Declared Context Degradation, low-priority mandatory context, unrestricted retrieval

**Context Sufficiency Decision**:
The immutable Host determination before Context Assembly Manifest commit that the operation is Complete, Degraded under one exact Declared Context Degradation with explicit unmet needs, or Blocked with reasons. A model cannot author the Decision, and a Degraded result cannot satisfy the original unmodified completion criteria.
_Avoid_: Warning, confidence score, model self-assessment, success with omissions

**Host Control Context**:
The exact Operation Requirement, identities, policies, grants, Approvals, budgets, destination contracts, eligibility results, and manifest evidence that the Host must use to govern one Context Assembly. It is mandatory control input but is not automatically model-, Tool-, MCP-, or provider-visible.
_Avoid_: System prompt, destination payload, hidden grant expansion

**Mandatory Context Projection**:
The minimum destination-visible projection required to perform one eligible Operation Requirement, including its Purpose and exact initiating instruction or typed cause, current author instruction when applicable, bounded Run Continuity Context and Working Target Context when applicable, applicable explicit Author Preferences and author-required sources, selected Skill instruction and outcome contracts, actual Tool contracts, and only the operational constraints needed for planning. Whole transcripts, manuscripts, Agent Memory, Research collections, and author-owned outlines remain dynamic sources by default rather than mandatory payloads.
_Avoid_: Host Control Context, full transcript, whole-project prompt, author outline as plan

**Run Continuity Context**:
The bounded ordered conversation input and task state prepared for one RunStep, including prior author input, Steering Input, decisions, and settled results or their recorded compaction. It preserves known references and gaps without making every original source a fresh read or replacing durable conversation history.
_Avoid_: Whole Project, Provider state as history, semantic influence graph

**Working Target Context**:
The exact current object of work and its necessary local structure and Revisions, such as the current passage or selection, adjacent structure needed to interpret it, applicable Authoritative Revisions, and any Proposal under review. It does not make the surrounding chapter, manuscript, project library, or author outline mandatory by proximity.
_Avoid_: Whole manuscript, project dump, inferred story plan

**Context Source Version**:
The exact immutable owning-domain reference by which one Context Candidate is identified and replayed: an Authoritative or Artifact Revision where that domain uses Revisions, or an exact Operational Record, typed terminal outcome, Snapshot, ToolSpec, SkillPackage Snapshot, policy version, Registration revision, or other versioned contract where it does not. Context Assembly never invents a fake Revision or resolves mutable latest content after the fact.
_Avoid_: Mutable source, locator only, index row, universal Revision type

**Context Candidate**:
An exact source identity and Context Source Version discovered for possible use by one Operation Requirement, before current eligibility, selection, projection, or disclosure has been established. A mandatory source must become a Context Candidate for consideration but receives no exemption from later gates.
_Avoid_: Candidate Artifact, selected context, qualified source, index hit as permission

**Candidate Discovery**:
The second Context Assembly gate, which enumerates mandatory sources and locates dynamic Context Candidates only from source classes allowed by the Operation Requirement. Discovery and Retrieval Index hits establish neither current eligibility nor permission to use or disclose content.
_Avoid_: Source Eligibility Gate, context selection

**Dynamic Retrieval**:
The bounded discovery of non-universal context for one Operation Requirement through Deterministic Requirement Retrieval, an Agent Retrieval Request, or Author-requested Retrieval. Retrieved content becomes Context Candidates only and cannot mutate the current Step Snapshot or an in-flight Model Attempt.
_Avoid_: Automatic prompt injection, unrestricted project search, mutable current context

**Deterministic Requirement Retrieval**:
Host-initiated retrieval that resolves exact or typed context obligations already declared by the Operation Requirement, such as its Working Target, applicable Author Preference, or a selected Skill's required context role, without making a semantic relevance guess. Results remain subject to the complete Context Assembly pipeline before use.
_Avoid_: Similarity-based first-turn injection, Agent Retrieval Request, implicit source expansion

**Agent Retrieval Request**:
A typed request in one persisted Agent Decision that states the retrieval Purpose, allowed source classes, scope, and budget for later context work. Its results can enter only a subsequent RunStep through a new Context Assembly and never rewrite the requesting Step Snapshot or Model Attempt.
_Avoid_: Mid-step context mutation, free-form search side effect, automatic injection

**Author-requested Retrieval**:
A source lookup requested by the author through an explicit reference or interpreted by the Agent from an ordinary Message. It grants no source authority, access, budget exemption, or disclosure permission and creates no persistent context-control rule.
_Avoid_: Context Pin, semantic intent classifier, source promotion, direct disclosure

**Speculative Context Prefetch**:
A disposable optimization that may warm a StoryOS-controlled, Project Scope-bound index or cache without making its results selected, manifested, or destination-visible. Any prefetch requiring a model, Tool, MCP server, embedding service, or other External Processing Destination is a separate operation that must cross all seven Context Assembly gates.
_Avoid_: Background disclosure, pre-approved context, first-turn injection

**Source Eligibility Gate**:
The third Context Assembly gate, which fail-closed checks every Context Candidate's exact domain identity and Context Source Version, matching Project Scope and Project Isolation for project-bearing content, caller permission, Source Integrity, Context Trust Assessment, Disclosure Eligibility, and every owning-domain qualification that applies to that source kind. A globally or User-reusable schema, ToolSpec, policy, Adapter definition, public capability description, or other definition may remain source-unscoped only when it contains no project-derived data or project authority; its selection and use still bind the current Project Scope. Applicable qualifications include current memory-use settings for newly read Memory Documents, Lifecycle, Archive, Tombstone, Retention State, Story Scope, and Epistemic Scope. A non-applicable qualification is recorded as such rather than invented; an applicable check that fails or cannot be established excludes the Context Candidate, while an ineligible required source blocks the operation or enters an explicit recorded degradation mode.
_Avoid_: Relevance threshold, confidence warning, ranking penalty, best-effort inclusion

**Context Trust Assessment**:
The orthogonal determination of a Context Candidate's Source Integrity, Instruction Authority, domain or evidentiary status, Execution Trust, and Disclosure Eligibility for one operation. No axis implies another: executable infrastructure does not make its content true, authoritative, instructive, or disclosable.
_Avoid_: Trusted boolean, source reputation score, server trust as content authority

**Source Integrity**:
The evidence that content matches its claimed source identity, exact Context Source Version, and applicable digest or capture boundary. Integrity proves attribution and unchanged bytes, not truth, authority, Instruction Authority, or current eligibility.
_Avoid_: Source truth, trusted content, evidentiary sufficiency

**Instruction Authority**:
The closed eligibility to direct Agent behavior, held only by applicable StoryOS Host product, domain, policy, and safety constraints; exact current author instructions, Steering Input, Wait Resolutions, and authoritative Author Preferences; the exact Project Instruction Revision named by the applicable Project Instruction Binding; and the current Step's exact selected Skill Instruction Context. Instruction Authority never arises from prose that merely looks imperative, source ownership, signatures, repetition, retrieval rank, Tool or MCP execution trust, or model output.
_Avoid_: Prompt position, trusted server instructions, author-owned document, model-generated rule

**Data-only Context**:
Context that may inform an Agent Decision but has no Instruction Authority, including Tool and MCP output, model output, Agent Memory, Research, external or imported documents, manuscript prose, and author-owned outlines unless separately expressed through an authoritative or instructional domain path. Imperative text inside it remains quoted data rather than executable instruction.
_Avoid_: Lower-priority instruction, untrusted system prompt, implicit Author Plan

**Execution Trust**:
The current Host determination that one exact registered model, Tool, MCP, adapter, or other implementation may be invoked through its governed execution boundary. It grants no content truth, Instruction Authority, Authoritative State, capability, or destination disclosure permission.
_Avoid_: Tool Exposure, Capability Grant, trusted output, server authority

**Disclosure Eligibility**:
The current operation- and destination-specific determination that one exact source or Projection may be included in an Outbound Disclosure under the same Project Scope and applicable policy, grants, Purpose, data categories, and minimization rules. It does not follow from source read permission, Source Integrity, Execution Trust, selection, or prior disclosure.
_Avoid_: Network access, source eligibility alone, cached consent, provider trust

**Selection and Ranking**:
The fourth Context Assembly gate, which gives eligible Mandatory Context budget priority and applies one exact Context Ranking Profile only to eligible dynamic Context Candidates. Selection and rank grant no truth, authority, evidentiary status, binding force, or disclosure permission.
_Avoid_: Authority ranking, permission score

**Context Ranking Profile**:
An immutable, versioned, Purpose- and source-class-specific comparison contract defining allowed relevance, scope specificity, structural or causal proximity, coverage, diversity, evidence-balance, and genuinely time-sensitive currency signals plus budget behavior and a stable non-semantic final tie-break. It cannot use a global trust score, source authority or ownership as a bonus, prior access or retrieval frequency, popularity, repetition, model confidence, or wall-clock decay for still-applicable fiction truth and historical evidence.
_Avoid_: Universal relevance score, trust ranking, access-frequency boost, authority weight

**Bounded Projection**:
The fifth Context Assembly gate and its attributable minimum-necessary transformation of selected input under a versioned policy, Purpose, destination, and intake boundary. Its immutable Operational Record retains known input references and loss or unknown-state evidence without replacing the source; each StoryOS-submitted generation request passes its own Context Assembly.
_Avoid_: Source Revision, silent truncation, rewritten history, evidence replacement

**Context Projection Policy**:
The immutable, versioned source-class and destination contract selecting exactly one projection mode: Exact Required, Deterministic Excerpt, Derived Summary, or Reference Only, with hard item bounds and failure behavior. Budget is allocated first to non-degradable Mandatory Context, then declared degradable Mandatory Context, then ranked dynamic context; an unfit Exact Required item requires another eligible route, a new reframed operation, or a Blocked decision.
_Avoid_: Arbitrary token truncation, best-effort fit, provider-side default

**Exact Required Projection**:
A Projection mode that preserves the complete eligible content and semantics of an item such as the current author instruction, governing authority or safety constraint, exact required Skill or Tool contract, or declared Working Target. It cannot be silently truncated, summarized, or replaced to fit a destination limit.
_Avoid_: High-priority excerpt, auto-summary, head-tail truncation

**Deterministic Context Excerpt**:
A Projection mode that selects exact locatable ranges or complete domain units and records the original extent and cut boundaries. It cannot replace Exact Required content with a partial excerpt or claim the destination inspected omitted material.
_Avoid_: Silent truncation, summary, required-content omission

**Derived Context Summary**:
A lossy Projection from recorded input and prior projection references under an identified generation policy, producer, and Projection Loss Indicator. It does not replace original evidence or prove complete semantic preservation, and a StoryOS-submitted generation request requires its own Context Assembly.
_Avoid_: Rewritten history, semantic equivalence proof, source evidence

**Context Compaction Projection**:
An immutable Operational Record of one bounded active-context replacement, with known input and prior projection references, producer, output or native opaque reference, reported usage, and loss or unknown-state evidence. It supplies later requests without rewriting prior Messages, Run Events, Tool results, Step Snapshots, manifests, or request history, and is neither an Artifact nor Authoritative State.
_Avoid_: Durable history deletion, Memory Document, semantic erasure, mutable past request

**Opaque Provider Continuity**:
A Provider-specific cache handle, prior-response reference, encrypted compaction object, or other non-inspectable mechanism bound to its original destination and validated Adapter mapping. It may support an admitted request but is neither StoryOS history nor authorization, and unknown internal content or model influence is not reconstructed from the reference.
_Avoid_: Context Candidate, Provider state as source of truth, shared Project session

**Context Cache Entry**:
A disposable prompt, retrieval, Projection, embedding, Tool-schema, or other acceleration product keyed by exact Project Scope, Context Source Versions, policy and transformation versions, qualification state, destination identity, grant, and Adapter mapping. It owns no source meaning, eligibility, authorization, historical evidence, or authority and is never reusable across either Project Scope identity.
_Avoid_: Context Candidate, Context Assembly Manifest, durable memory, cached permission

**Context Cache Reuse Decision**:
The current fail-closed check of a cache entry against its actual input, Scope, permission, policy, destination, grant, and Adapter dependencies. Cached new-source reads require current source versions and applicable Memory settings; cached historical input uses its recorded identity and retained-copy restrictions without recursively resolving every mentioned source to latest.
_Avoid_: Cache hit, stale read as current, prior consent, semantic influence closure

**Context Inspect**:
A read-only audit query of current or historical Operation Requirements, discovery, eligibility, selection, Projections and loss, manifests, Wire Payload Projections, Disclosure Events, and Destination Attempts, preserving historical facts while showing current invalidity separately. Its exact wire view means the exact non-secret application payload and protocol projection plus opaque secret-injection placeholders, never credential values, credential-value digests, or an unredacted transport envelope. Inspection obeys current Project Isolation, permissions, and redaction and distinguishes exactly reconstructable, reference-known, and provider-opaque context without presenting inference as fact. It serves diagnosis and verification; the Protected Web Client does not show it to the author.
_Avoid_: History rewrite, model-use claim, unredacted debug dump, author Run details view

**Default Context Experience**:
The author-facing promise that the editor Agent receives the eligible current Working Target, instructions, and bounded conversation continuity, with optional Memory navigation and on-demand reads. Ordinary assistance needs no context scopes, source-version strategies, pins, character sheet, Project Instruction, or routine confirmation; real settings remain available when needed, and the author sees assistance results, not Run internals.
_Avoid_: Manual context setup, semantic control registry, context confirmation on every step

**Context Reference**:
A Reference Only Projection exposing bounded catalog information and an exact source-qualified locator without exposing the referenced payload. It lets an Agent request later retrieval but does not prove that the referenced content was model-visible, eligible for disclosure, or used.
_Avoid_: Loaded context, citation as disclosure, implicit retrieval

**Projection Loss Indicator**:
The recorded known cuts, transformations, expected information loss, and unknown semantic effects of one lossy Projection. Deterministic cuts can be exact; generated output cannot prove an exhaustive inventory of omitted meaning or model influence.
_Avoid_: Exact semantic-preservation proof, hidden truncation, confidence score

**Context Assembly Manifest**:
The immutable provider-neutral Operational Record of StoryOS preparation, committed at gate six before controlled destination I/O and binding Scope, requirement, input snapshot, sufficiency, considered candidates, selected input, known references, projections, policies, budgets, and gaps. It establishes neither Provider-internal content nor model attention; failure to persist it prevents submission.
_Avoid_: ContextManifest, prompt dump, model-use proof, mutable request log

**Destination-specific Disclosure and Attempt**:
The seventh Context Assembly gate that minimizes and authorizes one StoryOS-controlled submission, binds its exact destination and any permitted hosted processing, and establishes its Destination Attempt evidence. Cache or continuation grants no authority; an external dispatch requires disclosure evidence, while invisible Provider-internal steps do not create invented Host Attempts.
_Avoid_: Shared provider payload, cached authorization, prior Destination Attempt reuse

**Settled Source Version**:
An exact durable Revision or typed terminal outcome whose settlement meaning has committed and is stable enough to analyze as a derivation source. Settlement makes that version eligible for extraction but does not prove a generalization, make it permanently current, or prevent later source change from invalidating derived results.
_Avoid_: Live stream, intermediate event, latest value, permanently true source

**Retrieval Index**:
A disposable, rebuildable full-text, vector, graph, or other access projection over exact domain identities, Context Source Versions, current qualification records, and one build-policy version. Its internal IDs and scores have no domain meaning, and every result must fail closed unless the current source version, every applicable owning-domain qualification such as Lifecycle and current memory-use settings when reading Memory Documents, scope, and permission eligibility can be revalidated before context use.
_Avoid_: Semantic memory, vector store of record, index ID as Durable Identity, ranking as truth
