// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { test } from "vitest";
import {
  acceptProposal, applyAuthorEdit, createAgentRun, createEditorSession,
  digestAcceptProposal, digestApplyAuthorEdit, digestCreateAgentRun,
  digestCreateEditorSession, getAgentRun, getChapter, getProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  AcceptProposalRequest, ApplyAuthorEditRequest, CreateAgentRunRequest,
  CreateEditorSessionRequest, GetProposalResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { BINDING, PROSE, USER_A, challenged, drainLeftoverWork, id, prepare, settleOnce,
  startRealServer } from "../support/acceptance.ts";

const SECOND_PROSE = "Keep the second block voice in this passage.";

async function seedTwoBlocks(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  ns: string,
) {
  const sessionRequest: CreateEditorSessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}1`),
  };
  const session = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/editor-sessions",
    sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest),
    id(`${ns}2`),
    (antiForgery) => createEditorSession({
      baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}2`), antiForgery,
      request: sessionRequest,
    }),
  );
  if (session.writer.kind !== "current_writer") throw new Error("expected current writer");
  const insertRequest: ApplyAuthorEditRequest = {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}3`),
    editor_session_id: session.editor_session.editor_session_id,
    writer_generation: session.writer.writer_generation,
    chapter_id: session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: [],
    target_refs: session.base_snapshot.target_refs,
    observed_ownership_partition: "authoritative",
    editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: id(`${ns}4`),
    completed_intent_record_id: id(`${ns}5`),
    local_intent_sequence: "1",
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from: 0, to: 0, text: "Hello World" }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 0,
      },
    }],
  };
  const inserted = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/manuscript/author-edits",
    insertRequest.command_schema, await digestApplyAuthorEdit(insertRequest), id(`${ns}6`),
    (antiForgery) => applyAuthorEdit({
      baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}6`), antiForgery,
      request: insertRequest,
    }),
  );
  if (inserted.effect.kind !== "authoritative_applied") {
    throw new Error("expected inserted passage");
  }
  const firstBlock = inserted.effect.authoritative_revision.blocks[0];
  if (!firstBlock) throw new Error("expected first Block");
  const splitRequest: ApplyAuthorEditRequest = {
    ...insertRequest,
    correlation_id: id(`${ns}7`),
    expected_authoritative_revision_id: inserted.effect.authoritative_revision.revision_id,
    undo_group_id: id(`${ns}8`),
    completed_intent_record_id: id(`${ns}9`),
    local_intent_sequence: "2",
    author_edit_units: [{
      normalized_primitives: [{
        kind: "split_block",
        manuscript_block_id: firstBlock.manuscript_block_id,
        offset: 6,
        new_manuscript_block_id: id(`${ns}b2`),
      }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 6, to: 6,
      },
    }],
  };
  const split = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/manuscript/author-edits",
    splitRequest.command_schema, await digestApplyAuthorEdit(splitRequest), id(`${ns}a`),
    (antiForgery) => applyAuthorEdit({
      baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}a`), antiForgery,
      request: splitRequest,
    }),
  );
  if (split.effect.kind !== "authoritative_applied") {
    throw new Error("expected split Blocks");
  }
  assert.equal(split.effect.authoritative_revision.blocks.length, 2);
  return {
    session,
    revisionId: split.effect.authoritative_revision.revision_id,
    secondBlockId: split.effect.authoritative_revision.blocks[1]!.manuscript_block_id,
  };
}

async function admitPassages(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  text: string,
  key: string,
  beforeSettle?: () => Promise<void>,
  settle = true,
) {
  const request: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" },
      author_message: { text },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: id(key.slice(-4)),
    },
  };
  const created = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema, await digestCreateAgentRun(request), key,
    (antiForgery) => createAgentRun({
      baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request,
    }),
  );
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  await beforeSettle?.();
  if (settle) await settleOnce();
  return getAgentRun({ baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
}

test("a later reservation on the second Block refuses the whole admitted set", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Reserved Multi Target Novel", `${ns}2`);
    const seeded = await seedTwoBlocks(started.baseUrl, prepared.fetchImpl, prepared.projectId, `${ns}3`);
    const queried = await admitPassages(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
      "Revise these passages: keep the voice.", id(`${ns}41`), async () => {
        const other = await admitPassages(
          started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
          "Revise this passage: keep the voice.", id(`${ns}51`), undefined, false,
        );
        const proposalId = id(`${ns}61`);
        await queryPostgres(`
          INSERT INTO storyos.proposals
            (owner_user_id, project_id, proposal_id, kind, chapter_id,
             manuscript_block_id, source_run_id, source_decision_id)
          VALUES ('${USER_A}'::uuid, '${prepared.projectId}'::uuid, '${proposalId}'::uuid,
            'block_edit', '${prepared.chapterId}'::uuid, '${seeded.secondBlockId}'::uuid,
            '${other.run_id}'::uuid, '${id(`${ns}62`)}'::uuid);
          INSERT INTO storyos.proposal_operations
            (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
             resolution, reservation_state, candidate_text)
          VALUES ('${USER_A}'::uuid, '${prepared.projectId}'::uuid, '${proposalId}'::uuid,
            '${id(`${ns}63`)}'::uuid, '${seeded.secondBlockId}'::uuid, 'pending', 'unresolved', 'reserved');
        `);
      },
    );
    assert.equal(queried.decision.kind, "prose_change");
    if (queried.decision.kind !== "prose_change") throw new Error("expected prose decision");
    assert.deepEqual(queried.decision.opened_proposal, { kind: "absent" });
    assert.equal(queried.context.current_availability.working_target.kind, "current");
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.proposals
      WHERE source_run_id = '${queried.run_id}'::uuid;`), "0");
    const chapter = await getChapter({ baseUrl: started.baseUrl,
      projectId: prepared.projectId, chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    assert.equal(chapter.chapter.current_revision.revision_id, seeded.revisionId);
    await settleOnce();
  } finally {
    await stopRealServer(started.server);
  }
});

