# Tiptap / ProseMirror durable Proposal mechanics

Research ticket: [Establish Durable Proposal Mechanics in Tiptap and ProseMirror](https://github.com/FrankQDWang/StoryOS/issues/42)

## Scope and source baseline

This note answers whether the settled StoryOS interaction contract can be built on Tiptap and ProseMirror without sacrificing author editing, IME correctness, position mapping, crash recovery, or deterministic operation-level Acceptance and Rejection. It is architecture research, not a production implementation. Its domain terms follow the accepted [Artifact and Authoritative-State Domain Model](../foundation/artifact-domain-model.md); older imported grilling language such as “accepted Proposal,” `UndoAccept`, and `CompoundProposal` is normalized below rather than carried into the implementation contract.

Only first-party material was used:

- current official ProseMirror and Tiptap documentation;
- `ProseMirror/prosemirror-model` at `6264de069d8439131e88f8ba06973551916184e4`;
- `ProseMirror/prosemirror-state` at `ffad5d9450a0b93438be53a801deee1a223a81bf`;
- `ProseMirror/prosemirror-history` at `445409bc99c88550c2312f5610829ecb25105a5f`;
- `ProseMirror/prosemirror-view` at `ca4c78e9b56f1b164c0b3758b59d8748f11b7534`;
- `ueberdosis/tiptap` at `ecc0d4035820e0d19a3ca758ed2f337821a09dbf`.

The implementation plan must later pin actual package versions and rerun the proposed prototype against those exact versions.

## Conclusion

The settled Proposal experience is feasible, but only with a deliberate separation of four concerns:

1. **Authoritative State:** StoryOS Core-owned prose represented by immutable Authoritative Revisions and project-ordered Authoritative Commits. Tiptap is never this source of truth.
2. **Proposal Artifact history:** immutable Proposal Revisions containing stable Proposal Operations, exact authoritative targets and base versions, current editable candidate content, workflow axes, provenance, and recovery fences.
3. **Editor review projection:** one normal, schema-valid ProseMirror document that presents current authoritative prose with unresolved Proposal candidates substituted into their review locations. It is a reconstructible editing surface, not a fourth durable authority space.
4. **Editor plugin projection:** a keyed ProseMirror plugin holding transient mapped ranges, reservations, and a derived `DecorationSet`; durable Domain Receipts, Artifact Lifecycle Events, Run Events, and authoritative records remain StoryOS-owned Operational Records outside the plugin.

Proposal identity and Diff markup should **not** be encoded as Proposal wrapper nodes or marks in the manuscript. ProseMirror decorations can change rendering without changing the document, while NodeViews are designed around actual document nodes. This makes decorations a direct fit for inline Diff ranges, block highlighting, and widget controls, whereas a NodeView cannot uniformly represent both an arbitrary inline range and several ordinary top-level blocks. [ProseMirror decorations](https://prosemirror.net/docs/ref/#view.Decoration) [ProseMirror NodeView](https://prosemirror.net/docs/ref/#view.NodeView)

Two mechanisms participate in undo but neither is independently authoritative:

- ProseMirror history supplies session-local inverse editor transactions for the author's typing and candidate editing. StoryOS must classify and persist the result of every undo/redo transaction as either a Direct Author Action or a new Proposal Revision.
- StoryOS Core handles durable `UndoAcceptance` from an immutable Acceptance Receipt. Safe compensation creates Authoritative Revisions, an Authoritative Commit, and—when permitted—a new Proposal Revision with reopened pending operations; otherwise it produces a derived or Reversal Proposal. Re-applying later is a new validated `AcceptProposal`, not a `RedoAccept` status toggle.

The separation is necessary because off-history Agent batches are deliberately preserved by ProseMirror selective undo, while Acceptance of already-visible candidate bytes may create no document step at all even though it creates authoritative records. The highest-risk claims are therefore boundary behavior across multiple APIs. A disposable browser prototype must prove Core/editor undo routing, real IME interruption, block-ID policy under structural edits, mixed-ownership author transactions, and reload recovery before the foundation design is considered complete.

## Settled interaction contract carried into this research

- The current Proposal candidate is ordinary editable content in the editor review projection. Editing it appends a Proposal Revision and resets Core validation; it is not a Direct Author Action against prose Authoritative State.
- Diff is a derived view of the operation's exact base slice versus the current Proposal Revision, including author edits.
- Inline Proposal scope is one continuous range within one textblock.
- Block Proposal scope is one or more consecutive complete top-level blocks.
- A multi-block `BlockEditProposal` resolves by stable top-level Proposal Operations; the historical `CompoundProposal` label does not create another Artifact kind, and character/word hunks are never persistence identities.
- A chapter may contain multiple non-overlapping pending Proposals; overlap never silently rebases or merges.
- Author input immediately pauses streaming, gives the author the sole writer role, and preserves the applied partial result with generation `ready_partial`.
- `AcceptProposal` selects pending operations on one exact eligible Proposal Revision. `RejectProposalOperations` rejects selected pending operations without changing Authoritative State.
- `UndoAcceptance` preserves the original Acceptance Receipt, compensates only when safe, and reopens content through a new Proposal Revision rather than rewriting history.
- Autosave and reload preserve Authoritative Revisions and Commits, Proposal Revisions and all four workflow axes, operation resolutions, Receipts, and Run evidence. There is no top-level `accepted`, `rejected`, or `stale` Proposal status; operation resolution is `pending | applied | rejected`, while an invalidated target projects validation as `conflicted`.

## What the upstream mechanisms prove

### Editor state and plugin state

ProseMirror state is immutable: transactions produce new state, and normal editor updates pass through transaction dispatch. A plugin may own a keyed state field whose `apply` function receives every applied transaction, the previous field value, and the old/new editor states. Transactions can carry arbitrary metadata addressed by string, plugin, or `PluginKey`. These are the right primitives for a deterministic editor-side Proposal projection. [ProseMirror guide: transactions and plugins](https://prosemirror.net/docs/guide/#state) [Plugin state fields](https://prosemirror.net/docs/ref/#state.StateField) [Transaction metadata](https://prosemirror.net/docs/ref/#state.Transaction.setMeta)

Tiptap officially supports installing ProseMirror plugins through `addProseMirrorPlugins()` and, in its current Extension API, wrapping dispatch so an extension can inspect and add metadata before forwarding a transaction. StoryOS can therefore implement one Tiptap extension while keeping its core state machine in ProseMirror primitives. [Tiptap Extension API](https://tiptap.dev/docs/editor/extensions/custom-extensions/create-new/extension)

`filterTransaction` can reject a transaction before it is applied, while `appendTransaction` can deterministically append another transaction after a batch. An appender may be called again when another plugin appends work, so it is unsuitable for network calls or domain side effects. StoryOS should reserve it for deterministic editor maintenance such as ID repair. [Plugin transaction hooks](https://prosemirror.net/docs/ref/#state.PluginSpec.filterTransaction)

### Mapping and transient positions

Every transform step provides a `StepMap`; a transaction's `Mapping` is the ordered pipeline of those maps. `mapResult` reports not only a new position but whether content on either side or across the position was deleted. This supports mapping live Proposal envelopes through every transaction and recognizing when an anchor has been destroyed rather than guessing from integer positions. [ProseMirror position mapping](https://prosemirror.net/docs/ref/#transform.Mappable) [Mapping pipeline](https://prosemirror.net/docs/ref/#transform.Mapping)

ProseMirror document nodes are immutable values and do not have stable object identity or parent links. Absolute positions are also version-relative. Durable identity must therefore come from explicit node attributes and Proposal records, never node object identity or a cached integer `pos`. [ProseMirror guide: identity and persistence](https://prosemirror.net/docs/guide/#doc)

ProseMirror positions use its schema-dependent token coordinate system: entering/leaving a non-leaf node counts as one token, an inline leaf such as an image or hard break counts as one, and text contributes its JavaScript string length. In the pinned model source, `TextNode.nodeSize` is `this.text.length`, so supplementary Unicode characters occupy two UTF-16 code units; marks are metadata and add no position units. Rust byte offsets, Unicode scalar counts, and grapheme-cluster counts are therefore not interchangeable with a ProseMirror offset. [ProseMirror indexing guide](https://prosemirror.net/docs/guide/#doc.indexing) [Pinned `nodeSize` implementation](https://github.com/ProseMirror/prosemirror-model/blob/6264de069d8439131e88f8ba06973551916184e4/src/node.ts#L49-L54) [Pinned `TextNode.nodeSize`](https://github.com/ProseMirror/prosemirror-model/blob/6264de069d8439131e88f8ba06973551916184e4/src/node.ts#L367-L385)

### Decorations and NodeViews

Decorations influence how a document is drawn without modifying the document. ProseMirror provides:

- inline decorations for an arbitrary inline range;
- node decorations for exactly one document node;
- widget decorations at a position for controls or status UI;
- `DecorationSet.map()` to move a decoration set through a transaction mapping, with a callback when a decoration is dropped. [Decoration API](https://prosemirror.net/docs/ref/#view.Decoration) [DecorationSet mapping](https://prosemirror.net/docs/ref/#view.DecorationSet.map)

Inline decorations expose `inclusiveStart` and `inclusiveEnd`, so typing at a boundary has an explicit rendering policy. This does not by itself settle StoryOS's *domain* ownership policy at an ambiguous boundary; the persisted range mapping must use matching association rules and the prototype must validate them. [Inline decoration boundaries](https://prosemirror.net/docs/ref/#view.Decoration.inline)

NodeViews provide custom rendering and behavior for an actual node. Their `getPos()` may return `undefined` once the node leaves the document, and custom views must correctly handle DOM mutations and selection behavior. They are useful for true embedded node types, not as the primary identity mechanism for a range-based Proposal. [NodeView constructor and lifecycle](https://prosemirror.net/docs/ref/#view.NodeViewConstructor)

### History and off-history streaming

The ProseMirror history plugin is selective rather than a snapshot rollback mechanism. A transaction tagged `addToHistory: false` is not itself undone, but the history branch still stores its position maps so older inverse steps can be remapped onto the current document. This behavior is explicit in the source and is exercised by upstream tests. [History source, map-only items](https://github.com/ProseMirror/prosemirror-history/blob/445409bc99c88550c2312f5610829ecb25105a5f/src/history.ts#L5-L19) [History source, off-history mappings](https://github.com/ProseMirror/prosemirror-history/blob/445409bc99c88550c2312f5610829ecb25105a5f/src/history.ts#L258-L297) [Upstream off-history tests](https://github.com/ProseMirror/prosemirror-history/blob/445409bc99c88550c2312f5610829ecb25105a5f/test/test-history.ts#L94-L125)

This proves that streamed transactions may be excluded from the undo stack without making earlier history blind to their positional effects. It also proves an important negative: map-only items do not merge later off-history content into an earlier tracked event. In the upstream test, undoing tracked `hello` deliberately leaves the off-history `oops!` content in place. Therefore the earlier hypothesis “track the first batch, mark later batches off-history, then one ProseMirror undo removes the full candidate” is unsupported and likely false for normal append-at-boundary streaming. StoryOS must not adopt it without contrary prototype evidence. [Selective undo preserves off-history content](https://github.com/ProseMirror/prosemirror-history/blob/445409bc99c88550c2312f5610829ecb25105a5f/test/test-history.ts#L94-L100)

`closeHistory(tr)` starts a history boundary. The source processes that flag before returning for a no-step transaction, so a metadata-only transaction can close the prior event. The history implementation groups later tracked transactions by time, adjacency, and composition metadata. [History boundary source](https://github.com/ProseMirror/prosemirror-history/blob/445409bc99c88550c2312f5610829ecb25105a5f/src/history.ts#L258-L286) [closeHistory implementation](https://github.com/ProseMirror/prosemirror-history/blob/445409bc99c88550c2312f5610829ecb25105a5f/src/history.ts#L365-L390)

A transaction with no steps is not added as an undo event. The history plugin also owns a `beforeinput` handler for native `historyUndo` and `historyRedo`. Thus `UndoAcceptance` must coordinate with keymap and `beforeinput` ordering rather than hoping a status-only editor transaction represents an Acceptance in ProseMirror history. [No-step history behavior](https://github.com/ProseMirror/prosemirror-history/blob/445409bc99c88550c2312f5610829ecb25105a5f/src/history.ts#L258-L269) [Native undo handler](https://github.com/ProseMirror/prosemirror-history/blob/445409bc99c88550c2312f5610829ecb25105a5f/src/history.ts#L391-L420)

The upstream history plugin state defines only `init` and `apply`, not `toJSON` / `fromJSON`. ProseMirror can serialize plugin fields only when the plugin explicitly supplies serialization hooks. Therefore the normal editing undo stack is session-local unless StoryOS deliberately builds another persisted step log; Proposal Artifact history, Acceptance Receipts, and `UndoAcceptance` must not depend on browser history surviving reload. [History plugin state](https://github.com/ProseMirror/prosemirror-history/blob/445409bc99c88550c2312f5610829ecb25105a5f/src/history.ts#L391-L420) [EditorState plugin serialization](https://prosemirror.net/docs/ref/#state.EditorState.toJSON)

### IME and composition

`EditorView.composing` is officially exposed and is true during an active composition. `handleDOMEvents` handlers run before ProseMirror's own handler; returning false lets normal input continue. This lets StoryOS synchronously mark a run as locally paused at `compositionstart` without swallowing the user's input. [EditorView.composing](https://prosemirror.net/docs/ref/#view.EditorView.composing) [DOM event ordering](https://prosemirror.net/docs/ref/#view.EditorProps.handleDOMEvents)

ProseMirror's own view source flushes pending DOM observation at composition start, tracks a composition ID, and may defer the final DOM flush until a microtask after `compositionend`. It also includes browser-specific handling for Safari, Firefox, Android, and Windows Chrome. StoryOS must not replace this machinery with React input state. [Composition source](https://github.com/ProseMirror/prosemirror-view/blob/ca4c78e9b56f1b164c0b3758b59d8748f11b7534/src/input.ts#L435-L523)

Upstream tests using simulated DOM composition events demonstrate that a dispatched document change overlapping the simulated composition cancels it, while a change elsewhere can remain compatible. They do not prove behavior with a real OS IME. StoryOS's stronger settled rule—pause all Agent document writes as soon as the author starts input—is technically conservative, but real Chinese/Japanese IME verification remains a hard prototype gate. [Simulated composition overlap tests](https://github.com/ProseMirror/prosemirror-view/blob/ca4c78e9b56f1b164c0b3758b59d8748f11b7534/test/webtest-composition.ts#L238-L270)

### Stable node IDs

Tiptap's official `UniqueID` extension adds an ID attribute to configured node types and documents intended behavior across split, merge, undo/redo, crop, and paste. IDs become part of the document JSON rather than relying on object identity. [Tiptap UniqueID](https://tiptap.dev/docs/editor/extensions/functionality/uniqueid)

The current implementation adds schema attributes, excludes initial ID generation from history, and uses `appendTransaction` for later changed-range maintenance. It repairs editing-related duplicates detected in changed ranges, strips IDs from pasted copies, and distinguishes internal drag sources. A critical pinned-source detail is that the ordinary changed-range repair tags `__uniqueIDTransaction` but does **not** set `addToHistory: false`; only initial generation and a special branch shown earlier in the source explicitly do so. Appended transactions are produced inside `EditorState.applyTransaction` from one root transaction, so they are not new calls through Tiptap's outer `dispatchTransaction` middleware. StoryOS must therefore own/adapt the ID-maintenance plugin or explicitly classify its appended metadata; it cannot assume dispatch middleware will tag it or that stock repair is off-history. The source also does **not** prove arbitrary preloaded duplicates or custom-generator collisions impossible. StoryOS must validate global uniqueness during hydration and before Core validation/Acceptance, and define semantic Proposal ownership when a block is split or joined. [UniqueID attributes and initial generation](https://github.com/ueberdosis/tiptap/blob/ecc0d4035820e0d19a3ca758ed2f337821a09dbf/packages/extension-unique-id/src/unique-id.ts#L69-L184) [UniqueID changed-range handling](https://github.com/ueberdosis/tiptap/blob/ecc0d4035820e0d19a3ca758ed2f337821a09dbf/packages/extension-unique-id/src/unique-id.ts#L275-L371) [UniqueID paste and drag handling](https://github.com/ueberdosis/tiptap/blob/ecc0d4035820e0d19a3ca758ed2f337821a09dbf/packages/extension-unique-id/src/unique-id.ts#L374-L451) [ProseMirror appended-transaction loop](https://github.com/ProseMirror/prosemirror-state/blob/ffad5d9450a0b93438be53a801deee1a223a81bf/src/state.ts#L132-L167)

### Persistence and reload

Tiptap recommends storing JSON and restores content either at editor construction or with `setContent`. `editor.getJSON()` persists document content; it is not a durable Proposal domain store. ProseMirror can serialize selected plugin fields, but the Proposal registry should remain a StoryOS contract rather than a browser-plugin serialization detail. [Tiptap persistence](https://tiptap.dev/docs/editor/core-concepts/persistence) [ProseMirror plugin serialization](https://prosemirror.net/docs/ref/#state.StateField.toJSON)

Tiptap distinguishes `update`, which follows content changes, from `transaction`, which follows any editor state transaction. A Proposal workflow transition or history boundary can change without document content changing, so autosave and domain persistence cannot rely only on `onUpdate`. [Tiptap events](https://tiptap.dev/docs/editor/api/events)

## Recommended StoryOS mechanism

### 1. Persist immutable Proposal Revisions, not an editor status object

The editor consumes the canonical Proposal Artifact contract. A prose-edit payload needs fields conceptually shaped like the following; the later contract ticket owns exact Rust and wire types:

```text
ProposalArtifact
  artifact_id
  kind: InlineEditProposal | BlockEditProposal
  retention: retained | archived | tombstoned

ProposalRevision
  artifact_revision_id
  parent_revision_id?
  operations: ProposalOperation[]
  run_provenance
  last_applied_stream_seq
  admitted_through_stream_seq
  pause_fence_hash

ProposalWorkflowProjection (derived from durable lifecycle events)
  exact_artifact_revision_id
  generation: generating | ready_partial | ready
  validation: pending | valid | invalid | conflicted
  closure: open | withdrawn | superseded

ProposalOperation
  operation_id
  resolution_projection: pending | applied | rejected
  target_block_ids
  base_authoritative_revision_refs
  preconditions
  base_slice_json
  base_slice_hash
  current_candidate_slice_json
  current_candidate_hash
  inline_anchor? { block_id, from_offset, to_offset, boundary_assoc }
```

The immutable Proposal Revision stores content and provenance. Generation, validation, operation resolution, and closure are orthogonal durable lifecycle projections tied to an exact revision; a status-only transition records an Artifact Lifecycle Event rather than mutating or duplicating that revision. Retention remains the independent common Artifact axis.

The exact persisted schema belongs to later domain design, but these invariants are required:

- base content is pinned to exact Authoritative Revisions and is never reconstructed from a visual Diff;
- an absolute ProseMirror position is never a durable anchor;
- editing candidate bytes appends a new immutable Proposal Revision under `expected_revision_id`, retains operation IDs only when target and semantic identity are unchanged, and resets validation to `pending`;
- the exact Proposal Revision and selected pending operation IDs must match the editor review projection by hash before `AcceptProposal` can pass Core validation;
- each top-level block ID is unique in the authoritative chapter and editor review projection;
- unresolved Proposals satisfy a non-overlap invariant;
- a deleted, duplicated, or unresolvable target projects validation as `conflicted` rather than creating a `stale` status or triggering an implicit rebase;
- applied and rejected operation incarnations remain frozen against the exact Proposal Revision that resolved them.

An inline anchor must explicitly declare `coordinate_space = prosemirror-token-offset-v1`, the schema version/digest, top-level `block_id`, block-relative `from`/`to`, boundary associations, exact base Authoritative Revision, and base-slice hash. Offsets count ProseMirror content tokens: text by UTF-16 code units, inline leaf atoms and `hard_break` as one, and marks as zero. Rust must validate/convert this coordinate space explicitly; it must never reinterpret the integers as UTF-8 byte, Unicode-scalar, or grapheme offsets. A schema change that alters node sizes invalidates or migrates the anchor under an explicit versioned contract.

Autosave must append the exact Proposal Revision no later than the first checkpoint that acknowledges the corresponding editor transaction. Streaming may coalesce several admitted deltas into one Proposal Revision, but the revision must identify the closed sequence interval and expected parent so duplicate or missing batches are detectable. Author typing inside an open pending operation appends an author-created Proposal Revision; author typing outside all Proposal reservations follows the Direct Author Action path and creates Authoritative Revisions plus an Authoritative Commit. A transaction crossing those ownership boundaries is a prototype gate, not a license to guess or silently promote candidate text.

### 2. Store candidate text as ordinary content in a non-authoritative review projection

Do not add a Proposal mark or wrapper node merely to carry identity. Replace or insert the candidate as normal schema-valid content in the editor review projection, and use exact Proposal Revisions plus the editor plugin to distinguish candidate content from current authoritative prose. The projection may be checkpointed as a recovery cache, but it cannot become an alternate authoritative manuscript.

This yields one representation for both Proposal kinds:

- an `InlineEditProposal` is a mapped range inside one identified textblock;
- a `BlockEditProposal` is an ordered set of identified top-level blocks;
- a pending continuation has an empty original slice and a current inserted range or created block IDs.

Before the first non-empty stream batch, render streaming status with a widget decoration anchored to the durable insertion point. A zero-width range has ambiguous boundary mapping, so it should not be the only identity of a pending Proposal.

### 3. Use one keyed Proposal plugin plus an adapter-owned runtime gate

The immutable plugin state should contain only values derivable from the editor review projection and exact durable Proposal Revisions:

```text
ProposalPluginState
  live_ranges_by_proposal_id
  reservation_index
  decoration_set
  hydration_generation
```

The synchronous `compositionstart` pause latch cannot live only in plugin state because changing immutable plugin state requires dispatching a transaction—the very boundary that must already be closed. The StoryOS editor adapter therefore owns a small mutable runtime gate, outside React state and outside the ProseMirror plugin state, keyed by Run/Proposal identity. DOM handlers close this gate synchronously before returning; every Agent-origin dispatch checks it before touching ProseMirror. The durable pause fence remains in StoryOS Core and is reconciled back into Proposal/Run records.

For every transaction, its pure `apply` path should:

1. map live endpoints and unaffected decorations through `tr.mapping`;
2. inspect `mapResult` deletion flags;
3. consume typed StoryOS transaction metadata;
4. determine which Proposal envelopes intersect changed ranges;
5. re-extract and re-hash affected candidate slices;
6. recompute Diff hunks and decorations for affected Proposals;
7. flag destroyed or ambiguous targets for application reconciliation into validation `conflicted`.

Mapping old Diff decorations is insufficient when Proposal text itself changes: the old hunk semantics are no longer valid. Map the broad envelope, then recompute Diff from the exact operation `base_slice_json` versus the candidate slice extracted for the new Proposal Revision. Unaffected Proposal decorations may simply be mapped.

Suggested transaction metadata:

```text
storyos.origin: direct_author | proposal_author_edit | agent_stream | core_projection | hydration | id_maintenance
storyos.proposal:
  action: create | stream_batch | pause | apply_operations | reject_operations | reopen_operations
  proposal_id
  proposal_revision_id?
  operation_ids?
  run_id?
  stream_seq?
```

All StoryOS-generated document transactions must carry an origin. Current Tiptap dispatch middleware can add metadata before forwarding. Raw input transactions are classified against the reservation index before persistence; they are not all assumed to be Direct Author Actions. Because appended transactions bypass that outer wrapper, the recommended StoryOS-owned/adapted block-ID extension must set both `storyos.origin = id_maintenance` and `addToHistory = false` inside its own `appendTransaction`; stock `__uniqueIDTransaction` remains an explicit compatibility signal, not an assumed StoryOS contract.

Do not perform HTTP, persistence, or other side effects inside `StateField.apply` or `appendTransaction`. Emit application work after the new editor state is established, through the StoryOS editor adapter, with document and Proposal versions for idempotency.

#### Deterministic routing for author transactions

Before applying a root author transaction, the editor adapter must calculate its changed old-document spans and classify them against the reservation index:

1. If every changed span is outside every Proposal reservation, route the transaction as one Direct Author Action against exact authoritative targets.
2. If every changed span is inside the same pending Proposal Operation, route it as one author-created Proposal Revision under the exact expected Proposal head; validation returns to `pending`.
3. If a StoryOS-issued typed editor command declares several independent subcommands up front, Core may atomically persist the authoritative and Proposal results together once the storage contract supports that unit.
4. For the first slice, an arbitrary ProseMirror transaction with a span crossing a Proposal boundary, touching multiple Proposal Operations, or mixing authoritative and Proposal ownership is refused as a whole before application. It is never heuristically split, partially persisted, or promoted to Authoritative State. The UI must preserve enough attempted input to retry, explain the boundary, and offer an explicit operation such as narrowing the selection; IME cursor insertion at an exact boundary follows the settled boundary association.

This conservative rule is preferable to silently converting a cross-boundary edit into a larger Proposal or Direct Author Action. Root-transaction preflight belongs in the adapter/dispatch boundary, not in a side-effecting plugin `apply`. The prototype must prove DOM reconciliation for refused paste, drop, cut, deletion, and composition-generated transactions; if refusal cannot preserve real IME correctness, the foundation must choose and specify an explicit boundary-expansion command before implementation.

### 4. Render Diff and controls entirely with decorations

- Use inline decorations for replacement/addition hunks.
- Use node decorations for whole-block state or block-level selection.
- Use widget decorations for workflow state, operation-level Accept/Reject actions, and the on-demand original/Diff affordance.
- Put `proposalId`, exact `proposalRevisionId`, `operationId`, visual kind, and a stable widget key in decoration specs.
- Keep deletion text outside editable current prose; show it in the expanded original/Diff view.

`inclusiveStart` / `inclusiveEnd` and the corresponding mapping associations must be defined together. An insertion at the same numeric position can mean “edit inside the Proposal” or “write immediately outside it”; the prototype must capture the actual selection context and establish one deterministic rule.

### 5. Resolve every stream batch from identity, never a cached position

For each durable `proposal.delta`:

1. discard duplicates using `(proposal_id, stream_seq)`;
2. resolve the current anchor from block IDs and the plugin's latest mapped range;
3. verify the expected Proposal Revision, generation state, review-projection version, and non-overlap reservation;
4. locate the actual textblock through a resolved position;
5. apply one batched transaction tagged `agent_stream`;
6. update the applied sequence and schedule persistence only after the transaction succeeds.

Network cancellation is not instantaneous. Once local author input is detected, later arriving deltas may remain in the durable Run event stream but must not be dispatched into the document. The Proposal head ends at the last successfully applied `stream_seq`, and its generation projection becomes `ready_partial`.

The pause command must carry the client's last applied sequence, current Proposal Revision/hash, and expected review-projection version. Appending the exact partial candidate revision, recording its generation lifecycle event as `ready_partial`, and fencing generation at the admitted-through sequence must be one durable unit. Deltas beyond the fence remain Run evidence only; they are never eligible for editor replay.

### 6. Keep Agent generation out of ProseMirror history and reverse it through Artifact revisions

Do not use the rejected first-tracked/later-off-history hypothesis as the undo mechanism. The default design candidate for the prototype is:

1. Create the durable Proposal and display a widget; do not create a fake empty text node.
2. At Proposal activation, close the preceding ProseMirror history event with a metadata-only `closeHistory` transaction when no composition is active.
3. Apply **all** Agent stream batches with `addToHistory: false`, explicit Proposal metadata, and sequence/version preconditions.
4. Preserve every durable candidate state as an immutable Proposal Revision linked to its exact base Authoritative Revisions and admitted stream sequence range.
5. When streaming completes or pauses, freeze the local admitted sequence and persist the corresponding Proposal Revision no later than the first author-edited checkpoint; typing remains immediate, but later Core commands wait for that checkpoint acknowledgement.
6. Reverting the generated candidate appends a new Proposal Revision that restores an earlier candidate/base slice under expected-revision and target preconditions, or withdraws the Proposal when that is the author's intent. It never rewrites Artifact history or Authoritative State. Reapplying likewise appends a new Proposal Revision and requires revalidation; exact command names belong to the Proposal contract ticket.
7. Treat the author's later candidate edits as normal tracked ProseMirror transactions that also append author-created Proposal Revisions. Treat edits outside reservations as Direct Author Actions that create authoritative records.

This candidate avoids pretending that ProseMirror grouped the stream, but it introduces a Core/editor undo coordinator. The prototype must prove that artifact-restoration and authoritative-compensation transactions map older ProseMirror history safely, persist the right durable objects, and keep action ordering intuitive after author edits. A custom Proposal-aware history plugin is a fallback investigation, not the default implementation.

### 7. Pause before IME content is disturbed

Install `handleDOMEvents` hooks for at least `compositionstart`, `beforeinput`, `paste`, `drop`, and `cut`, while allowing ProseMirror to continue its default input handling.

At the first author-edit signal while a Proposal is streaming:

1. synchronously close the editor adapter's mutable runtime gate for the affected Run/Proposal before any ProseMirror dispatch;
2. make every Agent-origin dispatch check that gate and refuse document mutation after it closes;
3. send an idempotent asynchronous pause command containing `last_applied_stream_seq`, exact Proposal Revision/hash, and expected projection version;
4. add `closeHistory` to the first resulting author transaction;
5. append the author-created `ready_partial` Proposal Revision through expected-revision checking;
6. never auto-resume at `compositionend`—resume only by a later explicit author command.

Any dispatch middleware must withhold `agent_stream` document changes whenever `view.composing` or the adapter runtime gate is closed. The gate is synchronously mutable by design; plugin state and React state are too late to prevent the first racing dispatch. Late payloads may remain in the durable Run event stream, but must not sit in an editor-transaction queue that auto-applies after composition. Do not dispatch a document-changing “pause transaction” during composition. ProseMirror remains responsible for the composition DOM and its final flush.

### 8. Coordinate ProseMirror history with operation commands and `UndoAcceptance`

Before `AcceptProposal`, the adapter must flush or await every earlier editor checkpoint. Core then atomically verifies that the exact Proposal Revision is retained, generation `ready`, validation `valid` for the current targets, closure `open`, and that every selected operation is still `pending`; it also checks the current Validation Receipt, target versions, permission, idempotency key, candidate hashes, and projection checkpoint reference. Success creates the selected Authoritative Revisions, one Authoritative Commit, immutable operation-resolution events, and an Acceptance Receipt. The Proposal Artifact is not promoted or marked accepted. Since the same candidate bytes may already be visible in the review projection, the successful projection update may contain no content step even though authority changed.

`RejectProposalOperations` similarly names one exact Proposal Revision, selected pending operation IDs, expected target versions, expected Proposal head, and an idempotency key. It does not change Authoritative State. On success it freezes those rejected operation incarnations against that revision, then the editor projection replaces only their candidate ranges with the current authoritative target content:

- for an InlineEditProposal operation, replace only its exact reserved range;
- for a BlockEditProposal, replace only the selected operation's addressed top-level block or blocks;
- for a multi-operation BlockEditProposal, leave every unselected pending operation and its decorations intact.

The adapter must never perform the visual replacement before durable success. Uncheckpointed author input, a changed Proposal head/hash, changed targets, or a cross-boundary transaction makes the command fail rather than overwrite the author's text. Reopening rejection appends a Proposal Revision; it retains an operation ID only when target and semantic identity are unchanged, resets validation to `pending`, and leaves the rejected historical incarnation immutable.

`UndoAcceptance` names the immutable Acceptance Receipt plus expected current target versions, expected Proposal head when applicable, and a new idempotency key. A safe linear head produces compensating Authoritative Revisions, an Authoritative Commit, an Undo Acceptance Receipt, and a new Proposal Revision reopening the operations as pending. An incompatible Proposal head produces a new derived Proposal; overlapping later author changes produce a Reversal Proposal; neither case overwrites later work. There is no durable `RedoAccept`: a later re-application is a fresh Core validation and `AcceptProposal` against the new exact revision with a new idempotency key.

The editor therefore needs a coordinator, but not an independent domain action ledger. Durable ordering comes from Authoritative Commits, Artifact Revisions, and Receipts; ProseMirror history remains session-local. When Acceptance is the newest reversible user-visible action, high-priority `Mod-z` and native `beforeinput historyUndo` may route to `UndoAcceptance` before ProseMirror history. Ordinary ProseMirror undo/redo transactions must still pass through ownership classification and persist their inverse as either a Direct Author Action or Proposal Revision. Cross-stack ordering is a prototype gate.

### 9. Treat multi-block review units as stable Proposal Operations

The historical `CompoundProposal` interaction maps to one multi-operation BlockEditProposal, not another Artifact type and not a Proposal Bundle. Each stable operation owns exact base Authoritative Revision references, addressed block IDs, base content, and current candidate content rather than a visual hunk. `AcceptProposal` may select any pending operations, with the selected set applied atomically. `RejectProposalOperations` freezes only the selected operations and restores only their editor projection from Authoritative State. The Proposal is fully resolved only when no operation remains pending; that state is derived, not a top-level status.

The first production slice should preserve one stable top-level block identity per operation. If author candidate editing splits, joins, changes block type, or makes one operation span multiple current blocks, the edit may be checkpointed as a new Proposal Revision but must project validation as `conflicted` until an explicit replan either preserves semantic identity or replaces the operation with a new ID. Research does not prove which reshaping interaction feels correct; the prototype must surface this decision.

### 10. Enforce non-overlap without blocking the author

Maintain the same reservation model in the durable domain and the editor plugin:

- a block Proposal reserves its addressed blocks;
- an inline Proposal reserves a range within an identified block;
- creation of an Agent Proposal that intersects an existing reservation is rejected before dispatch;
- an Agent stream transaction may touch only its own reservation;
- ordinary author input outside reservations remains a Direct Author Action; cross-boundary input follows the explicit whole-transaction refusal rule and is never silently discarded or partially persisted.

If an author candidate edit wholly inside one operation splits, joins, moves, or deletes its anchor, preserve the author-created Proposal Revision but project validation as `conflicted`, pause affected Runs, and require explicit replan. A Direct Author Action elsewhere that invalidates an exact target likewise makes the affected Proposal conflicted. Never silently rebase it.

Whether two disjoint inline Proposals in the same block are allowed is still underspecified. Reserving the whole block is safer and simpler; interval-level coexistence is more flexible but makes boundary edits and block restructuring harder. The prototype should compare these policies before the domain contract is finalized.

### 11. Rehydrate from durable state, not serialized decorations

Durable autosave must persist a crash-consistent unit containing at least:

- exact Authoritative Revision heads and Authoritative Commit sequence;
- exact Proposal Revision heads, four workflow axes, and operation resolutions;
- candidate/base hashes, versioned durable anchors, and provenance;
- last applied stream sequence plus any admitted-through/pause fence for every streaming or partial Proposal.

The editor review JSON may be checkpointed with exact authoritative and Proposal head references for fast recovery, but remains a reconstructible cache. Workflow or operation-resolution changes do not necessarily fire a content update, so persistence must be driven by Core commands and classified editor transactions, not only Tiptap `onUpdate`.

On reload:

1. load Authoritative State from exact current Authoritative Revisions;
2. load retained open Proposal heads and overlay only pending operations into the review projection, or validate a saved projection checkpoint against those exact heads;
3. validate schema version, unique block IDs, coordinate spaces, anchors, hashes, target versions, and non-overlap;
4. replay later stream events only when the exact Proposal head is generation `generating`, no pause fence excludes them, and sequence/hash/version preconditions hold;
5. hydrate plugin state and recompute Decorations from exact base and current candidate slices;
6. expose invalid records as validation `conflicted` recovery items and disable Acceptance.

Generation `ready_partial` must never consume later Run deltas. If a crash occurs between the runtime gate closing and the durable pause fence, recovery must compare the projection checkpoint, exact Proposal Revision/hash, and sequence bounds and require explicit reconciliation rather than replaying blindly. Do not persist `DecorationSet`, NodeViews, DOM, or document-wide integer positions. The normal ProseMirror keystroke undo stack will not survive reload unless StoryOS explicitly persists steps; Artifact Revisions, authoritative history, Receipts, and operation outcomes survive independently.

## Proven mechanisms, remaining risks, and required evidence

| Area | Proven by first-party material | StoryOS-specific risk |
|---|---|---|
| Plugin state | Keyed state fields receive every transaction and can consume metadata | Purity and application-side effect ordering must be enforced |
| Position mapping | StepMap/Mapping/mapResult map positions and expose deletion | Boundary association and structural edits can change ownership semantics |
| Durable offsets | ProseMirror defines schema-token positions; pinned text size uses JS string length | Rust must implement the versioned UTF-16/token coordinate contract, not byte or scalar offsets |
| Diff rendering | Decorations are view-only and can be mapped | Changed Proposal content requires Diff recomputation, not mapping alone |
| Controls | Widget decorations can host position-bound DOM | Event, focus, selection, and IME behavior need browser tests |
| Node identity | Explicit attrs persist; Tiptap UniqueID maintains IDs along documented edit paths | Ordinary repair is not off-history and bypasses outer dispatch middleware; StoryOS must own/adapt and validate it |
| Transaction ownership | Root transactions and changed spans are inspectable before apply | Cross-boundary and mixed authoritative/Proposal transactions require explicit atomic routing or whole refusal |
| Streaming undo | Off-history changes remain present while their maps rebase older history | Exact candidate restore requires a new Proposal Revision and cross-stack coordinator |
| History boundaries | `closeHistory` and grouping rules are defined | First author input must close the Agent event without disturbing composition |
| Undo Acceptance | Core compensation and Receipts persist independently | Safe-head/overlap routing and ordering against ProseMirror undo/redo need a coordinator |
| IME | `view.composing` exists; simulated overlap tests cancel composition | Real browser/OS IME ordering and cancellation latency require manual testing |
| Autosave | JSON content can be saved/restored; plugin serialization is optional | Browser/server crash windows and a durable admission/pause fence need a concrete protocol |
| Multiple Proposals | Mapped ranges and reservations are implementable | Same-block intervals and author-created structural collisions remain decisions |
| Multi-operation BlockEditProposal | Stable block IDs support durable operation identity | Split, join, count-changing rewrite, and operation reassignment rules are unresolved |

## Prototype acceptance matrix

The follow-up disposable prototype should fail unless it demonstrates all of the following against the exact pinned package versions.

### Transaction and history tests

- A negative regression proves first-batch-tracked plus later-off-history streaming does not accidentally masquerade as one complete history event.
- All Agent stream batches remain outside ProseMirror history; ordinary ProseMirror undo never partially removes a generated candidate.
- Restoring an earlier generated candidate appends an exact Proposal Revision, and reapplying it appends another revision, for inline replacement, continuation insert, complete-block, and multi-block shapes.
- Artifact-restoration and authoritative-compensation projection transactions preserve schema and stable block IDs and safely remap older author history.
- An aborted stream before its first batch leaves no fake document content.
- An interrupted stream after several batches becomes `ready_partial`; later arriving deltas do not enter the editor.
- The author's first edit after interruption is a separate history event.
- Author edits at both Proposal boundaries follow the chosen association and persist as the correct Direct Author Action or Proposal Revision.
- A transaction wholly outside reservations persists only authoritative records; one wholly inside one pending operation persists only a Proposal Revision.
- Cross-boundary, multiple-reservation, and mixed-ownership paste/drop/cut/delete/replace transactions are refused atomically, never partially applied, and expose a recoverable retry path.
- Stock UniqueID ordinary repair is proven to enter history/classification as observed at the pinned commit; the StoryOS-owned/adapted path is proven tagged and off-history, including appended transactions that never traverse outer dispatch middleware.

### IME and input tests

- Real Chinese Pinyin and Japanese IME checks on current Chrome, Safari, and Firefox on macOS.
- Composition starts inside a streaming Proposal, at both boundaries, and elsewhere in the chapter.
- `compositionstart` synchronously closes the adapter-owned RunWriteGate before any subsequent ProseMirror dispatch; no Agent transaction passes it even when network deltas race.
- The composed text, selection, marks, and undo grouping remain correct after `compositionend`.
- Paste, drop, cut, direct typing, mobile/native `beforeinput`, and rapid consecutive compositions all trigger the same author-priority pause policy.
- Composition at a Proposal boundary follows one ownership association; a selected range crossing a boundary follows the explicit whole-transaction policy without corrupting the DOM.

Synthetic composition events are useful regression tests but are not sufficient evidence for real IME behavior.

### Stable identity and conflict tests

- Split, join, backspace-join, Enter-split, block type change, copy/paste, cut/paste, drag/move, undo, and redo each produce the intended block-ID result.
- Duplicate or missing IDs are detected before Core validation and Acceptance.
- Edits before, after, inside, and across one Proposal map its envelope correctly.
- Two non-overlapping Proposals remain independent when content changes elsewhere.
- A structural candidate edit that destroys or overlaps reservations is preserved in a Proposal Revision while validation becomes `conflicted`.
- Compare whole-block reservation with disjoint same-block inline reservations.
- Round-trip block-relative anchors containing BMP text, emoji/surrogate pairs, combining sequences, marks, `hard_break`, and inline atom nodes between TypeScript and Rust without byte/scalar drift.
- Schema-version mismatch and changed inline-node `nodeSize` fail closed or migrate explicitly rather than relocating a Proposal silently.

### Persistence and domain-undo tests

- Reload reconstructs exact generation/validation/closure axes and pending/applied/rejected operation incarnations for InlineEditProposal and multi-operation BlockEditProposal with identical visible Diff.
- Reload between a durable Run delta, Proposal Revision, and projection checkpoint is either replayable or explicitly detected without duplication.
- A durably `ready_partial` Proposal Revision never replays Run deltas beyond its admitted-through pause fence.
- A crash between the runtime RunWriteGate closing and durable fence produces explicit reconciliation rather than blind replay.
- `AcceptProposal` refuses anything except an exact retained, ready, currently valid, open Proposal Revision with a current Validation Receipt and selected pending operation IDs; success creates Authoritative Revisions, an Authoritative Commit, frozen applied incarnations, and an Acceptance Receipt atomically.
- `RejectProposalOperations` freezes only selected pending incarnations, never mutates Authoritative State, never overwrites uncheckpointed author edits, and leaves unselected operations editable.
- Reopening a rejected operation appends a new Proposal Revision and requires validation; the rejected incarnation remains immutable.
- Safe `UndoAcceptance` appends compensating Authoritative Revisions/Commit, an Undo Acceptance Receipt, and a new Proposal Revision; incompatible heads derive a new Proposal and overlapping author work yields a Reversal Proposal.
- Undo routing wins over both keyboard `Mod-z` and native `beforeinput historyUndo` only when Acceptance is the newest reversible action; every ordinary editor undo is reclassified and persisted, and older editor history remains available afterward.

## Decisions this research supports

The research narrows the foundation to this design direction; the prototype remains the gate for the cross-mechanism items called out above:

1. one StoryOS Proposal Tiptap extension backed by a keyed ProseMirror plugin;
2. one reconstructible editor review projection over Authoritative State and exact Proposal Revisions, never another authority source;
3. stable block IDs plus a versioned ProseMirror-token/UTF-16 coordinate contract;
4. transient mapped ranges and derived DecorationSets;
5. Agent generation outside ProseMirror history, with exact restoration expressed as immutable Proposal Revisions;
6. an adapter-owned synchronous RunWriteGate closed on any author edit signal, especially composition start;
7. deterministic root-transaction ownership classification and conservative whole refusal for unsplittable mixed ownership;
8. one coordinator for Artifact restoration, `UndoAcceptance`, Direct Author Action undo, and session-local ProseMirror history;
9. exact-head autosave checkpoints, durable pause fences, and validation-driven rehydration;
10. validation `conflicted` rather than a `stale` status or implicit overlapping-Proposal rebase;
11. operation-level `AcceptProposal` / `RejectProposalOperations`, with immutable historical outcomes.

The prototype must resolve the remaining interaction policy for boundary typing, same-block non-overlap, structural edits inside a Proposal Operation, multi-block operation reshaping, refused mixed-ownership input, and cross-stack undo ordering.
