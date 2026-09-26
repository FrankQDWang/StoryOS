# Refused Edit Input Release 1 Profile

Issue 827 delivers the first public Refused Edit Draft service. Core owns the
ownership decision. The protocol owns the wire shape. Artifact owns retained
Draft content and its lifecycle. Admission owns protected transport and outcome
reconciliation. This profile implements those existing contracts; it does not
deliver the production editor capture or Draft controls from Issue 824.

`storyos.editor-contract.release-1.v3` retains the lawful single-owner primitive
shapes and adds one complete `replace_structured_selection` intent. Its
`ordered_selection` uses `storyos.editor.ordered-source.v1`. Each source names
its exact Manuscript block or Proposal, operation, Revision, and Manuscript
block. It retains the source kind, complete text, coordinate profile, and exact
UTF-16 range. Sources are in projection order. Anchor and head retain selection
direction. Source offsets are never interpreted as manuscript-document offsets.
The replacement retains ordered paragraph and heading blocks and exact text.
This branch has one unit and one primitive. It does not coalesce mixed intents.

Core proves the current Heads, complete ordered source span, exact source text,
scalar boundaries, eligible selected Proposal state, and selected inline
Anchors. A missing, stale, reordered, omitted, or changed source cannot create
a Draft. Current facts include the blocks between the first and last source and
the intersecting unresolved Proposal operations. A remote candidate outside
that span does not change the decision. A selected generating Proposal causes
`Conflicted`; this branch does not pause generation before Admission.

The existing whole public JSON body ceiling is 1 MiB. The existing 240-unit and
240-primitive ceilings are separate and are not a source-count policy. A
complete source string must fit within that body. SQL excludes a larger
candidate string from proof instead of loading a prefix. A selected candidate
is also bounded by the caller's complete declared source byte length. Each
operation's text is read once; further Anchor rows contain metadata only.
Unselected intervening candidates need identity and range facts, not their text.
Thus changed stored candidates cannot multiply a small request into many large
backend text reads. Core still compares each complete selected string. SQL reads at most the
declared source count plus one operation/Anchor row; excess rows make proof
incomplete. Heads use the expected Head count plus one, so a truncated prefix
cannot prove equality. Neither limit is a new tuning default. The current
Authoritative payload table retains its existing 1 MiB SQL byte constraint.
Core indexes Proposal facts by block and borrows source text while it builds
the selected projection.

The Draft payload is `storyos.refused-edit-payload.v1`. It contains the chapter,
expected Heads, targets, complete units and selections, undo group, completed
intent identity, and local intent sequence. It excludes the transport envelope,
client/session bindings, cookies, and anti-forgery nonce. Its canonical JSON
SHA-256 profile is `storyos.refused-edit-payload.jcs.v1`. One atomic settlement
creates a Draft, immutable first Revision, immutable creation event, and exact
Receipt/Admission/command association. It creates no Authoritative or Proposal
Revision, Authoritative Commit, Author Action, or Project Activity. Exact replay
returns the original identities. Creation is an Artifact lifecycle fact and is
not a new Project Activity stream entry.

`getRefusedEditDraft` reads one retained Draft under the authenticated exact
Scope. It returns complete content, digest, original creation source, and current
closure and retention. An archived Project or non-retained Draft is unavailable.
The query has the existing 4 MiB page ceiling. The separate 64 MiB referenced
payload ceiling does not enlarge the inline request or claim a larger supported
Draft. The three Draft tables belong to `artifact-proposal-draft`; Project Export
Archive retains complete eligible and authorized archived rows. A tombstoned
Draft exports its safe Revision metadata and creation/source/lifecycle facts;
it withholds both Revision `payload` and the exact Admission `command_payload`
copy. Each affected entry row has `payload_availability` with the exact entry,
record, field, Draft identity, digest, and `withheld_due_to_tombstone` reason.
A prior Project Export pinned source can contain the same forbidden bytes,
including nested prior pinned sources. The Draft copy check traverses only
known Archive families and exact Scope/Revision/Admission/command associations.
It verifies complete facts digests and uses iterative traversal and indexed
identity lookup. A containing pinned source exports its safe pin identity,
Snapshot, profile, and `facts_sha256`; a third gap type marks its withheld
`facts` and exact restricted Draft identities. All entry gaps enter the
existing root `known_purged_gaps` slot. That slot
name does not claim physical deletion. Creation verifies the original digests
and exact association before withholding. Persist and download derive gaps from
the same pinned entry bytes. Download does not rebuild a historical root from
current lifecycle. The download still checks current Draft copy eligibility:
an old package that contains now-tombstoned input is unavailable. Its original
entries and root remain unchanged. An authorized archived package and a package
with the forbidden content already withheld remain eligible. Missing association,
unknown required copy structure, or digest proof causes a typed failure.
The existing physical isolated-restore path retains exact storage rows.
Restoration does not reopen or revive a closed, archived, or tombstoned record.

