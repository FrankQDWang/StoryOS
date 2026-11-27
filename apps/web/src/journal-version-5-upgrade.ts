import { digestApplyAuthorEdit } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { GetEditorSessionResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorWorkspace, JournalPartition, JournalSubmissionGroup } from "./editor-types.ts";
import { uuidV7 } from "./client-identity.ts";
import { requestResult, transactionResult } from "./indexed-db-request.ts";
import {
  candidateTextsFromJournal, hasCurrentSubmissionShape, journalHasOpenWork, readJournalSnapshot,
  rebuildPendingProjection, validatePayloadChains,
} from "./local-edit-journal.ts";
import { earlierJournalKey, type EarlierJournalText } from "./local-recovery-record.ts";

/**
 * The upgrade of a version 5 Local Edit Journal to version 6 (the ADR 0099 rule for a later version, ADR 0107). A
 * version 5 partition identifier has the writer generation after the Editor Session identifier, and the rows have the
 * writer fields of before ADR 0107. The upgrade changes the partition identifier in each field and key that stores it,
 * and it removes the writer fields. A frozen request keeps the form that the client sent, because its digest names that
 * form. A group that the client sent keeps its v1 request, because the Server keeps that request and its digest for the
 * outcome Query. A group that the client did not send gets a new v2 freeze after the open.
 */
const V1_AUTHOR_EDIT = "storyos.command.apply-author-edit.request.v1";
const V2_AUTHOR_EDIT = "storyos.command.apply-author-edit.request.v2";
const UNSENT_KEY = "version_5_unsent_groups";
const KEPT_KEY = "version_5_kept_partitions";
const SESSIONS_KEY = "version_5_sessions";
const WRITER_FIELDS = ["writer_generation", "partition_disposition", "resulting_writer_generation"];
const PARTITION_FIELDS = ["journal_partition_id", "working_set_partition_id"];
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const positiveU64 = (value: unknown): value is string => typeof value === "string" && U64.test(value)
  && BigInt(value) > 0n && BigInt(value) <= 18446744073709551615n;

type Row = Record<string, unknown>;
type Version5Partition = Row & {
  journal_partition_id: string; project_scope: { owner_user_id: string; project_id: string };
  editor_session_id: string; writer_generation: string; client_session_binding_ref: string;
  client_session_generation: string; client_contract_revision: string; security_policy_revision: string;
  limit_profile_revision: string;
};

/** Tells if `group` has the v1 request of before ADR 0107: the v1 form with a positive writer generation. */
export function hasVersion5Request(group: { command_schema?: unknown; frozen_request_body?: unknown }): boolean {
  const request = group.frozen_request_body as { command_schema?: unknown; writer_generation?: unknown } | undefined;
  return group.command_schema === V1_AUTHOR_EDIT && request?.command_schema === V1_AUTHOR_EDIT
    && positiveU64(request.writer_generation);
}

const SETTLEMENTS = ["unsettled", "applied_receipt_settled", "zero_authority_receipt_settled",
  "outcome_query_rejected_no_admission", "outcome_query_requires_reconfirmation"];

/** Tells if version 6 reads the version 5 `group`. */
function supportedGroup(group: Row): boolean {
  return hasVersion5Request(group) && SETTLEMENTS.includes(String((group.settlement as { kind?: unknown })?.kind))
    && hasCurrentSubmissionShape(group as JournalSubmissionGroup);
}

function identifier(partition: Version5Partition, generation: string[]): string {
  return [partition.project_scope.owner_user_id, partition.project_scope.project_id, partition.editor_session_id,
    ...generation, partition.client_session_binding_ref, partition.client_session_generation,
    partition.client_contract_revision, partition.security_policy_revision, partition.limit_profile_revision].join(":");
}

function without(row: Row, fields: readonly string[]): Row {
  return Object.fromEntries(Object.entries(row).filter(([field]) => !fields.includes(field)));
}

/** A version 5 partition identifier and its version 6 identifier, for each partition that the upgrade changes. */
type Changes = Map<string, string>;

/**
 * `value` in the version 6 form: each partition identifier that `changes` names changes, and the writer fields go. The
 * walk changes no other string, so the text of the author stays.
 */
function upgradeValue(value: unknown, changes: Changes): unknown {
  if (Array.isArray(value)) return value.map((item) => upgradeValue(item, changes));
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value).filter(([field]) => !WRITER_FIELDS.includes(field))
    .map(([field, item]) => [field, field === "frozen_request_body" ? item
      : PARTITION_FIELDS.includes(field) && typeof item === "string" ? changes.get(item) ?? item
        : upgradeValue(item, changes)]));
}

