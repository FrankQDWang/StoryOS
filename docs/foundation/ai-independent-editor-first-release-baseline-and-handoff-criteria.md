# AI-Independent Editor-First Release Baseline and Handoff Criteria

- Status: current
- Canonical issue: [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62)
- Product goal: [GOAL.md](../../GOAL.md)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Web editor session: [Web Editor Session, Synchronization, and Recovery Semantics](web-editor-session-synchronization-and-recovery-semantics.md)
- Core editor semantics: [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md)
- Public protocol: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)
- PostgreSQL: [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md)
- Retention and recovery visibility: [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md)
- Trust boundary: [StoryOS Service, Client, and External Trust Boundaries](storyos-service-client-external-trust-boundaries-threat-model.md)
- Verification: [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md)

## 1. Release destination

The first author-usable StoryOS release is a high-quality novel editor that
remains fully useful when every model, Agent, Tool, MCP server, research
service, and network-dependent AI capability is unavailable.

An author can begin from a new or controlled Project initialization, organize a
novel, write and revise manually, understand save state, recover safely,
navigate and search, inspect basic progress, and export readable prose. This
baseline establishes the editor as the primary product before adjacent AI
assistance enters delivery.

## 2. Required author-visible capabilities

### 2.1 Project and manuscript organization

- Create, open, rename, and archive a Project.
- Create, rename, reorder, and remove volumes and chapters through typed
  author commands.
- Navigate the manuscript tree without losing pending editor work.
- Open a chapter from a bounded current Snapshot and display its save state.

### 2.2 Manual writing

- Type, paste, cut, delete, replace selections, split, join, move, and retype
  supported manuscript blocks.
- Support desktop Chrome with Chinese and English input, including complete IME
  composition, keyboard navigation, clipboard, and author-visible undo.
- Render input immediately through the Pending Edit Projection.
- Preserve every unsettled intent in the Project Scope-bound IndexedDB Local
  Edit Journal.
- Keep StoryOS Core and PostgreSQL as the authoritative state and Receipt
  source.

### 2.3 Save and recovery

- Show `saving`, `saved`, and `needs_attention` from durable settlement and
  projection evidence.
- Recover after reload, Web Client process crash, Server restart, lost HTTP
  acknowledgement, duplicated or reordered Project Activity, and replay-floor
  resynchronization.
- Preserve complete Recovery Drafts and Refused Edit Drafts with copy, retry,
  narrow, and discard actions.
- Permit one active Project writer generation and explicit takeover from a
  second tab while preserving the prior tab's unsettled text.

### 2.4 Find, inspect, and export

- Navigate by Project, volume, and chapter.
- Search the current chapter and full manuscript with bounded results.
- Replace one current visible match through a direct typed author command.
  Replacing an explicitly selected multi-match or cross-location result set
  creates an inspectable Core Proposal whose exact targets require author
  Acceptance.
- Show basic word, character, chapter, and manuscript progress statistics.
- Export a human-readable manuscript with deterministic volume/chapter order
  and explicit representation of currently unavailable content.
- Produce the versioned portable Project Export Archive owned by the PostgreSQL
  and retention contracts. Disaster recovery remains a separate verified
  Recovery Copy or PITR path.

### 2.5 Long-session quality

