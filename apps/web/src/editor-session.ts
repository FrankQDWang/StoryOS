import { createJournalWorkingIndexes } from "./journal-working-set.ts";
import {
  createEditorSession,
  createProjectCommandChallenge,
  digestCreateEditorSession,
  getEditorSession,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateEditorSessionRequest,
  CreateProjectCommandChallengeResponse,
  GetChapterResponse,
  GetEditorSessionResponse,
  GetProjectResponse,
  ProjectScope,
  Release1ProtocolProfile,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  EditorState,
  EditorWorkspace,
  JournalPartition,
} from "./editor-types.ts";
import {
  JOURNAL_DATABASE_VERSION,
  JOURNAL_OBJECT_STORES,
  journalHasOpenWork,
  rebuildPendingProjection,
} from "./local-edit-journal.ts";
import { SECURITY_POLICY_REVISION, uuidV7 } from "./client-identity.ts";
import { requestResult, transactionResult } from "./indexed-db-request.ts";
import { recoveryPrefix } from "./local-recovery-record.ts";
import { freezeUnsentVersion5Groups, retainVersion5PartitionText, upgradeVersion5Journal }
  from "./journal-version-5-upgrade.ts";

export {
  freezeOneIntentSubmission,
  submitOnePendingAuthorEdit,
} from "./author-edit-submission.ts";
export { appendAuthorEdit, rebuildPendingProjection } from "./local-edit-journal.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const boundedU64 = (value: unknown): value is string => typeof value === "string" && U64.test(value)
  && BigInt(value) <= 18446744073709551615n;

async function openJournalDatabase(
  indexedDBImpl: IDBFactory | undefined,
  scope: ProjectScope,
  cryptoImpl: Crypto,
  maxJsonStringUtf8Bytes: number,
): Promise<IDBDatabase> {
  if (!indexedDBImpl) throw new Error("IndexedDB is unavailable");
  const name = `storyos-local-edit-journal:${scope.owner_user_id}:${scope.project_id}`;
  const request = indexedDBImpl.open(name, JOURNAL_DATABASE_VERSION);
  request.onupgradeneeded = (event) => {
    // A version 5 Journal has the writer fields of before ADR 0107. The upgrade removes them from each row.
    if (event.oldVersion === 5) {
      upgradeVersion5Journal(request.transaction!);
      request.transaction!.objectStore("metadata").put({ key: "schema", version: JOURNAL_DATABASE_VERSION });
      return;
    }
    // A Journal of an earlier version has no upgrade path (ADR 0099). The abort keeps it, and the open fails.
    if (event.oldVersion !== 0) {
      request.transaction!.abort();
      return;
    }
    const database = request.result;
    database.createObjectStore("metadata", { keyPath: "key" });
    database.createObjectStore("partitions", { keyPath: "journal_partition_id" });
    database.createObjectStore("payload_chains", { keyPath: "payload_chain_id" })
      .createIndex("partition", "journal_partition_id", { unique: false });
    database.createObjectStore("intents", {
      keyPath: ["journal_partition_id", "local_intent_sequence"],
    }).createIndex("partition", "journal_partition_id", { unique: false });
    database.createObjectStore("submission_groups", {
      keyPath: "journal_submission_group_id",
    }).createIndex("partition", "journal_partition_id", { unique: false });
    database.createObjectStore("transport_capsules", {
      keyPath: "exact_transport_retry_capsule_id",
    }).createIndex("group", "journal_submission_group_id", { unique: false });
    database.createObjectStore("transport_attempts", {
      keyPath: "transport_attempt_id",
    }).createIndex("group", "journal_submission_group_id", { unique: false });
    database.createObjectStore("protocol_observations", {
      keyPath: "protocol_observation_id",
    }).createIndex("group", "journal_submission_group_id", { unique: false });
    database.createObjectStore("outcome_query_attempts", {
      keyPath: "outcome_query_attempt_id",
    }).createIndex("group", "journal_submission_group_id", { unique: false });
    database.createObjectStore("outcome_query_observations", {
      keyPath: "outcome_query_observation_id",
    }).createIndex("group", "journal_submission_group_id", { unique: false });
    createJournalWorkingIndexes(request.transaction!);
    request.transaction!.objectStore("metadata")
      .put({ key: "schema", version: JOURNAL_DATABASE_VERSION });
  };
  let database: IDBDatabase;
  try {
    database = await new Promise<IDBDatabase>((resolve, reject) => {
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
      // A page of an earlier version keeps its connection, so the upgrade waits for that page. The load enters local
      // recovery now, and the connection that opens later closes at once.
      request.onblocked = () => {
        request.onsuccess = () => request.result.close();
        reject(new Error("Local Edit Journal upgrade waits for another page"));
      };
    });
  } catch (error) {
    if (request.readyState === "done") request.result?.close();
    throw error;
  }
  if (JOURNAL_OBJECT_STORES.some((name) => !database.objectStoreNames.contains(name))) {
    database.close();
    throw new Error("Local Edit Journal schema is incompatible");
  }
  const schema = await requestResult(database.transaction("metadata", "readonly")
    .objectStore("metadata").get("schema")) as { version?: unknown } | undefined;
  if (schema?.version !== JOURNAL_DATABASE_VERSION) {
    database.close();
    throw new Error("Local Edit Journal schema is incompatible");
  }
  try {
    await freezeUnsentVersion5Groups(database, cryptoImpl);
    await retainVersion5PartitionText(database, cryptoImpl, maxJsonStringUtf8Bytes);
  } catch (error) {
    database.close();
    throw error;
  }
  return database;
}

