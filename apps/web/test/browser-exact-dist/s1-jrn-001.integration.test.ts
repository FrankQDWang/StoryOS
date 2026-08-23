import { afterEach, expect, it } from "vitest";

import { activityStream } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { JOURNAL_DATABASE_VERSION } from "../../src/local-edit-journal.ts";
import {
  applyTrustedInput,
  updateClientSessionCookie,
  updateClipboardPermission,
} from "../support/browser-command-client.ts";
import {
  AFTER_IME,
  AFTER_PASTE,
  AFTER_TYPE,
  AFTER_UNSETTLED,
  CHAPTER,
  OPEN_BODY,
  PROJECT_A,
  USER_A,
  type JourneyJournal,
  type JourneyRow,
  type ObservedStage1Journey,
  expectedStage1Journey,
  normalizeStage1Journey,
} from "./stage1-journey-expectation.ts";

interface PendingSurface {
  readonly authoritative_revision_id: string | null;
  readonly body: string | null;
  readonly save_state: string | null;
  readonly unsettled_intent_count: number;
}

const JOURNAL_NAME = `storyos-local-edit-journal:${USER_A}:${PROJECT_A}`;
const ACTIVE_SESSION_KEY = `active_session:${USER_A}:${PROJECT_A}`;
let applicationFrame: HTMLIFrameElement | undefined;

function isRow(value: unknown): value is JourneyRow {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function row(value: unknown, label: string): JourneyRow {
  if (!isRow(value)) throw new Error(`${label} is not an object`);
  return value;
}

function rows(value: unknown, label: string): JourneyRow[] {
  if (!Array.isArray(value) || !value.every(isRow)) {
    throw new Error(`${label} is not an object array`);
  }
  return value;
}

function valueAt(value: unknown, key: string): unknown {
  return Reflect.get(row(value, key), key);
}

function stringAt(value: unknown, key: string): string {
  const result = valueAt(value, key);
  if (typeof result !== "string") throw new Error(`${key} is not a string`);
  return result;
}

function first<Value>(values: readonly Value[], label: string): Value {
  const value = values[0];
  if (value === undefined) throw new Error(`${label} is empty`);
  return value;
}

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist Stage 1 application frame did not load"));
    }, 10_000);
    frame.addEventListener("load", () => {
      window.clearTimeout(timeout);
      resolve();
    }, { once: true });
  });
}

async function waitFor(
  label: string,
  condition: () => boolean | Promise<boolean>,
  timeoutMs = 20_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await condition()) return;
    await new Promise<void>((resolve) => {
      window.setTimeout(resolve, 25);
    });
  }
  throw new Error(`the exact-dist Stage 1 journey timed out while waiting for ${label}`);
}

function applicationWindow(frame: HTMLIFrameElement): Window & typeof globalThis {
  const result = frame.contentWindow;
  if (result === null) throw new Error("the exact-dist application realm is unavailable");
  return result as Window & typeof globalThis;
}

function editor(frame: HTMLIFrameElement): HTMLTextAreaElement {
  const childWindow = applicationWindow(frame);
  const result = frame.contentDocument?.querySelector("textarea");
  if (!(result instanceof childWindow.HTMLTextAreaElement)) {
    throw new Error("the Stage 1 editor is unavailable");
  }
  return result;
}

function readSurface(frame: HTMLIFrameElement) {
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) throw new Error("the Stage 1 root is unavailable");
  const currentEditor = frame.contentDocument?.querySelector("textarea");
  return {
    alert: root.querySelector('[role="alert"]') !== null,
    bootState: root.getAttribute("data-boot-state"),
    chapter: root.querySelector("h2")?.textContent ?? null,
    heading: root.querySelector("h1")?.textContent ?? null,
    readOnly: currentEditor?.hasAttribute("readonly") ?? null,
  };
}

