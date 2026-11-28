import { afterEach, beforeEach, expect, it } from "vitest";
import { userEvent } from "vitest/browser";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import deleteChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/delete-chapter.json";
import setCurrentChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-current-chapter.json";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { POINTER_STAY_MS, createChapterCopies } from "../../src/chapter-copies.ts";
import { AUTHOR_EDIT_BATCH_IDLE_MS } from "../../src/local-edit-journal.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { CHAPTER_SWITCH_WAITING_SENTENCE } from "../../src/chapter-navigation.ts";
import { applyImeComposition, applyTrustedInput, updateClipboardPermission } from "../support/browser-command-client.ts";
import { beginTreeAction } from "../support/inline-chapter-creation.ts";
import { focusManuscriptEnd, manuscriptBody, manuscriptEditor, manuscriptIsEditable } from "../support/manuscript-surface.ts";
import {
  BLOCK, CHAPTER, OWNER, PROJECT, REVISION, SESSION, chapterRevision, closeTrackedDatabases, createAppliedActivityEvent,
  createAppliedAuthorEditResponse, createBrowserScenario, deleteJournal, jsonResponse, requestHeaders, requireDigestValue,
  trackDatabase,
} from "./scenario.ts";

// The Arriving Chapter of a selection in the contents (ADR 0105, sections 1 and 2).
const CHAPTER_B = "018f0000-0000-7001-8000-000000000803";
const CHAPTER_C = "018f0000-0000-7001-8000-000000000804";
const VOLUME = "018f0000-0000-7001-8000-000000000815";
const P = `/api/v1/projects/${PROJECT}`;
const READ_B = `GET ${P}/chapters/${CHAPTER_B}`;
const READ_C = `GET ${P}/chapters/${CHAPTER_C}`;
const CHALLENGE = `POST ${P}/anti-forgery-challenges`;
const SWITCH = `PUT ${P}/current-chapter`;
const AUTHOR_EDITS = `POST ${P}/manuscript/author-edits`;
const ACTIVITY = `GET ${P}/activity`;
const SEARCH = `POST ${P}/queries/manuscript-search`;
const SESSION_READ = `GET ${P}/editor-sessions/${SESSION}`;
const UNDO = `POST ${P}/author-actions/undo`;
const openProposals = (chapterId: string) => `GET ${P}/chapters/${chapterId}/open-proposals`;
const PROPOSAL = "018f0000-0000-7001-8000-000000000471";

type Text = { revision: string; title: string; body: string };
type Primitive = { kind: string; from: number; to: number; text: string };
type Timer = { callback: () => void; cleared: boolean; fired: boolean };

async function sha256(body: string): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * A fake Server with Chapters A (current), B, and C. It serves only the requests of these tests, records each request
 * with no filter, and throws on each other request. `hold(name)` holds the next request of that name until
 * `release(name)`, which releases the earliest held request of that name. `refuse` answers each request of the given names with a refusal. A switch settles a new Snapshot,
 * which the Editor Session base and the switch response name. `activity` is the frame of the Activity read. With
 * `assistance`, the writing assistant is available. With `undo`, the Editor Session has an Author Action to undo, and
 * the Server refuses the Undo. `lists` gives, for a Chapter, the Revision of the one open Proposal `PROPOSAL` in its
 * list. The Server does not find the Proposal itself. `effect` is the effect of each switch, which then changes nothing.
 * With `limited`, the Server refuses the next
 * Challenge with a Challenge rate limit. With `emptyAfterSwitch`, the switch response names its Chapter, and each later
 * read of the Project gives a Project with no Current Chapter. With `truncated`, each list of open suggestions is not
 * complete.
 * `answered` has each request whose response is complete. With `acceptEdits`, each Author Edit applies the primitives of
 * its units, in their order, to the text of the Current Chapter, and `editRequests` has its request. A name in `refuse` of the open
 * suggestions of a Chapter fails its read. `moveTo` changes the Current Chapter as another tab does. `switches` has the
 * expected and the new Current Chapter of each Set Current Chapter command. As the Server does, the fake refuses a command
 * whose expected Current Chapter is not the Current Chapter with the conflict `stale_current_chapter`. With
 * `lostSwitch`, the next `lostSwitch` responses of a switch do not arrive, after the fake answered each one. A repeat
 * with the same idempotency key gets the response of the first one. The Server does not get the next `unreachableSwitch`
 * switch requests. A switch to a Chapter in `conflicts` has the conflict `wrong_target_head`. With `deleteChapter`, the
 * removal of Chapter C applies.
 * `holdAnswer(name)` holds the response of the next request of that name, after the fake answered it, until
 * `release(name)`. A name in `refuse` of the Activity read, the Project read, the Editor Session read, or the Command
 * Challenge has no answer.
 */
