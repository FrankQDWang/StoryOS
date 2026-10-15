import assert from "node:assert/strict";
import { getSchema } from "@tiptap/core";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { Transform } from "@tiptap/pm/transform";
import { test } from "vitest";

import type { SelectionReference } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readManuscriptParagraphs } from "../../src/manuscript-doc.ts";
import { storyosManuscriptExtensions } from "../../src/manuscript-tiptap-adapter.ts";
import {
  canAddReference, candidateRange, chapterTextReference, formatReferenceLabel, mapReferenceRange, mergeReferences,
  referenceLabel, referenceRange, selectionReferences, SELECTION_REFERENCE_LIMIT, type ChapterTextReference,
} from "../../src/selection-reference.ts";

const id = (suffix: string) => `018f0000-0000-7001-8000-000000000${suffix}`;
const [HEADING, FIRST, SECOND, THIRD, LATER] = ["a01", "a02", "a03", "a04", "a05"].map(id);
const CHAPTER = id("c03");
const REVISION = id("d01");
const schema = getSchema(storyosManuscriptExtensions(FIRST!));

const text = (value: string) => [{ type: "text", text: value }];
const paragraph = (block: string, ...content: object[]) => ({ type: "paragraph", attrs: { id: block }, content });
const inline = (source: string, candidate: string) => ({ type: "inlineProposal",
  attrs: { proposalId: id("e01"), operationId: id("e02"), revisionId: id("e03"), blockId: THIRD, sourceText: source },
  content: text(candidate) });

/** 一 / 雨夜。渡口 / [candidate] / 船夫😀等着 / 灯[亮起]。 / 二, where 亮起 is an inline candidate for 灭了. */
function manuscript(withBlockCandidate: boolean): ProseMirrorNode {
  return schema.nodeFromJSON({ type: "doc", content: [
    { type: "heading", attrs: { id: HEADING, level: 1 }, content: text("一") },
    paragraph(FIRST!, ...text("雨夜。渡口")),
    ...withBlockCandidate ? [{ type: "blockProposal", attrs: { proposalId: id("e04"), blockId: FIRST },
      content: text("雨停了。") }] : [],
    paragraph(SECOND!, ...text("船夫😀等着")),
    paragraph(THIRD!, ...text("灯"), inline("灭了", "亮起"), ...text("。")),
    { type: "heading", attrs: { id: LATER, level: 1 }, content: text("二") },
  ] });
}

/** The document position of an offset in the editor content of a block. */
function at(doc: ProseMirrorNode, block: string, offset: number): number {
  let position = -1;
  doc.forEach((node, nodeOffset) => {
    if (node.attrs.id === block && node.type.name !== "blockProposal") position = nodeOffset + 1 + offset;
  });
  return position;
}

function reference(start: [string, number], end: [string, number]): ChapterTextReference {
  return { kind: "chapter_text", chapter_id: CHAPTER, base_authoritative_revision_id: REVISION,
    coordinate_profile: "storyos.editor.utf16-code-unit.v1",
    start: { manuscript_block_id: start[0], offset: start[1] },
    end: { manuscript_block_id: end[0], offset: end[1] } };
}

