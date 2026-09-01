import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const BODY_A = "Hello 中文";
const BODY_B = "Beta prose";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist Stage 2 journey page did not load"));
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
  if (root === null || root === undefined) {
    throw new Error("the production page root is missing");
  }
  return root;
}

function chapterButton(root: Element, title: string): HTMLButtonElement | undefined {
  return [...root.querySelectorAll<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )].find((button) => button.textContent === title);
}

async function waitSaved(root: Element, previousRevisionId?: string): Promise<void> {
  await expect.poll(() => {
    const node = root.querySelector("[data-save-state]");
    const revision = node?.getAttribute("data-authoritative-revision-id") ?? "";
    const failure = node?.getAttribute("data-editor-failure") ?? "";
    const save = node?.getAttribute("data-save-state");
    const unsettled = node?.getAttribute("data-unsettled-intent-count");
    if (save === "saved"
      && unsettled === "0"
      && failure === ""
      && (previousRevisionId === undefined || revision !== previousRevisionId)) {
      return { ok: true as const, failure: "" };
    }
    return { ok: false as const, save, unsettled, failure, revision };
  }, { timeout: 10_000 }).toEqual({ ok: true, failure: "" });
}

async function typeExpected(
  frame: HTMLIFrameElement,
  inserted: string,
  expected: string,
): Promise<void> {
  const root = appRoot(frame);
  const editor = manuscriptEditor(root, applicationWindow(frame));
  const before = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  editor.focus();
  focusManuscriptEnd(editor, applicationWindow(frame));
  await applyTrustedInput({ operation: "insert_text", text: inserted });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(expected);
  await waitSaved(root, before);
}

async function makeCurrent(
  frame: HTMLIFrameElement,
  chapterId: string,
  heading: string,
): Promise<void> {
  appRoot(frame).querySelector<HTMLButtonElement>(
    `[data-make-current-chapter="${chapterId}"]`,
  )?.click();
  await expect.poll(() => {
    const root = appRoot(frame);
    const editor = root.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return root.querySelector("h2")?.textContent === heading
      && editor !== null
      && editor !== undefined
      && manuscriptIsEditable(editor);
  }, { timeout: 15_000 }).toBe(true);
}