function partitionFrom(
  session: GetEditorSessionResponse,
  scope: ProjectScope,
  profile: Release1ProtocolProfile,
): JournalPartition {
  const binding = session.editor_session;
  const journalPartitionId = [scope.owner_user_id, scope.project_id, binding.editor_session_id,
    binding.client_session_binding_ref, binding.client_session_generation,
    binding.client_contract_revision, binding.security_policy_revision,
    profile.limit_profile_revision].join(":");
  return {
    journal_partition_id: journalPartitionId,
    project_scope: scope,
    editor_session_id: binding.editor_session_id,
    client_session_binding_ref: binding.client_session_binding_ref,
    client_session_generation: binding.client_session_generation,
    client_contract_revision: binding.client_contract_revision,
    security_policy_revision: binding.security_policy_revision,
    limit_profile_revision: profile.limit_profile_revision,
    created_at: binding.opened_at,
  };
}

type SessionCreationOptions = {
  baseUrl: string;
  project: GetProjectResponse;
  profile: Release1ProtocolProfile;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
};

/** An Editor Session creation and its Challenge request, which starts at once (#1899). */
type SessionCreation = {
  request: CreateEditorSessionRequest;
  idempotencyKey: string;
  challenge: Promise<CreateProjectCommandChallengeResponse>;
};

function prepareSessionCreation({ baseUrl, project, profile, fetchImpl, cryptoImpl }: SessionCreationOptions):
  SessionCreation {
  const request: CreateEditorSessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: profile.release_identity.web_client_contract_revision,
    security_policy_revision: SECURITY_POLICY_REVISION,
    correlation_id: uuidV7(cryptoImpl),
  };
  const idempotencyKey = uuidV7(cryptoImpl);
  const challenge = digestCreateEditorSession(request, cryptoImpl).then((canonicalCommandDigest) =>
    createProjectCommandChallenge({
      baseUrl,
      projectId: project.project.project_id,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/editor-sessions",
        command_schema: request.command_schema,
        canonical_command_digest: canonicalCommandDigest,
        idempotency_key: idempotencyKey,
      },
      fetchImpl,
    }));
  // An open that stops before the creation does not use the Challenge.
  challenge.catch(() => {});
  return { request, idempotencyKey, challenge };
}

async function createSession(creation: SessionCreation, options: SessionCreationOptions):
  Promise<GetEditorSessionResponse> {
  const challenge = await creation.challenge;
  return createEditorSession({
    baseUrl: options.baseUrl, projectId: options.project.project.project_id, request: creation.request,
    idempotencyKey: creation.idempotencyKey, antiForgery: challenge.nonce, fetchImpl: options.fetchImpl,
  });
}

