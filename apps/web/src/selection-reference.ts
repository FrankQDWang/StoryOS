import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import type { Mappable } from "@tiptap/pm/transform";
import type { SelectionReference } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ManuscriptParagraph } from "./manuscript-doc.ts";

/** The most references that one message or one AgentRun can hold (ADR 0062). */
export const SELECTION_REFERENCE_LIMIT = 8;

export type ChapterTextReference = Extract<SelectionReference, { kind: "chapter_text" }>;
export type EditorRange = { from: number; to: number };
/** The part of the manuscript tree that a label reads: the Chapters of each Volume in reading order. */
export type ChapterOrder = { volumes: readonly { chapters: readonly { chapter_id: string; title: string }[] }[] };
type TextPoint = ChapterTextReference["start"];

/** The label part after the Chapter: a paragraph range, or one heading when no paragraph is selected. */
export type ReferenceLabel = {
  chapter: string;
  part: { kind: "paragraphs"; first: number; last: number } | { kind: "heading"; text: string };
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

/** True when the range touches the text of a pending candidate, in its own block or inline. */
function touchesCandidate(doc: ProseMirrorNode, range: EditorRange): boolean {
  let touched = false;
  doc.nodesBetween(range.from, range.to, (node, position) => {
    if ((node.type.name === "blockProposal" || node.type.name === "inlineProposal")
      && position < range.to && position + node.nodeSize > range.from) touched = true;
    return !touched;
  });
  return touched;
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

/**
 * The Chapter text reference of an editor selection at the Revision of the editor document. An empty
 * selection, or one that touches candidate text, has none.
 */
export function chapterTextReference(doc: ProseMirrorNode, range: EditorRange, chapterId: string,
  revisionId: string): ChapterTextReference | undefined {
  if (range.to <= range.from || touchesCandidate(doc, range)) return undefined;
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
  reference: ChapterTextReference): ReferenceLabel | undefined {
  const chapters = tree.volumes.flatMap((volume) => volume.chapters);
  const index = chapters.findIndex((chapter) => chapter.chapter_id === reference.chapter_id);
  const first = blocks.findIndex((block) => block.manuscript_block_id === reference.start.manuscript_block_id);
  const last = blocks.findIndex((block) => block.manuscript_block_id === reference.end.manuscript_block_id);
  if (index < 0 || first < 0 || last < first) return undefined;
  const title = chapters[index]!.title;
  const chapter = title.length > 0 ? `第 ${index + 1} 章 ${title}` : `第 ${index + 1} 章`;
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

/** The plain text of a label, for example 「第 3 章 渡口（第 2–4 段）」. */
export function formatReferenceLabel(label: ReferenceLabel): string {
  const part = label.part.kind === "heading" ? `小标题「${label.part.text}」`
    : label.part.first === label.part.last ? `第 ${label.part.first} 段`
      : `第 ${label.part.first}–${label.part.last} 段`;
  return `${label.chapter}（${part}）`;
}
