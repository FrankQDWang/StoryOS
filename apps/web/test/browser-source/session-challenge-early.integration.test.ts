import { afterEach, expect, it, vi } from "vitest";
import { digestCreateEditorSession } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateEditorSessionRequest, GetEditorSessionResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { openControlledProject, reopenControlledProject } from "../../src/boot.ts";
import type { ControlledProjectState } from "../../src/editor-types.ts";
import {
  CHAPTER, OWNER, PROJECT, SESSION, chapterRevision, createBrowserScenario, deleteJournal, jsonResponse,
} from "./scenario.ts";

// A new tab requests the Challenge of its Editor Session creation with the Chapter read (#1899).

const P = `/api/v1/projects/${PROJECT}`;
const CHAPTER_B = "018f0000-0000-7001-8000-000000000903";
const PROTOCOL = "GET /api/v1/protocol";
const LIST = "GET /api/v1/projects";
const PROJECT_READ = `GET ${P}`;
const CHAPTER_READ = `GET ${P}/chapters/${CHAPTER}`;
const CHAPTER_B_READ = `GET ${P}/chapters/${CHAPTER_B}`;
const TREE = `GET ${P}/manuscript/tree`;
const CHALLENGE = `POST ${P}/anti-forgery-challenges`;
const CREATION = `POST ${P}/editor-sessions`;
const SESSION_READ = `GET ${P}/editor-sessions/${SESSION}`;
/** The class `shared` of the Server: 10 new Challenges in each fixed window of 60 s (`ChallengeRateClass`). */
const SHARED_CAPACITY = 10;
const WINDOW_MS = 60_000;
/** A Challenge of the Server expires 5 minutes after its issue (`project_command_challenges.expires_at`). */
const CHALLENGE_LIFETIME_MS = 5 * 60_000;

type Challenge = { idempotencyKey: string; digest: string; nonce: string; expiresAt: number; used: boolean };

/**
 * A fake Server of one Project whose Current Chapter is Chapter A, or an Empty Project. It serves only the requests of
 * these tests, records each other request, and throws. It records the start and the end of each request, and each
 * response body that the client read. As on the Server, a Challenge has a new nonce, expires 5 minutes after its issue,
 * and counts in the fixed window of the class `shared`: the 11th Challenge of a window gets `429
 * challenge_rate_limited`. A creation needs the nonce of an unused, unexpired Challenge with its idempotency key and
 * the digest of its request, else `422 challenge_invalid`. Only the first created Editor Session holds the writer.
 * `hold(names)` holds the next request of each given name until `releaseOnly`. `failNext(names)` answers the next
 * request of each given name with the 503 problem of an unavailable Project store.
 */
