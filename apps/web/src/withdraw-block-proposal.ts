import { createProjectCommandChallenge, digestWithdrawProposal, withdrawProposal }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DigestValue, ProjectScope, WithdrawProposalRequest, WithdrawProposalResponse }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE }
  from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { uuidV7 } from "./acceptance-journal.ts";
import type { EditorReadyState, EditorWorkspace } from "./editor-types.ts";

const SCHEMA = "storyos.local-edit-journal.withdraw-proposal.v1";
const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

type WithdrawRecord = {
  key: string;
  schema_id: typeof SCHEMA;
  project_scope: ProjectScope;
  proposal_id: string;
  revision_id: string;
  journal_partition_id: string;
  writer_generation: string;
  idempotency_key: string;
  nonce?: string;
  request: WithdrawProposalRequest;
  digest: DigestValue;
  settlement: "unresolved" | "settled";
  response?: WithdrawProposalResponse;
};

export type WithdrawTarget = {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  workspace: EditorReadyState;
  proposalId: string;
  proposalRevisionId: string;
  targetRevisionId: string;
};

function withdrawKey(partitionId: string, proposalId: string, revisionId: string): string {
  return `withdraw:${partitionId}:${proposalId}:${revisionId}`;
}

function readRequest<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Withdrawal record read failed"));
  });
}

function committed(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Withdrawal record write failed"));
    transaction.onerror = () => reject(transaction.error ?? new Error("Withdrawal record write failed"));
  });
}

async function putRecord(database: IDBDatabase, record: WithdrawRecord): Promise<void> {
  const transaction = database.transaction("metadata", "readwrite", { durability: "strict" });
  transaction.objectStore("metadata").put(record);
  await committed(transaction);
}

async function readRecord(database: IDBDatabase, key: string): Promise<WithdrawRecord | undefined> {
  const row = await readRequest(database.transaction("metadata", "readonly")
    .objectStore("metadata").get(key));
  const record = row as WithdrawRecord | undefined;
  return record?.schema_id === SCHEMA ? record : undefined;
}

export async function pendingWithdrawIds(workspace: EditorWorkspace): Promise<string[]> {
  const prefix = `withdraw:${workspace.partition.journal_partition_id}:`;
  const rows = await readRequest(workspace.database.transaction("metadata", "readonly")
    .objectStore("metadata").getAll(IDBKeyRange.bound(prefix, `${prefix}\uffff`)));
  return (rows as WithdrawRecord[]).filter((row) => row.schema_id === SCHEMA
    && row.settlement === "unresolved").map((row) => row.proposal_id);
}

function responseMatches(record: WithdrawRecord, response: WithdrawProposalResponse): boolean {
  return response.schema_id === "storyos.command.withdraw-proposal.response.v1"
    && response.correlation_id === record.request.withdraw_proposal_input.correlation_id
    && response.project_scope.owner_user_id === record.project_scope.owner_user_id
    && response.project_scope.project_id === record.project_scope.project_id
    && response.receipt.proposal_id === record.proposal_id
    && response.receipt.idempotency_key === record.idempotency_key
    && response.receipt.result === response.effect.kind
    && response.receipt.proposal_revision_id === record.revision_id
    && JSON.stringify(response.receipt.command_digest) === JSON.stringify(record.digest);
}

async function sendWithdraw(options: Pick<WithdrawTarget, "baseUrl" | "fetchImpl" | "cryptoImpl"
  | "workspace">, record: WithdrawRecord): Promise<WithdrawProposalResponse> {
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
        route_template: "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
        command_schema: current.request.command_schema,
        canonical_command_digest: current.digest,
        idempotency_key: current.idempotency_key,
      },
    });
    current = { ...current, nonce: challenge.nonce };
    await putRecord(options.workspace.database, current);
  }
  try {
    const response = await withdrawProposal({
      baseUrl: options.baseUrl,
      projectId: current.project_scope.project_id,
      proposalId: current.proposal_id,
      fetchImpl: options.fetchImpl,
      idempotencyKey: current.idempotency_key,
      antiForgery: current.nonce ?? "",
      request: current.request,
    });
    if (!responseMatches(current, response)) {
      throw new Error("Withdrawal acknowledgement does not match the frozen command");
    }
    await putRecord(options.workspace.database, { ...current, settlement: "settled", response });
    return response;
  } catch (error) {
    await putRecord(options.workspace.database, { ...current, settlement: "unresolved" });
    throw error;
  }
}

