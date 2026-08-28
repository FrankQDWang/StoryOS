import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  applyAuthorEdit, createEditorSession, createProjectCommandChallenge, digestApplyAuthorEdit,
  digestCreateEditorSession, digestTakeOverProjectWriter, getEditorSession, getSnapshot,
  takeOverProjectWriter,
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
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32"
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
    "018f0000-0000-7001-8000-000000000338",
    "018f0000-0000-7001-8000-000000000339",
  );
  if (session.writer.kind !== "current_writer") {
    throw new Error("the fixture did not create the current writer");
  }
  return session;
}

test("an observer takeOverProjectWriter fences the prior writer and refuses its Author Edit", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const writer = await loadCurrentWriter(baseUrl);
    if (writer.writer.kind !== "current_writer") {
      throw new Error("the takeover fixture lost current-writer status");
    }
    const observer = await openEditorSession(
      baseUrl,
      "018f0000-0000-7001-8000-000000000330",
      "018f0000-0000-7001-8000-000000000331",
    );
    if (observer.writer.kind !== "read_only") {
      throw new Error("the observer unexpectedly became the current writer");
    }
    assert.equal(observer.writer.reason, "secondary_session");
    const priorGeneration = writer.writer.writer_generation;
    const resultingGeneration = String(BigInt(priorGeneration) + 1n);
    const authorityBefore = await manuscriptAuthority();

    const takeoverRequest = {
      command_schema: "storyos.command.take-over-project-writer.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000332",
      editor_session_id: observer.editor_session.editor_session_id,
      observed_writer_generation: priorGeneration,
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
    };
    const takeoverKey = "018f0000-0000-7001-8000-000000000333";
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
    const takeoverOptions = {
      baseUrl, projectId: PROJECT_A,
      editorSessionId: observer.editor_session.editor_session_id,
      request: takeoverRequest, idempotencyKey: takeoverKey,
      antiForgery: takeoverChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    };
    const takeover = await takeOverProjectWriter(takeoverOptions);
    assert.deepEqual(await takeOverProjectWriter(takeoverOptions), takeover);
    if (takeover.result.kind !== "takeover_applied") {
      throw new Error("the fixture takeover did not apply");
    }

    assert.match(takeover.command_id, UUID);
    assert.match(takeover.author_command_admission_id, UUID);
    assert.match(takeover.receipt.receipt_id, UUID);
    assert.match(takeover.receipt.created_at, ISO_INSTANT);
    assert.match(takeover.receipt.command_digest.value_hex_lowercase, DIGEST_HEX);
    assert.match(takeover.result.resulting_snapshot_id, UUID);
    assert.equal(takeover.result.resulting_heads.length, 1);
    const heads = takeover.result.resulting_heads;
    const firstHead = heads.at(0);
    assert.ok(firstHead);
    assert.match(firstHead, UUID);
    assert.deepEqual(takeover, {
      schema_id: "storyos.command.take-over-project-writer.response.v1",
      correlation_id: takeoverRequest.correlation_id,
      project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
      command_id: takeover.command_id,
      author_command_admission_id: takeover.author_command_admission_id,
      receipt: {
        receipt_id: takeover.receipt.receipt_id,
        project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
        command_kind: "takeOverProjectWriter",
        command_digest: {
          algorithm: "sha256",
          profile: "storyos.command.takeOverProjectWriter.jcs.v1",
          value_hex_lowercase: takeover.receipt.command_digest.value_hex_lowercase,
        },
        idempotency_key: takeoverKey,
        producer_cause: "author_command_admission",
        author_command_admission_id: takeover.author_command_admission_id,
        expected_heads: heads,
        prior_heads: heads,
        resulting_heads: heads,
        authoritative_revision_ids: [],
        proposal_revision_ids: [],
        authoritative_commit_ids: [],
        author_action_sequence: null,
        draft_artifact_refs: [],
        artifact_lifecycle_event_refs: [],
        condition_refs: [],
        result: "no_effect",
        created_at: takeover.receipt.created_at,
      },
      result: {
        kind: "takeover_applied",
        prior_editor_session_id: writer.editor_session.editor_session_id,
        prior_writer_generation: priorGeneration,
        resulting_editor_session_id: observer.editor_session.editor_session_id,
        resulting_writer_generation: resultingGeneration,
        resulting_snapshot_id: takeover.result.resulting_snapshot_id,
        resulting_snapshot_activity_position:
          takeover.result.resulting_snapshot_activity_position,
        resulting_heads: heads,
      },
    });
    assert.match(takeover.result.resulting_snapshot_activity_position, /^[1-9][0-9]*$/);

    const snapshot = await getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId: takeover.result.resulting_snapshot_id,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(snapshot.snapshot.snapshot_id, takeover.result.resulting_snapshot_id);
    assert.ok(
      BigInt(snapshot.snapshot.project_activity_position)
        >= BigInt(takeover.result.resulting_snapshot_activity_position),
    );

    const winner = await getEditorSession({
      baseUrl, projectId: PROJECT_A,
      editorSessionId: observer.editor_session.editor_session_id,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.deepEqual(winner.writer, {
      kind: "current_writer", writer_generation: resultingGeneration,
    });
    const fenced = await getEditorSession({
      baseUrl, projectId: PROJECT_A,
      editorSessionId: writer.editor_session.editor_session_id,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.deepEqual(fenced.writer, {
      kind: "read_only",
      observed_writer_generation: resultingGeneration,
      reason: "superseded_by_takeover",
    });

    const authorityAfterTakeover = await manuscriptAuthority();
    assert.deepEqual({
      ...authorityAfterTakeover,
      takeover_payloads: authorityBefore.takeover_payloads,
    }, authorityBefore);
    assert.equal(authorityAfterTakeover.takeover_payloads, authorityBefore.takeover_payloads + 1);

    const staleRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000334",
      editor_session_id: writer.editor_session.editor_session_id,
      writer_generation: priorGeneration,
      chapter_id: writer.base_snapshot.chapter_id,
      expected_authoritative_revision_id:
        writer.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids: writer.base_snapshot.proposal_head_revision_ids,
      target_refs: writer.base_snapshot.target_refs,
      observed_ownership_partition: writer.base_snapshot.observed_ownership_partition,
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
      undo_group_id: "018f0000-0000-7001-8000-000000000335",
      completed_intent_record_id: "018f0000-0000-7001-8000-000000000336",
      local_intent_sequence: "99",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from: 0, to: 0, text: "z" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 0,
        },
      }],
    };
    const staleKey = "018f0000-0000-7001-8000-000000000337";
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
    assert.deepEqual(await manuscriptAuthority(), authorityAfterTakeover);
  } finally {
    await stopRealServer(server);
  }
});
