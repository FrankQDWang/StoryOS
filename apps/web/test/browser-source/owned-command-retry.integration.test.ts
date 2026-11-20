import { afterEach, expect, it, vi } from "vitest";

import { digestDeleteChapter, digestDeleteVolume }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DeleteChapterRequest, DeleteChapterResponse, DeleteVolumeRequest, DeleteVolumeResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { deleteOwnedChapter } from "../../src/delete-chapter.ts";
import { deleteOwnedVolume } from "../../src/delete-volume.ts";
import { jsonResponse, OWNER } from "./scenario.ts";

// Delete Chapter and Delete Volume wait after a Challenge rate limit before their one retry (#1720).
const CHAPTER = "018f0000-0000-7001-8000-000000172002";
const VOLUME = "018f0000-0000-7001-8000-000000172003";
const OTHER_CHAPTER = "018f0000-0000-7001-8000-000000172004";
const COMMIT = "018f0000-0000-7001-8000-000000172024";
const NONCE = "a".repeat(64);
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

type Exchange =
  | { step: "challenge" | "command"; key: string | null; nonce: string | null }
  | { step: "wait"; timeout: number | undefined };

type Kind = "chapter" | "volume";
type Delete = DeleteChapterRequest | DeleteVolumeRequest;
type Challenge = { method: string; route_template: string; command_schema: string;
  canonical_command_digest: { algorithm: string; profile: string; value_hex_lowercase: string };
  idempotency_key: string };

// A second copy of the lookup of `refused-command-challenge.integration.test.ts:101`, kept by a coordinator decision.
const CATALOG_URL = new URL("../../../../generated/route-catalog/versioned-protocol-release-1-route-catalog.json",
  import.meta.url);
const { operations } = await (await fetch(CATALOG_URL)).json() as { operations: {
  operation_id: string; kind: string; method: string; path: string; schemas: { request: string } }[] };
/** The rows of the route catalog that are the Project command targets of the Server. */
const PROJECT_COMMANDS = operations.filter(({ kind, path }) =>
  kind === "command" && path.startsWith("/api/v1/projects/{project_id}"));

const problem = (code: string, message: string, status: number) =>
  jsonResponse({ schema_id: "storyos.problem.v1", code, message }, status);

/**
 * The refusal of the Server for a Challenge request, in the order of `project_command_challenge.rs:63`: a target that
 * is not in the route catalog (`lib.rs:908`), then a bad digest or a key that is not UUID v7 (`lib.rs:884`).
 */
function challengeRefusal(challenge: Challenge): Response | undefined {
  const target = PROJECT_COMMANDS.find((operation) => operation.method === challenge.method
    && operation.path === challenge.route_template && operation.schemas.request === challenge.command_schema);
  if (target === undefined) {
    return problem("command_target_refused", "The command target is not in the closed Release 1 contract.", 422);
  }
  const { algorithm, profile, value_hex_lowercase: value } = challenge.canonical_command_digest;
  if (algorithm !== "sha256" || profile !== `storyos.command.${target.operation_id}.jcs.v1`
    || !/^[0-9a-f]{64}$/.test(value) || !UUID_V7.test(challenge.idempotency_key)) {
    return problem("invalid_request", "The request is invalid.", 400);
  }
  return undefined;
}

const digest = (kind: Kind, request: Delete) => kind === "chapter"
  ? digestDeleteChapter(request as DeleteChapterRequest, crypto)
  : digestDeleteVolume(request as DeleteVolumeRequest, crypto);

/** The problem of `crates/storyos-server/src/lib.rs:931` for a command that does not match its Challenge. */
const challengeInvalid = () => problem("challenge_invalid", "The command challenge is invalid or expired.", 422);

/**
 * The applied response of the Server (`crates/storyos-server/src/delete_chapter.rs:85`, `delete_volume.rs:82`). The
 * receipt has the Authoritative Commit and the Author Action sequence of the settlement
 * (`crates/storyos-server/src/command_admission.rs:474`).
 */
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
      command_digest: await digest(kind, request), idempotency_key: key,
      producer_cause: "author_command_admission" as const,
      author_command_admission_id: ids.author_command_admission_id, expected_heads: [], prior_heads: [],
      resulting_heads: [], authoritative_revision_ids: [], proposal_revision_ids: [],
      authoritative_commit_ids: [COMMIT], author_action_sequence: "3", draft_artifact_refs: [],
      artifact_lifecycle_event_refs: [], condition_refs: [],
      result: "authoritative_applied" as const, created_at: "2026-11-20T12:00:00.000Z" },
    project: { project_id: projectId, title: "Project",
      open: { kind: "current_chapter" as const, current_chapter_id: OTHER_CHAPTER } },
  };
  return kind === "chapter"
    ? { ...response, effect: { kind: "authoritative_applied", chapter_id: CHAPTER, volume_id: VOLUME,
      tree_revision: "5", project_activity_position: "5" } } satisfies DeleteChapterResponse
    : { ...response, effect: { kind: "authoritative_applied", volume_id: VOLUME, tree_revision: "5",
      project_activity_position: "5" } } satisfies DeleteVolumeResponse;
}

