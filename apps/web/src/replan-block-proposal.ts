import { createProjectCommandChallenge, digestReplanProposal, replanProposal }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DigestValue, ProjectScope, ReplanProposalRequest, ReplanProposalResponse,
  ReplanSourceCondition } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE }
  from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { uuidV7 } from "./acceptance-journal.ts";
import type { EditorReadyState, EditorWorkspace } from "./editor-types.ts";

const SCHEMA = "storyos.local-edit-journal.replan-proposal.v1";
const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

type ReplanRecord = {
  key: string;
  schema_id: typeof SCHEMA;
  project_scope: ProjectScope;
  proposal_id: string;
  revision_id: string;
  journal_partition_id: string;
  writer_generation: string;
  idempotency_key: string;
  nonce?: string;
  request: ReplanProposalRequest;
  digest: DigestValue;
  settlement: "unresolved" | "settled";
  response?: ReplanProposalResponse;
};

export type ReplanTarget = {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  workspace: EditorReadyState;
  proposalId: string;
  operationId: string;
  proposalRevisionId: string;
  targetRevisionId: string;
  sourceCondition: ReplanSourceCondition;
};

function replanKey(partitionId: string, proposalId: string, revisionId: string): string {
  return `replan:${partitionId}:${proposalId}:${revisionId}`;
}

function readRequest<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Replan record read failed"));
  });
}

function committed(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Replan record write failed"));
    transaction.onerror = () => reject(transaction.error ?? new Error("Replan record write failed"));
  });
}

async function putRecord(database: IDBDatabase, record: ReplanRecord): Promise<void> {
  const transaction = database.transaction("metadata", "readwrite", { durability: "strict" });
  transaction.objectStore("metadata").put(record);
  await committed(transaction);
}

async function readRecord(database: IDBDatabase, key: string): Promise<ReplanRecord | undefined> {
  const row = await readRequest(database.transaction("metadata", "readonly")
    .objectStore("metadata").get(key));
  const record = row as ReplanRecord | undefined;
  return record?.schema_id === SCHEMA ? record : undefined;
}

export async function pendingReplanIds(workspace: EditorWorkspace): Promise<string[]> {
  const prefix = `replan:${workspace.partition.journal_partition_id}:`;
  const rows = await readRequest(workspace.database.transaction("metadata", "readonly")
    .objectStore("metadata").getAll(IDBKeyRange.bound(prefix, `${prefix}\uffff`)));
  return (rows as ReplanRecord[]).filter((row) => row.schema_id === SCHEMA
    && row.settlement === "unresolved").map((row) => row.proposal_id);
}

function responseMatches(record: ReplanRecord, response: ReplanProposalResponse): boolean {
  return response.schema_id === "storyos.command.replan-proposal.response.v1"
    && response.correlation_id === record.request.replan_proposal_input.correlation_id
    && response.project_scope.owner_user_id === record.project_scope.owner_user_id
    && response.project_scope.project_id === record.project_scope.project_id
    && response.receipt.proposal_id === record.proposal_id
    && response.receipt.idempotency_key === record.idempotency_key
    && response.receipt.result === response.effect.kind
    && response.receipt.source_proposal_revision_id === record.revision_id
    && JSON.stringify(response.receipt.command_digest) === JSON.stringify(record.digest);
}

async function sendReplan(options: Pick<ReplanTarget, "baseUrl" | "fetchImpl" | "cryptoImpl"
  | "workspace">, record: ReplanRecord): Promise<ReplanProposalResponse> {
  if (record.settlement === "settled" && record.response !== undefined
    && responseMatches(record, record.response)) return record.response;
  let current = record;
  if (current.nonce === undefined) {
    const challenge = await createProjectCommandChallenge({
      baseUrl: options.baseUrl,
      projectId: current.project_scope.project_id,
      fetchImpl: options.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans",
        command_schema: current.request.command_schema,
        canonical_command_digest: current.digest,
        idempotency_key: current.idempotency_key,
      },
    });
    current = { ...current, nonce: challenge.nonce };
    await putRecord(options.workspace.database, current);
  }
  try {
    const response = await replanProposal({
      baseUrl: options.baseUrl,
      projectId: current.project_scope.project_id,
      proposalId: current.proposal_id,
      fetchImpl: options.fetchImpl,
      idempotencyKey: current.idempotency_key,
      antiForgery: current.nonce ?? "",
      request: current.request,
    });
    if (!responseMatches(current, response)) {
      throw new Error("Replan acknowledgement does not match the frozen command");
    }
    await putRecord(options.workspace.database, { ...current, settlement: "settled", response });
    return response;
  } catch (error) {
    await putRecord(options.workspace.database, { ...current, settlement: "unresolved" });
    throw error;
  }
}

