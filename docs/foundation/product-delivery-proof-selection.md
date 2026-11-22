# Product Delivery Proof Selection

- Owner: [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60).
- Contract revision: `proof-responses-memory-2026-09-15-v1`.
- Status: current proof contract; product evidence remains unrun until implementation.
- Parent: [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md).

## 1. Accepted inputs and minimum work

The accepted release revision is `release-responses-memory-2026-09-14-v1`.
[Align release stages with Responses and background Memory](https://github.com/FrankQDWang/StoryOS/pull/684)
first accepted the two sources, at `cca8e646f137bccc27414014470211451a1ac474`.
The current accepted binding is later. The author, who owns the release
contract, approved the specification of Stage 11 on 2026-11-22
([Deliver Stage 11: Character Intents](https://github.com/FrankQDWang/StoryOS/issues/1786),
[ADR 0101](../adr/0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md)).
[Add the Stage 11 Requirements to the Release Sources and Bind the Proof Documents to Them](https://github.com/FrankQDWang/StoryOS/pull/1796)
accepted the two sources with the rows of Stage 11 at
`f4a072c6de1d3901dd6e2e328193d6fee562239e`, tree
`49bf6f0cb929f066a5b483e53eaf491f6908c943`.
That commit is the exact source snapshot
consumed by both proof documents, not the release owner's earlier Claim baseline.

| Accepted source | UTF-8/LF SHA-256 |
| --- | --- |
| [Editor-first release](ai-independent-editor-first-release-baseline-and-handoff-criteria.md) | `58caa4df6bb3bc17ec5aa16fb51b939579fe06f685c1cf2bfb5dfd7e30066937` |
| [Product continuation](storyos-product-delivery-continuation.md) | `611b2c9423faa23bcc15ee82a0005d52888683ae953f6c43d859b36fae66bfbe` |

The continuation source owns the exact 71-source disposition inventory:
original Issues 2 through 65, 67 through 70, and 75 through 77. Its additional
completed governance sources remain implementation evidence. The combined
release sources define 127 table IDs and eleven journeys. These counts describe
the accepted source set; they do not replace an exact-ID and semantic review.
A source change needs its original owner's review and a new accepted binding.
The two Eval sources remain recorded as deferred outside MVP. They require
no MVP implementation or proof. Source retention is not delivery approval.

Use the smallest implementation that satisfies a named accepted requirement.
Reuse existing Core, Gateway, Context, Worker, Journal, PostgreSQL, and test
boundaries. Add tests at the changed behavior; do not build future capability
infrastructure or a general proof/graph platform as a planning prerequisite.
This correction changes only the two proof documents. Release criteria remain
with the accepted release owner.
Existing repository checks and the independent source/graph review below
are the verification mechanism.

A child ticket selects its own observable cases and applicable durable cuts.
It does not need the whole stage to pass before that child can close. The
stage's final evidence owner combines all required cases and runs the full
author journey. Planning coverage, implemented behavior, and stage release
remain separate results.

## 2. Positive fixtures and independent expectations

These are proof inputs, not production APIs or a second implementation.
Reuse the parent's schedules, fault points, and safe bundle shapes. Each
fixture records exact Scope, source versions, configuration, expected facts,
and introduced durable records. Tests compare those facts at the public
boundary and in authoritative storage.

| Fixture | Required positive cases | Oracle | Independent expectation |
| --- | --- | --- | --- |
| `FX-PRODUCTION-EDITOR` | Production Tiptap/ProseMirror and fixed workspace; at least two writable Chapters; stable Block coordinates; old prose and unsettled Journal; hydrate/edit/settle/reload. | `ORC-PRODUCTION-EDITOR` | The accepted package/adoption and browser workspace match; admitted Chapter changes install the correct base under one Project writer; old input remains recoverable; programmatic projection creates no author intent; canonical and visible prose agree after settlement/reload. |
| `FX-PROPOSAL-INTERACTION` | Inline/Block scope; stable Operations; multiple non-overlapping Proposals; input pause; editable candidate; exact-version optional comparison with coherent replacement spans; Undo Accept/reopen/fresh-Acceptance redo. | `ORC-PROPOSAL-INTERACTION` | Each interaction matches the Core state machine; adjacent fragmented matches normalize without changing Operation identity; input pause preserves author work; only an explicit current Acceptance changes authority; safe Proposal lineage reopens the exact Proposal; lineage drift can derive a new Proposal without blocking safe compensation; authoritative Head drift requires a ReversalProposal or Unavailable; redo uses fresh Acceptance. |
| `FX-PRODUCT-5` | Exact Tool/MCP contract and Project use; native function/result correlation; complete business batch; research sources/gaps/partial results; cancellation and unknown outcomes. | `ORC-PRODUCT-5` | StoryOS effects match their own admission. Host facts and unknowns stay separate. No ambient intake, invalid batch execution, duplicate accounting, direct creative write, or blind retry. Section 2.3 defines the exact cases. |
| `FX-PRODUCT-6` | Instruction-only Skills of the deployment, one installation scope, selection and the unique name, immutable package snapshots, progressive resources, precedence/composition, and a change of the Skills of the deployment. ADR 0089 deferred Tool/script Skills, optional extensions, outcome obligations, revocation, and each Skill that an author provides. | `ORC-PRODUCT-6` | The selected exact package remains recorded; loading and composition grant no authority; an active Run never switches snapshots after a change of the Skills of the deployment. |
| `FX-PRODUCT-7` | One Project Agent across conversations; the Project Instruction; Host default settings; background capture of Conversations and Chapters and consolidation; Memory Entries, Observations, and the Memory Index; selective recall; manuscript search and read; the vector index; recovery. ADR 0094 deferred the fiction and preference records, restrictions, and cleanup. | `ORC-PRODUCT-7` | Atomic consolidation, current read permission, bounded records/effects, and the protection of an author entry follow ADR 0094 and section 2.4. Generated Memory grants no authority. Ordinary corrections do not reset continuation. |
| `FX-PRODUCT-8` | Character, relationship, timeline, and research views; immutable resources/View revisions; disposable sandbox Instances; negotiation/limits/revocation; Prepared Receipt and terminal fallback; admitted persisted actions and same-Instance responses. | `ORC-PRODUCT-8` | Opening/replaying a view never repeats a ToolCall; semantic actions have fresh applicable Admission; the sandbox cannot grant authority, call Tools directly, or send a response to another Instance. |
| `FX-PRODUCT-9` | Optional RunPlan with RunPlan Revisions and PlanSteps; durable RunStep, wait, steering, and cancellation; bounded Subruns with narrowed inputs and budget reservations; Mailbox/follow-up/interrupt/join/backpressure/Seal; parent-child recovery; proactive grants/misfires; guardrails and explicit model policy. | `ORC-PRODUCT-9` | Reservation, child creation, effects, and settlement follow the owner transaction; duplicates/late results never reopen work; proactive work needs its recorded grant; configured routing does not imply a second Provider. |
| `FX-PRODUCT-10` | The default writing rules in the Host instructions of a tool-loop request, with a Project Instruction that has a sample passage; the SkillPackages of the StoryOS distribution in the Skill directory of the release package; the marks on the new words of a Proposal candidate, with the table of the expected marks for each test input, the two cases with no mark, an author edit of a candidate, and an IME composition (ADR 0100). Stage 10 adds no record family. | `ORC-PRODUCT-10` | The request has the exact rule text, and the Registration has the mapping revision of that text. Each package passes the rules of the Skill directory, and the Skill picker lists it. The marks agree with the table of the expected marks, and they change no document, no record of the Local Edit Journal, no command, and no copied text. No evidence asserts that the model obeys a rule. |
| `FX-PRODUCT-11` | One Character File in Agent Memory with its two parts: the part of the author and the part of the background work. The write rule of each part, the refusal of a change of the part of the background work by the author, and the delete marks. The Intent Pass with its request, its start rules, and its result. One Intent Pass whose first reply the Host refuses and whose second reply is correct. A Run of the author that pinned its Memory revision before a change of the Character File, and a Run of the author that pins its Memory revision after that change. The Memory Index of a capture. The section of the characters on the settings page with the search, the file, the intent, and the questions (ADR 0101). | `ORC-PRODUCT-11` | Background work changes no text of the part of the author. A Project has at most one part of each kind for each character. The two parts of one Character File are the only source of knowledge about the character in each request of an Intent Pass. After a refused reply, the second request also has the refusal of the Host as a Tool result, and a correct second call records the result. A result changes no text of the file. A Run of the author that pinned its Memory revision before a change of the Character File keeps the intent of that revision in each RunStep. A Run of the author that pins its Memory revision after the change gets no intent of that character until a new result. No capture and no pass gets an intent, and no Run gets a question. The settings page has no control that edits a file, that starts a pass, or that answers a question. No evidence asserts that the model obeys a rule of its instructions. |

The production editor and old-Journal cases explicitly reuse the complete
`ACK_LOSS_AUTHOR_COMMAND_PROFILE` in the parent, including both the admitted
pre-Core cut and the outcome-response-before-Journal cut. The applicable
stage maps retain its exact gates, fixtures, schedules, oracles, and bundles.

The new fixtures use the existing semantic cuts for Scope, Core commit,
Proposal decision/compensation, manifest commit, dispatch/unknown, Fence,
Mailbox Seal, lifecycle invalidation, and restore visibility. Apply each cut
to the exact introduced record or operation named by the ticket. If an
implementation introduces a durable boundary that none of those cuts
describes, its original semantic owner must name that boundary before the
ticket can claim its proof; this does not authorize a new general scheduler.

A real Tool/MCP integration has
`EV-INT` for StoryOS integration facts and, where useful, `EV-RDA` for
external observations. A fake or successful catalog parse cannot replace it.

Each added record family enters the existing migration, Archive, replay,
retention, deletion, and physical-restore fixtures at its delivery stage.
The complete Stage 2 AI-disabled journey remains a regression. Its old
restore report cannot certify new records. The accepted daily base backup,
continuous WAL, separate failure domain, RPO at most 15 minutes, and RTO at
most two hours remain hard requirements; no HA cluster or automatic failover
is added.

### 2.1 Current proof boundary and stage allocation

The cases below are mandatory refinements of the existing fixtures, oracles,
and stage maps. They create no product API, new gate, scheduler, or proof
runtime. Implement each case at the public Host/Application/Core, Gateway,
production Web Client, and PostgreSQL boundaries that own the observed fact.
Use fixed synthetic inputs, independently stated complete expected records,
and controlled interleavings. A screen, success response, or generated report
alone cannot prove durable settlement. A deterministic script supplies external
outcomes; it does not prescribe generated prose or claim model understanding.

| Case group | Release rows refined | Gates and oracle | Required cuts and schedules | Bundles |
| --- | --- | --- | --- | --- |
| Conversation and settings admission | `S3-REQ-003`, `S3-EVD-002`; Stage 7 settings also refine `S7-REQ-006`, `S7-EVD-002`. | `DVG-01`, `DVG-02`, `DVG-07`, `DVG-11`; `ORC-CONTRACT`, `ORC-SCOPE`, `ORC-RECOVERY-ATOMICITY`. | `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-SCOPE-BEFORE-QUERY`; `SCH-NORMAL/CRASH/REORDER/SCOPE`. | `B-CONTRACT`, `B-CONTEXT`, `B-RECOVERY`, `B-SCOPE`. |
| Typed output, continuation, and active compaction | `S3-REQ-006`, `S3-EVD-006`, `S4-REQ-002`, `S4-REQ-003`, `S4-EVD-002`. | `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-09`, `DVG-11`; `ORC-CONTEXT-DISCLOSURE`, `ORC-DISPATCH-DISCLOSURE`, `ORC-RUN-FINALIZATION`. | `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`; `SCH-NORMAL/CRASH/REORDER/FENCE/LIFECYCLE/SCOPE`. | `B-FAKE`, `B-CONTEXT`, `B-RECOVERY`, `B-REPLAY`. |
| Expiry, unknown create, and cancellation | `S3-REQ-006`, `S3-EVD-005`, `S4-REQ-004`, `S4-EVD-003`. | `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11`; `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`. | `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-LATE-RESULT`; `SCH-UNKNOWN/CRASH/FENCE/REORDER`. | `B-FAKE`, `B-CONTEXT`, `B-RECOVERY`. |
| Exact real-route qualification | `S4-REQ-001`, `S4-EVD-001`. | `DVG-01`, `DVG-05`, `DVG-11`; `ORC-CONTRACT`, `ORC-DISPATCH-DISCLOSURE`. | `CFP-CONTRACT-DRIFT`, `CFP-DISPATCH-BEFORE-CLAIM`; `SCH-DRIFT/NORMAL/SCOPE`. | `B-CONTRACT`, `B-CONTEXT`, `B-REAL-ADVISORY`. |
| Business ToolCall | `S5-REQ-001`, `S5-REQ-003`, `S5-REQ-005`, `S5-EVD-001`, `S5-EVD-002`, `S5-EVD-003`. | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11`; `ORC-PRODUCT-5`. | `CFP-MODEL-BEFORE-DECISION`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-LATE-RESULT`; `SCH-NORMAL/SCOPE/DRIFT/UNKNOWN/FENCE`. | `B-CONTEXT`, `B-FAKE`, `B-RECOVERY`, `B-REAL-ADVISORY`. |
| Memory capture and consolidation (the publication fault points apply to the commit of a consolidation pass) | `S7-REQ-005`, `S7-REQ-006`, `S7-EVD-002`. | `DVG-02`, `DVG-04`, `DVG-07`, `DVG-11`; `ORC-PRODUCT-7`, `ORC-RECOVERY-ATOMICITY`. | `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-SCOPE-BEFORE-QUERY`; `SCH-NORMAL/CRASH/FENCE/REORDER/SCOPE`. | `B-CONTEXT`, `B-RECOVERY`, `B-REPLAY`, `B-SCOPE`. |
| Manuscript retrieval and Memory portability (ADR 0094 withdrew the restriction and cleanup cases, and their byte-acquisition and cleanup fault points have no subject) | `S7-REQ-007`, `S7-EVD-003`. | `DVG-02`, `DVG-04`, `DVG-07`, `DVG-09`, `DVG-10`, `DVG-11`; `ORC-PRODUCT-7`, `ORC-RESTORE-LIFECYCLE`. | `CFP-MEMORY-BEFORE-BYTE-ACQUISITION`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-RESTORE-STAGING`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-AFTER-VISIBILITY`; `SCH-LIFECYCLE/CRASH/REORDER/RESTORE/SCOPE`. | `B-CONTEXT`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`. |

Each refined row adds its case group's exact selection to its existing cells;
none replaces the original row or removes an earlier obligation. A shared
case may support several rows, but each acceptance responsibility has one
implementation owner. Stage 3 uses `FX-FAKE-MODEL` and no real destination.
Stage 4 adds `FX-REAL-MODEL-ADVISORY` observations to the same deterministic
proof. Stage 5 adds `FX-PRODUCT-5`; Stage 7 adds `FX-PRODUCT-7`. All groups
also use the applicable existing Scope, Context, contract, recovery, replay,
and restore fixtures from their stage map. `EV-CP`, `EV-IT`, and `EV-INT`
prove the Host cases; physical restore uses `EV-PRD`. Real qualification uses
actual integration plus `EV-RDA`, never a fake capability claim. `PASS-POS`
requires the stated positive facts; refusal, unknown, and hold use their exact
existing dispositions. Every missing required case is `BLOCK-ALL`.

### 2.2 Responses and conversation cases

1. Admit `createAgentRun` v2 with explicit new/existing conversation selection.
   Verify atomic Conversation/settings/Run creation, both initial settings
   enabled, exact retry identities, changed-digest rejection, inaccessible
   existing identity refusal, and no partial records after failed admission.
   Query the captured settings revision and exact Attempt evidence. A new
   conversation has its own continuation; the same conversation can continue
   across terminal Runs without reopening them. Substitute Scope, conversation,
   destination/account, Registration, Adapter mapping, and use/compatibility
   bindings; none can borrow a foreign or stale continuation association.
2. Script ordered native items, roles/phases, item IDs, function-call IDs,
   arguments/results, refusals, declared summaries, unrequested item types, usage, and
   opaque replay references. Test partial, complete, incomplete, failed,
   cancelled, unknown, invalid, and unselected candidates. Partial arguments
   execute nothing. Exactly one whole validated durable Decision can expose
   its continuation. Restart before and after that boundary recovers its
   original identity, not a second answer. Rejected output cannot enter normal
   continuation through a repair request or a renamed partial result.
3. Append ordinary corrections as ordered Messages/Steering Input. Compare the
   next request's current instructions and exact Working Target against its
   Snapshot. Exercise valid incremental input, full input when required by the
   mapping, and transport continuation change without changing conversation
   identity. No ordinary correction creates a semantic registry or automatic
   chain reset. A current required source that is unavailable blocks or takes
   only its declared degradation; old history is not silently rewritten.
4. Compact between calls in an active Run without requiring terminality/Seal.
   Verify immutable known inputs/prior projections, producer/model/prompt or
   native mapping, output/reference, and loss/unknown facts. Preserve source
   history and earlier request evidence. Stage the result, crash, install only
   for a later request, and reject stale or restricted input. Each extra model
   submission has its own admission, manifest, Attempt, and usage. Summaries
   do not prove semantic equivalence or replace exact required current input.
5. Separate confirmed reference expiry from unknown create. For expiry, prove
   all prior submissions settled, same permitted destination, current inputs,
   authority and budget, fresh assembly, and a new RunStep/Invocation when the
   Effective Model Context changes. Missing required input or authority blocks
   automatic rebuild. Preserve conversation identity and historical evidence.
6. For unknown create, first attempt supported original-result retrieval when
   its retained reference permits it. Retrieval is separately admitted and
   does not resubmit the original work. If still unknown, allow at most one
   additional Attempt for the same Invocation/request/route only with current
   authority, budget for both, and no unresolved Tool effect. Persist
   predecessor fence and allowance consumption before dispatch. Restart at that
   cut and retry recovery: a second successor is forbidden. Keep the original
   unknown outcome and worst-case reservation until attributable settlement.
7. Persist cancellation before best-effort abort. Cancellation, spent allowance,
   insufficient budget/authority, and unresolved effects each prevent the
   automatic successor. A late predecessor/cancelled result may settle evidence
   and usage only: no ToolCall, new Decision, replacement answer, normal context,
   or continuation advance. A successor's success cannot settle its predecessor.
8. For the exact Agent Plan endpoint/model/account and entitlement, record
   separately validated continuation, streaming, implicit/explicit cache,
   structured/native text output, native/Host compaction,
   retrieval, and abort, including required combinations and gaps. Test current
   instructions/output requirements with optional cache disabled, incompatible,
   and missed. No cache hit, native compaction, resumed stream, create idempotency,
   savings, or remote stop is inferred from a setting or response identifier.
   Unknown required behavior blocks that real route; optional absence follows
   the accepted profile. Host facts, Provider reports, opaque references, and
   missing evidence remain distinct, including reported/estimated/unknown usage.

### 2.3 Tool and research cases

[ADR 0084](../adr/0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md) withdrew the Provider-hosted Operation. A model request
carries no Provider-side tool. The earlier hosted-set admission, hosted Tool
Approval, and qualified hosted journey cases are withdrawn, and Stage 5 waived
them ([#363](https://github.com/FrankQDWang/StoryOS/issues/363)). Tool
Approval keeps one target, the StoryOS ToolCall, for a later Stage that adds
an effect outside the Project grant.

`ORC-PRODUCT-5` retains the research/Proposal cases and also requires:

- Exercise unchanged work under the current Run Grant without another prompt.
  StoryOS ToolCalls retain their own effects and one-shot rules.
- Validate the whole requested business-tool batch before deriving ToolCalls;
  one invalid member rejects it. A native item of a type that the request did
  not ask for makes the Agent Decision invalid. Return each settled Tool result
  with its exact native call correlation, without executing it again. Later
  StoryOS-controlled submissions re-enter Context Assembly with returned
  content as data.
- Keep Host observations separate from unknown usage. A completed outer
  Attempt with a fully validated Decision may deliver complete sourced
  sub-results and an unmet research objective. An incomplete stream or
  cancelled/fenced result cannot use that partial-deliverable path.
- Interrupt after dispatch; retain unknown effects and reservations, fence new
  work, reconcile through admitted retrieval, and refuse automatic rerun while
  effects remain unknown. A local timeout or expiry proves no remote stop.
  Exercise the actual real Tool/MCP journey separately from scripted failure
  proof. Missing required live evidence blocks Stage 5.

### 2.4 Memory cases

`ORC-PRODUCT-7` retains the Project Instruction proof and adds the following
records/effects cases of ADR 0094 and ADR 0095. ADR 0094 withdrew these cases
of the earlier form: the fiction and preference proof, a change of the settings
by the author, the Memory Note, the published document set, the covered-copy
restriction, and the payload cleanup.

1. A save of the Project Instruction makes one immutable Revision. A Run binds
   the current Revision at its creation and keeps it. A save above the Context
   item limit is refused. The absence of a Project Instruction does not block
   assistance.
2. The Host writes the default Conversation Memory Settings, and no author
   control changes them. A Run gets the Memory Index and the Memory tools only
   when the settings permit use. A capture reads a Project Conversation only
   when the settings permit contribution. Stages 3 through 6 record settings
   but perform no Memory execution.
3. On the request of the author, the Agent creates, changes, or deletes one
   Memory Entry with a Host Tool. The change commits with its ToolCall, and the
   earlier version stays. A duplicate delivery makes no second entry. A wrong
   Scope is refused.
4. A capture reads the settled messages of a quiet Project Conversation or the
   authoritative text of a quiet changed Chapter. It is one bounded tool loop
   with read-only tools. Pin its input and the model and prompt versions. No
   live-buffer read and no default whole transcript or Memory dump is allowed.
   The Host limits the observations. A capture writes no entry, starts no web
   search, and changes no manuscript. A duplicate delivery makes no duplicate
   observation.
5. Compete two consolidation passes; crash before and after the commit. The
   operations of one pass apply as one settlement, and a reader never sees a
   part of a pass. A pass whose input is older than the current entries
   applies nothing; a retry reads the current entries again. The Host refuses
   each background operation on an entry of the author and records only a
   conflict flag. The pruning obeys the Host limits.
6. A delete by the author removes the entry from the Memory Index, the tools,
   and the page, and leaves a mark. A later pass does not create that entry
   again. The versions stay.
7. Inspect an empty Memory and an unavailable Memory separately. The Memory
   Index is bounded and holds no entry text. A read returns the current
   version of an entry in the Project Scope and records the time of the read.
   Ordinary corrections and source edits do not reset Provider continuation.
   Retired semantic controls have no aliases.
8. The keyword search and the read return the current authoritative text in
   the bounds of a Tool result. The hybrid search returns no vector hit whose
   text changed after its vector was made. Without an embedding configuration
   or the vector extension, the hybrid search is not exposed and the other
   tools work. An embedding request records its grant scope and its
   disclosure; a request outside the scope is refused. The vector index builds
   again from the Chapters with no model request of a Run and no loss of
   author content.
9. Export/restore the Project Instruction Revisions, the Memory Entries with
   their versions, the delete marks, the conflict flags, the observations that
   wait, the settings, and the outcomes. Discarded vector indexes rebuild.
   Include new families in their owning migration and physical recovery proof;
   current activation mismatch requires a separate authorized upgrade path.
10. Inject hostile text, wrong Scope, a forged origin, forbidden writes,
    secrets, oversized output, disabled contribution, unavailable inputs, and
    duplicate/stale/partial passes. Inspect actual Host reads, requests,
    records, and effects. Background work can write only observations, conflict
    flags, and the Memory Entries that the author did not ask for, not Skills,
    ToolSpecs, policy, credentials, the Project Instruction, or Authoritative
    State. Do not assert semantic truth, precise forgetting, or inevitable
    model obedience. The full editor remains usable with failed or unavailable
    Memory; background work does not block manual writing.

## 3. Additional first-four-stage coverage

Each row below completes the parent's crosswalk. Its passing disposition is
`PASS-POS` for the named positive facts and the owner-defined negative/hold
disposition for those test cases. Every row uses `BLOCK-ALL`.

| ID | Proven fact and owner | Gates | Evidence | Fixture, fault, schedule, oracle | Bundle | Block |
| --- | --- | --- | --- | --- | --- | --- |
| `REL-007` | Exact source, stage, acceptance owner, and evidence coverage; `OWN-REL`, `OWN-DVG`, `OWN-GOV`. | `DVG-01`, `DVG-13` | `EC-01`; `EV-CP/SE` | `FX-CONTRACT-R1`, `FX-HANDOFF`; `CFP-CONTRACT-DRIFT`; `SCH-DRIFT`; `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS` | `B-CONTRACT`, `B-HANDOFF` | `BLOCK-ALL`; missing or duplicate acceptance responsibility, source omission, unapproved graph, or prototype substitution blocks planning handoff. |
| `S2-REQ-009` | Production editor, workspace, stable Blocks, coordinates, and old-data preservation; `OWN-WEB`, `OWN-CORE`, `OWN-PG`, `OWN-PROTO`, `OWN-GOV`. | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-07`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-08` | `EC-01/02/03/04/05`; `EV-CP/IT/INT/PRD/SE` | `FX-PRODUCTION-EDITOR`, `FX-JOURNAL-GROUP`, `FX-RESTORE-LIFECYCLE`; `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`; `SCH-NORMAL/CRASH/RESTORE/SCOPE`; `ORC-PRODUCTION-EDITOR`, `ORC-RESTORE-LIFECYCLE` | `B-CONTRACT`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-RESTORE` | `BLOCK-ALL`; prototype/textarea-only evidence or lost old prose/Journal blocks. |
| `S2-EVD-009` | Exact adoption/package, production browser visual, Block/coordinate, and prior-data evidence; `OWN-WEB`, `OWN-CORE`, `OWN-PG`, `OWN-GOV`. | `DVG-01`, `DVG-03`, `DVG-07`, `DVG-10`, `DVG-13`, `DVG-02`, `DVG-08`, `DVG-11` | `EC-01/02/03/04`; `EV-CP/IT/INT/PRD/SE` | `FX-PRODUCTION-EDITOR`, `FX-HANDOFF`; `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`; `SCH-DRIFT/NORMAL/CRASH`; `ORC-PRODUCTION-EDITOR`, `ORC-CROSSWALK-COMPLETENESS` | `B-CONTRACT`, `B-EDITOR`, `B-RECOVERY`, `B-RESTORE`, `B-HANDOFF` | `BLOCK-ALL`. |
| `S3-REQ-008` | Complete Proposal interactions and normalized comparison spans; `OWN-CORE`, `OWN-WEB`, `OWN-ADM`, `OWN-AGENT`. | `DVG-03`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-11` | `EC-02/03/04/07`; `EV-CP/IT/INT/SE` | `FX-PROPOSAL-INTERACTION`, `FX-CORE-PROPOSAL`, `FX-FAKE-MODEL`; `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-UNDO-BEFORE-SETTLEMENT`; `SCH-NORMAL/CRASH/FENCE/REPLAY`; `ORC-PROPOSAL-INTERACTION`, `ORC-ATOMIC-AUTHORITY`, `ORC-RUN-FINALIZATION` | `B-CORE`, `B-EDITOR`, `B-FAKE`, `B-RECOVERY`, `B-REPLAY` | `BLOCK-ALL`; an atomic Acceptance test alone does not prove the interaction set. |
| `S3-EVD-008` | Attributable proof for every interaction, exact comparison, compensation, reopen, and fresh redo; `OWN-CORE`, `OWN-WEB`, `OWN-DVG`. | `DVG-03`, `DVG-07`, `DVG-09`, `DVG-13` | `EC-01/02/03/04/07`; `EV-CP/IT/INT/SE` | `FX-PROPOSAL-INTERACTION`, `FX-HANDOFF`; `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-UNDO-BEFORE-SETTLEMENT`; `SCH-NORMAL/CRASH/REPLAY`; `ORC-PROPOSAL-INTERACTION`, `ORC-CROSSWALK-COMPLETENESS` | `B-CORE`, `B-EDITOR`, `B-RECOVERY`, `B-HANDOFF` | `BLOCK-ALL`. |

## 4. Continuation stage maps

Each map contains the complete Stage 2 AI-disabled regression, the positive
Stage 3 Proposal and Stage 4 model boundaries, and every positive capability
profile delivered through the selected stage. Run the regression with those
capabilities disabled; run their positive cases separately. Do not inherit
the Stage 3/4 absent-capability conditions as positive implementation proof.

The following exclusion sets apply only to `FX-ABSENT-EXECUTION` and
`B-ABSENT` in the selected continuation map. All stages still test prohibited
authority, Scope, permission, and disclosure paths.

| Selected stage | Capabilities still absent |
| --- | --- |
| Stage 5 | Skill, Memory, embedding, MCP App, and Subrun execution. |
| Stage 6 | Memory, embedding, MCP App, and Subrun execution. |
| Stage 7 | MCP App, and Subrun execution. |
| Stage 8 | Subrun execution. |
| Stage 9 | No remaining feature family in this MVP route; unauthorized operations remain refused. |
| Stage 10 | MCP App, and Subrun execution. Stage 10 comes after Stage 7 and before Stage 8 (ADR 0096). |
| Stage 11 | MCP App, and Subrun execution. Stage 11 comes after Stage 10 (ADR 0101). |

The maps use the parent's finite token grammar. All named positive fixtures
and oracles are defined in section 2. `EV-SR` is never a mandatory input.

| Selector | Stage | Gate set | Evidence classes/layers | Fixtures | Contract Fault Points | Schedules | Oracles | Mandatory bundle set |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SMAP-STAGE-5` | Stage 5 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5` | `B-S5-MANDATORY-SET` |
| `SMAP-STAGE-6` | Stage 6 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6` | `B-S6-MANDATORY-SET` |
| `SMAP-STAGE-7` | Stage 7 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-MEMORY-BEFORE-BYTE-ACQUISITION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7` | `B-S7-MANDATORY-SET` |
| `SMAP-STAGE-8` | Stage 8 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7`, `FX-PRODUCT-8`, `FX-PRODUCT-10`, `FX-PRODUCT-11` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-MEMORY-BEFORE-BYTE-ACQUISITION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7`, `ORC-PRODUCT-8`, `ORC-PRODUCT-10`, `ORC-PRODUCT-11` | `B-S8-MANDATORY-SET` |
| `SMAP-STAGE-9` | Stage 9 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7`, `FX-PRODUCT-8`, `FX-PRODUCT-9`, `FX-PRODUCT-10`, `FX-PRODUCT-11` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-MEMORY-BEFORE-BYTE-ACQUISITION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7`, `ORC-PRODUCT-8`, `ORC-PRODUCT-9`, `ORC-PRODUCT-10`, `ORC-PRODUCT-11` | `B-S9-MANDATORY-SET` |
| `SMAP-STAGE-10` | Stage 10 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7`, `FX-PRODUCT-10` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-MEMORY-BEFORE-BYTE-ACQUISITION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7`, `ORC-PRODUCT-10` | `B-S10-MANDATORY-SET` |
| `SMAP-STAGE-11` | Stage 11 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7`, `FX-PRODUCT-10`, `FX-PRODUCT-11` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-MEMORY-BEFORE-BYTE-ACQUISITION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7`, `ORC-PRODUCT-10`, `ORC-PRODUCT-11` | `B-S11-MANDATORY-SET` |

| Bundle aggregate | Exact members |
| --- | --- |
| `B-S5-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S6-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S7-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S8-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S9-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S10-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S11-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |

Bundle shapes are reused; case identity and current-stage expectations are
not interchangeable. `B-CONTEXT` records admitted Tool/Skill/Memory/App
inputs; `B-REPLAY` records their retained versions and lifecycle.
Each bundle includes the exact positive-case facts defined in section 2.

## 5. Continuation requirement, evidence, and journey crosswalk

The selector resolves the exact gates, evidence, fixtures, fault points,
schedules, oracles, and bundles from section 4. The row's named facts and its
full accepted source obligation must pass; the short text does not narrow the
source. `PASS-POS` is required for positive facts. Expected refusal, unknown,
and hold cases use the parent's exact dispositions and cannot replace them.
Every row uses `BLOCK-ALL` for missing, failed, stale, or unreplayable evidence.

| ID | Required facts and canonical owners | Proof selector | Block |
| --- | --- | --- | --- |
| `S5-REQ-001` | The ordinary Agent conversation can perform bounded research through StoryOS ToolCalls. StoryOS dispatch uses the one Tool Gateway. StoryOS performs no Provider-hosted Operation (ADR 0084). ToolSpec, Registration, Enablement, Exposure, Capability, and Approval stay distinct. `OWN-TOOL`, `OWN-CTX`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-002` | A third-party MCP server is an untrusted registered integration. Bind its exact contract and Project use, reject incompatible drift, mediate each effect, and keep credentials outside project records and output. `OWN-MCP`, `OWN-TRUST`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-003` | Preserve native function-call/result correlation and validate the complete model-requested business-tool batch before deriving StoryOS ToolCalls. A native item of a type that the request did not ask for makes the Agent Decision invalid (ADR 0084). Returned content passes Context Assembly before a later StoryOS-controlled submission. Results grant no instruction, permission, or authority. `OWN-MODEL`, `OWN-CTX`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-004` | The author can inspect a research synthesis, its claims, exact sources, and supporting, conflicting, or limiting evidence. A missing or unavailable source remains visible as a gap, not an invented citation. `OWN-MEM`, `OWN-ARTIFACT`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-005` | Tool cancellation, interruption, unknown effects, and unknown usage stay durable and inspectable. Enforce registered intake, actual outward-processing bounds, and finite worst-case reservations before submission. Resume/reconciliation obey existing fences; a timeout proves neither remote stop nor retry permission. Incomplete, rejected, cancelled, or fenced output cannot advance normal continuation. `OWN-AGENT`, `OWN-TOOL`, `OWN-RET`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-006` | Research, Tool, and MCP results can supply bounded assistance or an editable Proposal. They cannot directly change prose, fiction facts, preferences, or manuscript structure. `OWN-ARTIFACT`, `OWN-CORE`, `OWN-ADM`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-EVD-001` | Real Tool/MCP qualification, registration, use, intake, bounds, grants, and contract-drift evidence. Unauthorized or unbounded operations refuse before dispatch. `OWN-TOOL`, `OWN-MCP`, `OWN-TRUST`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-EVD-002` | Native function/result correlation, complete batch validation, research claims and exact available evidence, truthful gaps/partial results, and re-entry at StoryOS-controlled submissions. Provider reports remain distinct from Host observations and opaque internal work. `OWN-MEM`, `OWN-CTX`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-EVD-003` | Durable interruption, cancellation, unknown-effect/usage recovery, single accounting, selected-result continuation, Proposal-only creative change, and AI-independent regression evidence. `OWN-AGENT`, `OWN-CORE`, `OWN-WEB`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-JRN-001` | Execute every step of the accepted Stage 5 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-5` | `BLOCK-ALL`; every journey step is mandatory. |
| `S6-REQ-001` | Standard package selection, one source and one installation scope (the deployment), the unique name, snapshot, and explicit reason without mandatory extensions (ADR 0089). `OWN-SKILL`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-REQ-002` | Progressive instructions/resources and fixed precedence without permission or authority. `OWN-SKILL`, `OWN-CTX`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-REQ-003` | Withdrawn for Stage 6 by ADR 0089: Tool roles, extensions, scripts, and outcomes stay deferred. `OWN-SKILL`, `OWN-TOOL`, `OWN-AGENT`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-REQ-004` | The usage record of each turn, and a change of the Skills of the deployment with fixed active snapshots (ADR 0089). `OWN-SKILL`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-EVD-001` | Compatibility, selection/snapshot, loading, composition, and instruction-boundary evidence. `OWN-SKILL`, `OWN-CTX`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-EVD-002` | A change of the Skills of the deployment with a Run in progress, invalid-package refusal, fixed Run binding, and editor regression (ADR 0089). `OWN-SKILL`, `OWN-TOOL`, `OWN-AGENT`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-JRN-001` | Execute every step of the accepted Stage 6 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-6` | `BLOCK-ALL`; every journey step is mandatory. |
| `S7-REQ-001` | Preserve one Project main Agent across distinct Project Conversations using their existing identities. The author can open an earlier Project Conversation again and continue it (ADR 0094). Cross-conversation continuity uses the Project Instruction, the manuscript tools, and Agent Memory; each conversation retains its own Provider continuation. Do not load the entire transcript, manuscript, or Memory collection by default. `OWN-AGENT`, `OWN-CTX`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-002` | Withdrawn for Stage 7 by ADR 0094. The structured fiction records stay deferred. A character, a relationship, and a major event are Memory Entries of S7-REQ-005, and they are not Authoritative State. `OWN-ARTIFACT`, `OWN-MEM`, `OWN-CORE`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-003` | Withdrawn for Stage 7 by ADR 0094. The Author Preference and the Inferred Preference stay deferred as separate records. The lasting instruction of the author is the Project Instruction of S7-REQ-004. A habit that the author states in a Conversation is a Memory Entry, and a Memory Entry is never an instruction. `OWN-MEM`, `OWN-ADM`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-004` | Support optional author-edited Project Instruction, immutable revisions, and exact top-level Run binding. The author writes it on the settings page of the Project (ADR 0094). Existing bindings stay fixed across Subruns and context compaction; absence does not block ordinary assistance. This does not defer any instruction binding already required in an earlier stage. `OWN-CTX`, `OWN-AGENT`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-005` | Keep Agent Memory as typed Memory Entries (ADR 0094). The Agent writes an entry at once on the request of the author. Bounded background captures of quiet Project Conversations and of changed Chapters give typed Memory Observations. A fenced consolidation pass applies validated typed operations in one transaction and prunes the entries. Background work cannot change an entry of the author; it records a conflict flag. A bounded Memory Index is in the context of a Run, and the Agent searches and reads entries with Host Tools without embedding. Keep origins, source references, model/prompt/input identities, and maintenance outcomes recorded. `OWN-MEM`, `OWN-RET`. `OWN-PG`. `OWN-TRUST`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-006` | Apply the Host defaults of the Conversation Memory Settings with no author control (ADR 0094). Offer author inspection of each Memory Entry with its origin, source, and conflict flag on the settings page, and a delete that background work does not undo. Ordinary corrections remain Messages. Retire semantic Include/Pin/Exclude and per-claim Admission/Suppression controls without aliases. `OWN-CTX`. `OWN-MEM`. `OWN-PROTO`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-007` | Give the Agent the whole manuscript (ADR 0095): the Chapter list in the context, a keyword search, a read, and a hybrid search. The vector index is rebuildable, and the embedding requests keep their own grant scope, bounds, and disclosure record. Deliver archive/export/restore and recovery of the Memory records. Existing active compaction and Provider continuation stay separate; ordinary corrections do not reset continuation. Memory recall does not require embedding. The removal of superseded Memory payload bytes and the fence of a covered-copy restriction stay deferred (ADR 0094). `OWN-CTX`, `OWN-MODEL`, `OWN-PG`, `OWN-RET`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-EVD-001` | Cross-conversation identity, Project Instruction revision/binding, generated-recall limitations, and author-authority evidence. ADR 0094 withdrew the structured-truth and explicit-preference evidence. `OWN-AGENT`, `OWN-MEM`, `OWN-ADM`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-EVD-002` | Host default settings behavior, eligible capture of Conversations and Chapters, the bounds of a capture, atomic consolidation, stale/duplicate work refusal, the protection and the conflict flag of an author entry, bounded recall, and author inspection and delete. Include AP-22 hostile-input, wrong-Scope, forbidden-write, secret, oversize, disabled, and unavailable-source cases; inspect records and effects, not semantic truth. `OWN-MEM`, `OWN-CTX`, `OWN-RET`. `OWN-TRUST`. `OWN-PG`. `OWN-PROTO`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-EVD-003` | Manuscript search and read, the embedding disclosure and its grant scope, vector index rebuild, the refusal of a stale vector hit, active-compaction separation, and Memory recovery/portability. Cover failed maintenance and truthful gaps with the editor still usable. ADR 0094 withdrew the covered-copy restriction and payload cleanup evidence. `OWN-CTX`, `OWN-MODEL`, `OWN-WEB`. `OWN-PG`. `OWN-RET`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-JRN-001` | Execute every step of the accepted Stage 7 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-7` | `BLOCK-ALL`; every journey step is mandatory. |
| `S8-REQ-001` | Production character, relationship, timeline, and research transcript views over StoryOS-owned data. `OWN-APP`, `OWN-ARTIFACT`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-REQ-002` | Immutable resources/View revisions, sandbox Instances, negotiation, limits, revocation, and termination. `OWN-APP`, `OWN-TRUST`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-REQ-003` | Stored-resource replay, Prepared Receipt, and terminal static fallback without repeated ToolCall. `OWN-APP`, `OWN-RET`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-REQ-004` | Persisted semantic actions with fresh applicable Admission and requesting-Instance delivery. `OWN-APP`, `OWN-TOOL`, `OWN-ADM`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-EVD-001` | Four production views, sandbox/resource binding, lifecycle/negotiation/limits/revocation. `OWN-APP`, `OWN-TRUST`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-EVD-002` | Fallback, non-executing replay, persisted action routing, instance-scoped response, and editor regression. `OWN-APP`, `OWN-RET`, `OWN-WEB`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-JRN-001` | Execute every step of the accepted Stage 8 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-8` | `BLOCK-ALL`; every journey step is mandatory. |
| `S9-REQ-001` | Optional RunPlan, RunPlan Revision, PlanStep, durable RunStep, lease, hold/wait, steering/cancellation/finalization, and layered timeline. `OWN-AGENT`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-002` | Hierarchical child, narrowed scope/context/Capability, budget reservation, execution record, and typed result. `OWN-SUBRUN`, `OWN-AGENT`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-003` | Mailbox/follow-up/interrupt, Required/Advisory join, backpressure, Seal, late result, and parent-child recovery. `OWN-SUBRUN`, `OWN-RET`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-004` | Explicit proactive event/schedule enablement, grants, bounds, misfire, and duplicate-effect refusal. `OWN-AGENT`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-005` | Resource/safety holds, budgets, usage classification, and exact settlement across each delivered operation. `OWN-AGENT`, `OWN-TOOL`, `OWN-SKILL`, `OWN-MODEL`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-006` | Configured model policy and visible route/fallback decisions without hidden destination or Agent identity change. `OWN-MODEL`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-EVD-001` | RunPlan, RunPlan Revision, and PlanStep planning evidence; RunStep, lease, wait/hold, steering, interruption, finalization, and timeline proof. `OWN-AGENT`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-EVD-002` | Child scope/reservation/Mailbox/join/Seal/late-result/cancellation/recovery proof. `OWN-SUBRUN`, `OWN-RET`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-EVD-003` | Authorized proactive work, misfire/deduplication, guardrails/model policy, and editor regression. `OWN-AGENT`, `OWN-MODEL`, `OWN-WEB`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-JRN-001` | Execute every step of the accepted Stage 9 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-9` | `BLOCK-ALL`; every journey step is mandatory. |
| `S10-REQ-001` | Default writing rules for prose in the Host instructions of each tool-loop request. Each rule states the wanted behaviour, and the rules are no list of forbidden phrases. They are defaults for the style of the prose. A different style applies before them when it is in the author request, in the Project Instruction, in a sample passage that they give, or in the instructions of a Skill. For the instructions of a Skill, this is the exception of ADR 0100 to the order of ADR 0089. A change of the rules gets a new mapping revision and a new Registration. `OWN-CTX`, `OWN-MODEL`. | `SMAP-STAGE-10` | `BLOCK-ALL`. |
| `S10-REQ-002` | The Project Instruction stays the one place for the lasting demands and the sample passages of the author, with no new record, no new control, and no automatic capture. StoryOS infers no preference of the author automatically (ADR 0100). `OWN-CTX`. | `SMAP-STAGE-10` | `BLOCK-ALL`. |
| `S10-REQ-003` | The SkillPackages of the StoryOS distribution as standard packages that the release package has as files: a local revision, a conflict check that changes no text, and an agreement before a scene while its ticket is in the ticket graph of the specification. The operator sets the Skill directory, and an absent setting gives no Skill (ADR 0089, ADR 0100). `OWN-SKILL`. | `SMAP-STAGE-10` | `BLOCK-ALL`. |
| `S10-REQ-004` | Marks on the new words of each Proposal candidate, a tick at each removal with no replacement, and no mark for an empty target Block or for a candidate whose text is almost all new. The old words of a mark show in the target Block only while the pointer is on that mark. The marks follow a change of the candidate text by the author, and they are in the view only (ADR 0100). `OWN-WEB`, `OWN-CORE`. | `SMAP-STAGE-10` | `BLOCK-ALL`. |
| `S10-EVD-001` | The exact rule text in the request tests, the new mapping revision and Registration, and one live run on the real route with its evidence tag and its record. `OWN-CTX`, `OWN-MODEL`. | `SMAP-STAGE-10` | `BLOCK-ALL`. |
| `S10-EVD-002` | The validation of each SkillPackage of the distribution by the rules of the Skill directory, the copy in the release package, and one live run for each package that shows its effect. `OWN-SKILL`. | `SMAP-STAGE-10` | `BLOCK-ALL`. |
| `S10-EVD-003` | The table of the expected marks for each test input, the view tests (marks, pair, author edit, IME composition, the two cases with no mark, and no change of the document or of the Local Edit Journal), and the journey on the exact dist. `OWN-WEB`, `OWN-CORE`. | `SMAP-STAGE-10` | `BLOCK-ALL`. |
| `S10-JRN-001` | Execute every step of the accepted Stage 10 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-10` | `BLOCK-ALL`; every journey step is mandatory. |
| `S11-REQ-001` | The Memory of one character as a character file with two parts, each one entry of the type character with the name of the character as its title. The part of the author holds what the author told the assistant to remember about the character and that the character knows, and background work never changes its text. The part of the background work holds the view of the character from the manuscript. A request of the author does not make the part of the background work an entry of the author, and a Project has at most one part of each kind for each character (ADR 0101). `OWN-CTX`, `OWN-MEM`. | `SMAP-STAGE-11` | `BLOCK-ALL`. |
| `S11-REQ-002` | One intent pass for a character after the file of that character changed, with no request of the author. The two parts of that file are the only source of knowledge about the character in the pass. A pass sends one model request. When the Host refuses the reply, the pass sends one more request with that refusal as a Tool result, so that the model can correct the reply. Its result is one intent and at most three questions to the author. The Host keeps the newest result with the part of the background work, and a result changes no text of the file (ADR 0101). `OWN-AGENT`, `OWN-MEM`. | `SMAP-STAGE-11` | `BLOCK-ALL`. |
| `S11-REQ-003` | The current intent of each character in each Run of the author that pins its Memory revision, as data that is no fact and no instruction. No capture, no consolidation pass, and no intent pass gets an intent, and no Run gets a question. The writing workspace shows no intent and no question (ADR 0101). `OWN-CTX`, `OWN-MODEL`. | `SMAP-STAGE-11` | `BLOCK-ALL`. |
| `S11-REQ-004` | A navigation on the left side of the settings page of a Project with the sections "项目说明", "记忆", and "人物". The section "人物" has a search of the names, and for the selected character the file with the part of the author first, the intent with the Chapter at which it was made, and the questions. The page has no control that edits a file, that starts a pass, or that answers a question (ADR 0101). `OWN-WEB`. | `SMAP-STAGE-11` | `BLOCK-ALL`. |
| `S11-EVD-001` | The tests of the two parts, the exact instruction texts in the request tests, the new mapping revision and Registration, and one live run of the file on the real route with its evidence tag and its record. The record has the length of each file after each Chapter and the check of the limit of knowledge. `OWN-CTX`, `OWN-MEM`, `OWN-MODEL`. | `SMAP-STAGE-11` | `BLOCK-ALL`. |
| `S11-EVD-002` | The tests of the intent pass, the tests that no capture and no pass gets an intent, the exact request texts, and one live run of the pass and of a continuation with the intents on the real route with its evidence tag and its record. `OWN-AGENT`, `OWN-MEM`, `OWN-CTX`. | `SMAP-STAGE-11` | `BLOCK-ALL`. |
| `S11-EVD-003` | The tests of the response of `listMemoryEntries`, the browser tests of the navigation and of the section "人物", and the journey on the exact dist. `OWN-WEB`, `OWN-PROTO`. | `SMAP-STAGE-11` | `BLOCK-ALL`. |
| `S11-JRN-001` | Execute every step of the accepted Stage 11 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-11` | `BLOCK-ALL`; every journey step is mandatory. |

## 6. Release branches

Each branch consumes its mandatory map and author journey only after both
pass on one exact candidate. It emits `EV-SR`, `PASS-STAGE`, and that
candidate's commit/tree. No branch consumes a future-stage result.

| Release selector | Mandatory input | Journey | Successor |
| --- | --- | --- | --- |
| `SMAP-RELEASE-STAGE-5` | `SMAP-STAGE-5` | `S5-JRN-001` | Exact Stage 6 input. |
| `SMAP-RELEASE-STAGE-6` | `SMAP-STAGE-6` | `S6-JRN-001` | Exact Stage 7 input. |
| `SMAP-RELEASE-STAGE-7` | `SMAP-STAGE-7` | `S7-JRN-001` | Exact Stage 10 input (ADR 0096). |
| `SMAP-RELEASE-STAGE-10` | `SMAP-STAGE-10` | `S10-JRN-001` | Exact Stage 11 input (ADR 0101). |
| `SMAP-RELEASE-STAGE-11` | `SMAP-STAGE-11` | `S11-JRN-001` | The input of the next stage. This table selects no next stage: a later ADR decides it when that work starts (ADR 0101 section 1). Stage 8 and Stage 9 stay on EXECUTION HOLD. |
| `SMAP-RELEASE-STAGE-8` | `SMAP-STAGE-8` | `S8-JRN-001` | Exact Stage 9 input. |
| `SMAP-RELEASE-STAGE-9` | `SMAP-STAGE-9` | `S9-JRN-001` | Complete accepted local MVP route; no further stage is inferred. |

Controlled-cloud delivery remains a separate gate after the first four
stages. It proves the exact selected released local stage, not all future
capabilities. Eval is outside MVP and is not an entry condition.

## 7. Source and ticket audit

Before planning handoff, the planning owner and an independent reviewer:

1. Compare the exact accepted release ID set with this combined crosswalk.
   Check every full source obligation, not only headings or counts.
2. Check all 71 source dispositions, including the two deferred Eval sources.
   Follow current MVP obligations to requirements and concrete acceptance
   responsibilities. Assign each responsibility to one ticket; a broad
   Requirement may have several contributing tickets. Deferred scope creates
   no MVP ticket, evidence, or release obligation.
3. Read the proposed tickets in reverse. Each delivers a named accepted
   behavior or fixes a current defect. Remove speculative infrastructure.
   Preserve closed owners and their evidence when the existing contract fits.
4. Check the proposed list against native parent/blocker state, actual input
   dependencies, no cycles, and the applicable product stage release gates. Use
   [Delivery](../agents/delivery.md) for parallel implementation
   of independent dependency-ready tickets. A parent
   specification need not close before its own children can execute.
5. Record approval of the exact breakdown, current body revisions, and the
   read-only audit. No new or split child is published as executable before
   that approval. A held graph has no current implementation Claim.
6. Reject lost source/requirement/journey, duplicate acceptance ownership,
   unknown proof reference, wrong successor, body/edge mismatch, positive
   capability backed only by absence, and prototype/history used as current
   product proof. Record each uncovered item; a summary count cannot erase it.

The current proof revision hands off to the existing [Stage 3](https://github.com/FrankQDWang/StoryOS/issues/361),
[Stage 4](https://github.com/FrankQDWang/StoryOS/issues/362),
[Stage 5](https://github.com/FrankQDWang/StoryOS/issues/363), and
[Stage 7](https://github.com/FrankQDWang/StoryOS/issues/365) parent specifications.
The current Stage 3 specification owns its accepted implementation handoff
and pending final release evidence. Any later change to a specification or
child graph requires alignment with the `to-spec` skill. Before publication, the
`to-tickets` skill review includes the changed breakdown and native blocking edges. Reuse current
owners and closed prerequisites. Stage 8 and Stage 9 remain on EXECUTION HOLD
(ADR 0096).
This proof contract does not claim that any product stage has passed.

Use the existing repository checks plus this bounded source/graph review.
The historical Stage 1 checker keeps its original inputs and claim ceiling.
A permanent new ledger, parser, workflow runtime, or CI system is not required
to complete this planning correction. Implementation agents add behavior
tests through existing repository commands when they implement each ticket.
