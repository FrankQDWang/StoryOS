import { Editor } from "@tiptap/core";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import { projectBlockProposals, type BlockProposalProjection } from "../../src/block-proposal-decoration.ts";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import { ManuscriptEditor } from "../../src/manuscript-editor.tsx";
import { manuscriptJson } from "../../src/manuscript-doc.ts";
import { storyosManuscriptExtensions } from "../../src/manuscript-tiptap-adapter.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { BLOCK } from "./scenario.ts";

const id = proposalFixture.proposal.proposal_id;

function inlineCandidate(blockId: string, controls: Partial<BlockProposalProjection>): BlockProposalProjection {
  return { proposalId: id, operationId: id, revisionId: id, blockId, sourceRunId: id, sourceDecisionId: id,
    text: "AS", eligible: false, expectedHeads: [id], inlineProposal: true,
    inlineAnchor: { from: 1, to: 3, sourceText: "as" }, ...controls };
}

function shown(host: HTMLElement) {
  return { paragraph: host.querySelector("p")?.textContent,
    inline: host.querySelector("[data-inline-proposal-id]")?.textContent ?? null,
    candidate: host.querySelector(".block-proposal-text")?.textContent,
    accept: host.querySelectorAll("button[data-proposal-accept]").length };
}

it("projects an Inline candidate and its Accept control only when the Block text agrees with the anchor", () => {
  const results = ["Base", "Bx", "", "Bxse"].flatMap((body) =>
    [{ eligible: true }, { retryPending: true }].map((controls) => {
      const host = document.createElement("div"); document.body.append(host);
      const editor = new Editor({ element: host, extensions: storyosManuscriptExtensions(BLOCK),
        content: manuscriptJson(BLOCK, body), injectCSS: false });
      try {
        projectBlockProposals(editor, [inlineCandidate(BLOCK, controls)]);
        return shown(host);
      } finally {
        editor.destroy(); host.remove();
      }
    }));
  const unchanged = (paragraph: string) => [1, 2].map(() => ({ paragraph, inline: null, candidate: "AS", accept: 0 }));
  expect(results).toEqual([
    { paragraph: "BASe", inline: "AS", candidate: "", accept: 1 },
    { paragraph: "BASe", inline: "AS", candidate: "", accept: 1 },
    ...unchanged("Bx"), ...unchanged(""), ...unchanged("Bxse"),
  ]);
});

it("keeps the editor usable when a newer Proposal list arrives with newer Blocks and shows the candidate after the hydrate", async () => {
  const test = await openJournalAppendTestWorkspace();
  const blockId = test.workspace.openedProjection.blocks[0]!.manuscript_block_id;
  const failures: unknown[] = [];
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin,
    fetchImpl: () => new Promise<Response>(() => {}), onFailure: (error) => { failures.push(error); } });
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(ManuscriptEditor, {
      blocks: [{ manuscript_block_id: blockId, text: "" }], editable: true, writing })); });
    // The projection effect runs before the hydrate effect, so it first meets the older empty Block.
    await act(async () => { root.render(createElement(ManuscriptEditor, {
      blocks: [{ manuscript_block_id: blockId, text: "Base" }],
      proposals: [inlineCandidate(blockId, { eligible: true })], editable: true, writing })); });
    expect({ ...shown(host), editable: host.querySelector("[data-manuscript-editor]")?.getAttribute("contenteditable"),
      failures }).toEqual({ paragraph: "BASe", inline: "AS", candidate: "", accept: 1, editable: "true", failures: [] });
  } finally {
    await act(async () => { root.unmount(); });
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
    writing.close(); host.remove(); await test.close();
  }
});
