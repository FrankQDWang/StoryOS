import { Editor } from "@tiptap/core";
import { afterEach, beforeAll, expect, it } from "vitest";
import { userEvent } from "vitest/browser";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";

import { projectBlockProposals, type BlockProposalProjection } from "../../src/block-proposal-decoration.ts";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import { readJournalSnapshot } from "../../src/local-edit-journal.ts";
import { manuscriptJson } from "../../src/manuscript-doc.ts";
import { ManuscriptEditor } from "../../src/manuscript-editor.tsx";
import {
  capturedCandidateEditFromTransaction, storyosEditorProps, storyosManuscriptExtensions,
} from "../../src/manuscript-tiptap-adapter.ts";
import { proposalChangeRuns } from "../../src/proposal-change-runs.ts";
import { selectionReferences } from "../../src/selection-reference.ts";
import {
  applyImeComposition, applyTrustedInput, updateClipboardPermission,
} from "../support/browser-command-client.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { candidateFor, proposalTransport, renderTab, withActEnvironment } from "./proposal-display-harness.ts";
import { ROW_1, shownCandidate } from "../support/proposal-change-shown.ts";
import { BLOCK, CHAPTER, REVISION } from "./scenario.ts";

const PROPOSAL = "018f0000-0000-7001-8000-0000000000e1";
const editors: Editor[] = [];

beforeAll(async () => {
  const styles = Object.assign(document.createElement("link"), { rel: "stylesheet", href: "/src/writing-workspace.css" });
  const loaded = new Promise((resolve, reject) => { styles.onload = resolve; styles.onerror = reject; });
  document.head.append(styles);
  await loaded;
});

afterEach(() => {
  for (const editor of editors.splice(0)) {
    const host = editor.options.element;
    editor.destroy();
    if (host instanceof HTMLElement) host.remove();
  }
});

function candidate(text: string, overrides: Partial<BlockProposalProjection> = {}): BlockProposalProjection {
  return { proposalId: PROPOSAL, operationId: PROPOSAL, revisionId: PROPOSAL, blockId: BLOCK, sourceRunId: PROPOSAL,
    sourceDecisionId: PROPOSAL, text, eligible: true, expectedHeads: [PROPOSAL], ...overrides };
}

function openEditor(blockText: string, proposals: readonly BlockProposalProjection[]): Editor {
  const host = document.createElement("div");
  document.body.append(host);
  const editor = new Editor({ element: host, extensions: storyosManuscriptExtensions(BLOCK, undefined, () => true),
    content: manuscriptJson(BLOCK, blockText), injectCSS: false, editorProps: storyosEditorProps(BLOCK) });
  editors.push(editor);
  projectBlockProposals(editor, proposals);
  return editor;
}

function candidateText(editor: Editor): HTMLElement {
  return editor.view.dom.querySelector<HTMLElement>(".block-proposal .block-proposal-text")!;
}

function targetBlock(editor: Editor): HTMLElement {
  return editor.view.dom.firstElementChild as HTMLElement;
}

/** The rendered candidate in the notation of the fixture table: `【…】` for a mark and `▏` for a tick. */
function shownInEditor(element: Node): string {
  return Array.from(element.childNodes, (node) => {
    if (node instanceof Element && node.matches(".proposal-change-tick")) return "▏";
    if (node instanceof Element && node.matches(".proposal-change-mark")) return `【${node.textContent}】`;
    return node instanceof Element ? shownInEditor(node) : node.textContent ?? "";
  }).join("");
}

it("shows the marks and the tick of fixture row 1 and leaves the candidate text and the target Block as they are", () => {
  const editor = openEditor(ROW_1.old, [candidate(ROW_1.new)]);
  const plain = openEditor(ROW_1.old, []);
  expect({ shown: shownInEditor(candidateText(editor)), text: candidateText(editor).textContent,
    target: targetBlock(editor).outerHTML }).toEqual({
    shown: "他站在门口，【把钥匙从左手换到右手，说】：“妈，先借我两千。”母亲【看了他一眼，】没有问为什么，▏转身进了里屋。"
      + "他听见抽屉被拉开【，又听见她在里面翻了很久。他低头去看自己的鞋尖，鞋带松了一边】。",
    text: ROW_1.new, target: targetBlock(plain).outerHTML });
});

const PAIR_COLOUR = "rgba(52, 50, 46, 0.12)";