function openedProposal(queried: Awaited<ReturnType<typeof getAgentRun>>): string {
  if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened prose");
  }
  return queried.decision.opened_proposal.proposal_id;
}

function acceptRequest(
  opened: GetProposalResponse,
  selected: string[],
  expectedRevisionId: string,
  editorSessionId: string,
  correlationId: string,
): AcceptProposalRequest {
  if (opened.proposal.validation_receipt.kind !== "present") {
    throw new Error("expected validation receipt");
  }
  return {
    command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: {
      proposal_revision_id: opened.proposal.revision_id,
      validation_receipt_id: opened.proposal.validation_receipt.validation_receipt_id,
      selected_operation_ids: selected,
      expected_authoritative_revision_id: expectedRevisionId,
      editor_session_id: editorSessionId,
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

test("acceptProposal applies a reversed multi-operation set without using array order as authority", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f3820111"), "Multi Operation Novel", "f3822");
    const seeded = await seedTwoBlocks(started.baseUrl, prepared.fetchImpl, prepared.projectId, "f3823");
    const queried = await admitPassages(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
      "Revise these passages: keep the voice.", id("f3820131"),
    );
    const opened = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: openedProposal(queried), fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.proposal.operations.length, 2);
    assert.deepEqual(opened.proposal.operations.map((operation) => operation.resolution), [
      "pending", "pending",
    ]);
    const selected = [...opened.proposal.operations.map((operation) => operation.operation_id)].reverse();
    const request = acceptRequest(
      opened, selected, seeded.revisionId,
      seeded.session.editor_session.editor_session_id, id("f3820151"),
    );
    const accepted = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      request.command_schema, await digestAcceptProposal(request), id("f3820152"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f3820152"), antiForgery, request,
      }),
    );
    assert.equal(accepted.effect.kind, "applied");
    assert.deepEqual(accepted.receipt.selected_operation_ids, selected);
    if (accepted.effect.kind !== "applied") throw new Error("expected applied");
    assert.equal(accepted.effect.authoritative_revision.body, `${PROSE}\n${SECOND_PROSE}`);
    const inspected = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(inspected.proposal.operations.map((operation) => operation.resolution), [
      "applied", "applied",
    ]);
    const after = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(after.chapter.current_revision.body, `${PROSE}\n${SECOND_PROSE}`);
  } finally {
    await stopRealServer(started.server);
  }
});

