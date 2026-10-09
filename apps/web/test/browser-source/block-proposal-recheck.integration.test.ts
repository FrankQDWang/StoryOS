import { Editor } from "@tiptap/core";
import { act, createElement, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { projectBlockProposals, type BlockProposalProjection } from "../../src/block-proposal-decoration.ts";
import { BlockProposalDisplay, type ProposalLocator } from "../../src/block-proposal-display.tsx";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import type { EditorReadyState } from "../../src/editor-types.ts";
import { manuscriptJson } from "../../src/manuscript-doc.ts";
import { storyosManuscriptExtensions } from "../../src/manuscript-tiptap-adapter.ts";
import { applyImeComposition, applyTrustedInput } from "../support/browser-command-client.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { BLOCK, createAppliedAuthorEditResponse, createBrowserScenario, jsonResponse } from "./scenario.ts";

type Proposal = typeof proposalFixture.proposal;

function candidateFor(workspace: EditorReadyState): Proposal {
  const blockId = workspace.openedProjection.blocks[0]!.manuscript_block_id;
  return { ...proposalFixture.proposal, chapter_id: workspace.session.base_snapshot.chapter_id,
    manuscript_block_id: blockId, base_authoritative_revision_id: workspace.openedProjection.authoritative_revision_id,
    operations: proposalFixture.proposal.operations.map((operation) => ({ ...operation, manuscript_block_id: blockId })) };
}

async function renderCandidate(workspace: EditorReadyState, proposal: Proposal, fetchImpl: typeof fetch) {
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  const locator: ProposalLocator = { proposalId: proposal.proposal_id, runId: proposal.source.run_id,
    decisionId: proposal.source.decision_id };
  const failures: unknown[] = [];
  const writing = createEditorSessionWritingController({ workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failures.push(error); } });
  function View() {
    const { projection } = useSyncExternalStore(writing.subscribe, writing.snapshot);
    return createElement(BlockProposalDisplay, { scope: workspace.partition.project_scope,
      chapterId: workspace.session.base_snapshot.chapter_id, authoritativeRevisionId: projection.authoritative_revision_id,
      locators: [locator], refreshKey: 0, safeToProject: true, onAccepted: async () => {},
      focusProposal: { proposalId: proposal.proposal_id, operationId: proposal.operation_id,
        revisionId: proposal.revision_id, blockId: proposal.manuscript_block_id },
      blocks: projection.blocks, editable: true, persistWorkspace: workspace, writing, baseUrl: location.origin, fetchImpl,
      cryptoImpl: crypto, onFailure: (error) => { failures.push(error); } });
  }
  await act(async () => { root.render(createElement(View)); });
  const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
  await expect.poll(() => surface.querySelector(`[data-candidate-proposal-id="${proposal.proposal_id}"]`)).not.toBeNull();
  await expect.poll(() => surface.getAttribute("contenteditable")).toBe("true");
  const transitions: (string | null)[] = [];
  const observer = new MutationObserver((records) => {
    transitions.push(...records.map((record) => (record.target as Element).getAttribute("contenteditable")));
  });
  observer.observe(surface, { attributes: true, attributeFilter: ["contenteditable"] });
  return { host, surface, writing, failures,
    transitions: () => [...transitions, ...observer.takeRecords().map((record) =>
      (record.target as Element).getAttribute("contenteditable"))],
    async unmount() { observer.disconnect(); await act(async () => { root.unmount(); }); writing.close(); host.remove(); } };
}

async function withActEnvironment(action: () => Promise<void>) {
  const previous = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try { await action(); } finally { Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previous }); }
}

