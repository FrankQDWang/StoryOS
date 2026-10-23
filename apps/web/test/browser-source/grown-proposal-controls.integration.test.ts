import { act } from "react";
import { expect, it } from "vitest";
import type { AcceptProposalRequest, RejectProposalOperationsRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import { openEditorWorkspace } from "../../src/editor-session.ts";
import { proposalTransport, renderTab, withActEnvironment, type Proposal } from "./proposal-display-harness.ts";
import { BLOCK, SESSION, createBrowserScenario, deleteJournal, jsonResponse, requireEditorReady } from "./scenario.ts";

const SECOND_BLOCK = "018f0000-0000-7001-8000-0000000000b2";
const JOINED_OPERATION = "018f0000-0000-7001-8000-000000000b09";

// A Chapter with two Blocks, and the writer workspace of its editor session.
async function openTwoBlockWorkspace() {
  const scenario = createBrowserScenario();
  const revision = { ...scenario.chapter.chapter.current_revision, body: "Base\nSecond", blocks: [
    { manuscript_block_id: BLOCK, block_kind: "paragraph" as const, text: "Base" },
    { manuscript_block_id: SECOND_BLOCK, block_kind: "paragraph" as const, text: "Second" },
  ] };
  scenario.chapter.chapter.current_revision = revision;
  scenario.session.base_snapshot.materialized_revision = revision;
  scenario.session.base_snapshot.materialized_payload_digest.value_hex_lowercase = [...new Uint8Array(
    await crypto.subtle.digest("SHA-256", new TextEncoder().encode(revision.body)))]
    .map((n) => n.toString(16).padStart(2, "0")).join("");
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-12-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    throw new Error(`Unexpected request ${path}`);
  };
  await deleteJournal(scenario.journalName);
  const workspace = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: scenario.chapter, profile: scenario.profile, fetchImpl });
  requireEditorReady(workspace);
  return { workspace, async close() { workspace.database.close(); await deleteJournal(scenario.journalName); } };
}

// ADR 0071: a grown Proposal has one Operation on each of the two Blocks and the atomic bundle policy.
function grownProposal(workspace: Awaited<ReturnType<typeof openTwoBlockWorkspace>>["workspace"]): Proposal {
  const [first] = proposalFixture.proposal.operations;
  return { ...proposalFixture.proposal, chapter_id: workspace.session.base_snapshot.chapter_id, manuscript_block_id: BLOCK,
    base_authoritative_revision_id: workspace.openedProjection.authoritative_revision_id, bundle_policy: "atomic",
    operations: [{ ...first!, manuscript_block_id: BLOCK },
      { ...first!, operation_id: JOINED_OPERATION, manuscript_block_id: SECOND_BLOCK, candidate_text: "Second, joined." }] };
}

it.each([
  ["accept", "/acceptances", (body: string) =>
    (JSON.parse(body) as AcceptProposalRequest).accept_proposal_input.selected_operation_ids],
  ["reject", "/rejections", (body: string) =>
    (JSON.parse(body) as RejectProposalOperationsRequest).reject_proposal_operations_input.selected_pending_operation_ids],
] as const)("shows each part of a grown Proposal at its Block, and %s on one part selects all parts", async (control, route, selected) => {
  const test = await openTwoBlockWorkspace();
  const proposal = grownProposal(test.workspace);
  const decisions: string[] = [];
  const transport = proposalTransport(test.workspace, proposal, { proposals: [proposal], completeness: "complete" },
    async (input, init) => {
      const path = new URL(input instanceof Request ? input.url : input).pathname;
      if (path.endsWith("/anti-forgery-challenges")) {
        return jsonResponse({ nonce: "b".repeat(64), expires_at: "2026-12-13T08:05:00.000Z" });
      }
      if (path.endsWith(route)) decisions.push(String(init?.body));
      return jsonResponse({ code: "service_unavailable" }, 503);
    });
  await withActEnvironment(async () => {
    const tab = await renderTab(test.workspace, [{ proposalId: proposal.proposal_id, runId: proposal.source.run_id,
      decisionId: proposal.source.decision_id }], transport.fetchImpl);
    const parts = () => [...tab.host.querySelectorAll<HTMLElement>(`.block-proposal[data-proposal-id="${proposal.proposal_id}"]`)];
    try {
      await expect.poll(() => parts().map((part) => part.getAttribute("data-proposal-eligibility")))
        .toEqual(["eligible", "eligible"]);
      expect(parts().map((part) => ({ block: part.dataset.proposalTargetId, text: part.querySelector(".block-proposal-text")?.textContent })))
        .toEqual(proposal.operations.map((operation) => ({ block: operation.manuscript_block_id, text: operation.candidate_text })));
      await act(async () => { parts()[1]!.querySelector<HTMLButtonElement>(`button[data-proposal-${control}]`)!.click(); });
      await expect.poll(() => decisions.length).toBe(1);
      expect(selected(decisions[0]!)).toEqual(proposal.operations.map((operation) => operation.operation_id).sort());
    } finally {
      await tab.unmount(); await test.close();
    }
  });
});
