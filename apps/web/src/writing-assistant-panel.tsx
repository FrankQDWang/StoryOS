import { useEffect, useRef, useState, type FormEvent } from "react";

import {
  activityStream, createAgentRun, createProjectCommandChallenge, digestCreateAgentRun,
  getAgentRun, getProposal, getManuscriptTree, getProjectAssistance,
  StoryOSProtocolError,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateAgentRunRequest, CreateAgentRunResponse, GetAgentRunResponse, ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE, historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";
import { ProposalLocationLinks } from "./proposal-location-links.tsx";
import type { ProposalDestination, ProposalFocus } from "./proposal-navigation.ts";
import type { ProposalLocator } from "./block-proposal-display.tsx";
import { AssistantRunDetails, type SelectedRunDetails } from "./assistant-run-details.tsx";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

type TranscriptExchange = {
  requestId: string;
  chapterId?: string;
  runId?: string;
  conversationId: string;
  message: string;
  status: GetAgentRunResponse["status"];
  result?: string;
};

type RequestReference = {
  scope: ProjectScope;
  message: string;
  chapterId: string;
  correlationId: string;
  idempotencyKey: string;
  snapshotId: string;
  runId?: string;
  conversationId?: string;
  conversationChoice?: "new";
  history?: TranscriptExchange[];
  selectedRequestId?: string;
};

export type AssistantContext = {
  scope: ProjectScope;
  chapterId?: string;
  candidateTarget?: ProposalFocus | undefined;
  canSubmit: boolean;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
};

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

function storageKey(scope: ProjectScope): string {
  return `prose_request:${scope.owner_user_id}:${scope.project_id}`;
}

function readReference(scope: ProjectScope): RequestReference | undefined {
  const raw = globalThis.sessionStorage?.getItem(storageKey(scope));
  if (raw === null || raw === undefined) return undefined;
  try {
    const value: unknown = JSON.parse(raw);
    if (value === null || typeof value !== "object") return undefined;
    const ref = value as RequestReference;
    if (ref.scope?.owner_user_id !== scope.owner_user_id
      || ref.scope.project_id !== scope.project_id
      || typeof ref.message !== "string"
      || !UUID.test(ref.chapterId)
      || !UUID.test(ref.correlationId)
      || !UUID.test(ref.idempotencyKey)
      || !UUID.test(ref.snapshotId)
      || (ref.runId !== undefined && !UUID.test(ref.runId))
      || (ref.conversationId !== undefined && !UUID.test(ref.conversationId))
      || (ref.conversationChoice !== undefined && ref.conversationChoice !== "new")
      || (ref.selectedRequestId !== undefined && !UUID.test(ref.selectedRequestId))
      || (ref.history !== undefined && (!Array.isArray(ref.history)
        || !ref.history.every((exchange) => exchange !== null && typeof exchange === "object"
          && UUID.test(exchange.requestId) && UUID.test(exchange.conversationId)
          && (exchange.runId === undefined || UUID.test(exchange.runId))
          && typeof exchange.message === "string" && Object.hasOwn(runLabels, exchange.status)
          && (exchange.result === undefined || typeof exchange.result === "string"))))) return undefined;
    return ref;
  } catch {
    return undefined;
  }
}

function saveReference(ref: RequestReference): void {
  globalThis.sessionStorage?.setItem(storageKey(ref.scope), JSON.stringify(ref));
}

function runFromActivity(body: string, ref: RequestReference): string | undefined {
  for (const block of body.split("\n\n")) {
    const line = block.split("\n").find((part) => part.startsWith("data: "));
    if (line === undefined) continue;
    try {
      const event: unknown = JSON.parse(line.slice(6));
      if (event === null || typeof event !== "object") continue;
      const item = event as {
        event_kind?: string;
        correlation_id?: string;
        project_scope?: ProjectScope;
        payload?: { run_id?: string };
      };
      if (item.event_kind === "agent_run_created"
        && item.correlation_id === ref.correlationId
        && item.project_scope?.owner_user_id === ref.scope.owner_user_id
        && item.project_scope.project_id === ref.scope.project_id
        && UUID.test(item.payload?.run_id ?? "")) return item.payload?.run_id;
    } catch {
      continue;
    }
  }
  return undefined;
}

function resultText(run: GetAgentRunResponse): string | undefined {
  switch (run.decision.kind) {
    case "advisory":
    case "prose_change":
      return run.decision.text;
    case "clarification":
      return run.decision.question;
    case "execution_refused":
      return "本次请求未能执行。";
    case "absent":
      return undefined;
  }
}

const runLabels: Record<GetAgentRunResponse["status"], string> = {
  queued: "等待开始",
  claimed: "正在处理",
  waiting: "等待继续",
  paused: "已暂停",
  completed: "已完成",
  refused: "无法完成",
  cancelled: "已取消",
};

export function WritingAssistantPanel({
  collapsed, context, onOpenedProposal, onNavigateProposal,
}: {
  onNavigateProposal?: ((destination: ProposalDestination) => void) | undefined;
  collapsed: boolean;
  context?: AssistantContext | undefined;
  onOpenedProposal?: ((locator: ProposalLocator) => void) | undefined;
}) {
  const [availability, setAvailability] = useState<"available" | "unavailable">("unavailable");
  const [reference, setReference] = useState<RequestReference | undefined>(
    () => context === undefined ? undefined : readReference(context.scope),
  );
  const [run, setRun] = useState<GetAgentRunResponse>();
  const [status, setStatus] = useState("");
  const [sending, setSending] = useState(false);
  const [refused, setRefused] = useState(false);
  const [details, setDetails] = useState<SelectedRunDetails>({ kind: "loading" });
  const detailSequence = useRef(0);

  useEffect(() => {
    if (context === undefined) {
      setAvailability("unavailable");
      return;
    }
    let active = true;
    void getProjectAssistance({
      baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
      projectId: context.scope.project_id,
    }).then((response) => {
      if (!active) return;
      setAvailability(
        response.project_scope.owner_user_id === context.scope.owner_user_id
          && response.project_scope.project_id === context.scope.project_id
          ? response.assistance.availability : "unavailable",
      );
    }).catch(() => {
      if (active) setAvailability("unavailable");
    });
    return () => { active = false; };
  }, [context?.baseUrl, context?.fetchImpl, context?.scope.owner_user_id, context?.scope.project_id]);

  const inspect = async (current: RequestReference): Promise<GetAgentRunResponse | undefined> => {
    if (context === undefined) return;
    const stillCurrent = () =>
      readReference(current.scope)?.correlationId === current.correlationId;
    let runId = current.runId;
    if (runId === undefined) {
      const body = await activityStream({
        baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
        projectId: current.scope.project_id, snapshotId: current.snapshotId,
        protocolRelease: "storyos.public.release.1",
      });
      if (!stillCurrent()) return;
      runId = runFromActivity(body, current);
      if (runId === undefined) {
        setStatus("请求结果仍待确认。请稍后检查。");
        return;
      }
      current = { ...(readReference(current.scope) ?? current), runId };
      saveReference(current);
      setReference(current);
    }
    const result = await getAgentRun({
      baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
      projectId: current.scope.project_id, runId,
    });
    if (!stillCurrent()) return;
    if (result.project_scope.owner_user_id !== current.scope.owner_user_id
      || result.project_scope.project_id !== current.scope.project_id
      || result.run_id !== runId
      || (current.conversationId !== undefined
        && result.conversation_id !== current.conversationId)) throw new Error("Run Scope mismatch");
    current = { ...(readReference(current.scope) ?? current), conversationId: result.conversation_id };
    saveReference(current);
    setReference(current);
    setRun(result);
    if (result.decision.kind === "prose_change") {
      for (const location of result.decision.locations ?? []) {
        if (location.outcome.kind === "opened") onOpenedProposal?.({
          proposalId: location.outcome.proposal_id, runId: result.run_id,
          decisionId: result.decision.decision_id,
        });
      }
      if ((result.decision.locations?.length ?? 0) === 0
        && result.decision.opened_proposal.kind === "present") onOpenedProposal?.({
          proposalId: result.decision.opened_proposal.proposal_id, runId: result.run_id,
          decisionId: result.decision.decision_id,
        });
    }
    setStatus("");
    return result;
  };

  const inspectDetails = async (requestId: string): Promise<void> => {
    if (context === undefined) return;
    const cached = readReference(context.scope);
    if (cached === undefined) return;
    const exchange = cached.history?.find((entry) => entry.requestId === requestId);
    const latest = requestId === cached.correlationId;
    if (!latest && exchange === undefined) return;
    const chosen = { ...cached, selectedRequestId: requestId };
    const expectedRunId = latest ? cached.runId : exchange?.runId;
    const expectedConversationId = latest ? cached.conversationId : exchange?.conversationId;
    const sequence = ++detailSequence.current;
    const stillSelected = () => sequence === detailSequence.current
      && readReference(context.scope)?.selectedRequestId === requestId;
    saveReference(chosen);
    setReference(chosen);
    setDetails({ kind: "loading" });
    try {
      const result = latest ? await inspect(chosen) : expectedRunId === undefined ? undefined
        : await getAgentRun({ baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
          projectId: context.scope.project_id, runId: expectedRunId });
      if (!stillSelected()) return;
      if (result === undefined
        || result.project_scope.owner_user_id !== context.scope.owner_user_id
        || result.project_scope.project_id !== context.scope.project_id
        || (expectedRunId !== undefined && result.run_id !== expectedRunId)
        || (expectedConversationId !== undefined && result.conversation_id !== expectedConversationId)) {
        setDetails({ kind: "unavailable" });
        return;
      }
      setDetails({ kind: "known", run: result, selection: { projectScope: context.scope,
        runId: result.run_id, conversationId: result.conversation_id } });
    } catch {
      if (stillSelected()) setDetails({ kind: "unavailable" });
    }
  };

  useEffect(() => {
    if (reference === undefined || context === undefined) return;
    if (reference.selectedRequestId !== undefined) void inspectDetails(reference.selectedRequestId);
    if (reference.selectedRequestId !== reference.correlationId) {
      void inspect(reference).catch(() => setStatus("无法确认请求结果，请稍后检查。"));
    }
    return () => { detailSequence.current += 1; };
  }, [context?.scope.owner_user_id, context?.scope.project_id]);

  const terminal = run !== undefined
    && (run.status === "completed" || run.status === "refused" || run.status === "cancelled");
  const unresolved = reference !== undefined && (!terminal || reference.conversationId === undefined);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (availability === "unavailable") {
      setRefused(true);
      return;
    }
    if (context === undefined || context.chapterId === undefined
      || !context.canSubmit || availability !== "available" || sending
      || unresolved) return;
    const input = event.currentTarget.elements.namedItem("assistant-message");
    if (!(input instanceof HTMLInputElement)) return;
    const message = input.value.trim();
    if (message.length === 0) return;
    const previous = reference;
    const previousRun = run;
    let storedNew = false;
    let commandSent = false;
    const restorePrevious = () => {
      if (previous === undefined) {
        globalThis.sessionStorage?.removeItem(storageKey(context.scope));
      } else {
        saveReference(previous);
      }
      setReference(previous);
      setRun(previousRun);
    };
    setSending(true);
    setRefused(false);
    setStatus("");
    void (async () => {
      const assistance = await getProjectAssistance({
        baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
        projectId: context.scope.project_id,
      });
      if (assistance.project_scope.owner_user_id !== context.scope.owner_user_id
        || assistance.project_scope.project_id !== context.scope.project_id
        || assistance.assistance.availability !== "available") {
        setAvailability("unavailable");
        setStatus("写作助手当前不可用。你仍可以直接写作。");
        return;
      }
      const tree = await getManuscriptTree({
        baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
        projectId: context.scope.project_id,
      });
      if (tree.project_scope.owner_user_id !== context.scope.owner_user_id
        || tree.project_scope.project_id !== context.scope.project_id
        || tree.snapshot.project_scope.owner_user_id !== context.scope.owner_user_id
        || tree.snapshot.project_scope.project_id !== context.scope.project_id) {
        throw new Error("Working Target Scope mismatch");
      }
      const selected = context.candidateTarget;
      if (selected !== undefined) {
        const response = await getProposal({ baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
          projectId: context.scope.project_id, proposalId: selected.proposalId });
        if (response.project_scope.owner_user_id !== context.scope.owner_user_id
          || response.project_scope.project_id !== context.scope.project_id
          || response.proposal.chapter_id !== context.chapterId
          || !response.proposal.operations.some((operation) => operation.operation_id === selected.operationId
            && operation.manuscript_block_id === selected.blockId && operation.resolution === "pending"
            && operation.reservation_state === "unresolved")) throw new Error("候选位置已变化。");
        selected.revisionId = response.proposal.revision_id;
      }
      const current: RequestReference = {
        scope: context.scope, message, chapterId: context.chapterId!,
        correlationId: uuidV7(context.cryptoImpl),
        idempotencyKey: uuidV7(context.cryptoImpl),
        snapshotId: tree.snapshot.snapshot_id,
        ...(previous?.conversationId === undefined || previous.conversationChoice === "new"
          ? {} : { conversationId: previous.conversationId }),
        history: previous === undefined || previousRun === undefined ? [] : [
          ...(previous.history ?? []), {
            requestId: previous.correlationId, chapterId: previous.chapterId, conversationId: previousRun.conversation_id,
            runId: previousRun.run_id,
            message: previous.message, status: previousRun.status,
            ...(resultText(previousRun) === undefined ? {} : { result: resultText(previousRun)! }),
          },
        ],
      };
      const request: CreateAgentRunRequest = {
        command_schema: "storyos.command.create-agent-run.request.v2",
        create_agent_run_input: {
          conversation: current.conversationId === undefined ? { kind: "new" }
            : { kind: "existing", conversation_id: current.conversationId },
          author_message: { text: current.message },
          working_target: selected === undefined ? { kind: "current_chapter", chapter_id: current.chapterId }
            : { kind: "proposal_candidate", source_chapter_id: current.chapterId, target: {
              proposal_id: selected.proposalId, operation_id: selected.operationId, revision_id: selected.revisionId,
            } },
          instruction: { kind: "absent" },
          cause: { kind: "author_request" },
          client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
          security_policy_revision: SECURITY_POLICY_REVISION,
          correlation_id: current.correlationId,
        },
      };
      const challenge = await createProjectCommandChallenge({
        baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
        projectId: context.scope.project_id,
        request: {
          method: "POST",
          route_template: "/api/v1/projects/{project_id}/agent-runs",
          command_schema: request.command_schema,
          canonical_command_digest: await digestCreateAgentRun(request, context.cryptoImpl),
          idempotency_key: current.idempotencyKey,
        },
      });
      saveReference(current);
      storedNew = true;
      setReference(current);
      setRun(undefined);
      commandSent = true;
      let admitted: CreateAgentRunResponse;
      try {
        admitted = await createAgentRun({
          baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
          projectId: context.scope.project_id,
          idempotencyKey: current.idempotencyKey,
          antiForgery: challenge.nonce, request,
        });
      } catch (error) {
        if (historicalAcknowledgementUnavailable(error)) {
          setStatus(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
          return;
        }
        if (error instanceof StoryOSProtocolError
          && error.code === "command_http_error"
          && error.status !== undefined && error.status >= 400 && error.status < 500) {
          restorePrevious();
          setStatus("本次请求未被接收。请检查当前章节和写作助手状态。");
          return;
        }
        throw error;
      }
      if (admitted.project_scope.owner_user_id !== current.scope.owner_user_id
        || admitted.project_scope.project_id !== current.scope.project_id
        || (current.conversationId !== undefined && admitted.conversation_id !== current.conversationId)) {
        throw new Error("Run admission Scope mismatch");
      }
      const acknowledged = { ...current, runId: admitted.effect.run_id,
        conversationId: admitted.conversation_id };
      saveReference(acknowledged);
      setReference(acknowledged);
      input.value = "";
      await inspect(acknowledged);
    })().catch(() => {
      if (!commandSent) {
        if (storedNew) restorePrevious();
        setStatus("无法提交请求，请稍后再试。");
      } else {
        setStatus("无法确认请求结果，请稍后检查。");
      }
    })
      .finally(() => setSending(false));
  };

  return (
    <aside id="writing-assistant-panel" className="agent-panel" data-writing-assistant=""
      data-assistant-availability={availability}
      data-assistant-dispatch={sending ? "sending" : reference?.runId === undefined
        && reference !== undefined ? "uncertain" : refused ? "refused" : run?.status ?? "idle"}
      data-assistant-run-id={reference?.runId ?? ""}
      data-assistant-request-id={reference?.correlationId ?? ""}
      aria-label={collapsed ? "写作助手已收起" : "写作助手对话"}>
      <div className="assistant-body" hidden={collapsed}>
        <header className="agent-header">
          <strong>写作助手</strong>
          <button type="button" data-assistant-new-conversation=""
            disabled={reference === undefined || unresolved || sending || reference.conversationChoice === "new"}
            onClick={() => {
              if (reference === undefined || unresolved || sending) return;
              const selected = { ...reference, conversationChoice: "new" as const };
              saveReference(selected);
              setReference(selected);
              setStatus("");
            }}>新对话</button>
        </header>
        <div className="assistant-conversation" aria-live="polite">
          {availability === "unavailable" ? (
            <p className="assistant-status">写作助手当前不可用。你仍可以直接写作。</p>
          ) : null}
          {reference?.history?.map((exchange, index, history) => (
            <section className="assistant-exchange" key={exchange.requestId}>
              {index > 0 && history[index - 1]?.conversationId !== exchange.conversationId
                ? <p className="assistant-conversation-boundary">新对话</p> : null}
              <p className="assistant-author-message">{exchange.message}</p>
              <p>{runLabels[exchange.status]}</p>
              {exchange.result === undefined ? null : <p className="assistant-result">{exchange.result}</p>}
              <button type="button" data-assistant-history-inspect={exchange.runId ?? ""}
                aria-expanded={reference.selectedRequestId === exchange.requestId}
                onClick={() => { void inspectDetails(exchange.requestId); }}>检查结果</button>
              {reference.selectedRequestId === exchange.requestId
                ? <><AssistantRunDetails details={details} />
                  {details.kind === "known" ? <ProposalLocationLinks run={details.run}
                    sourceChapterId={exchange.chapterId ?? reference.chapterId} onNavigate={onNavigateProposal} /> : null}</> : null}
            </section>
          ))}
          {reference === undefined ? null : (
            <section className="assistant-exchange">
              {(reference.history?.length ?? 0) > 0
                && reference.history?.at(-1)?.conversationId !== reference.conversationId
                ? <p className="assistant-conversation-boundary">新对话</p> : null}
              <p className="assistant-author-message">{reference.message}</p>
              <p data-assistant-run-status="">{run === undefined ? "请求结果待确认" : runLabels[run.status]}</p>
              {run === undefined ? null : <><p className="assistant-result" data-assistant-result="">{resultText(run) ?? "结果尚未生成。"}</p>
                <ProposalLocationLinks run={run} sourceChapterId={reference.chapterId} onNavigate={onNavigateProposal} /></>}
              <button type="button" data-assistant-inspect=""
                aria-expanded={reference.selectedRequestId === reference.correlationId} onClick={() => {
                void inspectDetails(reference.correlationId);
              }}>检查结果</button>
              {reference.selectedRequestId === reference.correlationId
                ? <AssistantRunDetails details={details} /> : null}
            </section>
          )}
          {reference?.conversationChoice === "new"
            ? <p className="assistant-status">下一条消息将开始新对话。</p> : null}
          {status.length > 0 ? <p role="status">{status}</p> : null}
        </div>
        <form className="composer" data-writing-assistant-composer="" onSubmit={submit}>
          <input name="assistant-message" aria-label="给写作助手的消息" maxLength={4000}
            placeholder="描述想修改的当前章节文字" />
          <button type="submit" disabled={availability !== "available" || !context?.canSubmit
            || context.chapterId === undefined || sending
            || unresolved}>发送</button>
        </form>
      </div>
    </aside>
  );
}
