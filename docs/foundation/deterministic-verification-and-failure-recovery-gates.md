# Deterministic Verification and Failure-Recovery Gates

- Status: accepted
- Wayfinder resolution: [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Architectural decision: [ADR 0012](../adr/0012-adopt-deterministic-contract-verification.md)
- Protocol boundary: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)
- Run, mailbox, replay, retention, and archival boundary: [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md)
- Manuscript and Proposal boundary: [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md)
- Artifact boundary: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Context and disclosure boundary: [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- Tool, MCP, and model boundary: [Persistent Agent Run and Orchestration Semantics](https://github.com/FrankQDWang/StoryOS/issues/47), [ToolSpec, Capability, Tool Gateway, MCP, and Skill Semantics](https://github.com/FrankQDWang/StoryOS/issues/48), and [ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50)
- Memory and research boundary: [Fiction Memory and Research Provenance Semantics](fiction-memory-and-research-provenance-semantics.md)
- Storage and recovery boundary: [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md)
- Web editor boundary: [Web Editor Session, Synchronization, and Recovery Semantics](web-editor-session-synchronization-and-recovery-semantics.md)
- Trust boundary: [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md)
- Eval boundary: [Foundation Evidence for the Standalone Eval Surface](eval-evidence-foundation.md)
- Repository and delivery boundary: [Modular-Monolith and Repository Governance Boundaries](modular-monolith-and-repository-governance-boundaries.md)

## 1. Purpose and authority

This specification defines the deterministic implementation gates that later
StoryOS code must satisfy. It turns accepted authority, protocol, durability,
disclosure, recovery, isolation, and retention contracts into inspectable
verification obligations. It is normative for a claimed implementation of
those contracts; it does not reopen their domain semantics.

The gate boundary is deliberately narrow. A deterministic gate can prove what
StoryOS prepared, admitted, persisted, dispatched at its local boundary,
recovered, refused, redacted, or made observable. It cannot prove that an
opaque model understood prose, that a Provider used every supplied token, or
how a Provider retained, trained on, logged, or later processed transferred
data. Those are not silently transformed into assertions merely because a fake
destination can simulate them.

The contract applies to the initial local Foundation Validation Deployment and
to a later controlled cloud deployment. Every fixture, generated trace,
fixture-derived record, fake-destination interaction, and oracle fact binds an
exact `ProjectScope { owner_user_id, project_id }`. A local deployment,
single-author Project, current Bailian adapter, cache, or test process never
creates a global User, Project, credential, destination, or authority shortcut.

This specification does not select:

- Rust crates, test frameworks, property-test libraries, fake-server
  implementation, process model, database schema, or fault-injection API;
- a CI vendor, runner topology, test-shard count, benchmark corpus, or
  performance default;
- a real Provider, embedding service, model, Tool, MCP server, account, or
  deployment destination;
- a production UI flow, editor-first release stage, or its acceptance
  scope; or
- a new source of truth, test-only authority path, Provider assurance, or
  exception to Context Assembly, Proposal/Acceptance, Project Isolation, or
  retention.

The modular-monolith governance contract owns repository verification command
mechanics, test framework choice, dependency direction, and CI wiring. The
editor-first baseline owns which coherent product subset first ships and its
product acceptance criteria. This contract supplies neither a hidden product
slice nor a way around that owner.

## 2. Fixed verification architecture

### 2.1 Deterministic proof boundary

Every deterministic gate uses only synthetic, non-secret project content,
scripted destinations, explicit schedules, and exact expected facts. A real
Provider call may produce separately labelled advisory evidence, but it is not
a substitute for a required gate and cannot make a missing or failed gate pass.

The proof target is always one or more StoryOS-owned facts:

| Proof target | A gate may prove | A gate must not claim |
| --- | --- | --- |
| Authoritative transition | Exact command classification, atomic current Heads, Revisions, Commit, Receipt, Event, and outbox intent | That a generated proposal is creatively good or that the author should accept it |
| External attempt | Correct seven-gate admission, manifests, wire projection, dispatch claim, Attempt identity, and known certainty | Provider receipt, internal use, retention, or semantic interpretation without separately admitted evidence |
| Run and worker lifecycle | Leases, fences, Mailbox ordering and seal, terminal settlement, replay, and late-result quarantine | That a dead process's volatile state was canonical |
| Recovery | The conservative recovered state, lifecycle visibility, and no duplicate authority/effect | That absence of a response proves non-submission |
| Security | Lack of unauthorized effect or disclosure and non-oracular public behavior | The existence or contents of another Scope |
| Retention and restore | Historical fact preservation, availability gaps, correct resync, and non-revival | Byte-identical replay of payload intentionally made unavailable |

### 2.2 Independent executable oracle

The required `Deterministic Verification Oracle` is a small executable state
model, maintained independently of the production runtime. It consumes a
synthetic command and event trace together with a deterministic schedule. It
defines the permitted set of durable facts after normal execution, interruption,
recovery, replay, or adversarial refusal.

The oracle models only contract-level identities, transitions, facts, and
uncertainty: Project Scope, command/idempotency binding, Receipts, Revisions,
Attempts, fences, Mailbox obligations, lifecycle decisions, event positions,
and allowed `OutcomeUnknown` states. It does not model database tables, worker
queues, cache structures, Provider internals, a browser editor implementation,
or an Adapter's private code. A test that mirrors a production repository or
uses private mock calls as its oracle does not conform.

Curated examples and Foundation Contract Walks are required readable witnesses.
Generated state-machine and property traces supply the coverage that examples
cannot: duplicate, reordered, delayed, concurrent, stale, crashed, recovered,
and cross-Scope combinations. A minimized counterexample remains a valid
fixture and must be replayable from its evidence bundle.

### 2.3 Controlled time and concurrency

Every deterministic gate uses a virtual monotonic clock and an explicit
interleaving schedule. A fixture controls start, pause, crash, restart, lease
expiry, retry eligibility, queue delivery, seal, compaction, and late result
delivery. Randomness is seeded and recorded; wall-clock sleep, thread timing,
network delay, and test-runner load cannot choose an outcome.

This does not erase the real recovery-service commitments. The Foundation
Recovery Service Profile still requires a release-candidate restore proof with
its declared RPO/RTO evidence. That drill measures its actual profile and is
not a synthetic latency/throughput benchmark or a substitute for deterministic
recovery semantics.

### 2.4 Contract-faithful fake destinations

A fake Provider, embedding service, Tool, MCP server, research adapter, or
MCP App-facing external resource is a Contract-Faithful Fake Destination. It
replaces only nondeterministic remote transport and scripted observations. The
normal Host path still derives identity and scope, applies Context Assembly,
checks current authority and compatibility, persists manifests and Attempts,
maps the non-secret wire projection, performs the dispatch claim, validates
the result, and treats a later external result as a new Context Candidate when
it is reused.

Each fake declares a bounded script vocabulary. The implementation may choose
its representation, but scripts must be able to express at least the following
contract observations where the destination class permits them:

| Scripted observation | Required interpretation |
| --- | --- |
| no route or pre-admission refusal | No Attempt or egress authority is created beyond the owning contract's refused evidence |
| admitted request, no dispatch claim | No external I/O and no Outbound Disclosure Event |
| dispatch claim then crash, timeout, disconnect, or lost response | `OutcomeUnknown`; no inferred success, failure, or zero usage |
| explicit later submission confirmation | Immutable reconciliation evidence may append certainty without rewriting the original claim |
| typed valid result or stream | Validate against the pinned contract before it can affect the next Run decision or become context |
| malformed, oversized, secret-bearing, hostile, or cross-Scope result | Refuse/quarantine it without authority, secret leakage, or cross-Scope use |
| duplicate, delayed, reordered, or late result | Deduplicate/fence/quarantine according to the owning Attempt, Mailbox, and Run contracts |
| contract, destination, credential-generation, or compatibility drift | Quarantine new work and require fresh admission; a name, alias, cache, or prior success is insufficient |

The fake has no privileged ingress into Core. It cannot manufacture an Agent
Decision, ToolCall result, Receipt, Proposal Acceptance, Destination Attempt,
or confirmed remote fact by direct mutation. A test-only hook that bypasses
one of these boundaries is a hard gate failure.

### 2.5 Evidence bundles and dispositions

Every deterministic case emits a non-secret `Verification Evidence Bundle`.
The contract does not prescribe a serialized schema or retention location, but
the bundle must make the verdict repeatable and must include:

1. stable gate identifier and fixture identity or property seed;
2. Project Scopes and synthetic identities used, with safe synthetic values;
3. exact source contract, schema, protocol, policy, profile, Adapter, and
   compatibility revisions that affect the case;
4. fake-destination script and virtual clock/interleaving schedule;
5. named Contract Fault Points taken and their pre/post boundary;
6. oracle's expected classification and expected fact set or fact digests;
7. observed safe durable facts or their stable digests, including receipts,
   attempts, fence generations, lifecycle gaps, and safe egress capture; and
8. a sanitized diagnostic summary sufficient to rerun and minimize a failure.

The Bundle contains no real manuscript content, credential value or digest,
session handle, nonce, raw provider transport, raw rejected request, or
undeclared foreign-Scope identifier. Declared synthetic fixture identities are
safe test inputs, not product data. A pass validates its expected evidence
shape; a failure preserves the same reproducible safe material. It is
verification evidence, not a new product Operational Record, telemetry
channel, or archive format.

The only deterministic gate dispositions are:

| Disposition | Meaning |
| --- | --- |
| `passed` | The oracle and all required evidence agree. |
| `expected_refusal` | The case proves a contract-required fail-closed rejection with its required no-effect or no-change evidence. |
| `expected_outcome_unknown` | The case proves conservative uncertainty after a possible effect boundary, with no hidden resolution or blind repeat. |
| `expected_recovery_hold` | The case proves a required recovery hold when lifecycle/recovery proof is incomplete or unsafe. |
| `failed` | The implementation contradicts the oracle or emits forbidden evidence/effect. |
| `unverified` | A required case was not run, cannot replay, lacks required evidence, or reaches an unclassified boundary. |
| `advisory` | A labelled non-gate observation, such as a real Provider experiment or empirical tuning result. It can never satisfy another disposition. |

`failed` and `unverified` are Fail-Closed Verification Gate results. They
block a claim that the affected contract implementation is complete. Re-running
until green, silently skipping, quarantining, or relabelling a case as advisory
does not change that result.

## 3. Contract fault matrix

Fault injection occurs at semantic boundaries, never at arbitrary source lines
or storage calls. A named point is part of the verification contract: the
owner defines its before/after fact boundary, and an implementation adds it to
the deterministic scheduler. A future durable or externally irreversible
boundary must add a corresponding row before its implementation can claim
conformance.

| Fault-point family | Required cuts | Required recovered proof | Semantics owner |
| --- | --- | --- | --- |
| `core.transition` | before commit; after commit before acknowledgement | Before: no partial domain effect. After: all Revisions, Heads, Receipts, Events, sequences, and outbox intents are one settled transition; exact idempotent retry returns the same acknowledgement. | Manuscript/Proposal, Artifact, PostgreSQL, Protocol |
| `editor.input` | before local journal durability; before and after Editor Input Fence and Proposal Pause Fence commit; before and after grouped command admission; at admission expiry; before first Core invocation; after admission before Receipt; after settlement before client acknowledgement or Project Activity | Every completed local intent has exact, non-overlapping coverage by one unsettled journal range until settlement or Draft recovery; an Agent batch cannot cross a committed input fence; only the same unexpired fully matching direct-edit admission may invoke during recovery; an explicit, expired, changed, or unverifiable command requires reconfirmation; OutcomeUnknown blocks blind invocation; and acknowledgement loss, resync, or regrouping cannot lose or duplicate prose. | Web Editor Session, Author Command Admission, Manuscript/Proposal, Protocol, PostgreSQL |
| `outbox.delivery` | before claim; after claim before consumer acknowledgement; duplicate delivery | Intent is not business settlement; consumer deduplicates and validates the current fence. | Run/Mailbox, PostgreSQL |
| `external.dispatch` | before durable dispatch claim; after claim before/after bytes or response; reconciliation arrival | Before claim proves no external I/O or Disclosure Event. After claim remains `OutcomeUnknown` until ordinary immutable evidence settles it. | Context/Disclosure, Model Gateway, Tool/MCP, PostgreSQL, Protocol |
| `lease.fence` | old lease expiry; recovery fence; late worker/result | Old, duplicate, cancelled, or superseded worker cannot settle, publish, consume authority, or create new effects. | Run/Subrun/Mailbox, PostgreSQL |
| `mailbox.seal` | before seal; after seal; duplicate/reordered/late message | Seal preserves high-watermark and deduplication proof; late input cannot reapply or reopen terminal work. | Run/Mailbox/Retention |
| `replay.generation` | before compaction boundary; after new generation/Snapshot publication; old cursor resume | No guessed mapping or silent gap; valid old cursor resumes only where retained, otherwise gets cursor-too-old and safe resync. | Protocol, Retention |
| `lifecycle.invalidation` | before and after Redaction, Tombstone, suppression, archival, or deletion settlement | Current eligibility changes immediately; cache, projection, export, restore, and Provider continuity cannot revive unavailable content or authority. | Artifact, Memory/Research, Context/Disclosure, Retention |
| `restore.visibility` | restore staging validation; recovery-chain/lifecycle proof; first visibility | Bad staging changes no live state; incomplete later lifecycle proof gives a recovery hold; visible restore has exact Scope and no merge/remap/revival. | PostgreSQL, Retention, Protocol |
| `project.deletion` | request admission; in-flight settlement; post-settlement delivery/recovery | New work and disclosure are fenced; known work settles or stays explicit `OutcomeUnknown`; deletion cannot be bypassed by archive or recovery. | ADR 0011, Retention, PostgreSQL |

The table names contract families, not mandatory function names. A case may take
several points. The evidence bundle records each selected point and the
observed boundary. A missing applicable point is `unverified`, not an
opportunity to infer that a code path has no failure mode.

## 4. State-machine and property obligations

### 4.1 Multi-Scope adversarial base world

Every property and recovery generator starts from at least two Users and two
Project Scopes. It creates same-shaped foreign Artifacts, Revisions, Runs,
Messages, Attempts, cursors, Snapshots, grants, approvals, Credential
References, Processing Destination Identities, manifests, cache entries, and
archive records. It then substitutes them at each relevant join.

A property passes only if foreign substitution produces the exact safe
classification required by the owning contract: non-oracular rejection or
safe `resource_not_found` where applicable, no foreign data in diagnostics,
no authoritative mutation, no context eligibility, no egress, no budget
effect, no accidental cache reuse, and no cross-Scope idempotency/key
collision. Single-Scope positive fixtures remain useful examples but cannot
prove Project Isolation.

### 4.2 Required cross-cutting properties

The independent oracle and generated traces must enforce all of these
properties wherever their owning contract applies:

1. **Scope noninterference.** Changing a foreign Scope's data, projection,
   cache, registration use, or lifecycle cannot change an in-Scope operation
   except through a safe, attributable refusal of a shared resource boundary.
2. **Exact idempotency.** An identical scoped command/key/digest replay yields
   the same logical acknowledgement and Receipt; a changed route, kind, Scope,
   or digest cannot reuse it.
3. **Atomic visible settlement.** A Core transition is either absent or has its
   complete contract write set. Refusals/conflicts may have their required
   no-change Receipt, but no partial authority effect appears.
4. **Monotonic fencing.** No stale, duplicate, expired, cancelled, or
   superseded lease/fence holder may settle, publish, or consume new authority.
5. **Attempt separation.** Each retry, resend, redispatch, repair, fallback,
   or changed destination has a new Attempt. A prior unknown Attempt remains
   attributable and consumes the required conservative reservation.
6. **Manifest-before-egress.** No external bytes leave before the complete
   current admission, context, destination, disclosure, wire, and dispatch
   claim evidence exists; every returned external value re-enters Context
   Assembly before another destination sees it.
7. **Provider opacity.** StoryOS evidence never upgrades a fake script,
   transport absence, cache hint, or opaque Provider state into a claim of
   internal use, receipt, retention, semantic quality, or zero usage.
8. **Replay truthfulness.** Replay, Query, and Snapshot responses remain bound
   to their current generation, authorization, redaction, and availability;
   they never guess a cursor mapping or silently hide a lifecycle gap.
9. **Non-revival.** A redacted, tombstoned, compacted-unavailable, deleted, or
   unavailable payload cannot become readable, eligible, exportable, or
   authoritative through any rebuildable or recovery path.
10. **No hidden authority.** Tool, MCP, App, Provider, Eval, cache, model
    output, generated test fixture, and fake destination cannot create an
    authoritative change outside a direct Author Action or a Proposal accepted
    by the author.

### 4.3 Generated trace limits

Property generation is bounded by the accepted Protocol Limit Profile and
fixture-specific safe bounds. It must shrink a counterexample while preserving
the relevant contract identity, schedule, and evidence. A seed cannot evade a
hard semantic limit, and a larger random campaign cannot replace a missing
fixed regression fixture.

No future latency, throughput, token-cost, corpus-quality, or model-quality
number is declared verified here. Existing protocol ceilings, exact token
counting mappings, archive validation limits, and the Foundation Recovery
Service Profile retain their own accepted meanings and proofs.

## 5. Required deterministic gate catalogue

Every row is a required gate family for the matching implementation surface.
The listed contracts own the semantics; this catalogue owns the deterministic
proof, failure schedule, and evidence requirement. A surface not yet in a
selected editor-first release stage is not implemented merely because this
document names its eventual gate; the baseline alone selects when a coherent
subset is first delivered.

| Gate ID | Required proof and adversarial cases | Required evidence and passing disposition | Semantic owner |
| --- | --- | --- | --- |
| `DVG-01` Contract-source, generated-schema, and golden-wire consistency | Regeneration/diff gates for Rust contract source, OpenAPI, JSON Schema, TypeScript, catalogs, examples, the active same-release corpus, historical projections, limit profiles, Application Wire Records, and SSE frames. Execute the protocol's canonical positive and all adversarial fixture families, including closed inputs, duplicate names, unknown control values, schema/limit drift, archive path/profile, release mismatch, and cache-refresh fixtures. | Generated corpus digest, active release/profile, safe positive/negative result, and exact drift classification. Any uncontrolled worktree drift, mixed-release activation, unsafe historical projection, or fixture mismatch is `failed`. | Protocol; repository governance owns command mechanics. |
| `DVG-02` Request, Query, cursor, and scope isolation | Generate Host, Origin, Client Session Binding/generation, accepted client-contract/security-policy revision, nonce/idempotency, prospective-Project creation, action-class, command-digest, target, expected-Head, Editor Session, and writer-generation substitutions; also generate wrong owner/Project/object/cursor/Snapshot/Capability/Approval/Credential references, stale/wrong replay cursors, and projection lag. Run direct non-owner runtime/RLS probes with missing, partial, stale, and cross-Scope transaction-local settings. Verify exact scope joins, non-oracular error classes, required canonical snapshots, exclusive resume, bounded at-least-once duplicate handling, and no cache/projection shortcut. | Multi-Scope trace, forced-RLS/runtime-role posture, public safe result, sanitized durable pre-admission refusal evidence where applicable, zero unauthorized effect/egress, and exact admission/Snapshot/cursor evidence. Expected refusal may pass only with Negative Evidence Closure; it never receives an `AuthorCommandAdmissionId`. | Author Command Admission; Web Editor Session; Protocol; PostgreSQL; trust model. |
| `DVG-03` Author edit, Web Editor Session, and Proposal transaction | Browser integration proves complete IME and editor-intent capture, journal-before-submission durability, immediate pending projection, bounded command grouping, writer-generation fencing, acknowledgement/Event convergence, resync, and recovery without loss or duplication. For every admission action class, generate exact duplicates, changed-digest reuse, refusal, expiry-edge, binding-change, crash, OutcomeUnknown, read-only reconciliation, acknowledgement-loss, and reconfirmation traces across the full User/Scope, Client Session, client-contract, security-policy, Editor Session/writer, request-contract, digest, target/Head, nonce/idempotency, and lifetime binding set. Core properties also generate authoritative, Proposal, mixed, stale, concurrent, undo, conflict, Validation, selected-operation, dry-run, Acceptance, and no-effect edits. | Editor Session fixture plus oracle facts: complete settled-or-recoverable input; at most one admission for the scoped idempotency record; append-only pending/OutcomeUnknown/reconciliation evidence; exactly one `ReceiptSettled` or `RequiresReconfirmation` terminal settlement; whole-command outcome; current Heads; exact Revisions/Commit where applicable; typed Receipt; Event/outbox intent; same-admission idempotent replay; and journal collection only after convergence. Only an unexpired fully matching direct edit with validated no-Receipt evidence may invoke during recovery. An explicit, expired, changed, or intent-unrecoverable command requires visible reconfirmation and a new admission; a mixed edit creates only the complete refused Draft; Acceptance or Undo without a Receipt never executes automatically. | Web Editor Session; Author Command Admission; Manuscript/Proposal; Artifact; Protocol; PostgreSQL. |
| `DVG-04` Context Assembly, retrieval, Memory, and disclosure | For every source role, test all seven gates, eligibility-before-ranking, source revision/suppression/redaction changes, deterministic excerpt/summary boundaries, dynamic retrieval, cache/provider-continuity non-bypass, and re-entry of Tool/MCP/Provider/research output. Include hostile content in every source class and cross-Scope retrieval/index/cache rows. | Context and destination manifests, candidate/selection/refusal reasons, exact source revisions, safe egress capture, and zero use of ineligible content. Missing evidence or a widening is `failed`; absent proof of safe recovery is `unverified`. | Context/Disclosure; Memory/Research; trust model. |
| `DVG-05` Model Gateway, Provider, Tool, MCP, and App mediation | Drive Contract-Faithful Fakes through registration/use-binding/identity/compatibility/admission, adapter wire mapping, Credential generation, cancellation, repair/retry/fallback, Tool effect ceilings, MCP discovery drift, App bridge spoof/replay/sequence/termination, SSRF/redirect/DNS/private-address cases, and hostile external output. Verify controlled and external destinations separately. | Pinned non-secret registration/binding/identity evidence, manifests, Attempt/fence/admission record, wire digest, scripted observation, validated/quarantined result, and disclosure evidence. Direct Core shortcut, hidden SDK retry, missing seven-gate reference, or drift reuse is `failed`. | Tool/MCP; Model Gateway; Context/Disclosure; Protocol; trust model. |
| `DVG-06` Run, Subrun, Mailbox, and finalization state machines | Generate parent/child Run and Subrun traces with message duplicates, reordering, loss/restart, direct-child delivery, waits/holds, finalization intent, terminal result, seal, late message, parent recovery, and cancellation. Exercise stale lease/fence and resource/budget accounting schedules. | Run/Step/Subrun identities, directional high-watermarks, deduplication and Seal facts, Result/Outcome, terminal Events, delivery intent, and quarantine evidence. A terminal Run cannot reopen or claim success with unresolved uncertainty. | Persistent Run; Subrun/Mailbox; Retention; PostgreSQL. |
| `DVG-07` Transaction, outbox, lease, fence, and crash recovery | Take every applicable Contract Fault Point for Core transitions, outbox claims, dispatch claims, lease recovery, and stale result delivery. Compare interrupted/restarted state to the oracle's permitted facts and rerun the same schedule. | Fault schedule, before/after facts, exact idempotent outcome, Attempt/fence generations, outbox dedupe, recovery decision, and safe diagnostic. Partial transition, late settlement, duplicate authoritative effect, or unreplayable crash case is `failed`. | PostgreSQL; Run/Mailbox; Protocol; Context/Disclosure. |
| `DVG-08` `OutcomeUnknown` and conservative reconciliation | For model, Tool/MCP, research, embedding, export, and other effectful destinations, inject crash/timeout/disconnect after a durable claim. Test no inferred no-send/success/failure/zero usage, explicit late confirmation only through ordinary ingress, new Attempt/disclosure/budget on successor, and cancellation/recovery fences. | Original claim, manifests/wire projection, `OutcomeUnknown` settlement, conservative budget/usage state, any separately admitted reconciliation, and successor Attempt relation. A fake's hidden receipt cannot resolve the case. | Context/Disclosure; Model Gateway; Tool/MCP; Protocol; PostgreSQL. |
| `DVG-09` Replay, compaction, redaction, and archival | Test cursor resume, older retained resume duplicates, generation handoff, cursor-too-old/resync, compaction Evidence Floor, post-Seal Mailbox dedupe, historical wire preservation, redaction/suppression/tombstone immediate logical effect, and cache/projection/Provider-continuity invalidation. | Snapshot/generation/floor/resync facts, lifecycle decision, availability gap, safe historical descriptor, and negative scans of cache/projection/export paths. A guessed mapping, silent gap, or revived unavailable bytes is `failed`. | Retention/Archival; Protocol; Memory/Research; Context/Disclosure. |
| `DVG-10` Archive, restore, recovery visibility, and Project deletion | Use a deterministic hostile archive corpus: path encoding/collision/traversal/device/link/bomb/digest/profile/signature/Scope/reference failures; valid export/restore; corrupt/missing WAL and lifecycle range; role/RLS restore checks; deletion settlement with pending/unknown work. Verify no partial visibility, no remap/merge/revival, and no post-deletion operation. | Archive/root/protected-input proof or safe rejection, staging disposition, exact Scope/identity, Recovery Visibility Proof or recovery hold, projection rebuild comparison, deletion settlement/fence evidence. Any visible partial state or revival is `failed`. | PostgreSQL; Retention/Archival; Protocol; ADR 0011; trust model. |
| `DVG-11` Security negative-evidence corpus | Execute the trust model's hostile Origin, bridge, Tool/MCP, prompt, Provider, SSRF, archive, role/RLS, credential, log/support/telemetry, stale-worker, replay, tamper, restore, and resource-bound cases. For each, inspect emitted public output and all safe test artifacts rather than only source code or an HTTP status. | Negative Evidence Closure: no unauthorized authority, context, egress, Attempt, budget effect, secret, or undeclared foreign identity; non-oracular public shape; safe scanned artifacts. A leak or a forbidden side effect is `failed`. | Trust model; Protocol; PostgreSQL; Context/Disclosure; Retention. |
| `DVG-12` Standalone Eval boundary | Test that opening/refreshing Eval is a scoped redacted read with no model/judge/egress/Run side effect; Eval Case/Corpus selection is explicit; advisory assessment is a new ordinary attempt; external judge output is advisory and re-enters normal boundaries; baseline/feedback never control writing or routing. | Eval Case/View facts, read Snapshot/visibility gap, zero-egress evidence for view-only operations, and ordinary Attempt/manifest evidence for explicit assessment. Any ambient monitoring, page-load dispatch, authority, or hidden scoring effect is `failed`. | Eval; Context/Disclosure; Run; Protocol. |
| `DVG-13` Foundation Contract Walks | Execute the small synthetic cross-boundary walks in section 6, including their adversarial/recovery variants, using the same oracle, fake destinations, scheduler, and evidence bundles as all other gates. | One Bundle per walk with boundary facts and exact expected disposition. A real Provider, selected product slice, or unbounded UI journey is not a passing substitute. | This specification; each linked semantic owner. |

## 6. Required Foundation Contract Walks

These walks are conformance witnesses. They intentionally use synthetic prose
and fake destinations; they are not a claim that the complete product UI or a
specific editor-first release stage exists.

### 6.1 Current-passage author edit and acceptance

An author makes complete editor input against a current passage. The browser
durably journals the input, projects it immediately, and submits the bounded
semantic command units selected by the production editor-session contract.
The walk covers Author Command Admission, authoritative edit, Proposal
revision, mixed-target refusal, exact idempotent retry, stale Head conflict,
Proposal validation, selected Acceptance, writer takeover, resync, and crash
cuts before and after journal, admission, Core commit, acknowledgement, and
Project Activity. It proves no direct Agent/Tool write, no partial mixed
change, no lost or duplicated input, and exact settlement or recovery of every
journaled intent.

### 6.2 Agent request, external dispatch, and unknown recovery

An author starts a scoped AgentRun from current passage context. A fake model
destination is admitted through the full seven-gate and Model Gateway path.
The scheduler crashes immediately after the dispatch claim. The recovered
system records `OutcomeUnknown`, preserves conservative budget and disclosure
evidence, and does not resubmit. A separately scripted reconciliation or a new
authorized successor creates the only permitted settlement path. The fake may
then return a valid candidate; it becomes a Proposal, never an authoritative
edit.

### 6.3 Tool/MCP result re-entry and drift

An admitted Tool or MCP call returns bounded synthetic output. The walk proves
that the result is validated and retained only as its typed result/provenance,
then crosses Context Assembly again before a model or another destination uses
it. A same-name replacement, changed Adapter, widened effect, stale credential
generation, spoofed App bridge message, or incompatible contract observation
quarantines new work rather than reusing past eligibility or authority.

### 6.4 Subrun, Mailbox, and fenced recovery

A root Run creates a Subrun and has both directions of Mailbox delivery. The
schedule duplicates and reorders messages, expires an old worker lease, seals
the root, crashes, then delivers late results. It proves stable Message IDs,
directional order/deduplication, sealed rejection, fence enforcement,
one-result terminal settlement, and no revived parent/child work.

### 6.5 Replay, redaction, archive, and recovery visibility

A Project creates events and an external Attempt, crosses a replay-generation
boundary, redacts or tombstones a payload, and is restored from a valid or
incomplete recovery chain. The walk proves authorized Snapshot resync, visible
availability gaps, non-revival across cache/projection/export/restore, and a
recovery hold until Recovery Visibility Proof is complete. A malformed archive
or foreign Scope never becomes partially visible.

### 6.6 Cross-Scope hostile path and Eval read-only path

Two synthetic Projects use similarly shaped identities. The path substitutes
foreign objects through commands, queries, cursor resume, context retrieval,
cache, Tool/MCP, Provider, archive, and test diagnostics. It proves Negative
Evidence Closure. It then opens an Eval Evidence View for one scoped case and
proves that this read causes no egress, judge request, hidden monitoring,
authority change, or score-based control.

## 7. Failure and recovery classification

The test suite must distinguish normal fail-closed refusal from uncertainty;
it must not treat every failure as `OutcomeUnknown` or every timeout as a
known failure.

| Situation | Required gate result | Required StoryOS evidence |
| --- | --- | --- |
| Invalid/missing/foreign scope, session, authority, Capability, Approval, manifest, profile, compatible contract, or schema before admission | `expected_refusal` | Safe typed refusal/no-change Receipt where the owning command contract requires one; zero unauthorized authority/egress/effect and Negative Evidence Closure. |
| Core transition fails before commit | `expected_refusal` or normal recovery re-execution as the owning command permits | No partial domain effect; no acknowledged success. |
| Core transition commits but acknowledgement is lost | `passed` after recovery | Exact same scoped idempotent acknowledgement/Receipt; no duplicate transition. |
| Author Command Admission exists but authoritative settlement cannot yet be proven | `expected_outcome_unknown` | Same admission and idempotency evidence, last provable boundary, no terminal settlement, no blind invocation, and an author-visible reconciliation requirement. |
| Validated authoritative storage proves no Receipt for an admitted author command | `passed` only through the owning recovery branch | The same unexpired fully matching direct edit may invoke once under the same admission; an explicit, expired, changed, or intent-unrecoverable command settles `RequiresReconfirmation` without Core execution. |
| External operation fails before durable dispatch claim | `expected_refusal` or typed pre-dispatch failure | No external I/O/Outbound Disclosure Event; any prepared evidence remains truthful and non-authorizing. |
| External operation can have crossed a durable dispatch/effect boundary but response/settlement is absent | `expected_outcome_unknown` | Exact Attempt/claim/manifests/wire facts, conservative budget/usage, no inference, no blind retry. |
| Late ordinary reconciliation proves a result | `passed` after reconciled oracle comparison | Immutable later evidence appended to the original Attempt; historical claim is not rewritten. |
| Required recovery lifecycle range, archive integrity, Scope, identity, or staged restore proof is absent/invalid | `expected_recovery_hold` or `expected_refusal` | No ordinary visibility/execution/partial restore; safe lifecycle/recovery diagnostic only. |
| Redaction/Tombstone/deletion makes payload unavailable | `passed` only with non-revival | Preserved permitted historical fact and availability gap; no readable/eligible/exported/recovered payload. |
| Any result lacks a Bundle, deterministic replay, expected owner fact, or classified fault point | `unverified` | Fail-closed gate block; no advisory evidence may upgrade it. |

## 8. Security and observability rules

Security gates use the Multi-Scope Adversarial Verification World and must
exercise all trust-model attack paths at the public, Core, Worker, Adapter,
destination, archive, recovery, and observable-diagnostics boundaries. A
passing gate is not a process that returns `403` or `404`; it is Negative
Evidence Closure over the complete safe test observation set.

At a minimum, the corpus must include the protocol's canonical adversarial
fixtures and the threat-model families for:

- forged/cross-Scope request identities, cached identities, cursors, grant and
  credential substitutions, role/RLS context omissions, and connection-pool
  reuse;
- hostile Host/Origin/session/nonce/idempotency combinations and non-oracular
  object/cursor/query behavior;
- context/prompt injection across manuscript, memory, research, summary,
  Tool, App, retrieval, and Provider-returned content;
- Tool/MCP discovery and schema drift, effect widening, nested egress, bridge
  spoofing, bridge sequence/replay abuse, and App termination;
- Provider/embedding/research redirects, DNS rebinding, private/link-local
  address, proxy/metadata credential, fallback, and disclosure widening;
- malformed, duplicated, zip-bomb, path-confused, forged, or cross-Scope
  archive/import/restore material;
- credential values/digests/locators and raw project content in arguments,
  results, wire records, logs, diagnostics, telemetry, support bundles,
  archives, exports, and generated test output; and
- tampered, duplicate, delayed, reordered, stale, crashed, replayed, or
  restore-derived durable facts and resource-bound exhaustion.

The exact public status code and safe representation are owned by the Protocol.
The exact scan implementation is owned by repository governance. This contract
requires their observable result: no raw secret or undeclared foreign identity in the
allowed test artifacts, and no hidden external effect or authority mutation.

## 9. Replay, retention, and recovery proof

Recovery tests begin from durable facts and explicitly discard process memory,
live connections, uncommitted transaction state, cache contents, and fake
destination private state. They restart with only the evidence that the owning
contract permits. The recovered result must equal an oracle-permitted state;
it must not be repaired by peeking at a fake's hidden receipt or by manually
editing canonical data.

For replay and retention, equality means historical truthfulness plus current
availability, not byte-for-byte preservation of unavailable payload. The
minimum evidence floor remains inspectable, but redacted, tombstoned,
compacted-unavailable, archived-only, or deleted payload cannot return through
an index, cache, cursor mapping, export, Project Restore, recovery copy,
Provider continuity, or projection rebuild.

Project Restore and disaster recovery require both:

1. physical recovery/archive integrity, exact Project Scope/identity, and
   canonical reconstruction proof under the storage/protocol contracts; and
2. Recovery Visibility Proof that applies every recoverable later lifecycle
   decision before ordinary readability or execution.

A test may report an expected recovery hold. It may never turn a missing WAL
range, lifecycle event, redaction/tombstone decision, archive proof, or
deletion settlement into an apparently current Project.

## 10. Ownership and handoff

| Owner | Receives from this contract | Still owns |
| --- | --- | --- |
| Repository governance and modular monolith | Fail-closed verification requirement, evidence-bundle obligations, generated/deterministic gate families | Rust/test package layout, selected libraries, fake implementation, scripts, CI runner, final `verify` command, dependency-direction checks |
| Versioned Protocol | Execution of required wire/generation/adversarial corpus and safe result classification | DTO/schema/wire/cursor/error/compatibility/limit semantics and generators |
| PostgreSQL storage | Crash, concurrency, RLS, migration, export/restore, and recovery proof obligations | Tables, roles, policies, transaction mechanics, migration and backup implementation |
| Run/Subrun/Mailbox and retention | State-machine, seal/fence, outbox, replay, compaction, lifecycle, and recovery tests | Run/Event/Mailbox/Retention semantics and physical compaction/archive details |
| Context, Model Gateway, Tool/MCP, Memory/Research | Contract-faithful fake, seven-gate, manifest, egress, drift, result-reentry, and unknown-outcome tests | Eligibility, disclosure, destination, routing, ToolSpec, capability, and source semantics |
| Trust model | Negative-evidence corpus and scan/assertion obligations | Attack-path catalog, risk treatment, threat boundaries |
| Eval | No-egress read-only and advisory-assessment gates | Eval product semantics, evidence model, later UX/datasets/metrics |
| AI-independent editor-first release baseline | The complete menu of gates relevant to each selected stage | Delivery order, concrete author journey, effective operating defaults, product acceptance, and handoff scope |

No owner may cite this test catalogue to weaken another accepted contract. In
particular, a passing fake result does not grant authority, a passing replay
test does not revive payload, a passing Eval test does not make Eval a control
plane, and a future release stage cannot call a missing required gate
"implemented later" while claiming the covered contract is complete.

## 11. Acceptance and change rules

An implementation may claim the deterministic verification foundation only
when every applicable `DVG-*` family has a Fail-Closed Verification Gate,
replayable safe evidence bundle, named fault-point coverage, and an oracle that
does not depend on real Provider behavior or private production internals.

Changing a semantic contract, profile, protocol release, public error,
manifest, Attempt, fence, retention/replay behavior, fake-destination contract,
or fault boundary requires an explicit review of its affected gate fixtures,
oracle transitions, evidence bundle, and regression corpus. A generated diff
or a property counterexample is evidence of a contract-impacting change, not
noise to be normalized away.

New performance measurements, benchmark datasets, Provider experiments, and
creative-quality evaluation may be added as explicitly advisory work. They
must state their limits and cannot create a hidden score, routing preference,
gate bypass, or unreviewed numerical acceptance threshold. The existing
absolute Protocol Limit Profile and recovery-service commitments retain their
separate accepted evidence requirements.

## 12. Accepted inputs

This specification composes and does not redefine:

- the canonical terms and product invariants in [CONTEXT.md](../../CONTEXT.md)
  and [AGENTS.md](../../AGENTS.md);
- the accepted Protocol, Artifact, Manuscript/Proposal, Persistent Run,
  Subrun/Mailbox, Tool/MCP/Skill, Model Gateway, Memory/Research, Context and
  Disclosure, Eval, PostgreSQL, retention/archival, repository-governance, and
  trust-model contracts linked above; and
- the accepted recovery and project-deletion ADRs plus [ADR 0012](../adr/0012-adopt-deterministic-contract-verification.md).

It records no production implementation, schema, migration, runtime Provider
call, fake server, test harness, deployment action, or CI configuration.
