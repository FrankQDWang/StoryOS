import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { LocalRecoveryPanel } from "../../src/local-recovery-panel.tsx";
import { openEditorWorkspace, persistReplaceSelection, rebuildPendingProjection, submitOnePendingAuthorEdit } from "../../src/editor-session.ts";
import { persistCandidateSelection, candidateProjectionFromJournal, readJournalSnapshot, validateJournalSnapshot } from "../../src/local-edit-journal.ts";
import { readLocalRecovery } from "../../src/local-edit-recovery.ts";
import { BLOCK, OWNER, PROJECT, SESSION, createBrowserScenario, deleteJournal, jsonResponse, requireEditorReady } from "./scenario.ts";

it.each(["multi_block", "candidate"] as const)("keeps complete %s recovery visible through Copy, explicit continuation and reload", async (kind) => {
  const scenario = createBrowserScenario();
  const target = { proposal_id: "018f0000-0000-7001-8000-000000000101", operation_id: "018f0000-0000-7001-8000-000000000102",
    revision_id: "018f0000-0000-7001-8000-000000000103", manuscript_block_id: BLOCK };
  if (kind === "multi_block") {
    const revision = { ...scenario.chapter.chapter.current_revision, body: "Base\nSecond", blocks: [
      { manuscript_block_id: BLOCK, block_kind: "paragraph" as const, text: "Base" },
      { manuscript_block_id: "018f0000-0000-7001-8000-0000000000b2", block_kind: "paragraph" as const, text: "Second" },
    ] };
    scenario.chapter.chapter.current_revision = revision;
    scenario.session.base_snapshot.materialized_revision = revision;
    scenario.session.base_snapshot.materialized_payload_digest.value_hex_lowercase = [...new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(revision.body)))].map((n) => n.toString(16).padStart(2, "0")).join("");
  }
  let posts = 0;
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-12-13T08:05:00.000Z", limit_profile_revision: "storyos.foundation.absolute.v1" });
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    if (path.endsWith("/manuscript/author-edits")) { posts += 1; throw new Error("Acknowledgement lost"); }
    if (path.includes("/manuscript/author-edit-outcomes/")) return jsonResponse({
      schema_id: "storyos.query.apply-author-edit-outcome.response.v1", correlation_id: "018f0000-0000-7001-8000-000000000082",
      project_scope: scenario.project.project_scope, outcome: { outcome_kind: "requires_reconfirmation",
        command_id: "018f0000-0000-7001-8000-000000000031", author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
        reconfirmation_reason: "direct_edit_intent_unrecoverable", recovery_draft_ref: null } });
    throw new Error(`Unexpected request ${path}`);
  };
  const open = async () => {
    const state = await openEditorWorkspace({ baseUrl: location.origin,
      project: scenario.project, chapter: scenario.chapter, profile: scenario.profile, fetchImpl });
    requireEditorReady(state); return state;
  };
  await deleteJournal(scenario.journalName);
  let workspace = await open();
  const host = document.createElement("div"); document.body.append(host);
  let root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  const clipboard = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
  try {
    if (kind === "candidate") await persistCandidateSelection(workspace, { kind: "candidate_selection", target, expectedProposalHeads: [target.revision_id], priorText: "Candidate", from: 9, to: 9, text: "!", resultingBody: "Candidate!" });
    else await persistReplaceSelection(workspace, { manuscript_block_id: BLOCK, from: 4, to: 4, text: "!", resultingBody: "Base!\nSecond" });
    // Cut after strict local persistence and before the first Author Edit POST.
    workspace.database.close(); workspace = await open();
    expect(posts).toBe(0);
    const before = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
    const completeText = kind === "candidate" ? "Candidate!" : "Base!\nSecond";
    expect(before.bodyBySequence.get(before.records.at(-1)!.local_intent_sequence)).toBe(completeText);
    workspace.pending = await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto });
    expect(workspace.pending.save_state).toBe("needs_attention");
    let continued = false;
    const render = async () => act(async () => root.render(createElement(LocalRecoveryPanel, { workspace, refreshKey: "0", onContinue: async () => { continued = true; } })));
    await render();
    await expect.poll(() => host.querySelector("[data-local-recovery-text]")?.textContent).toBe(completeText);
    expect(host.textContent).toContain("服务器无法恢复原写入请求");
    await act(async () => host.querySelector<HTMLButtonElement>("[data-local-recovery-copy]")!.click());
    expect(clipboard).toHaveBeenCalledWith(completeText); expect(posts).toBe(1);
    await act(async () => host.querySelector<HTMLButtonElement>("[data-local-recovery-continue]")!.click());
    await expect.poll(() => continued).toBe(true);
    const retained = await readLocalRecovery(workspace);
    expect(retained[0]?.text).toBe(completeText);
    expect(await candidateProjectionFromJournal(workspace, target)).toBeUndefined();
    expect(await rebuildPendingProjection(workspace)).toMatchObject({ body: scenario.chapter.chapter.current_revision.body, save_state: "saved", unsettled_intent_count: 0 });
    await persistReplaceSelection(workspace, { manuscript_block_id: BLOCK, from: 4, to: 4, text: "+", resultingBody: kind === "candidate" ? "Base+" : "Base+\nSecond" });
    await act(async () => root.unmount());
    workspace.database.close(); workspace = await open(); root = createRoot(host);
    await render();
    await expect.poll(() => host.querySelector("[data-local-recovery-text]")?.textContent).toBe(completeText);
    expect(host.querySelector("[data-local-recovery-continue]")).toBeNull();
    expect(await readLocalRecovery(workspace)).toEqual(retained);
    expect(workspace.pending.body).toBe(kind === "candidate" ? "Base+" : "Base+\nSecond");
    expect(posts).toBe(1);
  } finally {
    await act(async () => root.unmount()); clipboard.mockRestore(); host.remove(); workspace.database.close();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`); await deleteJournal(scenario.journalName);
  }
});
