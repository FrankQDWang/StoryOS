import { expect, it } from "vitest";

import type {
  DigestValue,
  GetEditorSessionResponse,
  GetManuscriptTreeResponse,
  GetSnapshotResponse,
  SnapshotDescriptor,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  freezeOneIntentSubmission,
  openEditorWorkspace,
  persistReplaceSelection,
} from "../../src/editor-session.ts";
import {
  JOURNAL_OBJECT_STORES,
  readJournalSnapshot,
  rebuildPendingProjection,
} from "../../src/local-edit-journal.ts";
import {
  ingestProjectActivityFrames,
  readProjectActivityIngest,
  resyncProjectActivityFromSnapshot,
} from "../../src/project-activity-ingest.ts";
import { attachManualInput } from "../../src/manual-input.ts";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import {
  OWNER,
  PROJECT,
  REVISION,
  SESSION,
  closeTrackedDatabases,
  createAppliedAuthorEditResponse,
  createAppliedActivityEvent,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  requestResult,
  requireDigestValue,
  requireEditorReady,
  requireRequestBody,
  requireString,
  trackDatabase,
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
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

it.each([
  "valid", "valid_other_session", "scope", "locator_scope", "session", "generation", "binding",
  "reused_base", "digest", "snapshot", "position", "expired", "drift",
])("resumes a Takeover winner without rebinding old journal evidence: %s", async (fault) => {
  const scenario = createBrowserScenario();
  const otherSession = "018f0000-0000-7001-8000-000000000414";
  const interleaved = fault === "valid_other_session";
  const expectedBody = interleaved ? "Base!?" : "Base!";
  const databases = new Set<IDBDatabase>();
  const requests: string[] = [];
  const failures: unknown[] = [];
  let resuming = false;
  let sessionReads = 0;
  let commands = 0;
  let commandDigest: DigestValue | undefined;
  const session: GetEditorSessionResponse = {
    ...structuredClone(scenario.session), schema_id: "storyos.query.editor-session.response.v1",
    writer: { kind: "current_writer", writer_generation: "3" },
    base_snapshot: { ...scenario.session.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000410",
      project_activity_position: "5", created_at: "2026-08-20T04:00:00.000Z" },
  };
  const chapter = { ...structuredClone(scenario.chapter), project_activity_position: "5" };
  const canonical: SnapshotDescriptor = {
    snapshot_id: CANONICAL_SNAPSHOT, project_scope: scenario.project.project_scope,
    snapshot_kind: "canonical", project_activity_position: fault === "position" ? "6" : "5",
    source_watermarks: {}, projection_generations: {}, redaction_profile: "storyos.author.v1",
    schema_profile: "storyos.public.release.1", replay_generation: "2",
    created_at: "2026-08-20T04:00:00.000Z",
    expires_at: fault === "expired" ? "2020-01-01T00:00:00.000Z" : null,
  };
  if (fault === "scope") session.project_scope.project_id = OWNER;
  if (fault === "session") session.editor_session.editor_session_id = OWNER;
  if (interleaved) session.editor_session.editor_session_id = otherSession;
  if (fault === "generation") session.writer = { kind: "current_writer", writer_generation: "03" };
  if (fault === "binding") session.editor_session.client_session_binding_ref = "binding:changed";
  if (fault === "reused_base") session.base_snapshot.snapshot_id = scenario.session.base_snapshot.snapshot_id;
  if (fault === "digest") session.base_snapshot.materialized_payload_digest = {
    ...session.base_snapshot.materialized_payload_digest, value_hex_lowercase: "0".repeat(64),
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    requests.push(`${init?.method ?? "GET"} ${path}`);
    expect(new Headers(init?.headers).has("last-event-id")).toBe(false);
    if (path.endsWith("/anti-forgery-challenges")) {
      commandDigest = requireDigestValue(
        requireRequestBody(init).canonical_command_digest, "command digest",
      );
      return jsonResponse({ nonce: "a".repeat(64),
        expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`) || path.endsWith(`/editor-sessions/${otherSession}`)) {
      sessionReads += 1;
      const current = structuredClone(session);
      if (fault === "drift" && sessionReads > 1) {
        current.writer = { kind: "current_writer", writer_generation: "4" };
        current.base_snapshot = { ...current.base_snapshot,
          snapshot_id: "018f0000-0000-7001-8000-000000000411", project_activity_position: "6" };
      }
      return jsonResponse(resuming ? current : scenario.session);
    }
    if (path.endsWith("/manuscript/tree")) {
      const tree: GetManuscriptTreeResponse = {
        schema_id: "storyos.query.manuscript-tree.response.v1", correlation_id: SESSION,
        project_scope: fault === "locator_scope"
          ? { owner_user_id: OWNER, project_id: OWNER } : scenario.project.project_scope,
        tree_revision: "2", snapshot: canonical,
        volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000412",
          title: "Volume A", order: "0", chapters: [{ chapter_id: chapter.chapter.chapter_id,
            title: chapter.chapter.title, order: "0" }] }],
      };
      return jsonResponse(tree);
    }
    if (path.endsWith(`/snapshots/${canonical.snapshot_id}`)) {
      return jsonResponse({ schema_id: "storyos.query.snapshot.response.v1",
        correlation_id: SESSION, project_scope: scenario.project.project_scope,
        snapshot: fault === "snapshot" ? { ...canonical, snapshot_id: OWNER } : canonical });
    }
    if (path.endsWith("/manuscript/author-edits")) {
      commands += 1;
      if (!commandDigest) throw new Error("The command challenge is missing");
      const response = createAppliedAuthorEditResponse({ request: requireRequestBody(init),
        commandDigest, idempotencyKey: requireString(new Headers(init?.headers)
          .get("idempotency-key"), "idempotency key"), body: expectedBody, projectActivityPosition: "6" });
      if (response.effect.kind !== "authoritative_applied") throw new Error("The edit did not apply");
      response.receipt.author_action_sequence = "1";
      response.effect.author_action_sequence = "1";
      const digest = new Uint8Array(await crypto.subtle.digest(
        "SHA-256", new TextEncoder().encode(response.effect.authoritative_revision.body),
      ));
      session.base_snapshot = { ...session.base_snapshot,
        snapshot_id: "018f0000-0000-7001-8000-000000000413", project_activity_position: "6",
        authoritative_head_revision_id: response.effect.authoritative_revision.revision_id,
        materialized_revision: response.effect.authoritative_revision,
        materialized_payload_digest: { ...session.base_snapshot.materialized_payload_digest,
          value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("") },
        created_at: new Date().toISOString() };
      canonical.snapshot_id = session.base_snapshot.snapshot_id;
      canonical.project_activity_position = "6";
      canonical.created_at = session.base_snapshot.created_at;
      return jsonResponse(response);
    }
    throw new Error(`unexpected request: ${path}`);
  };
  let controller: ReturnType<typeof attachManualInput> | undefined;
  const editor = document.createElement("textarea");
  await deleteJournal(scenario.journalName);
  try {
    const old = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
      chapter: scenario.chapter, profile: scenario.profile, fetchImpl });
    requireEditorReady(old);
    trackDatabase(old.database, databases);
    const edit = { from: 4, to: 4, text: " retained", resultingBody: "Base retained",
      undoGroupId: "018f0000-0000-7001-8000-000000000415", createdAt: "2026-08-20T05:00:00.000Z" };
    await persistReplaceSelection(old, edit);
    if (!interleaved) await freezeOneIntentSubmission(old);
    let retained = await readJournalSnapshot(old);
    const readAll = async (): Promise<Record<string, unknown[]>> => {
      const transaction = old.database.transaction([...JOURNAL_OBJECT_STORES], "readonly");
      return Object.fromEntries(await Promise.all(JOURNAL_OBJECT_STORES.map(async (name) =>
        [name, await requestResult(transaction.objectStore(name).getAll())])));
    };
    const before = await readAll();
    resuming = true;
    requests.length = 0;
    if (interleaved) sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, otherSession);
    const options = { baseUrl: location.origin, project: scenario.project,
      chapter, profile: scenario.profile, fetchImpl };
    const winner = await openEditorWorkspace(options);
    if (winner.kind === "editor-ready") trackDatabase(winner.database, databases);
    expect(requests.every((request) => request.startsWith("GET "))).toBe(true);
    if (!fault.startsWith("valid")) {
      expect(winner.kind).toBe("editor-read-only-recovery");
      expect(await readAll()).toEqual(before);
      return;
    }
    requireEditorReady(winner);
    expect(winner.session).toEqual(session);
    expect(winner.partition.writer_generation).toBe("3");
    expect(winner.partition.journal_partition_id).not.toBe(old.partition.journal_partition_id);
    expect(await readProjectActivityIngest(winner)).toEqual({ replay_generation: "2",
      processed_through_stream_sequence: "5", events: [], held: [] });
    const installed = await readAll();
    const fenced = interleaved ? old.partition : { ...old.partition, disposition: "read_only_observer" };
    expect(installed.partitions).toEqual([fenced, winner.partition]);
    expect(installed.metadata).toEqual(expect.arrayContaining(before.metadata!));
    for (const name of JOURNAL_OBJECT_STORES.filter((name) => name !== "metadata" && name !== "partitions")) {
      expect(installed[name]).toEqual(before[name]);
    }
    expect(await readJournalSnapshot(old)).toEqual(retained);
    if (!interleaved) await expect(persistReplaceSelection(old, edit)).rejects.toThrow();
    expect(await readAll()).toEqual(installed);
    const reload = await openEditorWorkspace(options);
    requireEditorReady(reload);
    trackDatabase(reload.database, databases);
    expect(reload.partition).toEqual(winner.partition);
    expect(await readAll()).toEqual(installed);
    editor.value = "Base";
    document.body.append(editor);
    controller = attachManualInput({ editor, workspace: reload, baseUrl: location.origin, fetchImpl,
      setTimeoutImpl: () => 1, clearTimeoutImpl: () => {},
      nowImpl: () => Date.parse("2026-08-20T05:00:00.000Z"),
      onFailure: (error) => { failures.push(error); } });
    editor.focus();
    editor.setSelectionRange(4, 4);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await controller.whenIdle();
    if (interleaved) {
      // The old tab has not observed a writer refusal. Its local input has no
      // authority, but it still consumes the shared allocator before that fence.
      await persistReplaceSelection(old, { ...edit, from: edit.resultingBody.length,
        to: edit.resultingBody.length, text: "?", resultingBody: `${edit.resultingBody}?`,
        createdAt: "2026-08-20T05:00:00.001Z" });
      retained = await readJournalSnapshot(old);
      await applyTrustedInput({ operation: "insert_text", text: "?" });
      await controller.whenIdle();
    }
    await controller.flush();
    expect(failures).toEqual([]);
    expect(reload.pending).toEqual({ body: expectedBody, save_state: "saved", unsettled_intent_count: 0,
      authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id });
    expect(commands).toBe(1);
    const journal = await readJournalSnapshot(reload);
    const sequences = interleaved ? [2, 4] : [2];
    expect(journal.records.map((record) => ({ sequence: record.local_intent_sequence,
      dependency: record.projection_dependency }))).toEqual(sequences.map((sequence, index) => ({ sequence,
      dependency: { snapshot_id: winner.session.base_snapshot.snapshot_id,
        prior_sequence: sequences[index - 1] ?? 0 } })));
    expect(journal.groups.map((group) => group.covered_sequence_range))
      .toEqual([{ first: 2, last: sequences.at(-1) }]);
    const savedOptions = { ...options, chapter: { ...chapter, project_activity_position: "6",
      chapter: { ...chapter.chapter, current_revision: session.base_snapshot.materialized_revision } } };
    const saved = await openEditorWorkspace(savedOptions);
    requireEditorReady(saved);
    trackDatabase(saved.database, databases);
    expect(saved.partition).toEqual(winner.partition);
    expect(saved.pending).toEqual(reload.pending);
    expect(await readJournalSnapshot(old)).toEqual(retained);
    expect((await readAll()).partitions).toEqual(installed.partitions);
    const beforeDowngrade = await readAll();
    session.writer = { kind: "current_writer", writer_generation: "2" };
    const downgraded = await openEditorWorkspace(savedOptions);
    if (downgraded.kind === "editor-ready") trackDatabase(downgraded.database, databases);
    expect(downgraded.kind).toBe("editor-read-only-recovery");
    expect(await readAll()).toEqual(beforeDowngrade);
  } finally {
    controller?.close();
    editor.remove();
    closeTrackedDatabases(databases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
