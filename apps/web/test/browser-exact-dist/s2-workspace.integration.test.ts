import { afterEach, expect, it } from "vitest";

import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";
import {
  appRoot,
  applicationWindow,
  closeApplicationFrame,
  openApplicationFrame,
  openFirstChapterForm,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
} from "../support/exact-dist-frame.ts";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

function workspace(frame: HTMLIFrameElement): HTMLElement {
  const childWindow = applicationWindow(frame);
  const node = frame.contentDocument?.querySelector("[data-writing-workspace]");
  if (!(node instanceof childWindow.HTMLElement)) {
    throw new Error("the writing workspace is missing");
  }
  return node;
}

it("the production page uses the approved workspace without losing writing state", async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist writing workspace");
  await submitProjectForm(frame, "Workspace Novel");

  const empty = appRoot(frame);
  const emptyWorkspace = workspace(frame);
  expect(empty.querySelector('nav[aria-label="稿件目录"]')).not.toBeNull();
  expect(empty.querySelector(".editor-panel textarea")).toBeNull();
  expect(emptyWorkspace.querySelector("[data-writing-assistant]")?.getAttribute(
    "data-assistant-availability",
  )).toBe("unavailable");
  expect(empty.textContent).not.toContain("模型");
  expect(empty.textContent).not.toContain("Agent");
  expect(empty.textContent).not.toContain("Provider");
  expect(empty.textContent).not.toContain("Receipt");
  expect(empty.textContent).not.toContain("权威修订");

  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  await submitChapterForm(frame, "Chapter A");
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
    createChapter: root.querySelector("[data-add-chapter]") !== null,
    renameVolume: root.querySelector("[data-create-chapter-menu]") !== null,
    renameChapter: root.querySelector("[data-chapter-menu]") !== null,
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
  expect(composer.querySelector<HTMLButtonElement>("button[type=submit]")?.disabled).toBe(true);
  composer.requestSubmit();
  expect(assistant.getAttribute("data-assistant-dispatch")).toBe("idle");
  expect(manuscriptBody(editor)).toBe("Quiet prose");
  expect(shell.getAttribute("data-writer-generation")).toBe("1");

  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: " remains" });
  await expect.poll(() => manuscriptBody(editor)).toBe("Quiet prose remains");
});
