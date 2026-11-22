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

The serial release order is Stage 5 through Stage 7, then Stage 10, then
Stage 11
([ADR 0096](../adr/0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md),
[ADR 0101](../adr/0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md)).
A stage number is the identity of the stage, and it is not its place in the
order. Stage 8 and Stage 9 stay on EXECUTION HOLD. ADR 0101 records the order
that the author agreed for the work after Stage 11, and a later ADR decides
about Stage 8 and about Stage 9 when that work starts. Each stage starts
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
| S5-REQ-001 | The ordinary Agent conversation can perform bounded research through StoryOS ToolCalls. StoryOS dispatch uses the one Tool Gateway. StoryOS performs no Provider-hosted Operation (ADR 0084). ToolSpec, Registration, Enablement, Exposure, Capability, and Approval stay distinct. | Tool/MCP owner and Context owner |
| S5-REQ-002 | A third-party MCP server is an untrusted registered integration. Bind its exact contract and Project use, reject incompatible drift, mediate each effect, and keep credentials outside project records and output. | Tool/MCP owner and trust owner |
| S5-REQ-003 | Preserve native function-call/result correlation and validate the complete model-requested business-tool batch before deriving StoryOS ToolCalls. A native item of a type that the request did not ask for makes the Agent Decision invalid (ADR 0084). Returned content passes Context Assembly before a later StoryOS-controlled submission. Results grant no instruction, permission, or authority. | Model Gateway and Context owners |
| S5-REQ-004 | The author can inspect a research synthesis, its claims, exact sources, and supporting, conflicting, or limiting evidence. A missing or unavailable source remains visible as a gap, not an invented citation. | Research/Memory owner |
| S5-REQ-005 | Tool cancellation, interruption, unknown effects, and unknown usage stay durable and inspectable. Enforce registered intake, actual outward-processing bounds, and finite worst-case reservations before submission. Resume/reconciliation obey existing fences; a timeout proves neither remote stop nor retry permission. Incomplete, rejected, cancelled, or fenced output cannot advance normal continuation. | AgentRun, Tool/MCP, and retention owners |
| S5-REQ-006 | Research, Tool, and MCP results can supply bounded assistance or an editable Proposal. They cannot directly change prose, fiction facts, preferences, or manuscript structure. | Artifact, Core/Proposal, and Admission owners |

**Author journey S5-JRN-001.** Ask a research question in the normal Agent
panel. Use a real StoryOS Tool/MCP path; inspect each operation's scope and
any required Approval. Continue
with the exact native function/result correlation. Inspect returned sources,
reports, known gaps, and a validated partial result with its limitations. Use
a result as discussion or a Proposal and reject one creative change. Interrupt
an external operation, recover its exact disposition, then disable the
integrations and continue manual writing.

