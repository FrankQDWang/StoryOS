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
  nextFrameLoad,
  openApplicationFrame,
  waitSaved,
} from "../support/exact-dist-frame.ts";

const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const RESTORED_TITLE = "WAL after base backup";
const SETTLED_BODY = "Authoritative A 中文 EN";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

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

it("writes Chinese and English after restore, settles, reloads, and keeps isolation", async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist restored Project");
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