it("keeps the editor editable and the caret in place while the Proposal checks run again after an Author Edit settles", async () => {
  const test = await openJournalAppendTestWorkspace();
  const scenario = createBrowserScenario();
  const proposal = candidateFor(test.workspace);
  let canonical = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
  let reads = 0, submissions = 0;
  let rereadStarted!: () => void, releaseReread!: (value: Proposal) => void;
  const reread = new Promise<void>((resolve) => { rereadStarted = resolve; });
  const rereadResult = new Promise<Proposal>((resolve) => { releaseReread = resolve; });
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith(`/proposals/${proposal.proposal_id}`)) {
      reads += 1;
      if (reads > 1) rereadStarted();
      return jsonResponse({ ...proposalFixture, project_scope: scenario.project.project_scope,
        proposal: reads === 1 ? proposal : await rereadResult });
    }
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse({ nonce: "a".repeat(64),
      expires_at: new Date(Date.now() + 60_000).toISOString(), limit_profile_revision: "storyos.foundation.absolute.v1" });
    if (path.includes("/editor-sessions/")) return jsonResponse(canonical);
    if (!path.endsWith("/manuscript/author-edits")) throw new Error(`No request handler: ${path}`);
    // Subsequent input stays in the Journal. This test monitors only the first settlement.
    if ((submissions += 1) > 1) return new Promise<Response>(() => {});
    const request = JSON.parse(String(init?.body));
    const response = createAppliedAuthorEditResponse({ request, body: "Base!",
      commandDigest: await digestApplyAuthorEdit(request), idempotencyKey: new Headers(init?.headers).get("idempotency-key")! });
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode("Base!"));
    canonical = { ...canonical, base_snapshot: { ...canonical.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000080", project_activity_position: "1",
      authoritative_head_revision_id: response.effect.kind === "authoritative_applied"
        ? response.effect.authoritative_revision.revision_id : "",
      materialized_revision: { ...canonical.base_snapshot.materialized_revision, revision_id: "018f0000-0000-7001-8000-000000000034",
        body: "Base!", blocks: canonical.base_snapshot.materialized_revision.blocks.map((item) => ({ ...item, text: "Base!" })) },
      materialized_payload_digest: { ...canonical.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("") } } };
    return jsonResponse(response);
  };
  await withActEnvironment(async () => {
    const view = await renderCandidate(test.workspace, proposal, fetchImpl);
    try {
      const paragraph = view.surface.querySelector("p")!;
      view.surface.focus();
      window.getSelection()!.setBaseAndExtent(paragraph.firstChild!, 4, paragraph.firstChild!, 4);
      document.dispatchEvent(new Event("selectionchange"));
      await applyTrustedInput({ operation: "insert_text", text: "!" });
      await act(async () => { await view.writing.flush(); await reread; });
      await act(async () => { await new Promise((resolve) => { setTimeout(resolve); }); });
      await applyTrustedInput({ operation: "insert_text", text: "?" });
      expect({ paragraph: paragraph.textContent, candidate: view.surface.querySelector(".block-proposal-text")?.textContent })
        .toEqual({ paragraph: "Base!?", candidate: proposal.candidate_text });

      await act(async () => { releaseReread({ ...proposal, validation: "invalid" }); });
      await expect.poll(() => view.host.querySelector(`[data-proposal-decision="${proposal.proposal_id}"]`)?.textContent)
        .toBe("候选文字的验证已失效，请检查当前结果。");
      expect(view.surface.querySelector(`[data-candidate-proposal-id="${proposal.proposal_id}"]`)
        ?.getAttribute("contenteditable")).toBe("false");
      expect({ reads, transitions: view.transitions(), failures: view.failures })
        .toEqual({ reads: 2, transitions: [], failures: [] });
    } finally {
      releaseReread(proposal);
      await view.unmount(); await test.close();
    }
  });
});