// The fake Server serves the exact requests of the test: the method, the path with the Project of the test, and the
// Chapter or Volume of the test. It records each other request and throws, so that request fails the test.
/**
 * As the Server (ADR 0076): a Challenge request that the Server refuses gets its refusal. Of the other Challenges,
 * the first gets a rate limit with Retry-After 1, and the next a nonce.
 * A command is applied only when its nonce, key, method, route, schema, and digest match the Challenge of the nonce.
 */
function fakeServer(kind: Kind, projectId: string) {
  const sent: Exchange[] = [];
  const unserved: string[] = [];
  servers.push({ unserved });
  const target = kind === "chapter" ? `chapters/${CHAPTER}` : `volumes/${VOLUME}`;
  const issued = new Map<string, Challenge>();
  let challenges = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const route = `${init?.method ?? "GET"} ${path}`;
    if (route === `POST /api/v1/projects/${projectId}/anti-forgery-challenges`) {
      const challenge = JSON.parse(String(init?.body)) as Challenge;
      sent.push({ step: "challenge", key: challenge.idempotency_key, nonce: null });
      const refusal = challengeRefusal(challenge);
      if (refusal !== undefined) return refusal;
      challenges += 1;
      if (challenges === 1) {
        // `crates/storyos-server/src/lib.rs:936`: status 429, Retry-After, and the problem `challenge_rate_limited`.
        return new Response(JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
          message: "The command challenge rate limit is exceeded." }),
        { status: 429, headers: { "content-type": "application/json", "retry-after": "1" } });
      }
      issued.set(NONCE, challenge);
      // `crates/storyos-server/src/project_command_challenge.rs:142`.
      return jsonResponse({ nonce: NONCE, expires_at: "2026-11-20T12:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (route !== `DELETE /api/v1/projects/${projectId}/${target}`) {
      unserved.push(route);
      throw new Error(`The fake Server does not serve ${route}`);
    }
    const headers = new Headers(init?.headers);
    const key = headers.get("idempotency-key");
    const nonce = headers.get("x-storyos-anti-forgery");
    sent.push({ step: "command", key, nonce });
    const request = JSON.parse(String(init?.body)) as Delete;
    const binding = nonce === null ? undefined : issued.get(nonce);
    const template = new RegExp(`^${binding?.route_template.replace(/\{[a-z_]+\}/g, "[^/]+") ?? "$^"}$`);
    if (binding === undefined || binding.idempotency_key !== key || binding.method !== init?.method
      || !template.test(path) || binding.command_schema !== request.command_schema
      || JSON.stringify(binding.canonical_command_digest) !== JSON.stringify(await digest(kind, request))) {
      return challengeInvalid();
    }
    return jsonResponse(await applied(kind, projectId, request, key ?? ""));
  };
  // The fake timer records the timeout that the module asks for and ends at once, so no real time goes by.
  const realSetTimeout = globalThis.setTimeout;
  vi.spyOn(globalThis, "setTimeout").mockImplementation(((callback: () => void, timeout?: number) => {
    sent.push({ step: "wait", timeout });
    return realSetTimeout(callback, 0);
  }) as typeof setTimeout);
  return { sent, fetchImpl, unserved };
}

/** The fake Servers of the current test. */
const servers: { unserved: string[] }[] = [];

afterEach(() => {
  vi.restoreAllMocks();
  // The module sent only the requests that the fake Server serves.
  const unserved = servers.flatMap((server) => server.unserved);
  servers.length = 0;
  expect(unserved).toEqual([]);
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

it("does not serve a request with another method, on another Project, or for another target", async () => {
  const projectId = "018f0000-0000-7001-8000-000000172011";
  const otherProject = "018f0000-0000-7001-8000-000000172013";
  const server = fakeServer("chapter", projectId);
  const requests: [string, RequestInit][] = [
    [`/api/v1/projects/${projectId}/anti-forgery-challenges`, { method: "GET" }],
    [`/api/v1/projects/${otherProject}/anti-forgery-challenges`, { method: "POST", body: "{}" }],
    [`/api/v1/projects/${projectId}/chapters/${OTHER_CHAPTER}`, { method: "DELETE", body: "{}" }],
  ];
  for (const [path, init] of requests) {
    await expect(server.fetchImpl(`${location.origin}${path}`, init)).rejects.toThrow("The fake Server does not serve");
  }
  expect(server.unserved).toEqual([
    `GET /api/v1/projects/${projectId}/anti-forgery-challenges`,
    `POST /api/v1/projects/${otherProject}/anti-forgery-challenges`,
    `DELETE /api/v1/projects/${projectId}/chapters/${OTHER_CHAPTER}`,
  ]);
  server.unserved.length = 0;
});