The batch policy `storyos.author-edit-batch.release-1.preview.v1` keeps its legacy
250 ms, 240-unit, and 240-primitive selection. Its retained synthetic browser
measurements and their exact [retained policy source](../research/author-edit-batch-prerelease-policy-source.v1.json) cover the legacy primitive workload only. The verifier checks their original digest and compares only the unchanged legacy settings against the current mapping. They do not qualify the
new nested source vector, storage, query, or export/restore workload. Issue 827
uses public service checks for the new shape, including a source vector above
240 items and a request near the inherited whole-body ceiling. No new latency,
throughput, retention, or capacity claim follows from those checks.

The active Server requires the current Editor Contract revision. A v2 request
cannot be silently rewritten or dispatched as v3. Old Local Edit Journal bytes,
payload chains, frozen groups, and outcomes remain retained. A v2 record in an
active partition fails current validation and enters the existing
`local_journal_unavailable` read-only recovery state; validation does not delete
or collect it. It cannot cause automatic replay, fresh challenge issuance, or
base convergence. Recovery of that retained intent needs an explicit compatible
inspection and author action. Issue 824 owns new production mixed capture and
Draft recovery controls.

## Public Discard service

[S3-12b: Discard a Refused Edit Draft Through the Public Core Path](https://github.com/FrankQDWang/StoryOS/issues/825) adds only the abandoned Discard of one exact open retained Refused
Edit Draft. The command uses the existing 1 MiB complete JSON body ceiling.
It carries finite identifiers, the exact Revision and SHA-256 digest, and
accepted Editor Session, exact writer generation, client and security identities. It carries no source
text. The store reads one current Revision only when the Draft is retained.
The existing Draft payload bound limits that integrity check. An unavailable
source returns a settled refusal without loading or returning its payload.

One serializable transaction consumes the exact challenge and commits one
Admission, Receipt, immutable close event, Forward Author Action and closed
projection. No Manuscript or Proposal Head, content Revision, Authoritative
Commit or Project Activity changes. Exact replay returns the original result
and identities. A new command against a closed source is refused; stale exact
Revision or digest conflicts. Response loss is reconciled by an exact replay
with the original key, command bytes and nonce after reload or Server restart.
No new command identity is required for reconciliation.

The close event belongs to `artifact-proposal-draft` and uses
`storyos.artifact-lifecycle.v1`. It binds the source Revision/digest, exact
close Admission/command/Receipt, and Forward sequence. Its bounded metadata
has no source content or authentication secret. The authenticated retained
query adds optional `closure_event` and preserves immutable creation/content.
Eligible exports include close events, Receipts and actions. Tombstone gaps
still withhold source bytes; close metadata never revives them. The existing
physical isolated restore retains the same closed projection and identities.
No new capacity, latency or retention-duration claim is made.

Until [S3-12b2: Undo a Refused Edit Draft Discard](https://github.com/FrankQDWang/StoryOS/issues/831) registers the exact Root Undo handler, this Forward action
is a non-skippable Barrier. [S3-12b1-web: Discard a Refused Edit Draft in the Production Editor](https://github.com/FrankQDWang/StoryOS/issues/832) owns the production controls and explicit
local Discard record. This service exposes neither control nor compensation.
