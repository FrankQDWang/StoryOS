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

/** Whether the Server refused a Command Challenge. The same idempotency key gets the refused nonce again (ADR 0038). */
export const challengeRefused = (error: unknown): boolean => refusedWith(error, "challenge_invalid");

/** Removes `flight` from `flights`. A newer flight of the same intent stays. */
export function forgetFlight<Flight>(flights: Map<string, Flight>, identity: string, flight: Flight): void {
  if (flights.get(identity) === flight) flights.delete(identity);
}

/** Gives the result of `send`. When the Server refuses its Command Challenge, `flight` is forgotten. */
export async function forgettingRefusedFlight<Flight, Result>(
  flights: Map<string, Flight>,
  identity: string,
  flight: Flight,
  send: () => Promise<Result>,
): Promise<Result> {
  try {
    return await send();
  } catch (error) {
    if (challengeRefused(error)) forgetFlight(flights, identity, flight);
    throw error;
  }
}
