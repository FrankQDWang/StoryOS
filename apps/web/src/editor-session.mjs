import {
  createEditorSession,
  createProjectCommandChallenge,
  digestCreateEditorSession,
  getEditorSession,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  JOURNAL_DATABASE_VERSION,
  JOURNAL_OBJECT_STORES,
  persistReplaceSelection,
  rebuildPendingProjection,
} from "./local-edit-journal.mjs";

export {
  freezeOneIntentSubmission,
  submitOnePendingAuthorEdit,
} from "./author-edit-submission.mjs";
export { persistReplaceSelection, rebuildPendingProjection } from "./local-edit-journal.mjs";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const boundedU64 = (value) => typeof value === "string" && U64.test(value)
  && BigInt(value) <= 18446744073709551615n;
const positiveU64 = (value) => boundedU64(value) && BigInt(value) > 0n
  && BigInt(value) <= 18446744073709551615n;

const requestResult = (request) => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
});
const transactionResult = (transaction) => new Promise((resolve, reject) => {
  transaction.oncomplete = resolve;
  transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
});

function uuidV7(cryptoImpl = globalThis.crypto, now = Date.now()) {
  const bytes = cryptoImpl.getRandomValues(new Uint8Array(16));
  for (let offset = 5; offset >= 0; offset -= 1) { bytes[offset] = now & 0xff; now = Math.floor(now / 256); }
  bytes[6] = (bytes[6] & 0x0f) | 0x70;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

async function openJournalDatabase(indexedDBImpl, scope) {
  if (!indexedDBImpl) throw new Error("IndexedDB is unavailable");
  const name = `storyos-local-edit-journal:${scope.owner_user_id}:${scope.project_id}`;
  const request = indexedDBImpl.open(name, JOURNAL_DATABASE_VERSION);
  request.onupgradeneeded = (event) => {
    if (event.oldVersion !== 0) {
      request.transaction.abort();
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
    request.transaction.objectStore("metadata")
      .put({ key: "schema", version: JOURNAL_DATABASE_VERSION });
  };
  let database;
  try {
    database = await requestResult(request);
  } catch (error) {
    request.result?.close();
    throw error;
  }
  if (JOURNAL_OBJECT_STORES.some((name) => !database.objectStoreNames.contains(name))) {
    database.close();
    throw new Error("Local Edit Journal schema is incompatible");
  }
  const schema = await requestResult(database.transaction("metadata", "readonly")
    .objectStore("metadata").get("schema"));
  if (schema?.version !== JOURNAL_DATABASE_VERSION) {
    database.close();
    throw new Error("Local Edit Journal schema is incompatible");
  }
  return database;
}

function partitionFrom(session, scope, profile) {
  const binding = session.editor_session;
  const writerGeneration = session.writer.writer_generation ?? session.writer.observed_writer_generation;
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

async function createSession({ baseUrl, project, profile, fetchImpl, cryptoImpl }) {
  const request = {
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

async function validateSession(session, scope, chapter, profile, cryptoImpl) {
  const generation = session?.writer?.writer_generation ?? session?.writer?.observed_writer_generation;
  const binding = session?.editor_session;
  const base = session?.base_snapshot;
  if (session?.project_scope?.owner_user_id !== scope.owner_user_id
    || session?.project_scope?.project_id !== scope.project_id
    || !binding?.editor_session_id || !binding.client_session_binding_ref
    || !boundedU64(binding.client_session_generation)
    || binding.client_contract_revision !== profile.release_identity.web_client_contract_revision
    || binding.security_policy_revision !== SECURITY_POLICY_REVISION
    || binding.disposition !== "open"
    || (session?.writer?.kind === "current_writer"
      ? (!positiveU64(session.writer.writer_generation)
        || "observed_writer_generation" in session.writer || "reason" in session.writer)
      : session?.writer?.kind === "read_only"
        ? (!positiveU64(session.writer.observed_writer_generation)
          || session.writer.reason !== "secondary_session" || "writer_generation" in session.writer)
        : true)
    || base?.chapter_id !== chapter.chapter.chapter_id
    || base?.materialized_revision?.revision_id !== chapter.chapter.current_revision.revision_id
    || base?.authoritative_head_revision_id !== base?.materialized_revision?.revision_id
    || base?.materialized_revision?.body !== chapter.chapter.current_revision.body
    || base?.project_activity_position !== chapter.project_activity_position
    || base?.observed_ownership_partition !== "authoritative"
    || JSON.stringify(base?.target_refs) !== JSON.stringify([`manuscript:${base?.chapter_id}`])
    || !Array.isArray(base?.proposal_head_revision_ids)
    || base.proposal_head_revision_ids.length !== 0
    || base?.materialized_payload_digest?.algorithm !== "sha256"
    || base?.materialized_payload_digest?.profile !== "storyos.canonical-payload.sha256.v1"
    || !/^[0-9a-f]{64}$/.test(base?.materialized_payload_digest?.value_hex_lowercase ?? "")
    || !Number.isSafeInteger(profile.max_json_string_utf8_bytes)
    || profile.max_json_string_utf8_bytes <= 0
    || new TextEncoder().encode(base?.materialized_revision?.body ?? "").byteLength
      > profile.max_json_string_utf8_bytes
    || !positiveU64(generation)) throw new Error("Editor Session binding mismatch");
  const bytes = new TextEncoder().encode(base.materialized_revision.body);
  const digest = new Uint8Array(await cryptoImpl.subtle.digest("SHA-256", bytes));
  const digestHex = [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  if (digestHex !== base.materialized_payload_digest.value_hex_lowercase) {
    throw new Error("Editor Session base Snapshot digest mismatch");
  }
}

export async function openEditorWorkspace({
  baseUrl, project, chapter, profile, fetchImpl = globalThis.fetch,
  indexedDBImpl = globalThis.indexedDB, cryptoImpl = globalThis.crypto,
}) {
  const scope = project.project_scope;
  let database;
  try {
    database = await openJournalDatabase(indexedDBImpl, scope);
    const metadata = database.transaction("metadata", "readonly").objectStore("metadata");
    const activeSessionKey = `active_session:${scope.owner_user_id}:${scope.project_id}`;
    const storedActive = await requestResult(metadata.get(activeSessionKey));
    const active = globalThis.sessionStorage?.getItem(activeSessionKey) ?? storedActive?.value;
    const session = active
      ? await getEditorSession({ baseUrl, projectId: scope.project_id,
        editorSessionId: active, fetchImpl })
      : await createSession({ baseUrl, project, profile, fetchImpl, cryptoImpl });
    await validateSession(session, scope, chapter, profile, cryptoImpl);
    const partition = partitionFrom(session, scope, profile);
    const transaction = database.transaction(["metadata", "partitions"], "readwrite");
    const partitions = transaction.objectStore("partitions");
    const activeBaseKey = `active_base:${partition.journal_partition_id}`;
    const [existingPartition, activeBase] = await Promise.all([
      requestResult(partitions.get(partition.journal_partition_id)),
      requestResult(transaction.objectStore("metadata").get(activeBaseKey)),
    ]);
    if (existingPartition && JSON.stringify(existingPartition) !== JSON.stringify(partition)) {
      transaction.abort();
      throw new Error("Local Edit Journal partition is corrupt");
    }
    if (activeBase && JSON.stringify(activeBase.value) !== JSON.stringify(session.base_snapshot)) {
      transaction.abort();
      throw new Error("Local Edit Journal active base is corrupt");
    }
    if (!existingPartition) partitions.add(partition);
    if (!activeBase) {
      transaction.objectStore("metadata").add({ key: activeBaseKey, value: session.base_snapshot });
    }
    transaction.objectStore("metadata").put({
      key: activeSessionKey, value: session.editor_session.editor_session_id,
    });
    await transactionResult(transaction);
    globalThis.sessionStorage?.setItem(activeSessionKey, session.editor_session.editor_session_id);
    const workspace = { database, partition, session, cryptoImpl,
      maxJsonStringUtf8Bytes: profile.max_json_string_utf8_bytes };
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
