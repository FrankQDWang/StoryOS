import type { AcceptProposalRefusalReason, BlockProposalInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { recoveryStatus, type SessionPosture } from "./proposal-recovery-surface.ts";

/** The Core refusal of the latest Acceptance of a Proposal Revision. `earlier` marks a refusal that the Local Edit Journal kept from an earlier page. */
export type AcceptanceRefusalStatus = { revisionId: string; reason: AcceptProposalRefusalReason; earlier: boolean };

function refusalText(refusal: AcceptanceRefusalStatus): string {
  return refusal.reason === "missing_target" ? "目标段落已不在本章中，接受未生效。拒绝会关闭此候选。"
    : `${refusal.earlier ? "上次" : "此次"}接受已被拒绝，候选文字仍保留。`;
}

export function ProposalDecisionStatus({
  proposalId, proposal, problem, rejectionResult, writerOpen, sessionBlocked,
  pendingRejection, pendingWithdraw, pendingAcceptance, decisionMessage, acceptanceRefusal,
  acceptanceChecked, retryDisabled, onRetryPending, onRetryRejection, onRetryWithdraw,
}: {
  proposalId: string;
  proposal: BlockProposalInspect | undefined;
  problem: { status: number; code: string; message: string } | undefined;
  rejectionResult: "resolved" | "conflicted" | "refused" | undefined;
  writerOpen: boolean;
  sessionBlocked: boolean;
  pendingRejection: boolean;
  pendingWithdraw: boolean;
  pendingAcceptance: boolean;
  decisionMessage: string | undefined;
  /** Only a refusal of the current Proposal Revision. */
  acceptanceRefusal: AcceptanceRefusalStatus | undefined;
  acceptanceChecked: boolean;
  retryDisabled: boolean;
  onRetryPending: () => void;
  onRetryRejection: () => void;
  onRetryWithdraw: () => void;
}) {
  const posture: SessionPosture = !writerOpen ? "closed" : sessionBlocked ? "blocked" : "current";
  const message = pendingRejection
    ? "拒绝结果尚未确认。请重试同一操作。"
    : rejectionResult === "conflicted" ? "正文已变化，拒绝结果请检查。"
      : rejectionResult === "refused" ? "此次拒绝未生效，请检查当前候选文字。"
        : pendingWithdraw
          ? "撤回结果尚未确认。请重试同一操作。"
          : problem !== undefined && problem.code !== "acceptance_session_ineligible"
            ? `Acceptance ${problem.code} (HTTP ${problem.status}): ${problem.message}`
            : pendingAcceptance
              ? proposal?.operation_resolution === "applied"
                ? "正文已变化；此次接受结果尚未确认。请重试同一操作。"
                : decisionMessage ?? "接受结果尚未确认。请重试同一操作。"
              : proposal !== undefined && proposal.operations.every(operation => operation.resolution !== "pending")
                ? !acceptanceChecked ? "正在同步正文。"
                  : decisionMessage?.includes("请刷新") ? decisionMessage : undefined
                : proposal === undefined ? decisionMessage
                  : recoveryStatus(proposal, posture)
                    ?? (acceptanceRefusal === undefined ? undefined : refusalText(acceptanceRefusal));
  if (message === undefined) return null;
  return (
    <p data-proposal-decision={proposalId} role="status">
      {message}
      {pendingAcceptance && problem === undefined ? (
        <button type="button" disabled={retryDisabled} onClick={onRetryPending}>重试接受</button>
      ) : null}
      {pendingRejection ? (
        <button type="button" disabled={retryDisabled} onClick={onRetryRejection}>重试拒绝</button>
      ) : null}
      {pendingWithdraw ? (
        <button type="button" data-proposal-withdraw={proposalId} disabled={retryDisabled}
          onClick={onRetryWithdraw}>重试撤回</button>
      ) : null}
      {decisionMessage === "已复制候选文字。" && message !== "已复制候选文字。"
        ? <span>已复制候选文字。</span> : null}
    </p>
  );
}
