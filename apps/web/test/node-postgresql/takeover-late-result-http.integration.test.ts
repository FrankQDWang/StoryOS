import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  applyAuthorEdit, createEditorSession, createProjectCommandChallenge, digestApplyAuthorEdit,
  digestCreateEditorSession, digestTakeOverProjectWriter, getApplyAuthorEditOutcome,
  getEditorSession, takeOverProjectWriter,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateEditorSessionResponse,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32"
  ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const ISO_INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const DIGEST_HEX = /^[0-9a-f]{64}$/;
async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A, "session-b": USER_B },
  });
}

async function manuscriptAuthority() {
  return JSON.parse(await queryPostgres(`SELECT json_build_object(
    'author_edit_activities', (SELECT count(*) FROM storyos.project_activity_events
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'actions', (SELECT count(*) FROM storyos.author_action_entries
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'commits', (SELECT count(*) FROM storyos.authoritative_commits
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'apply_admissions', (SELECT count(*) FROM storyos.author_command_admissions
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
        AND command_kind = 'applyAuthorEdit'),
    'takeover_payloads', (SELECT count(*) FROM storyos.project_activity_event_payloads
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid)
  )::text`));
}

async function openEditorSession(
  baseUrl: string,
  correlationId: string,
  idempotencyKey: string,
): Promise<CreateEditorSessionResponse> {
  const request = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: "storyos.web-client.release-1.v3",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: correlationId,
  };
  const digest = await digestCreateEditorSession(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/editor-sessions",
      command_schema: request.command_schema,
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  return createEditorSession({
    baseUrl, projectId: PROJECT_A, request, idempotencyKey,
    antiForgery: challenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
  });
}

