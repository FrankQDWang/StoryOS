import {
  createProjectCommandChallenge,
  digestUndoLatestAuthorAction,
  getEditorSession,
  undoLatestAuthorAction,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ChallengeAdmissionWait } from "./challenge-admission-wait.ts";
import { historicalAcknowledgementUnavailable } from "./historical-acknowledgement.ts";
import type {
  EditorBaseSnapshot,
  UndoLatestAuthorActionRequest,
  UndoLatestAuthorActionResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import type { EditorReadyState, EditorWorkspace, PendingEditProjection } from "./editor-types.ts";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const positiveU64 = (value: unknown): value is string =>
  typeof value === "string" && U64.test(value) && BigInt(value) > 0n
    && BigInt(value) <= 18446744073709551615n;

type UndoSubmission = {
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

function uuidV7(cryptoImpl: Crypto, now = Date.now()): string {
  const bytes = cryptoImpl.getRandomValues(new Uint8Array(16));
  for (let offset = 5; offset >= 0; offset -= 1) {
    bytes[offset] = now & 0xff;
    now = Math.floor(now / 256);
  }
  bytes[6] = (bytes[6]! & 0x0f) | 0x70;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export async function installAuthoritativeBaseSnapshot(
  workspace: EditorWorkspace,
  base: EditorBaseSnapshot,
): Promise<void> {
  const partitionId = workspace.partition.journal_partition_id;
  const transaction = workspace.database.transaction(["metadata"], "readwrite", {
    durability: "strict",
  });
  transaction.objectStore("metadata").put({
    key: `active_base:${partitionId}`,
    value: base,
  });
  await new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(
      transaction.error ?? new Error("IndexedDB transaction aborted"),
    );
    transaction.onerror = () => reject(
      transaction.error ?? new Error("IndexedDB transaction failed"),
    );
  });
}

export async function undoOwnedLatestAuthorAction(options: {
  workspace: EditorReadyState;
  /** The projection that the editor shows when the Undo starts. */
  projection: PendingEditProjection;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  isCurrent: () => boolean;
  challengeAdmission: ChallengeAdmissionWait;
  onChallengeWait: () => void;
}): Promise<UndoLatestAuthorActionResponse | undefined> {
  if (options.workspace.partition.disposition !== "current_writer_open"
    || options.projection.save_state !== "saved") {
    throw new Error("Author Undo requires a settled current writer");
  }
  const canonical = await getEditorSession({
    baseUrl: options.baseUrl,
    projectId: options.workspace.partition.project_scope.project_id,
    editorSessionId: options.workspace.partition.editor_session_id,
    fetchImpl: options.fetchImpl,
  });
  if (canonical.project_scope.owner_user_id
      !== options.workspace.partition.project_scope.owner_user_id
    || canonical.project_scope.project_id
      !== options.workspace.partition.project_scope.project_id
    || canonical.editor_session.editor_session_id
      !== options.workspace.partition.editor_session_id
    || canonical.writer.kind !== "current_writer"
    || canonical.writer.writer_generation !== options.workspace.partition.writer_generation
    || canonical.base_snapshot.chapter_id
      !== options.workspace.session.base_snapshot.chapter_id
    || canonical.base_snapshot.authoritative_head_revision_id
      !== options.workspace.session.base_snapshot.authoritative_head_revision_id) {
    throw new Error("Author Undo requires the current Editor Session");
  }
  if (!options.isCurrent()) throw new Error("Undo view changed");
  options.workspace.session = canonical;
  const frontier = canonical.author_undo_frontier_sequence;
  const expectedHead = canonical.base_snapshot.authoritative_head_revision_id;
  if (!positiveU64(frontier)) return undefined;
  const flight: UndoSubmission = {
    idempotencyKey: uuidV7(options.cryptoImpl),
    correlationId: uuidV7(options.cryptoImpl),
  };
  try {
    const settled = await submitUndo(options, frontier, expectedHead, flight);
    if (settled.effect.kind === "compensated" || settled.effect.kind === "reversal_required") {
      await refreshSessionAfterCompensation(options);
    }
    return settled;
  } catch (error) {
    if (!options.isCurrent() || historicalAcknowledgementUnavailable(error)) throw error;
    const settled = await submitUndo(options, frontier, expectedHead, flight);
    if (settled.effect.kind === "compensated" || settled.effect.kind === "reversal_required") {
      await refreshSessionAfterCompensation(options);
    }
    return settled;
  }
}

async function refreshSessionAfterCompensation(options: {
  workspace: EditorWorkspace;
  baseUrl: string;
  fetchImpl: typeof fetch;
  isCurrent: () => boolean;
}): Promise<void> {
  const canonical = await getEditorSession({
    baseUrl: options.baseUrl,
    projectId: options.workspace.partition.project_scope.project_id,
    editorSessionId: options.workspace.partition.editor_session_id,
    fetchImpl: options.fetchImpl,
  });
  if (!options.isCurrent()) throw new Error("Undo view changed");
  await installAuthoritativeBaseSnapshot(options.workspace, canonical.base_snapshot);
  if (!options.isCurrent()) throw new Error("Undo view changed");
  options.workspace.session = canonical;
}

async function submitUndo(
  options: {
    workspace: EditorWorkspace;
    baseUrl: string;
    fetchImpl: typeof fetch;
    cryptoImpl: Crypto;
    isCurrent: () => boolean;
    challengeAdmission: ChallengeAdmissionWait;
    onChallengeWait: () => void;
  },
  frontier: string,
  expectedHead: string,
  flight: UndoSubmission,
): Promise<UndoLatestAuthorActionResponse> {
  const request = undoRequest(options.workspace, frontier, expectedHead, flight.correlationId);
  if (flight.nonce === undefined) {
    const challenge = await options.challengeAdmission.retry(async () => createProjectCommandChallenge({
      baseUrl: options.baseUrl,
      projectId: options.workspace.partition.project_scope.project_id,
      fetchImpl: options.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/author-actions/undo",
        command_schema: request.command_schema,
        canonical_command_digest: await digestUndoLatestAuthorAction(
          request,
          options.cryptoImpl,
        ),
        idempotency_key: flight.idempotencyKey,
      },
    }), options.onChallengeWait);
    if (challenge === undefined) throw new Error("Undo view changed");
    flight.nonce = challenge.nonce;
  }
  // An abandoned Undo is never sent, also when its Challenge request completed after the abandon.
  if (!options.isCurrent()) throw new Error("Undo view changed");
  return undoLatestAuthorAction({
    baseUrl: options.baseUrl,
    projectId: options.workspace.partition.project_scope.project_id,
    fetchImpl: options.fetchImpl,
    idempotencyKey: flight.idempotencyKey,
    antiForgery: flight.nonce,
    request,
  });
}

function undoRequest(workspace: EditorWorkspace, frontier: string, expectedHead: string, correlationId: string): UndoLatestAuthorActionRequest {
  return {
    command_schema: "storyos.command.undo-latest-author-action.request.v1",
    undo_latest_author_action_input: {
      expected_author_undo_frontier_sequence: frontier,
      expected_authoritative_revision_id: expectedHead,
      editor_session_id: workspace.partition.editor_session_id,
      client_contract_revision:
        RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: SECURITY_POLICY_REVISION,
      correlation_id: correlationId,
    },
  };
}
