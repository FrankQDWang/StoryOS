# Deterministic Verification and Failure-Recovery Gates

- Status: current Release 1 verification contract for [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60)
- Canonical issue: [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60)
- Contract revision: `deterministic-gates-author-edit-batch-2026-08-16-v4`
- Exact release baseline consumed: `main@10fa48b5dfd89b21508481dacb572d8af3a5e69e`
- Exact baseline tree consumed: `1e60ae74e6b42e3ba9fcbc4bb79c04f7bd7285fe`
- Predecessor release-contract revision: `release-baseline-editor-first-2026-07-31-issue62`
- Release baseline owner: [AI-Independent Editor-First Release Baseline and Handoff Criteria](ai-independent-editor-first-release-baseline-and-handoff-criteria.md)
- Deterministic-method ADR: [ADR 0012](../adr/0012-adopt-deterministic-contract-verification.md)
- Glossary: [CONTEXT.md](../../CONTEXT.md)

This document closes the deterministic proof-selection contract after the
editor-first Release 1 baseline. It does not add product behavior. It names
which existing owner contract is authoritative, which deterministic gate
proves each accepted obligation, how a proof is replayed, and when the result
blocks a stage or release. The crosswalk in section 12 is normative.

## 1. Authority, claim ceiling, and non-scope

### 1.1 Authority order

The accepted Release 1 baseline is the source of the `REL-*`, `S1-*`, `S2-*`,
`S3-*`, `S4-*`, and `HND-*` meanings. The owner contracts listed in section 4
remain the source of domain, authority, protocol, storage, retention,
disclosure, Agent, model, Tool, MCP, editor, trust, and Eval semantics. This
document owns only the deterministic verification boundary and failure/recovery
gate selection. A gate may prove an owner-defined fact; it may not redefine the
fact.

The exact baseline is intentionally recorded here so that a later proof cannot
silently mix a newer contract revision, a different route catalog, or a
different persistence identity with this crosswalk. A future release must
create a new contract revision and re-run the source, generated, wire, and
crosswalk checks.

### 1.2 What a deterministic gate may claim

The gates can prove StoryOS-owned facts such as:

- exact scope, identity, admission, idempotency, authorization, refusal,
  atomic settlement, Receipt, Author Action, Proposal state, Fence, replay,
  retention, lifecycle, restore visibility, recovery, disclosure ordering, and
  evidence availability;
- that a contract source, generated schema/catalog, golden wire record, and
  same-release identity agree within the accepted contract;
- that a contract-faithful fake destination follows the same Host, Context
  Assembly, manifest, Attempt, dispatch-fence, `OutcomeUnknown`,
  reconciliation, Proposal, and disclosure path as the real destination
  boundary; and
- that a protected negative path refuses, holds, or exposes uncertainty without
  creating an authority effect or an oracle leak.

The gates cannot prove a Provider's internal attention, training, retention,
model quality, literary merit, hidden SDK behavior, or any other fact outside
the StoryOS-owned boundary. Real-destination observations are advisory unless
the relevant Release 1 obligation expressly asks for that advisory class. A
completed verification gate is not a substitute for implementation, an
author journey, a physical recovery drill, or a later controlled-cloud
deployment proof.

### 1.3 Explicit non-scope

This contract does not:

- change the four stages, author journeys, product priority, required or absent
  capabilities, controlled-cloud position, or handoff order in [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62);
- redefine a domain term or owner contract, select a framework/vendor/provider/
  model/topology, or invent an SLA, timeout, performance, RPO/RTO, retention,
  or other numeric default;
- implement Rust, TypeScript, browser, server, worker, database, migration,
  scheduler, fake server, UI, CI, deployment, Tool, MCP, research, embedding,
  Memory, Skill, Subrun, or Eval execution;
- turn a Proposal, transcript, browser projection, local journal, model output,
  Tool/MCP result, or Eval view into authoritative state;
