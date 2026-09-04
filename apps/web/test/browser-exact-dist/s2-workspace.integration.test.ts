import { afterEach, expect, it } from "vitest";

import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist workspace page did not load"));
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

function workspace(frame: HTMLIFrameElement): HTMLElement {
  const childWindow = applicationWindow(frame);
  const node = frame.contentDocument?.querySelector("[data-writing-workspace]");
  if (!(node instanceof childWindow.HTMLElement)) {
    throw new Error("the writing workspace is missing");
  }
  return node;
}

it("the production page uses the approved workspace without losing writing state", async () => {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist writing workspace";
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
  title.value = "Workspace Novel";
  form.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");

  const empty = appRoot(frame);
  const emptyWorkspace = workspace(frame);
  expect(empty.querySelector('nav[aria-label="稿件目录"]')).not.toBeNull();
  expect(empty.querySelector("textarea")).toBeNull();
  expect(emptyWorkspace.querySelector("[data-writing-assistant]")?.getAttribute(
    "data-assistant-availability",
  )).toBe("unavailable");
  expect(empty.textContent).not.toContain("模型");
  expect(empty.textContent).not.toContain("Agent");
  expect(empty.textContent).not.toContain("Provider");
  expect(empty.textContent).not.toContain("Receipt");
  expect(empty.textContent).not.toContain("权威修订");

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
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }).toBe(true);

  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const nextShell = workspace(frame);
    const nextWindow = applicationWindow(frame);
    const nextAssistant = nextShell.querySelector("[data-writing-assistant]");
    const nextToggle = nextShell.querySelector("[data-assistant-toggle]");
    const nextChapter = nextRoot.querySelector(
      'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
    );
    return nextAssistant instanceof nextWindow.HTMLElement
      && nextToggle instanceof nextWindow.HTMLButtonElement
      && nextChapter !== null
      && nextChapter.textContent === "Chapter A";
  }, { timeout: 10_000 }).toBe(true);

  const root = appRoot(frame);
  const shell = workspace(frame);
  const childWindow = applicationWindow(frame);
  const editor = manuscriptEditor(root, childWindow);
  const assistant = shell.querySelector("[data-writing-assistant]");
  const toggle = shell.querySelector("[data-assistant-toggle]");
  const currentChapter = root.querySelector(
    'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
  );
  if (!(assistant instanceof childWindow.HTMLElement)
    || !(toggle instanceof childWindow.HTMLButtonElement)
    || currentChapter === null) {
    throw new Error("the production writing columns are incomplete");
  }
  expect({
    tree: root.querySelector('nav[aria-label="稿件目录"]') !== null,
    createChapter: root.querySelector("form[data-create-chapter]") !== null,
    renameVolume: root.querySelector("form[data-rename-volume]") !== null,
    renameChapter: root.querySelector("form[data-rename-chapter]") !== null,
    expandVolume: root.querySelector("[data-volume-expand]") !== null,
    chapter: currentChapter.textContent,
    heading: root.querySelector("h2")?.textContent ?? null,
    writerKind: shell.getAttribute("data-writer-kind"),
    writerGeneration: shell.getAttribute("data-writer-generation"),
    availability: assistant.getAttribute("data-assistant-availability"),
  }).toEqual({
    tree: true,
    createChapter: true,
    renameVolume: true,
    renameChapter: true,
    expandVolume: true,
    chapter: "Chapter A",
    heading: "Chapter A",
    writerKind: "current_writer",
    writerGeneration: "1",
    availability: "unavailable",
  });
  expect(Number.parseInt(childWindow.getComputedStyle(currentChapter).fontWeight, 10))
    .toBeGreaterThanOrEqual(600);
  expect(root.textContent).not.toContain("模型");
  expect(root.textContent).not.toContain("Agent");
  expect(root.textContent).not.toContain("Provider");
  expect(root.textContent).not.toContain("Receipt");
  expect(root.textContent).not.toContain("权威修订");
  expect(root.textContent).not.toMatch(/\{[^{]*"code"/);

  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: "Quiet prose" });
  await expect.poll(() => manuscriptBody(editor)).toBe("Quiet prose");

  expect(toggle.getAttribute("aria-expanded")).toBe("true");
  toggle.click();
  await expect.poll(() => toggle.getAttribute("aria-expanded")).toBe("false");
  expect(Math.round(assistant.getBoundingClientRect().width)).toBe(44);
  expect(assistant.innerText.trim()).toBe("");
  expect({
    body: manuscriptBody(editor),
    chapter: root.querySelector(
      'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
    )?.textContent ?? null,
    writerKind: shell.getAttribute("data-writer-kind"),
    writerGeneration: shell.getAttribute("data-writer-generation"),
    readOnly: !manuscriptIsEditable(editor),
  }).toEqual({
    body: "Quiet prose",
    chapter: "Chapter A",
    writerKind: "current_writer",
    writerGeneration: "1",
    readOnly: false,
  });

  toggle.click();
  await expect.poll(() => toggle.getAttribute("aria-expanded")).toBe("true");
  const composer = shell.querySelector<HTMLFormElement>("form[data-writing-assistant-composer]");
  if (composer === null) throw new Error("the writing-assistant composer is missing");
  composer.requestSubmit();
  await expect.poll(() => assistant.getAttribute("data-assistant-dispatch")).toBe("refused");
  expect(manuscriptBody(editor)).toBe("Quiet prose");
  expect(shell.getAttribute("data-writer-generation")).toBe("1");

  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: " remains" });
  await expect.poll(() => manuscriptBody(editor)).toBe("Quiet prose remains");
});