test("a selection becomes the exact Chapter text range of its blocks and returns to the same range", () => {
  const doc = manuscript(false);
  const select = (from: number, to: number) => chapterTextReference(doc, { from, to }, CHAPTER, REVISION);
  // 灯[亮起]。: the editor offset 4 is after the candidate, and the source offset is 1 + 2.
  const cases: [number, number, ChapterTextReference | undefined][] = [
    [at(doc, FIRST!, 3), at(doc, FIRST!, 5), reference([FIRST!, 3], [FIRST!, 5])],
    [at(doc, SECOND!, 2), at(doc, THIRD!, 1), reference([SECOND!, 2], [THIRD!, 1])],
    [at(doc, THIRD!, 5), at(doc, THIRD!, 6), reference([THIRD!, 3], [THIRD!, 4])],
    [at(doc, HEADING!, 0), at(doc, FIRST!, 2), reference([HEADING!, 0], [FIRST!, 2])],
    // A range that only touches the next block at its start ends in the previous block.
    [at(doc, FIRST!, 3), at(doc, SECOND!, 0), reference([FIRST!, 3], [FIRST!, 5])],
    [at(doc, FIRST!, 5), at(doc, SECOND!, 0), undefined],
    [at(doc, FIRST!, 2), at(doc, FIRST!, 2), undefined],
    [at(doc, THIRD!, 0), at(doc, THIRD!, 3), undefined],
    [at(doc, THIRD!, 2), at(doc, THIRD!, 3), undefined],
  ];
  for (const [from, to, expected] of cases) {
    assert.deepEqual(select(from, to), expected, `${from}..${to}`);
    if (expected !== undefined) {
      const range = referenceRange(doc, expected);
      assert.deepEqual(range === undefined ? undefined : select(range.from, range.to), expected);
    }
  }
  // A range in the source text of an inline candidate has no editor range.
  assert.equal(referenceRange(doc, reference([THIRD!, 2], [THIRD!, 4])), undefined);
  assert.equal(referenceRange(doc, reference([id("a99"), 0], [THIRD!, 1])), undefined);
});

test("a selection across a pending candidate block has no reference", () => {
  const doc = manuscript(true);
  assert.equal(chapterTextReference(doc, { from: at(doc, FIRST!, 3), to: at(doc, SECOND!, 2) }, CHAPTER, REVISION),
    undefined);
  assert.deepEqual(chapterTextReference(doc, { from: at(doc, SECOND!, 0), to: at(doc, SECOND!, 2) }, CHAPTER, REVISION),
    reference([SECOND!, 0], [SECOND!, 2]));
});

test("a range moves with the author edits and disappears when all of its text is deleted", () => {
  const doc = manuscript(false);
  const range = { from: at(doc, SECOND!, 2), to: at(doc, SECOND!, 4) };
  const inserted = new Transform(doc).insert(at(doc, SECOND!, 0), schema.text("夜"));
  const moved = mapReferenceRange(range, inserted.mapping);
  assert.deepEqual(moved, { from: range.from + 1, to: range.to + 1 });
  assert.deepEqual(chapterTextReference(inserted.doc, moved!, CHAPTER, id("d02")),
    { ...reference([SECOND!, 3], [SECOND!, 5]), base_authoritative_revision_id: id("d02") });
  // An edit at an edge does not join the range.
  const edge = new Transform(doc).insert(range.to, schema.text("。"));
  assert.deepEqual(mapReferenceRange(range, edge.mapping), range);
  const deleted = new Transform(doc).delete(range.from - 1, range.to + 1);
  assert.equal(mapReferenceRange(range, deleted.mapping), undefined);
});

test("a message and an AgentRun hold at most eight distinct references", () => {
  const references = Array.from({ length: SELECTION_REFERENCE_LIMIT }, (_, index) =>
    reference([FIRST!, index], [FIRST!, index + 1]));
  const candidate: SelectionReference = { kind: "proposal_candidate_text", chapter_id: CHAPTER,
    proposal_id: id("e01"), operation_id: id("e02"), revision_id: id("e03"),
    coordinate_profile: "storyos.editor.utf16-code-unit.v1", start_offset: 0, end_offset: 2 };
  const seven = mergeReferences(references.slice(0, 4), [...references.slice(2, 7), references[0]!]);
  assert.deepEqual(seven, references.slice(0, 7));
  assert.equal(canAddReference(seven), true);
  assert.deepEqual(mergeReferences(seven, [candidate, { ...candidate }]), [...seven, candidate]);
  assert.equal(canAddReference(mergeReferences(seven, [candidate])), false);
});

