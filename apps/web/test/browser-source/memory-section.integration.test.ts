import { createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it } from "vitest";
import { page } from "vitest/browser";

import type { GetProjectResponse, MemoryEntryInspect }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { ProjectSettingsView } from "../../src/project-settings.tsx";
import { OWNER, jsonResponse } from "./scenario.ts";

// The Memory section of the settings page (ADR 0094 section 9).
const PROJECT = "018f0000-0000-7001-8000-0000000d2001";
const SCOPE = { owner_user_id: OWNER, project_id: PROJECT };
const MEMORY = `/api/v1/projects/${PROJECT}/memory-entries`;
const CONVERSATION = "018f0000-0000-7001-8000-0000000d2002";
const CHAPTER = "018f0000-0000-7001-8000-0000000d2003";
const PROJECT_VIEW: GetProjectResponse = {
  schema_id: "storyos.query.project.response.v1",
  correlation_id: "018f0000-0000-7001-8000-0000000d2010",
  project_scope: SCOPE,
  project: { project_id: PROJECT, title: "Memory Novel", open: { kind: "empty" } },
};
const MARKUP = "<b>他看灯。</b>\nhttps://example.com/lamp";

function entry(number: number, fields: Partial<MemoryEntryInspect>): MemoryEntryInspect {
  return { memory_entry_id: `018f0000-0000-7001-8000-0000000d21${String(number).padStart(2, "0")}`,
    entry_number: String(number), version: "1", entry_type: "other", title: `条目${number}`,
    description: `描述${number}。`, text: `内容${number}。`, origin: "author_request",
    source: { kind: "conversation", conversation_id: CONVERSATION, availability: "available" },
    changed_at: `2026-11-1${number}T08:30:00.000Z`, ...fields };
}

// One entry for each case of the origin line, in the order of the query.
const ENTRIES: MemoryEntryInspect[] = [
  entry(4, { entry_type: "writing_preference", title: "语气", text: "克制。\n不用感叹号。" }),
  entry(2, { entry_type: "relationship", title: "林舟", text: MARKUP, origin: "conversation",
    source: { kind: "conversation", conversation_id: CONVERSATION, availability: "unavailable" } }),
  entry(3, { entry_type: "relationship", title: "阿青", origin: "chapter",
    source: { kind: "chapter", chapter_id: CHAPTER, title: "渡口", availability: "available" } }),
  entry(1, { title: "旧事", origin: "chapter",
    source: { kind: "chapter", chapter_id: CHAPTER, title: null, availability: "unavailable" } }),
];

type Answer = Response | Error;

/** An answer that the test gives later with `give`. */
function held() {
  let give: (answer: Answer) => void = () => {};
  const answer = new Promise<Answer>((resolve) => { give = resolve; });
  return { answer, give };
}

type Sent = { method: string; path: string; key: string | null; nonce: string | null; body: unknown };

/**
 * A fake Server whose answer to each read of the Memory Entries is the next item of `reads`, and whose answer to each
 * delete is the next item of `deletes`.
 */
