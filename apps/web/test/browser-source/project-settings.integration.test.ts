import { createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it } from "vitest";

import type { GetProjectResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { ProjectSettingsView } from "../../src/project-settings.tsx";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { OWNER, jsonResponse } from "./scenario.ts";

// The settings page of a Project and its Project Instruction section (ADR 0094 sections 2 and 9).
const PROJECT = "018f0000-0000-7001-8000-0000000a2001";
const FOREIGN = "018f0000-0000-7001-8000-0000000a2002";
const SCOPE = { owner_user_id: OWNER, project_id: PROJECT };
const INSTRUCTION = `/api/v1/projects/${PROJECT}/instruction`;
const MEMORY = `/api/v1/projects/${PROJECT}/memory-entries`;
const PROJECT_VIEW: GetProjectResponse = {
  schema_id: "storyos.query.project.response.v1",
  correlation_id: "018f0000-0000-7001-8000-0000000a2010",
  project_scope: SCOPE,
  project: { project_id: PROJECT, title: "Settings Novel", open: { kind: "empty" } },
};

type Sent = { method: string; path: string; key: string | null; body: unknown };
/** The answer of the fake Server to each `PUT .../instruction`, in order. */
type SaveAnswer = Response | Error | Promise<Response>;

/** A fake Server with one Project of the session whose current Revision 1 has the text `Old rule.`. */
function fakeServer(saves: SaveAnswer[] = []) {
  const sent: Sent[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    sent.push({ method, path, key: new Headers(init?.headers).get("idempotency-key"),
      body: typeof init?.body === "string" ? JSON.parse(init.body) : undefined });
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-0000000a2011", owner_user_id: OWNER,
        projects: [{ project_scope: SCOPE, title: "Settings Novel", revision: "1", open: { kind: "empty" } }] });
    }
    if (path === `/api/v1/projects/${PROJECT}`) return jsonResponse(PROJECT_VIEW);
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse({ project_scope: SCOPE, tree_revision: "1", volumes: [],
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-0000000a2012", project_scope: SCOPE } });
    }
    if (path === INSTRUCTION && method === "GET") {
      return jsonResponse({ schema_id: "storyos.query.project-instruction.response.v1",
        correlation_id: "018f0000-0000-7001-8000-0000000a2013", project_scope: SCOPE,
        instruction: { instruction_revision_id: "018f0000-0000-7001-8000-0000000a2014", revision: "1",
          text: "Old rule." } });
    }
    if (path === MEMORY) {
      return jsonResponse({ schema_id: "storyos.query.memory-entries.response.v1",
        correlation_id: "018f0000-0000-7001-8000-0000000a2017", project_scope: SCOPE, entries: [] });
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-11-16T12:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path === INSTRUCTION && method === "PUT") {
      const answer = saves.shift();
      if (answer === undefined) throw new Error("no save answer");
      if (answer instanceof Error) throw answer;
      return answer;
    }
    return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
  };
  return { sent, fetchImpl };
}

function saved(effect: Record<string, unknown>): Response {
  return jsonResponse({ schema_id: "storyos.command.update-project-instruction.response.v1",
    correlation_id: "018f0000-0000-7001-8000-0000000a2015", project_scope: SCOPE, effect });
}

const applied = (revision: string) => saved({ kind: "authoritative_applied", revision,
  instruction_revision_id: "018f0000-0000-7001-8000-0000000a2016", project_activity_position: "7" });

const roots: Root[] = [];

async function open(pathname: string, fetchImpl: typeof fetch) {
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document, fetchImpl, cryptoImpl: crypto,
    locationImpl: { origin: location.origin, pathname } });
  roots.push(mountStage1View(loaded.root, loaded));
  return loaded;
}

/** Renders the settings page with a navigation that the test records. */
async function settingsPage(fetchImpl: typeof fetch) {
  document.body.innerHTML = '<main id="app"></main>';
  const navigated: string[] = [];
  const root = createRoot(document.querySelector("#app")!);
  roots.push(root);
  root.render(createElement(ProjectSettingsView, {
    baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, navigate: (href) => navigated.push(href),
    state: { kind: "project-settings-ready", profile: RELEASE_1_PROTOCOL_PROFILE, project: PROJECT_VIEW },
  }));
  await expect.poll(() => field()?.value).toBe("Old rule.");
  return navigated;
}

