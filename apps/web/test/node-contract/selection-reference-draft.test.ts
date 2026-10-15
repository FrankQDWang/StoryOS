import assert from "node:assert/strict";
import { getSchema } from "@tiptap/core";
import { EditorState } from "@tiptap/pm/state";
import { test } from "vitest";

import { storyosManuscriptExtensions } from "../../src/manuscript-tiptap-adapter.ts";
import type { ChapterTextReference } from "../../src/selection-reference.ts";
import {
  acceptDraftCandidates, addDraftReferences, clearDraftReferences, copiedReferences, pasteReferences, draftCanAdd, draftEntries, rebaseDraftReferences, removeDraftReference, setRunReferences,
  trackableDraftReferences, trackDraftReferences, type DraftReference,
} from "../../src/selection-reference-draft.ts";

const id = (suffix: string) => `018f0000-0000-7001-8000-000000000${suffix}`;
const [FIRST, SECOND] = [id("b01"), id("b02")];
const CHAPTER = id("b03");
const [REVISION, NEXT] = [id("b04"), id("b05")];
const schema = getSchema(storyosManuscriptExtensions(FIRST!));
const tree = { volumes: [{ chapters: [{ chapter_id: CHAPTER, title: "渡口" }] }] };
let project = 0;

function scope() {
  project += 1;
  return { owner_user_id: id("a00"), project_id: id(`c${String(project).padStart(2, "0")}`) };
}

function state(first: string, second: string) {
  return EditorState.create({ doc: schema.nodeFromJSON({ type: "doc", content: [
    { type: "paragraph", attrs: { id: FIRST }, content: [{ type: "text", text: first }] },
    { type: "paragraph", attrs: { id: SECOND }, content: [{ type: "text", text: second }] },
  ] }) });
}

function reference(block: string, start: number, end: number, revision = REVISION): ChapterTextReference {
  return { kind: "chapter_text", chapter_id: CHAPTER, base_authoritative_revision_id: revision,
    coordinate_profile: "storyos.editor.utf16-code-unit.v1",
    start: { manuscript_block_id: block, offset: start }, end: { manuscript_block_id: block, offset: end } };
}

const entry = (value: ChapterTextReference): Omit<DraftReference, "key"> =>
  ({ reference: value, label: { chapter: "第 1 章 渡口", part: { kind: "paragraphs", first: 2, last: 2 } }, moved: false });
/** Adds one reference and returns its key, or undefined when the draft has it or cannot hold it. */
const add = (project: ReturnType<typeof scope>, value: ChapterTextReference) =>
  addDraftReferences(project, [entry(value)])?.[0];

test("a draft adds distinct references while the message and the active AgentRun hold fewer than eight", () => {
  const project = scope();
  const run = Array.from({ length: 6 }, (_, index) => reference(FIRST!, index, index + 1));
  setRunReferences(project, run);
  const key = add(project, reference(SECOND!, 0, 2));
  assert.ok(key !== undefined);
  assert.equal(add(project, reference(SECOND!, 0, 2)), undefined);
  // A reference that the Run already has does not count again.
  assert.ok(add(project, run[0]!) !== undefined);
  assert.equal(draftCanAdd({ entries: draftEntries(project), run }, [reference(SECOND!, 2, 4)]), true);
  assert.ok(add(project, reference(SECOND!, 2, 4)) !== undefined);
  assert.equal(add(project, reference(SECOND!, 4, 5)), undefined);
  removeDraftReference(project, key);
  assert.deepEqual(draftEntries(project).map((item) => item.reference), [run[0], reference(SECOND!, 2, 4)]);
});

