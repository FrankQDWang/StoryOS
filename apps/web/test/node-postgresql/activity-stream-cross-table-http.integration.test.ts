// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/activity-stream-duplicate-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import {
  activityStream,
  applyAuthorEdit,
  createChapter,
  createEditorSession,
  createVolume,
  digestApplyAuthorEdit,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  digestUpdateProjectAssistance,
  getEditorSession,
  getManuscriptTree,
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateChapterRequest,
  CreateVolumeRequest,
  GetEditorSessionResponse,
  UpdateProjectAssistanceRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { BINDING, PROJECT_A, USER_A, challenged, createEmpty, eventPayloadDigest, parseSseFrames, postRun,
  runRequest, startServer } from "../support/http-harness.ts";
import {
  queryStoryOSPostgres as queryPostgres,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";

const VOLUME_CORRELATION = "018f0000-0000-7001-8000-000000000e50";
const VOLUME_KEY = "018f0000-0000-7001-8000-000000000e51";
const EDIT_CORRELATION = "018f0000-0000-7001-8000-000000000e52";
const EDIT_UNDO = "018f0000-0000-7001-8000-000000000e53";
const EDIT_INTENT = "018f0000-0000-7001-8000-000000000e54";
const EDIT_KEY = "018f0000-0000-7001-8000-000000000e55";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const ISO_INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;

function nextPosition(position: string, delta: bigint): string {
  return (BigInt(position) + delta).toString();
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
    correlation_id: "018f0000-0000-7001-8000-000000000e56",
  };
  const digest = await digestCreateEditorSession(request);
  const session = await challenged(baseUrl, browserFetch(baseUrl, "session-a"), PROJECT_A, "POST",
    "/api/v1/projects/{project_id}/editor-sessions", request.command_schema, digest,
    "018f0000-0000-7001-8000-000000000e57", (antiForgery) => createEditorSession({
      baseUrl, projectId: PROJECT_A, request,
      idempotencyKey: "018f0000-0000-7001-8000-000000000e57",
      antiForgery, fetchImpl: browserFetch(baseUrl, "session-a"),
    }));
  if (session.writer.kind !== "current_writer") {
    throw new Error("the fixture did not create the current writer");
  }
  return session;
}

async function postVolume(
  baseUrl: string,
  expectedTreeRevision: string,
) {
  const request: CreateVolumeRequest = {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: {
      title: "Cross-table Activity Volume",
      expected_tree_revision: expectedTreeRevision,
      client_contract_revision:
        RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: VOLUME_CORRELATION,
    },
  };
  const digest = await digestCreateVolume(request);
  return challenged(baseUrl, browserFetch(baseUrl, "session-a"), PROJECT_A, "POST",
    "/api/v1/projects/{project_id}/volumes", request.command_schema, digest, VOLUME_KEY,
    (antiForgery) => createVolume({
      baseUrl, projectId: PROJECT_A, request, idempotencyKey: VOLUME_KEY,
      antiForgery, fetchImpl: browserFetch(baseUrl, "session-a"),
    }));
}