/** Tells if `value` stores the identifier of a partition that the upgrade changes. */
function namesChange(value: unknown, changes: Changes): boolean {
  if (value === null || typeof value !== "object") return false;
  return Object.entries(value).some(([field, item]) => field !== "frozen_request_body"
    && ((PARTITION_FIELDS.includes(field) && typeof item === "string" && changes.has(item)) || namesChange(item, changes)));
}

/** A metadata key with the changed partition identifier after its kind, as `active_base:<partition>`. */
function upgradeKey(key: string, changes: Changes): string {
  const at = key.indexOf(":") + 1;
  for (const [prior, next] of changes) {
    const end = at + prior.length;
    if (at > 0 && key.startsWith(prior, at) && (key.length === end || key[end] === ":")) {
      return key.slice(0, at) + next + key.slice(end);
    }
  }
  return key;
}

/**
 * Changes each row of a version 5 Journal in the version-change `transaction`. For each version 6 identifier, it
 * changes the partition of the highest writer generation that has records or groups. The rows of each other partition
 * stay as they are: a partition with an identifier that does not match its fields, a partition with a group that has
 * no version 5 request, and a partition whose version 6 identifier is the identifier of a stored partition. A kept
 * partition with rows gets its text in local recovery after the open. A failure aborts the transaction, and the open
 * fails.
 */
export function upgradeVersion5Journal(transaction: IDBTransaction): void {
  void (async () => {
    const partitions = await requestResult(transaction.objectStore("partitions").getAll()) as Version5Partition[];
    const stored = new Set(partitions.map((partition) => partition.journal_partition_id));
    const chosen = new Map<string, { partition: Version5Partition; rows: boolean }>();
    const withRows: string[] = [];
    for (const partition of partitions) {
      const id = partition.journal_partition_id;
      const groups = await requestResult(transaction.objectStore("submission_groups").index("partition")
        .getAll(id)) as Row[];
      const rows = groups.length + await requestResult(transaction.objectStore("intents").index("partition")
        .count(id)) > 0;
      if (rows) withRows.push(id);
      // The version 5 client wrote each group with a v1 request and a writer generation, and with a settlement that
      // version 6 reads.
      if (!positiveU64(partition.writer_generation) || id !== identifier(partition, [partition.writer_generation])
        || !groups.every(supportedGroup) || stored.has(identifier(partition, []))) continue;
      const next = identifier(partition, []);
      const prior = chosen.get(next);
      if (prior === undefined || (rows && !prior.rows) || (rows === prior.rows
        && BigInt(partition.writer_generation) > BigInt(prior.partition.writer_generation))) {
        chosen.set(next, { partition, rows });
      }
    }
    const changes: Changes = new Map([...chosen].map(([next, { partition }]) => [partition.journal_partition_id, next]));
    const unsent: string[] = [];
    for (const [prior, next] of changes) {
      const store = transaction.objectStore("partitions");
      const partition = await requestResult(store.get(prior)) as Row;
      store.delete(prior);
      store.put({ ...without(partition, ["writer_generation", "disposition"]), journal_partition_id: next });
      for (const name of ["intents", "payload_chains", "submission_groups"]) {
        const rows = transaction.objectStore(name);
        for (const row of await requestResult(rows.index("partition").getAll(prior)) as Row[]) {
          if (name === "intents") rows.delete([prior, row.local_intent_sequence as number]);
          rows.put(upgradeValue(row, changes));
          const group = row as Partial<JournalSubmissionGroup>;
          if (name === "submission_groups" && hasVersion5Request(group) && group.settlement?.kind === "unsettled"
            && group.reconciliation === undefined && await requestResult(transaction.objectStore("transport_capsules")
            .index("group").count(group.journal_submission_group_id!)) === 0) unsent.push(group.journal_submission_group_id!);
        }
      }
    }
    const metadata = transaction.objectStore("metadata");
    for (const row of await requestResult(metadata.getAll()) as Row[]) {
      const key = upgradeKey(row.key as string, changes);
      if (key === row.key && !namesChange(row, changes)) continue;
      metadata.delete(row.key as string);
      metadata.put({ ...upgradeValue(row, changes) as Row, key });
    }
    if (unsent.length > 0) metadata.put({ key: UNSENT_KEY, value: unsent });
    const sessions: Version5Session[] = [...chosen].filter(([, { rows }]) => rows).map(([next, { partition }]) =>
      ({ editor_session_id: partition.editor_session_id, journal_partition_id: next }));
    if (sessions.length > 0) metadata.put({ key: SESSIONS_KEY, value: sessions });
    const kept = withRows.filter((partitionId) => !changes.has(partitionId));
    if (kept.length > 0) metadata.put({ key: KEPT_KEY, value: kept });
  })().catch(() => {
    try {
      transaction.abort();
    } catch {
      // The transaction ended already.
    }
  });
}

