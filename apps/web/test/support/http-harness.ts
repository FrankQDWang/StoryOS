import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { afterAll, beforeAll } from "vitest";

import * as client from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateEditorSessionResponse, DigestValue, GetProposalResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, USER_A, admitProse, drainLeftoverWork, id, settleOnce } from "./acceptance.ts";
import { createEmptyProject, queryStoryOSPostgres, requireStoryOSProtocolError, sessionFetch, startStoryOSServer,
  stopStoryOSServer, withChallengeBudget } from "./node-integration.ts";
import type { StoryOSServer } from "./node-integration.ts";

export { BINDING, USER_A, drainLeftoverWork, id, settleOnce };

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const packaged = (name: string) =>
  join(repositoryRoot, "target", "release-package", process.platform === "win32" ? `${name}.exe` : name);
const execFileAsync = promisify(execFile);

export const USER_B = "018f0000-0000-7001-8000-000000000101";
export const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

/** A fresh namespace of 8 hex digits, so that the keys of a test do not collide with the keys of another run. */
export function freshNamespace(): string {
  return randomBytes(4).toString("hex");
}

// Server

/** One packaged Server. `session-a` and `session-c` are User A, and `session-b` is User B. */
export class TestServer {
  private readonly extraEnv: Readonly<Record<string, string>>;
  private current: StoryOSServer | undefined;

  constructor(extraEnv: Readonly<Record<string, string>> = {}) {
    this.extraEnv = extraEnv;
  }

  static async start(extraEnv: Readonly<Record<string, string>> = {}): Promise<TestServer> {
    const server = new TestServer(extraEnv);
    await server.start();
    return server;
  }

  async start(): Promise<void> {
    this.current = await startStoryOSServer({
      repositoryRoot,
      serverBinary: packaged("storyos-server"),
      sessions: { "session-a": USER_A, "session-b": USER_B, "session-c": USER_A },
      extraEnv: this.extraEnv,
    });
  }

  get baseUrl(): string {
    assert.ok(this.current, "the Server is not started");
    return this.current.baseUrl;
  }

  fetch(session = "session-a"): typeof fetch {
    return sessionFetch(this.baseUrl, session);
  }

  /** Stops the Server process and starts a new one on the same database. */
  async restart(): Promise<void> {
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    if (this.current !== undefined) await stopStoryOSServer(this.current.server);
    this.current = undefined;
  }
}

/** One Server for all tests of the file. */
export function serverForFile(extraEnv: Readonly<Record<string, string>> = {}): TestServer {
  const server = new TestServer(extraEnv);
  beforeAll(() => server.start());
  afterAll(() => server.stop());
  return server;
}

// Project commands

type Client = typeof client;
type CommandOptions = { baseUrl: string; projectId: string; idempotencyKey: string; antiForgery?: string; request: unknown };
type ProjectCommandId = {
  [K in keyof Client]: `digest${Capitalize<K & string>}` extends keyof Client
    ? Client[K] extends (options: infer Options) => Promise<unknown>
      ? Options extends CommandOptions ? K : never
      : never
    : never;
}[keyof Client];
type Options<Op extends ProjectCommandId> = Parameters<Client[Op]>[0];
export type CommandRequest<Op extends ProjectCommandId> = Options<Op>["request"];
export type CommandResponse<Op extends ProjectCommandId> = Awaited<ReturnType<Client[Op]>>;
type CommandPath<Op extends ProjectCommandId> = Omit<Options<Op>, keyof CommandOptions | "fetchImpl" | "signal">;
/** The request of one project command, its path parameters, and an optional fixed idempotency key. */
export type CommandInput<Op extends ProjectCommandId> = { request: CommandRequest<Op>; key?: string }
  & (keyof CommandPath<Op> extends never ? { path?: undefined } : { path: CommandPath<Op> });

/** One response of a project command with the exact bytes of its body. */
export interface Delivered<Op extends ProjectCommandId> {
  readonly response: CommandResponse<Op>;
  readonly body: string;
}

/** One project command with a solved Command Challenge. Each `send` uses the same idempotency key and nonce. */
export interface Challenged<Op extends ProjectCommandId> {
  readonly key: string;
  send(options?: { request?: CommandRequest<Op>; session?: string; fetchImpl?: typeof fetch }): Promise<Delivered<Op>>;
}

export interface Sent<Op extends ProjectCommandId> extends Delivered<Op> {
  readonly command: Challenged<Op>;
}

const ROUTES: ReadonlyMap<string, { method: string; path: string }> = new Map(
  (JSON.parse(readFileSync(join(repositoryRoot,
    "generated/route-catalog/versioned-protocol-release-1-route-catalog.json"), "utf8")) as {
    operations: { operation_id: string; method: string; path: string }[];
  }).operations.map((operation) => [operation.operation_id, operation]),
);