it("keeps the editor read-only after an Acceptance with an unknown result until the Journal check finds it", async () => {
  const test = await openJournalAppendTestWorkspace();
  const scenario = createBrowserScenario();
  const proposal = candidateFor(test.workspace);
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith(`/proposals/${proposal.proposal_id}`)) {
      return jsonResponse({ ...proposalFixture, project_scope: scenario.project.project_scope, proposal });
    }
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse({ nonce: "a".repeat(64),
      expires_at: new Date(Date.now() + 60_000).toISOString(), limit_profile_revision: "storyos.foundation.absolute.v1" });
    if (path.endsWith(`/proposals/${proposal.proposal_id}/acceptances`)) throw new TypeError("Failed to fetch");
    throw new Error(`No request handler: ${path}`);
  };
  await withActEnvironment(async () => {
    const view = await renderCandidate(test.workspace, proposal, fetchImpl);
    try {
      await act(async () => { view.surface.querySelector<HTMLButtonElement>("button[data-proposal-accept]")!.click(); });
      await expect.poll(() => view.host.querySelector(`[data-proposal-decision="${proposal.proposal_id}"] button`)?.textContent)
        .toBe("重试接受");
      expect({ editable: [...new Set(view.transitions())], failures: view.failures })
        .toEqual({ editable: ["false"], failures: [] });
    } finally {
      await view.unmount(); await test.close();
    }
  });
});

it("accepts input in a candidate during and after its own settlement and keeps the text when the new Revision shows", async () => {
  const test = await openJournalAppendTestWorkspace();
  const scenario = createBrowserScenario();
  const proposal = candidateFor(test.workspace);
  const revisions = ["018f0000-0000-7001-8000-000000000c03", "018f0000-0000-7001-8000-000000000c04"];
  const targets: string[] = [];
  let reads = 0;
  let rereadStarted!: () => void, releaseReread!: (value: Proposal) => void;
  const reread = new Promise<void>((resolve) => { rereadStarted = resolve; });
  const rereadResult = new Promise<Proposal>((resolve) => { releaseReread = resolve; });
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith(`/proposals/${proposal.proposal_id}`)) {
      reads += 1;
      if (reads > 1) rereadStarted();
      return jsonResponse({ ...proposalFixture, project_scope: scenario.project.project_scope,
        proposal: reads === 1 ? proposal : await rereadResult });
    }
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse({ nonce: "a".repeat(64),
      expires_at: new Date(Date.now() + 60_000).toISOString(), limit_profile_revision: "storyos.foundation.absolute.v1" });
    if (path.includes("/editor-sessions/")) return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    if (!path.endsWith("/manuscript/author-edits")) throw new Error(`No request handler: ${path}`);
    const request = JSON.parse(String(init?.body));
    targets.push(request.proposal_target.revision_id);
    const revised = revisions[targets.length - 1]!;
    const applied = createAppliedAuthorEditResponse({ request, receiptId: revised,
      commandDigest: await digestApplyAuthorEdit(request), idempotencyKey: new Headers(init?.headers).get("idempotency-key")! });
    const head = request.expected_authoritative_revision_id as string;
    return jsonResponse({ ...applied, effect: { kind: "proposal_revised", proposal_revision_id: revised,
      author_action_sequence: String(targets.length) },
    receipt: { ...applied.receipt, result: "proposal_revised", prior_heads: [head], resulting_heads: [head],
      proposal_revision_ids: [revised], authoritative_revision_ids: [], authoritative_commit_ids: [],
      author_action_sequence: String(targets.length) } });
  };
  await withActEnvironment(async () => {
    const view = await renderCandidate(test.workspace, proposal, fetchImpl);
    try {
      const candidate = () => view.surface.querySelector<HTMLElement>(`[data-candidate-proposal-id="${proposal.proposal_id}"]`)!;
      const text = () => candidate().querySelector(".block-proposal-text")!;
      view.surface.focus();
      window.getSelection()!.setBaseAndExtent(text().firstChild!, proposal.candidate_text.length,
        text().firstChild!, proposal.candidate_text.length);
      document.dispatchEvent(new Event("selectionchange"));
      await applyTrustedInput({ operation: "insert_text", text: "!" });
      await act(async () => { await view.writing.flush(); await reread; });
      // The display still shows the settled Revision. The controller journals this input against the new Revision.
      await applyTrustedInput({ operation: "insert_text", text: "+" });
      await act(async () => { await view.writing.flush(); });
      expect({ candidate: candidate().getAttribute("contenteditable"), text: text().textContent, targets })
        .toEqual({ candidate: null, text: `${proposal.candidate_text}!+`, targets: [proposal.revision_id, revisions[0]] });

      const edited = `${proposal.candidate_text}!`;
      await act(async () => { releaseReread({ ...proposal, revision_id: revisions[0]!, candidate_text: edited,
        operations: proposal.operations.map((operation) => ({ ...operation, candidate_text: edited })) }); });
      await expect.poll(() => candidate().getAttribute("data-proposal-revision-id")).toBe(revisions[0]);
      await applyTrustedInput({ operation: "insert_text", text: "?" });
      expect({ text: text().textContent, transitions: view.transitions(), failures: view.failures })
        .toEqual({ text: `${edited}+?`, transitions: [], failures: [] });
    } finally {
      releaseReread(proposal);
      await view.unmount(); await test.close();
    }
  });
});