/** The Editor Session of a changed partition with rows, and the version 6 identifier of that partition. */
export interface Version5Session { editor_session_id: string; journal_partition_id: string }

const heldHere = new Set<string>();

/**
 * Holds the Web Lock of `session` while this page lives, so that no other tab opens that Session. The browser releases
 * the lock when the page closes. It tells if this page holds the lock.
 */
function holdSession(session: Version5Session): Promise<boolean> {
  const name = editorSessionLock(session.editor_session_id);
  const locks = globalThis.navigator?.locks;
  if (heldHere.has(name) || locks === undefined) return Promise.resolve(true);
  return new Promise((resolve) => {
    void locks.request(name, { ifAvailable: true }, (lock) => {
      resolve(lock !== null);
      if (lock === null) return undefined;
      heldHere.add(name);
      return new Promise<void>(() => {});
    });
  });
}

/**
 * The name of the Web Lock of an Editor Session. The tab that has the Session open holds it, so that no other open tab
 * opens that Session.
 */
export function editorSessionLock(editorSessionId: string): string {
  return `storyos-editor-session:${editorSessionId}`;
}

/**
 * Selects the Editor Session that a tab opens after the upgrade of a version 5 Journal. A tab with a `stored` Session
 * of the version 5 rows opens it when no other open tab has it, else the tab opens a new Session. A tab with no stored
 * Session, as a new tab or a browser that started again, opens the first Session of the version 5 rows that no other
 * open tab has. `claimed` is that Session.
 */
export async function claimVersion5Session(database: IDBDatabase, stored: string | null | undefined):
  Promise<{ active: string | null | undefined; claimed?: Version5Session }> {
  const marker = await requestResult(database.transaction("metadata", "readonly").objectStore("metadata")
    .get(SESSIONS_KEY)) as { value: Version5Session[] } | undefined;
  const sessions = marker?.value ?? [];
  if (stored) {
    const session = sessions.find((item) => item.editor_session_id === stored);
    return { active: session === undefined || await holdSession(session) ? stored : null };
  }
  for (const session of sessions) {
    if (await holdSession(session)) return { active: session.editor_session_id, claimed: session };
  }
  return { active: stored };
}

/**
 * Removes `session`, which did not open, from the Sessions of the version 5 rows. Its text goes to local recovery first,
 * so that a new Session of the tab does not hide it.
 */
export async function releaseVersion5Session(database: IDBDatabase, session: Version5Session, cryptoImpl: Crypto,
  maxJsonStringUtf8Bytes: number): Promise<void> {
  await retainPartitionTexts(database, [session.journal_partition_id], cryptoImpl, maxJsonStringUtf8Bytes);
  const transaction = database.transaction("metadata", "readwrite");
  const metadata = transaction.objectStore("metadata");
  const marker = await requestResult(metadata.get(SESSIONS_KEY)) as { value: Version5Session[] } | undefined;
  const rest = (marker?.value ?? []).filter((item) => item.editor_session_id !== session.editor_session_id);
  if (rest.length === 0) metadata.delete(SESSIONS_KEY);
  else metadata.put({ key: SESSIONS_KEY, value: rest });
  await transactionResult(transaction);
}

/**
 * Freezes each version 5 group that the client did not send again as v2, after the open: the same coverage and
 * correlation, no writer generation, a new idempotency key, and a new digest. No POST of that group went to the
 * Server, because the client commits a capsule before it sends, so a new key cannot repeat a command. A group that has
 * no version 5 request had its new freeze in another tab, and it stays.
 */
export async function freezeUnsentVersion5Groups(database: IDBDatabase, cryptoImpl: Crypto): Promise<void> {
  const marker = await requestResult(database.transaction("metadata", "readonly").objectStore("metadata")
    .get(UNSENT_KEY)) as { value: string[] } | undefined;
  if (marker === undefined) return;
  const read = database.transaction("submission_groups", "readonly").objectStore("submission_groups");
  const groups = (await Promise.all(marker.value.map((id) => requestResult(read.get(id)))) as
    (JournalSubmissionGroup | undefined)[]).filter((group) => group !== undefined);
  const frozen = await Promise.all(groups.map(async (group) => {
    const { writer_generation: _writer, ...rest } = group.frozen_request_body as { writer_generation?: string };
    const request = { ...rest, command_schema: V2_AUTHOR_EDIT } as JournalSubmissionGroup["frozen_request_body"];
    return { ...group, command_schema: V2_AUTHOR_EDIT, idempotency_key: uuidV7(cryptoImpl),
      frozen_request_body: request, frozen_request_digest: await digestApplyAuthorEdit(request, cryptoImpl) };
  }));
  const transaction = database.transaction(["metadata", "submission_groups", "transport_capsules"], "readwrite");
  const store = transaction.objectStore("submission_groups");
  for (const [index, group] of groups.entries()) {
    const current = await requestResult(store.get(group.journal_submission_group_id)) as JournalSubmissionGroup;
    // Another tab completed the new freeze of this group, before or after the read of this tab.
    if (current !== undefined && !hasVersion5Request(current)) continue;
    const sent = await requestResult(transaction.objectStore("transport_capsules").index("group")
      .count(group.journal_submission_group_id));
    if (JSON.stringify(current) !== JSON.stringify(group) || sent !== 0) {
      transaction.abort();
      throw new Error("A version 5 group changed before its new freeze");
    }
    store.put(frozen[index]);
  }
  transaction.objectStore("metadata").delete(UNSENT_KEY);
  await transactionResult(transaction);
}