function readPending(frame: HTMLIFrameElement): PendingSurface {
  const saveState = frame.contentDocument?.querySelector("[data-save-state]");
  return {
    body: frame.contentDocument?.querySelector("textarea")?.getAttribute("value")
      ?? frame.contentDocument?.querySelector("textarea")?.textContent
      ?? null,
    save_state: saveState?.getAttribute("data-save-state") ?? null,
    unsettled_intent_count: Number(
      saveState?.getAttribute("data-unsettled-intent-count") ?? "NaN",
    ),
    authoritative_revision_id:
      saveState?.getAttribute("data-authoritative-revision-id") ?? null,
  };
}

function pendingWithLiveBody(frame: HTMLIFrameElement): PendingSurface {
  const pending = readPending(frame);
  return { ...pending, body: editor(frame).value };
}

function focusAtEnd(frame: HTMLIFrameElement): number {
  const currentEditor = editor(frame);
  currentEditor.focus();
  currentEditor.setSelectionRange(currentEditor.value.length, currentEditor.value.length);
  return currentEditor.value.length;
}

function requestResult<Result>(request: IDBRequest<Result>): Promise<Result> {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => resolve(request.result), { once: true });
    request.addEventListener("error", () => reject(request.error), { once: true });
  });
}

async function readJourneyJournal(childWindow: Window): Promise<JourneyJournal> {
  const database = await requestResult(
    childWindow.indexedDB.open(JOURNAL_NAME, JOURNAL_DATABASE_VERSION),
  );
  try {
    const transaction = database.transaction(
      ["submission_groups", "intents", "metadata"],
      "readonly",
    );
    const groupsRequest = transaction.objectStore("submission_groups").getAll();
    const intentsRequest = transaction.objectStore("intents").getAll();
    const metadataRequest = transaction.objectStore("metadata").getAll();
    const [groupValues, intentValues, metadataValues] = await Promise.all([
      requestResult(groupsRequest),
      requestResult(intentsRequest),
      requestResult(metadataRequest),
    ]);
    const groupsResult = rows(groupValues, "Journal groups").sort((left, right) =>
      Number(valueAt(valueAt(left, "covered_sequence_range"), "first"))
      - Number(valueAt(valueAt(right, "covered_sequence_range"), "first")));
    const intentsResult = rows(intentValues, "Journal intents").sort((left, right) =>
      Number(valueAt(left, "local_intent_sequence"))
      - Number(valueAt(right, "local_intent_sequence")));
    const fences = rows(metadataValues, "Journal metadata")
      .filter((entry) => stringAt(entry, "key").startsWith("collection_fences:"))
      .flatMap((entry) => rows(valueAt(entry, "value"), "Journal collection fences"));
    return { fences, groups: groupsResult, intents: intentsResult };
  } finally {
    database.close();
  }
}

function collectedGroups(journal: JourneyJournal, count: number): boolean {
  return journal.groups.filter((group) => {
    const collection = valueAt(group, "payload_collection");
    return isRow(collection) && valueAt(collection, "kind") === "collected";
  }).length === count
    && journal.intents.length === count
    && journal.intents.every((intent) => valueAt(intent, "author_edit_unit") === undefined);
}

function hasRetainedIntent(journal: JourneyJournal): boolean {
  return journal.intents.some((intent) => valueAt(intent, "author_edit_unit") !== undefined);
}

function settledAuthority(journal: JourneyJournal): {
  readonly effects: JourneyRow[];
  readonly receipts: JourneyRow[];
} {
  const settlements = journal.groups.map((group) => row(valueAt(group, "settlement"), "settlement"));
  const receipts = settlements.map((settlement) => row(valueAt(settlement, "receipt"), "receipt"));
  const effects = settlements.map((settlement) => ({
    kind: valueAt(settlement, "kind") === "applied_receipt_settled"
      ? "authoritative_applied"
      : valueAt(settlement, "kind"),
    authoritative_revision: valueAt(settlement, "authoritative_revision"),
    authoritative_commit_id: valueAt(settlement, "authoritative_commit_id"),
    author_action_sequence: valueAt(settlement, "author_action_sequence"),
    project_activity_position: valueAt(settlement, "project_activity_position"),
  }));
  return { effects, receipts };
}