const field = () => document.querySelector<HTMLTextAreaElement>("[data-project-instruction]");
const outcome = () => document.querySelector("[data-project-instruction-outcome]")?.textContent;
const confirmStep = () => document.querySelector("[data-settings-leave-confirm]")?.textContent;
const click = (selector: string) => document.querySelector<HTMLElement>(selector)!.click();

function type(text: string) {
  const textarea = field()!;
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(textarea, text);
  textarea.dispatchEvent(new Event("input", { bubbles: true }));
}

/** Whether the page sets the leave prompt of the browser for a reload or a close of the tab. */
function leavePrompt(): boolean {
  const event = new Event("beforeunload", { cancelable: true });
  globalThis.dispatchEvent(event);
  return event.defaultPrevented;
}

const puts = (sent: Sent[]) => sent.filter((request) => request.method === "PUT");

/** The request body of a save of `text` on `expected`, with the correlation identity that the page chose. */
function saveBody(sent: Sent | undefined, text: string, expected: string) {
  const { correlation_id: correlationId } = (sent?.body as { update_project_instruction_input: {
    correlation_id: string } }).update_project_instruction_input;
  return { command_schema: "storyos.command.update-project-instruction.request.v1",
    update_project_instruction_input: { text, expected_instruction_revision: expected,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: correlationId } };
}

afterEach(() => {
  for (const root of roots.splice(0)) root.unmount();
  document.body.replaceChildren();
});

it("opens the settings page of a Project of the session and no other Project or path", async () => {
  const server = fakeServer();
  const loaded = await open(`/projects/${PROJECT}/settings/`, server.fetchImpl);
  expect(loaded.state.kind).toBe("project-settings-ready");
  await expect.poll(() => field()?.value).toBe("Old rule.");
  expect([...loaded.root.querySelectorAll("header a, header p, h1, h2, .settings-hint")]
    .map((element) => [element.textContent, element.getAttribute("href")])).toEqual([
    ["返回写作", `/projects/${PROJECT}`],
    ["Settings Novel", null],
    ["项目设置", null],
    ["项目指令", null],
    ["保存后从下一次请求开始生效，进行中的请求不受影响。", null],
    ["记忆", null],
  ]);
  // The page opens no Editor Session.
  expect(server.sent.map(({ method, path }) => `${method} ${path}`)).toEqual([
    "GET /api/v1/protocol", `GET /api/v1/projects/${PROJECT}`, "GET /api/v1/projects", `GET ${INSTRUCTION}`, `GET ${MEMORY}`]);

  const foreign = fakeServer();
  const blocked = await open(`/projects/${FOREIGN}/settings`, foreign.fetchImpl);
  expect([blocked.state.kind, blocked.state.kind === "project-blocked" && blocked.state.code])
    .toEqual(["project-blocked", "project_unavailable"]);
  expect(foreign.sent.map(({ path }) => path)).toEqual(["/api/v1/protocol", `/api/v1/projects/${FOREIGN}`]);

  for (const pathname of ["/projects/not-a-uuid/settings", `/projects/${PROJECT}/settings/extra`,
    `/projects/${PROJECT}/SETTINGS`]) {
    const invalid = fakeServer();
    const state = (await open(pathname, invalid.fetchImpl)).state;
    expect([state.kind, state.kind === "project-blocked" && state.code, invalid.sent])
      .toEqual(["project-blocked", "project_url_invalid", []]);
  }
});

it("shows the gear control in the tree footer of a workspace with no Chapter", async () => {
  const loaded = await open(`/projects/${PROJECT}`, fakeServer().fetchImpl);
  expect(loaded.state.kind).toBe("empty-project-ready");
  await expect.poll(() => loaded.root.querySelector(".tree-panel .tree-footer a[aria-label=\"项目设置\"]")
    ?.getAttribute("href")).toBe(`/projects/${PROJECT}/settings`);
});

