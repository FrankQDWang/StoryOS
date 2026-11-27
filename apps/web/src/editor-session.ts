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
  appendAuthorEdit,
  journalHasOpenWork,
  readJournalSnapshot,
  rebuildPendingProjection,
  validateJournalSnapshot,
} from "./local-edit-journal.ts";
import { blocksOnCurrent, stepsOnCurrent, type EarlierBaseText } from "./earlier-base-catch-up.ts";
import { paragraphsEqual } from "./manuscript-doc.ts";
import type { ManuscriptParagraph } from "./manuscript-doc.ts";
import { SECURITY_POLICY_REVISION, uuidV7 } from "./client-identity.ts";
import { requestResult, transactionResult } from "./indexed-db-request.ts";
import { lockEditorSession, releaseEarlierEditorSessions } from "./editor-session-lock.ts";
import { recoveryPrefix } from "./local-recovery-record.ts";
import {
  claimVersion5Session, freezeRejectedVersion5Groups, freezeUnsentVersion5Groups, releaseVersion5Session,
  retainVersion5PartitionText, upgradeVersion5Journal, type Version5Session,
} from "./journal-version-5-upgrade.ts";

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
    // A version 5 Journal has the writer fields of before ADR 0107. The upgrade removes them from each row, also from
    // the partition row, and keeps the frozen requests.
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
    await freezeRejectedVersion5Groups(database, cryptoImpl);
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
  finish(chapter: GetChapterResponse, project?: GetProjectResponse, shown?: readonly ManuscriptParagraph[])
    : Promise<OpenedEditorWorkspace>;
  /**
   * The stored Editor Session and the Chapter of its base, when that Chapter is not `currentChapterId` and the Session
   * has text that can catch up on it: another tab set another Chapter current (ADR 0107, section 2).
   */
  otherChapter(currentChapterId: string, shown?: readonly ManuscriptParagraph[])
    : Promise<{ chapterId: string; editorSessionId: string } | undefined>;
  /** Closes the Journal when the open stops before `finish`. The promise ends after the Session read ended. */
  abandon(): Promise<void>;
}

export interface OpenedEditorWorkspace {
  editor: EditorState;
  /** The Chapter of the editor: the Chapter read, or the later base of a new Editor Session. */
  chapter: GetChapterResponse;
}

type EditorSessionStart =
  | { kind: "started"; database: IDBDatabase; active: string | null | undefined; sessionValue: unknown;
    creation: SessionCreation | undefined; claimed?: Version5Session }
  | { kind: "failed"; database: IDBDatabase | undefined; error: unknown; claimed?: Version5Session };

