import { useEffect, useRef, useState } from "react";

import {
  activityStream, createAgentRun, createProjectCommandChallenge, digestCreateAgentRun,
  getAgentRun, getProposal, getManuscriptTree, getProjectAssistance, listSkills,
  StoryOSProtocolError,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateAgentRunRequest, CreateAgentRunResponse, GetAgentRunResponse, GetManuscriptTreeResponse,
  ProjectScope, ProseChangeLocationInspect, SelectionReference, SkillCatalogEntry, SkillPackageReference,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE, historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";
import { AssistantComposer } from "./assistant-composer.tsx";
import { createChallengeAdmissionWait, type ChallengeAdmissionTimers, type ChallengeAdmissionWait }
  from "./challenge-admission-wait.ts";
import {
  CONVERSATION_TOO_LONG_SENTENCE, DESTINATION_REFUSED_SENTENCE, NO_RESULT_SENTENCE,
  PREVIOUS_REQUEST_UNRESOLVED_SENTENCE, RESEARCH_SYNTHESIS_SENTENCE, RESEARCH_TOOL_UNAVAILABLE_SENTENCE,
  USE_RESEARCH_MESSAGE, proposalSentence, runResults, settledRun, toolCallLines,
  stoppedWithoutResult,
} from "./assistant-run-outcome.ts";
import { ProposalLocationLinks } from "./proposal-location-links.tsx";
import { ProjectConversationList } from "./project-conversation-list.tsx";
import { readOpenedConversation, type OpenedConversation, type OpenedRun } from "./project-conversation-read.ts";
import { ResearchTurn, researchDecision } from "./research-turn.tsx";
import { ManuscriptUseList } from "./manuscript-use-list.tsx";
import { RunResultList } from "./run-result-list.tsx";
import { SkillUseList } from "./skill-use-list.tsx";
import type { ProposalDestination } from "./proposal-navigation.ts";
import type { ProposalLocator } from "./block-proposal-display.tsx";
import { mergeReferences } from "./selection-reference.ts";
import { clearDraftReferences, draftEntries, draftLabels, setRunReferences } from "./selection-reference-draft.ts";
import { SKILL_UNAVAILABLE_SENTENCE, skillSelectionUnavailable } from "./skill-selection.ts";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";
export const CONVERSATION_NOT_FOUND_SENTENCE = "找不到这段对话。";
export const CONVERSATION_OPEN_FAILED_SENTENCE = "无法打开这段对话，请稍后再试。";
export const EARLIER_TURNS_EXPIRED_SENTENCE = "这段对话已超过 72 小时，助手可能不记得之前的内容。";
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
  skillNames?: string[];
  guidance?: GuidanceReferences[];
};

/** The labels and the references that one retained steering input added (ADR 0062). */
type GuidanceReferences = { steeringInputId: string; labels: string[]; references: SelectionReference[] };

type RequestReference = {
  scope: ProjectScope;
  message: string;
  chapterId: string;
  correlationId: string;
  /** The key and the Snapshot of a request of this browser session. A Conversation that the panel opened from the
   * Host has neither, because its Runs have their identities. */
  idempotencyKey?: string;
  snapshotId?: string;
  runId?: string;
  conversationId?: string;
  conversationChoice?: "new";
  history?: TranscriptExchange[];
  /** The references of the message, and their labels for the history. */
  references?: SelectionReference[];
  referenceLabels?: string[];
  /** The draft keys of the references, which go from the composer when the Host admitted the Run. */
  referenceKeys?: string[];
  /** The names of the Skills that the message selected, for their labels. */
  skillNames?: string[];
  guidance?: GuidanceReferences[];
  /** The Conversation that the panel opened from the Host, for its notice lines. */
  opened?: OpenedConversation;
};

/** The references of a message, their labels for the history, and their draft keys. */
export type SentReferences = { references: SelectionReference[]; labels: string[]; keys: string[] };

