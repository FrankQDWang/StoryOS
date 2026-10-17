import { act, createElement, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import { BlockProposalDisplay, type ProposalLocator } from "../../src/block-proposal-display.tsx";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import type { EditorReadyState } from "../../src/editor-types.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { jsonResponse, withOpenProposalList, type OpenProposalListAnswer } from "./scenario.ts";

type Proposal = typeof proposalFixture.proposal;

function candidateFor(workspace: EditorReadyState): Proposal {
  const blockId = workspace.openedProjection.blocks[0]!.manuscript_block_id;
  return { ...proposalFixture.proposal, chapter_id: workspace.session.base_snapshot.chapter_id,
    manuscript_block_id: blockId, base_authoritative_revision_id: workspace.openedProjection.authoritative_revision_id,
    operations: proposalFixture.proposal.operations.map((operation) => ({ ...operation, manuscript_block_id: blockId })) };
}

// One tab of the author: the display gets only the locators of this tab and reads the rest from the transport.
async function renderTab(workspace: EditorReadyState, locators: ProposalLocator[], fetchImpl: typeof fetch) {
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  const failures: unknown[] = [];
  const writing = createEditorSessionWritingController({ workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failures.push(error); } });
  function View() {
    const { projection } = useSyncExternalStore(writing.subscribe, writing.snapshot);
    return createElement(BlockProposalDisplay, { scope: workspace.partition.project_scope,
      chapterId: workspace.session.base_snapshot.chapter_id, authoritativeRevisionId: projection.authoritative_revision_id,
      locators, refreshKey: 0, safeToProject: true, onAccepted: async () => {},
      blocks: projection.blocks, editable: true, persistWorkspace: workspace, writing, baseUrl: location.origin, fetchImpl,
      cryptoImpl: crypto, onFailure: (error) => { failures.push(error); } });
  }
  await act(async () => { root.render(createElement(View)); });
  return { host, failures,
    async unmount() { await act(async () => { root.unmount(); }); writing.close(); host.remove(); } };
}

async function withActEnvironment(action: () => Promise<void>) {
  const previous = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try { await action(); } finally { Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previous }); }
}

function proposalTransport(workspace: EditorReadyState, proposal: Proposal, list: OpenProposalListAnswer) {
  const reads: string[] = [];
  let listAnswers = 0;
  const answered = withOpenProposalList(async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (!path.endsWith(`/proposals/${proposal.proposal_id}`)) throw new Error(`No request handler: ${path}`);
    reads.push(proposal.proposal_id);
    return jsonResponse({ ...proposalFixture, project_scope: workspace.partition.project_scope, proposal });
  }, workspace, list);
  const fetchImpl: typeof fetch = async (input, init) => {
    const response = await answered(input, init);
    if (new URL(input instanceof Request ? input.url : input).pathname.endsWith("/open-proposals")) listAnswers += 1;
    return response;
  };
  return { fetchImpl, reads, listAnswers: () => listAnswers };
}

it("shows the Proposal of the open Proposal list in a second tab that has no locator", async () => {
  const test = await openJournalAppendTestWorkspace();
  const proposal = candidateFor(test.workspace);
  const transport = proposalTransport(test.workspace, proposal, { proposals: [proposal], completeness: "complete" });
  await withActEnvironment(async () => {
    const tab = await renderTab(test.workspace, [], transport.fetchImpl);
    try {
      await expect.poll(() => tab.host.querySelector(`.block-proposal[data-proposal-id="${proposal.proposal_id}"]`)
        ?.getAttribute("data-proposal-eligibility")).toBe("eligible");
      expect({ reads: [...new Set(transport.reads)], failures: tab.failures })
        .toEqual({ reads: [proposal.proposal_id], failures: [] });
    } finally {
      await tab.unmount(); await test.close();
    }
  });
});

it.each<[string, OpenProposalListAnswer]>([
  ["failed", "failed"],
  ["truncated", { proposals: [], completeness: "truncated" }],
])("keeps the decision controls unavailable when the open Proposal list is %s", async (_, list) => {
  const test = await openJournalAppendTestWorkspace();
  const proposal = candidateFor(test.workspace);
  const transport = proposalTransport(test.workspace, proposal, list);
  const locator = { proposalId: proposal.proposal_id, runId: proposal.source.run_id,
    decisionId: proposal.source.decision_id };
  await withActEnvironment(async () => {
    const tab = await renderTab(test.workspace, [locator], transport.fetchImpl);
    try {
      const decoration = () => tab.host.querySelector(`.block-proposal[data-proposal-id="${proposal.proposal_id}"]`);
      await expect.poll(() => decoration()?.getAttribute("data-proposal-eligibility")).toBe("ineligible");
      await expect.poll(() => tab.host.querySelector("[data-manuscript-editor]")?.getAttribute("contenteditable"))
        .toBe("true");
      await expect.poll(transport.listAnswers).toBeGreaterThan(0);
      await act(async () => {});
      expect({ eligibility: decoration()?.getAttribute("data-proposal-eligibility"), failures: tab.failures })
        .toEqual({ eligibility: "ineligible", failures: [] });
    } finally {
      await tab.unmount(); await test.close();
    }
  });
});

