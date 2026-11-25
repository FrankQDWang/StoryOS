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
  CHAPTER, EDITOR_SNAPSHOT, OWNER, PROJECT, SESSION, chapterRevision, closeTrackedDatabases, createAppliedActivityEvent, createBrowserScenario,
  deleteJournal, jsonResponse, requestResult, requireEditorReady, trackDatabase,
} from "./scenario.ts";

// An open with writer generation 2 that sends none of the three reads of the takeover check is the open of generation
// 1 with the form checks and the partition checks (#1897, decisions 1 and 2 of the coordinator).
const P = `/api/v1/projects/${PROJECT}`;
const CHAPTER_B = "018f0000-0000-7001-8000-000000000841";
const VOLUME = "018f0000-0000-7001-8000-000000000853";
const REVISION_B = "018f0000-0000-7001-8000-000000000842";
const BLOCK_B = "018f0000-0000-7001-8000-000000000843";
const SWITCH_SNAPSHOT = "018f0000-0000-7001-8000-000000000845";
const SESSION_READ = `GET ${P}/editor-sessions/${SESSION}`;
const TREE = `GET ${P}/manuscript/tree`;
const snapshotRead = (snapshotId: string) => `GET ${P}/snapshots/${snapshotId}`;
const ACTIVITY = `GET ${P}/activity`;
const CHALLENGE = `POST ${P}/anti-forgery-challenges`;
const AUTHOR_EDIT = `POST ${P}/manuscript/author-edits`;
const outcomeQuery = (idempotencyKey: string) => `GET ${P}/manuscript/author-edit-outcomes/${idempotencyKey}`;

const hex = async (text: string) => [...new Uint8Array(await crypto.subtle.digest("SHA-256",
  new TextEncoder().encode(text)))].map((byte) => byte.toString(16).padStart(2, "0")).join("");

/**
 * An Activity Event of `kind` in the form of the stream of the Server: the aggregate of the kind and the payload with its
 * `kind` field, digested with sorted keys.
 */
async function activityEvent(kind: "chapter_updated" | "current_chapter_set", schema: string, sequence: string,
  ids: string[], aggregateId: string, fields: Record<string, string>) {
  const [eventId, commandId, receiptId, correlationId] = ids as [string, string, string, string];
  const event = await createAppliedActivityEvent({ eventId, commandId, receiptId, correlationId,
    revisionId: eventId, commitId: commandId, sequence, occurredAt: `2026-08-20T0${sequence}:00:00.000Z` });
  const payload = Object.fromEntries(Object.entries({ ...fields, kind }).sort(([left], [right]) =>
    left < right ? -1 : 1));
  return { ...event, event_kind: kind, event_schema: schema, aggregate_ref: { kind: "chapter", id: aggregateId },
    payload, payload_digest: { ...event.payload_digest, value_hex_lowercase: await hex(JSON.stringify(payload)) } };
}

/**
 * A fake Server of one Project whose Editor Session holds writer generation 2. The takeover settled the Snapshot of the
 * Session base at position 1. The fake serves only the exact requests of these tests and records each other request.
 * Its stream has a rename of Chapter A in another tab at position 2. `switchChapter` settles a Chapter switch to Chapter
 * B, as the Server does: the Session base and the canonical Snapshot are the Snapshot of the switch at position 3, the
 * stream has the `current_chapter_set` Event of the switch, and then a rename of Chapter B in another tab. A stream
 * request starts after its `Last-Event-ID`, or else after the position of its Snapshot. After `takeOverElsewhere`, another Editor Session holds
 * generation 3, this Session is read-only, and the Server refuses an author edit of this Session
 * (`editor_writer_stale`).
 */