test("activityStream delivers a payload-table Event then an Author Edit in position order", async () => {
  const { baseUrl, server } = await startServer();
  try {
    const writer = await loadCurrentWriter(baseUrl);
    if (writer.writer.kind !== "current_writer") {
      throw new Error("the Activity fixture lost current-writer status");
    }
    const snapshotId = writer.base_snapshot.snapshot_id;
    const snapshotPosition = writer.base_snapshot.project_activity_position;
    const volumePosition = nextPosition(snapshotPosition, 1n);
    const editPosition = nextPosition(snapshotPosition, 2n);
    const tree = await getManuscriptTree({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    const volume = await postVolume(baseUrl, tree.tree_revision);
    if (volume.effect.kind !== "authoritative_applied") {
      throw new Error("the fixture Create Volume was not authoritative");
    }
    assert.equal(volume.effect.project_activity_position, volumePosition);
    const from = writer.base_snapshot.materialized_revision.body.length;
    const authorEditRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: EDIT_CORRELATION,
      editor_session_id: writer.editor_session.editor_session_id,
      writer_generation: writer.writer.writer_generation,
      chapter_id: writer.base_snapshot.chapter_id,
      expected_authoritative_revision_id: writer.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids: writer.base_snapshot.proposal_head_revision_ids,
      target_refs: writer.base_snapshot.target_refs,
      observed_ownership_partition: writer.base_snapshot.observed_ownership_partition,
      editor_contract_revision: "storyos.editor-contract.release-1.v3",
      undo_group_id: EDIT_UNDO,
      completed_intent_record_id: EDIT_INTENT,
      local_intent_sequence: "1",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from, to: from, text: "#" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from,
        },
      }],
    };
    const authorEditDigest = await digestApplyAuthorEdit(authorEditRequest);
    const applied = await challenged(baseUrl, browserFetch(baseUrl, "session-a"), PROJECT_A, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", authorEditRequest.command_schema, authorEditDigest,
      EDIT_KEY, (antiForgery) => applyAuthorEdit({
        baseUrl, projectId: PROJECT_A, request: authorEditRequest, idempotencyKey: EDIT_KEY,
        antiForgery, fetchImpl: browserFetch(baseUrl, "session-a"),
      }));
    if (applied.effect.kind !== "authoritative_applied") {
      throw new Error("the fixture Author Edit was not authoritative");
    }
    assert.equal(applied.effect.project_activity_position, editPosition);
    const volumeDurable = JSON.parse(await queryPostgres(`SELECT json_build_object(
      'event_id', payload.project_activity_event_id::text,
      'created_at', to_char(payload.created_at AT TIME ZONE 'UTC',
        'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
    )::text
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = '${USER_A}'::uuid
       AND payload.project_id = '${PROJECT_A}'::uuid
       AND payload.receipt_id = '${volume.receipt.receipt_id}'::uuid`));
    const editDurable = JSON.parse(await queryPostgres(`SELECT json_build_object(
      'event_id', activity.project_activity_event_id::text,
      'created_at', to_char(activity.created_at AT TIME ZONE 'UTC',
        'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
    )::text
      FROM storyos.project_activity_events AS activity
     WHERE activity.owner_user_id = '${USER_A}'::uuid
       AND activity.project_id = '${PROJECT_A}'::uuid
       AND activity.receipt_id = '${applied.receipt.receipt_id}'::uuid`));
    assert.match(volumeDurable.event_id, UUID);
    assert.match(volumeDurable.created_at, ISO_INSTANT);
    assert.match(editDurable.event_id, UUID);
    assert.match(editDurable.created_at, ISO_INSTANT);
    const volumePayload = {
      kind: "volume_created",
      volume_id: volume.effect.volume_id,
      title: volume.effect.title,
      tree_revision: volume.effect.tree_revision,
      order: volume.effect.order,
    };
    const editPayload = {
      chapter_id: authorEditRequest.chapter_id,
      authoritative_revision_id: applied.effect.authoritative_revision.revision_id,
      authoritative_commit_id: applied.effect.authoritative_commit_id,
      author_action_sequence: applied.effect.author_action_sequence,
    };
    const expectedVolume = {
      envelope_version: 1,
      activity_profile: "storyos.project-activity.v1",
      event_id: volumeDurable.event_id,
      event_schema: "storyos.event.volume-created.v2",
      event_kind: "volume_created",
      project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
      requester_user_id: USER_A,
      actor: { kind: "author", id: USER_A },
      project_sequence: volumePosition,
      stream_sequence: volumePosition,
      agent_run_id: null,
      run_step_id: null,
      run_sequence: null,
      aggregate_ref: { kind: "volume", id: volume.effect.volume_id },
      correlation_id: volume.correlation_id,
      causation: { kind: "command", id: volume.command_id },
      command_id: volume.command_id,
      receipt_ref: { kind: "domain_receipt", id: volume.receipt.receipt_id },
      occurred_at: volumeDurable.created_at,
      recorded_at: volumeDurable.created_at,
      payload: volumePayload,
      payload_digest: await eventPayloadDigest(volumePayload),
      application_wire_record_ref: volumeDurable.event_id,
      limit_profile_revision: "storyos.foundation.absolute.v1",
    };
    const expectedEdit = {
      envelope_version: 1,
      activity_profile: "storyos.project-activity.v1",
      event_id: editDurable.event_id,
      event_schema: "storyos.event.authoritative-author-edit-applied.v1",
      event_kind: "authoritative_author_edit_applied",
      project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
      requester_user_id: USER_A,
      actor: { kind: "author", id: USER_A },
      project_sequence: editPosition,
      stream_sequence: editPosition,
      agent_run_id: null,
      run_step_id: null,
      run_sequence: null,
      aggregate_ref: { kind: "chapter", id: authorEditRequest.chapter_id },
      correlation_id: applied.correlation_id,
      causation: { kind: "command", id: applied.command_id },
      command_id: applied.command_id,
      receipt_ref: { kind: "domain_receipt", id: applied.receipt.receipt_id },
      occurred_at: editDurable.created_at,
      recorded_at: editDurable.created_at,
      payload: editPayload,
      payload_digest: await eventPayloadDigest(editPayload),
      application_wire_record_ref: editDurable.event_id,
      limit_profile_revision: "storyos.foundation.absolute.v1",
    };
    const streamOptions = {
      baseUrl, projectId: PROJECT_A, snapshotId,
      protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    };
    const firstFrames = parseSseFrames(await activityStream(streamOptions));
    assert.equal(firstFrames.length, 2);
    const volumeFrame = firstFrames.at(0);
    const editFrame = firstFrames.at(1);
    assert.ok(volumeFrame);
    assert.ok(editFrame);
    assert.match(volumeFrame.id, /./);
    assert.match(editFrame.id, /./);
    assert.equal(volumeFrame.event, "storyos.project-activity");
    assert.equal(editFrame.event, "storyos.project-activity");
    assert.deepEqual(volumeFrame.data, expectedVolume);
    assert.deepEqual(editFrame.data, expectedEdit);
    const resumed = parseSseFrames(await activityStream({
      ...streamOptions, lastEventId: volumeFrame.id,
    }));
    assert.equal(resumed.length, 1);
    assert.deepEqual(resumed.at(0)?.data, expectedEdit);
    assert.equal(await activityStream({
      ...streamOptions, lastEventId: editFrame.id,
    }), "");
    const replayed = parseSseFrames(await activityStream(streamOptions));
    assert.equal(replayed.length, 2);
    assert.deepEqual(replayed.at(0)?.data, expectedVolume);
    assert.deepEqual(replayed.at(1)?.data, expectedEdit);
  } finally {
    await stopRealServer(server);
  }
});

