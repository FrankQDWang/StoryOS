import { expect, it } from "vitest";

import { openEditorWorkspace } from "../../src/editor-session.ts";
import { rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import {
  ingestProjectActivityFrames,
  readProjectActivityIngest,
} from "../../src/project-activity-ingest.ts";
import type { ProjectActivityFrame } from "../../src/editor-types.ts";
import {
  CHAPTER,
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

const EVENT_A = "018f0000-0000-7001-8000-000000000371";
const EVENT_B = "018f0000-0000-7001-8000-000000000372";

it("converges duplicate and reordered Activity frames to one durable ingest", async () => {
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
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const frame = (id: string, data: unknown): ProjectActivityFrame => ({
    id,
    event: "storyos.project-activity",
    data,
  });
  const openDatabases = new Set<IDBDatabase>();

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
    trackDatabase(workspace.database, openDatabases);
    const eventA = await createAppliedActivityEvent({
      eventId: EVENT_A,
      commandId: "018f0000-0000-7001-8000-000000000373",
      receiptId: "018f0000-0000-7001-8000-000000000375",
      correlationId: "018f0000-0000-7001-8000-000000000381",
      revisionId: "018f0000-0000-7001-8000-000000000377",
      commitId: "018f0000-0000-7001-8000-000000000379",
      sequence: "1",
      occurredAt: "2026-08-20T03:00:00.000Z",
    });
    const eventB = await createAppliedActivityEvent({
      eventId: EVENT_B,
      commandId: "018f0000-0000-7001-8000-000000000374",
      receiptId: "018f0000-0000-7001-8000-000000000376",
      correlationId: "018f0000-0000-7001-8000-000000000382",
      revisionId: "018f0000-0000-7001-8000-000000000378",
      commitId: "018f0000-0000-7001-8000-000000000380",
      sequence: "2",
      occurredAt: "2026-08-20T03:00:00.001Z",
    });
    const expectedProjection = {
      body: "Base",
      save_state: "clean",
      unsettled_intent_count: 0,
      authoritative_revision_id: REVISION,
    };

    await ingestProjectActivityFrames(workspace, [frame("cursor-b", eventB)]);
    expect(await readProjectActivityIngest(workspace)).toEqual({
      replay_generation: "1",
      processed_through_stream_sequence: "0",
      events: [],
      held: [eventB],
    });
    await ingestProjectActivityFrames(workspace, [
      frame("cursor-a", eventA),
      frame("cursor-a", eventA),
    ]);
    const contiguous = {
      replay_generation: "1",
      processed_through_stream_sequence: "2",
      events: [eventA, eventB],
      held: [],
    };
    expect(await readProjectActivityIngest(workspace)).toEqual(contiguous);
    await ingestProjectActivityFrames(workspace, [
      frame("cursor-b", eventB),
      frame("cursor-a", eventA),
    ]);
    expect(await readProjectActivityIngest(workspace)).toEqual(contiguous);
    expect(await rebuildPendingProjection(workspace)).toEqual(expectedProjection);
    workspace.database.close();

    await deleteJournal(scenario.journalName);
    const inOrder = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter: scenario.chapter,
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    requireEditorReady(inOrder);
    trackDatabase(inOrder.database, openDatabases);
    await ingestProjectActivityFrames(inOrder, [
      frame("cursor-a", eventA),
      frame("cursor-b", eventB),
    ]);
    expect(await readProjectActivityIngest(inOrder)).toEqual(contiguous);
    inOrder.database.close();
  } finally {
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
