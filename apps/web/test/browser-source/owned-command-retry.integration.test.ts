import { afterEach, expect, it, vi } from "vitest";

import { deleteOwnedChapter } from "../../src/delete-chapter.ts";
import { deleteOwnedVolume } from "../../src/delete-volume.ts";
import { jsonResponse } from "./scenario.ts";

// Delete Chapter and Delete Volume wait after a Challenge rate limit before their one retry (#1720).
const CHAPTER = "018f0000-0000-7001-8000-000000172002";
const VOLUME = "018f0000-0000-7001-8000-000000172003";
const ACCEPTED = { accepted: true };
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

type Exchange =
  | { step: "challenge" | "command"; key: string | null; nonce: string | null }
  | { step: "wait"; timeout: number | undefined };

/** As the Server (ADR 0076): the first Challenge gets a rate limit with Retry-After 1, and the next Challenge a nonce. */
function fakeServer() {
  const sent: Exchange[] = [];
  let challenges = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      const challenge = JSON.parse(String(init?.body)) as { idempotency_key: string };
      sent.push({ step: "challenge", key: challenge.idempotency_key, nonce: null });
      challenges += 1;
      if (challenges === 1) {
        return new Response(JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
          message: "The command challenge rate limit is exceeded." }),
        { status: 429, headers: { "content-type": "application/json", "retry-after": "1" } });
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-11-20T12:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    const headers = new Headers(init?.headers);
    sent.push({ step: "command", key: headers.get("idempotency-key"), nonce: headers.get("x-storyos-anti-forgery") });
    return jsonResponse(ACCEPTED);
  };
  // Each wait is recorded with its timeout and then ends at once, so the test does not wait 2 seconds.
  const realSetTimeout = globalThis.setTimeout;
  vi.spyOn(globalThis, "setTimeout").mockImplementation(((callback: () => void, timeout?: number) => {
    sent.push({ step: "wait", timeout });
    return realSetTimeout(callback, 0);
  }) as typeof setTimeout);
  return { sent, fetchImpl };
}

afterEach(() => {
  vi.restoreAllMocks();
});

const connection = (projectId: string, fetchImpl: typeof fetch) =>
  ({ baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, projectId });

it.each([
  ["Delete Chapter", (fetchImpl: typeof fetch) => deleteOwnedChapter({
    ...connection("018f0000-0000-7001-8000-000000172011", fetchImpl), chapterId: CHAPTER, expectedTreeRevision: "4" })],
  ["Delete Volume", (fetchImpl: typeof fetch) => deleteOwnedVolume({
    ...connection("018f0000-0000-7001-8000-000000172012", fetchImpl), volumeId: VOLUME, expectedTreeRevision: "4" })],
])("waits Retry-After and one second after a Challenge rate limit of %s, then retries with the same key", async (_, send) => {
  const server = fakeServer();

  expect(await send(server.fetchImpl)).toEqual(ACCEPTED);
  const key = (server.sent[0] as { key: string | null }).key;
  expect(key).toMatch(UUID_V7);
  expect(server.sent).toEqual([
    { step: "challenge", key, nonce: null },
    { step: "wait", timeout: 2_000 },
    { step: "challenge", key, nonce: null },
    { step: "command", key, nonce: "a".repeat(64) },
  ]);
});