async function validateSession(
  value: unknown,
  scope: ProjectScope,
  chapter: GetChapterResponse,
  profile: Release1ProtocolProfile,
  cryptoImpl: Crypto,
): Promise<GetEditorSessionResponse> {
  const candidate = value as {
    project_scope?: { owner_user_id?: unknown; project_id?: unknown };
    editor_session?: {
      editor_session_id?: unknown;
      client_session_binding_ref?: unknown;
      client_session_generation?: unknown;
      client_contract_revision?: unknown;
      security_policy_revision?: unknown;
      disposition?: unknown;
    };
    base_snapshot?: {
      chapter_id?: unknown;
      project_activity_position?: unknown;
      authoritative_head_revision_id?: unknown;
      proposal_head_revision_ids?: unknown;
      target_refs?: unknown;
      observed_ownership_partition?: unknown;
      materialized_revision?: {
        revision_id?: unknown;
        body?: unknown;
        blocks?: unknown;
      };
      materialized_payload_digest?: {
        algorithm?: unknown; profile?: unknown; value_hex_lowercase?: unknown;
      };
    };
  };
  const binding = candidate?.editor_session;
  const base = candidate?.base_snapshot;
  const basePosition = base?.project_activity_position;
  const currentPosition = chapter.project_activity_position;
  if (candidate?.project_scope?.owner_user_id !== scope.owner_user_id
    || candidate?.project_scope?.project_id !== scope.project_id
    || !binding?.editor_session_id || !binding.client_session_binding_ref
    || !boundedU64(binding.client_session_generation)
    || binding.client_contract_revision !== profile.release_identity.web_client_contract_revision
    || binding.security_policy_revision !== SECURITY_POLICY_REVISION
    || binding.disposition !== "open"
    || base?.chapter_id !== chapter.chapter.chapter_id
    || base?.materialized_revision?.revision_id !== chapter.chapter.current_revision.revision_id
    || base?.authoritative_head_revision_id !== base?.materialized_revision?.revision_id
    || base?.materialized_revision?.body !== chapter.chapter.current_revision.body
    || JSON.stringify(base?.materialized_revision?.blocks)
      !== JSON.stringify(chapter.chapter.current_revision.blocks)
    || !boundedU64(basePosition)
    || !boundedU64(currentPosition)
    || BigInt(basePosition) > BigInt(currentPosition)
    || base?.observed_ownership_partition !== "authoritative"
    || JSON.stringify(base?.target_refs)
      !== JSON.stringify([`manuscript:${base?.chapter_id as string}`])
    || !Array.isArray(base?.proposal_head_revision_ids)
    || base.proposal_head_revision_ids.length !== 0
    || base?.materialized_payload_digest?.algorithm !== "sha256"
    || base?.materialized_payload_digest?.profile !== "storyos.canonical-payload.sha256.v1"
    || !/^[0-9a-f]{64}$/.test(
      (base?.materialized_payload_digest?.value_hex_lowercase ?? "") as string,
    )
    || !Number.isSafeInteger(profile.max_json_string_utf8_bytes)
    || profile.max_json_string_utf8_bytes <= 0
    || new TextEncoder().encode((base?.materialized_revision?.body ?? "") as string).byteLength
      > profile.max_json_string_utf8_bytes) throw new Error("Editor Session binding mismatch");
  const session = value as GetEditorSessionResponse;
  const validatedBase = session.base_snapshot;
  const bytes = new TextEncoder().encode(validatedBase.materialized_revision.body);
  const digest = new Uint8Array(await cryptoImpl.subtle.digest("SHA-256", bytes));
  const digestHex = [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  if (digestHex !== validatedBase.materialized_payload_digest.value_hex_lowercase) {
    throw new Error("Editor Session base Snapshot digest mismatch");
  }
  return session;
}

type EditorWorkspaceOptions = {
  baseUrl: string;
  project: GetProjectResponse;
  profile: Release1ProtocolProfile;
  fetchImpl?: typeof fetch;
  indexedDBImpl?: IDBFactory;
  cryptoImpl?: Crypto;
  /** False keeps the Challenge of a Session creation until `finish`. */
  earlyChallenge?: boolean;
};

/**
 * The Editor Session of an open. The read of a stored Session needs only the Scope of the Project. `finish` creates a
 * Session when the tab has none, and validates the Session against the Chapter and `project`, by default the Project of
 * the start.
 */
export interface EditorWorkspaceOpening {
  finish(chapter: GetChapterResponse, project?: GetProjectResponse): Promise<OpenedEditorWorkspace>;
  /** Closes the Journal when the open stops before `finish`. The promise ends after the Session read ended. */
  abandon(): Promise<void>;
}

export interface OpenedEditorWorkspace {
  editor: EditorState;
}

type EditorSessionStart =
  | { kind: "started"; database: IDBDatabase; active: string | null | undefined; sessionValue: unknown;
    creation: SessionCreation | undefined }
  | { kind: "failed"; database: IDBDatabase | undefined; error: unknown };

/** Opens the Journal and reads the stored Editor Session at once, so that the read is in flight with the Chapter read. */
export function startEditorWorkspace({
  baseUrl, project, profile, fetchImpl = globalThis.fetch,
  indexedDBImpl = globalThis.indexedDB, cryptoImpl = globalThis.crypto, earlyChallenge = true,
}: EditorWorkspaceOptions): EditorWorkspaceOpening {
  const scope = project.project_scope;
  const started = (async (): Promise<EditorSessionStart> => {
    let database: IDBDatabase | undefined;
    try {
      const activeSessionKey = `active_session:${scope.owner_user_id}:${scope.project_id}`;
      const active = globalThis.sessionStorage
        ? globalThis.sessionStorage.getItem(activeSessionKey) : undefined;
      // The creation waits for the Chapter read (#1884), and its Challenge starts with that read (#1899).
      const creation = !active && earlyChallenge
        ? prepareSessionCreation({ baseUrl, project, profile, fetchImpl, cryptoImpl }) : undefined;
      database = await openJournalDatabase(indexedDBImpl, scope, cryptoImpl, profile.max_json_string_utf8_bytes);
      // A stored Session read changes nothing on the Server, so it runs with the Chapter read.
      const sessionValue: unknown = active
        ? await getEditorSession({ baseUrl, projectId: scope.project_id,
          editorSessionId: active, fetchImpl })
        : undefined;
      return { kind: "started", database, active, sessionValue, creation };
    } catch (error) {
      return { kind: "failed", database, error };
    }
  })();
  return {
    finish: (chapter, read = project) => finishEditorWorkspace(started, { baseUrl, project: read, chapter, profile,
      fetchImpl, cryptoImpl }),
    abandon: () => started.then((start) => { start.database?.close(); }),
  };
}

export async function openEditorWorkspace(options: EditorWorkspaceOptions & {
  chapter: GetChapterResponse;
}): Promise<EditorState> {
  return (await startEditorWorkspace(options).finish(options.chapter)).editor;
}

/** Tells if the stored Editor Session has a base that is not the text of the Chapter read. */
function baseIsNotChapter(value: unknown, chapter: GetChapterResponse): boolean {
  const base = (value as { base_snapshot?: { chapter_id?: unknown; materialized_revision?: { revision_id?: unknown } } }
    | undefined)?.base_snapshot;
  return base?.chapter_id !== chapter.chapter.chapter_id
    || base?.materialized_revision?.revision_id !== chapter.chapter.current_revision.revision_id;
}

/**
 * Tells if the Journal partition of the stored Editor Session has work that the Server does not have: Author Edit work
 * that the rule of the working set keeps open (`journalHasOpenWork`), a decision flight, an unresolved Withdrawal, or
 * a retained recovery row. Work that this check counts as open keeps the stored Editor Session.
 */
async function partitionHasOpenWork(database: IDBDatabase, value: unknown, scope: ProjectScope,
  profile: Release1ProtocolProfile, cryptoImpl: Crypto): Promise<boolean> {
  const served = value as GetEditorSessionResponse;
  const partition = partitionFrom(served, scope, profile);
  const partitionId = partition.journal_partition_id;
  const stored = (await requestResult(database.transaction("metadata", "readonly").objectStore("metadata")
    .get(`active_base:${partitionId}`)) as { value?: GetEditorSessionResponse["base_snapshot"] } | undefined)?.value;
  // A deletion moves the base of each Editor Session on the Server (ADR 0107). The Journal then has the earlier base,
  // which the check of its rows uses. Another earlier base is corrupt, and the check counts it as open work.
  const moved = stored !== undefined && boundedU64(stored.project_activity_position)
    && boundedU64(served.base_snapshot.project_activity_position)
    && BigInt(stored.project_activity_position) < BigInt(served.base_snapshot.project_activity_position);
  const session = moved ? { ...served, base_snapshot: stored } : served;
  if (await journalHasOpenWork({ database, partition, session, cryptoImpl,
    maxJsonStringUtf8Bytes: profile.max_json_string_utf8_bytes })) return true;
  const metadata = database.transaction("metadata", "readonly").objectStore("metadata");
  const rows = (prefix: string) => requestResult(metadata.getAll(IDBKeyRange.bound(
    `${prefix}${partitionId}:`, `${prefix}${partitionId}:\uffff`)));
  const [acceptances, rejections, withdrawals, retained] = await Promise.all([rows("acceptance:"),
    rows("rejection:"), rows("withdraw:"), rows(recoveryPrefix)]) as [unknown[], unknown[],
    { settlement?: unknown }[], unknown[]];
  return acceptances.length + rejections.length + retained.length > 0
    || withdrawals.some((row) => row.settlement === "unresolved");
}

async function finishEditorWorkspace(started: Promise<EditorSessionStart>, {
  baseUrl, project, chapter, profile, fetchImpl, cryptoImpl,
}: {
  baseUrl: string;
  project: GetProjectResponse;
  chapter: GetChapterResponse;
  profile: Release1ProtocolProfile;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}): Promise<OpenedEditorWorkspace> {
  const scope = project.project_scope;
  let start = await started;
  // ADR 0107: another Editor Session can write, so the stored Session can have an earlier base than the Chapter read.
  // When its Journal partition has no open work, the load opens a new Editor Session, as a new tab does.
  if (start.kind === "started" && start.active && baseIsNotChapter(start.sessionValue, chapter)) {
    let open = true;
    try {
      open = await partitionHasOpenWork(start.database, start.sessionValue, scope, profile, cryptoImpl);
    } catch {
      // An unreadable Journal keeps the stored Session, so its recovery runs.
    }
    if (!open) start = { ...start, active: null };
  }
  if (start.kind === "started" && !start.active) {
    try {
      const options = { baseUrl, project, profile, fetchImpl, cryptoImpl };
      start = { ...start, sessionValue: await createSession(start.creation ?? prepareSessionCreation(options),
        options) };
    } catch (error) {
      start = { kind: "failed", database: start.database, error };
    }
  }
  if (start.kind === "failed") {
    start.database?.close();
    // The Journal did not open, for example a Journal of an earlier version (ADR 0099), or the Session request failed.
    return { editor: { kind: "editor-read-only-recovery", code: "local_journal_unavailable", error: start.error } };
  }
  const { database, sessionValue } = start;
  try {
    const activeSessionKey = `active_session:${scope.owner_user_id}:${scope.project_id}`;
    const session = await validateSession(sessionValue, scope, chapter, profile, cryptoImpl);
    const partition = partitionFrom(session, scope, profile);
    const workspace: EditorWorkspace = { database, partition, session, cryptoImpl,
      maxJsonStringUtf8Bytes: profile.max_json_string_utf8_bytes };
    const transaction = database.transaction(["metadata", "partitions"], "readwrite");
    const partitions = transaction.objectStore("partitions");
    const metadata = transaction.objectStore("metadata");
    const activeBaseKey = `active_base:${partition.journal_partition_id}`;
    const [existingPartition, activeBase] = await Promise.all([
      requestResult(partitions.get(partition.journal_partition_id)),
      requestResult(metadata.get(activeBaseKey)),
    ]);
    if (existingPartition && JSON.stringify(existingPartition) !== JSON.stringify(partition)) {
      transaction.abort();
      throw new Error("Local Edit Journal partition is corrupt");
    }
    const storedBase = (activeBase as { value?: GetEditorSessionResponse["base_snapshot"] } | undefined)
      ?.value;
    const storedMatchesSession = activeBase !== undefined
      && JSON.stringify(storedBase) === JSON.stringify(session.base_snapshot);
    // The Session may install a later authorized base in this Journal partition. An admitted current-Chapter
    // transition changes the snapshot identity and advances project_activity_position.
    const successorAuthorizedBase = storedBase !== undefined
      && UUID.test(storedBase.snapshot_id)
      && UUID.test(storedBase.chapter_id)
      && boundedU64(storedBase.project_activity_position)
      && storedBase.snapshot_id !== session.base_snapshot.snapshot_id
      && BigInt(storedBase.project_activity_position)
        < BigInt(session.base_snapshot.project_activity_position);
    if (activeBase && !storedMatchesSession && !successorAuthorizedBase) {
      transaction.abort();
      throw new Error("Local Edit Journal active base is corrupt");
    }
    if (!existingPartition) partitions.add(partition);
    if (!activeBase) {
      metadata.add({ key: activeBaseKey, value: session.base_snapshot });
    } else if (!storedMatchesSession) {
      metadata.put({ key: activeBaseKey, value: session.base_snapshot });
    }
    await transactionResult(transaction);
    globalThis.sessionStorage?.setItem(activeSessionKey, session.editor_session.editor_session_id);
    const pending = await rebuildPendingProjection(workspace);
    return { editor: { kind: "editor-ready", ...workspace, openedProjection: pending } };
  } catch (error) {
    database.close();
    return { editor: { kind: "editor-read-only-recovery", code: "local_journal_unavailable", error } };
  }
}
