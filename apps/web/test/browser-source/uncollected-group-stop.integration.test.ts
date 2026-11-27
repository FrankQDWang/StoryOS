// Acceptance tests of issue 1758: a closed group leaves the working set with the collected prefix (ADR 0036).
import { expect, it, vi } from "vitest";

import type {
  DigestValue,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import statisticsFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-statistics.json";
import openProposalsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-chapter-open-proposals.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import setCurrentChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-current-chapter.json";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import type { ChallengeAdmissionTimers } from "../../src/challenge-admission-wait.ts";
import { uuidV7 } from "../../src/client-identity.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
} from "../support/manuscript-surface.ts";
import {
  CHAPTER,
  OWNER,
  PROJECT,
  chapterRevision,
  closeTrackedDatabases,
  createAppliedAuthorEditResponse,
  createBrowserScenario,
  deleteJournal,
  emptyActivityStream,
  jsonResponse,
  requestHeaders,
  requestResult,
  requireDigestValue,
  requireRequestBody,
  trackDatabase,
} from "./scenario.ts";

const AUTHOR_EDIT_SCHEMA = "storyos.command.apply-author-edit.request.v2";
const SET_CURRENT_CHAPTER_SCHEMA = "storyos.command.set-current-chapter.request.v1";
// Chapter B and its head are the ones of the golden response of Set Current Chapter.
const CHAPTER_B = "018f0000-0000-7001-8000-000000000803";
const REVISION_B = "018f0000-0000-7001-8000-000000000805";

/**
 * "no_effect": the first Author Edit has the effect `no_effect` (`content_unchanged`).
 * "changed_no_effect": the same, also when the group changed the text. The real Server does not give it.
 * "challenge_wait": the first Author Edit Challenge has a rate limit, so the input of the wait follows the first group.
 */
type Mode = "no_effect" | "changed_no_effect" | "challenge_wait";

type Timer = { callback: () => void; timeout: number; cleared: boolean };

function injectedTimers(timers: Timer[]): ChallengeAdmissionTimers {
  return {
    setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, cleared: false }) - 1,
    clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].cleared = true; },
  };
}

function fire(timers: Timer[], timeout: number) {
  for (const timer of timers.filter((item) => item.timeout === timeout && !item.cleared)) {
    timer.cleared = true;
    timer.callback();
  }
}

async function sha256Hex(body: string) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

type Primitive = { kind: string; from: number; to: number; text?: string };

/**
 * A Server with Chapter A (current) and Chapter B that applies each Author Edit on the current head of the current
 * Chapter. The mode gives the first Author Edit or its Challenge (ADR 0065).
 */