function clientFunction<Call>(name: string): Call {
  const value = (client as Record<string, unknown>)[name];
  assert.equal(typeof value, "function", `the generated client has no ${name}`);
  return value as Call;
}

/** Fresh UUIDs in one random namespace. */
class Keys {
  private readonly namespace = freshNamespace();
  private sequence = 0;

  next(): string {
    this.sequence += 1;
    return id(`${this.namespace}${this.sequence.toString(16).padStart(4, "0")}`);
  }
}

/** One Project of one session. It gives fresh keys and sends project commands through their Command Challenge. */
export class TestProject {
  readonly server: TestServer;
  readonly projectId: string;
  readonly session: string;
  private readonly keys: Keys;

  constructor(server: TestServer, projectId: string, session: string, keys = new Keys()) {
    this.server = server;
    this.projectId = projectId;
    this.session = session;
    this.keys = keys;
  }

  get baseUrl(): string {
    return this.server.baseUrl;
  }

  get fetchImpl(): typeof fetch {
    return this.server.fetch(this.session);
  }

  /** A fresh UUID for an idempotency key or a correlation ID. */
  key(): string {
    return this.keys.next();
  }

  /** The release binding with a fresh correlation ID. */
  binding(): typeof BINDING & { correlation_id: string } {
    return { ...BINDING, correlation_id: this.key() };
  }

  /** The same Project as another session sees it. */
  as(session: string): TestProject {
    return new TestProject(this.server, this.projectId, session);
  }

  async challenge<Op extends ProjectCommandId>(operation: Op, input: CommandInput<Op>): Promise<Challenged<Op>> {
    const route = ROUTES.get(operation);
    assert.ok(route, `the route catalog has no ${operation}`);
    const digest = clientFunction<(request: CommandRequest<Op>) => Promise<DigestValue>>(
      `digest${operation.charAt(0).toUpperCase()}${operation.slice(1)}`);
    const { request } = input;
    const key = input.key ?? this.key();
    const digested = await digest(request);
    const challenge = await withChallengeBudget(this.projectId, () => client.createProjectCommandChallenge({
      baseUrl: this.baseUrl,
      projectId: this.projectId,
      fetchImpl: this.fetchImpl,
      request: {
        method: route.method,
        route_template: route.path,
        command_schema: (request as { command_schema: string }).command_schema,
        canonical_command_digest: digested,
        idempotency_key: key,
      },
    }));
    const send = clientFunction<(options: object) => Promise<CommandResponse<Op>>>(operation);
    return {
      key,
      send: async (options = {}) => {
        let body = "";
        const inner = options.fetchImpl ?? this.server.fetch(options.session ?? this.session);
        const capturing: typeof fetch = async (input, init) => {
          const response = await inner(input, init);
          body = await response.clone().text();
          return response;
        };
        const response = await send({
          ...input.path,
          baseUrl: this.baseUrl,
          projectId: this.projectId,
          fetchImpl: capturing,
          idempotencyKey: key,
          antiForgery: challenge.nonce,
          request: options.request ?? request,
        });
        return { response, body };
      },
    };
  }

  /** Solves the Command Challenge and sends the command once. */
  async send<Op extends ProjectCommandId>(operation: Op, input: CommandInput<Op>): Promise<Sent<Op>> {
    const command = await this.challenge(operation, input);
    return { ...await command.send(), command };
  }

  async openEditorSession(): Promise<CreateEditorSessionResponse> {
    const opened = await this.send("createEditorSession", { request: {
      command_schema: "storyos.command.create-editor-session.request.v1",
      ...this.binding(),
    } });
    return opened.response;
  }

  async treeRevision(): Promise<string> {
    return (await client.getManuscriptTree({ baseUrl: this.baseUrl, projectId: this.projectId,
      fetchImpl: this.fetchImpl })).tree_revision;
  }
}

/** A Project with one Volume and its Chapters. */
export interface TestStory extends TestProject {
  readonly volumeId: string;
  readonly chapterIds: readonly string[];
}

export async function createProject(server: TestServer, options: { title: string; session?: string }): Promise<TestProject> {
  const session = options.session ?? "session-a";
  const keys = new Keys();
  const projectId = await createEmptyProject({
    baseUrl: server.baseUrl,
    fetchImpl: server.fetch(session),
    createKey: keys.next(),
    correlationId: keys.next(),
    title: options.title,
  });
  return new TestProject(server, projectId, session, keys);
}

