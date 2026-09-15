# Product Delivery Proof Selection

- Owner: [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60).
- Contract revision: `proof-responses-memory-2026-09-15-v1`.
- Status: current proof contract; product evidence remains unrun until implementation.
- Parent: [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md).

## 1. Accepted inputs and minimum work

The accepted release revision is `release-responses-memory-2026-09-14-v1`.
[Align release stages with Responses and background Memory](https://github.com/FrankQDWang/StoryOS/pull/684)
accepted the two sources at `cca8e646f137bccc27414014470211451a1ac474`, tree
`04b33642b800cd60345bf75bf366c1c08c59a1c3`. This is the exact source snapshot
consumed by both proof documents, not the release owner's earlier Claim baseline.

| Accepted source | UTF-8/LF SHA-256 |
| --- | --- |
| [Editor-first release](ai-independent-editor-first-release-baseline-and-handoff-criteria.md) | `c690baa01b2f28b14dfcc85f128b6ba3876c4e9a31de283a4d86a487791a75f9` |
| [Product continuation](storyos-product-delivery-continuation.md) | `3ce5355b41326d696b27f1e54a28ec03ffd4d769cb2d659ee2d68cda788f602c` |

The continuation source owns the exact 71-source disposition inventory:
original Issues 2 through 65, 67 through 70, and 75 through 77. Its additional
completed governance sources remain implementation evidence. The combined
release sources define 113 table IDs and nine journeys. These counts describe
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
| `FX-PRODUCT-5` | Exact Tool/MCP contract and Project use; separately admitted hosted set/intake/bounds; native function/result correlation; complete business batch; distinct Approval targets; research sources/gaps/partial results; cancellation and unknown outcomes. | `ORC-PRODUCT-5` | StoryOS effects match their own admission. Hosted scope is bounded before submission, with Host facts, Provider reports, and unknowns separate. No ambient intake, invalid batch execution, duplicate accounting, direct creative write, or blind retry. Section 2.3 defines the exact cases. |
| `FX-PRODUCT-6` | Instruction-only and Tool/script Skills, installation scopes, selection and name conflict, immutable package snapshots, progressive resources, precedence/composition, optional extensions, outcome obligations, creation/update/revocation. | `ORC-PRODUCT-6` | The selected exact package and declared outcomes remain inspectable; loading and composition grant no authority; an active Run never switches snapshots after an update or revocation. |
| `FX-PRODUCT-7` | One Project Agent across conversations; fiction/preference/Instruction boundaries; independent settings; background extraction/consolidation; exact publications/Documents/Notes; selective recall; restrictions, cleanup, and recovery. | `ORC-PRODUCT-7` | Atomic publication, current read permission, bounded records/effects, and exact lifecycle evidence follow section 2.4. Generated Memory grants no authority. Ordinary corrections do not reset continuation; real covered-copy restrictions remain enforced. |
| `FX-PRODUCT-8` | Character, relationship, timeline, and research views; immutable resources/View revisions; disposable sandbox Instances; negotiation/limits/revocation; Prepared Receipt and terminal fallback; admitted persisted actions and same-Instance responses. | `ORC-PRODUCT-8` | Opening/replaying a view never repeats a ToolCall; semantic actions have fresh applicable Admission; the sandbox cannot grant authority, call Tools directly, or send a response to another Instance. |
| `FX-PRODUCT-9` | Optional RunPlan with RunPlan Revisions and PlanSteps; durable RunStep, wait, steering, and cancellation; bounded Subruns with narrowed inputs and budget reservations; Mailbox/follow-up/interrupt/join/backpressure/Seal; parent-child recovery; proactive grants/misfires; guardrails and explicit model policy. | `ORC-PRODUCT-9` | Reservation, child creation, effects, and settlement follow the owner transaction; duplicates/late results never reopen work; proactive work needs its recorded grant; configured routing does not imply a second Provider. |

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
| Exact real-route qualification | `S4-REQ-001`, `S4-EVD-001`; hosted qualification also refines `S5-EVD-001`. | `DVG-01`, `DVG-05`, `DVG-11`; `ORC-CONTRACT`, `ORC-DISPATCH-DISCLOSURE`. | `CFP-CONTRACT-DRIFT`, `CFP-DISPATCH-BEFORE-CLAIM`; `SCH-DRIFT/NORMAL/SCOPE`. | `B-CONTRACT`, `B-CONTEXT`, `B-REAL-ADVISORY`. |
| Separate hosted operation and business ToolCall | `S5-REQ-001`, `S5-REQ-003`, `S5-REQ-005`, `S5-EVD-001`, `S5-EVD-002`, `S5-EVD-003`. | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11`; `ORC-PRODUCT-5`. | `CFP-HOSTED-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-LATE-RESULT`; `SCH-NORMAL/SCOPE/DRIFT/UNKNOWN/FENCE`. | `B-CONTEXT`, `B-FAKE`, `B-RECOVERY`, `B-REAL-ADVISORY`. |
| Memory maintenance and publication | `S7-REQ-005`, `S7-REQ-006`, `S7-EVD-002`. | `DVG-02`, `DVG-04`, `DVG-07`, `DVG-11`; `ORC-PRODUCT-7`, `ORC-RECOVERY-ATOMICITY`. | `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-SCOPE-BEFORE-QUERY`; `SCH-NORMAL/CRASH/FENCE/REORDER/SCOPE`. | `B-CONTEXT`, `B-RECOVERY`, `B-REPLAY`, `B-SCOPE`. |
| Memory restriction, cleanup, and portability | `S7-REQ-007`, `S7-EVD-003`. | `DVG-02`, `DVG-04`, `DVG-07`, `DVG-09`, `DVG-10`, `DVG-11`; `ORC-PRODUCT-7`, `ORC-RESTORE-LIFECYCLE`. | `CFP-MEMORY-BEFORE-BYTE-ACQUISITION`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-RESTORE-STAGING`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-AFTER-VISIBILITY`; `SCH-LIFECYCLE/CRASH/REORDER/RESTORE/SCOPE`. | `B-CONTEXT`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`. |

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
   arguments/results, refusals, declared summaries, hosted items, usage, and
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
   authority, budget for both, and no unresolved Tool or hosted effect. Persist
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
   structured/native text output, hosted capabilities, native/Host compaction,
   retrieval, and abort, including required combinations and gaps. Test current
   instructions/output requirements with optional cache disabled, incompatible,
   and missed. No cache hit, native compaction, resumed stream, create idempotency,
   savings, or remote stop is inferred from a setting or response identifier.
   Unknown required behavior blocks that real route; optional absence follows
   the accepted profile. Host facts, Provider reports, opaque references, and
   missing evidence remain distinct, including reported/estimated/unknown usage.

### 2.3 Hosted-operation cases

`ORC-PRODUCT-5` retains the research/Proposal cases and also requires:

- Admit the complete enabled hosted set before submission, including a response
  that uses none. Check each exact Registration, ToolSpec, current use binding,
  compatibility, explicit intake/reference, processor/destination, allowed
  search/read/scratch scope, effect ceiling, and finite worst-case reservation.
  Unknown required bounds, a prompt-only limit, or untrusted read-only metadata
  cannot authorize dispatch. No ambient conversation access, external business
  write/message/publication, or direct StoryOS write enters the hosted scope.
- Exercise unchanged work under the current Run Grant without another prompt;
  expanded authority requires its exact hosted Tool Approval. Substitute a
  ToolCall Approval, disclosure-only Approval, changed digest, rejected/expired
  Approval, or another Attempt: each unauthorized request refuses. Separately
  authorized StoryOS ToolCalls retain their own effects and one-shot rules.
- Validate the whole requested business-tool batch before deriving ToolCalls;
  one invalid member rejects it. Return each settled Tool result with its exact
  native call correlation, without executing it again. Rejection does not undo
  hosted work that already occurred. Provider events are not Host ToolCalls or
  invented per-step manifests; later StoryOS-controlled submissions re-enter
  Context Assembly with returned content as data.
- Record one physical submission/accounting boundary. Preserve Host observations,
  Provider reports, unobserved internal processing, and unknown usage separately.
  A completed outer Attempt with a fully validated Decision may deliver complete
  sourced sub-results and an unmet research objective. An incomplete stream or
  cancelled/fenced result cannot use that partial-deliverable path.
- Interrupt after dispatch; retain unknown effects and reservations, fence new
  work, reconcile through admitted retrieval, and refuse automatic rerun while
  effects remain unknown. A local timeout or expiry proves no remote stop.
  Exercise actual real Tool/MCP and qualified hosted journeys separately from
  scripted failure proof. Missing required live evidence blocks Stage 5.

### 2.4 Memory cases

`ORC-PRODUCT-7` retains fiction, preference, and Project Instruction proof and
adds the following records/effects cases from the Memory, protocol, storage,
retention, and AP-22 owners:

1. Change both independent settings atomically against an expected revision
   only when every foreground root Run is terminal/absent. Race update with Run
   admission; queued, waiting, paused, blocked, cancelling, recovering, and
   finalizing work remains busy. An independent background job does not block
   settings. Exact successful retry replays even after a Run starts; a conflict
   does not queue a later change. The Run tree retains its captured revision.
   Stages 3 through 6 record settings but perform no Memory execution.
2. With Agent use disabled, reject new navigation/search/document intake but
   allow authorized author inspection. Disable contribution before extraction
   read/dispatch to prevent new work. Disable it after dispatch and prove that
   this alone does not revoke publication permission. Real access/copy
   restrictions still apply. Neither switch deletes published Memory or history.
3. Extract from bounded eligible idle conversation snapshots, then consolidate
   captured extraction outputs, pending Notes, and the exact publication under
   current grants/bounds. Pin inputs, model/prompt versions, producer, lease,
   generation, and availability fence. No live-buffer read or default whole
   transcript/Memory dump is allowed. Staged output is not a published set.
4. Compete two publishers; crash before and after commit. Check exact members,
   summary, Heads, pointer generation, processed inputs/Notes, outcome, Activity,
   and invalidation as one settlement. Readers never mix old membership with
   a new summary. A stale worker cannot overwrite the winner or relabel old
   generated output; retry rereads the current publication and inputs.
5. A Note needs its exact requesting author Message and validated Decision/
   ToolCall, and commits with pending intent and Activity. Duplicate delivery
   returns the same Note. Distinguish `note_recorded`, `published`, `no_op`,
   `failed`, and `source_unavailable`; only `published` carries a publication
   reference in its outcome Event. A verified no-op can process captured Notes
   without a new set; uncaptured Notes stay pending. Failure preserves the
   prior pointer. Consideration does not prove that a semantic request succeeded.
6. Inspect no-publication-yet, valid empty, and unavailable states separately.
   Recall pins the exact permitted publication and document revisions; an index
   hit or latest alias cannot replace them. Rebuild indexes without model use.
   Regeneration is separate admitted work and need not reproduce identical text.
   Ordinary corrections/Notes/source edits do not reset Provider continuation
   or delete generated paraphrases. Retired semantic controls have no aliases.
7. Commit a real covered-copy restriction before cleanup. Reliably isolated
   restrictions fence only the required scope; otherwise use conservative
   set-wide unavailability. Test older/staged Documents, jobs, cached/indexed
   reads, captured export inputs, downloads, and restore. Rebuild only from
   allowed inputs; insufficient input stays unavailable. A new publication
   cannot lift old payload fences or prove erasure of past model influence.
8. Race cleanup with publication and byte-dependent inspection/export/work
   acquisition. Permit only due, superseded generated revisions under an adopted
   Profile, never a current member or Artifact Head, author-edited revision,
   Message/Note, Research, Proposal/Draft, or required active/recovery payload.
   Missing profile/lifecycle evidence blocks cleanup. Persist Decision, identity,
   digest, use/publication references, and gap; fence/invalidate before idempotent
   physical removal. Shared bytes survive another retained reference, while
   the fenced logical revision stays unreadable. No new Tombstone is fabricated.
9. Export/restore permitted exact Documents, Notes, publications, settings,
   pending work, outcomes, and lifecycle/use evidence. Missing lifecycle proof
   holds visibility; discarded indexes rebuild, restricted bytes never revive.
   Include new families in their owning migration and physical recovery proof;
   current activation mismatch requires a separate authorized upgrade path.
10. Inject hostile text/false citations, wrong Scope, forged Note origin,
    forbidden writes, secrets, oversized output, disabled generation, unavailable
    inputs, and duplicate/stale/partial publications. Inspect actual Host reads,
    requests, records, and effects. Maintenance can write only its admitted
    generated set, not Skills, ToolSpecs, policy, credentials, Author Preferences,
    or Authoritative State. Do not assert semantic truth, precise forgetting,
    or inevitable model obedience. The full editor remains usable with failed
    or unavailable Memory; background work does not block manual writing.

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

The maps use the parent's finite token grammar. All named positive fixtures
and oracles are defined in section 2. `EV-SR` is never a mandatory input.

| Selector | Stage | Gate set | Evidence classes/layers | Fixtures | Contract Fault Points | Schedules | Oracles | Mandatory bundle set |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SMAP-STAGE-5` | Stage 5 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-HOSTED-BEFORE-ADMISSION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5` | `B-S5-MANDATORY-SET` |
| `SMAP-STAGE-6` | Stage 6 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-HOSTED-BEFORE-ADMISSION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6` | `B-S6-MANDATORY-SET` |
| `SMAP-STAGE-7` | Stage 7 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-HOSTED-BEFORE-ADMISSION`, `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-MEMORY-BEFORE-BYTE-ACQUISITION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7` | `B-S7-MANDATORY-SET` |
| `SMAP-STAGE-8` | Stage 8 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7`, `FX-PRODUCT-8` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-HOSTED-BEFORE-ADMISSION`, `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-MEMORY-BEFORE-BYTE-ACQUISITION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7`, `ORC-PRODUCT-8` | `B-S8-MANDATORY-SET` |
| `SMAP-STAGE-9` | Stage 9 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7`, `FX-PRODUCT-8`, `FX-PRODUCT-9` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL`, `CFP-CONVERSATION-BEFORE-ADMISSION`, `CFP-MODEL-BEFORE-DECISION`, `CFP-MODEL-AFTER-DECISION-BEFORE-CONTINUATION`, `CFP-CONTEXT-BEFORE-INSTALL`, `CFP-MODEL-AFTER-SUCCESSOR-FENCE`, `CFP-HOSTED-BEFORE-ADMISSION`, `CFP-MEMORY-BEFORE-PUBLICATION`, `CFP-MEMORY-AFTER-PUBLICATION`, `CFP-MEMORY-BEFORE-BYTE-ACQUISITION` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7`, `ORC-PRODUCT-8`, `ORC-PRODUCT-9` | `B-S9-MANDATORY-SET` |

| Bundle aggregate | Exact members |
| --- | --- |
| `B-S5-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S6-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S7-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S8-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |
| `B-S9-MANDATORY-SET` | `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-MEASURE`, `B-HANDOFF`, `B-REAL-ADVISORY`. |

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
| `S5-REQ-001` | The ordinary Agent conversation can perform bounded research through StoryOS ToolCalls and separately admitted Provider-hosted Operations. StoryOS dispatch uses the one Tool Gateway; hosted search, reading, and bounded temporary computation use ADR 0034 whole-operation admission. ToolSpec, Registration, Enablement, Exposure, Capability, and Approval stay distinct. `OWN-TOOL`, `OWN-CTX`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-002` | A third-party MCP server is an untrusted registered integration. Bind its exact contract and Project use, reject incompatible drift, mediate each effect, and keep credentials outside project records and output. `OWN-MCP`, `OWN-TRUST`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-003` | Preserve native function-call/result correlation and validate the complete model-requested business-tool batch before deriving StoryOS ToolCalls. Hosted items remain evidence of the separately admitted whole operation, not invented Host ToolCalls. Returned content passes Context Assembly before a later StoryOS-controlled submission; invisible Provider-internal steps do not claim Host gates. Results grant no instruction, permission, or authority. `OWN-MODEL`, `OWN-CTX`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-004` | The author can inspect a research synthesis, its claims, exact sources, and supporting, conflicting, or limiting evidence. A missing or unavailable source remains visible as a gap, not an invented citation. `OWN-MEM`, `OWN-ARTIFACT`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-005` | Tool and hosted-operation cancellation, interruption, unknown effects, and unknown usage stay durable and inspectable. Enforce registered intake, actual outward-processing bounds, and finite worst-case reservations before submission. Resume/reconciliation obey existing fences; a timeout proves neither remote stop nor retry permission. Incomplete, rejected, cancelled, or fenced hosted output cannot advance normal continuation. `OWN-AGENT`, `OWN-TOOL`, `OWN-RET`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-006` | Research, Tool, and MCP results can supply bounded assistance or an editable Proposal. They cannot directly change prose, fiction facts, preferences, or manuscript structure. `OWN-ARTIFACT`, `OWN-CORE`, `OWN-ADM`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-EVD-001` | Real Tool/MCP and exact hosted-mode qualification, registration, use, complete-set intake, bounds, grants, distinct Approval targets, and contract-drift evidence. Unauthorized or unbounded operations refuse before dispatch. `OWN-TOOL`, `OWN-MCP`, `OWN-TRUST`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-EVD-002` | Native function/result correlation, complete batch validation, research claims and exact available evidence, truthful gaps/partial results, and re-entry at StoryOS-controlled submissions. Provider reports remain distinct from Host observations and opaque internal work. `OWN-MEM`, `OWN-CTX`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-EVD-003` | Durable interruption, cancellation, unknown-effect/usage recovery, single accounting, selected-result continuation, Proposal-only creative change, and AI-independent regression evidence. `OWN-AGENT`, `OWN-CORE`, `OWN-WEB`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-JRN-001` | Execute every step of the accepted Stage 5 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-5` | `BLOCK-ALL`; every journey step is mandatory. |
| `S6-REQ-001` | Standard package selection, installation scope, name conflict, snapshot, and explicit reason without mandatory extensions. `OWN-SKILL`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-REQ-002` | Progressive instructions/resources and fixed precedence without permission or authority. `OWN-SKILL`, `OWN-CTX`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-REQ-003` | Tool roles, extensions, bounded scripts, outcomes, and missing/conflicting prerequisites through existing execution. `OWN-SKILL`, `OWN-TOOL`, `OWN-AGENT`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-REQ-004` | Inspectable selection/composition/conflicts/outcomes and create/install/update/revoke lifecycle with fixed active snapshots. `OWN-SKILL`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-EVD-001` | Compatibility, selection/snapshot, loading, composition, and instruction-boundary evidence. `OWN-SKILL`, `OWN-CTX`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-EVD-002` | Tool/script admission, outcomes, failure/revocation, fixed Run binding, and editor regression. `OWN-SKILL`, `OWN-TOOL`, `OWN-AGENT`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-JRN-001` | Execute every step of the accepted Stage 6 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-6` | `BLOCK-ALL`; every journey step is mandatory. |
| `S7-REQ-001` | Preserve one Project main Agent across distinct Project Conversations using their existing identities. Cross-conversation continuity uses eligible Project sources and generated Memory; each conversation retains its own Provider continuation. Do not load the entire transcript or Memory collection by default. `OWN-AGENT`, `OWN-CTX`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-002` | Represent author-owned world facts, characters, relationships, and timeline with the accepted fiction assertion, Story Scope, and Epistemic Scope semantics. Conflicting claims remain distinguishable; generated claims need explicit author-authorized settlement before authority. `OWN-ARTIFACT`, `OWN-MEM`, `OWN-CORE`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-003` | Keep current feedback, explicit future-facing Author Preferences, and Inferred Preferences distinct. Inference is never an automatic lasting rule. The author can inspect and change explicit preferences through their owning commands. `OWN-MEM`, `OWN-ADM`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-004` | Support optional author-edited Project Instruction, immutable revisions, and exact top-level Run binding. Existing bindings stay fixed across Subruns and context compaction; absence does not block ordinary assistance. This does not defer any instruction binding already required in an earlier stage. `OWN-CTX`, `OWN-AGENT`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-005` | Extract from bounded eligible idle conversation snapshots and consolidate generated Memory Documents through fenced background work. Publish one complete set of exact revisions and a bounded navigation summary atomically. Keep source references, model/prompt/input identities, pending Notes, and maintenance outcomes inspectable. Search/read tools use currently permitted publications; indexes rebuild from retained documents without model calls. Generation is separate and need not reproduce identical text. `OWN-MEM`, `OWN-RET`. `OWN-PG`. `OWN-TRUST`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-006` | Apply the release's independent conversation Memory defaults and atomic idle-only settings contract. Offer author inspection of settings, publication, documents, Notes, sources, and maintenance outcomes even with Agent use disabled. Ordinary corrections remain Messages; an explicit lasting Memory request may record a Note for later consolidation. Note-recorded, published, no-op, failed, and unavailable are distinct. Retire semantic Include/Pin/Exclude and per-claim Admission/Suppression controls without aliases. `OWN-CTX`. `OWN-MEM`. `OWN-PROTO`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-007` | Deliver Memory source restrictions, lifecycle, qualified generated-payload cleanup, archive/export/restore, and recovery under exact source/destination bindings. Preserve current publication/Artifact Head/author/active-work/shared-payload protections and identity/use/Decision/gap evidence. Real covered-copy restrictions fence stale work and retained copies; ordinary corrections do not reset continuation. Existing active compaction and Provider continuation stay separate. Independently enabled embedding keeps its own capability, permission, budget, and disclosure gates; basic Memory recall does not require embedding. `OWN-CTX`, `OWN-MODEL`, `OWN-PG`, `OWN-RET`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-EVD-001` | Cross-conversation identity, structured truth, explicit preference, instruction revision/binding, generated-recall limitations, and author-authority evidence. `OWN-AGENT`, `OWN-MEM`, `OWN-ADM`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-EVD-002` | Independent default/settings behavior and idle/admission race, eligible extraction, complete publication, stale/duplicate work refusal, bounded recall, author inspection, Note origin and distinct maintenance outcomes. Include AP-22 hostile-input, wrong-Scope, forbidden-write, secret, oversize, disabled, and unavailable-source cases; inspect records and effects, not semantic truth. `OWN-MEM`, `OWN-CTX`, `OWN-RET`. `OWN-TRUST`. `OWN-PG`. `OWN-PROTO`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-EVD-003` | Context and independently enabled embedding/disclosure, exact publication reads, active-compaction separation, real covered-copy restrictions, allowed-input/index rebuild, and Memory recovery/portability. Cover cleanup racing publication or byte acquisition, protected/shared payloads, pending Notes, failed maintenance, missing lifecycle evidence, and truthful gaps with the editor still usable. `OWN-CTX`, `OWN-MODEL`, `OWN-WEB`. `OWN-PG`. `OWN-RET`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
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

## 6. Release branches

Each branch consumes its mandatory map and author journey only after both
pass on one exact candidate. It emits `EV-SR`, `PASS-STAGE`, and that
candidate's commit/tree. No branch consumes a future-stage result.

| Release selector | Mandatory input | Journey | Successor |
| --- | --- | --- | --- |
| `SMAP-RELEASE-STAGE-5` | `SMAP-STAGE-5` | `S5-JRN-001` | Exact Stage 6 input. |
| `SMAP-RELEASE-STAGE-6` | `SMAP-STAGE-6` | `S6-JRN-001` | Exact Stage 7 input. |
| `SMAP-RELEASE-STAGE-7` | `SMAP-STAGE-7` | `S7-JRN-001` | Exact Stage 8 input. |
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
   dependencies, no cycles, and the published serial priority. A parent
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
Their current bodies and children still require `/to-spec` alignment, then
`/to-tickets` review of any changed breakdown and native blocking edges before
publication. Reuse current owners and closed prerequisites. This proof decision
does not claim that graph alignment, global planning closure, or any product
stage has passed. Stage 3 and later implementation remains EXECUTION HOLD.

Use the existing repository checks plus this bounded source/graph review.
The historical Stage 1 checker keeps its original inputs and claim ceiling.
A permanent new ledger, parser, workflow runtime, or CI system is not required
to complete this planning correction. Implementation agents add behavior
tests through existing repository commands when they implement each ticket.
