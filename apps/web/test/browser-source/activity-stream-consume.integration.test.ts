import { expect, it } from "vitest";

import type {
  GetManuscriptTreeResponse,
  GetSnapshotResponse,
  SnapshotDescriptor,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { persistReplaceSelection, openEditorWorkspace } from "../../src/editor-session.ts";
import { readJournalSnapshot } from "../../src/local-edit-journal.ts";
import { consumeOwnedProjectActivity } from "../../src/project-activity-sync.ts";
import { readProjectActivityIngest } from "../../src/project-activity-ingest.ts";
import {
  EDITOR_SNAPSHOT,
  BLOCK,
  OWNER,
  PROJECT,
  REVISION,
  SESSION,
  closeTrackedDatabases,
  createAppliedActivityEvent,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  requireEditorReady,
  trackDatabase,
} from "./scenario.ts";

const CANONICAL_SNAPSHOT = "018f0000-0000-7001-8000-000000000390";
const PROTOCOL = "storyos.public.release.1";

function activitySse(id: string, data: unknown): Response {
  return new Response(
    `id: ${id}\nevent: storyos.project-activity\ndata: ${JSON.stringify(data)}\n\n`,
    { status: 200, headers: { "content-type": "text/event-stream" } },
  );
}

function problemResponse(status: number, code: string): Response {
  return jsonResponse({
    schema_id: "storyos.problem.v1",
    code,
    message: code,
  }, status);
}

function canonicalSnapshot(overrides: Partial<SnapshotDescriptor> = {}): SnapshotDescriptor {
  return {
    snapshot_id: CANONICAL_SNAPSHOT,
    project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    snapshot_kind: "canonical",
    project_activity_position: "5",
    source_watermarks: {},
    projection_generations: {},
    redaction_profile: "storyos.author.v1",
    schema_profile: PROTOCOL,
    replay_generation: "2",
    created_at: "2026-08-20T04:00:00.000Z",
    expires_at: null,
    ...overrides,
  };
}

function treeResponse(snapshot: SnapshotDescriptor, scope = snapshot.project_scope): GetManuscriptTreeResponse {
  return {
    schema_id: "storyos.query.manuscript-tree.response.v1",
    correlation_id: SESSION,
    project_scope: scope,
    tree_revision: "2",
    snapshot,
    volumes: [],
  };
}

function snapshotResponse(snapshot: SnapshotDescriptor): GetSnapshotResponse {
  return {
    schema_id: "storyos.query.snapshot.response.v1",
    correlation_id: SESSION,
    project_scope: snapshot.project_scope,
    snapshot,
  };
}

async function openReadyWorkspace(fetchImpl: typeof fetch) {
  const scenario = createBrowserScenario();
  const workspace = await openEditorWorkspace({
    baseUrl: location.origin,
    project: scenario.project,
    chapter: scenario.chapter,
    profile: scenario.profile,
    fetchImpl,
  });
  requireEditorReady(workspace);
  await persistReplaceSelection(workspace, {
    from: 4,
    to: 4,
    text: "!?",
    resultingBody: "Base!?",
    inputOrigin: "paste",
    undoGroupId: "018f0000-0000-7001-8000-000000000040",
    createdAt: "2026-08-15T08:00:00.000Z",
  });
  return { scenario, workspace };
}

it("resumes a retained cursor and resynchronizes from an authorized Snapshot after 409", async () => {
  const scenario = createBrowserScenario();
  const snapshot = canonicalSnapshot();
  const eventA = await createAppliedActivityEvent({
    eventId: "018f0000-0000-7001-8000-000000000392",
    commandId: "018f0000-0000-7001-8000-000000000393",
    receiptId: "018f0000-0000-7001-8000-000000000394",
    correlationId: "018f0000-0000-7001-8000-000000000397",
    revisionId: "018f0000-0000-7001-8000-000000000395",
    commitId: "018f0000-0000-7001-8000-000000000396",
    sequence: "1",
    occurredAt: "2026-08-20T03:00:00.000Z",
  });
  const eventB = await createAppliedActivityEvent({
    eventId: "018f0000-0000-7001-8000-000000000398",
    commandId: "018f0000-0000-7001-8000-000000000399",
    receiptId: "018f0000-0000-7001-8000-000000000400",
    correlationId: "018f0000-0000-7001-8000-000000000403",
    revisionId: "018f0000-0000-7001-8000-000000000401",
    commitId: "018f0000-0000-7001-8000-000000000402",
    sequence: "6",
    authorActionSequence: "2",
    occurredAt: "2026-08-20T04:00:01.000Z",
  });
  const activity: { snapshotId: string; lastEventId: string | null }[] = [];
  let expired = false;
  const fetchImpl: typeof fetch = async (input, init) => {
    const url = new URL(input instanceof Request ? input.url : input);
    const path = url.pathname;
    const lastEventId = new Headers(init?.headers).get("last-event-id");
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({
        ...scenario.session,
        schema_id: "storyos.query.editor-session.response.v1",
      });
    }
    if (path.endsWith("/manuscript/tree")) return jsonResponse(treeResponse(snapshot));
    if (path.endsWith(`/snapshots/${CANONICAL_SNAPSHOT}`)) {
      return jsonResponse(snapshotResponse(snapshot));
    }
    if (path.endsWith("/activity")) {
      activity.push({ snapshotId: url.searchParams.get("snapshot_id") ?? "", lastEventId });
      expect(url.searchParams.get("protocol_release")).toBe(PROTOCOL);
      if (url.searchParams.get("snapshot_id") === CANONICAL_SNAPSHOT) {
        expect(lastEventId).toBeNull();
        return activitySse("cursor-gen2-b", eventB);
      }
      if (lastEventId !== null) {
        return expired
          ? problemResponse(409, "activity_cursor_too_old")
          : new Response("", { status: 200, headers: { "content-type": "text/event-stream" } });
      }
      return activitySse("cursor-gen1-a", eventA);
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  try {
    const { workspace } = await openReadyWorkspace(fetchImpl);
    trackDatabase(workspace.database, openDatabases);
    const first = await consumeOwnedProjectActivity(workspace, {
      baseUrl: location.origin, fetchImpl,
    });
    expect(first).toEqual({
      kind: "replayed",
      last_event_id: "cursor-gen1-a",
      ingest: {
        replay_generation: "1",
        processed_through_stream_sequence: "1",
        events: [eventA],
        held: [],
      },
    });
    const duplicate = await consumeOwnedProjectActivity(workspace, {
      baseUrl: location.origin, fetchImpl,
    });
    expect(duplicate).toEqual({
      kind: "idle",
      last_event_id: "cursor-gen1-a",
      ingest: first.kind === "replayed" ? first.ingest : undefined,
    });
    const retainedJournal = await readJournalSnapshot(workspace);
    expired = true;
    const resynced = await consumeOwnedProjectActivity(workspace, {
      baseUrl: location.origin, fetchImpl,
    });
    expect(resynced).toEqual({
      kind: "resynchronized",
      last_event_id: "cursor-gen2-b",
      snapshot,
      ingest: {
        replay_generation: "2",
        processed_through_stream_sequence: "6",
        events: [eventB],
        held: [],
      },
    });
    expect(await readProjectActivityIngest(workspace)).toEqual(resynced.kind === "resynchronized"
      ? resynced.ingest : undefined);
    expect(await readJournalSnapshot(workspace)).toEqual(retainedJournal);
    expect(activity).toEqual([
      { snapshotId: EDITOR_SNAPSHOT, lastEventId: null },
      { snapshotId: EDITOR_SNAPSHOT, lastEventId: "cursor-gen1-a" },
      { snapshotId: EDITOR_SNAPSHOT, lastEventId: "cursor-gen1-a" },
      { snapshotId: CANONICAL_SNAPSHOT, lastEventId: null },
    ]);
    workspace.database.close();
  } finally {
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

it("does not treat an unrecoverable expired cursor as an empty successful replay", async () => {
  const scenario = createBrowserScenario();
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({
        ...scenario.session,
        schema_id: "storyos.query.editor-session.response.v1",
      });
    }
    if (path.endsWith("/activity")) return problemResponse(409, "activity_cursor_too_old");
    if (path.endsWith("/manuscript/tree")) {
      return jsonResponse(treeResponse(canonicalSnapshot({
        snapshot_id: EDITOR_SNAPSHOT,
        project_activity_position: "0",
        replay_generation: "1",
      })));
    }
    if (path.endsWith(`/snapshots/${EDITOR_SNAPSHOT}`)) {
      return problemResponse(404, "resource_unavailable");
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  try {
    const { workspace } = await openReadyWorkspace(fetchImpl);
    trackDatabase(workspace.database, openDatabases);
    const before = await readProjectActivityIngest(workspace);
    expect(await consumeOwnedProjectActivity(workspace, {
      baseUrl: location.origin, fetchImpl,
    })).toEqual({ kind: "unavailable", code: "resource_unavailable" });
    expect(await readProjectActivityIngest(workspace)).toEqual(before);
    workspace.database.close();
  } finally {
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

it("refuses a Snapshot whose Scope does not match the Editor Session", async () => {
  const scenario = createBrowserScenario();
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({
        ...scenario.session,
        schema_id: "storyos.query.editor-session.response.v1",
      });
    }
    if (path.endsWith("/activity")) return problemResponse(409, "activity_cursor_too_old");
    if (path.endsWith("/manuscript/tree")) {
      return jsonResponse(treeResponse(
        canonicalSnapshot({ project_scope: { owner_user_id: OWNER, project_id: OWNER } }),
        { owner_user_id: OWNER, project_id: OWNER },
      ));
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  try {
    const { workspace } = await openReadyWorkspace(fetchImpl);
    trackDatabase(workspace.database, openDatabases);
    const before = await readProjectActivityIngest(workspace);
    await expect(consumeOwnedProjectActivity(workspace, {
      baseUrl: location.origin, fetchImpl,
    })).rejects.toThrow();
    expect(await readProjectActivityIngest(workspace)).toEqual(before);
    workspace.database.close();
  } finally {
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

it("ingests assistance and Run Activity without disabling manual writing or skipping unknown Events", async () => {
  const scenario = createBrowserScenario();
  const assistancePayload = { kind: "project_assistance_updated", availability: "available", revision: "1" };
  const runPayload = {
    kind: "agent_run_created",
    project_agent_id: "018f0000-0000-7001-8000-000000000451",
    conversation_id: "018f0000-0000-7001-8000-000000000452",
    memory_settings_revision: "018f0000-0000-7001-8000-000000000453",
    run_id: "018f0000-0000-7001-8000-000000000454",
  };
  const digest = async (payload: Record<string, unknown>) => ({
    algorithm: "sha256" as const,
    profile: "storyos.event-payload.jcs.v1",
    value_hex_lowercase: [...new Uint8Array(await crypto.subtle.digest(
      "SHA-256", new TextEncoder().encode(JSON.stringify(payload)),
    ))].map((byte) => byte.toString(16).padStart(2, "0")).join(""),
  });
  const base = await createAppliedActivityEvent({
    eventId: "018f0000-0000-7001-8000-000000000455",
    commandId: "018f0000-0000-7001-8000-000000000456",
    receiptId: "018f0000-0000-7001-8000-000000000457",
    correlationId: "018f0000-0000-7001-8000-000000000458",
    revisionId: REVISION,
    commitId: "018f0000-0000-7001-8000-000000000459",
    sequence: "1",
    occurredAt: "2026-08-20T04:00:01.000Z",
  });
  const assistance = {
    ...base,
    event_schema: "storyos.event.project-assistance-updated.v1",
    event_kind: "project_assistance_updated",
    aggregate_ref: { kind: "project", id: PROJECT },
    payload: assistancePayload,
    payload_digest: await digest(assistancePayload),
  };
  const run = {
    ...base,
    event_id: "018f0000-0000-7001-8000-000000000460",
    application_wire_record_ref: "018f0000-0000-7001-8000-000000000460",
    event_schema: "storyos.event.agent-run-created.v1",
    event_kind: "agent_run_created",
    project_sequence: "2",
    stream_sequence: "2",
    aggregate_ref: { kind: "agent_run", id: runPayload.run_id },
    payload: runPayload,
    payload_digest: await digest(runPayload),
  };
  const unknown = {
    ...run,
    event_id: "018f0000-0000-7001-8000-000000000461",
    application_wire_record_ref: "018f0000-0000-7001-8000-000000000461",
    event_kind: "future_activity",
    event_schema: "storyos.event.future-activity.v1",
    project_sequence: "3",
    stream_sequence: "3",
  };
  const cursors: Array<string | null> = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse({
      nonce: "a".repeat(64), expires_at: "2026-08-13T08:05:00.000Z",
      limit_profile_revision: "storyos.foundation.absolute.v1",
    });
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse({
      ...scenario.session, schema_id: "storyos.query.editor-session.response.v1",
    });
    if (path.endsWith("/activity")) {
      cursors.push(new Headers(init?.headers).get("last-event-id"));
      const frame = [assistance, run, unknown, unknown][cursors.length - 1];
      if (frame === undefined) throw new Error("unexpected Activity request");
      return activitySse(`cursor-${cursors.length}`, frame);
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  try {
    const workspace = await openEditorWorkspace({
      baseUrl: location.origin, project: scenario.project, chapter: scenario.chapter,
      profile: scenario.profile, fetchImpl,
    });
    requireEditorReady(workspace);
    trackDatabase(workspace.database, openDatabases);
    const first = await consumeOwnedProjectActivity(workspace, { baseUrl: location.origin, fetchImpl });
    const second = await consumeOwnedProjectActivity(workspace, { baseUrl: location.origin, fetchImpl });
    expect(first.kind).toBe("replayed");
    expect(second.kind).toBe("replayed");
    expect((await readProjectActivityIngest(workspace)).events).toEqual([assistance, run]);
    await expect(consumeOwnedProjectActivity(workspace, {
      baseUrl: location.origin, fetchImpl,
    })).rejects.toThrow("Project Activity Event is invalid");
    await expect(consumeOwnedProjectActivity(workspace, {
      baseUrl: location.origin, fetchImpl,
    })).rejects.toThrow("Project Activity Event is invalid");
    expect(cursors).toEqual([null, "cursor-1", "cursor-2", "cursor-2"]);
    expect((await readProjectActivityIngest(workspace)).events).toEqual([assistance, run]);
    const before = await readJournalSnapshot(workspace);
    const projected = await persistReplaceSelection(workspace, {
      from: 4, to: 4, text: "!", resultingBody: "Base!",
      inputOrigin: "paste", undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-20T04:00:02.000Z",
    });
    expect(projected.body).toBe("Base!");
    const after = await readJournalSnapshot(workspace);
    expect(after.records).toHaveLength(before.records.length + 1);
    expect(after.records.at(-1)?.author_edit_unit).toEqual({
      normalized_primitives: [{
        kind: "replace_block_selection", manuscript_block_id: BLOCK, from: 4, to: 4, text: "!",
      }],
      selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 4, to: 4 },
    });
    workspace.database.close();
  } finally {
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
