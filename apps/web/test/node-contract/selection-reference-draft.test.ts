import assert from "node:assert/strict";
import { getSchema } from "@tiptap/core";
import { EditorState } from "@tiptap/pm/state";
import { test } from "vitest";

import { storyosManuscriptExtensions } from "../../src/manuscript-tiptap-adapter.ts";
import type { ChapterTextReference } from "../../src/selection-reference.ts";
import {
  addDraftReference, draftCanAdd, draftEntries, rebaseDraftReferences, removeDraftReference, setRunReferences,
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

test("a draft adds distinct references while the message and the active AgentRun hold fewer than eight", () => {
  const project = scope();
  const run = Array.from({ length: 6 }, (_, index) => reference(FIRST!, index, index + 1));
  setRunReferences(project, run);
  const key = addDraftReference(project, entry(reference(SECOND!, 0, 2)));
  assert.ok(key !== undefined);
  assert.equal(addDraftReference(project, entry(reference(SECOND!, 0, 2))), undefined);
  // A reference that the Run already has does not count again.
  assert.ok(addDraftReference(project, entry(run[0]!)) !== undefined);
  assert.equal(draftCanAdd({ entries: draftEntries(project), run }), true);
  assert.ok(addDraftReference(project, entry(reference(SECOND!, 2, 4))) !== undefined);
  assert.equal(addDraftReference(project, entry(reference(SECOND!, 4, 5))), undefined);
  removeDraftReference(project, key);
  assert.deepEqual(draftEntries(project).map((item) => item.reference), [run[0], reference(SECOND!, 2, 4)]);
});

test("an author edit moves a tracked reference, and the saved Revision becomes its base", () => {
  const project = scope();
  const before = state("雨夜。", "船夫在等着。");
  addDraftReference(project, entry(reference(SECOND!, 2, 4)));
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
  addDraftReference(project, entry(reference(FIRST!, 0, 2)));
  addDraftReference(project, entry(reference(SECOND!, 2, 4)));
  const tracked = new Set(trackableDraftReferences(project, CHAPTER, REVISION));
  trackDraftReferences(project, CHAPTER, tracked, before.tr.delete(1, 4), tree);
  assert.deepEqual(draftEntries(project).map((item) => item.reference), [reference(SECOND!, 2, 4)]);
  // A replacement of all of the document with the same text keeps the tracking.
  const same = state("。", "船夫在等着。");
  trackDraftReferences(project, CHAPTER, tracked,
    same.tr.replaceWith(0, same.doc.content.size, state("。", "船夫在等着。").doc.content), tree);
  assert.equal(tracked.size, 1);
  trackDraftReferences(project, CHAPTER, tracked,
    same.tr.replaceWith(0, same.doc.content.size, state("。", "渔夫在等着。").doc.content), tree);
  assert.equal(tracked.size, 0);
  // The deletion in the first block moved the head, so the reference waits for the saved Revision.
  // Without tracking, a later edit does not move it, and the Host blocks its stale base.
  trackDraftReferences(project, CHAPTER, tracked, same.tr.insertText("冬", 4), tree);
  assert.deepEqual(draftEntries(project).map((item) => [item.reference, item.moved]), [[reference(SECOND!, 2, 4), true]]);
});
