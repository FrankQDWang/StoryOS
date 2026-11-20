import { afterEach, expect, it, vi } from "vitest";

import { digestDeleteChapter, digestDeleteVolume }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DeleteChapterRequest, DeleteChapterResponse, DeleteVolumeRequest, DeleteVolumeResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { deleteOwnedChapter } from "../../src/delete-chapter.ts";
import { deleteOwnedVolume } from "../../src/delete-volume.ts";
import { jsonResponse, OWNER } from "./scenario.ts";
import { CHALLENGE_ROUTE, type Challenge, contentTypeRefusal, identityRefusal, isCatalogRoute, problem, readChallenge,
  shapeRefusal, UUID_V7, webFallback } from "./project-challenge-route.ts";

// Delete Chapter and Delete Volume wait after a Challenge rate limit before their one retry (#1720).
const CHAPTER = "018f0000-0000-7001-8000-000000172002";
const VOLUME = "018f0000-0000-7001-8000-000000172003";
const OTHER_CHAPTER = "018f0000-0000-7001-8000-000000172004";
const COMMIT = "018f0000-0000-7001-8000-000000172024";
const NONCE = "a".repeat(64);

type Exchange =
  | { step: "challenge" | "command"; key: string | null; nonce: string | null }
  | { step: "wait"; timeout: number | undefined };

type Kind = "chapter" | "volume";
type Delete = DeleteChapterRequest | DeleteVolumeRequest;
const digest = (kind: Kind, request: Delete) => kind === "chapter"
  ? digestDeleteChapter(request as DeleteChapterRequest, crypto)
  : digestDeleteVolume(request as DeleteVolumeRequest, crypto);

/**
 * The problem of `crates/storyos-server/src/command_admission.rs:576` for a command that does not match its Challenge.
 * The Server uses the Challenge before it reads the Project (`storyos-adapter-postgres/src/command_sequence/skeleton.rs:55`),
 * and the nonce binds the Project text of the Challenge (`project_command_challenge.rs:150`), so a command on the path
 * of another Project, or of the same Project in another spelling, also gets this problem.
 */
const challengeInvalid = (kind: Kind) => problem("challenge_invalid",
  `The ${kind === "chapter" ? "Delete Chapter" : "Delete Volume"} challenge is invalid.`, 422);
/** The route of the Delete command of `kind`, with the Project and the target segments. */
const deleteRoute = (kind: Kind) => kind === "chapter"
  ? /^\/api\/v1\/projects\/([^/]+)\/chapters\/([^/]+)$/ : /^\/api\/v1\/projects\/([^/]+)\/volumes\/([^/]+)$/;

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

/**
 * As the Server (ADR 0076): the Challenge route gives each refusal of `readChallenge`. Of the other Challenges, the
 * first gets a rate limit with Retry-After 1, and the next a nonce.
 * The Delete route refuses a path identity that is not a UUID and a request with no JSON content type, before the
 * body (`command_admission.rs:274`). A command is applied only when its nonce, key, method, route, schema, digest,
 * and Project text match the Challenge of the nonce. A path that is no route gets the Web fallback.
 */
