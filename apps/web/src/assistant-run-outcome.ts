import type { GetAgentRunResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

/** The sentence for a request whose result did not arrive. The author approved it (ADR 0057). */
export const NO_RESULT_SENTENCE = "没有收到这次的结果，正文没有改动。请开始新对话重试。";

/** The sentence for a request after one without a result. The author approved it (ADR 0057). */
export const PREVIOUS_REQUEST_UNRESOLVED_SENTENCE = "上一条请求还没有结果，请开始新对话。";

/** The sentence for a request that the model destination refused. The author approved it (#1110). */
export const DESTINATION_REFUSED_SENTENCE = "模型服务拒绝了这次请求，正文没有改动。你可以重试，或继续手动写作。";

/** The sentence for a conversation that is too long for the model (ADR 0056). */
export const CONVERSATION_TOO_LONG_SENTENCE = "这段对话已经太长，请开始一个新对话。你可以继续手动写作。";

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
