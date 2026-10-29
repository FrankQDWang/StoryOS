import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { afterAll, beforeAll } from "vitest";

import * as client from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateEditorSessionResponse, DigestValue, GetProposalResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, PROSE, USER_A, admitProse, admitRun, attempt, challenged, drainLeftoverWork, id, inspectRun, issueChallenge, postRun,
  prepare, produced, rewriteChapter, runRequest, runWorker, selected, settleOnce, settleRun, streamRequest } from "./acceptance.ts";
import { zipStoreFiles } from "./archive.ts";
import { createEmptyProject, queryStoryOSPostgres, requireStoryOSProtocolError, sessionFetch, startStoryOSServer,
  stopStoryOSServer, withChallengeBudget } from "./node-integration.ts";
import type { StoryOSServer } from "./node-integration.ts";

export { BINDING, PROSE, USER_A, admitProse, admitRun, attempt, challenged, drainLeftoverWork, id, inspectRun, issueChallenge, postRun,
  prepare, produced, rewriteChapter, runRequest, runWorker, selected, settleOnce, settleRun, streamRequest };

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
/** The path of one binary of the release package. */
export const packaged = (name: string) =>
  join(repositoryRoot, "target", "release-package", process.platform === "win32" ? `${name}.exe` : name);
const execFileAsync = promisify(execFile);

export const USER_B = "018f0000-0000-7001-8000-000000000101";
/** Project A of User A in the shared database fixture. */
export const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
export const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

/** A fresh namespace of 8 hex digits, so that the keys of a test do not collide with the keys of another run. */
export function freshNamespace(): string {
  return randomBytes(4).toString("hex");
}

// Server

/** The sessions of a test Server: `session-a` and `session-c` are User A, and `session-b` is User B. */
export const SESSIONS: Readonly<Record<string, string>> = { "session-a": USER_A, "session-b": USER_B, "session-c": USER_A };

/** The options of `startServer`. The default sessions are `SESSIONS`. */
export type ServerOptions = Omit<Parameters<typeof startStoryOSServer>[0], "repositoryRoot" | "serverBinary">;

/** Starts one packaged Server process. */
export function startServer(options: ServerOptions = {}): Promise<StoryOSServer> {
  return startStoryOSServer({ ...options, repositoryRoot, serverBinary: packaged("storyos-server"),
    sessions: options.sessions ?? SESSIONS });
}

/** Stops the Server process and starts a new one on the same address. */
export async function restartServer(started: StoryOSServer, options: Omit<ServerOptions, "bind"> = {}): Promise<StoryOSServer> {
  const url = new URL(started.baseUrl);
  await stopStoryOSServer(started.server);
  return startServer({ ...options, bind: `${url.hostname}:${url.port}` });
}

