// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/activity-stream-duplicate-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  activityStream, applyAuthorEdit, createAgentRun, createChapter, createEditorSession,
  createProjectCommandChallenge, createVolume, digestApplyAuthorEdit, digestCreateAgentRun,
  digestCreateChapter, digestCreateEditorSession, digestCreateVolume, digestUpdateProjectAssistance,
  getEditorSession, getManuscriptTree, updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateAgentRunRequest,
  CreateChapterRequest,
  CreateVolumeRequest,
  DigestValue,
  GetEditorSessionResponse,
  UpdateProjectAssistanceRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  createEmptyProject,
  queryStoryOSPostgres as queryPostgres,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeBudget,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32"
  ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const VOLUME_CORRELATION = "018f0000-0000-7001-8000-000000000e50";
const VOLUME_KEY = "018f0000-0000-7001-8000-000000000e51";
const EDIT_CORRELATION = "018f0000-0000-7001-8000-000000000e52";
const EDIT_UNDO = "018f0000-0000-7001-8000-000000000e53";
const EDIT_INTENT = "018f0000-0000-7001-8000-000000000e54";
const EDIT_KEY = "018f0000-0000-7001-8000-000000000e55";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const ISO_INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;

async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A, "session-b": USER_B },
  });
}

function canonicalJson(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value).sort().map((key) => [key, canonicalJson(Reflect.get(value, key))]),
    );
  }
  return value;
}

async function eventPayloadDigest(payload: unknown): Promise<DigestValue> {
  const digest = new Uint8Array(await crypto.subtle.digest(
    "SHA-256", new TextEncoder().encode(JSON.stringify(canonicalJson(payload))),
  ));
  return {
    algorithm: "sha256",
    profile: "storyos.event-payload.jcs.v1",
    value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join(""),
  };
}

interface SseFrame {
  data: unknown;
  event: string;
  id: string;
}

function parseSseFrames(body: string): SseFrame[] {
  return body.split("\n\n").filter((block) => block.trim().length > 0).map((block) => {
    const frame = { id: "", event: "", data: null };
    for (const line of block.split("\n")) {
      if (line.startsWith("id:")) frame.id = line.slice(3).trim();
      if (line.startsWith("event:")) frame.event = line.slice(6).trim();
      if (line.startsWith("data:")) frame.data = JSON.parse(line.slice(5).trim());
    }
    return frame;
  });
}

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
  const challenge = await withChallengeBudget(PROJECT_A, () => createProjectCommandChallenge({
    baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/editor-sessions",
      command_schema: request.command_schema,
      canonical_command_digest: digest,
      idempotency_key: "018f0000-0000-7001-8000-000000000e57",
    },
  }));
  const session = await createEditorSession({
    baseUrl, projectId: PROJECT_A, request,
    idempotencyKey: "018f0000-0000-7001-8000-000000000e57",
    antiForgery: challenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
  });
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
  const challenge = await withChallengeBudget(PROJECT_A, () => createProjectCommandChallenge({
    baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/volumes",
      command_schema: request.command_schema,
      canonical_command_digest: digest,
      idempotency_key: VOLUME_KEY,
    },
  }));
  return createVolume({
    baseUrl, projectId: PROJECT_A, request, idempotencyKey: VOLUME_KEY,
    antiForgery: challenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
  });
}

test("activityStream delivers a payload-table Event then an Author Edit in position order", async () => {
  const { baseUrl, server } = await startRealServer();
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
    const authorEditChallenge = await withChallengeBudget(PROJECT_A, () => createProjectCommandChallenge({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
        command_schema: authorEditRequest.command_schema,
        canonical_command_digest: authorEditDigest,
        idempotency_key: EDIT_KEY,
      },
    }));
    const applied = await applyAuthorEdit({
      baseUrl, projectId: PROJECT_A, request: authorEditRequest, idempotencyKey: EDIT_KEY,
      antiForgery: authorEditChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    });
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

const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};

function activityId(suffix: string): string {
  return `018f0000-0000-7001-8000-00000000${suffix}`;
}

