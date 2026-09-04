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
const TOKEN = "AlphaToken";
const ONE = "BetaToken";
const SOURCE = `${TOKEN} ${TOKEN} ${TOKEN}`;
const AFTER_ONE = `${ONE} ${TOKEN} ${TOKEN}`;

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist manuscript-replace page did not load"));
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

async function search(
  frame: HTMLIFrameElement,
  query: string,
  count: string,
): Promise<Element> {
  const root = appRoot(frame);
  const input = root.querySelector<HTMLInputElement>('input[name="manuscript-search-query"]');
  const radio = root.querySelector<HTMLInputElement>(
    'input[name="manuscript-search-selection"][value="current_chapter"]',
  );
  const form = root.querySelector<HTMLFormElement>("form[data-manuscript-search-form]");
  if (input === null || radio === null || form === null) {
    throw new Error("the manuscript search form is missing");
  }
  radio.click();
  input.value = query;
  form.requestSubmit();
  await expect.poll(() => {
    const node = root.querySelector("[data-search-outcome='ready']");
    return node?.getAttribute("data-search-query") === query
      && node.getAttribute("data-search-selection") === "current_chapter"
      ? node.getAttribute("data-search-count")
      : undefined;
  }, { timeout: 10_000 }).toBe(count);
  const outcome = [...root.querySelectorAll("[data-search-outcome='ready']")]
    .find((node) => node.getAttribute("data-search-query") === query
      && node.getAttribute("data-search-selection") === "current_chapter");
  if (outcome === undefined) throw new Error("the search outcome is missing");
  return outcome;
}

async function readBody(
  frame: HTMLIFrameElement,
  projectId: string,
  chapterId: string,
): Promise<string> {
  const childWindow = applicationWindow(frame);
  const chapter = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  return chapter.chapter.current_revision.body;
}

it("replaces one visible match and refuses a broader replace without authority", {
  timeout: 90_000,
}, async () => {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist manuscript replace";
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
  title.value = "Replace Novel";
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

  const root = appRoot(frame);
  await typeIntoCurrent(frame, SOURCE);
  const projectId = root.querySelector("form[data-rename]")?.getAttribute("data-rename");
  const chapterId = root.querySelector<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterId === null || chapterId === undefined
    || !UUID.test(projectId)
    || !UUID.test(chapterId)) {
    throw new Error("the Project or Chapter identity is missing");
  }

  const outcome = await search(frame, TOKEN, "3");
  expect(outcome.querySelector("[data-replace-one]")).toBeInstanceOf(
    applicationWindow(frame).HTMLButtonElement,
  );
  expect(outcome.querySelector("[data-replace-all]")).toBeInstanceOf(
    applicationWindow(frame).HTMLButtonElement,
  );
  const replacement = outcome.querySelector<HTMLInputElement>(
    'input[name="manuscript-search-replacement"]',
  );
  const replaceOne = outcome.querySelector<HTMLButtonElement>("[data-replace-one]");
  if (replacement === null || replaceOne === null) {
    throw new Error("the one-match replace control is missing");
  }
  const beforeOne = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  replacement.value = ONE;
  replaceOne.click();
  await expect.poll(() =>
    root.querySelector("[data-replace-outcome]")?.getAttribute("data-replace-outcome"),
    { timeout: 10_000 },
  ).toBe("applied");
  await waitSaved(root, beforeOne);
  const editor = manuscriptEditor(root, applicationWindow(frame));
  expect(manuscriptIsEditable(editor)).toBe(true);
  expect(manuscriptBody(editor)).toBe(AFTER_ONE);
  expect(await readBody(frame, projectId, chapterId)).toBe(AFTER_ONE);

  const remaining = await search(frame, TOKEN, "2");
  const remainingReplacement = remaining.querySelector<HTMLInputElement>(
    'input[name="manuscript-search-replacement"]',
  );
  const replaceAll = remaining.querySelector<HTMLButtonElement>("[data-replace-all]");
  if (remainingReplacement === null || replaceAll === null) {
    throw new Error("the broader replace control is missing");
  }
  remainingReplacement.value = "GammaToken";
  replaceAll.click();
  await expect.poll(() =>
    root.querySelector("[data-replace-outcome]")?.getAttribute("data-replace-outcome"),
    { timeout: 10_000 },
  ).toBe("refused");
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
    { timeout: 10_000 },
  ).toBe("needs_attention");
  expect(manuscriptBody(editor)).toBe(AFTER_ONE);
  expect(await readBody(frame, projectId, chapterId)).toBe(AFTER_ONE);
});
