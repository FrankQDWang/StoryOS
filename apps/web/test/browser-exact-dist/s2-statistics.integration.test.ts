import { afterEach, expect, it } from "vitest";

import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const CHAPTER_A = "Hello world";
const CHAPTER_B = "雨落在窗沿。";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist manuscript-statistics page did not load"));
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

async function waitSaved(root: Element): Promise<void> {
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
    { timeout: 10_000 },
  ).toBe("saved");
}

async function typeIntoCurrent(frame: HTMLIFrameElement, text: string): Promise<void> {
  const root = appRoot(frame);
  const editor = manuscriptEditor(root, applicationWindow(frame));
  editor.focus();
  focusManuscriptEnd(editor, applicationWindow(frame));
  await applyTrustedInput({ operation: "insert_text", text });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(text);
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
    { timeout: 10_000 },
  ).toBe("saving");
  await waitSaved(root);
}

async function waitStatistics(
  frame: HTMLIFrameElement,
  expected: {
    chapterWords: string;
    chapterCharacters: string;
    manuscriptWords: string;
    manuscriptCharacters: string;
    chapterCount: string;
  },
): Promise<Element> {
  const root = appRoot(frame);
  await expect.poll(() => {
    const node = root.querySelector("[data-statistics-outcome='ready']");
    if (node === null) return undefined;
    return [
      node.getAttribute("data-statistics-chapter-words"),
      node.getAttribute("data-statistics-chapter-characters"),
      node.getAttribute("data-statistics-manuscript-words"),
      node.getAttribute("data-statistics-manuscript-characters"),
      node.getAttribute("data-statistics-chapter-count"),
    ].join(":");
  }, { timeout: 10_000 }).toBe([
    expected.chapterWords,
    expected.chapterCharacters,
    expected.manuscriptWords,
    expected.manuscriptCharacters,
    expected.chapterCount,
  ].join(":"));
  const outcome = root.querySelector("[data-statistics-outcome='ready']");
  if (outcome === null) throw new Error("the statistics outcome is missing");
  return outcome;
}

it("rebuilds Chapter and manuscript statistics after edit, switch, and deletion", { timeout: 120_000 }, async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist manuscript statistics";
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
  title.value = "Statistics Novel";
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
  const chapterAId = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  if (chapterAId === null || chapterAId === undefined
    || chapterBId === null || chapterBId === undefined) {
    throw new Error("the Chapter identity is missing");
  }
  root.querySelector<HTMLButtonElement>(`[data-make-current-chapter="${chapterAId}"]`)?.click();
  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const editor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return nextRoot.querySelector("h2")?.textContent === "Chapter A"
      && editor !== null
      && manuscriptIsEditable(editor);
  }, { timeout: 15_000 }).toBe(true);
  await typeIntoCurrent(frame, CHAPTER_A);
  const afterA = await waitStatistics(frame, {
    chapterWords: "2",
    chapterCharacters: "11",
    manuscriptWords: "2",
    manuscriptCharacters: "11",
    chapterCount: "2",
  });
  expect(afterA.getAttribute("data-statistics-lag")).toBe("0");
  expect(afterA.getAttribute("data-statistics-snapshot-id")).toMatch(UUID);
  expect(afterA.getAttribute("data-statistics-watermark")).toMatch(/^[1-9][0-9]*$/);
  expect(afterA.getAttribute("data-statistics-counting-profile"))
    .toBe("storyos.statistics.unicode-16.0.0.v1");

  root.querySelector<HTMLButtonElement>(`[data-make-current-chapter="${chapterBId}"]`)?.click();
  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const editor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return nextRoot.querySelector("h2")?.textContent === "Chapter B"
      && editor !== null
      && manuscriptIsEditable(editor);
  }, { timeout: 15_000 }).toBe(true);
  await typeIntoCurrent(frame, CHAPTER_B);
  await waitStatistics(frame, {
    chapterWords: "1",
    chapterCharacters: "6",
    manuscriptWords: "3",
    manuscriptCharacters: "17",
    chapterCount: "2",
  });

  const row = [...(frame.contentDocument?.querySelectorAll("li[data-chapter-id]") ?? [])]
    .find((item) => item.getAttribute("data-chapter-id") === chapterBId);
  row?.querySelector<HTMLButtonElement>("button[data-delete-chapter]")?.click();
  await expect.poll(() =>
    row?.querySelector("button[data-confirm-delete-chapter]")?.tagName
  ).toBe("BUTTON");
  row?.querySelector<HTMLButtonElement>("button[data-confirm-delete-chapter]")?.click();
  await expect.poll(() => {
    const next = appRoot(frame);
    return next.querySelector(`[data-chapter-id="${chapterBId}"]`) === null
      && next.querySelector("h2")?.textContent === "Chapter A";
  }, { timeout: 15_000 }).toBe(true);

  await waitStatistics(frame, {
    chapterWords: "2",
    chapterCharacters: "11",
    manuscriptWords: "2",
    manuscriptCharacters: "11",
    chapterCount: "1",
  });
});
