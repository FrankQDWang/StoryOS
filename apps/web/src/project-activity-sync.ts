import {
  activityStream,
  getManuscriptTree,
  StoryOSProtocolError,
  type SnapshotDescriptor,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import type { EditorWorkspace, ProjectActivityIngest } from "./editor-types.ts";
import {
  ingestProjectActivityFrames,
  resyncProjectActivityFromSnapshot,
} from "./project-activity-ingest.ts";

export type ProjectActivitySyncResult =
  | {
    kind: "replayed";
    ingest: ProjectActivityIngest;
    last_event_id: string | null;
  }
  | {
    kind: "idle";
    ingest: ProjectActivityIngest;
    last_event_id: string | null;
  }
  | {
    kind: "resynchronized";
    ingest: ProjectActivityIngest;
    last_event_id: string | null;
    snapshot: SnapshotDescriptor;
  }
  | {
    kind: "unavailable";
    code: "activity_cursor_too_old" | "resource_unavailable" | "snapshot_expired";
  };

interface ActivityStreamCursor {
  snapshot_id: string;
  last_event_id: string | null;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const requestResult = (request: IDBRequest): Promise<unknown> => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
});
const transactionResult = (transaction: IDBTransaction): Promise<void> => new Promise(
  (resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(
      transaction.error ?? new Error("IndexedDB transaction aborted"),
    );
    transaction.onerror = () => reject(
      transaction.error ?? new Error("IndexedDB transaction failed"),
    );
  },
);

function cursorKey(workspace: EditorWorkspace): string {
  const scope = workspace.partition.project_scope;
  return `project_activity_stream:${scope.owner_user_id}:${scope.project_id}`;
}

function problemCode(error: unknown): string | undefined {
  if (!(error instanceof StoryOSProtocolError)) return undefined;
  try {
    const code = Reflect.get(JSON.parse(error.responseBody ?? ""), "code");
    return typeof code === "string" ? code : undefined;
  } catch {
    return undefined;
  }
}

type UnavailableActivityCode = Extract<ProjectActivitySyncResult, { kind: "unavailable" }>["code"];

function unavailableCode(error: unknown): UnavailableActivityCode | undefined {
  const code = problemCode(error);
  if (code === "activity_cursor_too_old"
    || code === "resource_unavailable"
    || code === "snapshot_expired") {
    return code;
  }
  return undefined;
}

function needsAuthorizedSnapshot(error: unknown): boolean {
  return error instanceof StoryOSProtocolError
    && error.status === 409
    && (problemCode(error) === "activity_cursor_too_old"
      || problemCode(error) === "snapshot_expired");
}

function lastFrameId(frames: unknown[]): string | null {
  const last = frames.at(-1);
  if (last === null || typeof last !== "object") return null;
  const id = Reflect.get(last, "id");
  return typeof id === "string" && id.length > 0 ? id : null;
}

function parseSseFrames(body: string): unknown[] {
  return body.split("\n\n").filter((block) => block.trim().length > 0).map((block) => {
    let id = "";
    let event = "";
    let data: unknown;
    for (const line of block.split("\n")) {
      if (line.startsWith("id:")) id = line.slice(3).trim();
      if (line.startsWith("event:")) event = line.slice(6).trim();
      if (line.startsWith("data:")) data = JSON.parse(line.slice(5).trim());
    }
    return { id, event, data };
  });
}

async function readActivityStreamCursor(workspace: EditorWorkspace): Promise<ActivityStreamCursor> {
  const transaction = workspace.database.transaction(["metadata"], "readonly");
  const stored = await requestResult(
    transaction.objectStore("metadata").get(cursorKey(workspace)),
  ) as { value?: { snapshot_id?: unknown; last_event_id?: unknown } } | undefined;
  const snapshotId = stored?.value?.snapshot_id;
  const lastEventId = stored?.value?.last_event_id;
  if (typeof snapshotId === "string" && UUID.test(snapshotId)) {
    return {
      snapshot_id: snapshotId,
      last_event_id: typeof lastEventId === "string" && lastEventId.length > 0 ? lastEventId : null,
    };
  }
  return {
    snapshot_id: workspace.session.base_snapshot.snapshot_id,
    last_event_id: null,
  };
}

async function writeActivityStreamCursor(
  workspace: EditorWorkspace,
  cursor: ActivityStreamCursor,
): Promise<void> {
  const transaction = workspace.database.transaction(["metadata"], "readwrite");
  transaction.objectStore("metadata").put({ key: cursorKey(workspace), value: cursor });
  await transactionResult(transaction);
}