const SENTENCE = {
  applied: "已保存。新的指令从下一次请求开始生效。",
  unchanged: "没有改动。",
  conflicted: "项目指令已在别处修改。请刷新后再保存。",
  overLimit: "项目指令太长，没有保存。请缩短后再保存。",
  unknown: "无法确认是否已保存，请再试一次。",
};
const OVER_LIMIT = "字".repeat(32_001);
const rows: Array<[string, string, SaveAnswer, string]> = [
  ["authoritative_applied", "New rule.", applied("2"), SENTENCE.applied],
  ["no_effect", "Old rule.", saved({ kind: "no_effect", reason: "text_unchanged" }), SENTENCE.unchanged],
  ["conflicted", "Stale rule.", saved({ kind: "conflicted", reason: "stale_instruction_revision" }),
    SENTENCE.conflicted],
  ["instruction_over_item_limit", OVER_LIMIT, jsonResponse({ schema_id: "storyos.problem.v1",
    code: "instruction_over_item_limit", message: "The text is above the item limit." }, 422), SENTENCE.overLimit],
  ["a lost response", "Lost rule.", new TypeError("Failed to fetch"), SENTENCE.unknown],
];

it.each(rows)("shows the sentence of %s and sends the text with the revision that it read", async (_, text, answer,
  sentence) => {
  const server = fakeServer([answer]);
  await settingsPage(server.fetchImpl);
  type(text);
  click("[data-project-instruction-save]");
  await expect.poll(outcome).toBe(sentence);
  const [request] = puts(server.sent);
  expect([request?.body, field()?.value]).toEqual([saveBody(request, text, "1"), text]);
});

it("keeps the new revision after a save and sends the same idempotency key after a lost response", async () => {
  let answerFirst: (response: Response) => void = () => {};
  const server = fakeServer([new Promise<Response>((resolve) => { answerFirst = resolve; }),
    new TypeError("Failed to fetch"), applied("3")]);
  const navigated = await settingsPage(server.fetchImpl);
  type("");
  click("[data-project-instruction-save]");
  await expect.poll(() => puts(server.sent).length).toBe(1);
  expect([document.querySelector<HTMLButtonElement>("[data-project-instruction-save]")?.disabled,
    field()?.readOnly]).toEqual([true, true]);
  answerFirst(applied("2"));
  await expect.poll(outcome).toBe(SENTENCE.applied);
  // The empty text is a correct save, and the page has no unsaved change after it.
  expect([field()?.value, leavePrompt()]).toEqual(["", false]);

  type("Second rule.");
  click("[data-project-instruction-save]");
  await expect.poll(outcome).toBe(SENTENCE.unknown);
  click("[data-project-instruction-save]");
  await expect.poll(outcome).toBe(SENTENCE.applied);
  const [first, lost, retry] = puts(server.sent);
  expect([first?.body, lost?.body, retry?.body, retry?.key === lost?.key, first?.key === lost?.key, navigated])
    .toEqual([saveBody(first, "", "1"), saveBody(lost, "Second rule.", "2"), lost?.body, true, false, []]);
});

it("asks one time before the author leaves with a text that is not saved", async () => {
  const server = fakeServer();
  const navigated = await settingsPage(server.fetchImpl);
  expect(leavePrompt()).toBe(false);
  click("[data-return-to-writing]");
  expect([navigated, confirmStep()]).toEqual([[`/projects/${PROJECT}`], undefined]);

  type("Unsaved rule.");
  await expect.poll(leavePrompt).toBe(true);
  click("[data-return-to-writing]");
  await expect.poll(confirmStep).toBe("项目指令有未保存的修改。确定离开吗？离开留下");
  click("[data-settings-stay]");
  await expect.poll(confirmStep).toBe(undefined);
  expect([field()?.value, navigated.length, leavePrompt()]).toEqual(["Unsaved rule.", 1, true]);

  click("[data-return-to-writing]");
  await expect.poll(confirmStep).toBeDefined();
  click("[data-settings-leave]");
  expect([navigated, puts(server.sent), leavePrompt()])
    .toEqual([[`/projects/${PROJECT}`, `/projects/${PROJECT}`], [], false]);
});
