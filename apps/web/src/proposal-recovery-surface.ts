import type { BlockProposalInspect } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export type ProposalConditionKind = "absent" | "proposal_conflict" | "proposal_recovery_conflict";
export type SessionPosture = "current" | "blocked" | "closed";

const SESSION_NOTE = "当前编辑会话不能写入，系统不会自动提交。";

export function proposalConditionKind(proposal: BlockProposalInspect): ProposalConditionKind {
  const condition = proposal.source_condition;
  return condition.kind === "proposal_conflict" || condition.kind === "proposal_recovery_conflict"
    ? condition.kind : "absent";
}

export function recoveryStatus(proposal: BlockProposalInspect,
  posture: SessionPosture): string | undefined {
  const condition = proposalConditionKind(proposal);
  const refusal = proposal.latest_acceptance_refusal;
  const refused = posture === "blocked" || (refusal.kind === "present"
    && (refusal.reason === "stale_writer" || refusal.reason === "session_changed"));
  const sessionNote = posture === "current" ? "" : SESSION_NOTE;
  if (condition === "proposal_conflict") return `正文已变化，候选文字尚未接受。${sessionNote}`;
  if (condition === "proposal_recovery_conflict") {
    return `候选文字的恢复材料冲突，正文保持不变。${sessionNote}`;
  }
  if (proposal.validation === "invalid") return "候选文字的验证已失效，请检查当前结果。";
  if (proposal.generation === "ready" && proposal.validation === "pending") {
    return "候选文字正在等待当前版本验证，尚不能接受。";
  }
  if (refused && posture === "current") {
    return "上次接受因编辑会话失效而被拒绝，候选文字仍保留。请重新点击接受。";
  }
  if (refused) {
    return "上次接受因编辑会话失效而被拒绝，候选文字仍保留。请恢复写作会话后再明确接受。系统不会自动提交。";
  }
  if (posture !== "current") {
    return "当前编辑会话不能写入。候选文字仍然有效。请接管写作后再明确操作。系统不会自动提交。";
  }
  if (proposal.latest_acceptance_refusal.kind === "present"
    && proposal.latest_acceptance_refusal.reason === "invalid_challenge") {
    return "上次接受请求已被拒绝，候选文字仍保留。";
  }
  return undefined;
}
