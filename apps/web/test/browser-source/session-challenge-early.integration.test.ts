import { afterEach, expect, it, vi } from "vitest";
import { digestCreateEditorSession } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateEditorSessionRequest, GetEditorSessionResponse, GetProjectResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { openControlledProject, reopenControlledProject } from "../../src/boot.ts";
import type { ControlledProjectState } from "../../src/editor-types.ts";
import {
  CHAPTER, EDITOR_SNAPSHOT, OWNER, PROJECT, SESSION, chapterRevision, createBrowserScenario, deleteJournal,
  jsonResponse,
} from "./scenario.ts";

// An open that reads the Chapter as the first open does requests the Challenge of its Editor Session creation with
// that Chapter read (#1899).

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
/** The exact requests of each kind of open. */
const NEW_TAB = [PROTOCOL, PROJECT_READ, LIST, CHAPTER_READ, CHALLENGE, CREATION];
const EMPTY = [PROTOCOL, PROJECT_READ, LIST, TREE];
const MISSING = [PROTOCOL, PROJECT_READ, LIST];
const STORED = [PROTOCOL, PROJECT_READ, LIST, CHAPTER_READ, SESSION_READ];
/** The class `shared` of the Server: 10 new Challenges in each fixed window of 60 s (`ChallengeRateClass`). */
const SHARED_CAPACITY = 10;
const WINDOW_MS = 60_000;
/** A Challenge of the Server expires 5 minutes after its issue (`project_command_challenges.expires_at`). */
const CHALLENGE_LIFETIME_MS = 5 * 60_000;

type Open = "current_chapter" | "empty" | "missing";
type Challenge = { idempotencyKey: string; digest: string; nonce: string; expiresAt: number; used: boolean };
type Entry = { open: number; name: string };

/**
 * A fake Server of one Project whose Current Chapter is Chapter A, an Empty Project, or a Project that is not there. It
 * serves only the requests in `serves` that the state of the Project permits, records each other request, and throws.
 * It records, for each open, the start of each request, each response body that the client read, and, in one order,
 * the start and the end of each request. As on the Server, a Challenge has a new nonce, expires 5 minutes after its
 * issue, and counts in the fixed window of the class `shared`: the 11th Challenge of a window gets `429
 * challenge_rate_limited`. A creation needs the nonce of an unused, unexpired Challenge with its idempotency key and
 * the digest of its request, else `422 challenge_invalid`. Each created Editor Session has its own base Snapshot, and
 * its response has the correlation identity of its request. Only the first created Editor Session holds the writer.
 * `hold(names)` holds the next request of each given name until `releaseOnly`. `failNext(names)` answers the next
 * request of each given name with the 503 problem of an unavailable Project store.
 */
