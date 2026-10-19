import { act } from "react";
import { expect, it } from "vitest";
import { digestAcceptProposal, digestRejectProposalOperations }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, RejectProposalOperationsRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorReadyState } from "../../src/editor-types.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { candidateFor, proposalTransport, renderTab, withActEnvironment, type Proposal }
  from "./proposal-display-harness.ts";
import { jsonResponse, requestResult, withOpenProposalList, type OpenProposalListAnswer } from "./scenario.ts";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";

const EARLIER_BASE = "018f0000-0000-7001-8000-000000000e01";
const CONDITION = "018f0000-0000-7001-8000-000000000e02";
const NOTE = "正文已变化。接受会用候选文字替换目标的当前文字。";

type Sent = { path: string; body: string };

// The display of one Proposal with a controlled transport. Each request that is not a read is recorded.
async function display(workspace: EditorReadyState, proposal: Proposal, other?: typeof fetch) {
  const sent: Sent[] = [];
  const transport = proposalTransport(workspace, proposal, { proposals: [proposal], completeness: "complete" },
    async (input, init) => {
      const path = new URL(input instanceof Request ? input.url : input).pathname;
      sent.push({ path, body: String(init?.body ?? "") });
      return other?.(input, init) ?? jsonResponse({ code: "service_unavailable" }, 503);
    });
  const locator = { proposalId: proposal.proposal_id, runId: proposal.source.run_id,
    decisionId: proposal.source.decision_id };
  const tab = await renderTab(workspace, [locator], transport.fetchImpl);
  const card = () => tab.host.querySelector<HTMLElement>(`.block-proposal[data-proposal-id="${proposal.proposal_id}"]`);
  const controls = () => ({
    accept: card()?.querySelectorAll("button[data-proposal-accept]").length,
    reject: card()?.querySelectorAll("button[data-proposal-reject]").length ?? 0,
    withdraw: card()?.querySelectorAll("button[data-proposal-withdraw]").length,
    replan: tab.host.querySelectorAll("[data-proposal-replan]").length,
    editable: card()?.getAttribute("contenteditable") !== "false",
    note: card()?.querySelector("[data-proposal-earlier-base]")?.textContent ?? null,
  });
  return { tab, sent, card, controls };
}

function withCondition(proposal: Proposal, kind: "proposal_conflict" | "proposal_recovery_conflict"): Proposal {
  const ref = kind === "proposal_conflict" ? "proposal_conflict_ref" : "proposal_recovery_conflict_ref";
  return { ...proposal, base_authoritative_revision_id: EARLIER_BASE, condition_refs: [CONDITION],
    source_condition: { kind, [ref]: CONDITION } } as Proposal;
}

it("shows Accept, Reject, and candidate editing and no Replan control for a Proposal with a Proposal Conflict condition", async () => {
  const test = await openJournalAppendTestWorkspace();
  const proposal = withCondition(candidateFor(test.workspace), "proposal_conflict");
  await withActEnvironment(async () => {
    const shown = await display(test.workspace, proposal);
    try {
      await expect.poll(() => shown.card()?.getAttribute("data-proposal-eligibility")).toBe("eligible");
      expect({ ...shown.controls(), failures: shown.tab.failures }).toEqual({ accept: 1, reject: 1, withdraw: 0,
        replan: 0, editable: true, note: NOTE, failures: [] });
    } finally {
      await shown.tab.unmount(); await test.close();
    }
  });
});

