import { useEffect, useRef, useState } from "react";
import { getRefusedEditDraft } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ProjectScope, RefusedEditDraftInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { canonicalDraftValue as canonical, discardRefusedEdit, reconcileDiscard, type DiscardObservation } from "./refused-edit-discard.ts";
import { rebuildPendingProjection, readJournalSnapshot, validateJournalSnapshot } from "./local-edit-journal.ts";
import type { EditorWorkspace, JournalSubmissionGroup, PendingEditProjection } from "./editor-types.ts";

export function RefusedEditDraftDisplay({ workspace, scope, baseUrl, fetchImpl, refreshKey, onHoldChange, onProjection }: {
  workspace: EditorWorkspace | undefined; scope: ProjectScope; baseUrl: string;
  fetchImpl: typeof fetch; refreshKey: string; onHoldChange?: ((hold: boolean) => void) | undefined;
  onProjection?: ((projection: PendingEditProjection) => void) | undefined;
}) {
  const [reads, setReads] = useState<{ group: JournalSubmissionGroup;
    draft?: RefusedEditDraftInspect; copied?: boolean; discard?: DiscardObservation | undefined }[]>([]);
  const lifetime = useRef(0);
  const [busy, setBusy] = useState(false);
  const [settledWriter, setSettledWriter] = useState(false);
  async function read(group: JournalSubmissionGroup): Promise<RefusedEditDraftInspect> {
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
    if ((draft.closure === "closed" && (!event || event.schema_id !== "storyos.event.editor-flow-draft-closed.v1"
      || event.event_kind !== "editor_flow_draft_closed" || canonical(event.project_scope) !== canonical(scope)
      || event.draft_id !== effect.draft_id || event.draft_revision_id !== effect.draft_revision_id
      || event.payload_digest !== draft.payload_digest || event.closure !== "closed" || event.prior_closure !== "open"
      || event.close_reason !== "abandoned")) || (draft.closure === "open" && event != null)) throw new Error("Draft unavailable");
    if (result.schema_id !== "storyos.query.refused-edit-draft.response.v1"
      || canonical(result.project_scope) !== canonical(scope)
      || canonical(group.project_scope) !== canonical(scope)
      || draft.kind !== "refused_edit" || draft.retention_state !== "retained"
      || !["open", "closed"].includes(draft.closure)
      || draft.draft_id !== effect.draft_id || draft.draft_revision_id !== effect.draft_revision_id
      || canonical(draft.payload) !== canonical(expected) || draft.payload_digest !== hex
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
      if (!active) return;
      setSettledWriter(workspace.partition.disposition === "current_writer_open"
        && workspace.session.writer.kind === "current_writer" && projection.save_state === "saved"
        && projection.unsettled_intent_count === 0);
      const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
      const groups = snapshot.groups.filter((group) => group.settlement.kind === "zero_authority_receipt_settled"
        && group.settlement.effect.kind === "refused_to_draft");
      const next = await Promise.all(groups.map(async (group) => {
        try { const draft = await read(group);
          return { group, draft, discard: await reconcileDiscard(workspace, draft) }; } catch { return { group }; }
      }));
      const currentProjection = await rebuildPendingProjection(workspace);
      if (active) { setReads(next); onProjection?.(currentProjection); }
    })().catch(() => { if (active) setReads([]); });
    return () => { active = false; lifetime.current += 1; };
  }, [workspace, scope.owner_user_id, scope.project_id, baseUrl, fetchImpl, refreshKey]);
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
  return reads.map(({ group, draft, copied, discard: observation }) => {
    const settled = group.settlement;
    if (settled.kind !== "zero_authority_receipt_settled" || settled.effect.kind !== "refused_to_draft") return null;
    const id = settled.effect.draft_id;
    if (draft === undefined) return <p role="status" key={id} data-draft-unavailable={id}>Draft unavailable.</p>;
    const unit = draft.payload.author_edit_units[0]!;
    return <section key={id} data-refused-edit-draft={id} aria-label="Preserved edit draft">
      <p>Draft preserved. The manuscript and proposal remain unchanged.</p>
      {unit.normalized_primitives.flatMap((primitive) => primitive.kind === "replace_structured_selection"
        ? primitive.replacement.map((block, index) => <pre key={index} data-draft-replacement={block.block_kind}>{block.text}</pre>) : [])}
      {draft.closure === "open" && observation === undefined && settledWriter
        ? <button type="button" data-draft-discard disabled={busy} onClick={() => { void discard(group); }}>Discard</button> : null}
      {observation?.kind === "unresolved" ? <p role="status" data-discard-unresolved>Discard outcome unresolved. No new Discard was submitted.</p> : null}
      {observation?.kind === "settled" && observation.response.effect.kind !== "draft_closure_changed"
        ? <p role="status" data-discard-settled>Discard {observation.response.effect.kind}. The Draft was not closed by this command.</p> : null}
      {draft.closure === "closed" && draft.closure_event ? <p data-draft-closed>
        Closed: {draft.closure_event.close_reason}. Event: {draft.closure_event.event_id}. Undo unavailable: non-skippable Barrier.</p> : null}
      <button type="button" data-draft-copy onClick={() => { void copy(group); }}>Copy</button>
      {copied ? <p role="status">Copied</p> : null}
      <details><summary>Source and draft identity</summary>
        <p>Draft: {id}. Revision: {draft.draft_revision_id}. Creation: {draft.creation.creation_event_id}.</p>
        <p>Command: {draft.creation.source.command_id}. Admission: {draft.creation.source.author_command_admission_id}. Idempotency key: {draft.creation.source.idempotency_key}.</p>
        <p>Receipt: {draft.creation.source.receipt_id}. Digest: {draft.payload_digest}. Status: {draft.closure}, {draft.retention_state}.</p>
        <p>Selection: {JSON.stringify(unit.selection_snapshot?.ordered_selection?.anchor)} to {JSON.stringify(unit.selection_snapshot?.ordered_selection?.head)}.</p>
        {unit.selection_snapshot?.ordered_selection?.sources.map((source, index) => <div key={index}>
          <code>{JSON.stringify(source.owner)}; {source.block_kind}; {source.coordinate_profile}; {source.from}–{source.to}</code>
          <pre>{source.source_text}</pre></div>)}
      </details>
    </section>;
  });
}