/** One packaged Server. */
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
    this.current = await startServer({ extraEnv: this.extraEnv });
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
export type ProjectCommandId = {
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
  readonly nonce: string;
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
      nonce: challenge.nonce,
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

  /** Sends the command once with a fresh key and a nonce that no Command Challenge issued. */
  async sendUnchallenged<Op extends ProjectCommandId>(operation: Op, input: CommandInput<Op>,
    session = this.session): Promise<CommandResponse<Op>> {
    const send = clientFunction<(options: object) => Promise<CommandResponse<Op>>>(operation);
    return send({ ...input.path, baseUrl: this.baseUrl, projectId: this.projectId,
      fetchImpl: this.server.fetch(session), idempotencyKey: this.key(), antiForgery: "0".repeat(64),
      request: input.request });
  }

  /** Solves the Command Challenge as this session and sends the command once from another session. */
  async sendAs<Op extends ProjectCommandId>(session: string, operation: Op, input: CommandInput<Op>): Promise<Delivered<Op>> {
    return (await this.challenge(operation, input)).send({ session });
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

/** Creates one empty Project of the session with this idempotency key and correlation ID. */
export async function createEmpty(baseUrl: string, session: string, idempotencyKey: string, title: string,
  correlationId: string): Promise<{ fetchImpl: typeof fetch; projectId: string }> {
  const fetchImpl = sessionFetch(baseUrl, session);
  const projectId = await createEmptyProject({ baseUrl, fetchImpl, createKey: idempotencyKey, correlationId, title });
  return { fetchImpl, projectId };
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

/** A new Project with assistance, one open Proposal with one pending Proposal Operation, and a writer Editor
 * Session. */
export async function openedProposal(server: TestServer, title: string) {
  await drainLeftoverWork();
  const story = await createStory(server, { title, assistance: true });
  const chapter = async () => (await client.getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
    chapterId: story.chapterIds[0] ?? "", fetchImpl: story.fetchImpl })).chapter;
  const before = await chapter();
  const opened = (await openProposal(story)).proposal;
  const session = await story.openEditorSession();
  const inspect = async () => (await client.getProposal({ baseUrl: story.baseUrl, projectId: story.projectId,
    proposalId: opened.proposal_id, fetchImpl: story.fetchImpl })).proposal;
  return { story, before, chapter, opened, inspect, editorSessionId: session.editor_session.editor_session_id };
}

/** The value of `openedProposal`. */
export type OpenedProposal = Awaited<ReturnType<typeof openedProposal>>;

/** The rejection of the pending Proposal Operation of an `openedProposal`. */
export function rejection({ story, opened, before, editorSessionId }: OpenedProposal, options: {
  revisionId?: string; targetRevisionId?: string; editorSessionId?: string;
} = {}): CommandInput<"rejectProposalOperations"> {
  return { request: {
    command_schema: "storyos.command.reject-proposal-operations.request.v1",
    reject_proposal_operations_input: {
      proposal_revision_id: options.revisionId ?? opened.revision_id,
      selected_pending_operation_ids: [opened.operation_id],
      expected_target_revisions: [options.targetRevisionId ?? before.current_revision.revision_id],
      rejection_reason: { kind: "author_declined", note: { kind: "omitted" } },
      editor_session_id: options.editorSessionId ?? editorSessionId,
      ...story.binding(),
    },
  }, path: { proposalId: opened.proposal_id } };
}

/** Exports the Project archive through the Worker and returns a reader of its canonical tables. */
export async function archivedTables(story: TestStory): Promise<(table: string) => unknown[]> {
  const archive = await story.send("exportProjectArchive", { request: {
    command_schema: "storyos.command.export-project-archive.request.v1",
    export_project_archive_input: { ...story.binding(), archive_profile: "storyos.project-export.v1",
      archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" },
  } });
  if (archive.response.effect.kind !== "admitted") throw new Error("expected an admitted archive");
  const exportId = archive.response.effect.export_id;
  await settleOnce();
  assert.equal((await client.getExportOperation({ baseUrl: story.baseUrl, projectId: story.projectId, exportId,
    fetchImpl: story.fetchImpl })).status, "ready");
  const download = await story.fetchImpl(`${story.baseUrl}/api/v1/projects/${story.projectId}/exports/${exportId}`,
    { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
  assert.equal(download.status, 200);
  const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
  return (table) => JSON.parse(new TextDecoder().decode(files.get(`canonical/${table}.json`))) as unknown[];
}

// Structure inputs

/** The input of `createVolume`. */
export function createVolumeInput(project: TestProject, title: string,
  expectedTreeRevision: string): CommandInput<"createVolume"> {
  return { request: {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: { title, expected_tree_revision: expectedTreeRevision, ...project.binding() },
  } };
}

/** The input of `updateVolume`. */
export function updateVolumeInput(project: TestProject, volumeId: string, title: string, order: string,
  expectedTreeRevision: string): CommandInput<"updateVolume"> {
  return { request: {
    command_schema: "storyos.command.update-volume.request.v1",
    update_volume_input: { title, order, expected_tree_revision: expectedTreeRevision, ...project.binding() },
  }, path: { volumeId } };
}

/** The input of `deleteVolume`. */
export function deleteVolumeInput(project: TestProject, volumeId: string,
  expectedTreeRevision: string): CommandInput<"deleteVolume"> {
  return { request: {
    command_schema: "storyos.command.delete-volume.request.v1",
    delete_volume_input: { expected_tree_revision: expectedTreeRevision, ...project.binding() },
  }, path: { volumeId } };
}

/** The input of `createChapter` at the end of the Volume. */
export function createChapterInput(project: TestProject, volumeId: string, title: string,
  expectedTreeRevision: string): CommandInput<"createChapter"> {
  return { request: {
    command_schema: "storyos.command.create-chapter.request.v1",
    create_chapter_input: { title, expected_tree_revision: expectedTreeRevision, ...project.binding() },
  }, path: { volumeId } };
}

/** The input of `updateChapter`. */
export function updateChapterInput(project: TestProject, chapterId: string, title: string, order: string,
  expectedTreeRevision: string): CommandInput<"updateChapter"> {
  return { request: {
    command_schema: "storyos.command.update-chapter.request.v1",
    update_chapter_input: { title, order, expected_tree_revision: expectedTreeRevision, ...project.binding() },
  }, path: { chapterId } };
}

/** The input of `deleteChapter`. */
export function deleteChapterInput(project: TestProject, chapterId: string,
  expectedTreeRevision: string): CommandInput<"deleteChapter"> {
  return { request: {
    command_schema: "storyos.command.delete-chapter.request.v1",
    delete_chapter_input: { expected_tree_revision: expectedTreeRevision, ...project.binding() },
  }, path: { chapterId } };
}

/** The applied effect of `createVolume`. It throws when the command did not apply. */
export function appliedVolume(created: CommandResponse<"createVolume">) {
  if (created.effect.kind !== "authoritative_applied") {
    throw new Error("Create Volume must apply");
  }
  return created.effect;
}

// Problems

/** The status and the problem code of a refused HTTP response. */
export interface Problem {
  readonly status: number | undefined;
  readonly code: string | undefined;
  readonly body: string;
}

/** The problem code of a refused protocol call, or `undefined` when its body has no code. */
export function problemCode(refused: unknown): string | undefined {
  const body = requireStoryOSProtocolError(refused).responseBody;
  if (body === undefined) return undefined;
  try {
    return (JSON.parse(body) as { code?: string }).code;
  } catch {
    return undefined;
  }
}

/** True when the refusal has this status and its body does not match `sensitive`. */
export function sanitizedProtocolFailure(error: unknown, status: number, sensitive: RegExp): boolean {
  const protocolError = requireStoryOSProtocolError(error);
  return protocolError.status === status && !sensitive.test(protocolError.responseBody ?? "");
}

/** Waits for a refused protocol call and reads its problem. */
export async function problemOf(call: Promise<unknown>): Promise<Problem> {
  const error = await call.then(
    () => assert.fail("expected a refused response"),
    (refused: unknown) => requireStoryOSProtocolError(refused),
  );
  return { status: error.status, code: problemCode(error), body: error.responseBody ?? "" };
}

// Fixture Project A

/** The counts of the Author Edit authority records of Project A. */
export async function manuscriptAuthority() {
  return JSON.parse(await queryStoryOSPostgres(`SELECT json_build_object(
    'author_edit_activities', (SELECT count(*) FROM storyos.project_activity_events
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'actions', (SELECT count(*) FROM storyos.author_action_entries
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'commits', (SELECT count(*) FROM storyos.authoritative_commits
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'apply_admissions', (SELECT count(*) FROM storyos.author_command_admissions
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
        AND command_kind = 'applyAuthorEdit'),
    'takeover_payloads', (SELECT count(*) FROM storyos.project_activity_event_payloads
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid)
  )::text`));
}

/** Opens one Editor Session of `session-a` on Project A. */
export async function openEditorSession(baseUrl: string, correlationId: string,
  idempotencyKey: string): Promise<CreateEditorSessionResponse> {
  const request = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: "storyos.web-client.release-1.v3",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: correlationId,
  };
  const digest = await client.digestCreateEditorSession(request);
  return challenged(baseUrl, sessionFetch(baseUrl, "session-a"), PROJECT_A, "POST",
    "/api/v1/projects/{project_id}/editor-sessions", request.command_schema, digest, idempotencyKey,
    (antiForgery) => client.createEditorSession({
      baseUrl, projectId: PROJECT_A, request, idempotencyKey,
      antiForgery, fetchImpl: sessionFetch(baseUrl, "session-a"),
    }));
}

// Activity streams

/** One Server-Sent Event frame. */
export interface SseFrame {
  data: unknown;
  event: string;
  id: string;
}

/** Splits a Server-Sent Events body into its frames. */
export function parseSseFrames(body: string): SseFrame[] {
  return body.split("\n\n").filter((block) => block.trim().length > 0).map((block) => {
    const frame = { id: "", event: "", data: null };
    for (const line of block.split("\n")) {
      if (line.startsWith("id:")) frame.id = line.slice(3).trim();
      if (line.startsWith("event:")) frame.event = line.slice(6).trim();
      if (line.startsWith("data:")) frame.data = JSON.parse(line.slice(5).trim());
    }
    return frame;
  });
}

function canonicalJson(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value).sort().map((key) => [key, canonicalJson(Reflect.get(value, key))]),
    );
  }
  return value;
}