/** The Skills that a message selects, and the step that removes them when the deployment refuses them. */
export type SentSkills = { references: SkillPackageReference[]; unavailable: () => void };

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
      || (ref.idempotencyKey === undefined ? ref.runId === undefined : !UUID.test(ref.idempotencyKey))
      || (ref.snapshotId === undefined ? ref.runId === undefined : !UUID.test(ref.snapshotId))
      || (ref.runId !== undefined && !UUID.test(ref.runId))
      || (ref.opened !== undefined && (ref.opened === null || typeof ref.opened !== "object"
        || ![ref.opened.conversationId, ref.opened.firstRunId, ref.opened.newestRunId].every((value) => UUID.test(value))
        || typeof ref.opened.truncated !== "boolean" || typeof ref.opened.expired !== "boolean"))
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

function resultText(run: GetAgentRunResponse): string | undefined {
  if (stoppedWithoutResult(run)) return NO_RESULT_SENTENCE;
  switch (run.decision.kind) {
    case "advisory":
      return run.decision.text;
    case "prose_change":
      return proposalSentence(run.decision.locations ?? []);
    case "tool_request":
      return toolCallLines(run.decision.calls);
    case "research_synthesis":
      return RESEARCH_SYNTHESIS_SENTENCE;
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
        case "research_tool_unavailable":
          return RESEARCH_TOOL_UNAVAILABLE_SENTENCE;
        default:
          return "本次请求未能执行。";
      }
    case "absent":
      return undefined;
  }
}

/** The names of the Skills that the author selected for a Run that the panel read from the Host (ADR 0089 section 8). */
function selectedSkillNames(run: GetAgentRunResponse): { skillNames?: string[] } {
  const names = (run.skill_uses ?? []).flatMap((use) => use.provenance === "author_selected" ? [use.name] : []);
  return names.length === 0 ? {} : { skillNames: names };
}

/** One exchange of a Conversation that the panel opened from the Host. It has no reference label (ADR 0062). */
function openedExchange({ row, run }: OpenedRun): TranscriptExchange {
  const result = resultText(run);
  return { requestId: row.run_id, chapterId: row.chapter_id, runId: row.run_id, conversationId: run.conversation_id,
    run, message: row.author_message, status: run.status, ...selectedSkillNames(run),
    ...(result === undefined ? {} : { result }) };
}

/** The Proposals that a Run shows: each Proposal result of a settled Run, or else the Proposal of its decision. */
function shownProposals(run: GetAgentRunResponse): { decisionId: string; proposalId?: string; locations: ProseChangeLocationInspect[] }[] {
  const results = runResults(run);
  if (results !== undefined) return results.flatMap(result => result.kind === "proposal"
    ? [{ decisionId: result.decision_id, proposalId: result.proposal_id, locations: result.locations }] : []);
  if (run.decision.kind !== "prose_change") return [];
  return [{ decisionId: run.decision.decision_id, locations: run.decision.locations ?? [],
    ...(run.decision.opened_proposal.kind === "present" ? { proposalId: run.decision.opened_proposal.proposal_id } : {}) }];
}

/** The labels of the references and of the Skills that went with one message. */
function ReferenceLabels({ labels, skills }: {
  labels: readonly string[] | undefined;
  skills?: readonly string[] | undefined;
}) {
  return (labels?.length ?? 0) + (skills?.length ?? 0) === 0 ? null : <ul className="assistant-reference-labels">
    {labels?.map((label, index) => <li key={index} data-sent-reference-label="">{label}</li>)}
    {skills?.map((name) => <li key={`$${name}`} data-sent-skill-label="">${name}</li>)}
  </ul>;
}

