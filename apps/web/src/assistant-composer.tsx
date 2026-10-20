import { useEffect, useRef, useState } from "react";
import { StoryOSProtocolError } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { GetAgentRunResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AssistantContext, SentReferences } from "./writing-assistant-panel.tsx";
import { settledRun } from "./assistant-run-outcome.ts";
import { clearResendRunControl, clearRunControl, freezeRunControl, readRunControl, saveRunControl,
  submitRunControl, type RunControlIntent } from "./run-control-intent.ts";
import { historicalAcknowledgementUnavailable, HISTORICAL_ACKNOWLEDGEMENT_MESSAGE }
  from "./historical-acknowledgement.ts";
import {
  clearDraftReferences, draftLabels, pasteReferences, REFERENCE_CLIPBOARD_TYPE, removeDraftReference,
  useReferenceDraft,
} from "./selection-reference-draft.ts";
import type { SelectionReference } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export function AssistantComposer({ context, available, sending, current, run, onSend, onRefresh, onStatus,
  onGuidanceRetained }: {
  context?: AssistantContext | undefined;
  available: boolean;
  sending: boolean;
  current?: { runId?: string; conversationId?: string } | undefined;
  run?: GetAgentRunResponse | undefined;
  onSend: (message: string, acknowledged: () => void, frozen?: SentReferences, recorded?: () => void) => void;
  onRefresh: () => Promise<unknown>;
  onStatus: (status: string) => void;
  /** Keeps the labels and the references of a retained steering input for the history. */
  onGuidanceRetained?: (steeringInputId: string, labels: string[], references: SelectionReference[]) => void;
}) {
  const draft = useReferenceDraft(context?.scope);
  const [message, setMessage] = useState("");
  const [pending, setPending] = useState<RunControlIntent | undefined>(() => {
    const stored = context === undefined ? undefined : readRunControl(context.scope);
    return stored?.kind === "guidance" && stored.resend === true ? undefined : stored;
  });
  const [busy, setBusy] = useState(false);
  // A guidance that arrived after its Run settled. It goes as the next AgentRun.
  const [resend, setResend] = useState<Extract<RunControlIntent, { kind: "guidance" }>>();
  // A guidance that waited for its resend when the page reloaded. The author sends it again.
  const [unsent, setUnsent] = useState(false);
  const alive = useRef(true);
  const recovered = useRef(false);
  // An edit of the author after a guidance takes over its message, so a conflicted reply does not resend it.
  const edited = useRef(false);
  const known = run !== undefined && run.run_id === current?.runId
    && run.conversation_id === current.conversationId;
  const active = known && !settledRun(run);
  const pause = active && run.status !== "paused" && message.trim().length === 0;
  const enabled = available && context?.canSubmit === true && context.chapterId !== undefined
    && !sending && !busy && pending === undefined && (current === undefined || known);

  const submitControl = async (intent: RunControlIntent) => {
    if (context === undefined) return;
    saveRunControl(intent);
    edited.current = false;
    setPending(intent);
    setBusy(true);
    onStatus("正在确认任务操作…");
    try {
      const response = await submitRunControl(context, intent);
      // The message waits for the next AgentRun, also through a reload, until the panel records its request.
      if (response.effect.kind === "conflicted" && intent.kind === "guidance" && !edited.current) {
        saveRunControl({ ...intent, resend: true });
      } else clearRunControl(intent);
      if (!alive.current) return;
      setPending(undefined);
      if (response.effect.kind === "retained" && intent.kind === "guidance") {
        setMessage("");
        clearDraftReferences(intent.scope, intent.referenceKeys ?? []);
        onGuidanceRetained?.(response.effect.steering_input_id, intent.labels ?? [],
          intent.request.steer_agent_run_input.selection_references ?? []);
      }
      try {
        await onRefresh();
      } catch {
        if (alive.current) onStatus(response.effect.kind === "conflicted" && intent.kind === "guidance"
          ? "消息未发送，任务状态暂不可读取。请稍后重新发送。" : "操作已确认，任务状态暂不可读取。请稍后检查。");
        return;
      }
      if (!alive.current) return;
      if (response.effect.kind === "conflicted" && intent.kind === "guidance" && !edited.current) {
        setResend(intent);
        return;
      }
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

  const recorded = () => { if (context !== undefined) clearResendRunControl(context.scope); };

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);
  // A later render sends it, because the send of this render still sees the Run as active.
  useEffect(() => {
    if (resend === undefined || !known || active) return;
    setResend(undefined);
    if (edited.current) {
      onStatus("任务已结束，这次操作未执行。");
      return;
    }
    const input = resend.request.steer_agent_run_input;
    onSend(input.author_message.text, () => setMessage(""), { references: input.selection_references ?? [],
      labels: resend.labels ?? [], keys: resend.referenceKeys ?? [] }, recorded);
  }, [resend, known, active]);
  // The read of the Run clears the status, so the status shows after it.
  useEffect(() => {
    if (!unsent || (current !== undefined && !known)) return;
    setUnsent(false);
    onStatus("上次的消息未发送，已放回输入框。请检查后重新发送。");
  }, [unsent, known]);
  useEffect(() => {
    if (context === undefined || recovered.current || !available) return;
    recovered.current = true;
    const retained = readRunControl(context.scope);
    if (retained?.kind === "guidance" && retained.resend === true) {
      // Its references stay in the draft until the panel records the request.
      setMessage(retained.request.steer_agent_run_input.author_message.text);
      setUnsent(true);
    } else if (retained !== undefined) {
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
          pause ? { kind: "pause" } : { kind: "guidance", text: message.trim(),
            references: draft.entries.map((entry) => entry.reference), labels: draftLabels(draft.entries),
            keys: draft.entries.map((entry) => entry.key) }));
      } else if (message.trim().length > 0) onSend(message.trim(), () => setMessage(""), undefined, recorded);
    }}>
      {draft.entries.length === 0 || context === undefined ? null : <ul className="composer-references">
        {draft.entries.map((entry) => <li key={entry.key} data-selection-reference-label={draftLabels([entry])[0]}>
          {/* The text of a heading can be long, so only that part has an ellipsis. */}
          <span className="reference-label">{entry.label.part.kind === "heading"
            ? <>{entry.label.chapter}（小标题「<span className="reference-heading">{entry.label.part.text}</span>」
              {entry.label.candidate === true ? "的修改建议" : ""}）</>
            : draftLabels([entry])[0]}</span>
          <button type="button" aria-label="移除引用" data-remove-selection-reference=""
            onClick={() => removeDraftReference(context.scope, entry.key)}>×</button>
        </li>)}
      </ul>}
      <textarea name="assistant-message" aria-label="给写作助手的消息" rows={2} maxLength={4000}
        value={message} onChange={(event) => {
          setMessage(event.currentTarget.value);
          edited.current = true;
          if (context !== undefined) clearResendRunControl(context.scope);
        }}
        // A paste of a copy from the editor adds its references. Other data pastes as text.
        onPaste={(event) => {
          const data = event.clipboardData.getData(REFERENCE_CLIPBOARD_TYPE);
          if (context !== undefined && data.length > 0 && pasteReferences(context.scope, data)) event.preventDefault();
        }}
        placeholder="描述想修改的当前章节文字" />
      <div className="composer-controls">
        <button className="send-button" type="submit" aria-label={pause ? "暂停" : "发送"}
          disabled={!enabled || (!pause && message.trim().length === 0)}>{pause ? "■" : "↑"}</button>
      </div>
    </form>
  </div>;
}
