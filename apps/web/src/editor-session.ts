import {
  createEditorSession,
  createProjectCommandChallenge,
  digestCreateEditorSession,
  getEditorSession,
  getManuscriptTree,
  getSnapshot,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateEditorSessionRequest,
  GetChapterResponse,
  GetEditorSessionResponse,
  GetProjectResponse,
  ProjectScope,
  Release1ProtocolProfile,
  SnapshotDescriptor,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  EditorState,
  EditorWorkspace,
  JournalPartition,
} from "./editor-types.ts";
import {
  JOURNAL_DATABASE_VERSION,
  JOURNAL_OBJECT_STORES,
  persistReplaceSelection,
  rebuildPendingProjection,
} from "./local-edit-journal.ts";
import {
  stageProjectActivitySnapshot,
  validatedCanonicalSnapshot,
} from "./project-activity-ingest.ts";

export {
  freezeOneIntentSubmission,
  submitOnePendingAuthorEdit,
} from "./author-edit-submission.ts";
export { persistReplaceSelection, rebuildPendingProjection } from "./local-edit-journal.ts";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const boundedU64 = (value: unknown): value is string => typeof value === "string" && U64.test(value)
  && BigInt(value) <= 18446744073709551615n;
const positiveU64 = (value: unknown): value is string => boundedU64(value) && BigInt(value) > 0n
  && BigInt(value) <= 18446744073709551615n;

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

