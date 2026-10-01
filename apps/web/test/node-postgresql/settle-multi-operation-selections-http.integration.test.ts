// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { test } from "vitest";
import {
  acceptProposal, applyAuthorEdit, createAgentRun, createEditorSession,
  digestAcceptProposal, digestApplyAuthorEdit, digestCreateAgentRun,
  digestCreateEditorSession, digestExportProjectArchive, digestReplanProposal,
  exportProjectArchive, getAgentRun, getChapter, getExportOperation, getProposal, replanProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  AcceptProposalRequest, ApplyAuthorEditRequest, CreateAgentRunRequest,
  CreateEditorSessionRequest, GetProposalResponse, ReplanProposalRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, sessionFetch as browserFetch, requireStoryOSProtocolError,
  stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { canonicalDraftValue } from "../../src/refused-edit-discard.ts";
import { zipStoreFiles } from "../support/archive.ts";
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

test("one request produces explained exact locations and keeps them after restart", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const { fetchImpl, projectId, chapterId } = await prepare(started.baseUrl, id(`${ns}11`), "Explained locations", `${ns}2`);
    await seedTwoBlocks(started.baseUrl, fetchImpl, projectId, `${ns}3`);
    const before = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    const run = await admitPassages(started.baseUrl, fetchImpl, projectId, chapterId,
      "Revise these passages: keep the voice.", id(`${ns}41`));
    assert.equal(run.decision.kind, "prose_change");
    const locations = (run.decision as unknown as { locations?: Array<Record<string, unknown>> }).locations;
    assert.equal(locations?.length, 2);
    assert.deepEqual(locations!.map(({ chapter_id, manuscript_block_id, base_authoritative_revision_id,
      candidate_text, explanation }) => ({ chapter_id, manuscript_block_id, base_authoritative_revision_id, candidate_text, explanation })),
      before.chapter.current_revision.blocks.map((block, index) => ({ chapter_id: chapterId,
        manuscript_block_id: block.manuscript_block_id,
        base_authoritative_revision_id: before.chapter.current_revision.revision_id,
        candidate_text: index === 0 ? PROSE : SECOND_PROSE,
        explanation: index === 0 ? "Preserve the narrator voice in the first passage." : "Keep the second passage consistent with the narrator voice.",
      })));
    const opened = await getProposal({ baseUrl: started.baseUrl, projectId, fetchImpl,
      proposalId: openedProposal(run) });
    assert.deepEqual(opened.proposal.operations.map((operation) =>
      (operation as unknown as { candidate_text: string }).candidate_text), [PROSE, SECOND_PROSE]);
    assert.equal(opened.proposal.validation_receipt.kind, "present");
    const after = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    assert.deepEqual(after.chapter, before.chapter);
    await stopRealServer(started.server);
    started = await startRealServer();
    const reloadedFetch = browserFetch(started.baseUrl, "session-a");
    const reloaded = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: run.run_id, fetchImpl: reloadedFetch });
    assert.deepEqual(reloaded.decision, run.decision);
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: reloadedFetch })).proposal, opened.proposal);
    await settleOnce();
    assert.deepEqual((await getAgentRun({ baseUrl: started.baseUrl, projectId,
      runId: run.run_id, fetchImpl: reloadedFetch })).decision, run.decision);
  } finally { await stopRealServer(started.server); }
});

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