async function challenged<T>(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  projectId: string;
  method: string;
  route: string;
  schema: string;
  digest: DigestValue;
  key: string;
  send: (antiForgery: string) => Promise<T>;
}): Promise<T> {
  const challenge = await withChallengeBudget(options.projectId, () => createProjectCommandChallenge({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    fetchImpl: options.fetchImpl,
    request: {
      method: options.method,
      route_template: options.route,
      command_schema: options.schema,
      canonical_command_digest: options.digest,
      idempotency_key: options.key,
    },
  }));
  return options.send(challenge.nonce);
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
  await challenged({
    baseUrl, fetchImpl, projectId, method: "PUT",
    route: "/api/v1/projects/{project_id}/assistance",
    schema: assistance.command_schema, digest: await digestUpdateProjectAssistance(assistance), key: key(1),
    send: (antiForgery) => updateProjectAssistance({
      baseUrl, projectId, fetchImpl, idempotencyKey: key(1), antiForgery, request: assistance,
    }),
  });
  const volume: CreateVolumeRequest = {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: { title: "Volume A", expected_tree_revision: "1", ...BINDING, correlation_id: key(2) },
  };
  const createdVolume = await challenged({
    baseUrl, fetchImpl, projectId, method: "POST",
    route: "/api/v1/projects/{project_id}/volumes",
    schema: volume.command_schema, digest: await digestCreateVolume(volume), key: key(3),
    send: (antiForgery) => createVolume({
      baseUrl, projectId, fetchImpl, idempotencyKey: key(3), antiForgery, request: volume,
    }),
  });
  if (createdVolume.effect.kind !== "authoritative_applied") {
    throw new Error("Create Volume must apply");
  }
  const volumeId = createdVolume.effect.volume_id;
  const chapter: CreateChapterRequest = {
    command_schema: "storyos.command.create-chapter.request.v1",
    create_chapter_input: { title: "Chapter A", expected_tree_revision: "2", ...BINDING, correlation_id: key(4) },
  };
  const createdChapter = await challenged({
    baseUrl, fetchImpl, projectId, method: "POST",
    route: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
    schema: chapter.command_schema, digest: await digestCreateChapter(chapter), key: key(5),
    send: (antiForgery) => createChapter({
      baseUrl, projectId, volumeId, fetchImpl, idempotencyKey: key(5), antiForgery, request: chapter,
    }),
  });
  if (createdChapter.effect.kind !== "authoritative_applied") {
    throw new Error("Create Chapter must apply");
  }
  return createdChapter.effect.chapter_id;
}

function runRequest(
  conversation: CreateAgentRunRequest["create_agent_run_input"]["conversation"],
  chapterId: string,
  correlationId: string,
): CreateAgentRunRequest {
  return {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation,
      author_message: { text: "Help with this passage." },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

async function postRun(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  key: string,
  request: CreateAgentRunRequest,
) {
  const admitted = await challenged({
    baseUrl, fetchImpl, projectId, method: "POST",
    route: "/api/v1/projects/{project_id}/agent-runs",
    schema: request.command_schema, digest: await digestCreateAgentRun(request), key,
    send: (antiForgery) => createAgentRun({
      baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request,
    }),
  });
  return { admitted };
}

test("implemented assistance and Run Events replay from a retained public cursor", async () => {
  const started = await startRealServer();
  try {
    const fetchImpl = browserFetch(started.baseUrl, "session-a");
    const first = {
      fetchImpl,
      projectId: await createEmptyProject({
        baseUrl: started.baseUrl, fetchImpl, createKey: "018f0000-0000-7001-8000-00001309f730",
        correlationId: activityId("f731"), title: "Activity Novel",
      }),
    };
    const tree = await getManuscriptTree({
      baseUrl: started.baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl,
    });
    const chapterId = await prepareRunProject(started.baseUrl, first.fetchImpl, first.projectId, "f7");
    const run = await postRun(
      started.baseUrl, first.fetchImpl, first.projectId, activityId("f732"),
      runRequest({ kind: "new" }, chapterId, activityId("f733")),
    );
    if (run.admitted.effect.kind !== "admitted") throw new Error("Run admission did not apply");
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
      kind: "agent_run", id: run.admitted.effect.run_id,
    });
    const afterAssistance = await activityStream({ ...options, lastEventId: frames[0]!.id });
    assert.equal((afterAssistance.match(/event: storyos.project-activity/g) ?? []).length, 3);
    assert.equal(await activityStream({ ...options, lastEventId: frames[3]!.id }), "");
  } finally {
    await stopRealServer(started.server);
  }
});
