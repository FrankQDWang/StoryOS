import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  applyTrustedInput, updateAuthorEditSubmissionHold, updateClientSessionCookie,
} from "../support/browser-command-client.ts";
import { selectTreeChapter } from "../support/inline-chapter-creation.ts";
import {
  focusManuscriptEnd, manuscriptBody, manuscriptEditor, manuscriptIsEditable,
} from "../support/manuscript-surface.ts";
import {
  appRoot, applicationWindow, chapterButton, closeApplicationFrame, makeCurrent, openApplicationFrame,
  openFirstChapterForm, reloadProject, submitChapterForm, submitProjectForm, submitVolumeForm, typeIntoCurrent,
  waitLastChapter, waitSaved,
} from "../support/exact-dist-frame.ts";

// The Arriving Chapter of a selection in the contents, with the real Server (ADR 0105). ICS-03 and ICS-04 add their
// steps to this journey.

afterEach(async () => {
  await updateAuthorEditSubmissionHold({ action: "release" });
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

/** The text of each block of the Chapter as the Server stores it. */
async function storedText(win: Window & typeof globalThis, projectId: string, chapterId: string): Promise<string[]> {
  const read = await getChapter({ baseUrl: win.location.origin, projectId, chapterId, fetchImpl: win.fetch.bind(win) });
  return read.chapter.current_revision.blocks.map((block) => block.text);
}

it("stores the input of the earlier Chapter that was not saved at the selection, and refuses a key there later",
  async () => {
    const frame = await openApplicationFrame("Chapter arrival");
    await submitProjectForm(frame, "Chapter arrival");
    await submitVolumeForm(frame, "Volume A");
    await openFirstChapterForm(frame);
    for (const name of ["Chapter A", "Chapter B"] as const) {
      await submitChapterForm(frame, name);
      await waitLastChapter(frame, name);
    }
    const root = () => appRoot(frame);
    const win = applicationWindow(frame);
    const projectId = root().querySelector("[data-project-id]")?.getAttribute("data-project-id") ?? "";
    const [a = "", b = ""] = ["Chapter A", "Chapter B"].map((title) => chapterButton(root(), title)
      ?.getAttribute("data-chapter-id") ?? "");
    expect([projectId, a, b].every((value) => value.length > 0)).toBe(true);
    await typeIntoCurrent(frame, "Alpha");
    const shown = (editor: HTMLElement) => ({ title: root().querySelector("h2")?.textContent,
      body: manuscriptBody(editor), editable: manuscriptIsEditable(editor), connected: editor.isConnected });

    // Chapter B has no copy: the editor of Chapter A stays on the screen and accepts no input.
    const editorA = manuscriptEditor(root(), win);
    await updateAuthorEditSubmissionHold({ action: "hold" });
    focusManuscriptEnd(editorA, win);
    await applyTrustedInput({ operation: "insert_text", text: " one" });
    await expect.poll(() => manuscriptBody(editorA)).toBe("Alpha one");
    await selectTreeChapter(root(), b);
    await expect.poll(() => shown(editorA)).toEqual({ title: "Chapter A", body: "Alpha one", editable: false,
      connected: true });
    focusManuscriptEnd(editorA, win);
    const focused = editorA.ownerDocument.activeElement === editorA;
    // A read-only editor takes no focus. Then the key goes to the button of the selection, as after a click.
    if (!focused) chapterButton(root(), "Chapter B")?.focus();
    await applyTrustedInput({ operation: "insert_text", text: "Z" });
    const afterKey = { ...shown(editorA), focused };
    await updateAuthorEditSubmissionHold({ action: "release" });
    await makeCurrent(frame, b, "Chapter B");
    await waitSaved(root(), undefined, { timeout: 15_000 });

    // Chapter A has the copy of its settled base: it is on the screen at once, and the editor of Chapter B ends.
    const editorB = manuscriptEditor(root(), win);
    await updateAuthorEditSubmissionHold({ action: "hold" });
    focusManuscriptEnd(editorB, win);
    await applyTrustedInput({ operation: "insert_text", text: "Beta" });
    await expect.poll(() => manuscriptBody(editorB)).toBe("Beta");
    await selectTreeChapter(root(), a);
    const arriving = () => ({ ...shown(manuscriptEditor(root(), win)), earlier: editorB.isConnected });
    await expect.poll(arriving).toEqual({ title: "Chapter A", body: "Alpha one", editable: false, connected: true,
      earlier: false });
    await updateAuthorEditSubmissionHold({ action: "release" });
    await makeCurrent(frame, a, "Chapter A");
    await waitSaved(root(), undefined, { timeout: 15_000 });

    const reloaded = await reloadProject(frame, projectId);
    const loaded = applicationWindow(frame);
    expect({ afterKey, shown: manuscriptBody(manuscriptEditor(reloaded, loaded)),
      storedA: await storedText(loaded, projectId, a), storedB: await storedText(loaded, projectId, b) }).toEqual({
      afterKey: { title: "Chapter A", body: "Alpha one", editable: false, connected: true, focused: false },
      shown: "Alpha one",
      storedA: ["Alpha one"], storedB: ["Beta"] });
  });
