import { RefusedEditRetryControls } from "./refused-edit-retry-controls.tsx";
import { RefusedDraftChapterSource } from "./refused-draft-chapter-source.tsx";
import { retryRefusedEdit, type RetryTargetRead } from "./refused-edit-retry.ts";
import { expandWholeDraft } from "./refused-edit-expansion.ts";
import type { DraftPayloadPosition } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { useEffect, useRef, useState } from "react";
import { getRefusedEditDraft } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ProjectScope, RefusedEditDraftInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { canonicalDraftValue as canonical, discardRefusedEdit, MAX_DISCARD_RECORDS, reconcileDiscard, type DiscardObservation } from "./refused-edit-discard.ts";
import { validDraftReopen, reconcileDraftUndo } from "./draft-undo-journal.ts";
import { rebuildPendingProjection, readJournalSnapshot, validateJournalSnapshot } from "./local-edit-journal.ts";
import type { EditorWorkspace, JournalSubmissionGroup, PendingEditProjection } from "./editor-types.ts";

export function RefusedEditDraftDisplay({ workspace, scope, baseUrl, fetchImpl, refreshKey, onHoldChange, onProjection, onResult }: {
  workspace: EditorWorkspace | undefined; scope: ProjectScope; baseUrl: string;
  fetchImpl: typeof fetch; refreshKey: string; onHoldChange?: ((hold: boolean) => void) | undefined;
  onProjection?: ((projection: PendingEditProjection) => void) | undefined; onResult?: (() => void) | undefined;
}) {
  const [reads, setReads] = useState<{ group: JournalSubmissionGroup;
    draft?: RefusedEditDraftInspect; copied?: boolean; discard?: DiscardObservation | undefined }[]>([]);
  const lifetime = useRef(0);
  const [retryRefresh, setRetryRefresh] = useState(0);
  const [retryResults, setRetryResults] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [settledWriter, setSettledWriter] = useState(false);
  async function read(group: JournalSubmissionGroup): Promise<RefusedEditDraftInspect> {
    const started = lifetime.current;
    const settled = group.settlement;
    if (workspace === undefined || settled.kind !== "zero_authority_receipt_settled"
      || settled.effect.kind !== "refused_to_draft") throw new Error("Draft unavailable");
    const effect = settled.effect;
    const result = await getRefusedEditDraft({ baseUrl, fetchImpl,
      projectId: scope.project_id, draftId: effect.draft_id });
    const draft = result.draft;
    const request = group.frozen_request_body;
    const expected = { schema_revision: "storyos.refused-edit-payload.v1",
      chapter_id: request.chapter_id, expected_authoritative_revision_id: request.expected_authoritative_revision_id,
      expected_proposal_head_revision_ids: request.expected_proposal_head_revision_ids,
      target_refs: request.target_refs, author_edit_units: request.author_edit_units,
      undo_group_id: request.undo_group_id, completed_intent_record_id: request.completed_intent_record_id,
      local_intent_sequence: request.local_intent_sequence };
    const digest = await workspace.cryptoImpl.subtle.digest("SHA-256",
      new TextEncoder().encode(canonical(expected)));
    const hex = [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    const event = draft.closure_event;
    if (!validDraftReopen(draft, scope)) throw new Error("Draft unavailable");
    if ((draft.closure === "closed" && (!event || event.schema_id !== "storyos.event.editor-flow-draft-closed.v1"
      || event.event_kind !== "editor_flow_draft_closed" || canonical(event.project_scope) !== canonical(scope)
      || event.draft_id !== effect.draft_id || event.draft_revision_id !== effect.draft_revision_id
      || event.payload_digest !== draft.payload_digest || event.closure !== "closed" || event.prior_closure !== "open"
      || !["abandoned", "superseded"].includes(event.close_reason))) || (draft.closure === "open" && !validDraftReopen(draft, scope))) throw new Error("Draft unavailable");
    if (result.schema_id !== "storyos.query.refused-edit-draft.response.v1"
      || canonical(result.project_scope) !== canonical(scope)
      || canonical(group.project_scope) !== canonical(scope)
      || draft.kind !== "refused_edit" || draft.retention_state !== "retained"
      || !["open", "closed"].includes(draft.closure)
      || draft.draft_id !== effect.draft_id || draft.draft_revision_id !== effect.draft_revision_id
      || canonical(draft.payload) !== canonical(expected) || draft.payload_digest !== hex
      || canonical(draft.replacement_provenance) !== canonical(effect.replacement_provenance)
      || draft.payload_digest_profile !== "storyos.refused-edit-payload.jcs.v1"
      || draft.creation.event_kind !== "refused_edit_draft_created"
      || draft.creation.schema_id !== "storyos.event.refused-edit-draft-created.v1"
      || canonical(draft.creation.project_scope) !== canonical(scope)
      || draft.creation.draft_id !== effect.draft_id || draft.creation.draft_revision_id !== effect.draft_revision_id
      || draft.creation.creation_event_id !== effect.creation_event_id
      || draft.creation.creator.kind !== "core_transition"
      || draft.creation.creator.receipt_id !== settled.receipt.receipt_id
      || canonical(draft.creation.source) !== canonical({ command_id: settled.command_id,
        author_command_admission_id: settled.author_command_admission_id,
        receipt_id: settled.receipt.receipt_id, idempotency_key: group.idempotency_key,
        command_digest: group.frozen_request_digest })) throw new Error("Draft unavailable");
    await reconcileDraftUndo(workspace, draft, () => started === lifetime.current);
    return draft;
  }
  useEffect(() => {
    lifetime.current += 1;
    let active = true;
    setReads([]);
    setBusy(false);
    setSettledWriter(false);
    onHoldChange?.(false);
    if (workspace !== undefined) void (async () => {
      const projection = await rebuildPendingProjection(workspace);
      const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
      if (!active) return;
      setSettledWriter(workspace.partition.disposition === "current_writer_open"
        && workspace.session.writer.kind === "current_writer" && projection.save_state === "saved"
        && projection.unsettled_intent_count === 0 && (snapshot.explicitDiscard?.length ?? MAX_DISCARD_RECORDS) < MAX_DISCARD_RECORDS);
      const groups = snapshot.groups.filter((group) => group.settlement.kind === "zero_authority_receipt_settled"
        && group.settlement.effect.kind === "refused_to_draft");
      const results: Record<string, string> = {};
      for (const group of snapshot.groups) {
        const source = group.frozen_request_body.retry_source;
        if (!source) continue;
        if (group.settlement.kind === "applied_receipt_settled") {
          results[source.source_draft_id] = "The selected text was applied to the manuscript.";
        } else if (group.settlement.kind === "zero_authority_receipt_settled") {
          const messages = {
            proposal_revised: "The selected text was applied to the proposal.",
            refused_to_draft: "The retry was not applied. Its complete text was preserved in a new draft.",
            no_effect: "The retry made no change.",
            conflicted: "The text changed before this retry could apply. The draft remains available.",
            refused: "The retry could not apply. The draft remains available.",
          };
          results[source.source_draft_id] = messages[group.settlement.effect.kind];
        } else results[source.source_draft_id] = "The retry outcome is not yet confirmed. No new retry was submitted.";
      }
      setRetryResults(results);
      const next = await Promise.all(groups.map(async (group) => {
        try { const draft = await read(group);
          return { group, draft, discard: await reconcileDiscard(workspace, draft) }; } catch { return { group }; }
      }));
      const currentProjection = await rebuildPendingProjection(workspace);
      if (active) { setReads(next);
        setSettledWriter(workspace.partition.disposition === "current_writer_open"
          && workspace.session.writer.kind === "current_writer" && currentProjection.save_state === "saved"
          && currentProjection.unsettled_intent_count === 0 && (snapshot.explicitDiscard?.length ?? MAX_DISCARD_RECORDS) < MAX_DISCARD_RECORDS);
        onProjection?.(currentProjection); }
    })().catch(() => { if (active) setReads([]); });
    return () => { active = false; lifetime.current += 1; };
  }, [workspace, scope.owner_user_id, scope.project_id, baseUrl, fetchImpl, refreshKey, retryRefresh]);
  async function discard(group: JournalSubmissionGroup) {
    if (workspace === undefined || busy) return;
    const started = lifetime.current;
    setBusy(true);
    onHoldChange?.(true);
    try { const draft = await read(group);
      if (started !== lifetime.current) return;
      await discardRefusedEdit({ workspace, draft, baseUrl, fetchImpl, isCurrent: () => started === lifetime.current }); }
    catch { /* Only an exact public settlement can close the Draft. */ }
    try {
      const draft = await read(group);
      const observation = await reconcileDiscard(workspace, draft);
      if (started !== lifetime.current) return;
      const projection = await rebuildPendingProjection(workspace);
      if (started !== lifetime.current) return;
      setReads((current) => current.map((item) => item.group === group
        ? { group, draft, discard: observation } : item));
      onProjection?.(projection);
    } catch {
      if (started === lifetime.current) setReads((current) => current.map((item) => item.group === group ? { group } : item));
    } finally { if (started === lifetime.current) { setBusy(false); onHoldChange?.(false); } }
  }
  async function retry(group: JournalSubmissionGroup, from: DraftPayloadPosition, to: DraftPayloadPosition,
    target: string, targetFrom: number, targetTo: number, targetRead: RetryTargetRead) {
    if (!workspace || busy) return;
    const started = lifetime.current;
    setBusy(true); onHoldChange?.(true);
    try {
      const draft = await read(group);
      if (started !== lifetime.current) throw new Error("Retry view changed");
      const projection = await retryRefusedEdit({ workspace, draft, from, to, target, targetFrom, targetTo, targetRead,
        baseUrl, fetchImpl, isCurrent: () => started === lifetime.current });
      if (started !== lifetime.current) return;
      onProjection?.(projection); onResult?.();
      setRetryRefresh((value) => value + 1);
    } finally { if (started === lifetime.current) { setBusy(false); onHoldChange?.(false); } }
  }
  async function copy(group: JournalSubmissionGroup) {
    const started = lifetime.current;
    setReads((current) => current.map((item) => item.group === group ? { group } : item));
    try {
      const draft = await read(group);
      if (lifetime.current !== started) return;
      const text = draft.payload.author_edit_units.flatMap((unit) => unit.normalized_primitives.flatMap((primitive) =>
        primitive.kind === "replace_structured_selection" ? primitive.replacement.map((block) => block.text) : [])).join("\n");
      await navigator.clipboard.writeText(text);
      const observation = await reconcileDiscard(workspace!, draft);
      if (lifetime.current !== started) return;
      setReads((current) => current.map((item) => item.group === group ? { group, draft, copied: true, discard: observation } : item));
    } catch { /* The fresh query is required before Copy. */ }
  }
  async function expand(group: JournalSubmissionGroup, target: string, start: number, end: number, targetRead: RetryTargetRead) {
    if (!workspace || busy) return;
    const started = lifetime.current; setBusy(true); onHoldChange?.(true);
    try {
      const draft = await read(group);
      await expandWholeDraft(workspace, draft, target, start, end, targetRead, baseUrl, fetchImpl, () => started === lifetime.current);
    } finally {
      if (started === lifetime.current) { setBusy(false); onHoldChange?.(false); onResult?.(); setRetryRefresh((value) => value + 1); }
    }
  }
  return reads.map(({ group, draft, copied, discard: observation }) => {
    const settled = group.settlement;
    if (settled.kind !== "zero_authority_receipt_settled" || settled.effect.kind !== "refused_to_draft") return null;
    const id = settled.effect.draft_id;
    if (draft === undefined) return <p role="status" key={id} data-draft-unavailable={id}>Draft unavailable.</p>;
    const unit = draft.payload.author_edit_units[0]!;
    return <section className="editor-recovery" key={id} data-refused-edit-draft={id} aria-label="Preserved edit draft">
      <header className="editor-recovery-heading">
        <span>Preserved edit</span>
        <h2>This edit was not applied</h2>
        <p>The manuscript and proposal were not changed by this edit. Your complete text is preserved below.</p>
        <RefusedDraftChapterSource scope={scope} sourceChapterId={draft.payload.chapter_id}
          currentChapterId={workspace?.session.project_scope.owner_user_id === scope.owner_user_id
            && workspace.session.project_scope.project_id === scope.project_id
            ? workspace.session.base_snapshot.chapter_id : undefined}
          baseUrl={baseUrl} fetchImpl={fetchImpl} />
      </header>
      <div className="editor-recovery-copy"><span>Full edit</span>
      {unit.normalized_primitives.flatMap((primitive) => primitive.kind === "replace_structured_selection"
        ? primitive.replacement.map((block, index) => <pre key={index} data-draft-replacement={block.block_kind}>{block.text}</pre>) : [])}</div>
      {draft.closure === "open" && observation === undefined && settledWriter
        ? <button type="button" data-draft-discard disabled={busy} onClick={() => { void discard(group); }}>Discard</button> : null}
      {draft.closure === "open" && observation === undefined && settledWriter
        ? <RefusedEditRetryControls draft={draft} workspace={workspace!} baseUrl={baseUrl} fetchImpl={fetchImpl} disabled={busy}
          submit={(from, to, target, start, end, read) => retry(group, from, to, target, start, end, read)}
          expand={(target, start, end, read) => expand(group, target, start, end, read)} /> : null}
      <button type="button" data-draft-copy onClick={() => { void copy(group); }}>Copy</button>
      {copied ? <p role="status">Copied</p> : null}
      {retryResults[id] ? <p role="status" data-draft-retry-result>{retryResults[id]}</p> : null}
      {draft.replacement_provenance ? <p data-draft-replacement-source>This draft preserves the complete text of a retry from an earlier draft.</p> : null}
      {observation?.kind === "unresolved" ? <p role="status" data-discard-unresolved>The discard outcome is not yet confirmed. No new discard was submitted.</p> : null}
      {observation?.kind === "settled" && observation.response.effect.kind !== "draft_closure_changed"
        ? <p role="status" data-discard-settled>This discard did not close the draft. Its full text remains available.</p> : null}
      {draft.closure === "closed" && draft.closure_event ? <p data-draft-closed>
        {draft.closure_event.close_reason === "abandoned" ? "Discarded." : "Replaced by the retry result."} The full text remains available to copy.</p> : null}
      {draft.reopen_event ? <p data-draft-reopened>Restored by Undo. This draft is available again.</p> : null}
      <details><summary>Original selected text</summary>
        {unit.selection_snapshot?.ordered_selection?.sources.map((source, index) => <div key={index}>
          <p>{source.owner.kind === "proposal" ? "Proposal text" : "Manuscript text"}</p>
          <pre>{source.source_text}</pre></div>)}
      </details>
    </section>;
  });
}
