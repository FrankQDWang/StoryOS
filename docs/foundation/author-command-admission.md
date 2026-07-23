# Author Command Admission

- Status: current
- Canonical issue: [Specify Author Command Admission](https://github.com/FrankQDWang/StoryOS/issues/68)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Trust boundary: [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md)
- Public protocol: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)
- Core state machine: [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md)
- Web editor continuity: [Web Editor Session, Synchronization, and Recovery Semantics](web-editor-session-synchronization-and-recovery-semantics.md)
- Evidence classification owner: [Define the Authoritative-State and Artifact Domain Vocabulary](https://github.com/FrankQDWang/StoryOS/issues/44)
- Decision: [ADR 0013](../adr/0013-trust-the-storyos-web-client-for-author-command-admission.md)

## 1. Purpose and owner

This specification is the sole owner of StoryOS author-command admission. It
defines the durable identity, exact bindings, action class, replay fence,
lifetime, lifecycle evidence, and terminal settlement required before StoryOS
Core may evaluate one author-owned command.

The Manuscript state machine owns command meaning and atomic domain outcomes.
The Web Editor Session contract owns browser-local continuity. The public
protocol owns wire representation. Those contracts consume the admission
defined here.

The domain-vocabulary owner classifies admission issuance, refusal,
reconciliation, and settlement evidence as Operational Records rather than
Artifacts or creative Authoritative State. This specification defines their
admission-specific shape without reopening that cross-domain classification.

## 2. Release 1 trust boundary

Release 1 includes the deployed StoryOS Web Client in the trusted computing
boundary. StoryOS protects that client with controlled deployment, exact
application assets, restrictive browser security policy, authenticated Client
Session Bindings, allowed Host and first-party Origin checks, and
anti-forgery validation.

The StoryOS Server creates an `AuthorCommandAdmissionId` only after it has
derived the authenticated User, validated the exact Project Scope and target,
and bound the request to one canonical command digest. The resulting claim is:

> The trusted StoryOS Web Client submitted this exact digest-covered command,
> for this authenticated User and Project Scope, through this explicit author
> action class, and the StoryOS Server admitted that one command for Core
> evaluation.

This claim does not attest one physical human gesture, browser-event
cardinality, user presence, user verification, or a trusted display of command
semantics. A stronger claim requires a separately trusted confirmation surface
that StoryOS has not selected or simulated for Release 1.

## 3. Exact admission bindings

`AuthorCommandAdmissionId` is the only Release 1 author-command admission
identity. It identifies one immutable issuance record and cannot be supplied by
the Web Client, reused for another command, or replaced by an asserted
physical-human intent identity, an Approval, a Capability Grant, a session
role, or browser-input evidence.

One `AuthorCommandAdmission` binds all of these fields:

- `author_command_admission_id: AuthorCommandAdmissionId`;
- the Server-assigned `command_id`;
- the Server-derived `requester_user_id` and exact
  `ProjectScope { owner_user_id, project_id }`, whose User identities must be
  equal;
- `scope_kind: existing_project | prospective_create_project`;
- the exact opaque Client Session Binding record identity and
  `session_generation`;
- the accepted `client_contract_revision` and `security_policy_revision`;
- `editor_session_id` and the exact Project `writer_generation` when the
  command is editor-bound, otherwise explicit not-applicable values;
- one closed `author_action_class`;
- `api_major`, method, `route_template`, `command_schema`, and exact
  `command_kind`;
- the canonical `command_digest` and `digest_profile`;
- every target identity and expected Head or Revision covered by that digest;
- the pre-domain idempotency-record reference and `idempotency_key`;
- the consumed one-use anti-forgery nonce-record reference, never the nonce
  value; and
- Server-clock `issued_at` and `expires_at`, with
  `issued_at < expires_at`.

The admission transaction atomically claims that exact idempotency record,
consumes its bound nonce, assigns the Command and Admission identities, and
appends the issuance record. A crash before this transaction commits leaves no
admission, nonce consumption, Command, Core effect, or acknowledgement. All
issuance fields are immutable after commit.

Expiry limits when a pending admission may begin its first Core invocation; it
may begin only while Server time satisfies
`issued_at <= now < expires_at`. Expiry never erases the admission, frees its
key or nonce, invalidates a committed Receipt, or changes terminal settlement.
An exact retry after expiry continues to resolve retained idempotency and
settlement evidence without executing another Author Action.

`CreateProject` is the only command admitted before its Project row exists.
The Server allocates a prospective `ProjectId` under the authenticated User's
idempotency record and challenge, then binds the admission to that exact
prospective `ProjectScope`. Core either creates that same scoped Project or
settles without creation. The client never chooses an owner or reuses the
prospective Scope for another command.

The closed Release 1 action classes are:

| Action class | Author-facing source | Examples |
| --- | --- | --- |
| `direct_editor_action` | immediate direct manipulation in the manuscript editor | typing, paste, delete, deterministic structural edit |
| `explicit_editor_command` | an explicit editor control over fully displayed current state | Proposal accept/reject/withdraw, retry or discard a Draft, Author Undo |
| `explicit_project_command` | an explicit project control over fully displayed current scope | export, project settings, Project deletion request |

Agent, Tool, MCP, extension, Worker, Provider, replay, and recovery producers
use their own typed causes. They do not receive an
`AuthorCommandAdmissionId`.

## 4. Lifecycle and terminal settlement

An admission has one append-only lifecycle:

```text
pending -> ReceiptSettled
pending -> RequiresReconfirmation
pending -> outcome_unknown
outcome_unknown -> outcome_unknown
outcome_unknown -> ReceiptSettled
outcome_unknown -> RequiresReconfirmation
```

`pending` means issuance committed and no terminal settlement exists.
`outcome_unknown` is a durable, author-visible, nonterminal recovery condition
used only when StoryOS cannot yet prove whether the admitted Core transition
committed. It records the last provable boundary, reason, observation time, and
`reconciliation_required` disposition. It is never success, refusal, or
permission to invoke, and it may remain visible until authoritative storage
can be validated.

Reconciliation appends evidence to the same admission. It first reads the exact
idempotency record and typed Receipt under the bound Project Scope; it never
uses browser, process, network, timestamp, missing-response, or cache state as
an oracle. Reconciliation may append repeated read-only observations, but it
cannot create another admission or Author Action.

Exactly one terminal settlement may be appended:

```text
AuthorCommandAdmissionSettlement =
  ReceiptSettled {
    receipt_ref,
    project_activity_position,
    settled_at
  }
  | RequiresReconfirmation {
      reason:
        explicit_command_recovery
        | admission_expired
        | binding_changed
        | direct_edit_intent_unrecoverable,
      recovery_draft_ref | null,
      settled_at
    }
```

`ReceiptSettled` is linked atomically with the Core Transition that creates the
typed Receipt. The Receipt owns the exhaustive domain result, including
committed change, refusal, invalidity, conflict, and no effect. A lost HTTP
acknowledgement cannot create a gap between that Receipt and admission
settlement.

`RequiresReconfirmation` proves that this admission will create no Core effect.
It carries no Receipt, cannot later become `ReceiptSettled`, and cannot be
reopened. A later visible author confirmation creates a new idempotency record,
nonce, Command, Admission, and eventual Receipt.

## 5. First invocation and recovery rules

Before a first Core invocation, the Server revalidates exact equality of every
binding that can drift: requester User and Project Scope, Client Session
Binding and generation, accepted client-contract and security-policy
revisions, applicable Editor Session and writer generation, action class,
request contract, canonical digest/profile and every digest-covered field,
targets, expected Heads or Revisions, idempotency record and key, and
nonce-consumption record.

The only automatic recovery invocation is an unsettled
`direct_editor_action` whose same admission is unexpired, whose every binding
still matches, and whose complete Local Edit Journal intent remains visibly
recoverable. It invokes the exact already-admitted command under the same
Command, Admission, idempotency, and nonce-consumption evidence. It does not
mint or upgrade authority and is not a new Author Action.

After validated storage proves that no Receipt exists, recovery must append
`RequiresReconfirmation` instead of invoking Core when:

- the admission expired before recovery invocation;
- the action class is `explicit_editor_command` or
  `explicit_project_command`;
- any binding is changed, stale, missing, or unverifiable; or
- the complete direct-edit intent cannot be proven from the owning Web Editor
  Session boundary.

Visible reconfirmation preserves the complete direct-edit intent as a Recovery
Draft when applicable. The Web Editor Session owner defines journal,
projection, Draft presentation, takeover, and local garbage collection; it
cannot weaken these invocation rules.

## 6. Admission flow

1. The Web Client forms one semantic command from the current visible editor
   or project state.
2. It submits through its browser-held session handle and supplies the Editor
   Session and writer generation when applicable, action class, Project target,
   expected Heads, idempotency key, anti-forgery challenge, and exact typed
   command.
3. The Server resolves the Client Session Binding, derives User and Project
   Scope, validates client and security policy identities, recomputes the
   canonical digest, and validates the nonce, idempotency record, target,
   expected Heads, writer generation, and action class.
4. The Server commits one `AuthorCommandAdmission`, revalidates the invocation
   boundary, and invokes the owning Core command with its identity.
5. Core appends one typed Receipt and atomically links it through
   `ReceiptSettled`, or recovery appends `RequiresReconfirmation` without Core
   execution.
6. Exact duplicates and retries resolve the same pending state or terminal
   settlement. A distinct or reconfirmed command receives a distinct
   idempotency key, nonce, Command, Admission, and Receipt.

Admission permits Core evaluation of one command. Core still determines
authority ownership, Proposal routing, conflict, refusal, no-effect, and
committed domain outcome.

## 7. Positive failure, duplicate, and recovery evidence

- **Pre-admission refusal.** Invalid or unauthorized input creates no
  `AuthorCommandAdmissionId`, Command, nonce consumption, Receipt, or Core
  effect. StoryOS retains a durable, bounded, sanitized refusal record with
  correlation, reached validation boundary, safe typed reason, applicable
  server-derived User and in-Scope Project reference, contract/profile
  identities, and audit time. It retains no rejected body, nonce, session
  handle, foreign identity, or existence-bearing detail.
- **Domain refusal.** Once Core begins the valid first attempt, refusal,
  invalidity, conflict, and no effect are positive typed Receipt results and
  settle the admission as `ReceiptSettled`.
- **Duplicate.** The same scoped command kind, idempotency key, and digest
  resolves one pre-domain idempotency record and at most one admission. A
  concurrent duplicate observes `command_in_progress`; an exact retry observes
  the same lifecycle or terminal settlement. A changed digest conflicts
  without another admission or effect.
- **Expiry.** Expiry is an immutable observed fact. It does not turn a known
  key, nonce, admission, or command into a reusable identity. An expired
  pending admission settles as `RequiresReconfirmation`.
- **Crash and acknowledgement loss.** Before admission commit there is no
  admission effect. After admission commit and before terminal settlement,
  recovery observes the same pending admission and follows section 5. After
  `ReceiptSettled`, exact retry or the settlement Query returns the same
  acknowledgement and Receipt without another Core invocation.
- **OutcomeUnknown.** A timeout, inaccessible authoritative store, process
  death, or missing acknowledgement cannot prove failure or effect absence.
  StoryOS records `outcome_unknown`, blocks blind invocation, and reconciles
  only from ordinary authoritative evidence.
- **Reconciliation.** Receipt found settles `ReceiptSettled`; validated storage
  plus no Receipt follows the direct-versus-explicit rules in section 5.
  Unverifiable evidence remains `outcome_unknown` or terminally requires
  reconfirmation when non-execution is proven. Reconciliation never silently
  repeats an Author Action.
- **Browser continuity.** A pending edit remains author-visible through the Web
  Editor Session contract until settlement and projection convergence, or
  until it becomes a Refused Edit Draft or Recovery Draft.

## 8. Verification

Deterministic verification covers:

- exact existing and prospective User and Project Scope derivation;
- allowed Host and first-party Origin plus exact Client Session Binding and
  generation;
- exact client-contract and security-policy revisions;
- request contract, action class, digest/profile, target, expected-Head,
  nonce-record, idempotency-record, Editor Session, and writer-generation
  binding;
- issued/expiry boundary behavior under a controlled Server clock;
- direct and explicit action classes;
- duplicate, reordered, expired, stale-session, stale-policy, stale-contract,
  stale-Head, and stale-writer requests;
- crash cuts before and after admission, before first Core invocation, during
  Core, after terminal settlement, and before acknowledgement, including the
  direct-versus-explicit and changed-binding branches;
- sanitized non-oracular pre-admission refusal evidence and zero unauthorized
  effect;
- durable `outcome_unknown`, read-only reconciliation evidence, and no blind
  invocation;
- exactly one terminal settlement per admission; and
- for an executed command, one-to-one linkage among admission, command,
  Receipt, and Project Activity.

Passing evidence reports the exact admitted claim and durable settlement. It
never substitutes browser event cardinality, a session role, an Approval, a
Capability Grant, or an Agent/Tool cause for `AuthorCommandAdmission`.
