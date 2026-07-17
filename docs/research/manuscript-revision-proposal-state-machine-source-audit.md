# Manuscript revision and Proposal state-machine source audit

- Audited: 2026-07-16
- Status: research complete; decision input for [Specify the Manuscript Revision and Proposal State Machine](https://github.com/FrankQDWang/StoryOS/issues/46), not an implementation or domain decision
- Scope: external facts still needed after the accepted StoryOS domain model and the existing Tiptap / ProseMirror research

## Executive conclusion

StoryOS already owns the substantive authority and Proposal semantics. External
sources do not reveal a missing industry state machine that StoryOS should copy.
They instead establish five implementation boundaries that issue 46 should make
explicit:

1. Stable record identities may use RFC 9562 UUIDv7 for locality, but every ID
   remains opaque. UUID order is not the authoritative project order; one
   project-local `AuthoritativeCommit` sequence must be allocated by the same
   atomic Core transition that records the commit.
2. A digest is meaningful only over a versioned canonical byte contract. For
   the closed manuscript and Proposal schemas, SHA-256 over UTF-8 RFC 8785 JCS
   is a practical cross-Rust/TypeScript profile only if StoryOS first enforces
   I-JSON, represents values outside JavaScript's exact integer range as
   strings, rejects invalid Unicode, and preserves prose code points exactly.
   Ordinary JSON serialization is not a digest contract.
3. `compositionstart` and `beforeinput` are synchronous author-intent signals,
   but IME updates are not reliably cancelable and the relevant W3C work is
   still a Working Draft with implementation testing in progress. StoryOS may
   close its in-memory Agent-write gate on these signals, but must not treat
   event cancellation as the correctness boundary.
4. ProseMirror history remains a session-local editor mechanism. At the
   prototype's exact package versions, off-history transactions retain mapping
   information but their content is not undone, native `historyUndo` has no
   target ranges, and Tiptap UniqueID repair still needs StoryOS-owned
   classification. Durable `UndoAcceptance` must therefore remain a new Core
   compensation transition, never a database rollback or editor-history flag.
5. Issue 46 should require one atomic Core transition that appends immutable
   records, advances guarded heads and project order, binds an idempotency key
   to one command digest, updates Proposal operation resolution, and persists
   any outbox intent before publishing success. SQLite can supply an atomic
   transaction boundary, but journal mode, schema layout, `synchronous` policy,
   backup, and migration belong to [Specify the Self-Contained Project Storage and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56).

The remaining uncertainty is not a lack of documentation. Boundary ownership,
same-block Proposal coexistence, structural Proposal reshaping, mixed-ownership
DOM recovery, real IME behavior, and cross-stack undo ordering remain StoryOS
policy or prototype decisions.

## Local baseline and audit method

The audit treats these repository artifacts as settled local authority:

- [`CONTEXT.md`](../../CONTEXT.md), especially `Authoritative Revision`,
  `Authoritative Commit`, `Proposal`, `Proposal Operation`, `Acceptance`, and
  `Undo Acceptance`;
- [ADR 0001](../adr/0001-separate-authoritative-state-artifacts-and-operational-records.md);
- the [Artifact and Authoritative-State Domain Model](../foundation/artifact-domain-model.md);
- [Tiptap / ProseMirror durable Proposal mechanics](./tiptap-prosemirror-proposal-mechanics.md);
- the disposable prototype's [`NOTES.md`](../../prototypes/tiptap-proposal-lab/NOTES.md).

The existing editor research already establishes ProseMirror mapping,
decorations, transient positions, UTF-16/token coordinates, history grouping,
and reconstructible projections. This pass did not repeat that work. It checked
the prototype's exact dependency versions and researched only the remaining
identity, digest, browser-event, atomicity, and durability facts. Sources are
official specifications, official project documentation, and pinned upstream
source/tests. No secondary architecture article or generic event-sourcing
analogy is used.

The prototype currently locks Tiptap `3.27.3`, ProseMirror history `1.5.0`,
state `1.4.4`, transform `1.12.0`, model `1.25.11`, and view `1.42.1` in its
[`package-lock.json`](../../prototypes/tiptap-proposal-lab/package-lock.json).
The corresponding Tiptap tag resolves to commit
[`24d6aba8f46071ec04dd6a6598040d96bbeb68c3`](https://github.com/ueberdosis/tiptap/tree/24d6aba8f46071ec04dd6a6598040d96bbeb68c3),
and ProseMirror history `1.5.0` resolves to commit
[`554d6eedb76704d1db62d85768611ce4b611abd2`](https://github.com/ProseMirror/prosemirror-history/tree/554d6eedb76704d1db62d85768611ce4b611abd2).
The package publishers' official Git metadata resolves ProseMirror model
`1.25.11` to
[`09098e3b00a2e36843040bcde1b7af9adf76816e`](https://code.haverbeke.berlin/prosemirror/prosemirror-model/src/commit/09098e3b00a2e36843040bcde1b7af9adf76816e)
and view `1.42.1` to
[`df890dde77a4f7baed9bd4c5df11cb352e18a6e1`](https://code.haverbeke.berlin/prosemirror/prosemirror-view/src/commit/df890dde77a4f7baed9bd4c5df11cb352e18a6e1).

Each finding below separates:

- **External fact**: what the primary source proves;
- **StoryOS implication**: a design consequence inferred for this project;
- **Already settled**: a local contract that issue 46 must preserve;
- **Still to decide or prove**: policy or prototype work that sources cannot do.

## Source-backed findings

### 1. UUIDv7 is a good opaque identifier, not a commit clock

**External fact.** UUIDv7 places a 48-bit Unix-millisecond timestamp in its
most significant bits and normally fills the remaining non-version/non-variant
bits with randomness. Sub-millisecond fractions or counters are optional ways
to add monotonicity inside one millisecond; they are not inherent in every v7
generator. RFC 9562 also requires implementers to handle same-tick generation,
counter rollover, and clock anomalies rather than knowingly emit duplicates.
([RFC 9562 section 5.7](https://www.rfc-editor.org/rfc/rfc9562.html#section-5.7),
[section 6.2](https://www.rfc-editor.org/rfc/rfc9562.html#section-6.2))

RFC 9562 recommends treating UUIDs as opaque, says UUIDv6/v7 byte order is
sortable for index locality, and explicitly permits additional identifiers for
integrity and feedback. It also warns that UUIDs are not security capabilities
and that UUIDv7 timestamps disclose approximate creation order.
([sections 6.11-6.13](https://www.rfc-editor.org/rfc/rfc9562.html#section-6.11),
[section 8](https://www.rfc-editor.org/rfc/rfc9562.html#section-8))

**StoryOS implication.** Typed UUIDv7 newtypes are a reasonable default for
manuscript object, block, Authoritative Revision, Authoritative Commit,
Proposal, Proposal Revision, Proposal Operation, Receipt, and command
idempotency identities. They improve index locality and remain globally
portable. StoryOS must nevertheless:

- generate them with a CSPRNG-capable, RFC-conforming implementation;
- enforce uniqueness at the Core/store boundary;
- never parse their timestamp to decide freshness, causality, authority, or
  conflict;
- never grant capability merely because a caller knows an ID.

**Already settled.** Artifact, Proposal, operation, and authoritative identities
are stable logical identities, not content hashes. Revision chains are guarded
by an expected prior revision, while `AuthoritativeCommit` supplies a
project-ordered atomic record.

**Still to decide.** Issue 46 should choose whether every named identity above
uses UUIDv7 or whether some local-only records use another opaque type. Whatever
the choice, `AuthoritativeCommit.sequence` must remain a separate project-local
monotonic integer assigned atomically. UUID lexical order is at most a storage
locality hint.

### 2. Canonical bytes must precede every content or command digest

**External fact.** RFC 8785 JCS produces deterministic JSON by constraining the
input to I-JSON, using ECMAScript-compatible primitive serialization, emitting
no insignificant whitespace, preserving array order, and sorting object member
names by UTF-16 code units. JCS requires duplicate-free object names, rejects
lone surrogates and non-finite numbers, and only directly supports numbers
expressible as IEEE-754 binary64.
([RFC 8785 sections 3.1-3.2](https://www.rfc-editor.org/rfc/rfc8785.html#section-3),
[RFC 7493 sections 2.1-2.3](https://www.rfc-editor.org/rfc/rfc7493.html#section-2))

RFC 7493 warns that integers outside
`[-9007199254740991, 9007199254740991]` are not reliably exact in JavaScript and
recommends strings when larger exact numeric values must cross JSON. RFC 8785
is an Informational RFC rather than an IETF Standards Track document, and its
cross-language value comes from implementing its exact rules, not from calling
some JSON output "canonical."
([RFC 7493 section 2.2](https://www.rfc-editor.org/rfc/rfc7493.html#section-2.2),
[RFC 8785 status and terminology](https://www.rfc-editor.org/rfc/rfc8785.html#section-2))

**External fact.** JCS deliberately performs no Unicode normalization; every
participant must preserve string data as-is. Unicode defines canonically
equivalent sequences that can have different binary representations until a
normalization form is deliberately applied. NFC also changes offsets for some
canonically equivalent sequences, and normalization forms are not closed under
simple string concatenation.
([RFC 8785 section 3.1](https://www.rfc-editor.org/rfc/rfc8785.html#section-3.1),
[Unicode Standard Annex #15 sections 1.1-1.4](https://www.unicode.org/reports/tr15/#Introduction))

**External fact.** NIST FIPS 180-4 specifies SHA-256 as a message-digest
algorithm for detecting message changes. A hash does not itself define what
the message bytes mean, nor does an unkeyed digest authenticate an actor.
([FIPS 180-4](https://csrc.nist.gov/pubs/fips/180-4/upd1/final))

**StoryOS implication.** Issue 46 must not specify `hash(JSON.stringify(x))`,
hash rendered prose, or hash a language-runtime object. It needs a named,
versioned digest profile. A practical first profile for the closed manuscript
and Proposal schemas is:

```text
DigestValue {
  algorithm: sha256
  profile: storyos.<purpose>.jcs.v1
  bytes: SHA-256(UTF-8(JCS(profile_input)))
}
```

The canonical `profile_input` should itself include the profile name, schema
version, coordinate version, and the exact typed fields for that purpose. This
domain separation prevents the same serialized bytes from being silently
reused as a manuscript-slice hash, Proposal payload hash, and command digest.
All 64-bit sequences and any other values outside I-JSON's exact integer range
should be encoded as schema-defined decimal strings. Floats, duplicate names,
lone surrogates, unknown schema fields, and non-conforming data should fail
closed before hashing.

For prose, preserve the exact accepted Unicode sequence. Silent NFC/NFKC in the
revision or digest path would change content and can shift the UTF-16/token
coordinates used by ProseMirror anchors. Canonical-equivalence folding may be
useful in a derived search index, but it is not the authoritative payload.

At minimum, issue 46 needs distinct profiles for:

- an immutable manuscript/authoritative payload;
- a Proposal Revision payload and each operation candidate/base slice;
- an anchor base slice, including schema/coordinate versions and structural
  block identity rather than only visible text;
- each idempotent Core command envelope.

**Already settled.** `Content Digest` is integrity/deduplication metadata and
never identity or authority. `AcceptProposal` and `UndoAcceptance` bind an
idempotency key to one exact command digest; exact retries return the prior
Receipt, while reuse with different input fails.

**Still to decide.** JCS is the strongest fit for the existing JSON editor
boundary, but external sources cannot choose it over a separately specified
canonical binary encoding. If issue 46 chooses JCS, the restrictions above are
part of the contract, not implementation notes. The project-wide Artifact
digest profile beyond manuscript and Proposal records may be broader work.

### 3. Browser input events are early signals, not an authority boundary

**External fact.** Input Events Level 2 defines `beforeinput` as synchronous.
For contenteditable hosts it exposes target ranges for most input types, but
`historyUndo` and `historyRedo` always return an empty target-range array. It
marks ordinary typing, paste/drop, deletion, and history undo/redo as
cancelable, but `insertCompositionText` emitted inside an IME composition as
non-cancelable. The document is a W3C Working Draft and says its test suite and
implementation reports remain in progress.
([Input Events Level 2 sections 6.1-6.2](https://www.w3.org/TR/input-events-2/#interface-InputEvent-Attributes),
[document status](https://www.w3.org/TR/input-events-2/#status-of-this-document))

**External fact.** UI Events defines the composition-event order as
`compositionstart`, zero or more `compositionupdate` events, then
`compositionend`. It describes `compositionstart` as synchronous but notes that
it may fire when composition is "about to begin (or has begun)" depending on
the text system. It also says some IMEs cannot honor cancellation. During
composition, `beforeinput` precedes `compositionupdate` and the DOM update, but
most IMEs do not support canceling those updates.
([UI Events sections 3.6.2 and 3.6.6-3.6.7](https://www.w3.org/TR/uievents/#events-compositionevents))

**StoryOS implication.** The editor adapter should synchronously close its
in-memory Agent `RunWriteGate` on the earliest of `compositionstart`,
`beforeinput`, paste, drop, or cut, and then persist a pause/fence through Core.
It should not call `preventDefault()` merely to implement the pause. Correctness
comes from rejecting every later Agent transaction at the gate and from Core
sequence/head checks, not from assuming the browser will cancel IME work.

Because native history events expose no target range, a StoryOS undo coordinator
must choose routing from its own newest reversible durable action plus current
editor-history availability. The raw `historyUndo` event cannot identify an
Acceptance or prove which target the browser intends to change.

**Already settled.** Author input wins over Agent generation; Agent writes stop
before disturbing composition; unsplittable mixed-authority transactions are
refused as a whole; acceptance and compensation remain Core commands.

**Still to decide or prove.** Specifications do not prove real Chinese Pinyin
ordering in StoryOS's supported desktop Chrome environment, nor DOM recovery
after refusing a cross-boundary edit. Other browsers and author-input languages
are outside the product support profile and therefore are not release gates. The
current conservative gate is research-supported, but supported-environment
browser/OS testing remains mandatory.

### 4. Exact Tiptap / ProseMirror versions preserve the known constraints

**External fact.** At ProseMirror model `1.25.11`, non-leaf nodes still use
`content.size + 2`, leaf nodes use one unit, and `TextNode.nodeSize` is still
JavaScript `text.length`. The `1.25.10` and `1.25.11` changelog entries concern
`DOMOutputSpec` and leading/trailing whitespace when parsing a body element;
the audited diff from `1.25.9` to `1.25.11` does not touch `node.ts`,
`fragment.ts`, or `resolvedpos.ts`. Thus the existing schema-token/UTF-16
coordinate and version-relative-position conclusions still hold. The
`parseSlice` whitespace fix may affect a narrow pasted/parsed DOM slice, so it
does not prove mixed-input recovery behavior.
([pinned node-size source](https://code.haverbeke.berlin/prosemirror/prosemirror-model/src/commit/09098e3b00a2e36843040bcde1b7af9adf76816e/src/node.ts#L49-L54),
[pinned TextNode source](https://code.haverbeke.berlin/prosemirror/prosemirror-model/src/commit/09098e3b00a2e36843040bcde1b7af9adf76816e/src/node.ts#L353-L385),
[model changelog](https://code.haverbeke.berlin/prosemirror/prosemirror-model/src/commit/09098e3b00a2e36843040bcde1b7af9adf76816e/CHANGELOG.md#L1-L17))

**External fact.** At ProseMirror view `1.42.1`, the composition handlers still
flush pending DOM observations on composition start, maintain explicit
composition state/IDs, and defer a pending post-`compositionend` flush to a
microtask. Its DOM observer still contains browser-specific composition repair
and converts observed mutations back through the document-change handler. The
audited diff from `1.41.9` to `1.42.1` does not touch `input.ts`,
`domobserver.ts`, `domchange.ts`, or the composition tests, including the tests
where overlapping document changes cancel simulated composition and changes
elsewhere do not. The releases do change clipboard style parsing, mark-view
updates, and scroll-coordinate handling. Therefore the prior composition and
DOM-observer mechanism findings remain source-valid, while paste shape and real
browser DOM recovery must be tested at `1.42.1`; this audit makes no broader
"no material drift" claim.
([pinned composition source](https://code.haverbeke.berlin/prosemirror/prosemirror-view/src/commit/df890dde77a4f7baed9bd4c5df11cb352e18a6e1/src/input.ts#L517-L573),
[pinned DOM observer](https://code.haverbeke.berlin/prosemirror/prosemirror-view/src/commit/df890dde77a4f7baed9bd4c5df11cb352e18a6e1/src/domobserver.ts#L45-L85),
[DOM-change handoff](https://code.haverbeke.berlin/prosemirror/prosemirror-view/src/commit/df890dde77a4f7baed9bd4c5df11cb352e18a6e1/src/domobserver.ts#L174-L253),
[pinned simulated-composition tests](https://code.haverbeke.berlin/prosemirror/prosemirror-view/src/commit/df890dde77a4f7baed9bd4c5df11cb352e18a6e1/test/webtest-composition.ts#L238-L270),
[view changelog](https://code.haverbeke.berlin/prosemirror/prosemirror-view/src/commit/df890dde77a4f7baed9bd4c5df11cb352e18a6e1/CHANGELOG.md#L1-L24))

**External fact.** At Tiptap `3.27.3` commit
`24d6aba8f46071ec04dd6a6598040d96bbeb68c3`, UniqueID initial generation and a
special initialization path explicitly set `addToHistory: false`. Ordinary
changed-range repair ends by tagging `__uniqueIDTransaction` without the same
off-history flag, while paste handling strips copied IDs so new ones can be
generated. This source proves mechanics, not StoryOS semantic identity for
split/join/move operations.
([pinned UniqueID source](https://github.com/ueberdosis/tiptap/blob/24d6aba8f46071ec04dd6a6598040d96bbeb68c3/packages/extension-unique-id/src/unique-id.ts#L137-L170),
[repair and paste paths](https://github.com/ueberdosis/tiptap/blob/24d6aba8f46071ec04dd6a6598040d96bbeb68c3/packages/extension-unique-id/src/unique-id.ts#L226-L426))

**External fact.** At ProseMirror history `1.5.0`, a transaction with
`addToHistory: false` adds its maps to history branches but not its content.
The upstream test confirms that undo removes tracked text while preserving
off-history text. The same plugin intercepts native `beforeinput` values
`historyUndo` and `historyRedo`. Its state field provides `init` and `apply`,
not a durable serialization contract.
([pinned history source](https://github.com/ProseMirror/prosemirror-history/blob/554d6eedb76704d1db62d85768611ce4b611abd2/src/history.ts#L258-L297),
[upstream selective-undo test](https://github.com/ProseMirror/prosemirror-history/blob/554d6eedb76704d1db62d85768611ce4b611abd2/test/test-history.ts#L94-L125),
[native undo handler](https://github.com/ProseMirror/prosemirror-history/blob/554d6eedb76704d1db62d85768611ce4b611abd2/src/history.ts#L384-L420))

Appended transactions run inside `EditorState.applyTransaction`; they are not
new calls through an application's outer dispatch wrapper. The exact
ProseMirror state `1.4.4` source therefore confirms the existing research's
warning that StoryOS must classify or own UniqueID maintenance rather than
assuming outer dispatch metadata is inherited.
([pinned state source](https://github.com/ProseMirror/prosemirror-state/blob/d6fdcd19c4f7f68206b0a8d49649860365672585/src/state.ts#L132-L167))

**StoryOS implication.** The specifically audited position, composition,
DOM-observer, history, and UniqueID findings remain valid at the lockfile's
exact versions. Production code must still pin and test the complete dependency
set. Agent stream batches stay outside ProseMirror history; exact candidate
restoration appends a Proposal Revision; and every ordinary undo/redo or ID
repair transaction crosses the same ownership classifier before persistence.

**Already settled.** Tiptap is a reconstructible review projection. Stable
block IDs live in schema attributes, durable anchors live in Proposal records,
and integer positions are never durable identities.

**Still to decide or prove.** UniqueID behavior cannot decide whether a split
preserves the left or right block's semantic operation identity, whether two
same-block ranges may coexist, or how a moved/multi-block operation reshapes.
Those are domain and prototype decisions.

### 5. SQLite proves the atomic boundary, not the StoryOS storage design

**External fact.** SQLite documents that all changes within one transaction
occur completely or not at all, including interruption by process crash,
operating-system crash, or power failure, and describes its transactions as
serializable. It also permits only one simultaneous write transaction.
([SQLite Is Transactional](https://www.sqlite.org/transactional.html),
[Transaction section 2.1](https://www.sqlite.org/lang_transaction.html#read_transactions_versus_write_transactions))

The durability guarantee depends on the selected journal and `synchronous`
configuration. SQLite explicitly says, for example, that WAL with
`synchronous=NORMAL` can lose a committed transaction after power loss even
though it remains atomic, consistent, and isolated. Rollback-journal and WAL
atomic commits use different mechanisms.
([SQLite `synchronous`](https://www.sqlite.org/pragma.html#pragma_synchronous),
[Atomic Commit introduction](https://www.sqlite.org/atomiccommit.html#introduction))

**StoryOS implication.** Issue 46 can require an abstract atomic Core
transition with this all-or-nothing logical write set:

```text
validate expected heads, exact Proposal revision, command digest, and key
append immutable authoritative / Proposal revisions as applicable
append AuthoritativeCommit with the next project sequence when authority changes
append immutable Domain Receipt and operation/lifecycle events
advance normalized current heads and projections
persist outbox / wakeup intent required by the transition
commit once
publish success only after commit
```

A duplicate idempotency key must be resolved inside that boundary: the exact
same command digest returns the existing Receipt; a different digest fails.
The digest detects command mismatch, while a uniqueness constraint and the
transaction prevent duplicate effects. A hash alone is not idempotency.

**Already settled.** Local project data is authoritative; operational records
and transitions are durable and inspectable; external effects follow persisted
intent; acceptance selections are atomic; recovery never treats a model or
network process as truth.

**Still to decide.** Issue 46 owns the logical transaction contract and crash
states visible to the domain. Issue 56 owns SQLite tables, file boundaries,
journal mode, `synchronous` level, savepoints, indexes, content-addressed blobs,
backups, restore, migration, and portability. This audit intentionally makes no
choice among them.

### 6. Committed undo is compensation, not rollback

**External fact.** SQLite `ROLLBACK` applies to an active uncommitted database
transaction. Once a domain transition has committed and later authoritative
work may depend on it, storage rollback is not a semantic undo mechanism.
([SQLite transaction control](https://www.sqlite.org/lang_transaction.html))

**StoryOS implication.** No external state-machine or event-sourcing pattern is
needed to justify `UndoAcceptance`. StoryOS's own authority invariant determines
the rule: undo is a new author-authorized Core command against expected current
heads. It appends compensation when the exact safe linear head still holds;
otherwise it creates a derived or Reversal Proposal and preserves later work.
The original Acceptance Receipt and revisions remain immutable.

**Already settled.** There is no history deletion, mutable accepted status, or
durable `RedoAccept`. Reapplication is a newly validated `AcceptProposal` with
a new idempotency key.

**Still to decide or prove.** Issue 46 must make the safe-head and overlap
predicate exact. The prototype must still establish whether and when keyboard
or native history undo routes to `UndoAcceptance` ahead of ordinary editor
history without making older author history inaccessible.

## Reconciliation with the StoryOS contract

| Issue 46 dimension | Settled local contract | Externally proven constraint | Remaining StoryOS work |
|---|---|---|---|
| Identity | Stable logical IDs; revisions are immutable linear histories | UUIDv7 is sortable but only optionally monotonic inside a millisecond; IDs should be opaque | Choose typed ID representation and generation/uniqueness rules |
| Project order | `AuthoritativeCommit` gives one project order | UUID timestamps and lexical order are not a reliable domain sequence | Define atomic allocation and overflow/error semantics for `sequence` |
| Revision concurrency | Every write carries an expected prior revision | One atomic transaction can compare/update heads and append records | Specify exact conflict outcomes and retry/readback rules |
| Hashes | Digests are integrity, not identity; command key binds one digest | JCS requires I-JSON, exact number/string rules, and no Unicode normalization | Choose and version canonical profiles and exact field sets |
| Proposal state | Orthogonal generation, validation, closure, and per-operation resolution axes | No external source can choose domain states | Specify legal transitions, command guards, and derived projections without adding `accepted`, `rejected`, or `stale` |
| Anchors | Stable block identity plus versioned ProseMirror-token/UTF-16 offsets; no durable absolute positions | Mapping can detect deleted sides; current UniqueID repairs IDs but not semantic ownership | Specify exact anchor/hash payload, boundary association, split/join/move rules, and conflict projection |
| Author input | Direct author manipulation may write authority; Proposal edits append Proposal Revisions | `beforeinput` is synchronous; IME composition updates are not reliably cancelable | Define whole-transaction classifier and prove recovery in real browsers |
| Acceptance | Exact eligible revision and selected pending operations apply atomically | A transactional store can make the logical write set all-or-nothing | Specify full command envelope, revalidation order, Receipt result union, and no-op behavior |
| Undo | Safe compensation appends records; conflicting work produces a Reversal Proposal | Editor history is selective/session-local; native history events have no target ranges | Define newest-reversible ordering, safe-head/overlap predicate, and routing |
| Recovery | Durable records and exact heads are truth; editor/plugin state is reconstructed | Transaction atomicity supplies before/after crash boundary; storage settings affect power-loss durability | Specify replay/fence reconciliation; defer physical storage and durability settings to issue 56 |

## Decision inputs for issue 46

The following boundary set is narrow enough to decide now and strong enough for
the later storage and implementation tickets:

1. **Opaque typed identities.** Prefer RFC 9562 UUIDv7 newtypes for every
   durable entity/record ID, generated with a conforming CSPRNG implementation
   and protected by uniqueness constraints. IDs carry no authority or order.
2. **Independent ordering.** Allocate one project-local unsigned
   `AuthoritativeCommit.sequence` inside the authority-changing Core transition.
   Revision causality comes from explicit parent/expected-head fields, not ID
   sorting or wall-clock time.
3. **Immutable revision envelopes.** Every Authoritative Revision and Proposal
   Revision binds stable object ID, revision ID, parent ID, schema version,
   creator/cause, exact payload digest, and exact prior/head preconditions.
   Alternative histories derive a new identity rather than branching one chain.
4. **Versioned digest profiles.** Prefer `sha256 + JCS` for the closed
   manuscript/Proposal command boundary only with enforced I-JSON, decimal
   strings for wide integers, exact-as-stored Unicode, explicit profile/schema/
   coordinate versions, and cross-language golden vectors. If these constraints
   are rejected, specify another canonical byte format before naming a hash.
5. **One logical atomic transition.** Validation, immutable appends, head
   changes, project sequence, Proposal resolutions, Receipts, lifecycle events,
   and outbox intent commit together. Duplicate command handling occurs inside
   this boundary. Physical SQLite design remains deferred.
6. **No state collapse.** Preserve the four local axes and derive completion
   from operation resolutions. A content revision resets validation; current
   eligibility always binds the exact Proposal Revision and target heads.
7. **Fail-closed anchors and conflicts.** Durable anchors name structural block
   IDs, schema/coordinate version, block-relative range, exact target revision,
   and canonical base-slice digest. Missing/duplicate IDs, schema drift, deleted
   anchors, hash mismatch, overlap, or changed targets disable Acceptance and
   project `conflicted`; no silent rebase.
8. **Separate editor and domain undo.** ProseMirror undo/redo remains
   session-local and every resulting transaction is reclassified/persisted.
   `UndoAcceptance` is a new idempotent Core transition with exact Receipt and
   expected-head guards; no durable redo status exists.
9. **Reconstructible recovery.** Persist domain facts and stream/pause fences,
   not decorations, NodeViews, DOM, or document-wide integer positions. On
   ambiguous crash windows, expose a recovery conflict instead of replaying or
   guessing.

This is a research recommendation, not the resolution of issue 46. The HITL
session still owns names, exact field types, transition tables, error/result
unions, and policy choices.

## What external research cannot decide

- Whether typing exactly at an inline Proposal boundary belongs inside or
  outside the Proposal.
- Whether two disjoint inline Proposals may coexist in one block or whether a
  pending Proposal reserves the whole block.
- Which block retains semantic operation identity after split, join, move,
  block-type change, or multi-block reshaping.
- Whether a structurally edited operation may retain its ID after explicit
  replan, and the precise semantic-identity test for doing so.
- The user-facing recovery interaction for a refused mixed-ownership paste,
  drop, cut, delete, or composition.
- The exact cross-stack ordering among `UndoAcceptance`, Proposal candidate
  history, Direct Author Action history, and remaining ProseMirror history.
- Whether JCS or a canonical binary format is the long-term project-wide digest
  representation; sources only establish the constraints each choice must meet.
- SQLite schema, journaling, `synchronous` mode, blob layout, migration,
  portability, backup, and restore; those are issue 56 decisions.
- Retention policy, tombstone payload boundaries, or how long editor history is
  kept. They arise from StoryOS product/privacy policy, not these mechanisms.

## Remaining prototype gates

The contract pass now covers exclusive Proposal boundaries, whole refusal and
Draft recovery for keyboard and native clipboard edits, Safe Mode, unified
Acceptance undo/redo, Block ID structural behavior, and the settled
same-block/structural policies at the pinned package versions. The author also
manually verified real Chinese Pinyin input in the contract probe on supported
desktop Chrome on 2026-07-17; deterministic tests cover the composition fence
and Proposal ownership classification. On the same date, real Chrome `Mod-v`
and `Mod-x` over the mixed `｜提` selection emitted `native_paste` and
`native_cut`, left the authoritative document unchanged, preserved the refused
result as a Draft, and—on cut—placed the actual `｜提` selection on the
clipboard. The author then manually dragged the mixed `｜提` selection across
the ownership boundary in real desktop Chrome: the browser emitted
`native_dragstart` and `native_drop`, the contract emitted
`refused_edit_draft`, the authoritative document remained unchanged, and the
attempted moved result was preserved as a Draft.

The browser/editor contract evidence required by issue #45 is complete. The
remaining system-level prototype gate is outside that ticket:

- crash-window reconciliation across a real Core transaction, durable stream
  event, pause fence, Proposal Revision, and editor projection checkpoint. This
  is intentionally deferred until issues #46 and #56 define and implement the
  required Core and storage boundaries.

Synthetic events and unit tests remain useful regressions, but they cannot
replace real browser/OS IME evidence or a real durable Core boundary.