async function reloadProject(frame: HTMLIFrameElement, projectId: string): Promise<void> {
  const reopened = nextFrameLoad(frame);
  frame.src = `/projects/${projectId}`;
  await reopened;
  await expect.poll(() => {
    const nextRoot = frame.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
}

it("runs the AI-disabled production journey without losing Chapter work", {
  timeout: 90_000,
}, async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Stage 2 journey";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = frame.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Stage 2 Journey";
  form.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  const volumeTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-volume] input[name="volume-title"]',
  );
  const volumeForm = volumeTitle?.form;
  if (volumeTitle === null || volumeTitle === undefined
    || volumeForm === null || volumeForm === undefined) {
    throw new Error("the Create Volume form is missing");
  }
  volumeTitle.value = "Volume A";
  volumeForm.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app form[data-create-chapter]') !== null
  ).toBe(true);
  for (const name of ["Chapter A", "Chapter B"] as const) {
    const chapterTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
      '#app form[data-create-chapter] input[name="chapter-title"]',
    );
    const chapterForm = chapterTitle?.form;
    if (chapterTitle === null || chapterTitle === undefined
      || chapterForm === null || chapterForm === undefined) {
      throw new Error("the Create Chapter form is missing");
    }
    chapterTitle.value = name;
    chapterForm.requestSubmit();
    await expect.poll(() => {
      const root = frame.contentDocument?.querySelector("#app");
      const titles = [...(root?.querySelectorAll(
        'nav[aria-label="稿件目录"] button[data-chapter-id]',
      ) ?? [])].map((button) => button.textContent);
      return root?.getAttribute("data-boot-state") === "project-ready"
        && titles[titles.length - 1] === name;
    }).toBe(true);
  }

  const root = appRoot(frame);
  expect(root.querySelector('nav[aria-label="稿件目录"]')).not.toBeNull();
  expect(root.querySelector(MANUSCRIPT_EDITOR_SELECTOR)).not.toBeNull();
  expect(root.querySelector("[data-writing-assistant]")).not.toBeNull();
  expect(root.querySelector("[data-writing-assistant]")
    ?.getAttribute("data-assistant-availability")).toBe("unavailable");
  expect(root.textContent).not.toContain("模型");
  expect(root.textContent).not.toContain("Agent");
  const projectId = root.querySelector("form[data-rename]")?.getAttribute("data-rename");
  const chapterAId = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterAId === null || chapterAId === undefined
    || chapterBId === null || chapterBId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }

  await typeExpected(frame, BODY_A, BODY_A);
  await makeCurrent(frame, chapterBId, "Chapter B");
  await typeExpected(frame, BODY_B, BODY_B);
  await reloadProject(frame, projectId);
  expect(manuscriptBody(manuscriptEditor(appRoot(frame), applicationWindow(frame)))).toBe(BODY_B);

  const searchInput = appRoot(frame).querySelector<HTMLInputElement>(
    'input[name="manuscript-search-query"]',
  );
  const searchRadio = appRoot(frame).querySelector<HTMLInputElement>(
    'input[name="manuscript-search-selection"][value="manuscript"]',
  );
  const searchForm = appRoot(frame).querySelector<HTMLFormElement>("form[data-manuscript-search-form]");
  if (searchInput === null || searchRadio === null || searchForm === null) {
    throw new Error("the manuscript search form is missing");
  }
  searchRadio.click();
  searchInput.value = "Hello";
  searchForm.requestSubmit();
  await expect.poll(() => {
    const node = appRoot(frame).querySelector("[data-search-outcome='ready']");
    return node?.getAttribute("data-search-query") === "Hello"
      ? node.querySelectorAll("[data-search-match]").length
      : 0;
  }, { timeout: 10_000 }).toBeGreaterThan(0);
  await expect.poll(() =>
    appRoot(frame).querySelector("[data-statistics-outcome='ready']")?.getAttribute("data-statistics-lag")
  ).toBe("0");
  expect(appRoot(frame).querySelector("[data-statistics-outcome='ready']")
    ?.getAttribute("data-statistics-snapshot-id")).toMatch(UUID);

  const exportButton = [...appRoot(frame).querySelectorAll<HTMLButtonElement>(
    "[data-readable-export] button",
  )].find((candidate) => candidate.textContent === "导出可读稿件");
  if (exportButton === undefined) throw new Error("the readable export request button is missing");
  exportButton.click();
  await expect.poll(() => {
    const bytes = appRoot(frame).querySelector("[data-readable-export-bytes]")?.textContent ?? "";
    return bytes.includes(BODY_A) && bytes.includes(BODY_B);
  }, { timeout: 10_000 }).toBe(true);

  await reloadProject(frame, projectId);
  const reopenedRoot = appRoot(frame);
  expect(reopenedRoot.textContent).not.toContain("模型");
  expect(reopenedRoot.textContent).not.toContain("Agent");
  expect(manuscriptBody(manuscriptEditor(reopenedRoot, applicationWindow(frame)))).toBe(BODY_B);
  const childWindow = applicationWindow(frame);
  const [authoritativeA, authoritativeB] = await Promise.all([
    getChapter({
      baseUrl: childWindow.location.origin,
      projectId,
      chapterId: chapterAId,
      fetchImpl: childWindow.fetch.bind(childWindow),
    }),
    getChapter({
      baseUrl: childWindow.location.origin,
      projectId,
      chapterId: chapterBId,
      fetchImpl: childWindow.fetch.bind(childWindow),
    }),
  ]);
  expect(authoritativeA.chapter.current_revision.body).toBe(BODY_A);
  expect(authoritativeB.chapter.current_revision.body).toBe(BODY_B);
});
