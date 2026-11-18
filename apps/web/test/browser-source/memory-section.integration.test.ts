import { createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it } from "vitest";

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
  entry(2, { entry_type: "character", title: "林舟", text: MARKUP, origin: "conversation",
    source: { kind: "conversation", conversation_id: CONVERSATION, availability: "unavailable" } }),
  entry(3, { entry_type: "character", title: "阿青", origin: "chapter",
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
  expect(headings).toEqual(["项目指令", "记忆"]);
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
    ["人物", [
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

it("shows the name of each of the six types in the order of the query", async () => {
  const types = ["writing_preference", "foreshadowing", "character", "relationship", "major_event", "other"] as const;
  await settingsPage(fakeServer([listed(types.map((entryType, index) => entry(index + 1, { entry_type: entryType })))])
    .fetchImpl);
  await expect.poll(() => [...document.querySelectorAll("[data-memory-group] > h3")].map((name) => name.textContent))
    .toEqual(["写作偏好", "伏笔", "人物", "关系", "重大事件", "其他"]);
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