function uuidV7(cryptoImpl: Crypto = globalThis.crypto, now = Date.now()): string {
  const bytes = cryptoImpl.getRandomValues(new Uint8Array(16));
  for (let offset = 5; offset >= 0; offset -= 1) { bytes[offset] = now & 0xff; now = Math.floor(now / 256); }
  bytes[6] = (bytes[6]! & 0x0f) | 0x70;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

async function openJournalDatabase(
  indexedDBImpl: IDBFactory | undefined,
  scope: ProjectScope,
): Promise<IDBDatabase> {
  if (!indexedDBImpl) throw new Error("IndexedDB is unavailable");
  const name = `storyos-local-edit-journal:${scope.owner_user_id}:${scope.project_id}`;
  const request = indexedDBImpl.open(name, JOURNAL_DATABASE_VERSION);
  request.onupgradeneeded = (event) => {
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
    request.transaction!.objectStore("metadata")
      .put({ key: "schema", version: JOURNAL_DATABASE_VERSION });
  };
  let database: IDBDatabase;
  try {
    database = await requestResult(request) as IDBDatabase;
  } catch (error) {
    request.result?.close();
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
  return database;
}

function partitionFrom(
  session: GetEditorSessionResponse,
  scope: ProjectScope,
  profile: Release1ProtocolProfile,
): JournalPartition {
  const binding = session.editor_session;
  const writerGeneration = session.writer.kind === "current_writer"
    ? session.writer.writer_generation
    : session.writer.observed_writer_generation;
  const journalPartitionId = [scope.owner_user_id, scope.project_id, binding.editor_session_id,
    writerGeneration, binding.client_session_binding_ref, binding.client_session_generation,
    binding.client_contract_revision, binding.security_policy_revision,
    profile.limit_profile_revision].join(":");
  return {
    journal_partition_id: journalPartitionId,
    project_scope: scope,
    editor_session_id: binding.editor_session_id,
    writer_generation: writerGeneration,
    client_session_binding_ref: binding.client_session_binding_ref,
    client_session_generation: binding.client_session_generation,
    client_contract_revision: binding.client_contract_revision,
    security_policy_revision: binding.security_policy_revision,
    limit_profile_revision: profile.limit_profile_revision,
    created_at: binding.opened_at,
    disposition: session.writer.kind === "current_writer" ? "current_writer_open" : "read_only_observer",
  };
}

async function createSession({ baseUrl, project, profile, fetchImpl, cryptoImpl }: {
  baseUrl: string;
  project: GetProjectResponse;
  profile: Release1ProtocolProfile;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}): Promise<GetEditorSessionResponse> {
  const request: CreateEditorSessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: profile.release_identity.web_client_contract_revision,
    security_policy_revision: SECURITY_POLICY_REVISION,
    correlation_id: uuidV7(cryptoImpl),
  };
  const idempotencyKey = uuidV7(cryptoImpl);
  const canonicalCommandDigest = await digestCreateEditorSession(request, cryptoImpl);
  const challenge = await createProjectCommandChallenge({
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
  });
  return createEditorSession({
    baseUrl, projectId: project.project.project_id, request, idempotencyKey,
    antiForgery: challenge.nonce, fetchImpl,
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
    writer?: {
      kind?: unknown;
      writer_generation?: unknown;
      observed_writer_generation?: unknown;
      reason?: unknown;
    };
    base_snapshot?: {
      chapter_id?: unknown;
      project_activity_position?: unknown;
      authoritative_head_revision_id?: unknown;
      proposal_head_revision_ids?: unknown;
      target_refs?: unknown;
      observed_ownership_partition?: unknown;
      materialized_revision?: { revision_id?: unknown; body?: unknown };
      materialized_payload_digest?: {
        algorithm?: unknown; profile?: unknown; value_hex_lowercase?: unknown;
      };
    };
  };
  const generation = candidate?.writer?.writer_generation
    ?? candidate?.writer?.observed_writer_generation;
  const binding = candidate?.editor_session;
  const base = candidate?.base_snapshot;
  if (candidate?.project_scope?.owner_user_id !== scope.owner_user_id
    || candidate?.project_scope?.project_id !== scope.project_id
    || !binding?.editor_session_id || !binding.client_session_binding_ref
    || !boundedU64(binding.client_session_generation)
    || binding.client_contract_revision !== profile.release_identity.web_client_contract_revision
    || binding.security_policy_revision !== SECURITY_POLICY_REVISION
    || binding.disposition !== "open"
    || (candidate?.writer?.kind === "current_writer"
      ? (!positiveU64(candidate.writer.writer_generation)
        || "observed_writer_generation" in candidate.writer || "reason" in candidate.writer)
      : candidate?.writer?.kind === "read_only"
        ? (!positiveU64(candidate.writer.observed_writer_generation)
          || candidate.writer.reason !== "secondary_session" || "writer_generation" in candidate.writer)
        : true)
    || base?.chapter_id !== chapter.chapter.chapter_id
    || base?.materialized_revision?.revision_id !== chapter.chapter.current_revision.revision_id
    || base?.authoritative_head_revision_id !== base?.materialized_revision?.revision_id
    || base?.materialized_revision?.body !== chapter.chapter.current_revision.body
    || base?.project_activity_position !== chapter.project_activity_position
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
      > profile.max_json_string_utf8_bytes
    || !positiveU64(generation)) throw new Error("Editor Session binding mismatch");
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

export async function openEditorWorkspace({
  baseUrl, project, chapter, profile, fetchImpl = globalThis.fetch,
  indexedDBImpl = globalThis.indexedDB, cryptoImpl = globalThis.crypto,
}: {
  baseUrl: string;
  project: GetProjectResponse;
  chapter: GetChapterResponse;
  profile: Release1ProtocolProfile;
  fetchImpl?: typeof fetch;
  indexedDBImpl?: IDBFactory;
  cryptoImpl?: Crypto;
}): Promise<EditorState> {
  const scope = project.project_scope;
  let database: IDBDatabase | undefined;
  try {
    database = await openJournalDatabase(indexedDBImpl, scope);
    const activeSessionKey = `active_session:${scope.owner_user_id}:${scope.project_id}`;
    const active = globalThis.sessionStorage
      ? globalThis.sessionStorage.getItem(activeSessionKey) : undefined;
    const sessionValue: unknown = active
      ? await getEditorSession({ baseUrl, projectId: scope.project_id,
        editorSessionId: active, fetchImpl })
      : await createSession({ baseUrl, project, profile, fetchImpl, cryptoImpl });
    const session = await validateSession(sessionValue, scope, chapter, profile, cryptoImpl);
    const partition = partitionFrom(session, scope, profile);
    const workspace: EditorWorkspace = { database, partition, session, cryptoImpl,
      maxJsonStringUtf8Bytes: profile.max_json_string_utf8_bytes };
    let canonicalSnapshot: SnapshotDescriptor | undefined;
    if (session.writer.kind === "current_writer" && BigInt(session.writer.writer_generation) > 1n) {
      const base = session.base_snapshot;
      if (active !== session.editor_session.editor_session_id
        || session.schema_id !== "storyos.query.editor-session.response.v1"
        || !UUID.test(session.correlation_id)
        || !UUID.test(session.editor_session.editor_session_id)
        || !UUID.test(base.snapshot_id) || !UUID.test(base.chapter_id)
        || !UUID.test(base.authoritative_head_revision_id)
        || !boundedU64(base.project_activity_position)
        || !Number.isFinite(Date.parse(base.created_at))
        || !Number.isFinite(Date.parse(session.editor_session.opened_at))
        || project.project.project_id !== scope.project_id
        || JSON.stringify(chapter.project_scope) !== JSON.stringify(scope)) {
        throw new Error("Takeover Session binding mismatch");
      }
      // The base and the canonical query boundary have separate roles. Read the
      // locator independently, then recheck the Session before local writes.
      const tree = await getManuscriptTree({ baseUrl, projectId: scope.project_id, fetchImpl });
      const snapshotId = tree?.snapshot?.snapshot_id;
      if (tree?.schema_id !== "storyos.query.manuscript-tree.response.v1"
        || !UUID.test(tree.correlation_id) || !boundedU64(tree.tree_revision)
        || JSON.stringify(tree.project_scope) !== JSON.stringify(scope)
        || !UUID.test(snapshotId ?? "")) throw new Error("Canonical Snapshot locator is invalid");
      canonicalSnapshot = validatedCanonicalSnapshot(await getSnapshot({
        baseUrl, projectId: scope.project_id, snapshotId, fetchImpl,
      }), workspace, snapshotId);
      if (JSON.stringify(canonicalSnapshot) !== JSON.stringify(tree.snapshot)
        || canonicalSnapshot.project_activity_position !== base.project_activity_position) {
        throw new Error("Takeover Snapshot binding mismatch");
      }
      const current = await validateSession(await getEditorSession({
        baseUrl, projectId: scope.project_id, editorSessionId: active, fetchImpl,
      }), scope, chapter, profile, cryptoImpl);
      if (!UUID.test(current.correlation_id)
        || JSON.stringify({ ...current, correlation_id: session.correlation_id }) !== JSON.stringify(session)) {
        throw new Error("Takeover Session changed during Snapshot validation");
      }
    }
    const transaction = database.transaction(canonicalSnapshot
      ? ["metadata", "partitions", "payload_chains", "intents", "submission_groups"]
      : ["metadata", "partitions"], "readwrite");
    const partitions = transaction.objectStore("partitions");
    const metadata = transaction.objectStore("metadata");
    const activeBaseKey = `active_base:${partition.journal_partition_id}`;
    const [existingPartition, activeBase, storedPartitions] = await Promise.all([
      requestResult(partitions.get(partition.journal_partition_id)),
      requestResult(metadata.get(activeBaseKey)),
      canonicalSnapshot ? requestResult(partitions.getAll()) : Promise.resolve([]),
    ]);
    if (existingPartition && JSON.stringify(existingPartition) !== JSON.stringify(partition)) {
      transaction.abort();
      throw new Error("Local Edit Journal partition is corrupt");
    }
    if (activeBase && JSON.stringify((activeBase as { value?: unknown }).value)
      !== JSON.stringify(session.base_snapshot)) {
      transaction.abort();
      throw new Error("Local Edit Journal active base is corrupt");
    }
    const fences: JournalPartition[] = [];
    for (const older of storedPartitions as JournalPartition[]) {
      const sameSession = older.editor_session_id === partition.editor_session_id;
      const binding = sameSession ? session.editor_session : {
        ...older, opened_at: older.created_at, disposition: "open" as const,
      };
      const expected = partitionFrom({ ...session,
        editor_session: binding,
        writer: { kind: "current_writer", writer_generation: older.writer_generation } },
      scope, sameSession ? profile : { ...profile, limit_profile_revision: older.limit_profile_revision });
      if (!UUID.test(older.editor_session_id) || !positiveU64(older.writer_generation)
        || !boundedU64(older.client_session_generation)
        || !Number.isFinite(Date.parse(older.created_at))
        || [older.client_session_binding_ref, older.client_contract_revision,
          older.security_policy_revision, older.limit_profile_revision]
          .some((value) => typeof value !== "string" || value.length === 0)
        || BigInt(older.writer_generation) > BigInt(partition.writer_generation)
        || (!sameSession && older.writer_generation === partition.writer_generation
          && older.disposition === "current_writer_open")
        || (older.disposition !== "current_writer_open" && older.disposition !== "read_only_observer")
        || JSON.stringify(older) !== JSON.stringify({ ...expected, disposition: older.disposition })) {
        transaction.abort();
        throw new Error("Retained Journal partition binding mismatch");
      }
      if (older.writer_generation === partition.writer_generation) continue;
      const oldBase = await requestResult(metadata.get(`active_base:${older.journal_partition_id}`)) as {
        value?: GetEditorSessionResponse["base_snapshot"];
      } | undefined;
      if (!oldBase?.value || !UUID.test(oldBase.value.snapshot_id)
        || !boundedU64(oldBase.value.project_activity_position)
        || oldBase.value.snapshot_id === session.base_snapshot.snapshot_id
        || BigInt(oldBase.value.project_activity_position) >= BigInt(session.base_snapshot.project_activity_position)) {
        transaction.abort();
        throw new Error("Takeover reused a retained Journal base");
      }
      if (older.disposition === "current_writer_open") {
        fences.push({ ...older, disposition: "read_only_observer" });
      }
    }
    // Snapshot ingest, the new partition, and old-generation fences commit as
    // one unit across this Project. Retained bases, payloads, groups, and outcomes
    // are not rewritten. Each old Session keeps its own immutable binding.
    if (canonicalSnapshot) await stageProjectActivitySnapshot(workspace, transaction, canonicalSnapshot);
    for (const fenced of fences) partitions.put(fenced);
    if (!existingPartition) partitions.add(partition);
    if (!activeBase) {
      metadata.add({ key: activeBaseKey, value: session.base_snapshot });
    }
    await transactionResult(transaction);
    globalThis.sessionStorage?.setItem(activeSessionKey, session.editor_session.editor_session_id);
    const pending = await rebuildPendingProjection(workspace);
    if (partition.disposition !== "current_writer_open") {
      database.close();
      return { kind: "editor-read-only-recovery", code: "editor_session_read_only",
        editor_session: session.editor_session, writer: session.writer, pending };
    }
    return { kind: "editor-ready", ...workspace, pending };
  } catch (error) {
    database?.close();
    return { kind: "editor-read-only-recovery", code: "local_journal_unavailable", error };
  }
}
