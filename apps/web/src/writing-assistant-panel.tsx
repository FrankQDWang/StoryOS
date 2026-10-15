import { useEffect, useRef, useState } from "react";

import {
  activityStream, createAgentRun, createProjectCommandChallenge, digestCreateAgentRun,
  getAgentRun, getProposal, getManuscriptTree, getProjectAssistance,
  StoryOSProtocolError,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateAgentRunRequest, CreateAgentRunResponse, GetAgentRunResponse, GetManuscriptTreeResponse, ProjectScope,
  SelectionReference,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE, historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";
import { AssistantComposer } from "./assistant-composer.tsx";
import {
  CONVERSATION_TOO_LONG_SENTENCE, DESTINATION_REFUSED_SENTENCE, MALFORMED_REPLY_SENTENCE, NO_RESULT_SENTENCE,
  PREVIOUS_REQUEST_UNRESOLVED_SENTENCE, malformedStructuredReply, settledRun, stoppedWithoutResult,
} from "./assistant-run-outcome.ts";
import { ProposalLocationLinks } from "./proposal-location-links.tsx";
import type { ProposalDestination, ProposalFocus } from "./proposal-navigation.ts";
import type { ProposalLocator } from "./block-proposal-display.tsx";
import { mergeReferences } from "./selection-reference.ts";
import { clearDraftReferences, draftEntries, draftLabels, setRunReferences } from "./selection-reference-draft.ts";

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
  run?: GetAgentRunResponse;
  referenceLabels?: string[];
  guidance?: GuidanceReferences[];
};

/** The labels and the references that one retained steering input added (ADR 0062). */
type GuidanceReferences = { steeringInputId: string; labels: string[]; references: SelectionReference[] };

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
  /** The references of the message, and their labels for the history. */
  references?: SelectionReference[];
  referenceLabels?: string[];
  guidance?: GuidanceReferences[];
};

/** Calls `refresh` later, and returns a function that cancels the call. */
export type RefreshSchedule = (refresh: () => void) => () => void;

const RUN_REFRESH_MS = 2000;

const scheduleRunRefresh: RefreshSchedule = (refresh) => {
  const timer = setTimeout(refresh, RUN_REFRESH_MS);
  return () => clearTimeout(timer);
};