export async function replanDisplayedBlockProposal(options: ReplanTarget): Promise<ReplanProposalResponse> {
  if (globalThis.navigator?.locks === undefined) {
    throw new Error("Protected Replan transport lock is unavailable");
  }
  const workspace = options.workspace;
  return navigator.locks.request(
    `storyos-decision:${workspace.partition.journal_partition_id}:${options.proposalId}`,
    async () => {
      if (workspace.partition.disposition !== "current_writer_open"
        || workspace.session.writer.kind !== "current_writer") {
        throw new Error("请先恢复写作会话。");
      }
      const key = replanKey(workspace.partition.journal_partition_id, options.proposalId,
        options.proposalRevisionId);
      const existing = await readRecord(workspace.database, key);
      if (existing !== undefined) return sendReplan(options, existing);
      const request: ReplanProposalRequest = {
        command_schema: "storyos.command.replan-proposal.request.v1",
        replan_proposal_input: {
          conflicted_proposal_revision_id: options.proposalRevisionId,
          expected_current_proposal_head: options.proposalRevisionId,
          expected_current_target_revisions: [options.targetRevisionId],
          replacement_operations: [options.operationId],
          source_condition: options.sourceCondition,
          editor_session_id: workspace.partition.editor_session_id,
          client_contract_revision:
            RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
          security_policy_revision: SECURITY_POLICY_REVISION,
          correlation_id: uuidV7(options.cryptoImpl),
        },
      };
      const digest = await digestReplanProposal(request, options.cryptoImpl);
      const record: ReplanRecord = {
        key,
        schema_id: SCHEMA,
        project_scope: workspace.partition.project_scope,
        proposal_id: options.proposalId,
        revision_id: options.proposalRevisionId,
        journal_partition_id: workspace.partition.journal_partition_id,
        writer_generation: workspace.partition.writer_generation,
        idempotency_key: uuidV7(options.cryptoImpl),
        request,
        digest,
        settlement: "unresolved",
      };
      await putRecord(workspace.database, record);
      return sendReplan(options, record);
    });
}

export async function retryPendingDisplayedReplan(options: Pick<ReplanTarget, "baseUrl"
  | "fetchImpl" | "cryptoImpl" | "workspace" | "proposalId">): Promise<ReplanProposalResponse> {
  if (globalThis.navigator?.locks === undefined) {
    throw new Error("Protected Replan transport lock is unavailable");
  }
  const workspace = options.workspace;
  return navigator.locks.request(
    `storyos-decision:${workspace.partition.journal_partition_id}:${options.proposalId}`,
    async () => {
      const prefix = `replan:${workspace.partition.journal_partition_id}:${options.proposalId}:`;
      const rows = await readRequest(workspace.database.transaction("metadata", "readonly")
        .objectStore("metadata").getAll(IDBKeyRange.bound(prefix, `${prefix}\uffff`)));
      const pending = (rows as ReplanRecord[]).filter((row) => row.schema_id === SCHEMA
        && row.settlement === "unresolved");
      const record = pending.length === 1 ? pending[0] : undefined;
      if (record === undefined) throw new Error("Replan recovery is not available");
      if (workspace.partition.disposition !== "current_writer_open"
        || workspace.session.writer.kind !== "current_writer"
        || record.writer_generation !== workspace.partition.writer_generation
        || record.journal_partition_id !== workspace.partition.journal_partition_id) {
        throw new Error("请先恢复写作会话。");
      }
      return sendReplan(options, record);
    });
}
