# Plain-Language Discovery-Writing Assistance Semantics

- Status: current
- Canonical issue: [Define Plain-Language Discovery-Writing Assistance Semantics](https://github.com/FrankQDWang/StoryOS/issues/75)
- Product goal: [GOAL.md](../../GOAL.md)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Core and Proposal owner: [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md)
- Editor continuity owner: [Web Editor Session, Synchronization, and Recovery Semantics](web-editor-session-synchronization-and-recovery-semantics.md)
- Author-command owner: [Author Command Admission](author-command-admission.md)
- Context and disclosure owner: [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- Artifact owner: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Preference and research owner: [Fiction Memory and Research Provenance Semantics](fiction-memory-and-research-provenance-semantics.md)
- Verification owner: [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md)

## 1. Purpose, authority, and owner boundary

This specification is the sole owner of author-facing intent interpretation and
plain-language discovery-writing assistance in StoryOS. It defines how the one
general, Project-scoped Agent distinguishes discussion, explanation, research,
brainstorming, limited alternatives, and requests to change prose; how it
resolves ambiguity; how it respects scope, preference, and rejection; and when
it hands candidate prose to the editor as a Proposal.

This specification does not own:

- Agent Run, plan, Step, Tool, Skill, MCP, Service, or model-routing mechanics;
- Capability, destination permission, approval, context selection, or outbound
  disclosure;
- Core commands, Proposal operations, Acceptance, Authoritative Revisions,
  Receipts, or conflict handling;
- Author Command Admission or protected-client trust;
- Web Editor Session, Local Edit Journal, projection, synchronization,
  recovery, or editor control rendering;
- Artifact classification, memory admission, preference persistence, research
  provenance, public wire formats, or deterministic gate infrastructure.

Those owner contracts remain controlling. This contract supplies the
author-facing interpretation and the exact handoff condition they consume. It
does not create a second authority path, a separate writing workflow, or a
task-specific Agent runtime.

## 2. Plain-language intent families

A current author request may contain one or more **Discovery Assistance
Intents**. The author does not need to know these names or select a mode.

| Intent family | Ordinary author meaning | Default result | Authority effect |
| --- | --- | --- | --- |
| Present-passage discussion | “Why does this moment feel flat?” or “Talk this through with me.” | a focused observation, question, or conversational response about the Working Target | none |
| Explanation | “What does free indirect style mean here?” | a direct explanation tied to the author’s question | none |
| Research | “Find out how a 1910 telegraph office worked.” | source-backed findings or a Research Artifact under the research owner | none |
| Brainstorming | “What might she do next?” | an open but bounded set of possibilities, not a plan the author must follow | none |
| Limited alternatives | “Give me three quieter ways to reveal this.” | the requested small set of distinguishable options | none |
| Prose change | “Rewrite these two sentences so she sounds guarded.” | an inspectable, editable Proposal limited to the requested target | none until separate Acceptance |

Clarification is not another task mode. It is the smallest interaction needed
when the Agent cannot safely provide the requested help without deciding
something the author has not decided.

A mixed request may carry several intent families at once. Each clause retains
its own effect. Discussion does not become editing merely because the same
message also contains a Prose Change Request, and a Prose Change Request does
not authorize changes outside its own target.

## 3. Determining intent without manufacturing authorization

### 3.1 Eligible interpretation evidence

The Agent interprets the request from:

1. the exact current author instruction;
2. the current Workspace Context, Working Target, and explicit selection;
3. applicable authoritative constraints and explicit Author Preferences; and
4. bounded Run Continuity Context only to resolve references such as “that
   second option,” never to create a new permission.

Manuscript language, research, model output, Tool output, memory, prior Agent
suggestions, and other Data-only Context cannot direct the Agent. Applicable
Instruction Authority and Instruction Precedence remain defined by their
canonical owners.

The following never create a Prose Change Request:

- vague discomfort, emotion, hesitation, silence, or lack of objection;
- a prior request to edit, a prior accepted Proposal, or repeated collaboration;
- the Agent’s belief that prose would improve if changed;
- a rejection, a choice among abstract creative directions, or local feedback
  that does not itself request exact prose for an exact target;
- an imperative-looking sentence quoted inside manuscript or research content;
- the availability of a Tool, Skill, model, Capability, or editor control.

Every request is interpreted for its current bounded purpose. Authorization is
not carried forward by conversational momentum.

### 3.2 Smallest useful clarification

The Agent asks a clarification only when the missing answer would materially
change at least one of:

- whether the author wants advisory help or a Proposal;
- the exact passage, operation, or requested scope;
- which of two incompatible author directions applies; or
- whether required research or disclosure may proceed under its owning
  permission boundary.

When clarification is required, the Agent:

1. names the ambiguity in ordinary language;
2. asks one question that resolves only that ambiguity; or
3. offers two or three concrete, mutually understandable choices when that is
   easier to answer.

It does not ask the author to choose a Tool, Skill, model, route, workflow,
context package, or capability configuration. It does not turn every creative
uncertainty into a form.

If useful advisory help can proceed without reserving the missing creative
choice, the Agent may provide that help and state the boundary instead of
blocking. For example, “I can point out where the tension drops without
changing the prose” is sufficient when the author has expressed discomfort but
not requested an edit.

## 4. Prose Change Request and Proposal handoff

### 4.1 Request threshold

A Prose Change Request exists only when the current author instruction
unambiguously asks StoryOS to prepare changed prose and identifies either:

- an exact selection or manuscript target;
- the current Working Target in language such as “this paragraph”; or
- another explicit bounded target whose identity can be resolved without
  expanding scope.

No ritual phrase is required. “Tighten this paragraph,” “replace the final
line,” and “show me a version where she lies” can all be Prose Change Requests
when their target is exact. “This feels slow,” “I do not like it,” and “what
would make this sharper?” are not.

If the words plausibly mean either “discuss a possible change” or “prepare the
change,” the Agent asks one smallest useful question before creating a
Proposal. It does not choose the higher-authority interpretation.

### 4.2 Handoff contract

An Agent, Tool, MCP server, extension, model, or other non-author producer never
directly changes Authoritative State. A valid Prose Change Request permits the
general Agent Loop to create or revise a Proposal through the Core and Proposal
owner for only the requested target and scope.

The author-facing result must:

- identify the target and make the proposed change inspectable in the editor;
- remain editable as permitted by the Proposal state machine;
- expose only the accept, reject, copy, or other controls permitted by the
  exact current Proposal state and the Web Editor control matrix;
- let the author stop or abandon the current assistance without penalty,
  explanation, or obligation to continue;
- distinguish proposed prose from current authoritative prose;
- avoid language or projection that implies Acceptance already occurred; and
- preserve discussion, explanation, research, and alternatives as advisory
  content rather than silently inserting them into the Proposal.

Stopping or abandoning assistance is not a Proposal lifecycle operation. It
does not reject, withdraw, hide, delete, accept, or otherwise settle an open
Proposal, and it causes no loss of access to other assistance.

Proposal creation allocates no Authoritative Commit. Acceptance is a later,
explicit editor decision governed by the Proposal, Author Command Admission,
and Web Editor Session contracts. An ordinary-language reply such as “looks
good” in the Agent transcript is not itself Proposal Acceptance unless a future
owner contract explicitly introduces and protects such an action path.

A durable Proposal lifecycle change occurs only through a path permitted by
the exact current Core state and Web Editor control matrix. An author-owned
`WithdrawProposal` begins from a fully displayed permitted surface as a
protected `explicit_editor_command` and requires Author Command Admission and
settlement. The exact `AgentRunStep` or `ToolCall` recorded as the current
Proposal Revision’s producer may withdraw only through its separately governed
producer cause, which must record `CurrentProducerWithdrew` and receives no
Author Command Admission.

An ordinary Proposal does not gain a withdraw control because the author or
Agent calls stopping “abandonment.” A `ProposalRecoveryConflict` retains
withdraw only where the Web Editor owner permits it, and another surface gains
no such control by analogy. Conversation alone cannot simulate Rejection or
Withdrawal, hide a Proposal, or describe an unrecorded outcome as settled.

### 4.3 Mixed requests

When a mixed request is already exact, the Agent may satisfy its clauses
together. For example:

> Tell me why the middle sentence drags, then tighten only that sentence.

The Agent may explain the pacing and produce one Proposal for the middle
sentence. The explanation does not enter the Proposal unless the author asked
for it as prose, and the edit clause does not extend to adjacent sentences.

When the edit clause or boundary is not exact, the Agent may answer the
discussion clause but creates no Proposal until the smallest useful
clarification is resolved.

## 5. Present-passage focus and explicit scope changes

The default assistance scope is the exact current Working Target and the
present creative choice. A selection narrows the target. The surrounding
Working Target Context may explain the passage but never silently expands the
target of a Prose Change Request.

The author may explicitly broaden or move the scope to another passage, a
chapter, the manuscript, or a project-wide creative question. The Agent follows
that stated scope while preserving every authority, context, disclosure,
Proposal, and budget boundary. A broad advisory request remains advisory; a
broad Prose Change Request remains Proposal-gated and must still be
inspectable.

Scope does not broaden merely because:

- a nearby passage has the same problem;
- a retrieved source mentions another chapter or project object;
- an Agent-authored outline would make the work easier;
- the previous turn discussed a broader theme; or
- the current model, Tool, or Skill can process more material.

The Agent may suggest a next creative choice, but it cannot impose an
Agent-authored outline, require an Author Plan, force a fixed sequence of
stages, or switch to an independently authoritative task runtime. The author
may ignore an option, change direction, or return to the current passage
without completing a workflow.

## 6. Preference, choice, and rejection

The Agent must distinguish the temporal and semantic scope of author feedback.

| Author signal | Current meaning | Persistence and authority effect |
| --- | --- | --- |
| Rejecting one Proposal through its editor control | the exact Proposal operations are rejected | the Proposal Rejection is retained, but no general preference follows |
| Saying “not that version” in conversation | the exact candidate is not chosen for the current work | local feedback does not fabricate Proposal Rejection or create a standing rule |
| “Use the second option as the direction for this scene” when the option is an abstract creative direction | a current choice for the identified work | no standing rule follows |
| “Use that second wording in this paragraph” when the prior option is concrete candidate prose and the target is exact | the current instruction is a new Prose Change Request; Run Continuity Context only resolves “second” | one inspectable Proposal, never direct application or a standing rule |
| “Keep this scene terse while we work on it” | a current scene-scoped instruction | no future-facing preference follows |
| “From now on, keep this scene terse” | explicit future-facing scene-scoped preference intent | no Author Preference exists until the protected owner path settles |
| “From now on, use US English for this Project” | explicit future-facing Project-scoped preference intent | no Author Preference exists until the protected owner path settles |
| “I hate this” without a stated future scope | local negative feedback requiring interpretation in context | at most a bounded Inferred Preference; never a binding rule |

Run Continuity Context may resolve which option the author means but supplies
no authority from the prior option, turn, or permission. When the current exact
instruction asks to use concrete candidate prose at an exact target, that
current instruction—not the earlier option—is the Prose Change Request.

This contract may recognize explicit future-facing Author Preference intent
and hand it to the owner-faithful protected path. It cannot persist, revise,
remove, or claim settlement of the preference. The state change begins only
from a fully displayed exact scope through an authoritative
`explicit_project_command`, receives its own Author Command Admission, and
becomes Authoritative State only when the owning Core transition succeeds and
the Admission settles `ReceiptSettled` to its typed Receipt. An
`AgentRunStep`, `ToolCall`, inference, repetition, or conversational
acknowledgement receives no Author Command Admission and cannot stand in for
that command.

When wording such as “keep this scene terse” is materially ambiguous between a
current scene-scoped instruction and a future-facing scene-scoped preference,
the Agent asks one smallest useful question about duration. An Inferred
Preference remains nonbinding regardless of repetition, confidence, retrieval,
prior use, or author silence.

Applicable current instructions and preferences follow canonical Instruction
Precedence. Within the same authority layer, the more specific applicable scope
prevails. If two equally applicable explicit directions cannot both be
honored, the Agent states the conflict and asks one smallest useful question;
it does not silently choose, average, or permanently rewrite either direction.

After rejection, the Agent respects the exact rejected candidate and does not
re-present it as though it remained selected. It may continue discussion,
research, brainstorming, or alternatives under the author’s current request.
Rejection creates no penalty, forced explanation, memory-maintenance
interruption, or new permission to prepare prose.

If the author expresses rejection in the conversation while a Proposal remains
open, the Agent respects that feedback immediately but does not claim the
Proposal state changed. When the exact current state permits Rejection, the Web
Editor Session exposes its protected control under the Proposal and Admission
contracts.

## 7. Author-facing simplicity and mandatory disclosures

The default author experience speaks in terms of the writing task: passage,
question, sources, options, proposed change, and author decision. It does not
expose or require configuration of internal Tools, Skills, routing,
capabilities, context assembly, model selection, workflow stages, or runtime
topology.

This simplicity does not hide a safety, permission, provenance, or disclosure
fact that a canonical owner requires the author to see. When an operation needs
author action, the interface presents the minimum exact fact and choice owned
by that boundary, such as the intended destination, outbound data category,
purpose, scope, or unavailable permission. It does not translate “avoid
internal configuration” into ambient access or silent disclosure.

Research remains source-backed and inspectable under the research and
provenance owner. Tool or model availability never licenses external use,
changes a Research Artifact into authority, or turns research findings into
prose without a separate Prose Change Request.

## 8. Positive scenario and authorization matrix

The following matrix is normative. “Must ask” means ask only under the stated
condition; it is not a requirement to interrupt a request that is already
exact.

| Scenario and ordinary request | Agent may do | Must ask | Agent must not do | Proposal condition |
| --- | --- | --- | --- | --- |
| Vague discomfort: “Something feels off here.” | reflect the discomfort, inspect the current passage, name a small number of plausible tensions, or offer discussion | only if the author’s answer is needed to choose between advice and an edit, or to identify the passage | rewrite, infer a desired outcome, or treat emotion as permission | none until a later unambiguous Prose Change Request |
| Explanation: “Why does this point of view feel distant?” | explain the relevant technique using the current passage | only when the referent is materially unclear | alter prose, force alternatives, or begin research not needed for the answer | none |
| Research: “Find out how a 1910 telegraph office worked.” | frame the exact question, conduct permitted source-backed research, and present inspectable findings | only for a material research-boundary ambiguity or an owning permission/disclosure choice | invent sources, expose routing controls by default, or insert findings into prose | none unless the author separately requests a prose change |
| Open brainstorming: “What could she do next?” | offer a bounded spread of possibilities and follow the author’s interest | only when no useful possibility can be offered without reserving an unstated constraint | impose an outline, pick canon, or turn an idea into a standing plan | none |
| Limited alternatives: “Give me three quieter ways to reveal this.” | provide three distinguishable, concise alternatives | only for a missing constraint that would make the alternatives misleading | create more ceremony, select for the author, or change the manuscript | none when alternatives are offered; a later exact instruction such as “use the second wording in this paragraph” is a new Prose Change Request |
| Explicit prose change: “Rewrite these two sentences so she sounds guarded.” | prepare the bounded change, hand it to the editor, and let the author stop assistance at any time | only if the target or requested scope cannot be resolved exactly | directly mutate authoritative prose, broaden the edit, imply Acceptance, or turn stopping assistance into Rejection or Withdrawal | one inspectable Proposal for the exact requested work |
| Mixed discussion and edit: “Why is this slow? Tighten only the middle sentence.” | explain the pacing and prepare the exact requested sentence change | only if the edit clause or its target remains ambiguous | let the discussion authorize more editing or put advisory explanation into prose | one Proposal for only the explicit change clause |
| Continue after recorded rejection: after using the editor’s Reject control, “Let’s keep exploring.” | honor the recorded Proposal Rejection and continue with new discussion or options | only if “keep exploring” leaves a material direction unresolved | reopen or repeat the rejected candidate as selected, globalize the rejection, or demand justification | none until a new Prose Change Request |
| Preference conflict: “Keep it spare here,” while an equally specific current instruction requires elaboration | identify the conflict and preserve both directions until resolved | one question choosing which equally applicable direction governs this work | silently choose, average the instructions, or persist a new global preference | only after the applicable direction and a Prose Change Request are clear |
| Current passage to global scope: “Now assess this problem across the whole manuscript.” | switch the advisory analysis to the explicitly named manuscript scope | only if “this problem” or the intended objects cannot be resolved | stay artificially local, impose a global outline, or treat analysis as a bulk edit | none unless the author explicitly requests inspectable prose changes |

## 9. Canonical owner interfaces

| Owner | This contract supplies | This contract consumes and does not redefine |
| --- | --- | --- |
| Core and Proposal state machine | the existence, target, and bounded scope of a Prose Change Request | Proposal identity, revisions, operations, conditions, Acceptance, Rejection, Withdrawal, conflicts, Receipts, and authority effects |
| Author Command Admission | no Agent-generated author-command claim | the rule that Proposal decisions use their exact protected `explicit_editor_command`, while an Author Preference state change uses its fully displayed exact-scope `explicit_project_command`, each with its own Admission and settlement |
| Web Editor Session | the author-facing need to inspect or edit a Proposal, decide among currently permitted actions, or stop assistance without changing Proposal state | which exact controls the current surface exposes, plus editor rendering, journal, projection, synchronization, recovery, acknowledgement, and convergence |
| Context and disclosure | one exact current purpose, Working Target, and author instruction | context qualification, limits, destination identity, permission, manifest, disclosure, routing, and degradation |
| Artifact domain model | the author-facing distinction among advice, research, options, and proposed prose | Artifact classification, Revision identity, provenance, lifecycle, and the binary authority boundary |
| Memory, preference, and research | explicit feedback scope and the distinction between current choice and future-facing preference | Author Preference persistence, Inferred Preference non-authority, Research Artifact and claim provenance, suppression, and retrieval |
| Deterministic verification | the normative scenario matrix and authority invariants below | executable gate organization, fixtures, fault evidence, and release verdicts |

No interface permits this contract to invoke another owner’s mechanics by
analogy. A Proposal-like chat message is not a Proposal, a positive response is
not Acceptance, a transcript is not Authoritative State, and an internal route
selection is not author authorization.

## 10. Verification obligations

Implementations of this contract require public-boundary integration evidence,
not model-prompt inspection alone. The deterministic verification owner must be
able to exercise the matrix with fixed Project Scope, Working Target, current
instructions, preference facts, and editor state, then assert structural
outcomes without requiring exact generated prose.

At minimum, verification must prove:

1. discussion, explanation, research, brainstorming, and limited alternatives
   create no Proposal, Authoritative Revision, Authoritative Commit, or
   fabricated editor decision;
2. vague discomfort and materially ambiguous mixed requests create no Proposal
   before the smallest useful clarification is resolved;
3. an exact Prose Change Request produces an inspectable Proposal for only the
   requested target and creates no authority effect;
4. Proposal Acceptance remains a separate explicit editor command and no Agent
   response, stream completion, retry, or recovery path fabricates it;
5. a mixed exact request can produce advisory content plus only the explicitly
   requested Proposal;
6. stopping or abandoning assistance alone creates no Rejection, Withdrawal,
   Acceptance, closure, or hidden editor state, while author and current
   producer withdrawal exercise only their exact permitted causes and surfaces;
7. choosing an abstract option remains a current creative choice, while a
   current exact instruction to use concrete candidate prose at an exact target
   is a new Prose Change Request whose result is a Proposal;
8. a current scene instruction, future-facing scene preference intent,
   Project-scoped preference intent, and Inferred Preference remain
   distinguishable, and no Author Preference state change appears before its
   protected `explicit_project_command`, successful Core transition, and
   `ReceiptSettled` Admission settlement;
9. prior permission, prior Acceptance, prior candidate prose, repetition,
   emotion, silence, and conversational acknowledgement never authorize a new
   Proposal, expand its scope, or persist an Author Preference;
10. current-passage default scope and an explicit broader scope both preserve
   Working Target, context, Proposal, and disclosure boundaries;
11. the ordinary success path exposes no Tool, Skill, route, capability, or
   workflow configuration; and
12. required safety, permission, provenance, and disclosure choices remain
    visible and exact despite that default simplicity.

Fault cases must fail closed with an inspectable advisory response,
clarification need, refusal, or owning-boundary problem. They must not be
recovered by silently choosing a higher-authority intent.

## 11. Normative invariants

1. StoryOS has one general Project-scoped Agent Loop; intent families do not
   create separate workflow runtimes.
2. The current author instruction is interpreted narrowly enough that
   discussion, research, explanation, brainstorming, alternatives, and prose
   change remain distinguishable.
3. Ambiguity never resolves toward more authority, more scope, or more outbound
   disclosure.
4. A Prose Change Request authorizes only an inspectable Proposal for its exact
   target; it never authorizes Acceptance or directly changes Authoritative
   State.
5. The default scope is the current Working Target and present creative choice;
   only explicit author direction broadens it.
6. Rejection, current choice, scoped instruction, Inferred Preference, and
   explicit Author Preference remain distinct.
7. Internal execution configuration stays out of the ordinary writing
   conversation, while mandatory safety and disclosure facts remain visible.
8. No Agent-authored outline, fixed workflow, Tool, Skill, model, memory, or
   prior permission can become creative authority by conversational momentum.
9. Stopping or abandoning assistance changes no Proposal lifecycle; Rejection
   and Withdrawal use only their exact owner-permitted current paths and causes.
10. A prior option supplies no authorization: only a current exact instruction
    can turn referenced concrete candidate prose into a Prose Change Request.
11. Recognizing future-facing preference intent is not persistence; an Author
    Preference changes only through its protected exact-scope command,
    successful Core transition, and `ReceiptSettled` Admission settlement.