it("accepts a Block Proposal on an earlier base with one click and keeps Accept and Reject after a deleted target refuses it", async () => {
  const test = await openJournalAppendTestWorkspace();
  const proposal = { ...candidateFor(test.workspace), base_authoritative_revision_id: EARLIER_BASE };
  const workspace = test.workspace;
  const accept: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({ nonce: "b".repeat(64), expires_at: "2026-12-13T08:05:00.000Z" });
    }
    if (!path.endsWith("/acceptances")) return jsonResponse({ code: "service_unavailable" }, 503);
    const request = JSON.parse(String(init?.body)) as AcceptProposalRequest;
    const input_ = request.accept_proposal_input;
    return jsonResponse({ schema_id: "storyos.command.accept-proposal.response.v1",
      correlation_id: input_.correlation_id, project_scope: workspace.partition.project_scope,
      command_id: "018f0000-0000-7001-8000-000000000e05", author_command_admission_id: "018f0000-0000-7001-8000-000000000e06",
      receipt: { receipt_id: "018f0000-0000-7001-8000-000000000e07", project_scope: workspace.partition.project_scope,
        command_digest: await digestAcceptProposal(request, crypto),
        idempotency_key: new Headers(init?.headers).get("idempotency-key") ?? "",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000e06", proposal_id: proposal.proposal_id,
        proposal_revision_id: input_.proposal_revision_id, validation_receipt_id: input_.validation_receipt_id,
        selected_operation_ids: input_.selected_operation_ids,
        prior_authoritative_revision_ids: [input_.expected_authoritative_revision_id],
        resulting_authoritative_revision_ids: [input_.expected_authoritative_revision_id],
        authoritative_commit_ids: [], condition_refs: [], result: "refused", created_at: "2026-10-18T08:00:00.000Z" },
      project: { project_id: workspace.partition.project_scope.project_id, title: "Project A",
        open: { kind: "current_chapter", current_chapter_id: workspace.session.base_snapshot.chapter_id } },
      effect: { kind: "refused", reason: "missing_target" } });
  };
  const confirmed: string[] = [];
  const previousConfirm = window.confirm;
  window.confirm = (message) => { confirmed.push(String(message)); return true; };
  await withActEnvironment(async () => {
    const shown = await display(workspace, proposal, accept);
    try {
      await expect.poll(() => shown.card()?.getAttribute("data-proposal-eligibility")).toBe("eligible");
      expect(shown.controls()).toEqual({ accept: 1, reject: 1, withdraw: 0, replan: 0, editable: true, note: NOTE });
      await act(async () => { shown.card()?.querySelector<HTMLButtonElement>("button[data-proposal-accept]")?.click(); });
      await expect.poll(() => shown.sent.filter(({ path }) => path.endsWith("/acceptances")).length).toBe(1);
      await expect.poll(() => shown.controls().reject).toBe(1);
      expect({ confirmed, dialogs: document.querySelectorAll("dialog, [role=dialog], [role=alertdialog]").length,
        accept: shown.controls().accept, failures: shown.tab.failures })
        .toEqual({ confirmed: [], dialogs: 0, accept: 1, failures: [] });
    } finally {
      window.confirm = previousConfirm;
      await shown.tab.unmount(); await test.close();
    }
  });
});

it("shows an Inline Proposal on an earlier base in its candidate card with Accept, Reject, candidate editing, and the note", async () => {
  const test = await openJournalAppendTestWorkspace();
  const block = test.workspace.openedProjection.blocks[0]!;
  const base = candidateFor(test.workspace);
  const proposal = { ...base, kind: "inline_edit", base_authoritative_revision_id: EARLIER_BASE, candidate_text: "AS",
    operations: base.operations.map((operation) => ({ ...operation, candidate_text: "AS" })),
    anchors: [{ manuscript_block_id: block.manuscript_block_id, base_authoritative_revision_id: EARLIER_BASE,
      manuscript_schema_version: 1, coordinate_profile: "prosemirror-token-utf16.v1", from: 0, to: 1,
      boundary_profile: "exclusive-authoritative-edges.v1", base_slice_digest: `sha256:${"0".repeat(64)}` }] } as Proposal;
  await withActEnvironment(async () => {
    const shown = await display(test.workspace, proposal);
    try {
      await expect.poll(() => shown.card()?.getAttribute("data-proposal-eligibility")).toBe("eligible");
      expect({ ...shown.controls(), candidate: shown.card()?.querySelector(".block-proposal-text")?.textContent,
        inline: shown.tab.host.querySelectorAll("[data-inline-proposal-id]").length, failures: shown.tab.failures })
        .toEqual({ accept: 1, reject: 1, withdraw: 0, replan: 0, editable: true, note: NOTE, candidate: "AS",
          inline: 0, failures: [] });
    } finally {
      await shown.tab.unmount(); await test.close();
    }
  });
});

