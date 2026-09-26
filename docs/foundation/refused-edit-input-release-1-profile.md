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
Archive and the existing physical isolated-restore path retain their exact rows.
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