test("a send removes only its references, and a reference added while it waits stays", () => {
  const project = scope();
  const sent = add(project, reference(SECOND!, 2, 4));
  const tracked = new Set(trackableDraftReferences(project, CHAPTER, REVISION));
  // An edit while the send waits moves the sent reference, and the author adds one more.
  trackDraftReferences(project, CHAPTER, tracked, state("雨夜。", "船夫在等着。").tr.insertText("冬", 7), tree);
  add(project, reference(FIRST!, 0, 2));
  clearDraftReferences(project, [sent!]);
  assert.deepEqual(draftEntries(project).map((item) => item.reference), [reference(FIRST!, 0, 2)]);
});

test("an author edit moves a tracked reference, and the saved Revision becomes its base", () => {
  const project = scope();
  const before = state("雨夜。", "船夫在等着。");
  add(project, reference(SECOND!, 2, 4));
  const tracked = new Set(trackableDraftReferences(project, CHAPTER, REVISION));
  assert.equal(tracked.size, 1);
  trackDraftReferences(project, CHAPTER, tracked, before.tr.insertText("冬", 7), tree);
  assert.deepEqual(draftEntries(project).map(({ reference: value, label, moved }) => ({ value, label, moved })),
    [{ value: reference(SECOND!, 3, 5), moved: true,
      label: { chapter: "第 1 章 渡口", part: { kind: "paragraphs", first: 2, last: 2 } } }]);
  rebaseDraftReferences(project, CHAPTER, tracked, NEXT);
  assert.deepEqual(draftEntries(project).map((item) => [item.reference, item.moved]),
    [[reference(SECOND!, 3, 5, NEXT), false]]);
});

test("a reference disappears when its text is deleted, and a replaced block stops its tracking", () => {
  const project = scope();
  const before = state("雨夜。", "船夫在等着。");
  add(project, reference(FIRST!, 0, 2));
  add(project, reference(SECOND!, 2, 4));
  const tracked = new Set(trackableDraftReferences(project, CHAPTER, REVISION));
  trackDraftReferences(project, CHAPTER, tracked, before.tr.delete(1, 4), tree);
  assert.deepEqual(draftEntries(project).map((item) => item.reference), [reference(SECOND!, 2, 4)]);
  rebaseDraftReferences(project, CHAPTER, tracked, NEXT);
  // A replacement of all of the document with the same text keeps the tracking. The new document can be
  // a new Revision, so the reference waits for its rebase.
  const same = state("。", "船夫在等着。");
  trackDraftReferences(project, CHAPTER, tracked,
    same.tr.replaceWith(0, same.doc.content.size, state("。", "船夫在等着。").doc.content), tree);
  assert.equal(tracked.size, 1);
  assert.deepEqual(draftEntries(project).map((item) => [item.reference, item.moved]),
    [[reference(SECOND!, 2, 4, NEXT), true]]);
  trackDraftReferences(project, CHAPTER, tracked,
    same.tr.replaceWith(0, same.doc.content.size, state("。", "渔夫在等着。").doc.content), tree);
  assert.equal(tracked.size, 0);
  // Without tracking, a later edit does not move the reference, and the Host blocks its stale base.
  trackDraftReferences(project, CHAPTER, tracked, same.tr.insertText("冬", 4), tree);
  assert.deepEqual(draftEntries(project).map((item) => [item.reference, item.moved]),
    [[reference(SECOND!, 2, 4, NEXT), true]]);
});

const [PROPOSAL, OPERATION, CANDIDATE_REVISION, NEXT_CANDIDATE] = [id("b06"), id("b07"), id("b08"), id("b09")];

/** 雨夜。 / [雨停了。], with a block candidate for the first block. */
function withCandidate(candidate: string, revision = CANDIDATE_REVISION) {
  return EditorState.create({ doc: schema.nodeFromJSON({ type: "doc", content: [
    { type: "paragraph", attrs: { id: FIRST }, content: [{ type: "text", text: "雨夜。" }] },
    { type: "blockProposal", attrs: { proposalId: PROPOSAL, operationId: OPERATION, revisionId: revision, blockId: FIRST },
      content: [{ type: "text", text: candidate }] },
  ] }) });
}

