import { expect } from "vitest";

import { applyTrustedInput } from "./browser-command-client.ts";
import { beginInlineChapterCreation, beginInlineVolumeCreation, selectTreeChapter } from "./inline-chapter-creation.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "./manuscript-surface.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

let applicationFrame: HTMLIFrameElement | undefined;

export function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist page did not load"));
    }, 10_000);
    frame.addEventListener("load", () => {
      window.clearTimeout(timeout);
      resolve();
    }, { once: true });
  });
}

export async function destroyApplicationFrame(frame: HTMLIFrameElement): Promise<void> {
  if (!frame.isConnected) return;
  const unloaded = nextFrameLoad(frame);
  frame.src = "about:blank";
  await unloaded;
  frame.remove();
}

/** Loads the page at `path` in a new frame. `closeApplicationFrame` destroys the frame that this function opened last. */
export async function openApplicationFrame(title: string, path = "/"): Promise<HTMLIFrameElement> {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = title;
  const loaded = nextFrameLoad(frame);
  frame.src = path;
  document.body.append(frame);
  await loaded;
  return frame;
}

export async function closeApplicationFrame(): Promise<void> {
  if (applicationFrame !== undefined) await destroyApplicationFrame(applicationFrame);
  applicationFrame = undefined;
}

export function applicationWindow(frame: HTMLIFrameElement): Window & typeof globalThis {
  const result = frame.contentWindow;
  if (result === null) throw new Error("the production page realm is unavailable");
  return result as Window & typeof globalThis;
}

export function appRoot(frame: HTMLIFrameElement): Element {
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) {
    throw new Error("the production page root is missing");
  }
  return root;
}

/** Waits until the save state is saved with no failure, after `previousRevisionId` when it is given. Returns the new revision. */
export async function waitSaved(
  root: Element,
  previousRevisionId?: string,
  { timeout = 10_000 }: { timeout?: number } = {},
): Promise<string> {
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
  }, { timeout }).toEqual({ ok: true, failure: "" });
  return root.querySelector("[data-save-state]")?.getAttribute("data-authoritative-revision-id") ?? "";
}

/** The button of the Chapter `title` in the manuscript tree. */
export function chapterButton(root: Element | null | undefined, title: string): HTMLButtonElement | undefined {
  return [...(root?.querySelectorAll<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  ) ?? [])].find((button) => button.textContent === title);
}

/** Selects the Chapter `chapterId` in the tree and waits until its editor shows `heading` and is editable. */
export async function makeCurrent(
  frame: HTMLIFrameElement,
  chapterId: string,
  heading: string,
): Promise<void> {
  await selectTreeChapter(appRoot(frame), chapterId);
  await expect.poll(() => {
    const root = appRoot(frame);
    const editor = root.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return root.querySelector("h2")?.textContent === heading
      && editor !== null
      && editor !== undefined
      && manuscriptIsEditable(editor);
  }, { timeout: 15_000 }).toBe(true);
}

/** Loads the Project page again and waits until the Project is ready and has an editor. */
export async function reloadProject(frame: HTMLIFrameElement, projectId: string): Promise<Element> {
  const reopened = nextFrameLoad(frame);
  frame.src = `/projects/${projectId}`;
  await reopened;
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    if (root === null || root === undefined) return false;
    return root.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  return appRoot(frame);
}

/** Types at the end of the Current Chapter and waits for a new saved Authoritative Revision with `body`. */
export async function typeIntoCurrent(frame: HTMLIFrameElement, text: string, body = text): Promise<void> {
  const root = appRoot(frame);
  const editor = manuscriptEditor(root, applicationWindow(frame));
  const previousRevision = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id");
  if (previousRevision === null || previousRevision === undefined) {
    throw new Error("the authoritative revision is missing");
  }
  editor.focus();
  focusManuscriptEnd(editor, applicationWindow(frame));
  await applyTrustedInput({ operation: "insert_text", text });
  // A new authoritative revision proves that the input reached the saved manuscript. The end of a Chapter switch can
  // replace the editor after the key, so each step reads the editor that is on the screen.
  await expect.poll(() => {
    const save = root.querySelector("[data-save-state]");
    return {
      body: manuscriptBody(manuscriptEditor(root, applicationWindow(frame))),
      saveState: save?.getAttribute("data-save-state"),
      unsettledIntentCount: save?.getAttribute("data-unsettled-intent-count"),
      revisionChanged: save?.getAttribute("data-authoritative-revision-id") !== previousRevision,
    };
  }, { timeout: 10_000 }).toEqual({
    body,
    saveState: "saved",
    unsettledIntentCount: "0",
    revisionChanged: true,
  });
}

/** Submits the protected-ready form and waits for the empty Project. */
export async function submitProjectForm(frame: HTMLIFrameElement, projectTitle: string): Promise<void> {
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = frame.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = projectTitle;
  form.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
}

export async function submitVolumeForm(frame: HTMLIFrameElement, volumeTitle: string): Promise<void> {
  await beginInlineVolumeCreation(frame.contentDocument);
  const title = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-volume] input[name="volume-title"]',
  );
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the Create Volume form is missing");
  }
  title.value = volumeTitle;
  form.requestSubmit();
}

/** Waits until the new Volume can take a Chapter, and opens the Create Chapter form. */
export async function openFirstChapterForm(frame: HTMLIFrameElement): Promise<void> {
  await beginInlineChapterCreation(frame.contentDocument);
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app form[data-create-chapter]') !== null
  ).toBe(true);
}

export async function submitChapterForm(frame: HTMLIFrameElement, chapterTitle: string): Promise<void> {
  await beginInlineChapterCreation(frame.contentDocument);
  const title = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-chapter] input[name="chapter-title"]',
  );
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the Create Chapter form is missing");
  }
  title.value = chapterTitle;
  form.requestSubmit();
}

/** Waits until the last Chapter of the tree is `chapterTitle` in the ready Project. */
export async function waitLastChapter(frame: HTMLIFrameElement, chapterTitle: string): Promise<void> {
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const titles = [...(root?.querySelectorAll(
      'nav[aria-label="稿件目录"] button[data-chapter-id]',
    ) ?? [])].map((button) => button.textContent);
    return root?.getAttribute("data-boot-state") === "project-ready"
      && titles[titles.length - 1] === chapterTitle;
  }).toBe(true);
}

/** Creates a Project with "Volume A" and "Chapter A" through the forms, and opens the Chapter in the editor. */
export async function openChapterEditor(frameTitle: string, projectTitle: string): Promise<{
  frame: HTMLIFrameElement;
  projectId: string;
  chapterId: string;
}> {
  const frame = await openApplicationFrame(frameTitle);
  await submitProjectForm(frame, projectTitle);
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  await submitChapterForm(frame, "Chapter A");
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const projectId = root?.querySelector("[data-project-id]")?.getAttribute("data-project-id");
    const chapterId = root?.querySelector(
      'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
    )?.getAttribute("data-chapter-id");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null
      && typeof projectId === "string"
      && UUID.test(projectId)
      && typeof chapterId === "string"
      && UUID.test(chapterId);
  }, { timeout: 10_000 }).toBe(true);
  const root = appRoot(frame);
  const projectId = root.querySelector("[data-project-id]")?.getAttribute("data-project-id");
  const chapterId = root.querySelector(
    'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
  )?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterId === null || chapterId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  return { frame, projectId, chapterId };
}
