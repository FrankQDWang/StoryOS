import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";
import {
  appRoot,
  applicationWindow,
  closeApplicationFrame,
  destroyApplicationFrame,
  openApplicationFrame,
  openChapterEditor,
  waitSaved,
} from "../support/exact-dist-frame.ts";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

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

it("undoes the admitted Author Actions one at a time, also after a reload", async () => {
  const { frame, projectId, chapterId } = await openChapterEditor(
    "StoryOS exact-dist undo",
    "Undo Novel",
  );
  const root = appRoot(frame);
  const childWindow = applicationWindow(frame);
  const editor = manuscriptEditor(root, childWindow);
  const before = await readRevision(frame, projectId, chapterId);
  const blockId = before.blocks[0]?.manuscript_block_id;
  if (before.blocks.length !== 1 || blockId === undefined) {
    throw new Error("the starting Block is missing");
  }

  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: "Hello" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello");
  await waitSaved(root, before.revision_id);
  const hello = await readRevision(frame, projectId, chapterId);
  expect(hello.blocks).toEqual([
    { manuscript_block_id: blockId, block_kind: "paragraph", text: "Hello" },
  ]);
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-author-undo-frontier")
  ).toMatch(/^[1-9][0-9]*$/);
  const helloFrontier = root.querySelector("[data-save-state]")
    ?.getAttribute("data-author-undo-frontier") ?? "";

  await applyTrustedInput({ operation: "insert_text", text: " World" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello World");
  await waitSaved(root, hello.revision_id);
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-author-undo-frontier")
  ).not.toBe(helloFrontier);

  await applyTrustedInput({ operation: "undo_latest_author_action" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello");
  await waitSaved(root);
  const undone = await readRevision(frame, projectId, chapterId);
  expect(undone.blocks).toEqual([
    { manuscript_block_id: blockId, block_kind: "paragraph", text: "Hello" },
  ]);
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-author-undo-frontier")
  ).toBe(helloFrontier);

  await destroyApplicationFrame(frame);
  const reopened = await openApplicationFrame("StoryOS exact-dist undo reload", `/projects/${projectId}`);
  await expect.poll(() => {
    const nextRoot = reopened.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const reopenedRoot = appRoot(reopened);
  const reopenedEditor = manuscriptEditor(reopenedRoot, applicationWindow(reopened));
  await expect.poll(() => manuscriptBody(reopenedEditor)).toBe("Hello");
  const reopenedRevision = await readRevision(reopened, projectId, chapterId);
  expect(reopenedRevision.blocks).toEqual([
    { manuscript_block_id: blockId, block_kind: "paragraph", text: "Hello" },
  ]);
  await expect.poll(() =>
    reopenedRoot.querySelector("[data-save-state]")?.getAttribute("data-author-undo-frontier")
  ).toBe(helloFrontier);

  // The next Undo reverses the earlier edit, and the editor stays writable (ADR 0067).
  const reopenedWindow = applicationWindow(reopened);
  reopenedEditor.focus();
  focusManuscriptEnd(reopenedEditor, reopenedWindow);
  await applyTrustedInput({ operation: "undo_latest_author_action" });
  await expect.poll(() => manuscriptBody(reopenedEditor), { timeout: 10_000 }).toBe("");
  await waitSaved(reopenedRoot);
  expect((await readRevision(reopened, projectId, chapterId)).blocks).toEqual(before.blocks);
  await expect.poll(() =>
    reopenedRoot.querySelector("[data-save-state]")?.getAttribute("data-author-undo-frontier")
  ).not.toBe(helloFrontier);
  expect(reopenedEditor.getAttribute("contenteditable")).toBe("true");
});