function fakeServer({ serves, open: initial = "current_chapter" }: { serves: string[]; open?: Open | undefined }) {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  let state: Open = initial;
  const log = { started: [] as Entry[], consumed: [] as Entry[], events: [] as string[], unexpected: [] as string[] };
  const challenges: Challenge[] = [];
  const creations: { idempotencyKey: string; nonce: string }[] = [];
  let createdSessions = 0;
  let holding = new Set<string>();
  const failing = new Set<string>();
  const held: { name: string; resolve: () => void }[] = [];
  const projectOpen = (): GetProjectResponse["project"]["open"] =>
    state === "empty" ? { kind: "empty" } : scenario.project.project.open;
  const created = (index: number, correlationId: string): GetEditorSessionResponse => ({ ...scenario.session,
    correlation_id: correlationId,
    editor_session: { ...scenario.session.editor_session,
      editor_session_id: index === 0 ? SESSION : `018f0000-0000-7001-8000-0000000009${String(index).padStart(2, "0")}` },
    base_snapshot: { ...scenario.session.base_snapshot,
      snapshot_id: index === 0 ? EDITOR_SNAPSHOT : `018f0000-0000-7001-8000-0000000008${String(index).padStart(2, "0")}` },
    writer: index === 0 ? { kind: "current_writer", writer_generation: "1" }
      : { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" } });
  const refuse = (name: string): never => {
    log.unexpected.push(`${name} (${state})`);
    throw new Error(`unexpected request: ${name}`);
  };
  const answer = async (name: string, init: RequestInit | undefined): Promise<Response> => {
    if (!serves.includes(name)) return refuse(name);
    if (failing.delete(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "project_store_unavailable",
        message: "The Project store is unavailable." }, 503);
    }
    if (name === PROTOCOL) return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (name === PROJECT_READ && state === "missing") {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable",
        message: "The requested resource is unavailable." }, 404);
    }
    if (name === PROJECT_READ) {
      return jsonResponse({ ...scenario.project, project: { ...scenario.project.project, open: projectOpen() } });
    }
    if (name === LIST) {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000913", owner_user_id: OWNER,
        projects: state === "missing" ? [] : [{ project_scope: scope, title: "Project A", revision: "1",
          open: projectOpen() }] });
    }
    if (name === CHAPTER_READ && state === "current_chapter") return jsonResponse(scenario.chapter);
    if (name === CHAPTER_B_READ && state === "current_chapter") {
      return jsonResponse({ ...scenario.chapter, chapter: { chapter_id: CHAPTER_B, title: "Chapter B",
        current_revision: chapterRevision("018f0000-0000-7001-8000-000000000904", "") } });
    }
    if (name === TREE && state === "empty") {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000914", project_scope: scope, tree_revision: "1",
        snapshot: { snapshot_id: scenario.session.base_snapshot.snapshot_id, project_scope: scope }, volumes: [] });
    }
    if (name === SESSION_READ && state === "current_chapter") {
      return jsonResponse({ ...created(0, "018f0000-0000-7001-8000-000000000915"),
        schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (name === CHALLENGE && state === "current_chapter") {
      const body = JSON.parse(String(init?.body)) as { method: string; route_template: string; command_schema: string;
        canonical_command_digest: { value_hex_lowercase: string }; idempotency_key: string };
      if (body.method !== "POST" || body.route_template !== "/api/v1/projects/{project_id}/editor-sessions"
        || body.command_schema !== "storyos.command.create-editor-session.request.v1") {
        return refuse(`${name} for ${body.method} ${body.route_template}`);
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
    if (name === CREATION && state === "current_chapter") {
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
      return jsonResponse(created(createdSessions - 1, request.correlation_id));
    }
    return refuse(name);
  };
  /** The fetch of the open `open`. */
  const fetchFor = (open: number): typeof fetch => async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    log.started.push({ open, name });
    log.events.push(`start ${name}`);
    try {
      if (holding.delete(name)) await new Promise<void>((resolve) => { held.push({ name, resolve }); });
      const response = await answer(name, init);
      const text = response.text.bind(response);
      response.text = async () => {
        const body = await text();
        log.consumed.push({ open, name });
        return body;
      };
      return response;
    } finally {
      log.events.push(`end ${name}`);
    }
  };
  const tally = (entries: Entry[], name: string, open?: number) =>
    entries.filter((entry) => entry.name === name && (open === undefined || entry.open === open)).length;
  const server = {
    scenario, log, fetchFor, challenges, creations,
    setOpen(next: Open) { state = next; },
    hold(names: string[]) { holding = new Set(names); },
    failNext(names: string[]) { for (const name of names) failing.add(name); },
    releaseOnly(names: string[]) {
      for (const entry of held.filter(({ name }) => names.includes(name))) {
        held.splice(held.indexOf(entry), 1);
        entry.resolve();
      }
    },
    releaseAll() {
      holding = new Set();
      for (const { resolve } of held.splice(0)) resolve();
    },
    /** The started requests of `name`, of the open `open` or of each open. */
    count: (name: string, open?: number) => tally(log.started, name, open),
    /** The read response bodies of `name`, of the open `open` or of each open. */
    consumed: (name: string, open?: number) => tally(log.consumed, name, open),
    /** True when the fake answered each request of `before` before the first start of `after`. */
    answeredBefore(before: string[], after: string) {
      const start = log.events.indexOf(`start ${after}`);
      return start >= 0 && before.every((name) => {
        const end = log.events.indexOf(`end ${name}`);
        return end >= 0 && end < start;
      });
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

/** Opens the Project in a tab with the session storage of this test, as the open of a new tab. */
function open(server: ReturnType<typeof fakeServer>, index = 0): Promise<ControlledProjectState> {
  const state = openControlledProject({ baseUrl: location.origin, projectId: PROJECT,
    fetchImpl: server.fetchFor(index), indexedDBImpl: indexedDB, cryptoImpl: crypto });
  opens.push(state);
  return state;
}

/** Reopens the Project from a page whose Project read had the open `opened`. */
function reopen(server: ReturnType<typeof fakeServer>, opened: GetProjectResponse["project"]["open"],
  { index = 0, cryptoImpl = crypto }: { index?: number; cryptoImpl?: Crypto } = {}): Promise<ControlledProjectState> {
  const { scenario } = server;
  const state = reopenControlledProject({ baseUrl: location.origin, fetchImpl: server.fetchFor(index),
    indexedDBImpl: indexedDB, cryptoImpl,
    opened: { profile: scenario.profile, project: { ...scenario.project,
      project: { ...scenario.project.project, open: opened } } } });
  opens.push(state);
  return state;
}

/** A crypto that counts the digests that start and end. */
function countingCrypto() {
  const digests = { started: 0, ended: 0 };
  const subtle = new Proxy(crypto.subtle, { get: (target, key) => key === "digest"
    ? async (...args: Parameters<SubtleCrypto["digest"]>) => {
      digests.started += 1;
      try { return await target.digest(...args); } finally { digests.ended += 1; }
    }
    : (Reflect.get(target, key) as (...args: unknown[]) => unknown).bind(target) });
  const cryptoImpl = new Proxy(crypto, { get: (target, key) => key === "subtle" ? subtle
    : (Reflect.get(target, key) as (...args: unknown[]) => unknown).bind(target) });
  return { cryptoImpl, digests };
}

/** The writer of the editor of an open, or the kind of the open when it has no ready editor. */
const writerOf = (state: ControlledProjectState) => state.kind !== "project-ready" ? state.kind
  : state.editor.kind === "editor-ready" ? state.editor.session.writer.kind : state.editor.kind;

it.each([
  { order: "the Chapter read and then the Challenge", first: CHAPTER_READ, second: CHALLENGE },
  { order: "the Challenge and then the Chapter read", first: CHALLENGE, second: CHAPTER_READ },
])("opens a new tab with the Challenge request before the answer of the Chapter read, and creates the Editor Session"
  + " after the fake answers $order", async ({ first, second }) => {
  const server = fakeServer({ serves: NEW_TAB });
  server.hold([CHAPTER_READ, CHALLENGE]);
  const opening = open(server);

  // The fake holds the Chapter read, and the Challenge request starts.
  await expect.poll(() => [server.count(CHAPTER_READ), server.count(CHALLENGE)]).toEqual([1, 1]);
  expect(server.count(CREATION)).toBe(0);
  server.releaseOnly([first]);
  await expect.poll(() => server.consumed(first)).toBe(1);
  expect(server.count(CREATION)).toBe(0);
  server.releaseOnly([second]);

  expect({ writer: writerOf(await opening), challenges: server.count(CHALLENGE), creations: server.count(CREATION),
    creationAfterAnswers: server.answeredBefore([CHAPTER_READ, CHALLENGE], CREATION),
    unexpected: server.log.unexpected }).toEqual({ writer: "current_writer", challenges: 1, creations: 1,
    creationAfterAnswers: true, unexpected: [] });
});

it.each([
  { order: "after the Challenge", first: CHALLENGE },
  { order: "before the Challenge", first: CHAPTER_READ },
])("creates no Editor Session when the Chapter read of a new tab fails $order, and the next open uses its own"
  + " Challenge", async ({ first }) => {
  const server = fakeServer({ serves: NEW_TAB });
  server.hold([CHAPTER_READ, CHALLENGE]);
  server.failNext([CHAPTER_READ]);
  const failing = open(server, 0);
  await expect.poll(() => [server.count(CHAPTER_READ), server.count(CHALLENGE)]).toEqual([1, 1]);
  server.releaseOnly([first]);
  await expect.poll(() => server.consumed(first)).toBe(1);
  server.releaseOnly([CHALLENGE, CHAPTER_READ]);
  const failed = await failing;
  await expect.poll(() => server.consumed(CHALLENGE, 0)).toBe(1);

  const next = await open(server, 1);

  // The Server refuses a creation with the nonce of another Challenge, so the next open is the writer only with its own.
  expect({ failed: failed.kind, next: writerOf(next), challenges: [server.count(CHALLENGE, 0), server.count(CHALLENGE, 1)],
    creations: server.creations, unexpected: server.log.unexpected }).toEqual({ failed: "project-blocked",
    next: "current_writer", challenges: [1, 1], unexpected: [],
    creations: [{ idempotencyKey: server.challenges[1]!.idempotencyKey, nonce: server.challenges[1]!.nonce }] });
});

it("reopens with the Challenge request of the replacement open with its Chapter read when the Project read names"
  + " another Chapter, and requests one Challenge in sum", async () => {
  const server = fakeServer({ serves: [PROJECT_READ, LIST, CHAPTER_B_READ, CHAPTER_READ, CHALLENGE, CREATION] });
  server.hold([CHAPTER_READ]);
  // The early open reads Chapter B, and the Project read names Chapter A: the reopen discards the early open.
  const reopening = reopen(server, { kind: "current_chapter", current_chapter_id: CHAPTER_B });

  await expect.poll(() => [server.count(CHAPTER_READ), server.count(CHALLENGE)]).toEqual([1, 1]);
  expect(server.count(CREATION)).toBe(0);
  server.releaseOnly([CHAPTER_READ]);

  expect({ writer: writerOf(await reopening),
    chapters: server.log.started.filter(({ name }) => name.includes("/chapters/")).map(({ name }) => name),
    challenges: server.count(CHALLENGE), creationAfterAnswers: server.answeredBefore([CHAPTER_READ], CREATION),
    unexpected: server.log.unexpected }).toEqual({ writer: "current_writer", chapters: [CHAPTER_B_READ, CHAPTER_READ],
    challenges: 1, creationAfterAnswers: true, unexpected: [] });
});

it("reopens with the early open and no stored Editor Session, and requests its one Challenge at the creation after"
  + " the Chapter read", async () => {
  const server = fakeServer({ serves: [PROJECT_READ, LIST, CHAPTER_READ, CHALLENGE, CREATION] });
  const { cryptoImpl, digests } = countingCrypto();
  server.hold([CHAPTER_READ]);
  const reopening = reopen(server, { kind: "current_chapter", current_chapter_id: CHAPTER }, { cryptoImpl });

  // The reopen uses the early open after the Project read. A Challenge of the early open would start its request when
  // its digest ends.
  await expect.poll(() => [server.consumed(PROJECT_READ), server.consumed(LIST), digests.ended === digests.started])
    .toEqual([1, 1, true]);
  expect(server.count(CHALLENGE)).toBe(0);
  server.releaseOnly([CHAPTER_READ]);

  expect({ writer: writerOf(await reopening), challenges: server.count(CHALLENGE),
    challengeAfterChapter: server.answeredBefore([CHAPTER_READ], CHALLENGE), unexpected: server.log.unexpected })
    .toEqual({ writer: "current_writer", challenges: 1, challengeAfterChapter: true, unexpected: [] });
});

it.each([
  { path: "the open of a new tab, as the page of an Empty Project does after its first Chapter",
    run: (server: ReturnType<typeof fakeServer>) => open(server, 1) },
  { path: "a reopen from the page of the Empty Project, which has no early open",
    run: (server: ReturnType<typeof fakeServer>) => reopen(server, { kind: "empty" }, { index: 1 }) },
])("opens an Empty Project with no Challenge, and after its first Chapter, with no stored Editor Session, requests one"
  + " Challenge with the Chapter read by $path", async ({ run }) => {
  const server = fakeServer({ serves: [...EMPTY, CHAPTER_READ, CHALLENGE, CREATION], open: "empty" });
  const empty = await open(server, 0);
  // The author creates the first Chapter, and the Project names it.
  server.setOpen("current_chapter");
  server.hold([CHAPTER_READ]);
  const opening = run(server);

  await expect.poll(() => [server.count(CHAPTER_READ, 1), server.count(CHALLENGE, 1)]).toEqual([1, 1]);
  expect(server.count(CREATION)).toBe(0);
  server.releaseOnly([CHAPTER_READ]);

  expect({ empty: empty.kind, next: writerOf(await opening),
    challenges: [server.count(CHALLENGE, 0), server.count(CHALLENGE, 1)], creations: server.count(CREATION),
    unexpected: server.log.unexpected }).toEqual({ empty: "empty-project-ready", next: "current_writer",
    challenges: [0, 1], creations: 1, unexpected: [] });
});

it("opens a Project with the stored Editor Session of the tab with no Challenge", async () => {
  const server = fakeServer({ serves: STORED });
  sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);

  const state = await open(server);

  expect({ writer: writerOf(state), challenges: server.count(CHALLENGE), unexpected: server.log.unexpected })
    .toEqual({ writer: "current_writer", challenges: 0, unexpected: [] });
});

// On the Server, only the first created Editor Session holds the writer. A later one opens in the read-only recovery.
it.each([
  { kind: "a new tab", per: 1, first: "current_writer", later: "editor-read-only-recovery", serves: NEW_TAB },
  { kind: "a failed Chapter read", per: 1, first: "project-blocked", later: "project-blocked", serves: NEW_TAB },
  { kind: "an Empty Project", per: 0, first: "empty-project-ready", later: "empty-project-ready", serves: EMPTY,
    open: "empty" as const },
  { kind: "a Project that is not there", per: 0, first: "project-blocked", later: "project-blocked", serves: MISSING,
    open: "missing" as const },
  { kind: "a stored Editor Session", per: 0, first: "current_writer", later: "current_writer", serves: STORED },
])("requests at most one Challenge for each open of $kind, so 10 opens in one window of 60 s get no refusal",
  async ({ kind, per, first, later, serves, open: projectOpen }) => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(Math.ceil(Date.now() / WINDOW_MS) * WINDOW_MS);
    const server = fakeServer({ serves, open: projectOpen });
    const outcomes: string[] = [];
    for (let index = 0; index < SHARED_CAPACITY; index += 1) {
      await closeOpens();
      sessionStorage.clear();
      await deleteJournal(server.scenario.journalName);
      if (kind === "a stored Editor Session") sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
      if (kind === "a failed Chapter read") server.failNext([CHAPTER_READ]);
      const state = await open(server, index);
      // The open of a failed Chapter read ends when the client read the response of its Challenge.
      if (kind === "a failed Chapter read") await expect.poll(() => server.consumed(CHALLENGE, index)).toBe(1);
      outcomes.push(writerOf(state));
    }

    expect({ counts: Array.from({ length: SHARED_CAPACITY }, (_, index) => server.count(CHALLENGE, index)), outcomes,
      unexpected: server.log.unexpected }).toEqual({ counts: Array(SHARED_CAPACITY).fill(per),
      outcomes: [first, ...Array(SHARED_CAPACITY - 1).fill(later)], unexpected: [] });
  });
