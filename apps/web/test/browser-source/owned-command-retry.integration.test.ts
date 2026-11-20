import { expect, it } from "vitest";

import { deleteOwnedChapter } from "../../src/delete-chapter.ts";
import { deleteOwnedVolume } from "../../src/delete-volume.ts";
import { setOwnedCurrentChapter } from "../../src/set-current-chapter.ts";
import { takeOverOwnedProjectWriter } from "../../src/take-over-writer.ts";
import { historicalProblem } from "./historical-acknowledgement-fixture.ts";
import { jsonResponse } from "./scenario.ts";

// Three command modules have a retry rule that is not the shared rule of sendOwnedCommand (#1720).
const PROJECT = "018f0000-0000-7001-8000-000000172001";
const CHAPTER = "018f0000-0000-7001-8000-000000172002";
const VOLUME = "018f0000-0000-7001-8000-000000172003";
const SESSION = "018f0000-0000-7001-8000-000000172004";
const REVISION = "018f0000-0000-7001-8000-000000172005";
const ACCEPTED = { accepted: true };
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

type Exchange = { step: "challenge" | "command"; key: string | null; nonce: string | null };

/** A Challenge rate limit that tells the client to wait 0 seconds. */
const rateLimited = () => new Response(JSON.stringify({ schema_id: "storyos.problem.v1",
  code: "challenge_rate_limited", message: "Too many Challenges." }),
{ status: 429, headers: { "content-type": "application/json", "retry-after": "0" } });

/** As the Server: each Challenge gets the next nonce, and each command gets the next answer of `answers`. */
function fakeServer(answers: (() => Response)[]) {
  const sent: Exchange[] = [];
  let nonces = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      const challenge = JSON.parse(String(init?.body)) as { idempotency_key: string };
      sent.push({ step: "challenge", key: challenge.idempotency_key, nonce: null });
      nonces += 1;
      return jsonResponse({ nonce: String.fromCharCode(96 + nonces).repeat(64),
        expires_at: "2026-11-20T12:05:00.000Z", limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    const headers = new Headers(init?.headers);
    sent.push({ step: "command", key: headers.get("idempotency-key"), nonce: headers.get("x-storyos-anti-forgery") });
    const answer = answers.shift();
    if (answer === undefined) throw new Error(`Unexpected command ${path}`);
    return answer();
  };
  return { sent, fetchImpl };
}

const connection = (projectId: string, fetchImpl: typeof fetch) =>
  ({ baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, projectId });

it.each([
  ["Delete Chapter", (fetchImpl: typeof fetch) => deleteOwnedChapter({
    ...connection("018f0000-0000-7001-8000-000000172011", fetchImpl), chapterId: CHAPTER, expectedTreeRevision: "4" })],
  ["Delete Volume", (fetchImpl: typeof fetch) => deleteOwnedVolume({
    ...connection("018f0000-0000-7001-8000-000000172012", fetchImpl), volumeId: VOLUME, expectedTreeRevision: "4" })],
])("waits after a Challenge rate limit of %s and retries with the same key and a new Challenge", async (_, send) => {
  const server = fakeServer([rateLimited, () => jsonResponse(ACCEPTED)]);

  expect(await send(server.fetchImpl)).toEqual(ACCEPTED);
  const key = server.sent[0]!.key;
  expect(key).toMatch(UUID_V7);
  expect(server.sent).toEqual([
    { step: "challenge", key, nonce: null },
    { step: "command", key, nonce: "a".repeat(64) },
    { step: "challenge", key, nonce: null },
    { step: "command", key, nonce: "b".repeat(64) },
  ]);
});

it("does not retry Set Current Chapter after a Challenge rate limit, and the next call keeps the key and the nonce", async () => {
  const server = fakeServer([rateLimited, () => jsonResponse(ACCEPTED)]);
  const send = () => setOwnedCurrentChapter({ ...connection(PROJECT, server.fetchImpl), chapterId: CHAPTER,
    expectedCurrentChapterId: VOLUME, expectedTargetRevisionId: REVISION, editorSessionId: SESSION });

  await expect(send()).rejects.toMatchObject({ status: 429 });
  expect(server.sent).toHaveLength(2);
  expect(await send()).toEqual(ACCEPTED);
  const key = server.sent[0]!.key;
  expect(key).toMatch(UUID_V7);
  expect(server.sent).toEqual([
    { step: "challenge", key, nonce: null },
    { step: "command", key, nonce: "a".repeat(64) },
    { step: "command", key, nonce: "a".repeat(64) },
  ]);
});

it("retries a Takeover one time with the same key and nonce after historical_acknowledgement_unavailable", async () => {
  const server = fakeServer([() => historicalProblem("The original Takeover acknowledgement cannot be recovered."),
    () => jsonResponse(ACCEPTED)]);

  expect(await takeOverOwnedProjectWriter({ ...connection(PROJECT, server.fetchImpl), editorSessionId: SESSION,
    observedWriterGeneration: "1" })).toEqual(ACCEPTED);
  const key = server.sent[0]!.key;
  expect(key).toMatch(UUID_V7);
  expect(server.sent).toEqual([
    { step: "challenge", key, nonce: null },
    { step: "command", key, nonce: "a".repeat(64) },
    { step: "command", key, nonce: "a".repeat(64) },
  ]);
});