function fakeServer({ empty = false }: { empty?: boolean } = {}) {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  const open = empty ? { kind: "empty" as const } : scenario.project.project.open;
  const log = { started: [] as string[], events: [] as string[], consumed: [] as string[],
    unexpected: [] as string[], active: new Map<string, number>() };
  const challenges: Challenge[] = [];
  const creations: { idempotencyKey: string; nonce: string }[] = [];
  let createdSessions = 0;
  let holding = new Set<string>();
  const failing = new Set<string>();
  const held: { name: string; resolve: () => void }[] = [];
  const created = (index: number): GetEditorSessionResponse => ({ ...scenario.session,
    editor_session: { ...scenario.session.editor_session,
      editor_session_id: index === 0 ? SESSION : `018f0000-0000-7001-8000-0000000009${String(index).padStart(2, "0")}` },
    writer: index === 0 ? { kind: "current_writer", writer_generation: "1" }
      : { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" } });
  const answer = async (name: string, init: RequestInit | undefined): Promise<Response> => {
    if (failing.delete(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "project_store_unavailable",
        message: "The Project store is unavailable." }, 503);
    }
    if (name === PROTOCOL) return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (name === PROJECT_READ) return jsonResponse({ ...scenario.project, project: { ...scenario.project.project, open } });
    if (name === LIST) {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000913", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: "Project A", revision: "1", open }] });
    }
    if (name === CHAPTER_READ && !empty) return jsonResponse(scenario.chapter);
    if (name === CHAPTER_B_READ && !empty) {
      return jsonResponse({ ...scenario.chapter, chapter: { chapter_id: CHAPTER_B, title: "Chapter B",
        current_revision: chapterRevision("018f0000-0000-7001-8000-000000000904", "") } });
    }
    if (name === TREE && empty) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000914", project_scope: scope, tree_revision: "1",
        snapshot: { snapshot_id: scenario.session.base_snapshot.snapshot_id, project_scope: scope }, volumes: [] });
    }
    if (name === SESSION_READ) return jsonResponse({ ...created(0), schema_id: "storyos.query.editor-session.response.v1" });
    if (name === CHALLENGE) {
      const body = JSON.parse(String(init?.body)) as { method: string; route_template: string; command_schema: string;
        canonical_command_digest: { value_hex_lowercase: string }; idempotency_key: string };
      if (body.method !== "POST" || body.route_template !== "/api/v1/projects/{project_id}/editor-sessions"
        || body.command_schema !== "storyos.command.create-editor-session.request.v1") {
        log.unexpected.push(`${name} for ${body.method} ${body.route_template}`);
        throw new Error(`unexpected Challenge: ${body.route_template}`);
      }
      const window = Math.floor(Date.now() / WINDOW_MS);
      if (challenges.filter(({ expiresAt }) =>
        Math.floor((expiresAt - CHALLENGE_LIFETIME_MS) / WINDOW_MS) === window).length >= SHARED_CAPACITY) {
        return new Response(JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
          message: "The command challenge rate limit is exceeded." }), { status: 429,
          headers: { "content-type": "application/json",
            "retry-after": String(Math.ceil(((window + 1) * WINDOW_MS - Date.now()) / 1000)) } });
      }
      const challenge = { idempotencyKey: body.idempotency_key, digest: body.canonical_command_digest.value_hex_lowercase,
        nonce: (challenges.length + 1).toString(16).padStart(64, "0"), expiresAt: Date.now() + CHALLENGE_LIFETIME_MS,
        used: false };
      challenges.push(challenge);
      return jsonResponse({ nonce: challenge.nonce, expires_at: new Date(challenge.expiresAt).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (name === CREATION) {
      const headers = new Headers(init?.headers);
      const idempotencyKey = headers.get("idempotency-key") ?? "";
      const nonce = headers.get("x-storyos-anti-forgery") ?? "";
      const request = JSON.parse(String(init?.body)) as CreateEditorSessionRequest;
      const digest = (await digestCreateEditorSession(request, crypto)).value_hex_lowercase;
      const challenge = challenges.find((entry) => entry.nonce === nonce);
      creations.push({ idempotencyKey, nonce });
      if (challenge === undefined || challenge.used || challenge.expiresAt <= Date.now()
        || challenge.idempotencyKey !== idempotencyKey || challenge.digest !== digest) {
        return jsonResponse({ schema_id: "storyos.problem.v1", code: "challenge_invalid",
          message: "The command challenge is invalid or expired." }, 422);
      }
      challenge.used = true;
      createdSessions += 1;
      return jsonResponse(created(createdSessions - 1));
    }
    log.unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    log.started.push(name);
    log.events.push(`start ${name}`);
    log.active.set(name, (log.active.get(name) ?? 0) + 1);
    try {
      if (holding.delete(name)) await new Promise<void>((resolve) => { held.push({ name, resolve }); });
      const response = await answer(name, init);
      const text = response.text.bind(response);
      response.text = async () => {
        const body = await text();
        log.consumed.push(name);
        return body;
      };
      return response;
    } finally {
      log.active.set(name, log.active.get(name)! - 1);
      log.events.push(`end ${name}`);
    }
  };
  const server = {
    scenario, log, fetchImpl, challenges, creations,
    hold(names: string[]) { holding = new Set(names); },
    failNext(names: string[]) { for (const name of names) failing.add(name); },
    releaseOnly(names: string[]) {
      for (const entry of held.filter(({ name }) => names.includes(name))) {
        held.splice(held.indexOf(entry), 1);
        entry.resolve();
      }
    },
    inFlight: () => [...log.active].filter(([, count]) => count > 0).map(([name]) => name).sort(),
    count: (name: string) => log.started.filter((entry) => entry === name).length,
    releaseAll() {
      holding = new Set();
      for (const { resolve } of held.splice(0)) resolve();
    },
  };
  servers.push(server);
  return server;
}