it("drops a stored Replan entry of an earlier page at load and sends no request", async () => {
  const test = await openJournalAppendTestWorkspace();
  const workspace = test.workspace;
  const proposal = withCondition(candidateFor(workspace), "proposal_conflict");
  const key = `replan:${workspace.partition.journal_partition_id}:${proposal.proposal_id}:${proposal.revision_id}`;
  const store = (mode: IDBTransactionMode) => workspace.database.transaction("metadata", mode).objectStore("metadata");
  // The record of an earlier page whose Replan result is unknown.
  await requestResult(store("readwrite").put({ key, schema_id: "storyos.local-edit-journal.replan-proposal.v1",
    project_scope: workspace.partition.project_scope, proposal_id: proposal.proposal_id,
    revision_id: proposal.revision_id, journal_partition_id: workspace.partition.journal_partition_id,
    writer_generation: workspace.partition.writer_generation, idempotency_key: "018f0000-0000-7001-8000-000000000e03",
    nonce: "c".repeat(64), request: { command_schema: "storyos.command.replan-proposal.request.v1" },
    digest: { algorithm: "sha256", value_hex_lowercase: "0".repeat(64) }, settlement: "unresolved" }));
  await withActEnvironment(async () => {
    const shown = await display(workspace, proposal);
    try {
      await expect.poll(() => shown.controls().reject + shown.controls().replan).toBeGreaterThan(0);
      await act(async () => { await new Promise((resolve) => setTimeout(resolve, 200)); });
      expect({ stored: await requestResult(store("readonly").get(key)) !== undefined,
        replan: shown.controls().replan, reject: shown.controls().reject,
        retry: shown.tab.host.textContent?.includes("重新规划") ?? false,
        sent: shown.sent.map(({ path }) => path), failures: shown.tab.failures })
        .toEqual({ stored: false, replan: 0, reject: 1, retry: false, sent: [], failures: [] });
    } finally {
      await shown.tab.unmount(); await test.close();
    }
  });
});

it("shows Reject and Withdraw and no Accept or candidate editing for a Proposal Recovery Conflict", async () => {
  const test = await openJournalAppendTestWorkspace();
  const proposal = withCondition(candidateFor(test.workspace), "proposal_recovery_conflict");
  await withActEnvironment(async () => {
    const shown = await display(test.workspace, proposal);
    try {
      await expect.poll(() => shown.controls().withdraw).toBe(1);
      await act(async () => {});
      expect({ ...shown.controls(), failures: shown.tab.failures }).toEqual({ accept: 0, reject: 1, withdraw: 1,
        replan: 0, editable: false, note: null, failures: [] });
    } finally {
      await shown.tab.unmount(); await test.close();
    }
  });
});

