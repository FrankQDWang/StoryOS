import { afterEach, expect, it } from "vitest";

import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const RESTORED_TITLE = "WAL after base backup";
const SETTLED_BODY = "Authoritative A 中文 EN";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist physical-drill page did not load"));
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

function chapterTitles(root: Element | null | undefined): string[] {
  return [...(root?.querySelectorAll('nav[aria-label="稿件目录"] [data-chapter-title]') ?? [])]
    .map((node) => node.textContent?.trim() ?? "");
}

function assertIsolation(root: Element): void {
  expect(root.textContent).not.toContain("Project B secret");
  expect(root.textContent).not.toContain("Chapter B secret");
  expect(root.textContent).not.toContain("Authoritative B secret");
  expect(root.textContent).not.toContain("Stale Chapter A");
  expect(root.textContent).not.toContain("Stale A");
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

it("writes Chinese and English after restore, settles, reloads, and keeps isolation", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist restored Project";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  await expect.poll(() =>
    [...(frame.contentDocument?.querySelectorAll('#app button[data-open="current_chapter"]') ?? [])]
      .some((button) => button.textContent === RESTORED_TITLE),
  ).toBe(true);
  const library = appRoot(frame);
  expect(library.getAttribute("data-boot-state")).toBe("protected-ready");
  assertIsolation(library);
  const openButton = [...library.querySelectorAll<HTMLButtonElement>(
    'button[data-open="current_chapter"]',
  )].find((button) => button.textContent === RESTORED_TITLE);
  openButton?.click();
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const editor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return root?.getAttribute("data-boot-state") === "project-ready"
      && editor !== null
      && editor !== undefined
      && manuscriptIsEditable(editor);
  }, { timeout: 10_000 }).toBe(true);
  await expect.poll(() => chapterTitles(frame.contentDocument?.querySelector("#app")))
    .toEqual(["Chapter A"]);

  const root = appRoot(frame);
  assertIsolation(root);
  const childWindow = applicationWindow(frame);
  const editor = manuscriptEditor(root, childWindow);
  expect(manuscriptBody(editor)).toBe("Authoritative A");
  const beforeRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: " 中文 EN" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(SETTLED_BODY);
  await waitSaved(root, beforeRevisionId);

  const reopened = nextFrameLoad(frame);
  frame.src = `/projects/${PROJECT_A}`;
  await reopened;
  await expect.poll(() => {
    const nextRoot = frame.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const reopenedRoot = appRoot(frame);
  await expect.poll(() => chapterTitles(reopenedRoot)).toEqual(["Chapter A"]);
  assertIsolation(reopenedRoot);
  expect(manuscriptBody(manuscriptEditor(reopenedRoot, applicationWindow(frame))))
    .toBe(SETTLED_BODY);
  await waitSaved(reopenedRoot);
});
