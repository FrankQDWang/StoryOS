import {
  createEditorSession,
  createProjectCommandChallenge,
  digestCreateEditorSession,
  getEditorSession,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

const DATABASE_VERSION = 1;
const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";
const EDITOR_CONTRACT_REVISION = "storyos.editor-contract.release-1.v1";
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

async function digestJson(value, cryptoImpl) {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  const digest = new Uint8Array(await cryptoImpl.subtle.digest("SHA-256", bytes));
  return {
    algorithm: "sha256",
    profile: "storyos.local-edit-journal.payload.sha256.v1",
    value_hex_lowercase: [...digest]
      .map((byte) => byte.toString(16).padStart(2, "0")).join(""),
  };
}

async function openJournalDatabase(indexedDBImpl, scope) {
  if (!indexedDBImpl) throw new Error("IndexedDB is unavailable");
  const name = `storyos-local-edit-journal:${scope.owner_user_id}:${scope.project_id}`;
  const request = indexedDBImpl.open(name, DATABASE_VERSION);
  request.onupgradeneeded = (event) => {
    if (event.oldVersion !== 0) {
      request.transaction.abort();
      return;
    }
    const database = request.result;
    const metadata = database.createObjectStore("metadata", { keyPath: "key" });
    metadata.put({ key: "schema", version: DATABASE_VERSION });
    database.createObjectStore("partitions", { keyPath: "journal_partition_id" });
    const payloadChains = database.createObjectStore("payload_chains", { keyPath: "payload_chain_id" });
    payloadChains.createIndex("partition", "journal_partition_id", { unique: false });
    const intents = database.createObjectStore("intents", {
      keyPath: ["journal_partition_id", "local_intent_sequence"],
    });
    intents.createIndex("partition", "journal_partition_id", { unique: false });
  };
  const database = await requestResult(request);
  if (["metadata", "partitions", "payload_chains", "intents"]
    .some((name) => !database.objectStoreNames.contains(name))) {
    database.close();
    throw new Error("Local Edit Journal schema is incompatible");
  }
  const schema = await requestResult(database.transaction("metadata", "readonly")
    .objectStore("metadata").get("schema"));
  if (schema?.version !== DATABASE_VERSION) {
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
    || base?.project_activity_position !== "0"
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
    const active = globalThis.sessionStorage
      ? globalThis.sessionStorage.getItem(activeSessionKey) : undefined;
    const session = active
      ? await getEditorSession({ baseUrl, projectId: scope.project_id,
        editorSessionId: active, fetchImpl })
      : await createSession({ baseUrl, project, profile, fetchImpl, cryptoImpl });
    await validateSession(session, scope, chapter, profile, cryptoImpl);
    const partition = partitionFrom(session, scope, profile);
    const transaction = database.transaction("partitions", "readwrite");
    const partitions = transaction.objectStore("partitions");
    const existingPartition = await requestResult(partitions.get(partition.journal_partition_id));
    if (existingPartition && JSON.stringify(existingPartition) !== JSON.stringify(partition)) {
      transaction.abort();
      throw new Error("Local Edit Journal partition is corrupt");
    }
    if (!existingPartition) partitions.add(partition);
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

export async function persistReplaceSelection(workspace, edit, cryptoImpl = globalThis.crypto) {
  if (workspace.partition.disposition !== "current_writer_open") throw new Error("Editor Session is read only");
  const base = workspace.session.base_snapshot;
  if (!Number.isSafeInteger(edit.from) || !Number.isSafeInteger(edit.to)
    || edit.from < 0 || edit.to < edit.from || typeof edit.text !== "string"
    || typeof edit.resultingBody !== "string"
    || new TextEncoder().encode(edit.text).byteLength > workspace.maxJsonStringUtf8Bytes
    || new TextEncoder().encode(edit.resultingBody).byteLength
      > workspace.maxJsonStringUtf8Bytes) {
    throw new Error("Local Edit Journal limit failed");
  }
  const completedIntentRecordId = uuidV7(cryptoImpl);
  const payloadChainId = uuidV7(cryptoImpl);
  const patchId = uuidV7(cryptoImpl);
  const resultingPayloadDigest = await digestJson(edit.resultingBody, cryptoImpl);
  const payloadChain = {
    payload_chain_id: payloadChainId,
    journal_partition_id: workspace.partition.journal_partition_id,
    checkpoint_ref: {
      chapter_object_id: base.chapter_id,
      materialized_payload: base.materialized_revision.body,
      materialized_payload_digest: base.materialized_payload_digest,
      source_snapshot_id: base.snapshot_id,
      source_heads: [base.authoritative_head_revision_id],
    },
    ordered_patch_refs: [{
      patch_id: patchId,
      completed_intent_record_id: completedIntentRecordId,
      normalized_primitives: [{ kind: "replace_selection", from: edit.from, to: edit.to,
        text: edit.text }],
      resulting_payload_digest: resultingPayloadDigest,
    }],
  };
  const authorEditUnit = {
    normalized_primitives: [{ kind: "replace_selection", from: edit.from, to: edit.to,
      text: edit.text }],
    selection_snapshot: { from: edit.from, to: edit.to },
  };
  const payloadDigest = await digestJson(authorEditUnit, cryptoImpl);
  const transaction = workspace.database
    .transaction(["metadata", "partitions", "payload_chains", "intents"], "readwrite");
  const metadata = transaction.objectStore("metadata");
  const intents = transaction.objectStore("intents");
  const [schema, partition, current, partitionIntentCount] = await Promise.all([
    requestResult(metadata.get("schema")),
    requestResult(transaction.objectStore("partitions")
      .get(workspace.partition.journal_partition_id)),
    requestResult(metadata.get("local_intent_sequence")),
    requestResult(intents.index("partition").count(workspace.partition.journal_partition_id)),
  ]);
  if (schema?.version !== DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)
    || partitionIntentCount !== 0) {
    transaction.abort();
    throw new Error("Local Edit Journal schema or partition is incompatible");
  }
  const sequence = (current?.value ?? 0) + 1;
  if (!Number.isSafeInteger(sequence)) {
    transaction.abort();
    throw new Error("Local Edit Journal limit failed");
  }
  const record = {
    completed_intent_record_id: completedIntentRecordId,
    local_intent_sequence: sequence,
    journal_partition_id: workspace.partition.journal_partition_id,
    project_scope: workspace.partition.project_scope,
    editor_session_id: workspace.partition.editor_session_id,
    writer_generation: workspace.partition.writer_generation,
    limit_profile_revision: workspace.partition.limit_profile_revision,
    chapter_object_id: base.chapter_id,
    base_snapshot_id: base.snapshot_id,
    base_activity_position: base.project_activity_position,
    target_refs: base.target_refs,
    expected_authoritative_heads: [base.authoritative_head_revision_id],
    expected_proposal_heads: base.proposal_head_revision_ids,
    proposal_anchors: [],
    observed_ownership_partition: base.observed_ownership_partition,
    author_edit_unit: authorEditUnit,
    retry_source: { kind: "fresh_editor_intent" },
    editor_contract_revision: EDITOR_CONTRACT_REVISION,
    undo_group_binding: { kind: "ordinary_typing", undo_group_id: uuidV7(cryptoImpl) },
    payload_chain_ref: payloadChainId,
    payload_digest: payloadDigest,
    projection_dependency: { snapshot_id: base.snapshot_id, prior_sequence: sequence - 1 },
    created_at: new Date().toISOString(),
  };
  metadata.put({ key: "local_intent_sequence", value: sequence });
  metadata.put({ key: `durable_high_watermark:${workspace.partition.journal_partition_id}`,
    value: sequence });
  payloadChain.ordered_patch_refs[0].local_intent_sequence = sequence;
  transaction.objectStore("payload_chains").add(payloadChain);
  intents.add(record);
  await transactionResult(transaction);
  return rebuildPendingProjection(workspace);
}

export async function rebuildPendingProjection(workspace) {
  const transaction = workspace.database
    .transaction(["metadata", "payload_chains", "intents"], "readonly");
  const recordsRequest = transaction.objectStore("intents")
    .index("partition").getAll(workspace.partition.journal_partition_id, 2);
  const payloadChainsRequest = transaction.objectStore("payload_chains").index("partition")
    .getAll(workspace.partition.journal_partition_id, 2);
  const watermarkRequest = transaction.objectStore("metadata")
    .get(`durable_high_watermark:${workspace.partition.journal_partition_id}`);
  const [records, payloadChains, watermark] = await Promise.all([
    requestResult(recordsRequest), requestResult(payloadChainsRequest), requestResult(watermarkRequest),
  ]);
  const chainsById = new Map(payloadChains.map((chain) => [chain.payload_chain_id, chain]));
  if (records.length > 1 || payloadChains.length > 1) {
    throw new Error("Local Edit Journal exceeds this Editor Session scope");
  }
  records.sort((left, right) => left.local_intent_sequence - right.local_intent_sequence);
  const base = workspace.session.base_snapshot;
  let body = base.materialized_revision.body;
  let priorSequence = 0;
  for (const record of records) {
    const payloadChain = chainsById.get(record.payload_chain_ref);
    const checkpoint = payloadChain?.checkpoint_ref;
    const patchRefs = payloadChain?.ordered_patch_refs ?? [];
    const [patchRef] = patchRefs;
    const expectedDigest = await digestJson(record.author_edit_unit, workspace.cryptoImpl);
    if (record.journal_partition_id !== workspace.partition.journal_partition_id
      || record.editor_session_id !== workspace.partition.editor_session_id
      || record.writer_generation !== workspace.partition.writer_generation
      || record.limit_profile_revision !== workspace.partition.limit_profile_revision
      || record.chapter_object_id !== base.chapter_id
      || record.base_snapshot_id !== base.snapshot_id
      || record.base_activity_position !== base.project_activity_position
      || JSON.stringify(record.project_scope) !== JSON.stringify(workspace.partition.project_scope)
      || record.local_intent_sequence !== priorSequence + 1
      || record.projection_dependency?.snapshot_id !== base.snapshot_id
      || record.projection_dependency?.prior_sequence !== record.local_intent_sequence - 1
      || record.retry_source?.kind !== "fresh_editor_intent"
      || record.editor_contract_revision !== EDITOR_CONTRACT_REVISION
      || record.undo_group_binding?.kind !== "ordinary_typing"
      || !record.undo_group_binding?.undo_group_id
      || JSON.stringify(record.target_refs) !== JSON.stringify(base.target_refs)
      || JSON.stringify(record.expected_authoritative_heads)
        !== JSON.stringify([base.authoritative_head_revision_id])
      || JSON.stringify(record.expected_proposal_heads)
        !== JSON.stringify(base.proposal_head_revision_ids)
      || JSON.stringify(record.proposal_anchors) !== JSON.stringify([])
      || record.observed_ownership_partition !== base.observed_ownership_partition
      || payloadChain?.payload_chain_id !== record.payload_chain_ref
      || payloadChain?.journal_partition_id !== workspace.partition.journal_partition_id
      || patchRefs.length !== 1
      || checkpoint?.chapter_object_id !== base.chapter_id
      || checkpoint?.source_snapshot_id !== base.snapshot_id
      || checkpoint?.materialized_payload !== base.materialized_revision.body
      || JSON.stringify(checkpoint?.source_heads)
        !== JSON.stringify([base.authoritative_head_revision_id])
      || JSON.stringify(checkpoint?.materialized_payload_digest)
        !== JSON.stringify(base.materialized_payload_digest)
      || patchRef?.completed_intent_record_id !== record.completed_intent_record_id
      || patchRef?.local_intent_sequence !== record.local_intent_sequence
      || JSON.stringify(patchRef?.normalized_primitives)
        !== JSON.stringify(record.author_edit_unit.normalized_primitives)
      || JSON.stringify(record.payload_digest) !== JSON.stringify(expectedDigest)) {
      throw new Error("Local Edit Journal is corrupt");
    }
    const primitives = patchRef.normalized_primitives;
    const [patch] = primitives;
    if (primitives.length !== 1 || !patch || patch.kind !== "replace_selection"
      || patch.from < 0 || patch.to < patch.from
      || patch.to > body.length) throw new Error("Local Edit Journal reconstruction failed");
    body = `${body.slice(0, patch.from)}${patch.text}${body.slice(patch.to)}`;
    if (new TextEncoder().encode(body).byteLength > workspace.maxJsonStringUtf8Bytes) {
      throw new Error("Local Edit Journal limit failed");
    }
    const resultingDigest = await digestJson(body, workspace.cryptoImpl);
    if (JSON.stringify(resultingDigest) !== JSON.stringify(patchRef.resulting_payload_digest)) {
      throw new Error("Local Edit Journal is corrupt");
    }
    priorSequence = record.local_intent_sequence;
  }
  if ((records.length === 0 && watermark !== undefined)
    || (records.length > 0 && watermark?.value !== records.at(-1).local_intent_sequence)) {
    throw new Error("Local Edit Journal is corrupt");
  }
  return {
    body,
    save_state: records.length ? "saving" : "clean",
    unsettled_intent_count: records.length,
    authoritative_revision_id: workspace.session.base_snapshot.authoritative_head_revision_id,
  };
}
