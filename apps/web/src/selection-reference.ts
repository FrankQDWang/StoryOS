import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import type { Mappable } from "@tiptap/pm/transform";
import type { SelectionReference } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ManuscriptParagraph } from "./manuscript-doc.ts";

/** The most references that one message or one AgentRun can hold (ADR 0062). */
export const SELECTION_REFERENCE_LIMIT = 8;

export type ChapterTextReference = Extract<SelectionReference, { kind: "chapter_text" }>;
export type CandidateTextReference = Extract<SelectionReference, { kind: "proposal_candidate_text" }>;
export type EditorRange = { from: number; to: number };
/** The Chapter block of a candidate, and the source offset of an inline candidate in that block. */
export type CandidateTarget = { blockId: string; inlineFrom: number | null };
/** One reference of a selection. A candidate reference also has the target of its candidate. */
export type SelectedReference = { reference: SelectionReference; candidate?: CandidateTarget };
/** The part of the manuscript tree that a label reads: the Chapters of each Volume in reading order. */
export type ChapterOrder = { volumes: readonly { chapters: readonly { chapter_id: string; title: string }[] }[] };
type TextPoint = ChapterTextReference["start"];

/**
 * The label part after the Chapter: a paragraph range, or one heading when no paragraph is selected. The
 * label of a candidate reference names the block of its candidate.
 */
export type ReferenceLabel = {
  chapter: string;
  part: { kind: "paragraphs"; first: number; last: number } | { kind: "heading"; text: string };
  candidate?: true;
};

/** One manuscript block of the editor document, with the document position of its content. */
type EditorBlock = { node: ProseMirrorNode; id: string; start: number; end: number };

function editorBlocks(doc: ProseMirrorNode): EditorBlock[] {
  const blocks: EditorBlock[] = [];
  doc.forEach((node, offset) => {
    if (node.type.name === "paragraph" || node.type.name === "heading") {
      blocks.push({ node, id: node.attrs.id as string, start: offset + 1, end: offset + node.nodeSize - 1 });
    }
  });
  return blocks;
}

/** The authoritative offset of a document position in a block. An inline candidate holds the length of its source text. */
function blockOffset(block: EditorBlock, position: number): number | undefined {
  let offset = 0;
  let result: number | undefined;
  block.node.forEach((child, childOffset) => {
    if (result !== undefined) return;
    const start = block.start + childOffset;
    if (child.type.name === "inlineProposal") {
      if (position > start && position < start + child.nodeSize) result = -1;
      else if (position <= start) result = offset;
      else offset += (child.attrs.sourceText as string | null ?? "").length;
    } else if (position <= start + child.nodeSize) {
      result = offset + Math.max(0, position - start);
    } else {
      offset += child.nodeSize;
    }
  });
  if (result === -1) return undefined;
  return result ?? offset;
}

/** The document position of an authoritative offset in a block, outside the inline candidates. */
function blockPosition(block: EditorBlock, offset: number): number | undefined {
  let remaining = offset;
  let result: number | undefined;
  block.node.forEach((child, childOffset) => {
    if (result !== undefined) return;
    const start = block.start + childOffset;
    const length = child.type.name === "inlineProposal"
      ? (child.attrs.sourceText as string | null ?? "").length : child.nodeSize;
    if (remaining === 0) result = start;
    else if (remaining < length) result = child.type.name === "inlineProposal" ? -1 : start + remaining;
    else remaining -= length;
  });
  if (result === -1) return undefined;
  return result ?? (remaining === 0 ? block.end : undefined);
}

function isCandidate(node: ProseMirrorNode): boolean {
  return node.type.name === "blockProposal" || node.type.name === "inlineProposal";
}

/**
 * The Chapter text reference of an editor selection at the Revision of the editor document. An empty
 * selection, or one that touches candidate text, has none.
 */
export function chapterTextReference(doc: ProseMirrorNode, range: EditorRange, chapterId: string,
  revisionId: string): ChapterTextReference | undefined {
  if (range.to <= range.from) return undefined;
  // A range that touches the text of a pending candidate, in its own block or inline, has no reference.
  let touched = false;
  doc.nodesBetween(range.from, range.to, (node, position) => {
    if (isCandidate(node) && position < range.to && position + node.nodeSize > range.from) touched = true;
    return !touched;
  });
  return touched ? undefined : chapterRangeReference(doc, range, chapterId, revisionId);
}

