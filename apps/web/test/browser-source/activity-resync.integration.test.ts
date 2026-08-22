import { expect, it } from "vitest";

import type {
  GetSnapshotResponse,
  SnapshotDescriptor,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  openEditorWorkspace,
  persistReplaceSelection,
} from "../../src/editor-session.ts";
import {
  readJournalSnapshot,
  rebuildPendingProjection,
} from "../../src/local-edit-journal.ts";
import {
  ingestProjectActivityFrames,
  readProjectActivityIngest,
  resyncProjectActivityFromSnapshot,
} from "../../src/project-activity-ingest.ts";
import {
  OWNER,
  PROJECT,
  REVISION,
  SESSION,
  createAppliedActivityEvent,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  requireEditorReady,
} from "./scenario.ts";

const CANONICAL_SNAPSHOT = "018f0000-0000-7001-8000-000000000390";

it("preserves local payload and resumes after a new Snapshot generation", async () => {
  const scenario = createBrowserScenario();
  const canonicalSnapshot: SnapshotDescriptor = {
    snapshot_id: CANONICAL_SNAPSHOT,
    project_scope: scenario.project.project_scope,
    snapshot_kind: "canonical",
    project_activity_position: "5",
    source_watermarks: {},
    projection_generations: {},
    redaction_profile: "storyos.author.v1",
    schema_profile: "storyos.public.release.1",
    replay_generation: "2",
    created_at: "2026-08-20T04:00:00.000Z",
    expires_at: null,
  };
  const snapshotResponse: GetSnapshotResponse = {
    schema_id: "storyos.query.snapshot.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000391",
    project_scope: scenario.project.project_scope,
    snapshot: canonicalSnapshot,
  };
  const counts = { activity: 0, snapshots: 0 };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    expect(new Headers(init?.headers).has("last-event-id")).toBe(false);
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
    if (path.endsWith(`/snapshots/${CANONICAL_SNAPSHOT}`)) {
      counts.snapshots += 1;
      return jsonResponse(snapshotResponse);
    }
    if (path.endsWith("/activity")) {
      counts.activity += 1;
      throw new Error("resync must not resume the old Activity cursor");
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };

  await deleteJournal(scenario.journalName);
  try {
    const workspace = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter: scenario.chapter,
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    requireEditorReady(workspace);
    const pending = await persistReplaceSelection(workspace, {
      from: 4,
      to: 4,
      text: "!?",
      resultingBody: "Base!?",
      inputOrigin: "paste",
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    });
    const expectedProjection = {
      body: "Base!?",
      save_state: "saving",
      unsettled_intent_count: 1,
      authoritative_revision_id: REVISION,
    };
    expect(pending).toEqual(expectedProjection);
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
      occurredAt: "2026-08-20T04:00:01.000Z",
    });
    await ingestProjectActivityFrames(workspace, [{
      id: "cursor-gen1-a",
      event: "storyos.project-activity",
      data: eventA,
    }]);
    const retainedJournal = await readJournalSnapshot(workspace);
    expect(retainedJournal.records).toHaveLength(1);
    expect(retainedJournal.payloadChains).toHaveLength(1);
    expect(await readProjectActivityIngest(workspace)).toEqual({
      replay_generation: "1",
      processed_through_stream_sequence: "1",
      events: [eventA],
      held: [],
    });
    const expectedIngest = {
      replay_generation: "2",
      processed_through_stream_sequence: "5",
      events: [],
      held: [],
    };
    expect(await resyncProjectActivityFromSnapshot(workspace, {
      baseUrl: location.origin,
      snapshotId: CANONICAL_SNAPSHOT,
      fetchImpl,
    })).toEqual({ snapshot: canonicalSnapshot, ingest: expectedIngest });
    expect(await readProjectActivityIngest(workspace)).toEqual(expectedIngest);
    await ingestProjectActivityFrames(workspace, [{
      id: "cursor-gen2-b",
      event: "storyos.project-activity",
      data: eventB,
    }]);
    expect(await readProjectActivityIngest(workspace)).toEqual({
      replay_generation: "2",
      processed_through_stream_sequence: "6",
      events: [eventB],
      held: [],
    });
    expect(await readJournalSnapshot(workspace)).toEqual(retainedJournal);
    expect(await rebuildPendingProjection(workspace)).toEqual(expectedProjection);
    expect(counts).toEqual({ activity: 0, snapshots: 1 });
    workspace.database.close();
  } finally {
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