/** Creates a Project with one Volume and one Chapter for each title. The first Chapter is the Current Chapter. */
export async function createStory(server: TestServer, options: {
  title: string;
  chapters?: readonly string[];
  session?: string;
  assistance?: boolean;
}): Promise<TestStory> {
  const project = await createProject(server, options);
  if (options.assistance === true) {
    await project.send("updateProjectAssistance", { request: {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: { availability: "available", expected_assistance_revision: "0", ...project.binding() },
    } });
  }
  return addVolume(project, options.chapters ?? ["Chapter A"]);
}

/** Adds one Volume with one Chapter for each title to the Project. On an empty Project, the first Chapter becomes
 * the Current Chapter. */
export async function addVolume(project: TestProject, chapters: readonly string[]): Promise<TestStory> {
  const volume = await project.send("createVolume", { request: {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: { title: "Volume A", expected_tree_revision: await project.treeRevision(), ...project.binding() },
  } });
  assert.equal(volume.response.effect.kind, "authoritative_applied");
  if (volume.response.effect.kind !== "authoritative_applied") throw new Error("Create Volume must apply");
  const volumeId = volume.response.effect.volume_id;
  const chapterIds: string[] = [];
  for (const title of chapters) {
    const chapter = await project.send("createChapter", { request: {
      command_schema: "storyos.command.create-chapter.request.v1",
      create_chapter_input: { title, expected_tree_revision: await project.treeRevision(), ...project.binding() },
    }, path: { volumeId } });
    if (chapter.response.effect.kind !== "authoritative_applied") throw new Error("Create Chapter must apply");
    chapterIds.push(chapter.response.effect.chapter_id);
  }
  return Object.assign(project, { volumeId, chapterIds });
}

/** Admits one prose AgentRun on the first Chapter and returns the Proposal that it opened. */
export async function openProposal(story: TestStory): Promise<GetProposalResponse> {
  const run = await admitProse(story.baseUrl, story.fetchImpl, story.projectId, story.chapterIds[0] ?? "", story.key());
  if (run.decision.kind !== "prose_change" || run.decision.opened_proposal.kind !== "present") {
    throw new Error("expected an opened Proposal");
  }
  return client.getProposal({ baseUrl: story.baseUrl, projectId: story.projectId,
    proposalId: run.decision.opened_proposal.proposal_id, fetchImpl: story.fetchImpl });
}

// Problems

/** The status and the problem code of a refused HTTP response. */
export interface Problem {
  readonly status: number | undefined;
  readonly code: string | undefined;
  readonly body: string;
}

/** Waits for a refused protocol call and reads its problem. */
export async function problemOf(call: Promise<unknown>): Promise<Problem> {
  const error = await call.then(
    () => assert.fail("expected a refused response"),
    (refused: unknown) => requireStoryOSProtocolError(refused),
  );
  const body = error.responseBody ?? "";
  let code: string | undefined;
  try {
    code = (JSON.parse(body) as { code?: string }).code;
  } catch {
    code = undefined;
  }
  return { status: error.status, code, body };
}

// Worker settlement

/** Polls `probe` until `match` holds. */
export async function waitFor<Value>(probe: () => Promise<Value>, match: (value: Value) => boolean,
  options: { attempts?: number; intervalMs?: number } = {}): Promise<Value> {
  let last: Value | undefined;
  for (let attempt = 0; attempt < (options.attempts ?? 200); attempt += 1) {
    last = await probe();
    if (match(last)) return last;
    await new Promise((resolve) => setTimeout(resolve, options.intervalMs ?? 25));
  }
  throw new Error(`the wait timed out; last value: ${JSON.stringify(last)}`);
}

/** Waits until the command of this key has its settled outcome, also when the Server holds its response. */
export async function waitForSettledKey(project: TestProject, key: string): Promise<void> {
  await waitFor(() => queryStoryOSPostgres(`
    SELECT outcome_kind FROM storyos.command_idempotency
     WHERE project_id = '${project.projectId}'::uuid AND idempotency_key = '${key}'::uuid;
  `), (outcome) => outcome === "settled", { intervalMs: 10 });
}

/** Starts one Worker pass that a test holds through `extraEnv`. The returned promise settles when the pass exits. */
export function settleHeld(extraEnv: Readonly<Record<string, string>>): Promise<void> {
  const env = { ...process.env, ...extraEnv };
  if (process.env.STORYOS_TEST_DATABASE_URL !== undefined) env.STORYOS_DATABASE_URL = process.env.STORYOS_TEST_DATABASE_URL;
  const settled = execFileAsync(packaged("storyos-worker"), ["--once"], {
    cwd: repositoryRoot, env, timeout: 60_000, killSignal: "SIGKILL",
  }).then(() => undefined);
  void settled.catch(() => undefined);
  return settled;
}