/** Each element of the editor with the pair background or outline, with its place, text, and colours. */
function pairs(editor: Editor) {
  return Array.from(editor.view.dom.querySelectorAll(".proposal-change-pair"), (element) => {
    const line = element.querySelector(".proposal-change-tick-line");
    return { place: element.closest(".block-proposal") === null ? "target" : "candidate", text: element.textContent,
      background: getComputedStyle(element).backgroundColor,
      outline: line === null ? "none" : `${getComputedStyle(line).outlineWidth} ${getComputedStyle(line).outlineColor}` };
  });
}

it("pairs a mark or the tick with its old words in the target Block only while the pointer is on it", async () => {
  const editor = openEditor(ROW_1.old, [candidate(ROW_1.new)]);
  const mark = () => candidateText(editor).querySelectorAll(".proposal-change-mark")[1]!;
  const tick = () => candidateText(editor).querySelector<HTMLElement>(".proposal-change-tick")!;
  const box = tick().getBoundingClientRect();
  const middle = box.top + box.height / 2;
  // A hit test takes whole pixels: the first and the last pixel of the 8 px are the tick.
  expect([box.width, document.elementFromPoint(box.left, middle)?.closest(".proposal-change-tick"),
    document.elementFromPoint(box.right - 1, middle)?.closest(".proposal-change-tick")])
    .toEqual([8, tick(), tick()]);
  await userEvent.hover(mark());
  expect(pairs(editor)).toEqual([
    { place: "target", text: "的眼中闪过一丝复杂的情绪，她", background: PAIR_COLOUR, outline: "none" },
    { place: "candidate", text: "看了他一眼，", background: PAIR_COLOUR, outline: "none" },
  ]);
  await userEvent.hover(tick());
  expect(pairs(editor)).toEqual([
    { place: "target", text: "只是默默地", background: PAIR_COLOUR, outline: "none" },
    { place: "candidate", text: "", background: "rgba(0, 0, 0, 0)", outline: `3px ${PAIR_COLOUR}` },
  ]);
  await userEvent.hover(targetBlock(editor));
  expect(pairs(editor)).toEqual([]);
  await userEvent.hover(mark());
  await userEvent.unhover(mark());
  expect({ pairs: pairs(editor), target: targetBlock(editor).outerHTML })
    .toEqual({ pairs: [], target: targetBlock(openEditor(ROW_1.old, [])).outerHTML });
});

/** The document position of an offset in the text of the target Block, or of the first candidate. */
function position(editor: Editor, place: "target" | "candidate", offset: number): number {
  let start = -1;
  editor.state.doc.forEach((node, nodeOffset) => {
    if (start < 0 && (node.type.name === "blockProposal") === (place === "candidate")) start = nodeOffset + 1;
  });
  return start + offset;
}

async function typeAt(editor: Editor, place: "target" | "candidate", offset: number, text: string): Promise<void> {
  editor.commands.setTextSelection(position(editor, place, offset));
  editor.view.focus();
  await applyTrustedInput({ operation: "insert_text", text });
}

/** The marks that the rule gives for the current texts of the target Block and of the candidate. */
function shownByRule(editor: Editor): string {
  const old = targetBlock(editor).textContent!;
  const current = candidateText(editor).textContent!;
  return shownCandidate(current, proposalChangeRuns(old, current));
}

it("marks a word that the author types into an editable candidate and captures the same candidate edit", async () => {
  const editor = openEditor(ROW_1.old, [candidate(ROW_1.new)]);
  const captured: unknown[] = [];
  editor.on("transaction", ({ transaction }) => {
    const edit = capturedCandidateEditFromTransaction(transaction);
    if (edit !== undefined) captured.push(edit);
  });
  await typeAt(editor, "candidate", 40, "他");
  const resultingBody = `${ROW_1.new.slice(0, 40)}他${ROW_1.new.slice(40)}`;
  expect({ shown: shownInEditor(candidateText(editor)), captured }).toEqual({
    shown: "他站在门口，【把钥匙从左手换到右手，说】：“妈，先借我两千。”母亲【看了他一眼，】没有问【他】为什么，▏转身进了里屋。"
      + "他听见抽屉被拉开【，又听见她在里面翻了很久。他低头去看自己的鞋尖，鞋带松了一边】。",
    captured: [{
      proposal: { proposalId: PROPOSAL, operationId: PROPOSAL, revisionId: PROPOSAL, blockId: BLOCK,
        sourceRunId: PROPOSAL, sourceDecisionId: PROPOSAL, text: resultingBody, eligible: true, retryPending: false,
        rejectEligible: false, retryRejection: false, earlierBase: false, withdrawEligible: false,
        copyEligible: false, conditionKind: "absent", validity: "", sessionEligible: false,
        expectedHeads: [PROPOSAL] },
      priorText: ROW_1.new, from: 40, to: 40, text: "他", resultingBody,
    }],
  });
});