const servers: { releaseAll(): void }[] = [];
const opens: Promise<ControlledProjectState>[] = [];

/** Closes the Journal of each open of the test. */
async function closeOpens() {
  for (const state of await Promise.all(opens.splice(0))) {
    if (state.kind === "project-ready" && "database" in state.editor) state.editor.database.close();
  }
}

afterEach(async () => {
  vi.useRealTimers();
  // A failed test can leave held requests and an open in progress, which holds the Journal.
  for (const server of servers.splice(0)) server.releaseAll();
  await closeOpens();
  sessionStorage.clear();
  await deleteJournal(createBrowserScenario().journalName);
});

/** Opens the Project in a tab with the session storage of this test. */
function open(server: ReturnType<typeof fakeServer>): Promise<ControlledProjectState> {
  const state = openControlledProject({ baseUrl: location.origin, projectId: PROJECT,
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  opens.push(state);
  return state;
}

/** The writer of the editor of an open, or the kind of the open when it has no ready editor. */
const writerOf = (state: ControlledProjectState) => state.kind !== "project-ready" ? state.kind
  : state.editor.kind === "editor-ready" ? state.editor.session.writer.kind : state.editor.kind;

it.each([
  { order: "the Chapter read and then the Challenge", first: CHAPTER_READ, second: CHALLENGE },
  { order: "the Challenge and then the Chapter read", first: CHALLENGE, second: CHAPTER_READ },
])("opens a new tab with the Challenge in flight with the Chapter read, and creates the Editor Session after"
  + " $order", async ({ first, second }) => {
  const server = fakeServer();
  server.hold([CHAPTER_READ, CHALLENGE]);
  const opening = open(server);

  // The fake sees the Challenge before it answers the Chapter read.
  await expect.poll(server.inFlight).toEqual([CHALLENGE, CHAPTER_READ].sort());
  server.releaseOnly([first]);
  // After the client read the body, the steps that follow it in the client end before the next poll.
  await expect.poll(() => server.log.consumed.includes(first)).toBe(true);
  expect(server.inFlight()).toEqual([second]);
  server.releaseOnly([second]);

  expect({ writer: writerOf(await opening), challenges: server.count(CHALLENGE), creations: server.count(CREATION),
    unexpected: server.log.unexpected }).toEqual({ writer: "current_writer", challenges: 1, creations: 1,
    unexpected: [] });
});

it.each([
  { order: "after the Challenge", first: CHALLENGE },
  { order: "before the Challenge", first: CHAPTER_READ },
])("creates no Editor Session when the Chapter read of a new tab fails $order, and the next open uses its own"
  + " Challenge", async ({ first }) => {
  const server = fakeServer();
  server.hold([CHAPTER_READ, CHALLENGE]);
  server.failNext([CHAPTER_READ]);
  const failing = open(server);
  await expect.poll(server.inFlight).toEqual([CHALLENGE, CHAPTER_READ].sort());
  server.releaseOnly([first]);
  await expect.poll(() => server.log.consumed.includes(first)).toBe(true);
  server.releaseOnly([CHALLENGE, CHAPTER_READ]);
  const failed = await failing;
  await expect.poll(() => server.log.consumed.includes(CHALLENGE)).toBe(true);

  const next = await open(server);

  // The Server refuses a creation with the nonce of another Challenge, so the next open is the writer only with its own.
  expect({ failed: failed.kind, next: writerOf(next), creations: server.creations,
    unexpected: server.log.unexpected }).toEqual({ failed: "project-blocked", next: "current_writer",
    creations: [{ idempotencyKey: server.challenges[1]!.idempotencyKey, nonce: server.challenges[1]!.nonce }],
    unexpected: [] });
});

it("opens an Empty Project in a new tab with no Challenge and no creation", async () => {
  const server = fakeServer({ empty: true });

  const state = await open(server);

  expect({ kind: state.kind, challenges: server.count(CHALLENGE), creations: server.count(CREATION),
    unexpected: server.log.unexpected }).toEqual({ kind: "empty-project-ready", challenges: 0, creations: 0,
    unexpected: [] });
});

it("replaces a Challenge that expired before the end of the Chapter read, and creates the Editor Session with it",
  async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    const server = fakeServer();
    server.hold([CHAPTER_READ]);
    const opening = open(server);
    await expect.poll(() => server.log.consumed.includes(CHALLENGE)).toBe(true);
    // The Chapter read ends after the expiry of the Challenge.
    vi.setSystemTime(Date.now() + CHALLENGE_LIFETIME_MS + 1_000);
    server.releaseOnly([CHAPTER_READ]);

    expect({ writer: writerOf(await opening), creations: server.creations, unexpected: server.log.unexpected })
      .toEqual({ writer: "current_writer", unexpected: [],
        creations: [{ idempotencyKey: server.challenges[1]!.idempotencyKey, nonce: server.challenges[1]!.nonce }] });
  });

it("opens a Project with the stored Editor Session of the tab with no Challenge", async () => {
  const server = fakeServer();
  sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);

  const state = await open(server);

  expect({ writer: writerOf(state), challenges: server.count(CHALLENGE), creations: server.count(CREATION),
    unexpected: server.log.unexpected }).toEqual({ writer: "current_writer", challenges: 0, creations: 0,
    unexpected: [] });
});