async function loadCurrentWriter(baseUrl: string): Promise<GetEditorSessionResponse> {
  const existing = await queryPostgres(`SELECT current_editor_session_id::text
    FROM storyos.project_writer_generations
    WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
    ORDER BY writer_generation DESC LIMIT 1`);
  if (existing) {
    const session = await getEditorSession({
      baseUrl, projectId: PROJECT_A, editorSessionId: existing,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    if (session.writer.kind !== "current_writer") {
      throw new Error("the fixture did not expose the current writer");
    }
    return session;
  }
  const session = await openEditorSession(
    baseUrl,
    "018f0000-0000-7001-8000-000000000348",
    "018f0000-0000-7001-8000-000000000349",
  );
  if (session.writer.kind !== "current_writer") {
    throw new Error("the fixture did not create the current writer");
  }
  return session;
}

test("a fenced writer's late ApplyAuthorEdit result does not mutate authority", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const writer = await loadCurrentWriter(baseUrl);
    if (writer.writer.kind !== "current_writer") {
      throw new Error("the late-result fixture lost current-writer status");
    }
    const observer = await openEditorSession(
      baseUrl,
      "018f0000-0000-7001-8000-000000000340",
      "018f0000-0000-7001-8000-000000000341",
    );
    if (observer.writer.kind !== "read_only") {
      throw new Error("the observer unexpectedly became the current writer");
    }
    const priorGeneration = writer.writer.writer_generation;
    const from = writer.base_snapshot.materialized_revision.body.length;
    const expectedBody = `${writer.base_snapshot.materialized_revision.body}!`;
    const authorEditRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000342",
      editor_session_id: writer.editor_session.editor_session_id,
      writer_generation: priorGeneration,
      chapter_id: writer.base_snapshot.chapter_id,
      expected_authoritative_revision_id: writer.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids: writer.base_snapshot.proposal_head_revision_ids,
      target_refs: writer.base_snapshot.target_refs,
      observed_ownership_partition: writer.base_snapshot.observed_ownership_partition,
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
      undo_group_id: "018f0000-0000-7001-8000-000000000343",
      completed_intent_record_id: "018f0000-0000-7001-8000-000000000344",
      local_intent_sequence: "1",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from, to: from, text: "!" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from,
        },
      }],
    };
    const authorEditKey = "018f0000-0000-7001-8000-000000000345";
    const authorEditDigest = await digestApplyAuthorEdit(authorEditRequest);
    const authorEditChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
        command_schema: authorEditRequest.command_schema,
        canonical_command_digest: authorEditDigest,
        idempotency_key: authorEditKey,
      },
    }));
    const authorEditOptions = {
      baseUrl, projectId: PROJECT_A, request: authorEditRequest, idempotencyKey: authorEditKey,
      antiForgery: authorEditChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    };
    await assert.rejects(applyAuthorEdit({
      ...authorEditOptions,
      fetchImpl: async (url, options) => {
        const response = await browserFetch(baseUrl, "session-a")(url, options);
        await response.arrayBuffer();
        throw new Error("simulated acknowledgement delivery loss");
      },
    }), /simulated acknowledgement delivery loss/);

    const takeoverRequest = {
      command_schema: "storyos.command.take-over-project-writer.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000346",
      editor_session_id: observer.editor_session.editor_session_id,
      observed_writer_generation: priorGeneration,
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
    };
    const takeoverKey = "018f0000-0000-7001-8000-000000000347";
    const takeoverDigest = await digestTakeOverProjectWriter(takeoverRequest);
    const takeoverChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
      request: {
        method: "POST",
        route_template:
          "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers",
        command_schema: takeoverRequest.command_schema,
        canonical_command_digest: takeoverDigest,
        idempotency_key: takeoverKey,
      },
    }));
    await takeOverProjectWriter({
      baseUrl, projectId: PROJECT_A,
      editorSessionId: observer.editor_session.editor_session_id,
      request: takeoverRequest, idempotencyKey: takeoverKey,
      antiForgery: takeoverChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    const authorityAfterFence = await manuscriptAuthority();

    const committedOutcome = await getApplyAuthorEditOutcome({
      baseUrl, projectId: PROJECT_A, idempotencyKey: authorEditKey,
      antiForgery: authorEditChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    const late = await applyAuthorEdit(authorEditOptions);
    if (committedOutcome.outcome.outcome_kind !== "committed") {
      throw new Error("the lost acknowledgement did not settle as committed");
    }
    assert.deepEqual(committedOutcome.outcome.response, late);
    const { correlation_id: firstQueryCorrelation, ...firstQuery } = committedOutcome;
    assert.match(firstQueryCorrelation, UUID);
    assert.notEqual(firstQueryCorrelation, authorEditRequest.correlation_id);
    const repeatedOutcome = await getApplyAuthorEditOutcome({
      baseUrl, projectId: PROJECT_A, idempotencyKey: authorEditKey,
      antiForgery: authorEditChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    const { correlation_id: repeatedQueryCorrelation, ...repeatedQuery } = repeatedOutcome;
    assert.match(repeatedQueryCorrelation, UUID);
    assert.notEqual(repeatedQueryCorrelation, authorEditRequest.correlation_id);
    assert.deepEqual(repeatedQuery, firstQuery);
    assert.deepEqual(await applyAuthorEdit(authorEditOptions), late);

    if (late.effect.kind !== "authoritative_applied") {
      throw new Error("the late Author Edit result was not authoritative");
    }

    assert.match(late.command_id, UUID);
    assert.match(late.author_command_admission_id, UUID);
    assert.match(late.receipt.receipt_id, UUID);
    assert.match(late.receipt.created_at, ISO_INSTANT);
    assert.match(late.receipt.command_digest.value_hex_lowercase, DIGEST_HEX);
    assert.match(late.effect.authoritative_revision.revision_id, UUID);
    assert.match(late.effect.authoritative_commit_id, UUID);
    const revisionId = late.effect.authoritative_revision.revision_id;
    const commitId = late.effect.authoritative_commit_id;
    assert.deepEqual(late, {
      schema_id: "storyos.command.apply-author-edit.response.v2",
      correlation_id: authorEditRequest.correlation_id,
      project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
      command_id: late.command_id,
      author_command_admission_id: late.author_command_admission_id,
      receipt: {
        receipt_id: late.receipt.receipt_id,
        project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
        command_kind: "applyAuthorEdit",
        command_digest: {
          algorithm: "sha256",
          profile: "storyos.command.applyAuthorEdit.jcs.v1",
          value_hex_lowercase: late.receipt.command_digest.value_hex_lowercase,
        },
        idempotency_key: authorEditKey,
        producer_cause: "author_command_admission",
        author_command_admission_id: late.author_command_admission_id,
        expected_heads: [writer.base_snapshot.authoritative_head_revision_id],
        prior_heads: [writer.base_snapshot.authoritative_head_revision_id],
        resulting_heads: [revisionId],
        authoritative_revision_ids: [revisionId],
        proposal_revision_ids: [],
        authoritative_commit_ids: [commitId],
        author_action_sequence: late.effect.author_action_sequence,
        draft_artifact_refs: [],
        artifact_lifecycle_event_refs: [],
        condition_refs: [],
        result: "authoritative_applied",
        created_at: late.receipt.created_at,
      },
      effect: {
        kind: "authoritative_applied",
        authoritative_revision: { revision_id: revisionId, body: expectedBody },
        authoritative_commit_id: commitId,
        author_action_sequence: late.effect.author_action_sequence,
        project_activity_position: late.effect.project_activity_position,
      },
      completed_intent_record_id: authorEditRequest.completed_intent_record_id,
      local_intent_sequence: "1",
    });
    assert.match(late.effect.author_action_sequence, /^[1-9][0-9]*$/);
    assert.match(late.effect.project_activity_position, /^[1-9][0-9]*$/);
    assert.deepEqual(await manuscriptAuthority(), authorityAfterFence);

    const staleRequest: ApplyAuthorEditRequest = {
      ...authorEditRequest,
      correlation_id: "018f0000-0000-7001-8000-000000000350",
      undo_group_id: "018f0000-0000-7001-8000-000000000351",
      completed_intent_record_id: "018f0000-0000-7001-8000-000000000352",
      local_intent_sequence: "2",
      expected_authoritative_revision_id: revisionId,
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from: from + 1, to: from + 1, text: "z" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: from + 1, to: from + 1,
        },
      }],
    };
    const staleKey = "018f0000-0000-7001-8000-000000000353";
    const staleDigest = await digestApplyAuthorEdit(staleRequest);
    const staleChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
        command_schema: staleRequest.command_schema,
        canonical_command_digest: staleDigest,
        idempotency_key: staleKey,
      },
    }));
    await assert.rejects(applyAuthorEdit({
      baseUrl, projectId: PROJECT_A, request: staleRequest, idempotencyKey: staleKey,
      antiForgery: staleChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => {
      const protocolError = requireStoryOSProtocolError(error);
      return protocolError.status === 412
        && Reflect.get(JSON.parse(protocolError.responseBody ?? "{}"), "code")
          === "editor_writer_stale";
    });
    assert.deepEqual(await manuscriptAuthority(), authorityAfterFence);
  } finally {
    await stopRealServer(server);
  }
});
