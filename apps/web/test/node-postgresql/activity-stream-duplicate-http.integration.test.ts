// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { test } from "vitest";

import {
  activityStream, applyAuthorEdit, createEditorSession,
  digestApplyAuthorEdit, digestCreateEditorSession, getEditorSession,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { PROJECT_A, USER_A, challenged, eventPayloadDigest, parseSseFrames, startServer } from "../support/http-harness.ts";
import {
  queryStoryOSPostgres as queryPostgres,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const ISO_INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;

async function manuscriptAuthority() {
  return JSON.parse(await queryPostgres(`SELECT json_build_object(
    'author_edit_activities', (SELECT count(*) FROM storyos.project_activity_events
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'actions', (SELECT count(*) FROM storyos.author_action_entries
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'commits', (SELECT count(*) FROM storyos.authoritative_commits
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid)
  )::text`));
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
  const request = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: "storyos.web-client.release-1.v3",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000360",
  };
  const digest = await digestCreateEditorSession(request);
  const session = await challenged(baseUrl, browserFetch(baseUrl, "session-a"), PROJECT_A, "POST",
    "/api/v1/projects/{project_id}/editor-sessions", request.command_schema, digest,
    "018f0000-0000-7001-8000-000000000361", (antiForgery) => createEditorSession({
      baseUrl, projectId: PROJECT_A, request,
      idempotencyKey: "018f0000-0000-7001-8000-000000000361",
      antiForgery, fetchImpl: browserFetch(baseUrl, "session-a"),
    }));
  if (session.writer.kind !== "current_writer") {
    throw new Error("the fixture did not create the current writer");
  }
  return session;
}

test("activityStream replays a complete applied Event and Last-Event-ID resumes exclusively", async () => {
  const { baseUrl, server } = await startServer();
  try {
    const writer = await loadCurrentWriter(baseUrl);
    if (writer.writer.kind !== "current_writer") {
      throw new Error("the Activity fixture lost current-writer status");
    }
    const snapshotId = writer.base_snapshot.snapshot_id;
    const from = writer.base_snapshot.materialized_revision.body.length;
    const authorEditRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000362",
      editor_session_id: writer.editor_session.editor_session_id,
      writer_generation: writer.writer.writer_generation,
      chapter_id: writer.base_snapshot.chapter_id,
      expected_authoritative_revision_id: writer.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids: writer.base_snapshot.proposal_head_revision_ids,
      target_refs: writer.base_snapshot.target_refs,
      observed_ownership_partition: writer.base_snapshot.observed_ownership_partition,
      editor_contract_revision: "storyos.editor-contract.release-1.v3",
      undo_group_id: "018f0000-0000-7001-8000-000000000363",
      completed_intent_record_id: "018f0000-0000-7001-8000-000000000364",
      local_intent_sequence: "1",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from, to: from, text: "+" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from,
        },
      }],
    };
    const authorEditKey = "018f0000-0000-7001-8000-000000000365";
    const authorEditDigest = await digestApplyAuthorEdit(authorEditRequest);
    const applied = await challenged(baseUrl, browserFetch(baseUrl, "session-a"), PROJECT_A, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", authorEditRequest.command_schema, authorEditDigest,
      authorEditKey, (antiForgery) => applyAuthorEdit({
        baseUrl, projectId: PROJECT_A, request: authorEditRequest, idempotencyKey: authorEditKey,
        antiForgery, fetchImpl: browserFetch(baseUrl, "session-a"),
      }));
    if (applied.effect.kind !== "authoritative_applied") {
      throw new Error("the fixture Author Edit was not authoritative");
    }
    const durable = JSON.parse(await queryPostgres(`SELECT json_build_object(
      'event_id', activity.project_activity_event_id::text,
      'created_at', to_char(activity.created_at AT TIME ZONE 'UTC',
        'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
    )::text
      FROM storyos.project_activity_events AS activity
     WHERE activity.owner_user_id = '${USER_A}'::uuid
       AND activity.project_id = '${PROJECT_A}'::uuid
       AND activity.receipt_id = '${applied.receipt.receipt_id}'::uuid`));
    assert.match(durable.event_id, UUID);
    assert.match(durable.created_at, ISO_INSTANT);
    const payload = {
      chapter_id: authorEditRequest.chapter_id,
      authoritative_revision_id: applied.effect.authoritative_revision.revision_id,
      authoritative_commit_id: applied.effect.authoritative_commit_id,
      author_action_sequence: applied.effect.author_action_sequence,
    };
    const expectedData = {
      envelope_version: 1,
      activity_profile: "storyos.project-activity.v1",
      event_id: durable.event_id,
      event_schema: "storyos.event.authoritative-author-edit-applied.v1",
      event_kind: "authoritative_author_edit_applied",
      project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
      requester_user_id: USER_A,
      actor: { kind: "author", id: USER_A },
      project_sequence: applied.effect.project_activity_position,
      stream_sequence: applied.effect.project_activity_position,
      agent_run_id: null,
      run_step_id: null,
      run_sequence: null,
      aggregate_ref: { kind: "chapter", id: authorEditRequest.chapter_id },
      correlation_id: applied.correlation_id,
      causation: { kind: "command", id: applied.command_id },
      command_id: applied.command_id,
      receipt_ref: { kind: "domain_receipt", id: applied.receipt.receipt_id },
      occurred_at: durable.created_at,
      recorded_at: durable.created_at,
      payload,
      payload_digest: await eventPayloadDigest(payload),
      application_wire_record_ref: durable.event_id,
      limit_profile_revision: "storyos.foundation.absolute.v1",
    };
    const streamOptions = {
      baseUrl, projectId: PROJECT_A, snapshotId,
      protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    };
    const firstBody = await activityStream(streamOptions);
    const firstFrames = parseSseFrames(firstBody);
    assert.equal(firstFrames.length, 1);
    const firstFrame = firstFrames.at(0);
    assert.ok(firstFrame);
    assert.match(firstFrame.id, /./);
    assert.equal(firstFrame.event, "storyos.project-activity");
    assert.deepEqual(firstFrame.data, expectedData);
    const authorityAfterFirst = await manuscriptAuthority();
    const duplicateBody = await activityStream(streamOptions);
    assert.equal(duplicateBody, firstBody);
    assert.deepEqual(await manuscriptAuthority(), authorityAfterFirst);
    assert.equal(await activityStream({
      ...streamOptions, lastEventId: firstFrame.id,
    }), "");
    assert.deepEqual(await manuscriptAuthority(), authorityAfterFirst);
  } finally {
    await stopRealServer(server);
  }
});