it("reopens with one Challenge when the Project read names another Chapter and the tab has no stored Editor Session",
  async () => {
    const server = fakeServer();
    const { scenario } = server;
    const project = { ...scenario.project, project: { ...scenario.project.project,
      open: { kind: "current_chapter" as const, current_chapter_id: CHAPTER_B } } };

    const reopening = reopenControlledProject({ baseUrl: location.origin, fetchImpl: server.fetchImpl,
      indexedDBImpl: indexedDB, cryptoImpl: crypto, opened: { profile: scenario.profile, project } });
    opens.push(reopening);
    const state = await reopening;

    expect({ writer: writerOf(state), chapters: server.log.started.filter((name) => name.includes("/chapters/")),
      challenges: server.count(CHALLENGE), creations: server.count(CREATION), unexpected: server.log.unexpected })
      .toEqual({ writer: "current_writer", chapters: [CHAPTER_B_READ, CHAPTER_READ], challenges: 1, creations: 1,
        unexpected: [] });
  });

// On the Server, only the first created Editor Session holds the writer. A later one opens in the read-only recovery.
it.each([
  { kind: "a new tab", per: 1, first: "current_writer", later: "editor-read-only-recovery" },
  { kind: "a failed Chapter read", per: 1, first: "project-blocked", later: "project-blocked" },
  { kind: "an Empty Project", per: 0, first: "empty-project-ready", later: "empty-project-ready" },
  { kind: "a stored Editor Session", per: 0, first: "current_writer", later: "current_writer" },
])("requests at most one Challenge for each open of $kind, so 10 opens in one window of 60 s get no refusal",
  async ({ kind, per, first, later }) => {
    // The open of an expired Challenge requests two Challenges; it is not in this table.
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(Math.ceil(Date.now() / WINDOW_MS) * WINDOW_MS);
    const server = fakeServer({ empty: kind === "an Empty Project" });
    const counts: number[] = [];
    const outcomes: string[] = [];
    for (let index = 0; index < SHARED_CAPACITY; index += 1) {
      await closeOpens();
      sessionStorage.clear();
      await deleteJournal(server.scenario.journalName);
      if (kind === "a stored Editor Session") sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
      if (kind === "a failed Chapter read") server.failNext([CHAPTER_READ]);
      const before = server.count(CHALLENGE);
      const state = await open(server);
      counts.push(server.count(CHALLENGE) - before);
      outcomes.push(writerOf(state));
    }

    expect({ counts, outcomes, unexpected: server.log.unexpected }).toEqual({
      counts: Array(SHARED_CAPACITY).fill(per),
      outcomes: [first, ...Array(SHARED_CAPACITY - 1).fill(later)], unexpected: [] });
  });
