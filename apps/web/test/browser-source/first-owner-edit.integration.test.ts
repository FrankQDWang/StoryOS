import { Editor, type JSONContent } from "@tiptap/core";
import { afterEach, expect, it } from "vitest";

import { applyTrustedInput, updateClipboardPermission } from "../support/browser-command-client.ts";
import { readManuscriptParagraphs } from "../../src/manuscript-doc.ts";
import {
  capturedCandidateEditFromTransaction,
  capturedManuscriptEditFromTransaction,
  isStoryosHydrateTransaction,
  storyosEditorProps,
  storyosManuscriptExtensions,
} from "../../src/manuscript-tiptap-adapter.ts";
import { selectionReferences } from "../../src/selection-reference.ts";

const LEFT = "11111111-1111-4111-8111-111111111111";
const RIGHT = "22222222-2222-4222-8222-222222222222";
const FIRST = "33333333-3333-4333-8333-333333333333";
const SECOND = "44444444-4444-4444-8444-444444444444";
const CHAPTER = "55555555-5555-4555-8555-555555555555";

const candidate = (proposalId: string) => ({ proposalId, operationId: proposalId, revisionId: proposalId,
  blockId: LEFT, eligible: true, expectedHeads: [proposalId] });
const text = (value: string) => [{ type: "text", text: value }];
const paragraph = (id: string, ...content: JSONContent[]) => ({ type: "paragraph", attrs: { id }, content });

// Hello / [Candidate] / World. The candidate text is at 8 to 17.
const BLOCK: JSONContent = { type: "doc", content: [paragraph(LEFT, ...text("Hello")),
  { type: "blockProposal", attrs: candidate(FIRST), content: text("Candidate") },
  paragraph(RIGHT, ...text("World"))] };
// Guard [voice -> tone] here. The candidate text is at 8 to 12.
const INLINE: JSONContent = { type: "doc", content: [paragraph(LEFT, ...text("Guard "),
  { type: "inlineProposal", attrs: { ...candidate(FIRST), inlineFrom: 6, inlineTo: 11, sourceText: "voice" },
    content: text("tone") }, ...text(" here"))] };
// Hello / [One] / [Two]. The two candidates are at 8 to 11 and 13 to 16.
const TWO: JSONContent = { type: "doc", content: [paragraph(LEFT, ...text("Hello")),
  { type: "blockProposal", attrs: candidate(FIRST), content: text("One") },
  { type: "blockProposal", attrs: candidate(SECOND), content: text("Two") }] };

let editor: Editor | undefined;

afterEach(async () => {
  editor?.destroy();
  editor = undefined;
  document.body.replaceChildren();
  await updateClipboardPermission({ action: "clear" });
});

function open(content: JSONContent, captured: string[]): Editor {
  const host = document.createElement("div");
  document.body.append(host);
  editor = new Editor({ element: host, injectCSS: false, content,
    extensions: storyosManuscriptExtensions(LEFT, undefined, () => true),
    editorProps: storyosEditorProps(LEFT),
    onTransaction({ transaction }) {
      if (isStoryosHydrateTransaction(transaction) || !transaction.docChanged) return;
      const chapter = capturedManuscriptEditFromTransaction(transaction);
      const proposal = capturedCandidateEditFromTransaction(transaction);
      captured.push(chapter !== undefined ? `chapter ${chapter.kind}`
        : proposal !== undefined ? `candidate ${proposal.proposal.proposalId}` : "other");
    },
  });
  return editor;
}

/** Applies one trusted input to the selection from `anchor` to `head` and returns what changed. */
async function edit(content: JSONContent, anchor: number, head: number, input: () => Promise<unknown>) {
  const captured: string[] = [];
  const current = open(content, captured);
  current.commands.setTextSelection({ from: anchor, to: head });
  current.view.focus();
  await input();
  const candidates: string[] = [];
  current.state.doc.descendants((node) => {
    if (node.type.name === "blockProposal" || node.type.name === "inlineProposal") candidates.push(node.textContent);
  });
  return { chapter: readManuscriptParagraphs(current.state.doc)?.map((block) => block.text), candidates,
    selection: { anchor: current.state.selection.anchor, head: current.state.selection.head }, captured };
}

const type = (value: string) => () => applyTrustedInput({ operation: "insert_text", text: value });