function canonicalJson(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (isRow(value)) {
    return Object.fromEntries(
      Object.keys(value).sort().map((key) => [key, canonicalJson(Reflect.get(value, key))]),
    );
  }
  return value;
}

async function eventPayloadDigest(payload: unknown) {
  const bytes = new TextEncoder().encode(JSON.stringify(canonicalJson(payload)));
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return {
    algorithm: "sha256",
    profile: "storyos.event-payload.jcs.v1",
    value_hex_lowercase: [...digest]
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join(""),
  };
}

function parseSseFrames(body: string): Array<{ readonly data: unknown; readonly event: string }> {
  return body.split("\n\n").filter((block) => block.trim().length > 0).map((block) => {
    let data: unknown;
    let event = "";
    for (const line of block.split("\n")) {
      if (line.startsWith("event:")) event = line.slice(6).trim();
      if (line.startsWith("data:")) data = JSON.parse(line.slice(5).trim()) as unknown;
    }
    return { data, event };
  });
}

async function projectActivities(snapshotId: string): Promise<JourneyRow[]> {
  const body = await activityStream({
    baseUrl: location.origin,
    projectId: PROJECT_A,
    snapshotId,
    protocolRelease: "storyos.public.release.1",
  });
  const activities: JourneyRow[] = [];
  for (const frame of parseSseFrames(body)) {
    expect(frame.event).toBe("storyos.project-activity");
    const activity = row(frame.data, "Project Activity");
    expect(valueAt(activity, "payload_digest")).toEqual(
      await eventPayloadDigest(valueAt(activity, "payload")),
    );
    activities.push(activity);
  }
  return activities;
}

async function deleteJournalDatabase(): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const request = indexedDB.deleteDatabase(JOURNAL_NAME);
    request.addEventListener("success", () => resolve(), { once: true });
    request.addEventListener("error", () => reject(request.error), { once: true });
    // A close-pending connection can finish an active transaction before this request succeeds.
    // The blocked event is an intermediate state, not a failed deletion.
  });
}

async function destroyApplicationFrame(frame: HTMLIFrameElement): Promise<void> {
  if (!frame.isConnected) return;
  const unloaded = nextFrameLoad(frame);
  frame.src = "about:blank";
  await unloaded;
  frame.remove();
}

afterEach(async () => {
  if (applicationFrame !== undefined) await destroyApplicationFrame(applicationFrame);
  applicationFrame = undefined;
  await updateClientSessionCookie({ action: "clear" });
  await updateClipboardPermission({ action: "clear" });
  sessionStorage.removeItem(ACTIVE_SESSION_KEY);
  await deleteJournalDatabase();
  document.body.replaceChildren();
});

