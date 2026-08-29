import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist Tiptap editor page did not load"));
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

async function readChapter(frame: HTMLIFrameElement, projectId: string, chapterId: string) {
  const childWindow = applicationWindow(frame);
  const chapter = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  const revision = chapter.chapter.current_revision;
  const block = revision.blocks[0];
  if (revision.blocks.length !== 1 || block === undefined) {
    throw new Error("the Chapter Block is missing");
  }
  return {
    revisionId: revision.revision_id,
    body: revision.body,
    manuscriptBlockId: block.manuscript_block_id,
  };
}

it("settles one Tiptap Block replacement and preserves text, Block identity, and Head", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Tiptap Block replacement";
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
  title.value = "Tiptap Novel";
  form.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  expect(appRoot(frame).querySelector("textarea")).toBeNull();
  expect(appRoot(frame).querySelector(MANUSCRIPT_EDITOR_SELECTOR)).toBeNull();

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

  const root = appRoot(frame);
  const childWindow = applicationWindow(frame);
  expect(root.querySelector("textarea")).toBeNull();
  const editor = manuscriptEditor(root, childWindow);
  expect(manuscriptBody(editor)).toBe("");
  expect(editor.getAttribute("contenteditable")).toBe("true");
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count")
  ).toBe("0");

  const projectId = root.querySelector("form[data-rename]")?.getAttribute("data-rename");
  const chapterId = root.querySelector(
    'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
  )?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterId === null || chapterId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  const before = await readChapter(frame, projectId, chapterId);
  expect(editor.getAttribute("data-manuscript-block-id")).toBe(before.manuscriptBlockId);

  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: "Visible block." });
  await expect.poll(() => manuscriptBody(editor)).toBe("Visible block.");
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-save-state") === "saved"
    && root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count") === "0"
  ).toBe(true);

  const settled = await readChapter(frame, projectId, chapterId);
  expect(settled.body).toBe("Visible block.");
  expect(settled.manuscriptBlockId).toBe(before.manuscriptBlockId);
  expect(settled.revisionId).not.toBe(before.revisionId);

  const reloaded = nextFrameLoad(frame);
  childWindow.location.reload();
  await reloaded;
  await expect.poll(() => {
    const nextRoot = frame.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }).toBe(true);
  const reloadedRoot = appRoot(frame);
  const reloadedEditor = manuscriptEditor(reloadedRoot, applicationWindow(frame));
  expect(reloadedRoot.querySelector("textarea")).toBeNull();
  expect(manuscriptBody(reloadedEditor)).toBe("Visible block.");
  const afterReload = await readChapter(frame, projectId, chapterId);
  expect(afterReload).toEqual(settled);
  expect(reloadedEditor.getAttribute("data-manuscript-block-id")).toBe(before.manuscriptBlockId);
  expect(reloadedEditor.querySelectorAll("p")).toHaveLength(1);

  reloadedEditor.focus();
  await applyTrustedInput({ operation: "insert_text", text: "x" });
  await expect.poll(() => manuscriptBody(reloadedEditor)).toBe("Visible block.x");
  const bold = new KeyboardEvent("keydown", { key: "b", ctrlKey: true, bubbles: true });
  reloadedEditor.dispatchEvent(bold);
  expect(reloadedEditor.querySelector("strong")).toBeNull();
  expect(reloadedEditor.querySelector("b")).toBeNull();
  expect(manuscriptBody(reloadedEditor)).toBe("Visible block.x");
});