test("a label numbers the Chapter in reading order and counts only the paragraphs", () => {
  const tree = { volumes: [
    { chapters: [{ chapter_id: id("c01"), title: "雨" }, { chapter_id: id("c02"), title: "" }] },
    { chapters: [{ chapter_id: CHAPTER, title: "渡口" }] },
  ] };
  const blocks = readManuscriptParagraphs(manuscript(true))!;
  const label = (start: [string, number], end: [string, number]) => {
    const value = referenceLabel(tree, blocks, { reference: reference(start, end) });
    return value === undefined ? undefined : formatReferenceLabel(value);
  };
  assert.equal(label([HEADING!, 0], [SECOND!, 1]), "第 3 章 渡口（第 1–2 段）");
  assert.equal(label([THIRD!, 1], [THIRD!, 2]), "第 3 章 渡口（第 3 段）");
  assert.equal(label([THIRD!, 0], [LATER!, 1]), "第 3 章 渡口（第 3 段）");
  assert.equal(label([LATER!, 0], [LATER!, 1]), "第 3 章 渡口（小标题「二」）");
  assert.deepEqual(referenceLabel(tree, blocks, { reference: { ...reference([LATER!, 0], [LATER!, 1]),
    chapter_id: id("c02") } }),
    { chapter: "第 2 章", part: { kind: "heading", text: "二" } });
  assert.equal(label([SECOND!, 0], [FIRST!, 1]), undefined);
});

test("a selection over Chapter text and candidates gives one reference for each part", () => {
  const candidateAttrs = (suffix: string, blockId: string) =>
    ({ proposalId: id(`f${suffix}1`), operationId: id(`f${suffix}2`), revisionId: id(`f${suffix}3`), blockId });
  // 雨夜。渡口 / [雨停了。] / 灯[亮起]。, with a block candidate for the first block and an inline one for 灭了.
  const doc = schema.nodeFromJSON({ type: "doc", content: [
    paragraph(FIRST!, ...text("雨夜。渡口")),
    { type: "blockProposal", attrs: candidateAttrs("1", FIRST!), content: text("雨停了。") },
    paragraph(THIRD!, ...text("灯"), { type: "inlineProposal",
      attrs: { ...candidateAttrs("2", THIRD!), inlineFrom: 1, sourceText: "灭了" }, content: text("亮起") }, ...text("。")),
  ] });
  const candidate = (suffix: string, start: number, end: number) => ({ kind: "proposal_candidate_text",
    chapter_id: CHAPTER, proposal_id: id(`f${suffix}1`), operation_id: id(`f${suffix}2`), revision_id: id(`f${suffix}3`),
    coordinate_profile: "storyos.editor.utf16-code-unit.v1", start_offset: start, end_offset: end }) as const;
  // From 渡口 in the first block to 亮 in the inline candidate.
  const selected = selectionReferences(doc, { from: at(doc, FIRST!, 3), to: 17 }, CHAPTER, REVISION);
  assert.deepEqual(selected, [
    { reference: reference([FIRST!, 3], [FIRST!, 5]) },
    { reference: candidate("1", 0, 4), candidate: { blockId: FIRST, inlineFrom: null } },
    { reference: reference([THIRD!, 0], [THIRD!, 1]) },
    { reference: candidate("2", 0, 1), candidate: { blockId: THIRD, inlineFrom: 1 } },
  ]);
  assert.deepEqual(candidateRange(doc, candidate("2", 0, 1)), { from: 16, to: 17 });
  // A selection in candidate text alone gives only the candidate reference.
  assert.deepEqual(selectionReferences(doc, { from: 9, to: 11 }, CHAPTER, REVISION).map((item) => item.reference),
    [candidate("1", 1, 3)]);
  const tree = { volumes: [{ chapters: [{ chapter_id: CHAPTER, title: "渡口" }] }] };
  const blocks = readManuscriptParagraphs(doc)!;
  assert.deepEqual(selected.map((item) => formatReferenceLabel(referenceLabel(tree, blocks, item)!)), [
    "第 1 章 渡口（第 1 段）", "第 1 章 渡口（第 1 段的修改建议）", "第 1 章 渡口（第 2 段）", "第 1 章 渡口（第 2 段的修改建议）",
  ]);
});
