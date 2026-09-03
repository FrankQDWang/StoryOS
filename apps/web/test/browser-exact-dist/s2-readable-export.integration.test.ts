import { afterEach, expect, it } from "vitest";

import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

const CHAPTER_A = "Hello world";
const CHAPTER_B = "雨落在窗沿。";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist readable-export page did not load"));
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

async function waitSaved(root: Element): Promise<void> {
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
    { timeout: 10_000 },
  ).toBe("saved");
}

async function typeIntoCurrent(frame: HTMLIFrameElement, text: string): Promise<void> {
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const editor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return editor !== null && editor !== undefined && manuscriptIsEditable(editor);
  }, { timeout: 10_000 }).toBe(true);
  const root = appRoot(frame);
  const realm = applicationWindow(frame);
  realm.focus();
  const editor = manuscriptEditor(root, realm);
  editor.click();
  editor.focus();
  focusManuscriptEnd(editor, realm);
  await applyTrustedInput({ operation: "insert_text", text });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(text);
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
    { timeout: 10_000 },
  ).toBe("saving");
  await waitSaved(root);
}

async function requestExport(root: Element): Promise<void> {
  const previousId =
    root.querySelector("[data-export-id]")?.getAttribute("data-export-id") ?? "";
  const button = [...root.querySelectorAll<HTMLButtonElement>(
    "[data-readable-export] button",
  )].find((candidate) => candidate.textContent === "导出可读稿件");
  if (button === undefined) throw new Error("the readable export request button is missing");
  button.click();
  await expect.poll(() => {
    const panel = root.querySelector("[data-readable-export]");
    const exportId = root.querySelector("[data-export-id]")?.getAttribute("data-export-id") ?? "";
    if (
      panel?.getAttribute("data-export-outcome") !== "in_progress"
      || exportId.length === 0
      || exportId === previousId
    ) {
      return "";
    }
    return exportId;
  }, { timeout: 10_000 }).not.toBe("");
  expect(root.querySelector("[data-readable-export-bytes]")).toBeNull();
}

it("admits a durable human-readable export and does not treat the first inspect as ready", { timeout: 120_000 }, async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist readable export";
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
  title.value = "Readable Export Novel";
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
  const chapterTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-chapter] input[name="chapter-title"]',
  );
  const chapterForm = chapterTitle?.form;
  if (chapterTitle === null || chapterTitle === undefined
    || chapterForm === null || chapterForm === undefined) {
    throw new Error("the Create Chapter form is missing");
  }
  chapterTitle.value = "Chapter A";
  chapterForm.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");
  await typeIntoCurrent(frame, CHAPTER_A);
  const root = appRoot(frame);
  await requestExport(root);
  expect(root.querySelector("[data-readable-export]")?.getAttribute("data-export-outcome"))
    .toBe("in_progress");
  await requestExport(root);
  expect(root.querySelector("[data-readable-export-bytes]")).toBeNull();
  const secondChapter = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-chapter] input[name="chapter-title"]',
  );
  const secondForm = secondChapter?.form;
  if (secondChapter === null || secondChapter === undefined
    || secondForm === null || secondForm === undefined) {
    throw new Error("the second Create Chapter form is missing");
  }
  secondChapter.value = "Chapter B";
  secondForm.requestSubmit();
  await expect.poll(() =>
    [...appRoot(frame).querySelectorAll('nav[aria-label="稿件目录"] button[data-chapter-id]')]
      .some((button) => button.textContent === "Chapter B")
  ).toBe(true);
  const chapterBId = [...appRoot(frame).querySelectorAll<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )].find((button) => button.textContent === "Chapter B")?.getAttribute("data-chapter-id");
  if (chapterBId === null || chapterBId === undefined) throw new Error("Chapter B is missing");
  await expect.poll(() =>
    appRoot(frame).querySelector(`[data-make-current-chapter="${chapterBId}"]`) !== null
  ).toBe(true);
  appRoot(frame).querySelector<HTMLButtonElement>(
    `[data-make-current-chapter="${chapterBId}"]`,
  )?.click();
  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const editor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return nextRoot.querySelector("h2")?.textContent === "Chapter B"
      && editor !== null
      && manuscriptIsEditable(editor);
  }, { timeout: 15_000 }).toBe(true);
  await typeIntoCurrent(frame, CHAPTER_B);
  const afterRebuild = appRoot(frame);
  await requestExport(afterRebuild);
  expect(afterRebuild.querySelector("[data-readable-export]")?.getAttribute("data-export-outcome"))
    .toBe("in_progress");
  expect(afterRebuild.querySelector("[data-readable-export-bytes]")).toBeNull();
  expect(
    [...afterRebuild.querySelectorAll<HTMLButtonElement>("[data-readable-export] button")]
      .some((candidate) => candidate.textContent === "下载"),
  ).toBe(false);
});