function fakeServer(kind: Kind, projectId: string) {
  const sent: Exchange[] = [];
  const issued = new Map<string, { challenge: Challenge; projectText: string }>();
  let challenges = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const challengeProject = CHALLENGE_ROUTE.exec(path)?.[1];
    if (challengeProject !== undefined) {
      const read = readChallenge(challengeProject, projectId, init);
      if ("refusal" in read) return read.refusal;
      sent.push({ step: "challenge", key: read.challenge.idempotency_key, nonce: null });
      challenges += 1;
      if (challenges === 1) {
        // `crates/storyos-server/src/lib.rs:926`: status 429, Retry-After, and the problem `challenge_rate_limited`.
        return new Response(JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
          message: "The command challenge rate limit is exceeded." }),
        { status: 429, headers: { "content-type": "application/json", "retry-after": "1" } });
      }
      issued.set(NONCE, { challenge: read.challenge, projectText: challengeProject });
      // `crates/storyos-server/src/project_command_challenge.rs:142`.
      return jsonResponse({ nonce: NONCE, expires_at: "2026-11-20T12:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    const [, pathProject, target] = deleteRoute(kind).exec(path) ?? [];
    if (pathProject === undefined || target === undefined || init?.method !== "DELETE") {
      if (!isCatalogRoute(path)) return webFallback();
      throw new Error(`The fake Server does not serve ${init?.method ?? "GET"} ${path}`);
    }
    const refusal = identityRefusal(pathProject, target) ?? contentTypeRefusal(init);
    if (refusal !== undefined) return refusal;
    let request: Delete;
    try { request = JSON.parse(String(init?.body)) as Delete; } catch { return shapeRefusal(); }
    const headers = new Headers(init?.headers);
    const key = headers.get("idempotency-key");
    const nonce = headers.get("x-storyos-anti-forgery");
    sent.push({ step: "command", key, nonce });
    const issue = nonce === null ? undefined : issued.get(nonce);
    const binding = issue?.challenge;
    if (binding === undefined || issue?.projectText !== pathProject || binding.idempotency_key !== key
      || binding.method !== init.method || binding.route_template !== `/api/v1/projects/{project_id}/${kind}s/{${kind}_id}`
      || binding.command_schema !== request.command_schema
      || JSON.stringify(binding.canonical_command_digest) !== JSON.stringify(await digest(kind, request))) {
      return challengeInvalid(kind);
    }
    return jsonResponse(await applied(kind, projectId, request, key ?? ""));
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

// Each request of the module, sent again with one change that the Server refuses.
it("refuses each Challenge request and each Delete command that the Server refuses", async () => {
  const projectId = "018f0000-0000-7001-8000-000000172011";
  const server = fakeServer("chapter", projectId);
  const calls: { url: string; init: RequestInit }[] = [];
  const recording: typeof fetch = (input, init) => {
    calls.push({ url: input instanceof Request ? input.url : String(input), init: init ?? {} });
    return server.fetchImpl(input, init);
  };
  await deleteOwnedChapter({ ...connection(projectId, recording), chapterId: CHAPTER, expectedTreeRevision: "4" });
  const challenge = calls.find(({ url }) => CHALLENGE_ROUTE.test(new URL(url).pathname));
  const command = calls.at(-1);
  if (challenge === undefined || command === undefined) throw new Error("The module sent no Challenge or command");
  const body = JSON.parse(String(challenge.init.body)) as Challenge;
  const at = (url: string, project: string) => url.replace(projectId, project);
  const send = async (url: string, init: RequestInit) => {
    const response = await server.fetchImpl(url, init);
    return { status: response.status, allow: response.headers.get("allow"), body: await response.text() };
  };
  const refused = (status: number, code: string, message: string) =>
    ({ status, allow: null, body: JSON.stringify({ schema_id: "storyos.problem.v1", code, message }) });
  const identity = refused(400, "invalid_request", "The request identity is invalid.");

  expect({
    challengeGet: await send(challenge.url, { method: "GET" }),
    challengeOfInvalidProject: await send(at(challenge.url, "project"), challenge.init),
    challengeOfUrnWithoutHyphens: await send(at(challenge.url, `urn:uuid:${projectId.replaceAll("-", "")}`),
      challenge.init),
    challengeWithoutContentType: await send(challenge.url, { ...challenge.init, headers: { accept: "application/json" } }),
    challengeWithExtraField: await send(challenge.url, { ...challenge.init,
      body: JSON.stringify({ ...body, extra: "field" }) }),
    challengeOfOtherProject: await send(at(challenge.url, "018f0000-0000-7001-8000-000000172013"), challenge.init),
    pathThatIsNoRoute: await send(`${challenge.url}/extra`, challenge.init),
    commandOfOtherProject: await send(at(command.url, "018f0000-0000-7001-8000-000000172013"), command.init),
    commandOfInvalidChapter: await send(command.url.replace(CHAPTER, "chapter"), command.init),
    commandOfProject: (await send(command.url, command.init)).status,
    // The Server reads the Project by its UUID, so another spelling of the same Project gets a nonce.
    challengeOfUppercaseProject: (await send(at(challenge.url, projectId.toUpperCase()), challenge.init)).status,
  }).toEqual({
    challengeGet: { status: 405, allow: "POST", body: "" },
    challengeOfInvalidProject: identity,
    challengeOfUrnWithoutHyphens: identity,
    challengeWithoutContentType: refused(415, "unsupported_content_type", "The request must use one JSON content type."),
    challengeWithExtraField: refused(422, "invalid_request_shape", "The request does not match its closed schema."),
    challengeOfOtherProject: refused(404, "resource_unavailable", "The requested resource is unavailable."),
    pathThatIsNoRoute: { status: 404, allow: null, body: "" },
    commandOfOtherProject: refused(422, "challenge_invalid", "The Delete Chapter challenge is invalid."),
    commandOfInvalidChapter: identity,
    commandOfProject: 200,
    challengeOfUppercaseProject: 200,
  });
});
