# Product Delivery Proof Selection

- Owner: [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60).
- Contract revision: `product-delivery-proof-mvp-boundary-2026-08-29-v2`.
- Status: current proof contract; product evidence remains unrun until implementation.
- Parent: [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md).

## 1. Accepted inputs and minimum work

The accepted release revision remains `product-delivery-mvp-boundary-2026-08-29-v1`.
[Keep the MVP route within confirmed scope](https://github.com/FrankQDWang/StoryOS/pull/370)
accepted it at `7de68e2790b009814a354cb267687b132b4060f1`, tree
`d14ae2136276b258694787789e6fe099c9008179`.

This binding includes the single `S3–S9` source-range correction in
[Remove inherited Eval proof gates from MVP](https://github.com/FrankQDWang/StoryOS/pull/371).
The exact source snapshot is `d1a016f871c553af1d8b1f601bde8b8d2931d11f`, tree
`86ed192bf1e73ce2226e0ccf3a97fe181e2099ad`. The original release owner reviewed the
copy edit: it changes no release criterion, Requirement, journey, source
membership, or guardrail. It is not a new release acceptance.

| Accepted source | UTF-8/LF SHA-256 |
| --- | --- |
| [Editor-first release](ai-independent-editor-first-release-baseline-and-handoff-criteria.md) | `550d22270aed7fd2749a9e7d0c7f92fbb737d76e67d5955d46624b364b4ad211` |
| [Product continuation](storyos-product-delivery-continuation.md) | `2380ed5413541e9c16398d5843337252cb1851a9a72b0ec22721441d20f757b5` |

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
This correction changes the two proof documents and one source-range
reference in the release continuation. Release criteria remain unchanged.
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
| `FX-PRODUCT-5` | Registration, exact MCP contract and Project use, Capability/Approval, bounded Tool dispatch, result re-entry, Research Claims with supporting/conflicting/limiting sources and gaps, Proposal-only creative changes, cancellation and unknown outcomes. | `ORC-PRODUCT-5` | Approved operations create exactly the admitted effect and provenance; incompatible drift, missing Approval, foreign Scope, secret output, direct creative writes, and blind retry fail. |
| `FX-PRODUCT-6` | Instruction-only and Tool/script Skills, installation scopes, selection and name conflict, immutable package snapshots, progressive resources, precedence/composition, optional extensions, outcome obligations, creation/update/revocation. | `ORC-PRODUCT-6` | The selected exact package and declared outcomes remain inspectable; loading and composition grant no authority; an active Run never switches snapshots after an update or revocation. |
| `FX-PRODUCT-7` | One Project Agent across threads; fiction assertions and scopes; explicit and inferred preferences; optional Project Instruction binding; Memory candidate/admission/suppression; context inspection/include/pin/exclude; retrieval, embedding, compaction/cache/Provider continuity. | `ORC-PRODUCT-7` | Only eligible exact sources enter context; author truth, inference, Working Context, and Memory stay distinct; old Run bindings stay fixed; rebuild, cache, and continuity cannot revive suppressed or unavailable data. |
| `FX-PRODUCT-8` | Character, relationship, timeline, and research views; immutable resources/View revisions; disposable sandbox Instances; negotiation/limits/revocation; Prepared Receipt and terminal fallback; admitted persisted actions and same-Instance responses. | `ORC-PRODUCT-8` | Opening/replaying a view never repeats a ToolCall; semantic actions have fresh applicable Admission; the sandbox cannot grant authority, call Tools directly, or send a response to another Instance. |
| `FX-PRODUCT-9` | Durable Plan/Step/wait/steering/cancellation; bounded Subruns with narrowed inputs and budget reservations; Mailbox/follow-up/interrupt/join/backpressure/Seal; parent-child recovery; proactive grants/misfires; guardrails and explicit model policy. | `ORC-PRODUCT-9` | Reservation, child creation, effects, and settlement follow the owner transaction; duplicates/late results never reopen work; proactive work needs its recorded grant; configured routing does not imply a second Provider. |

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
| `SMAP-STAGE-5` | Stage 5 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5` | `B-S5-MANDATORY-SET` |
| `SMAP-STAGE-6` | Stage 6 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6` | `B-S6-MANDATORY-SET` |
| `SMAP-STAGE-7` | Stage 7 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7` | `B-S7-MANDATORY-SET` |
| `SMAP-STAGE-8` | Stage 8 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7`, `FX-PRODUCT-8` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7`, `ORC-PRODUCT-8` | `B-S8-MANDATORY-SET` |
| `SMAP-STAGE-9` | Stage 9 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P`, `FX-PRODUCTION-EDITOR`, `FX-PROPOSAL-INTERACTION`, `FX-PRODUCT-5`, `FX-PRODUCT-6`, `FX-PRODUCT-7`, `FX-PRODUCT-8`, `FX-PRODUCT-9` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-UNDO-BEFORE-SETTLEMENT`, `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE`, `ORC-PRODUCTION-EDITOR`, `ORC-PROPOSAL-INTERACTION`, `ORC-PRODUCT-5`, `ORC-PRODUCT-6`, `ORC-PRODUCT-7`, `ORC-PRODUCT-8`, `ORC-PRODUCT-9` | `B-S9-MANDATORY-SET` |

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
| `S5-REQ-001` | Ordinary research through one governed Gateway; distinct registration, exposure, Capability, Approval, and effect roles. `OWN-TOOL`, `OWN-CTX`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-002` | Exact untrusted MCP registration, Project use, incompatible drift refusal, and protected credentials. `OWN-MCP`, `OWN-TRUST`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-003` | Normalized model Tool request and result re-entry through Context Assembly. `OWN-MODEL`, `OWN-CTX`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-004` | Research synthesis, exact claims/sources, supporting/conflicting/limiting evidence, and visible gaps. `OWN-MEM`, `OWN-ARTIFACT`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-005` | Durable cancellation, interruption, unknown effect, and permitted reconciliation without blind resend. `OWN-AGENT`, `OWN-TOOL`, `OWN-RET`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-REQ-006` | Research/Tool/MCP assistance can create inspectable Proposals, never direct creative authority. `OWN-ARTIFACT`, `OWN-CORE`, `OWN-ADM`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-EVD-001` | Real integration, registration/Project use, effect/approval/drift, and zero-authority evidence. `OWN-TOOL`, `OWN-MCP`, `OWN-TRUST`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-EVD-002` | Source/claim/synthesis/provenance/gap and complete Context/disclosure re-entry evidence. `OWN-MEM`, `OWN-CTX`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-EVD-003` | Interrupted/unknown operation, Proposal-only change, and complete AI-disabled regression evidence. `OWN-AGENT`, `OWN-CORE`, `OWN-WEB`. | `SMAP-STAGE-5` | `BLOCK-ALL`. |
| `S5-JRN-001` | Execute every step of the accepted Stage 5 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-5` | `BLOCK-ALL`; every journey step is mandatory. |
| `S6-REQ-001` | Standard package selection, installation scope, name conflict, snapshot, and explicit reason without mandatory extensions. `OWN-SKILL`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-REQ-002` | Progressive instructions/resources and fixed precedence without permission or authority. `OWN-SKILL`, `OWN-CTX`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-REQ-003` | Tool roles, extensions, bounded scripts, outcomes, and missing/conflicting prerequisites through existing execution. `OWN-SKILL`, `OWN-TOOL`, `OWN-AGENT`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-REQ-004` | Inspectable selection/composition/conflicts/outcomes and create/install/update/revoke lifecycle with fixed active snapshots. `OWN-SKILL`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-EVD-001` | Compatibility, selection/snapshot, loading, composition, and instruction-boundary evidence. `OWN-SKILL`, `OWN-CTX`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-EVD-002` | Tool/script admission, outcomes, failure/revocation, fixed Run binding, and editor regression. `OWN-SKILL`, `OWN-TOOL`, `OWN-AGENT`. | `SMAP-STAGE-6` | `BLOCK-ALL`. |
| `S6-JRN-001` | Execute every step of the accepted Stage 6 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-6` | `BLOCK-ALL`; every journey step is mandatory. |
| `S7-REQ-001` | One Project main Agent and eligible exact continuity across threads. `OWN-AGENT`, `OWN-CTX`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-002` | Author-owned fiction assertions, characters/relations/timeline, Story/Epistemic Scope, conflicts, and explicit settlement. `OWN-ARTIFACT`, `OWN-MEM`, `OWN-CORE`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-003` | Current feedback, explicit Author Preference, and Inferred Preference stay distinct and inspectable. `OWN-MEM`, `OWN-ADM`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-004` | Optional Project Instruction revisions, top-level binding, descendants, compaction, and old/new Run separation. `OWN-CTX`, `OWN-AGENT`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-005` | Source-bearing Memory, admission/invalidation/suppression, conflicts/gaps, and rebuild. `OWN-MEM`, `OWN-RET`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-006` | Inspect/include/pin/exclude controls, usable defaults, mandatory/dynamic context, budget/projection/manifests. `OWN-CTX`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-REQ-007` | Exact embedding/retrieval/compaction/cache/Provider-continuity bindings without revival or authority. `OWN-CTX`, `OWN-MODEL`, `OWN-PG`, `OWN-RET`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-EVD-001` | Cross-thread identity, structured truth/preference, instruction revision/binding, and author-authority proof. `OWN-AGENT`, `OWN-MEM`, `OWN-ADM`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-EVD-002` | Memory eligibility, suppression/invalidation, controls, rebuild, lifecycle, and non-revival. `OWN-MEM`, `OWN-CTX`, `OWN-RET`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-EVD-003` | Complete context/embedding/disclosure, continuity/cache/compaction, recovery, and editor regression. `OWN-CTX`, `OWN-MODEL`, `OWN-WEB`. | `SMAP-STAGE-7` | `BLOCK-ALL`. |
| `S7-JRN-001` | Execute every step of the accepted Stage 7 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-7` | `BLOCK-ALL`; every journey step is mandatory. |
| `S8-REQ-001` | Production character, relationship, timeline, and research transcript views over StoryOS-owned data. `OWN-APP`, `OWN-ARTIFACT`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-REQ-002` | Immutable resources/View revisions, sandbox Instances, negotiation, limits, revocation, and termination. `OWN-APP`, `OWN-TRUST`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-REQ-003` | Stored-resource replay, Prepared Receipt, and terminal static fallback without repeated ToolCall. `OWN-APP`, `OWN-RET`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-REQ-004` | Persisted semantic actions with fresh applicable Admission and requesting-Instance delivery. `OWN-APP`, `OWN-TOOL`, `OWN-ADM`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-EVD-001` | Four production views, sandbox/resource binding, lifecycle/negotiation/limits/revocation. `OWN-APP`, `OWN-TRUST`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-EVD-002` | Fallback, non-executing replay, persisted action routing, instance-scoped response, and editor regression. `OWN-APP`, `OWN-RET`, `OWN-WEB`. | `SMAP-STAGE-8` | `BLOCK-ALL`. |
| `S8-JRN-001` | Execute every step of the accepted Stage 8 author journey on one exact released candidate, including the complete AI-disabled regression; `OWN-REL` and the stage's named owners. | `SMAP-STAGE-8` | `BLOCK-ALL`; every journey step is mandatory. |
| `S9-REQ-001` | Adaptive Plan, durable Step, lease, hold/wait, steering/cancellation/finalization, and layered timeline. `OWN-AGENT`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-002` | Hierarchical child, narrowed scope/context/Capability, budget reservation, execution record, and typed result. `OWN-SUBRUN`, `OWN-AGENT`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-003` | Mailbox/follow-up/interrupt, Required/Advisory join, backpressure, Seal, late result, and parent-child recovery. `OWN-SUBRUN`, `OWN-RET`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-004` | Explicit proactive event/schedule enablement, grants, bounds, misfire, and duplicate-effect refusal. `OWN-AGENT`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-005` | Resource/safety holds, budgets, usage classification, and exact settlement across each delivered operation. `OWN-AGENT`, `OWN-TOOL`, `OWN-SKILL`, `OWN-MODEL`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-REQ-006` | Configured model policy and visible route/fallback decisions without hidden destination or Agent identity change. `OWN-MODEL`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
| `S9-EVD-001` | Plan/Step/lease/wait/hold/steering/interruption/finalization/timeline proof. `OWN-AGENT`. | `SMAP-STAGE-9` | `BLOCK-ALL`. |
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

Use the existing repository checks plus this bounded source/graph review.
The historical Stage 1 checker keeps its original inputs and claim ceiling.
A permanent new ledger, parser, workflow runtime, or CI system is not required
to complete this planning correction. Implementation agents add behavior
tests through existing repository commands when they implement each ticket.