function activityId(suffix: string): string {
  return `018f0000-0000-7001-8000-00000000${suffix}`;
}

/** Makes assistance available and adds one Volume and one Chapter. */
async function prepareRunProject(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  keyNs: string,
) {
  const key = (index: number) => `018f0000-0000-7001-8000-000000000${keyNs}${index}`;
  const assistance: UpdateProjectAssistanceRequest = {
    command_schema: "storyos.command.update-project-assistance.request.v1",
    update_project_assistance_input: {
      availability: "available", expected_assistance_revision: "0", ...BINDING, correlation_id: key(0),
    },
  };
  await challenged(baseUrl, fetchImpl, projectId, "PUT",
    "/api/v1/projects/{project_id}/assistance",
    assistance.command_schema, await digestUpdateProjectAssistance(assistance), key(1),
    (antiForgery) => updateProjectAssistance({
      baseUrl, projectId, fetchImpl, idempotencyKey: key(1), antiForgery, request: assistance,
    }));
  const volume: CreateVolumeRequest = {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: { title: "Volume A", expected_tree_revision: "1", ...BINDING, correlation_id: key(2) },
  };
  const createdVolume = await challenged(baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/volumes",
    volume.command_schema, await digestCreateVolume(volume), key(3),
    (antiForgery) => createVolume({
      baseUrl, projectId, fetchImpl, idempotencyKey: key(3), antiForgery, request: volume,
    }));
  if (createdVolume.effect.kind !== "authoritative_applied") {
    throw new Error("Create Volume must apply");
  }
  const volumeId = createdVolume.effect.volume_id;
  const chapter: CreateChapterRequest = {
    command_schema: "storyos.command.create-chapter.request.v1",
    create_chapter_input: { title: "Chapter A", expected_tree_revision: "2", ...BINDING, correlation_id: key(4) },
  };
  const createdChapter = await challenged(baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
    chapter.command_schema, await digestCreateChapter(chapter), key(5),
    (antiForgery) => createChapter({
      baseUrl, projectId, volumeId, fetchImpl, idempotencyKey: key(5), antiForgery, request: chapter,
    }));
  if (createdChapter.effect.kind !== "authoritative_applied") {
    throw new Error("Create Chapter must apply");
  }
  return createdChapter.effect.chapter_id;
}

test("implemented assistance and Run Events replay from a retained public cursor", async () => {
  const started = await startServer();
  try {
    const first = await createEmpty(started.baseUrl, "session-a", "018f0000-0000-7001-8000-00001309f730",
      "Activity Novel", activityId("f731"));
    const tree = await getManuscriptTree({
      baseUrl: started.baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl,
    });
    const chapterId = await prepareRunProject(started.baseUrl, first.fetchImpl, first.projectId, "f7");
    const run = await postRun(
      started.baseUrl, first.fetchImpl, first.projectId, activityId("f732"),
      runRequest(chapterId, activityId("f733"), "Help with this passage."),
    );
    if (run.effect.kind !== "admitted") throw new Error("Run admission did not apply");
    const options = {
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      snapshotId: tree.snapshot.snapshot_id,
      protocolRelease: "storyos.public.release.1",
      fetchImpl: first.fetchImpl,
    };
    const body = await activityStream(options);
    const frames = parseSseFrames(body).map((frame) => ({
      id: frame.id,
      data: frame.data as {
        event_kind: string; event_schema: string; aggregate_ref: { kind: string; id: string };
      },
    }));
    assert.deepEqual(frames.map((frame) => frame.data.event_kind), [
      "project_assistance_updated", "volume_created", "chapter_created", "agent_run_created",
    ]);
    assert.equal(frames[0]?.data.event_schema, "storyos.event.project-assistance-updated.v1");
    assert.deepEqual(frames[0]?.data.aggregate_ref, { kind: "project", id: first.projectId });
    assert.equal(frames[3]?.data.event_schema, "storyos.event.agent-run-created.v1");
    assert.deepEqual(frames[3]?.data.aggregate_ref, {
      kind: "agent_run", id: run.effect.run_id,
    });
    const afterAssistance = await activityStream({ ...options, lastEventId: frames[0]!.id });
    assert.equal((afterAssistance.match(/event: storyos.project-activity/g) ?? []).length, 3);
    assert.equal(await activityStream({ ...options, lastEventId: frames[3]!.id }), "");
  } finally {
    await stopRealServer(started.server);
  }
});