it("computes the marks again from the two current texts when the author changes the target Block", async () => {
  const editor = openEditor(ROW_1.old, [candidate(ROW_1.new)]);
  await typeAt(editor, "target", ROW_1.old.indexOf("母亲") + 2, "看了他一眼，");
  expect({ shown: shownInEditor(candidateText(editor)), target: targetBlock(editor).textContent }).toEqual({
    shown: "他站在门口，【把钥匙从左手换到右手，说】：“妈，先借我两千。”母亲看了他一眼，▏没有问为什么，▏转身进了里屋。"
      + "他听见抽屉被拉开【，又听见她在里面翻了很久。他低头去看自己的鞋尖，鞋带松了一边】。",
    target: ROW_1.old.replace("母亲", "母亲看了他一眼，") });
});

it("keeps the decorations during an IME composition in a candidate and computes them again after it", async () => {
  const editor = openEditor(ROW_1.old, [candidate(ROW_1.new)]);
  const decorated = () => Array.from(editor.view.dom.querySelectorAll(".proposal-change-mark, .proposal-change-tick"));
  const before = decorated();
  editor.commands.setTextSelection(position(editor, "candidate", ROW_1.new.length));
  editor.view.focus();
  // The composition offsets count the rendered text of the editor, which the selection text gives.
  const end = window.getSelection()!.getRangeAt(0);
  window.getSelection()!.setBaseAndExtent(editor.view.dom, 0, end.endContainer, end.endOffset);
  const offset = window.getSelection()!.toString().length;
  editor.commands.setTextSelection(position(editor, "candidate", ROW_1.new.length));
  await applyImeComposition({ text: "zhong", replacementStart: offset, replacementEnd: offset,
    selectionStart: 5, selectionEnd: 5 });
  const during = decorated();
  expect({ text: candidateText(editor).textContent, same: during.length === before.length
    && during.every((element, index) => element === before[index]) })
    .toEqual({ text: `${ROW_1.new}zhong`, same: true });
  await applyTrustedInput({ operation: "insert_text", text: "中" });
  await expect.poll(() => ({ text: candidateText(editor).textContent, shown: shownInEditor(candidateText(editor)) }))
    .toEqual({ text: `${ROW_1.new}中`, shown: "他站在门口，【把钥匙从左手换到右手，说】：“妈，先借我两千。”"
      + "母亲【看了他一眼，】没有问为什么，▏转身进了里屋。他听见抽屉被拉开【，又听见她在里面翻了很久。他低头去看自己的鞋尖，鞋带松了一边。中】" });
});

it("gives the same selection reference and clipboard text for a selection that crosses a mark and a tick", async () => {
  const editor = openEditor(ROW_1.old, [candidate(ROW_1.new)]);
  const mark = candidateText(editor).querySelectorAll(".proposal-change-mark")[1]!.firstChild!;
  const afterTick = candidateText(editor).querySelector(".proposal-change-tick")!.nextSibling!;
  editor.view.focus();
  window.getSelection()!.setBaseAndExtent(mark, 3, afterTick, 1);
  await expect.poll(() => [editor.state.selection.from, editor.state.selection.to])
    .toEqual([position(editor, "candidate", 34), position(editor, "candidate", 45)]);
  const clipboard = new DataTransfer();
  editor.view.dom.dispatchEvent(new ClipboardEvent("copy", { clipboardData: clipboard, bubbles: true, cancelable: true }));
  expect({ references: selectionReferences(editor.state.doc, editor.state.selection, CHAPTER, REVISION),
    clipboard: clipboard.getData("text/plain") }).toEqual({
    references: [{ reference: { kind: "proposal_candidate_text", chapter_id: CHAPTER, proposal_id: PROPOSAL,
      operation_id: PROPOSAL, revision_id: PROPOSAL, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
      start_offset: 34, end_offset: 45 }, candidate: { blockId: BLOCK } }],
    clipboard: "一眼，没有问为什么，转" });
});

