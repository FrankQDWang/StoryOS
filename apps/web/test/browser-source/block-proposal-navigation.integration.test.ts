import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import { BlockProposalDisplay, type ProposalLocator } from "../../src/block-proposal-display.tsx";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { jsonResponse } from "./scenario.ts";

it("keeps the editor editable when a Proposal navigation only reorders the known locations", async () => {
  const test = await openJournalAppendTestWorkspace();
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  const { workspace } = test;
  const scope = workspace.partition.project_scope;
  const locators: ProposalLocator[] = ["018f0000-0000-7001-8000-000000000c01", "018f0000-0000-7001-8000-000000000c02"]
    .map((proposalId) => ({ proposalId, runId: proposalFixture.proposal.source.run_id,
      decisionId: proposalFixture.proposal.source.decision_id }));
  const reads: string[] = [];
  let readsSettled!: () => void;
  let settled = new Promise<void>((resolve) => { readsSettled = resolve; });
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const proposalId = path.split("/").at(-1)!;
    if (!locators.some((locator) => locator.proposalId === proposalId)) throw new Error(`Unexpected request ${path}`);
    reads.push(proposalId);
    if (reads.length % locators.length === 0) readsSettled();
    return jsonResponse({ ...proposalFixture, project_scope: scope,
      proposal: { ...proposalFixture.proposal, proposal_id: proposalId } });
  };
  const props = { scope, chapterId: workspace.session.base_snapshot.chapter_id,
    authoritativeRevisionId: workspace.pending.authoritative_revision_id, refreshKey: 0, safeToProject: true,
    onAccepted: async () => {}, blocks: workspace.pending.blocks, editable: true, persistWorkspace: workspace,
    baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, controllerRef: { current: null },
    onProjection: () => {}, onFailure: (error: unknown) => { throw error; } };
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    // One task boundary lets the read chain finish its promise jobs before act flushes the update.
    const readsApplied = async () => { await settled; await new Promise((resolve) => { setTimeout(resolve); }); };
    await act(async () => { root.render(createElement(BlockProposalDisplay, { ...props, locators })); });
    await act(readsApplied);
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    await expect.poll(() => surface.getAttribute("contenteditable")).toBe("true");
    const transitions: (string | null)[] = [];
    const observer = new MutationObserver((records) => {
      transitions.push(...records.map((record) => (record.target as Element).getAttribute("contenteditable")));
    });
    observer.observe(surface, { attributes: true, attributeFilter: ["contenteditable"] });
    settled = new Promise<void>((resolve) => { readsSettled = resolve; });
    await act(async () => { root.render(createElement(BlockProposalDisplay, { ...props, locators: [...locators].reverse() })); });
    await act(readsApplied);
    await expect.poll(() => surface.getAttribute("contenteditable")).toBe("true");
    transitions.push(...observer.takeRecords().map((record) => (record.target as Element).getAttribute("contenteditable")));
    observer.disconnect();
    expect({ reads: reads.length, transitions }).toEqual({ reads: 4, transitions: [] });
  } finally {
    await act(async () => { root.unmount(); });
    host.remove(); await test.close(); Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
