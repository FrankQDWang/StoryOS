import { act, createElement, useState, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import acceptFixture from "../../../../generated/golden-wire/storyos-public-release-1/accept-proposal.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import treeFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-manuscript-tree.json";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import rejectFixture from "../../../../generated/golden-wire/storyos-public-release-1/reject-proposal-operations.boundary.json";
import { digestAcceptProposal, digestRejectProposalOperations }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, RejectProposalOperationsRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BlockProposalDisplay } from "../../src/block-proposal-display.tsx";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import type { EditorReadyState } from "../../src/editor-types.ts";
import type { ProposalFocus } from "../../src/proposal-navigation.ts";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { jsonResponse } from "./scenario.ts";

type Proposal = typeof proposalFixture.proposal;

function candidateFor(workspace: EditorReadyState): Proposal {
  const blockId = workspace.openedProjection.blocks[0]!.manuscript_block_id;
  return { ...proposalFixture.proposal, chapter_id: workspace.session.base_snapshot.chapter_id,
    manuscript_block_id: blockId, base_authoritative_revision_id: workspace.openedProjection.authoritative_revision_id,
    operations: proposalFixture.proposal.operations.map((operation) => ({ ...operation, manuscript_block_id: blockId })) };
}

function resolved(proposal: Proposal, resolution: "rejected" | "applied"): Proposal {
  return { ...proposal, operation_resolution: resolution, reservation_state: "resolved",
    operations: proposal.operations.map((operation) => ({ ...operation, resolution, reservation_state: "resolved" })) };
}

/** The Server of the panel. `proposal` gives the current Proposal read, and `runs` records each Run request body. */
function assistantFetch(workspace: EditorReadyState, proposal: () => Proposal, runs: unknown[],
  decision: (path: string, init: RequestInit | undefined) => Promise<Response | undefined>): typeof fetch {
  const scope = workspace.partition.project_scope;
  return async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const answer = await decision(path, init);
    if (answer !== undefined) return answer;
    if (path.endsWith(`/proposals/${proposal().proposal_id}`)) {
      return jsonResponse({ ...proposalFixture, project_scope: scope, proposal: proposal() });
    }
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith("/manuscript/tree")) {
      return jsonResponse({ ...treeFixture, project_scope: scope, snapshot: { ...treeFixture.snapshot, project_scope: scope } });
    }
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse({ nonce: "a".repeat(64),
      expires_at: new Date(Date.now() + 60_000).toISOString(), limit_profile_revision: "storyos.foundation.absolute.v1" });
    if (path.includes("/editor-sessions/")) {
      return jsonResponse({ ...workspace.session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (path.endsWith("/agent-runs") && init?.method === "POST") {
      runs.push(JSON.parse(String(init.body)));
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "conflict", message: "Not admitted." }, 409);
    }
    throw new Error(`No request handler: ${path}`);
  };
}

async function renderWorkspace(workspace: EditorReadyState, fetchImpl: typeof fetch, proposal: Proposal,
  candidateTarget?: ProposalFocus) {
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  const writing = createEditorSessionWritingController({ workspace, baseUrl: location.origin, fetchImpl,
    onFailure: () => {} });
  const scope = workspace.partition.project_scope;
  const chapterId = workspace.session.base_snapshot.chapter_id;
  // The candidate focus state of stage1-view: the editor sets it, and the panel reads it.
  function View() {
    const { projection } = useSyncExternalStore(writing.subscribe, writing.snapshot);
    const [target, setTarget] = useState<ProposalFocus | undefined>(candidateTarget);
    return createElement("div", { "data-candidate-target": target?.proposalId ?? "" },
      createElement(BlockProposalDisplay, { scope, chapterId, authoritativeRevisionId: projection.authoritative_revision_id,
        locators: [{ proposalId: proposal.proposal_id, runId: proposal.source.run_id, decisionId: proposal.source.decision_id }],
        refreshKey: 0, safeToProject: true, onAccepted: async () => {}, onCandidateFocus: setTarget,
        blocks: projection.blocks, editable: true, persistWorkspace: workspace, writing, baseUrl: location.origin, fetchImpl,
        cryptoImpl: crypto, onFailure: () => {} }),
      createElement(WritingAssistantPanel, { collapsed: false, scheduleRefresh: () => () => {},
        context: { scope, chapterId, candidateTarget: target, canSubmit: true, baseUrl: location.origin, fetchImpl,
          cryptoImpl: crypto } }));
  }
  await act(async () => { root.render(createElement(View)); });
  return { host, focus: () => host.firstElementChild!.getAttribute("data-candidate-target"),
    async send(message: string) {
      const textarea = host.querySelector<HTMLTextAreaElement>("[data-writing-assistant-composer] textarea")!;
      await expect.poll(() => host.querySelector("[data-writing-assistant]")?.getAttribute("data-assistant-availability"))
        .toBe("available");
      await act(async () => {
        Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(textarea, message);
        textarea.dispatchEvent(new Event("input", { bubbles: true }));
      });
      await act(async () => { host.querySelector<HTMLButtonElement>(".send-button")!.click(); });
      await expect.poll(() => host.querySelector("[data-writing-assistant]")?.getAttribute("data-assistant-dispatch"))
        .not.toBe("sending");
    },
    async unmount() {
      await act(async () => { root.unmount(); }); writing.close(); host.remove(); sessionStorage.clear();
    } };
}

