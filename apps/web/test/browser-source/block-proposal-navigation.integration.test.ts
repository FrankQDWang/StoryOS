import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import { BlockProposalDisplay, type ProposalLocator } from "../../src/block-proposal-display.tsx";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
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
  const settled = new Promise<void>((resolve) => { readsSettled = resolve; });
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const proposalId = path.split("/").at(-1)!;
    if (!locators.some((locator) => locator.proposalId === proposalId)) throw new Error(`Unexpected request ${path}`);
    reads.push(proposalId);
    if (reads.length % locators.length === 0) readsSettled();
    return jsonResponse({ ...proposalFixture, project_scope: scope,
      proposal: { ...proposalFixture.proposal, proposal_id: proposalId } });
  };
  const writing = createEditorSessionWritingController({ workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { throw error; } });
  const props = { scope, chapterId: workspace.session.base_snapshot.chapter_id,
    authoritativeRevisionId: workspace.openedProjection.authoritative_revision_id, refreshKey: 0, safeToProject: true,
    onAccepted: async () => {}, blocks: workspace.openedProjection.blocks, editable: true, persistWorkspace: workspace, writing,
    baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
    onFailure: (error: unknown) => { throw error; } };
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(BlockProposalDisplay, { ...props, locators })); });
    // The editor accepts input after the reads of the Proposals and the Acceptance check.
    await act(async () => { await settled; });
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    await expect.poll(() => surface.getAttribute("contenteditable")).toBe("true");
    const transitions: (string | null)[] = [];
    const observer = new MutationObserver((records) => {
      transitions.push(...records.map((record) => (record.target as Element).getAttribute("contenteditable")));
    });
    observer.observe(surface, { attributes: true, attributeFilter: ["contenteditable"] });
    const before = { projection: writing.snapshot().projection, text: surface.textContent };
    // The same refresh has a read of each Proposal, so a new order of the locators reads nothing again (#1937). The
    // display starts each Proposal read in its effect, and act runs that effect, so the count is final after act.
    await act(async () => { root.render(createElement(BlockProposalDisplay, { ...props, locators: [...locators].reverse() })); });
    const reordered = reads.length;
    await expect.poll(() => surface.getAttribute("contenteditable")).toBe("true");
    transitions.push(...observer.takeRecords().map((record) => (record.target as Element).getAttribute("contenteditable")));
    observer.disconnect();
    expect({ reads: reordered, transitions, projection: writing.snapshot().projection, text: surface.textContent })
      .toEqual({ reads: 2, transitions: [], ...before });
  } finally {
    await act(async () => { root.unmount(); });
    writing.close(); host.remove(); await test.close(); Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});

it("reads a Proposal again one time when its read failed and its locator loses its Run, also after a new order",
  async () => {
    const test = await openJournalAppendTestWorkspace();
    const host = document.createElement("div"); document.body.append(host);
    const root = createRoot(host);
    const { workspace } = test;
    const scope = workspace.partition.project_scope;
    const chapterId = workspace.session.base_snapshot.chapter_id;
    const [first, second] = ["018f0000-0000-7001-8000-000000000c01", "018f0000-0000-7001-8000-000000000c02"]
      .map((proposalId) => ({ proposalId, runId: proposalFixture.proposal.source.run_id,
        decisionId: proposalFixture.proposal.source.decision_id })) as [ProposalLocator, ProposalLocator];
    const reads: string[] = [];
    const unexpected: string[] = [];
    let releaseFailure!: () => void;
    const failure = new Promise<void>((resolve) => { releaseFailure = resolve; });
    // The fake serves the Proposal reads and the list of the Chapter. The first read of `first` fails after the test
    // releases it.
    const fetchImpl: typeof fetch = async (input, init) => {
      const request = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
      const proposalId = request.split("/").at(-1)!;
      const project = `/api/v1/projects/${scope.project_id}`;
      if (request === `GET ${project}/chapters/${chapterId}/open-proposals`) {
        return jsonResponse({ schema_id: "storyos.query.chapter-open-proposals.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000c71", project_scope: scope, chapter_id: chapterId,
          completeness: "complete", proposals: [] });
      }
      if (![first, second].some((locator) => request === `GET ${project}/proposals/${locator.proposalId}`)) {
        unexpected.push(request);
        throw new Error(`Unexpected request ${request}`);
      }
      reads.push(proposalId);
      if (proposalId === first.proposalId && reads.length === 1) {
        await failure;
        return jsonResponse({ schema_id: "storyos.problem.v1", code: "service_unavailable", message: "Unavailable." },
          503);
      }
      return jsonResponse({ ...proposalFixture, project_scope: scope,
        proposal: { ...proposalFixture.proposal, proposal_id: proposalId } });
    };
    const writing = createEditorSessionWritingController({ workspace, baseUrl: location.origin, fetchImpl,
      onFailure: (error) => { throw error; } });
    const props = { scope, chapterId, authoritativeRevisionId: workspace.openedProjection.authoritative_revision_id,
      refreshKey: 0, safeToProject: true, onAccepted: async () => {}, blocks: workspace.openedProjection.blocks,
      editable: true, persistWorkspace: workspace, writing, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
      onFailure: (error: unknown) => { throw error; } };
    const show = (locators: ProposalLocator[]) => act(async () => {
      root.render(createElement(BlockProposalDisplay, { ...props, locators }));
    });
    const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
    try {
      await show([first, second]);
      // The Journal then gives `first` with no Run, and a navigation gives a new order, while the first read waits.
      const noRun = { proposalId: first.proposalId, runId: "", decisionId: "" };
      await show([noRun, second]);
      await show([second, noRun]);
      const started = [...reads];

      releaseFailure();

      // Each locator with no Run uses the same retry of the failed read.
      await expect.poll(() => reads.length).toBe(3);
      await act(async () => { await failure; });
      expect({ started, reads, unexpected }).toEqual({ started: [first.proposalId, second.proposalId],
        reads: [first.proposalId, second.proposalId, first.proposalId], unexpected: [] });
    } finally {
      await act(async () => { root.unmount(); });
      writing.close(); host.remove(); await test.close();
      Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
    }
  });