/** The author messages of an exchange: the request, then each steering input, with their labels. */
function AuthorMessages({ message, labels, skills, run, guidance }: {
  message: string;
  labels: readonly string[] | undefined;
  skills: readonly string[] | undefined;
  run: GetAgentRunResponse | undefined;
  guidance: readonly GuidanceReferences[] | undefined;
}) {
  return <>
    <p className="assistant-author-message">{message}</p>
    <ReferenceLabels labels={labels} skills={skills} />
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
  requestedConversationId, challengeTimers,
}: {
  /** The timers of the Retry-After wait of a Command Challenge rate limit. Tests inject their own clock. */
  challengeTimers?: ChallengeAdmissionTimers | undefined;
  /** The Conversation that the workspace address names. The panel opens it one time, when it first gets it. */
  requestedConversationId?: string | undefined;
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
  const [skills, setSkills] = useState<SkillCatalogEntry[]>();
  // A refused selection reads the Skills of the deployment again.
  const [skillReads, setSkillReads] = useState(0);
  const [listOpen, setListOpen] = useState(false);
  // The address opens its Conversation only after the panel read the stored request, so that it knows its state.
  const [storedReadDone, setStoredReadDone] = useState(reference === undefined);
  const requestedOpened = useRef(false);
  // A send waits for the Retry-After of a Command Challenge rate limit. Leaving the Project ends the wait (#1679).
  const admission = useRef<ChallengeAdmissionWait | undefined>(undefined);
  useEffect(() => {
    const wait = createChallengeAdmissionWait(challengeTimers);
    admission.current = wait;
    return () => { wait.cancel(); };
  }, [context?.scope.owner_user_id, context?.scope.project_id]);

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
      const owned = response.project_scope.owner_user_id === context.scope.owner_user_id
        && response.project_scope.project_id === context.scope.project_id;
      setAvailability(owned ? response.assistance?.availability ?? "available" : "unavailable");
    }).catch(() => {
      if (active) setAvailability("unavailable");
    });
    return () => { active = false; };
  }, [context?.baseUrl, context?.fetchImpl, context?.scope.owner_user_id, context?.scope.project_id]);

  useEffect(() => {
    setSkills(undefined);
    if (context === undefined) return;
    let active = true;
    void listSkills({ baseUrl: context.baseUrl, fetchImpl: context.fetchImpl, projectId: context.scope.project_id })
      .then((response) => {
        if (active && response.project_scope.owner_user_id === context.scope.owner_user_id
          && response.project_scope.project_id === context.scope.project_id) setSkills(response.skills);
      }, () => {});
    return () => { active = false; };
  }, [context?.baseUrl, context?.fetchImpl, context?.scope.owner_user_id, context?.scope.project_id, skillReads]);

  const inspect = async (current: RequestReference): Promise<GetAgentRunResponse | undefined> => {
    if (context === undefined) return;
    const stillCurrent = () =>
      readReference(current.scope)?.correlationId === current.correlationId;
    let runId = current.runId;
    if (runId === undefined) {
      if (current.snapshotId === undefined) return;
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
    // The Run exists, so its references go from the composer also after a lost acknowledgement.
    clearDraftReferences(current.scope, current.referenceKeys ?? []);
    if (!await showProposals(current, result)) return;
    setStatus("");
    return result;
  };

  /** Gives each Proposal of the Run to the editor. Gives false when the request record changed during a read. */
  const showProposals = async (current: RequestReference, result: GetAgentRunResponse): Promise<boolean> => {
    if (context === undefined) return false;
    const stillCurrent = () =>
      readReference(current.scope)?.correlationId === current.correlationId;
    for (const proposal of shownProposals(result)) {
      for (const location of proposal.locations) {
        if (location.outcome.kind === "revised") {
          const response = await getProposal({ baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
            projectId: current.scope.project_id, proposalId: location.outcome.proposal_id });
          if (!stillCurrent()) return false;
          if (response.project_scope.owner_user_id === current.scope.owner_user_id
            && response.project_scope.project_id === current.scope.project_id
            && response.proposal.source.kind === "agent_run_decision") onOpenedProposal?.({
              proposalId: response.proposal.proposal_id, runId: response.proposal.source.run_id,
              decisionId: response.proposal.source.decision_id,
            });
        }
        if (location.outcome.kind === "opened") onOpenedProposal?.({
          proposalId: location.outcome.proposal_id, runId: result.run_id, decisionId: proposal.decisionId,
        });
      }
      if (proposal.locations.length === 0 && proposal.proposalId !== undefined) onOpenedProposal?.({
        proposalId: proposal.proposalId, runId: result.run_id, decisionId: proposal.decisionId,
      });
    }
    return true;
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
    void inspect(reference).catch(() => setStatus("无法确认请求结果，请稍后检查。"))
      .finally(() => setStoredReadDone(true));
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

  /**
   * Opens a Conversation of the Project from the Host (ADR 0094 section 10). Its newest Run becomes the current
   * exchange, and the panel shows the exchanges of no other Conversation. A failure keeps the panel as it was.
   */
  const openConversation = async (conversationId: string, failure: string) => {
    if (context === undefined) return;
    if (!UUID.test(conversationId) || sending || unresolved) {
      setStatus(failure);
      return;
    }
    setListOpen(false);
    setSending(true);
    setStatus("");
    try {
      const { runs, opened } = await readOpenedConversation({
        scope: context.scope, baseUrl: context.baseUrl, fetchImpl: context.fetchImpl, conversationId,
      });
      const newest = runs.at(-1)!;
      const next: RequestReference = {
        scope: context.scope, message: newest.row.author_message, chapterId: newest.row.chapter_id,
        correlationId: uuidV7(context.cryptoImpl), runId: newest.row.run_id, conversationId,
        history: runs.slice(0, -1).map(openedExchange), opened, ...selectedSkillNames(newest.run),
      };
      inspectSequence.current += 1;
      saveReference(next);
      setReference(next);
      setRun(newest.run);
      void showProposals(next, newest.run).catch(() => {});
    } catch {
      setStatus(failure);
    } finally {
      setSending(false);
    }
  };

  useEffect(() => {
    const conversationId = requestedConversationId;
    if (conversationId === undefined || requestedOpened.current || context === undefined || !storedReadDone) return;
    requestedOpened.current = true;
    void openConversation(conversationId, CONVERSATION_NOT_FOUND_SENTENCE);
  }, [storedReadDone, context === undefined, requestedConversationId]);

  /** `onRecorded` runs in the same step that stores the request record, before the request goes. */
  const submit = (message: string, onAdmitted: () => void, frozen?: SentReferences, onRecorded?: () => void,
    selected?: SentSkills) => {
    if (availability === "unavailable") {
      setRefused(true);
      return;
    }
    if (context === undefined || context.chapterId === undefined
      || !context.canSubmit || availability !== "available" || sending
      || unresolved) return;
    if (message.length === 0) return;
    const admissionWait = admission.current;
    if (admissionWait === undefined) return;
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
      // A Project without a binding uses the destination that the deployment offers (ADR 0048).
      if (assistance.project_scope.owner_user_id !== context.scope.owner_user_id
        || assistance.project_scope.project_id !== context.scope.project_id
        || (assistance.assistance !== null && assistance.assistance.availability !== "available")) {
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
      const sent = frozen ?? { references: entries.map((entry) => entry.reference), labels: draftLabels(entries),
        keys: entries.map((entry) => entry.key) };
      const current: RequestReference & { idempotencyKey: string } = {
        scope: context.scope, message, chapterId: context.chapterId!,
        correlationId: uuidV7(context.cryptoImpl),
        idempotencyKey: uuidV7(context.cryptoImpl),
        snapshotId: tree.snapshot.snapshot_id,
        ...(previous?.opened === undefined ? {} : { opened: previous.opened }),
        ...(previous?.conversationId === undefined || previous.conversationChoice === "new"
          ? {} : { conversationId: previous.conversationId }),
        history: previous === undefined || previousRun === undefined ? [] : [
          ...(previous.history ?? []), {
            requestId: previous.correlationId, chapterId: previous.chapterId, conversationId: previousRun.conversation_id,
            runId: previousRun.run_id, run: previousRun,
            message: previous.message, status: previousRun.status,
            ...(previous.referenceLabels === undefined ? {} : { referenceLabels: previous.referenceLabels }),
            ...(previous.skillNames === undefined ? {} : { skillNames: previous.skillNames }),
            ...(previous.guidance === undefined ? {} : { guidance: previous.guidance }),
            ...(resultText(previousRun) === undefined ? {} : { result: resultText(previousRun)! }),
          },
        ],
      };
      const { references } = sent;
      if (references.length > 0) {
        current.references = references;
        current.referenceLabels = sent.labels;
        current.referenceKeys = sent.keys;
      }
      if (selected !== undefined) current.skillNames = selected.references.map((skill) => skill.name);
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
          ...(selected === undefined ? {} : { skill_selections: selected.references }),
          client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
          security_policy_revision: SECURITY_POLICY_REVISION,
          correlation_id: current.correlationId,
        },
      };
      const commandDigest = await digestCreateAgentRun(request, context.cryptoImpl);
      const challenge = await admissionWait.retry(() => createProjectCommandChallenge({
        baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
        projectId: context.scope.project_id,
        request: {
          method: "POST",
          route_template: "/api/v1/projects/{project_id}/agent-runs",
          command_schema: request.command_schema,
          canonical_command_digest: commandDigest,
          idempotency_key: current.idempotencyKey,
        },
      }));
      // The author left the Project during the wait, so no request goes.
      if (challenge === undefined) return;
      saveReference(current);
      onRecorded?.();
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
        // The message and its references stay in the composer, and the author selects again or sends it.
        if (skillSelectionUnavailable(error)) {
          restorePrevious();
          selected?.unavailable();
          setSkillReads((count) => count + 1);
          setStatus(SKILL_UNAVAILABLE_SENTENCE);
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
      clearDraftReferences(context.scope, current.referenceKeys ?? []);
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

  // The lines of an opened Conversation come only from its read (S7-F1a). The panel calculates no time.
  const earlierHidden = (runId: string | undefined) => reference?.opened?.truncated
    && runId === reference.opened.firstRunId
    ? <p className="assistant-status" data-assistant-earlier-hidden="">更早的消息没有显示。</p> : null;
  const earlierExpired = (runId: string | undefined) => reference?.opened?.expired
    && runId === reference.opened.newestRunId
    ? <p className="assistant-status" data-assistant-earlier-expired="">{EARLIER_TURNS_EXPIRED_SENTENCE}</p> : null;

  const useResearch = <button type="button" data-assistant-use-research="" disabled={sending || unresolved}
    onClick={() => submit(USE_RESEARCH_MESSAGE, () => {}, { references: [], labels: [], keys: [] })}>用到故事里</button>;

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
          <button type="button" data-assistant-conversation-history="" aria-expanded={listOpen}
            disabled={context === undefined || unresolved || sending}
            onClick={() => setListOpen((open) => !open)}>历史对话</button>
        </header>
        <div className="assistant-conversation" aria-live="polite">
          {listOpen && context !== undefined ? <ProjectConversationList scope={context.scope} baseUrl={context.baseUrl}
            fetchImpl={context.fetchImpl} currentConversationId={reference?.conversationId} disabled={unresolved || sending}
            onSelect={(conversationId) => void openConversation(conversationId, CONVERSATION_OPEN_FAILED_SENTENCE)} />
            : null}
          {availability === "unavailable" ? (
            <p className="assistant-status">写作助手当前不可用。你仍可以直接写作。</p>
          ) : null}
          {reference?.history?.map((exchange, index, history) => (
            <section className="assistant-exchange" key={exchange.requestId} data-assistant-history-run={exchange.runId}>
              {earlierHidden(exchange.runId)}
              {index > 0 && history[index - 1]?.conversationId !== exchange.conversationId
                ? <p className="assistant-conversation-boundary">新对话</p> : null}
              <AuthorMessages message={exchange.message} labels={exchange.referenceLabels}
                skills={exchange.skillNames} run={exchange.run} guidance={exchange.guidance} />
              {/* The button "用到故事里" stays with the current Run, because it sends in the current Conversation. */}
              {exchange.run !== undefined && runResults(exchange.run) !== undefined
                ? <RunResultList run={exchange.run} results={runResults(exchange.run)!} context={context} history
                  tree={context?.tree} sourceChapterId={exchange.chapterId ?? reference.chapterId}
                  onNavigate={onNavigateProposal} />
                : <>
                  {exchange.result === undefined ? null : <p className="assistant-result">{exchange.result}</p>}
                  {exchange.run === undefined ? null
                    : <><SkillUseList run={exchange.run} /><ManuscriptUseList run={exchange.run} /></>}
                  {exchange.run === undefined || context === undefined || !researchDecision(exchange.run) ? null
                    : <ResearchTurn context={context} run={exchange.run} history />}
                  {exchange.run?.decision.kind !== "prose_change" ? null
                    : <ProposalLocationLinks locations={exchange.run.decision.locations ?? []} tree={context?.tree}
                      sourceChapterId={exchange.chapterId ?? reference.chapterId} onNavigate={onNavigateProposal} />}
                </>}
              {earlierExpired(exchange.runId)}
            </section>
          ))}
          {reference === undefined ? null : (
            <section className="assistant-exchange" data-assistant-current-run="">
              {earlierHidden(reference.runId)}
              {(reference.history?.length ?? 0) > 0
                && reference.history?.at(-1)?.conversationId !== reference.conversationId
                ? <p className="assistant-conversation-boundary">新对话</p> : null}
              <AuthorMessages message={reference.message} labels={reference.referenceLabels}
                skills={reference.skillNames} run={run} guidance={reference.guidance} />
              {!terminal ? <p data-assistant-run-status="">{run === undefined ? "请求结果待确认" : runLabels[run.status]}</p> : null}
              {run === undefined ? null : runResults(run) !== undefined
                ? <RunResultList run={run} results={runResults(run)!} context={context} history={false} tree={context?.tree}
                  sourceChapterId={reference.chapterId} onNavigate={onNavigateProposal}>{useResearch}</RunResultList>
                : <><p className="assistant-result" data-assistant-result="">{resultText(run) ?? "结果尚未生成。"}</p>
                  <SkillUseList run={run} />
                  <ManuscriptUseList run={run} />
                  {context === undefined ? null : <ResearchTurn context={context} run={run}>{useResearch}</ResearchTurn>}
                  {run.decision.kind !== "prose_change" ? null : <ProposalLocationLinks locations={run.decision.locations ?? []}
                    tree={context?.tree} sourceChapterId={reference.chapterId} onNavigate={onNavigateProposal} />}</>}
              {!terminal ? <button type="button" data-assistant-inspect="" onClick={() => {
                void inspectResult();
              }}>检查结果</button> : null}
              {resultUnavailable ? <p role="status">请求结果暂不可读取，请稍后检查。</p> : null}
              {earlierExpired(reference.runId)}
            </section>
          )}
          {reference?.conversationChoice === "new"
            ? <p className="assistant-status">下一条消息将开始新对话。</p> : null}
          {status.length > 0 ? <p role="status">{status}</p> : null}
        </div>
        <AssistantComposer key={context === undefined ? "none" : `${context.scope.owner_user_id}:${context.scope.project_id}`}
          context={context} available={availability === "available"} sending={sending} current={reference} run={run}
          skills={skills}
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