it("S1-JRN-001 uses the Vite production page, storyos-server, Application, Core, and PostgreSQL", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  await updateClipboardPermission({ action: "grant" });

  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Stage 1 application";
  const loaded = nextFrameLoad(frame);
  frame.src = `/projects/${PROJECT_A}`;
  document.body.append(frame);
  await loaded;
  await waitFor("the Project-ready editor", () =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state") === "project-ready"
    && frame.contentDocument?.querySelector("textarea") !== null);

  const firstRealm = applicationWindow(frame);
  Reflect.set(firstRealm, "__storyosStage1Realm", "first");
  Reflect.set(window, "__storyosStage1Orchestrator", "retained");
  const open = { ...readSurface(frame), pending: pendingWithLiveBody(frame) };
  expect(open.pending.body).toBe(OPEN_BODY);

  focusAtEnd(frame);
  await applyTrustedInput({ operation: "insert_text", text: " Hello" });
  await waitFor("the first saving projection", () => {
    const pending = pendingWithLiveBody(frame);
    return pending.body === AFTER_TYPE && pending.save_state === "saving";
  });
  const input = pendingWithLiveBody(frame);
  await waitFor("the first collected Journal group", async () =>
    collectedGroups(await readJourneyJournal(applicationWindow(frame)), 1));
  await waitFor("the first saved projection", () => {
    const pending = pendingWithLiveBody(frame);
    return pending.body === AFTER_TYPE && pending.save_state === "saved";
  });
  const afterType = pendingWithLiveBody(frame);

  focusAtEnd(frame);
  await applyTrustedInput({ operation: "insert_text", text: "中文" });
  await waitFor("the IME saving projection", () => {
    const pending = pendingWithLiveBody(frame);
    return pending.body === AFTER_IME && pending.save_state === "saving";
  });
  const afterImeInput = pendingWithLiveBody(frame);
  await waitFor("the second collected Journal group", async () =>
    collectedGroups(await readJourneyJournal(applicationWindow(frame)), 2));
  await waitFor("the IME saved projection", () => {
    const pending = pendingWithLiveBody(frame);
    return pending.body === AFTER_IME && pending.save_state === "saved";
  });
  const afterIme = pendingWithLiveBody(frame);

  await applicationWindow(frame).navigator.clipboard.writeText(" EN");
  focusAtEnd(frame);
  await applyTrustedInput({ operation: "paste" });
  await waitFor("the paste saving projection", () => {
    const pending = pendingWithLiveBody(frame);
    return pending.body === AFTER_PASTE && pending.save_state === "saving";
  });
  const afterPasteInput = pendingWithLiveBody(frame);
  await waitFor("the third collected Journal group", async () =>
    collectedGroups(await readJourneyJournal(applicationWindow(frame)), 3));
  await waitFor("the paste saved projection", () => {
    const pending = pendingWithLiveBody(frame);
    return pending.body === AFTER_PASTE && pending.save_state === "saved";
  });
  const settlePending = pendingWithLiveBody(frame);
  const settledJournal = await readJourneyJournal(applicationWindow(frame));

  focusAtEnd(frame);
  await applyTrustedInput({ operation: "insert_text", text: "!" });
  await waitFor("the retained fourth intent", async () => {
    const pending = pendingWithLiveBody(frame);
    return pending.body === AFTER_UNSETTLED
      && pending.save_state === "saving"
      && hasRetainedIntent(await readJourneyJournal(applicationWindow(frame)));
  });
  const interruptPending = pendingWithLiveBody(frame);
  const unsettledJournal = await readJourneyJournal(applicationWindow(frame));

  const reloaded = nextFrameLoad(frame);
  firstRealm.location.reload();
  await reloaded;
  await waitFor("the recovered Project-ready editor", () =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state") === "project-ready"
    && frame.contentDocument?.querySelector("textarea") !== null);
  expect(Reflect.get(window, "__storyosStage1Orchestrator")).toBe("retained");
  expect(Reflect.get(applicationWindow(frame), "__storyosStage1Realm")).toBeUndefined();
  await waitFor("the recovered pending projection", () =>
    pendingWithLiveBody(frame).body === AFTER_UNSETTLED);
  const recoveredPending = pendingWithLiveBody(frame);
  await waitFor("the recovered saved projection", () => {
    const pending = pendingWithLiveBody(frame);
    return pending.body === AFTER_UNSETTLED
      && pending.save_state === "saved"
      && readSurface(frame).readOnly === false;
  });
  const recoveredSaved = {
    ...readSurface(frame),
    pending: pendingWithLiveBody(frame),
  };
  const collectedJournal = await readJourneyJournal(applicationWindow(frame));
  const authority = settledAuthority(collectedJournal);
  const activities = await projectActivities(
    stringAt(first(collectedJournal.intents, "Journal intents"), "base_snapshot_id"),
  );
  const observed: ObservedStage1Journey = {
    id: "S1-JRN-001",
    open,
    input,
    afterType,
    afterImeInput,
    afterIme,
    afterPasteInput,
    settle: { pending: settlePending, journal: settledJournal },
    interrupt: { pending: interruptPending, journal: unsettledJournal },
    recover: {
      pending: recoveredPending,
      saved: recoveredSaved,
      journal: collectedJournal,
    },
    authority: {
      ...authority,
      activities,
      manuscript: {
        revision_id: recoveredSaved.pending.authoritative_revision_id,
        body: recoveredSaved.pending.body,
      },
    },
  };
  expect(normalizeStage1Journey(observed)).toEqual(expectedStage1Journey());
});
