import { StoryOSProtocolError } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export function historicalAcknowledgementUnavailable(error: unknown): boolean {
  if (!(error instanceof StoryOSProtocolError) || error.status !== 409 || typeof error.responseBody !== "string") {
    return false;
  }
  try {
    const problem = JSON.parse(error.responseBody) as { code?: string };
    return problem.code === "historical_acknowledgement_unavailable";
  } catch {
    return false;
  }
}

export const HISTORICAL_ACKNOWLEDGEMENT_MESSAGE = "原始回复无法恢复。请刷新后查看当前结果。";
