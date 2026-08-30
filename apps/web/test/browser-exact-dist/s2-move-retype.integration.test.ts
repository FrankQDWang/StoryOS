import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  applyTrustedInput,
  updateClientSessionCookie,
  updateClipboardPermission,
} from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist move-retype page did not load"));
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
  await updateClipboardPermission({ action: "clear" });
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

async function openChapterEditor(): Promise<{
  frame: HTMLIFrameElement;
  projectId: string;
  chapterId: string;
}> {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist move and retype";
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
  title.value = "Move Retype Novel";
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
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const projectId = root?.querySelector("form[data-rename]")?.getAttribute("data-rename");
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
  const projectId = root.querySelector("form[data-rename]")?.getAttribute("data-rename");
  const chapterId = root.querySelector(
    'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
  )?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterId === null || chapterId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  return { frame, projectId, chapterId };
}

async function readRevision(frame: HTMLIFrameElement, projectId: string, chapterId: string) {
  const childWindow = applicationWindow(frame);
  const chapter = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  return chapter.chapter.current_revision;
}

function focusBlock(
  editor: HTMLElement,
  realm: Window & typeof globalThis,
  index: number,
): void {
  const block = editor.querySelectorAll("p, h1")[index];
  if (block === undefined) throw new Error("the Block is missing");
  editor.focus();
  const selection = realm.getSelection();
  if (selection === null) throw new Error("the manuscript selection is unavailable");
  const range = editor.ownerDocument.createRange();
  range.setStart(block, 0);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

it("moves and retypes Blocks with stable identity and refuses copy as a move", async () => {
  const { frame, projectId, chapterId } = await openChapterEditor();
  const root = appRoot(frame);
  const childWindow = applicationWindow(frame);
  const editor = manuscriptEditor(root, childWindow);
  const before = await readRevision(frame, projectId, chapterId);
  const leftId = before.blocks[0]?.manuscript_block_id;
  if (before.blocks.length !== 1 || leftId === undefined) {
    throw new Error("the starting Block is missing");
  }

  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: "Hello" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello");
  await waitSaved(root, before.revision_id);
  const helloRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  await applyTrustedInput({ operation: "enter" });
  await expect.poll(() => editor.querySelectorAll("p, h1").length, { timeout: 10_000 }).toBe(2);
  await waitSaved(root, helloRevisionId);
  const splitRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  await applyTrustedInput({ operation: "insert_text", text: "World" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello\nWorld");
  await waitSaved(root, splitRevisionId);
  const split = await readRevision(frame, projectId, chapterId);
  const rightId = split.blocks[1]?.manuscript_block_id;
  expect(split.blocks.map((block) => block.manuscript_block_id)).toEqual([leftId, rightId]);
  expect(rightId).toMatch(UUID);
  expect(rightId).not.toBe(leftId);

  focusBlock(editor, childWindow, 1);
  await applyTrustedInput({ operation: "move_block_up" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("World\nHello");
  await waitSaved(root, split.revision_id);
  const moved = await readRevision(frame, projectId, chapterId);
  expect(moved.blocks).toEqual([
    { manuscript_block_id: rightId, block_kind: "paragraph", text: "World" },
    { manuscript_block_id: leftId, block_kind: "paragraph", text: "Hello" },
  ]);

  await updateClipboardPermission({ action: "grant" });
  await childWindow.navigator.clipboard.writeText("\nCopied");
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "paste" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("World\nHello\nCopied");
  await waitSaved(root, moved.revision_id);
  const pasted = await readRevision(frame, projectId, chapterId);
  const copiedId = pasted.blocks[2]?.manuscript_block_id;
  expect(pasted.blocks.map((block) => block.manuscript_block_id)).toEqual([
    rightId, leftId, copiedId,
  ]);
  expect(copiedId).toMatch(UUID);
  expect(copiedId).not.toBe(leftId);
  expect(copiedId).not.toBe(rightId);

  focusBlock(editor, childWindow, 0);
  await applyTrustedInput({ operation: "retype_block" });
  await expect.poll(() => editor.getAttribute("data-manuscript-block-kinds"), { timeout: 10_000 })
    .toBe("heading paragraph paragraph");
  await waitSaved(root, pasted.revision_id);
  const retyped = await readRevision(frame, projectId, chapterId);
  expect(retyped.blocks).toEqual([
    { manuscript_block_id: rightId, block_kind: "heading", text: "World" },
    { manuscript_block_id: leftId, block_kind: "paragraph", text: "Hello" },
    { manuscript_block_id: copiedId, block_kind: "paragraph", text: "Copied" },
  ]);

  await destroyApplicationFrame(frame);
  const reopened = document.createElement("iframe");
  applicationFrame = reopened;
  reopened.title = "StoryOS exact-dist move and retype reload";
  const reopenedLoaded = nextFrameLoad(reopened);
  reopened.src = `/projects/${projectId}`;
  document.body.append(reopened);
  await reopenedLoaded;
  await expect.poll(() => {
    const nextRoot = reopened.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const reopenedEditor = manuscriptEditor(appRoot(reopened), applicationWindow(reopened));
  await expect.poll(() => reopenedEditor.getAttribute("data-manuscript-block-kinds"))
    .toBe("heading paragraph paragraph");
  expect(manuscriptBody(reopenedEditor)).toBe("World\nHello\nCopied");
  const reopenedRevision = await readRevision(reopened, projectId, chapterId);
  expect(reopenedRevision.blocks).toEqual([
    { manuscript_block_id: rightId, block_kind: "heading", text: "World" },
    { manuscript_block_id: leftId, block_kind: "paragraph", text: "Hello" },
    { manuscript_block_id: copiedId, block_kind: "paragraph", text: "Copied" },
  ]);
});
