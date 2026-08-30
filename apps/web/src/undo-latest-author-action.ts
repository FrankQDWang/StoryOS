import {
  createProjectCommandChallenge,
  digestUndoLatestAuthorAction,
  getEditorSession,
  undoLatestAuthorAction,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  EditorBaseSnapshot,
  UndoLatestAuthorActionResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import type { EditorWorkspace } from "./editor-types.ts";

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
  workspace: EditorWorkspace;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}): Promise<UndoLatestAuthorActionResponse | undefined> {
  const frontier = options.workspace.session.author_undo_frontier_sequence;
  const expectedHead = options.workspace.session.base_snapshot.authoritative_head_revision_id;
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
  try {
    const settled = await submitUndo(options, frontier, expectedHead, flight);
    inFlight.delete(identity);
    if (settled.effect.kind === "compensated") {
      await refreshSessionAfterCompensation(options);
    }
    return settled;
  } catch {
    const settled = await submitUndo(options, frontier, expectedHead, flight);
    inFlight.delete(identity);
    if (settled.effect.kind === "compensated") {
      await refreshSessionAfterCompensation(options);
    }
    return settled;
  }
}

async function refreshSessionAfterCompensation(options: {
  workspace: EditorWorkspace;
  baseUrl: string;
  fetchImpl: typeof fetch;
}): Promise<void> {
  const canonical = await getEditorSession({
    baseUrl: options.baseUrl,
    projectId: options.workspace.partition.project_scope.project_id,
    editorSessionId: options.workspace.partition.editor_session_id,
    fetchImpl: options.fetchImpl,
  });
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
): Promise<UndoLatestAuthorActionResponse> {
  const request = {
    command_schema: "storyos.command.undo-latest-author-action.request.v1",
    undo_latest_author_action_input: {
      expected_author_undo_frontier_sequence: frontier,
      expected_authoritative_revision_id: expectedHead,
      editor_session_id: options.workspace.partition.editor_session_id,
      client_contract_revision:
        RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: SECURITY_POLICY_REVISION,
      correlation_id: flight.correlationId,
    },
  };
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
