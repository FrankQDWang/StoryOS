import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { requestResult } from "../../src/indexed-db-request.ts";
import { JOURNAL_DATABASE_VERSION } from "../../src/local-edit-journal.ts";
import {
  applyTrustedInput, assertPageProjectReceipts, updateAuthorEditSubmissionHold, updateClientSessionCookie, updateClipboardPermission,
} from "../support/browser-command-client.ts";
import { beginTreeAction, selectTreeChapter } from "../support/inline-chapter-creation.ts";
import {
  focusManuscriptEnd, manuscriptBody, manuscriptEditor, manuscriptIsEditable,
} from "../support/manuscript-surface.ts";
import {
  appRoot, applicationWindow, chapterButton, closeApplicationFrame, nextFrameLoad, openApplicationFrame,
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

// The journey sends 14 Shared Command Challenges: 5 for the Project, the Volume, and the Chapters, 8 for Set Current
// Chapter, and 1 for the removal of a Chapter. The Server gives 10 in one minute, so a switch or the removal can wait
// for the Retry-After, at most 61 s (#1679). Each wait for a switch or a removal, and the time limit of the journey,
// permit that wait.
const SWITCH_WAIT = 75_000;

/** The text of each block of the Chapter as the Server stores it. */
async function storedText(win: Window & typeof globalThis, projectId: string, chapterId: string): Promise<string[]> {
  const read = await getChapter({ baseUrl: win.location.origin, projectId, chapterId, fetchImpl: win.fetch.bind(win) });
  return read.chapter.current_revision.blocks.map((block) => block.text);
}

/** The current Revision of the Chapter as the Server stores it. */
async function revisionOf(win: Window & typeof globalThis, projectId: string, chapterId: string): Promise<string> {
  const read = await getChapter({ baseUrl: win.location.origin, projectId, chapterId, fetchImpl: win.fetch.bind(win) });
  return read.chapter.current_revision.revision_id;
}

/** Puts the caret at the start of the paragraph `index` of `editor`. */
function caretAtStart(editor: HTMLElement, index: number, realm: Window & typeof globalThis): void {
  const paragraph = editor.querySelectorAll(":scope > p")[index];
  const selection = realm.getSelection();
  if (paragraph === undefined || selection === null) throw new Error("the paragraph or the selection is missing");
  editor.focus();
  const range = editor.ownerDocument.createRange();
  range.setStart(paragraph, 0);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

it("stores the input of the earlier Chapter at the selection, stores the keys in the Arriving Chapter one time after the hand-over, and runs a second selection after it",
  async () => {
    const frame = await openApplicationFrame("Chapter arrival");
    await submitProjectForm(frame, "Chapter arrival");
    await submitVolumeForm(frame, "Volume A");
    await openFirstChapterForm(frame);
    for (const name of ["Chapter A", "Chapter B", "Chapter C"] as const) {
      await submitChapterForm(frame, name);
      await waitLastChapter(frame, name);
    }
    const root = () => appRoot(frame);
    const win = applicationWindow(frame);
    const projectId = root().querySelector("[data-project-id]")?.getAttribute("data-project-id") ?? "";
    const [a = "", b = "", c = ""] = ["Chapter A", "Chapter B", "Chapter C"].map((title) => chapterButton(root(), title)
      ?.getAttribute("data-chapter-id") ?? "");
    expect([projectId, a, b, c].every((value) => value.length > 0)).toBe(true);
    await typeIntoCurrent(frame, "Alpha");
    const shown = (editor: HTMLElement) => ({ title: root().querySelector("h2")?.textContent,
      body: manuscriptBody(editor), editable: manuscriptIsEditable(editor) });
    /**
     * Waits with no new click until the switch of the selection has the editor of the Chapter `title`. An Arriving
     * Chapter can be editable, so the wait also needs the end of the arrival.
     */
    const switched = async (title: string, body: string, win = applicationWindow(frame)) => {
      const alert = () => root().querySelector('.editor-panel [role="alert"]')?.textContent;
      const arriving = () => root().querySelector("[data-arrival-no-open-suggestion]")
        ?.getAttribute("data-arrival-no-open-suggestion");
      await expect.poll(() => ({ ...shown(manuscriptEditor(root(), win)), alert: alert(), arriving: arriving() }),
        { timeout: SWITCH_WAIT }).toEqual({ title, body, editable: true, alert: undefined, arriving: "" });
      await waitSaved(root(), undefined, { timeout: SWITCH_WAIT });
    };

    // Chapter B has no copy: the editor of Chapter A ends with its focus, and the earlier text stays read-only.
    const editorA = manuscriptEditor(root(), win);
    await updateAuthorEditSubmissionHold({ action: "hold" });
    focusManuscriptEnd(editorA, win);
    await applyTrustedInput({ operation: "insert_text", text: " one" });
    await expect.poll(() => manuscriptBody(editorA)).toBe("Alpha one");
    // A click of the author on a button of the contents moves the focus to that button.
    chapterButton(root(), "Chapter B")?.focus();
    await selectTreeChapter(root(), b, { arrival: "in-progress" });
    const earlier = () => ({ ...shown(manuscriptEditor(root(), win)), earlierEditor: editorA.isConnected,
      focusInEditor: editorA.ownerDocument.activeElement?.closest("[data-manuscript-editor]") != null });
    await expect.poll(earlier).toEqual({ title: "Chapter A", body: "Alpha one", editable: false,
      earlierEditor: false, focusInEditor: false });
    // A key press after the selection, while the save waits, goes where the focus is. It changes no text of Chapter A:
    // in the editor of Chapter A, a Backspace removes the last character.
    await applyTrustedInput({ operation: "backspace" });
    const atSelection = earlier();
    await updateAuthorEditSubmissionHold({ action: "release" });
    await switched("Chapter B", "");

    // Chapter A has the copy of its settled base: it is on the screen at once, and the editor of Chapter B ends. The
    // copy has the fact "no open suggestion", so the Arriving Chapter is editable (ADR 0105, section 3).
    const editorB = manuscriptEditor(root(), win);
    await updateAuthorEditSubmissionHold({ action: "hold" });
    focusManuscriptEnd(editorB, win);
    await applyTrustedInput({ operation: "insert_text", text: "Beta" });
    await expect.poll(() => manuscriptBody(editorB)).toBe("Beta");
    await selectTreeChapter(root(), a, { arrival: "in-progress" });
    const arriving = () => ({ ...shown(manuscriptEditor(root(), win)), earlier: editorB.isConnected });
    await expect.poll(arriving).toEqual({ title: "Chapter A", body: "Alpha one", editable: true, earlier: false });
    // Keys at once in the Arriving Chapter, while the quiet step of Chapter B waits for its save: text, a new
    // paragraph, a deletion that joins two paragraphs, and a paste of plain text of two paragraphs.
    const arrivingA = manuscriptEditor(root(), win);
    focusManuscriptEnd(arrivingA, win);
    await applyTrustedInput({ operation: "insert_text", text: " two" });
    await applyTrustedInput({ operation: "enter" });
    await applyTrustedInput({ operation: "insert_text", text: "Gamma" });
    await expect.poll(() => manuscriptBody(arrivingA)).toBe("Alpha one two\nGamma");
    caretAtStart(arrivingA, 1, win);
    await applyTrustedInput({ operation: "backspace" });
    await expect.poll(() => manuscriptBody(arrivingA)).toBe("Alpha one twoGamma");
    await updateClipboardPermission({ action: "grant" });
    await win.navigator.clipboard.writeText("Delta\nEpsilon");
    focusManuscriptEnd(arrivingA, win);
    await applyTrustedInput({ operation: "paste" });
    await expect.poll(() => manuscriptBody(arrivingA)).toBe("Alpha one twoGammaDelta\nEpsilon");
    await updateAuthorEditSubmissionHold({ action: "release" });
    // The hand-over gives the Arrival Input to the editor of Chapter A, and input after it is normal input.
    await switched("Chapter A", "Alpha one twoGammaDelta\nEpsilon");
    await typeIntoCurrent(frame, " Zeta", "Alpha one twoGammaDelta\nEpsilon Zeta");

    const reloaded = await reloadProject(frame, projectId);
    const loaded = applicationWindow(frame);
    const afterLoad = manuscriptBody(manuscriptEditor(reloaded, loaded));

    // Three fast selections with no Arrival Input: Chapter B, Chapter A, and Chapter C. The command of Chapter B is in
    // flight at the second and the third selection, so the command of Chapter C expects Chapter B. A command that
    // expects another Chapter gets the conflict `stale_current_chapter` and opens no Chapter C.
    await updateAuthorEditSubmissionHold({ action: "hold_set_current_chapter" });
    await selectTreeChapter(root(), b, { arrival: "in-progress" });
    await expect.poll(() => loaded.document.documentElement.dataset.storyosHeldRequests, { timeout: SWITCH_WAIT })
      .toBe("1");
    await selectTreeChapter(root(), a, { arrival: "in-progress" });
    await selectTreeChapter(root(), c, { arrival: "in-progress" });
    // The contents mark the newest selection while the command of Chapter B waits.
    await expect.poll(() => chapterButton(root(), "Chapter C")?.getAttribute("aria-current")).toBe("true");
    await updateAuthorEditSubmissionHold({ action: "release" });
    await switched("Chapter C", "", loaded);

    // A second selection with Arrival Input waits until the hand-over ended. Then its quiet step waits for the save of
    // the input in Chapter A, and the tab goes on to Chapter B (ADR 0105, section 6). The command to Chapter A expects
    // the Revision of Chapter A before that save.
    const headA = await revisionOf(loaded, projectId, a);
    await updateAuthorEditSubmissionHold({ action: "hold_set_current_chapter" });
    await selectTreeChapter(root(), a, { arrival: "in-progress" });
    // The count of held requests of this page includes the command of Chapter B above.
    await expect.poll(() => loaded.document.documentElement.dataset.storyosHeldRequests, { timeout: SWITCH_WAIT })
      .toBe("2");
    await expect.poll(() => shown(manuscriptEditor(root(), loaded))).toEqual({ title: "Chapter A",
      body: "Alpha one twoGammaDelta\nEpsilon Zeta", editable: true });
    focusManuscriptEnd(manuscriptEditor(root(), loaded), loaded);
    await applyTrustedInput({ operation: "insert_text", text: " Eta" });
    await selectTreeChapter(root(), b, { arrival: "in-progress" });
    const waiting = { ...shown(manuscriptEditor(root(), loaded)),
      marked: chapterButton(root(), "Chapter A")?.getAttribute("aria-current") };
    await updateAuthorEditSubmissionHold({ action: "release" });
    await switched("Chapter B", "Beta", loaded);
    const lastLoad = await reloadProject(frame, projectId);
    const last = applicationWindow(frame);
    expect({ atSelection, afterLoad, waiting, last: shown(manuscriptEditor(lastLoad, last)),
      storedA: await storedText(last, projectId, a), storedB: await storedText(last, projectId, b),
      storedC: await storedText(last, projectId, c) }).toEqual({
      atSelection: { title: "Chapter A", body: "Alpha one", editable: false, earlierEditor: false,
        focusInEditor: false },
      afterLoad: "Alpha one twoGammaDelta\nEpsilon Zeta",
      waiting: { title: "Chapter A", body: "Alpha one twoGammaDelta\nEpsilon Zeta Eta", editable: true,
        marked: "true" },
      last: { title: "Chapter B", body: "Beta", editable: true },
      storedA: ["Alpha one twoGammaDelta", "Epsilon Zeta Eta"], storedB: ["Beta"], storedC: [""] });
    // The Server stores a Receipt for each Set Current Chapter command, also for a conflict. The journey sends six:
    // A to B and B to A, then A to B, B to C, C to A, and A to B. A stale command or a command of a replaced selection
    // is a seventh one. The heads of a Receipt are the Revision of its Chapter, so the last two Receipts show C to A
    // and A to B, applied.
    await assertPageProjectReceipts({ projectId, kinds: "listed", receipts: { setCurrentChapter: 6 },
      lastResults: [{ result: "authoritative_applied", heads: [headA] },
        { result: "authoritative_applied", heads: [await revisionOf(last, projectId, b)] }] });

    // After the new load, the tab has no copy of Chapter C. The switch reads its open suggestions together with its
    // Chapter read, so the Arriving Chapter accepts a key while the command waits (#1934).
    await updateAuthorEditSubmissionHold({ action: "hold_set_current_chapter" });
    await selectTreeChapter(root(), c, { arrival: "in-progress" });
    await expect.poll(() => ({ ...shown(manuscriptEditor(root(), last)), held: last.document.documentElement.dataset
      .storyosHeldRequests }), { timeout: SWITCH_WAIT }).toEqual({ title: "Chapter C", body: "", editable: true,
      held: "1" });
    focusManuscriptEnd(manuscriptEditor(root(), last), last);
    await applyTrustedInput({ operation: "insert_text", text: "Theta" });
    await updateAuthorEditSubmissionHold({ action: "release" });
    await switched("Chapter C", "Theta", last);
    const thetaLoad = await reloadProject(frame, projectId);
    const theta = applicationWindow(frame);
    const thetaStep = { shown: shown(manuscriptEditor(thetaLoad, theta)),
      stored: await storedText(theta, projectId, c) };

    // The case of the instance (#1934). A Journal of a later version does not open, so the tab opens in read-only
    // local recovery, and it shows Chapter B read-only. The display of Chapter B records the fact "no open
    // suggestion". A removal of Chapter A opens the Project again with a ready editor of Chapter C, and the Arriving
    // Chapter B accepts a key at once: the switch reads no list of Chapter B.
    const journal = (await theta.indexedDB.databases()).map((database) => database.name)
      .find((name) => name?.startsWith("storyos-local-edit-journal:") === true && name.endsWith(`:${projectId}`)) ?? "";
    const home = nextFrameLoad(frame);
    frame.src = "/";
    await home;
    (await requestResult(indexedDB.open(journal, JOURNAL_DATABASE_VERSION + 1))).close();
    const recovery = await reloadProject(frame, projectId);
    const recoveryWindow = applicationWindow(frame);
    await expect.poll(() => shown(manuscriptEditor(recovery, recoveryWindow))).toEqual({ title: "Chapter C",
      body: "Theta", editable: false });
    const listsOfB = () => recoveryWindow.performance.getEntriesByType("resource")
      .filter((entry) => entry.name.endsWith(`/chapters/${b}/open-proposals`)).length;
    await selectTreeChapter(root(), b, { arrival: "in-progress" });
    await expect.poll(() => ({ ...shown(manuscriptEditor(root(), recoveryWindow)), lists: listsOfB() }))
      .toEqual({ title: "Chapter B", body: "Beta", editable: false, lists: 1 });
    await requestResult(indexedDB.deleteDatabase(journal));
    await beginTreeAction(root(), `li[data-chapter-id="${a}"]`, "[data-delete-chapter]");
    root().querySelector<HTMLButtonElement>(`[data-delete-chapter="${a}"]`)?.click();
    await expect.poll(() => root().querySelector(`[data-confirm-delete-chapter="${a}"]`)?.tagName).toBe("BUTTON");
    root().querySelector<HTMLButtonElement>(`[data-confirm-delete-chapter="${a}"]`)?.click();
    await expect.poll(() => shown(manuscriptEditor(root(), recoveryWindow)), { timeout: SWITCH_WAIT })
      .toEqual({ title: "Chapter C", body: "Theta", editable: true });
    await updateAuthorEditSubmissionHold({ action: "hold_set_current_chapter" });
    await selectTreeChapter(root(), b, { arrival: "in-progress" });
    await expect.poll(() => ({ ...shown(manuscriptEditor(root(), recoveryWindow)), held: recoveryWindow.document
      .documentElement.dataset.storyosHeldRequests }), { timeout: SWITCH_WAIT }).toEqual({ title: "Chapter B",
      body: "Beta", editable: true, held: "1" });
    focusManuscriptEnd(manuscriptEditor(root(), recoveryWindow), recoveryWindow);
    await applyTrustedInput({ operation: "insert_text", text: " Iota" });
    const listsAtKey = listsOfB();
    await updateAuthorEditSubmissionHold({ action: "release" });
    await switched("Chapter B", "Beta Iota", recoveryWindow);
    const iotaLoad = await reloadProject(frame, projectId);
    const iota = applicationWindow(frame);
    expect({ thetaStep, listsAtKey, shown: shown(manuscriptEditor(iotaLoad, iota)),
      stored: await storedText(iota, projectId, b) }).toEqual({
      thetaStep: { shown: { title: "Chapter C", body: "Theta", editable: true }, stored: ["Theta"] },
      listsAtKey: 1, shown: { title: "Chapter B", body: "Beta Iota", editable: true }, stored: ["Beta Iota"] });
  }, 300_000);
