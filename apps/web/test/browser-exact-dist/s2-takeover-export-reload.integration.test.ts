import { beginInlineChapterCreation, beginInlineVolumeCreation, beginTreeAction } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  applyTrustedInput,
  resetCommandChallengeRateWindows,
  settleWorkerOnce,
  updateClientSessionCookie,
} from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

// Three Chapters of 10,000 Unicode scalars give the 30,000-scalar baseline size.
const CHAPTER_SCALARS = 10_000;
const PHRASES = ["夜雨落在旧书房的窗沿，", "守夜人在渡口找到一枚旧铜钱，",
  "风从门缝里吹了进来。", "“我们该走了？”小舟低声问道。"];

let applicationFrame: HTMLIFrameElement | undefined;

function chapterText(seed: number): string {
  const scalars: string[] = [];
  for (let index = seed; scalars.length < CHAPTER_SCALARS; index++) {
    scalars.push(...PHRASES[index % PHRASES.length]!);
  }
  return scalars.slice(0, CHAPTER_SCALARS).join("");
}

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist takeover export page did not load"));
    }, 10_000);
    frame.addEventListener("load", () => {
      window.clearTimeout(timeout);
      resolve();
    }, { once: true });
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
  document.body.replaceChildren();
});

function applicationWindow(frame: HTMLIFrameElement): Window & typeof globalThis {
  const result = frame.contentWindow;
  if (result === null) throw new Error("the production page realm is unavailable");
  return result as Window & typeof globalThis;
}

function appRoot(frame: HTMLIFrameElement): Element {
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) throw new Error("the production page root is missing");
  return root;
}

function chapterIds(root: Element): Record<string, string> {
  return Object.fromEntries([...root.querySelectorAll<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )].map((button) => [button.textContent ?? "", button.getAttribute("data-chapter-id") ?? ""]));
}

async function submitForm(frame: HTMLIFrameElement, selector: string, value: string): Promise<void> {
  const input = frame.contentDocument?.querySelector<HTMLInputElement>(selector);
  const form = input?.form;
  if (input === null || input === undefined || form === null || form === undefined) {
    throw new Error(`the form of ${selector} is missing`);
  }
  input.value = value;
  form.requestSubmit();
}

async function createChapter(frame: HTMLIFrameElement, title: string): Promise<void> {
  await beginInlineChapterCreation(frame.contentDocument);
  await submitForm(frame, '#app form[data-create-chapter] input[name="chapter-title"]', title);
  await expect.poll(() => chapterIds(appRoot(frame))[title] ?? "").not.toBe("");
}

// The poll returns the failure facts, so a stuck editor shows its state in the diff.
async function waitWritable(frame: HTMLIFrameElement, previousRevisionId?: string): Promise<void> {
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const node = root?.querySelector("[data-save-state]");
    const editor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    const revision = node?.getAttribute("data-authoritative-revision-id") ?? "";
    const state = {
      save: node?.getAttribute("data-save-state"),
      unsettled: node?.getAttribute("data-unsettled-intent-count"),
      failure: node?.getAttribute("data-editor-failure") ?? "",
      editable: editor !== null && editor !== undefined && manuscriptIsEditable(editor),
    };
    return previousRevisionId !== undefined && revision === previousRevisionId
      ? { ...state, save: "unchanged revision" } : state;
  }, { timeout: 30_000 }).toEqual({ save: "saved", unsettled: "0", failure: "", editable: true });
}

async function typeIntoCurrent(frame: HTMLIFrameElement, text: string, expected: string): Promise<void> {
  await waitWritable(frame);
  const root = appRoot(frame);
  const realm = applicationWindow(frame);
  const before = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  const editor = manuscriptEditor(root, realm);
  realm.focus();
  editor.focus();
  focusManuscriptEnd(editor, realm);
  await applyTrustedInput({ operation: "insert_text", text });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(expected);
  await waitWritable(frame, before);
}

async function makeCurrent(frame: HTMLIFrameElement, chapterId: string, heading: string): Promise<void> {
  await beginTreeAction(appRoot(frame), `li[data-chapter-id="${chapterId}"]`, "[data-make-current-chapter]");
  appRoot(frame).querySelector<HTMLButtonElement>(`[data-make-current-chapter="${chapterId}"]`)?.click();
  await expect.poll(() => appRoot(frame).querySelector("h2")?.textContent).toBe(heading);
  await waitWritable(frame);
}

async function reloadProject(frame: HTMLIFrameElement, projectId: string): Promise<void> {
  const loaded = nextFrameLoad(frame);
  applicationWindow(frame).location.href = `/projects/${projectId}`;
  await loaded;
  await expect.poll(() => appRoot(frame).getAttribute("data-boot-state"), { timeout: 15_000 })
    .toBe("project-ready");
}

async function requestExport(frame: HTMLIFrameElement): Promise<string> {
  const root = appRoot(frame);
  const button = [...root.querySelectorAll<HTMLButtonElement>("[data-readable-export] button")]
    .find((candidate) => candidate.textContent === "导出可读稿件");
  if (button === undefined) throw new Error("the readable export request button is missing");
  button.click();
  await expect.poll(() =>
    root.querySelector("[data-readable-export]")?.getAttribute("data-export-outcome"),
  ).toBe("in_progress");
  await settleWorkerOnce();
  await expect.poll(() =>
    root.querySelector("[data-readable-export]")?.getAttribute("data-export-outcome"),
  { timeout: 15_000 }).toBe("ready");
  return root.querySelector("[data-readable-export-bytes]")?.textContent ?? "";
}