test("acceptProposal applies one domain Operation and leaves the other pending", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f3820311"), "Subset Operation Novel", "f3826");
    const seeded = await seedTwoBlocks(started.baseUrl, prepared.fetchImpl, prepared.projectId, "f3827");
    const queried = await admitPassages(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
      "Revise these passages: keep the voice.", id("f3820331"),
    );
    const opened = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: openedProposal(queried), fetchImpl: prepared.fetchImpl,
    });
    const firstOperation = opened.proposal.operations[0];
    const secondOperation = opened.proposal.operations[1];
    if (!firstOperation || !secondOperation) throw new Error("expected two Operations");
    const firstRequest = acceptRequest(
      opened, [firstOperation.operation_id], seeded.revisionId,
      seeded.session.editor_session.editor_session_id, id("f3820351"),
    );
    const firstAccepted = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      firstRequest.command_schema, await digestAcceptProposal(firstRequest), id("f3820352"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f3820352"), antiForgery, request: firstRequest,
      }),
    );
    assert.equal(firstAccepted.effect.kind, "applied");
    const afterFirst = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(
      afterFirst.proposal.operations.map((operation) => operation.resolution),
      ["applied", "pending"],
    );
    if (firstAccepted.effect.kind !== "applied") throw new Error("expected applied");
    const secondRequest = acceptRequest(
      afterFirst, [secondOperation.operation_id],
      firstAccepted.effect.authoritative_revision.revision_id,
      seeded.session.editor_session.editor_session_id, id("f3820353"),
    );
    const secondAccepted = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      secondRequest.command_schema, await digestAcceptProposal(secondRequest), id("f3820354"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f3820354"), antiForgery, request: secondRequest,
      }),
    );
    assert.equal(secondAccepted.effect.kind, "applied");
    if (secondAccepted.effect.kind !== "applied") throw new Error("expected applied");
    assert.equal(secondAccepted.effect.authoritative_revision.body, `${PROSE}\n${SECOND_PROSE}`);
  } finally {
    await stopRealServer(started.server);
  }
});

test("acceptProposal refuses incomplete atomic Bundle closure then applies the closed set", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f3820211"), "Bundle Operation Novel", "f3824");
    const seeded = await seedTwoBlocks(started.baseUrl, prepared.fetchImpl, prepared.projectId, "f3825");
    const queried = await admitPassages(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
      "Revise these passages as a bundle: keep the voice.", id("f3820231"),
    );
    const opened = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: openedProposal(queried), fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.proposal.operations.length, 2);
    const firstOperation = opened.proposal.operations[0];
    if (!firstOperation) throw new Error("expected first Operation");
    const incomplete = acceptRequest(
      opened, [firstOperation.operation_id], seeded.revisionId,
      seeded.session.editor_session.editor_session_id, id("f3820251"),
    );
    const refused = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      incomplete.command_schema, await digestAcceptProposal(incomplete), id("f3820252"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f3820252"), antiForgery, request: incomplete,
      }),
    );
    assert.deepEqual(refused.effect, { kind: "refused", reason: "incomplete_bundle_closure" });
    const closed = acceptRequest(
      opened,
      [...opened.proposal.operations.map((operation) => operation.operation_id)].reverse(),
      seeded.revisionId, seeded.session.editor_session.editor_session_id, id("f3820253"),
    );
    const accepted = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      closed.command_schema, await digestAcceptProposal(closed), id("f3820254"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f3820254"), antiForgery, request: closed,
      }),
    );
    assert.equal(accepted.effect.kind, "applied");
    if (accepted.effect.kind !== "applied") throw new Error("expected applied");
    assert.equal(accepted.effect.authoritative_revision.body, `${PROSE}\n${SECOND_PROSE}`);
  } finally {
    await stopRealServer(started.server);
  }
});