function arrivalServer() {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  const texts: Record<string, Text> = {
    [CHAPTER]: { revision: REVISION, title: "Chapter A", body: "Base" },
    [CHAPTER_B]: { revision: "018f0000-0000-7001-8000-000000000805", title: "Chapter B", body: "Beta" },
    [CHAPTER_C]: { revision: "018f0000-0000-7001-8000-000000000806", title: "Chapter C", body: "Gamma" },
  };
  let current = CHAPTER;
  let snapshot = { id: scenario.session.base_snapshot.snapshot_id, position: "0" };
  const holding = new Set<string>();
  const holdingAnswers = new Set<string>();
  const held = new Map<string, (() => void)[]>();
  const wait = (name: string) => new Promise<void>((resolve) => { held.set(name, [...held.get(name) ?? [], resolve]); });
  // The idempotency keys of the Author Edit commands that the network refused.
  const editKeys = new Set<string>();
  // The command digest that the Challenge of the next Author Edit names.
  let editDigest: ReturnType<typeof requireDigestValue> | undefined;
  // The response of each applied switch, by its idempotency key.
  const switchReplies = new Map<string, unknown>();
  const server = { requests: [] as string[], unexpected: [] as string[], texts, refuse: new Set<string>(),
    activity: undefined as unknown, assistance: false, undo: false, editKeys, lists: {} as Record<string, string>,
    effect: undefined as unknown, limited: false, emptyAfterSwitch: false,
    answered: [] as string[], acceptEdits: false, editRequests: [] as Record<string, unknown>[], truncated: false,
    run: undefined as ({ run_id: string } & Record<string, unknown>) | undefined, switches: [] as string[][],
    lostSwitch: 0, unreachableSwitch: 0, conflicts: new Set<string>(), deleteChapter: false,
    hold(name: string) { holding.add(name); },
    holdAnswer(name: string) { holdingAnswers.add(name); },
    /** Another tab makes `chapterId` the Current Chapter. */
    moveTo(chapterId: string) { current = chapterId; },
    isHeld(name: string) { return (held.get(name)?.length ?? 0) > 0; },
    release(name: string) { held.get(name)?.shift()?.(); } };
  const session = async () => {
    const text = texts[current]!;
    const base = { ...scenario.session.base_snapshot, chapter_id: current, authoritative_head_revision_id: text.revision,
      target_refs: [`manuscript:${current}`], materialized_revision: chapterRevision(text.revision, text.body),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: await sha256(text.body) } };
    return { ...scenario.session, ...(server.undo ? { author_undo_frontier_sequence: "1" } : {}),
      base_snapshot: { ...base, snapshot_id: snapshot.id, project_activity_position: snapshot.position } };
  };
  // With `emptyAfterSwitch`, the Project becomes empty after the response of the switch.
  let empty = false;
  const open = () => empty ? { kind: "empty" as const }
    : { kind: "current_chapter" as const, current_chapter_id: current };
  const answer = async (name: string, init: RequestInit | undefined): Promise<Response> => {
    if (name === "GET /api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (name === "GET /api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: "Project A", revision: "1", open: open() }] });
    }
    if (name === `GET ${P}` && server.refuse.has(name)) throw new TypeError("Failed to fetch");
    if (name === `GET ${P}`) return jsonResponse({ ...scenario.project, project: { ...scenario.project.project, open: open() } });
    const chapterId = Object.keys(texts).find((id) => name === `GET ${P}/chapters/${id}`);
    if (chapterId !== undefined) {
      if (server.refuse.has(name)) {
        return jsonResponse({ schema_id: "storyos.problem.v1", code: "snapshot_expired",
          message: "The Snapshot is no longer available." }, 409);
      }
      const text = texts[chapterId]!;
      return jsonResponse({ ...scenario.chapter, project_activity_position: snapshot.position,
        chapter: { chapter_id: chapterId, title: text.title, current_revision: chapterRevision(text.revision, text.body) } });
    }
    const listed = Object.keys(texts).find((id) => name === openProposals(id));
    if (listed !== undefined && server.refuse.has(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (listed !== undefined) {
      return jsonResponse({ schema_id: "storyos.query.chapter-open-proposals.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000c71", project_scope: scope, chapter_id: listed,
        completeness: server.truncated ? "truncated" : "complete", proposals: server.lists[listed] === undefined ? [] : [{ kind: "block_edit",
          proposal_id: PROPOSAL, revision_id: server.lists[listed], source: { kind: "agent_run_decision",
            run_id: "018f0000-0000-7001-8000-000000000472", decision_id: "018f0000-0000-7001-8000-000000000473" } }] });
    }
    if (name === `GET ${P}/proposals/${PROPOSAL}` && (Object.keys(server.lists).length > 0 || server.run !== undefined)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (name === `GET ${P}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope, tree_revision: "3",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scope },
        volumes: [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [CHAPTER, CHAPTER_B, CHAPTER_C]
          .map((id, index) => ({ chapter_id: id, title: texts[id]!.title, order: String(index + 1) })) }] });
    }
    if (name === CHALLENGE && server.refuse.has(name)) throw new TypeError("Failed to fetch");
    if (name === CHALLENGE && server.limited) {
      server.limited = false;
      return new Response(JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
        message: "The command challenge rate limit is exceeded." }),
      { status: 429, headers: { "content-type": "application/json", "retry-after": "7" } });
    }
    if (name === CHALLENGE) {
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      if (request.command_schema === "storyos.command.apply-author-edit.request.v2") {
        editDigest = requireDigestValue(request.canonical_command_digest, "command digest");
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-09-12T00:00:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (name === `POST ${P}/editor-sessions`) return jsonResponse(await session());
    if (name === UNDO && server.undo) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (name === SESSION_READ && server.refuse.has(name)) throw new TypeError("Failed to fetch");
    if (name === SESSION_READ) {
      return jsonResponse({ ...await session(), schema_id: "storyos.query.editor-session.response.v2" });
    }
    if (name === ACTIVITY && server.refuse.has(name)) throw new TypeError("Failed to fetch");
    if (name === ACTIVITY) {
      return new Response(server.activity === undefined ? ""
        : `id: cursor-1\nevent: storyos.project-activity\ndata: ${JSON.stringify(server.activity)}\n\n`,
      { status: 200, headers: { "content-type": "text/event-stream" } });
    }
    if (name === SEARCH) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-search.response.v1", completeness: "complete",
        items: [{ chapter_id: CHAPTER_B, manuscript_block_id: "018f0000-0000-7001-8000-0000000000b1", start: "0",
          end: "4" }], lag: "0", projection_watermark: "1",
        source_snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scope } });
    }
    // With no network, the Author Edit command and the read of its outcome fail.
    if (name === AUTHOR_EDITS && server.refuse.has(name)) {
      editKeys.add(new Headers(init?.headers).get("idempotency-key") ?? "");
      throw new TypeError("Failed to fetch");
    }
    if ([...editKeys].some((key) => name === `GET ${P}/manuscript/author-edit-outcomes/${key}`)) {
      throw new TypeError("Failed to fetch");
    }
    if (name === AUTHOR_EDITS && server.acceptEdits) {
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      server.editRequests.push(request);
      const units = request.author_edit_units as { normalized_primitives: Primitive[] }[];
      const body = units.flatMap((unit) => unit.normalized_primitives).reduce((text, primitive) => {
        if (primitive.kind !== "replace_block_selection") throw new Error(`unexpected primitive: ${primitive.kind}`);
        return `${text.slice(0, primitive.from)}${primitive.text}${text.slice(primitive.to)}`;
      }, texts[current]!.body);
      const prior = texts[current]!.revision;
      const revision = `018f0000-0000-7001-8000-00000000096${server.editRequests.length}`;
      texts[current] = { ...texts[current]!, revision, body };
      const position = Number(snapshot.position) + 1;
      snapshot = { id: `018f0000-0000-7001-8000-00000000095${position}`, position: String(position) };
      return jsonResponse(createAppliedAuthorEditResponse({ request, body, priorRevisionId: prior,
        authoritativeRevisionId: revision, projectActivityPosition: snapshot.position,
        commandDigest: editDigest!,
        idempotencyKey: requestHeaders(init).get("idempotency-key") ?? "" }));
    }
    if (server.run !== undefined && name === `GET ${P}/agent-runs/${server.run.run_id}`) return jsonResponse(server.run);
    if (server.run !== undefined && name === `GET ${P}/agent-runs/${server.run.run_id}/research-sources`) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (name === `GET ${P}/assistance` && server.assistance) {
      return jsonResponse({ ...assistanceFixture, project_scope: scope });
    }
    if (name === `DELETE ${P}/chapters/${CHAPTER_C}` && server.refuse.has(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (name === `DELETE ${P}/chapters/${CHAPTER_C}` && server.deleteChapter) {
      const reply = JSON.parse(JSON.stringify(deleteChapterFixture).replaceAll(deleteChapterFixture.project.project_id,
        PROJECT)) as typeof deleteChapterFixture;
      return jsonResponse({ ...reply, project: { ...reply.project, open: open() },
        effect: { ...reply.effect, chapter_id: CHAPTER_C, volume_id: VOLUME } });
    }
    const switchInput = name !== SWITCH ? undefined : (JSON.parse(String(init?.body)) as {
      set_current_chapter_input: { chapter_id: string; expected_current_chapter_id: string } }).set_current_chapter_input;
    if (switchInput !== undefined) server.switches.push([switchInput.expected_current_chapter_id, switchInput.chapter_id]);
    const replied = switchReplies.get(requestHeaders(init).get("idempotency-key") ?? "");
    if (switchInput !== undefined && replied !== undefined) return jsonResponse(replied);
    if (switchInput !== undefined && server.effect === undefined && switchInput.expected_current_chapter_id !== current) {
      return jsonResponse({ ...setCurrentChapterFixture, project: { ...setCurrentChapterFixture.project, open: open() },
        effect: { kind: "conflicted", reason: "stale_current_chapter" },
        receipt: { ...setCurrentChapterFixture.receipt, result: "conflicted" } });
    }
    if (switchInput !== undefined && server.conflicts.has(switchInput.chapter_id)) {
      return jsonResponse({ ...setCurrentChapterFixture, project: { ...setCurrentChapterFixture.project, open: open() },
        effect: { kind: "conflicted", reason: "wrong_target_head" },
        receipt: { ...setCurrentChapterFixture.receipt, result: "conflicted" } });
    }
    if (name === SWITCH && server.refuse.has(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (name === SWITCH && server.effect !== undefined) {
      const effect = server.effect as { kind: string };
      return jsonResponse({ ...setCurrentChapterFixture, project: { ...setCurrentChapterFixture.project, open: open() },
        effect, receipt: { ...setCurrentChapterFixture.receipt, result: effect.kind } });
    }
    if (name === SWITCH) {
      current = (JSON.parse(String(init?.body)) as { set_current_chapter_input: { chapter_id: string } })
        .set_current_chapter_input.chapter_id;
      const position = Number(snapshot.position) + 1;
      snapshot = { id: `018f0000-0000-7001-8000-00000000094${position}`, position: String(position) };
      const revision = texts[current]!.revision;
      const reply = { ...setCurrentChapterFixture,
        project: { ...setCurrentChapterFixture.project, open: open() },
        effect: { ...setCurrentChapterFixture.effect, base_snapshot_id: snapshot.id, current_chapter_id: current,
          project_activity_position: snapshot.position },
        receipt: { ...setCurrentChapterFixture.receipt, expected_heads: [revision], prior_heads: [revision],
          resulting_heads: [revision] } };
      switchReplies.set(requestHeaders(init).get("idempotency-key") ?? "", reply);
      empty = server.emptyAfterSwitch;
      return jsonResponse(reply);
    }
    // The tests are not about these panels.
    if ([`GET ${P}/manuscript/statistics`, `GET ${P}/assistance`, `GET ${P}/skills`].includes(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    server.unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    server.requests.push(name);
    if (holding.delete(name)) await wait(name);
    if (name === SWITCH && server.unreachableSwitch > 0) {
      server.unreachableSwitch -= 1;
      throw new TypeError("Failed to fetch");
    }
    const response = await answer(name, init);
    if (name === SWITCH && server.lostSwitch > 0) {
      server.lostSwitch -= 1;
      throw new TypeError("Failed to fetch");
    }
    if (holdingAnswers.delete(name)) await wait(name);
    server.answered.push(name);
    return response;
  };
  return { scenario, server, fetchImpl };
}

const openDatabases = new Set<IDBDatabase>();
// The workspace that opens again after a switch opens the Journal again, so the test closes each connection.
const openJournal = indexedDB.open.bind(indexedDB);
const unmounts: (() => void)[] = [];
const servers: ReturnType<typeof arrivalServer>["server"][] = [];
// The stores of the read of the Acceptance records of the Local Edit Journal. While `failAcceptanceRead` is true, a
// transaction of these stores fails, as a read of the Journal can fail.
const ACCEPTANCE_STORES = ["intents", "submission_groups", "transport_capsules", "transport_attempts"].join();
let failAcceptanceRead = false;
// The number of the next opens of the Journal that fail.
let failJournalOpens = 0;

beforeEach(() => {
  failAcceptanceRead = false;
  failJournalOpens = 0;
  Object.defineProperty(indexedDB, "open", { configurable: true, value: (name: string, version?: number) => {
    if (failJournalOpens > 0) {
      failJournalOpens -= 1;
      throw new DOMException("The Journal did not open.", "UnknownError");
    }
    const request = openJournal(name, version);
    request.addEventListener("success", () => {
      const database = request.result;
      trackDatabase(database, openDatabases);
      const transaction = database.transaction.bind(database);
      database.transaction = ((stores, mode, options) => {
        if (failAcceptanceRead && [stores].flat().join() === ACCEPTANCE_STORES) {
          throw new DOMException("The read of the Journal failed.", "UnknownError");
        }
        return transaction(stores, mode, options);
      }) as IDBDatabase["transaction"];
    });
    return request;
  } });
});

afterEach(async () => {
  const unexpected = servers.splice(0).flatMap((server) => server.unexpected);
  for (const unmount of unmounts.splice(0)) unmount();
  Reflect.deleteProperty(indexedDB, "open");
  closeTrackedDatabases(openDatabases);
  document.body.replaceChildren();
  sessionStorage.clear();
  await deleteJournal(createBrowserScenario().journalName);
  expect(unexpected).toEqual([]);
});

/**
 * Opens the workspace of Chapter A with the writer. The stay timers of the pointer wait until the test runs them; each
 * other timer is a real timer.
 */
async function openWorkspace(prepare: (server: ReturnType<typeof arrivalServer>["server"]) => void = () => {},
  { editable = true } = {}) {
  const { scenario, server, fetchImpl } = arrivalServer();
  servers.push(server);
  prepare(server);
  await deleteJournal(scenario.journalName);
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const stays: Timer[] = [];
  // The wait for the Retry-After of a Challenge rate limit, which the test ends.
  const waits: (() => void)[] = [];
  // After `stopIdle()`, an idle timer of the Author Edit batch runs only when the test calls `runIdle()`. Then the
  // writing controller reads the outcome of a refused Author Edit no second time while a test examines the requests of
  // the quiet step.
  let idleStopped = false;
  const idles: { callback: () => void; cleared: boolean }[] = [];
  const stopIdle = () => { idleStopped = true; };
  /** Runs each idle timer that is not cleared, and gives false when no idle timer waits. */
  const runIdle = () => {
    const live = idles.filter((idle) => !idle.cleared);
    for (const idle of live) {
      idle.cleared = true;
      idle.callback();
    }
    return live.length > 0;
  };
  const view = mountStage1View(loaded.root, { ...loaded, writingTimers: {
    setTimeoutImpl: (callback, timeout) => timeout === POINTER_STAY_MS
      ? -stays.push({ callback, cleared: false, fired: false })
      : timeout === 7_000 ? -1_000 - waits.push(callback)
      : idleStopped && timeout === AUTHOR_EDIT_BATCH_IDLE_MS ? -100_000 - idles.push({ callback, cleared: false })
      : globalThis.setTimeout(callback, timeout),
    // A cleared wait stays in `waits`, and the test does not run it.
    clearTimeoutImpl: (timer) => {
      if (typeof timer !== "number" || timer >= 0) globalThis.clearTimeout(timer);
      else if (timer <= -100_000) idles[-timer - 100_001]!.cleared = true;
      else if (timer > -1_000) stays[-timer - 1]!.cleared = true;
    },
  } });
  unmounts.push(() => view.unmount());
  const root = loaded.root;
  const button = (chapterId: string) =>
    root.querySelector<HTMLButtonElement>(`.tree-panel button[data-chapter-id="${chapterId}"]`)!;
  await expect.poll(() => root.querySelector(`.tree-panel button[data-chapter-id="${CHAPTER_C}"]`)).not.toBeNull();
  await expect.poll(() => manuscriptIsEditable(manuscriptEditor(root, window))).toBe(editable);
  const editor = () => manuscriptEditor(root, window);
  const shown = () => ({ title: root.querySelector(".editor-panel h2")?.textContent, body: manuscriptBody(editor()),
    editable: manuscriptIsEditable(editor()), alert: root.querySelector('.editor-panel [role="alert"]')?.textContent });
  /**
   * The screen when no arrival is in progress. An Arriving Chapter can be editable, so during an arrival the result has
   * one more key, `arriving`, and is not equal to the screen of a ready editor.
   */
  const ready = () => {
    const arriving = root.querySelector("[data-arrival-no-open-suggestion]")?.getAttribute("data-arrival-no-open-suggestion");
    return { ...shown(), ...(arriving === "" ? {} : { arriving }) };
  };
  /** The fact "no open suggestion" of the Arriving Chapter on the screen. */
  const fact = () => root.querySelector("[data-arrival-no-open-suggestion]")?.getAttribute("data-arrival-no-open-suggestion");
  const pointer = (chapterId: string, inside: boolean) => button(chapterId)
    .dispatchEvent(new PointerEvent(inside ? "pointerover" : "pointerout", { bubbles: true }));
  /** Runs each stay timer that waits. */
  const stay = () => {
    const waiting = stays.filter((timer) => !timer.cleared && !timer.fired);
    for (const timer of waiting) { timer.fired = true; timer.callback(); }
    return waiting.length;
  };
  /** The number of stay timers that a stay of the pointer on a Chapter starts. A Chapter with a copy starts none. */
  const stayTimers = (chapterId: string) => {
    pointer(chapterId, true);
    const started = stays.filter((timer) => !timer.cleared && !timer.fired).length;
    pointer(chapterId, false);
    return started;
  };
  /**
   * Keeps the pointer on a Chapter until the tab has its copy. A copy with no fact "no open suggestion" reads its open
   * suggestions again at each stay (#1934). Such a read starts only after the copy has text, so then the helper waits
   * for one such read and its answer.
   */
  const copy = async (chapterId: string) => {
    const list = openProposals(chapterId);
    const lists = (names: readonly string[]) => names.filter((name) => name === list).length;
    pointer(chapterId, true);
    expect(stay()).toBe(1);
    pointer(chapterId, false);
    await expect.poll(() => [`GET ${P}/chapters/${chapterId}`, list].every((name) => server.answered.includes(name)))
      .toBe(true);
    const before = lists(server.requests);
    await expect.poll(() => {
      if (stayTimers(chapterId) === 0) return true;
      pointer(chapterId, true);
      stay();
      pointer(chapterId, false);
      return lists(server.requests) > before;
    }).toBe(true);
    await expect.poll(() => lists(server.answered)).toBe(lists(server.requests));
  };
  // Each read of the open answered before a test starts, so no read of the open comes after `requests.length`. A read
  // that the test holds in `prepare` waits until the test releases it.
  const openReads = [openProposals(CHAPTER), `GET ${P}/assistance`, `GET ${P}/skills`, `GET ${P}/manuscript/tree`,
    ACTIVITY, `GET ${P}/manuscript/statistics`];
  if (editable) {
    await expect.poll(() => openReads.filter((name) => !server.answered.includes(name) && !server.isHeld(name)))
      .toEqual([]);
  }
  /** The notice of Arrival Input that is not saved, and whether it is directly under the Chapter title. */
  const notice = () => {
    const element = root.querySelector("[data-arrival-notice]");
    return element === null ? undefined : { role: element.getAttribute("role"),
      chapter: element.getAttribute("data-arrival-notice"), heading: element.querySelector("h3")?.textContent,
      sentence: element.querySelector("h3 + p")?.textContent,
      text: element.querySelector("[data-arrival-notice-text]")?.textContent,
      underTitle: element.previousElementSibling?.matches(".editor-panel h2") === true };
  };
  const saveState = () => root.querySelector("[data-save-state]")?.getAttribute("data-save-state");
  /** The text of the save state in the footer of the editor. */
  const footer = () => root.querySelector("[data-save-state]")?.textContent;
  /** The text of each notice, in order. */
  const noticeTexts = () => [...root.querySelectorAll("[data-arrival-notice-text]")].map((text) => text.textContent);
  /**
   * Types `text` with trusted input: at the end of the shown editor when it is editable. A read-only editor cannot have
   * the focus, so then the key goes to the Chapter button in the contents, which has the focus after a click.
   */
  const typeAtEnd = async (text: string, chapterId = CHAPTER_B) => {
    if (manuscriptIsEditable(editor())) focusManuscriptEnd(editor(), window);
    else button(chapterId).focus();
    await applyTrustedInput({ operation: "insert_text", text });
  };
  return { server, root, button, editor, shown, ready, fact, pointer, stay, stays, stayTimers, copy, notice, noticeTexts,
    saveState, footer,
    typeAtEnd, waits, stopIdle, runIdle, journal: scenario.journalName };
}

/** Each title that the editor part showed, in order. The observer sees each change of the page. */
function watchTitles(root: HTMLElement) {
  const titles: string[] = [];
  const observer = new MutationObserver(() => {
    const title = root.querySelector(".editor-panel h2")?.textContent ?? "";
    if (titles.at(-1) !== title) titles.push(title);
  });
  observer.observe(root, { subtree: true, childList: true, characterData: true });
  return { titles, stop: () => observer.disconnect() };
}

it("reads a Chapter and its open suggestions when the pointer stays on it, with no command and no Challenge", async () => {
  const workspace = await openWorkspace();
  const before = workspace.server.requests.length;

  // The read of the open gave the copy of the Current Chapter.
  expect(workspace.stayTimers(CHAPTER)).toBe(0);
  // A pointer that only moves across the list starts no read.
  workspace.pointer(CHAPTER_B, true);
  workspace.pointer(CHAPTER_B, false);
  expect({ stays: workspace.stay(), cleared: workspace.stays.map((timer) => timer.cleared) })
    .toEqual({ stays: 0, cleared: [true] });

  workspace.pointer(CHAPTER_B, true);
  expect(workspace.stay()).toBe(1);
  workspace.pointer(CHAPTER_B, false);
  // A second stay on a Chapter with a copy starts no read.
  await expect.poll(() => workspace.stayTimers(CHAPTER_B)).toBe(0);
  expect(workspace.server.requests.slice(before)).toEqual([READ_B, openProposals(CHAPTER_B)]);
  // A Chapter has at most one such read in flight.
  workspace.server.hold(READ_C);
  workspace.pointer(CHAPTER_C, true);
  expect(workspace.stay()).toBe(1);
  await expect.poll(() => workspace.server.isHeld(READ_C)).toBe(true);
  workspace.pointer(CHAPTER_C, false);
  workspace.pointer(CHAPTER_C, true);
  expect(workspace.stay()).toBe(1);
  workspace.pointer(CHAPTER_C, false);
  workspace.server.release(READ_C);
  await expect.poll(() => workspace.stayTimers(CHAPTER_C)).toBe(0);
  expect(workspace.server.requests.slice(before)).toEqual([READ_B, openProposals(CHAPTER_B), READ_C,
    openProposals(CHAPTER_C)]);
});

// A copy with text and no list of its open suggestions, as after a failed list, the read-only view of a Chapter, or a
// switch (#1934).
it("reads only the open suggestions at the pointer when a copy has text and no list of them", async () => {
  const workspace = await openWorkspace((server) => { server.refuse.add(openProposals(CHAPTER_B)); });
  workspace.pointer(CHAPTER_B, true);
  expect(workspace.stay()).toBe(1);
  workspace.pointer(CHAPTER_B, false);
  await expect.poll(() => [READ_B, openProposals(CHAPTER_B)].every((name) => workspace.server.answered.includes(name)))
    .toBe(true);
  workspace.server.refuse.delete(openProposals(CHAPTER_B));
  const before = workspace.server.requests.length;

  // A stay while the first read is in flight starts no read, so the first new read starts after the copy has text.
  await expect.poll(() => {
    workspace.pointer(CHAPTER_B, true);
    workspace.stay();
    workspace.pointer(CHAPTER_B, false);
    return workspace.server.requests.length > before;
  }).toBe(true);

  await expect.poll(() => workspace.stayTimers(CHAPTER_B)).toBe(0);
  expect(workspace.server.requests.slice(before)).toEqual([openProposals(CHAPTER_B)]);
});

// A copy with text and no fact "no open suggestion" reads the open suggestions again at each stay (#1934).
it.each([
  { reason: "a Proposal change came after the list of the copy", events: "1" },
  { reason: "the list of the copy has an open suggestion", events: "0" },
])("reads the open suggestions again at the pointer when $reason", async ({ reason, events }) => {
  const workspace = await openWorkspace((server) => {
    server.hold(ACTIVITY);
    if (events === "0") server.lists[CHAPTER_B] = "018f0000-0000-7001-8000-000000000474";
  });
  await workspace.copy(CHAPTER_B);
  if (events === "1") workspace.server.activity = await runEvent();
  workspace.server.release(ACTIVITY);
  await expect.poll(() => workspace.root.querySelector("[data-activity-processed-through]")
    ?.getAttribute("data-activity-processed-through")).toBe(events);
  const before = workspace.server.requests.length;
  const answered = workspace.server.answered.length;

  workspace.pointer(CHAPTER_B, true);
  expect({ reason, stays: workspace.stay() }).toEqual({ reason, stays: 1 });
  workspace.pointer(CHAPTER_B, false);

  await expect.poll(() => workspace.server.answered.length - answered).toBe(1);
  expect(workspace.server.requests.slice(before)).toEqual([openProposals(CHAPTER_B)]);
});

it.each([
  { first: READ_B, second: openProposals(CHAPTER_B) },
  { first: openProposals(CHAPTER_B), second: READ_B },
])("writes each result of the pointer read when it answers, when $first answers first", async ({ first, second }) => {
  const workspace = await openWorkspace();
  workspace.server.hold(READ_B);
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.pointer(CHAPTER_B, true);
  expect(workspace.stay()).toBe(1);
  workspace.pointer(CHAPTER_B, false);
  await expect.poll(() => [first, second].every((name) => workspace.server.isHeld(name))).toBe(true);

  workspace.server.release(first);
  // A copy with text and no list of its open suggestions still reads at a stay (#1934).
  expect(workspace.stayTimers(CHAPTER_B)).toBe(1);
  workspace.server.release(second);
  await expect.poll(() => workspace.stayTimers(CHAPTER_B)).toBe(0);
  workspace.server.hold(READ_B);
  workspace.button(CHAPTER_B).click();

  await expect.poll(() => ({ ...workspace.shown(), fact: workspace.fact() })).toEqual({ title: "Chapter B", body: "Beta",
    editable: true, alert: undefined, fact: "true" });
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  workspace.server.release(READ_B);
  await expect.poll(workspace.ready).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
});

it.each([
  // From a selection in the contents, the one editable editor element is the editor of the Arriving Chapter.
  { source: "contents", during: { title: "Chapter B", body: "Beta", editable: true, alert: undefined, editors: 1 } },
  { source: "a search result", during: { title: "Chapter A", body: "Base", editable: true, alert: undefined,
    editors: 1 } },
  // A search result has no Arriving Chapter, so its switch reads no open suggestions of a Chapter with no copy (#1934).
  { source: "a search result with no copy", during: { title: "Chapter A", body: "Base", editable: true,
    alert: undefined, editors: 1 } },
])("shows the selection from $source, and sends the requests of a switch of today in the same order", async ({
  source, during }) => {
  const workspace = await openWorkspace();
  if (source !== "a search result with no copy") await workspace.copy(CHAPTER_B);
  if (source !== "contents") {
    const query = workspace.root.querySelector<HTMLInputElement>('input[name="manuscript-search-query"]')!;
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(query, "Beta");
    query.dispatchEvent(new Event("input", { bubbles: true }));
    query.form!.requestSubmit();
    await expect.poll(() => workspace.root.querySelector("[data-search-match] button")).not.toBeNull();
  }
  const before = workspace.server.requests.length;
  workspace.server.hold(READ_B);

  (source === "contents" ? workspace.button(CHAPTER_B)
    : workspace.root.querySelector<HTMLButtonElement>("[data-search-match] button")!).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await expect.poll(() => ({ ...workspace.shown(),
    editors: document.querySelectorAll('[contenteditable="true"]').length })).toEqual(during);
  workspace.server.release(READ_B);

  await expect.poll(workspace.ready).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
  // The reopen reads the Chapter, the Project, the list, and the Editor Session at the same time. Then the panels of
  // the new editor part read with no order between them: the Activity read starts after a read of the Journal.
  const steps = () => workspace.server.requests.slice(before);
  await expect.poll(() => [...steps().slice(0, 7), ...steps().slice(7).sort()]).toEqual([READ_B, CHALLENGE, SWITCH,
    READ_B, `GET ${P}`, "GET /api/v1/projects", `GET ${P}/editor-sessions/${SESSION}`,
    ...[openProposals(CHAPTER_B), `GET ${P}/manuscript/tree`, ACTIVITY, `GET ${P}/manuscript/statistics`].sort()]);
});

it("replaces the shown text and the copy when the Chapter read of the switch gives a newer Revision", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.texts[CHAPTER_B] = { revision: "018f0000-0000-7001-8000-000000000807", title: "Chapter B",
    body: "Beta two" };
  workspace.server.refuse.add(SWITCH);
  workspace.server.hold(CHALLENGE);

  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
  const replaced = workspace.shown();
  workspace.server.release(CHALLENGE);
  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: "无法打开这一章。" });
  // The next selection shows the newer Revision from the copy before its Chapter read answers.
  workspace.server.hold(READ_B);
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  const again = workspace.shown();
  workspace.server.release(READ_B);
  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: "无法打开这一章。" });

  expect({ replaced, again }).toEqual({
    replaced: { title: "Chapter B", body: "Beta two", editable: true, alert: undefined },
    // The sentence of the earlier end stays until the next end, as today.
    again: { title: "Chapter B", body: "Beta two", editable: true, alert: "无法打开这一章。" },
  });
});

it.each([
  // The quiet step reads the outcome of the Author Edit that the network refused.
  { end: "the quiet step refuses", refuse: AUTHOR_EDITS, body: "Base!", alert: "无法切换章节：请先完成当前输入。",
    steps: ["outcome"] },
  // The command of today sends its request again one time after the refusal.
  { end: "the command is refused", refuse: SWITCH, body: "Base", alert: "无法打开这一章。",
    steps: [READ_B, CHALLENGE, SWITCH, SWITCH] },
  { end: "the Chapter read fails", refuse: READ_B, body: "Base", alert: "无法打开这一章：快照已过期。", steps: [READ_B] },
])("shows the earlier Chapter editable again with the sentence and the requests of today when $end", async ({
  refuse, body, alert, steps }) => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.refuse.add(refuse);
  if (refuse === AUTHOR_EDITS) {
    workspace.stopIdle();
    focusManuscriptEnd(workspace.editor(), window);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await expect.poll(workspace.runIdle).toBe(true);
    await expect.poll(() => workspace.server.requests.includes(AUTHOR_EDITS)).toBe(true);
  }
  const watch = watchTitles(workspace.root);
  const before = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();

  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body, editable: true, alert });
  watch.stop();
  // The display of the earlier Chapter stayed, so it reads its open suggestions no second time.
  expect({ titles: watch.titles, steps: workspace.server.requests.slice(before) }).toEqual({
    titles: ["Chapter B", "Chapter A"], steps: steps.map((step) => step !== "outcome" ? step
      : `GET ${P}/manuscript/author-edit-outcomes/${[...workspace.server.editKeys].join()}`) });
});

it("keeps the earlier editor during an IME composition, and the quiet step refuses as today", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  const editor = workspace.editor();
  focusManuscriptEnd(editor, window);
  editor.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
  const before = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();

  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: "无法切换章节：请先完成当前输入。" });
  const during = { connected: editor.isConnected, steps: workspace.server.requests.slice(before) };
  // The composition ends in the earlier editor, and then a selection switches as today.
  editor.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "" }));
  workspace.button(CHAPTER_B).click();
  await expect.poll(workspace.ready).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
  expect(during).toEqual({ connected: true, steps: [] });
});

it("keeps the earlier editor during an Author Undo, so a switch that does not reach the editor keeps the Undo", async () => {
  const workspace = await openWorkspace((server) => { server.undo = true; });
  await workspace.copy(CHAPTER_B);
  const editor = workspace.editor();
  workspace.server.refuse.add(READ_B);
  workspace.server.hold(SESSION_READ);
  focusManuscriptEnd(editor, window);
  await applyTrustedInput({ operation: "undo_latest_author_action" });
  await expect.poll(() => workspace.server.isHeld(SESSION_READ)).toBe(true);
  const selected = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();

  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: "无法打开这一章：快照已过期。" });
  const connected = editor.isConnected;
  const before = workspace.server.requests.length;
  const switchSteps = workspace.server.requests.slice(selected);
  workspace.server.release(SESSION_READ);
  // From the selection to the end, the switch sends the requests of today. The Undo continues after the end of the
  // switch: it sends its Challenge and its command. The command of today sends its request again one time after the
  // refusal, and the failure reads the statistics again.
  await expect.poll(() => workspace.server.requests.slice(before)).toEqual([CHALLENGE, UNDO, UNDO,
    `GET ${P}/manuscript/statistics`]);
  expect({ connected, switchSteps }).toEqual({ connected: true, switchSteps: [READ_B] });
});

it("sends no message to the assistant during the arrival", async () => {
  const workspace = await openWorkspace((server) => { server.assistance = true; });
  await workspace.copy(CHAPTER_B);
  const message = workspace.root.querySelector<HTMLTextAreaElement>('textarea[name="assistant-message"]')!;
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(message, "Help.");
  message.dispatchEvent(new Event("input", { bubbles: true }));
  const send = () => workspace.root.querySelector<HTMLButtonElement>("button.send-button")?.disabled;
  await expect.poll(send).toBe(false);
  workspace.server.hold(READ_B);

  workspace.button(CHAPTER_B).click();

  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  const during = { title: workspace.shown().title, disabled: send() };
  workspace.server.release(READ_B);
  await expect.poll(() => ({ editable: workspace.shown().editable, disabled: send() }))
    .toEqual({ editable: true, disabled: false });
  expect(during).toEqual({ title: "Chapter B", disabled: true });
});

it.each([
  { activity: "a Run event before the selection", facts: ["false"] },
  { activity: "no event", facts: ["true"] },
  { activity: "a Run event during the arrival", facts: ["true", "false"] },
])("keeps the fact of no open suggestion in a copy only while the tab learned of no Proposal change ($activity)",
  async ({ activity, facts }) => {
    const workspace = await openWorkspace((server) => {
      server.hold(ACTIVITY);
    });
    await workspace.copy(CHAPTER_B);
    if (activity !== "no event") workspace.server.activity = await runEvent();
    const processed = () => workspace.root.querySelector("[data-activity-processed-through]")
      ?.getAttribute("data-activity-processed-through");
    const consume = async () => {
      workspace.server.release(ACTIVITY);
      await expect.poll(processed).toBe(activity === "no event" ? "0" : "1");
    };
    if (activity !== "a Run event during the arrival") await consume();
    workspace.server.hold(READ_B);

    workspace.button(CHAPTER_B).click();

    await expect.poll(workspace.fact).toBe(facts[0]);
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    // The screen follows the removal of the fact while the Chapter read of the switch waits.
    if (activity === "a Run event during the arrival") {
      await consume();
      await expect.poll(workspace.fact).toBe(facts[1]);
    }
    workspace.server.release(READ_B);
    await expect.poll(workspace.ready).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
  });

it("keeps the fact of no open suggestion from the settled display of the Chapter that the tab leaves", async () => {
  const workspace = await openWorkspace();
  workspace.button(CHAPTER_B).click();
  await expect.poll(workspace.ready).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
  workspace.server.hold(`GET ${P}/chapters/${CHAPTER}`);

  workspace.button(CHAPTER).click();

  await expect.poll(() => ({ ...workspace.shown(), fact: workspace.fact() })).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: undefined, fact: "true" });
  await expect.poll(() => workspace.server.isHeld(`GET ${P}/chapters/${CHAPTER}`)).toBe(true);
  workspace.server.release(`GET ${P}/chapters/${CHAPTER}`);
  await expect.poll(workspace.ready).toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined });
});

it("removes the fact of no open suggestion when a list of the same Proposal has a new Proposal Revision", async () => {
  const workspace = await openWorkspace((server) => {
    server.lists[CHAPTER_C] = "018f0000-0000-7001-8000-000000000474";
  });
  await workspace.copy(CHAPTER_C);
  await workspace.copy(CHAPTER_B);
  // Another tab edits the candidate text of the Proposal in Chapter C. The switch reads the list again, because the
  // copy has no fact, and then the display of Chapter C reads it (#1934).
  workspace.server.lists[CHAPTER_C] = "018f0000-0000-7001-8000-000000000475";
  const mark = workspace.server.requests.length;
  workspace.button(CHAPTER_C).click();
  await expect.poll(() => ({ title: workspace.shown().title, listed: workspace.server.requests.slice(mark)
    .filter((name) => name === openProposals(CHAPTER_C)).length })).toEqual({ title: "Chapter C", listed: 2 });
  await expect.poll(() => workspace.root.querySelector("[data-save-state]")?.getAttribute("data-save-state"))
    .toBe("saved");
  // The switch reads the open suggestions of Chapter B again, because its copy has no fact (#1934).
  workspace.server.hold(READ_B);
  workspace.server.hold(openProposals(CHAPTER_B));

  workspace.button(CHAPTER_B).click();

  await expect.poll(() => ({ fact: workspace.fact(), held: [READ_B, openProposals(CHAPTER_B)]
    .every(workspace.server.isHeld) })).toEqual({ fact: "false", held: true });
  workspace.server.release(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(workspace.ready).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
});

/** An `agent_run_created` event at stream position 1. */
async function runEvent() {
  const payload = { kind: "agent_run_created", project_agent_id: "018f0000-0000-7001-8000-000000000451",
    conversation_id: "018f0000-0000-7001-8000-000000000452",
    memory_settings_revision: "018f0000-0000-7001-8000-000000000453", run_id: "018f0000-0000-7001-8000-000000000454" };
  const base = await createAppliedActivityEvent({ eventId: "018f0000-0000-7001-8000-000000000455",
    commandId: "018f0000-0000-7001-8000-000000000456", receiptId: "018f0000-0000-7001-8000-000000000457",
    correlationId: "018f0000-0000-7001-8000-000000000458", revisionId: REVISION,
    commitId: "018f0000-0000-7001-8000-000000000459", sequence: "1", occurredAt: "2026-08-20T04:00:01.000Z" });
  return { ...base, event_schema: "storyos.event.agent-run-created.v1", event_kind: "agent_run_created",
    aggregate_ref: { kind: "agent_run", id: payload.run_id }, payload,
    payload_digest: { ...base.payload_digest, value_hex_lowercase: await sha256(JSON.stringify(payload)) } };
}

it("removes the fact of no open suggestion from each copy at a Proposal change, also for a read in flight", () => {
  const copies = createChapterCopies({ scope: createBrowserScenario().project.project_scope,
    baseUrl: location.origin, fetchImpl: async () => { throw new Error("no request"); } });
  const listed = (chapterId: string, proposalIds?: string[]) => copies.suggestionRead(chapterId)(proposalIds);
  const text = { title: "Chapter B", revisionId: "018f0000-0000-7001-8000-000000000805", blocks: [] };
  copies.write(CHAPTER_B, text);
  copies.write(CHAPTER_C, { ...text, title: "Chapter C" });
  listed(CHAPTER_B, []);
  listed(CHAPTER_C, []);
  const earlier = copies.suggestionRead(CHAPTER_B);
  const facts = () => [CHAPTER_B, CHAPTER_C].map((id) => copies.get(id)?.noOpenSuggestion);
  const before = facts();

  copies.proposalsChanged();
  earlier([]);
  const after = facts();
  listed(CHAPTER_B, []);
  copies.activity([{ event_id: "018f0000-0000-7001-8000-000000000455", event_kind: "agent_run_created" }]);
  const firstRun = facts();
  listed(CHAPTER_B, []);
  // A Run event that the copies saw before is no new change, and an event of another kind is no change.
  copies.activity([{ event_id: "018f0000-0000-7001-8000-000000000455", event_kind: "agent_run_created" },
    { event_id: "018f0000-0000-7001-8000-000000000460", event_kind: "authoritative_author_edit_applied" }]);
  const seen = facts();
  listed(CHAPTER_C, []);
  // A list of one Chapter that is not its last list is a change for each copy. The same list again is no change.
  listed(CHAPTER_C, ["018f0000-0000-7001-8000-000000000461"]);
  const newList = facts();
  listed(CHAPTER_B, []);
  listed(CHAPTER_C, ["018f0000-0000-7001-8000-000000000461"]);
  const sameList = facts();
  listed(CHAPTER_C);

  expect({ before, after, firstRun, seen, newList, sameList, incomplete: facts() }).toEqual({ before: [true, true],
    after: [false, false], firstRun: [false, false], seen: [true, false], newList: [false, false],
    sameList: [true, false], incomplete: [false, false] });
});

/** Each record of each store of the Local Edit Journal. */
async function journalRecords(name: string) {
  const database = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(name);
    request.onsuccess = () => { resolve(request.result); };
    request.onerror = () => { reject(request.error ?? new Error("no Journal")); };
  });
  try {
    const stores = [...database.objectStoreNames];
    const transaction = database.transaction(stores, "readonly");
    return Object.fromEntries(await Promise.all(stores.map((store) => new Promise<[string, unknown[]]>((resolve) => {
      const request = transaction.objectStore(store).getAll();
      request.onsuccess = () => { resolve([store, request.result]); };
    }))));
  } finally {
    database.close();
  }
}

/** The records of the Journal without the Editor Session base, which the open of an editor writes. */
function withoutBase(records: Record<string, unknown[]>) {
  return { ...records, metadata: records.metadata!.filter((record) =>
    !String((record as { key: string }).key).startsWith("active_base:")) };
}

const SWITCH_STEPS = [READ_B, CHALLENGE, SWITCH];
// The reopen reads the Chapter, the Project, the list, and the Editor Session at the same time. Then the panels of the
// new editor part read with no order between them.
const REOPEN_STEPS = [READ_B, `GET ${P}`, "GET /api/v1/projects", SESSION_READ];
const PANEL_READS = [openProposals(CHAPTER_B), `GET ${P}/manuscript/tree`, `GET ${P}/manuscript/statistics`];
// The reopen of a Chapter, and then the panels of its editor part with no order between them.
const reopenSteps = (chapterId: string) => [`GET ${P}/chapters/${chapterId}`, `GET ${P}`, "GET /api/v1/projects",
  SESSION_READ];
const panelReads = (chapterId: string) => [openProposals(chapterId), `GET ${P}/manuscript/tree`,
  `GET ${P}/manuscript/statistics`, ACTIVITY];

const ARRIVING_C = { title: "Chapter C", body: "Gamma", editable: true, alert: undefined, arriving: "true" };
const UNSAVED_HEADING = "这段文字没有保存";
const UNSAVED_SENTENCE = "切换章节时输入的文字没有保存到这一章。请复制这段文字，再粘贴到需要的位置。";

it.each([
  // The quiet step reads the outcome of the Author Edit that the network refused, and the test types while it waits.
  { end: "the quiet step refuses", hold: "outcome", steps: ["outcome"], panels: [],
    shown: { title: "Chapter A", body: "Base!", editable: true, alert: "无法切换章节：请先完成当前输入。" } },
  { end: "the switch is refused", effect: { kind: "refused", reason: "invalid_chapter_join" }, hold: READ_B,
    steps: SWITCH_STEPS, panels: [],
    shown: { title: "Chapter A", body: "Base", editable: true, alert: "无法打开这一章。" } },
  { end: "the switch has a conflict", effect: { kind: "conflicted", reason: "wrong_target_head" }, hold: READ_B,
    steps: SWITCH_STEPS, panels: [],
    shown: { title: "Chapter A", body: "Base", editable: true, alert: "无法打开这一章。" } },
  { end: "the Chapter read fails", refuse: READ_B, hold: READ_B, steps: [READ_B], panels: [],
    shown: { title: "Chapter A", body: "Base", editable: true, alert: "无法打开这一章：快照已过期。" } },
])("sends no Author Edit of the Arrival Input and shows it in a notice that stays when $end", async ({
  hold, effect, refuse, steps, panels, shown }) => {
  const workspace = await openWorkspace((server) => {
    server.effect = effect;
  });
  await workspace.copy(CHAPTER_B);
  if (hold === "outcome") {
    workspace.server.refuse.add(AUTHOR_EDITS);
    workspace.stopIdle();
    await workspace.typeAtEnd("!");
    await expect.poll(workspace.runIdle).toBe(true);
    await expect.poll(() => workspace.server.requests.includes(AUTHOR_EDITS)).toBe(true);
  }
  if (refuse !== undefined) workspace.server.refuse.add(refuse);
  const held = hold !== "outcome" ? hold
    : `GET ${P}/manuscript/author-edit-outcomes/${[...workspace.server.editKeys].join()}`;
  workspace.server.hold(held);
  const before = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(held)).toBe(true);
  await workspace.typeAtEnd("?");
  const typed = { ...workspace.shown(), save: workspace.saveState() };
  workspace.server.release(held);

  await expect.poll(() => ({ ...workspace.ready(), notice: workspace.notice() })).toEqual({ ...shown, notice: {
    role: "status", chapter: CHAPTER_B, heading: UNSAVED_HEADING, sentence: UNSAVED_SENTENCE, text: "Beta?",
    underTitle: true } });
  await expectRequests(workspace.server, before, steps.map((step) => step !== "outcome" ? step : held), panels);
  expect(typed).toEqual({ title: "Chapter B", body: "Beta?", editable: true, alert: undefined, save: "saving" });
});

/** A group of requests with no order between them. */
type AnyOrder = { anyOrder: string[] };

/**
 * Each request from `before`: the steps in their order, then the panels of the new editor part with no order, and then
 * each group of `after`, in its order or with no order.
 */
async function expectRequests(server: { requests: string[] }, before: number, steps: string[], panels: string[],
  ...after: (string[] | AnyOrder)[]) {
  const groups = [steps, { anyOrder: panels }, ...after];
  await expect.poll(() => {
    let start = before;
    return groups.flatMap((group) => {
      const names = Array.isArray(group) ? group : group.anyOrder;
      const sent = server.requests.slice(start, start += names.length);
      return Array.isArray(group) ? sent : sent.sort();
    }).concat(server.requests.slice(start));
  }).toEqual(groups.flatMap((group) => Array.isArray(group) ? group : [...group.anyOrder].sort()));
}

// The save of an Author Edit of Chapter B: the command, the read of the Editor Session, and then reads with no order.
const SAVE_B: (string[] | AnyOrder)[] = [[CHALLENGE, AUTHOR_EDITS, SESSION_READ],
  { anyOrder: [openProposals(CHAPTER_B), `GET ${P}/manuscript/statistics`, ACTIVITY] }];

const READY = { title: "Chapter B", body: "Beta", editable: true, alert: undefined };
// The footer while an Arriving Chapter accepts no input (#1934).
const OPENING = "正在打开本章…";

type Workspace = Awaited<ReturnType<typeof openWorkspace>>;

const RUN = "018f0000-0000-7001-8000-000000000c81";
// A change location of a Run in Chapter C. Its result then has a return to Chapter C.
const RETURN_C = { chapter_id: CHAPTER_C, manuscript_block_id: BLOCK,
  base_authoritative_revision_id: "018f0000-0000-7001-8000-000000000806", candidate_text: "Gamma two.",
  explanation: "Keep the voice.", current: null, outcome: { kind: "opened", proposal_id: PROPOSAL,
    operation_id: "018f0000-0000-7001-8000-000000000c85", revision_id: "018f0000-0000-7001-8000-000000000c86",
    validation_receipt_id: "018f0000-0000-7001-8000-000000000c87" } };

/** Stores an assistant request of this tab whose completed Run from `chapterId` opened a Proposal at `locations`. */
function seedRun(server: ReturnType<typeof arrivalServer>["server"], chapterId: string, locations: object[]) {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({ scope, message: "Help.", chapterId,
    correlationId: "018f0000-0000-7001-8000-000000000c82", idempotencyKey: "018f0000-0000-7001-8000-000000000c83",
    snapshotId: "018f0000-0000-7001-8000-000000000c84", runId: RUN, conversationId: agentRunFixture.conversation_id }));
  server.assistance = true;
  server.run = { ...agentRunFixture, project_scope: scope, run_id: RUN, status: "completed",
    decision: { kind: "prose_change", decision_id: "018f0000-0000-7001-8000-000000000473", selected: true,
      text: "Kept the voice.", producer_input: "[]", locations, continuation: { kind: "absent" },
      authoritative: false, opened_proposal: { kind: "present", proposal_id: PROPOSAL } } };
}

/** Puts the caret at `offset` in the text of the first paragraph of the editor, which has the focus. */
function caretAt(editor: HTMLElement, offset: number) {
  focusManuscriptEnd(editor, window);
  getSelection()!.collapse(editor.querySelector("p")!.firstChild!, offset);
  document.dispatchEvent(new Event("selectionchange"));
}

it.each([
  // The answer of the list ends the hand-over, and the Activity read still waits.
  { first: openProposals(CHAPTER_B), second: ACTIVITY, afterFirst: { arriving: undefined, save: "saving" } },
  // The answer of the Activity read changes nothing, and the hand-over still waits for the list.
  { first: ACTIVITY, second: openProposals(CHAPTER_B), afterFirst: { arriving: "true", save: "saving" } },
])("hands the Arrival Input over to the editor of the Chapter after the read of its open suggestions, when $first answers first",
  async ({ first, second, afterFirst }) => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.stopIdle();
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    await workspace.typeAtEnd("?");
    caretAt(workspace.editor(), 2);
    await applyTrustedInput({ operation: "insert_text", text: "-" });
    workspace.server.hold(openProposals(CHAPTER_B));
    workspace.server.hold(ACTIVITY);
    workspace.server.release(READ_B);
    await expect.poll(() => [first, second].every((name) => workspace.server.isHeld(name))).toBe(true);
    // The editor of the Chapter shows the Arrival Input. It has the caret of the Arriving Chapter, and the key goes to
    // the Arrival Input.
    await applyTrustedInput({ operation: "insert_text", text: "+" });
    const waiting = { ...workspace.ready(), save: workspace.saveState(),
      sent: workspace.server.requests.includes(AUTHOR_EDITS) };
    workspace.server.acceptEdits = true;
    const answered = workspace.server.answered.length;
    workspace.server.release(first);
    await expect.poll(() => workspace.server.answered.slice(answered)).toEqual([first]);
    await expect.poll(() => ({ arriving: workspace.ready().arriving, save: workspace.saveState() }))
      .toEqual(afterFirst);
    const beforeSave = workspace.server.requests.includes(AUTHOR_EDITS);
    workspace.server.release(second);
    await expect.poll(workspace.runIdle).toBe(true);
    await expect.poll(workspace.saveState).toBe("saved");
    // Input after the hand-over is normal input.
    await applyTrustedInput({ operation: "insert_text", text: "x" });
    await expect.poll(workspace.runIdle).toBe(true);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...READY, body: "Be-+xta?", save: "saved", notice: undefined });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
      ...SAVE_B, ...SAVE_B);
    expect({ waiting, beforeSave, stored: workspace.server.texts[CHAPTER_B]!.body }).toEqual({
      waiting: { ...READY, body: "Be-+ta?", arriving: "true", save: "saving", sent: false }, beforeSave: false,
      stored: "Be-+xta?" });
  });

it.each([
  { end: "the list has a Proposal", panels: [...PANEL_READS, ACTIVITY, `GET ${P}/proposals/${PROPOSAL}`],
    prepare: (server: ReturnType<typeof arrivalServer>["server"]) => {
      server.lists[CHAPTER_B] = "018f0000-0000-7001-8000-000000000475"; } },
  { end: "the list is not complete", panels: [...PANEL_READS, ACTIVITY],
    prepare: (server: ReturnType<typeof arrivalServer>["server"]) => { server.truncated = true; } },
  { end: "the read of the list fails", panels: [...PANEL_READS, ACTIVITY],
    prepare: (server: ReturnType<typeof arrivalServer>["server"]) => { server.refuse.add(openProposals(CHAPTER_B)); } },
])("sends no Author Edit of the Arrival Input, shows the text of the Server, and shows the input in a notice when $end",
  async ({ panels, prepare }) => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    prepare(workspace.server);
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    await workspace.typeAtEnd("?");

    workspace.server.release(READ_B);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...READY, save: "saved", notice: { role: "status", chapter: CHAPTER_B, heading: UNSAVED_HEADING,
        sentence: UNSAVED_SENTENCE, text: "Beta?", underTitle: true } });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], panels);
  });

it("sends no Author Edit of the Arrival Input when the base Revision of the editor is not the Revision of the copy",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.server.hold(CHALLENGE);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
    await workspace.typeAtEnd("?");
    // The Chapter changes after the Chapter read of the switch, so the reopen reads a newer Revision.
    workspace.server.texts[CHAPTER_B] = { revision: "018f0000-0000-7001-8000-000000000810", title: "Chapter B",
      body: "Beta three" };

    workspace.server.release(CHALLENGE);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice()?.text }))
      .toEqual({ ...READY, body: "Beta three", save: "saved", notice: "Beta?" });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY]);
  });

it("hands the Arrival Input over only after an IME composition that is open when the editor of the Chapter is ready ends",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.stopIdle();
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    const arriving = workspace.editor();
    focusManuscriptEnd(arriving, window);
    await applyImeComposition({ text: "zhong", replacementStart: 4, replacementEnd: 4, selectionStart: 5,
      selectionEnd: 5 });
    await expect.poll(() => manuscriptBody(arriving)).toBe("Betazhong");
    const answered = workspace.server.answered.length;

    workspace.server.release(READ_B);
    // The reads of the reopen answered. The editor of the Chapter waits for the end of the composition.
    await expect.poll(() => workspace.server.answered.slice(answered).filter((name) => name === SESSION_READ))
      .toEqual([SESSION_READ]);
    await applyImeComposition({ text: "zhongw", replacementStart: 4, replacementEnd: 9, selectionStart: 6,
      selectionEnd: 6 });
    const waiting = { same: workspace.editor() === arriving, body: manuscriptBody(arriving), save: workspace.saveState(),
      listed: workspace.server.requests.slice(before).includes(openProposals(CHAPTER_B)) };
    workspace.server.acceptEdits = true;
    // The end of the composition saves the input with no idle timer.
    await applyTrustedInput({ operation: "insert_text", text: "中" });
    await expect.poll(workspace.saveState).toBe("saved");
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await expect.poll(workspace.runIdle).toBe(true);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...READY, body: "Beta中!", save: "saved", notice: undefined });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
      ...SAVE_B, ...SAVE_B);
    expect({ waiting, replaced: workspace.editor() !== arriving, stored: workspace.server.texts[CHAPTER_B]!.body })
      .toEqual({ waiting: { same: true, body: "Betazhongw", save: "saving", listed: false }, replaced: true,
        stored: "Beta中!" });
  });

it("runs no Author Undo, adds no Selection Reference, and sends no message to the assistant while the hand-over waits",
  async () => {
  const workspace = await openWorkspace((server) => {
    server.undo = true;
    server.assistance = true;
  });
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  const message = workspace.root.querySelector<HTMLTextAreaElement>('textarea[name="assistant-message"]')!;
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(message, "Help.");
  message.dispatchEvent(new Event("input", { bubbles: true }));
  const send = () => workspace.root.querySelector<HTMLButtonElement>("button.send-button")?.disabled;
  await expect.poll(send).toBe(false);
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);

  // The Editor Session has an Author Action to undo. The undo key while the hand-over waits sends no request.
  await applyTrustedInput({ operation: "undo_latest_author_action" });
  await applyTrustedInput({ operation: "insert_text", text: "!" });
  const select = () => {
    const text = workspace.editor().querySelector("p")!.firstChild!;
    getSelection()!.setBaseAndExtent(text, 0, text, 2);
    document.dispatchEvent(new Event("selectionchange"));
  };
  const reference = () => workspace.root.querySelector("[data-add-selection-reference]")?.tagName;
  select();
  await expect.poll(() => getSelection()!.toString()).toBe("Be");
  const waiting = { ...workspace.ready(), disabled: send(), reference: reference() };
  workspace.server.acceptEdits = true;
  workspace.server.release(openProposals(CHAPTER_B));
  await expect.poll(workspace.runIdle).toBe(true);
  await expect.poll(workspace.saveState).toBe("saved");
  // After the hand-over, the same selection gives the control of a Selection Reference.
  select();

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), disabled: send(),
    reference: reference() })).toEqual({ ...READY, body: "Beta?!", save: "saved", disabled: false, reference: "BUTTON" });
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
    ...SAVE_B);
  expect(waiting).toEqual({ ...READY, body: "Beta?!", arriving: "true", disabled: true, reference: undefined });
  });

it("hands the Arrival Input over only after an IME composition in the editor of the Chapter ends", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);
  await applyImeComposition({ text: "zhong", replacementStart: 5, replacementEnd: 5, selectionStart: 5,
    selectionEnd: 5 });
  await expect.poll(() => manuscriptBody(workspace.editor())).toBe("Beta?zhong");
  const answered = workspace.server.answered.length;

  workspace.server.release(openProposals(CHAPTER_B));
  await expect.poll(() => workspace.server.answered.slice(answered)).toEqual([openProposals(CHAPTER_B)]);
  // The list has no suggestion, and the hand-over waits for the end of the composition.
  const waiting = { ...workspace.ready(), save: workspace.saveState() };
  // The end of a composition starts a new group, so the typed key and the composition are two Author Edits.
  workspace.server.acceptEdits = true;
  await applyTrustedInput({ operation: "insert_text", text: "中" });

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ ...READY, body: "Beta?中", save: "saved", notice: undefined });
  // The reads after the first save come with no order to the second save. The state line changes to "saved" one time,
  // so the statistics read one time.
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
    [CHALLENGE, AUTHOR_EDITS, SESSION_READ], { anyOrder: [openProposals(CHAPTER_B), ACTIVITY, CHALLENGE, AUTHOR_EDITS,
      SESSION_READ, openProposals(CHAPTER_B), ACTIVITY, `GET ${P}/manuscript/statistics`] });
  expect(waiting).toEqual({ ...READY, body: "Beta?zhong", arriving: "true", save: "saving" });
});

it.each([
  { report: "a Proposal in Chapter B", locations: [], listed: true, end: { ...READY, save: "saved", notice: "Beta?中" },
    saved: false },
  { report: "a Proposal in Chapter C", locations: [RETURN_C], listed: false,
    end: { ...READY, body: "Beta?中", save: "saved", notice: undefined }, saved: true },
  // The second change location of the same Proposal reports it one more time.
  { report: "a known Proposal again", locations: [RETURN_C, { ...RETURN_C, outcome: { ...RETURN_C.outcome,
    operation_id: "018f0000-0000-7001-8000-000000000c88" } }], listed: false,
    end: { ...READY, body: "Beta?中", save: "saved", notice: undefined }, saved: true },
])("decides the hand-over with the next list when an assistant Run reports $report during a composition", async ({
  locations, listed, end, saved }) => {
  // The assistant request of this tab has a Run that opened a Proposal. Its read answers while the hand-over waits for
  // the end of a composition.
  const run = `GET ${P}/agent-runs/${RUN}`;
  const workspace = await openWorkspace((server) => {
    seedRun(server, CHAPTER_B, locations);
    server.hold(run);
  });
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);
  await applyImeComposition({ text: "zhong", replacementStart: 5, replacementEnd: 5, selectionStart: 5,
    selectionEnd: 5 });
  await expect.poll(() => manuscriptBody(workspace.editor())).toBe("Beta?zhong");
  workspace.server.release(openProposals(CHAPTER_B));
  await expect.poll(() => workspace.server.answered.includes(openProposals(CHAPTER_B))).toBe(true);
  // The first list has no Proposal. Then the Run reports its Proposal, and the composition ends while the next list
  // waits. The earlier list does not decide.
  if (listed) workspace.server.lists[CHAPTER_B] = workspace.server.texts[CHAPTER_B]!.revision;
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(run);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);
  await applyTrustedInput({ operation: "insert_text", text: "中" });
  await expect.poll(() => manuscriptBody(workspace.editor())).toBe("Beta?中");
  const waiting = { ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice()?.text };
  workspace.server.acceptEdits = true;

  workspace.server.release(openProposals(CHAPTER_B));

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice()?.text }))
    .toEqual(end);
  // The read of the Proposal fails, and the tests are not about it. The typed key and the composition are two Author
  // Edits, and the reads after the first save come with no order to the second save.
  const proposal = `GET ${P}/proposals/${PROPOSAL}`;
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
    { anyOrder: [openProposals(CHAPTER_B), proposal, `${run}/research-sources`] },
    ...(saved ? [[CHALLENGE, AUTHOR_EDITS, SESSION_READ], { anyOrder: [openProposals(CHAPTER_B), proposal, ACTIVITY,
      CHALLENGE, AUTHOR_EDITS, SESSION_READ, openProposals(CHAPTER_B), proposal, ACTIVITY,
      `GET ${P}/manuscript/statistics`] }] : []));
  // The report of a Proposal ends the fact "no open suggestion" of each copy.
  expect(waiting).toEqual({ ...READY, body: "Beta?中", arriving: "false", save: "saving", notice: undefined });
});

it("shows the Arrival Input in a notice when the author removes a Chapter while the hand-over waits", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);
  const remove = `DELETE ${P}/chapters/${CHAPTER_C}`;
  workspace.server.refuse.add(remove);

  await beginTreeAction(workspace.root, `li[data-chapter-id="${CHAPTER_C}"]`, "[data-delete-chapter]");
  workspace.root.querySelector<HTMLButtonElement>(`[data-delete-chapter="${CHAPTER_C}"]`)!.click();
  await expect.poll(() => workspace.root.querySelector(`[data-confirm-delete-chapter="${CHAPTER_C}"]`)?.tagName)
    .toBe("BUTTON");
  workspace.root.querySelector<HTMLButtonElement>(`[data-confirm-delete-chapter="${CHAPTER_C}"]`)!.click();

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice()?.text }))
    .toEqual({ ...READY, alert: "无法删除章节。", save: "saved", notice: "Beta?" });
  workspace.server.release(openProposals(CHAPTER_B));
  await expect.poll(() => workspace.server.answered.includes(openProposals(CHAPTER_B))).toBe(true);
  // The owned command sends the refused removal one more time. No Author Edit goes out.
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
    [`GET ${P}/manuscript/tree`, CHALLENGE, remove, remove]);
});

it("opens no Chapter for a Proposal navigation that started before a selection, and hands the input over", async () => {
  const workspace = await openWorkspace((server) => { seedRun(server, CHAPTER_C, [RETURN_C]); });
  await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  // The navigation to Chapter C waits for its Chapter read when the author selects Chapter B.
  workspace.server.hold(READ_C);
  const before = workspace.server.requests.length;
  workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
  await expect.poll(() => workspace.server.isHeld(READ_C)).toBe(true);
  workspace.server.hold(READ_B);
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.acceptEdits = true;
  workspace.server.release(READ_B);
  await expect.poll(workspace.runIdle).toBe(true);
  await expect.poll(workspace.saveState).toBe("saved");

  workspace.server.release(READ_C);
  await expect.poll(() => workspace.server.answered.includes(READ_C)).toBe(true);
  // A save after the read: the navigation sends no request after its read.
  await workspace.typeAtEnd("!");
  await expect.poll(workspace.runIdle).toBe(true);

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ ...READY, body: "Beta?!", save: "saved", notice: undefined });
  // Each read of the list also reads the Proposal of the Run, and that read fails. The tests are not about it.
  const proposal = `GET ${P}/proposals/${PROPOSAL}`;
  const save: (string[] | AnyOrder)[] = [[CHALLENGE, AUTHOR_EDITS, SESSION_READ],
    { anyOrder: [openProposals(CHAPTER_B), proposal, `GET ${P}/manuscript/statistics`, ACTIVITY] }];
  await expectRequests(workspace.server, before, [READ_C, ...SWITCH_STEPS, ...REOPEN_STEPS],
    [...PANEL_READS, ACTIVITY, proposal], ...save, ...save);
});

it.each([
  { order: "before the list read that ends the hand-over", listFirst: false },
  { order: "after the waiting selection ran", listFirst: true },
])("opens no Chapter for a Proposal navigation that started before a waiting selection when its read answers $order",
  async ({ listFirst }) => {
  const workspace = await openWorkspace((server) => { seedRun(server, CHAPTER_C, [RETURN_C]); });
  await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  // The navigation to Chapter C starts during the switch and waits for its Chapter read.
  workspace.server.hold(READ_C);
  workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
  await expect.poll(() => workspace.server.isHeld(READ_C)).toBe(true);
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);

  workspace.button(CHAPTER).click();
  workspace.server.acceptEdits = true;
  if (listFirst) {
    workspace.server.release(openProposals(CHAPTER_B));
    await expect.poll(() => ({ title: workspace.ready().title, save: workspace.saveState() }))
      .toEqual({ title: "Chapter A", save: "saved" });
  }
  workspace.server.release(READ_C);
  await expect.poll(() => workspace.server.answered.includes(READ_C)).toBe(true);
  if (!listFirst) workspace.server.release(openProposals(CHAPTER_B));

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined, save: "saved", notice: undefined });
  // Each read of the list also reads the Proposal of the Run, and that read fails. The tests are not about it.
  const proposal = `GET ${P}/proposals/${PROPOSAL}`;
  await expectRequests(workspace.server, before, [READ_B, READ_C, CHALLENGE, SWITCH, ...REOPEN_STEPS],
    [...PANEL_READS, ACTIVITY, proposal], [CHALLENGE, AUTHOR_EDITS, SESSION_READ],
    { anyOrder: [openProposals(CHAPTER_B), proposal, `GET ${P}/manuscript/statistics`, ACTIVITY] },
    [`GET ${P}/chapters/${CHAPTER}`, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER)],
    { anyOrder: [...panelReads(CHAPTER), proposal] });
  expect({ stored: workspace.server.texts[CHAPTER_B]!.body, switches: workspace.server.switches }).toEqual({
    stored: "Beta?", switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER]] });
});

it("sends no command of a Proposal navigation whose Command Challenge answers after a waiting selection", async () => {
  const workspace = await openWorkspace((server) => { seedRun(server, CHAPTER_C, [RETURN_C]); });
  await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  // The navigation to Chapter C starts during the switch, and its Command Challenge waits.
  workspace.server.hold(CHALLENGE);
  workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
  await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);

  workspace.button(CHAPTER).click();
  workspace.server.release(CHALLENGE);
  await expect.poll(() => workspace.server.answered.filter((name) => name === CHALLENGE).length).toBe(2);
  workspace.server.acceptEdits = true;
  workspace.server.release(openProposals(CHAPTER_B));

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined, save: "saved", notice: undefined });
  // The navigation reads Chapter C and opens the Project, and its Command Challenge answers with no command after it.
  // Each read of the list also reads the Proposal of the Run, and that read fails. The tests are not about it.
  const proposal = `GET ${P}/proposals/${PROPOSAL}`;
  await expectRequests(workspace.server, before, [READ_B, READ_C, "GET /api/v1/protocol", `GET ${P}`,
    "GET /api/v1/projects", `GET ${P}/chapters/${CHAPTER}`, SESSION_READ, CHALLENGE, CHALLENGE, SWITCH,
    ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY, proposal], [CHALLENGE, AUTHOR_EDITS, SESSION_READ],
    { anyOrder: [openProposals(CHAPTER_B), proposal, `GET ${P}/manuscript/statistics`, ACTIVITY] },
    [`GET ${P}/chapters/${CHAPTER}`, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER)],
    { anyOrder: [...panelReads(CHAPTER), proposal] });
  expect({ stored: workspace.server.texts[CHAPTER_B]!.body, switches: workspace.server.switches }).toEqual({
    stored: "Beta?", switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER]] });
});

it("shows the Arrival Input in a notice and ends a waiting selection when a Proposal navigation starts while the hand-over waits",
  async () => {
  // The Run of the assistant request came from Chapter C, and its result has a return to Chapter C.
  const workspace = await openWorkspace((server) => {
    seedRun(server, CHAPTER_C, [RETURN_C]);
  });
  await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);

  workspace.server.hold(READ_C);
  // The selection of Chapter A waits for the hand-over. The navigation is a later choice, so Chapter A does not open.
  workspace.button(CHAPTER).click();

  workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
  // The list of Chapter B answers after the quiet step of the navigation, and no part of the input goes to the writing
  // controller: no idle timer of the Author Edit batch waits.
  await expect.poll(() => workspace.server.isHeld(READ_C)).toBe(true);
  workspace.server.release(openProposals(CHAPTER_B));
  await expect.poll(() => workspace.server.answered.includes(openProposals(CHAPTER_B))).toBe(true);
  const idle = workspace.runIdle();
  workspace.server.release(READ_C);

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice()?.text }))
    .toEqual({ title: "Chapter C", body: "Gamma", editable: true, alert: undefined, save: "saved", notice: "Beta?" });
  expect(idle).toBe(false);
  // The read of the Proposal fails, and the tests are not about it. The navigation runs its steps, and no Author Edit
  // goes out.
  const proposal = `GET ${P}/proposals/${PROPOSAL}`;
  const open = ["GET /api/v1/protocol", `GET ${P}`, "GET /api/v1/projects"];
  // The last open of the navigation starts the tree read and the statistics read with its Chapter read (#1937).
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS],
    [...PANEL_READS, ACTIVITY, proposal],
    [READ_C, ...open, READ_B, SESSION_READ, CHALLENGE, SWITCH, ...open, `GET ${P}/manuscript/tree`,
      `GET ${P}/manuscript/statistics`, READ_C, SESSION_READ],
    { anyOrder: [openProposals(CHAPTER_C), proposal, ACTIVITY] });
});

it.each([
  { command: "a removal of Chapter C", alert: "无法删除章节：请先完成当前输入。", start: async (workspace: Workspace) => {
    await beginTreeAction(workspace.root, `li[data-chapter-id="${CHAPTER_C}"]`, "[data-delete-chapter]");
    workspace.root.querySelector<HTMLButtonElement>(`[data-delete-chapter="${CHAPTER_C}"]`)!.click();
    await expect.poll(() => workspace.root.querySelector(`[data-confirm-delete-chapter="${CHAPTER_C}"]`)?.tagName)
      .toBe("BUTTON");
    workspace.root.querySelector<HTMLButtonElement>(`[data-confirm-delete-chapter="${CHAPTER_C}"]`)!.click();
  } },
  { command: "a Proposal navigation to Chapter C", alert: "请先完成当前输入。", run: true,
    start: (workspace: Workspace) => {
      workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
    } },
])("refuses $command while the hand-over has an open IME composition, as the quiet step does", async ({ alert, start,
  run }) => {
  // The Run of a stored assistant request came from Chapter C, and its result has a return to Chapter C. Each read of
  // the list then also reads its Proposal.
  const workspace = await openWorkspace((server) => { if (run === true) seedRun(server, CHAPTER_C, [RETURN_C]); });
  const proposal = run === true ? [`GET ${P}/proposals/${PROPOSAL}`] : [];
  if (run === true) {
    await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
  }
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);
  await applyImeComposition({ text: "zhong", replacementStart: 5, replacementEnd: 5, selectionStart: 5,
    selectionEnd: 5 });
  await expect.poll(() => manuscriptBody(workspace.editor())).toBe("Beta?zhong");

  await start(workspace);
  await expect.poll(() => workspace.ready().alert).toBe(alert);
  const refused = { ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() };
  // The composition ends, and the hand-over gives the input to the writing controller.
  workspace.server.release(openProposals(CHAPTER_B));
  await expect.poll(() => workspace.server.answered.includes(openProposals(CHAPTER_B))).toBe(true);
  workspace.server.acceptEdits = true;
  await applyTrustedInput({ operation: "insert_text", text: "中" });

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ ...READY, body: "Beta?中", alert, save: "saved", notice: undefined });
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS],
    [...PANEL_READS, ACTIVITY, ...proposal], [CHALLENGE, AUTHOR_EDITS, SESSION_READ],
    { anyOrder: [openProposals(CHAPTER_B), ...proposal, ACTIVITY, CHALLENGE, AUTHOR_EDITS, SESSION_READ,
      openProposals(CHAPTER_B), ...proposal, ACTIVITY, `GET ${P}/manuscript/statistics`] });
  expect(refused).toEqual({ ...READY, body: "Beta?zhong", arriving: "true", alert, save: "saving", notice: undefined });
});

/**
 * Selects Chapter B, types "?" in the Arriving Chapter, and gives the editor of Chapter B, whose list read waits.
 * `during` runs while the switch is in flight.
 */
async function waitingHandOver(workspace: Workspace, during = () => {}) {
  workspace.server.hold(READ_B);
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  during();
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);
}

it.each([
  { during: "the switch is in flight", switching: true, compose: false, key: "!", saves: SAVE_B },
  { during: "the hand-over waits", switching: false, compose: false, key: "!", saves: SAVE_B },
  // The key "中" ends the composition. The typed key and the composition are two Author Edits, and the reads after the
  // first save come with no order to the second save.
  { during: "an IME composition of the hand-over is open", switching: false, compose: true, key: "中", saves: [
    [CHALLENGE, AUTHOR_EDITS, SESSION_READ], { anyOrder: [openProposals(CHAPTER_B), ACTIVITY, CHALLENGE, AUTHOR_EDITS,
      SESSION_READ, openProposals(CHAPTER_B), `GET ${P}/manuscript/statistics`, ACTIVITY] }] },
])("runs a selection of Chapter C after the hand-over ended when $during, so the Arrival Input is saved first",
  async ({ switching, compose, key, saves }) => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.stopIdle();
    const before = workspace.server.requests.length;
    const select = () => { workspace.button(CHAPTER_C).click(); };
    await waitingHandOver(workspace, switching ? select : undefined);
    if (compose) {
      await applyImeComposition({ text: "zhong", replacementStart: 5, replacementEnd: 5, selectionStart: 5,
        selectionEnd: 5 });
      await expect.poll(() => manuscriptBody(workspace.editor())).toBe("Beta?zhong");
    }

    if (!switching) select();
    // The editor of the hand-over accepts input while the selection waits.
    await applyTrustedInput({ operation: "insert_text", text: key });
    const waiting = { ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() };
    workspace.server.acceptEdits = true;
    // No idle timer runs, so the quiet step of the selection sends the Author Edit.
    workspace.server.release(openProposals(CHAPTER_B));

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...ARRIVING_C, arriving: undefined, save: "saved", notice: undefined });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
      ...saves, [READ_C, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER_C)], { anyOrder: panelReads(CHAPTER_C) });
    expect({ waiting, stored: [workspace.server.texts[CHAPTER_B]!.body, workspace.server.texts[CHAPTER_C]!.body],
      switches: workspace.server.switches }).toEqual({
      waiting: { ...READY, body: `Beta?${key}`, arriving: "true", save: "saving", notice: undefined },
      stored: [`Beta?${key}`, "Gamma"], switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER_C]] });
  });

it.each([
  { later: "Chapter A", chapter: CHAPTER, shown: { title: "Chapter A", body: "Base", editable: true, alert: undefined },
    idle: false, after: [[`GET ${P}/chapters/${CHAPTER}`, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER)],
      { anyOrder: panelReads(CHAPTER) }], switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER]] },
  // A selection of the Chapter of the hand-over ends the waiting selection. Then the idle timer sends the Author Edit.
  { later: "Chapter B", chapter: CHAPTER_B, shown: { ...READY, body: "Beta?" }, idle: true, after: [],
    switches: [[CHAPTER, CHAPTER_B]] },
])("replaces a waiting selection of Chapter C with a later selection of $later while the hand-over waits",
  async ({ chapter, shown, idle, after, switches }) => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.stopIdle();
    const before = workspace.server.requests.length;
    await waitingHandOver(workspace);

    workspace.button(CHAPTER_C).click();
    workspace.button(chapter).click();
    workspace.server.acceptEdits = true;
    workspace.server.release(openProposals(CHAPTER_B));
    if (idle) await expect.poll(workspace.runIdle).toBe(true);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...shown, save: "saved", notice: undefined });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
      ...SAVE_B, ...after);
    expect({ stored: workspace.server.texts[CHAPTER_B]!.body, switches: workspace.server.switches }).toEqual({
      stored: "Beta?", switches });
  });

it.each([
  { later: "no later selection", chapters: [CHAPTER_C],
    shown: { ...ARRIVING_C, arriving: undefined }, steps: [READ_C, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER_C)],
    panels: panelReads(CHAPTER_C), switches: [[CHAPTER, CHAPTER_C]] },
  // A selection of the Arriving Chapter ends the waiting selection, so the failed switch is not sent again.
  { later: "a later selection of the Arriving Chapter", chapters: [CHAPTER_C, CHAPTER_B],
    shown: { title: "Chapter A", body: "Base", editable: true, alert: "无法打开这一章：快照已过期。" }, steps: [],
    panels: [], switches: [] },
])("runs a waiting selection after a switch that ended with the Arrival Input in a notice, with $later", async ({
  chapters, shown, steps, panels, switches }) => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  await workspace.copy(CHAPTER_C);
  workspace.server.refuse.add(READ_B);
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");

  for (const chapter of chapters) workspace.button(chapter).click();
  const waiting = { ...workspace.ready(), notice: workspace.notice() };
  workspace.server.release(READ_B);

  // The notice stays through the switch of the waiting selection.
  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notices: workspace.noticeTexts() }))
    .toEqual({ ...shown, save: "saved", notices: ["Beta?"] });
  await expectRequests(workspace.server, before, [READ_B, ...steps], panels);
  expect({ waiting, switches: workspace.server.switches }).toEqual({
    waiting: { ...READY, body: "Beta?", arriving: "true", notice: undefined }, switches });
});

it("sends no Author Edit of the Arrival Input when the Local Edit Journal cannot give the input check of the editor",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    await workspace.typeAtEnd("?");
    workspace.server.hold(SESSION_READ);
    workspace.server.release(READ_B);
    await expect.poll(() => workspace.server.isHeld(SESSION_READ)).toBe(true);

    // The read of the Acceptance records fails when the editor of Chapter B starts.
    failAcceptanceRead = true;
    workspace.server.release(SESSION_READ);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice()?.text }))
      .toEqual({ ...READY, editable: false, alert: "接受记录暂不可读取，请检查本地数据。", save: "saved",
        notice: "Beta?" });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY]);
  });

it("sends no Author Edit of the Arrival Input when another tab changes the Current Chapter before the reopen", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.hold(`GET ${P}`);
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(`GET ${P}`)).toBe(true);

  workspace.server.moveTo(CHAPTER_C);
  workspace.server.release(`GET ${P}`);

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notices: workspace.noticeTexts() }))
    .toEqual({ title: "Chapter C", body: "Gamma", editable: true, alert: undefined, save: "saved",
      notices: ["Beta?"] });
  // The reopen finds Chapter C in the Project, and reads it and the Editor Session again.
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS, READ_C, SESSION_READ],
    [openProposals(CHAPTER_C), `GET ${P}/manuscript/tree`, `GET ${P}/manuscript/statistics`, ACTIVITY]);
});

// The caret of the Arriving Chapter goes only to the editor of the same Chapter (#1934).
it("gives no focus and no caret of a click in the Arriving Chapter to the editor of another Chapter after the reopen",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    await userEvent.click(workspace.editor().querySelector("p")!);
    const clicked = workspace.editor().contains(document.activeElement);
    workspace.server.hold(`GET ${P}`);
    workspace.server.release(READ_B);
    await expect.poll(() => workspace.server.isHeld(`GET ${P}`)).toBe(true);

    workspace.server.moveTo(CHAPTER_C);
    workspace.server.release(`GET ${P}`);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() }))
      .toEqual({ title: "Chapter C", body: "Gamma", editable: true, alert: undefined, save: "saved" });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS, READ_C, SESSION_READ],
      [openProposals(CHAPTER_C), `GET ${P}/manuscript/tree`, `GET ${P}/manuscript/statistics`, ACTIVITY]);
    expect({ clicked, focus: workspace.editor().contains(document.activeElement) }).toEqual({ clicked: true,
      focus: false });
  });

it("accepts no input in an Arriving Chapter whose copy does not have the fact of no open suggestion", async () => {
  const workspace = await openWorkspace((server) => {
    server.lists[CHAPTER_B] = "018f0000-0000-7001-8000-000000000474";
  });
  await workspace.copy(CHAPTER_B);
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  // A cut event in a selection of the read-only text changes nothing.
  getSelection()!.selectAllChildren(workspace.editor());
  await expect.poll(() => getSelection()!.toString()).toBe("Beta");
  workspace.editor().dispatchEvent(new ClipboardEvent("cut", { bubbles: true, cancelable: true,
    clipboardData: new DataTransfer() }));
  const typed = { ...workspace.shown(), fact: workspace.fact(), save: workspace.saveState(), footer: workspace.footer() };
  workspace.server.release(READ_B);

  await expect.poll(() => ({ ...workspace.ready(), notice: workspace.notice(), footer: workspace.footer() }))
    .toEqual({ ...READY, notice: undefined, footer: "已保存" });
  // The copy has no fact, so the switch reads the open suggestions again with its Chapter read (#1934).
  await expectRequests(workspace.server, before, [openProposals(CHAPTER_B), ...SWITCH_STEPS, ...REOPEN_STEPS],
    [...PANEL_READS, ACTIVITY, `GET ${P}/proposals/${PROPOSAL}`]);
  expect(typed).toEqual({ ...READY, editable: false, fact: "false", save: "saved", footer: OPENING });
});

// The switch of the ticket after ADR 0107 (#1934): the copy of Chapter B has text from a pointer read and no fact, and
// the author types before the Chapter read of the switch answers.
it("accepts a key before the Chapter read of the switch answers in a Chapter whose copy has text and no fact",
  async () => {
    const workspace = await openWorkspace((server) => { server.refuse.add(openProposals(CHAPTER_B)); });
    await workspace.copy(CHAPTER_B);
    workspace.server.refuse.delete(openProposals(CHAPTER_B));
    workspace.stopIdle();
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;

    workspace.button(CHAPTER_B).click();
    await expect.poll(() => ({ held: workspace.server.isHeld(READ_B), fact: workspace.fact() }))
      .toEqual({ held: true, fact: "true" });
    await workspace.typeAtEnd("?");
    const typed = { ...workspace.shown(), fact: workspace.fact() };
    workspace.server.acceptEdits = true;
    workspace.server.release(READ_B);
    await expect.poll(workspace.runIdle).toBe(true);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() }))
      .toEqual({ ...READY, body: "Beta?", save: "saved" });
    await expectRequests(workspace.server, before, [openProposals(CHAPTER_B), ...SWITCH_STEPS, ...REOPEN_STEPS],
      [...PANEL_READS, ACTIVITY], ...SAVE_B);
    expect({ typed, stored: workspace.server.texts[CHAPTER_B]!.body, edits: workspace.server.editRequests.length })
      .toEqual({ typed: { ...READY, body: "Beta?", fact: "true" }, stored: "Beta?", edits: 1 });
  });

// The case of the instance (#1934). The display of a Chapter that the tab shows read-only records the fact "no open
// suggestion", so the Arriving Chapter accepts input at once and the switch reads no list.
it("accepts input at once in a Chapter that the tab showed read-only, before the Chapter read of the switch answers",
  async () => {
    // The Journal does not open, so the tab opens in read-only local recovery.
    failJournalOpens = 1;
    const workspace = await openWorkspace((server) => { server.deleteChapter = true; }, { editable: false });
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => ({ title: workspace.shown().title,
      listed: workspace.server.answered.includes(openProposals(CHAPTER_B)) })).toEqual({ title: "Chapter B",
      listed: true });
    // A removal of Chapter C opens the Project again, now with a ready editor of Chapter A.
    await beginTreeAction(workspace.root, `li[data-chapter-id="${CHAPTER_C}"]`, "[data-delete-chapter]");
    workspace.root.querySelector<HTMLButtonElement>(`[data-delete-chapter="${CHAPTER_C}"]`)!.click();
    await expect.poll(() => workspace.root.querySelector(`[data-confirm-delete-chapter="${CHAPTER_C}"]`)?.tagName)
      .toBe("BUTTON");
    const answered = workspace.server.answered.length;
    workspace.root.querySelector<HTMLButtonElement>(`[data-confirm-delete-chapter="${CHAPTER_C}"]`)!.click();
    await expect.poll(() => ({ ...workspace.shown(), read: panelReads(CHAPTER)
      .every((name) => workspace.server.answered.slice(answered).includes(name)) })).toEqual({ title: "Chapter A",
      body: "Base", editable: true, alert: undefined, read: true });
    workspace.stopIdle();
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;

    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    await workspace.typeAtEnd("?");
    const typed = { ...workspace.shown(), fact: workspace.fact() };
    workspace.server.acceptEdits = true;
    workspace.server.release(READ_B);
    await expect.poll(workspace.runIdle).toBe(true);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() }))
      .toEqual({ ...READY, body: "Beta?", save: "saved" });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
      ...SAVE_B);
    expect({ typed, stored: workspace.server.texts[CHAPTER_B]!.body, edits: workspace.server.editRequests.length })
      .toEqual({ typed: { ...READY, body: "Beta?", fact: "true" }, stored: "Beta?", edits: 1 });
  });

// ADR 0105, section 3: the tab reads the open suggestions of a Chapter with no copy together with the Chapter read of
// the switch, not after it (#1934).
it.each([
  ["the Chapter read", READ_B, openProposals(CHAPTER_B)],
  ["the read of the open suggestions", openProposals(CHAPTER_B), READ_B],
])("accepts input in an Arriving Chapter with no copy when both reads answered, %s first", async (_, first, second) => {
  const workspace = await openWorkspace();
  workspace.stopIdle();
  for (const name of [READ_B, openProposals(CHAPTER_B), CHALLENGE]) workspace.server.hold(name);
  const before = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();
  await expect.poll(() => [READ_B, openProposals(CHAPTER_B)].every(workspace.server.isHeld)).toBe(true);
  const answered = workspace.server.answered.length;
  workspace.server.release(first);
  await expect.poll(() => workspace.server.answered.slice(answered)).toEqual([first]);
  const afterFirst = workspace.shown().editable;
  workspace.server.release(second);
  await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
  await expect.poll(() => workspace.shown().editable).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.acceptEdits = true;
  workspace.server.release(CHALLENGE);
  await expect.poll(workspace.runIdle).toBe(true);

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() }))
    .toEqual({ ...READY, body: "Beta?", save: "saved" });
  await expectRequests(workspace.server, before, [], [READ_B, openProposals(CHAPTER_B)],
    [CHALLENGE, SWITCH, ...REOPEN_STEPS], { anyOrder: [...PANEL_READS, ACTIVITY] }, ...SAVE_B);
  expect({ afterFirst, stored: workspace.server.texts[CHAPTER_B]!.body }).toEqual({ afterFirst: false,
    stored: "Beta?" });
});

it("keeps the focus and the caret of a click in an Arriving Chapter that accepts no input, until it accepts input",
  async () => {
    const workspace = await openWorkspace((server) => {
      server.lists[CHAPTER_B] = "018f0000-0000-7001-8000-000000000474";
    });
    await workspace.copy(CHAPTER_B);
    workspace.stopIdle();
    workspace.server.hold(READ_B);
    workspace.server.hold(openProposals(CHAPTER_B));
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => [READ_B, openProposals(CHAPTER_B)].every(workspace.server.isHeld)).toBe(true);

    await userEvent.click(workspace.editor().querySelector("p")!);
    const clicked = { footer: workspace.footer(), editable: workspace.shown().editable,
      focus: workspace.editor().contains(document.activeElement), offset: getSelection()!.anchorOffset };
    // Another tab decided the Proposal, so the list of the switch has none.
    delete workspace.server.lists[CHAPTER_B];
    workspace.server.release(openProposals(CHAPTER_B));
    await expect.poll(() => workspace.shown().editable).toBe(true);
    await applyTrustedInput({ operation: "insert_text", text: "?" });
    workspace.server.acceptEdits = true;
    workspace.server.release(READ_B);
    await expect.poll(workspace.runIdle).toBe(true);

    const body = `${"Beta".slice(0, clicked.offset)}?${"Beta".slice(clicked.offset)}`;
    await expect.poll(() => ({ ...workspace.ready(), footer: workspace.footer() }))
      .toEqual({ ...READY, body, footer: "已保存" });
    await expectRequests(workspace.server, before, [openProposals(CHAPTER_B), ...SWITCH_STEPS, ...REOPEN_STEPS],
      [...PANEL_READS, ACTIVITY], ...SAVE_B);
    expect({ ...clicked, stored: workspace.server.texts[CHAPTER_B]!.body }).toEqual({ footer: OPENING, editable: false,
      focus: true, offset: clicked.offset, stored: body });
  });

// With no Arrival Input, the switch ends with the editor of the Chapter. That editor takes the focus and the caret of
// a click in the read-only Arriving Chapter (#1934).
it("keeps the focus and the caret of a click in an Arriving Chapter with no Arrival Input in the editor of the reopen",
  async () => {
    const workspace = await openWorkspace((server) => {
      server.lists[CHAPTER_B] = "018f0000-0000-7001-8000-000000000474";
    });
    await workspace.copy(CHAPTER_B);
    workspace.stopIdle();
    workspace.server.hold(READ_B);
    workspace.server.hold(openProposals(CHAPTER_B));
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => [READ_B, openProposals(CHAPTER_B)].every(workspace.server.isHeld)).toBe(true);
    await userEvent.click(workspace.editor().querySelector("p")!);
    const offset = getSelection()!.anchorOffset;
    const earlier = workspace.editor();

    // Another tab decided the Proposal. The author types nothing, so the switch ends with the editor of Chapter B.
    delete workspace.server.lists[CHAPTER_B];
    workspace.server.release(openProposals(CHAPTER_B));
    workspace.server.release(READ_B);
    await expect.poll(() => ({ ...workspace.ready(), newEditor: !earlier.isConnected, save: workspace.saveState() }))
      .toEqual({ ...READY, newEditor: true, save: "saved" });
    const reopened = { focus: workspace.editor().contains(document.activeElement), caret: getSelection()!.anchorOffset };
    await applyTrustedInput({ operation: "insert_text", text: "?" });
    workspace.server.acceptEdits = true;
    await expect.poll(workspace.runIdle).toBe(true);

    const body = `${"Beta".slice(0, offset)}?${"Beta".slice(offset)}`;
    await expect.poll(workspace.ready).toEqual({ ...READY, body });
    await expectRequests(workspace.server, before, [openProposals(CHAPTER_B), ...SWITCH_STEPS, ...REOPEN_STEPS],
      [...PANEL_READS, ACTIVITY], ...SAVE_B);
    expect({ reopened, stored: workspace.server.texts[CHAPTER_B]!.body }).toEqual({ reopened: { focus: true,
      caret: offset }, stored: body });
  });

// ADR 0105, section 4: with no Arrival Input, a newer Revision of the Chapter read of the switch replaces the shown
// text. The editor of the newer text keeps the focus and the caret of the click (#1934).
it("keeps the focus and the caret of a click when a newer Revision replaces the text of an Arriving Chapter", async () => {
  const workspace = await openWorkspace((server) => {
    server.lists[CHAPTER_B] = "018f0000-0000-7001-8000-000000000474";
  });
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  // The tab keeps the flight of a switch command for each target Revision, so this test has a Revision of its own.
  workspace.server.texts[CHAPTER_B] = { revision: "018f0000-0000-7001-8000-000000000811", title: "Chapter B",
    body: "Beta two" };
  workspace.server.hold(READ_B);
  workspace.server.hold(openProposals(CHAPTER_B));
  // The Challenge waits, so that the switch with no Arrival Input does not end before the checks.
  workspace.server.hold(CHALLENGE);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => [READ_B, openProposals(CHAPTER_B)].every(workspace.server.isHeld)).toBe(true);
  await userEvent.click(workspace.editor().querySelector("p")!);
  const offset = getSelection()!.anchorOffset;
  const earlier = workspace.editor();

  workspace.server.release(READ_B);
  await expect.poll(() => ({ body: workspace.shown().body, challenge: workspace.server.isHeld(CHALLENGE) }))
    .toEqual({ body: "Beta two", challenge: true });
  const replaced = { editable: workspace.shown().editable, newEditor: !earlier.isConnected,
    focus: workspace.editor().contains(document.activeElement) };
  delete workspace.server.lists[CHAPTER_B];
  workspace.server.release(openProposals(CHAPTER_B));
  await expect.poll(() => workspace.shown().editable).toBe(true);
  // A read-only view does not show its selection, so the caret shows when the editor accepts input.
  const caret = getSelection()!.anchorOffset;
  await applyTrustedInput({ operation: "insert_text", text: "?" });
  workspace.server.acceptEdits = true;
  workspace.server.release(CHALLENGE);
  await expect.poll(workspace.runIdle).toBe(true);

  const body = `${"Beta two".slice(0, offset)}?${"Beta two".slice(offset)}`;
  await expect.poll(() => ({ ...workspace.ready(), footer: workspace.footer() }))
    .toEqual({ ...READY, body, footer: "已保存" });
  await expectRequests(workspace.server, before, [openProposals(CHAPTER_B), ...SWITCH_STEPS, ...REOPEN_STEPS],
    [...PANEL_READS, ACTIVITY], ...SAVE_B);
  expect({ replaced, caret, stored: workspace.server.texts[CHAPTER_B]!.body }).toEqual({ replaced: { editable: false,
    newEditor: true, focus: true }, caret: offset, stored: body });
});

it("accepts no new input while the switch waits for a Challenge rate limit", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.limited = true;
  const before = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.waits.length).toBe(1);
  await workspace.typeAtEnd("?");
  const waiting = { ...workspace.shown(), save: workspace.saveState(),
    status: workspace.root.querySelector('.editor-panel > [role="status"]')?.textContent };
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.waits[0]!();
  // With no Arrival Input, no hand-over waits for the read of the open suggestions of the editor.
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);
  const listing = { ...workspace.ready(), save: workspace.saveState() };
  workspace.server.release(openProposals(CHAPTER_B));

  await expect.poll(() => ({ ...workspace.ready(), notice: workspace.notice() })).toEqual({ ...READY,
    notice: undefined });
  await expectRequests(workspace.server, before, [READ_B, CHALLENGE, CHALLENGE, SWITCH, ...REOPEN_STEPS],
    [...PANEL_READS, ACTIVITY]);
  expect({ waiting, listing }).toEqual({
    waiting: { ...READY, editable: false, save: "saved", status: CHAPTER_SWITCH_WAITING_SENTENCE },
    listing: { ...READY, save: "saved" } });
});

it("keeps the Arrival Input from before a wait for a Challenge rate limit at a selection, and runs the selection after the switch",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.stopIdle();
    workspace.server.limited = true;
    workspace.server.hold(CHALLENGE);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
    await workspace.typeAtEnd("?");
    workspace.server.release(CHALLENGE);
    await expect.poll(() => workspace.waits.length).toBe(1);

    workspace.button(CHAPTER).click();
    // The Arriving Chapter accepts no new input during the wait.
    await workspace.typeAtEnd("!");
    const waiting = { ...workspace.ready(), notice: workspace.notice(),
      status: workspace.root.querySelector('.editor-panel > [role="status"]')?.textContent };
    workspace.server.acceptEdits = true;
    workspace.waits[0]!();

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined, save: "saved", notice: undefined });
    await expectRequests(workspace.server, before, [READ_B, CHALLENGE, CHALLENGE, SWITCH, ...REOPEN_STEPS],
      [...PANEL_READS, ACTIVITY], ...SAVE_B, [`GET ${P}/chapters/${CHAPTER}`, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER)],
      { anyOrder: panelReads(CHAPTER) });
    expect({ waiting, stored: workspace.server.texts[CHAPTER_B]!.body, switches: workspace.server.switches })
      .toEqual({ waiting: { ...READY, body: "Beta?", editable: false, arriving: "true", notice: undefined,
        status: CHAPTER_SWITCH_WAITING_SENTENCE }, stored: "Beta?", switches: [[CHAPTER, CHAPTER_B],
        [CHAPTER_B, CHAPTER]] });
  });

it("accepts no more input when the Chapter read of the switch gives another Revision", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  // The tab keeps the flight of a switch command for each target Revision, so this test has a Revision of its own.
  workspace.server.texts[CHAPTER_B] = { revision: "018f0000-0000-7001-8000-000000000808", title: "Chapter B",
    body: "Beta two" };
  workspace.server.hold(READ_B);
  workspace.server.hold(CHALLENGE);
  const before = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
  await workspace.typeAtEnd("!");
  // The text of the Arrival Input stays on the screen.
  const stopped = { ...workspace.shown(), save: workspace.saveState(), footer: workspace.footer() };
  workspace.server.release(CHALLENGE);

  await expect.poll(() => ({ ...workspace.ready(), notice: workspace.notice()?.text })).toEqual({ ...READY,
    body: "Beta two", notice: "Beta?" });
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY]);
  // While Arrival Input exists, the footer shows the saving state (ADR 0105, section 3).
  expect(stopped).toEqual({ ...READY, body: "Beta?", editable: false, save: "saving", footer: "保存中" });
});

it("accepts 240 operations of Arrival Input, refuses the next one, writes none to the Local Edit Journal before the hand-over, and saves them after it",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.stopIdle();
    workspace.server.hold(READ_B);
    workspace.server.hold(openProposals(CHAPTER_B));
    workspace.server.acceptEdits = true;
    const journal = withoutBase(await journalRecords(workspace.journal));
    const before = workspace.server.requests.length;

    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    focusManuscriptEnd(workspace.editor(), window);
    for (let key = 0; key < 241; key += 1) await applyTrustedInput({ operation: "insert_text", text: "x" });
    const bounded = manuscriptBody(workspace.editor());
    workspace.server.release(READ_B);
    // The editor of the Chapter is ready, and the hand-over waits for the read of its open suggestions.
    await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);
    const waiting = { body: manuscriptBody(workspace.editor()),
      journal: withoutBase(await journalRecords(workspace.journal)) };
    workspace.server.release(openProposals(CHAPTER_B));
    // The 240 operations fill the group, so the writing controller submits it with no idle timer. The 240 writes to
    // the Local Edit Journal take some seconds.
    await expect.poll(workspace.saveState, { timeout: 15_000 }).toBe("saved");
    await applyTrustedInput({ operation: "insert_text", text: "y" });
    await expect.poll(workspace.runIdle).toBe(true);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...READY, body: `Beta${"x".repeat(240)}y`, save: "saved", notice: undefined });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
      ...SAVE_B, ...SAVE_B);
    expect({ bounded, waiting, stored: workspace.server.texts[CHAPTER_B]!.body }).toEqual({
      bounded: `Beta${"x".repeat(240)}`, waiting: { body: `Beta${"x".repeat(240)}`, journal },
      stored: `Beta${"x".repeat(240)}y` });
  });

it("marks the Arriving Chapter in the contents, and marks the earlier Chapter again when the switch ends with no editor",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.server.refuse.add(READ_B);
    workspace.server.hold(READ_B);
    const marked = () => [CHAPTER, CHAPTER_B].map((id) => workspace.button(id).getAttribute("aria-current"));
    const before = workspace.server.requests.length;

    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    const during = marked();
    workspace.server.release(READ_B);

    await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
      alert: "无法打开这一章：快照已过期。" });
    await expectRequests(workspace.server, before, [READ_B], []);
    expect({ during, after: marked() }).toEqual({ during: ["false", "true"], after: ["true", "false"] });
  });

it("copies the text of the notice with each paragraph, and the notice closes only when the author closes it",
  async () => {
    await updateClipboardPermission({ action: "grant" });
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.server.refuse.add(READ_B);
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    await workspace.typeAtEnd("?");
    await applyTrustedInput({ operation: "enter" });
    await applyTrustedInput({ operation: "insert_text", text: "Z" });
    workspace.server.release(READ_B);
    await expect.poll(() => workspace.notice()?.text).toBe("Beta?\nZ");
    const action = (name: string) => workspace.root.querySelector<HTMLButtonElement>(`[data-arrival-notice-${name}]`)!;

    action("copy").click();
    await expect.poll(() => action("copy").textContent).toBe("已复制");
    const copied = await navigator.clipboard.readText();
    action("close").click();

    await expect.poll(() => ({ ...workspace.shown(), notice: workspace.notice() })).toEqual({ title: "Chapter A",
      body: "Base", editable: true, alert: "无法打开这一章：快照已过期。", notice: undefined });
    await expectRequests(workspace.server, before, [READ_B], []);
    expect(copied).toBe("Beta?\nZ");
  });

it.each([
  { first: READ_B, second: SESSION_READ },
  { first: SESSION_READ, second: READ_B },
])("keeps the notice on the failure screen when the reopen fails, when $first answers first", async ({ first, second }) => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.hold(CHALLENGE);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
  await workspace.typeAtEnd("?");
  // The Chapter read of the reopen fails, so the open of today shows its failure screen.
  workspace.server.refuse.add(READ_B);
  workspace.server.hold(READ_B);
  workspace.server.hold(SESSION_READ);

  workspace.server.release(CHALLENGE);
  await expect.poll(() => [READ_B, SESSION_READ].every((name) => workspace.server.isHeld(name))).toBe(true);
  const answered = workspace.server.answered.length;
  workspace.server.release(first);
  await expect.poll(() => workspace.server.answered.slice(answered)).toEqual([first]);
  workspace.server.release(second);

  await expect.poll(() => ({ heading: workspace.root.querySelector('section[role="alert"] h1')?.textContent,
    notices: workspace.noticeTexts() })).toEqual({ heading: "StoryOS 无法打开项目", notices: ["Beta?"] });
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], []);
});

it("accepts no input while a notice is open, and keeps the one notice through a later switch", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.refuse.add(READ_B);
  const before = workspace.server.requests.length;
  const arrive = async (text: string) => {
    const start = workspace.server.answered.length;
    workspace.server.hold(READ_B);
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    await workspace.typeAtEnd(text);
    const during = { ...workspace.shown(), save: workspace.saveState() };
    workspace.server.release(READ_B);
    // The one Chapter read of this switch answered.
    await expect.poll(() => workspace.server.answered.slice(start)).toEqual([READ_B]);
    await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
      alert: "无法打开这一章：快照已过期。" });
    return during;
  };
  await arrive("?");

  const blocked = await arrive("!");
  const kept = workspace.noticeTexts();
  workspace.root.querySelector<HTMLButtonElement>("[data-arrival-notice-close]")!.click();
  await expect.poll(workspace.noticeTexts).toEqual([]);
  const again = await arrive("!");

  await expectRequests(workspace.server, before, [READ_B, READ_B, READ_B], []);
  expect({ blocked, kept, again, notices: workspace.noticeTexts() }).toEqual({
    blocked: { title: "Chapter B", body: "Beta", editable: false, alert: "无法打开这一章：快照已过期。", save: "saved" },
    kept: ["Beta?"],
    // The sentence of the earlier end stays until the next end, as today.
    again: { title: "Chapter B", body: "Beta!", editable: true, alert: "无法打开这一章：快照已过期。", save: "saving" },
    notices: ["Beta!"] });
});

it("shows the text of an IME composition that is open when the switch ends in the notice", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.refuse.add(READ_B);
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  focusManuscriptEnd(workspace.editor(), window);
  await applyImeComposition({ text: "中", replacementStart: 4, replacementEnd: 4, selectionStart: 1, selectionEnd: 1 });
  await expect.poll(() => ({ body: manuscriptBody(workspace.editor()), save: workspace.saveState() }))
    .toEqual({ body: "Beta中", save: "saving" });

  workspace.server.release(READ_B);

  await expect.poll(() => ({ ...workspace.shown(), notices: workspace.noticeTexts() })).toEqual({ title: "Chapter A",
    body: "Base", editable: true, alert: "无法打开这一章：快照已过期。", notices: ["Beta中"] });
  await expectRequests(workspace.server, before, [READ_B], []);
});

it("keeps the text of an IME composition that ends after another Revision in the switch read stopped the input",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.server.texts[CHAPTER_B] = { revision: "018f0000-0000-7001-8000-000000000809", title: "Chapter B",
      body: "Beta two" };
    workspace.server.hold(READ_B);
    workspace.server.hold(CHALLENGE);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    const editor = workspace.editor();
    const ends: string[] = [];
    editor.addEventListener("compositionend", (event) => { ends.push(event.data); });
    focusManuscriptEnd(editor, window);
    await applyImeComposition({ text: "中", replacementStart: 4, replacementEnd: 4, selectionStart: 1,
      selectionEnd: 1 });
    await expect.poll(() => manuscriptBody(editor)).toBe("Beta中");

    workspace.server.release(READ_B);
    await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
    // The stop makes the editor read-only, and the browser ends the composition in the editor.
    await expect.poll(() => ({ editable: workspace.shown().editable, ends })).toEqual({ editable: false,
      ends: ["中"] });
    workspace.server.release(CHALLENGE);

    await expect.poll(() => ({ ...workspace.ready(), notices: workspace.noticeTexts() })).toEqual({ ...READY,
      body: "Beta two", notices: ["Beta中"] });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY]);
  });

it("keeps the notice and ends a waiting selection when the reopen after the switch finds a Project with no Current Chapter",
  async () => {
  const workspace = await openWorkspace((server) => { server.emptyAfterSwitch = true; });
  await workspace.copy(CHAPTER_B);
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await workspace.typeAtEnd("?");
  // The workspace and its contents end with the reopen, so the selection of Chapter C does not run.
  workspace.button(CHAPTER_C).click();

  workspace.server.release(READ_B);

  await expect.poll(() => ({ editor: workspace.root.querySelector("[data-manuscript-editor]"),
    notices: workspace.noticeTexts() })).toEqual({ editor: null, notices: ["Beta?"] });
  // The open of a Project with no Current Chapter reads with no order between the reads.
  await expectRequests(workspace.server, before, [...SWITCH_STEPS], [READ_B, `GET ${P}`, "GET /api/v1/projects",
    SESSION_READ, `GET ${P}/manuscript/tree`, `GET ${P}/assistance`, `GET ${P}/skills`]);
});

it("runs a waiting selection from the Current Chapter of the Server when the reopen after an applied switch fails",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.server.hold(SWITCH);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    await workspace.typeAtEnd("?");
    workspace.button(CHAPTER_C).click();
    // The reopen reads Chapter B again, and that read fails.
    workspace.server.refuse.add(READ_B);
    workspace.server.hold(READ_C);

    workspace.server.release(SWITCH);
    // The failed reopen ends before its read of the Editor Session can go out, so that read has no order to the read
    // of Chapter C.
    await expect.poll(() => ({ held: workspace.server.isHeld(READ_C),
      session: workspace.server.requests.slice(before).filter((name) => name === SESSION_READ).length }))
      .toEqual({ held: true, session: 1 });
    // The editor part shows the failure sentence while the waiting selection reads Chapter C.
    const failed = { ...workspace.ready(), notices: workspace.noticeTexts() };
    workspace.server.release(READ_C);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notices: workspace.noticeTexts() }))
      .toEqual({ ...ARRIVING_C, arriving: undefined, save: "saved", notices: ["Beta?"] });
    await expectRequests(workspace.server, before, SWITCH_STEPS, [...REOPEN_STEPS, READ_C],
      [CHALLENGE, SWITCH, ...reopenSteps(CHAPTER_C)], { anyOrder: panelReads(CHAPTER_C) });
    expect({ failed, switches: workspace.server.switches }).toEqual({ failed: { ...ARRIVING_C, editable: false,
      alert: "无法打开这一章。", notices: ["Beta?"] }, switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER_C]] });
  });

/** True when the page asks before the tab unloads, for a reload or a close of the tab. */
function leavePrompt(): boolean {
  const event = new Event("beforeunload", { cancelable: true });
  globalThis.dispatchEvent(event);
  return event.defaultPrevented;
}

it("asks before the tab unloads while Arrival Input exists, and does not ask with no Arrival Input", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.stopIdle();
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  const opened = leavePrompt();
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  const arriving = leavePrompt();
  await workspace.typeAtEnd("?");
  const typed = leavePrompt();
  workspace.server.acceptEdits = true;
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.server.release(READ_B);
  // The editor of Chapter B has the Arrival Input, and the hand-over waits for the list.
  await expect.poll(() => workspace.server.isHeld(openProposals(CHAPTER_B))).toBe(true);
  const handOver = leavePrompt();
  workspace.server.release(openProposals(CHAPTER_B));
  await expect.poll(workspace.runIdle).toBe(true);

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ ...READY, body: "Beta?", save: "saved", notice: undefined });
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
    ...SAVE_B);
  expect({ opened, arriving, typed, handOver, saved: leavePrompt() })
    .toEqual({ opened: false, arriving: false, typed: true, handOver: true, saved: false });
});

it.each([
  // No command of Chapter B goes out, and the command of Chapter C expects Chapter A.
  { order: "before the command of Chapter B goes out", hold: READ_B, steps: [READ_B, READ_C, CHALLENGE, SWITCH],
    switches: [[CHAPTER, CHAPTER_C]] },
  // The Command Challenge of Chapter B completes, and its command does not go out.
  { order: "while the Command Challenge of Chapter B is in flight", hold: CHALLENGE,
    steps: [READ_B, CHALLENGE, READ_C, CHALLENGE, SWITCH], switches: [[CHAPTER, CHAPTER_C]] },
  // The command of Chapter B completes, and the command of Chapter C expects Chapter B.
  { order: "while the command of Chapter B is in flight", hold: SWITCH,
    steps: [READ_B, CHALLENGE, SWITCH, READ_C, CHALLENGE, SWITCH], switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER_C]] },
  // The reopen of Chapter B completes, and no editor of Chapter B mounts.
  { order: "while the reopen of Chapter B is in flight", hold: SESSION_READ,
    steps: [READ_B, CHALLENGE, SWITCH, ...REOPEN_STEPS, READ_C, CHALLENGE, SWITCH],
    switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER_C]] },
  // With no copy of Chapter C, the earlier text shows read-only until the read of the switch writes the copy.
  { order: "before a copy of Chapter C exists", hold: READ_B, noCopy: true,
    steps: [READ_B, openProposals(CHAPTER_C), READ_C, CHALLENGE, SWITCH],
    switches: [[CHAPTER, CHAPTER_C]], second: { title: "Chapter A", body: "Base", editable: false, alert: undefined },
    titles: ["Chapter B", "Chapter A", "Chapter C"] },
])("shows a second selection with no Arrival Input at once, and opens an editor only for it, when it comes $order",
  async ({ hold, noCopy, steps, switches, second = ARRIVING_C, titles: shownTitles = ["Chapter B", "Chapter C"] }) => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    if (noCopy !== true) await workspace.copy(CHAPTER_C);
    workspace.server.hold(hold);
    const before = workspace.server.requests.length;
    const titles = watchTitles(workspace.root);
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(hold)).toBe(true);
    workspace.button(CHAPTER_C).click();
    // The Arriving Chapter C is on the screen while the step of Chapter B waits.
    await expect.poll(workspace.ready).toEqual(second);
    const waited = workspace.server.isHeld(hold);
    workspace.server.release(hold);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ title: "Chapter C",
      body: "Gamma", editable: true, alert: undefined, save: "saved" });
    titles.stop();
    // No editor part of Chapter B reads its panels.
    await expectRequests(workspace.server, before, [...steps, ...reopenSteps(CHAPTER_C)], panelReads(CHAPTER_C));
    expect({ waited, titles: titles.titles, switches: workspace.server.switches }).toEqual({ waited: true,
      titles: shownTitles, switches });
  });

it.each([
  // The command back to Chapter A expects Chapter B, the result of the command in flight. Chapter C gets no step.
  { end: "Chapter A after the command of Chapter B", hold: SWITCH, last: CHAPTER,
    steps: [READ_B, CHALLENGE, SWITCH, `GET ${P}/chapters/${CHAPTER}`, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER)],
    panels: panelReads(CHAPTER), switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER]],
    shown: { title: "Chapter A", body: "Base" } },
  // Chapter A is the Current Chapter of the Server, so no command goes out, and the editor of Chapter A stays.
  { end: "Chapter A before a command", hold: READ_B, last: CHAPTER, steps: [READ_B], panels: [], switches: [],
    shown: { title: "Chapter A", body: "Base" } },
  // Chapter B is the result of the command in flight, so the last selection needs no command.
  { end: "Chapter B after its command", hold: SWITCH, last: CHAPTER_B,
    steps: [READ_B, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER_B)], panels: panelReads(CHAPTER_B),
    switches: [[CHAPTER, CHAPTER_B]], shown: { title: "Chapter B", body: "Beta" } },
])("ends three fast selections with no Arrival Input at the last one, with at most two commands, for $end",
  async ({ hold, last, steps, panels, switches, shown }) => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.server.hold(hold);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(hold)).toBe(true);
    workspace.button(CHAPTER_C).click();
    await expect.poll(() => workspace.ready().title).toBe("Chapter C");
    workspace.button(last).click();
    await expect.poll(() => workspace.ready().title).toBe(shown.title);
    workspace.server.release(hold);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...shown, editable: true, alert: undefined, save: "saved", notice: undefined });
    await expectRequests(workspace.server, before, steps, panels);
    expect(workspace.server.switches).toEqual(switches);
  });

it.each([
  // A click during the wait for a Command Challenge rate limit ends the wait (#1679).
  { end: "waits for a Command Challenge rate limit", steps: [READ_C, CHALLENGE], prepare: (workspace: Workspace) => {
    workspace.server.limited = true; }, ended: async (workspace: Workspace) => {
    await expect.poll(() => workspace.waits.length).toBe(1); }, shown: { title: "Chapter C", body: "Gamma",
    editable: false, alert: undefined, arriving: "true" } },
  // The editor of Chapter A accepts no input, because the Server moved the Editor Session to Chapter B.
  { end: "fails at the read of Chapter C", steps: [READ_C], prepare: (workspace: Workspace) => {
    workspace.server.refuse.add(READ_C); }, ended: async (workspace: Workspace) => {
    await expect.poll(() => workspace.shown().alert).toBe("无法打开这一章：快照已过期。"); },
    shown: { title: "Chapter A", body: "Base", editable: false, alert: "无法打开这一章：快照已过期。" } },
].flatMap((row) => [
  // A command back to Chapter A expects Chapter B.
  { ...row, last: CHAPTER, name: "Chapter A", after: [`GET ${P}/chapters/${CHAPTER}`, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER)],
    switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER]], ready: { title: "Chapter A", body: "Base" } },
  // Chapter B is the Current Chapter of the Server, so its editor opens with no command.
  { ...row, last: CHAPTER_B, name: "Chapter B", after: reopenSteps(CHAPTER_B), switches: [[CHAPTER, CHAPTER_B]],
    ready: { title: "Chapter B", body: "Beta" } },
]))("opens the selection of $name with the Current Chapter of the Server, Chapter B, when the switch to Chapter C $end",
  async ({ steps, prepare, ended, shown, last, after, switches, ready }) => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.server.hold(SWITCH);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    workspace.button(CHAPTER_C).click();
    await expect.poll(() => workspace.ready().title).toBe("Chapter C");
    prepare(workspace);
    workspace.server.release(SWITCH);
    await ended(workspace);
    const stopped = workspace.ready();
    workspace.button(last).click();

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ ...ready,
      editable: true, alert: undefined, save: "saved" });
    await expectRequests(workspace.server, before, [READ_B, CHALLENGE, SWITCH, ...steps, ...after], panelReads(last));
    expect({ stopped, switches: workspace.server.switches }).toEqual({ stopped: shown, switches });
  });

it("repeats a command whose response did not arrive with its key after a newer selection, and goes on from its result",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.server.hold(SWITCH);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    workspace.button(CHAPTER_C).click();
    await expect.poll(() => workspace.ready().title).toBe("Chapter C");
    workspace.server.lostSwitch = 1;
    workspace.server.release(SWITCH);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ title: "Chapter C",
      body: "Gamma", editable: true, alert: undefined, save: "saved" });
    // The repeat of the command of Chapter B gets the response of the Server, so the command of Chapter C expects B.
    await expectRequests(workspace.server, before, [READ_B, CHALLENGE, SWITCH, SWITCH, READ_C, CHALLENGE, SWITCH,
      ...reopenSteps(CHAPTER_C)], panelReads(CHAPTER_C));
    expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_B], [CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER_C]]);
  });

it("runs a second selection during an IME composition with no text after the switch, which has no Arrival Input",
  async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  await workspace.copy(CHAPTER_C);
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  // The composition starts and has no text yet.
  workspace.editor().dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
  workspace.button(CHAPTER_C).click();
  const composing = workspace.ready();
  workspace.editor().dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "" }));
  workspace.server.release(READ_B);

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ ...ARRIVING_C, arriving: undefined, save: "saved", notice: undefined });
  await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
    [READ_C, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER_C)], { anyOrder: panelReads(CHAPTER_C) });
  expect({ composing, switches: workspace.server.switches }).toEqual({ composing: { ...READY, arriving: "true" },
    switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER_C]] });
});

it("replaces a waiting selection with a selection after an IME composition with no text ended", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  await workspace.copy(CHAPTER_C);
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  workspace.editor().dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
  workspace.button(CHAPTER_C).click();
  workspace.editor().dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "" }));

  // With no Arrival Input, the selection of Chapter A replaces the Arriving Chapter and the waiting selection.
  workspace.button(CHAPTER).click();
  workspace.server.release(READ_B);

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined, save: "saved", notice: undefined });
  await expectRequests(workspace.server, before, [READ_B], []);
  expect(workspace.server.switches).toEqual([]);
});

it("replaces a waiting selection with a selection that ends the wait for a rate limit with no Arrival Input", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  await workspace.copy(CHAPTER_C);
  workspace.server.limited = true;
  workspace.server.hold(CHALLENGE);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
  workspace.editor().dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
  workspace.button(CHAPTER_C).click();
  workspace.server.release(CHALLENGE);
  await expect.poll(() => workspace.waits.length).toBe(1);
  workspace.editor().dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "" }));
  // The selection of Chapter A ends the wait and the waiting selection of Chapter C.
  workspace.button(CHAPTER).click();
  await expect.poll(() => workspace.ready()).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: undefined });

  workspace.button(CHAPTER_B).click();

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ ...READY, save: "saved", notice: undefined });
  await expectRequests(workspace.server, before, [READ_B, CHALLENGE, ...SWITCH_STEPS, ...REOPEN_STEPS],
    [...PANEL_READS, ACTIVITY]);
  expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_B]]);
});

it("skips a replaced command whose repeated Command Challenge waits after a rate limit, and sends no command", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.limited = true;
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.waits.length).toBe(1);
  workspace.server.hold(CHALLENGE);
  workspace.waits[0]!();
  await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
  // The flight of the command of Chapter B exists, but its command did not go out.
  workspace.button(CHAPTER).click();
  await expect.poll(() => workspace.ready().title).toBe("Chapter A");
  workspace.server.release(CHALLENGE);

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ title: "Chapter A",
    body: "Base", editable: true, alert: undefined, save: "saved" });
  await expectRequests(workspace.server, before, [READ_B, CHALLENGE, CHALLENGE], []);
  expect(workspace.server.switches).toEqual([]);
});

it("keeps the editor of Chapter A read-only when the command back to it applied and the switch to Chapter C failed, and reopens it",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.server.hold(SWITCH);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    workspace.button(CHAPTER).click();
    await expect.poll(() => workspace.ready().title).toBe("Chapter A");
    workspace.server.hold(SWITCH);
    workspace.server.release(SWITCH);
    // The command from Chapter B back to Chapter A is in flight.
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    workspace.button(CHAPTER_C).click();
    await expect.poll(() => workspace.ready().title).toBe("Chapter C");
    workspace.server.refuse.add(READ_C);
    workspace.server.release(SWITCH);
    await expect.poll(() => workspace.shown().alert).toBe("无法打开这一章：快照已过期。");
    const stopped = workspace.ready();
    workspace.button(CHAPTER).click();

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ title: "Chapter A",
      body: "Base", editable: true, alert: undefined, save: "saved" });
    // The Server is at Chapter A with a new base, so the selection opens it with no command.
    await expectRequests(workspace.server, before, [READ_B, CHALLENGE, SWITCH, `GET ${P}/chapters/${CHAPTER}`, CHALLENGE,
      SWITCH, READ_C, ...reopenSteps(CHAPTER)], panelReads(CHAPTER));
    expect({ stopped, switches: workspace.server.switches }).toEqual({ stopped: { title: "Chapter A", body: "Base",
      editable: false, alert: "无法打开这一章：快照已过期。" }, switches: [[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER]] });
  });

// ICS-06 (#1928): four cases between the tickets of the chain.

// The requests of the open of the workspace, and of the Run of `seedRun`.
const OPEN_REQUESTS = ["GET /api/v1/protocol", `GET ${P}`, "GET /api/v1/projects", `GET ${P}/chapters/${CHAPTER}`,
  CHALLENGE, `POST ${P}/editor-sessions`, openProposals(CHAPTER), `GET ${P}/assistance`, `GET ${P}/skills`,
  `GET ${P}/manuscript/tree`, ACTIVITY, `GET ${P}/manuscript/statistics`];
const RUN_REQUESTS = [`GET ${P}/agent-runs/${RUN}`, openProposals(CHAPTER), `GET ${P}/proposals/${PROPOSAL}`,
  `GET ${P}/agent-runs/${RUN}/research-sources`];
/** The reads of `copy(chapterId)`. */
const copyReads = (chapterId: string) => [`GET ${P}/chapters/${chapterId}`, openProposals(chapterId)];

/** Each request before `before`, with no order: the open of the workspace and the `setup` requests of the test. */
function expectSetup(server: { requests: string[] }, before: number, ...setup: string[]) {
  expect(server.requests.slice(0, before).sort()).toEqual([...OPEN_REQUESTS, ...setup].sort());
}

it("hands the Arrival Input of a return to the Current Chapter over to its editor when the read of the replaced selection answers",
  async () => {
    const workspace = await openWorkspace();
    workspace.stopIdle();
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    workspace.button(CHAPTER).click();
    await expect.poll(workspace.ready).toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined,
      arriving: "true" });
    await workspace.typeAtEnd("?", CHAPTER);
    workspace.server.acceptEdits = true;

    workspace.server.release(READ_B);
    await expect.poll(workspace.runIdle).toBe(true);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ title: "Chapter A", body: "Base?", editable: true, alert: undefined, save: "saved",
        notice: undefined });
    // The editor reads its list of open suggestions again, and the list decides the hand-over. No command goes out.
    await expectRequests(workspace.server, before, [openProposals(CHAPTER_B), READ_B, openProposals(CHAPTER)], [],
      [CHALLENGE, AUTHOR_EDITS, SESSION_READ],
      { anyOrder: [openProposals(CHAPTER), `GET ${P}/manuscript/statistics`, ACTIVITY] });
    expect({ stored: workspace.server.texts[CHAPTER]!.body, switches: workspace.server.switches }).toEqual({
      stored: "Base?", switches: [] });
    expectSetup(workspace.server, before);
  });

it("keeps an IME composition with no text open in a return to the Current Chapter until it ends", async () => {
  const workspace = await openWorkspace();
  workspace.server.hold(READ_B);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  workspace.button(CHAPTER).click();
  await expect.poll(() => workspace.ready().arriving).toBe("true");
  const arriving = workspace.editor();
  // The composition starts and has no text yet.
  arriving.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));

  workspace.server.release(READ_B);
  await expect.poll(() => workspace.server.answered.includes(READ_B)).toBe(true);
  const composing = { same: workspace.editor() === arriving, arriving: workspace.ready().arriving };
  arriving.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "" }));

  await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
    .toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined, save: "saved", notice: undefined });
  await expectRequests(workspace.server, before, [openProposals(CHAPTER_B), READ_B], []);
  expect({ composing, switches: workspace.server.switches }).toEqual({ composing: { same: true, arriving: "true" },
    switches: [] });
  expectSetup(workspace.server, before);
});

it.each([
  { end: "the editor of the newest selection", conflicts: [CHAPTER_B],
    shown: { title: "Chapter C", body: "Gamma", editable: true, alert: undefined }, after: reopenSteps(CHAPTER_C),
    panels: panelReads(CHAPTER_C) },
  // The Server refuses the command of the newest selection too, so the switch ends with the sentence of today.
  { end: "the sentence of today when the Server also refuses the command of the newest selection",
    conflicts: [CHAPTER_B, CHAPTER_C],
    shown: { title: "Chapter A", body: "Base", editable: true, alert: "无法打开这一章。" }, after: [], panels: [] },
])("sends the switch to the newest selection from the Current Chapter that the tab knows when the Server refuses the replaced command, and ends at $end",
  async ({ conflicts, shown, after, panels }) => {
    const workspace = await openWorkspace((server) => { for (const id of conflicts) server.conflicts.add(id); });
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.server.hold(SWITCH);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    workspace.button(CHAPTER_C).click();
    await expect.poll(() => workspace.ready().title).toBe("Chapter C");

    workspace.server.release(SWITCH);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...shown, save: "saved", notice: undefined });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, READ_C, CHALLENGE, SWITCH, ...after], panels);
    expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_B], [CHAPTER, CHAPTER_C]]);
    expectSetup(workspace.server, before, ...copyReads(CHAPTER_B), ...copyReads(CHAPTER_C));
  });

const STATISTICS = `GET ${P}/manuscript/statistics`;

const READY_C = { title: "Chapter C", body: "Gamma", editable: true, alert: undefined };

it("reopens the Chapter that an applied command of a stale Proposal navigation gives when the author selects that Chapter",
  async () => {
    const workspace = await openWorkspace((server) => { seedRun(server, CHAPTER_C, [RETURN_C]); });
    await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
    await workspace.copy(CHAPTER_C);
    // The Server applies the command of the navigation to Chapter C, and its response waits.
    workspace.server.holdAnswer(SWITCH);
    const before = workspace.server.requests.length;
    workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    const reads = workspace.server.answered.length;
    workspace.button(CHAPTER_C).click();
    await expect.poll(() => workspace.server.answered.slice(reads).includes(READ_C)).toBe(true);
    workspace.server.release(SWITCH);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ ...READY_C, save: "saved" });
    // The navigation moved the Current Chapter to Chapter C, so the selection needs no command and reopens Chapter C.
    await expectRequests(workspace.server, before, [READ_C, "GET /api/v1/protocol", `GET ${P}`, "GET /api/v1/projects",
      `GET ${P}/chapters/${CHAPTER}`, SESSION_READ, CHALLENGE, SWITCH, READ_C, ...reopenSteps(CHAPTER_C)],
      [...panelReads(CHAPTER_C), `GET ${P}/proposals/${PROPOSAL}`]);
    expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_C]]);
    expectSetup(workspace.server, before, ...RUN_REQUESTS, ...copyReads(CHAPTER_C));
  });

it("sends the command of a selection back to the earlier Chapter from the result of a Proposal navigation command in flight",
  async () => {
    const workspace = await openWorkspace((server) => { seedRun(server, CHAPTER_C, [RETURN_C]); });
    await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
    await workspace.copy(CHAPTER_B);
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    // The Server applies the command of the navigation to Chapter C, and its response waits.
    workspace.server.holdAnswer(SWITCH);
    workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    // The newest selection is the Chapter of the editor, so the switch needs no command before the navigation ends.
    workspace.button(CHAPTER).click();
    workspace.server.release(READ_B);
    await expect.poll(() => workspace.server.answered.filter((name) => name === READ_B).length).toBe(2);
    workspace.server.release(SWITCH);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() }))
      .toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined, save: "saved" });
    // The navigation moved the Current Chapter to Chapter C, so the command of Chapter A expects Chapter C.
    await expectRequests(workspace.server, before, [READ_B, READ_C, "GET /api/v1/protocol", `GET ${P}`,
      "GET /api/v1/projects", `GET ${P}/chapters/${CHAPTER}`, SESSION_READ, CHALLENGE, SWITCH, `GET ${P}/chapters/${CHAPTER}`,
      CHALLENGE, SWITCH, ...reopenSteps(CHAPTER)], [...panelReads(CHAPTER), `GET ${P}/proposals/${PROPOSAL}`]);
    expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_C], [CHAPTER_C, CHAPTER]]);
    expectSetup(workspace.server, before, ...RUN_REQUESTS, ...copyReads(CHAPTER_B));
  });

it("sends the command of the newest selection from the Current Chapter of the read after an earlier applied command",
  async () => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.server.holdAnswer(SWITCH);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    workspace.button(CHAPTER_C).click();
    // The command from Chapter B to Chapter C and its repeat do not reach the Server.
    workspace.server.unreachableSwitch = 2;
    workspace.server.hold(SWITCH);
    workspace.server.release(SWITCH);
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    workspace.button(CHAPTER).click();
    workspace.server.release(SWITCH);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() }))
      .toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined, save: "saved" });
    // The read shows Chapter B, so the command of the newest selection, Chapter A, expects Chapter B.
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, READ_C, CHALLENGE, SWITCH, SWITCH,
      ...learnReads(CHAPTER, CHAPTER_B), `GET ${P}/chapters/${CHAPTER}`, CHALLENGE, SWITCH, ...reopenSteps(CHAPTER)],
      panelReads(CHAPTER));
    expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_B], [CHAPTER_B, CHAPTER]]);
    expectSetup(workspace.server, before, ...copyReads(CHAPTER_B), ...copyReads(CHAPTER_C));
  });

it("sends the command of a selection from the Current Chapter that the applied command of a stale Proposal navigation gives",
  async () => {
    const workspace = await openWorkspace((server) => { seedRun(server, CHAPTER_C, [RETURN_C]); });
    await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
    await workspace.copy(CHAPTER_B);
    // The Server applies the command of the navigation to Chapter C, and its response waits.
    workspace.server.holdAnswer(SWITCH);
    const before = workspace.server.requests.length;
    workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    // The selection makes the navigation stale. Its command waits for the end of the navigation command, and the
    // test holds its Command Challenge.
    workspace.server.hold(CHALLENGE);
    const reads = workspace.server.answered.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.answered.slice(reads).includes(READ_B)).toBe(true);
    workspace.server.release(SWITCH);
    await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
    workspace.server.release(CHALLENGE);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ ...READY, save: "saved" });
    // The command of Chapter B expects Chapter C, so the Server refuses no command.
    await expectRequests(workspace.server, before, [READ_C, "GET /api/v1/protocol", `GET ${P}`,
      "GET /api/v1/projects", `GET ${P}/chapters/${CHAPTER}`, SESSION_READ, CHALLENGE, SWITCH, ...SWITCH_STEPS,
      ...REOPEN_STEPS], [...panelReads(CHAPTER_B), `GET ${P}/proposals/${PROPOSAL}`]);
    expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_C], [CHAPTER_C, CHAPTER_B]]);
    expectSetup(workspace.server, before, ...RUN_REQUESTS, ...copyReads(CHAPTER_B));
  });

it.each([
  // The editor accepts no input when the list answers, so the input goes to the notice.
  { order: "before the list answers", activityFirst: true, shown: { ...READY, editable: false }, notice: "Beta?",
    save: "saved", after: [[STATISTICS, STATISTICS]] },
  // The editor accepts input when the list answers, so the input is normal input, and the editor stops after it.
  { order: "after the list answered", activityFirst: false, shown: { ...READY, body: "Beta?", editable: false },
    notice: undefined, save: "needs_attention", after: [...SAVE_B, [STATISTICS]] },
])("decides the hand-over with the input state of the editor at the end when the Activity read fails $order",
  async ({ activityFirst, shown, notice, save, after }) => {
    const workspace = await openWorkspace();
    await workspace.copy(CHAPTER_B);
    workspace.stopIdle();
    const before = workspace.server.requests.length;
    await waitingHandOver(workspace, () => {
      workspace.server.refuse.add(ACTIVITY);
      if (!activityFirst) workspace.server.hold(ACTIVITY);
    });
    workspace.server.acceptEdits = true;
    if (activityFirst) {
      // The Activity read of the editor of Chapter B fails, and the editor accepts no input.
      await expect.poll(() => workspace.shown().editable).toBe(false);
      workspace.server.release(openProposals(CHAPTER_B));
    } else {
      await expect.poll(() => workspace.server.isHeld(ACTIVITY)).toBe(true);
      workspace.server.release(openProposals(CHAPTER_B));
      await expect.poll(workspace.runIdle).toBe(true);
      // The Author Edit applies. The next Activity read of the new Revision fails before the held one.
      await expect.poll(() => workspace.server.answered.includes(AUTHOR_EDITS)).toBe(true);
      workspace.server.release(ACTIVITY);
    }

    await expect.poll(() => ({ ...workspace.ready(), notice: workspace.notice()?.text })).toEqual({ ...shown, notice });
    expect({ save: workspace.saveState(), idle: workspace.runIdle(), stored: workspace.server.texts[CHAPTER_B]!.body })
      .toEqual({ save, idle: false, stored: shown.body });
    // Each change of the save state reads the statistics again: the failure, and the next state of the writing
    // controller before a save.
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, ...REOPEN_STEPS], [...PANEL_READS, ACTIVITY],
      ...after);
    expectSetup(workspace.server, before, ...copyReads(CHAPTER_B));
  });

// #1925: a Set Current Chapter command with no answer of the Server has an unknown result (ADR 0105, section 5).

it("reads the Project and ends with the ready editor of the selected Chapter when the responses of a command and of its repeat are lost",
  async () => {
    const workspace = await openWorkspace((server) => { server.lostSwitch = 2; });
    await workspace.copy(CHAPTER_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...READY, save: "saved", notice: undefined });
    // After the two commands come the reads of the reopen, and no third command.
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, SWITCH, ...REOPEN_STEPS], panelReads(CHAPTER_B));
    expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_B], [CHAPTER, CHAPTER_B]]);
    expectSetup(workspace.server, before, ...copyReads(CHAPTER_B));
  });

/**
 * The read of the Project after an unknown result, which opens the goal `goal` at once. With another Current Chapter
 * `current` on the Server, the open reads that Chapter and the Editor Session again.
 */
const learnReads = (goal: string, current: string) => [...reopenSteps(goal), `GET ${P}/chapters/${current}`,
  SESSION_READ];
const UNKNOWN_NO_EFFECT_READS = learnReads(CHAPTER_B, CHAPTER);
// After the unknown result of a navigation command, the selection sends no Command Challenge before the read of the
// Project. The read shows Chapter C, and the command of Chapter B expects Chapter C.
const NAVIGATION_RACE_STEPS = [...learnReads(CHAPTER_B, CHAPTER_C), CHALLENGE, SWITCH, ...REOPEN_STEPS];
const STOPPED_UNKNOWN = { title: "Chapter A", body: "Base", editable: true, alert: "无法打开这一章。" };

it("shows the earlier Chapter with the sentence of today and an editor that accepts input when the command and its repeat do not reach the Server",
  async () => {
    const workspace = await openWorkspace((server) => { server.unreachableSwitch = 2; });
    await workspace.copy(CHAPTER_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...STOPPED_UNKNOWN, save: "saved", notice: undefined });
    const stopped = workspace.server.requests.length;
    // The Command Challenge and its repeat get no answer. No command went out, so the result is known, and the tab
    // reads no Project.
    workspace.server.refuse.add(CHALLENGE);
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => ({ ...workspace.ready(), challenges: workspace.server.requests.slice(stopped)
      .filter((name) => name === CHALLENGE).length })).toEqual({ ...STOPPED_UNKNOWN, challenges: 2 });
    workspace.server.refuse.delete(CHALLENGE);
    // The read of the Project showed Chapter A, so the tab forgot the command, and a later selection of Chapter B sends
    // a new Command Challenge.
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ ...READY, save: "saved" });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, SWITCH, ...UNKNOWN_NO_EFFECT_READS], [],
      [READ_B, CHALLENGE, CHALLENGE], [], [...SWITCH_STEPS, ...REOPEN_STEPS], { anyOrder: panelReads(CHAPTER_B) });
    expect({ stopped: stopped - before, switches: workspace.server.switches }).toEqual({
      stopped: 4 + UNKNOWN_NO_EFFECT_READS.length, switches: [[CHAPTER, CHAPTER_B]] });
    expectSetup(workspace.server, before, ...copyReads(CHAPTER_B));
  });

it.each([
  { command: "applied", later: CHAPTER_C, shown: { title: "Chapter C", body: "Gamma" } },
  // A selection of the Chapter of the editor also reads the Project.
  { command: "applied", later: CHAPTER, shown: { title: "Chapter A", body: "Base" } },
  // The read shows that the command had no effect, and the later selection sends its command from Chapter A.
  { command: "had no effect", later: CHAPTER_C, shown: { title: "Chapter C", body: "Gamma" } },
])("keeps the editor read-only when the command $command and the read after it fails, and a later selection of $shown.title reads the Project before its command",
  async ({ command, later, shown }) => {
    const applied = command === "applied";
    const workspace = await openWorkspace((server) => {
      if (applied) server.lostSwitch = 2;
      else server.unreachableSwitch = 2;
    });
    await workspace.copy(CHAPTER_B);
    await workspace.copy(CHAPTER_C);
    workspace.server.refuse.add(`GET ${P}`);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() }))
      .toEqual({ ...STOPPED_UNKNOWN, editable: false, save: "saved" });
    // The Editor Session read of the failed read goes out after the Journal opens.
    await expect.poll(() => workspace.server.requests.slice(before).filter((name) => name === SESSION_READ).length)
      .toBe(1);
    const stopped = workspace.server.requests.length;
    workspace.server.refuse.delete(`GET ${P}`);
    workspace.button(later).click();

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice() }))
      .toEqual({ ...shown, editable: true, alert: undefined, save: "saved", notice: undefined });
    // The command of the later selection expects the Current Chapter that the read of the Project shows.
    const current = applied ? CHAPTER_B : CHAPTER;
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, SWITCH], REOPEN_STEPS,
      learnReads(later, current), [`GET ${P}/chapters/${later}`, CHALLENGE, SWITCH, ...reopenSteps(later)],
      { anyOrder: panelReads(later) });
    expect({ stopped: stopped - before, switches: workspace.server.switches }).toEqual({ stopped: 8,
      switches: [...applied ? [[CHAPTER, CHAPTER_B], [CHAPTER, CHAPTER_B]] : [], [current, later]] });
    expectSetup(workspace.server, before, ...copyReads(CHAPTER_B), ...copyReads(CHAPTER_C));
  });

it.each([
  { end: "the command applied", prepare: (server: Workspace["server"]) => { server.lostSwitch = 2; },
    shown: { ...READY, body: "Beta?" }, notice: undefined, stored: "Beta?",
    steps: [...SWITCH_STEPS, SWITCH, ...REOPEN_STEPS], panels: [...PANEL_READS, ACTIVITY], after: SAVE_B },
  { end: "the command had no effect", prepare: (server: Workspace["server"]) => { server.unreachableSwitch = 2; },
    shown: STOPPED_UNKNOWN, notice: "Beta?", stored: "Beta",
    steps: [...SWITCH_STEPS, SWITCH], panels: [], after: [] as (string[] | AnyOrder)[] },
])("applies section 4 to the Arrival Input with the editor of the read after an unknown result when $end",
  async ({ prepare, shown, notice, stored, steps, panels, after }) => {
    const workspace = await openWorkspace(prepare);
    await workspace.copy(CHAPTER_B);
    workspace.stopIdle();
    workspace.server.hold(READ_B);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    await workspace.typeAtEnd("?");
    workspace.server.acceptEdits = true;
    workspace.server.hold(`GET ${P}`);
    workspace.server.release(READ_B);
    // While the tab reads the Project, the Arriving Chapter accepts no input.
    await expect.poll(() => workspace.server.isHeld(`GET ${P}`)).toBe(true);
    const reading = workspace.ready();
    workspace.server.release(`GET ${P}`);
    if (notice === undefined) await expect.poll(workspace.runIdle).toBe(true);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), notice: workspace.notice()?.text }))
      .toEqual({ ...shown, save: "saved", notice });
    expect(reading).toEqual({ ...READY, body: "Beta?", editable: false, arriving: "true" });
    await expectRequests(workspace.server, before, notice === undefined ? steps : [...steps,
      ...UNKNOWN_NO_EFFECT_READS], panels, ...after);
    expect({ stored: workspace.server.texts[CHAPTER_B]!.body, edits: workspace.server.editRequests.length,
      idle: workspace.runIdle() }).toEqual({ stored, edits: notice === undefined ? 1 : 0, idle: false });
    expectSetup(workspace.server, before, ...copyReads(CHAPTER_B));
  });

it("keeps the editor read-only after an unknown result of the command of a Proposal navigation until a selection reads the Project",
  async () => {
    const workspace = await openWorkspace((server) => { seedRun(server, CHAPTER_C, [RETURN_C]); });
    await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
    await workspace.copy(CHAPTER_B);
    const message = workspace.root.querySelector<HTMLTextAreaElement>('textarea[name="assistant-message"]')!;
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(message, "Help.");
    message.dispatchEvent(new Event("input", { bubbles: true }));
    const send = () => workspace.root.querySelector<HTMLButtonElement>("button.send-button")?.disabled;
    await expect.poll(send).toBe(false);
    workspace.server.lostSwitch = 2;
    const before = workspace.server.requests.length;
    workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
    await expect.poll(() => workspace.server.requests.filter((name) => name === SWITCH).length).toBe(2);

    // The editor accepts no input, and no message to the assistant can go out.
    await expect.poll(() => ({ ...workspace.ready(), disabled: send() })).toEqual({ ...STOPPED_UNKNOWN, editable: false,
      alert: "无法打开写作会话。", disabled: true });
    const stopped = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState(), disabled: send() })).toEqual({ ...READY,
      save: "saved", disabled: false });
    // The read of the Project shows Chapter C, so the command of Chapter B expects Chapter C.
    await expectRequests(workspace.server, before, [READ_C, "GET /api/v1/protocol", `GET ${P}`,
      "GET /api/v1/projects", `GET ${P}/chapters/${CHAPTER}`, SESSION_READ, CHALLENGE, SWITCH, SWITCH], [],
      learnReads(CHAPTER_B, CHAPTER_C), [...SWITCH_STEPS, ...REOPEN_STEPS],
      { anyOrder: [...panelReads(CHAPTER_B), `GET ${P}/proposals/${PROPOSAL}`] });
    expect({ stopped: stopped - before, switches: workspace.server.switches }).toEqual({ stopped: 9,
      switches: [[CHAPTER, CHAPTER_C], [CHAPTER, CHAPTER_C], [CHAPTER_C, CHAPTER_B]] });
    expectSetup(workspace.server, before, ...RUN_REQUESTS, ...copyReads(CHAPTER_B));
  });

it.each([
  // The Editor Session read of the read fails, so the read gives a read-only editor of Chapter A.
  { end: "a read-only editor of the earlier Chapter", prepare: (server: Workspace["server"]) => {
    server.unreachableSwitch = 2;
  }, refuse: SESSION_READ, after: [...REOPEN_STEPS, `GET ${P}/chapters/${CHAPTER}`, SESSION_READ] },
  // The command applied, and the Project then has no Current Chapter.
  { end: "a Project with no Current Chapter", prepare: (server: Workspace["server"]) => {
    server.lostSwitch = 2;
    server.emptyAfterSwitch = true;
  }, after: [] as string[] },
])("shows $end when the read after an unknown result gives it", async ({ end, prepare, refuse, after }) => {
  const workspace = await openWorkspace(prepare);
  await workspace.copy(CHAPTER_B);
  if (refuse !== undefined) workspace.server.refuse.add(refuse);
  const before = workspace.server.requests.length;
  workspace.button(CHAPTER_B).click();

  if (refuse !== undefined) {
    await expect.poll(() => ({ ...workspace.ready(), notice: workspace.notice() }))
      .toEqual({ title: "Chapter A", body: "Base", editable: false, alert: undefined, notice: undefined });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, SWITCH, ...after],
      [openProposals(CHAPTER), `GET ${P}/manuscript/tree`, `GET ${P}/manuscript/statistics`]);
  } else {
    // The ready screen of a Project with no Current Chapter.
    await expect.poll(() => ({ editor: workspace.root.querySelector("[data-manuscript-editor]"),
      empty: [...workspace.root.querySelectorAll("p")].some((line) => line.textContent === "空工作区"),
      alert: workspace.root.querySelector('[role="alert"]')?.textContent }))
      .toEqual({ editor: null, empty: true, alert: undefined });
    // The open of a Project with no Current Chapter reads with no order between the reads.
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, SWITCH], [READ_B, `GET ${P}`,
      "GET /api/v1/projects", SESSION_READ, `GET ${P}/manuscript/tree`, `GET ${P}/assistance`, `GET ${P}/skills`]);
  }
  expect({ end, switches: workspace.server.switches }).toEqual({ end,
    switches: refuse !== undefined ? [] : [[CHAPTER, CHAPTER_B], [CHAPTER, CHAPTER_B]] });
  expectSetup(workspace.server, before, ...copyReads(CHAPTER_B));
});

it("shows the read-only editor of the read when the author selects the earlier Chapter during a command with an unknown result",
  async () => {
    const workspace = await openWorkspace((server) => { server.unreachableSwitch = 2; });
    await workspace.copy(CHAPTER_B);
    workspace.server.hold(SWITCH);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    workspace.button(CHAPTER).click();
    // The Editor Session read of the read fails, so the read gives a read-only editor of Chapter A.
    workspace.server.refuse.add(SESSION_READ);
    workspace.server.release(SWITCH);

    await expect.poll(() => ({ ...workspace.ready(), notice: workspace.notice() }))
      .toEqual({ title: "Chapter A", body: "Base", editable: false, alert: undefined, notice: undefined });
    await expectRequests(workspace.server, before, [...SWITCH_STEPS, SWITCH, ...reopenSteps(CHAPTER)],
      [openProposals(CHAPTER), `GET ${P}/manuscript/tree`, `GET ${P}/manuscript/statistics`]);
    expect(workspace.server.switches).toEqual([]);
    expectSetup(workspace.server, before, ...copyReads(CHAPTER_B));
  });

it.each([
  { answer: "after the end of the navigation command", during: false },
  // The command of the selection waits for the end of the navigation command.
  { answer: "while the navigation command is in flight", during: true },
])("reads the Project before the command of a selection when a Proposal navigation gets an unknown result and the read of the selected Chapter answers $answer",
  async ({ during }) => {
    const workspace = await openWorkspace((server) => { seedRun(server, CHAPTER_C, [RETURN_C]); });
    await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
    await workspace.copy(CHAPTER_B);
    // The Server applies the command of the navigation to Chapter C, and the two responses do not arrive.
    workspace.server.hold(SWITCH);
    workspace.server.lostSwitch = 2;
    const before = workspace.server.requests.length;
    workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
    await expect.poll(() => workspace.server.isHeld(SWITCH)).toBe(true);
    // The selection reads Chapter B while the command of the navigation is in flight.
    workspace.server.hold(READ_B);
    const reads = workspace.server.answered.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    if (during) {
      workspace.server.release(READ_B);
      await expect.poll(() => workspace.server.answered.slice(reads).includes(READ_B)).toBe(true);
    }
    workspace.server.release(SWITCH);
    // During the wait of the selection, its command can go out at once after the repeat of the navigation command.
    await expect.poll(() => workspace.server.requests.filter((name) => name === SWITCH).length >= 2).toBe(true);
    if (!during) workspace.server.release(READ_B);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ ...READY, save: "saved" });
    // The read of the Project shows Chapter C, so the command of Chapter B expects Chapter C.
    await expectRequests(workspace.server, before, [READ_C, "GET /api/v1/protocol", `GET ${P}`,
      "GET /api/v1/projects", `GET ${P}/chapters/${CHAPTER}`, SESSION_READ, CHALLENGE, SWITCH, READ_B, SWITCH], [],
      NAVIGATION_RACE_STEPS, { anyOrder: [...panelReads(CHAPTER_B), `GET ${P}/proposals/${PROPOSAL}`] });
    expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_C], [CHAPTER, CHAPTER_C], [CHAPTER_C, CHAPTER_B]]);
    expectSetup(workspace.server, before, ...RUN_REQUESTS, ...copyReads(CHAPTER_B));
  });

it("sends no command of a selection after a Proposal navigation command that went out during its Command Challenge",
  async () => {
    const workspace = await openWorkspace((server) => { seedRun(server, CHAPTER_C, [RETURN_C]); });
    await expect.poll(() => workspace.root.querySelector("[data-proposal-return]")?.tagName).toBe("BUTTON");
    await workspace.copy(CHAPTER_B);
    workspace.server.lostSwitch = 2;
    workspace.server.hold(CHALLENGE);
    const before = workspace.server.requests.length;
    workspace.button(CHAPTER_B).click();
    await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
    // The Server applies the command of the navigation to Chapter C, and the two responses do not arrive.
    workspace.root.querySelector<HTMLButtonElement>("[data-proposal-return]")!.click();
    await expect.poll(() => workspace.server.requests.filter((name) => name === SWITCH).length).toBe(2);
    workspace.server.release(CHALLENGE);

    await expect.poll(() => ({ ...workspace.ready(), save: workspace.saveState() })).toEqual({ ...READY, save: "saved" });
    // The command of the selection does not go out after its Command Challenge. The read of the Project shows Chapter
    // C, so the next command of Chapter B expects Chapter C.
    await expectRequests(workspace.server, before, [READ_B, CHALLENGE, READ_C, "GET /api/v1/protocol", `GET ${P}`,
      "GET /api/v1/projects", `GET ${P}/chapters/${CHAPTER}`, SESSION_READ, CHALLENGE, SWITCH, SWITCH], [],
      NAVIGATION_RACE_STEPS, { anyOrder: [...panelReads(CHAPTER_B), `GET ${P}/proposals/${PROPOSAL}`] });
    expect(workspace.server.switches).toEqual([[CHAPTER, CHAPTER_C], [CHAPTER, CHAPTER_C], [CHAPTER_C, CHAPTER_B]]);
    expectSetup(workspace.server, before, ...RUN_REQUESTS, ...copyReads(CHAPTER_B));
  });
