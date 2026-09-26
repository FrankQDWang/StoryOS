import {
  createProjectCommandChallenge,
  digestUndoLatestAuthorAction,
  getEditorSession,
  getRefusedEditDraft,
  undoLatestAuthorAction,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { historicalAcknowledgementUnavailable } from "./historical-acknowledgement.ts";
import type {
  EditorBaseSnapshot,
  UndoLatestAuthorActionRequest,
  EditorFlowDraftReopened,
  UndoLatestAuthorActionResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import type { EditorReadyState, EditorWorkspace } from "./editor-types.ts";

import { readDiscardJournal, canonicalDraftValue } from "./refused-edit-discard.ts";
import { freezeDraftUndo, readDraftUndoJournal, observeDraftUndo, reconcileDraftUndo,
  type DraftUndoRecord } from "./draft-undo-journal.ts";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const positiveU64 = (value: unknown): value is string =>
  typeof value === "string" && U64.test(value) && BigInt(value) > 0n
    && BigInt(value) <= 18446744073709551615n;

type InFlightUndo = {
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

const inFlight = new Map<string, InFlightUndo>();

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

function undoIdentity(options: {
  projectId: string;
  expectedAuthorUndoFrontierSequence: string;
  expectedAuthoritativeRevisionId: string;
}): string {
  return `${options.projectId}\n${options.expectedAuthorUndoFrontierSequence}\n${options.expectedAuthoritativeRevisionId}`;
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
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  isCurrent: () => boolean;
}): Promise<UndoLatestAuthorActionResponse | { effect: { kind: "draft_reconciled"; event: EditorFlowDraftReopened } } | undefined> {
  const retainedUndo = await readDraftUndoJournal(options.workspace);
  const pendingUndo = retainedUndo.filter(({ observation }) => observation === undefined);
  if (options.workspace.partition.disposition !== "current_writer_open"
    || (options.workspace.pending.save_state !== "saved"
      && options.workspace.pending.unsettled_intent_count !== pendingUndo.length)) {
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
  let durable: DraftUndoRecord | undefined = pendingUndo[0]?.record;
  if (durable !== undefined && durable.journal_partition_id !== options.workspace.partition.journal_partition_id) {
    throw new Error("Original Undo belongs to another writer");
  }
  const frontier = durable?.request.undo_latest_author_action_input.expected_author_undo_frontier_sequence
    ?? canonical.author_undo_frontier_sequence;
  const expectedHead = durable?.request.undo_latest_author_action_input.expected_authoritative_revision_id
    ?? canonical.base_snapshot.authoritative_head_revision_id;
  if (!positiveU64(frontier)) return undefined;
  const identity = undoIdentity({
    projectId: options.workspace.partition.project_scope.project_id,
    expectedAuthorUndoFrontierSequence: frontier,
    expectedAuthoritativeRevisionId: expectedHead,
  });
  let flight = inFlight.get(identity);
  if (flight === undefined) {
    flight = {
      idempotencyKey: uuidV7(options.cryptoImpl),
      correlationId: uuidV7(options.cryptoImpl),
    };
    inFlight.set(identity, flight);
  }
  if (durable === undefined) {
    const closed = (await readDiscardJournal(options.workspace)).flatMap(({ observation }) =>
      observation?.kind === "settled_closed" ? [observation.event]
        : observation?.kind === "settled" && observation.response.effect.kind === "draft_closure_changed"
          ? [observation.response.effect.event] : []).find((event) => event.author_action_sequence === frontier);
    if (closed !== undefined) {
      const current = await getRefusedEditDraft({ baseUrl: options.baseUrl, projectId: canonical.project_scope.project_id,
        draftId: closed.draft_id, fetchImpl: options.fetchImpl });
      if (current.draft.closure !== "closed" || current.draft.retention_state !== "retained"
        || canonicalDraftValue(current.draft.closure_event) !== canonicalDraftValue(closed)) throw new Error("Undo source changed");
      durable = await freezeDraftUndo(options.workspace, closed,
        undoRequest(options.workspace, frontier, expectedHead, flight.correlationId), flight.idempotencyKey, options.isCurrent);
    }
  }
  if (durable !== undefined) {
    flight.idempotencyKey = durable.idempotency_key;
    flight.correlationId = durable.request.undo_latest_author_action_input.correlation_id;
  }
  const guarded = { ...options, fetchImpl: ((input, init) => {
    if (!options.isCurrent()) throw new Error("Undo view changed");
    return options.fetchImpl(input, init);
  }) as typeof fetch };
  try {
    const settled = await submitUndo(durable === undefined ? options : guarded, frontier, expectedHead, flight, durable?.request);
    if (durable !== undefined) await observeDraftUndo(options.workspace, durable, { response: settled }, options.isCurrent);
    inFlight.delete(identity);
    if (settled.effect.kind === "compensated" || settled.effect.kind === "draft_compensated") {
      await refreshSessionAfterCompensation(options);
    }
    return settled;
  } catch (error) {
    if (durable !== undefined) {
      if (!options.isCurrent()) throw error;
      const current = await getRefusedEditDraft({ baseUrl: options.baseUrl,
        projectId: durable.project_scope.project_id, draftId: durable.source_close.draft_id, fetchImpl: options.fetchImpl });
      const event = await reconcileDraftUndo(options.workspace, current.draft, options.isCurrent);
      if (event !== undefined) { inFlight.delete(identity); await refreshSessionAfterCompensation(options);
        return { effect: { kind: "draft_reconciled", event } }; }
      throw error;
    }
    if (historicalAcknowledgementUnavailable(error)) {
      throw error;
    }
    const settled = await submitUndo(options, frontier, expectedHead, flight);
    inFlight.delete(identity);
    if (settled.effect.kind === "compensated" || settled.effect.kind === "draft_compensated") {
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
  options.workspace.session = canonical;
}

async function submitUndo(
  options: {
    workspace: EditorWorkspace;
    baseUrl: string;
    fetchImpl: typeof fetch;
    cryptoImpl: Crypto;
  },
  frontier: string,
  expectedHead: string,
  flight: InFlightUndo,
  frozen?: UndoLatestAuthorActionRequest,
): Promise<UndoLatestAuthorActionResponse> {
  const request = frozen ?? undoRequest(options.workspace, frontier, expectedHead, flight.correlationId);
  if (flight.nonce === undefined) {
    const challenge = await createProjectCommandChallenge({
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
    });
    flight.nonce = challenge.nonce;
  }
  const nonce = flight.nonce;
  if (nonce === undefined) {
    throw new Error("the Undo Latest Author Action challenge nonce is missing");
  }
  return undoLatestAuthorAction({
    baseUrl: options.baseUrl,
    projectId: options.workspace.partition.project_scope.project_id,
    fetchImpl: options.fetchImpl,
    idempotencyKey: flight.idempotencyKey,
    antiForgery: nonce,
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