it.each([["Block", "block_edit"], ["Inline", "inline_edit"]])("rejects an open %s Proposal whose target Block is not in the Chapter Head, with no Accept or candidate editing", async (_, kind) => {
  const test = await openJournalAppendTestWorkspace();
  const workspace = test.workspace;
  const deleted = "018f0000-0000-7001-8000-000000000e09";
  const base = candidateFor(workspace);
  const open = { ...base, kind, manuscript_block_id: deleted,
    operations: base.operations.map((operation) => ({ ...operation, manuscript_block_id: deleted })),
    anchors: kind === "inline_edit" ? [{ manuscript_block_id: deleted, base_authoritative_revision_id: base.base_authoritative_revision_id,
      manuscript_schema_version: 1, coordinate_profile: "prosemirror-token-utf16.v1", from: 0, to: 1,
      boundary_profile: "exclusive-authoritative-edges.v1", base_slice_digest: `sha256:${"0".repeat(64)}` }] : [] } as Proposal;
  const server: { proposal: Proposal; list: OpenProposalListAnswer } = {
    proposal: open, list: { proposals: [open], completeness: "complete" } };
  const rejections: RejectProposalOperationsRequest[] = [];
  const fetchImpl: typeof fetch = async (input, init) => withOpenProposalList(async (request, options) => {
    const path = new URL(request instanceof Request ? request.url : request).pathname;
    if (path.endsWith(`/proposals/${open.proposal_id}`)) {
      return jsonResponse({ ...proposalFixture, project_scope: workspace.partition.project_scope, proposal: server.proposal });
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({ nonce: "b".repeat(64), expires_at: "2026-12-13T08:05:00.000Z" });
    }
    if (!path.endsWith("/rejections")) throw new Error(`No request handler: ${path}`);
    const body = JSON.parse(String(options?.body)) as RejectProposalOperationsRequest;
    rejections.push(body);
    const value = body.reject_proposal_operations_input;
    const operationIds = value.selected_pending_operation_ids;
    server.proposal = { ...open, operation_resolution: "rejected",
      operations: open.operations.map((operation) => ({ ...operation, resolution: "rejected" })) };
    server.list = { proposals: [], completeness: "complete" };
    return jsonResponse({ schema_id: "storyos.command.reject-proposal-operations.response.v1",
      correlation_id: value.correlation_id, project_scope: workspace.partition.project_scope,
      command_id: "018f0000-0000-7001-8000-000000000e0a", author_command_admission_id: "018f0000-0000-7001-8000-000000000e0b",
      receipt: { receipt_id: "018f0000-0000-7001-8000-000000000e0c", project_scope: workspace.partition.project_scope,
        command_digest: await digestRejectProposalOperations(body, crypto),
        idempotency_key: new Headers(options?.headers).get("idempotency-key") ?? "",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000e0b", proposal_id: open.proposal_id,
        proposal_revision_id: value.proposal_revision_id, selected_pending_operation_ids: operationIds,
        expected_target_revisions: value.expected_target_revisions,
        prior_authoritative_revision_ids: value.expected_target_revisions,
        resulting_authoritative_revision_ids: value.expected_target_revisions, authoritative_commit_ids: [],
        result: "resolved", created_at: "2026-10-19T08:00:00.000Z" },
      project: { project_id: workspace.partition.project_scope.project_id, title: "Project A",
        open: { kind: "current_chapter", current_chapter_id: workspace.session.base_snapshot.chapter_id } },
      effect: { kind: "resolved", author_action_sequence: "1", undo_disposition: { kind: "not_undoable" },
        operation_ids: operationIds, prior_resolution: "pending", resulting_resolution: "rejected",
        rejection_reason: { kind: "author_declined", note: { kind: "omitted" } }, preserved_generation: "ready",
        preserved_validation: "valid", preserved_closure: "open", resolution_event_refs: [] } });
  }, workspace, server.list)(input, init);
  const locator = { proposalId: open.proposal_id, runId: open.source.run_id, decisionId: open.source.decision_id };
  await withActEnvironment(async () => {
    const tab = await renderTab(workspace, [locator], fetchImpl);
    const surface = () => tab.host.querySelector(`[data-proposal-unavailable="${open.proposal_id}"]`);
    try {
      await expect.poll(() => surface()?.querySelectorAll("button[data-proposal-reject]").length ?? 0).toBe(1);
      expect({ accept: tab.host.querySelectorAll("[data-proposal-accept]").length,
        candidate: tab.host.querySelectorAll(`.block-proposal[data-proposal-id="${open.proposal_id}"]`).length,
        editable: surface()?.querySelectorAll("[contenteditable=true]").length })
        .toEqual({ accept: 0, candidate: 0, editable: 0 });
      await act(async () => { surface()?.querySelector<HTMLButtonElement>("button[data-proposal-reject]")?.click(); });
      await expect.poll(() => surface()).toBeNull();
      expect({ rejections: rejections.map(({ reject_proposal_operations_input: value }) =>
        [value.proposal_revision_id, value.selected_pending_operation_ids]), failures: tab.failures })
        .toEqual({ rejections: [[open.revision_id, [open.operations[0]!.operation_id]]], failures: [] });
    } finally {
      await tab.unmount(); await test.close();
    }
  });
});
