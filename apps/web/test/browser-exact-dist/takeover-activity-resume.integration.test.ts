import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, assertPageProjectReceipts, updateClientSessionCookie }
  from "../support/browser-command-client.ts";
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
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
} from "../support/exact-dist-frame.ts";

const JOURNAL_DATABASE_PREFIX = "storyos-local-edit-journal:";

async function unloadApplicationFrame(frame: HTMLIFrameElement): Promise<void> {
  const unloaded = nextFrameLoad(frame);
  frame.src = "about:blank";
  await unloaded;
}

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

// The poll returns the failure facts, so a stuck editor shows its state in the diff.
async function waitWritableAndSynchronized(frame: HTMLIFrameElement): Promise<void> {
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const node = root?.querySelector("[data-save-state]");
    const activity = root?.querySelector("[data-activity-resync]");
    const editor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return {
      save: node?.getAttribute("data-save-state"),
      unsettled: node?.getAttribute("data-unsettled-intent-count"),
      failure: node?.getAttribute("data-editor-failure") ?? "",
      editable: editor !== null && editor !== undefined && manuscriptIsEditable(editor),
      activityResync: activity?.getAttribute("data-activity-resync"),
      activitySynchronized: (activity?.getAttribute("data-activity-replay-generation") ?? "") !== "",
    };
  }, { timeout: 30_000 }).toEqual({
    save: "saved", unsettled: "0", failure: "", editable: true,
    activityResync: "", activitySynchronized: true,
  });
}

// A new origin has no Journal database, so the client reads activity from the base snapshot.
async function deleteJournalDatabase(projectId: string): Promise<void> {
  const names = (await indexedDB.databases())
    .map((database) => database.name ?? "")
    .filter((name) => name.startsWith(JOURNAL_DATABASE_PREFIX) && name.endsWith(`:${projectId}`));
  expect(names).toHaveLength(1);
  for (const name of names) {
    await new Promise<void>((resolve, reject) => {
      const request = indexedDB.deleteDatabase(name);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error ?? new Error("the Journal database delete failed"));
      request.onblocked = () => reject(new Error("an open connection blocks the Journal database delete"));
    });
  }
}

it("synchronizes activity and keeps the editor writable after a takeover with no later command", {
  timeout: 120_000,
}, async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist takeover activity resume");
  await submitProjectForm(frame, "Takeover activity resume");
  await submitVolumeForm(frame, "Volume A");
  await submitChapterForm(frame, "Chapter A");
  await expect.poll(() => appRoot(frame).getAttribute("data-boot-state")).toBe("project-ready");
  const chapterSelector = 'nav[aria-label="稿件目录"] button[data-chapter-id]';
  await expect.poll(() => appRoot(frame).querySelector(chapterSelector)?.getAttribute("data-chapter-id") ?? "")
    .not.toBe("");
  const projectId = appRoot(frame).querySelector("[data-project-id]")?.getAttribute("data-project-id");
  const chapterId = appRoot(frame).querySelector(chapterSelector)?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined || chapterId === null || chapterId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  await waitWritableAndSynchronized(frame);

  // A secondary Editor Session with an empty Journal takes over, as a second Server origin does.
  const realm = applicationWindow(frame);
  for (const key of Object.keys(realm.sessionStorage)) {
    if (key.startsWith("active_session:") && key.endsWith(`:${projectId}`)) realm.sessionStorage.removeItem(key);
  }
  await unloadApplicationFrame(frame);
  await deleteJournalDatabase(projectId);
  const reloaded = nextFrameLoad(frame);
  frame.src = `/projects/${projectId}`;
  await reloaded;
  await expect.poll(() => appRoot(frame).querySelector("[data-take-over-writer]")?.tagName, { timeout: 15_000 })
    .toBe("BUTTON");
  appRoot(frame).querySelector<HTMLButtonElement>("[data-take-over-writer]")?.click();
  await expect.poll(() => appRoot(frame).querySelector("[data-take-over-writer]")).toBeNull();
  await waitWritableAndSynchronized(frame);

  const root = appRoot(frame);
  const page = applicationWindow(frame);
  const editor = manuscriptEditor(root, page);
  page.focus();
  editor.focus();
  focusManuscriptEnd(editor, page);
  await applyTrustedInput({ operation: "insert_text", text: "接管后" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("接管后");
  await waitWritableAndSynchronized(frame);
  const saved = await getChapter({ baseUrl: page.location.origin, projectId, chapterId,
    fetchImpl: page.fetch.bind(page) });
  expect(saved.chapter.current_revision.body).toBe("接管后");
  await assertPageProjectReceipts({ projectId, kinds: "listed", receipts: { takeOverProjectWriter: 1 } });
});