for (const history of ["retained", "legacy_overwritten"]) {
test(`partial Acceptance preserves ${history} evidence and conflicts the remaining Operation`, async () => {
  const ns = randomBytes(3).toString("hex");
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id(`${ns}0311`), "Subset Operation Novel", `${ns}6`);
    const seeded = await seedTwoBlocks(started.baseUrl, prepared.fetchImpl, prepared.projectId, `${ns}7`);
    const queried = await admitPassages(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
      "Revise these passages: keep the voice.", id(`${ns}0331`),
    );
    const opened = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: openedProposal(queried), fetchImpl: prepared.fetchImpl,
    });
    const firstOperation = opened.proposal.operations[0];
    const secondOperation = opened.proposal.operations[1];
    if (!firstOperation || !secondOperation) throw new Error("expected two Operations");
    const retainedHistory = async () => JSON.parse(await queryPostgres(`SELECT jsonb_build_object(
      'revision', (SELECT to_jsonb(record) FROM storyos.proposal_revisions AS record
        WHERE project_id='${prepared.projectId}'::uuid AND revision_id='${opened.proposal.revision_id}'::uuid),
      'validation', (SELECT to_jsonb(record) FROM storyos.validation_receipts AS record
        WHERE project_id='${prepared.projectId}'::uuid AND proposal_revision_id='${opened.proposal.revision_id}'::uuid))::text`));
    const originalHistory = await retainedHistory();
    const beforeAcceptance = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    const firstRequest = acceptRequest(
      opened, [firstOperation.operation_id], seeded.revisionId,
      seeded.session.editor_session.editor_session_id, id(`${ns}0351`),
    );
    let durableResponse: Awaited<ReturnType<typeof acceptProposal>> | undefined;
    const lossyFetch: typeof fetch = async (input, init) => {
      const response = await prepared.fetchImpl(input, init);
      if (String(input).endsWith("/acceptances") && response.status === 200) {
        durableResponse = await response.clone().json();
        if (started.server.exitCode === null && started.server.signalCode === null) await stopRealServer(started.server);
        throw new Error("Controlled acknowledgement loss");
      }
      return response;
    };
    let nonce = "";
    await assert.rejects(challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      firstRequest.command_schema, await digestAcceptProposal(firstRequest), id(`${ns}0352`),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: lossyFetch,
        idempotencyKey: id(`${ns}0352`), antiForgery: (nonce = antiForgery), request: firstRequest,
      }),
    ), /Controlled acknowledgement loss/);
    const address = new URL(started.baseUrl);
    started = await startRealServer(`${address.hostname}:${address.port}`);
    const fetchImpl = browserFetch(started.baseUrl, "session-a");
    const firstAccepted = await acceptProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl,
      idempotencyKey: id(`${ns}0352`), antiForgery: nonce, request: firstRequest,
    });
    assert.deepEqual(firstAccepted, durableResponse);
    assert.deepEqual(await retainedHistory(), originalHistory);
    assert.equal(firstAccepted.effect.kind, "applied");
    const afterFirst = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(afterFirst.proposal, { ...opened.proposal,
      operation_resolution: "applied", reservation_state: "resolved",
      operations: opened.proposal.operations.map((operation) => operation.operation_id === firstOperation.operation_id
        ? { ...operation, resolution: "applied", reservation_state: "resolved" } : operation),
    });
    if (firstAccepted.effect.kind !== "applied") throw new Error("expected applied");
    assert.deepEqual(firstAccepted.effect.authoritative_revision, {
      ...beforeAcceptance.chapter.current_revision, revision_id: firstAccepted.effect.authoritative_revision.revision_id,
      body: `${PROSE}\nWorld`,
      blocks: beforeAcceptance.chapter.current_revision.blocks.map((block) =>
        block.manuscript_block_id === firstOperation.manuscript_block_id ? { ...block, text: PROSE } : block),
    });
    if (history === "legacy_overwritten") {
      await queryPostgres(`UPDATE storyos.proposal_revisions SET base_authoritative_revision_id=
        '${firstAccepted.effect.authoritative_revision.revision_id}'::uuid
        WHERE project_id='${prepared.projectId}'::uuid AND revision_id='${opened.proposal.revision_id}'::uuid;
        UPDATE storyos.validation_receipts SET base_authoritative_revision_id=
        '${firstAccepted.effect.authoritative_revision.revision_id}'::uuid
        WHERE project_id='${prepared.projectId}'::uuid AND proposal_revision_id='${opened.proposal.revision_id}'::uuid`);
      await assert.rejects(getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl }),
        (error) => requireStoryOSProtocolError(error).status === 503);
    }
    const unchangedHistory = await retainedHistory();
    const secondRequest = acceptRequest(
      afterFirst, [secondOperation.operation_id],
      firstAccepted.effect.authoritative_revision.revision_id,
      seeded.session.editor_session.editor_session_id, id(`${ns}0353`),
    );
    let secondNonce = "";
    const acceptRemaining = async () => secondNonce === "" ? challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      secondRequest.command_schema, await digestAcceptProposal(secondRequest), id(`${ns}0354`),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id(`${ns}0354`), antiForgery: (secondNonce = antiForgery), request: secondRequest,
      }),
    ) : acceptProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl,
      idempotencyKey: id(`${ns}0354`), antiForgery: secondNonce, request: secondRequest,
    });
    if (history === "retained") {
      const settlementState = async () => JSON.parse(await queryPostgres(`SELECT jsonb_build_object(${[
        "authoritative_heads", "authoritative_revisions", "authoritative_commits", "author_action_entries",
        "scope_counters", "proposal_heads", "proposal_revisions", "proposal_operations", "validation_receipts",
        "domain_receipts", "acceptance_receipts", "proposal_validation_conditions",
      ].map((table) => `'${table}', (SELECT coalesce(jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text), '[]'::jsonb)
        FROM storyos.${table} AS record WHERE owner_user_id='${USER_A}'::uuid AND project_id='${prepared.projectId}'::uuid)`).join(",")})::text`));
      const beforeFault = await settlementState();
      const fault = `validation_fault_${ns}`;
      await queryPostgres(`CREATE FUNCTION storyos.${fault}() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF NEW.project_id='${prepared.projectId}'::uuid THEN RAISE EXCEPTION 'controlled condition fault'; END IF;
        RETURN NEW; END $$; CREATE TRIGGER ${fault} BEFORE INSERT ON storyos.proposal_validation_conditions
        FOR EACH ROW EXECUTE FUNCTION storyos.${fault}();`);
      try {
        await assert.rejects(acceptRemaining, (error) => requireStoryOSProtocolError(error).status === 503);
        assert.deepEqual(await settlementState(), beforeFault);
      } finally {
        await queryPostgres(`DROP TRIGGER ${fault} ON storyos.proposal_validation_conditions; DROP FUNCTION storyos.${fault}()`);
      }
    }
    const secondAccepted = await acceptRemaining();
    assert.deepEqual(secondAccepted.effect, { kind: "conflicted", reason: "changed_head" });
    assert.deepEqual(await retainedHistory(), unchangedHistory);
    assert.deepEqual(await acceptProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl,
      idempotencyKey: id(`${ns}0352`), antiForgery: nonce, request: firstRequest,
    }), firstAccepted);
    const conflictRef = secondAccepted.receipt.condition_refs[0]!;
    if (history === "retained") {
      const afterConflict = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl,
    });
    assert.deepEqual(afterConflict.proposal, {
      ...afterFirst.proposal, validation: "conflicted",
      condition_refs: secondAccepted.receipt.condition_refs,
      source_condition: { kind: "proposal_conflict", proposal_conflict_ref: secondAccepted.receipt.condition_refs[0]! },
    });
    }
    const chapter = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl,
    });
    assert.deepEqual(chapter.chapter.current_revision, firstAccepted.effect.authoritative_revision);
    const replanRequest: ReplanProposalRequest = {
      command_schema: "storyos.command.replan-proposal.request.v1",
      replan_proposal_input: {
        ...BINDING, correlation_id: id(`${ns}0361`),
        conflicted_proposal_revision_id: opened.proposal.revision_id,
        expected_current_proposal_head: opened.proposal.revision_id,
        expected_current_target_revisions: [chapter.chapter.current_revision.revision_id],
        replacement_operations: [secondOperation.operation_id],
        source_condition: { kind: "proposal_conflict", proposal_conflict_ref: conflictRef },
        editor_session_id: seeded.session.editor_session.editor_session_id,
      },
    };
    const replanned = await challenged(started.baseUrl, fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans", replanRequest.command_schema,
      await digestReplanProposal(replanRequest), id(`${ns}0362`), (antiForgery) => replanProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId, proposalId: opened.proposal.proposal_id,
        fetchImpl, idempotencyKey: id(`${ns}0362`), antiForgery, request: replanRequest,
      }));
    assert.equal(replanned.effect.kind, "resolved");
    const afterReplan = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl });
    assert.notEqual(afterReplan.proposal.revision_id, opened.proposal.revision_id);
    assert.deepEqual(afterReplan.proposal, { ...afterFirst.proposal,
      revision_id: afterReplan.proposal.revision_id, validation: "pending",
      validation_receipt: { kind: "absent" },
      base_authoritative_revision_id: chapter.chapter.current_revision.revision_id,
      revision_comparison: afterReplan.proposal.revision_comparison,
    });
    const pendingRequest: AcceptProposalRequest = { ...secondRequest,
      accept_proposal_input: { ...secondRequest.accept_proposal_input,
        proposal_revision_id: afterReplan.proposal.revision_id, correlation_id: id(`${ns}0363`) } };
    const pendingAcceptance = await challenged(started.baseUrl, fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", pendingRequest.command_schema,
      await digestAcceptProposal(pendingRequest), id(`${ns}0364`), (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId, proposalId: opened.proposal.proposal_id,
        fetchImpl, idempotencyKey: id(`${ns}0364`), antiForgery, request: pendingRequest,
      }));
    assert.deepEqual(pendingAcceptance.effect, { kind: "refused", reason: "not_eligible" });
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl })).proposal, afterReplan.proposal);
    assert.deepEqual(await retainedHistory(), unchangedHistory);
    const foreign = browserFetch(started.baseUrl, "session-b");
    await assert.rejects(getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: foreign }),
      (error) => requireStoryOSProtocolError(error).status === 404);
    const exportRequest = { command_schema: "storyos.command.export-project-archive.request.v1",
      export_project_archive_input: { ...BINDING, correlation_id: id(`${ns}0371`),
        archive_profile: "storyos.project-export.v1",
        archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
    const archive = async (key = id(`${ns}0372`)) => challenged(started.baseUrl, fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/exports", exportRequest.command_schema,
      await digestExportProjectArchive(exportRequest), key, (antiForgery) => exportProjectArchive({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl,
        idempotencyKey: key, antiForgery, request: exportRequest,
      }));
    if (history === "legacy_overwritten") {
      await assert.rejects(() => archive(), (error) => {
        const problem = requireStoryOSProtocolError(error);
        return problem.status === 422 && JSON.parse(problem.responseBody ?? "null").code === "invalid_provenance";
      });
      assert.deepEqual(await retainedHistory(), unchangedHistory);
    } else {
      const admitted = await archive();
      if (admitted.effect.kind !== "admitted") throw new Error("expected archive Admission");
      await settleOnce();
      const exported = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        exportId: admitted.effect.export_id, fetchImpl });
      assert.equal(exported.status, "ready");
      const download = await fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${admitted.effect.export_id}`,
        { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
      assert.equal(download.status, 200);
      const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
      for (const [table, expected] of [["proposal_revisions", originalHistory.revision], ["validation_receipts", originalHistory.validation]] as const) {
        const records = JSON.parse(new TextDecoder().decode(files.get(`canonical/${table}.json`)));
        assert.deepEqual(records.find((row: { proposal_revision_id?: string; revision_id?: string }) =>
          (row.proposal_revision_id ?? row.revision_id) === opened.proposal.revision_id), expected);
      }
      const conditions = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_validation_conditions.json")));
      assert.equal(conditions.length, 1);
      assert.deepEqual(conditions, JSON.parse(await queryPostgres(`SELECT jsonb_agg(to_jsonb(record))::text
        FROM storyos.proposal_validation_conditions AS record WHERE project_id='${prepared.projectId}'::uuid`)));
      const historical = await archive(id(`${ns}0374`));
      if (historical.effect.kind !== "admitted") throw new Error("expected historical archive Admission");
      const exportId = historical.effect.export_id;
      const facts = JSON.parse(await queryPostgres(`SELECT facts::text FROM storyos.pinned_export_sources
        WHERE project_id='${prepared.projectId}'::uuid AND export_id='${exportId}'::uuid`));
      let replacement = "facts";
      for (const [familyIndex, family] of facts.families.entries()) {
        if (!["proposal_revisions", "validation_receipts"].includes(family.table)) continue;
        for (const [rowIndex, row] of family.rows.entries()) {
          if ((row.proposal_revision_id ?? row.revision_id) === opened.proposal.revision_id) {
            row.base_authoritative_revision_id = chapter.chapter.current_revision.revision_id;
            replacement = `jsonb_set(${replacement}, '{families,${familyIndex},rows,${rowIndex},base_authoritative_revision_id}',
              to_jsonb('${chapter.chapter.current_revision.revision_id}'::text))`;
          }
        }
      }
      const encoded = canonicalDraftValue(facts);
      const digest = createHash("sha256").update(encoded).digest("hex");
      await queryPostgres(`UPDATE storyos.pinned_export_sources SET facts=${replacement},
        facts_sha256='${digest}' WHERE project_id='${prepared.projectId}'::uuid AND export_id='${exportId}'::uuid`);
      await settleOnce();
      const failed = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        exportId, fetchImpl });
      assert.equal(failed.status, "failed");
      assert.equal("immutable_root" in failed, false);
      const refused = await fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${exportId}`,
        { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
      assert.equal(refused.status, 422);
      assert.deepEqual(await retainedHistory(), originalHistory);
    }

  } finally {
    if (started.server.exitCode === null && started.server.signalCode === null) await stopRealServer(started.server);
  }
});
}

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