function fakeServer(reads: (Answer | Promise<Answer>)[], deletes: (Answer | Promise<Answer>)[] = []) {
  const paths: string[] = [];
  const sent: Sent[] = [];
  const nonces = new Map<string, string>();
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    paths.push(path);
    if (method !== "GET") {
      const headers = new Headers(init?.headers);
      sent.push({ method, path, key: headers.get("idempotency-key"), nonce: headers.get("x-storyos-anti-forgery"),
        body: typeof init?.body === "string" ? JSON.parse(init.body) : undefined });
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      // As the Server, one nonce for each idempotency key: "a" * 64 for the first key, then "b" * 64.
      const key = (JSON.parse(String(init?.body)) as { idempotency_key: string }).idempotency_key;
      if (!nonces.has(key)) nonces.set(key, String.fromCharCode(97 + nonces.size).repeat(64));
      const nonce = nonces.get(key)!;
      return jsonResponse({ nonce, expires_at: "2026-11-17T12:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.startsWith(`${MEMORY}/`) && method === "DELETE") {
      const answer = await deletes.shift();
      if (answer === undefined) throw new Error("no delete answer");
      if (answer instanceof Error) throw answer;
      return answer;
    }
    if (path === `/api/v1/projects/${PROJECT}/instruction`) {
      return jsonResponse({ schema_id: "storyos.query.project-instruction.response.v1",
        correlation_id: "018f0000-0000-7001-8000-0000000d2011", project_scope: SCOPE, instruction: null });
    }
    if (path === MEMORY) {
      const answer = await reads.shift();
      if (answer === undefined) throw new Error("no read answer");
      if (answer instanceof Error) throw answer;
      return answer;
    }
    return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
  };
  return { paths, sent, fetchImpl };
}

function deleted(effect: Record<string, unknown>): Response {
  return jsonResponse({ schema_id: "storyos.command.delete-memory-entry.response.v1",
    correlation_id: "018f0000-0000-7001-8000-0000000d2013", project_scope: SCOPE, effect });
}

const challengeRefused = () => jsonResponse({ schema_id: "storyos.problem.v1", code: "challenge_invalid",
  message: "The challenge is invalid." }, 422);

const applied = (memoryEntryId: string) => deleted({ kind: "authoritative_applied", memory_entry_id: memoryEntryId,
  version: "2", memory_revision: "9", project_activity_position: "7" });

const listed = (entries: MemoryEntryInspect[], scope = SCOPE) => jsonResponse({
  schema_id: "storyos.query.memory-entries.response.v1", correlation_id: "018f0000-0000-7001-8000-0000000d2012",
  project_scope: scope, entries });

const roots: Root[] = [];

async function settingsPage(fetchImpl: typeof fetch) {
  document.body.innerHTML = '<main id="app"></main>';
  const root = createRoot(document.querySelector("#app")!);
  roots.push(root);
  root.render(createElement(ProjectSettingsView, {
    baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, navigate: () => {},
    state: { kind: "project-settings-ready", profile: RELEASE_1_PROTOCOL_PROFILE, project: PROJECT_VIEW },
  }));
  await expect.poll(() => document.querySelector("[data-project-instruction]")).not.toBeNull();
}

const section = () => document.querySelector<HTMLElement>("[data-memory-section]");
const titles = () => [...document.querySelectorAll("[data-memory-entry] h4")].map((title) => title.textContent);
/** The controls and the sentence of the entry `number`. */
const actions = (number: number) => [...document.querySelectorAll(
  `[data-memory-entry="${number}"] [data-memory-entry-actions] > *`)].map((element) => element.textContent);
const press = (number: number, control: string) => document.querySelector<HTMLButtonElement>(
  `[data-memory-entry="${number}"] [data-memory-${control}]`)!.click();
/** Presses "删除" of the entry `number`, waits for the confirm step, and presses "确认删除". */
async function confirmDelete(number: number) {
  press(number, "delete");
  await expect.poll(() => actions(number).slice(0, 2)).toEqual(["确认删除", "取消"]);
  press(number, "delete-confirm");
}
const sectionText = () => [...section()?.querySelectorAll("p, button") ?? []].map((element) => element.textContent);

afterEach(() => {
  for (const root of roots.splice(0)) root.unmount();
  document.body.replaceChildren();
});

it("shows each entry in the group of its type with its text, its origin line, and its time", async () => {
  await settingsPage(fakeServer([listed(ENTRIES)]).fetchImpl);
  await expect.poll(() => document.querySelectorAll("[data-memory-entry]").length).toBe(4);
  const headings = [...document.querySelectorAll(".settings-section > h2")].map((heading) => heading.textContent);
  expect(headings).toEqual(["项目指令", "记忆", "人物"]);
  const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
  const shown = [...section()!.querySelectorAll<HTMLElement>("[data-memory-group]")].map((group) => [
    group.querySelector("h3")?.textContent,
    [...group.querySelectorAll<HTMLElement>("[data-memory-entry]")].map((item) => ({
      title: item.querySelector("h4")?.textContent,
      text: item.querySelector(".memory-entry-text")?.textContent,
      origin: item.querySelector("[data-memory-entry-origin]")?.textContent,
      originKind: item.querySelector("[data-memory-entry-origin]")?.getAttribute("data-memory-entry-origin"),
      links: [...item.querySelectorAll("a")].map((link) => link.getAttribute("href")),
      time: item.querySelector("time")?.textContent,
    })),
  ]);
  const at = (day: number) => time.format(new Date(`2026-11-1${day}T08:30:00.000Z`));
  expect(shown).toEqual([
    ["写作偏好", [{ title: "语气", text: "克制。\n不用感叹号。", origin: "你让助手记住的", originKind: "conversation",
      links: [`/projects/${PROJECT}?conversation=${CONVERSATION}`], time: at(4) }]],
    ["关系", [
      { title: "林舟", text: MARKUP, origin: "来自对话来源已不可用", originKind: "unavailable", links: [],
        time: at(2) },
      { title: "阿青", text: "内容3。", origin: "来自章节《渡口》", originKind: "chapter",
        links: [`/projects/${PROJECT}?chapter=${CHAPTER}`], time: at(3) },
    ]],
    ["其他", [{ title: "旧事", text: "内容1。", origin: "来源已不可用", originKind: "unavailable", links: [],
      time: at(1) }]],
  ]);
  // The text is data: the markup is text, and the address in it is not a link.
  expect(section()!.querySelector("b")).toBeNull();
  // Each entry has only its delete control, and the place of the conflict flag is empty.
  for (const item of document.querySelectorAll("[data-memory-entry]")) {
    expect([...item.querySelectorAll("input, textarea, select, button")].map((control) => control.textContent))
      .toEqual(["删除"]);
    expect(item.querySelector("[data-memory-entry-flag]")?.childNodes.length).toBe(0);
  }
});

it("shows 可能已过时, the reason, the source, and the sentence on a flagged entry, with no other control", async () => {
  const flag = (source: NonNullable<MemoryEntryInspect["source"]>) =>
    ({ reason: "第三章说桥在下游。", source, flagged_at: "2026-11-17T09:00:00.000Z" });
  const flagged = [
    entry(1, { flag: flag({ kind: "chapter", chapter_id: CHAPTER, title: "渡口", availability: "available" }) }),
    entry(2, { flag: flag({ kind: "conversation", conversation_id: CONVERSATION, availability: "available" }) }),
    entry(3, { flag: flag({ kind: "chapter", chapter_id: CHAPTER, title: null, availability: "unavailable" }) }),
  ];
  await settingsPage(fakeServer([listed(flagged)]).fetchImpl);
  await expect.poll(() => document.querySelectorAll("[data-memory-entry-flag=flagged]").length).toBe(3);
  const shown = [...document.querySelectorAll<HTMLElement>("[data-memory-entry]")].map((item) => {
    const place = item.querySelector("h4 + [data-memory-entry-flag]");
    const source = place?.querySelector("[data-memory-flag-source]");
    return [[...place?.querySelectorAll("p") ?? []].map((line) => line.textContent),
      source?.getAttribute("data-memory-flag-source"), source?.getAttribute("href") ?? null,
      item.querySelector(".memory-entry-text")?.textContent,
      [...item.querySelectorAll("input, textarea, select, button")].map((control) => control.textContent)];
  });
  const lines = (source: string) => ["可能已过时 第三章说桥在下游。", source, "可以在对话中告诉助手哪个是对的，或删除这条记忆。"];
  expect(shown).toEqual([
    [lines("来自章节《渡口》"), "chapter", `/projects/${PROJECT}?chapter=${CHAPTER}`, "内容1。", ["删除"]],
    [lines("来自对话"), "conversation", `/projects/${PROJECT}?conversation=${CONVERSATION}`, "内容2。", ["删除"]],
    [lines("来源已不可用"), "unavailable", null, "内容3。", ["删除"]],
  ]);
});

it("asks one time before a delete, and 取消 restores the control and sends nothing", async () => {
  const server = fakeServer([listed(ENTRIES)]);
  await settingsPage(server.fetchImpl);
  await expect.poll(() => actions(4)).toEqual(["删除"]);
  press(4, "delete");
  await expect.poll(() => actions(4)).toEqual(["确认删除", "取消"]);
  expect(actions(2)).toEqual(["删除"]);
  press(4, "delete-cancel");
  await expect.poll(() => actions(4)).toEqual(["删除"]);
  expect([server.sent, titles()]).toEqual([[], ["语气", "林舟", "阿青", "旧事"]]);
});

it("removes a deleted entry and an entry that was deleted before, with no sentence", async () => {
  const [first, second] = [ENTRIES[0]!.memory_entry_id, ENTRIES[1]!.memory_entry_id];
  const server = fakeServer([listed(ENTRIES)],
    [applied(first), deleted({ kind: "no_effect", reason: "already_deleted" })]);
  await settingsPage(server.fetchImpl);
  await expect.poll(() => actions(4)).toEqual(["删除"]);
  await confirmDelete(4);
  await expect.poll(titles).toEqual(["林舟", "阿青", "旧事"]);
  await confirmDelete(2);
  await expect.poll(titles).toEqual(["阿青", "旧事"]);
  expect(section()!.querySelector("[role=alert]")).toBeNull();
  const deletes = server.sent.filter((item) => item.method === "DELETE");
  expect(deletes.map((item) => [item.path, (item.body as { delete_memory_entry_input: { expected_version: string } })
    .delete_memory_entry_input.expected_version])).toEqual([[`${MEMORY}/${first}`, "1"], [`${MEMORY}/${second}`, "1"]]);
  expect(new Set(deletes.map((item) => item.key)).size).toBe(2);
  expect(server.paths.filter((path) => path === MEMORY).length).toBe(1);
});

it("reads the list again after a changed entry, and shows the changed entry with one sentence", async () => {
  const changed = [{ ...ENTRIES[0]!, version: "2", text: "克制。" }, ...ENTRIES.slice(1)];
  const server = fakeServer([listed(ENTRIES), listed(changed)], [deleted({ kind: "conflicted", reason: "entry_changed" })]);
  await settingsPage(server.fetchImpl);
  await expect.poll(() => actions(4)).toEqual(["删除"]);
  await confirmDelete(4);
  await expect.poll(() => actions(4)).toEqual(["删除", "这条记忆已经改变，没有删除。"]);
  await expect.poll(() => document.querySelector('[data-memory-entry="4"] .memory-entry-text')?.textContent)
    .toBe("克制。");
  expect([server.paths.filter((path) => path === MEMORY).length, titles().length]).toEqual([2, 4]);
});

it("keeps the entry after a failed delete with one sentence, and the next confirm sends the same key", async () => {
  const first = ENTRIES[0]!.memory_entry_id;
  const server = fakeServer([listed(ENTRIES)], [new TypeError("Failed to fetch"), applied(first)]);
  await settingsPage(server.fetchImpl);
  await expect.poll(() => actions(4)).toEqual(["删除"]);
  await confirmDelete(4);
  await expect.poll(() => actions(4)).toEqual(["删除", "无法删除这条记忆，请稍后再试。"]);
  expect(titles()).toEqual(["语气", "林舟", "阿青", "旧事"]);
  await confirmDelete(4);
  await expect.poll(titles).toEqual(["林舟", "阿青", "旧事"]);
  // One Command Challenge, and one idempotency key for the delete intent.
  const sent = server.sent.map((item) => [item.method, item.key]);
  const key = sent[1]?.[1];
  expect([typeof key, sent]).toEqual(["string", [["POST", null], ["DELETE", key], ["DELETE", key]]]);
});

it("sends the next delete with a new idempotency key and a new Challenge after a refused Challenge", async () => {
  const first = ENTRIES[0]!.memory_entry_id;
  const server = fakeServer([listed(ENTRIES)], [challengeRefused(), applied(first)]);
  await settingsPage(server.fetchImpl);
  await expect.poll(() => actions(4)).toEqual(["删除"]);
  await confirmDelete(4);
  await expect.poll(() => actions(4)).toEqual(["删除", "无法删除这条记忆，请稍后再试。"]);
  await confirmDelete(4);
  await expect.poll(titles).toEqual(["林舟", "阿青", "旧事"]);
  const sent = server.sent.map((item) => [item.method, item.key, item.nonce]);
  const [refusedKey, retryKey] = [sent[1]?.[1], sent[3]?.[1]];
  expect([typeof refusedKey, typeof retryKey, refusedKey === retryKey, sent]).toEqual(["string", "string", false,
    [["POST", null, null], ["DELETE", refusedKey, "a".repeat(64)], ["POST", null, null],
      ["DELETE", retryKey, "b".repeat(64)]]]);
});

it("keeps the step and the key of each entry apart when the author deletes two entries", async () => {
  const [first, second] = [ENTRIES[0]!.memory_entry_id, ENTRIES[1]!.memory_entry_id];
  const failure = held();
  const server = fakeServer([listed(ENTRIES)], [failure.answer, applied(second), applied(first)]);
  await settingsPage(server.fetchImpl);
  await expect.poll(() => actions(4)).toEqual(["删除"]);
  await confirmDelete(4);
  await expect.poll(() => server.sent.filter((item) => item.method === "DELETE").length).toBe(1);
  // While the delete of entry 4 is sent, "删除" of entry 2 does not change the step of entry 4.
  press(2, "delete");
  await expect.poll(() => actions(2)).toEqual(["确认删除", "取消"]);
  expect(actions(4)).toEqual(["确认删除", "取消"]);
  expect(document.querySelector<HTMLButtonElement>('[data-memory-entry="4"] [data-memory-delete-confirm]')?.disabled)
    .toBe(true);
  failure.give(new TypeError("Failed to fetch"));
  await expect.poll(() => actions(4)).toEqual(["删除", "无法删除这条记忆，请稍后再试。"]);
  expect(actions(2)).toEqual(["确认删除", "取消"]);
  // The applied delete of entry 2 does not remove the delete intent of entry 4.
  press(2, "delete-confirm");
  await expect.poll(titles).toEqual(["语气", "阿青", "旧事"]);
  expect(actions(4)).toEqual(["删除", "无法删除这条记忆，请稍后再试。"]);
  await confirmDelete(4);
  await expect.poll(titles).toEqual(["阿青", "旧事"]);
  const deletes = server.sent.filter((item) => item.method === "DELETE").map((item) => [item.path, item.key]);
  const [firstKey, secondKey] = [deletes[0]?.[1], deletes[1]?.[1]];
  expect([typeof firstKey, firstKey === secondKey, deletes]).toEqual(["string", false,
    [[`${MEMORY}/${first}`, firstKey], [`${MEMORY}/${second}`, secondKey], [`${MEMORY}/${first}`, firstKey]]]);
});

it("does not show again a deleted entry when an earlier read gives it later", async () => {
  const second = ENTRIES[1]!.memory_entry_id;
  const reread = held();
  const changed = [{ ...ENTRIES[0]!, version: "2", text: "克制。" }, ...ENTRIES.slice(1)];
  const server = fakeServer([listed(ENTRIES), reread.answer],
    [deleted({ kind: "conflicted", reason: "entry_changed" }), applied(second)]);
  await settingsPage(server.fetchImpl);
  await expect.poll(() => actions(4)).toEqual(["删除"]);
  await confirmDelete(4);
  await expect.poll(() => server.paths.filter((path) => path === MEMORY).length).toBe(2);
  // The read after the changed entry is open, and the author deletes entry 2.
  await confirmDelete(2);
  await expect.poll(titles).toEqual(["语气", "阿青", "旧事"]);
  reread.give(listed(changed));
  await expect.poll(() => document.querySelector('[data-memory-entry="4"] .memory-entry-text')?.textContent)
    .toBe("克制。");
  expect([titles(), actions(4)]).toEqual([["语气", "阿青", "旧事"], ["删除", "这条记忆已经改变，没有删除。"]]);
});

it("shows the name of each type in the order of the query, and no group 人物", async () => {
  const types = ["writing_preference", "foreshadowing", "character", "relationship", "major_event", "other"] as const;
  await settingsPage(fakeServer([listed(types.map((entryType, index) => entry(index + 1, { entry_type: entryType })))])
    .fetchImpl);
  await expect.poll(() => [...document.querySelectorAll("[data-memory-group] > h3")].map((name) => name.textContent))
    .toEqual(["写作偏好", "伏笔", "关系", "重大事件", "其他"]);
  expect(titles()).toEqual(["条目1", "条目2", "条目4", "条目5", "条目6"]);
});

it("shows one sentence for a Project with no entry", async () => {
  await settingsPage(fakeServer([listed([])]).fetchImpl);
  await expect.poll(sectionText).toEqual(["助手还没有记住任何内容。"]);
  expect(document.querySelectorAll("[data-memory-group]").length).toBe(0);
});

it("reads again after a failed read, and the Project Instruction section stays usable", async () => {
  const foreign = { owner_user_id: OWNER, project_id: "018f0000-0000-7001-8000-0000000d2004" };
  const server = fakeServer([new TypeError("Failed to fetch"), listed(ENTRIES.slice(0, 1), foreign),
    listed(ENTRIES.slice(0, 1))]);
  await settingsPage(server.fetchImpl);
  await expect.poll(sectionText).toEqual(["无法读取记忆，请稍后再试。", "重新读取"]);
  // The section "人物" shows the same failure of the one read (#1793).
  expect([...document.querySelectorAll("[data-character-section] p, [data-character-section] button")]
    .map((element) => element.textContent)).toEqual(["无法读取记忆，请稍后再试。", "重新读取"]);
  const field = document.querySelector<HTMLTextAreaElement>("[data-project-instruction]")!;
  expect([field.readOnly, document.querySelector<HTMLButtonElement>("[data-project-instruction-save]")?.disabled])
    .toEqual([false, false]);

  // A response of another Project Scope is a failed read too.
  document.querySelector<HTMLButtonElement>("[data-memory-read-again]")!.click();
  await expect.poll(() => server.paths.filter((path) => path === MEMORY).length).toBe(2);
  await expect.poll(sectionText).toEqual(["无法读取记忆，请稍后再试。", "重新读取"]);
  document.querySelector<HTMLButtonElement>("[data-memory-read-again]")!.click();
  await expect.poll(() => [...document.querySelectorAll("[data-memory-entry] h4")].map((title) => title.textContent))
    .toEqual(["语气"]);
  expect(server.paths.filter((path) => path === MEMORY).length).toBe(3);
});

// The section "人物" of the settings page (ADR 0101 section 6).
const characters = () => document.querySelector<HTMLElement>("[data-character-section]");
const names = () => [...document.querySelectorAll("[data-character-name]")].map((name) => name.textContent);
const chosenName = () => document.querySelector('[data-character-name][aria-current="true"]')?.textContent;
const fileEntries = () => [...document.querySelectorAll("[data-character-entry]")]
  .map((item) => item.getAttribute("data-character-entry"));
const searchFor = (text: string) => {
  const field = document.querySelector<HTMLInputElement>("[data-character-search]")!;
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(field, text);
  field.dispatchEvent(new Event("input", { bubbles: true }));
};
const pressInFile = (number: number, control: string) => document.querySelector<HTMLButtonElement>(
  `[data-character-entry="${number}"] [data-memory-${control}]`)!.click();
async function confirmDeleteInFile(number: number) {
  pressInFile(number, "delete");
  await expect.poll(() => document.querySelector(`[data-character-entry="${number}"] [data-memory-delete-confirm]`))
    .not.toBeNull();
  pressInFile(number, "delete-confirm");
}
/** Opens the section "人物" with `entries`. `later` has the answers of the reads after the first read. */
async function openCharacters(entries: MemoryEntryInspect[], deletes: (Answer | Promise<Answer>)[] = [],
  later: (Answer | Promise<Answer>)[] = []) {
  const server = fakeServer([listed(entries), ...later], deletes);
  await settingsPage(server.fetchImpl);
  document.querySelector<HTMLButtonElement>('[data-settings-item="characters"]')!.click();
  await expect.poll(() => characters()?.checkVisibility()).toBe(true);
  return server;
}

const character = (number: number, title: string, fields: Partial<MemoryEntryInspect> = {}) =>
  entry(number, { entry_type: "character", title, origin: "chapter",
    source: { kind: "chapter", chapter_id: CHAPTER, title: "渡口", availability: "available" }, ...fields });

it("lists each character one time, and the search 搜索人物 keeps the names that hold the text (ADR 0101 section 6)",
  async () => {
    await openCharacters([character(5, "林舟"), entry(1, {}), character(6, "阿青", { origin: "author_request" }),
      character(7, "林舟", { origin: "author_request" }), character(8, "青石")]);
    await expect.poll(names).toEqual(["林舟", "阿青", "青石"]);
    expect([chosenName(), document.querySelector("[data-character-search]")?.getAttribute("placeholder")])
      .toEqual(["林舟", "搜索人物"]);
    const shown: unknown[] = [];
    for (const text of ["青", " 林舟 ", "林青", ""]) {
      searchFor(text);
      await expect.poll(() => (document.querySelector<HTMLInputElement>("[data-character-search]")?.value)).toBe(text);
      shown.push([text, names(), chosenName() ?? null, document.querySelector("[data-character-file] h3")?.textContent
        ?? null, characters()!.querySelectorAll("p").length]);
    }
    expect(shown).toEqual([
      ["青", ["阿青", "青石"], "阿青", "阿青", 2],
      [" 林舟 ", ["林舟"], "林舟", "林舟", 4],
      ["林青", [], null, null, 0],
      ["", ["林舟", "阿青", "青石"], "林舟", "林舟", 4],
    ]);
    document.querySelector<HTMLButtonElement>('[data-character-name="青石"]')!.click();
    await expect.poll(chosenName).toBe("青石");
    expect([document.querySelector("[data-character-file] h3")?.textContent, fileEntries()]).toEqual(["青石", ["8"]]);
  });

it("shows the file 档案 with the entry of the author first, its flag, and only the delete controls (ADR 0101 section 6)",
  async () => {
    const flag = { reason: "第三章说他会说话。", flagged_at: "2026-11-17T09:00:00.000Z",
      source: { kind: "chapter" as const, chapter_id: CHAPTER, title: "渡口", availability: "available" as const } };
    await openCharacters([character(5, "林舟", { text: "他看灯。" }),
      character(7, "林舟", { origin: "author_request", text: "他怕水。", flag,
        source: { kind: "conversation", conversation_id: CONVERSATION, availability: "available" } })]);
    await expect.poll(fileEntries).toEqual(["7", "5"]);
    const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
    const file = document.querySelector<HTMLElement>("[data-character-file]")!;
    expect([...file.querySelectorAll("h3, h4")].map((heading) => heading.textContent)).toEqual(["林舟", "档案"]);
    expect([...file.querySelectorAll<HTMLElement>("[data-character-entry]")].map((item) => ({
      flag: [...item.querySelectorAll("[data-memory-entry-flag] p")].map((line) => line.textContent),
      text: item.querySelector(".memory-entry-text")?.textContent,
      origin: item.querySelector("[data-memory-entry-origin]")?.textContent,
      links: [...item.querySelectorAll("a")].map((link) => link.getAttribute("href")),
      time: item.querySelector("time")?.textContent,
    }))).toEqual([
      { flag: ["可能已过时 第三章说他会说话。", "来自章节《渡口》", "可以在对话中告诉助手哪个是对的，或删除这条记忆。"], text: "他怕水。",
        origin: "你让助手记住的", links: [`/projects/${PROJECT}?conversation=${CONVERSATION}`,
          `/projects/${PROJECT}?chapter=${CHAPTER}`], time: time.format(new Date("2026-11-17T08:30:00.000Z")) },
      { flag: [], text: "他看灯。", origin: "来自章节《渡口》", links: [`/projects/${PROJECT}?chapter=${CHAPTER}`],
        time: time.format(new Date("2026-11-15T08:30:00.000Z")) },
    ]);
    // The section has no control that edits an entry, that starts a pass, or that answers a question.
    expect([...characters()!.querySelectorAll("input, textarea, select, button")].map((control) =>
      control.getAttribute("data-character-search") === null ? control.textContent : "search"))
      .toEqual(["search", "林舟", "删除", "删除"]);
  });

it("keeps the file after the delete of its first part in the Memory Index, and then selects the next name",
  async () => {
    const [background, author] = [character(5, "林舟"), character(7, "林舟", { origin: "author_request" })];
    // The delete of the part of the author starts a new read.
    const server = await openCharacters([background, character(6, "阿青"), author, character(8, "青石")],
      [applied(background.memory_entry_id), applied(author.memory_entry_id)],
      [listed([character(6, "阿青"), character(8, "青石")])]);
    await expect.poll(fileEntries).toEqual(["7", "5"]);
    await confirmDeleteInFile(5);
    await expect.poll(fileEntries).toEqual(["7"]);
    // The name moves to the place of its other part in the Memory Index, and it stays selected.
    expect([names(), chosenName()]).toEqual([["阿青", "林舟", "青石"], "林舟"]);
    await confirmDeleteInFile(7);
    await expect.poll(names).toEqual(["阿青", "青石"]);
    expect([chosenName(), fileEntries(), server.paths.filter((path) => path === MEMORY).length])
      .toEqual(["青石", ["8"], 2]);
    expect(server.sent.filter((item) => item.method === "DELETE").map((item) => item.path))
      .toEqual([`${MEMORY}/${background.memory_entry_id}`, `${MEMORY}/${author.memory_entry_id}`]);
  });

it("selects the next name when two deletes in progress remove each part of a character", async () => {
  const [background, author] = [character(5, "林舟"), character(7, "林舟", { origin: "author_request" })];
  const [first, second] = [held(), held()];
  // The delete of the part of the author starts a new read, which can end before the other delete.
  await openCharacters([character(6, "阿青"), background, author, character(8, "青石")],
    [first.answer, second.answer], [listed([character(6, "阿青"), background, character(8, "青石")])]);
  document.querySelector<HTMLButtonElement>('[data-character-name="林舟"]')!.click();
  await expect.poll(fileEntries).toEqual(["7", "5"]);
  await confirmDeleteInFile(7);
  await confirmDeleteInFile(5);
  first.give(applied(author.memory_entry_id));
  second.give(applied(background.memory_entry_id));
  await expect.poll(names).toEqual(["阿青", "青石"]);
  expect([chosenName(), fileEntries()]).toEqual(["青石", ["8"]]);
});

it("selects the next name of the search result after the delete of the last part of a character", async () => {
  const stone = character(7, "青石");
  await openCharacters([character(5, "阿青"), stone, character(6, "林舟"), character(8, "小青")],
    [applied(stone.memory_entry_id)]);
  searchFor("青");
  await expect.poll(names).toEqual(["阿青", "青石", "小青"]);
  document.querySelector<HTMLButtonElement>('[data-character-name="青石"]')!.click();
  await expect.poll(fileEntries).toEqual(["7"]);
  await confirmDeleteInFile(7);
  await expect.poll(names).toEqual(["阿青", "小青"]);
  expect([chosenName(), fileEntries()]).toEqual(["小青", ["8"]]);
});

it("selects the next name of the current search when the search changes before the delete completes", async () => {
  const stone = character(7, "青石");
  const reply = held();
  await openCharacters([character(5, "阿青"), stone, character(6, "林舟"), character(8, "小青")], [reply.answer]);
  searchFor("青");
  await expect.poll(names).toEqual(["阿青", "青石", "小青"]);
  document.querySelector<HTMLButtonElement>('[data-character-name="青石"]')!.click();
  await expect.poll(fileEntries).toEqual(["7"]);
  await confirmDeleteInFile(7);
  searchFor("");
  await expect.poll(names).toEqual(["阿青", "青石", "林舟", "小青"]);
  reply.give(applied(stone.memory_entry_id));
  await expect.poll(names).toEqual(["阿青", "林舟", "小青"]);
  expect([chosenName(), fileEntries()]).toEqual(["林舟", ["6"]]);
});

it("keeps the delete control inside the file column with a long Chapter title at the narrow width", async () => {
  // 920 px is the `min-width` of the workspace.
  const [width, height] = [innerWidth, innerHeight];
  const styles = Object.assign(document.createElement("link"), { rel: "stylesheet", href: "/src/writing-workspace.css" });
  const loaded = new Promise((resolve, reject) => { styles.onload = resolve; styles.onerror = reject; });
  document.head.append(styles);
  try {
    await loaded;
    await page.viewport(920, 700);
    await openCharacters([character(5, "林舟", { source: { kind: "chapter", chapter_id: CHAPTER,
      title: "Chapter".repeat(40), availability: "available" } }), character(7, "林舟", { origin: "author_request" })]);
    await expect.poll(fileEntries).toEqual(["7", "5"]);
    const file = document.querySelector("[data-character-file]")!.getBoundingClientRect();
    const boxes = [...document.querySelectorAll("[data-character-entry] :is([data-memory-delete], .memory-entry-origin)")]
      .map((element) => element.getBoundingClientRect());
    expect([boxes.length, boxes.every((box) => box.left >= file.left && box.right <= file.right)]).toEqual([4, true]);
  } finally {
    styles.remove();
    await page.viewport(width, height);
  }
});

it("shows the sentence of an empty Memory and no list for a Project with no character", async () => {
  await openCharacters([entry(1, {})]);
  await expect.poll(() => [...characters()!.querySelectorAll("p, input, li")].map((element) => element.textContent))
    .toEqual(["助手还没有记住任何内容。"]);
});

it("shows no entry and no sentence in the section 记忆 when each entry is a character", async () => {
  await openCharacters([character(5, "林舟")]);
  await expect.poll(names).toEqual(["林舟"]);
  expect([...section()!.querySelectorAll("p, [data-memory-group]")].length).toBe(0);
});

// The intent result of a character on the settings page (ADR 0101 sections 5 and 6).
const INTENT = "他想在涨水前把船修好，再去问母亲那封信的事。";
const QUESTIONS = ["母亲的信在谁手里？", "他知道阿青要走吗？"];
const result = (fields: Partial<NonNullable<MemoryEntryInspect["intent"]>> = {}) => ({ text: INTENT,
  questions: QUESTIONS, recorded_at: "2026-11-20T08:30:00.000Z",
  chapter: { chapter_id: CHAPTER, title: "渡口", availability: "available" as const },
  ...fields });
const blockLines = (selector: string) => [...document.querySelectorAll(`[data-character-file] ${selector} :is(h4, p, li)`)]
  .map((line) => line.textContent);

it("shows 接下来想做什么 with the intent and its Chapter, and 助手还不清楚的事 with each question, with no control",
  async () => {
    // The sketch of ADR 0101 section 6 gives the measures of the block.
    const styles = Object.assign(document.createElement("link"), { rel: "stylesheet", href: "/src/writing-workspace.css" });
    const loaded = new Promise((resolve, reject) => { styles.onload = resolve; styles.onerror = reject; });
    document.head.append(styles);
    try {
      await loaded;
      await openCharacters([entry(1, {}), character(5, "林舟", { intent: result() }),
        character(7, "林舟", { origin: "author_request" })]);
      await expect.poll(() => blockLines("[data-character-intent]"))
        .toEqual(["接下来想做什么", INTENT, "写到《渡口》时的想法"]);
      const file = document.querySelector<HTMLElement>("[data-character-file]")!;
      expect([[...file.querySelectorAll("h3, h4")].map((heading) => heading.textContent), blockLines("[data-character-questions]"),
        [...file.children].map((child) => child.getAttribute("data-character-entry") ?? child.querySelector("h4")?.textContent)]).toEqual([
        ["林舟", "档案", "接下来想做什么", "助手还不清楚的事"], ["助手还不清楚的事", ...QUESTIONS],
        ["7", "5", "接下来想做什么", "助手还不清楚的事"]]);
      const block = getComputedStyle(file.querySelector("[data-character-intent]")!);
      const line = (selector: string) => getComputedStyle(file.querySelector(`[data-character-intent] ${selector}`)!);
      expect([block.marginTop, block.padding, block.borderRadius !== "0px", line("h4").fontSize, line("h4").fontWeight,
        line(".character-intent-text").fontSize, line(".character-intent-text").lineHeight,
        line(".character-intent-chapter").fontSize]).toEqual(["16px", "10px 12px", true, "13px", "500", "13px",
        "22.1px", "12px"]);
      expect([...characters()!.querySelectorAll("input, textarea, select, button")].map((control) =>
        control.getAttribute("data-character-search") === null ? control.textContent : "search"))
        .toEqual(["search", "林舟", "删除", "删除"]);
      // The section 记忆 shows no intent and no question.
      expect([INTENT, ...QUESTIONS].filter((text) => section()!.textContent!.includes(text))).toEqual([]);
    } finally {
      styles.remove();
    }
  });

it("shows no question block with no question, no Chapter line with no available Chapter, and no block with no result",
  async () => {
    await openCharacters([
      character(5, "林舟", { intent: result({ questions: [], chapter: { chapter_id: CHAPTER, title: null,
        availability: "unavailable" } }) }),
      character(6, "阿青"),
      character(8, "青石", { intent: result({ chapter: null }) }),
    ]);
    const shown: unknown[] = [];
    for (const name of ["林舟", "阿青", "青石"]) {
      document.querySelector<HTMLButtonElement>(`[data-character-name="${name}"]`)!.click();
      await expect.poll(() => document.querySelector("[data-character-file] h3")?.textContent).toBe(name);
      shown.push([name, blockLines("[data-character-intent]"), blockLines("[data-character-questions]")]);
    }
    expect(shown).toEqual([
      ["林舟", ["接下来想做什么", INTENT], []],
      ["阿青", [], []],
      ["青石", ["接下来想做什么", INTENT], ["助手还不清楚的事", ...QUESTIONS]],
    ]);
  });

it("shows no block after the author deleted the part of the background work of a character", async () => {
  const background = character(5, "林舟", { intent: result() });
  await openCharacters([background, character(7, "林舟", { origin: "author_request" })],
    [applied(background.memory_entry_id)]);
  await expect.poll(() => blockLines("[data-character-intent]").length).toBe(3);
  await confirmDeleteInFile(5);
  await expect.poll(fileEntries).toEqual(["7"]);
  expect([blockLines("[data-character-intent]"), blockLines("[data-character-questions]")]).toEqual([[], []]);
});

it("shows no block after the author deleted the part of the author that the result names", async () => {
  const background = character(5, "林舟", { intent: result() });
  const author = character(7, "林舟", { origin: "author_request" });
  // The read after the delete gives no result, because the result named the deleted part.
  const reread = held();
  const server = fakeServer([listed([background, author]), reread.answer], [applied(author.memory_entry_id)]);
  await settingsPage(server.fetchImpl);
  document.querySelector<HTMLButtonElement>('[data-settings-item="characters"]')!.click();
  await expect.poll(() => blockLines("[data-character-intent]").length).toBe(3);
  await confirmDeleteInFile(7);
  await expect.poll(fileEntries).toEqual(["5"]);
  // Until the read after the delete ends, the page shows no result for the character.
  await expect.poll(() => server.paths.filter((path) => path === MEMORY).length).toBe(2);
  expect([blockLines("[data-character-intent]"), blockLines("[data-character-questions]")]).toEqual([[], []]);
  reread.give(listed([character(5, "林舟")]));
  await expect.poll(() => server.paths.filter((path) => path === MEMORY).length).toBe(2);
  expect([fileEntries(), blockLines("[data-character-intent]"), blockLines("[data-character-questions]")])
    .toEqual([["5"], [], []]);
});

it("reads again after the delete of a part of the author, and shows a result that is current again", async () => {
  // The result names no part of the author. The author made a part after the pass, so the result is not current until
  // the author deletes that part (#1791 item 1).
  const background = character(5, "林舟");
  const author = character(7, "林舟", { origin: "author_request" });
  const server = fakeServer([listed([background, author]), listed([{ ...background, intent: result() }])],
    [applied(author.memory_entry_id)]);
  await settingsPage(server.fetchImpl);
  document.querySelector<HTMLButtonElement>('[data-settings-item="characters"]')!.click();
  await expect.poll(fileEntries).toEqual(["7", "5"]);
  expect(blockLines("[data-character-intent]")).toEqual([]);
  await confirmDeleteInFile(7);
  await expect.poll(() => blockLines("[data-character-intent]"))
    .toEqual(["接下来想做什么", INTENT, "写到《渡口》时的想法"]);
  expect([server.paths.filter((path) => path === MEMORY).length, fileEntries(),
    blockLines("[data-character-questions]")]).toEqual([2, ["5"], ["助手还不清楚的事", ...QUESTIONS]]);
});

it("does not show a read that started before the delete of the part of the author, and shows the read after it",
  async () => {
    const background = character(5, "林舟", { intent: result() });
    const author = character(7, "林舟", { origin: "author_request" });
    // After the delete, a later intent pass gives a new result that names no part of the author.
    const later = { ...background, version: "3", intent: result({ text: "他想先去找阿青。", questions: [] }) };
    const [reread, after] = [held(), held()];
    const server = fakeServer([listed([background, author]), reread.answer, after.answer],
      [deleted({ kind: "conflicted", reason: "entry_changed" }), applied(author.memory_entry_id)]);
    await settingsPage(server.fetchImpl);
    document.querySelector<HTMLButtonElement>('[data-settings-item="characters"]')!.click();
    await expect.poll(() => blockLines("[data-character-intent]").length).toBe(3);
    // The changed part of the background work opens a read, and the author deletes the part of the author.
    await confirmDeleteInFile(5);
    await expect.poll(() => server.paths.filter((path) => path === MEMORY).length).toBe(2);
    await confirmDeleteInFile(7);
    await expect.poll(() => server.paths.filter((path) => path === MEMORY).length).toBe(3);
    expect([fileEntries(), blockLines("[data-character-intent]")]).toEqual([["5"], []]);
    // The read that started before the delete gives the ended result, and the page does not show it.
    reread.give(listed([background, author]));
    await reread.answer;
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect([fileEntries(), blockLines("[data-character-intent]")]).toEqual([["5"], []]);
    after.give(listed([later]));
    await expect.poll(() => blockLines("[data-character-intent]")).toEqual(["接下来想做什么", "他想先去找阿青。",
      "写到《渡口》时的想法"]);
    expect([server.paths.filter((path) => path === MEMORY).length, fileEntries(),
      blockLines("[data-character-questions]")]).toEqual([3, ["5"], []]);
  });