async function replayActivityBody(
  workspace: EditorWorkspace,
  cursor: ActivityStreamCursor,
  body: string,
): Promise<ProjectActivitySyncResult> {
  const frames = parseSseFrames(body);
  if (frames.length === 0) {
    const ingest = await ingestProjectActivityFrames(workspace, []);
    await writeActivityStreamCursor(workspace, cursor);
    return {
      kind: "idle",
      ingest,
      last_event_id: cursor.last_event_id,
    };
  }
  const ingest = await ingestProjectActivityFrames(workspace, frames);
  const lastEventId = lastFrameId(frames);
  await writeActivityStreamCursor(workspace, {
    snapshot_id: cursor.snapshot_id,
    last_event_id: lastEventId,
  });
  return { kind: "replayed", ingest, last_event_id: lastEventId };
}

async function resynchronizeOwnedProjectActivity(
  workspace: EditorWorkspace,
  options: { baseUrl: string; fetchImpl: typeof fetch; signal?: AbortSignal },
): Promise<ProjectActivitySyncResult> {
  const scope = workspace.partition.project_scope;
  try {
    const tree = await getManuscriptTree({
      baseUrl: options.baseUrl,
      projectId: scope.project_id,
      fetchImpl: options.fetchImpl,
      ...(options.signal === undefined ? {} : { signal: options.signal }),
    });
    if (JSON.stringify(tree.project_scope) !== JSON.stringify(scope)
      || JSON.stringify(tree.snapshot.project_scope) !== JSON.stringify(scope)) {
      throw new Error("Canonical Snapshot is invalid");
    }
    const { snapshot, ingest } = await resyncProjectActivityFromSnapshot(workspace, {
      baseUrl: options.baseUrl,
      snapshotId: tree.snapshot.snapshot_id,
      fetchImpl: options.fetchImpl,
      ...(options.signal === undefined ? {} : { signal: options.signal }),
    });
    const cursor = { snapshot_id: snapshot.snapshot_id, last_event_id: null };
    await writeActivityStreamCursor(workspace, cursor);
    // Resume strictly after the authorized Snapshot. Do not send the expired cursor.
    const body = await activityStream({
      baseUrl: options.baseUrl,
      projectId: scope.project_id,
      snapshotId: snapshot.snapshot_id,
      protocolRelease: RELEASE_1_PROTOCOL_PROFILE.public_protocol_release,
      fetchImpl: options.fetchImpl,
      ...(options.signal === undefined ? {} : { signal: options.signal }),
    });
    const frames = parseSseFrames(body);
    if (frames.length === 0) {
      return { kind: "resynchronized", ingest, snapshot, last_event_id: null };
    }
    const next = await ingestProjectActivityFrames(workspace, frames);
    const lastEventId = lastFrameId(frames);
    await writeActivityStreamCursor(workspace, {
      snapshot_id: snapshot.snapshot_id,
      last_event_id: lastEventId,
    });
    return { kind: "resynchronized", ingest: next, snapshot, last_event_id: lastEventId };
  } catch (error) {
    if (needsAuthorizedSnapshot(error) || unavailableCode(error) !== undefined) {
      return {
        kind: "unavailable",
        code: unavailableCode(error) ?? "activity_cursor_too_old",
      };
    }
    throw error;
  }
}

export async function consumeOwnedProjectActivity(workspace: EditorWorkspace, {
  baseUrl, fetchImpl = globalThis.fetch, signal,
}: {
  baseUrl: string;
  fetchImpl?: typeof fetch;
  signal?: AbortSignal;
}): Promise<ProjectActivitySyncResult> {
  const scope = workspace.partition.project_scope;
  const cursor = await readActivityStreamCursor(workspace);
  try {
    const body = await activityStream({
      baseUrl,
      projectId: scope.project_id,
      snapshotId: cursor.snapshot_id,
      protocolRelease: RELEASE_1_PROTOCOL_PROFILE.public_protocol_release,
      fetchImpl,
      ...(signal === undefined ? {} : { signal }),
      ...(cursor.last_event_id === null ? {} : { lastEventId: cursor.last_event_id }),
    });
    return await replayActivityBody(workspace, cursor, body);
  } catch (error) {
    if (needsAuthorizedSnapshot(error)) {
      return resynchronizeOwnedProjectActivity(workspace, {
        baseUrl,
        fetchImpl,
        ...(signal === undefined ? {} : { signal }),
      });
    }
    const code = unavailableCode(error);
    if (code !== undefined) return { kind: "unavailable", code };
    throw error;
  }
}
