import type { GetAgentRunResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

/** The sentence for a request whose result did not arrive. The author approved it (ADR 0057). */
export const NO_RESULT_SENTENCE = "没有收到这次的结果，正文没有改动。请开始新对话重试。";

/** The sentence for a request after one without a result. The author approved it (ADR 0057). */
export const PREVIOUS_REQUEST_UNRESOLVED_SENTENCE = "上一条请求还没有结果，请开始新对话。";

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