it("keeps an IME composition in a candidate while the controller publishes a saving projection", async () => {
  const test = await openJournalAppendTestWorkspace();
  const scenario = createBrowserScenario();
  const proposal = candidateFor(test.workspace);
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith(`/proposals/${proposal.proposal_id}`)) {
      return jsonResponse({ ...proposalFixture, project_scope: scenario.project.project_scope, proposal });
    }
    // The candidate edits stay unsettled. This test examines only the editor surface.
    return new Promise<Response>(() => {});
  };
  await withActEnvironment(async () => {
    const view = await renderCandidate(test.workspace, proposal, fetchImpl);
    try {
      const text = () => view.surface.querySelector(`[data-candidate-proposal-id="${proposal.proposal_id}"] .block-proposal-text`)!;
      const end = () => {
        view.surface.focus();
        window.getSelection()!.setBaseAndExtent(text().firstChild!, text().textContent!.length,
          text().firstChild!, text().textContent!.length);
        document.dispatchEvent(new Event("selectionchange"));
      };
      end();
      await applyTrustedInput({ operation: "insert_text", text: "!" });
      // The composition offsets count the rendered text of the editor, which the selection text gives.
      window.getSelection()!.setBaseAndExtent(view.surface, 0, text().firstChild!, text().textContent!.length);
      const offset = window.getSelection()!.toString().length;
      end();
      await applyImeComposition({ text: "zhong", replacementStart: offset, replacementEnd: offset,
        selectionStart: 5, selectionEnd: 5 });
      await act(async () => { await new Promise((resolve) => { setTimeout(resolve); }); });
      expect(text().textContent).toBe(`${proposal.candidate_text}!zhong`);
      await applyTrustedInput({ operation: "insert_text", text: "中" });
      await act(async () => { await new Promise((resolve) => { setTimeout(resolve); }); });
      expect({ text: text().textContent, failures: view.failures })
        .toEqual({ text: `${proposal.candidate_text}!中`, failures: [] });
    } finally {
      await view.unmount(); await test.close();
    }
  });
});

it("keeps the caret in place when only an inline Proposal changes its attributes", () => {
  const host = document.createElement("div"); document.body.append(host);
  const editor = new Editor({ element: host, extensions: storyosManuscriptExtensions(BLOCK),
    content: manuscriptJson(BLOCK, "Base"), injectCSS: false });
  try {
    const id = proposalFixture.proposal.proposal_id;
    const projection: BlockProposalProjection = { proposalId: id, operationId: id, revisionId: id, blockId: BLOCK,
      sourceRunId: id, sourceDecisionId: id, text: "AS", eligible: true, expectedHeads: [id],
      inlineProposal: true, inlineAnchor: { from: 1, to: 3, sourceText: "as" } };
    projectBlockProposals(editor, [projection]);
    editor.commands.setTextSelection(1);
    const before = { text: editor.state.doc.textContent, selection: editor.state.selection.toJSON() };
    projectBlockProposals(editor, [{ ...projection, eligible: false }]);
    expect({ text: editor.state.doc.textContent, selection: editor.state.selection.toJSON() }).toEqual(before);
  } finally {
    editor.destroy(); host.remove();
  }
});
