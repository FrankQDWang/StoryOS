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

/** A fake Server whose answer to each read of the Memory Entries is the next item of `reads`. */
function fakeServer(reads: Answer[]) {
  const paths: string[] = [];
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    paths.push(path);
    if (path === `/api/v1/projects/${PROJECT}/instruction`) {
      return jsonResponse({ schema_id: "storyos.query.project-instruction.response.v1",
        correlation_id: "018f0000-0000-7001-8000-0000000d2011", project_scope: SCOPE, instruction: null });
    }
    if (path === MEMORY) {
      const answer = reads.shift();
      if (answer === undefined) throw new Error("no read answer");
      if (answer instanceof Error) throw answer;
      return answer;
    }
    return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
  };
  return { paths, fetchImpl };
}

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
      { title: "阿青", text: "内容3。", origin: "来自章节《渡口》", originKind: "chapter", links: [], time: at(3) },
    ]],
    ["其他", [{ title: "旧事", text: "内容1。", origin: "来源已不可用", originKind: "unavailable", links: [],
      time: at(1) }]],
  ]);
  // The text is data: the markup is text, and the address in it is not a link.
  expect(section()!.querySelector("b")).toBeNull();
  // Each entry has no control, and its two places for later controls are empty.
  for (const item of document.querySelectorAll("[data-memory-entry]")) {
    expect(item.querySelectorAll("input, textarea, select, button").length).toBe(0);
    expect([...item.querySelectorAll("[data-memory-entry-flag], [data-memory-entry-actions]")]
      .map((place) => place.childNodes.length)).toEqual([0, 0]);
  }
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
