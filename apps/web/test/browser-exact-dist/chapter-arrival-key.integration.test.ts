import { afterEach, expect, it, vi } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { updateAuthorEditSubmissionHold, updateClientSessionCookie } from "../support/browser-command-client.ts";
import { selectTreeChapter } from "../support/inline-chapter-creation.ts";
import { focusManuscriptEnd, manuscriptBody, manuscriptEditor, manuscriptIsEditable } from "../support/manuscript-surface.ts";
import {
  appRoot, applicationWindow, chapterButton, closeApplicationFrame, openApplicationFrame, openFirstChapterForm,
  reloadProject, submitChapterForm, submitProjectForm, submitVolumeForm, waitLastChapter,
} from "../support/exact-dist-frame.ts";

// #1960, with the real Server: the switch finds no Arrival Input and changes the editor. A key in the Arriving Chapter
// in the same task, after that check, goes into the stored text of the Chapter.

afterEach(async () => {
  await updateAuthorEditSubmissionHold({ action: "release" });
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

/** Resolves after each transaction of the Journal `name` of `realm` that started before the call ended. */
async function journalTransactionsEnded(realm: Window & typeof globalThis, name: string): Promise<void> {
  const database = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = realm.indexedDB.open(name);
    request.onsuccess = () => { resolve(request.result); };
    request.onerror = () => { reject(request.error ?? new Error("The Journal did not open")); };
  });
  try {
    const transaction = database.transaction([...database.objectStoreNames], "readwrite");
    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => { resolve(); };
      transaction.onerror = () => { reject(transaction.error ?? new Error("The Journal transaction failed")); };
    });
  } finally {
    database.close();
  }
}

it("stores a key typed in an Arriving Chapter after the switch found no Arrival Input, also after a new load", {
  timeout: 90_000,
}, async () => {
  const frame = await openApplicationFrame("Chapter arrival key");
  await submitProjectForm(frame, "Chapter arrival key");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  for (const name of ["Chapter A", "Chapter B"] as const) {
    await submitChapterForm(frame, name);
    await waitLastChapter(frame, name);
  }
  const root = () => appRoot(frame);
  const win = applicationWindow(frame);
  const projectId = root().querySelector("[data-project-id]")?.getAttribute("data-project-id") ?? "";
  const chapterB = chapterButton(root(), "Chapter B")?.getAttribute("data-chapter-id") ?? "";
  expect([projectId, chapterB].every((value) => value.length > 0)).toBe(true);

  // The open of an Editor Session records it in the session storage after its Session read.
  const recorded = vi.spyOn(win.Storage.prototype, "setItem");
  // The Set Current Chapter command waits, so the Arriving Chapter stays on the screen.
  await updateAuthorEditSubmissionHold({ action: "hold_set_current_chapter" });
  await selectTreeChapter(root(), chapterB, { arrival: "in-progress" });
  await expect.poll(() => win.document.documentElement.dataset.storyosHeldRequests, { timeout: 15_000 }).toBe("1");
  const shown = () => ({ title: root().querySelector("h2")?.textContent,
    editable: manuscriptIsEditable(manuscriptEditor(root(), win)),
    arriving: root().querySelector("[data-arrival-no-open-suggestion]")?.getAttribute("data-arrival-no-open-suggestion") });
  await expect.poll(shown, { timeout: 15_000 }).toEqual({ title: "Chapter B", editable: true, arriving: "true" });
  const arriving = manuscriptEditor(root(), win);
  focusManuscriptEnd(arriving, win);
  // An IME composition with no text holds the end of the switch (ADR 0105, section 4).
  arriving.dispatchEvent(new win.CompositionEvent("compositionstart", { bubbles: true }));
  await updateAuthorEditSubmissionHold({ action: "release" });
  await expect.poll(() => recorded.mock.calls.some(([key]) => key.startsWith("active_session:")), { timeout: 15_000 })
    .toBe(true);
  const [, owner = "", project = ""] = recorded.mock.calls.map(([key]) => key)
    .find((key) => key.startsWith("active_session:"))?.split(":") ?? [];
  recorded.mockRestore();
  await journalTransactionsEnded(win, `storyos-local-edit-journal:${owner}:${project}`);

  arriving.dispatchEvent(new win.CompositionEvent("compositionend", { bubbles: true, data: "" }));
  // The switch decides in the microtasks after the end of the composition, and the key comes after its check.
  for (let step = 0; step < 100 && arriving.isConnected; step++) await Promise.resolve();
  win.document.execCommand("insertText", false, "?");

  // The save state controls the new load. The browser test of the Project view proves the text on the screen.
  await expect.poll(() => {
    const save = root().querySelector("[data-save-state]");
    return { save: save?.getAttribute("data-save-state"), unsettled: save?.getAttribute("data-unsettled-intent-count"),
      arriving: shown().arriving };
  }, { timeout: 15_000 }).toEqual({ save: "saved", unsettled: "0", arriving: "" });
  const reloaded = await reloadProject(frame, projectId);
  const loaded = applicationWindow(frame);
  const stored = await getChapter({ baseUrl: loaded.location.origin, projectId, chapterId: chapterB,
    fetchImpl: loaded.fetch.bind(loaded) });
  expect({ title: reloaded.querySelector("h2")?.textContent, body: manuscriptBody(manuscriptEditor(reloaded, loaded)),
    stored: stored.chapter.current_revision.blocks.map((block) => block.text) })
    .toEqual({ title: "Chapter B", body: "?", stored: ["?"] });
});
