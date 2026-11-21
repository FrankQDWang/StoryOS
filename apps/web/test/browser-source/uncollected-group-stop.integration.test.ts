// Acceptance tests of issue 1758: a closed group leaves the working set with the collected prefix (ADR 0036).
import { expect, it } from "vitest";

import type {
  DigestValue,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import type { ChallengeAdmissionTimers } from "../../src/challenge-admission-wait.ts";
import { uuidV7 } from "../../src/client-identity.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyImeComposition, applyTrustedInput } from "../support/browser-command-client.ts";
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

const AUTHOR_EDIT_SCHEMA = "storyos.command.apply-author-edit.request.v1";

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
 * A Server that applies each Author Edit on the current head. In the mode "no_effect", the first Author Edit
 * has the effect `no_effect` (`content_unchanged`). In the mode "challenge_wait", the first Author Edit Challenge
 * has a rate limit, so the input of the wait follows the first group (ADR 0065).
 */
function writingServer(scenario: ReturnType<typeof createBrowserScenario>, mode: "no_effect" | "challenge_wait" | "applied") {
  const sessionId = scenario.session.editor_session.editor_session_id;
  let session: GetEditorSessionResponse = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
  let body = session.base_snapshot.materialized_revision.body;
  let position = 0;
  let challenges = 0;
  const effects: string[] = [];
  let digest: DigestValue | undefined;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: { owner_user_id: OWNER, project_id: PROJECT }, title: "Project A",
          revision: "1", open: { kind: "current_chapter", current_chapter_id: CHAPTER } }] });
    }
    if (path === `/api/v1/projects/${PROJECT}`) return jsonResponse(scenario.project);
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}`) {
      return jsonResponse({ ...scenario.chapter, project_activity_position: String(position),
        chapter: { ...scenario.chapter.chapter, current_revision: session.base_snapshot.materialized_revision } });
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scenario.project.project_scope,
        tree_revision: "2",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scenario.project.project_scope },
        volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000815", title: "Volume A", order: "1",
          chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }] }] });
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
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
    if (path.endsWith("/editor-sessions")) {
      return jsonResponse({ ...session, schema_id: scenario.session.schema_id,
        editor_session: { ...session.editor_session, editor_session_id: sessionId } });
    }
    if (path.endsWith(`/editor-sessions/${sessionId}`)) {
      return jsonResponse({ ...session, editor_session: { ...session.editor_session, editor_session_id: sessionId } });
    }
    if (path.endsWith("/activity")) return emptyActivityStream();
    if (path.endsWith("/manuscript/author-edits") && digest !== undefined) {
      const idempotencyKey = requestHeaders(init).get("idempotency-key") ?? "";
      const request = requireRequestBody(init);
      const head = session.base_snapshot.authoritative_head_revision_id;
      const applied = createAppliedAuthorEditResponse({ request, commandDigest: digest, idempotencyKey,
        commandId: uuidV7(crypto), authorCommandAdmissionId: uuidV7(crypto), receiptId: uuidV7(crypto),
        priorRevisionId: head, projectActivityPosition: String(position + 1), body });
      if (mode === "no_effect" && effects.length === 0) {
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
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  return { fetchImpl, effects };
}

type Counts = { records: number; chains: number; groups: number };

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

function harness(mode: "no_effect" | "challenge_wait" | "applied") {
  const scenario = createBrowserScenario();
  const server = writingServer(scenario, mode);
  const openDatabases = new Set<IDBDatabase>();
  let view: ReturnType<typeof mountStage1View> | undefined;
  const mount = async (timers: Timer[]) => {
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
    await deleteJournal(scenario.journalName);
  };
  const view_ = (root: HTMLElement) => {
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
    const deadline = performance.now() + 60_000;
    while (performance.now() < deadline) {
      fire(timers, 250);
      const state = view_(root);
      const unsettled = root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count");
      if (state.saveState === "saved" && unsettled === "0" || state.alert !== null || state.saveState === "needs_attention") {
        return;
      }
      await new Promise((resolve) => { setTimeout(resolve, 50); });
    }
    throw new Error(`The input did not settle: ${JSON.stringify({ view: view_(root),
      unsettled: root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"),
      text: root.textContent?.slice(0, 400), effects: server.effects.length,
      journal: await journalCounts(scenario.journalName) })}`);
  };
  /**
   * Types batches of `size` "x" until input stops. An idle pause ends each batch, and the group saves.
   * Gives the working set after the first input of each batch, and the count of typed inputs.
   */
  const typeToStop = async (root: HTMLElement, timers: Timer[], batches: number, size: number,
    afterBatch: () => Promise<void> = async () => {}) => {
    const afterFirst: Array<Counts & { fences: number }> = [];
    let inputs = 0;
    for (let batch = 0; batch < batches && view_(root).alert === null; batch += 1) {
      for (let offset = 0; offset < size; offset += 1) {
        if (view_(root).alert !== null || !view_(root).editable) break;
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
  return { scenario, server, mount, unmount, cleanup, view: view_, status, settle, typeToStop };
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
    console.info(JSON.stringify({ issue: 1758, test: "no_effect", step1, step2, step3 }));

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
    // After the reload, the editor is saved and writable, and one more input saves.
    expect(step3).toMatchObject({ opened: { view: ready, status: { unsettled: "0" } }, view: ready,
      status: { unsettled: "0" }, body: "xxz", journal: { working: { records: 1, groups: 1 }, boundary: 2602 } });
  } finally {
    await cleanup();
  }
}, 1_800_000);

it("saves 240 inputs with no pause after a group with no effect", async () => {
  const { scenario, mount, cleanup, view, status, settle, typeToStop } = harness("no_effect");
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
    console.info(JSON.stringify({ issue: 1758, test: "no_effect_chain_limit", typed, view: view(root), journal }));
    expect({ inputs: typed.inputs, view: view(root), unsettled: status(root).unsettled, working: journal?.working,
      uncollected: journal?.uncollected }).toEqual({ inputs: 240,
      view: { alert: null, saveState: "saved", editable: true }, unsettled: "0",
      working: { records: 242, chains: 2, groups: 2 }, uncollected: ["1-2:no_effect"] });
  } finally {
    await cleanup();
  }
}, 300_000);

it("retires an applied group that the input of a Challenge wait follows with the collected group after it", async () => {
  const { scenario, mount, cleanup, view, settle, typeToStop } = harness("challenge_wait");
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
    console.info(JSON.stringify({ issue: 1758, test: "challenge_wait", afterWait, after }));
    // The first group is applied, but the collection does not take it. It leaves with the collected group after it.
    expect(afterWait).toMatchObject({ view: { alert: null, saveState: "saved", editable: true },
      journal: { working: { records: 4, groups: 2 }, uncollected: ["1-1:applied_receipt_settled"] } });
    expect(after.afterFirst.map((counts) => counts.records)).toEqual([1, 1]);
    expect(after.journal).toMatchObject({ working: { records: 240, groups: 1 }, boundary: 244, workingFences: 1,
      uncollected: [] });
  } finally {
    await cleanup();
  }
}, 600_000);

it("counts the Journal records of 1,000 characters of typing and of Chinese composition", async () => {
  let cleanup = async () => {};
  const measure = async (write: (root: HTMLElement, timers: Timer[], settle: (root: HTMLElement, timers: Timer[]) => Promise<void>) => Promise<void>) => {
    await cleanup();
    const { scenario, mount, unmount, settle, ...rest } = harness("applied");
    cleanup = rest.cleanup;
    await cleanup();
    const timers: Timer[] = [];
    const root = await mount(timers);
    await expect.poll(() => manuscriptIsEditable(manuscriptEditor(root, window)), { timeout: 10_000 }).toBe(true);
    manuscriptEditor(root, window).focus();
    focusManuscriptEnd(manuscriptEditor(root, window), window);
    await write(root, timers, settle);
    await settle(root, timers);
    const counts = Object.values(await journalCounts(scenario.journalName))[0]!;
    const body = manuscriptBody(manuscriptEditor(root, window)).length - "Base".length;
    unmount();
    return { characters: body, ...counts };
  };
  try {
    // Typing with a pause after each word of 5 characters and a space: the idle timer ends the group.
    const typing = await measure(async (root, timers, settle) => {
      for (let index = 0; index < 1000; index += 1) {
        await applyTrustedInput({ operation: "insert_text", text: index % 6 === 5 ? " " : "a" });
        if (index % 6 === 5) await settle(root, timers);
      }
    });
    // Chinese composition of words of 2 characters. Each confirmation is a hard boundary (ADR 0046).
    const composition = await measure(async (root, timers, settle) => {
      for (let index = 0; index < 500; index += 1) {
        const offset = manuscriptBody(manuscriptEditor(root, window)).length;
        manuscriptEditor(root, window).focus();
        focusManuscriptEnd(manuscriptEditor(root, window), window);
        await applyImeComposition({ text: "我们", replacementStart: offset, replacementEnd: offset,
          selectionStart: 2, selectionEnd: 2 });
        await applyTrustedInput({ operation: "insert_text", text: "我们" });
        await expect.poll(() => manuscriptBody(manuscriptEditor(root, window)).length, { timeout: 30_000 })
          .toBe("Base".length + (index + 1) * 2);
        // A writer confirms the next word after the save of this one, so the input queue stays short.
        await settle(root, timers);
      }
    });
    console.info(JSON.stringify({ issue: 1758, test: "records_per_1000_characters", typing, composition }));
    expect(typing).toMatchObject({ characters: 1000, all: { records: 1000, chains: 167, groups: 167 } });
    expect(composition).toMatchObject({ characters: 1000, all: { records: 500, chains: 500, groups: 500 } });
  } finally {
    await cleanup();
  }
}, 1_800_000);