- make a real Provider's behavior a deterministic oracle; or
- modify [Map the StoryOS Editor-First Product and Production Delivery Contract](https://github.com/FrankQDWang/StoryOS/issues/1), [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60), [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62), [Create and Lock the First Editor-First Implementation Issue](https://github.com/FrankQDWang/StoryOS/issues/77), the Wayfinder map, or `.reference/**`.

The upstream `.reference/codex` tree is read-only reference material and is not
a StoryOS product dependency or verification input.

## 2. Deterministic proof model

### 2.1 Required proof ingredients

Every mandatory gate run has all of the following named before execution:

1. a fixed synthetic fixture with exact Project Scope and relevant identity
   generations;
2. the owner contract revision and source/generated/wire identity being
   exercised;
3. one or more semantic [Contract Fault Points](#7-contract-fault-point-registry),
   never an incidental SQL, function, queue, or source-line hook;
4. a virtual monotonic clock and deterministic interleaving schedule;
5. an independent oracle that computes the expected durable facts without
   trusting the implementation's result projection;
6. a safe replayable [Verification Evidence Bundle](#8-safe-replayable-evidence-bundles);
   and
7. a disposition from [section 3](#3-dispositions-and-fail-closed-blocking).

“Independent” means independently constructed expected state and invariants;
it does not require a second product implementation. The oracle must inspect
durable facts, typed records, digests, event generations, and sanitized
egress—not only a success HTTP response or a browser display.

### 2.2 Virtual time and interleaving

The deterministic scheduler controls every named yield at which a fault,
acknowledgement, Event, lease, fence, recovery, or external response can be
observed. It advances a virtual monotonic clock only through a named schedule
step. Wall-clock timing, sleeps, random retries, provider latency, and
uncontrolled process scheduling are not proof inputs. Contract-owned limit
profiles are used when a test reaches an accepted bound; this document creates
no new bound.

A schedule is replayable when its seed, ordered event names, virtual-clock
steps, fault-point selections, input digests, and expected oracle checkpoints
are all in the bundle. A schedule that cannot reproduce the same semantic
interleaving is `unreplayable` and blocks.

### 2.3 Contract-faithful fake destinations

The fake model and fake external destinations used by deterministic proof must
exercise the real StoryOS-owned Host, Project Scope, Context Assembly,
selection, bounded projection, manifest-before-egress, Attempt, dispatch
fence, `OutcomeUnknown`, reconciliation, Proposal, Receipt, and disclosure
boundaries. They may return fixed synthetic content; they may not bypass those
boundaries or stand in for Provider internals. A real destination may add
advisory evidence but cannot replace the fake/oracle proof.

### 2.4 Release identity and drift

The proof input is the exact same-release identity, not a semantic-version
string alone. At minimum, the proof records the owner-defined source revision,
generated schema/catalog digest, golden Application Wire Record digest, route
catalog identity/revision/digest, PostgreSQL storage compatibility identity,
migration-chain digest, and effective contract profile. A mismatch is drift,
not an invitation to update the crosswalk during a run.

## 3. Dispositions and fail-closed blocking

### 3.1 Gate dispositions

The following are the only gate-level result meanings used by this contract:

| Disposition | Meaning | Release effect |
| --- | --- | --- |
| `passed` | The scheduled positive facts match the independent oracle and the safe bundle replays. | Satisfies that proof obligation. |
| `expected_refusal` | The scheduled request is refused at the owner-defined boundary, with no unauthorized record, effect, disclosure, or oracle leak. | Satisfies a negative/refusal obligation only. |
| `expected_outcome_unknown` | The dispatch boundary is crossed, the result is intentionally unknown, and the durable Attempt/fence/reconciliation facts match the oracle. | Satisfies the unknown-boundary obligation; it is not success and must not be silently upgraded. |
| `expected_recovery_hold` | Recovery preserves uncertainty or a Draft/reconfirmation hold without claiming an unproven commit. | Satisfies a recovery-hold obligation only. |
| `failed` | An expected fact is contradicted, an unauthorized effect occurs, or an oracle mismatch is found. | Blocks. |
| `unverified` | The evidence is missing, incomplete, not classifiable, or cannot establish the expected fact. | Blocks. |
| `advisory` | Useful observation outside the deterministic proof boundary, including real-destination evidence. | Never upgrades a mandatory gate. |

### 3.2 Evidence status and exact blocking result

The Release 1 baseline uses these evidence statuses: `Passed`, `Failed`,
`Unrun`, `Stale`, `Unavailable`, `Unreplayable`, and operational
`OutcomeUnknown`. For every mandatory row in the crosswalk:

`Failed`, `Unrun`, `Stale`, `Unavailable`, `Unverified`, or `Unreplayable`
means **the obligation is blocked, the affected stage cannot release, and no
later advisory or green parent check may upgrade it**. `OutcomeUnknown` is a
truthful operational state: it satisfies only a row whose expected result is
the unknown boundary and otherwise remains blocking until its owner-defined
reconciliation or recovery decision settles it. `expected_refusal` and
`expected_recovery_hold` pass only when the refusal/hold itself is the stated
oracle expectation. A missing mandatory bundle is `Unverified`, not `Passed`.

The shorthand `BLOCK-ALL` in the crosswalk expands to that exact rule. It is
used to make the 72-row crosswalk auditable without weakening the result
semantics.

### 3.3 No circular evidence

An implementation result, UI state, generated report, or Eval projection may
be included as evidence, but it cannot be the only oracle for the fact it
produced. `EV-SE` and its lower evidence layers are mandatory inputs to a
stage implementation proof; `EV-SR` is output-only and is never admitted into
an implementation map or used to certify that map. The release selector
consumes the already-passed mandatory map and the selected author journey,
then emits `EV-SR`. Stage evidence and stage release therefore consume the
applicable gate bundles; they do not certify themselves. A physical restore
drill must inspect the restored service and lifecycle facts independently of
the backup job's own success flag.

## 4. Accepted owners and proof inputs

The owner key in the crosswalk resolves to the following accepted contract. A
link identifies the source of meaning; this document does not copy or amend
that source.

| Owner key | Accepted owner and boundary |
| --- | --- |
| `OWN-REL` | [AI-Independent Editor-First Release Baseline and Handoff Criteria](ai-independent-editor-first-release-baseline-and-handoff-criteria.md), including `REL-*`, stage, evidence, and handoff meanings. |
| `OWN-ADM` | [Author Command Admission](author-command-admission.md), including exact binding, nonce/idempotency, refusal, lifecycle, and terminal settlement. |
| `OWN-CORE` | [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md), including atomic Core edit/Proposal outcomes, Acceptance, Rejection, Receipt, Author Action, and undo. |
| `OWN-WEB` | [Web Editor Session, Synchronization, and Recovery Semantics](web-editor-session-synchronization-and-recovery-semantics.md), including input, IME, Local Edit Journal, grouping, projection, acknowledgement/Event convergence, Snapshot/resync, takeover, and Recovery Draft. |
| `OWN-PROTO` | [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md), including public route/source/generation/wire identity, Host/Origin/session, scope, limits, errors, Attempts, and exports. |
| `OWN-PG` | [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md), including transaction atomicity, forced RLS, roles, compatibility, migrations, projections, backup, restore, Recovery Copy, and physical recovery profile. |
| `OWN-RET` | [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md), including Seal/Fence, settlement completeness, compaction, replay generations, cursor floor, archival, deletion, invalidation, gaps, and unavailable-content export. |
| `OWN-CTX` | [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md), including the seven assembly gates, manifest, bounded projection, destination identity, disclosure, Attempt, and reconciliation. |
| `OWN-MEM` | [Fiction Memory and Research Provenance Semantics](fiction-memory-and-research-provenance-semantics.md), including Memory/Research/embedding boundaries, provenance, suppression, and projection rebuild. |
| `OWN-PLAIN` | [Plain-Language Discovery-Writing Assistance Semantics](plain-language-discovery-writing-assistance-semantics.md), including intent interpretation, present-passage focus, Prose Change Request, Proposal handoff, and no manufactured authorization. |
| `OWN-TRUST` | [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md), including protected-client claims, security evidence classes, non-oracular refusal, minimum disclosure, and attack-path handoff. |
| `OWN-EVAL` | [Foundation Evidence for the Standalone Eval Surface](eval-evidence-foundation.md), including read-only Eval View, explicit assessment boundaries, availability/redaction/gaps, and no hidden authority or egress. |
| `OWN-GOV` | [Modular-Monolith and Repository Governance Boundaries](modular-monolith-and-repository-governance-boundaries.md), including source/generation ownership, repository/reference isolation, and final verification handoff. |
| `OWN-AGENT` | [Specify Persistent Agent Run and Orchestration Semantics](https://github.com/FrankQDWang/StoryOS/issues/47), the one general Project-scoped Agent Loop and AgentRun owner. This key does not create a task-specific runtime or add a new Agent contract. |
| `OWN-MODEL` | [Specify ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50), the Model Gateway/Provider owner. Provider internals and model-quality claims remain outside deterministic proof. |
| `OWN-TOOL` | [Specify ToolSpec, Capability, Approval, and MCP Trust Semantics](https://github.com/FrankQDWang/StoryOS/issues/48), the ToolSpec/capability/approval owner. Tool execution is explicitly absent from Stages 3 and 4. |
| `OWN-MCP` | [Specify ToolSpec, Capability, Approval, and MCP Trust Semantics](https://github.com/FrankQDWang/StoryOS/issues/48), the MCP/MCP App trust owner. MCP execution is explicitly absent from Stages 3 and 4. |
| `OWN-STAGE` | Release-stage and handoff owner `OWN-REL`; this key is used when a row checks stage evidence sequencing rather than a domain transition. |
| `OWN-MEASURE` | A named owner-adopted measurement contract, including [Measure the Representative Writing-Path Performance and Storage-Growth Envelope](https://github.com/FrankQDWang/StoryOS/issues/76) where applicable. No measurement row may invent a target value. |
| `OWN-DVG` | This document and [ADR 0012](../adr/0012-adopt-deterministic-contract-verification.md), limited to proof selection, oracle, fault-point, schedule, bundle, disposition, and handoff semantics. |

The tracked Release 1 catalog and verifier inputs are:

- [PostgreSQL Release 1 persistence catalog](postgresql-release-1-persistence-catalog.json)
  and its checked-in verifier/self-test;
- [Versioned Protocol Release 1 route catalog](versioned-protocol-release-1-route-catalog.json)
  and its checked-in verifier/self-test; and
- the source contracts, generated outputs, golden wire corpus, and exact
  same-release identity owned by `OWN-PROTO` and `OWN-PG`.

The catalog files are executable verification inputs. Their parse/self-tests
are necessary catalog checks, not a substitute for the crosswalk or for later
product implementation evidence.

## 5. Evidence layers and release evidence classes

### 5.1 Proof layers

These layer keys distinguish what kind of evidence a gate consumes. They must
not be collapsed into one green check.

| Layer | Meaning | Can it satisfy a mandatory Release 1 row? |
| --- | --- | --- |
| `EV-CP` | Deterministic contract proof: independent oracle, named fault point, virtual schedule, replayable bundle. | Yes, for contract facts and negative/refusal facts. |
| `EV-IT` | Implementation test evidence at the public or owner-defined implementation boundary. | Yes only when the row also requires implementation evidence and the owner has supplied it. |
| `EV-INT` | Integration/E2E evidence for a production-shaped service/editor journey. | Yes for journey and cross-boundary behavior; not by itself for a missing deterministic oracle. |
| `EV-PRD` | Physical recovery drill, such as isolated Recovery Copy/PITR restore, role/RLS, lifecycle, projection, visibility, and continued writing. | Yes for physical-recovery rows; not for deterministic contract rows alone. |
| `EV-RDA` | Real-destination advisory evidence, including one real model route. | Advisory only unless the baseline explicitly calls for that observation; never a deterministic oracle. |
| `EV-SE` | Attributable stage evidence assembled at the exact implementation baseline. | A release input, not an independent substitute for its underlying proof. |
| `EV-SR` | Stage-release disposition after the author journey and all mandatory evidence pass. | The resulting release decision only. |
| `EV-CCD` | Later controlled-cloud deployment evidence for identity, security, recovery, cache, upgrade, and same-release behavior. | Not part of local Release 1 stage release; required only for the later controlled-cloud handoff. |

### 5.2 Accepted Release 1 evidence classes

The following names are the baseline's accepted evidence classes. `EC-*` is a
classification, not a new product artifact.

| Class | Baseline evidence class | Typical layers |
| --- | --- | --- |
| `EC-01` | Contract crosswalk | `EV-CP`, `EV-SE` |
| `EC-02` | Browser author journey | `EV-INT`, `EV-SE`, with `EV-CP` support |
| `EC-03` | Core and persistence settlement | `EV-CP`, `EV-IT`, `EV-INT` |
| `EC-04` | Recovery and replay | `EV-CP`, `EV-INT`, `EV-PRD` |
| `EC-05` | Isolation and trust | `EV-CP`, `EV-IT`, `EV-INT`, `EV-PRD` |
| `EC-06` | Performance and storage growth | `EV-IT`, `EV-INT`, `EV-SE`; only named owner-adopted values |
| `EC-07` | Proposal and author decision | `EV-CP`, `EV-INT`, `EV-SE` |
| `EC-08` | Disclosure and destination | `EV-CP`, `EV-INT`, `EV-RDA` |

### 5.3 Result vocabulary used in the crosswalk

| Shorthand | Exact passing condition |
| --- | --- |
| `PASS-POS` | `passed`: every scheduled positive fact matches the oracle and the named bundle replays. |
| `PASS-REFUSAL` | `expected_refusal`: refusal is the scheduled oracle result, with no Admission/authority/effect/disclosure and a non-oracular response. |
| `PASS-UNKNOWN` | `expected_outcome_unknown`: the dispatch claim is durable, the uncertainty is explicit, the fence is active, and reconciliation is separately admitted; this is not success. |
| `PASS-HOLD` | `expected_recovery_hold`: recovery exposes the unresolved state or Recovery Draft/reconfirmation boundary without claiming commit. |
| `PASS-STAGE` | The row's mandatory evidence is current, attributable, replayable, and passed at the exact implementation baseline; stage release still requires the author journey. |
| `PASS-CLOUD` | Later controlled-cloud evidence satisfies the separately owned deployment handoff; it does not backfill a local-stage gap. |
| `BLOCK-ALL` | `failed`, `unrun`, `stale`, `unavailable`, `unverified`, or `unreplayable` mandatory evidence blocks the row and affected stage; advisory evidence cannot upgrade it. |

## 6. Fixture registry

Fixtures contain synthetic identifiers, short synthetic text, digests, and
redacted values only. They never require a real manuscript, credential,
foreign identity, raw transport capture, or Provider telemetry.

| Fixture | Scope and purpose |
| --- | --- |
| `FX-CONTRACT-R1` | Exact Release 1 source/generated/catalog/golden-wire identity, compatibility profile, and drift alternatives. |
| `FX-SCOPE-2U2P` | Two synthetic Users and two Projects with distinct exact `ProjectScope`, Host/Origin, Client Session, Editor Session, writer, and generation bindings. |
| `FX-EDITOR-IME` | One controlled Project/chapter with synthetic Chinese and English text, composition, keyboard, clipboard, selection replacement, cut/paste, delete, undo, and split/join intents. |
| `FX-JOURNAL-GROUP` | Immutable Local Edit Journal records, completed intent/group boundaries, idempotency keys, pending projections, and bounded duplicate/reordered acknowledgements. |
| `FX-CORE-PROPOSAL` | Direct edit, generated Proposal, editable Proposal revision, Acceptance, Rejection, conflict, refusal, NoEffect, Receipt, Author Action, and undo frontiers. |
| `FX-RECOVERY-EDITOR` | Settled and unsettled editor commands across reload, client crash, Server restart, PostgreSQL restart, writer takeover, Recovery Draft, and reconfirmation. |
| `FX-REPLAY-RETENTION` | Mailbox/Event/Activity facts with Seal, Fence, high-watermarks, compacted payload, replay generations, Snapshot, cursor floor, gaps, archival, redaction, and unavailable content. |
| `FX-RESTORE-LIFECYCLE` | Isolated Recovery Copy/PITR restore with roles/RLS, projection rebuild, lifecycle/deletion facts, Recovery Visibility Proof, continued writing, export, and non-revival assertions. |
| `FX-CONTEXT-DISCLOSURE` | Bounded current request, Host/Scope, source eligibility, selection/projection, Context Assembly Manifest, Disclosure Manifest, destination identity, Attempt, usage, and digest facts. |
| `FX-FAKE-MODEL` | Contract-faithful fake model through the normal Host/assembly/manifest/Attempt/fence/Proposal/Receipt path, including refusal and unknown outcomes. |
| `FX-REAL-MODEL-ADVISORY` | One registered Provider-neutral real model destination with synthetic prompt/result and no claim about Provider internals or quality. |
| `FX-ABSENT-EXECUTION` | Requests attempting bounded Tool, MCP, research, embedding, Memory, Skill, Subrun, or Eval execution in Stages 3/4; expected refusal/no effect. |
| `FX-EVAL-READONLY` | Eval View read/refresh, explicit case and assessment, redacted/unavailable evidence, and assessment OutcomeUnknown without hidden execution. |
| `FX-LONG-SESSION` | Owner-adopted long-session/repeated-chapter/reload/controlled-upgrade measurement envelope; no new numeric target. |
| `FX-HANDOFF` | Exact baseline, contract revisions, release IDs, gate bundles, stage disposition, next bounded issue, and later-cloud boundary. |

## 7. Contract Fault Point registry

A Contract Fault Point is a semantic boundary named by an owner contract. It
specifies the durable facts that must exist before and after the cut and the
permitted recovery classification. It is not a source-line, SQL statement,
queue implementation, or framework hook. Every mandatory proof selects at
least one point from this registry.

| Fault point | Semantic cut and required expectation |
| --- | --- |
| `CFP-CONTRACT-BEFORE-GENERATION` | Source contract is present before generation; missing/changed source cannot produce a green generated identity. |
| `CFP-CONTRACT-AFTER-GENERATION-BEFORE-WIRE` | Generated schema/catalog exists before golden wire projection; drift is detected before same-release acceptance. |
| `CFP-CONTRACT-DRIFT` | Any source/generated/catalog/wire/release identity mismatch is refused or marked unverified, never silently refreshed. |
| `CFP-SCOPE-BEFORE-QUERY` | Exact User/Project Scope and requester binding precede read, write, ranking, restore, export, or disclosure. |
| `CFP-ADMISSION-BEFORE-CORE` | Admission is final and exact before a direct Core effect. A mismatch/expiry has no Core effect. |
| `CFP-ADMISSION-EXPIRY` | Expiry is crossed before invocation or during recovery; no automatic retry may create a new command. |
| `CFP-CORE-BEFORE-COMMIT` | Core transaction has not committed; the oracle expects no partial authority, Receipt, Action, or outbox settlement. |
| `CFP-CORE-AFTER-COMMIT-BEFORE-ACK` | Core transaction is committed but the response/ack is absent; recovery reads durable settlement rather than guessing. |
| `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY` | Browser input is not yet durable in the Local Edit Journal; no later projection may claim it was submitted. |
| `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP` | Intent is durable but not yet frozen/admitted; only equal pre-admission bindings may coalesce. |
| `CFP-EDITOR-BEFORE-GROUP-ADMISSION` | A frozen group has not received admission; interruption leaves it pending/needs-attention without authority. |
| `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE` | Exact admitted group is waiting for Core; retry uses the same idempotency identity only under owner rules. |
| `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK` | Durable settlement exists before browser acknowledgement. Only an Activity-bearing result can wait for Activity delivery; Receipt-only results converge from the exact response or settlement query without inventing Activity. Replay must converge without duplication. |
| `CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL` | A valid `getApplyAuthorEditOutcome` response exists only in memory, and the complete Journal query observation has not committed. The prior durable reconciliation state stays authoritative. No saved, rejected, settled, queue-release, Receipt, Activity, or authority projection is visible. The client may repeat only the protected outcome GET. |
| `CFP-PROPOSAL-BEFORE-DECISION` | Proposal is inspectable but no Acceptance/Rejection/Withdrawal decision has occurred. |
| `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT` | Acceptance transaction boundary is crossed without a visible Receipt; only Core facts decide Applied/Invalid/Conflicted/Refused/NoEffect. |
| `CFP-UNDO-BEFORE-SETTLEMENT` | Undo request is admitted but not settled; no compensating authority effect may be assumed. |
| `CFP-OUTBOX-BEFORE-CLAIM` | Durable outbox record exists but no delivery claim; replay may claim once under the owner fence. |
| `CFP-OUTBOX-AFTER-CLAIM-BEFORE-ACK` | Delivery claim exists before consumer acknowledgement; duplicate/replay must be idempotent and observable. |
| `CFP-MANIFEST-BEFORE-COMMIT` | Required Context/Disclosure Manifest is not committed; external IO is forbidden. |
| `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS` | Manifest is committed before bytes leave StoryOS; the wire must match the manifest and scope. |
| `CFP-DISPATCH-BEFORE-CLAIM` | No external dispatch claim exists; no external bytes or attempt success may be reported. |
| `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO` | Dispatch claim/Attempt exists before IO; a crash yields a durable unknown boundary, not a failure guess or blind resend. |
| `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION` | Bytes may have left but confirmation is absent; the Attempt remains `OutcomeUnknown` until normal reconciliation. |
| `CFP-RECONCILIATION-BEFORE-SETTLEMENT` | Reconciliation is separately admitted and has not settled the original unknown; no duplicate original Attempt is allowed. |
| `CFP-LEASE-AFTER-EXPIRY` | Old lease is expired; the old worker cannot settle after the new owner/fence. |
| `CFP-FENCE-AFTER-TAKEOVER` | Writer/worker recovery fence is active; late work is rejected or recorded as late, never applied by stale ownership. |
| `CFP-LATE-RESULT` | A stale, duplicate, or late result arrives after settlement/fence; it cannot mutate authority or erase uncertainty history. |
| `CFP-MAILBOX-BEFORE-SEAL` | Root mailbox settlement is incomplete; compaction/archive/deletion cannot claim completeness. |
| `CFP-MAILBOX-AFTER-SEAL` | Seal and high-watermarks are durable; duplicate/reordered/late events cannot reopen or rewrite settlement. |
| `CFP-MAILBOX-LATE-DUPLICATE` | A late or duplicate event after Seal is rejected/recorded with truthful provenance and no new settlement. |
| `CFP-REPLAY-BEFORE-COMPACTION` | Operational Evidence Floor and required terminal facts precede compaction. |
| `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT` | New replay generation/Snapshot exists; old generation cursors cannot be guessed into a new stream. |
| `CFP-REPLAY-BELOW-FLOOR` | Cursor below the floor receives typed cursor-too-old plus fresh Snapshot/resync, never fabricated events. |
| `CFP-LIFECYCLE-BEFORE-INVALIDATION` | Redaction/Tombstone/Suppression/Archive/Deletion has not committed; current eligibility remains owner-defined. |
| `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP` | Current inspection/retrieval/cache/export/egress is invalidated before physical cleanup; historical evidence stays truthful. |
| `CFP-RESTORE-STAGING` | Recovery Copy/PITR is isolated and staged; no ordinary read, write, merge, remap, or revival is exposed. |
| `CFP-RESTORE-BEFORE-VISIBILITY` | Lifecycle, role/RLS, migration, and projection proof is incomplete; ordinary reads/execution remain held. |
| `CFP-RESTORE-AFTER-VISIBILITY` | Recovery Visibility Proof has passed; exact scope may resume ordinary writing under the restored authority. |
| `CFP-DELETE-BEFORE-SETTLEMENT` | Deletion request fences new work and settles known/unknown work before irreversible completion. |
| `CFP-DELETE-AFTER-SETTLEMENT` | Deleted scope is tombstoned/purged according to the owner contract and cannot be restored or revived. |

## 8. Safe replayable evidence bundles

Every bundle contains the exact source/revision/profile identifiers, synthetic
fixture seed, virtual schedule, selected fault points, oracle version, durable
fact digests, sanitized diagnostics, and disposition. It excludes real
manuscript content, credentials, raw external transport, foreign identities,
ambient telemetry, and Provider-private data.

| Bundle | Safe contents and replay boundary |
| --- | --- |
| `B-CONTRACT` | Source/generated/catalog/golden-wire/release identity digests, parser result, drift mutation, and expected catalog facts. |
| `B-SCOPE` | Pseudonymous two-user/two-project bindings, request envelope digests, RLS/refusal facts, and no content bytes. |
| `B-EDITOR` | Synthetic input event digests, IME composition states, journal/group identities, projection states, ack/Event order, and convergence facts. |
| `B-CORE` | Command/Proposal/Acceptance/undo digests, atomic write-set fact names, typed Receipt/Action causes, and revision frontiers. |
| `B-RECOVERY` | Fault cut, restart/takeover/fence sequence, journal and durable settlement digests, Recovery Draft/reconfirmation state, and no raw prose. |
| `B-REPLAY` | Seal/Fence/high-watermark/generation/floor/Snapshot/gap/late-event facts, compacted-content digests, and availability classifications. |
| `B-RESTORE` | Isolated restore identity, role/RLS/migration/projection/lifecycle checks, Recovery Visibility result, continued-writing proof, and non-revival facts. |
| `B-CONTEXT` | Request purpose, scope, candidate identifiers/digests, selection/projection bounds, manifests, Attempt/fence, disclosure categories, and wire digest. |
| `B-FAKE` | Fake destination script, synthetic result digest, host/manifest/Attempt/Proposal/Receipt facts, and unknown/reconciliation sequence. |
| `B-REAL-ADVISORY` | Real destination identity, registration/binding/profile references, disclosed categories, Attempt/outcome facts, and sanitized advisory observation only. |
| `B-ABSENT` | Requested absent capability, refusal code/class, scope, no-effect fact, and non-oracular response shape. |
| `B-EVAL` | Read-only Eval View query, evidence availability/redaction/gap facts, explicit assessment boundary, and no-egress/no-authority result. |
| `B-MEASURE` | Owner-adopted measurement profile, sampled synthetic workload, measured facts, and provenance; no invented target. |
| `B-HANDOFF` | Crosswalk IDs, gate dispositions, exact baseline/tree, source revisions, bundle digests, stage decision, and next owner/issue boundary. |
| `B-S1-MANDATORY-SET` | Exact Stage 1 mandatory bundle set; members are `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, and `B-HANDOFF`. It contains no physical-restore, fake-model, real-destination, Eval, or Stage 2-only bundle. |
| `B-S2-MANDATORY-SET` | Exact Stage 2 mandatory bundle set and complete AI-independent editor journey set; members are `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-ABSENT`, `B-MEASURE`, and `B-HANDOFF`. |
| `B-S3-MANDATORY-SET` | Exact Stage 3 mandatory bundle set, formed from the complete Stage 2 set plus the fake-model and read-only Eval/absence boundaries; members are `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-EVAL`, `B-MEASURE`, and `B-HANDOFF`. It contains no real-destination advisory bundle. |
| `B-S4-MANDATORY-SET` | Exact Stage 4 mandatory bundle set, formed from the complete Stage 3 deterministic set plus one real-destination advisory boundary; members are `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-EVAL`, `B-MEASURE`, `B-HANDOFF`, and `B-REAL-ADVISORY`. `B-FAKE` is a contract-faithful deterministic proof fixture, not a second Provider or user-facing route; real-destination evidence cannot replace it or `B-EVAL`. |
| `B-S2-JRN-001-EDITOR-RELEASE-SET` | Exact alias for the complete Stage 2 journey set; members are `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-ABSENT`, `B-MEASURE`, and `B-HANDOFF`. It is retained only for the `S2-JRN-001` row. |
| `B-S4-REQ-006-AUTHOR-JOURNEY-SET` | Exact Stage 4 author-journey set; members are exactly the Stage 4 mandatory proof members: `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-EVAL`, `B-MEASURE`, `B-HANDOFF`, and `B-REAL-ADVISORY`. The real route therefore carries the complete Stage 3 deterministic boundary as well as the complete Stage 2 editor evidence. |
| `B-S4-JRN-001-SET` | Exact Stage 4 journey set; members are exactly `B-S4-MANDATORY-SET`'s members: `B-CONTRACT`, `B-SCOPE`, `B-EDITOR`, `B-CORE`, `B-RECOVERY`, `B-REPLAY`, `B-RESTORE`, `B-CONTEXT`, `B-FAKE`, `B-ABSENT`, `B-EVAL`, `B-MEASURE`, `B-HANDOFF`, and `B-REAL-ADVISORY`. The model-disabled regression remains separately selected as `B-S2-MANDATORY-SET`. |

## 9. Deterministic schedule registry

Each schedule is a named interleaving, not a timing target. The scheduler
pauses at every selected fault point and emits an oracle checkpoint before
continuing.

| Schedule | Virtual-clock/interleaving sequence |
| --- | --- |
| `SCH-NORMAL` | Bind exact scope/session → admit → perform owner transition → commit durable facts → emit the typed Receipt and, only for an Activity-bearing result, its Event → acknowledge → replay and compare. |
| `SCH-REORDER` | Commit an Activity-bearing result → deliver Event before acknowledgement → deliver acknowledgement before Event → duplicate both → resync from Snapshot; compare one settled effect and one Activity history. Receipt-only results run the acknowledgement/query permutations without an Event. |
| `SCH-CRASH` | Pause at selected pre/post commit or journal cut → terminate the active process → restart from durable facts → replay/reconcile → inspect no partial/duplicate effect. |
| `SCH-FENCE` | Establish old writer/worker → expire lease or issue takeover → advance recovery Fence → deliver old completion → reject/record late result → settle only current owner. |
| `SCH-REPLAY` | Seal/settle root facts → create next replay generation/Snapshot → present old cursor and below-floor cursor → return typed gap/cursor-too-old → resync without guessing. |
| `SCH-RESTORE` | Stage isolated Recovery Copy/PITR → verify role/RLS/migration/lifecycle/projections → hold ordinary visibility → pass Recovery Visibility Proof → continue writing in exact scope. |
| `SCH-LIFECYCLE` | Start eligible record → commit redaction/suppression/archive/deletion at selected cut → attempt current read/retrieval/cache/export/egress → inspect historical truth, gap, invalidation, and non-revival. |
| `SCH-UNKNOWN` | External dispatch: commit manifest and dispatch claim → cut before/after external IO or response → persist `OutcomeUnknown` → forbid blind resend → separately admit reconciliation/new Attempt → settle truthful outcome. Author Command acknowledgement loss: commit the frozen group and protected capsule → cut before Admission, after Admission before Core, before command commit, after commit before acknowledgement, at acknowledgement delivery, or after outcome response before Journal → call `getApplyAuthorEditOutcome` under fixed virtual time → retain or append exact Journal evidence → repeat only that safe GET; no POST replay, process termination, or restart. |
| `SCH-SCOPE` | Run valid scope → substitute foreign user/project/Host/Origin/session/generation/credential/record → refuse non-oracularly → verify zero cross-scope effect/disclosure. |
| `SCH-DRIFT` | Mutate source, generated schema, catalog, golden wire, migration identity, or release identity one at a time → detect mismatch → hold activation/release. |
| `SCH-ABSENT` | Request one absent capability in normal and bounded variants → refusal/no-effect/non-oracular response → repeat with each excluded capability without execution or hidden fallback. |
| `SCH-LONG` | Use the owner-adopted long-session/repeated-chapter/reload/upgrade workload and its measured checkpoints; compare only to adopted values and storage facts. |

## 10. Gate catalogue

The `DVG-*` identifiers and their meanings are stable. Their gate sets may be
composed by the crosswalk, but a later contract revision must not silently
repurpose an identifier.

| Gate | Owner boundary and proof | Default fixture · fault points · schedule · oracle | Safe bundle and pass/block |
| --- | --- | --- | --- |
| `DVG-01` Contract source, generated schema/catalog, golden wire, same-release identity, bounds, and drift | `OWN-PROTO` + `OWN-PG` + `OWN-GOV`; proves source-to-generated-to-wire/catalog parity, exact release identity, accepted limits, and fail-closed drift. For `storyos.author-edit-batch.release-1.preview.v1`, it checks the #70-owned structured source, exact `storyos.editor-contract.release-1.v2` mapping, captured synthetic evidence binding, and document projections; it does not treat prose presence as semantic proof. | `FX-CONTRACT-R1` · `CFP-CONTRACT-BEFORE-GENERATION`, `CFP-CONTRACT-AFTER-GENERATION-BEFORE-WIRE`, `CFP-CONTRACT-DRIFT` · `SCH-DRIFT` · `ORC-CONTRACT`. | `B-CONTRACT`; `PASS-POS` for parity or `PASS-REFUSAL`/`PASS-HOLD` for drift. `BLOCK-ALL`. |
| `DVG-02` Request/query/cursor/Project Scope isolation | `OWN-PROTO` + `OWN-ADM` + `OWN-PG` + `OWN-TRUST`; proves exact User/Project Scope, Host/Origin/session generations, requester derivation, bounds, cursor handling, forced RLS, and non-oracular refusal. A prospective Author Edit unit, primitive, body, or policy-mapping failure must occur before Admission, nonce consumption, Core, or Domain Receipt. | `FX-SCOPE-2U2P` · `CFP-SCOPE-BEFORE-QUERY`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-STAGING` · `SCH-SCOPE`, `SCH-REPLAY` · `ORC-SCOPE`. | `B-SCOPE`; `PASS-POS` for valid scope, `PASS-REFUSAL` for substitution, or `PASS-HOLD` for invalid recovery visibility. `BLOCK-ALL`. |
| `DVG-03` Author edit, Web Editor Session, Proposal, Acceptance/Rejection, Receipt, Author Action, and undo transaction | `OWN-WEB` + `OWN-ADM` + `OWN-CORE` + `OWN-PLAIN`; proves browser input becomes only the owner-defined journal/group/command, Web rejects incomplete or mismatched local coverage before challenge, and Core outcomes are atomic. For Author Edit, only `AuthoritativeApplied` creates Project Activity, an Author Action, a Revision/Head advance, a checkpoint, projection convergence, or base roll-forward. An admitted `Refused`, `Conflicted`, or `NoEffect` keeps one typed zero-authority Receipt; a zero-authority Receipt creates no Project Activity. An infrastructure or transaction failure before commit is Receipt-free and also creates none of those facts. No path has prefix authority. | `FX-EDITOR-IME`, `FX-JOURNAL-GROUP`, `FX-CORE-PROPOSAL` · `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-ADMISSION-BEFORE-CORE`, `CFP-CORE-BEFORE-COMMIT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-UNDO-BEFORE-SETTLEMENT` · `SCH-NORMAL`, `SCH-CRASH`, `SCH-REORDER` · `ORC-EDITOR-JOURNAL`, `ORC-ATOMIC-AUTHORITY`. | `B-EDITOR` + `B-CORE`; `PASS-POS` for applied and admitted zero-authority settlements, `PASS-REFUSAL` only for a pre-Admission refusal, or `PASS-HOLD` only for an owner-defined recovery hold. In this contract, an admitted zero-authority settlement is `PASS-POS`. Any Project Activity attached to a zero-authority Author Edit Receipt fails the gate. `BLOCK-ALL`. |
| `DVG-04` Context Assembly, retrieval, Memory/Research eligibility, bounded projection, and disclosure | `OWN-CTX` + `OWN-MEM` + `OWN-PLAIN`; proves ordered seven-gate assembly, exact scope, source eligibility, bounded projection, manifest-before-egress, provenance, and re-entry boundaries. | `FX-CONTEXT-DISCLOSURE` · `CFP-SCOPE-BEFORE-QUERY`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP` · `SCH-SCOPE`, `SCH-LIFECYCLE`, `SCH-UNKNOWN` · `ORC-CONTEXT-DISCLOSURE`. | `B-CONTEXT`; `PASS-POS` or `PASS-REFUSAL` when eligibility/disclosure is denied. `BLOCK-ALL`. |
| `DVG-05` Model Gateway, Provider, Tool, MCP, and MCP App mediation | `OWN-CTX` + `OWN-MODEL` + `OWN-TOOL` + `OWN-MCP` + `OWN-TRUST`; proves destination identity/grant/capability/credential/policy, manifest, Attempt, disclosure, and no direct authority. For Stage 3/4 absent execution it proves refusal and zero fallback, not execution. | `FX-FAKE-MODEL`, `FX-REAL-MODEL-ADVISORY`, `FX-ABSENT-EXECUTION` · `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-LATE-RESULT` · `SCH-UNKNOWN`, `SCH-ABSENT`, `SCH-SCOPE` · `ORC-DISPATCH-DISCLOSURE`. | `B-FAKE` for deterministic proof, `B-REAL-ADVISORY` for advisory observation, `B-ABSENT` for excluded capabilities; `PASS-POS`, `PASS-UNKNOWN`, or `PASS-REFUSAL`. `BLOCK-ALL`; advisory never upgrades. |
| `DVG-06` AgentRun, Subrun, Mailbox, and finalization boundary | `OWN-AGENT` + `OWN-RET` + `OWN-CORE`; proves one general Project Agent Loop, bounded run/step/mailbox settlement, finalization, fence, and no task-specific authority path. | `FX-FAKE-MODEL`, `FX-ABSENT-EXECUTION` · `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-MAILBOX-LATE-DUPLICATE`, `CFP-FENCE-AFTER-TAKEOVER` · `SCH-FENCE`, `SCH-REPLAY`, `SCH-ABSENT` · `ORC-RUN-FINALIZATION`. | `B-FAKE` or `B-ABSENT`; `PASS-POS`, `PASS-REFUSAL`, or `PASS-HOLD`. `BLOCK-ALL`. |
| `DVG-07` Transaction/outbox/lease/fence/crash recovery | `OWN-CORE` + `OWN-PG` + `OWN-WEB` + `OWN-RET`; proves atomic write sets, outbox order, idempotency, lease/fence ownership, process/crash/restart recovery, writer takeover, and no stale settlement. | `FX-RECOVERY-EDITOR`, `FX-CORE-PROPOSAL` · `CFP-CORE-BEFORE-COMMIT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-OUTBOX-BEFORE-CLAIM`, `CFP-OUTBOX-AFTER-CLAIM-BEFORE-ACK`, `CFP-LEASE-AFTER-EXPIRY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT` · `SCH-CRASH`, `SCH-FENCE`, `SCH-REORDER` · `ORC-RECOVERY-ATOMICITY`. | `B-RECOVERY` + `B-CORE`; `PASS-POS`, `PASS-HOLD`, or `PASS-REFUSAL`. `BLOCK-ALL`. |
| `DVG-08` `OutcomeUnknown`, reconciliation, Attempts, and late settlement | External-dispatch branch: `OWN-PROTO` + `OWN-CTX` + `OWN-PG` + `OWN-RET`; keeps durable claim, separately admitted reconciliation/new Attempt, fence, and truthful late-result meaning. Author Command branch: `OWN-PROTO` + `OWN-PG` + `OWN-WEB` + `OWN-ADM` + `OWN-CORE`; reads the existing durable command identity through `getApplyAuthorEditOutcome` and never creates a reconciliation command or POST Attempt. | External dispatch keeps its destination fixtures and dispatch/reconciliation/late-result cuts. Author Command selects `FX-CONTRACT-R1`, `FX-JOURNAL-GROUP`, `FX-SCOPE-2U2P`; the Admission, Core, acknowledgement, outcome-response-before-Journal, and Scope cuts; `SCH-NORMAL`, `SCH-REORDER`, `SCH-SCOPE`, `SCH-UNKNOWN`; and `ORC-CONTRACT`, `ORC-EDITOR-JOURNAL`, `ORC-ATOMIC-AUTHORITY`, `ORC-OUTCOME-UNKNOWN`, `ORC-NEGATIVE-CLOSURE`, `ORC-SCOPE`. | External dispatch keeps `PASS-UNKNOWN` until owner-defined reconciliation. Author Command uses `PASS-POS` for `Committed`, `PASS-REFUSAL` only for public `Rejected` or the scheduled negative gate, and `PASS-HOLD` for `StillUnknown` or Query failure. It emits `B-CONTRACT` + `B-EDITOR` + `B-CORE` + `B-SCOPE`. `BLOCK-ALL`; unresolved state blocks settlement and dependent submission. |
| `DVG-09` Replay, compaction, redaction, archival, generation, floor, and unavailable content | `OWN-RET` + `OWN-PROTO` + `OWN-MEM`; proves Seal/Fence/settlement completeness, operational evidence floor, replay generations, Snapshot/cursor-too-old, redaction invalidation, archival, gaps, and truthful unavailable-content export. | `FX-REPLAY-RETENTION` · `CFP-MAILBOX-BEFORE-SEAL`, `CFP-MAILBOX-AFTER-SEAL`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP` · `SCH-REPLAY`, `SCH-LIFECYCLE`, `SCH-REORDER` · `ORC-REPLAY-TRUTH`. | `B-REPLAY`; `PASS-POS` or `PASS-REFUSAL` for stale/invalid cursor and lifecycle access. `BLOCK-ALL`. |
| `DVG-10` Archive, restore, Recovery Visibility, project deletion, and non-revival | `OWN-PG` + `OWN-RET` + `OWN-PROTO` + `OWN-TRUST`; proves isolated Recovery Copy/PITR, roles/RLS, migrations, projection rebuild, lifecycle gaps, Recovery Visibility Proof before ordinary reads, continued writing, export/portability, deletion settlement, and non-revival. | `FX-RESTORE-LIFECYCLE` · `CFP-RESTORE-STAGING`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DELETE-AFTER-SETTLEMENT` · `SCH-RESTORE`, `SCH-LIFECYCLE`, `SCH-SCOPE` · `ORC-RESTORE-LIFECYCLE`. | `B-RESTORE`; `PASS-HOLD` before visibility, then `PASS-POS` after proof, or `PASS-REFUSAL` for deleted/nonexistent scope. `BLOCK-ALL`. |
| `DVG-11` Security negative evidence | `OWN-TRUST` + `OWN-ADM` + `OWN-PROTO` + `OWN-PG` + `OWN-CTX`; proves cross-scope, Host/Origin/session, RLS, credential, stale-writer, injection, hidden retry, disclosure, non-oracle, and direct-authority negatives. | `FX-SCOPE-2U2P`, `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE` · all applicable substitution, expiry, fence, lifecycle, and dispatch fault points · `SCH-SCOPE`, `SCH-FENCE`, `SCH-ABSENT`, `SCH-UNKNOWN`, `SCH-LIFECYCLE` · `ORC-NEGATIVE-CLOSURE`. | `B-SCOPE` + `B-ABSENT` + `B-CONTEXT`; `PASS-REFUSAL` or `PASS-POS` for zero-effect/zero-disclosure expectations. `BLOCK-ALL`. |
| `DVG-12` Standalone Eval boundary | `OWN-EVAL` + `OWN-CTX` + `OWN-RET`; proves Eval View/read/refresh is read-only, explicit assessment follows normal boundaries, evidence gaps/redaction remain visible, and Eval cannot execute a model, egress, write prose, or route authority. | `FX-EVAL-READONLY`, `FX-ABSENT-EXECUTION` · `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP` · `SCH-ABSENT`, `SCH-LIFECYCLE`, `SCH-UNKNOWN` · `ORC-EVAL-READONLY`. | `B-EVAL`; `PASS-POS` for read-only view or `PASS-REFUSAL` for execution. `BLOCK-ALL`; Eval/advisory output cannot upgrade a stage. |
| `DVG-13` Foundation contract walks | `OWN-DVG` + `OWN-REL` + all named domain owners; proves every accepted obligation resolves to an owner, gate, fixture, fault point, schedule, oracle, bundle, disposition, and exact handoff boundary. Its walk result is consumed by stage evidence and handoff rows; it does not create a second release owner. | `FX-CONTRACT-R1`, `FX-HANDOFF` · `CFP-CONTRACT-DRIFT` plus every row-selected fault point · `SCH-DRIFT`, `SCH-NORMAL`, `SCH-ABSENT` · `ORC-CROSSWALK-COMPLETENESS`. | `B-CONTRACT` + `B-HANDOFF`; `PASS-STAGE` only when all required rows are current and passed; `BLOCK-ALL`. |

### 10.1 Acknowledgement-loss Author Command profile

This closed profile is the exact handoff for the acknowledgement-loss part of
`S1-REQ-004` and `S1-EVD-004`. It composes existing public, Web, Core,
PostgreSQL, and trust evidence. It does not claim later reload, restart,
takeover, replay-generation, Snapshot-resync, late-result, or retention proof.

<!-- ACK_LOSS_AUTHOR_COMMAND_PROFILE_START -->
```json
{
  "profile_id": "storyos.dvg.apply-author-edit-ack-loss.v1",
  "selection": {
    "gates": ["DVG-01", "DVG-02", "DVG-03", "DVG-08", "DVG-11"],
    "evidence_classes": [
      "EC-01", "EC-02", "EC-03", "EC-04", "EC-05",
      "EV-CP", "EV-IT", "EV-INT", "EV-SE"
    ],
    "fixtures": ["FX-CONTRACT-R1", "FX-JOURNAL-GROUP", "FX-SCOPE-2U2P"],
    "fault_points": [
      "CFP-EDITOR-BEFORE-GROUP-ADMISSION",
      "CFP-ADMISSION-EXPIRY",
      "CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE",
      "CFP-CORE-BEFORE-COMMIT",
      "CFP-CORE-AFTER-COMMIT-BEFORE-ACK",
      "CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK",
      "CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL",
      "CFP-SCOPE-BEFORE-QUERY"
    ],
    "schedules": ["SCH-NORMAL", "SCH-REORDER", "SCH-SCOPE", "SCH-UNKNOWN"],
    "oracles": [
      "ORC-ATOMIC-AUTHORITY", "ORC-CONTRACT", "ORC-EDITOR-JOURNAL",
      "ORC-NEGATIVE-CLOSURE", "ORC-OUTCOME-UNKNOWN", "ORC-SCOPE"
    ],
    "bundles": ["B-CONTRACT", "B-CORE", "B-EDITOR", "B-SCOPE"]
  },
  "external_dispatch_branch": {
    "unknown_disposition": "PASS-UNKNOWN",
    "reconciliation": "separately_admitted_reconciliation_or_new_attempt",
    "author_command_outcome_read": false
  },
  "author_command_branch": {
    "owners": ["OWN-WEB", "OWN-ADM", "OWN-CORE", "OWN-PROTO", "OWN-PG"],
    "operations": [
      "createProjectCommandChallenge", "applyAuthorEdit",
      "getApplyAuthorEditOutcome"
    ],
    "first_reconciliation_action": "getApplyAuthorEditOutcome",
    "repeat": "protected_outcome_get_only",
    "post_replay": "forbidden",
    "new_challenge": "forbidden",
    "new_admission": "forbidden",
    "separately_admitted_reconciliation": "forbidden",
    "process_termination_or_restart": "forbidden",
    "dispositions": {
      "committed": "PASS-POS",
      "rejected": "PASS-REFUSAL",
      "still_unknown_challenge_issued": "PASS-HOLD",
      "still_unknown_admission_committed": "PASS-HOLD",
      "query_transport_unavailable_or_malformed": "PASS-HOLD",
      "canonical_security_or_input_problem_gate": "PASS-REFUSAL",
      "canonical_security_or_input_problem_journal": "QueryUnavailable"
    },
    "journal": {
      "unresolved": "OutcomeQueryUnresolved",
      "dependent_submission": "blocked",
      "payload_and_capsule": "retained",
      "invented_success_or_rejection": "forbidden"
    },
    "authority": {
      "authoritative_applied": "one_receipt_one_activity_one_authority_effect",
      "no_effect_conflicted_refused": "one_receipt_zero_activity_zero_authority",
      "rejected": "zero_admission_receipt_activity_core_authority",
      "post_and_get": "one_settlement_same_command_admission_receipt"
    },
    "security": {
      "bindings": [
        "idempotency_key_path", "nonce_header", "Host", "ProjectScope",
        "current_client_session", "full_stored_binding", "forced_RLS"
      ],
      "route_policy": "SensitiveSafeReadWithRefererFallback",
      "nonce": "header_only_not_url_log_or_response",
      "cache_control": "no-store_on_success_and_problem",
      "failure": "uniform_non_oracular",
      "read_effects": "zero_nonce_consumption_core_receipt_activity_authority"
    }
  },
  "stage1_handoff": {
    "requirements": [
      "S1-REQ-004:acknowledgement_loss",
      "S1-EVD-004:acknowledgement_loss"
    ],
    "operations": [
      "createProjectCommandChallenge", "applyAuthorEdit",
      "getApplyAuthorEditOutcome"
    ],
    "gates": ["DVG-01", "DVG-02", "DVG-03", "DVG-08", "DVG-11"],
    "evidence_classes": [
      "EC-01", "EC-02", "EC-03", "EC-04", "EC-05",
      "EV-CP", "EV-IT", "EV-INT", "EV-SE"
    ],
    "fixtures": ["FX-CONTRACT-R1", "FX-JOURNAL-GROUP", "FX-SCOPE-2U2P"],
    "fault_points": [
      "CFP-EDITOR-BEFORE-GROUP-ADMISSION",
      "CFP-ADMISSION-EXPIRY",
      "CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE",
      "CFP-CORE-BEFORE-COMMIT",
      "CFP-CORE-AFTER-COMMIT-BEFORE-ACK",
      "CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK",
      "CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL",
      "CFP-SCOPE-BEFORE-QUERY"
    ],
    "schedules": ["SCH-NORMAL", "SCH-REORDER", "SCH-SCOPE", "SCH-UNKNOWN"],
    "oracles": [
      "ORC-ATOMIC-AUTHORITY", "ORC-CONTRACT", "ORC-EDITOR-JOURNAL",
      "ORC-NEGATIVE-CLOSURE", "ORC-OUTCOME-UNKNOWN", "ORC-SCOPE"
    ],
    "bundles": ["B-CONTRACT", "B-CORE", "B-EDITOR", "B-SCOPE"]
  },
  "excluded": {
    "identifiers": [
      "DVG-07", "FX-RECOVERY-EDITOR", "SCH-CRASH",
      "ORC-RECOVERY-ATOMICITY", "B-RECOVERY"
    ],
    "behaviors": [
      "reload", "restart", "takeover", "replay_generation", "snapshot_resync",
      "late_result", "retention", "proposal"
    ]
  }
}
```
<!-- ACK_LOSS_AUTHOR_COMMAND_PROFILE_END -->

The profile is not a Stage 1 artifact. The Stage 1 owner must later copy this
exact selection into its ticket source and generated crosswalk. Until that
binding closes, this owner stays open and #109 stays blocked.

### 10.2 Stable DVG coverage detail

The compact catalogue above is the selection index. The following detail is
retained from the established gate contract so that a stable `DVG-*` reference
does not lose an adversarial case merely because the Release 1 crosswalk is
more explicit. These are semantic cases, not a mandate to choose a framework
or a source-line fault hook.

- **`DVG-01` contract corpus.** Regenerate and compare Rust contract source,
  OpenAPI, JSON Schema, TypeScript, catalogs, examples, the active same-release
  corpus, historical projections, accepted limit profiles, Application Wire
  Records, and SSE frames. Run positive and adversarial closed-input,
  duplicate-name, unknown-control-value, schema/limit-drift, archive
  path/profile, release-mismatch, and cache-refresh fixtures. A generated
  digest, active release/profile, safe result, or exact drift classification is
  required; worktree drift, mixed-release activation, unsafe historical
  projection, and fixture mismatch fail the gate.
- **`DVG-02` request and scope corpus.** Generate exact Protected Web Client
  build/asset identity, Host, Origin, Client Session Binding/generation,
  accepted client-contract/security-policy revision, nonce/idempotency,
  prospective Project creation, action class, command digest, target,
  expected Head, Editor Session, and writer-generation substitutions. Also
  substitute wrong owner/Project/object/cursor/Snapshot/Capability/Approval/
  Credential references, stale/wrong replay cursors, and projection lag. Run
  missing/partial/stale/cross-Scope transaction-local settings against
  non-owner runtime/RLS paths. The oracle checks exact joins, non-oracular
  errors, canonical Snapshot, exclusive resume, bounded duplicate handling,
  and no cache/projection shortcut.
- **`DVG-03` editor and Proposal corpus.** Cover complete IME and editor-intent
  capture, journal-before-submission durability, immediate pending projection,
  bounded grouping, writer-generation fencing, ack/Event convergence, resync,
  and recovery without loss or duplication. For every admission action class,
  vary exact duplicates, changed-digest reuse, refusal, expiry, binding change,
  crash, `OutcomeUnknown`, read-only reconciliation, acknowledgement loss, and
  reconfirmation across the full binding set. Core cases include authoritative,
  Proposal, mixed, stale, concurrent, undo, conflict, Validation,
  selected-operation, dry-run, Acceptance, and NoEffect outcomes. The bundle
  records complete input coverage, journal/Admission/Fence/Receipt/Action/
  Draft/Conflict/Artifact/Revision classes and closed producer causes. Only an
  unexpired fully matching direct edit may recover automatically; changed,
  explicit, expired, or unrecoverable intent requires reconfirmation.
  Inject a Project Activity into each `Refused`, `Conflicted`, `NoEffect`, and
  invalid-later-unit zero-authority settlement. The oracle must reject every
  injection while it accepts the one typed Receipt and unchanged authority.
- **`DVG-04` context corpus.** For every source role, exercise all seven
  Context Assembly gates, eligibility-before-ranking, source revision,
  suppression/redaction, bounded excerpt/summary, dynamic retrieval,
  cache/provider-continuity non-bypass, and re-entry of Tool/MCP/Provider/
  research output. Hostile content and cross-Scope retrieval/index/cache rows
  remain in the fixture set. The oracle requires exact source revisions,
  candidate/selection/refusal reasons, bounded manifests, safe egress, and
  zero use of ineligible content.
- **`DVG-05` destination mediation corpus.** Drive Contract-Faithful Fakes
  through registration, use binding, identity, compatibility, admission,
  adapter wire mapping, Credential generation, cancellation, repair/retry/
  fallback, Tool effect ceilings, MCP discovery drift, MCP App bridge
  spoof/replay/sequence/termination, SSRF/redirect/DNS/private-address cases,
  and hostile external output. Controlled and external destinations are
  distinct fixtures. The oracle requires pinned non-secret identity,
  manifest, Attempt/fence/admission, wire digest, quarantine/result facts, and
  disclosure; direct Core shortcuts, hidden SDK retries, missing assembly,
  and drift reuse fail.
- **`DVG-06` Run/Mailbox corpus.** Generate parent/child Run and Subrun traces
  with duplicate, reordered, lost, and restarted messages; direct-child
  delivery; waits/holds; finalization intent; terminal result and Seal; late
  message; parent recovery; cancellation; stale lease/fence; and resource/
  budget accounting. The oracle checks directional high-watermarks,
  deduplication, terminal sealing, one final result, and no reopened success.
- **`DVG-07` atomic recovery corpus.** Take every applicable Contract Fault
  Point for Core transitions, outbox claims, dispatch claims, lease recovery,
  and stale-result delivery. Compare before/after durable facts to the
  independent oracle and replay the same schedule after restart. A partial
  transition, late settlement, duplicate authoritative effect, or
  unreplayable crash case fails.
- **`DVG-08` unknown/reconciliation corpus.** For model, Tool/MCP, research,
  embedding, export, and other effectful destinations, cut after a durable
  claim on crash/timeout/disconnect. Never infer no-send, success, failure, or
  zero usage. Late confirmation enters only through ordinary immutable
  ingress; a successor has a new Attempt/disclosure/budget and respects
  cancellation/recovery fences. A fake receipt cannot settle the original
  unknown. This external-dispatch branch does not use the Author Command read.
  For missing-first-acknowledgement `ApplyAuthorEdit`, cut at every selected
  Admission, Core, acknowledgement, and Journal boundary. Query the original
  protected identity with `getApplyAuthorEditOutcome` before any POST replay.
  Accept only the closed `Committed | Rejected | StillUnknown` result. Compare
  the complete Receipt, Activity, Journal, queue, capsule, and authority
  snapshot. Only the safe GET may repeat while the Journal is unresolved.
- **`DVG-09` replay/retention corpus.** Exercise cursor resume, retained
  duplicates, generation handoff, cursor-too-old/resync, compaction Evidence
  Floor, post-Seal Mailbox dedupe, historical wire preservation,
  redaction/suppression/Tombstone immediate logical effect, and
  cache/projection/Provider-continuity invalidation. The oracle keeps
  Snapshot, generation, floor, availability gap, lifecycle, and historical
  descriptor facts distinct; guessed mappings, silent gaps, or revived bytes
  fail.
- **`DVG-10` archive/restore corpus.** Use hostile archive path
  encoding/collision/traversal/device/link/bomb/digest/profile/signature/
  Scope/reference cases, valid export/restore, corrupt or missing WAL and
  lifecycle ranges, runtime-role/RLS restore checks, and deletion settlement
  with pending/unknown work. Verify no partial visibility, merge/remap,
  revival, or post-deletion operation. Recovery Visibility Proof and continued
  writing remain separate oracle checkpoints.
- **`DVG-11` trust negative corpus.** Exercise hostile Origin, XSS/DOM sink,
  dependency/asset substitution, third-party script, extension interference,
  stale build/service-worker cache, stale tab/takeover, bridge, Tool/MCP,
  prompt, Provider, SSRF, archive, role/RLS, credential, log/support/
  telemetry, controlled-cloud classification, stale worker, replay, tamper,
  restore, and resource-bound cases. Inspect public output and safe artifacts,
  not only source or HTTP status. Negative Evidence Closure requires no
  unauthorized authority, context, egress, Attempt, budget effect, secret, or
  foreign identity.
- **`DVG-12` Eval corpus.** Opening or refreshing Eval is a scoped redacted
  read with no model/judge/egress/Run side effect. Case/Corpus selection is
  explicit; an advisory assessment is a new ordinary Attempt; external judge
  output is advisory and re-enters normal boundaries; baseline/feedback never
  controls writing or routing. Any ambient monitoring, page-load dispatch,
  authority, or hidden score effect fails.
- **`DVG-13` walk corpus.** Execute the small synthetic cross-boundary walks
  in section 11, including adversarial and recovery variants, with the same
  oracle, fake destinations, scheduler, and evidence-bundle rules as every
  other gate. The walk proves conformance selection and handoff completeness;
  it is not a product-stage or real-Provider substitute.

### 10.3 Cross-cutting property oracle

The following established fault families remain named semantic groupings. The
`CFP-*` rows in section 7 are the concrete selection points for them.

| Fault family | CFP selection and recovered proof |
| --- | --- |
| `core.transition` | `CFP-CORE-BEFORE-COMMIT` and `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`; before commit has no partial domain effect, while after commit has the complete Revisions/Heads/Receipts/Events/sequence/outbox settlement and exact idempotent replay. |
| `editor.input` | Journal durability, group, Admission, expiry, pre-Core, and post-settlement CFPs; every completed intent has non-overlapping coverage until settlement or Draft recovery, fences cannot be crossed, and acknowledgement loss/resync/regrouping cannot lose or duplicate input. |
| `outbox.delivery` | `CFP-OUTBOX-BEFORE-CLAIM`, `CFP-OUTBOX-AFTER-CLAIM-BEFORE-ACK`; delivery intent is not business settlement, and the consumer validates current fence and deduplicates. |
| `external.dispatch` | Manifest/dispatch/reconciliation CFPs; before claim proves no external IO/Disclosure Event, after claim remains `OutcomeUnknown` until ordinary immutable settlement. |
| `lease.fence` | Expiry, takeover, and late-result CFPs; old, duplicate, cancelled, or superseded work cannot settle, publish, consume authority, or create new effects. |
| `mailbox.seal` | Before/after Seal and late-duplicate CFPs; Seal preserves high-watermark/deduplication proof and late input cannot reopen terminal work. |
| `replay.generation` | Compaction, new-generation Snapshot, and below-floor CFPs; valid retained cursors resume, otherwise typed cursor-too-old and safe resync replace guessed mappings. |
| `lifecycle.invalidation` | Before/after invalidation CFPs; current eligibility changes immediately and cache, projection, export, restore, and Provider continuity cannot revive unavailable content or authority. |
| `restore.visibility` | Staging, before-visibility, and after-visibility CFPs; bad staging changes no live state, incomplete lifecycle proof holds ordinary reads, and visible restore has exact Scope with no merge/remap/revival. |
| `project.deletion` | Delete-before/after-settlement CFPs; new work and disclosure are fenced, known/unknown work is settled truthfully, and archive/recovery cannot bypass deletion. |

Where applicable, the independent oracle also enforces these cross-cutting
properties:

1. **Scope noninterference:** changing a foreign Scope's data, projection,
   cache, registration, use, or lifecycle cannot change an in-Scope operation
   except through a safe, attributable shared-resource refusal.
2. **Exact idempotency:** an identical scoped command/key/digest replays to the
   same logical acknowledgement and Receipt; changed route, kind, Scope, or
   digest cannot reuse it.
3. **Atomic visible settlement:** a Core transition is absent or has its
   complete write set; an applied Author Edit has its exact Receipt, Author
   Action, Revision/Head, and Project Activity relation. Refusal, conflict, and
   no-effect may have their required no-change Receipt but never Project
   Activity or a partial authority effect.
4. **Monotonic fencing:** no stale, duplicate, expired, cancelled, or
   superseded lease/fence holder may settle, publish, or consume new authority.
5. **Attempt separation:** retry, resend, redispatch, repair, fallback, or
   changed destination uses a new Attempt; a prior unknown remains attributable
   and keeps its conservative reservation.
6. **Manifest-before-egress:** no external bytes leave before current
   admission, context, destination, disclosure, wire, and dispatch evidence;
   returned external values re-enter Context Assembly before another
   destination sees them.
7. **Provider opacity:** fake scripts, transport absence, cache hints, and
   opaque Provider state never become a claim of internal use, receipt,
   retention, semantic quality, or zero usage.
8. **Replay truthfulness:** Query, replay, and Snapshot stay bound to current
   generation, authorization, redaction, and availability; they never guess a
   cursor mapping or hide a lifecycle gap.
9. **Non-revival:** redacted, tombstoned, compacted-unavailable, deleted, or
   unavailable payloads cannot become readable, eligible, exportable, or
   authoritative through rebuild or recovery.
10. **No hidden authority:** Tool, MCP, App, Provider, Eval, cache, model
    output, generated fixture, or fake destination cannot create an
    authoritative change outside a direct Author Action or author-accepted
    Proposal.
11. **Positive domain classification:** Admission issuance, sanitized refusal,
    `OutcomeUnknown`, reconciliation, terminal settlement, Editor Input Fence,
    Author Action, and typed Receipt remain Operational Records; Refused Edit
    and Recovery Draft remain Draft Artifacts; Proposal Conflict remains a
    validation-axis condition; and only the named Core transition appends an
    Authoritative Revision. Every Artifact Creator and Receipt producer cause
    is a closed variant, with no browser/model/MCP/extension/local-projection/
    journal/generic `System` fallback.
12. **Protected-client claim ceiling:** a positive author-command case proves
    only the exact admitted Protected Web Client build, Client Session Binding
    generation, client-contract/security-policy identities, server-derived
    User/Project Scope, Editor Session/writer generation, action class, digest,
    nonce, idempotency, and lifetime matched. Browser events, synthetic
    fixtures, journals, caches, projections, and test harnesses cannot upgrade
    that result into proof of physical human gesture, trusted display, presence,
    or user verification.

Trace generation stays within the accepted Protocol Limit Profile and
fixture-safe bounds and must shrink counterexamples while preserving contract
identity, schedule, and evidence. No future latency, throughput, token-cost,
corpus-quality, or model-quality number is declared verified here; accepted
protocol ceilings, token counting, archive validation limits, and the
Foundation Recovery Service Profile retain their owner-defined meanings.

### 10.4 Independent oracle registry

The oracle labels used above and in the crosswalk are expectations, not
implementation APIs:

| Oracle | Independent expectation |
| --- | --- |
| `ORC-CONTRACT` | Source/generated/catalog/golden-wire/release identities are equal where required; every mutation is detected and held. |
| `ORC-SCOPE` | Valid exact scope reads/writes only its own facts; foreign substitution is refused without revealing existence or details. |
| `ORC-EDITOR-JOURNAL` | Every supported input/IME/clipboard intent is durable in the correct journal partition, group boundary, and writer generation before submission; replay converges to one owner-defined result. |
| `ORC-ATOMIC-AUTHORITY` | A Core write set is all-or-nothing; typed Receipt/Author Action/Proposal result matches durable state and exact idempotency. An applied Author Edit has one exact Project Activity relation; each zero-authority Author Edit result has one typed Receipt and zero Project Activity. |
| `ORC-CONTEXT-DISCLOSURE` | Only eligible bounded context reaches a committed manifest and matching destination disclosure; no manifest means zero IO. |
| `ORC-DISPATCH-DISCLOSURE` | Attempt, manifest, dispatch claim, fence, wire digest, and disclosure facts are ordered and scope-bound. |
| `ORC-RUN-FINALIZATION` | One general run/mailbox path settles under Seal/Fence; duplicate or late records do not reopen authority. |
| `ORC-RECOVERY-ATOMICITY` | Restart/takeover/replay recovers from durable facts, preserves uncertainty, and produces no stale or duplicate effect. |
| `ORC-OUTCOME-UNKNOWN` | External post-claim unknown remains unknown until separately admitted reconciliation/new Attempt; no blind resend. Author Command acknowledgement loss queries the original durable identity first: `Committed` is `PASS-POS`, public `Rejected` is `PASS-REFUSAL`, and `StillUnknown` and Query failure are `PASS-HOLD`. A scheduled canonical security or input Problem is gate-level `PASS-REFUSAL` but leaves the Journal unresolved. No unresolved branch invents success, rejection, Receipt, Activity, authority, queue release, collection, or POST retry. |
| `ORC-REPLAY-TRUTH` | Generation/floor/Snapshot/gap/availability/deletion facts remain distinct and are never guessed or revived. |
| `ORC-RESTORE-LIFECYCLE` | Isolated restore is held until visibility/lifecycle proof, then continues in exact scope; deleted scope never returns. |
| `ORC-NEGATIVE-CLOSURE` | Every prohibited substitution or absent capability has no unauthorized record/effect/disclosure and a non-oracular result. |
| `ORC-EVAL-READONLY` | Eval reads evidence only; no model/egress/authority/hidden routing occurs. |
| `ORC-CROSSWALK-COMPLETENESS` | Every required stable ID resolves to an existing owner, gate, evidence class, fixture, fault point, schedule, oracle, bundle, and disposition. |

## 11. Stage boundaries and proof walks

### 11.1 Fixed four-stage boundary

The four stages remain exactly those accepted by [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62). The proof order is:

1. **Stage 1 — dependable controlled writing:** manual editor input, journal,
   direct author edit, save/settlement, reload/restart recovery, exact scope,
   and production-shaped provenance.
2. **Stage 2 — complete dependable editor:** project hierarchy, all bounded
   direct editing, recovery/restore/visibility, search/replacement, stats,
   export/archive, and owner-adopted long-session evidence.
3. **Stage 3 — fake-model assistance:** one general Project Agent Loop,
   bounded current request, fake model through the real Host/Context/Attempt
   path, editable Proposal and explicit author decision, with S2 preserved.
4. **Stage 4 — one real external-model route:** one Provider-neutral real
   destination under the same identity, disclosure, Attempt/unknown/recovery,
   Proposal, and S2 fallback boundaries; no Provider-internal or literary claim.

A stage is not released because its next stage is planned, because a gate ran,
or because a model returned content. The author journey and every applicable
mandatory evidence row must be current, attributable to one exact baseline,
replayable, and passed.

### 11.2 Explicit Stage 3/4 exclusions

The following execution is absent from both Stages 3 and 4, including bounded
variants: Tool, MCP, research, embedding, Memory, Skill, Subrun, and Eval
execution. The corresponding proof is a negative/refusal proof through
`DVG-04`, `DVG-05`, `DVG-06`, `DVG-11`, and `DVG-12`; a fake bounded variant
does not authorize the capability. A requested excluded operation must produce
`PASS-REFUSAL` with `B-ABSENT`, not a hidden fallback, second route, local
authority, automatic Proposal, or direct write.

Stage 4 additionally excludes a second Provider, a second authority path, a
local fallback, hidden retry, hidden SDK behavior, automatic authority, an
Agent-authored outline, a cloud stage, and any claim about Provider attention,
retention, training, or literary quality. A real destination supplies
`EV-RDA` only for the StoryOS-owned boundary unless a later owner contract
adopts a different evidence obligation.

### 11.3 Foundation proof walks

The following walks are the minimum compositions used by `DVG-13`. A walk is
not a new runtime; it is a deterministic selection of existing gates.

| Walk | Sequence and purpose | Fixtures/schedules | Gates |
| --- | --- | --- | --- |
| `WALK-01` Contract and identity | Source → generated schema/catalog → golden wire → same-release identity → bound/drift refusal. | `FX-CONTRACT-R1` · `SCH-DRIFT` | `DVG-01`, `DVG-13` |
| `WALK-02` Editor and author authority | Scope/session → input/IME/journal/group → Admission → Core/Proposal → Receipt/Action/undo → ack/Event convergence. | `FX-EDITOR-IME`, `FX-JOURNAL-GROUP`, `FX-CORE-PROPOSAL` · `SCH-NORMAL`, `SCH-REORDER`, `SCH-CRASH` | `DVG-02`, `DVG-03`, `DVG-07`, `DVG-11` |
| `WALK-03` Recovery and restore | Crash/restart/takeover → replay/reconcile → isolated Recovery Copy/PITR → roles/RLS/migration/projection/lifecycle → Recovery Visibility → continued writing/export/non-revival. | `FX-RECOVERY-EDITOR`, `FX-RESTORE-LIFECYCLE` · `SCH-CRASH`, `SCH-FENCE`, `SCH-RESTORE`, `SCH-LIFECYCLE` | `DVG-02`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11` |
| `WALK-04` Proposal and plain-language boundary | Current request/Working Target → advisory vs Prose Change Request → editable Proposal → explicit Acceptance/Rejection/refusal/conflict → no direct authority. | `FX-CORE-PROPOSAL`, `FX-CONTEXT-DISCLOSURE` · `SCH-NORMAL`, `SCH-SCOPE` | `DVG-03`, `DVG-04`, `DVG-05`, `DVG-11` |
| `WALK-05` Retention and replay truth | Seal/Fence/settlement completeness → compaction → replay generation/Snapshot/cursor floor → redaction/archive/deletion/gap/unavailable export. | `FX-REPLAY-RETENTION` · `SCH-REPLAY`, `SCH-LIFECYCLE`, `SCH-REORDER` | `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11` |
| `WALK-06` Fake-model Stage 3 | Bounded current request → real Host/Scope/Context Assembly → manifest → fake Attempt/fence/unknown/reconcile → Proposal/author decision → disable fake and repeat S2. | `FX-FAKE-MODEL`, `FX-RECOVERY-EDITOR` · `SCH-NORMAL`, `SCH-UNKNOWN`, `SCH-FENCE` | `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-11`, `DVG-13` |
| `WALK-07` One-real-model Stage 4 | Registration/binding/capability/credential/policy → same assembly/manifest → one real Attempt → unknown/reconciliation → Proposal/author decision → disable and repeat S2. | `FX-REAL-MODEL-ADVISORY`, `FX-FAKE-MODEL` · `SCH-NORMAL`, `SCH-UNKNOWN`, `SCH-SCOPE`, `SCH-FENCE` | `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11`, `DVG-13` |
| `WALK-08` Absent execution and Eval | Request each excluded capability and Eval execution → refusal/read-only boundary → no hidden route, egress, authority, or model run. | `FX-ABSENT-EXECUTION`, `FX-EVAL-READONLY` · `SCH-ABSENT`, `SCH-SCOPE`, `SCH-LIFECYCLE` | `DVG-04`, `DVG-05`, `DVG-06`, `DVG-11`, `DVG-12` |

### 11.4 Crosswalk token grammar, stage maps, and aggregate bundle sets

Crosswalk cells use exact registered IDs separated by commas or semicolons;
`+` is also an explicit set operator and is never a hidden expansion.
The only shorthand is the finite shared-prefix form `PREFIX-A/B/C`. It
expands mechanically, in order and without inference, to exactly
`PREFIX-A`, `PREFIX-B`, and `PREFIX-C`. Only `EC`, `EV`, and `SCH` prefixes
may use this form; every expanded ID must exist in the corresponding registry.
Slash shorthand is forbidden for `OWN-*`, `DVG-*`, `CFP-*`, `FX-*`, `ORC-*`,
`B-*`, dispositions, and block results. A bundle aggregate is a registered
`B-*` ID, and its exact member set is the one listed in section 8; using an
aggregate never means “all other bundles” or permits an implementation to
choose additional members. The validator expands shared-prefix tokens first,
then resolves every gate, evidence, fixture, fault point, schedule, oracle,
bundle, disposition, and block token against this document.

The finite stage-map selectors below are also registered crosswalk tokens.
`SMAP-STAGE-1` through `SMAP-STAGE-4` are mandatory implementation-evidence
maps: each selects exactly one row of this table and never contains `EV-SR`.
`SMAP-EVALUATED-MANDATORY-STAGE` selects exactly one of those four maps from
the evidence record's required `evaluated_stage` field. A selector expands by
structured cell to the listed gates, evidence classes/layers, fixtures,
Contract Fault Points, schedules, oracles, and mandatory bundle set. In the
combined fixture/fault/schedule/oracle cell, expansion is respectively to
those four named subfields in left-to-right order; an explicit `+` operand
adds to its matching subfield. It never expands to the union of the four
rows, and it is not a fifth stage or a bundle wildcard.

The separate `SMAP-RELEASE-STAGE-1` through `SMAP-RELEASE-STAGE-4` selectors
are ordered release branches. Each consumes exactly one mandatory map and its
named author journey; only after both pass does it emit `EV-SR`, `PASS-STAGE`,
the exact resulting `main`, and the exact next-stage input. The dynamic
`SMAP-EVALUATED-RELEASE-STAGE` selects the one release branch corresponding to
the same `evaluated_stage`; it never adds a future-stage requirement to the
selected mandatory map. `REL-005`, `HND-003`, and `HND-004` are the only
crosswalk rows that may use the dynamic selectors. The Stage 3/4
AI-independent regression rows use the fixed `SMAP-STAGE-2` mandatory map as
their complete Stage 2 regression input; a Stage 2 release result is a
regression output, not an input to that map.

| Selector | Stage (mandatory implementation map) | Gate set | Evidence classes/layers | Fixtures | Contract Fault Points | Schedules | Oracles | Mandatory bundle set |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SMAP-STAGE-1` | Stage 1 (mandatory implementation evidence) | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/07`; `EV-CP/IT/INT/SE` | `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-RECOVERY-EDITOR`, `FX-SCOPE-2U2P` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-SCOPE-BEFORE-QUERY` | `SCH-CRASH/DRIFT/FENCE/NORMAL/REORDER/REPLAY/SCOPE` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-SCOPE` | `B-S1-MANDATORY-SET` |
| `SMAP-STAGE-2` | Stage 2 (mandatory implementation evidence) | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07`; `EV-CP/IT/INT/PRD/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-EDITOR-JOURNAL`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-SCOPE` | `B-S2-MANDATORY-SET` |
| `SMAP-STAGE-3` | Stage 3 (mandatory implementation evidence) | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-12`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/SE` (no real-destination advisory or stage-release output) | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-EVAL-READONLY`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-EVAL-READONLY`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE` | `B-S3-MANDATORY-SET` |
| `SMAP-STAGE-4` | Stage 4 (mandatory implementation evidence) | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-12`, `DVG-13` | `EC-01/02/03/04/05/06/07/08`; `EV-CP/IT/INT/PRD/RDA/SE` | `FX-ABSENT-EXECUTION`, `FX-CONTEXT-DISCLOSURE`, `FX-CONTRACT-R1`, `FX-CORE-PROPOSAL`, `FX-EDITOR-IME`, `FX-EVAL-READONLY`, `FX-FAKE-MODEL`, `FX-HANDOFF`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION`, `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P` | `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CONTRACT-DRIFT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-CORE-BEFORE-COMMIT`, `CFP-DELETE-AFTER-SETTLEMENT`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-DISPATCH-BEFORE-CLAIM`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-LATE-RESULT`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS`, `CFP-MANIFEST-BEFORE-COMMIT`, `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT`, `CFP-PROPOSAL-BEFORE-DECISION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-STAGING`, `CFP-SCOPE-BEFORE-QUERY` | `SCH-ABSENT/CRASH/DRIFT/FENCE/LIFECYCLE/LONG/NORMAL/REORDER/REPLAY/RESTORE/SCOPE/UNKNOWN` | `ORC-ATOMIC-AUTHORITY`, `ORC-CONTEXT-DISCLOSURE`, `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS`, `ORC-DISPATCH-DISCLOSURE`, `ORC-EDITOR-JOURNAL`, `ORC-EVAL-READONLY`, `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-RUN-FINALIZATION`, `ORC-SCOPE` | `B-S4-MANDATORY-SET` |

The release selectors are deliberately separate from these mandatory maps:

| Release selector | Stage | Mandatory evidence input | Author-journey input | Output after both inputs pass |
| --- | --- | --- | --- | --- |
| `SMAP-RELEASE-STAGE-1` | Stage 1 | `SMAP-STAGE-1` | `S1-JRN-001` | Emit `EV-SR`, `PASS-STAGE`, the exact resulting `main`, and the exact Stage 2 input. |
| `SMAP-RELEASE-STAGE-2` | Stage 2 | `SMAP-STAGE-2` | `S2-JRN-001` | Emit `EV-SR`, `PASS-STAGE`, the exact resulting `main`, and the exact Stage 3 input. |
| `SMAP-RELEASE-STAGE-3` | Stage 3 | `SMAP-STAGE-3` | `S3-JRN-001` | Emit `EV-SR`, `PASS-STAGE`, the exact resulting `main`, and the exact Stage 4 input. |
| `SMAP-RELEASE-STAGE-4` | Stage 4 | `SMAP-STAGE-4` | `S4-JRN-001` | Emit `EV-SR`, `PASS-STAGE`, and the exact resulting `main`; no successor is claimed by this row. |

`SMAP-EVALUATED-RELEASE-STAGE` is the finite dynamic selector for exactly one
of those four release rows, keyed by the same `evaluated_stage` that selects
`SMAP-EVALUATED-MANDATORY-STAGE`. It is a release branch, not an evidence
source: `EV-SR` exists only in its output after the selected mandatory map and
author journey have passed. The Stage 2 result produced while running a Stage
3 or Stage 4 regression is an observed regression outcome, never a required
input to `SMAP-STAGE-2` or to the later mandatory map.

The Stage 2 row is the complete AI-independent regression contract consumed
by Stage 3 and Stage 4. The Stage 3 map adds the accepted fake-model,
read-only-Eval, and explicit-absence boundaries to that complete Stage 2
contract. The Stage 4 map consumes that complete Stage 3 deterministic set and
adds one real-destination advisory boundary. Neither later map replaces the
earlier editor journey with a one-edit smoke test, and real-destination
evidence cannot replace fake, AgentRun-finalization, Eval-read-only, or
absence proof.

## 12. Release-to-proof crosswalk

This is the complete Release 1 crosswalk. Each row names the proven fact, the
accepted owner, the gate set, the evidence layer and baseline class, the
fixture/fault/schedule/oracle, the safe bundle and passing disposition, and the
exact blocking rule. Shared gates are intentionally repeated; no stable ID is
absorbed as an implementation detail.

### 12.1 Release invariants

| ID | What is proven · accepted owner | Gate set | Evidence | Fixture · fault points · schedule · oracle | Safe bundle · pass | Block |
| --- | --- | --- | --- | --- | --- | --- |
| `REL-001` | Editor is daily usable with AI disabled; `OWN-REL` with `OWN-WEB`, `OWN-CORE`, `OWN-PG`, `OWN-RET`. | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-07`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06`; `EV-CP/IT/INT/PRD/SE` | `FX-EDITOR-IME`, `FX-RECOVERY-EDITOR`, `FX-RESTORE-LIFECYCLE`, `FX-LONG-SESSION` · `CFP-CORE-BEFORE-COMMIT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-BEFORE-VISIBILITY` · `SCH-NORMAL/REORDER/CRASH/REPLAY/RESTORE/LONG` · `ORC-ATOMIC-AUTHORITY`, `ORC-RECOVERY-ATOMICITY`, `ORC-RESTORE-LIFECYCLE` · `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY` | `B-EDITOR` + `B-RECOVERY` + `B-RESTORE` + `B-MEASURE`; `PASS-STAGE` after author journey and adopted measurement evidence. | `BLOCK-ALL`; any missing journey, recovery, restore, isolation, or adopted measurement evidence blocks. |
| `REL-002` | Only direct author action or author-Accepted Proposal may change authoritative state; `OWN-CORE`, `OWN-ADM`, `OWN-PLAIN`, `OWN-TRUST`. | `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11` | `EC-03/05/07/08`; `EV-CP/IT/INT/SE` | `FX-CORE-PROPOSAL`, `FX-CONTEXT-DISCLOSURE`, `FX-ABSENT-EXECUTION` · admission/core/proposal/manifest/dispatch/late points · `SCH-NORMAL/SCOPE/UNKNOWN/ABSENT` · `ORC-ATOMIC-AUTHORITY`, `ORC-NEGATIVE-CLOSURE` · `CFP-ADMISSION-BEFORE-CORE` | `B-CORE` + `B-CONTEXT` + `B-ABSENT`; `PASS-POS` for allowed transition or `PASS-REFUSAL` for every non-author path. | `BLOCK-ALL`; any direct Agent/Tool/MCP/model/browser/projection effect or fabricated Acceptance blocks. |
| `REL-003` | Exact User/Project Scope is the authority boundary across Core, Postgres, routes, recovery, and disclosure; `OWN-PROTO`, `OWN-PG`, `OWN-ADM`, `OWN-TRUST`. | `DVG-01`, `DVG-02`, `DVG-07`, `DVG-10`, `DVG-11` | `EC-01/04/05/08`; `EV-CP/IT/INT/PRD/SE` | `FX-SCOPE-2U2P`, `FX-RESTORE-LIFECYCLE` · scope/query, restore, delete, RLS, fence points · `SCH-SCOPE/RESTORE/LIFECYCLE/FENCE` · `ORC-SCOPE`, `ORC-RESTORE-LIFECYCLE` · `CFP-SCOPE-BEFORE-QUERY` | `B-SCOPE` + `B-RESTORE`; `PASS-POS` for own scope and `PASS-REFUSAL`/`PASS-HOLD` for foreign or unsafe scope. | `BLOCK-ALL`; any leak, non-oracular mismatch, forced-RLS bypass, or recovery scope uncertainty blocks. |
| `REL-004` | Exactly four stages and their accepted order remain unchanged; `OWN-REL`, `OWN-STAGE`. | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-12`, `DVG-13` | `EC-01`; `EV-CP/SE` | `FX-HANDOFF`, `FX-CONTRACT-R1` · contract drift and stage-boundary points · `SCH-DRIFT/NORMAL/ABSENT` · `ORC-CROSSWALK-COMPLETENESS` · `CFP-CONTRACT-DRIFT` | `B-HANDOFF` + `B-CONTRACT`; `PASS-STAGE` when the four-stage sequence and evidence dependency graph match [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62). | `BLOCK-ALL`; any new stage, reordered stage, invented capability, or unresolved owner/gate row blocks. |
| `REL-005` | The currently evaluated stage is eligible only when its own author journey and mandatory evidence are current, attributable, replayable, and passed; the evidence record supplies exactly one `evaluated_stage`; `OWN-REL`, `OWN-DVG`. First resolve `SMAP-EVALUATED-MANDATORY-STAGE` for that one stage, then use the corresponding `SMAP-EVALUATED-RELEASE-STAGE` branch with that stage's named author journey. A later stage's unrun evidence is not a current-stage failure, and `EV-SR` is produced only after both selected inputs pass. | `SMAP-EVALUATED-MANDATORY-STAGE` | `SMAP-EVALUATED-MANDATORY-STAGE` | `SMAP-EVALUATED-MANDATORY-STAGE` | `SMAP-EVALUATED-RELEASE-STAGE`; `PASS-STAGE` and `EV-SR` only after the selected mandatory map and author journey are passed or have the row-specific expected negative/hold disposition. | `BLOCK-ALL`; a selected-stage mandatory item or journey step that is failed, unrun, stale, unavailable, unverified, or unreplayable blocks, while evidence belonging only to a later stage does not. |
| `REL-006` | Planning, implementation, stage evidence, stage release, and later controlled-cloud deployment remain separate; `OWN-REL`, `OWN-GOV`. | `DVG-01`, `DVG-10`, `DVG-13` | `EC-01/04/05`; `EV-CP/PRD/SE/CCD` | `FX-HANDOFF`, `FX-RESTORE-LIFECYCLE` · contract drift, restore, deletion, deployment-identity points · `SCH-DRIFT/RESTORE/LIFECYCLE` · `ORC-CONTRACT`, `ORC-RESTORE-LIFECYCLE`, `ORC-CROSSWALK-COMPLETENESS` · `CFP-CONTRACT-DRIFT` | `B-HANDOFF` + `B-RESTORE`; `PASS-STAGE` for local separation or `PASS-CLOUD` only for later controlled-cloud evidence. | `BLOCK-ALL`; planning cannot be reported as implementation/release, and cloud evidence cannot backfill a local gap. |

### 12.2 Stage 1 requirements, journey, and evidence

| ID | What is proven · accepted owner | Gate set | Evidence | Fixture · fault points · schedule · oracle | Safe bundle · pass | Block |
| --- | --- | --- | --- | --- | --- | --- |
| `S1-REQ-001` | Controlled Project/current chapter travels through protected Web Client/Server/Core/Postgres; `OWN-WEB`, `OWN-PROTO`, `OWN-CORE`, `OWN-PG`, `OWN-TRUST`. | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-07`, `DVG-11` | `EC-01/02/03/05`; `EV-CP/IT/INT/SE` | `FX-SCOPE-2U2P`, `FX-EDITOR-IME` · `CFP-SCOPE-BEFORE-QUERY`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-CORE-BEFORE-COMMIT` · `SCH-NORMAL/SCOPE` · `ORC-SCOPE`, `ORC-ATOMIC-AUTHORITY` | `B-SCOPE` + `B-EDITOR` + `B-CORE`; `PASS-POS`. | `BLOCK-ALL`; missing production-shaped route, exact scope, or atomic settlement blocks. |
| `S1-REQ-002` | Manual typing/paste/cut/delete/selection replacement, Chinese/English IME enter the real Editor Session/Scope Local Journal; `OWN-WEB`. | `DVG-02`, `DVG-03`, `DVG-07`, `DVG-11` | `EC-02/03/05`; `EV-CP/IT/INT/SE` | `FX-EDITOR-IME`, `FX-JOURNAL-GROUP` · `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION` · `SCH-NORMAL/CRASH/REORDER` · `ORC-EDITOR-JOURNAL` | `B-EDITOR`; `PASS-POS`. | `BLOCK-ALL`; any lost composition, wrong scope, implicit coalescing, or non-replayable journal evidence blocks. |
| `S1-REQ-003` | Direct edit goes through Admission/Core. `AuthoritativeApplied` produces the authoritative Revision, typed Receipt, Author Action, Project Activity, and save state. `Refused`, `Conflicted`, and `NoEffect` produce only their typed zero-authority Receipt and no Project Activity or save-state advance; `OWN-ADM`, `OWN-CORE`, `OWN-PROTO`, `OWN-PG`. | `DVG-02`, `DVG-03`, `DVG-07` | `EC-03/07`; `EV-CP/IT/INT/SE` | `FX-CORE-PROPOSAL`, `FX-JOURNAL-GROUP` · `CFP-ADMISSION-BEFORE-CORE`, `CFP-CORE-BEFORE-COMMIT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK` · `SCH-NORMAL/CRASH/REORDER` · `ORC-ATOMIC-AUTHORITY` | `B-CORE` + `B-EDITOR`; `PASS-POS` for applied and admitted zero-authority settlements, `PASS-REFUSAL` only before Admission, or `PASS-HOLD` only for recovery. | `BLOCK-ALL`; partial write, wrong Receipt producer, fake Activity on a zero-authority Receipt, duplicate action, or guessed acknowledgement blocks. |
| `S1-REQ-004` | Reload/process/restart recovers saved and unsettled work, including Recovery Draft/reconfirmation; `OWN-WEB`, `OWN-ADM`, `OWN-CORE`, `OWN-RET`. | `DVG-03`, `DVG-07`, `DVG-08`, `DVG-09` | `EC-03/04/07`; `EV-CP/IT/INT/SE` | `FX-RECOVERY-EDITOR` · `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-FENCE-AFTER-TAKEOVER` · `SCH-CRASH/FENCE/REORDER` · `ORC-RECOVERY-ATOMICITY`, `ORC-OUTCOME-UNKNOWN` | `B-RECOVERY`; `PASS-POS` for settled recovery or `PASS-HOLD` for Recovery Draft/reconfirmation. | `BLOCK-ALL`; any blind invoke, invented commit, lost journal, or stale settlement blocks. |
| `S1-REQ-005` | Exact scope, Host/Origin/session, bounds, forced RLS, and no leaks; `OWN-PROTO`, `OWN-PG`, `OWN-ADM`, `OWN-TRUST`. | `DVG-02`, `DVG-07`, `DVG-11` | `EC-05`; `EV-CP/IT/INT/SE` | `FX-SCOPE-2U2P` · `CFP-SCOPE-BEFORE-QUERY`, `CFP-ADMISSION-EXPIRY`, `CFP-LATE-RESULT` · `SCH-SCOPE/FENCE` · `ORC-SCOPE`, `ORC-NEGATIVE-CLOSURE` | `B-SCOPE`; `PASS-POS` for valid bindings and `PASS-REFUSAL` for every substitution. | `BLOCK-ALL`; any cross-scope fact, oracle leak, RLS/role mismatch, or bound exhaustion without refusal blocks. |
| `S1-REQ-006` | Production-shaped provenance is present; no disposable substitute, in-memory/local authority, test Adapter, or `.reference/**` dependency; `OWN-GOV` plus each exercised owner. | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-07`, `DVG-11`, `DVG-13` | `EC-01/02/03/05`; `EV-CP/IT/INT/SE` | `FX-CONTRACT-R1`, `FX-HANDOFF` · `CFP-CONTRACT-DRIFT`, `CFP-SCOPE-BEFORE-QUERY`, `CFP-CORE-BEFORE-COMMIT` · `SCH-DRIFT/NORMAL` · `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS` | `B-CONTRACT` + `B-HANDOFF`; `PASS-STAGE` only with attributable production-shaped evidence. | `BLOCK-ALL`; reference/dependency contamination or provenance gap blocks. |
| `S1-JRN-001` | The complete six-step controlled writing journey—open, CN/EN/IME input and clipboard, save observation, interrupt/restart settled/unsettled, journal convergence/recovery, one applied author effect/Receipt/Activity or one zero-authority Receipt with no Activity, no duplicate, and exact scope—is executable; `OWN-REL` with `OWN-WEB`, `OWN-CORE`, `OWN-ADM`, `OWN-PG`. | `DVG-02`, `DVG-03`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-11`, `DVG-13` | `EC-02/03/04/05/07`; `EV-CP/INT/SE` | `FX-EDITOR-IME`, `FX-JOURNAL-GROUP`, `FX-RECOVERY-EDITOR` · `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-LATE-RESULT` · `SCH-NORMAL/REORDER/CRASH/FENCE/REPLAY` · `ORC-EDITOR-JOURNAL`, `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH`, `ORC-SCOPE` · `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY` | `B-EDITOR` + `B-RECOVERY` + `B-REPLAY` + `B-HANDOFF`; `PASS-STAGE`. | `BLOCK-ALL`; any missing journey step, fake zero-authority Activity, or unresolved durable fact blocks Stage 1. |
| `S1-EVD-001` | Crosswalk names owner, revision, scope, source, and baseline for Stage 1; `OWN-REL`, `OWN-DVG`. | `DVG-01`, `DVG-13` | `EC-01`; `EV-CP/SE` | `FX-CONTRACT-R1`, `FX-HANDOFF` · contract drift points · `SCH-DRIFT` · `ORC-CROSSWALK-COMPLETENESS` · `CFP-CONTRACT-DRIFT` | `B-CONTRACT` + `B-HANDOFF`; `PASS-POS`. | `BLOCK-ALL`; missing owner/revision/scope/source or stale baseline is blocking. |
| `S1-EVD-002` | Browser input/IME/journal/projection/save evidence is complete and attributable; `OWN-WEB`. | `DVG-02`, `DVG-03`, `DVG-07` | `EC-02/03`; `EV-CP/IT/INT/SE` | `FX-EDITOR-IME`, `FX-JOURNAL-GROUP` · editor journal/group/settlement points · `SCH-NORMAL/REORDER/CRASH` · `ORC-EDITOR-JOURNAL` · `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY` | `B-EDITOR`; `PASS-POS`. | `BLOCK-ALL`; lost/reordered/unreplayable input or projection evidence blocks. |
| `S1-EVD-003` | Core/Admission/persistence evidence proves atomicity, the applied Receipt/Revision/Activity relation, the zero-authority Receipt/no-Activity relation, and exact idempotency; `OWN-ADM`, `OWN-CORE`, `OWN-PG`. | `DVG-02`, `DVG-03`, `DVG-07` | `EC-03`; `EV-CP/IT/INT/SE` | `FX-CORE-PROPOSAL` · admission/core/outbox/settlement points · `SCH-NORMAL/CRASH/REORDER` · `ORC-ATOMIC-AUTHORITY` · `CFP-CORE-BEFORE-COMMIT` | `B-CORE`; `PASS-POS` for applied and admitted zero-authority settlements, `PASS-REFUSAL` only before Admission, or `PASS-HOLD` only for recovery. | `BLOCK-ALL`; missing durable fact, fake Activity, duplicate effect, or wrong typed result blocks. |
| `S1-EVD-004` | Reload/restart/recovery-draft/reconfirmation/fence/resync evidence is complete; `OWN-WEB`, `OWN-ADM`, `OWN-CORE`, `OWN-RET`. | `DVG-03`, `DVG-07`, `DVG-08`, `DVG-09` | `EC-04/07`; `EV-CP/INT/SE` | `FX-RECOVERY-EDITOR` · crash/expiry/fence/replay points · `SCH-CRASH/FENCE/REPLAY` · `ORC-RECOVERY-ATOMICITY`, `ORC-OUTCOME-UNKNOWN` · `CFP-CORE-AFTER-COMMIT-BEFORE-ACK` | `B-RECOVERY`; `PASS-POS` or `PASS-HOLD` by expected recovery state. | `BLOCK-ALL`; any guessed commit, non-replayable recovery, or absent hold blocks. |
| `S1-EVD-005` | Isolation/trust evidence proves scope, Host/Origin/session, forced RLS, and bounded client claims; `OWN-TRUST`, `OWN-PROTO`, `OWN-PG`. | `DVG-02`, `DVG-07`, `DVG-11` | `EC-05`; `EV-CP/IT/INT/SE` | `FX-SCOPE-2U2P` · scope/RLS/fence points · `SCH-SCOPE/FENCE` · `ORC-SCOPE`, `ORC-NEGATIVE-CLOSURE` · `CFP-SCOPE-BEFORE-QUERY` | `B-SCOPE`; `PASS-POS` and `PASS-REFUSAL` as scheduled. | `BLOCK-ALL`; any leak, role bypass, or stale writer effect blocks. |
| `S1-EVD-006` | Provenance proves no prototype, in-memory, test Adapter, local authority, or `.reference/**` input; `OWN-GOV`. | `DVG-01`, `DVG-11`, `DVG-13` | `EC-01/05`; `EV-CP/SE` | `FX-CONTRACT-R1`, `FX-HANDOFF` · contract drift/reference boundary · `SCH-DRIFT` · `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS` · `CFP-CONTRACT-DRIFT` | `B-CONTRACT` + `B-HANDOFF`; `PASS-POS`. | `BLOCK-ALL`; any unaccounted provenance or reference dependency blocks. |

### 12.3 Stage 2 requirements, journey, and evidence

| ID | What is proven · accepted owner | Gate set | Evidence | Fixture · fault points · schedule · oracle | Safe bundle · pass | Block |
| --- | --- | --- | --- | --- | --- | --- |
| `S2-REQ-001` | New/controlled initialization and writing work with all AI services disabled; `OWN-REL`, `OWN-WEB`, `OWN-CORE`, `OWN-PG`. | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-07`, `DVG-11`, `DVG-13` | `EC-01/02/03/05`; `EV-CP/IT/INT/SE` | `FX-SCOPE-2U2P`, `FX-EDITOR-IME`, `FX-ABSENT-EXECUTION` · scope/editor/core/absent points · `SCH-NORMAL/ABSENT/SCOPE` · `ORC-SCOPE`, `ORC-ATOMIC-AUTHORITY`, `ORC-NEGATIVE-CLOSURE` · `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY` | `B-EDITOR` + `B-ABSENT` + `B-CORE`; `PASS-STAGE`. | `BLOCK-ALL`; AI dependency, hidden fallback, or missing manual path blocks. |
| `S2-REQ-002` | Project create/open/rename/archive stays in exact scope; `OWN-PROTO`, `OWN-CORE`, `OWN-PG`, `OWN-RET`. | `DVG-02`, `DVG-03`, `DVG-07`, `DVG-09`, `DVG-10`, `DVG-11` | `EC-03/04/05`; `EV-CP/IT/INT/PRD/SE` | `FX-SCOPE-2U2P`, `FX-RESTORE-LIFECYCLE` · scope/core/lifecycle/archive/delete points · `SCH-NORMAL/SCOPE/LIFECYCLE/RESTORE` · `ORC-SCOPE`, `ORC-RESTORE-LIFECYCLE` · `CFP-SCOPE-BEFORE-QUERY` | `B-SCOPE` + `B-RESTORE`; `PASS-POS` or owner-defined `PASS-REFUSAL`. | `BLOCK-ALL`; wrong owner, scope, archive state, or restore lifecycle blocks. |
| `S2-REQ-003` | Volumes/chapters can be managed, navigated, reopened, and authorized Snapshot views without loss; `OWN-WEB`, `OWN-PROTO`, `OWN-CORE`, `OWN-RET`. | `DVG-02`, `DVG-03`, `DVG-09`, `DVG-11` | `EC-02/03/04/05`; `EV-CP/IT/INT/SE` | `FX-EDITOR-IME`, `FX-REPLAY-RETENTION` · scope, replay generation/Snapshot, late-event points · `SCH-NORMAL/REPLAY/REORDER` · `ORC-SCOPE`, `ORC-REPLAY-TRUTH` · `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT` | `B-EDITOR` + `B-REPLAY`; `PASS-POS`. | `BLOCK-ALL`; guessed cursor mapping, loss, or cross-scope navigation blocks. |
| `S2-REQ-004` | Direct block operations—typing/paste/cut/selection/split/join/move/retype/keyboard/clipboard/undo/CN/EN IME—remain author-gated and continuous; `OWN-WEB`, `OWN-ADM`, `OWN-CORE`. | `DVG-02`, `DVG-03`, `DVG-07`, `DVG-11` | `EC-02/03/07`; `EV-CP/IT/INT/SE` | `FX-EDITOR-IME`, `FX-JOURNAL-GROUP`, `FX-CORE-PROPOSAL` · all editor/admission/core/undo points · `SCH-NORMAL/CRASH/REORDER/FENCE` · `ORC-EDITOR-JOURNAL`, `ORC-ATOMIC-AUTHORITY` · `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY` | `B-EDITOR` + `B-CORE`; `PASS-POS`. | `BLOCK-ALL`; an unsupported operation, lost composition, direct mutation, or non-atomic undo blocks. |
| `S2-REQ-005` | Save/recovery survives delay, ack/Event reordering, reload, client crash, Server restart, PostgreSQL restart, Activity replay-floor resync, takeover, Recovery Draft/reconfirmation, isolated restore/RLS/projection/lifecycle gaps, Recovery Visibility, and continued writing; `OWN-WEB`, `OWN-PG`, `OWN-RET`, `OWN-ADM`, `OWN-CORE`. | `DVG-02`, `DVG-03`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11` | `EC-03/04/05`; `EV-CP/IT/INT/PRD/SE` | `FX-RECOVERY-EDITOR`, `FX-RESTORE-LIFECYCLE`, `FX-REPLAY-RETENTION` · crash/fence/replay/restore/lifecycle points · `SCH-CRASH/REORDER/FENCE/REPLAY/RESTORE/LIFECYCLE` · `ORC-RECOVERY-ATOMICITY`, `ORC-OUTCOME-UNKNOWN`, `ORC-RESTORE-LIFECYCLE`, `ORC-REPLAY-TRUTH` · `CFP-RESTORE-BEFORE-VISIBILITY` | `B-RECOVERY` + `B-REPLAY` + `B-RESTORE`; `PASS-POS` or `PASS-HOLD` at the exact recovery cut. | `BLOCK-ALL`; any missing physical restore, visibility proof, continued-writing proof, stale writer, guessed replay, or lost settlement blocks. |
| `S2-REQ-006` | Bounded current/full search plus one visible direct replacement works; multi/cross-location changes remain Proposal-gated; `OWN-PROTO`, `OWN-CTX`, `OWN-CORE`, `OWN-PLAIN`. | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-09`, `DVG-11` | `EC-02/05/07`; `EV-CP/IT/INT/SE` | `FX-CONTEXT-DISCLOSURE`, `FX-CORE-PROPOSAL` · scope/selection/projection/proposal/lifecycle points · `SCH-SCOPE/LIFECYCLE/NORMAL` · `ORC-CONTEXT-DISCLOSURE`, `ORC-ATOMIC-AUTHORITY` · `CFP-MANIFEST-BEFORE-COMMIT` | `B-CONTEXT` + `B-CORE`; `PASS-POS` for bounded read/direct replacement and `PASS-REFUSAL`/Proposal result for broader change. | `BLOCK-ALL`; unbounded search, hidden bulk write, wrong scope, or direct cross-location effect blocks. |
| `S2-REQ-007` | Stats and deterministic human-readable export preserve scope, unavailable-content identity/digest/gap, Project Export Archive order, and lifecycle; `OWN-PROTO`, `OWN-PG`, `OWN-RET`, `OWN-TRUST`. | `DVG-01`, `DVG-02`, `DVG-09`, `DVG-10`, `DVG-11` | `EC-03/04/05`; `EV-CP/IT/INT/PRD/SE` | `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-SCOPE-2U2P` · replay/lifecycle/export/restore/delete/scope points · `SCH-REPLAY/LIFECYCLE/RESTORE/SCOPE` · `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE`, `ORC-SCOPE` · `CFP-REPLAY-BEFORE-COMPACTION` | `B-REPLAY` + `B-RESTORE` + `B-SCOPE`; `PASS-POS`. | `BLOCK-ALL`; placeholder bytes, missing gap/provenance, cross-scope export, or non-replayable archive blocks. |
| `S2-REQ-008` | Long session, repeated chapter, reload, controlled upgrade, and measured envelope use only values adopted by the named owner; `OWN-MEASURE`, `OWN-REL` with `OWN-WEB`, `OWN-PG`, `OWN-RET`. | `DVG-01`, `DVG-03`, `DVG-07`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/04/05/06`; `EV-IT/INT/PRD/SE` | `FX-LONG-SESSION`, `FX-RECOVERY-EDITOR`, `FX-RESTORE-LIFECYCLE` · owner-adopted measurement/recovery/upgrade points · `SCH-LONG/CRASH/REPLAY/RESTORE` · `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH` · `CFP-REPLAY-BEFORE-COMPACTION` | `B-MEASURE` + `B-RECOVERY` + `B-RESTORE`; `PASS-STAGE` only for adopted values and complete journey. | `BLOCK-ALL`; unadopted target, stale measurement, or missing recovery/storage provenance blocks. |
| `S2-JRN-001` | Complete twelve-step editor journey: AI-disabled init, hierarchy, CN/EN input, save state, reload/crash/server/PG restart, isolated restore/roles/RLS/projections/lifecycle/visibility/write, takeover, search/replace, stats, replay/resync, export/archive, long session; `OWN-REL` and all Stage 2 owners. | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05/06/07`; `EV-CP/IT/INT/PRD/SE` | `FX-CONTRACT-R1`, `FX-HANDOFF`, `FX-SCOPE-2U2P`, `FX-EDITOR-IME`, `FX-JOURNAL-GROUP`, `FX-CORE-PROPOSAL`, `FX-RECOVERY-EDITOR`, `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE`, `FX-CONTEXT-DISCLOSURE`, `FX-ABSENT-EXECUTION`, `FX-LONG-SESSION` · `CFP-SCOPE-BEFORE-QUERY`, `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY`, `CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP`, `CFP-EDITOR-BEFORE-GROUP-ADMISSION`, `CFP-ADMISSION-BEFORE-CORE`, `CFP-ADMISSION-EXPIRY`, `CFP-CORE-BEFORE-COMMIT`, `CFP-CORE-AFTER-COMMIT-BEFORE-ACK`, `CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK`, `CFP-FENCE-AFTER-TAKEOVER`, `CFP-REPLAY-BEFORE-COMPACTION`, `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT`, `CFP-REPLAY-BELOW-FLOOR`, `CFP-RESTORE-STAGING`, `CFP-RESTORE-BEFORE-VISIBILITY`, `CFP-RESTORE-AFTER-VISIBILITY`, `CFP-LIFECYCLE-AFTER-INVALIDATION-BEFORE-CLEANUP`, `CFP-DELETE-BEFORE-SETTLEMENT`, `CFP-DELETE-AFTER-SETTLEMENT` · `SCH-NORMAL/CRASH/REORDER/FENCE/REPLAY/RESTORE/LIFECYCLE/LONG` · `ORC-CROSSWALK-COMPLETENESS`, `ORC-RECOVERY-ATOMICITY`, `ORC-RESTORE-LIFECYCLE` | `B-S2-JRN-001-EDITOR-RELEASE-SET`; `PASS-STAGE`. | `BLOCK-ALL`; any of the twelve steps absent, stale, unrun, unavailable, or unreplayable blocks Stage 2. |
| `S2-EVD-001` | Contract/REL crosswalk is complete for Stage 2; `OWN-REL`, `OWN-DVG`. | `DVG-01`, `DVG-13` | `EC-01`; `EV-CP/SE` | `FX-CONTRACT-R1`, `FX-HANDOFF` · contract drift/crosswalk points · `SCH-DRIFT` · `ORC-CROSSWALK-COMPLETENESS` · `CFP-CONTRACT-DRIFT` | `B-CONTRACT` + `B-HANDOFF`; `PASS-POS`. | `BLOCK-ALL`. |
| `S2-EVD-002` | Init/project/hierarchy/Snapshot evidence is attributable and scope-safe; `OWN-WEB`, `OWN-PROTO`, `OWN-CORE`, `OWN-RET`. | `DVG-02`, `DVG-03`, `DVG-09`, `DVG-11` | `EC-02/03/04/05`; `EV-CP/IT/INT/SE` | `FX-SCOPE-2U2P`, `FX-REPLAY-RETENTION` · scope/Snapshot/replay points · `SCH-NORMAL/REPLAY/SCOPE` · `ORC-SCOPE`, `ORC-REPLAY-TRUTH` · `CFP-REPLAY-AFTER-GENERATION-SNAPSHOT` | `B-SCOPE` + `B-REPLAY`; `PASS-POS`. | `BLOCK-ALL`. |
| `S2-EVD-003` | Browser/IME/keyboard/clipboard/undo/blocks/continuity/long-session evidence is complete; `OWN-WEB`, `OWN-MEASURE`. | `DVG-03`, `DVG-07`, `DVG-13` | `EC-02/06`; `EV-CP/IT/INT/SE` | `FX-EDITOR-IME`, `FX-JOURNAL-GROUP`, `FX-LONG-SESSION` · editor/group/crash/measurement points · `SCH-NORMAL/CRASH/REORDER/LONG` · `ORC-EDITOR-JOURNAL`, `ORC-RECOVERY-ATOMICITY` · `CFP-EDITOR-BEFORE-JOURNAL-DURABILITY` | `B-EDITOR` + `B-MEASURE`; `PASS-POS`. | `BLOCK-ALL`. |
| `S2-EVD-004` | Durable save states, Admission/Core, Receipt/Event, idempotency, and atomic settlement evidence is complete; `OWN-ADM`, `OWN-CORE`, `OWN-PG`, `OWN-RET`. | `DVG-02`, `DVG-03`, `DVG-07`, `DVG-08` | `EC-03`; `EV-CP/IT/INT/SE` | `FX-CORE-PROPOSAL`, `FX-RECOVERY-EDITOR` · admission/core/outbox/unknown/fence points · `SCH-NORMAL/CRASH/UNKNOWN/FENCE` · `ORC-ATOMIC-AUTHORITY`, `ORC-OUTCOME-UNKNOWN` · `CFP-CORE-AFTER-COMMIT-BEFORE-ACK` | `B-CORE` + `B-RECOVERY`; `PASS-POS` or `PASS-UNKNOWN`/`PASS-HOLD` by cut. | `BLOCK-ALL`. |
| `S2-EVD-005` | Full recovery, physical restore, RLS, projection, lifecycle, Recovery Visibility, and continued writing evidence is complete; `OWN-PG`, `OWN-RET`, `OWN-TRUST`. | `DVG-07`, `DVG-08`, `DVG-09`, `DVG-10`, `DVG-11` | `EC-04/05`; `EV-CP/INT/PRD/SE` | `FX-RESTORE-LIFECYCLE`, `FX-RECOVERY-EDITOR` · restore/visibility/delete/fence/replay points · `SCH-RESTORE/CRASH/FENCE/LIFECYCLE` · `ORC-RESTORE-LIFECYCLE`, `ORC-RECOVERY-ATOMICITY` · `CFP-RESTORE-BEFORE-VISIBILITY` | `B-RESTORE` + `B-RECOVERY`; `PASS-HOLD` before visibility and `PASS-POS` after proof. | `BLOCK-ALL`. |
| `S2-EVD-006` | Search/navigation/stats and Proposal-gated multi-location change evidence is bounded and attributable; `OWN-CTX`, `OWN-CORE`, `OWN-PROTO`. | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-04`, `DVG-11` | `EC-02/05/07`; `EV-CP/IT/INT/SE` | `FX-CONTEXT-DISCLOSURE`, `FX-CORE-PROPOSAL` · scope/projection/proposal points · `SCH-SCOPE/NORMAL/LIFECYCLE` · `ORC-CONTEXT-DISCLOSURE`, `ORC-ATOMIC-AUTHORITY` · `CFP-MANIFEST-BEFORE-COMMIT` | `B-CONTEXT` + `B-CORE`; `PASS-POS` or expected Proposal/refusal. | `BLOCK-ALL`. |
| `S2-EVD-007` | Export/archive order, unavailable content, scope, provenance, and lifecycle evidence is complete; `OWN-PROTO`, `OWN-PG`, `OWN-RET`. | `DVG-01`, `DVG-02`, `DVG-09`, `DVG-10`, `DVG-11` | `EC-03/04/05`; `EV-CP/INT/PRD/SE` | `FX-REPLAY-RETENTION`, `FX-RESTORE-LIFECYCLE` · replay/lifecycle/export/restore/delete points · `SCH-REPLAY/LIFECYCLE/RESTORE/SCOPE` · `ORC-REPLAY-TRUTH`, `ORC-RESTORE-LIFECYCLE` · `CFP-REPLAY-BEFORE-COMPACTION` | `B-REPLAY` + `B-RESTORE`; `PASS-POS`. | `BLOCK-ALL`. |
| `S2-EVD-008` | Long-session/storage measurements use only adopted values and preserve recovery/provenance; `OWN-MEASURE`, `OWN-PG`, `OWN-WEB`. | `DVG-01`, `DVG-09`, `DVG-10`, `DVG-13` | `EC-01/04/06`; `EV-IT/INT/PRD/SE` | `FX-LONG-SESSION`, `FX-RESTORE-LIFECYCLE` · measurement/replay/restore points · `SCH-LONG/REPLAY/RESTORE` · `ORC-RECOVERY-ATOMICITY`, `ORC-REPLAY-TRUTH` · `CFP-REPLAY-BEFORE-COMPACTION` | `B-MEASURE` + `B-RESTORE`; `PASS-STAGE` only with named adoption. | `BLOCK-ALL`. |

### 12.4 Stage 3 requirements, journey, and evidence

| ID | What is proven · accepted owner | Gate set | Evidence | Fixture · fault points · schedule · oracle | Safe bundle · pass | Block |
| --- | --- | --- | --- | --- | --- | --- |
| `S3-REQ-001` | Complete Stage 2 editor remains, adjacent to one general Project Agent Loop; `OWN-REL`, `OWN-AGENT`, `OWN-WEB`, `OWN-CORE`. | `DVG-03`, `DVG-06`, `DVG-07`, `DVG-11`, `DVG-13` | `EC-01/02/03/04/05`; `EV-CP/IT/INT/SE` | `FX-FAKE-MODEL`, `FX-RECOVERY-EDITOR` · run/mailbox/fence/core/editor points · `SCH-NORMAL/CRASH/FENCE` · `ORC-RUN-FINALIZATION`, `ORC-RECOVERY-ATOMICITY` · `CFP-FENCE-AFTER-TAKEOVER` | `B-FAKE` + `B-RECOVERY` + `B-HANDOFF`; `PASS-STAGE` with S2 regression. | `BLOCK-ALL`; a second fixed runtime, lost S2 path, or direct agent authority blocks. |
| `S3-REQ-002` | Bounded current request/intents remain distinct from authorization; `OWN-PLAIN`, `OWN-AGENT`, `OWN-CTX`. | `DVG-04`, `DVG-05`, `DVG-06`, `DVG-11`, `DVG-13` | `EC-01/07/08`; `EV-CP/IT/INT/SE` | `FX-CONTEXT-DISCLOSURE`, `FX-FAKE-MODEL` · scope/manifest/dispatch/proposal/mailbox points · `SCH-NORMAL/SCOPE/ABSENT` · `ORC-CONTEXT-DISCLOSURE`, `ORC-NEGATIVE-CLOSURE` · `CFP-SCOPE-BEFORE-QUERY` | `B-CONTEXT` + `B-FAKE`; `PASS-POS` for bounded advisory or `PASS-REFUSAL` when ambiguous/unauthorized. | `BLOCK-ALL`; broadening, hidden plan, or manufactured authorization blocks. |
| `S3-REQ-003` | Fake model goes through real Host/Scope/Context Assembly/selection/projection/manifest-before-egress/Attempt/fence/recovery/AgentRun; `OWN-CTX`, `OWN-AGENT`, `OWN-MODEL`, `OWN-TRUST`. | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-11` | `EC-03/04/05/08`; `EV-CP/IT/INT/SE` | `FX-FAKE-MODEL`, `FX-CONTEXT-DISCLOSURE` · manifest/dispatch/unknown/fence/mailbox points · `SCH-NORMAL/UNKNOWN/FENCE/SCOPE` · `ORC-CONTEXT-DISCLOSURE`, `ORC-DISPATCH-DISCLOSURE`, `ORC-OUTCOME-UNKNOWN` · `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS` | `B-FAKE` + `B-CONTEXT`; `PASS-POS` or `PASS-UNKNOWN` at the selected external cut. | `BLOCK-ALL`; bypass, missing manifest, wrong scope, hidden retry, or non-replayable fake path blocks. |
| `S3-REQ-004` | Editable anchored Core Proposal supports validation/refusal/conflict/recovery and explicit Acceptance/Rejection; `OWN-CORE`, `OWN-ADM`, `OWN-WEB`, `OWN-PLAIN`. | `DVG-03`, `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11` | `EC-03/07/08`; `EV-CP/IT/INT/SE` | `FX-CORE-PROPOSAL`, `FX-FAKE-MODEL` · proposal/acceptance/core/fence/late points · `SCH-NORMAL/CRASH/FENCE/SCOPE` · `ORC-ATOMIC-AUTHORITY`, `ORC-RECOVERY-ATOMICITY` · `CFP-PROPOSAL-BEFORE-DECISION` | `B-CORE` + `B-FAKE`; `PASS-POS`, `PASS-REFUSAL`, or `PASS-HOLD` per result. | `BLOCK-ALL`; auto-acceptance, direct write, lost conflict, or stale decision blocks. |
| `S3-REQ-005` | Adjacent chat is non-authoritative; fake/transcript/App/Agent cannot directly write prose or outline; `OWN-PLAIN`, `OWN-CORE`, `OWN-AGENT`, `OWN-MCP`, `OWN-EVAL`. | `DVG-03`, `DVG-05`, `DVG-06`, `DVG-11`, `DVG-12` | `EC-05/07/08`; `EV-CP/IT/INT/SE` | `FX-FAKE-MODEL`, `FX-ABSENT-EXECUTION`, `FX-EVAL-READONLY` · proposal/dispatch/mailbox/absence/Eval points · `SCH-ABSENT/SCOPE/NORMAL` · `ORC-NEGATIVE-CLOSURE`, `ORC-EVAL-READONLY` · `CFP-MANIFEST-BEFORE-COMMIT` | `B-ABSENT` + `B-EVAL` + `B-FAKE`; `PASS-REFUSAL` for direct/absent execution, `PASS-POS` for advisory view. | `BLOCK-ALL`; transcript authority, Agent outline, App write, or hidden Eval execution blocks. |
| `S3-REQ-006` | Bounded inspectable fake result/unknown recovery proves the StoryOS-owned boundary without making a model/provider quality claim; `OWN-MODEL`, `OWN-CTX`, `OWN-AGENT`, `OWN-REL`. | `DVG-04`, `DVG-05`, `DVG-06`, `DVG-08`, `DVG-09`, `DVG-11` | `EC-04/08`; `EV-CP/INT/SE` | `FX-FAKE-MODEL` · dispatch/unknown/reconcile/fence/late points · `SCH-UNKNOWN/FENCE/REPLAY` · `ORC-OUTCOME-UNKNOWN`, `ORC-DISPATCH-DISCLOSURE` · `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION` | `B-FAKE`; `PASS-POS`/`PASS-UNKNOWN`; the fake boundary records no Provider-internal fact. | `BLOCK-ALL` for missing fake proof or a Provider-quality claim; no real-destination evidence is a Stage 3 requirement. |
| `S3-REQ-007` | When fake assistance is unavailable, the complete Stage 2 author journey and every Stage 2 mandatory evidence item remain usable and no fallback capability is invented; `OWN-REL`, `OWN-WEB`, `OWN-CORE`, `OWN-AGENT`. Resolve `SMAP-STAGE-2` as the complete AI-independent regression set. | `SMAP-STAGE-2` | `SMAP-STAGE-2` | `SMAP-STAGE-2` | `B-S2-MANDATORY-SET`; `PASS-REFUSAL` for the unavailable fake path and `PASS-STAGE` for the complete Stage 2 continuation. | `BLOCK-ALL`; any Stage 2 journey step or mandatory evidence item missing, failed, stale, unrun, unavailable, unverified, or unreplayable after fake disablement blocks. |
| `S3-JRN-001` | Six-step Stage 3 journey—current passage/request, fake full path, edit/accept/reject, refusal/conflict/interruption, and fake-off repeat of the complete Stage 2 journey—is executable; `OWN-REL` with `OWN-PLAIN`, `OWN-CTX`, `OWN-AGENT`, `OWN-CORE`, `OWN-WEB`. Resolve `SMAP-STAGE-3` in full: its mandatory map carries the complete Stage 2 editor contract plus the fake, read-only Eval, and absence boundaries. The Stage 2 journey result is a regression output, not an input that certifies the Stage 3 map. | `SMAP-STAGE-3` | `SMAP-STAGE-3` | `SMAP-STAGE-3` | `B-S3-MANDATORY-SET`; `PASS-STAGE`. `B-FAKE` is a deterministic proof fixture, not a second Provider route. | `BLOCK-ALL`; any fake-path step, AgentRun/finalization boundary, Eval refusal/read-only fact, proposal decision, unknown recovery fact, or complete Stage 2 regression item is missing, failed, stale, unrun, unavailable, unverified, or unreplayable. |
| `S3-EVD-001` | Adjacent Agent/intent evidence proves one general loop and no task-specific runtime; `OWN-AGENT`, `OWN-PLAIN`, `OWN-REL`. | `DVG-03`, `DVG-06`, `DVG-13` | `EC-01/07`; `EV-CP/IT/INT/SE` | `FX-FAKE-MODEL`, `FX-HANDOFF` · run/mailbox/fence/crosswalk points · `SCH-NORMAL/FENCE/DRIFT` · `ORC-RUN-FINALIZATION`, `ORC-CROSSWALK-COMPLETENESS` · `CFP-FENCE-AFTER-TAKEOVER` | `B-FAKE` + `B-HANDOFF`; `PASS-POS`. | `BLOCK-ALL`. |
| `S3-EVD-002` | Host/Scope/assembly/manifest/Attempt/fence/recovery/AgentRun evidence is complete; `OWN-CTX`, `OWN-AGENT`, `OWN-MODEL`, `OWN-TRUST`. | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-08`, `DVG-11` | `EC-03/04/05/08`; `EV-CP/IT/INT/SE` | `FX-FAKE-MODEL`, `FX-CONTEXT-DISCLOSURE` · scope/manifest/dispatch/unknown/fence/mailbox points · `SCH-NORMAL/UNKNOWN/FENCE/SCOPE` · `ORC-CONTEXT-DISCLOSURE`, `ORC-DISPATCH-DISCLOSURE`, `ORC-OUTCOME-UNKNOWN` · `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS` | `B-FAKE` + `B-CONTEXT`; `PASS-POS`/`PASS-UNKNOWN`. | `BLOCK-ALL`. |
| `S3-EVD-003` | Editable Proposal evidence proves anchors, validation, refusal/conflict, explicit decision, and non-destructive state; `OWN-CORE`, `OWN-ADM`, `OWN-WEB`. | `DVG-03`, `DVG-07`, `DVG-08`, `DVG-11` | `EC-03/07`; `EV-CP/IT/INT/SE` | `FX-CORE-PROPOSAL` · proposal/acceptance/core/fence/late points · `SCH-NORMAL/CRASH/FENCE/SCOPE` · `ORC-ATOMIC-AUTHORITY` · `CFP-PROPOSAL-BEFORE-DECISION` | `B-CORE`; `PASS-POS`, `PASS-REFUSAL`, or `PASS-HOLD`. | `BLOCK-ALL`. |
| `S3-EVD-004` | Acceptance/Rejection receipts/actions/revision/non-destructive evidence is explicit; `OWN-CORE`, `OWN-ADM`, `OWN-WEB`. | `DVG-03`, `DVG-07`, `DVG-08` | `EC-03/07`; `EV-CP/INT/SE` | `FX-CORE-PROPOSAL`, `FX-RECOVERY-EDITOR` · proposal decision/settlement/recovery points · `SCH-NORMAL/CRASH/REORDER` · `ORC-ATOMIC-AUTHORITY`, `ORC-RECOVERY-ATOMICITY` · `CFP-PROPOSAL-AFTER-ACCEPTANCE-BEFORE-RECEIPT` | `B-CORE` + `B-RECOVERY`; `PASS-POS` or `PASS-HOLD`. | `BLOCK-ALL`. |
| `S3-EVD-005` | Interruption/late/stale evidence proves no blind retry, no stale decision, and disclosure truth; `OWN-CTX`, `OWN-CORE`, `OWN-PG`, `OWN-RET`. | `DVG-04`, `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11` | `EC-04/05/08`; `EV-CP/IT/INT/SE` | `FX-FAKE-MODEL`, `FX-RECOVERY-EDITOR` · unknown/fence/late/manifest points · `SCH-UNKNOWN/FENCE/CRASH` · `ORC-OUTCOME-UNKNOWN`, `ORC-DISPATCH-DISCLOSURE` · `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION` | `B-FAKE` + `B-RECOVERY`; `PASS-UNKNOWN` or `PASS-HOLD` as expected. | `BLOCK-ALL`. |
| `S3-EVD-006` | Fake limitation evidence states exactly what is and is not claimed; `OWN-MODEL`, `OWN-REL`, `OWN-DVG`. | `DVG-04`, `DVG-05`, `DVG-08`, `DVG-13` | `EC-01/08`; `EV-CP/SE` | `FX-FAKE-MODEL`, `FX-HANDOFF` · dispatch/unknown/crosswalk points · `SCH-UNKNOWN/DRIFT` · `ORC-DISPATCH-DISCLOSURE`, `ORC-CROSSWALK-COMPLETENESS` · `CFP-CONTRACT-DRIFT` | `B-FAKE` + `B-HANDOFF`; `PASS-POS` for the fake-boundary limitation record; no real-destination observation is required. | `BLOCK-ALL` if deterministic fake proof or limitation disclosure is absent; real-model evidence cannot be introduced into Stage 3. |
| `S3-EVD-007` | The complete Stage 2 mandatory evidence and complete Stage 2 author journey remain releasable after fake disablement; `OWN-REL`, `OWN-WEB`, `OWN-CORE`, `OWN-PG`. Resolve `SMAP-STAGE-2` directly; this is not a one-edit or recovery-only subset. | `SMAP-STAGE-2` | `SMAP-STAGE-2` | `SMAP-STAGE-2` | `B-S2-MANDATORY-SET`; `PASS-STAGE`. | `BLOCK-ALL`; any complete Stage 2 journey step or mandatory evidence item is absent, failed, stale, unrun, unavailable, unverified, or unreplayable. |

### 12.5 Stage 4 requirements, journey, and evidence

| ID | What is proven · accepted owner | Gate set | Evidence | Fixture · fault points · schedule · oracle | Safe bundle · pass | Block |
| --- | --- | --- | --- | --- | --- | --- |
| `S4-REQ-001` | One Provider-neutral real external-model route exists under Registration, Use Binding, compatibility, capability, credential, and policy; `OWN-MODEL`, `OWN-CTX`, `OWN-TRUST`, `OWN-PROTO`. | `DVG-01`, `DVG-02`, `DVG-04`, `DVG-05`, `DVG-08`, `DVG-11` | `EC-01/05/08`; `EV-CP/INT/RDA/SE` | `FX-REAL-MODEL-ADVISORY`, `FX-CONTEXT-DISCLOSURE` · scope/manifest/dispatch/unknown/scope points · `SCH-NORMAL/SCOPE/UNKNOWN` · `ORC-SCOPE`, `ORC-DISPATCH-DISCLOSURE` · `CFP-DISPATCH-BEFORE-CLAIM` | `B-CONTEXT` + `B-REAL-ADVISORY`; `PASS-POS` for StoryOS-owned route facts; real destination observations are `advisory`. | `BLOCK-ALL`; missing registration/binding/compatibility/credential boundary blocks, and advisory cannot upgrade. |
| `S4-REQ-002` | Real route uses the same Host/assembly/projection/manifest/destination/Attempt/fence/recovery/Proposal path; `OWN-CTX`, `OWN-MODEL`, `OWN-CORE`, `OWN-ADM`. | `DVG-03`, `DVG-04`, `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11` | `EC-03/04/07/08`; `EV-CP/INT/RDA/SE` | `FX-REAL-MODEL-ADVISORY`, `FX-CONTEXT-DISCLOSURE`, `FX-CORE-PROPOSAL` · manifest/dispatch/fence/unknown/proposal points · `SCH-NORMAL/UNKNOWN/FENCE/CRASH` · `ORC-CONTEXT-DISCLOSURE`, `ORC-OUTCOME-UNKNOWN`, `ORC-ATOMIC-AUTHORITY` · `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS` | `B-CONTEXT` + `B-REAL-ADVISORY` + `B-CORE`; `PASS-POS`/`PASS-UNKNOWN` for the StoryOS boundary. | `BLOCK-ALL`; any second path, hidden retry, bypass, or missing fake-independent proof blocks. |
| `S4-REQ-003` | Exact scope/source/provenance/destination/binding/profile/wire/digest/usage facts are recorded without credentials; `OWN-CTX`, `OWN-PROTO`, `OWN-MODEL`, `OWN-TRUST`. | `DVG-01`, `DVG-02`, `DVG-04`, `DVG-05`, `DVG-08`, `DVG-09`, `DVG-11` | `EC-01/05/08`; `EV-CP/INT/RDA/SE` | `FX-REAL-MODEL-ADVISORY`, `FX-CONTEXT-DISCLOSURE` · contract/manifest/scope/dispatch/lifecycle points · `SCH-DRIFT/SCOPE/UNKNOWN/LIFECYCLE` · `ORC-CONTRACT`, `ORC-SCOPE`, `ORC-DISPATCH-DISCLOSURE` · `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS` | `B-CONTRACT` + `B-CONTEXT` + `B-REAL-ADVISORY`; `PASS-POS`. | `BLOCK-ALL`; digest/provenance mismatch, credential leak, or unavailable disclosure evidence blocks. |
| `S4-REQ-004` | Crash/timeout/disconnect/post-dispatch is `OutcomeUnknown`; fence/late result/no blind resend and separately admitted reconciliation/successor are preserved; `OWN-CTX`, `OWN-PROTO`, `OWN-PG`, `OWN-RET`. | `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11` | `EC-03/04/05/08`; `EV-CP/INT/RDA/SE` | `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR` · `CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO`, `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION`, `CFP-RECONCILIATION-BEFORE-SETTLEMENT`, `CFP-LATE-RESULT` · `SCH-UNKNOWN/FENCE/CRASH` · `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY` | `B-REAL-ADVISORY` + `B-RECOVERY`; `PASS-UNKNOWN` then owner-defined `PASS-POS` reconciliation, or `PASS-HOLD` while unresolved. | `BLOCK-ALL`; unknown reported as success/failure, blind resend, stale completion, or unadmitted reconciliation blocks. |
| `S4-REQ-005` | Editable Proposal with explicit Accept/Reject, refusal/conflict/recovery remains the only path to author change; `OWN-CORE`, `OWN-ADM`, `OWN-WEB`, `OWN-PLAIN`. | `DVG-03`, `DVG-05`, `DVG-07`, `DVG-08`, `DVG-11` | `EC-03/07/08`; `EV-CP/INT/RDA/SE` | `FX-REAL-MODEL-ADVISORY`, `FX-CORE-PROPOSAL` · proposal/acceptance/fence/late points · `SCH-NORMAL/CRASH/FENCE/SCOPE` · `ORC-ATOMIC-AUTHORITY` · `CFP-PROPOSAL-BEFORE-DECISION` | `B-CORE` + `B-REAL-ADVISORY`; `PASS-POS`, `PASS-REFUSAL`, or `PASS-HOLD`. | `BLOCK-ALL`; auto-authority, missing editable state, or stale/conflicted decision blocks. |
| `S4-REQ-006` | Real author can write/request/inspect/edit/Accept/Reject/recover all StoryOS-owned facts; `OWN-REL`, `OWN-WEB`, `OWN-CORE`, `OWN-ADM`, `OWN-CTX`, `OWN-RET`. Resolve `SMAP-STAGE-4` so the real route is proven only after the complete Stage 3 deterministic fake/AgentRun/Eval/absence boundary and the Stage 2 editor journey remain intact; `EV-RDA` is advisory only. | `SMAP-STAGE-4` | `SMAP-STAGE-4` | `SMAP-STAGE-4` | `B-S4-REQ-006-AUTHOR-JOURNEY-SET`; `PASS-STAGE` after the S4 journey and mandatory evidence. `B-FAKE` is not a second Provider and `B-REAL-ADVISORY` cannot replace deterministic proof. | `BLOCK-ALL`; any uninspectable fact, missing decision, recovery gap, missing AgentRun finalization, hidden Eval execution, or Provider claim substituted for StoryOS evidence blocks. |
| `S4-REQ-007` | StoryOS-owned disclosure/recovery is provable; no Provider attention/retention/training/hidden SDK/literary claim is made; `OWN-CTX`, `OWN-MODEL`, `OWN-TRUST`, `OWN-REL`. | `DVG-04`, `DVG-05`, `DVG-08`, `DVG-11` | `EC-05/08`; `EV-CP/RDA/SE` | `FX-REAL-MODEL-ADVISORY`, `FX-CONTEXT-DISCLOSURE` · manifest/dispatch/unknown/late/scope points · `SCH-SCOPE/UNKNOWN/FENCE` · `ORC-DISPATCH-DISCLOSURE`, `ORC-NEGATIVE-CLOSURE` · `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS` | `B-CONTEXT` + `B-REAL-ADVISORY`; `PASS-POS` for StoryOS facts, `advisory` for external limitations. | `BLOCK-ALL` for missing StoryOS proof; advisory cannot upgrade and prohibited Provider claims invalidate the row. |
| `S4-JRN-001` | Seven-step Stage 4 journey—standalone editor, bounded request, pre-egress manifest, edit/accept/reject, post-dispatch unknown, reconciliation/recovery, and model-disabled repeat of the complete Stage 2 journey—is executable; `OWN-REL` and all Stage 4 owners. Resolve `SMAP-STAGE-4` in full. The real request uses the one general AgentRun/mailbox/finalization path already proven by the deterministic fake path, and the complete Stage 2 journey is repeated with the model disabled; `B-FAKE` is a contract-faithful proof fixture, not a second Provider. `EV-RDA` only observes the real destination and cannot replace deterministic proof. | `SMAP-STAGE-4` | `SMAP-STAGE-4` | `SMAP-STAGE-4` | `B-S4-JRN-001-SET`; `PASS-STAGE`. | `BLOCK-ALL`; any real-route boundary, manifest, DVG-06 AgentRun finalization, DVG-12 Eval refusal/read-only fact, unknown recovery, author decision, fake-path proof, or complete Stage 2 regression item is missing, failed, stale, unrun, unavailable, unverified, or unreplayable. |
| `S4-EVD-001` | Real identity/registration/binding/compatibility/capability/policy/credential evidence is complete; `OWN-MODEL`, `OWN-CTX`, `OWN-PROTO`, `OWN-TRUST`. | `DVG-01`, `DVG-02`, `DVG-04`, `DVG-05`, `DVG-11` | `EC-01/05/08`; `EV-CP/INT/RDA/SE` | `FX-REAL-MODEL-ADVISORY`, `FX-CONTRACT-R1` · contract/scope/manifest/dispatch points · `SCH-DRIFT/SCOPE/NORMAL` · `ORC-CONTRACT`, `ORC-SCOPE`, `ORC-DISPATCH-DISCLOSURE` · `CFP-DISPATCH-BEFORE-CLAIM` | `B-CONTRACT` + `B-REAL-ADVISORY`; `PASS-POS` for StoryOS identity facts. | `BLOCK-ALL`; missing or stale binding/policy/credential evidence blocks; advisory cannot upgrade. |
| `S4-EVD-002` | Ordered assembly/projection/manifest/disclosure/wire/scope evidence is complete before egress; `OWN-CTX`, `OWN-PROTO`, `OWN-TRUST`. | `DVG-01`, `DVG-02`, `DVG-04`, `DVG-05`, `DVG-11` | `EC-01/05/08`; `EV-CP/INT/RDA/SE` | `FX-CONTEXT-DISCLOSURE`, `FX-REAL-MODEL-ADVISORY` · scope/manifest/dispatch/contract points · `SCH-NORMAL/SCOPE/DRIFT` · `ORC-CONTEXT-DISCLOSURE`, `ORC-DISPATCH-DISCLOSURE` · `CFP-MANIFEST-AFTER-COMMIT-BEFORE-EGRESS` | `B-CONTEXT` + `B-CONTRACT`; `PASS-POS`. | `BLOCK-ALL`; any bytes before manifest, wire mismatch, or scope drift blocks. |
| `S4-EVD-003` | Attempt/fence/usage/unknown/reconciliation/late evidence is durable and truthful; `OWN-PROTO`, `OWN-CTX`, `OWN-PG`, `OWN-RET`. | `DVG-05`, `DVG-07`, `DVG-08`, `DVG-09`, `DVG-11` | `EC-03/04/05/08`; `EV-CP/INT/RDA/SE` | `FX-REAL-MODEL-ADVISORY`, `FX-RECOVERY-EDITOR` · dispatch/unknown/reconciliation/fence/late points · `SCH-UNKNOWN/FENCE/CRASH/REORDER` · `ORC-OUTCOME-UNKNOWN`, `ORC-RECOVERY-ATOMICITY` · `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION` | `B-REAL-ADVISORY` + `B-RECOVERY`; `PASS-UNKNOWN`, `PASS-POS`, or `PASS-HOLD` by cut. | `BLOCK-ALL`. |
| `S4-EVD-004` | Proposal/Accept/Reject/Receipt/Action/conflict/recovery evidence is explicit and non-destructive until acceptance; `OWN-CORE`, `OWN-ADM`, `OWN-WEB`. | `DVG-03`, `DVG-05`, `DVG-07`, `DVG-08` | `EC-03/07/08`; `EV-CP/INT/RDA/SE` | `FX-CORE-PROPOSAL`, `FX-REAL-MODEL-ADVISORY` · proposal/acceptance/core/fence/late points · `SCH-NORMAL/CRASH/FENCE/SCOPE` · `ORC-ATOMIC-AUTHORITY` · `CFP-PROPOSAL-BEFORE-DECISION` | `B-CORE` + `B-REAL-ADVISORY`; `PASS-POS`, `PASS-REFUSAL`, or `PASS-HOLD`. | `BLOCK-ALL`. |
| `S4-EVD-005` | Negative evidence proves no credential/cross-scope/hidden retry/stale/automatic outline/auto-authority path; `OWN-TRUST`, `OWN-CTX`, `OWN-MODEL`, `OWN-PLAIN`. | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-08`, `DVG-11` | `EC-05/08`; `EV-CP/RDA/SE` | `FX-SCOPE-2U2P`, `FX-REAL-MODEL-ADVISORY`, `FX-ABSENT-EXECUTION` · scope/dispatch/late/absence points · `SCH-SCOPE/UNKNOWN/FENCE/ABSENT` · `ORC-NEGATIVE-CLOSURE`, `ORC-OUTCOME-UNKNOWN` · `CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION` | `B-SCOPE` + `B-ABSENT` + `B-REAL-ADVISORY`; `PASS-REFUSAL`/`PASS-UNKNOWN` as expected. | `BLOCK-ALL`; any negative gap or advisory-only substitution blocks. |
| `S4-EVD-006` | Provider-boundary limitations are explicit; no external quality, attention, retention, training, or hidden SDK claim is treated as StoryOS proof; `OWN-MODEL`, `OWN-TRUST`, `OWN-REL`. | `DVG-04`, `DVG-05`, `DVG-08`, `DVG-13` | `EC-01/08`; `EV-CP/RDA/SE` | `FX-REAL-MODEL-ADVISORY`, `FX-HANDOFF` · dispatch/unknown/crosswalk points · `SCH-UNKNOWN/DRIFT` · `ORC-DISPATCH-DISCLOSURE`, `ORC-CROSSWALK-COMPLETENESS` · `CFP-CONTRACT-DRIFT` | `B-REAL-ADVISORY` + `B-HANDOFF`; `PASS-POS` for limitation record; external observation remains `advisory`. | `BLOCK-ALL` if limitation/proof boundary is absent; advisory cannot upgrade. |
| `S4-EVD-007` | The model-disabled regression consumes the complete Stage 2 mandatory evidence and complete Stage 2 author journey, proving independent manual-editor continuity and no hidden fallback; `OWN-REL`, `OWN-WEB`, `OWN-CORE`, `OWN-PG`, `OWN-AGENT`. Resolve `SMAP-STAGE-2` directly; real-destination advisory evidence cannot upgrade this regression. | `SMAP-STAGE-2` | `SMAP-STAGE-2` | `SMAP-STAGE-2` | `B-S2-MANDATORY-SET`; `PASS-STAGE`. | `BLOCK-ALL`; any complete Stage 2 journey step or mandatory evidence item is absent, failed, stale, unrun, unavailable, unverified, or unreplayable, even if real-destination evidence is advisory-green. |

### 12.6 Handoff gates

| ID | What is proven · accepted owner | Gate set | Evidence | Fixture · fault points · schedule · oracle | Safe bundle · pass | Block |
| --- | --- | --- | --- | --- | --- | --- |
| `HND-001` | Planning closes against one exact `main` baseline with no unresolved contradiction; `OWN-REL`, `OWN-GOV`, `OWN-DVG`. | `DVG-01`, `DVG-13` | `EC-01`; `EV-CP/SE` | `FX-CONTRACT-R1`, `FX-HANDOFF` · contract drift points · `SCH-DRIFT` · `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS` · `CFP-CONTRACT-DRIFT` | `B-CONTRACT` + `B-HANDOFF`; `PASS-STAGE`. | `BLOCK-ALL`; mixed baseline, stale revision, contradiction, or missing crosswalk row blocks. |
| `HND-002` | Planning/handoff proof locks exactly one bounded Stage 1 implementation issue after [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60), records the exact baseline and selected Stage 1 requirements, and proves that no product implementation is hidden in this contract; `OWN-REL`, `OWN-GOV`, `OWN-STAGE`. It is a planning closure, not Stage 1 implementation evidence. | `DVG-01`, `DVG-13` | `EC-01`; `EV-CP/SE` | `FX-CONTRACT-R1`, `FX-HANDOFF` · `CFP-CONTRACT-DRIFT` · `SCH-DRIFT` · `ORC-CONTRACT`, `ORC-CROSSWALK-COMPLETENESS` | `B-CONTRACT` + `B-HANDOFF`; `PASS-POS` for the exact bounded handoff record only. | `BLOCK-ALL`; a wrong baseline, missing Stage 1 selection, unbounded scope, extra issue, premature product implementation, or missing handoff record blocks. |
| `HND-003` | For exactly one currently evaluated stage, implementation evidence is complete and attributable to the exact baseline; the evidence record supplies one `evaluated_stage` and resolves only `SMAP-EVALUATED-MANDATORY-STAGE`, not a four-stage union or a release result; `OWN-REL` with that stage's accepted owners. The map is implementation evidence only and contains no stage-release disposition. | `SMAP-EVALUATED-MANDATORY-STAGE` | `SMAP-EVALUATED-MANDATORY-STAGE` | `SMAP-EVALUATED-MANDATORY-STAGE` | `SMAP-EVALUATED-MANDATORY-STAGE`; `PASS-STAGE` for the attributable mandatory evidence map only. | `BLOCK-ALL`; one selected-stage mandatory item that is failed, unrun, stale, unavailable, unverified, or unreplayable blocks, while a later stage's unrun evidence does not. HND-003 never consumes a stage-release disposition. |
| `HND-004` | Release of exactly one currently evaluated stage requires that stage's author journey and that stage's selected mandatory set both pass; only then records that stage's exact resulting `main` and the next-stage input; `OWN-REL`, `OWN-STAGE`. The evidence record supplies one `evaluated_stage`, `SMAP-EVALUATED-MANDATORY-STAGE` supplies the implementation input, and `SMAP-EVALUATED-RELEASE-STAGE` selects the corresponding ordered release branch; no four-stage union is required. | `SMAP-EVALUATED-MANDATORY-STAGE` | `SMAP-EVALUATED-MANDATORY-STAGE` | `SMAP-EVALUATED-RELEASE-STAGE` | `SMAP-EVALUATED-RELEASE-STAGE`; only after its mandatory map and author journey pass does it emit `EV-SR`, `PASS-STAGE`, the exact resulting `main`, and the next-stage input. | `BLOCK-ALL`; passing gates without the selected stage journey, or the journey without the selected stage mandatory pass, is not release, and a later stage's unrun evidence does not block this stage. |
| `HND-005` | Later controlled-cloud deployment evidence separately proves identity/security/recovery/cache/same-release/upgrade; `OWN-REL`, `OWN-PG`, `OWN-PROTO`, `OWN-TRUST`. | `DVG-01`, `DVG-02`, `DVG-04`, `DVG-05`, `DVG-07`, `DVG-09`, `DVG-10`, `DVG-11` | `EC-04/05/08`; `EV-CP/PRD/RDA/CCD` | `FX-RESTORE-LIFECYCLE`, `FX-CONTEXT-DISCLOSURE`, `FX-HANDOFF` · contract/scope/manifest/restore/lifecycle points · `SCH-DRIFT/SCOPE/RESTORE/LIFECYCLE/UNKNOWN` · `ORC-CONTRACT`, `ORC-SCOPE`, `ORC-RESTORE-LIFECYCLE` · `CFP-RESTORE-STAGING` | `B-RESTORE` + `B-CONTEXT` + `B-HANDOFF`; `PASS-CLOUD` only when later deployment owner adopts and completes it. | `BLOCK-ALL` for the later cloud handoff; it cannot block or upgrade local Release 1 by implication. |
| `HND-006` | Serial direction remains [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62) → [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) → [Create and Lock the First Editor-First Implementation Issue](https://github.com/FrankQDWang/StoryOS/issues/77); no parallel frontier or successor execution is implied; `OWN-REL`, `OWN-GOV`. | `DVG-13` | `EC-01`; `EV-CP/SE` | `FX-HANDOFF` · contract-drift/crosswalk points · `SCH-DRIFT/NORMAL` · `ORC-CROSSWALK-COMPLETENESS` · `CFP-CONTRACT-DRIFT` | `B-HANDOFF`; `PASS-POS`. | `BLOCK-ALL`; any changed frontier, prematurely claimed or started successor, parallel successor execution, or missing handoff record blocks. The [Create and Lock the First Editor-First Implementation Issue](https://github.com/FrankQDWang/StoryOS/issues/77) remaining unclaimed is correct until [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) has merged, resolved, and closed and the Wayfinder map has refreshed. |

## 13. Failure, recovery, and release decision procedure

For every crosswalk row, the executing owner follows this order:

1. Resolve the exact contract source, generated/catalog/wire identity, owner
   revision, fixture, and scope. If identity drifts, stop with `unverified` or
   the owner-defined refusal; do not refresh the baseline silently.
2. Run the positive schedule and every required semantic fault cut. A fault
   point omitted from the named proof is not covered by implication.
3. Compare durable facts with the independent oracle: Admission and command
   binding, Core write set, Receipt/Action, journal/group, Event generation,
   Attempt/fence, manifest/wire, lifecycle, availability, restore visibility,
   and deletion/non-revival as applicable.
4. Reopen the safe bundle from its seed and digests. If it cannot be replayed
   without secret or raw-content access, mark `Unreplayable` and block.
5. Assign exactly one gate disposition. `OutcomeUnknown` is retained when the
   dispatch boundary was crossed; it is never rewritten as a failure merely
   because a response is absent and never rewritten as success merely because
   bytes may have been sent.
6. Publish the row's evidence status and bundle digest at the same exact
   implementation baseline. A later source or catalog change makes the row
   `Stale` until re-run.
7. Assemble stage evidence only after all applicable rows pass. A stage release
   additionally runs its author journey and records the resulting main/next
   input; it does not inherit a green result from a different stage.

### 13.1 Recovery classification

| Observed cut | Required classification | Permitted next action |
| --- | --- | --- |
| Before journal durability, admission, Core commit, manifest commit, or dispatch claim | Refused/pending/no effect according to the owner contract; never committed by absence of an error. | Replay only the owner-permitted local operation; no blind author/external invocation. |
| After Core/Admission settlement but before acknowledgement or an applicable Event | Durable settled fact; browser/server recovery reconciles from it. An applied result can converge through its Activity position; a zero-authority Receipt has no Activity and converges through exact response/replay/query evidence. | Exact idempotent replay/convergence without inventing an Event. |
| After dispatch claim and before confirmed external result | `OutcomeUnknown`. | Fence original Attempt; separately admit reconciliation or a new successor Attempt only under owner rules. |
| After lease expiry/takeover or after settlement | Stale/late result. | Reject or record truthfully; no authority mutation or reopened settlement. |
| Before Seal/retention/lifecycle/restore visibility | Recovery hold/incomplete settlement. | Complete the owner-defined proof; no compaction, ordinary read, egress, or deletion completion. |
| After redaction/deletion/non-revival boundary | Current content unavailable/invalidated or scope deleted. | Preserve historical fact/gap; no cache, export placeholder, restore, or revival. |

### 13.2 Separation of evidence types

The following are separate obligations and cannot satisfy one another by
label:

- deterministic contract proof (`EV-CP`);
- implementation test (`EV-IT`);
- integration/E2E author journey (`EV-INT`);
- physical recovery drill (`EV-PRD`);
- real-destination advisory evidence (`EV-RDA`);
- stage evidence and stage release (`EV-SE`, `EV-SR`); and
- later controlled-cloud deployment evidence (`EV-CCD`).

In particular, a catalog self-test does not prove browser IME behavior, a
browser screenshot does not prove forced RLS, a fake model does not prove
Provider quality, a real Provider response does not prove manifest ordering,
and a backup success flag does not prove Recovery Visibility or non-revival.

## 14. Verification and change-control obligations

Before this contract or a later implementation handoff is accepted, the
reviewer must verify all of the following without changing the locked tracker
state:

- every `REL-*`, `S1-*`, `S2-*`, `S3-*`, `S4-*`, and `HND-*` identifier occurs in
  the crosswalk exactly once and resolves to existing gate, owner, evidence,
  fixture, fault-point, schedule, oracle, bundle, pass, and block definitions;
- every `DVG-*`, `CFP-*`, `FX-*`, `SCH-*`, `B-*`, `EC-*`, `EV-*`, and `ORC-*`
  reference resolves to this document's catalogue;
- every relative link and internal anchor resolves, and every named tracked
  owner/catalog/verifier path exists;
- the checked-in PostgreSQL and route catalog parse/self-tests both pass,
  including their negative self-tests;
- the target document is UTF-8, LF-only, has exactly one final newline, and
  passes `git diff --check`;
- no `.reference/**` file changes or dependency enters the proof input, and
  only the intended target document is changed for this contract task; and
- the complete diff has been reviewed for owner distortion, circular handoff,
  unverifiable evidence, stale baseline references, implementation selection,
  invented numeric defaults, and accidental stable-identifier repurposing.

An identifier split, merge, rename, or meaning change requires a repository-
wide reference inventory and explicit propagation analysis. Stable `DVG-01`
through `DVG-13` remain unchanged in this revision.

## 15. Accepted inputs and handoff boundary

This contract consumes the exact-baseline versions of:

- [CONTEXT.md](../../CONTEXT.md), the live [Map the StoryOS Editor-First Product and Production Delivery Contract](https://github.com/FrankQDWang/StoryOS/issues/1), [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60), [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62), [Create and Lock the First Editor-First Implementation Issue](https://github.com/FrankQDWang/StoryOS/issues/77), and [issue-tracker instructions](../agents/issue-tracker.md);
- [AI-Independent Editor-First Release Baseline and Handoff Criteria](ai-independent-editor-first-release-baseline-and-handoff-criteria.md);
- [ADR 0012](../adr/0012-adopt-deterministic-contract-verification.md);
- the owner contracts listed in section 4;
- [PostgreSQL Release 1 persistence catalog](postgresql-release-1-persistence-catalog.json)
  with [its verifier](verify-postgresql-release-1-persistence-catalog.py); and
- [Versioned Protocol Release 1 route catalog](versioned-protocol-release-1-route-catalog.json)
  with [its verifier](verify-versioned-protocol-route-catalog.py).

Repository history and `.reference/**` are not authoritative execution inputs.
The resulting proof selection is handed one-way to the next bounded
implementation owner after independent review. This document does not create,
claim, edit, comment on, close, or execute [Create and Lock the First Editor-First Implementation Issue](https://github.com/FrankQDWang/StoryOS/issues/77), merge a PR, update the
Wayfinder map, or select any implementation technology.
