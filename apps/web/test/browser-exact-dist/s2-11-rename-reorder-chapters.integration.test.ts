import { beginInlineChapterCreation, beginTreeAction } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import { manuscriptBody, MANUSCRIPT_EDITOR_SELECTOR } from "../support/manuscript-surface.ts";
import {
  closeApplicationFrame,
  destroyApplicationFrame,
  openApplicationFrame,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
} from "../support/exact-dist-frame.ts";

function chapterTitles(root: Element | null | undefined): string[] {
  return [...(root?.querySelectorAll('nav[aria-label="稿件目录"] [data-chapter-title]') ?? [])]
    .map((node) => node.textContent?.trim() ?? "");
}

async function openNamedProject(root: Element, title: string): Promise<void> {
  await expect.poll(() =>
    [...root.querySelectorAll<HTMLButtonElement>('button[data-open="current_chapter"]')]
      .some((button) => button.textContent === title)
  ).toBe(true);
  const openButton = [...root.querySelectorAll<HTMLButtonElement>('button[data-open="current_chapter"]')]
    .find((button) => button.textContent === title);
  openButton?.click();
  await expect.poll(() => root.getAttribute("data-boot-state")).toBe("project-ready");
}

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("the author renames and reorders Chapters from the canonical tree and they survive reopen", async () => {
  const created = await openApplicationFrame("StoryOS exact-dist create for chapter update");
  await submitProjectForm(created, "Chapter Update Novel");
  const createdRoot = created.contentDocument;
  if (createdRoot === null || createdRoot === undefined) {
    throw new Error("the empty Project document is missing");
  }
  await submitVolumeForm(created, "Volume A");
  await beginInlineChapterCreation(createdRoot);
  await expect.poll(() =>
    createdRoot.querySelector('#app form[data-create-chapter] input[name="chapter-title"]')
      ?.tagName
  ).toBe("INPUT");
  await submitChapterForm(created, "Chapter A");
  await expect.poll(() => {
    const root = createdRoot.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && chapterTitles(root).join("\n") === "Chapter A";
  }).toBe(true);
  await submitChapterForm(created, "Chapter B");
  await expect.poll(() => chapterTitles(createdRoot.querySelector("#app"))).toEqual([
    "Chapter A",
    "Chapter B",
  ]);

  await beginTreeAction(createdRoot, "li[data-chapter-id]", "[data-begin-rename-chapter]");
  createdRoot.querySelector<HTMLButtonElement>("[data-begin-rename-chapter]")?.click();
  await expect.poll(() => createdRoot.querySelector('input[name="chapter-title"]')?.tagName).toBe("INPUT");
  const renameInput = createdRoot.querySelector<HTMLInputElement>(
    '#app form[data-rename-chapter] input[name="chapter-title"]',
  );
  const renameForm = renameInput?.form;
  if (renameInput === null || renameInput === undefined
    || renameForm === null || renameForm === undefined) {
    throw new Error("the Chapter rename form is missing");
  }
  renameInput.value = "Chapter C";
  renameForm.requestSubmit();
  await expect.poll(() => chapterTitles(createdRoot.querySelector("#app"))).toEqual([
    "Chapter C",
    "Chapter B",
  ]);
  await destroyApplicationFrame(created);

  const reopened = await openApplicationFrame("StoryOS exact-dist renamed Chapter library");
  const libraryRoot = reopened.contentDocument?.querySelector("#app");
  if (libraryRoot === null || libraryRoot === undefined) {
    throw new Error("the protected-ready library is missing");
  }
  await openNamedProject(libraryRoot, "Chapter Update Novel");
  await expect.poll(() => chapterTitles(libraryRoot)).toEqual(["Chapter C", "Chapter B"]);
  expect(libraryRoot.querySelector("h2")?.textContent).toBe("Chapter C");
  const libraryEditor = libraryRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(libraryEditor === null ? undefined : manuscriptBody(libraryEditor)).toBe("");
  expect(libraryRoot.textContent).not.toContain("模型");
  expect(libraryRoot.textContent).not.toContain("Agent");

  await beginTreeAction(libraryRoot, 'li[data-chapter-order="1"]', '[data-chapter-move="down"]');
  const moveDown = libraryRoot.querySelector<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] li[data-chapter-order="1"] button[data-chapter-move="down"]',
  );
  if (moveDown === null) {
    throw new Error("the canonical first Chapter has no move-down control");
  }
  moveDown.click();
  await expect.poll(() => chapterTitles(libraryRoot)).toEqual(["Chapter B", "Chapter C"]);
  await destroyApplicationFrame(reopened);

  const ordered = await openApplicationFrame("StoryOS exact-dist reordered Chapter library");
  const orderedRoot = ordered.contentDocument?.querySelector("#app");
  if (orderedRoot === null || orderedRoot === undefined) {
    throw new Error("the reopened library is missing");
  }
  await openNamedProject(orderedRoot, "Chapter Update Novel");
  await expect.poll(() => chapterTitles(orderedRoot)).toEqual(["Chapter B", "Chapter C"]);
  const first = orderedRoot.querySelector('nav[aria-label="稿件目录"] li[data-chapter-order="1"]');
  const second = orderedRoot.querySelector('nav[aria-label="稿件目录"] li[data-chapter-order="2"]');
  expect(first?.querySelector("[data-chapter-title]")?.textContent?.trim()).toBe("Chapter B");
  expect(second?.querySelector("[data-chapter-title]")?.textContent?.trim()).toBe("Chapter C");
  expect(orderedRoot.querySelector("h2")?.textContent).toBe("Chapter C");
});
