import { afterEach, expect, it, vi } from "vitest";

import { digestDeleteChapter, digestDeleteVolume } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DeleteChapterRequest, DeleteChapterResponse, DeleteVolumeRequest, DeleteVolumeResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { deleteOwnedChapter } from "../../src/delete-chapter.ts";
import { deleteOwnedVolume } from "../../src/delete-volume.ts";
import { jsonResponse, OWNER } from "./scenario.ts";

// Delete Chapter and Delete Volume wait after a Challenge rate limit before their one retry (#1720).
const CHAPTER = "018f0000-0000-7001-8000-000000172002";
const VOLUME = "018f0000-0000-7001-8000-000000172003";
const NONCE = "a".repeat(64);
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

type Exchange =
  | { step: "challenge" | "command"; key: string | null; nonce: string | null }
  | { step: "wait"; timeout: number | undefined };

type Kind = "chapter" | "volume";
type Delete = DeleteChapterRequest | DeleteVolumeRequest;

/** The applied response of the Server (`crates/storyos-server/src/delete_chapter.rs:85`, `delete_volume.rs:82`). */
async function applied(kind: Kind, projectId: string, request: Delete, key: string) {
  const ids = { command_id: "018f0000-0000-7001-8000-000000172021",
    author_command_admission_id: "018f0000-0000-7001-8000-000000172022" };
  const scope = { owner_user_id: OWNER, project_id: projectId };
  const input = "delete_chapter_input" in request ? request.delete_chapter_input : request.delete_volume_input;
  const response = {
    schema_id: `storyos.command.delete-${kind}.response.v1`, correlation_id: input.correlation_id,
    project_scope: scope, ...ids,
    receipt: { receipt_id: "018f0000-0000-7001-8000-000000172023", project_scope: scope,
      command_kind: kind === "chapter" ? "deleteChapter" as const : "deleteVolume" as const,
      command_digest: kind === "chapter"
        ? await digestDeleteChapter(request as DeleteChapterRequest, crypto)
        : await digestDeleteVolume(request as DeleteVolumeRequest, crypto),
      idempotency_key: key, producer_cause: "author_command_admission" as const,
      author_command_admission_id: ids.author_command_admission_id, expected_heads: [], prior_heads: [],
      resulting_heads: [], authoritative_revision_ids: [], proposal_revision_ids: [], authoritative_commit_ids: [],
      author_action_sequence: null, draft_artifact_refs: [], artifact_lifecycle_event_refs: [], condition_refs: [],
      result: "authoritative_applied" as const, created_at: "2026-11-20T12:00:00.000Z" },
    project: { project_id: projectId, title: "Project", open: { kind: "empty" as const } },
  };
  return kind === "chapter"
    ? { ...response, effect: { kind: "authoritative_applied", chapter_id: CHAPTER, volume_id: VOLUME,
      tree_revision: "5", project_activity_position: "5" } } satisfies DeleteChapterResponse
    : { ...response, effect: { kind: "authoritative_applied", volume_id: VOLUME, tree_revision: "5",
      project_activity_position: "5" } } satisfies DeleteVolumeResponse;
}

/** As the Server (ADR 0076): the first Challenge gets a rate limit with Retry-After 1, and the next Challenge a nonce. */
function fakeServer(kind: Kind, projectId: string) {
  const sent: Exchange[] = [];
  let challenges = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      const challenge = JSON.parse(String(init?.body)) as { idempotency_key: string };
      sent.push({ step: "challenge", key: challenge.idempotency_key, nonce: null });
      challenges += 1;
      if (challenges === 1) {
        // `crates/storyos-server/src/lib.rs:936`: status 429, Retry-After, and the problem `challenge_rate_limited`.
        return new Response(JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
          message: "The command challenge rate limit is exceeded." }),
        { status: 429, headers: { "content-type": "application/json", "retry-after": "1" } });
      }
      // `crates/storyos-server/src/project_command_challenge.rs:142`.
      return jsonResponse({ nonce: NONCE, expires_at: "2026-11-20T12:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    const headers = new Headers(init?.headers);
    const key = headers.get("idempotency-key");
    sent.push({ step: "command", key, nonce: headers.get("x-storyos-anti-forgery") });
    return jsonResponse(await applied(kind, projectId, JSON.parse(String(init?.body)) as Delete, key ?? ""));
  };
  // The fake timer records the timeout that the module asks for and ends at once, so no real time goes by.
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
  ["Delete Chapter", "chapter", "018f0000-0000-7001-8000-000000172011", (projectId: string, fetchImpl: typeof fetch) =>
    deleteOwnedChapter({ ...connection(projectId, fetchImpl), chapterId: CHAPTER, expectedTreeRevision: "4" })],
  ["Delete Volume", "volume", "018f0000-0000-7001-8000-000000172012", (projectId: string, fetchImpl: typeof fetch) =>
    deleteOwnedVolume({ ...connection(projectId, fetchImpl), volumeId: VOLUME, expectedTreeRevision: "4" })],
] as const)("waits Retry-After and one second after a Challenge rate limit of %s, then retries with the same key",
  async (_, kind, projectId, send) => {
    const server = fakeServer(kind, projectId);

    const response = await send(projectId, server.fetchImpl);
    const key = (server.sent[0] as { key: string | null }).key;
    expect(key).toMatch(UUID_V7);
    expect({ effect: response.effect.kind, sent: server.sent }).toEqual({ effect: "authoritative_applied", sent: [
      { step: "challenge", key, nonce: null },
      { step: "wait", timeout: 2_000 },
      { step: "challenge", key, nonce: null },
      { step: "command", key, nonce: NONCE },
    ] });
  });