export type AssistantContext = {
  scope: ProjectScope;
  chapterId?: string;
  candidateTarget?: ProposalFocus | undefined;
  tree?: GetManuscriptTreeResponse | undefined;
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

/** A Project without a binding uses the destination that the deployment offers (ADR 0048). */
function withoutBinding(error: unknown): boolean {
  if (!(error instanceof StoryOSProtocolError) || error.status !== 404) return false;
  try {
    return (JSON.parse(String(error.responseBody)) as { code?: unknown }).code === "assistance_not_bound";
  } catch {
    return false;
  }
}

function resultText(run: GetAgentRunResponse): string | undefined {
  if (stoppedWithoutResult(run)) return NO_RESULT_SENTENCE;
  switch (run.decision.kind) {
    case "advisory":
      return malformedStructuredReply(run.decision.text) ? MALFORMED_REPLY_SENTENCE : run.decision.text;
    case "prose_change":
      return (run.decision.locations?.length ?? 0) > 0
        ? `这次修改涉及 ${new Set(run.decision.locations!.map(location => location.chapter_id)).size} 个章节、${run.decision.locations!.length} 处文字，具体建议在下方。`
        : "本次修改建议已生成，可以在正文中查看。";
    case "clarification":
      return run.decision.question;
    case "execution_refused":
      switch (run.decision.capability) {
        case "model_runtime_qualification_pending":
          return "真实模型还没有通过运行验证，暂时不能使用。你可以继续手动写作。";
        case "disclosure_outside_grant":
          return "这次请求超出了本项目允许发送给 AI 的范围，没有发送任何内容。你可以继续手动写作。";
        case "conversation_too_long":
          return CONVERSATION_TOO_LONG_SENTENCE;
        case "destination_refused":
          return DESTINATION_REFUSED_SENTENCE;
        case "expiry_rebuild_unknown_create":
          return PREVIOUS_REQUEST_UNRESOLVED_SENTENCE;
        default:
          return "本次请求未能执行。";
      }
    case "absent":
      return undefined;
  }
}

/** The labels of the references that went with one message. */
function ReferenceLabels({ labels }: { labels: readonly string[] | undefined }) {
  return labels === undefined || labels.length === 0 ? null : <ul className="assistant-reference-labels">
    {labels.map((label, index) => <li key={index} data-sent-reference-label="">{label}</li>)}
  </ul>;
}

/** The author messages of an exchange: the request, then each steering input, with their labels. */
function AuthorMessages({ message, labels, run, guidance }: {
  message: string;
  labels: readonly string[] | undefined;
  run: GetAgentRunResponse | undefined;
  guidance: readonly GuidanceReferences[] | undefined;
}) {
  return <>
    <p className="assistant-author-message">{message}</p>
    <ReferenceLabels labels={labels} />
    {run?.steering_inputs?.map((input) => <div key={input.steering_input_id}>
      <p className="assistant-author-message">{input.author_message}</p>
      <ReferenceLabels labels={guidance?.find((item) => item.steeringInputId === input.steering_input_id)?.labels} />
    </div>)}
  </>;
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
  collapsed, context, onOpenedProposal, onNavigateProposal, scheduleRefresh = scheduleRunRefresh,
}: {
  /** Schedules the next read of a Run that has not settled. Tests inject their own clock. */
  scheduleRefresh?: RefreshSchedule;
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
  const [resultUnavailable, setResultUnavailable] = useState(false);
  const inspectSequence = useRef(0);
  const [failedReads, setFailedReads] = useState(0);

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
    }).catch((error: unknown) => {
      if (active) setAvailability(withoutBinding(error) ? "available" : "unavailable");
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
        if (location.outcome.kind === "revised") {
          const response = await getProposal({ baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
            projectId: current.scope.project_id, proposalId: location.outcome.proposal_id });
          if (!stillCurrent()) return;
          if (response.project_scope.owner_user_id === current.scope.owner_user_id
            && response.project_scope.project_id === current.scope.project_id
            && response.proposal.source.kind === "agent_run_decision") onOpenedProposal?.({
              proposalId: response.proposal.proposal_id, runId: response.proposal.source.run_id,
              decisionId: response.proposal.source.decision_id,
            });
        }
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

  const inspectResult = async (): Promise<void> => {
    if (context === undefined) return;
    const current = readReference(context.scope);
    if (current === undefined) return;
    const sequence = ++inspectSequence.current;
    const stillCurrent = () => sequence === inspectSequence.current
      && readReference(context.scope)?.correlationId === current.correlationId;
    setResultUnavailable(false);
    try {
      if (await inspect(current) === undefined && stillCurrent()) setResultUnavailable(true);
    } catch {
      if (stillCurrent()) setResultUnavailable(true);
    }
  };

  useEffect(() => {
    if (reference === undefined || context === undefined) return;
    void inspect(reference).catch(() => setStatus("无法确认请求结果，请稍后检查。"));
    return () => { inspectSequence.current += 1; };
  }, [context?.scope.owner_user_id, context?.scope.project_id]);

  const terminal = run !== undefined && settledRun(run);

  // The add control counts the references of the active AgentRun toward its limit.
  useEffect(() => {
    if (context === undefined) return;
    setRunReferences(context.scope, reference?.runId === undefined || terminal ? []
      : (reference.guidance ?? []).reduce((all, input) => mergeReferences(all, input.references),
        reference.references ?? []));
  }, [reference, terminal]);

  // A Run can wait in the queue of a separate Worker, so read it again until it settles. A failed
  // read schedules the next read too.
  useEffect(() => {
    if (reference?.runId === undefined || terminal || context === undefined) return;
    return scheduleRefresh(() => {
      void inspect(reference).catch(() => {
        setStatus("无法确认请求结果，请稍后检查。");
        setFailedReads((count) => count + 1);
      });
    });
  }, [reference?.runId, run, terminal, failedReads]);
  const unresolved = reference !== undefined && (!terminal || reference.conversationId === undefined);

  const submit = (message: string, onAdmitted: () => void) => {
    if (availability === "unavailable") {
      setRefused(true);
      return;
    }
    if (context === undefined || context.chapterId === undefined
      || !context.canSubmit || availability !== "available" || sending
      || unresolved) return;
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
      }).catch((error: unknown) => {
        if (withoutBinding(error)) return undefined;
        throw error;
      });
      if (assistance !== undefined
        && (assistance.project_scope.owner_user_id !== context.scope.owner_user_id
          || assistance.project_scope.project_id !== context.scope.project_id
          || assistance.assistance.availability !== "available")) {
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
      const entries = draftEntries(context.scope);
      let selected = context.candidateTarget;
      let candidateLength = 0;
      if (selected !== undefined) {
        const observedTarget = selected;
        const response = await getProposal({ baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
          projectId: context.scope.project_id, proposalId: selected.proposalId });
        if (response.project_scope.owner_user_id !== context.scope.owner_user_id
          || response.project_scope.project_id !== context.scope.project_id
          || response.proposal.chapter_id !== context.chapterId
          || !response.proposal.operations.some((operation) => operation.operation_id === observedTarget.operationId
            && operation.manuscript_block_id === observedTarget.blockId && operation.resolution === "pending"
            && operation.reservation_state === "unresolved")) throw new Error("候选位置已变化。");
        candidateLength = response.proposal.operations.find((operation) =>
          operation.operation_id === observedTarget.operationId)!.candidate_text.length;
        selected = { ...selected, revisionId: response.proposal.revision_id };
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
            runId: previousRun.run_id, run: previousRun,
            message: previous.message, status: previousRun.status,
            ...(previous.referenceLabels === undefined ? {} : { referenceLabels: previous.referenceLabels }),
            ...(previous.guidance === undefined ? {} : { guidance: previous.guidance }),
            ...(resultText(previousRun) === undefined ? {} : { result: resultText(previousRun)! }),
          },
        ],
      };
      // A transition until the candidate references of the composer: the caret candidate is one reference to all of its text.
      const references = mergeReferences(entries.map((entry) => entry.reference), selected === undefined ? [] : [{
        kind: "proposal_candidate_text", chapter_id: current.chapterId, proposal_id: selected.proposalId,
        operation_id: selected.operationId, revision_id: selected.revisionId,
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", start_offset: 0, end_offset: candidateLength,
      }]);
      if (references.length > 0) {
        current.references = references;
        current.referenceLabels = draftLabels(entries);
      }
      const request: CreateAgentRunRequest = {
        command_schema: "storyos.command.create-agent-run.request.v2",
        create_agent_run_input: {
          conversation: current.conversationId === undefined ? { kind: "new" }
            : { kind: "existing", conversation_id: current.conversationId },
          author_message: { text: current.message },
          working_target: references.length === 0 ? { kind: "current_chapter", chapter_id: current.chapterId }
            : { kind: "selection_references", source_chapter_id: current.chapterId, references },
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
      clearDraftReferences(context.scope);
      onAdmitted();
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
            <section className="assistant-exchange" key={exchange.requestId} data-assistant-history-run={exchange.runId}>
              {index > 0 && history[index - 1]?.conversationId !== exchange.conversationId
                ? <p className="assistant-conversation-boundary">新对话</p> : null}
              <AuthorMessages message={exchange.message} labels={exchange.referenceLabels} run={exchange.run}
                guidance={exchange.guidance} />
              {exchange.result === undefined ? null : <p className="assistant-result">{exchange.result}</p>}
              {exchange.run === undefined ? null : <ProposalLocationLinks run={exchange.run} tree={context?.tree}
                sourceChapterId={exchange.chapterId ?? reference.chapterId} onNavigate={onNavigateProposal} />}
            </section>
          ))}
          {reference === undefined ? null : (
            <section className="assistant-exchange" data-assistant-current-run="">
              {(reference.history?.length ?? 0) > 0
                && reference.history?.at(-1)?.conversationId !== reference.conversationId
                ? <p className="assistant-conversation-boundary">新对话</p> : null}
              <AuthorMessages message={reference.message} labels={reference.referenceLabels} run={run}
                guidance={reference.guidance} />
              {!terminal ? <p data-assistant-run-status="">{run === undefined ? "请求结果待确认" : runLabels[run.status]}</p> : null}
              {run === undefined ? null : <><p className="assistant-result" data-assistant-result="">{resultText(run) ?? "结果尚未生成。"}</p>
                <ProposalLocationLinks run={run} tree={context?.tree} sourceChapterId={reference.chapterId} onNavigate={onNavigateProposal} /></>}
              {!terminal ? <button type="button" data-assistant-inspect="" onClick={() => {
                void inspectResult();
              }}>检查结果</button> : null}
              {resultUnavailable ? <p role="status">请求结果暂不可读取，请稍后检查。</p> : null}
            </section>
          )}
          {reference?.conversationChoice === "new"
            ? <p className="assistant-status">下一条消息将开始新对话。</p> : null}
          {status.length > 0 ? <p role="status">{status}</p> : null}
        </div>
        <AssistantComposer key={context === undefined ? "none" : `${context.scope.owner_user_id}:${context.scope.project_id}`}
          context={context} available={availability === "available"} sending={sending} current={reference} run={run}
          onSend={submit} onRefresh={async () => { if (reference !== undefined) await inspect(reference); }}
          onGuidanceRetained={(steeringInputId, labels, references) => {
            const latest = context === undefined ? undefined : readReference(context.scope);
            if (latest === undefined || (labels.length === 0 && references.length === 0)) return;
            const next = { ...latest, guidance: [...latest.guidance ?? [], { steeringInputId, labels, references }] };
            saveReference(next);
            setReference(next);
          }}
          onStatus={setStatus} />
      </div>
    </aside>
  );
}
