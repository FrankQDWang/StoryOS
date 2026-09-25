import { expect, it } from "vitest";

import { digestAcceptProposal } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { acceptDisplayedBlockProposal, retryPendingDisplayedAcceptance }
  from "../../src/accept-block-proposal.ts";
import { readAcceptanceJournal } from "../../src/acceptance-journal.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { jsonResponse } from "./scenario.ts";

it("retries the same protected Acceptance after an unknown delivery", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    const { workspace } = test;
    const proposalId = "018f0000-0000-7001-8000-000000000101";
    const operationId = "018f0000-0000-7001-8000-000000000102";
    const proposalRevisionId = "018f0000-0000-7001-8000-000000000103";
    const validationReceiptId = "018f0000-0000-7001-8000-000000000104";
    const authoritativeRevisionId = workspace.session.base_snapshot.authoritative_head_revision_id;
    const sent: { body: string; key: string; nonce: string }[] = [];
    let denyNext = false;
    const fetchImpl: typeof fetch = async (input, init) => {
      const path = new URL(input instanceof Request ? input.url : input).pathname;
      if (path.endsWith("/anti-forgery-challenges")) {
        return jsonResponse({ nonce: "b".repeat(64),
          expires_at: "2026-12-13T08:05:00.000Z" });
      }
      if (!path.endsWith("/acceptances")) throw new Error(`Unexpected request: ${path}`);
      const headers = new Headers(init?.headers);
      const body = String(init?.body);
      const key = headers.get("idempotency-key") ?? "";
      const nonce = headers.get("x-storyos-anti-forgery") ?? "";
      sent.push({ body, key, nonce });
      if (denyNext) return jsonResponse({ code: "forbidden" }, 403);
      if (sent.length <= 2) throw new TypeError("Connection lost");
      const request = JSON.parse(body) as AcceptProposalRequest;
      const digest = await digestAcceptProposal(request, crypto);
      return jsonResponse({
        schema_id: "storyos.command.accept-proposal.response.v1",
        correlation_id: request.accept_proposal_input.correlation_id,
        project_scope: workspace.partition.project_scope,
        command_id: "018f0000-0000-7001-8000-000000000105",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000106",
        receipt: {
          receipt_id: "018f0000-0000-7001-8000-000000000107",
          project_scope: workspace.partition.project_scope,
          command_digest: digest,
          idempotency_key: key,
          author_command_admission_id: "018f0000-0000-7001-8000-000000000106",
          proposal_id: proposalId,
          proposal_revision_id: proposalRevisionId,
          validation_receipt_id: validationReceiptId,
          selected_operation_ids: [operationId],
          prior_authoritative_revision_ids: [authoritativeRevisionId],
          resulting_authoritative_revision_ids: [authoritativeRevisionId],
          authoritative_commit_ids: [],
          condition_refs: [],
          result: "conflicted",
          created_at: "2026-09-25T08:00:00.000Z",
        },
        project: { project_id: workspace.partition.project_scope.project_id,
          title: "Project A", open: { kind: "current_chapter",
            current_chapter_id: workspace.session.base_snapshot.chapter_id } },
        effect: { kind: "conflicted", reason: "changed_head" },
      });
    };
    const options = { baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, workspace,
      proposalId, operationId, proposalRevisionId, validationReceiptId,
      authoritativeRevisionId };
    const first = acceptDisplayedBlockProposal(options);
    const competing = expect(acceptDisplayedBlockProposal({ ...options,
      operationId: "018f0000-0000-7001-8000-000000000109" }))
      .rejects.toThrow(/prior Acceptance decision is unresolved/);
    await expect(first).rejects.toThrow("Connection lost");
    expect(sent).toHaveLength(2);
    expect(sent[0]).toEqual(sent[1]);
    const pending = await readAcceptanceJournal(workspace);
    expect(pending.groups[0]?.settlement).toEqual({ kind: "unsettled" });
    const crash = workspace.database.transaction("transport_attempts", "readwrite");
    const attempts = crash.objectStore("transport_attempts");
    const lastRequest = attempts.getAll();
    lastRequest.onsuccess = () => {
      const last = lastRequest.result.sort((left, right) =>
        left.attempt_ordinal - right.attempt_ordinal).at(-1) as Record<string, unknown>;
      attempts.put({ ...last, outcome: { kind: "in_flight" } });
    };
    await new Promise<void>((resolve, reject) => {
      crash.oncomplete = () => resolve();
      crash.onabort = () => reject(crash.error);
    });
    await competing;

    const response = await retryPendingDisplayedAcceptance({ baseUrl: location.origin,
      fetchImpl, cryptoImpl: crypto, workspace, proposalId });
    expect(response.effect).toEqual({ kind: "conflicted", reason: "changed_head" });
    expect(sent).toHaveLength(3);
    expect(sent[2]).toEqual(sent[0]);
    const settled = await readAcceptanceJournal(workspace);
    expect(settled.groups[0]?.settlement).toEqual({ kind: "settled", response });
    expect(await acceptDisplayedBlockProposal(options)).toEqual(response);
    await expect(acceptDisplayedBlockProposal({ ...options,
      validationReceiptId: "018f0000-0000-7001-8000-000000000108" }))
      .rejects.toThrow(/prior Acceptance decision is unresolved/);
    expect(sent).toHaveLength(3);
    denyNext = true;
    await expect(acceptDisplayedBlockProposal({ ...options,
      proposalId: "018f0000-0000-7001-8000-000000000110" }))
      .rejects.toThrow(/HTTP 403/);
    expect((await readAcceptanceJournal(workspace)).groups[1]?.acceptance_delivery)
      .toEqual({ kind: "known_problem", status: 403, code: "command_http_error",
        responseBody: '{"code":"forbidden"}' });
    await expect(acceptDisplayedBlockProposal({ ...options,
      proposalId: "018f0000-0000-7001-8000-000000000110" }))
      .rejects.toThrow(/requires review/);
    expect(sent).toHaveLength(4);
  } finally {
    await test.close();
  }
});