/** Opens the Journal and reads the stored Editor Session at once, so that the read is in flight with the Chapter read. */
export function startEditorWorkspace(options: EditorWorkspaceOptions): EditorWorkspaceOpening {
  const {
    baseUrl, project, profile, fetchImpl = globalThis.fetch,
    indexedDBImpl = globalThis.indexedDB, cryptoImpl = globalThis.crypto, earlyChallenge = true,
  } = options;
  const scope = project.project_scope;
  const started = (async (): Promise<EditorSessionStart> => {
    let database: IDBDatabase | undefined;
    let claimed: Version5Session | undefined;
    try {
      const activeSessionKey = `active_session:${scope.owner_user_id}:${scope.project_id}`;
      const stored = globalThis.sessionStorage
        ? globalThis.sessionStorage.getItem(activeSessionKey) : undefined;
      // The creation waits for the Chapter read (#1884), and its Challenge starts with that read (#1899).
      const creation = !stored && earlyChallenge
        ? prepareSessionCreation({ baseUrl, project, profile, fetchImpl, cryptoImpl }) : undefined;
      database = await openJournalDatabase(indexedDBImpl, scope, cryptoImpl, profile.max_json_string_utf8_bytes);
      // A tab with no stored Editor Session opens a Session of the rows of an upgraded version 5 Journal.
      const choice = await claimVersion5Session(database, stored);
      claimed = choice.claimed;
      // A Session that another page writes with is not this page's Session, so the open creates one.
      const active = choice.active && !await lockEditorSession(choice.active) ? null : choice.active;
      // A stored Session read changes nothing on the Server, so it runs with the Chapter read.
      const sessionValue: unknown = active
        ? await getEditorSession({ baseUrl, projectId: scope.project_id,
          editorSessionId: active, fetchImpl })
        : undefined;
      return { kind: "started", database, active, sessionValue, creation, ...(claimed ? { claimed } : {}) };
    } catch (error) {
      return { kind: "failed", database, error, ...(claimed ? { claimed } : {}) };
    }
  })();
  return {
    finish: async (chapter, read = project, shown) => {
      const opened = await finishEditorWorkspace(started, { baseUrl, project: read, chapter, profile, fetchImpl,
        cryptoImpl, shown });
      const { claimed } = await started;
      if (claimed === undefined || opened.editor.kind === "editor-ready") return opened;
      // The claimed Session of the version 5 rows did not open. Its text goes to local recovery, and the tab opens a
      // new Session.
      try {
        const database = await openJournalDatabase(indexedDBImpl, scope, cryptoImpl,
          profile.max_json_string_utf8_bytes);
        try {
          await releaseVersion5Session(database, claimed, cryptoImpl, profile.max_json_string_utf8_bytes);
        } finally {
          database.close();
        }
      } catch {
        return opened;
      }
      return startEditorWorkspace(options).finish(chapter, read, shown);
    },
    otherChapter: async (currentChapterId, shown) => {
      const start = await started;
      if (start.kind !== "started" || !start.active) return undefined;
      try {
        const stored = await storedWorkspace(start.database, start.sessionValue, scope, profile, cryptoImpl);
        const base = stored.session.base_snapshot;
        if (base.chapter_id === currentChapterId) return undefined;
        // A tab with no text to send opens the Current Chapter that the other tab set.
        if ((shown === undefined || paragraphsEqual(shown, base.materialized_revision.blocks))
          && !await partitionHasOpenWork(stored)) return undefined;
        return await earlierBaseText(stored, base.chapter_id, shown) === undefined ? undefined
          : { chapterId: base.chapter_id, editorSessionId: start.active };
      } catch {
        // An unreadable Journal keeps the stored Session, so its recovery runs.
        return undefined;
      }
    },
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
 * The Chapter read with the base of a new Editor Session of the same Chapter, when that base is later than the read.
 * Another tab can save between the Chapter read and the creation.
 */
function chapterAtLaterBase(chapter: GetChapterResponse, value: unknown): GetChapterResponse {
  const base = (value as Partial<GetEditorSessionResponse> | undefined)?.base_snapshot;
  if (base?.chapter_id !== chapter.chapter.chapter_id || base.materialized_revision === undefined
    || !boundedU64(base.project_activity_position) || !boundedU64(chapter.project_activity_position)
    || BigInt(base.project_activity_position) <= BigInt(chapter.project_activity_position)) return chapter;
  return { ...chapter, project_activity_position: base.project_activity_position,
    chapter: { ...chapter.chapter, current_revision: base.materialized_revision } };
}

/** The workspace of the stored Editor Session. A deletion moves its Server base (ADR 0107), so it has the Journal base. */
async function storedWorkspace(database: IDBDatabase, value: unknown, scope: ProjectScope,
  profile: Release1ProtocolProfile, cryptoImpl: Crypto): Promise<EditorWorkspace> {
  const served = value as GetEditorSessionResponse;
  const partition = partitionFrom(served, scope, profile);
  const stored = (await requestResult(database.transaction("metadata", "readonly").objectStore("metadata")
    .get(`active_base:${partition.journal_partition_id}`)) as { value?: GetEditorSessionResponse["base_snapshot"] }
    | undefined)?.value;
  // The Journal then has the earlier base, which the check of its rows uses. Another earlier base is corrupt, and the
  // check counts it as open work.
  const moved = stored !== undefined && boundedU64(stored.project_activity_position)
    && boundedU64(served.base_snapshot.project_activity_position)
    && BigInt(stored.project_activity_position) < BigInt(served.base_snapshot.project_activity_position);
  const session = moved ? { ...served, base_snapshot: stored } : served;
  return { database, partition, session, cryptoImpl, maxJsonStringUtf8Bytes: profile.max_json_string_utf8_bytes };
}

/** Tells if the partition of `workspace` has a decision flight, an unresolved Withdrawal, or a retained recovery row. */
async function partitionHasDecisionWork(workspace: EditorWorkspace): Promise<boolean> {
  const partitionId = workspace.partition.journal_partition_id;
  const metadata = workspace.database.transaction("metadata", "readonly").objectStore("metadata");
  const rows = (prefix: string) => requestResult(metadata.getAll(IDBKeyRange.bound(
    `${prefix}${partitionId}:`, `${prefix}${partitionId}:\uffff`)));
  const [acceptances, rejections, withdrawals, retained] = await Promise.all([rows("acceptance:"),
    rows("rejection:"), rows("withdraw:"), rows(recoveryPrefix)]) as [unknown[], unknown[],
    { settlement?: unknown }[], unknown[]];
  return acceptances.length + rejections.length + retained.length > 0
    || withdrawals.some((row) => row.settlement === "unresolved");
}

/**
 * Tells if the Journal partition of the stored Editor Session has work that the Server does not have: Author Edit work
 * that the rule of the working set keeps open (`journalHasOpenWork`), a decision flight, an unresolved Withdrawal, or
 * a retained recovery row. Work that this check counts as open keeps the stored Editor Session.
 */
async function partitionHasOpenWork(workspace: EditorWorkspace): Promise<boolean> {
  return await journalHasOpenWork(workspace) || partitionHasDecisionWork(workspace);
}

/**
 * The text of the stored Editor Session that a catch-up can put on the current Chapter (ADR 0107, section 2), or
 * `undefined` when its open work is not only Author Edit text of the Chapter. A group that went out with no known
 * result, input in a Proposal candidate, and decision work keep the stored Session, because the catch-up cannot know
 * if they applied. `shown` is the text that a live tab shows, with its input that is not in the Journal.
 */
async function earlierBaseText(workspace: EditorWorkspace, chapterId: string,
  shown: readonly ManuscriptParagraph[] | undefined): Promise<EarlierBaseText | undefined> {
  const base = workspace.session.base_snapshot;
  if (base.chapter_id !== chapterId || await partitionHasDecisionWork(workspace)) return undefined;
  const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  const revisedCandidate = new Set(snapshot.groups.flatMap((group) => group.settlement.kind
    === "zero_authority_receipt_settled" && group.settlement.effect.kind === "proposal_revised"
    ? group.ordered_coverage.map((item) => item.local_intent_sequence) : []));
  // Only a later head stopped the text. A refused edit, or a conflict for another reason, needs its recovery.
  const known = ({ settlement }: (typeof snapshot.groups)[number]) => settlement.kind === "applied_receipt_settled"
    || settlement.kind === "zero_authority_receipt_settled" && (settlement.effect.kind === "proposal_revised"
      || settlement.effect.kind === "no_effect" || settlement.effect.kind === "conflicted"
      && settlement.effect.reason === "stale_authoritative_head");
  if (!snapshot.groups.every(known) || snapshot.records.some((record) => record.proposal_target !== undefined
    && !revisedCandidate.has(record.local_intent_sequence))) return undefined;
  return { base: base.materialized_revision.blocks,
    shown: shown ?? (await rebuildPendingProjection(workspace)).blocks };
}

async function finishEditorWorkspace(started: Promise<EditorSessionStart>, {
  baseUrl, project, chapter, profile, fetchImpl, cryptoImpl, shown,
}: {
  baseUrl: string;
  project: GetProjectResponse;
  chapter: GetChapterResponse;
  profile: Release1ProtocolProfile;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  shown?: readonly ManuscriptParagraph[] | undefined;
}): Promise<OpenedEditorWorkspace> {
  const scope = project.project_scope;
  let start = await started;
  // The text of the stored Session that the new Editor Session puts on the current Chapter (ADR 0107, section 2).
  let text: EarlierBaseText | undefined;
  const mintId = () => uuidV7(cryptoImpl);
  // ADR 0107: another Editor Session can write, so the stored Session can have an earlier base than the Chapter read.
  // When its Journal partition has no open work, the load opens a new Editor Session, as a new tab does. When its open
  // work is Author Edit text that has an Author Edit form on the Chapter, the new Editor Session gets that text.
  if (start.kind === "started" && start.active && baseIsNotChapter(start.sessionValue, chapter)) {
    try {
      const stored = await storedWorkspace(start.database, start.sessionValue, scope, profile, cryptoImpl);
      if (!await partitionHasOpenWork(stored)) {
        start = { ...start, active: null };
      } else {
        text = await earlierBaseText(stored, chapter.chapter.chapter_id, shown);
        const current = chapter.chapter.current_revision.blocks;
        if (text !== undefined && stepsOnCurrent(current, blocksOnCurrent(text, current, mintId)) === undefined) {
          text = undefined;
        }
        if (text !== undefined) start = { ...start, active: null };
      }
    } catch {
      // An unreadable Journal keeps the stored Session, so its recovery runs.
      text = undefined;
    }
  }
  if (start.kind === "started" && !start.active) {
    try {
      const options = { baseUrl, project, profile, fetchImpl, cryptoImpl };
      start = { ...start, sessionValue: await createSession(start.creation ?? prepareSessionCreation(options),
        options) };
      // A save of another tab between the Chapter read and the creation gives the new Editor Session a later base.
      chapter = chapterAtLaterBase(chapter, start.sessionValue);
    } catch (error) {
      start = { kind: "failed", database: start.database, error };
    }
  }
  if (start.kind === "failed") {
    start.database?.close();
    // The Journal did not open, for example a Journal of an earlier version (ADR 0099), or the Session request failed.
    return { editor: { kind: "editor-read-only-recovery", code: "local_journal_unavailable", error: start.error },
      chapter };
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
    if (!await lockEditorSession(session.editor_session.editor_session_id)) {
      throw new Error("Another page writes with the Editor Session");
    }
    // The tab changes to the new Session only after its Journal has the text, so a stop keeps the stored Session.
    const current = session.base_snapshot.materialized_revision.blocks;
    const catchUp = text === undefined ? [] : stepsOnCurrent(current, blocksOnCurrent(text, current, mintId));
    if (catchUp === undefined) throw new Error("The text of the stored Editor Session has no Author Edit form");
    const undoGroupId = uuidV7(cryptoImpl);
    const createdAt = new Date().toISOString();
    for (const edit of catchUp) {
      await appendAuthorEdit(workspace, { ...edit, inputOrigin: "selection_replacement", undoGroupId, createdAt },
        cryptoImpl);
    }
    globalThis.sessionStorage?.setItem(activeSessionKey, session.editor_session.editor_session_id);
    releaseEarlierEditorSessions(session.editor_session.editor_session_id);
    const pending = await rebuildPendingProjection(workspace);
    return { editor: { kind: "editor-ready", ...workspace, openedProjection: pending }, chapter };
  } catch (error) {
    database.close();
    return { editor: { kind: "editor-read-only-recovery", code: "local_journal_unavailable", error }, chapter };
  }
}
