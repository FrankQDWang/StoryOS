import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import { manuscriptBody, MANUSCRIPT_EDITOR_SELECTOR } from "../support/manuscript-surface.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist update-chapter page did not load"));
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

async function loadApplication(title: string): Promise<HTMLIFrameElement> {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = title;
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  return frame;
}

function chapterTitles(root: Element | null | undefined): string[] {
  return [...(root?.querySelectorAll('nav[aria-label="稿件目录"] [data-chapter-title]') ?? [])]
    .map((node) => node.textContent?.trim() ?? "");
}

async function createNamedVolume(root: Document, title: string): Promise<void> {
  const volumeTitle = root.querySelector<HTMLInputElement>(
    '#app form[data-create-volume] input[name="volume-title"]',
  );
  const volumeForm = volumeTitle?.form;
  if (volumeTitle === null || volumeTitle === undefined
    || volumeForm === null || volumeForm === undefined) {
    throw new Error("the Create Volume form is missing");
  }
  volumeTitle.value = title;
  volumeForm.requestSubmit();
}

async function createNamedChapter(root: Document, title: string): Promise<void> {
  const chapterTitle = root.querySelector<HTMLInputElement>(
    '#app form[data-create-chapter] input[name="chapter-title"]',
  );
  const chapterForm = chapterTitle?.form;
  if (chapterTitle === null || chapterTitle === undefined
    || chapterForm === null || chapterForm === undefined) {
    throw new Error("the Create Chapter form is missing");
  }
  chapterTitle.value = title;
  chapterForm.requestSubmit();
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
  if (applicationFrame !== undefined) await destroyApplicationFrame(applicationFrame);
  applicationFrame = undefined;
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("the author renames and reorders Chapters from the canonical tree and they survive reopen", async () => {
  const created = await loadApplication("StoryOS exact-dist create for chapter update");
  await expect.poll(() =>
    created.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = created.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Empty Novel";
  form.requestSubmit();
  await expect.poll(() =>
    created.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  const createdRoot = created.contentDocument;
  if (createdRoot === null || createdRoot === undefined) {
    throw new Error("the empty Project document is missing");
  }
  await createNamedVolume(createdRoot, "Volume A");
  await expect.poll(() =>
    createdRoot.querySelector('#app form[data-create-chapter] input[name="chapter-title"]')
      ?.tagName
  ).toBe("INPUT");
  await createNamedChapter(createdRoot, "Chapter A");
  await expect.poll(() => {
    const root = createdRoot.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && chapterTitles(root).join("\n") === "Chapter A";
  }).toBe(true);
  await createNamedChapter(createdRoot, "Chapter B");
  await expect.poll(() => chapterTitles(createdRoot.querySelector("#app"))).toEqual([
    "Chapter A",
    "Chapter B",
  ]);

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

  const reopened = await loadApplication("StoryOS exact-dist renamed Chapter library");
  const libraryRoot = reopened.contentDocument?.querySelector("#app");
  if (libraryRoot === null || libraryRoot === undefined) {
    throw new Error("the protected-ready library is missing");
  }
  await openNamedProject(libraryRoot, "Empty Novel");
  await expect.poll(() => chapterTitles(libraryRoot)).toEqual(["Chapter C", "Chapter B"]);
  expect(libraryRoot.querySelector("h2")?.textContent).toBe("Chapter C");
  const libraryEditor = libraryRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(libraryEditor === null ? undefined : manuscriptBody(libraryEditor)).toBe("");
  expect(libraryRoot.textContent).not.toContain("模型");
  expect(libraryRoot.textContent).not.toContain("Agent");

  const moveDown = libraryRoot.querySelector<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] li[data-chapter-order="1"] button[data-chapter-move="down"]',
  );
  if (moveDown === null) {
    throw new Error("the canonical first Chapter has no move-down control");
  }
  moveDown.click();
  await expect.poll(() => chapterTitles(libraryRoot)).toEqual(["Chapter B", "Chapter C"]);
  await destroyApplicationFrame(reopened);

  const ordered = await loadApplication("StoryOS exact-dist reordered Chapter library");
  const orderedRoot = ordered.contentDocument?.querySelector("#app");
  if (orderedRoot === null || orderedRoot === undefined) {
    throw new Error("the reopened library is missing");
  }
  await openNamedProject(orderedRoot, "Empty Novel");
  await expect.poll(() => chapterTitles(orderedRoot)).toEqual(["Chapter B", "Chapter C"]);
  const first = orderedRoot.querySelector('nav[aria-label="稿件目录"] li[data-chapter-order="1"]');
  const second = orderedRoot.querySelector('nav[aria-label="稿件目录"] li[data-chapter-order="2"]');
  expect(first?.querySelector("[data-chapter-title]")?.textContent?.trim()).toBe("Chapter B");
  expect(second?.querySelector("[data-chapter-title]")?.textContent?.trim()).toBe("Chapter C");
  expect(orderedRoot.querySelector("h2")?.textContent).toBe("Chapter C");
});