it.each([
  { name: "typing from Chapter text into a candidate changes only the Chapter text", anchor: 3, head: 12,
    input: type("X"), chapter: ["HeX", "World"], candidates: ["Candidate"], caret: 4, captured: "chapter" },
  { name: "typing from a candidate into Chapter text changes only the candidate", anchor: 12, head: 21,
    input: type("X"), chapter: ["Hello", "World"], candidates: ["CandX"], caret: 13, captured: "candidate" },
  { name: "a backward selection from the next Chapter block finds the first owner in document order",
    anchor: 21, head: 3, input: type("X"), chapter: ["HeX", "World"], candidates: ["Candidate"], caret: 4,
    captured: "chapter" },
  { name: "paste from a candidate into Chapter text changes only the candidate", anchor: 12, head: 21,
    input: async () => {
      await updateClipboardPermission({ action: "grant" });
      await navigator.clipboard.writeText("Pasted");
      await applyTrustedInput({ operation: "paste" });
    }, chapter: ["Hello", "World"], candidates: ["CandPasted"], caret: 18, captured: "candidate" },
  { name: "Backspace from Chapter text into a candidate deletes only the Chapter text", anchor: 3, head: 12,
    input: () => applyTrustedInput({ operation: "backspace" }), chapter: ["He", "World"],
    candidates: ["Candidate"], caret: 3, captured: "chapter" },
  { name: "Delete from a candidate into Chapter text deletes only the candidate text", anchor: 12, head: 21,
    input: () => applyTrustedInput({ operation: "delete" }), chapter: ["Hello", "World"],
    candidates: ["Cand"], caret: 12, captured: "candidate" },
  { name: "Enter from a candidate into Chapter text breaks only the candidate line", anchor: 12, head: 21,
    input: () => applyTrustedInput({ operation: "enter" }), chapter: ["Hello", "World"],
    candidates: ["Cand\n"], caret: 13, captured: "candidate" },
])("$name", async ({ anchor, head, input, chapter, candidates, caret, captured }) => {
  expect(await edit(BLOCK, anchor, head, input)).toEqual({ chapter, candidates,
    selection: { anchor: caret, head: caret },
    captured: [captured === "chapter" ? "chapter replace_block_selection" : `candidate ${FIRST}`] });
});

it("cut from Chapter text into a candidate removes and copies only the Chapter text", async () => {
  await updateClipboardPermission({ action: "grant" });
  await navigator.clipboard.writeText("");
  const { selection, ...changed } = await edit(BLOCK, 3, 12, () => applyTrustedInput({ operation: "cut" }));
  expect({ changed, empty: selection.anchor === selection.head }).toEqual({ changed: { chapter: ["He", "World"],
    candidates: ["Candidate"], captured: ["chapter replace_block_selection"] }, empty: true });
  expect(await navigator.clipboard.readText()).toBe("llo");
});

it.each([
  { name: "Chapter text", anchor: 3, head: 10, chapter: ["GuXvoice here"], candidates: ["tone"], caret: 4,
    captured: "chapter replace_block_selection" },
  { name: "the candidate", anchor: 10, head: 16, chapter: ["Guard voice here"], candidates: ["toX"], caret: 11,
    captured: `candidate ${FIRST}` },
])("typing over an inline candidate changes only $name when it is first", async ({ anchor, head, chapter,
  candidates, caret, captured }) => {
  expect(await edit(INLINE, anchor, head, type("X"))).toEqual({ chapter, candidates,
    selection: { anchor: caret, head: caret }, captured: [captured] });
});

it("typing over two candidates changes only the first candidate", async () => {
  expect(await edit(TWO, 9, 14, type("X"))).toEqual({ chapter: ["Hello"], candidates: ["OX", "Two"],
    selection: { anchor: 10, head: 10 }, captured: [`candidate ${FIRST}`] });
});

it("a crossing selection keeps both references for the Agent, and an edit changes only its first owner", async () => {
  const captured: string[] = [];
  const current = open(BLOCK, captured);
  const references = selectionReferences(current.state.doc, { from: 3, to: 21 }, CHAPTER, FIRST)
    .map(({ reference }) => reference);
  expect(references).toEqual([
    { kind: "chapter_text", chapter_id: CHAPTER, base_authoritative_revision_id: FIRST,
      coordinate_profile: "storyos.editor.utf16-code-unit.v1",
      start: { manuscript_block_id: LEFT, offset: 2 }, end: { manuscript_block_id: LEFT, offset: 5 } },
    { kind: "proposal_candidate_text", chapter_id: CHAPTER, proposal_id: FIRST, operation_id: FIRST,
      revision_id: FIRST, coordinate_profile: "storyos.editor.utf16-code-unit.v1", start_offset: 0, end_offset: 9 },
    { kind: "chapter_text", chapter_id: CHAPTER, base_authoritative_revision_id: FIRST,
      coordinate_profile: "storyos.editor.utf16-code-unit.v1",
      start: { manuscript_block_id: RIGHT, offset: 0 }, end: { manuscript_block_id: RIGHT, offset: 2 } },
  ]);
  current.commands.setTextSelection({ from: 3, to: 21 });
  current.view.focus();
  await applyTrustedInput({ operation: "insert_text", text: "X" });
  expect({ chapter: readManuscriptParagraphs(current.state.doc)?.map((block) => block.text),
    candidate: selectionReferences(current.state.doc, { from: 0, to: current.state.doc.content.size }, CHAPTER, FIRST)
      .find(({ reference }) => reference.kind === "proposal_candidate_text")?.reference, captured })
    .toEqual({ chapter: ["HeX", "World"], candidate: references[1], captured: ["chapter replace_block_selection"] });
});