/** The digest of an event payload in the `storyos.event-payload.jcs.v1` profile. */
export async function eventPayloadDigest(payload: unknown): Promise<DigestValue> {
  const digest = new Uint8Array(await crypto.subtle.digest(
    "SHA-256", new TextEncoder().encode(JSON.stringify(canonicalJson(payload))),
  ));
  return {
    algorithm: "sha256",
    profile: "storyos.event-payload.jcs.v1",
    value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join(""),
  };
}

// Worker settlement

/** Proves that another session cannot send the command. The Challenge route answers 404 with no owner identity, and
 * the command route refuses the Challenge of the owner. */
export async function assertForeignRefused<Op extends ProjectCommandId>(project: TestProject, operation: Op,
  input: CommandInput<Op>, session = "session-b"): Promise<void> {
  const challenge = await problemOf(project.as(session).send(operation, input));
  const command = await problemOf(project.sendAs(session, operation, input));
  assert.deepEqual([challenge.status, command.status, command.code], [404, 422, "challenge_invalid"]);
  for (const { body } of [challenge, command]) assert.ok(!body.includes(USER_A));
}

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

/** Waits until the AgentRun has an opened Proposal for which `match` holds, and returns that Proposal. */
export async function waitForOpenedProposal(baseUrl: string,
  prepared: { readonly projectId: string; readonly fetchImpl: typeof fetch }, runId: string,
  match: (opened: GetProposalResponse) => boolean): Promise<GetProposalResponse> {
  const opened = await waitFor(async () => {
    const queried = await inspectRun(baseUrl, prepared, runId);
    if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") return undefined;
    return client.getProposal({ baseUrl, projectId: prepared.projectId,
      proposalId: queried.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl });
  }, (current) => current !== undefined && match(current));
  return opened as GetProposalResponse;
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
  const settled = execFileAsync(packaged("storyos-worker"), ["--once"], heldWorkerOptions(extraEnv)).then(() => undefined);
  void settled.catch(() => undefined);
  return settled;
}

/** Starts one Worker pass that a test holds through `extraEnv`, and gives its process so that the test can kill it. */
export function heldWorker(extraEnv: Readonly<Record<string, string>>): { child: ChildProcess; exited: Promise<void> } {
  const child = execFile(packaged("storyos-worker"), ["--once"], heldWorkerOptions(extraEnv));
  const exited = new Promise<void>((resolve) => {
    child.once("exit", () => resolve());
    child.once("error", () => resolve());
  });
  return { child, exited };
}

function heldWorkerOptions(extraEnv: Readonly<Record<string, string>>) {
  const env = { ...process.env, ...extraEnv };
  if (process.env.STORYOS_TEST_DATABASE_URL !== undefined) env.STORYOS_DATABASE_URL = process.env.STORYOS_TEST_DATABASE_URL;
  return { cwd: repositoryRoot, env, timeout: 60_000, killSignal: "SIGKILL" as const };
}
