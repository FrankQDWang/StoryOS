import { useEffect, useRef, useState } from "react";
import { StoryOSProtocolError } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { GetAgentRunResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AssistantContext } from "./writing-assistant-panel.tsx";
import { clearRunControl, freezeRunControl, readRunControl, saveRunControl, submitRunControl,
  type RunControlIntent } from "./run-control-intent.ts";
import { historicalAcknowledgementUnavailable, HISTORICAL_ACKNOWLEDGEMENT_MESSAGE }
  from "./historical-acknowledgement.ts";

export function AssistantComposer({ context, available, sending, current, run, onSend, onRefresh, onStatus }: {
  context?: AssistantContext | undefined;
  available: boolean;
  sending: boolean;
  current?: { runId?: string; conversationId?: string } | undefined;
  run?: GetAgentRunResponse | undefined;
  onSend: (message: string, acknowledged: () => void) => void;
  onRefresh: () => Promise<unknown>;
  onStatus: (status: string) => void;
}) {
  const [message, setMessage] = useState("");
  const [pending, setPending] = useState<RunControlIntent | undefined>(
    () => context === undefined ? undefined : readRunControl(context.scope),
  );
  const [busy, setBusy] = useState(false);
  const alive = useRef(true);
  const recovered = useRef(false);
  const known = run !== undefined && run.run_id === current?.runId
    && run.conversation_id === current.conversationId;
  const active = known && run.status !== "completed" && run.status !== "refused" && run.status !== "cancelled";
  const pause = active && run.status !== "paused" && message.trim().length === 0;
  const enabled = available && context?.canSubmit === true && context.chapterId !== undefined
    && !sending && !busy && pending === undefined && (current === undefined || known);

  const submitControl = async (intent: RunControlIntent) => {
    if (context === undefined) return;
    saveRunControl(intent);
    setPending(intent);
    setBusy(true);
    onStatus("正在确认任务操作…");
    try {
      const response = await submitRunControl(context, intent);
      clearRunControl(intent);
      if (!alive.current) return;
      setPending(undefined);
      if (response.effect.kind !== "conflicted" && intent.kind === "guidance") setMessage("");
      await onRefresh();
      if (!alive.current) return;
      onStatus(response.effect.kind === "conflicted" ? "任务已结束，这次操作未执行。"
        : intent.kind === "pause" ? "任务已暂停，候选文字保持原样。" : "要求已收到，将用于当前任务。");
    } catch (error) {
      if (!alive.current) return;
      if (historicalAcknowledgementUnavailable(error)) {
        onStatus(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
      } else if (error instanceof StoryOSProtocolError && error.code === "command_http_error"
        && error.status !== undefined && error.status >= 400 && error.status < 500) {
        clearRunControl(intent);
        setPending(undefined);
        onStatus("这次操作未被接收。请检查当前任务和写作权限。");
      } else {
        onStatus("操作结果待确认。重新打开后将检查同一条操作，不会创建新任务。");
      }
    } finally {
      if (alive.current) setBusy(false);
    }
  };

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);
  useEffect(() => {
    if (context === undefined || recovered.current || !available) return;
    recovered.current = true;
    const retained = readRunControl(context.scope);
    if (retained !== undefined) {
      setPending(retained);
      if (retained.kind === "guidance") setMessage(retained.request.steer_agent_run_input.author_message.text);
      if (context.canSubmit) void submitControl(retained);
      else onStatus("写作权限已变化，上次操作结果仍待确认。");
    }
  }, [context?.scope.owner_user_id, context?.scope.project_id, available]);

  return <div className="composer-dock">
    <form className="composer" data-writing-assistant-composer="" onSubmit={(event) => {
      event.preventDefault();
      if (!enabled) return;
      if (active && context !== undefined) {
        if (pause || message.trim().length > 0) void submitControl(freezeRunControl(context, run,
          pause ? { kind: "pause" } : { kind: "guidance", text: message.trim() }));
      } else if (message.trim().length > 0) onSend(message.trim(), () => setMessage(""));
    }}>
      <textarea name="assistant-message" aria-label="给写作助手的消息" rows={2} maxLength={4000}
        value={message} onChange={(event) => setMessage(event.currentTarget.value)}
        placeholder="描述想修改的当前章节文字" />
      <div className="composer-controls">
        <button className="send-button" type="submit" aria-label={pause ? "暂停" : "发送"}
          disabled={!enabled || (!pause && message.trim().length === 0)}>{pause ? "■" : "↑"}</button>
      </div>
    </form>
  </div>;
}