function writingServer(scenario: ReturnType<typeof createBrowserScenario>, mode: Mode) {
  const sessionId = scenario.session.editor_session.editor_session_id;
  let session: GetEditorSessionResponse = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v2" };
  let body = session.base_snapshot.materialized_revision.body;
  let position = 0;
  let challenges = 0;
  const effects: string[] = [];
  let digest: DigestValue | undefined;
  let chapterDigest: DigestValue | undefined;
  let current = CHAPTER;
  const bodies: Record<string, string> = { [CHAPTER]: body, [CHAPTER_B]: "B" };
  const unexpected: string[] = [];
  /** Makes Chapter B current with the body "B", as an admitted Set Current Chapter does. */
  const openChapterB = async () => {
    current = CHAPTER_B;
    body = bodies[CHAPTER_B]!;
    position += 1;
    session = { ...session, correlation_id: uuidV7(crypto), base_snapshot: { ...session.base_snapshot,
      snapshot_id: uuidV7(crypto), chapter_id: CHAPTER_B, project_activity_position: String(position),
      authoritative_head_revision_id: REVISION_B, target_refs: [`manuscript:${CHAPTER_B}`],
      materialized_revision: chapterRevision(REVISION_B, body),
      materialized_payload_digest: { algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
        value_hex_lowercase: await sha256Hex(body) } } };
  };
  const open = () => ({ kind: "current_chapter" as const, current_chapter_id: current });
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const route = `${init?.method ?? "GET"} ${path}`;
    const project = `/api/v1/projects/${PROJECT}`;
    if (route === "GET /api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (route === "GET /api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: { owner_user_id: OWNER, project_id: PROJECT }, title: "Project A",
          revision: "1", open: open() }] });
    }
    if (route === `GET ${project}`) {
      return jsonResponse({ ...scenario.project, project: { ...scenario.project.project, open: open() } });
    }
    if (route === `GET ${project}/chapters/${current}`) {
      return jsonResponse({ ...scenario.chapter, project_activity_position: String(position),
        chapter: { ...scenario.chapter.chapter, ...(current === CHAPTER ? {} : { chapter_id: CHAPTER_B, title: "Chapter B" }),
          current_revision: session.base_snapshot.materialized_revision } });
    }
    if (route === `GET ${project}/chapters/${CHAPTER_B}`) {
      return jsonResponse({ ...scenario.chapter, project_activity_position: String(position),
        chapter: { ...scenario.chapter.chapter, chapter_id: CHAPTER_B, title: "Chapter B",
          current_revision: chapterRevision(REVISION_B, "B") } });
    }
    // The page reads the statistics, the open Proposals, the assistance settings, and the Skills. Each text is one word.
    const scope = scenario.project.project_scope;
    if (route === `GET ${project}/manuscript/statistics`) {
      const count = (text: string) => ({ word_count: text.length > 0 ? "1" : "0", character_count: String(text.length) });
      const manuscript = { chapter_count: "2", word_count: String(Object.values(bodies).filter((text) => text.length > 0).length),
        character_count: String(Object.values(bodies).join("").length) };
      return jsonResponse({ ...statisticsFixture, project_scope: scope, manuscript,
        page_bytes: String(JSON.stringify(manuscript).length),
        current_chapter: { chapter_id: current, ...count(bodies[current]!) },
        projection_generation: String(position), projection_watermark: String(position),
        source_snapshot: { ...statisticsFixture.source_snapshot, project_scope: scope,
          project_activity_position: String(position) } });
    }
    if (route === `GET ${project}/chapters/${current}/open-proposals`) {
      return jsonResponse({ ...openProposalsFixture, project_scope: scope, chapter_id: current, proposals: [] });
    }
    if (route === `GET ${project}/assistance`) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (route === `GET ${project}/skills`) return jsonResponse({ ...skillsFixture, project_scope: scope, skills: [] });
    if (route === `PUT ${project}/current-chapter` && current === CHAPTER && chapterDigest !== undefined) {
      const input = requireRequestBody(init).set_current_chapter_input as { correlation_id: string };
      await openChapterB();
      // The answer names the base that the switch installs, and the receipt names the command of the request and
      // the Author Action Sequence of the Chapter selection.
      return jsonResponse({ ...setCurrentChapterFixture, correlation_id: input.correlation_id,
        project_scope: scenario.project.project_scope,
        effect: { ...setCurrentChapterFixture.effect, base_snapshot_id: session.base_snapshot.snapshot_id,
          project_activity_position: String(position) },
        receipt: { ...setCurrentChapterFixture.receipt, project_scope: scenario.project.project_scope,
          author_action_sequence: String(position), command_digest: chapterDigest,
          idempotency_key: requestHeaders(init).get("idempotency-key") ?? "" } });
    }
    if (route === `GET ${project}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scenario.project.project_scope,
        tree_revision: "2",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scenario.project.project_scope },
        volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000815", title: "Volume A", order: "1",
          chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" },
            { chapter_id: CHAPTER_B, title: "Chapter B", order: "2" }] }] });
    }
    if (route === `POST ${project}/anti-forgery-challenges`) {
      const request = requireRequestBody(init);
      if (request.command_schema === SET_CURRENT_CHAPTER_SCHEMA) {
        chapterDigest = requireDigestValue(request.canonical_command_digest, "command digest");
      }
      if (request.command_schema === AUTHOR_EDIT_SCHEMA) {
        challenges += 1;
        digest = requireDigestValue(request.canonical_command_digest, "command digest");
        if (challenges === 1 && mode === "challenge_wait") {
          return new Response(JSON.stringify({ schema_id: "storyos.problem.v1",
            code: "challenge_rate_limited", message: "The command challenge rate limit is exceeded." }),
          { status: 429, headers: { "content-type": "application/json", "retry-after": "7" } });
        }
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 600_000).toISOString(),
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (route === `POST ${project}/editor-sessions`) {
      return jsonResponse({ ...session, schema_id: scenario.session.schema_id,
        editor_session: { ...session.editor_session, editor_session_id: sessionId } });
    }
    if (route === `GET ${project}/editor-sessions/${sessionId}`) {
      return jsonResponse({ ...session, editor_session: { ...session.editor_session, editor_session_id: sessionId } });
    }
    if (route === `GET ${project}/activity`) return emptyActivityStream();
    if (route === `POST ${project}/manuscript/author-edits` && digest !== undefined) {
      const idempotencyKey = requestHeaders(init).get("idempotency-key") ?? "";
      const request = requireRequestBody(init);
      const head = session.base_snapshot.authoritative_head_revision_id;
      const applied = createAppliedAuthorEditResponse({ request, commandDigest: digest, idempotencyKey,
        commandId: uuidV7(crypto), authorCommandAdmissionId: uuidV7(crypto), receiptId: uuidV7(crypto),
        priorRevisionId: head, projectActivityPosition: String(position + 1), body });
      if ((mode === "no_effect" || mode === "changed_no_effect") && effects.length === 0) {
        effects.push("no_effect");
        return jsonResponse({ ...applied, effect: { kind: "no_effect", reason: "content_unchanged" },
          receipt: { ...applied.receipt, result: "no_effect", prior_heads: [head], resulting_heads: [head],
            proposal_revision_ids: [], authoritative_revision_ids: [], authoritative_commit_ids: [],
            author_action_sequence: null } });
      }
      effects.push("authoritative_applied");
      const units = request.author_edit_units as Array<{ normalized_primitives: Primitive[] }>;
      for (const unit of units) {
        for (const primitive of unit.normalized_primitives) {
          // Each test input of an applied group inserts text at the end of the one Block.
          body = `${body.slice(0, primitive.from)}${primitive.text ?? ""}${body.slice(primitive.to)}`;
        }
      }
      bodies[current] = body;
      position += 1;
      const revision = uuidV7(crypto);
      session = { ...session, correlation_id: uuidV7(crypto), base_snapshot: { ...session.base_snapshot,
        snapshot_id: uuidV7(crypto), project_activity_position: String(position), authoritative_head_revision_id: revision,
        materialized_revision: chapterRevision(revision, body),
        materialized_payload_digest: { algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
          value_hex_lowercase: await sha256Hex(body) },
        created_at: "2026-08-15T08:00:00.000Z" } };
      return jsonResponse(createAppliedAuthorEditResponse({ request, commandDigest: digest, idempotencyKey,
        commandId: uuidV7(crypto), authorCommandAdmissionId: uuidV7(crypto), receiptId: uuidV7(crypto),
        authoritativeRevisionId: revision, authoritativeCommitId: uuidV7(crypto),
        projectActivityPosition: String(position), priorRevisionId: head, body }));
    }
    unexpected.push(route);
    throw new Error(`unexpected fetch ${route}`);
  };
  return { fetchImpl, effects, unexpected, openChapterB };
}

type Counts = { records: number; chains: number; groups: number };

// The limit of each test is at least 60 s, because its longest single wait is 30 s, and a blocked wait must give its
// own message first. The two tests of 2,600 inputs take about 26 s, and on the code before #1758 they take up to
// 111 s before they show the stop. Thus their limit is 240 s.

/** The count of the working set and of all rows, for each Journal partition. */
async function journalCounts(name: string) {
  const database = await requestResult(indexedDB.open(name));
  try {
    const transaction = database.transaction(["partitions", "intents", "payload_chains", "submission_groups", "metadata"]);
    const partitions = await requestResult(transaction.objectStore("partitions").getAllKeys()) as string[];
    const count = (store: string, index: string, partition: string) =>
      requestResult(transaction.objectStore(store).index(index).count(partition));
    const result: Record<string, { working: Counts; all: Counts; boundary: unknown; workingFences: number;
      uncollected: string[] }> = {};
    for (const partition of partitions) {
      const groupRows = await requestResult(transaction.objectStore("submission_groups").index("working_partition")
        .getAll(partition)) as Array<{ covered_sequence_range: { first: number; last: number };
          settlement: { kind: string; effect?: { kind: string } }; payload_collection?: { kind: string } }>;
      const uncollected = groupRows.filter((group) => group.payload_collection?.kind !== "collected")
        .sort((left, right) => left.covered_sequence_range.first - right.covered_sequence_range.first)
        .map((group) => `${group.covered_sequence_range.first}-${group.covered_sequence_range.last}:`
          + `${group.settlement.effect?.kind ?? group.settlement.kind}`);
      const [records, chains, groups, allRecords, allChains, allGroups, boundary, fences] = await Promise.all([
        count("intents", "working_partition", partition), count("payload_chains", "working_partition", partition),
        count("submission_groups", "working_partition", partition), count("intents", "partition", partition),
        count("payload_chains", "partition", partition), count("submission_groups", "partition", partition),
        requestResult(transaction.objectStore("metadata").get(`working_boundary:${partition}`)),
        requestResult(transaction.objectStore("metadata").get(`collection_fences:${partition}`)),
      ]);
      result[partition.split(":")[2]!] = { working: { records, chains, groups },
        all: { records: allRecords, chains: allChains, groups: allGroups },
        boundary: (boundary as { value?: { last_sequence?: number } } | undefined)?.value?.last_sequence ?? null,
        workingFences: (fences as { value?: unknown[] } | undefined)?.value?.length ?? 0, uncollected };
    }
    return result;
  } finally {
    database.close();
  }
}

type Row = Record<string, unknown>;
const STORES = ["intents", "payload_chains", "submission_groups"] as const;
const rowKey = (row: Row) => String(row.completed_intent_record_id ?? row.payload_chain_id ?? row.journal_submission_group_id);

/** The rows of each store of the one partition: the working set, and the rows that left it (retained history). */
async function journalRows(name: string) {
  const database = await requestResult(indexedDB.open(name));
  try {
    const transaction = database.transaction([...STORES]);
    return Object.fromEntries(await Promise.all(STORES.map(async (store) => {
      const rows = await requestResult(transaction.objectStore(store).getAll()) as Row[];
      return [store, { working: rows.filter((row) => row.working_set_partition_id !== undefined),
        retained: rows.filter((row) => row.working_set_partition_id === undefined) }] as const;
    }))) as Record<(typeof STORES)[number], { working: Row[]; retained: Row[] }>;
  } finally {
    database.close();
  }
}

function harness(mode: Mode) {
  const scenario = createBrowserScenario();
  const server = writingServer(scenario, mode);
  const openDatabases = new Set<IDBDatabase>();
  let view: ReturnType<typeof mountStage1View> | undefined;
  let openSpy: { mockRestore: () => void } | undefined;
  const mount = async (timers: Timer[]) => {
    // A Chapter switch opens the Journal again in the view. The cleanup closes each connection, so the deletion is not blocked.
    const open = IDBFactory.prototype.open;
    openSpy ??= vi.spyOn(indexedDB, "open").mockImplementation((name, version) => {
      const request = open.call(indexedDB, name, version);
      request.addEventListener("success", () => trackDatabase(request.result, openDatabases));
      return request;
    });
    document.body.innerHTML = '<main id="app"></main>';
    const loaded = await loadStoryOSWebState({ documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    view = mountStage1View(loaded.root, { ...loaded, writingTimers: injectedTimers(timers) });
    if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
      trackDatabase(loaded.state.editor.database, openDatabases);
    }
    await expect.poll(() => loaded.root.querySelector("[data-save-state]") !== null, { timeout: 10_000 }).toBe(true);
    return loaded.root;
  };
  const unmount = () => {
    view?.unmount();
    closeTrackedDatabases(openDatabases);
  };
  const cleanup = async () => {
    unmount();
    document.body.replaceChildren();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    openSpy?.mockRestore();
    openSpy = undefined;
    await deleteJournal(scenario.journalName);
  };
  const editorState = (root: HTMLElement) => {
    const editor = root.querySelector("[contenteditable]");
    const status = root.querySelector("[data-save-state]");
    return { alert: root.querySelector('[role="alert"]')?.textContent ?? null,
      saveState: status?.getAttribute("data-save-state") ?? null,
      editable: editor === null ? null : manuscriptIsEditable(editor) };
  };
  /** What the author sees in the status line, and the failure that the editor keeps for it. */
  const status = (root: HTMLElement) => {
    const line = root.querySelector("[data-save-state]");
    return { label: line?.textContent ?? null, failure: line?.getAttribute("data-editor-failure") ?? null,
      unsettled: line?.getAttribute("data-unsettled-intent-count") ?? null,
      bootState: root.getAttribute("data-boot-state") };
  };
  /** Fires the idle timer until the save state is `saved`, or until input stops. */
  const settle = async (root: HTMLElement, timers: Timer[]) => {
    await expect.poll(() => {
      fire(timers, 250);
      const state = editorState(root);
      const unsettled = root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count");
      return state.saveState === "saved" && unsettled === "0" || state.alert !== null
        || state.saveState === "needs_attention";
    }, { timeout: 30_000 }).toBe(true);
  };
  /**
   * Types batches of `size` "x" until input stops. An idle pause ends each batch, and the group saves.
   * Gives the working set after the first input of each batch, and the count of typed inputs.
   */
  const typeToStop = async (root: HTMLElement, timers: Timer[], batches: number, size: number,
    afterBatch: () => Promise<void> = async () => {}) => {
    const afterFirst: Array<Counts & { fences: number }> = [];
    let inputs = 0;
    for (let batch = 0; batch < batches && editorState(root).alert === null; batch += 1) {
      for (let offset = 0; offset < size; offset += 1) {
        if (editorState(root).alert !== null || !editorState(root).editable) break;
        const before = offset === 0
          ? Object.values(await journalCounts(scenario.journalName))[0]?.all.records ?? 0 : 0;
        await applyTrustedInput({ operation: "insert_text", text: "x" });
        inputs += 1;
        if (offset === 0) {
          await expect.poll(async () => Object.values(await journalCounts(scenario.journalName))[0]?.all.records,
            { timeout: 30_000 }).toBe(before + 1);
          const counts = Object.values(await journalCounts(scenario.journalName))[0]!;
          afterFirst.push({ ...counts.working, fences: counts.workingFences });
        }
      }
      await settle(root, timers);
      await afterBatch();
    }
    return { afterFirst, inputs };
  };
  return { scenario, server, mount, unmount, cleanup, view: editorState, status, settle, typeToStop };
}

it("lets input continue after one group with no effect and 2,400 more inputs, and after a reload", async () => {
  const { scenario, server, mount, unmount, cleanup, view, status, settle, typeToStop } = harness("no_effect");
  await cleanup();
  // The latest content of each working row before it leaves the working set.
  const seen = new Map<string, Row>();
  const remember = async () => {
    const rows = await journalRows(scenario.journalName);
    for (const store of STORES) {
      for (const { working_set_partition_id: _index, ...row } of rows[store].working) seen.set(`${store}:${rowKey(row)}`, row);
    }
  };
  try {
    const timers: Timer[] = [];
    let root = await mount(timers);
    const editor = () => manuscriptEditor(root, window);
    focusManuscriptEnd(editor(), window);
    // Step 1: one group whose text does not change. The Server gives `no_effect`.
    await applyTrustedInput({ operation: "insert_text", text: "x" });
    await applyTrustedInput({ operation: "backspace" });
    await expect.poll(async () => Object.values(await journalCounts(scenario.journalName))[0]?.all.records).toBe(2);
    await settle(root, timers);
    await remember();
    const step1 = { effects: [...server.effects], view: view(root),
      journal: Object.values(await journalCounts(scenario.journalName))[0] };

    // Step 2: 13 saved groups of 200 inputs, 2,600 inputs in all.
    const typed = await typeToStop(root, timers, 13, 200, remember);
    const step2 = { ...typed, effects: server.effects.length, view: view(root), status: status(root),
      journal: Object.values(await journalCounts(scenario.journalName))[0] };
    const rows = await journalRows(scenario.journalName);

    const ready = { alert: null, saveState: "saved", editable: true };
    expect(step1).toMatchObject({ effects: ["no_effect"], view: ready,
      journal: { working: { records: 2, chains: 1, groups: 1 }, uncollected: ["1-2:no_effect"] } });
    // The group with no effect leaves with the first collected group, before the first input of the second batch.
    expect(step2.afterFirst.map((counts) => counts.records)).toEqual([3, ...Array.from({ length: 12 }, () => 1)]);
    expect(step2).toMatchObject({ inputs: 2600, effects: 14, view: ready, status: { unsettled: "0" },
      journal: { working: { records: 200, chains: 1, groups: 1 }, boundary: 2402, uncollected: [] } });
    // The retained history has each row that left the working set, with its content before it left.
    const working = new Set(STORES.flatMap((store) => rows[store].working.map((row) => `${store}:${rowKey(row)}`)));
    const left = [...seen].filter(([key]) => !working.has(key)).map(([, row]) => row);
    const retained = STORES.flatMap((store) => rows[store].retained);
    const byKey = (items: Row[]) => [...items].sort((a, b) => rowKey(a).localeCompare(rowKey(b)));
    expect(rows.intents.retained).toHaveLength(2402);
    expect(rows.submission_groups.retained).toHaveLength(13);
    expect(byKey(retained)).toEqual(byKey(left));
    expect(rows.intents.retained.filter((row) => row.author_edit_unit !== undefined)).toHaveLength(2);

    // Step 3: a reload in the same tab opens the same Editor Session and the same partition. Then one more input.
    unmount();
    const reloadTimers: Timer[] = [];
    root = await mount(reloadTimers);
    const step3Opened = { view: view(root), status: status(root) };
    editor().focus();
    focusManuscriptEnd(editor(), window);
    await applyTrustedInput({ operation: "insert_text", text: "z" });
    await expect.poll(async () => Object.values(await journalCounts(scenario.journalName))[0]?.all.records)
      .toBe(2603);
    await settle(root, reloadTimers);
    const step3 = { opened: step3Opened, view: view(root), status: status(root),
      body: manuscriptBody(editor()).slice(-3), journal: Object.values(await journalCounts(scenario.journalName))[0] };
    // After the reload, the editor is saved and writable, and one more input saves.
    expect(step3).toMatchObject({ opened: { view: ready, status: { unsettled: "0" } }, view: ready,
      status: { unsettled: "0" }, body: "xxz", journal: { working: { records: 1, groups: 1 }, boundary: 2602 } });
    expect(server.unexpected).toEqual([]);
  } finally {
    await cleanup();
  }
}, 240_000);

it("saves 240 inputs with no pause after a group with no effect", async () => {
  const { scenario, server, mount, cleanup, view, status, settle, typeToStop } = harness("no_effect");
  await cleanup();
  try {
    const timers: Timer[] = [];
    const root = await mount(timers);
    focusManuscriptEnd(manuscriptEditor(root, window), window);
    await applyTrustedInput({ operation: "insert_text", text: "x" });
    await applyTrustedInput({ operation: "backspace" });
    await expect.poll(async () => Object.values(await journalCounts(scenario.journalName))[0]?.all.records).toBe(2);
    await settle(root, timers);
    // Input after the settled group starts a new payload chain, so the chain limit counts only this run (ADR 0065).
    const typed = await typeToStop(root, timers, 1, 240);
    const journal = Object.values(await journalCounts(scenario.journalName))[0];
    expect({ inputs: typed.inputs, view: view(root), unsettled: status(root).unsettled, working: journal?.working,
      uncollected: journal?.uncollected }).toEqual({ inputs: 240,
      view: { alert: null, saveState: "saved", editable: true }, unsettled: "0",
      working: { records: 242, chains: 2, groups: 2 }, uncollected: ["1-2:no_effect"] });
    expect(server.unexpected).toEqual([]);
  } finally {
    await cleanup();
  }
}, 60_000);

it("retires an applied group that the input of a Challenge wait follows with the collected group after it", async () => {
  const { scenario, server, mount, cleanup, view, settle, typeToStop } = harness("challenge_wait");
  await cleanup();
  try {
    const timers: Timer[] = [];
    const root = await mount(timers);
    focusManuscriptEnd(manuscriptEditor(root, window), window);
    await applyTrustedInput({ operation: "insert_text", text: "!?" });
    await expect.poll(() => timers.some((timer) => timer.timeout === 250)).toBe(true);
    fire(timers, 250);
    await expect.poll(() => timers.some((timer) => timer.timeout === 7_000)).toBe(true);
    for (let index = 0; index < 3; index += 1) await applyTrustedInput({ operation: "insert_text", text: "x" });
    fire(timers, 7_000);
    await settle(root, timers);
    const afterWait = { view: view(root), journal: Object.values(await journalCounts(scenario.journalName))[0] };
    const typed = await typeToStop(root, timers, 2, 240);
    const after = { ...typed, journal: Object.values(await journalCounts(scenario.journalName))[0] };
    // The first group is applied, but the collection does not take it. It leaves with the collected group after it.
    expect(afterWait).toMatchObject({ view: { alert: null, saveState: "saved", editable: true },
      journal: { working: { records: 4, groups: 2 }, uncollected: ["1-1:applied_receipt_settled"] } });
    expect(after.afterFirst.map((counts) => counts.records)).toEqual([1, 1]);
    expect(after.journal).toMatchObject({ working: { records: 240, groups: 1 }, boundary: 244, workingFences: 1,
      uncollected: [] });
    expect(server.unexpected).toEqual([]);
  } finally {
    await cleanup();
  }
}, 60_000);

it("lets input continue in another Chapter after a group with no effect", async () => {
  const { scenario, server, mount, cleanup, view, status, settle, typeToStop } = harness("no_effect");
  await cleanup();
  try {
    const timers: Timer[] = [];
    const root = await mount(timers);
    focusManuscriptEnd(manuscriptEditor(root, window), window);
    await applyTrustedInput({ operation: "insert_text", text: "x" });
    await applyTrustedInput({ operation: "backspace" });
    await expect.poll(async () => Object.values(await journalCounts(scenario.journalName))[0]?.all.records).toBe(2);
    await settle(root, timers);
    expect(view(root)).toEqual({ alert: null, saveState: "saved", editable: true });
    // The author opens Chapter B before a later group of Chapter A. The group with no effect stays in the working set.
    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();
    await expect.poll(() => manuscriptBody(manuscriptEditor(root, window)), { timeout: 10_000 }).toBe("B");
    await expect.poll(() => view(root), { timeout: 10_000 })
      .toEqual({ alert: null, saveState: "saved", editable: true });
    focusManuscriptEnd(manuscriptEditor(root, window), window);
    const typed = await typeToStop(root, timers, 13, 200);
    const journal = Object.values(await journalCounts(scenario.journalName))[0];
    // The group of Chapter A is closed by the checkpoint of its chain, and it leaves with the first collected group.
    expect(typed.afterFirst.map((counts) => counts.records)).toEqual([3, ...Array.from({ length: 12 }, () => 1)]);
    expect({ inputs: typed.inputs, view: view(root), unsettled: status(root).unsettled, working: journal?.working,
      uncollected: journal?.uncollected }).toEqual({ inputs: 2600,
      view: { alert: null, saveState: "saved", editable: true }, unsettled: "0",
      working: { records: 200, chains: 1, groups: 1 }, uncollected: [] });
    expect(server.unexpected).toEqual([]);
  } finally {
    await cleanup();
  }
}, 240_000);

it("keeps a group with no effect of another Chapter when its final text is not the checkpoint of its chain", async () => {
  const { scenario, server, mount, unmount, cleanup, view, settle, typeToStop } = harness("changed_no_effect");
  await cleanup();
  try {
    const timers: Timer[] = [];
    let root = await mount(timers);
    focusManuscriptEnd(manuscriptEditor(root, window), window);
    // The group changes the text, but the fake Server gives `no_effect`. The group is not proven in Chapter A.
    await applyTrustedInput({ operation: "insert_text", text: "x" });
    await expect.poll(async () => Object.values(await journalCounts(scenario.journalName))[0]?.all.records).toBe(1);
    await settle(root, timers);
    expect(view(root).saveState).toBe("needs_attention");
    // Chapter B becomes current, and a reload opens it in the same partition.
    await server.openChapterB();
    unmount();
    const reloadTimers: Timer[] = [];
    root = await mount(reloadTimers);
    await expect.poll(() => manuscriptBody(manuscriptEditor(root, window)), { timeout: 10_000 }).toBe("B");
    focusManuscriptEnd(manuscriptEditor(root, window), window);
    const typed = await typeToStop(root, reloadTimers, 2, 200);
    const journal = Object.values(await journalCounts(scenario.journalName))[0];
    // The group is not closed, so nothing leaves. Input in Chapter B continues behind it (#1775).
    expect(typed.afterFirst.map((counts) => counts.records)).toEqual([2, 202]);
    expect({ view: view(root), working: journal?.working, uncollected: journal?.uncollected }).toEqual({
      view: { alert: null, saveState: "saved", editable: true }, working: { records: 401, chains: 3, groups: 3 },
      uncollected: ["1-1:no_effect"] });
    expect(server.unexpected).toEqual([]);
  } finally {
    await cleanup();
  }
}, 60_000);