async function fakeServer() {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  const eventA = await activityEvent("chapter_updated", "storyos.event.chapter-updated.v1", "2",
    ["018f0000-0000-7001-8000-000000000846", "018f0000-0000-7001-8000-000000000847",
      "018f0000-0000-7001-8000-000000000848", "018f0000-0000-7001-8000-000000000849"], CHAPTER,
    { chapter_id: CHAPTER, title: "Chapter A, renamed", tree_revision: "3", order: "1", prior_title: "Chapter A",
      prior_order: "1" });
  const eventB = await activityEvent("current_chapter_set", "storyos.event.current-chapter-set.v1", "3",
    ["018f0000-0000-7001-8000-00000000084c", "018f0000-0000-7001-8000-00000000084d",
      "018f0000-0000-7001-8000-00000000084e", "018f0000-0000-7001-8000-00000000084f"], CHAPTER_B,
    { prior_chapter_id: CHAPTER, current_chapter_id: CHAPTER_B, base_snapshot_id: SWITCH_SNAPSHOT });
  const eventC = await activityEvent("chapter_updated", "storyos.event.chapter-updated.v1", "4",
    ["018f0000-0000-7001-8000-000000000856", "018f0000-0000-7001-8000-000000000857",
      "018f0000-0000-7001-8000-000000000858", "018f0000-0000-7001-8000-000000000859"], CHAPTER_B,
    { chapter_id: CHAPTER_B, title: "Chapter B, renamed", tree_revision: "4", order: "2", prior_title: "Chapter B",
      prior_order: "2" });
  const chapterA: GetChapterResponse = { ...scenario.chapter, project_activity_position: "1" };
  const chapterB: GetChapterResponse = { ...scenario.chapter, project_activity_position: "3",
    chapter: { chapter_id: CHAPTER_B, title: "Chapter B", current_revision: chapterRevision(REVISION_B, "B", BLOCK_B) } };
  const baseB = { ...scenario.session.base_snapshot, snapshot_id: SWITCH_SNAPSHOT, chapter_id: CHAPTER_B,
    project_activity_position: "3", authoritative_head_revision_id: REVISION_B, target_refs: [`manuscript:${CHAPTER_B}`],
    materialized_revision: chapterB.chapter.current_revision, created_at: "2026-08-20T02:00:00.000Z",
    materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
      value_hex_lowercase: await hex("B") } };
  let session: GetEditorSessionResponse = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1",
    writer: { kind: "current_writer", writer_generation: "2" },
    base_snapshot: { ...scenario.session.base_snapshot, project_activity_position: "1" } };
  // The canonical Snapshot is the base of the Session.
  const descriptor = ({ snapshot_id, project_activity_position, created_at }: typeof baseB): SnapshotDescriptor => ({
    snapshot_id, project_scope: scope, snapshot_kind: "canonical", project_activity_position, source_watermarks: {},
    projection_generations: {}, redaction_profile: "storyos.author.v1", schema_profile: "storyos.public.release.1",
    replay_generation: "2", created_at, expires_at: null });
  let canonical = descriptor(session.base_snapshot);
  const positions = new Map([[EDITOR_SNAPSHOT, 1], [SWITCH_SNAPSHOT, 3]]);
  const stream = [{ id: "cursor-a", event: eventA }];
  const log = { requests: [] as string[], unexpected: [] as string[],
    activity: [] as { snapshotId: string | null; lastEventId: string | null }[] };
  let idempotencyKey: string | undefined;
  let challengeExpiresAt = "";
  const fetchImpl: typeof fetch = async (input, init) => {
    const url = new URL(input instanceof Request ? input.url : input);
    const name = `${init?.method ?? "GET"} ${url.pathname}`;
    log.requests.push(name);
    if (name === SESSION_READ) return jsonResponse(session);
    if (name === TREE) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000854", project_scope: scope,
        tree_revision: canonical.snapshot_id === SWITCH_SNAPSHOT ? "3" : "2",
        snapshot: canonical, volumes: [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [
          { chapter_id: CHAPTER, title: canonical.snapshot_id === SWITCH_SNAPSHOT ? "Chapter A, renamed" : "Chapter A",
            order: "1" }, { chapter_id: CHAPTER_B, title: "Chapter B",
            order: "2" }] }] });
    }
    if (name === snapshotRead(canonical.snapshot_id)) {
      return jsonResponse({ schema_id: "storyos.query.snapshot.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000855", project_scope: scope, snapshot: canonical });
    }
    if (name === ACTIVITY) {
      const lastEventId = new Headers(init?.headers).get("last-event-id");
      log.activity.push({ snapshotId: url.searchParams.get("snapshot_id"), lastEventId });
      const after = lastEventId === null ? positions.get(url.searchParams.get("snapshot_id") ?? "")
        : Number(stream.find((frame) => frame.id === lastEventId)?.event.stream_sequence);
      const next = stream.filter(({ event }) => Number(event.stream_sequence) > (after ?? Infinity));
      return new Response(next.map(({ id, event }) =>
        `id: ${id}\nevent: storyos.project-activity\ndata: ${JSON.stringify(event)}\n\n`).join(""),
      { status: 200, headers: { "content-type": "text/event-stream" } });
    }
    if (name === CHALLENGE) {
      challengeExpiresAt = new Date(Date.now() + 60_000).toISOString();
      return jsonResponse({ nonce: "a".repeat(64), expires_at: challengeExpiresAt,
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
          expires_at: challengeExpiresAt } } });
    }
    log.unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };
  return {
    scenario, log, fetchImpl, chapterA, chapterB, eventA, eventB, eventC,
    switchChapter() {
      session = { ...session, base_snapshot: baseB };
      canonical = descriptor(baseB);
      stream.push({ id: "cursor-b", event: eventB }, { id: "cursor-c", event: eventC });
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
 * Opens the Editor Session of the tab with generation 2, which runs the takeover check, then takes the first Event when
 * `consume` is true. Then the Chapter switch settles, and the open of Chapter B gives the partition of the first open as
 * the last ready open of this page.
 */
async function openAfterSwitch(server: Awaited<ReturnType<typeof fakeServer>>, consume = true) {
  const { scenario, fetchImpl } = server;
  await deleteJournal(scenario.journalName);
  sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  const options = { baseUrl: location.origin, project: scenario.project, profile: scenario.profile, fetchImpl };
  const first = await openEditorWorkspace({ ...options, chapter: server.chapterA });
  requireEditorReady(first);
  trackDatabase(first.database, databases);
  const firstActivity = consume ? await consumeOwnedProjectActivity(first, { baseUrl: location.origin, fetchImpl })
    : undefined;
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
        processed_through_stream_sequence: "2", events: [server.eventA], held: [] } },
      next: { kind: "replayed", last_event_id: "cursor-c", ingest: { replay_generation: "2",
        processed_through_stream_sequence: "4", events: [server.eventA, server.eventB, server.eventC], held: [] } },
      switchReads: [SESSION_READ],
      partition: first.partition,
      activity: [{ snapshotId: server.scenario.session.base_snapshot.snapshot_id, lastEventId: null },
        { snapshotId: server.scenario.session.base_snapshot.snapshot_id, lastEventId: "cursor-a" }],
      unexpected: [],
    });
  });

it("runs the takeover check at a Chapter switch before the first stream cursor, and takes the next Event",
  async () => {
    const server = await fakeServer();
    const { switched, switchReads } = await openAfterSwitch(server, false);

    const next = await consumeOwnedProjectActivity(switched, { baseUrl: location.origin, fetchImpl: server.fetchImpl });

    // The open staged the Snapshot of the switch, so the stream and the ingest record start at position 3.
    expect({ next, switchReads, activity: server.log.activity, unexpected: server.log.unexpected }).toEqual({
      next: { kind: "replayed", last_event_id: "cursor-c", ingest: { replay_generation: "2",
        processed_through_stream_sequence: "4", events: [server.eventC], held: [] } },
      switchReads: [SESSION_READ, TREE, snapshotRead(SWITCH_SNAPSHOT), SESSION_READ],
      activity: [{ snapshotId: SWITCH_SNAPSHOT, lastEventId: null }],
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
