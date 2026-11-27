import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  assertPageProjectReceipts, updateAuthorEditSubmissionHold, updateClientSessionCookie,
} from "../support/browser-command-client.ts";
import { selectTreeChapter } from "../support/inline-chapter-creation.ts";
import { manuscriptBody, manuscriptEditor, manuscriptIsEditable } from "../support/manuscript-surface.ts";
import {
  appRoot, applicationWindow, chapterButton, closeApplicationFrame, openApplicationFrame, openFirstChapterForm,
  reloadProject, submitChapterForm, submitProjectForm, submitVolumeForm, typeIntoCurrent, waitLastChapter, waitSaved,
} from "../support/exact-dist-frame.ts";

// A Set Current Chapter command whose response and the response of its repeat are lost, with the real Server (#1925,
// ADR 0105, section 5). The journey sends 5 Shared Command Challenges: the Project, the Volume, two Chapters, and one
// Set Current Chapter command. The repeat uses the same Challenge.

afterEach(async () => {
  await updateAuthorEditSubmissionHold({ action: "release" });
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("reads the Project after two lost responses of an applied Set Current Chapter command, and saves a letter in the selected Chapter",
  async () => {
    const frame = await openApplicationFrame("Chapter switch with an unknown result");
    await submitProjectForm(frame, "Unknown switch result");
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
    const shown = (realm = applicationWindow(frame)) => {
      const editor = manuscriptEditor(root(), realm);
      return { title: root().querySelector(".editor-panel h2")?.textContent, body: manuscriptBody(editor),
        editable: manuscriptIsEditable(editor), alert: root().querySelector('.editor-panel [role="alert"]')?.textContent,
        arriving: root().querySelector("[data-arrival-no-open-suggestion]")
          ?.getAttribute("data-arrival-no-open-suggestion") };
    };

    // The Server applies the command and replays it for the repeat, and the page gets neither response.
    await updateAuthorEditSubmissionHold({ action: "lose_set_current_chapter" });
    await selectTreeChapter(root(), b, { arrival: "in-progress" });
    await expect.poll(() => shown(win), { timeout: 30_000 }).toEqual({ title: "Chapter B", body: "", editable: true,
      alert: undefined, arriving: "" });
    await waitSaved(root());
    await updateAuthorEditSubmissionHold({ action: "release" });
    await typeIntoCurrent(frame, "x");

    const reloaded = await reloadProject(frame, projectId);
    const loaded = applicationWindow(frame);
    const read = await getChapter({ baseUrl: loaded.location.origin, projectId, chapterId: b,
      fetchImpl: loaded.fetch.bind(loaded) });
    expect({ title: reloaded.querySelector(".editor-panel h2")?.textContent,
      body: manuscriptBody(manuscriptEditor(reloaded, loaded)),
      stored: read.chapter.current_revision.blocks.map((block) => block.text) })
      .toEqual({ title: "Chapter B", body: "x", stored: ["x"] });
    // The repeat is a replay of the first command, so the Server stores one Receipt.
    await assertPageProjectReceipts({ projectId, kinds: "listed", receipts: { setCurrentChapter: 1 } });
  }, 120_000);