it("copies the candidate text with no added character from a candidate with marks", async () => {
  const test = await openJournalAppendTestWorkspace();
  const fixture = candidateFor(test.workspace);
  const text = "Base, and the rain stopped.";
  // An invalid Proposal offers "复制", and the author cannot edit its candidate.
  const proposal = { ...fixture, candidate_text: text, validation: "invalid",
    operations: fixture.operations.map((operation) => ({ ...operation, candidate_text: text })) };
  const transport = proposalTransport(test.workspace, proposal, { proposals: [proposal], completeness: "complete" });
  await withActEnvironment(async () => {
    const tab = await renderTab(test.workspace, [{ proposalId: proposal.proposal_id, runId: proposal.source.run_id,
      decisionId: proposal.source.decision_id }], transport.fetchImpl);
    try {
      const copy = () => tab.host.querySelector<HTMLButtonElement>(`button[data-proposal-copy="${proposal.proposal_id}"]`);
      await expect.poll(copy).not.toBeNull();
      await updateClipboardPermission({ action: "grant" });
      await navigator.clipboard.writeText("");
      await act(async () => { copy()!.click(); });
      await expect.poll(() => navigator.clipboard.readText()).toBe(text);
      expect({ shown: shownInEditor(tab.host.querySelector(".block-proposal-text")!), failures: tab.failures })
        .toEqual({ shown: "Base【, and the rain stopped.】", failures: [] });
    } finally {
      await tab.unmount(); await test.close();
    }
  });
});

it.each([
  { name: "a candidate with almost all new text", old: "她笑了笑，没有说话。", text: "窗外的车一辆接一辆地过去，谁也没有再开口。" },
  { name: "a candidate of an empty target Block", old: "", text: "天亮的时候，他已经走了。" },
])("shows no mark and no tick for $name", ({ old, text }) => {
  const editor = openEditor(old, [candidate(text)]);
  expect({ shown: shownInEditor(candidateText(editor)), decorated: editor.view.dom
    .querySelectorAll(".proposal-change-mark, .proposal-change-tick, .proposal-change-pair").length })
    .toEqual({ shown: text, decorated: 0 });
});

it("marks each candidate, also one that the author cannot edit, one that is not focused, and one on an earlier base", () => {
  const second = "018f0000-0000-7001-8000-0000000000e2";
  const editor = openEditor("她的声音很轻。", [candidate("她的声音很低。", { focused: true }),
    candidate("她的声音很轻，很慢。", { proposalId: second, operationId: second, revisionId: second,
      candidateEditable: false, earlierBase: true })]);
  expect(Array.from(editor.view.dom.querySelectorAll(".block-proposal-text"), shownInEditor))
    .toEqual(["她的声音很【低】。", "她的声音很轻【，很慢】。"]);
});

it("writes the same Local Edit Journal record for an edit of a candidate with marks", async () => {
  const test = await openJournalAppendTestWorkspace();
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const block = test.workspace.openedProjection.blocks[0]!.manuscript_block_id;
  const failures: unknown[] = [];
  // The test sends no request: it examines only the Journal record.
  const requests: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    requests.push(`${init?.method ?? "GET"} ${input instanceof Request ? input.url : String(input)}`);
    throw new Error("unexpected request");
  };
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin,
    fetchImpl, onFailure: (error) => { failures.push(error); } });
  await withActEnvironment(async () => {
    try {
      await act(async () => { root.render(createElement(ManuscriptEditor, { blocks: test.workspace.openedProjection.blocks,
        proposals: [candidate("Base, and the rain stopped.", { blockId: block })], editable: true, writing })); });
      const text = host.querySelector<HTMLElement>(".block-proposal-text")!;
      expect(shownInEditor(text)).toBe("Base【, and the rain stopped.】");
      const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
      surface.focus();
      const end = text.querySelector(".proposal-change-mark")!.firstChild!;
      window.getSelection()!.setBaseAndExtent(end, 5, end, 5);
      document.dispatchEvent(new Event("selectionchange"));
      await applyTrustedInput({ operation: "insert_text", text: " soon" });
      await expect.poll(async () => (await readJournalSnapshot(test.workspace)).records.length).toBe(1);
      const [record] = (await readJournalSnapshot(test.workspace)).records;
      // The normalized primitive puts the inserted text after the equal space.
      expect({ text: text.textContent, origin: record!.input_origin,
        unit: record!.author_edit_unit?.normalized_primitives, target: record!.proposal_target, failures }).toEqual({
        text: "Base, and soon the rain stopped.", origin: "typing",
        unit: [{ kind: "replace_selection", from: 10, to: 10, text: "soon " }],
        target: { proposal_id: PROPOSAL, operation_id: PROPOSAL, revision_id: PROPOSAL, manuscript_block_id: block },
        failures: [] });
      expect(requests).toEqual([]);
    } finally {
      await act(async () => { root.unmount(); }); writing.close(); host.remove(); await test.close();
    }
  });
});

