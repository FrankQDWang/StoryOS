import { beginInlineChapterCreation } from "../support/inline-chapter-creation.ts";
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
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
} from "../support/exact-dist-frame.ts";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

function chapterTitles(root: Element | null | undefined): string[] {
  return [...(root?.querySelectorAll('nav[aria-label="稿件目录"] [data-chapter-title]') ?? [])]
    .map((node) => node.textContent?.trim() ?? "");
}

async function waitSaveState(frame: HTMLIFrameElement): Promise<void> {
  await expect.poll(() => {
    const node = frame.contentDocument?.querySelector("[data-save-state]");
    return node?.getAttribute("data-save-state") === "saved"
      && node.getAttribute("data-unsettled-intent-count") === "0";
  }, { timeout: 10_000 }).toBe(true);
}

async function typeThroughSaving(frame: HTMLIFrameElement, text: string): Promise<void> {
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const editor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return editor !== null && editor !== undefined && manuscriptIsEditable(editor);
  }, { timeout: 10_000 }).toBe(true);
  const root = appRoot(frame);
  const realm = applicationWindow(frame);
  const editor = manuscriptEditor(root, realm);
  editor.focus();
  focusManuscriptEnd(editor, realm);
  await applyTrustedInput({ operation: "insert_text", text });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(text);
  await expect.poll(() => {
    return frame.contentDocument
      ?.querySelector("[data-save-state]")
      ?.getAttribute("data-save-state") === "saving";
  }, { timeout: 10_000 }).toBe(true);
  await waitSaveState(frame);
}

function currentChapterRow(frame: HTMLIFrameElement): Element | undefined {
  const heading = frame.contentDocument?.querySelector("h2")?.textContent;
  return [...(frame.contentDocument?.querySelectorAll("li[data-chapter-id]") ?? [])]
    .find((item) => item.querySelector("[data-chapter-title]")?.textContent?.trim() === heading);
}

async function confirmCurrentDelete(frame: HTMLIFrameElement): Promise<void> {
  const row = currentChapterRow(frame);
  row?.querySelector<HTMLButtonElement>("[data-create-chapter-menu], [data-chapter-menu]")?.click();
  await expect.poll(() => row?.querySelector("button[data-delete-volume], button[data-delete-chapter]")?.tagName).toBe("BUTTON");
  const start = row?.querySelector<HTMLButtonElement>("button[data-delete-chapter]");
  if (start === undefined || start === null) {
    throw new Error("the Delete Chapter control is missing");
  }
  start.click();
  await expect.poll(() =>
    currentChapterRow(frame)?.querySelector("button[data-confirm-delete-chapter]")?.tagName
  ).toBe("BUTTON");
  const confirm = currentChapterRow(frame)
    ?.querySelector<HTMLButtonElement>("button[data-confirm-delete-chapter]");
  if (confirm === undefined || confirm === null) {
    throw new Error("the Delete Chapter confirmation is missing");
  }
  confirm.click();
}

async function createThreeChapters(frame: HTMLIFrameElement): Promise<void> {
  await submitProjectForm(frame, "Delete Novel");

  await submitVolumeForm(frame, "Volume A");
  await beginInlineChapterCreation(frame.contentDocument);
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app form[data-create-chapter] input[name="chapter-title"]')
      ?.tagName
  ).toBe("INPUT");

  await submitChapterForm(frame, "Chapter A");
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");

  for (const [name, expected] of [
    ["Chapter B", "Chapter A\nChapter B"],
    ["Chapter C", "Chapter A\nChapter B\nChapter C"],
  ] as const) {
    await beginInlineChapterCreation(frame.contentDocument);
    await expect.poll(() =>
      frame.contentDocument?.querySelector(
        '#app form[data-create-chapter] input[name="chapter-title"]',
      )?.tagName
    ).toBe("INPUT");
    await submitChapterForm(frame, name);
    await expect.poll(() => {
      const root = frame.contentDocument?.querySelector("#app");
      return root?.getAttribute("data-boot-state") === "project-ready"
        && chapterTitles(root).join("\n") === expected;
    }).toBe(true);
  }
}

it("the author confirms Chapter removal, keeps the next current Chapter, then opens empty", { timeout: 90_000 }, async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist Delete Chapter");
  await createThreeChapters(frame);
  await typeThroughSaving(frame, "Alpha");
  await confirmCurrentDelete(frame);
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const alertNode = root?.querySelector("[role=alert]");
    const alert = alertNode?.textContent;
    if (alert !== undefined && alert !== null && alert.length > 0) {
      throw new Error(alert);
    }
    const save = root?.querySelector("[data-save-state]");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && chapterTitles(root).join("\n") === "Chapter B\nChapter C"
      && root?.querySelector("h2")?.textContent === "Chapter B"
      && manuscriptIsEditable(root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) ?? root)
      && save?.getAttribute("data-save-state") !== "saving"
      && save?.getAttribute("data-unsettled-intent-count") === "0";
  }, { timeout: 15_000 }).toBe(true);
  await confirmCurrentDelete(frame);
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const alertNode = root?.querySelector("[role=alert]");
    const alert = alertNode?.textContent;
    if (alert !== undefined && alert !== null && alert.length > 0) {
      throw new Error(alert);
    }
    const save = root?.querySelector("[data-save-state]");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && chapterTitles(root).join("\n") === "Chapter C"
      && root?.querySelector("h2")?.textContent === "Chapter C"
      && manuscriptIsEditable(root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) ?? root)
      && save?.getAttribute("data-save-state") !== "saving"
      && save?.getAttribute("data-unsettled-intent-count") === "0";
  }, { timeout: 15_000 }).toBe(true);
  await confirmCurrentDelete(frame);
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const alertNode = root?.querySelector("[role=alert]");
    const alert = alertNode?.textContent;
    if (alert !== undefined && alert !== null && alert.length > 0) {
      throw new Error(alert);
    }
    return root?.getAttribute("data-boot-state") === "empty-project-ready"
      && chapterTitles(root).join("\n") === "";
  }, { timeout: 15_000 }).toBe(true);
  const editor = frame.contentDocument?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(editor === null || editor === undefined ? undefined : manuscriptBody(editor)).toBe(undefined);
});