it("keeps the editor available for text input while the open Proposal list is still reading", async () => {
  const test = await openJournalAppendTestWorkspace();
  let releaseList = (): void => undefined;
  const listHeld = new Promise<Response>((resolve) => {
    releaseList = () => resolve(new Response(null, { status: 503 }));
  });
  let listReads = 0;
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (!path.endsWith("/open-proposals")) throw new Error(`No request handler: ${path}`);
    listReads += 1;
    return listHeld;
  };
  await withActEnvironment(async () => {
    const tab = await renderTab(test.workspace, [], fetchImpl);
    try {
      await expect.poll(() => listReads).toBeGreaterThan(0);
      await expect.poll(() => tab.host.querySelector("[data-manuscript-editor]")?.getAttribute("contenteditable"))
        .toBe("true");
      expect(tab.failures).toEqual([]);
    } finally {
      releaseList();
      await tab.unmount(); await test.close();
    }
  });
});

it("keeps text input available when the open Proposal list adds a Proposal after the editor is editable", async () => {
  const test = await openJournalAppendTestWorkspace();
  const proposal = candidateFor(test.workspace);
  const answered = proposalTransport(test.workspace, proposal, { proposals: [proposal], completeness: "complete" });
  let releaseList = (): void => undefined;
  const listReleased = new Promise<void>((resolve) => { releaseList = resolve; });
  const fetchImpl: typeof fetch = async (input, init) => {
    if (new URL(input instanceof Request ? input.url : input).pathname.endsWith("/open-proposals")) await listReleased;
    return answered.fetchImpl(input, init);
  };
  await withActEnvironment(async () => {
    const tab = await renderTab(test.workspace, [], fetchImpl);
    const editable: (string | null)[] = [];
    const observer = new MutationObserver(() => {
      editable.push(tab.host.querySelector("[data-manuscript-editor]")?.getAttribute("contenteditable") ?? null);
    });
    try {
      await expect.poll(() => tab.host.querySelector("[data-manuscript-editor]")?.getAttribute("contenteditable"))
        .toBe("true");
      observer.observe(tab.host, { attributes: true, attributeFilter: ["contenteditable"], subtree: true });
      releaseList();
      await expect.poll(() => tab.host.querySelector(`.block-proposal[data-proposal-id="${proposal.proposal_id}"]`)
        ?.getAttribute("data-proposal-eligibility")).toBe("eligible");
      expect({ editable: editable.filter((value) => value !== "true"), failures: tab.failures })
        .toEqual({ editable: [], failures: [] });
    } finally {
      observer.disconnect(); releaseList();
      await tab.unmount(); await test.close();
    }
  });
});

it("shows a withdrawn Proposal in no tab after the complete open Proposal list excludes it", async () => {
  const test = await openJournalAppendTestWorkspace();
  const open = candidateFor(test.workspace);
  const server: { proposal: Proposal; list: OpenProposalListAnswer } = {
    proposal: open, list: { proposals: [open], completeness: "complete" } };
  let listReads = 0;
  let proposalReads = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/open-proposals")) listReads += 1;
    else proposalReads += 1;
    return proposalTransport(test.workspace, server.proposal, server.list).fetchImpl(input, init);
  };
  const locator = { proposalId: open.proposal_id, runId: open.source.run_id, decisionId: open.source.decision_id };
  const decoration = (host: HTMLElement) =>
    host.querySelector(`.block-proposal[data-proposal-id="${open.proposal_id}"]`);
  await withActEnvironment(async () => {
    const before = await renderTab(test.workspace, [locator], fetchImpl);
    try {
      await expect.poll(() => decoration(before.host)?.getAttribute("data-proposal-eligibility")).toBe("eligible");
    } finally { await before.unmount(); }
    // Another tab withdraws the Proposal. Its Operation stays pending, and the open Proposal list excludes it.
    server.proposal = { ...open, closure: "withdrawn" };
    server.list = { proposals: [], completeness: "complete" };
    for (const locators of [[locator], []]) {
      listReads = 0;
      proposalReads = 0;
      const tab = await renderTab(test.workspace, locators, fetchImpl);
      try {
        await expect.poll(() => listReads).toBeGreaterThan(0);
        await expect.poll(() => proposalReads).toBeGreaterThanOrEqual(locators.length);
        await act(async () => { await new Promise((resolve) => setTimeout(resolve, 200)); });
        expect({ locators: locators.length, shown: decoration(tab.host) !== null, failures: tab.failures })
          .toEqual({ locators: locators.length, shown: false, failures: [] });
      } finally { await tab.unmount(); }
    }
  });
  await test.close();
});