function chapterRangeReference(doc: ProseMirrorNode, range: EditorRange, chapterId: string,
  revisionId: string): ChapterTextReference | undefined {
  if (range.to <= range.from) return undefined;
  const blocks = editorBlocks(doc);
  // A block that the range only touches at its end or at its start is not selected.
  const first = blocks.findIndex((block) => block.end > range.from);
  const last = blocks.filter((block) => block.start < range.to).length - 1;
  if (first < 0 || last < first) return undefined;
  const start = blockOffset(blocks[first]!, Math.max(range.from, blocks[first]!.start));
  const end = blockOffset(blocks[last]!, Math.min(range.to, blocks[last]!.end));
  if (start === undefined || end === undefined || (first === last && end <= start)) return undefined;
  const point = (block: EditorBlock, offset: number): TextPoint => ({ manuscript_block_id: block.id, offset });
  return { kind: "chapter_text", chapter_id: chapterId, base_authoritative_revision_id: revisionId,
    coordinate_profile: "storyos.editor.utf16-code-unit.v1",
    start: point(blocks[first]!, start), end: point(blocks[last]!, end) };
}

/** One pending candidate of the editor document, with the document positions of its text. */
type EditorCandidate = { node: ProseMirrorNode; position: number; start: number; end: number };

function editorCandidates(doc: ProseMirrorNode, range: EditorRange): EditorCandidate[] {
  const candidates: EditorCandidate[] = [];
  doc.nodesBetween(range.from, range.to, (node, position) => {
    if (!isCandidate(node)) return true;
    candidates.push({ node, position, start: position + 1, end: position + node.nodeSize - 1 });
    return false;
  });
  return candidates;
}

/** The reference of a range in the text of one candidate, at the Proposal Revision of the candidate. */
function candidateReference(chapterId: string, candidate: EditorCandidate,
  range: EditorRange): SelectedReference | undefined {
  const from = Math.max(range.from, candidate.start);
  const to = Math.min(range.to, candidate.end);
  const attrs = candidate.node.attrs;
  if (to <= from || typeof attrs.proposalId !== "string" || typeof attrs.operationId !== "string"
    || typeof attrs.revisionId !== "string" || typeof attrs.blockId !== "string") return undefined;
  return { reference: { kind: "proposal_candidate_text", chapter_id: chapterId, proposal_id: attrs.proposalId,
    operation_id: attrs.operationId, revision_id: attrs.revisionId, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
    start_offset: from - candidate.start, end_offset: to - candidate.start },
  candidate: { blockId: attrs.blockId, inlineFrom: candidate.node.type.name === "inlineProposal"
    ? attrs.inlineFrom as number : null } };
}

/**
 * The references of an editor selection. A selection over Chapter text and candidate text gives one
 * reference for each candidate part and one for each Chapter text part between them.
 */
export function selectionReferences(doc: ProseMirrorNode, range: EditorRange, chapterId: string,
  revisionId: string): SelectedReference[] {
  if (range.to <= range.from) return [];
  const references: SelectedReference[] = [];
  let from = range.from;
  const chapter = (to: number) => {
    const reference = chapterRangeReference(doc, { from, to }, chapterId, revisionId);
    if (reference !== undefined) references.push({ reference });
  };
  for (const candidate of editorCandidates(doc, range)) {
    chapter(Math.min(range.to, candidate.position));
    const reference = candidateReference(chapterId, candidate, range);
    if (reference !== undefined) references.push(reference);
    from = Math.max(from, candidate.position + candidate.node.nodeSize);
  }
  chapter(range.to);
  return references;
}

/** The editor range of a candidate reference while the editor shows the candidate. */
export function candidateRange(doc: ProseMirrorNode, reference: CandidateTextReference): EditorRange | undefined {
  const candidate = editorCandidates(doc, { from: 0, to: doc.content.size }).find(({ node }) =>
    node.attrs.proposalId === reference.proposal_id && node.attrs.operationId === reference.operation_id);
  if (candidate === undefined || candidate.start + reference.end_offset > candidate.end) return undefined;
  return { from: candidate.start + reference.start_offset, to: candidate.start + reference.end_offset };
}

/** The reference of a range in one candidate text, at the newest Proposal Revision of the editor document. */
export function candidateTextReference(doc: ProseMirrorNode, range: EditorRange,
  chapterId: string): CandidateTextReference | undefined {
  const candidate = editorCandidates(doc, range).find((item) => item.start <= range.from && range.to <= item.end);
  const selected = candidate === undefined ? undefined : candidateReference(chapterId, candidate, range);
  return selected?.reference.kind === "proposal_candidate_text" ? selected.reference : undefined;
}

/**
 * The editor range of a reference in a document at its base Revision. A missing block, or an offset
 * inside the source text of an inline candidate, has none.
 */
