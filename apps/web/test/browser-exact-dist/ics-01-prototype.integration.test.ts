// ICS-01 prototype journey. Throwaway code: it does not merge. Each case prints one JSON line "ICS-EVIDENCE".
import { afterEach, expect, it } from "vitest";
import { commands } from "vitest/browser";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  applyTrustedInput, updateClientSessionCookie, updateClipboardPermission,
} from "../support/browser-command-client.ts";
import { selectTreeChapter } from "../support/inline-chapter-creation.ts";
import {
  focusManuscriptEnd, manuscriptBody, manuscriptEditor, manuscriptIsEditable, MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";
import {
  appRoot, applicationWindow, chapterButton, closeApplicationFrame, openApplicationFrame, openFirstChapterForm,
  reloadProject, submitChapterForm, submitProjectForm, submitVolumeForm, typeIntoCurrent, waitLastChapter, waitSaved,
} from "../support/exact-dist-frame.ts";

type Win = Window & typeof globalThis;
type RequestRecord = { method: string; url: string; body: string | undefined; at: number; status?: number; end?: number };

afterEach(async () => {
  await holdCurrentChapter("release");
  await updateClipboardPermission({ action: "clear" });
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

async function holdCurrentChapter(action: "hold" | "release"): Promise<void> {
  const command = Reflect.get(commands, "storyosIcsCurrentChapterHold") as (request: unknown) => Promise<unknown>;
  await command({ action });
}

async function imeAtCaret(text: string): Promise<void> {
  const command = Reflect.get(commands, "storyosIcsImeAtCaret") as (request: unknown) => Promise<unknown>;
  await command({ text });
}

function evidence(name: string, value: unknown): void {
  console.log(`ICS-EVIDENCE ${JSON.stringify({ name, value })}`);
}

function requests(win: Win): RequestRecord[] {
  return (Reflect.get(win, "__icsRequests") as RequestRecord[] | undefined) ?? [];
}

function icsLog(win: Win): { name: string; at: number; detail: unknown }[] {
  return (Reflect.get(win, "__icsLog") as { name: string; at: number; detail: unknown }[] | undefined) ?? [];
}

/** The Author Edit commands after `from`: the origin and the primitives of each unit, and the response status. */
function authorEdits(win: Win, from: number) {
  return requests(win).slice(from)
    .filter((record) => record.method === "POST" && /\/manuscript\/author-edits$/u.test(new URL(record.url).pathname))
    .map((record) => {
      const units: string[][] = [];
      const walk = (value: unknown): void => {
        if (Array.isArray(value)) { value.forEach(walk); return; }
        if (typeof value !== "object" || value === null) return;
        for (const [key, item] of Object.entries(value)) {
          if (key === "normalized_primitives" && Array.isArray(item)) {
            units.push(item.map((primitive) => String((primitive as { kind?: unknown }).kind)));
          }
          walk(item);
        }
      };
      walk(JSON.parse(record.body ?? "{}"));
      const effect = String(Reflect.get(record, "effect"));
      const reason = String(Reflect.get(record, "reason"));
      return { status: record.status, effect, reason, units };
    });
}

async function project(frame: HTMLIFrameElement, title: string) {
  await submitProjectForm(frame, title);
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  for (const name of ["Chapter A", "Chapter B"] as const) {
    await submitChapterForm(frame, name);
    await waitLastChapter(frame, name);
  }
  const root = appRoot(frame);
  const projectId = root.querySelector("[data-project-id]")?.getAttribute("data-project-id") ?? "";
  const a = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id") ?? "";
  const b = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id") ?? "";
  expect([projectId, a, b].every((value) => value.length > 0)).toBe(true);
  return { projectId, a, b };
}

async function waitArrivalEditable(frame: HTMLIFrameElement, heading: string): Promise<HTMLElement> {
  await expect.poll(() => {
    const root = appRoot(frame);
    const editor = root.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return root.querySelector("h2")?.textContent === heading && editor !== null && manuscriptIsEditable(editor);
  }, { timeout: 15_000, interval: 5 }).toBe(true);
  return manuscriptEditor(appRoot(frame), applicationWindow(frame));
}

/** Puts the caret at `offset` of paragraph `index` of the editor. */
function caretAt(editor: HTMLElement, win: Win, index: number, offset: number): void {
  editor.focus();
  const paragraph = editor.querySelectorAll(":scope > p")[index];
  if (paragraph === undefined) throw new Error("the paragraph is missing");
  const range = editor.ownerDocument.createRange();
  const text = paragraph.firstChild;
  if (text instanceof win.Text) range.setStart(text, offset);
  else range.setStart(paragraph, 0);
  range.collapse(true);
  const selection = win.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range);
}

/** The paragraph index and the text offset of the caret, and if the editor has the focus. */
function caret(editor: HTMLElement, win: Win) {
  const selection = win.getSelection();
  const node = selection?.focusNode ?? null;
  const paragraphs = [...editor.querySelectorAll(":scope > p")];
  const index = paragraphs.findIndex((paragraph) => node !== null && paragraph.contains(node));
  let offset = selection?.focusOffset ?? -1;
  if (node !== null && node.nodeType !== win.Node.TEXT_NODE) offset = node.textContent?.length ?? 0;
  return { paragraph: index, offset, focused: editor.ownerDocument.activeElement === editor,
    collapsed: selection?.isCollapsed ?? false };
}

async function storedText(win: Win, projectId: string, chapterId: string): Promise<string[]> {
  const read = await getChapter({ baseUrl: win.location.origin, projectId, chapterId, fetchImpl: win.fetch.bind(win) });
  return read.chapter.current_revision.blocks.map((block) => block.text);
}

/** The Arrival Input of question 2, typed in the Arriving Chapter while the switch is held. The composition stays open. */
async function typeArrivalInput(frame: HTMLIFrameElement, editor: HTMLElement, win: Win): Promise<void> {
  focusManuscriptEnd(editor, win);
  // Typed text in one paragraph.
  await applyTrustedInput({ operation: "insert_text", text: "Beta" });
  // A new paragraph.
  await applyTrustedInput({ operation: "enter" });
  await applyTrustedInput({ operation: "insert_text", text: "Gamma" });
  await expect.poll(() => manuscriptBody(editor)).toBe("Beta\nGamma");
  // A deletion that joins two paragraphs.
  caretAt(editor, win, 1, 0);
  await applyTrustedInput({ operation: "backspace" });
  await expect.poll(() => manuscriptBody(editor)).toBe("BetaGamma");
  // A paste of plain text of two paragraphs.
  focusManuscriptEnd(editor, win);
  await updateClipboardPermission({ action: "grant" });
  await win.navigator.clipboard.writeText("Delta\nEpsilon");
  focusManuscriptEnd(editor, win);
  await applyTrustedInput({ operation: "paste" });
  await expect.poll(() => manuscriptBody(editor)).toBe("BetaGammaDelta\nEpsilon");
  for (let line = 0; line < 12; line += 1) {
    await applyTrustedInput({ operation: "enter" });
    await applyTrustedInput({ operation: "insert_text", text: `Line ${line}` });
  }
  // Chinese text from an IME composition: the composition opens here and stays open.
  focusManuscriptEnd(editor, win);
  const end = manuscriptBody(editor).length;
  await imeAtCaret("zhong");
  await expect.poll(() => manuscriptBody(editor).endsWith("Line 11zhong")).toBe(true).catch((error: unknown) => {
    evidence("ime-open-failed", { end, body: manuscriptBody(editor), html: editor.innerHTML.slice(-400) });
    throw error;
  });
}

const LINES = Array.from({ length: 12 }, (_, line) => `Line ${line}`);
const EXPECTED = ["BetaGammaDelta", "Epsilon", ...LINES.slice(0, -1), "Line 11中文"];

async function handOverRun(entry: "each" | "diff", swapDuringComposition: boolean) {
  const frame = await openApplicationFrame(`ICS-01 ${entry}`);
  frame.style.height = "420px";
  frame.style.width = "1100px";
  const ids = await project(frame, `ICS ${entry} ${swapDuringComposition ? "swap" : "wait"}`);
  await typeIntoCurrent(frame, "Alpha prose");
  const win = applicationWindow(frame);
  Reflect.set(win, "__icsEntry", entry);
  Reflect.set(win, "__icsSwapDuringComposition", swapDuringComposition);
  const fromRequest = requests(win).length;
  await holdCurrentChapter("hold");
  const clicked = win.performance.now();
  await selectTreeChapter(appRoot(frame), ids.b);
  const editor = await waitArrivalEditable(frame, "Chapter B");
  const editableAfterMs = win.performance.now() - clicked;
  await typeArrivalInput(frame, editor, win);
  const panel = appRoot(frame).querySelector<HTMLElement>(".editor-panel")!;
  panel.scrollTop = panel.scrollHeight;
  const scrollBefore = panel.scrollTop;
  const caretBefore = caret(editor, win);
  const mountsBefore = icsLog(win).filter((entry) => entry.name === "mount").length;
  await holdCurrentChapter("release");
  await expect.poll(() => icsLog(win).some((item) => item.name === (swapDuringComposition
    ? "handover-selection" : "handover-waits-for-composition")), { timeout: 15_000 }).toBe(true);
  // The switch is ready and the composition is still open: no new editor mounted yet.
  const mountsWhileComposing = icsLog(win).filter((entry) => entry.name === "mount").length;
  await expect.poll(() => icsLog(win).filter((item) => item.name === "mount").length,
    { timeout: swapDuringComposition ? 15_000 : 300 }).toBe(swapDuringComposition ? mountsBefore + 1 : mountsBefore)
    .catch(() => undefined);
  const editorWhileReady = appRoot(frame).querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  const sameEditorWhileReady = editorWhileReady === editor;
  const textWhileReady = editorWhileReady === null ? "" : manuscriptBody(editorWhileReady);
  // The editable attribute of each editor element after this point, with its time.
  const editableTrace: { at: number; editable: string | null; replaced: boolean }[] = [];
  const observer = new win.MutationObserver(() => {
    const current = appRoot(frame).querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    const value = current?.getAttribute("contenteditable") ?? null;
    const last = editableTrace.at(-1);
    if (last === undefined || last.editable !== value || (current !== editor) !== last.replaced) {
      editableTrace.push({ at: Math.round(win.performance.now()), editable: value, replaced: current !== editor });
    }
  });
  observer.observe(appRoot(frame), { subtree: true, childList: true, attributes: true,
    attributeFilter: ["contenteditable"] });
  // The author confirms the composition.
  await applyTrustedInput({ operation: "insert_text", text: "中文" }).catch((error: unknown) => {
    evidence(`${entry}-commit-error`, String(error));
  });
  await expect.poll(() => icsLog(win).filter((item) => item.name === "mount").length, { timeout: 15_000 })
    .toBeGreaterThan(mountsBefore);
  await expect.poll(() => manuscriptIsEditable(manuscriptEditor(appRoot(frame), win)), { timeout: 15_000, interval: 1 })
    .toBe(true).catch((error: unknown) => {
      evidence(`${entry}-after-swap`, { text: appRoot(frame).textContent?.slice(0, 300), log: icsLog(win) });
      throw error;
    });
  observer.disconnect();
  const handedOver = manuscriptEditor(appRoot(frame), win);
  const caretAfter = caret(handedOver, win);
  const scrollAfter = panel.scrollTop;
  const shownAfter = manuscriptBody(handedOver);
  // One more key after the hand-over goes to the controller as normal input.
  await applyTrustedInput({ operation: "insert_text", text: "!" }).catch(() => undefined);
  const root = appRoot(frame);
  await waitSaved(root, undefined, { timeout: 20_000 }).catch(() => undefined);
  const save = root.querySelector("[data-save-state]");
  const shown = manuscriptBody(manuscriptEditor(root, win));
  const edits = authorEdits(win, fromRequest);
  const stored = await storedText(win, ids.projectId, ids.b);
  const log = icsLog(win).filter((item) => item.name !== "arrival-capture")
    .map((item) => item.name.startsWith("handover") ? { [item.name]: item.detail } : item.name);
  const reloaded = await reloadProject(frame, ids.projectId);
  const afterReload = manuscriptBody(manuscriptEditor(reloaded, applicationWindow(frame)));
  const result = { entry, swapDuringComposition, editableTrace, editableAfterMs: Math.round(editableAfterMs), scrollBefore,
    scrollAfter, caretBefore, caretAfter, mountsBefore, mountsWhileComposing, sameEditorWhileReady, textWhileReady,
    shownAfter, shown, saveState: save?.getAttribute("data-save-state"),
    failure: save?.getAttribute("data-editor-failure"), stored, afterReload, edits, log };
  evidence(`handover-${entry}-${swapDuringComposition ? "swap" : "wait"}`, result);
  return result;
}

it("Q1/Q2: the hand-over after the composition keeps the composition, the caret, and the scroll (entry: each)", async () => {
  const result = await handOverRun("each", false);
  expect(result.sameEditorWhileReady).toBe(true);
  expect(result.textWhileReady.endsWith("Line 11zhong")).toBe(true);
  expect(result.shownAfter.split("\n")).toEqual(EXPECTED);
  expect(result.caretAfter).toEqual({ paragraph: EXPECTED.length - 1, offset: "Line 11中文".length, focused: true,
    collapsed: true });
  expect(result.scrollAfter).toBe(result.scrollBefore);
  expect(result.stored).toEqual([...EXPECTED.slice(0, -1), "Line 11中文!"]);
  expect(result.afterReload).toBe([...EXPECTED.slice(0, -1), "Line 11中文!"].join("\n"));
  expect(result.edits.every((edit) => edit.status === 200 && edit.effect === "authoritative_applied")).toBe(true);
});

it("Q2: the entry of one difference gives command shapes that normal input does not give (entry: diff)", async () => {
  const result = await handOverRun("diff", false);
  expect(result.stored.length).toBeGreaterThan(0);
});

it("Q1: a swap of the editor during an open composition loses the composition", async () => {
  const result = await handOverRun("each", true);
  expect(result.stored.length).toBeGreaterThan(0);
});

it("Q1/Q4: a return to a Chapter with a copy is editable at once, and the earlier Chapter keeps its journaled input", async () => {
  const frame = await openApplicationFrame("ICS-01 copy");
  const ids = await project(frame, "ICS copy");
  await typeIntoCurrent(frame, "Alpha prose");
  const win = applicationWindow(frame);
  // The first switch to B writes the copy of A at the quiet step.
  await selectTreeChapter(appRoot(frame), ids.b);
  await waitArrivalEditable(frame, "Chapter B");
  await expect.poll(() => icsLog(win).filter((item) => item.name === "mount").length, { timeout: 15_000 })
    .toBeGreaterThanOrEqual(2);
  await waitSaved(appRoot(frame), undefined, { timeout: 15_000 });
  // Journaled input of B that is not submitted yet: the click comes inside the idle window of the batch.
  const editorB = manuscriptEditor(appRoot(frame), win);
  focusManuscriptEnd(editorB, win);
  const fromRequest = requests(win).length;
  await applyTrustedInput({ operation: "insert_text", text: "Beta journaled" });
  await holdCurrentChapter("hold");
  const clicked = win.performance.now();
  await selectTreeChapter(appRoot(frame), ids.a);
  const editorA = await waitArrivalEditable(frame, "Chapter A");
  const editableAfterMs = win.performance.now() - clicked;
  const earlierEditorGone = !editorB.isConnected;
  const shownA = manuscriptBody(editorA);
  focusManuscriptEnd(editorA, win);
  await applyTrustedInput({ operation: "insert_text", text: " plus" });
  await holdCurrentChapter("release");
  await expect.poll(() => icsLog(win).filter((item) => item.name === "mount").length, { timeout: 15_000 })
    .toBeGreaterThanOrEqual(3);
  await waitSaved(appRoot(frame), undefined, { timeout: 20_000 });
  const order = requests(win).slice(fromRequest).filter((record) => record.method !== "GET")
    .map((record) => `${record.method} ${new URL(record.url).pathname.replace(/[0-9a-f-]{36}/gu, ":id")}`);
  const storedA = await storedText(win, ids.projectId, ids.a);
  const storedB = await storedText(win, ids.projectId, ids.b);
  const gets = requests(win).slice(fromRequest).filter((record) => record.method === "GET")
    .map((record) => ({ path: new URL(record.url).pathname.replace(/[0-9a-f-]{36}/gu, ":id"),
      ms: Math.round((record.end ?? record.at) - record.at) }));
  evidence("copy-and-earlier", { editableAfterMs: Math.round(editableAfterMs), earlierEditorGone, shownA, order,
    storedA, storedB, gets, log: icsLog(win).map((item) => item.name) });
  expect(earlierEditorGone).toBe(true);
  expect(shownA).toBe("Alpha prose");
  expect(storedB).toEqual(["Beta journaled"]);
  expect(storedA).toEqual(["Alpha prose plus"]);
});

it.each(["each", "diff"] as const)("Q2: Arrival Input in two separated paragraphs (entry: %s)", async (entry) => {
  const frame = await openApplicationFrame(`ICS-01 two places ${entry}`);
  const ids = await project(frame, `ICS two places ${entry}`);
  const win = applicationWindow(frame);
  const editorA = manuscriptEditor(appRoot(frame), win);
  await updateClipboardPermission({ action: "grant" });
  await win.navigator.clipboard.writeText("P1\nP2\nP3");
  focusManuscriptEnd(editorA, win);
  await applyTrustedInput({ operation: "paste" });
  await expect.poll(() => manuscriptBody(editorA)).toBe("P1\nP2\nP3");
  await waitSaved(appRoot(frame), undefined, { timeout: 15_000 });
  await selectTreeChapter(appRoot(frame), ids.b);
  await expect.poll(() => icsLog(win).filter((item) => item.name === "mount").length, { timeout: 15_000 })
    .toBeGreaterThanOrEqual(2);
  await waitSaved(appRoot(frame), undefined, { timeout: 15_000 });
  Reflect.set(win, "__icsEntry", entry);
  const fromRequest = requests(win).length;
  await holdCurrentChapter("hold");
  await selectTreeChapter(appRoot(frame), ids.a);
  const arriving = await waitArrivalEditable(frame, "Chapter A");
  caretAt(arriving, win, 0, 2);
  await applyTrustedInput({ operation: "insert_text", text: "X" });
  caretAt(arriving, win, 2, 2);
  await applyTrustedInput({ operation: "insert_text", text: "Y" });
  await expect.poll(() => manuscriptBody(arriving)).toBe("P1X\nP2\nP3Y");
  await holdCurrentChapter("release");
  await expect.poll(() => icsLog(win).filter((item) => item.name === "mount").length, { timeout: 15_000 })
    .toBeGreaterThanOrEqual(3);
  await waitSaved(appRoot(frame), undefined, { timeout: 20_000 }).catch(() => undefined);
  const shown = manuscriptBody(manuscriptEditor(appRoot(frame), win));
  const stored = await storedText(win, ids.projectId, ids.a);
  evidence(`two-places-${entry}`, { shown, stored, edits: authorEdits(win, fromRequest),
    handOver: icsLog(win).filter((item) => item.name.startsWith("handover")).map((item) => item.detail) });
  if (entry === "each") expect(stored).toEqual(["P1X", "P2", "P3Y"]);
  else expect(stored).toEqual(["P1", "P2", "P3"]);
});

it("Q6: a hand-over of 240 held operations and one key at once", async () => {
  const frame = await openApplicationFrame("ICS-01 bound");
  const ids = await project(frame, "ICS bound");
  const win = applicationWindow(frame);
  await holdCurrentChapter("hold");
  await selectTreeChapter(appRoot(frame), ids.b);
  const arriving = await waitArrivalEditable(frame, "Chapter B");
  focusManuscriptEnd(arriving, win);
  for (let key = 0; key < 240; key += 1) await applyTrustedInput({ operation: "insert_text", text: "k" });
  const mounts = icsLog(win).filter((item) => item.name === "mount").length;
  await holdCurrentChapter("release");
  await expect.poll(() => icsLog(win).filter((item) => item.name === "mount").length, { timeout: 15_000, interval: 1 })
    .toBeGreaterThan(mounts);
  await applyTrustedInput({ operation: "insert_text", text: "!" }).catch(() => undefined);
  await waitSaved(appRoot(frame), undefined, { timeout: 30_000 }).catch(() => undefined);
  const save = appRoot(frame).querySelector("[data-save-state]");
  const alert = appRoot(frame).querySelector("[role=alert]")?.textContent ?? "";
  const stored = await storedText(win, ids.projectId, ids.b);
  evidence("bound-240", { saveState: save?.getAttribute("data-save-state"),
    failure: save?.getAttribute("data-editor-failure"), alert, storedLength: stored.join("").length,
    shownLength: manuscriptBody(manuscriptEditor(appRoot(frame), win)).length });
});
