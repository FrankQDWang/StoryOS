# Author Command Admission

- Status: current
- Canonical issue: [Specify Author Command Admission](https://github.com/FrankQDWang/StoryOS/issues/68)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Trust boundary: [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md)
- Public protocol: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)
- Core state machine: [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md)
- Decision: [ADR 0013](../adr/0013-trust-the-storyos-web-client-for-author-command-admission.md)

## 1. Purpose and owner

This specification is the sole owner of StoryOS author-command admission. It
defines the trusted components, durable identity, exact binding, action class,
replay fence, lifetime, and settlement evidence required before StoryOS Core
may evaluate an author-owned command.

The Manuscript state machine owns command meaning and atomic domain outcomes.
The Web Editor Session contract owns browser-local continuity. The public
protocol owns wire representation. Those contracts consume the admission
defined here.

## 2. Release 1 trust boundary

Release 1 includes the deployed StoryOS Web Client in the trusted computing
boundary. StoryOS protects that client with controlled deployment, exact
application assets, restrictive browser security policy, authenticated Client
Session Bindings, allowed Host and first-party Origin checks, and
anti-forgery validation.

The StoryOS Server creates an `AuthorCommandAdmissionId` only after it has
derived the authenticated User, validated the exact Project Scope and target,
and bound the request to one canonical command digest. The resulting claim is:

> The trusted StoryOS Web Client submitted these exact command bytes, for this
> authenticated User and Project Scope, through this explicit author action
> class, and the StoryOS Server admitted that one command for Core evaluation.

## 3. Durable admission

`AuthorCommandAdmission` is an immutable Project Scope-bound record with:

- `author_command_admission_id: AuthorCommandAdmissionId`;
- the authenticated `user_id` and exact `ProjectScope`;
- `client_session_binding_id` and, for editor work, `editor_session_id` plus
  the exact Project writer generation;
- a closed `action_class`;
- the exact `command_kind`, canonical `command_digest`, target identities, and
  expected Heads;
- the idempotency key and one-use anti-forgery nonce record;
- admission and expiry times from the Server clock;
- the accepted security-policy and client-contract identities; and
- an append-only terminal settlement containing either the resulting typed
  Receipt or `requires_reconfirmation` when recovery proves that an explicit
  command did not commit and must not execute automatically.

Expiry limits when a pending admission may begin its first Core invocation; it
never erases the admission or changes a terminal settlement. An exact retry
continues to resolve retained idempotency evidence after expiry. A pending
direct edit recovered after expiry becomes `requires_reconfirmation` with its
Recovery Draft rather than executing automatically.

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

## 4. Admission flow

1. The Web Client forms one semantic command from the current visible editor
   or project state.
2. It supplies the current Client Session Binding, Editor Session and writer
   generation when applicable, action class, Project target, expected Heads,
   idempotency key, anti-forgery challenge, and canonical command bytes.
3. The Server derives User and Project Scope, validates client and security
   policy identities, recomputes the digest, and validates the nonce,
   idempotency record, target, expected Heads, writer generation, and action
   class.
4. The Server appends one `AuthorCommandAdmission` and invokes the owning Core
   command with its identity.
5. Core appends one typed Receipt and atomically links it to the admission.
6. Exact retries return the same settlement. A distinct command receives a
   distinct idempotency key, nonce, admission, and Receipt.

Admission permits Core evaluation of one command. Core still determines
authority ownership, Proposal routing, conflict, refusal, no-effect, and
committed domain outcome.

## 5. Failure and recovery

- A request rejected before durable admission produces no
  `AuthorCommandAdmissionId` and no Core effect.
- A durable admission without a terminal settlement remains pending
  reconciliation. Recovery first resolves the idempotency record and Core
  Receipt without re-execution.
- When no Receipt exists, an unexpired unsettled `direct_editor_action` may
  resume its exact admitted command automatically because its Local Edit
  Journal entry remains visible and idempotent.
- When no Receipt exists, an `explicit_editor_command` or
  `explicit_project_command` settles as `requires_reconfirmation`; it is never
  executed automatically. A later author confirmation creates a new
  idempotency key, nonce, admission, and Receipt.
- A committed Receipt with a lost HTTP acknowledgement is returned by exact
  retry or query without creating another admission or effect.
- Expired sessions, stale expected Heads, reused nonces, digest mismatches, and
  stale writer generations produce typed refusal or conflict evidence.
- A browser-local pending edit is author-visible through the Web Editor Session
  contract until its admission and Receipt are observed or it becomes a
  Recovery Draft.

## 6. Verification

Deterministic verification covers:

- User and Project Scope derivation;
- allowed Host and first-party Origin;
- exact client-contract and security-policy identity;
- digest, target, expected-Head, nonce, idempotency, Editor Session, and writer
  generation binding;
- direct and explicit action classes;
- duplicate, reordered, expired, stale-session, and stale-writer requests;
- crash cuts before admission, after admission, after Core commit, and before
  terminal settlement, including the direct-versus-explicit recovery branch;
- exactly one terminal settlement per admission; and
- for an executed command, one-to-one linkage among admission, command,
  Receipt, and Project Activity.

Passing evidence reports the exact admitted claim and durable settlement. It
never substitutes browser event cardinality, a session role, an Approval, a
Capability Grant, or an Agent/Tool cause for `AuthorCommandAdmission`.
