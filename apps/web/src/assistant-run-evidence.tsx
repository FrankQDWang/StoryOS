import type {
  ContextSourceClass, GetAgentRunResponse, ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export type AssistantRunSelection = Readonly<{
  projectScope: ProjectScope;
  runId: string;
  conversationId: string;
}>;

const sourceLabels: Record<ContextSourceClass, string> = {
  author_instruction: "你的要求",
  working_target: "当时的章节片段",
  instruction_binding: "写作约束",
  host_control: "宿主管理信息",
};

const recoveryReasons: Readonly<Record<string, string>> = {
  prior_submission_unsettled: "先前提交的请求尚未确认",
  fenced_predecessor: "先前请求已停止处理",
  authority_missing: "当前没有继续处理的权限",
  processing_boundary_changed: "处理请求的条件已改变",
  required_input_missing: "需要的内容无法读取",
  budget_insufficient: "可用额度不足",
  changed_input: "章节或请求内容已改变",
  restricted_source: "先前内容不能用于这次请求",
  exact_required_unsatisfied: "必须保留的原文不能完整使用",
};

function recoveryReason(reason: string | null | undefined): string {
  return recoveryReasons[reason ?? ""] ?? "暂时无法确认具体原因";
}

export function AssistantRunEvidence({ run, selection }: Readonly<{
  run: GetAgentRunResponse;
  selection: AssistantRunSelection;
}>) {
  if (run.run_id !== selection.runId || run.conversation_id !== selection.conversationId
    || run.project_scope.owner_user_id !== selection.projectScope.owner_user_id
    || run.project_scope.project_id !== selection.projectScope.project_id) return null;
  const context = run.context;
  const attempt = run.model_attempt;
  const evidence = attempt.kind === "present"
    ? run.evidence.filter((item) => item.attempt_id === attempt.model_attempt_id) : [];
  const compaction = run.active_compaction;
  const reference = run.reference_recovery;
  return (
    <section data-run-evidence="" data-run-evidence-run-id={run.run_id}
      data-run-evidence-attempt-id={attempt.kind === "present" ? attempt.model_attempt_id : ""}
      aria-label="本次请求的输入与恢复说明">
      <strong>输入范围</strong>
      <p>这里保留请求当时准备的章节片段和你的要求。超出可用范围的内容不会发送。</p>
      {context.sufficiency.kind === "complete" ? <p>这次需要的内容已完整准备。</p> : (
        <p>这次需要的内容无法完整准备，因此没有开始生成。</p>
      )}
      {context.selected.map((input) => input.source_class === "host_control" ? null : (
        <div key={`${input.source_class}:${input.source_version}`}>
          <p>{sourceLabels[input.source_class]}</p>
          <p style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{input.content || "（空内容）"}</p>
        </div>
      ))}
      {context.rejected.map((input) => (
        <p key={`${input.source_class}:${input.source_version}`}>{sourceLabels[input.source_class]}没有加入：
          {input.reason.kind === "over_item_token_limit" ? "内容过长，无法完整使用。" : "无法读取当时需要的版本。"}</p>
      ))}
      <p>{context.current_availability.working_target.kind === "current" ? "当时的章节版本仍是当前版本。"
        : context.current_availability.working_target.kind === "superseded" ? "章节已改变；这里保留请求当时的片段。"
          : "当前章节不可用；这不表示历史输入已过期。"}</p>
      <p>{context.destination_io.kind === "host_fake" ? "这次在本地处理，没有向外部服务发送这些内容。" : "这次没有开始生成。"}</p>
      {attempt.kind === "absent" ? <p>这次还没有开始生成的记录。</p> : (
        <>
          <p>{attempt.dispatch_state === "settled" ? "最初那次生成的结果已确认。"
            : attempt.dispatch_state === "uncertain" ? "最初那次生成的结果仍无法确认。" : "最初那次生成尚未确认完成。"}</p>
          {evidence.some((item) => item.kind === "sent_content" && item.availability === "current")
            ? <p>已保留最初发送内容的记录。</p> : <p>无法查看最初发送内容的记录。</p>}
          {evidence.some((item) => item.kind === "stored_reference")
            ? <p>保留了先前内容的引用，但不保证还能读取其中的内容。</p> : null}
          {evidence.some((item) => item.kind === "provider_opaque")
            ? <p>模型还记住了哪些内容，我们无法确认。</p> : null}
        </>
      )}
      {compaction.kind === "present" ? (
        <div data-run-compaction={compaction.install_state}>
          <strong>内容摘要</strong>
          <p>{compaction.install_state === "installed" ? "后面的生成已使用这份摘要。"
            : compaction.install_state === "staged" ? "摘要已准备好，尚未使用。" : "这份摘要没有采用。"}</p>
          <p style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{compaction.output_text}</p>
          {compaction.loss_facts.includes("semantic_preservation_unknown")
            ? <p>无法确认摘要是否保留了原文的全部意思。</p> : <p>摘要已经使用，也不代表原文的意思全部保留。</p>}
          {compaction.loss_facts.some((fact) => fact !== "semantic_preservation_unknown")
            ? <p>其他内容是否保留，目前也无法确认。</p> : null}
          {compaction.install_state === "refused" ? <p>未采用原因：{recoveryReason(compaction.refusal_reason)}。</p> : null}
        </div>
      ) : null}
      {reference.kind === "present" ? (
        <div data-run-reference-recovery={reference.disposition}>
          <strong>先前内容恢复</strong>
          <p>{reference.disposition === "unknown_create" ? "最初那次生成的结果还不确定，不能认定先前内容已经过期。" : "先前内容的引用已过期或无法使用。"}</p>
          <p>{reference.disposition === "rebuilt" ? "已用还能读取的内容重新准备这次请求。"
            : reference.disposition === "blocked" ? `无法找回先前内容：${recoveryReason(reference.block_reason)}。`
              : "最初那次生成的结果仍无法确认，不能据此找回先前内容。"}</p>
          {!reference.lossless_provider_reconstruction ? <p>不能保证模型记住的内容被完整找回。</p> : null}
          {!reference.semantic_erasure ? <p>引用失效不代表模型已经忘记先前内容。</p> : null}
          <p>{reference.opaque_reused ? "这次复用了模型内部保留的内容。" : "这次没有复用无法确认的模型内部内容。"}</p>
          {reference.covered_content_included ? <p>还能读取的先前内容已加入这次请求。</p>
            : <p>这次没有加入先前引用所指的内容。</p>}
        </div>
      ) : null}
    </section>
  );
}
