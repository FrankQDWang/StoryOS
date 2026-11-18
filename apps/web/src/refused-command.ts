import { StoryOSProtocolError } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

/**
 * Whether `error` is a refusal before Admission (status 422) with the code `code`: a text above the Context item
 * limit (ADR 0052), or a refused Command Challenge. The Server gives both only with the status 422.
 */
export function refusedWith(error: unknown, code: "instruction_over_item_limit" | "challenge_invalid"): boolean {
  if (!(error instanceof StoryOSProtocolError) || error.status !== 422 || typeof error.responseBody !== "string") {
    return false;
  }
  try {
    return (JSON.parse(error.responseBody) as { code?: unknown }).code === code;
  } catch {
    return false;
  }
}

/**
 * Whether the Server refused the Command Challenge of a command. The Server gives the same dead nonce again for the
 * same idempotency key (ADR 0038), so the next try must be a new flight.
 */
export const challengeRefused = (error: unknown): boolean => refusedWith(error, "challenge_invalid");

/** Gives the result of `send`. When the Server refuses its Command Challenge, `forget` removes the flight. */
export async function forgettingRefusedFlight<Result>(
  send: () => Promise<Result>,
  forget: () => void,
): Promise<Result> {
  try {
    return await send();
  } catch (error) {
    if (challengeRefused(error)) forget();
    throw error;
  }
}