it("keeps a taken-over writer editable after structure changes, a readable export, and a reload", {
  timeout: 300_000,
}, async () => {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist takeover export reload";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName,
  ).toBe("INPUT");
  await submitForm(frame, '#app input[name="title"]', "Takeover export reload");
  await expect.poll(() => appRoot(frame).getAttribute("data-boot-state")).toBe("empty-project-ready");
  await beginInlineVolumeCreation(frame.contentDocument);
  await submitForm(frame, '#app form[data-create-volume] input[name="volume-title"]', "Volume A");
  await beginInlineChapterCreation(frame.contentDocument);
  for (const title of ["Chapter A", "Chapter B", "Chapter C"]) await createChapter(frame, title);
  await expect.poll(() => appRoot(frame).getAttribute("data-boot-state")).toBe("project-ready");
  const ids = chapterIds(appRoot(frame));
  const projectId = appRoot(frame).querySelector("[data-project-id]")?.getAttribute("data-project-id");
  if (projectId === null || projectId === undefined) throw new Error("the Project identity is missing");
  const texts = { "Chapter A": chapterText(0), "Chapter B": chapterText(1), "Chapter C": chapterText(2) };
  await typeIntoCurrent(frame, texts["Chapter A"], texts["Chapter A"]);
  await makeCurrent(frame, ids["Chapter B"]!, "Chapter B");
  await typeIntoCurrent(frame, texts["Chapter B"], texts["Chapter B"]);
  await makeCurrent(frame, ids["Chapter C"]!, "Chapter C");
  await typeIntoCurrent(frame, texts["Chapter C"], texts["Chapter C"]);
  expect(Object.values(texts).reduce((total, text) => total + [...text].length, 0)).toBe(30_000);
  await resetCommandChallengeRateWindows();

  // A new Editor Session is a secondary session. Its public takeover makes writer generation 2.
  const realm = applicationWindow(frame);
  for (const key of Object.keys(realm.sessionStorage)) {
    if (key.startsWith("active_session:") && key.endsWith(`:${projectId}`)) realm.sessionStorage.removeItem(key);
  }
  await reloadProject(frame, projectId);
  await expect.poll(() => appRoot(frame).querySelector("[data-take-over-writer]")?.tagName).toBe("BUTTON");
  appRoot(frame).querySelector<HTMLButtonElement>("[data-take-over-writer]")?.click();
  await expect.poll(() => appRoot(frame).querySelector("[data-take-over-writer]")).toBeNull();
  await waitWritable(frame);

  await createChapter(frame, "Chapter D");
  const chapterDId = chapterIds(appRoot(frame))["Chapter D"]!;
  await beginTreeAction(appRoot(frame), `li[data-chapter-id="${ids["Chapter C"]}"]`, '[data-chapter-move="up"]');
  appRoot(frame).querySelector<HTMLButtonElement>(
    `li[data-chapter-id="${ids["Chapter C"]}"] button[data-chapter-move="up"]`,
  )?.click();
  await expect.poll(() => Object.keys(chapterIds(appRoot(frame))))
    .toEqual(["Chapter A", "Chapter C", "Chapter B", "Chapter D"]);
  await beginTreeAction(appRoot(frame), `li[data-chapter-id="${chapterDId}"]`, "button[data-delete-chapter]");
  appRoot(frame).querySelector<HTMLButtonElement>(`li[data-chapter-id="${chapterDId}"] button[data-delete-chapter]`)
    ?.click();
  await expect.poll(() => appRoot(frame).querySelector("button[data-confirm-delete-chapter]")?.tagName)
    .toBe("BUTTON");
  appRoot(frame).querySelector<HTMLButtonElement>("button[data-confirm-delete-chapter]")?.click();
  await expect.poll(() => Object.keys(chapterIds(appRoot(frame))))
    .toEqual(["Chapter A", "Chapter C", "Chapter B"]);
  await waitWritable(frame);
  await reloadProject(frame, projectId);
  await waitWritable(frame);

  expect(await requestExport(frame)).toBe(["# Volume A", "", "## Chapter A", "", texts["Chapter A"], "",
    "## Chapter C", "", texts["Chapter C"], "", "## Chapter B", "", texts["Chapter B"], ""].join("\n"));
  await reloadProject(frame, projectId);
  await waitWritable(frame);
  expect(appRoot(frame).querySelector("h2")?.textContent).toBe("Chapter C");
  await typeIntoCurrent(frame, "+", `${texts["Chapter C"]}+`);
  const saved = await getChapter({ baseUrl: realm.location.origin, projectId,
    chapterId: ids["Chapter C"]!, fetchImpl: applicationWindow(frame).fetch.bind(applicationWindow(frame)) });
  expect(saved.chapter.current_revision.body).toBe(`${texts["Chapter C"]}+`);
});