it("shows no pair for a touch tap on a mark", () => {
  const editor = openEditor(ROW_1.old, [candidate(ROW_1.new)]);
  const mark = candidateText(editor).querySelectorAll(".proposal-change-mark")[1]!;
  // A touch tap gives a touch pointer event and then compatibility mouse events.
  mark.dispatchEvent(new PointerEvent("pointerover", { pointerType: "touch", bubbles: true }));
  mark.dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));
  expect(pairs(editor)).toEqual([]);
});

it("removes the pair when the pointer leaves during a composition that changes no text", async () => {
  const editor = openEditor(ROW_1.old, [candidate(ROW_1.new)]);
  const mark = () => candidateText(editor).querySelectorAll(".proposal-change-mark")[1]!;
  await userEvent.hover(mark());
  expect(pairs(editor).length).toBe(2);
  editor.view.dom.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
  await userEvent.unhover(mark());
  expect(pairs(editor).length).toBe(2);
  editor.view.dom.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "" }));
  await expect.poll(() => pairs(editor)).toEqual([]);
});

it("computes the runs of the current texts when the two texts change together", () => {
  // The two pairs of texts give the same string when each pair is joined with U+0000.
  const editor = openEditor("他走了。\u0000她来了。", [candidate("他走了。她来了。")]);
  expect(shownInEditor(candidateText(editor))).toBe("他走了。▏她来了。");
  const transaction = editor.state.tr.insertText("他走了。", position(editor, "target", 0),
    position(editor, "target", 9));
  const start = transaction.mapping.map(position(editor, "candidate", 0));
  transaction.insertText("她来了。\u0000他走了。她来了。", start, start + 8).setMeta("storyos.hydrate", true);
  editor.view.dispatch(transaction);
  expect({ texts: [targetBlock(editor).textContent, candidateText(editor).textContent],
    shown: shownInEditor(candidateText(editor)) }).toEqual({
    texts: ["他走了。", "她来了。\u0000他走了。她来了。"],
    shown: shownCandidate("她来了。\u0000他走了。她来了。", proposalChangeRuns("他走了。", "她来了。\u0000他走了。她来了。")) });
});

it("removes only the decorations on the text that a composition replaces across a mark and a tick", async () => {
  const editor = openEditor(ROW_1.old, [candidate(ROW_1.new)]);
  const decorated = () => Array.from(candidateText(editor).querySelectorAll(".proposal-change-mark, .proposal-change-tick"));
  const [first, second] = decorated();
  const inMark = second!.firstChild!;
  const afterTick = candidateText(editor).querySelector(".proposal-change-tick")!.nextSibling!;
  editor.view.focus();
  // The composition offsets count the rendered text of the editor, which the selection text gives.
  const offset = (node: Node, at: number) => {
    window.getSelection()!.setBaseAndExtent(editor.view.dom, 0, node, at);
    return window.getSelection()!.toString().length;
  };
  const [start, end] = [offset(inMark, 3), offset(afterTick, 1)];
  window.getSelection()!.setBaseAndExtent(inMark, 3, afterTick, 1);
  await expect.poll(() => [editor.state.selection.from, editor.state.selection.to])
    .toEqual([position(editor, "candidate", 34), position(editor, "candidate", 45)]);
  await applyImeComposition({ text: "zhong", replacementStart: start, replacementEnd: end, selectionStart: 5, selectionEnd: 5 });
  const during = decorated();
  // The composed text is in the DOM of the rest of mark 2, where the browser puts it.
  expect({ text: candidateText(editor).textContent, shown: shownInEditor(candidateText(editor)),
    kept: [during[0] === first, during[1] === second] }).toEqual({
    text: `${ROW_1.new.slice(0, 34)}zhong${ROW_1.new.slice(45)}`,
    shown: "他站在门口，【把钥匙从左手换到右手，说】：“妈，先借我两千。”母亲【看了他zhong】身进了里屋。"
      + "他听见抽屉被拉开【，又听见她在里面翻了很久。他低头去看自己的鞋尖，鞋带松了一边】。", kept: [true, true] });
  await applyTrustedInput({ operation: "insert_text", text: "中" });
  await expect.poll(() => ({ text: candidateText(editor).textContent, shown: shownInEditor(candidateText(editor)) }))
    .toEqual({ text: `${ROW_1.new.slice(0, 34)}中${ROW_1.new.slice(45)}`, shown: "他站在门口，【把钥匙从左手换到右手，说】："
      + "“妈，先借我两千。”母亲【看了他中】身进了里屋。他听见抽屉被拉开【，又听见她在里面翻了很久。他低头去看自己的鞋尖，鞋带松了一边】。" });
});