StoryOS performs no Provider-hosted Operation
([ADR 0084](../adr/0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md)).
A model request carries no Provider-side tool. Existing grants cover unchanged
in-scope work without another prompt. Tool Approval keeps one target, the
StoryOS ToolCall, and stays a contract for a later Stage that adds an effect
outside the Project grant; disclosure Approval remains distinct. Each StoryOS
ToolCall retains its own permitted effects. Stage 5 waived the hosted stories
and the Approval stories of an exact operation
([#363](https://github.com/FrankQDWang/StoryOS/issues/363)).

| ID | Mandatory evidence |
| --- | --- |
| S5-EVD-001 | Real Tool/MCP qualification, registration, use, intake, bounds, grants, and contract-drift evidence. Unauthorized or unbounded operations refuse before dispatch. |
| S5-EVD-002 | Native function/result correlation, complete batch validation, research claims and exact available evidence, truthful gaps/partial results, and re-entry at StoryOS-controlled submissions. Provider reports remain distinct from Host observations and opaque internal work. |
| S5-EVD-003 | Durable interruption, cancellation, unknown-effect/usage recovery, single accounting, selected-result continuation, Proposal-only creative change, and AI-independent regression evidence. |

## 3. Stage 6: Production Skill packages and composition

| ID | Production obligation | Semantic owner |
| --- | --- | --- |
| S6-REQ-001 | The author or Agent can select an eligible standard Skill without a task-mode menu. Resolve its source, exact package snapshot, and explicit selection reason. One source and one installation scope exist: the deployment provides each Skill, and a Skill name is unique in one deployment (ADR 0089). Standard Skills work without mandatory StoryOS extensions. | Skill owner |
| S6-REQ-002 | Load instructions and resources progressively under the existing instruction precedence and Context rules. A package grants no permission, disclosure right, or authority. | Skill and Context owners |
| S6-REQ-003 | Withdrawn for Stage 6 by ADR 0089. Tool roles, optional extensions, scripts, and outcomes stay deferred with the StoryOS Skill Extension and the Skill Script Execution. | Skill, Tool/MCP, and AgentRun owners |
| S6-REQ-004 | The author sees the Skills that a turn used and the reason of the Agent. A change of the Skills of the deployment by its operator applies to later Runs only and does not silently replace a snapshot bound to active work. Each Skill that an author provides (creation, installation, replacement, removal), the inspection of composed instructions, conflicts, and outcome obligations, and the Skill Revocation stay deferred (ADR 0089). | Skill owner |

**Author journey S6-JRN-001.** Select and run a standard instruction-only
Skill of the deployment; let the Agent load a Skill and one of its resources;
use two Skills in one turn; inspect the usage record; change the Skills of the
deployment while a Run is in progress, and preserve prior Run evidence;
continue writing when the Skill is unavailable.

[ADR 0089](../adr/0089-deliver-standard-skills-from-the-deployment-and-load-them-with-host-tools.md)
withdrew the Tool-role and script step, the missing-prerequisite step, the
package inspection step, and the revocation step of the earlier journey.

| ID | Mandatory evidence |
| --- | --- |
| S6-EVD-001 | Standard-package compatibility, exact selection/snapshot, progressive loading, composition, and instruction-authority evidence. |
| S6-EVD-002 | A change of the Skills of the deployment with a Run in progress, the refusal of an invalid or oversized package at the start of the Server, active-run binding, and editor-regression evidence. ADR 0089 withdrew the Tool-role, script, outcome-obligation, and revocation evidence. |

## 4. Stage 7: Project continuity and Memory

| ID | Production obligation | Semantic owner |
| --- | --- | --- |
| S7-REQ-001 | Preserve one Project main Agent across distinct Project Conversations using their existing identities. The author can open an earlier Project Conversation again and continue it (ADR 0094). Cross-conversation continuity uses the Project Instruction, the manuscript tools, and Agent Memory; each conversation retains its own Provider continuation. Do not load the entire transcript, manuscript, or Memory collection by default. | Project Agent and Context owners |
| S7-REQ-002 | Withdrawn for Stage 7 by ADR 0094. The structured fiction records stay deferred. A character, a relationship, and a major event are Memory Entries of S7-REQ-005, and they are not Authoritative State. | Artifact and Research/Memory owners |
| S7-REQ-003 | Withdrawn for Stage 7 by ADR 0094. The Author Preference and the Inferred Preference stay deferred as separate records. The lasting instruction of the author is the Project Instruction of S7-REQ-004. A habit that the author states in a Conversation is a Memory Entry, and a Memory Entry is never an instruction. | Research/Memory and Admission owners |
| S7-REQ-004 | Support optional author-edited Project Instruction, immutable revisions, and exact top-level Run binding. The author writes it on the settings page of the Project (ADR 0094). Existing bindings stay fixed across Subruns and context compaction; absence does not block ordinary assistance. This does not defer any instruction binding already required in an earlier stage. | Context and AgentRun owners |
| S7-REQ-005 | Keep Agent Memory as typed Memory Entries (ADR 0094). The Agent writes an entry at once on the request of the author. Bounded background captures of quiet Project Conversations and of changed Chapters give typed Memory Observations. A fenced consolidation pass applies validated typed operations in one transaction and prunes the entries. Background work cannot change an entry of the author; it records a conflict flag. A bounded Memory Index is in the context of a Run, and the Agent searches and reads entries with Host Tools without embedding. Keep origins, source references, model/prompt/input identities, and maintenance outcomes recorded. | Research/Memory, storage, trust, and retention owners |
| S7-REQ-006 | Apply the Host defaults of the Conversation Memory Settings with no author control (ADR 0094). Offer author inspection of each Memory Entry with its origin, source, and conflict flag on the settings page, and a delete that background work does not undo. Ordinary corrections remain Messages. Retire semantic Include/Pin/Exclude and per-claim Admission/Suppression controls without aliases. | Context, Memory, and Protocol owners |
| S7-REQ-007 | Give the Agent the whole manuscript (ADR 0095): the Chapter list in the context, a keyword search, a read, and a hybrid search. The vector index is rebuildable, and the embedding requests keep their own grant scope, bounds, and disclosure record. Deliver archive/export/restore and recovery of the Memory records. Existing active compaction and Provider continuation stay separate; ordinary corrections do not reset continuation. Memory recall does not require embedding. The removal of superseded Memory payload bytes and the fence of a covered-copy restriction stay deferred (ADR 0094). | Context, Model Gateway, persistence, and retention owners |

**Author journey S7-JRN-001.**

1. Write a Project Instruction on the settings page of the Project and verify
   the old and new Run bindings.
2. Ask the Agent to remember a foreshadowing, and inspect the entry and its
   origin on the settings page. Let a quiet Project Conversation and a changed
   Chapter contribute through background capture and consolidation, then
   inspect the entries and their sources.
3. Continue in a second Project Conversation, then open the first one again
   from the Conversation list. The Agent uses the Memory Index and reads an
   entry. It finds a passage of another Chapter with the keyword
   search and with the hybrid search. Distinguish read/sent evidence from model
   attention and generated recall from author-owned truth.
4. Change a Chapter so that an entry of the author conflicts with it. Background
   work records a conflict flag and keeps the text; the Agent asks the author,
   and the entry changes only after the answer. Delete an entry, and verify
   that background work does not create it again.
5. Recover interrupted or stale background work without a partial result;
   build the vector index again without loss. Export and restore the Project
   with its Memory records. Continue writing with Memory and the embedding
   source unavailable.

[ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md)
withdrew the fiction fact, Author Preference, settings change, copy
restriction, and payload cleanup steps of the earlier journey.
[ADR 0095](../adr/0095-give-the-agent-the-whole-manuscript-through-host-tools-and-add-a-hybrid-search-with-pgvector-and-a-volcengine-embedding-model.md)
added the manuscript search steps.

Stage 7 adds no semantic exclusion classifier, per-claim memory confirmation,
precise-forgetting oracle, or required project policy made from generated text.
Background maintenance does not block the editor and may change only the
Memory Entries that the author did not ask for. It cannot change Skills,
ToolSpecs, policy, credentials, the Project Instruction, or Authoritative
State. ADR 0094 and ADR 0095 own the mechanism; this stage supplies their
implementation and measured limits.

| ID | Mandatory evidence |
| --- | --- |
| S7-EVD-001 | Cross-conversation identity, Project Instruction revision/binding, generated-recall limitations, and author-authority evidence. ADR 0094 withdrew the structured-truth and explicit-preference evidence. |
| S7-EVD-002 | Host default settings behavior, eligible capture of Conversations and Chapters, the bounds of a capture, atomic consolidation, stale/duplicate work refusal, the protection and the conflict flag of an author entry, bounded recall, and author inspection and delete. Include AP-22 hostile-input, wrong-Scope, forbidden-write, secret, oversize, disabled, and unavailable-source cases; inspect records and effects, not semantic truth. |
| S7-EVD-003 | Manuscript search and read, the embedding disclosure and its grant scope, vector index rebuild, the refusal of a stale vector hit, active-compaction separation, and Memory recovery/portability. Cover failed maintenance and truthful gaps with the editor still usable. ADR 0094 withdrew the covered-copy restriction and payload cleanup evidence. |

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

## 7. Stage 10: Author voice

Stage 10 comes after the release of Stage 7 and before Stage 8 and Stage 9
([ADR 0096](../adr/0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md)).
It works on the first problem of the author: generated prose reads as
machine-written. [ADR 0100](../adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md)
sets its scope: default writing rules in the Host instructions, the Project
Instruction as the one place for the sample passages of the author,
SkillPackages of the StoryOS distribution, and marks on the new words of a
Proposal candidate. StoryOS infers no preference of the author automatically.
The specification is
[Deliver Stage 10: Author Voice](https://github.com/FrankQDWang/StoryOS/issues/1760).
The author approved it on 2026-11-21, and that approval released the
execution of Stage 10. The Requirements, the author journey, and the evidence
rows below came with the specification. The other candidates stay in
[`Todos.md`](../../Todos.md), which is not a contract. The rule "no setup
before writing" of ADR 0096 applies.

| ID | Production obligation | Semantic owner |
| --- | --- | --- |
| S10-REQ-001 | Send default writing rules for prose in the Host instructions of each tool-loop request. Each rule states the wanted behaviour and says when a device is appropriate. The rules are no list of forbidden phrases. They are defaults for the style of the prose. A different style applies before them when it is in the author request, in the Project Instruction, in a sample passage that they give, or in the instructions of a Skill. The text of the rules says so. For the instructions of a Skill, this is the exception of ADR 0100 to the order of ADR 0089. A change of the rules gets a new mapping revision and a new Registration. | Context and Model Gateway owners |
| S10-REQ-002 | Keep the Project Instruction as the one place for the lasting demands and the sample passages of the author. The writing rules tell the model to write in the voice of a sample passage. Stage 10 adds no record, no control, and no automatic capture for them, and StoryOS infers no preference of the author automatically. | Context owner |
| S10-REQ-003 | Give each deployment the SkillPackages of the StoryOS distribution as files of the release package: a local revision that gives a candidate for each of a small number of places, and a conflict check that reports and changes no text. An agreement before a scene is the third package while its ticket is in the ticket graph of the specification (ADR 0100 section 2). Each is a standard SkillPackage. The operator sets the Skill directory, and an absent setting gives no Skill (ADR 0089). | Skill owner |
| S10-REQ-004 | Mark the new words of each Proposal candidate in the editor, with a tick where words were removed with no replacement. A candidate has no mark when its target Block is empty, and when almost all of its text is new. The pointer on a mark shows the old words of that mark in the target Block, and the target Block has no mark at other times. When the author changes the text of a candidate, the marks follow the text. The marks are in the view only: the document, the Local Edit Journal, each command, and the copied text do not change. | Workspace and Core/Proposal owners |

**Author journey S10-JRN-001.**

1. Start the packaged Server with the Skill directory of the release
   package. The Skill picker lists the SkillPackages of the distribution
   with their descriptions.
2. Select the local revision and ask for a revision of a Chapter. The
   assistant gives one Proposal with a candidate for each of two Blocks. The
   inputs of the journey are fixed. Each of the two candidates meets the
   marks rule of S10-REQ-004. The target Block of each candidate is not
   empty. Each candidate has new words, and the text of each candidate is
   not almost all new. At least one mark replaces old words, and at least
   one candidate has a removal with no replacement. The editor shows marks
   on the new words of each candidate and a tick at each removal with no
   replacement. The pointer on a mark that replaces old words shows those
   old words in the target Block.
3. Type a small number of new words into one candidate. That candidate
   still meets the marks rule: the typed words get a mark, and the other
   marks agree with the two current texts. Accept one candidate and reject
   the other. The manuscript has the accepted text with no mark.
4. Select the conflict check and ask for a check. The assistant answers with
   text and makes no Proposal.
5. Continue to write with no model destination: the editor works, and no
   mark shows.

The journey uses the fake model, which does not read an instruction text. The
live runs of S10-EVD-001 and S10-EVD-002 show the effect of the instruction
texts on the real route.

| ID | Mandatory evidence |
| --- | --- |
| S10-EVD-001 | The exact text of the writing rules in the request tests, the new mapping revision and Registration, and one live run on the real route with its evidence tag and its record under `docs/research/`. Record: [Agent Plan live run of the writing rules (2026-11-21)](../research/agent-plan-live-writing-rules-2026-11-21.md), tag `evidence/1762-live-writing-rules-2026-11-21`. Record of the changed rules: [Agent Plan live run of the changed writing rules (2026-11-21)](../research/agent-plan-live-writing-rules-findings-2026-11-21.md), tag `evidence/1774-live-writing-rules-findings-2026-11-21`. |
| S10-EVD-002 | The validation of each SkillPackage of the distribution by the rules of the Skill directory, the copy in the release package, and one live run for each package that shows its effect. Record of `conflict-check`: [agent-plan-live-conflict-check-2026-11-21](../research/agent-plan-live-conflict-check-2026-11-21.md). Record of `local-revision`: [agent-plan-live-local-revision-2026-11-21](../research/agent-plan-live-local-revision-2026-11-21.md). Run of `local-revision` with the changed rules: [agent-plan-live-writing-rules-findings-2026-11-21](../research/agent-plan-live-writing-rules-findings-2026-11-21.md). Record of `scene-brief`: [agent-plan-live-scene-brief-2026-11-21](../research/agent-plan-live-scene-brief-2026-11-21.md). |
| S10-EVD-003 | The table of the expected marks for each test input, the view tests (marks, pair, author edit, IME composition, the two no-mark cases, and no change of the document or of the Local Edit Journal), and the journey S10-JRN-001 on the exact dist. Journey: [author-voice-journey.integration.test.ts](../../apps/web/test/browser-exact-dist/author-voice-journey.integration.test.ts). |

Stage 10 adds no automatic capture of a preference, no second model, no mark
on a machine phrase, no trial that compares models or prompts, and no change
of the production model (ADR 0100 section 5).

## 8. Stage 11: Character intents

Stage 11 comes after the release of Stage 10 and before Stage 8 and Stage 9
([ADR 0101](../adr/0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md)).
It works on the third problem of the author: the plot grows from the
characters. ADR 0101 sets its scope: a character file in Agent Memory with a
part of the author and a part of the background work, an intent pass that
gives each character only its own file, the intents as context of a Run of
the author, and a section for the characters on the settings page of a
Project. StoryOS does not simulate the story, and the author decides what the
manuscript gets. The specification is
[Deliver Stage 11: Character Intents](https://github.com/FrankQDWang/StoryOS/issues/1786).
The Requirements, the author journey, and the evidence rows below came with
it. The rule "no setup before writing" of ADR 0096 applies.

| ID | Production obligation | Semantic owner |
| --- | --- | --- |
| S11-REQ-001 | Keep the Memory of one character as a character file with two parts, each one entry of the type character with the name of the character as its title. The part of the author holds what the author told the assistant to remember about the character and that the character knows, and background work never changes its text. The part of the background work holds the view of the character from the manuscript and holds no fact that the character cannot know. A request of the author does not make the part of the background work an entry of the author. A Project has at most one part of each kind for each character. | Context and Memory owners |
| S11-REQ-002 | After the file of a character changed, start one intent pass for that character with no request of the author. The two parts of that file are the only source of knowledge about the character in the pass. A pass sends one model request. When the Host refuses the reply, the pass sends one more request with that refusal as a Tool result, so that the model can correct the reply. Its result is one intent and at most three questions to the author. The Host keeps the newest result with the part of the background work, and a result changes no text of the file. | Agent Run and Memory owners |
| S11-REQ-003 | Give the current intent of each character to each Run of the author that pins its Memory revision, as data that is no fact and no instruction. Give no intent to a capture, to a consolidation pass, or to an intent pass, and give the questions to no Run. Show no intent and no question in the writing workspace. | Context and Model Gateway owners |
| S11-REQ-004 | Give the settings page of a Project a navigation on its left side with the sections "项目说明", "记忆", and "人物". The section "人物" has a search of the names, and for the selected character the file with the part of the author first, the intent with the Chapter at which it was made, and the questions. The page has no control that edits a file, that starts a pass, or that answers a question. | Workspace owner |

**Author journey S11-JRN-001.**

1. Start the packaged Server and the packaged Worker with the fake model. A
   Project has Chapters and one completed Run of the author. The capture of a
   Chapter and a consolidation pass write the part of the background work of
   one character, and an intent pass records one intent and its questions.
2. Open the settings page. The navigation on the left side has "项目说明",
   "记忆", and "人物". Select "人物". The list has the name of the character.
   Type a part of the name into the search: the list keeps the name. Type
   other characters: the list has no name.
3. Select the character. The page shows the file, the block "接下来想做什么"
   with the intent and the line of the Chapter, and the block
   "助手还不清楚的事" with the questions. The section "记忆" has no group
   "人物".
4. In the writing workspace, tell the assistant to remember one fact of that
   character. The Run completes. The settings page shows the part of the
   author first, with the origin line "你让助手记住的", and then the part of
   the background work. It shows no block of the intent until a new intent
   pass records a result, because the file changed. The writing workspace
   shows no intent and no question.
5. Continue to write with no model destination: the editor works, and the
   settings page shows the files that exist.

The journey uses the fake model, which does not read an instruction text. The
live runs of S11-EVD-001 and S11-EVD-002 show the effect of the instruction
texts on the real route.

| ID | Mandatory evidence |
| --- | --- |
| S11-EVD-001 | The tests of the two parts (the write rule, the refusal, the operations of a pass, and the delete marks), the exact instruction texts in the request tests, the new mapping revision and Registration, and one live run of the file on the real route with its evidence tag and its record under `docs/research/`. The record has the length of each file after each Chapter and the check of the limit of knowledge. |
| S11-EVD-002 | The tests of the intent pass (start, input, result, the rule of one pass for each text, and the result that is not kept), the tests that no capture and no pass gets an intent, the exact request texts, and one live run of the pass and of a continuation with the intents on the real route with its evidence tag and its record. |
| S11-EVD-003 | The tests of the response of `listMemoryEntries`, the browser tests of the navigation and of the section "人物", and the journey S11-JRN-001 on the exact dist. |

Stage 11 adds no simulation of the story, no list and no notice of the
intents in the writing workspace, no control that edits a Memory Entry, no
control that starts an intent pass or sets it on or off, and no structured
record of a character (ADR 0101 section 7).

## 9. Deferred scope outside MVP

[Eval](eval-evidence-foundation.md) is a future observation surface outside
MVP. It has no current stage, Requirement, author journey, evidence bundle,
implementation ticket, or release dependency. Its page, APIs, and behavior are
not being designed or implemented. The last stage that the author keeps after
the decision of ADR 0096 section 5 completes this local MVP route.

## 10. Canonical semantic owners

| Surface | Owner and required source |
| --- | --- |
| Workspace and browser continuity | [Specify Web Editor Session, Local Journal, Projection, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/70): [Web contract](web-editor-session-synchronization-and-recovery-semantics.md) and [approved workspace](../design/storyos-three-column-writing-workspace.md). |
| Core/Proposal | [Specify the Manuscript Revision and Proposal State Machine](https://github.com/FrankQDWang/StoryOS/issues/46): [state machine](manuscript-revision-proposal-state-machine.md), ADR 0003, and the approved workspace. |
| Artifact and authority | [Define the Authoritative-State and Artifact Domain Vocabulary](https://github.com/FrankQDWang/StoryOS/issues/44): [domain model](artifact-domain-model.md), [GLOSSARY](../../GLOSSARY.md), and ADR 0001. |
| Admission | [Specify Author Command Admission](https://github.com/FrankQDWang/StoryOS/issues/68): [admission](author-command-admission.md) and ADR 0013. |
| Project Agent and natural-language intent | [Define Plain-Language Discovery-Writing Assistance Semantics](https://github.com/FrankQDWang/StoryOS/issues/75): [discovery writing](plain-language-discovery-writing-assistance-semantics.md), with the retained Project Agent decisions. |
| AgentRun | [Specify Persistent Agent Run and Orchestration Semantics](https://github.com/FrankQDWang/StoryOS/issues/47): [GLOSSARY](../../GLOSSARY.md), protocol, retention, and [experimental tuning register](../../EXPERIMENTAL-TUNING-REGISTER.md). |
| Subrun | [Specify Subrun Control-Plane, Mailbox, and Observability Semantics](https://github.com/FrankQDWang/StoryOS/issues/63): [GLOSSARY](../../GLOSSARY.md) and [Mailbox/retention](run-event-mailbox-snapshot-retention-and-archival-semantics.md). |
| Tool/MCP | [Specify ToolSpec, Capability, Approval, and MCP Trust Semantics](https://github.com/FrankQDWang/StoryOS/issues/48): [GLOSSARY](../../GLOSSARY.md), Context Assembly, protocol, and threat model. |
| Skill | [Specify SkillPackage and Task-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/49): [GLOSSARY](../../GLOSSARY.md), Context Assembly, and protocol. |
| Research/Memory | [Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51): [fiction, Memory, and research](fiction-memory-and-research-provenance-semantics.md). |
| Context | [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54): [Context contract](context-assembly-retrieval-and-outbound-disclosure-semantics.md), [GLOSSARY](../../GLOSSARY.md), and ADR 0005. |
| Model Gateway | [Specify ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50): [GLOSSARY](../../GLOSSARY.md), protocol, and Context Assembly. |
| MCP App | [Specify Transcript and MCP App Lifecycle Semantics](https://github.com/FrankQDWang/StoryOS/issues/53): [ADR 0002](../adr/0002-specify-transcript-and-mcp-app-lifecycle-semantics.md), [GLOSSARY](../../GLOSSARY.md), and Artifact model. |
| Deferred Eval scope | [Record the Deferred Eval Observation Boundary](https://github.com/FrankQDWang/StoryOS/issues/61): [future scope only](eval-evidence-foundation.md); no MVP delivery obligation. |
| Persistence and retention | [Specify the PostgreSQL Project Storage, Isolation, and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56) and [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64): their [storage](postgresql-project-storage-isolation-and-migration-contract.md) and [retention](run-event-mailbox-snapshot-retention-and-archival-semantics.md) contracts, with ADR 0004 and ADRs 0008–0011. |
| Trust, protocol, and architecture | [Threat-Model the StoryOS Service, Client, and External Trust Boundaries](https://github.com/FrankQDWang/StoryOS/issues/57), [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58), and [Define the Modular-Monolith and Repository Governance Boundaries](https://github.com/FrankQDWang/StoryOS/issues/59): their [threat](storyos-service-client-external-trust-boundaries-threat-model.md), [protocol](versioned-command-query-artifact-event-protocol.md), and [governance](modular-monolith-and-repository-governance-boundaries.md) contracts. |

## 11. Retained-source coverage

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
| [Protect All Authoritative Creative State](https://github.com/FrankQDWang/StoryOS/issues/7) | Authority invariant in all stages; structured truth deferred by ADR 0094. | `REL-002`; `S7-REQ-002` |
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
| [Specify ToolSpec, Capability, Approval, and MCP Trust Semantics](https://github.com/FrankQDWang/StoryOS/issues/48) | Canonical Tool/MCP owner, with the Tool Gateway and Tool Approval of ADR 0034 as ADR 0084 narrows it; S5 and later callers. | `S5-REQ-001`; `S5-REQ-002`; `S5-REQ-003`; `S5-REQ-005` |
| [Specify SkillPackage and Task-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/49) | Canonical Skill owner; S6. | `S6-REQ-001`; `S6-REQ-002`; `S6-REQ-003`; `S6-REQ-004` |
| [Specify ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50) | Canonical Model Gateway and ADR 0033 owner; S3 fake continuation/compaction, S4 Agent Plan, S5 functions, S9 policy. | `S3-REQ-003`; `S3-REQ-006`; `S4-REQ-001`; `S4-REQ-004`; `S5-REQ-003`; `S9-REQ-006` |
| [Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51) | Canonical Memory and ADR 0035 owner; S3 conversation defaults, S5 research evidence, S7 background Memory. | `S3-REQ-003`; `S5-REQ-004`; `S7-REQ-002`; `S7-REQ-003`; `S7-REQ-005`; `S7-REQ-006` |
| [Validate a Transcript-Embedded MCP Apps Host](https://github.com/FrankQDWang/StoryOS/issues/52) | Retained prototype evidence; S8 production Host. | `S8-REQ-002`; `S8-REQ-003`; `S8-REQ-004` |
| [Specify Transcript and MCP App Lifecycle Semantics](https://github.com/FrankQDWang/StoryOS/issues/53) | Canonical MCP App owner; S8. | `S8-REQ-001`; `S8-REQ-002`; `S8-REQ-003`; `S8-REQ-004` |
| [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54) | Canonical Context owner; S3 onward, with Memory distinct from active history. | `S3-REQ-003`; `S3-REQ-006`; `S4-REQ-003`; `S5-REQ-003`; `S7-REQ-004`; `S7-REQ-006`; `S7-REQ-007` |
| [Prototype the Fixed Workspace Shell and Dynamic Surface Boundary](https://github.com/FrankQDWang/StoryOS/issues/55) | Approved visual/surface evidence; S2 production workspace, S8 Apps. | `S2-REQ-009`; `S8-REQ-001` |
| [Specify the PostgreSQL Project Storage, Isolation, and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56) | Canonical persistence owner; each new conversation, continuation, and Memory family includes Scope, lifecycle, export/restore, and separate deployed-upgrade gates. | `REL-003`; `S2-REQ-005`; `S2-REQ-007`; `S3-REQ-003`; `S7-REQ-005`; `S7-REQ-007` |
| [Threat-Model the StoryOS Service, Client, and External Trust Boundaries](https://github.com/FrankQDWang/StoryOS/issues/57) | Trust invariant at each crossing, including bounded recovery and AP-22 Memory maintenance. | `REL-002`; `REL-003`; `S3-REQ-006`; `S4-REQ-004`; `S5-REQ-002`; `S7-REQ-005`; `S7-REQ-006`; `S8-REQ-002` |
| [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58) | Canonical protocol and compatibility-matrix owner; S3 conversation/Run v2, the later Tool Approval target, S7 Memory behavior and retired semantic controls. | `REL-003`; `S2-REQ-005`; `S3-REQ-003`; `S5-REQ-001`; `S7-REQ-006` |
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

## 12. Planning handoff and decision boundaries

The release correction delivers these requirements to the existing proof
owner. That owner must cover every current requirement, journey, evidence,
stage, and successor without treating the historical Stage 1 crosswalk as
current route verification. The historical crosswalk evidence is on the tag
`evidence/pre-cleanup-round-2`. It also owns the mechanism that detects a missing
source disposition, duplicate owner, unknown identifier, or stale ticket graph.

After the proof owner consumes this release revision, the `to-spec` skill refreshes the
existing [Deliver Stage 3: The Complete Fake-Model Proposal Loop](https://github.com/FrankQDWang/StoryOS/issues/361),
[Deliver Stage 4: One Authorized Real-Model Journey](https://github.com/FrankQDWang/StoryOS/issues/362),
[Deliver Stage 5: Governed Research Tools and MCP](https://github.com/FrankQDWang/StoryOS/issues/363), and
[Deliver Stage 7: Project Continuity and Inspectable Memory](https://github.com/FrankQDWang/StoryOS/issues/365)
parent specifications.
Then the `to-tickets` skill presents any changed child breakdown and actual native
blocking edges for user approval before publication. Reuse current child
owners and stable requirements, preserve Stage 1/2 historical evidence and
unaffected Stage 6/8/9 promises, and audit forward/reverse coverage. Existing
old bodies do not certify these changes. The current Stage 3 specification
owns its accepted implementation handoff and pending final release evidence.
The author released the execution of Stage 4 through Stage 7 and of
Stage 10, and on 2026-11-22 the execution of Stage 11 (ADR 0101).
Stage 8 and Stage 9 stay on EXECUTION HOLD until the author
explicitly releases their execution (ADR 0096). This contract creates no
implementation ticket or stage release.

The two conversation defaults are recorded in the release baseline. Concrete
external services, accounts, spending, and destination authorization remain
explicit prerequisites of the affected real-integration ticket. No Eval
definition, evaluator, rubric, or API decision blocks MVP planning or delivery.
Publication and execution for Stage 8 and Stage 9 remain paused.