function candidateEntry(start: number, end: number): Omit<DraftReference, "key"> {
  return { reference: { kind: "proposal_candidate_text", chapter_id: CHAPTER, proposal_id: PROPOSAL,
    operation_id: OPERATION, revision_id: CANDIDATE_REVISION, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
    start_offset: start, end_offset: end }, candidate: { blockId: FIRST!, inlineFrom: null },
  label: { chapter: "第 1 章 渡口", part: { kind: "paragraphs", first: 1, last: 1 }, candidate: true }, moved: false };
}

test("a candidate reference moves with candidate edits and becomes Chapter text when the author accepts it", () => {
  const project = scope();
  addDraftReferences(project, [candidateEntry(1, 3)]);
  const tracked = new Set<string>();
  // The candidate block starts at 5, so its text starts at 6.
  const edited = withCandidate("雨停了。").tr.insertText("大", 6);
  trackDraftReferences(project, CHAPTER, tracked, edited, tree);
  // The saved candidate edit gives the newest Proposal Revision to the candidate.
  const revised = EditorState.create({ doc: edited.doc }).tr.setNodeMarkup(5, undefined,
    { ...edited.doc.child(1).attrs, revisionId: NEXT_CANDIDATE });
  trackDraftReferences(project, CHAPTER, tracked, revised, tree);
  assert.deepEqual(draftEntries(project).map((item) => item.reference), [{ ...candidateEntry(2, 4).reference,
    revision_id: NEXT_CANDIDATE }]);
  acceptDraftCandidates(project, PROPOSAL, [OPERATION!], NEXT);
  assert.deepEqual(draftEntries(project).map(({ reference: value, label, moved, candidate }) =>
    ({ value, label, moved, candidate })), [{ value: reference(FIRST!, 2, 4, NEXT), moved: false, candidate: undefined,
    label: { chapter: "第 1 章 渡口", part: { kind: "paragraphs", first: 1, last: 1 } } }]);
});

test("a candidate reference stays while a replaced document keeps its text, and goes with its candidate", () => {
  const project = scope();
  addDraftReferences(project, [candidateEntry(1, 3)]);
  const before = withCandidate("雨停了。");
  const replace = (next: EditorState) => before.tr.replaceWith(0, before.doc.content.size, next.doc.content);
  trackDraftReferences(project, CHAPTER, new Set(), replace(withCandidate("雨停了。", NEXT_CANDIDATE)), tree);
  assert.deepEqual(draftEntries(project).map((item) => item.reference),
    [{ ...candidateEntry(1, 3).reference, revision_id: NEXT_CANDIDATE }]);
  // A rejection or a withdrawal takes the candidate from the document.
  trackDraftReferences(project, CHAPTER, new Set(), replace(state("雨夜。", "船夫在等着。")), tree);
  assert.deepEqual(draftEntries(project), []);
});

test("a paste of a copy adds its references and tracks them in the editor of the copy", () => {
  const project = scope();
  const tracked = new Set<string>();
  const data = copiedReferences(project, tracked, [entry(reference(SECOND!, 0, 2)), candidateEntry(0, 1)]);
  assert.equal(pasteReferences(scope(), data), false);
  assert.equal(pasteReferences(project, "not data"), false);
  assert.equal(pasteReferences(project, data), true);
  assert.deepEqual(draftEntries(project).map((item) => item.reference),
    [reference(SECOND!, 0, 2), candidateEntry(0, 1).reference]);
  assert.deepEqual([...tracked], draftEntries(project).map((item) => item.key));
  // A paste that the draft cannot hold adds nothing.
  addDraftReferences(project, Array.from({ length: 6 }, (_, index) => entry(reference(FIRST!, index, index + 1))));
  assert.equal(pasteReferences(project, copiedReferences(project, tracked, [entry(reference(SECOND!, 2, 4))])), false);
  assert.equal(draftEntries(project).length, 8);
});
