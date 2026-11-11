import type {
  GetAgentRunResponse, ListResearchSourcesResponse, ResearchArtifactInspect, SourceGapInspect, SourceGapReason, ToolCallInspect,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

/** The sentence for a request whose result did not arrive. The author approved it (ADR 0057). */
export const NO_RESULT_SENTENCE = "没有收到这次的结果，正文没有改动。请开始新对话重试。";

/** The sentence for a request after one without a result. The author approved it (ADR 0057). */
export const PREVIOUS_REQUEST_UNRESOLVED_SENTENCE = "上一条请求还没有结果，请开始新对话。";

/** The sentence for a request that the model destination refused. The author approved it (#1110). */
export const DESTINATION_REFUSED_SENTENCE = "模型服务拒绝了这次请求，正文没有改动。你可以重试，或继续手动写作。";

/** The sentence for a conversation that is too long for the model (ADR 0056). */
export const CONVERSATION_TOO_LONG_SENTENCE = "这段对话已经太长，请开始一个新对话。你可以继续手动写作。";

/** The sentence for a research request that no Tool could execute now: the Project does not enable the research
 * Tool now, or its Registration is not available now (#1426). */
export const RESEARCH_TOOL_UNAVAILABLE_SENTENCE = "联网检索现在不可用，这次没有检索，正文没有改动。你可以重试，或继续手动写作。";

/** The sentence for a research request of the model that StoryOS could not accept (#1426). */
export const TOOL_REQUEST_INVALID_SENTENCE = "模型发出的联网检索请求不符合要求，这次没有检索，正文没有改动。你可以重试，或继续手动写作。";

/** The line of a research call that did not give a result (ADR 0040). */
export const TOOL_CALL_FAILED_SENTENCE = "有一次联网检索没有完成。";

/** One line for each research call of a function request, in order, with no identifier (ADR 0040).
 * A call with an unknown outcome may have run, and its line says so (ADR 0086). */
export function toolCallLines(calls: readonly ToolCallInspect[]): string {
  return calls.map(toolCallLine).join("\n");
}

function toolCallLine(call: ToolCallInspect): string {
  if (call.outcome === "failed") return TOOL_CALL_FAILED_SENTENCE;
  if (call.outcome === "unknown") {
    return call.kind === "search"
      ? `可能已联网搜索，但没有收到结果：${call.query}`
      : `可能已阅读，但没有收到结果：${call.urls.join("、")}`;
  }
  const done = call.outcome === "settled";
  return call.kind === "search"
    ? `${done ? "已联网搜索" : "正在联网搜索"}：${call.query}`
    : `${done ? "已阅读" : "正在阅读"}：${call.urls.join("、")}`;
}

/** The words for a requested page that StoryOS could not capture (ADR 0040). */
export const SOURCE_NOT_AVAILABLE = "无法获取";

/** The reason words of a Source Gap. The source list and the citation list use the same words. */
const gapReasons: Record<SourceGapReason, string> = {
  failed: "读取失败",
  empty: "网页没有文字",
  missing: "检索结果中没有这个网页",
  unattributed: "无法确认内容来自这个网页",
};

/** The line of a page that is not available: its URL, "not available", and the reason (ADR 0040). */
function gapLine(gap: SourceGapInspect): string {
  return `${gap.locator}：${SOURCE_NOT_AVAILABLE}（${gapReasons[gap.reason]}）`;
}

/** One line for each research source of a Run: the title or the URL with the capture time, then
 * each page that is not available with its reason, with no identifier (ADR 0040). Then each call
 * that may have run follows, except a call of the function request `shownDecisionId` that the
 * panel shows with its calls (ADR 0086). */
export function researchSourceLines(
  sources: ListResearchSourcesResponse, shownDecisionId: string | undefined,
): string[] {
  const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
  return [
    ...sources.snapshots.map(snapshot =>
      `${snapshot.title ?? snapshot.locator}（获取于 ${time.format(new Date(snapshot.captured_at))}）`),
    ...sources.gaps.map(gapLine),
    ...sources.unknown_calls
      .filter(unknown => unknown.decision_id !== shownDecisionId)
      .map(unknown => toolCallLine(unknown.call)),
  ];
}

/** The result sentence of a research synthesis. The claims and their sources follow it. */
export const RESEARCH_SYNTHESIS_SENTENCE = "研究结果如下。每条结论后的编号对应下方的来源。";

/** The message of the "Use this in the story" button: an ordinary request in the same conversation. */
export const USE_RESEARCH_MESSAGE = "请根据上面的研究结果修改当前章节的正文。";

/** The text of one Research Artifact with no identifier (ADR 0040, ADR 0087): each claim with the
 * numbers of its citations, and each numbered citation as the title or URL of its Snapshot with
 * the capture time, or as the URL of its Gap with "not available" and the reason. */
export function researchArtifactLines(artifact: ResearchArtifactInspect): { claims: string[]; citations: string[] } {
  const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
  return {
    claims: artifact.claims.map(claim => `${claim.text} ${claim.citations.map(number => `[${number}]`).join("")}`),
    citations: artifact.citations.map((citation, index) => citation.kind === "snapshot"
      ? `[${index + 1}] ${citation.snapshot.title ?? citation.snapshot.locator}（获取于 ${time.format(new Date(citation.snapshot.captured_at))}）`
      : `[${index + 1}] ${gapLine(citation.gap)}`),
  };
}

/** The sentence for a model reply that looks like a structured reply but does not validate (#1145). */
export const MALFORMED_REPLY_SENTENCE = "模型这次的回复格式不正确，正文没有改动。你可以重试，或继续手动写作。";

/**
 * An Advisory text that is a malformed structured reply. The function removes a Markdown code
 * fence, also a fence without its end. Then the text starts as an object with a `kind` key. A
 * valid reply does not arrive here as an Advisory text. The text can be invalid or incomplete JSON.
 */
export function malformedStructuredReply(text: string): boolean {
  const reply = text.trim().replace(/^```(?:json)?/, "").replace(/```$/, "").trim();
  return reply.startsWith("{") && reply.includes('"kind"');
}

/**
 * A request whose recovery stopped without a result: an unknown outcome that waits, a paused
 * successor, or a successor that also gave no result. Nothing changes it later.
 */
export function stoppedWithoutResult(run: GetAgentRunResponse): boolean {
  if (run.decision.kind !== "absent") return false;
  switch (run.status) {
    case "waiting":
      return true;
    case "paused":
      return run.unknown_create_successor.kind === "present";
    case "completed":
      return run.unknown_create_successor.kind === "present";
    case "queued":
    case "claimed":
    case "refused":
    case "cancelled":
      return false;
  }
}

/** A Run that changes no more by itself. */
export function settledRun(run: GetAgentRunResponse): boolean {
  return run.status === "completed" || run.status === "refused" || run.status === "cancelled"
    || stoppedWithoutResult(run);
}
