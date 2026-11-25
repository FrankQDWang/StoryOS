import { afterEach, expect, it } from "vitest";

import type {
  GetChapterResponse,
  GetEditorSessionResponse,
  SnapshotDescriptor,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { openEditorWorkspace } from "../../src/editor-session.ts";
import { createEditorSessionWritingController, type EditorSessionWritingController }
  from "../../src/editor-session-writing.ts";
import type { EditorReadyState } from "../../src/editor-types.ts";
import { readJournalSnapshot } from "../../src/local-edit-journal.ts";
import { consumeOwnedProjectActivity } from "../../src/project-activity-sync.ts";
import {
  OWNER, PROJECT, SESSION, chapterRevision, closeTrackedDatabases, createAppliedActivityEvent, createBrowserScenario,
  deleteJournal, jsonResponse, requestResult, requireEditorReady, trackDatabase,
} from "./scenario.ts";

// An open with writer generation 2 that sends none of the three reads of the takeover check is the open of generation
// 1 with the form checks and the partition checks (#1897, decisions 1 and 2 of the coordinator).
const P = `/api/v1/projects/${PROJECT}`;
const CHAPTER_B = "018f0000-0000-7001-8000-000000000841";
const REVISION_B = "018f0000-0000-7001-8000-000000000842";
const BLOCK_B = "018f0000-0000-7001-8000-000000000843";
const CANONICAL = "018f0000-0000-7001-8000-000000000844";
const SWITCH_SNAPSHOT = "018f0000-0000-7001-8000-000000000845";
const SESSION_READ = `GET ${P}/editor-sessions/${SESSION}`;
const TREE = `GET ${P}/manuscript/tree`;
const SNAPSHOT = `GET ${P}/snapshots/${CANONICAL}`;
const ACTIVITY = `GET ${P}/activity`;
const CHALLENGE = `POST ${P}/anti-forgery-challenges`;
const AUTHOR_EDIT = `POST ${P}/manuscript/author-edits`;
const outcomeQuery = (idempotencyKey: string) => `GET ${P}/manuscript/author-edit-outcomes/${idempotencyKey}`;
const EXPIRES_AT = "2026-08-13T08:05:00.000Z";

const hex = async (text: string) => [...new Uint8Array(await crypto.subtle.digest("SHA-256",
  new TextEncoder().encode(text)))].map((byte) => byte.toString(16).padStart(2, "0")).join("");

/**
 * A fake Server of one Project whose Editor Session holds writer generation 2. It serves only the exact requests of
 * these tests and records each other request. `switchChapter` settles a Chapter switch to Chapter B, as the Server
 * does: the Session base is the Snapshot of the switch, and the Activity stream has the next Event. After
 * `takeOverElsewhere`, another Editor Session holds generation 3, this Session is read-only, and the Server refuses
 * an author edit of this Session (`editor_writer_stale`).
 */
async function fakeServer() {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  const eventA = await createAppliedActivityEvent({ eventId: "018f0000-0000-7001-8000-000000000846",
    commandId: "018f0000-0000-7001-8000-000000000847", receiptId: "018f0000-0000-7001-8000-000000000848",
    correlationId: "018f0000-0000-7001-8000-000000000849", revisionId: "018f0000-0000-7001-8000-00000000084a",
    commitId: "018f0000-0000-7001-8000-00000000084b", sequence: "1", occurredAt: "2026-08-20T03:00:00.000Z" });
  const eventB = await createAppliedActivityEvent({ eventId: "018f0000-0000-7001-8000-00000000084c",
    commandId: "018f0000-0000-7001-8000-00000000084d", receiptId: "018f0000-0000-7001-8000-00000000084e",
    correlationId: "018f0000-0000-7001-8000-00000000084f", revisionId: "018f0000-0000-7001-8000-000000000850",
    commitId: "018f0000-0000-7001-8000-000000000851", sequence: "2", authorActionSequence: "2",
    occurredAt: "2026-08-20T04:00:00.000Z" });
  const chapterB: GetChapterResponse = { ...scenario.chapter, project_activity_position: "2",
    chapter: { chapter_id: CHAPTER_B, title: "Chapter B", current_revision: chapterRevision(REVISION_B, "B", BLOCK_B) } };
  const baseB = { ...scenario.session.base_snapshot, snapshot_id: SWITCH_SNAPSHOT, chapter_id: CHAPTER_B,
    project_activity_position: "2", authoritative_head_revision_id: REVISION_B, target_refs: [`manuscript:${CHAPTER_B}`],
    materialized_revision: chapterB.chapter.current_revision,
    materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
      value_hex_lowercase: await hex("B") } };
  let session: GetEditorSessionResponse = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1",
    writer: { kind: "current_writer", writer_generation: "2" } };
  const canonical: SnapshotDescriptor = { snapshot_id: CANONICAL, project_scope: scope, snapshot_kind: "canonical",
    project_activity_position: "0", source_watermarks: {}, projection_generations: {},
    redaction_profile: "storyos.author.v1", schema_profile: "storyos.public.release.1", replay_generation: "2",
    created_at: "2026-08-20T02:00:00.000Z", expires_at: null };
  const stream = [{ id: "cursor-a", event: eventA }];
  const log = { requests: [] as string[], unexpected: [] as string[],
    activity: [] as { snapshotId: string | null; lastEventId: string | null }[] };
  let idempotencyKey: string | undefined;
  const fetchImpl: typeof fetch = async (input, init) => {
    const url = new URL(input instanceof Request ? input.url : input);
    const name = `${init?.method ?? "GET"} ${url.pathname}`;
    log.requests.push(name);
    if (name === SESSION_READ) return jsonResponse(session);
    if (name === TREE) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1", correlation_id: SESSION,
        project_scope: scope, tree_revision: "2", snapshot: canonical, volumes: [] });
    }
    if (name === SNAPSHOT) {
      return jsonResponse({ schema_id: "storyos.query.snapshot.response.v1", correlation_id: SESSION,
        project_scope: scope, snapshot: canonical });
    }
    if (name === ACTIVITY) {
      const lastEventId = new Headers(init?.headers).get("last-event-id");
      log.activity.push({ snapshotId: url.searchParams.get("snapshot_id"), lastEventId });
      const next = stream.slice(stream.findIndex((frame) => frame.id === lastEventId) + 1);
      return new Response(next.map(({ id, event }) =>
        `id: ${id}\nevent: storyos.project-activity\ndata: ${JSON.stringify(event)}\n\n`).join(""),
      { status: 200, headers: { "content-type": "text/event-stream" } });
    }
    if (name === CHALLENGE) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: EXPIRES_AT,
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (name === AUTHOR_EDIT && session.writer.kind === "read_only") {
      idempotencyKey = new Headers(init?.headers).get("idempotency-key") ?? undefined;
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "editor_writer_stale",
        message: "The Editor Session is not the current writer." }, 412);
    }
    if (idempotencyKey !== undefined && name === outcomeQuery(idempotencyKey)) {
      return jsonResponse({ schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000852", project_scope: scope,
        outcome: { outcome_kind: "still_unknown", observation: { observation_kind: "challenge_issued",
          expires_at: EXPIRES_AT } } });
    }
    log.unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };
  return {
    scenario, log, fetchImpl, chapterB, eventA, eventB,
    switchChapter() {
      session = { ...session, base_snapshot: baseB };
      stream.push({ id: "cursor-b", event: eventB });
    },
    takeOverElsewhere() {
      session = { ...session, writer: { kind: "read_only", observed_writer_generation: "3",
        reason: "superseded_by_takeover" } };
    },
  };
}

