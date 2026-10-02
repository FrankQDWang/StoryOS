import { AssistantRunEvidence } from "./assistant-run-evidence";

import type { GetAgentRunResponse, ProjectScope }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export type RunDetailsSelection = {
  projectScope: ProjectScope;
  runId: string;
  conversationId: string;
};

export type SelectedRunDetails =
  | { kind: "loading" | "unavailable" }
  | { kind: "known"; selection: RunDetailsSelection; run: GetAgentRunResponse };

export function AssistantRunDetails({ details }: { details: SelectedRunDetails }) {
  if (details.kind !== "known") {
    return <div className="assistant-run-details">
      <div data-assistant-memory-settings="">{details.kind === "loading" ? "正在读取本次请求记录的记忆设置。"
        : "无法确认本次请求记录的记忆设置。"}</div>
      <p data-run-evidence-status={details.kind}>{details.kind === "loading"
        ? "正在读取本次请求的输入与恢复记录。" : "本次输入与恢复记录不可用，无法确认其内容。"}</p>
    </div>;
  }
  const { run, selection } = details;
  const settings = run.captured_memory_settings;
  const exact = run.project_scope.owner_user_id === selection.projectScope.owner_user_id
    && run.project_scope.project_id === selection.projectScope.project_id
    && run.run_id === selection.runId && run.conversation_id === selection.conversationId;
  return <div className="assistant-run-details">
    <div data-assistant-memory-settings="" data-assistant-memory-run-id={selection.runId}
      style={{ display: "grid", gap: 4 }}>
    {!exact || settings.kind === "unavailable"
      || settings.memory_settings_revision !== run.memory_settings_revision
      ? "无法确认本次请求记录的记忆设置。"
      : <><span>本次请求记录的记忆设置</span>
        <span>使用记忆：{settings.use_enabled ? "开启" : "关闭"}</span>
        <span>参与后续记忆整理：{settings.contribution_enabled ? "开启" : "关闭"}</span></>}
    </div>
    <AssistantRunEvidence run={run} selection={selection} />
  </div>;
}