/**
 * Keeps the text of each version 5 partition that the upgrade did not change, after the open. Its rows stay as they
 * are, and its text goes to local recovery, which each Editor Session of the Project shows.
 */
export async function retainVersion5PartitionText(
  database: IDBDatabase, cryptoImpl: Crypto, maxJsonStringUtf8Bytes: number,
): Promise<void> {
  const marker = await requestResult(database.transaction("metadata", "readonly").objectStore("metadata")
    .get(KEPT_KEY)) as { value: string[] } | undefined;
  if (marker === undefined) return;
  await retainPartitionTexts(database, marker.value, cryptoImpl, maxJsonStringUtf8Bytes);
  const transaction = database.transaction("metadata", "readwrite");
  transaction.objectStore("metadata").delete(KEPT_KEY);
  await transactionResult(transaction);
}

/**
 * Puts the text of each partition of `partitionIds` in local recovery when the partition has work that the Server does
 * not have: its last Chapter text when it changed, and the text of each candidate with unsent input. When the client
 * cannot read the groups of the partition, the text comes from its records only. A partition that the client cannot
 * read shows no text, as a corrupt row today.
 */
async function retainPartitionTexts(
  database: IDBDatabase, partitionIds: string[], cryptoImpl: Crypto, maxJsonStringUtf8Bytes: number,
): Promise<void> {
  const texts: EarlierJournalText[] = [];
  for (const partitionId of partitionIds) {
    const transaction = database.transaction(["partitions", "metadata"], "readonly");
    const [partition, base] = await Promise.all([
      requestResult(transaction.objectStore("partitions").get(partitionId)),
      requestResult(transaction.objectStore("metadata").get(`active_base:${partitionId}`)),
    ]) as [JournalPartition, { value: GetEditorSessionResponse["base_snapshot"] } | undefined];
    const workspace: EditorWorkspace = { database, partition, cryptoImpl, maxJsonStringUtf8Bytes,
      session: { base_snapshot: base?.value } as GetEditorSessionResponse };
    const text = (value: string, target?: EarlierJournalText["proposal_target"]): EarlierJournalText => ({
      format_version: 1, disposition: "retained_from_earlier_journal", project_scope: partition.project_scope,
      journal_partition_id: partitionId, ...(target ? { proposal_target: target } : {}), text: value });
    try {
      if (!await journalHasOpenWork(workspace)) continue;
      const projection = await rebuildPendingProjection(workspace);
      const candidates = await candidateTextsFromJournal(workspace);
      if (JSON.stringify(projection.blocks) !== JSON.stringify(base?.value.materialized_revision.blocks)) {
        texts.push(text(projection.body));
      }
      for (const [key, candidate] of candidates) {
        const [proposal_id, operation_id] = key.split(":") as [string, string];
        texts.push(text(candidate.text, { proposal_id, operation_id }));
      }
    } catch {
      try {
        // The groups do not pass the checks of the client. The last text of each target comes from the records and
        // their payloads, and the groups and records stay as they are.
        const snapshot = await readJournalSnapshot(workspace);
        const { bodyBySequence } = await validatePayloadChains(workspace, snapshot.records, snapshot.payloadChains);
        const last = new Map<string, { text: string; target?: EarlierJournalText["proposal_target"] }>();
        for (const record of snapshot.records) {
          const target = record.proposal_target;
          const value = bodyBySequence.get(record.local_intent_sequence);
          if (value === undefined) continue;
          last.set(target ? `${target.proposal_id}:${target.operation_id}` : "", { text: value, ...(target
            ? { target: { proposal_id: target.proposal_id, operation_id: target.operation_id } } : {}) });
        }
        texts.push(...[...last.values()].map((item) => text(item.text, item.target)));
      } catch {
        // The rows stay, and the page shows no text of them.
      }
    }
  }
  const transaction = database.transaction("metadata", "readwrite");
  const metadata = transaction.objectStore("metadata");
  for (const text of texts) metadata.put({ key: earlierJournalKey(text), value: text });
  await transactionResult(transaction);
}
