import { act } from "react";
import { expect, it } from "vitest";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { candidateFor, proposalTransport, renderTab, withActEnvironment, type Proposal }
  from "./proposal-display-harness.ts";
import type { OpenProposalListAnswer } from "./scenario.ts";

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

it.each<[string, OpenProposalListAnswer, string]>([
  ["failed", "failed", "the Chapter Head"],
  ["truncated", { proposals: [], completeness: "truncated" }, "the Chapter Head"],
  ["failed", "failed", "an earlier base"],
  ["truncated", { proposals: [], completeness: "truncated" }, "an earlier base"],
])("keeps the decision controls unavailable when the open Proposal list is %s for a Proposal on %s", async (_, list, base) => {
  const test = await openJournalAppendTestWorkspace();
  const proposal = { ...candidateFor(test.workspace), ...(base === "an earlier base"
    ? { base_authoritative_revision_id: "018f0000-0000-7001-8000-000000000e01" } : {}) };
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
      expect({ eligibility: decoration()?.getAttribute("data-proposal-eligibility"),
        controls: decoration()?.querySelectorAll("button").length,
        note: decoration()?.querySelector("[data-proposal-earlier-base]") ?? null, failures: tab.failures })
        .toEqual({ eligibility: "ineligible", controls: 0, note: null, failures: [] });
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
