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
const TOKEN_A = "AlphaSearch";
const TOKEN_B = "BetaSearch";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist manuscript-search page did not load"));
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

async function search(
  frame: HTMLIFrameElement,
  query: string,
  selection: "current_chapter" | "manuscript",
): Promise<Element> {
  const root = appRoot(frame);
  const input = root.querySelector<HTMLInputElement>('input[name="manuscript-search-query"]');
  const radio = root.querySelector<HTMLInputElement>(
    `input[name="manuscript-search-selection"][value="${selection}"]`,
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
      && node.getAttribute("data-search-selection") === selection
      ? node.getAttribute("data-search-outcome")
      : undefined;
  }, { timeout: 10_000 }).toBe("ready");
  const outcome = [...root.querySelectorAll("[data-search-outcome='ready']")]
    .find((node) => node.getAttribute("data-search-query") === query
      && node.getAttribute("data-search-selection") === selection);
  if (outcome === undefined) throw new Error("the search outcome is missing");
  return outcome;
}

function matchIdentities(outcome: Element): string[] {
  return [...outcome.querySelectorAll("[data-search-match]")].map((node) => [
    node.getAttribute("data-chapter-id"),
    node.getAttribute("data-block-id"),
    node.getAttribute("data-range-start"),
    node.getAttribute("data-range-end"),
  ].join(":"));
}

it("searches the current Chapter and manuscript with bounded Snapshot identity", { timeout: 90_000 }, async () => {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist manuscript search";
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
  title.value = "Search Novel";
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
  await typeIntoCurrent(frame, TOKEN_A);
  const chapterAId = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  if (chapterAId === null || chapterAId === undefined
    || chapterBId === null || chapterBId === undefined) {
    throw new Error("the Chapter identity is missing");
  }
  root.querySelector<HTMLButtonElement>(`[data-make-current-chapter="${chapterBId}"]`)?.click();
  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const editor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return nextRoot.querySelector("h2")?.textContent === "Chapter B"
      && editor !== null
      && manuscriptIsEditable(editor);
  }, { timeout: 15_000 }).toBe(true);
  await typeIntoCurrent(frame, TOKEN_B);

  const currentMiss = await search(frame, TOKEN_A, "current_chapter");
  expect(currentMiss.getAttribute("data-search-count")).toBe("0");
  expect(currentMiss.getAttribute("data-search-completeness")).toBe("complete");
  expect(currentMiss.getAttribute("data-search-lag")).toBe("0");
  expect(currentMiss.getAttribute("data-search-snapshot-id")).toMatch(UUID);
  expect(currentMiss.getAttribute("data-search-watermark")).toMatch(/^[1-9][0-9]*$/);

  const currentHit = await search(frame, TOKEN_B, "current_chapter");
  expect(currentHit.getAttribute("data-search-count")).toBe("1");
  const currentMatch = currentHit.querySelector("[data-search-match]");
  expect(currentMatch?.getAttribute("data-chapter-id")).toBe(chapterBId);
  expect(currentMatch?.getAttribute("data-block-id")).toMatch(UUID);
  expect(currentMatch?.getAttribute("data-range-start")).toBe("0");
  expect(currentMatch?.getAttribute("data-range-end")).toBe(String(TOKEN_B.length));

  const manuscriptHit = await search(frame, TOKEN_A, "manuscript");
  expect(manuscriptHit.getAttribute("data-search-count")).toBe("1");
  expect(manuscriptHit.querySelector("[data-search-match]")?.getAttribute("data-chapter-id"))
    .toBe(chapterAId);
  expect(matchIdentities(manuscriptHit)).toHaveLength(1);

  const none = await search(frame, "zzz-no-match", "manuscript");
  expect(none.getAttribute("data-search-count")).toBe("0");
  expect(none.getAttribute("data-search-completeness")).toBe("complete");

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

  const afterDelete = await search(frame, TOKEN_B, "manuscript");
  expect(afterDelete.getAttribute("data-search-count")).toBe("0");
  const stillA = await search(frame, TOKEN_A, "manuscript");
  expect(stillA.getAttribute("data-search-count")).toBe("1");
  expect(stillA.querySelector("[data-search-match]")?.getAttribute("data-chapter-id"))
    .toBe(chapterAId);
});