async function withActEnvironment(action: () => Promise<void>) {
  const previous = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try { await action(); } finally { Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previous }); }
}

it.each([
  ["rejects", "button[data-proposal-reject]"],
  ["accepts", "button[data-proposal-accept]"],
] as const)("sends the next request to the Current Chapter after the author %s the focused candidate", async (_, button) => {
  const test = await openJournalAppendTestWorkspace();
  const { workspace } = test;
  const pending = candidateFor(workspace);
  let proposal = pending;
  const runs: unknown[] = [];
  const fetchImpl = assistantFetch(workspace, () => proposal, runs, async (path, init) => {
    const headers = new Headers(init?.headers);
    if (path.endsWith("/rejections")) {
      const request = JSON.parse(String(init?.body)) as RejectProposalOperationsRequest;
      const input = request.reject_proposal_operations_input;
      proposal = resolved(pending, "rejected");
      return jsonResponse({ ...rejectFixture, correlation_id: input.correlation_id,
        project_scope: workspace.partition.project_scope,
        receipt: { ...rejectFixture.receipt, project_scope: workspace.partition.project_scope,
          proposal_id: pending.proposal_id, proposal_revision_id: input.proposal_revision_id,
          selected_pending_operation_ids: input.selected_pending_operation_ids,
          expected_target_revisions: input.expected_target_revisions,
          idempotency_key: headers.get("idempotency-key"),
          command_digest: await digestRejectProposalOperations(request, crypto) } });
    }
    if (path.endsWith("/acceptances")) {
      const request = JSON.parse(String(init?.body)) as AcceptProposalRequest;
      const input = request.accept_proposal_input;
      proposal = resolved(pending, "applied");
      return jsonResponse({ ...acceptFixture, correlation_id: input.correlation_id,
        project_scope: workspace.partition.project_scope,
        receipt: { ...acceptFixture.receipt, project_scope: workspace.partition.project_scope,
          proposal_id: pending.proposal_id, proposal_revision_id: input.proposal_revision_id,
          validation_receipt_id: input.validation_receipt_id, selected_operation_ids: input.selected_operation_ids,
          idempotency_key: headers.get("idempotency-key"),
          command_digest: await digestAcceptProposal(request, crypto) } });
    }
    return undefined;
  });
  await withActEnvironment(async () => {
    const view = await renderWorkspace(workspace, fetchImpl, pending);
    try {
      const candidate = () => view.host.querySelector<HTMLElement>(`[data-candidate-proposal-id="${pending.proposal_id}"]`);
      await expect.poll(() => candidate()?.querySelector<HTMLButtonElement>(button)?.disabled).toBe(false);
      await act(async () => { candidate()!.querySelector<HTMLElement>(".block-proposal-text")!.click(); });
      expect(view.focus()).toBe(pending.proposal_id);

      await act(async () => { candidate()!.querySelector<HTMLButtonElement>(button)!.click(); });
      // The resolved candidate leaves the editor after the next Proposal read.
      await expect.poll(candidate).toBeNull();
      await expect.poll(view.focus).toBe("");
      await view.send("Tighten this chapter.");

      expect(runs.map((run) => (run as { create_agent_run_input: { working_target: unknown } })
        .create_agent_run_input.working_target))
        .toEqual([{ kind: "current_chapter", chapter_id: workspace.session.base_snapshot.chapter_id }]);
    } finally {
      await view.unmount(); await test.close();
    }
  });
});

it.each([
  ["a resolved candidate goes to the Current Chapter", "rejected", "current_chapter",
    "本次请求未被接收。请检查当前章节和写作助手状态。"],
  ["a moved candidate shows one sentence", "moved", undefined, "候选位置已变化。请在正文中重新选择候选，然后再发送。"],
] as const)("checks a stale candidate target at submit: %s", async (_, change, target, status) => {
  const test = await openJournalAppendTestWorkspace();
  const { workspace } = test;
  const pending = candidateFor(workspace);
  const proposal = change === "rejected" ? resolved(pending, "rejected")
    : { ...pending, operations: pending.operations.map((operation) => ({ ...operation,
      manuscript_block_id: "018f0000-0000-7001-8000-0000000000b9" })) };
  const runs: unknown[] = [];
  const fetchImpl = assistantFetch(workspace, () => proposal, runs, async () => undefined);
  await withActEnvironment(async () => {
    const view = await renderWorkspace(workspace, fetchImpl, pending, { proposalId: pending.proposal_id,
      operationId: pending.operation_id, revisionId: pending.revision_id, blockId: pending.manuscript_block_id });
    try {
      await view.send("Tighten this chapter.");
      expect({
        targets: runs.map((run) => (run as { create_agent_run_input: { working_target: { kind: string } } })
          .create_agent_run_input.working_target.kind),
        status: view.host.querySelector("[data-writing-assistant] [role=status]")?.textContent,
      }).toEqual({ targets: target === undefined ? [] : [target], status });
    } finally {
      await view.unmount(); await test.close();
    }
  });
});
