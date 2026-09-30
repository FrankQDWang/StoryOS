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

const compactionRefusals: Readonly<Record<string, string>> = {
  changed_input: "输入已改变",
  restricted_source: "原内容不能用于这次请求",
  exact_required_unsatisfied: "必须完整保留的输入无法满足",
};

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
  return (
    <section data-run-evidence="" data-run-evidence-run-id={run.run_id}
      data-run-evidence-attempt-id={attempt.kind === "present" ? attempt.model_attempt_id : ""}
      aria-label="本次请求的输入与恢复说明">
      <strong>输入范围</strong>
      <p>本次围绕所选章节，以下保留当时已纳入的输入。单项输入计数上限：{context.token_counting_profile.item_token_limit}。</p>
      {context.sufficiency.kind === "complete" ? <p>必需输入已完整纳入。</p> : (
        <p>必需输入未能完整纳入，本次没有据此生成结果。</p>
      )}
      {context.selected.map((input) => input.source_class === "host_control" ? null : (
        <div key={`${input.source_class}:${input.source_version}`}>
          <p>{sourceLabels[input.source_class]}</p>
          <p style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{input.content || "（空内容）"}</p>
        </div>
      ))}
      {context.rejected.map((input) => (
        <p key={`${input.source_class}:${input.source_version}`}>{sourceLabels[input.source_class]}未纳入：
          {input.reason.kind === "over_item_token_limit" ? "超过单项输入限额。" : "所需版本不可用。"}</p>
      ))}
      <p>{context.current_availability.working_target.kind === "current" ? "当时的章节版本仍是当前版本。"
        : context.current_availability.working_target.kind === "superseded" ? "章节已改变；这里保留请求当时的片段。"
          : "当前章节不可用；这不表示历史输入已过期。"}</p>
      <p>{context.destination_io.kind === "host_fake" ? "本次由本地受控模型处理，没有向远程模型发送。" : "本次未发送模型请求。"}</p>
      <p>{context.host_control.destination_visible ? "处理目标可见宿主管理信息。" : "宿主管理信息与模型输入分开保留。"}</p>
      {attempt.kind === "absent" ? <p>本次没有已记录的模型尝试。</p> : (
        <>
          <p>{attempt.dispatch_state === "settled" ? "原模型尝试的结果已确认。"
            : attempt.dispatch_state === "uncertain" ? "原模型尝试的结果仍未知。" : "原模型尝试尚未确认完成。"}</p>
          {evidence.some((item) => item.kind === "sent_content" && item.availability === "current")
            ? <p>已保留原尝试发送内容的证据。</p> : <p>原尝试发送内容的证据不可用。</p>}
          {evidence.some((item) => item.kind === "stored_reference")
            ? <p>已保留引用；引用本身不能证明远程内容仍可读取。</p> : null}
          {evidence.some((item) => item.kind === "provider_opaque")
            ? <p>提供方内部内容仍未知，不能当作已知输入。</p> : null}
        </>
      )}
      {compaction.kind === "present" ? (
        <div data-run-compaction={compaction.install_state}>
          <strong>输入摘要</strong>
          <p>{compaction.install_state === "installed" ? "摘要已用于后续尝试。"
            : compaction.install_state === "staged" ? "摘要已准备，尚未用于后续尝试。" : "摘要未采用。"}</p>
          <p>摘要依据 {compaction.known_inputs.length} 项已知输入；{compaction.mapping_kind === "host_managed" ? "由宿主管理摘要。" : "使用模型原生摘要。"}</p>
          <p style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{compaction.output_text}</p>
          {compaction.loss_facts.includes("semantic_preservation_unknown")
            ? <p>摘要是否完整保留原文语义仍未知。</p> : <p>不能仅凭摘要已采用就证明内容无损。</p>}
          {compaction.loss_facts.some((fact) => fact !== "semantic_preservation_unknown")
            ? <p>另有内容保留情况尚无法解释。</p> : null}
          {compaction.install_state === "refused" ? <p>未采用原因：{compactionRefusals[compaction.refusal_reason ?? ""] ?? "具体原因尚无法解释"}。</p> : null}
        </div>
      ) : null}
    </section>
  );
}