const databases = new Set<IDBDatabase>();
let controller: EditorSessionWritingController | undefined;

afterEach(async () => {
  controller?.close();
  controller = undefined;
  closeTrackedDatabases(databases);
  sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
  await deleteJournal(createBrowserScenario().journalName);
});

/**
 * Opens the Editor Session of the tab with generation 2, which runs the takeover check, then takes the first Event.
 * Then the Chapter switch settles, and the open of Chapter B gives the partition of the first open as the last ready
 * open of this page.
 */
async function openAfterSwitch(server: Awaited<ReturnType<typeof fakeServer>>) {
  const { scenario, fetchImpl } = server;
  await deleteJournal(scenario.journalName);
  sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  const options = { baseUrl: location.origin, project: scenario.project, profile: scenario.profile, fetchImpl };
  const first = await openEditorWorkspace({ ...options, chapter: scenario.chapter });
  requireEditorReady(first);
  trackDatabase(first.database, databases);
  const firstActivity = await consumeOwnedProjectActivity(first, { baseUrl: location.origin, fetchImpl });
  server.switchChapter();
  const before = server.log.requests.length;
  const switched = await openEditorWorkspace({ ...options, chapter: server.chapterB,
    readyPartitionId: first.partition.journal_partition_id });
  requireEditorReady(switched);
  trackDatabase(switched.database, databases);
  return { first, firstActivity, switched, switchReads: server.log.requests.slice(before) };
}