export async function withdrawDisplayedBlockProposal(options: WithdrawTarget): Promise<WithdrawProposalResponse> {
  if (globalThis.navigator?.locks === undefined) {
    throw new Error("Protected Withdrawal transport lock is unavailable");
  }
  const workspace = options.workspace;
  return navigator.locks.request(
    `storyos-decision:${workspace.partition.journal_partition_id}:${options.proposalId}`,
    async () => {
      if (workspace.partition.disposition !== "current_writer_open"
        || workspace.session.writer.kind !== "current_writer") {
        throw new Error("请先恢复写作会话。");
      }
      const key = withdrawKey(workspace.partition.journal_partition_id, options.proposalId,
        options.proposalRevisionId);
      const existing = await readRecord(workspace.database, key);
      if (existing !== undefined) return sendWithdraw(options, existing);
      const request: WithdrawProposalRequest = {
        command_schema: "storyos.command.withdraw-proposal.request.v1",
        withdraw_proposal_input: {
          cause: "author",
          proposal_revision_id: options.proposalRevisionId,
          expected_target_revisions: [options.targetRevisionId],
          withdrawal_reason: { kind: "author_withdrew", note: { kind: "omitted" } },
          editor_session_id: workspace.partition.editor_session_id,
          client_contract_revision:
            RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
          security_policy_revision: SECURITY_POLICY_REVISION,
          correlation_id: uuidV7(options.cryptoImpl),
        },
      };
      const digest = await digestWithdrawProposal(request, options.cryptoImpl);
      const record: WithdrawRecord = {
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
      return sendWithdraw(options, record);
    });
}

export async function retryPendingDisplayedWithdraw(options: Pick<WithdrawTarget, "baseUrl"
  | "fetchImpl" | "cryptoImpl" | "workspace" | "proposalId">): Promise<WithdrawProposalResponse> {
  if (globalThis.navigator?.locks === undefined) {
    throw new Error("Protected Withdrawal transport lock is unavailable");
  }
  const workspace = options.workspace;
  return navigator.locks.request(
    `storyos-decision:${workspace.partition.journal_partition_id}:${options.proposalId}`,
    async () => {
      const prefix = `withdraw:${workspace.partition.journal_partition_id}:${options.proposalId}:`;
      const rows = await readRequest(workspace.database.transaction("metadata", "readonly")
        .objectStore("metadata").getAll(IDBKeyRange.bound(prefix, `${prefix}\uffff`)));
      const pending = (rows as WithdrawRecord[]).filter((row) => row.schema_id === SCHEMA
        && row.settlement === "unresolved");
      const record = pending.length === 1 ? pending[0] : undefined;
      if (record === undefined) throw new Error("Withdrawal recovery is not available");
      if (workspace.partition.disposition !== "current_writer_open"
        || workspace.session.writer.kind !== "current_writer"
        || record.writer_generation !== workspace.partition.writer_generation
        || record.journal_partition_id !== workspace.partition.journal_partition_id) {
        throw new Error("请先恢复写作会话。");
      }
      return sendWithdraw(options, record);
    });
}

type DisplayedWithdrawTarget = {
  proposalId: string;
  operationId: string;
  revisionId: string;
  text: string;
};

type DisplayedWithdrawControl = {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  workspace: EditorReadyState | undefined;
  authoritativeRevisionId: string;
  markBusy: (proposalId: string) => void;
  markIdle: () => void;
  report: (proposalId: string, message: string) => void;
  refresh: () => void;
  onAccepted: () => Promise<void>;
};

function withdrawFailureMessage(error: unknown): string {
  return error instanceof Error && error.message.startsWith("请先")
    ? error.message : "撤回结果尚未确认。请重试同一操作。";
}

function withdrawEffectMessage(kind: WithdrawProposalResponse["effect"]["kind"]): string {
  if (kind === "resolved") return "已撤回，正文保持不变。";
  if (kind === "conflicted") return "正文已变化，撤回未完成。";
  if (kind === "no_effect") return "此次撤回没有改变候选文字。";
  if (kind === "refused") return "撤回未生效，请检查当前候选文字。";
  return "撤回结果尚未确认。请重试同一操作。";
}

export function authorWithdrawControlReady(input: {
  controlsReady: boolean;
  condition: string;
  closure: string;
  resolution: string;
  pendingAcceptance: boolean;
  pendingRejection: boolean;
  pendingReplan: boolean;
  pendingWithdraw: boolean;
}): boolean {
  return input.controlsReady
    && input.condition === "proposal_recovery_conflict"
    && input.closure === "open"
    && input.resolution === "pending"
    && !input.pendingAcceptance
    && !input.pendingRejection
    && !input.pendingReplan
    && !input.pendingWithdraw;
}

export function dispatchDisplayedWithdraw(input: DisplayedWithdrawControl & {
  target: DisplayedWithdrawTarget;
  pending: boolean;
  busy: boolean;
  displayedRevisionId: string | undefined;
  eligible: boolean;
  writerReady: boolean;
}): void {
  if (input.busy) return;
  if (input.pending) {
    dispatchDisplayedWithdrawRetry({ ...input, proposalId: input.target.proposalId });
    return;
  }
  const workspace = input.workspace;
  if (input.displayedRevisionId === undefined || workspace === undefined || !input.eligible
    || !input.writerReady || input.displayedRevisionId !== input.target.revisionId) {
    input.report(input.target.proposalId, "候选文字已变化。请检查当前版本。");
    input.refresh();
    return;
  }
  input.markBusy(input.target.proposalId);
  void (async () => {
    try {
      const response = await withdrawDisplayedBlockProposal({
        baseUrl: input.baseUrl, fetchImpl: input.fetchImpl, cryptoImpl: input.cryptoImpl,
        workspace, proposalId: input.target.proposalId,
        proposalRevisionId: input.target.revisionId,
        targetRevisionId: input.authoritativeRevisionId,
      });
      if (response.receipt.proposal_id !== input.target.proposalId) {
        throw new Error("撤回结果的身份不匹配。");
      }
      input.report(input.target.proposalId, withdrawEffectMessage(response.effect.kind));
      input.refresh();
      await input.onAccepted();
    } catch (error) {
      input.report(input.target.proposalId, withdrawFailureMessage(error));
      input.refresh();
    } finally {
      input.markIdle();
    }
  })();
}

export function dispatchDisplayedWithdrawRetry(input: DisplayedWithdrawControl & {
  proposalId: string;
  busy: boolean;
}): void {
  const workspace = input.workspace;
  if (input.busy || workspace === undefined) return;
  input.markBusy(input.proposalId);
  void (async () => {
    try {
      const response = await retryPendingDisplayedWithdraw({
        baseUrl: input.baseUrl, fetchImpl: input.fetchImpl, cryptoImpl: input.cryptoImpl,
        workspace, proposalId: input.proposalId,
      });
      input.report(input.proposalId, response.effect.kind === "resolved"
        ? "已撤回，正文保持不变。"
        : "撤回结果尚未确认。请重试同一操作。");
      input.refresh();
      await input.onAccepted();
    } catch (error) {
      input.report(input.proposalId, withdrawFailureMessage(error));
      input.refresh();
    } finally {
      input.markIdle();
    }
  })();
}