- Maintain responsive local input during network delay.
- Bound cold open, chapter switch, Snapshot, replay, search, journal, Revision,
  Receipt, Event, index, backup, and restore growth against the representative
  writing-path evidence owned by [Measure the Representative Writing-Path Performance and Storage-Growth Envelope](https://github.com/FrankQDWang/StoryOS/issues/76).
- Preserve author work through an extended session, repeated chapter switches,
  reload, and controlled application upgrade.

### 2.6 Ordinary defaults and production boundaries

- The manual writing path requires no model or Provider configuration, role
  table, PIN, routing policy, Skill installation, context pin, retention
  profile, Eval setup, or Agent availability.
- Initial use runs the StoryOS Server and PostgreSQL locally for one
  bootstrapped User. The later controlled-cloud deployment uses the same
  User, Project Scope, command, isolation, migration, and recovery contracts.
- Every persisted Project fact and operation binds exact
  `{ owner_user_id, project_id }` scope and is protected by composite-scope
  integrity plus forced RLS under a non-owner runtime role.
- Production-shaped delivery may not substitute in-memory state, browser
  session state, a test Adapter, a prototype, `.reference/**`, or a local file
  convention for StoryOS Core and PostgreSQL authority.
- Account management, billing, teams, collaboration, ownership transfer, and
  multi-author editing are outside this release. A required outline,
  character-sheet setup, professional workflow, Agent-generated plan, MCP App,
  Skill, Memory, research service, embedding, or Eval step is also outside the
  manual baseline.

## 3. Complete acceptance journey

The release gate executes this journey with all AI destinations disabled:

1. Bootstrap the single local User and create a new Project.
2. Create a volume and three chapters, reorder them, and reopen the Project.
3. Write Chinese and English prose using IME, clipboard, block operations, and
   local undo while observing immediate pending and durable saved states.
4. Introduce network delay and exercise HTTP-before-SSE, SSE-before-HTTP,
   duplicate Event, reconnect, and Snapshot/resync paths.
5. Crash and reload the Web Client with saved and unsettled edits present.
6. Open a second tab, observe read-only state, perform explicit takeover, and
   recover the prior tab's pending text.
7. Search the manuscript, replace selected results, navigate among chapters,
   and verify statistics.
8. Export a human-readable manuscript and a versioned Project archive.
9. Restart Server and PostgreSQL, restore from the required Recovery Copy in an
   isolated verification environment, validate roles and forced RLS, rebuild
   disposable projections, prove lifecycle non-revival, and withhold ordinary
   visibility until a Recovery Visibility Proof succeeds.
10. Continue writing with the restored or original verified Project.

The journey passes only when every acknowledged author command has exactly one
Author Command Admission, Core settlement, Receipt, and resulting Project
Activity observation; every unsettled edit remains visible and recoverable;
cross-Scope, stale-session, hostile-origin, oversize, secret-bearing, and
unrecoverable inputs fail closed with their required negative evidence.

## 4. Canonical ownership

This specification owns the author-visible baseline and delivery order. It
consumes these owners:

| Concern | Sole owner |
| --- | --- |
| author command admission | Author Command Admission |
| Core edit, Proposal, conflict, Receipt, and undo meaning | Manuscript Revision and Proposal State Machine |
| browser session, IndexedDB journal, pending projection, writer takeover, and resync | Web Editor Session contract |
| route inventory, DTOs, errors, SSE, generated client, and same-release identity | Versioned Protocol |
| PostgreSQL schema, migration, backup, restore, and archive mechanics | PostgreSQL contract |
| retention, compaction, replay generations, and deletion | Run Event, Mailbox, Snapshot, Retention, and Archival contract |
| deterministic fixtures, fault points, and evidence bundles | Deterministic Verification contract |
| measured latency, scale, and growth envelope | [Measure the Representative Writing-Path Performance and Storage-Growth Envelope](https://github.com/FrankQDWang/StoryOS/issues/76), with accepted values adopted by each semantic owner and the Experimental Tuning Register |

The public route inventory is generated exclusively from the Rust contract
source owned by the Versioned Protocol. This specification names required
capabilities and author journeys without maintaining another route list.

## 5. Delivery stages

Production delivery proceeds in this order:

1. **Production-shaped manual-editor risk slice.** Existing controlled Project
   data exercises browser input, Local Edit Journal, Author Command Admission,
   Core settlement, PostgreSQL durability, Project Activity, reload, and crash
   recovery. It also proves exact Host/Origin/session admission, Project Scope,
   forced-RLS runtime, hard input bounds, and absence of credential or
   cross-Scope leakage.
2. **Complete AI-independent editor baseline.** Deliver every capability and
   acceptance step in sections 2 and 3.
3. **Contract-faithful fake-model Proposal loop.** Add the adjacent Agent,
   editable in-editor Proposal, validation, Acceptance, Rejection, refusal,
   conflict, and recovery using deterministic fake destinations. Rejection is
   a complete non-destructive author decision and leaves Authoritative State
   unchanged.
4. **One real external model path.** Add one Provider-neutral route with exact
   Context Assembly, destination identity, disclosure, Attempt, usage, and
   uncertainty evidence.
5. **Controlled-cloud release gate.** Validate deployment, identity,
   operational recovery, security, cache refresh, same-release activation, and
   upgrade evidence for the author's controlled domain and server.

Each stage begins from the resulting `main` of the preceding stage.

The stage-3 author journey starts from a current passage in an existing
Project. The author asks the adjacent Agent for bounded help; one durable root
AgentRun uses a contract-faithful fake through the same Context Assembly,
manifest, Model Gateway, Attempt, fence, recovery, and Proposal path required
of a real Adapter. The result must be an editable, anchored Proposal in the
editor. A chat reply is not a substitute, and neither the fake nor the Agent
may write authoritative prose, invent a required outline, or bypass the
author's explicit Acceptance or Rejection. Rejection is a successful,
non-destructive completion. Tool, MCP, Skill, research, embedding, Memory,
Subrun, and Eval execution are absent and any attempted Tool-request mode fails
closed.

Stage 4 keeps that same path and adds one separately admitted external-model
operation. A crash, timeout, or disconnect after dispatch claim remains
`OutcomeUnknown`; late results are fenced, blind resend is forbidden, and any
reconciliation or successor is separately admitted. Release evidence includes
a real-author session that edits, accepts, and rejects Proposals and recovers
Run, Proposal, disclosure, and authoritative facts without claiming Provider
internals or literary quality.

## 6. Issue-native handoff

Planning closure and a product release are distinct gates:

| Gate | Passing evidence | Failure effect |
| --- | --- | --- |
| planning closure and implementation handoff | every planning issue and tracked contract closes on one `main`; the terminal task creates and locks the first stage-1 implementation issue | no implementation issue is created and no product code begins |
| stage implementation and release | the issue's applicable deterministic gates actually run; evidence bundles are replayable; the stage's author journey and measured operating envelope pass | any failed, unrun, unreplayable, evidence-incomplete, or `unverified` required gate blocks that stage's release |

The selected verification closure is:

| Stage | Required deterministic gate families |
| --- | --- |
| manual-editor risk slice | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-07`, `DVG-11`, and the editor portion of `DVG-13` |
| complete AI-independent editor | the preceding set plus `DVG-09`, `DVG-10`, the complete manual acceptance journey, and the accepted performance/storage envelope |
| fake-model Proposal | `DVG-01` through `DVG-11`, plus the editor and fake-destination walks in `DVG-13`; Eval-only `DVG-12` remains absent |
| one real external model | the same deterministic fake-based closure plus the separately labelled real-author and real-destination evidence described in stage 4 |
| controlled cloud | every applicable non-Eval gate, migration/restore/security evidence, and the complete same-release activation journey |

After all planning issues close, the map's terminal task creates one
implementation issue for stage 1. It selects the smallest coherent crate,
route, migration, generated-artifact, and UI subset from the final current
contracts without maintaining another route inventory. Its body contains:

- Contract revision and exact `Baseline: main@<commit>`;
- stable Requirement IDs and exact authoritative inputs;
- Goal, scope, owning modules, and end-to-end data flow;
- author journey and acceptance criteria;
- red tests and deterministic fault points;
- migrations and generated artifacts;
- targeted checks and final non-mutating repository verification;
- pull-request and merge gates.

The implementation issue is the map's sole open implementation child. A later
stage issue is created only after its predecessor is merged, verified, resolved,
and closed.

## 7. Planning completion gate

Planning is complete when:

- Author Command Admission, Core edit semantics, Web Editor Session,
  PostgreSQL, public protocol, retention, performance, and deterministic
  verification form one coherent data flow;
- the complete AI-independent acceptance journey maps every requirement to one
  owner and one evidence gate;
- all planning issues are closed at the same current `main`; and
- the terminal task creates and locks the stage-1 implementation issue.