it("takes the next Activity Event from the stream after a Chapter switch with no read of the takeover check",
  async () => {
    const server = await fakeServer();
    const { first, firstActivity, switched, switchReads } = await openAfterSwitch(server);

    const next = await consumeOwnedProjectActivity(switched, { baseUrl: location.origin, fetchImpl: server.fetchImpl });

    expect({ firstActivity, next, switchReads, partition: switched.partition, activity: server.log.activity,
      unexpected: server.log.unexpected }).toEqual({
      firstActivity: { kind: "replayed", last_event_id: "cursor-a", ingest: { replay_generation: "2",
        processed_through_stream_sequence: "1", events: [server.eventA], held: [] } },
      next: { kind: "replayed", last_event_id: "cursor-b", ingest: { replay_generation: "2",
        processed_through_stream_sequence: "2", events: [server.eventA, server.eventB], held: [] } },
      switchReads: [SESSION_READ],
      partition: first.partition,
      activity: [{ snapshotId: server.scenario.session.base_snapshot.snapshot_id, lastEventId: null },
        { snapshotId: server.scenario.session.base_snapshot.snapshot_id, lastEventId: "cursor-a" }],
      unexpected: [],
    });
  });

it("makes the tab read-only and keeps the typed text when the Session lost the writer after an open with no check read",
  async () => {
    const server = await fakeServer();
    const { switched, switchReads } = await openAfterSwitch(server);
    const workspace: EditorReadyState = switched;
    const partition = structuredClone(workspace.partition);
    const failures: unknown[] = [];
    controller = createEditorSessionWritingController({ workspace, baseUrl: location.origin,
      fetchImpl: server.fetchImpl, setTimeoutImpl: () => 1, clearTimeoutImpl: () => {},
      onFailure: (error) => { failures.push(error); } });
    // Another tab takes over the writer after the open.
    server.takeOverElsewhere();

    controller.capture({ from: 1, to: 1, text: " typed", resultingBody: "B typed" }, "typing");
    await controller.whenIdle();
    await controller.flush();

    const journal = await readJournalSnapshot(workspace);
    expect({ switchReads, input: controller.canAcceptInput(false), body: controller.snapshot().projection.body,
      partition: workspace.partition, stored: await requestResult(workspace.database.transaction("partitions")
        .objectStore("partitions").get(partition.journal_partition_id)),
      records: journal.records.length, groups: journal.groups.length, failures: failures.length,
      commands: server.log.requests.filter((name) => name === AUTHOR_EDIT).length,
      unexpected: server.log.unexpected }).toEqual({
      switchReads: [SESSION_READ], input: false, body: "B typed",
      partition: { ...partition, disposition: "read_only_observer" },
      stored: { ...partition, disposition: "read_only_observer" },
      records: 1, groups: 1, failures: 1, commands: 1, unexpected: [] });
  });