export function referenceRange(doc: ProseMirrorNode, reference: ChapterTextReference): EditorRange | undefined {
  const blocks = editorBlocks(doc);
  const find = (point: TextPoint) => blocks.find((block) => block.id === point.manuscript_block_id);
  const [first, last] = [find(reference.start), find(reference.end)];
  if (first === undefined || last === undefined) return undefined;
  const from = blockPosition(first, reference.start.offset);
  const to = blockPosition(last, reference.end.offset);
  return from === undefined || to === undefined || to <= from ? undefined : { from, to };
}

/** The range after the author edits of one change. A range whose text is all deleted has none. */
export function mapReferenceRange(range: EditorRange, mapping: Mappable): EditorRange | undefined {
  const from = mapping.map(range.from, 1);
  const to = mapping.map(range.to, -1);
  return to > from ? { from, to } : undefined;
}

function referenceKey(reference: SelectionReference): string {
  return reference.kind === "chapter_text"
    ? JSON.stringify([reference.kind, reference.chapter_id, reference.base_authoritative_revision_id,
      reference.start.manuscript_block_id, reference.start.offset, reference.end.manuscript_block_id, reference.end.offset])
    : JSON.stringify([reference.kind, reference.chapter_id, reference.proposal_id, reference.operation_id,
      reference.revision_id, reference.start_offset, reference.end_offset]);
}

/** The references after `added` joins them in order. A reference that is already there does not join again. */
export function mergeReferences(existing: readonly SelectionReference[],
  added: readonly SelectionReference[]): SelectionReference[] {
  const merged = [...existing];
  const keys = new Set(merged.map(referenceKey));
  for (const reference of added) {
    const key = referenceKey(reference);
    if (!keys.has(key)) {
      keys.add(key);
      merged.push(reference);
    }
  }
  return merged;
}

/** True when the AgentRun or the message can hold one more reference. */
export function canAddReference(references: readonly SelectionReference[]): boolean {
  return references.length < SELECTION_REFERENCE_LIMIT;
}

/**
 * The label of a Chapter text reference. The Chapter number counts all Chapters of the tree in reading
 * order, from 1. The paragraph numbers count only the paragraphs of the Chapter, from 1.
 */
export function referenceLabel(tree: ChapterOrder, blocks: readonly ManuscriptParagraph[],
  selected: SelectedReference): ReferenceLabel | undefined {
  const { reference, candidate } = selected;
  if (reference.kind === "proposal_candidate_text") {
    const label = candidate === undefined ? undefined
      : blockLabel(tree, blocks, reference.chapter_id, candidate.blockId, candidate.blockId);
    return label === undefined ? undefined : { ...label, candidate: true };
  }
  return blockLabel(tree, blocks, reference.chapter_id, reference.start.manuscript_block_id,
    reference.end.manuscript_block_id);
}

/** The Chapter part of a label, for example 「第 3 章 渡口」. A Chapter that is not in the tree has none. */
export function chapterLabel(tree: ChapterOrder, chapterId: string): string | undefined {
  const chapters = tree.volumes.flatMap((volume) => volume.chapters);
  const index = chapters.findIndex((chapter) => chapter.chapter_id === chapterId);
  if (index < 0) return undefined;
  const title = chapters[index]!.title;
  return title.length > 0 ? `第 ${index + 1} 章 ${title}` : `第 ${index + 1} 章`;
}

function blockLabel(tree: ChapterOrder, blocks: readonly ManuscriptParagraph[], chapterId: string,
  firstId: string, lastId: string): ReferenceLabel | undefined {
  const chapter = chapterLabel(tree, chapterId);
  const first = blocks.findIndex((block) => block.manuscript_block_id === firstId);
  const last = blocks.findIndex((block) => block.manuscript_block_id === lastId);
  if (chapter === undefined || first < 0 || last < first) return undefined;
  const numbers: number[] = [];
  let paragraph = 0;
  blocks.forEach((block, position) => {
    if (block.block_kind === "heading") return;
    paragraph += 1;
    if (position >= first && position <= last) numbers.push(paragraph);
  });
  if (numbers.length > 0) return { chapter, part: { kind: "paragraphs", first: numbers[0]!, last: numbers.at(-1)! } };
  return { chapter, part: { kind: "heading", text: blocks[first]!.text } };
}

/** The plain text of a label, for example 「第 3 章 渡口（第 2–4 段）」 or 「第 3 章 渡口（第 2 段的修改建议）」. */
export function formatReferenceLabel(label: ReferenceLabel): string {
  const part = label.part.kind === "heading" ? `小标题「${label.part.text}」`
    : label.part.first === label.part.last ? `第 ${label.part.first} 段`
      : `第 ${label.part.first}–${label.part.last} 段`;
  return `${label.chapter}（${part}${label.candidate === true ? "的修改建议" : ""}）`;
}
