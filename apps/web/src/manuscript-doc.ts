import type { Node } from "@tiptap/pm/model";

import type { ReplaceSelectionEdit } from "./editor-types.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export interface ManuscriptParagraph {
  manuscript_block_id: string;
  text: string;
}

export type CapturedManuscriptEdit =
  | {
    kind: "replace_block_selection";
    manuscript_block_id: string;
    from: number;
    to: number;
    text: string;
    resultingBody: string;
    resultingBlocks: ManuscriptParagraph[];
  }
  | {
    kind: "split_block";
    manuscript_block_id: string;
    offset: number;
    new_manuscript_block_id: string;
    resultingBody: string;
    resultingBlocks: ManuscriptParagraph[];
  }
  | {
    kind: "join_blocks";
    left_manuscript_block_id: string;
    right_manuscript_block_id: string;
    caret: number;
    resultingBody: string;
    resultingBlocks: ManuscriptParagraph[];
  }
  | {
    kind: "contiguous_replacement";
    primitives: ContiguousReplacementPrimitive[];
    from: number;
    to: number;
    resultingBody: string;
    resultingBlocks: ManuscriptParagraph[];
  };

export type ContiguousReplacementPrimitive =
  | {
    kind: "replace_block_selection";
    manuscript_block_id: string;
    from: number;
    to: number;
    text: string;
  }
  | {
    kind: "split_block";
    manuscript_block_id: string;
    offset: number;
    new_manuscript_block_id: string;
  }
  | {
    kind: "join_blocks";
    left_manuscript_block_id: string;
    right_manuscript_block_id: string;
  };

export function flattenChapterBody(blocks: readonly { text: string }[]): string {
  return blocks.map((block) => block.text).join("\n");
}

export function manuscriptBlocksJson(blocks: readonly ManuscriptParagraph[]) {
  return {
    type: "doc",
    content: blocks.map((block) => ({
      type: "paragraph",
      attrs: { id: block.manuscript_block_id },
      ...(block.text.length === 0 ? {} : { content: [{ type: "text", text: block.text }] }),
    })),
  };
}

export function manuscriptJson(blockId: string, body: string) {
  return manuscriptBlocksJson([{ manuscript_block_id: blockId, text: body }]);
}

function isUtf16Boundary(body: string, offset: number): boolean {
  if (!Number.isSafeInteger(offset) || offset < 0 || offset > body.length) return false;
  if (offset === 0 || offset === body.length) return true;
  const prior = body.charCodeAt(offset - 1);
  const next = body.charCodeAt(offset);
  return !(prior >= 0xd800 && prior <= 0xdbff && next >= 0xdc00 && next <= 0xdfff);
}

function paragraphText(paragraph: Node): string | undefined {
  let text = "";
  for (let offset = 0; offset < paragraph.childCount; offset += 1) {
    const child = paragraph.child(offset);
    if (!child.isText || child.marks.length > 0) return undefined;
    text += child.text ?? "";
  }
  return text;
}

export function readManuscriptParagraphs(doc: Node): ManuscriptParagraph[] | undefined {
  if (doc.childCount < 1) return undefined;
  const blocks: ManuscriptParagraph[] = [];
  const seen = new Set<string>();
  for (let index = 0; index < doc.childCount; index += 1) {
    const paragraph = doc.child(index);
    if (paragraph.type.name !== "paragraph") return undefined;
    const id: unknown = paragraph.attrs.id;
    const text = paragraphText(paragraph);
    if (typeof id !== "string" || !UUID.test(id) || text === undefined || seen.has(id)) {
      return undefined;
    }
    seen.add(id);
    blocks.push({ manuscript_block_id: id, text });
  }
  return blocks;
}

export function paragraphUtf16(doc: Node): string | undefined {
  const blocks = readManuscriptParagraphs(doc);
  if (blocks === undefined || blocks.length !== 1) return undefined;
  return blocks[0]?.text;
}

export function paragraphsEqual(
  left: readonly ManuscriptParagraph[],
  right: readonly ManuscriptParagraph[],
): boolean {
  return left.length === right.length
    && left.every((block, index) =>
      block.manuscript_block_id === right[index]?.manuscript_block_id
      && block.text === right[index]?.text);
}

export function isSupportedManuscriptDoc(previous: Node, next: Node, blockId: string): boolean {
  const nextBlocks = readManuscriptParagraphs(next);
  if (nextBlocks === undefined) return false;
  if (previous.childCount === 0) return true;
  const previousBlocks = readManuscriptParagraphs(previous);
  if (previousBlocks === undefined) return nextBlocks.length === 1
    && nextBlocks[0]?.manuscript_block_id === blockId;
  return captureManuscriptChange(previousBlocks, nextBlocks) !== undefined
    || paragraphsEqual(previousBlocks, nextBlocks);
}

export function contiguousUtf16Replace(
  before: string,
  after: string,
): ReplaceSelectionEdit | undefined {
  let from = 0;
  const shared = Math.min(before.length, after.length);
  while (from < shared && before.charCodeAt(from) === after.charCodeAt(from)) from += 1;
  if (!isUtf16Boundary(before, from) || !isUtf16Boundary(after, from)) {
    from -= 1;
    if (!isUtf16Boundary(before, from) || !isUtf16Boundary(after, from)) return undefined;
  }
  let beforeEnd = before.length;
  let afterEnd = after.length;
  while (beforeEnd > from && afterEnd > from
    && before.charCodeAt(beforeEnd - 1) === after.charCodeAt(afterEnd - 1)) {
    beforeEnd -= 1;
    afterEnd -= 1;
  }
  if (!isUtf16Boundary(before, beforeEnd) || !isUtf16Boundary(after, afterEnd)) {
    beforeEnd += 1;
    afterEnd += 1;
    if (!isUtf16Boundary(before, beforeEnd) || !isUtf16Boundary(after, afterEnd)) {
      return undefined;
    }
  }
  const text = after.slice(from, afterEnd);
  const expected = `${before.slice(0, from)}${text}${before.slice(beforeEnd)}`;
  if (expected !== after) return undefined;
  return { from, to: beforeEnd, text, resultingBody: after };
}

export function captureManuscriptChange(
  previous: readonly ManuscriptParagraph[],
  next: readonly ManuscriptParagraph[],
): CapturedManuscriptEdit | undefined {
  if (paragraphsEqual(previous, next) || previous.length === 0 || next.length === 0) {
    return undefined;
  }
  if (next.length === previous.length) {
    let changed = -1;
    let identitiesMatch = true;
    for (let index = 0; index < previous.length; index += 1) {
      const before = previous[index];
      const after = next[index];
      if (before === undefined || after === undefined) return undefined;
      if (before.manuscript_block_id !== after.manuscript_block_id) {
        identitiesMatch = false;
        break;
      }
      if (before.text === after.text) continue;
      if (changed !== -1) {
        identitiesMatch = false;
        break;
      }
      changed = index;
    }
    if (identitiesMatch && changed !== -1) {
      const before = previous[changed]!;
      const after = next[changed]!;
      const replace = contiguousUtf16Replace(before.text, after.text);
      if (replace === undefined) return undefined;
      const resultingBlocks = next.map((block) => ({ ...block }));
      return {
        kind: "replace_block_selection",
        manuscript_block_id: before.manuscript_block_id,
        from: replace.from,
        to: replace.to,
        text: replace.text,
        resultingBlocks,
        resultingBody: flattenChapterBody(resultingBlocks),
      };
    }
  }
  if (next.length === previous.length + 1) {
    for (let index = 0; index < previous.length; index += 1) {
      const before = previous[index]!;
      const left = next[index];
      const right = next[index + 1];
      if (left === undefined || right === undefined) return undefined;
      if (left.manuscript_block_id !== before.manuscript_block_id) continue;
      if (right.manuscript_block_id === before.manuscript_block_id) continue;
      if (left.text + right.text !== before.text) continue;
      const prefix = previous.slice(0, index);
      const suffix = previous.slice(index + 1);
      const nextPrefix = next.slice(0, index);
      const nextSuffix = next.slice(index + 2);
      if (!paragraphsEqual(prefix, nextPrefix) || !paragraphsEqual(suffix, nextSuffix)) {
        continue;
      }
      if (next.some((block, blockIndex) =>
        blockIndex !== index + 1 && block.manuscript_block_id === right.manuscript_block_id)) {
        continue;
      }
      const resultingBlocks = next.map((block) => ({ ...block }));
      return {
        kind: "split_block",
        manuscript_block_id: before.manuscript_block_id,
        offset: left.text.length,
        new_manuscript_block_id: right.manuscript_block_id,
        resultingBlocks,
        resultingBody: flattenChapterBody(resultingBlocks),
      };
    }
  }
  if (next.length === previous.length - 1) {
    for (let index = 0; index < next.length; index += 1) {
      const left = previous[index];
      const right = previous[index + 1];
      const joined = next[index];
      if (left === undefined || right === undefined || joined === undefined) return undefined;
      if (joined.manuscript_block_id !== left.manuscript_block_id) continue;
      if (joined.text !== left.text + right.text) continue;
      const prefix = previous.slice(0, index);
      const suffix = previous.slice(index + 2);
      const nextPrefix = next.slice(0, index);
      const nextSuffix = next.slice(index + 1);
      if (!paragraphsEqual(prefix, nextPrefix) || !paragraphsEqual(suffix, nextSuffix)) {
        continue;
      }
      const resultingBlocks = next.map((block) => ({ ...block }));
      return {
        kind: "join_blocks",
        left_manuscript_block_id: left.manuscript_block_id,
        right_manuscript_block_id: right.manuscript_block_id,
        caret: left.text.length,
        resultingBlocks,
        resultingBody: flattenChapterBody(resultingBlocks),
      };
    }
  }
  return encodeContiguousReplacement(previous, next);
}

function encodeContiguousReplacement(
  previous: readonly ManuscriptParagraph[],
  next: readonly ManuscriptParagraph[],
): CapturedManuscriptEdit | undefined {
  let prefix = 0;
  const shared = Math.min(previous.length, next.length);
  while (prefix < shared
    && previous[prefix]?.manuscript_block_id === next[prefix]?.manuscript_block_id
    && previous[prefix]?.text === next[prefix]?.text) {
    prefix += 1;
  }
  let suffix = 0;
  while (prefix + suffix < previous.length
    && prefix + suffix < next.length
    && previous[previous.length - 1 - suffix]?.manuscript_block_id
      === next[next.length - 1 - suffix]?.manuscript_block_id
    && previous[previous.length - 1 - suffix]?.text === next[next.length - 1 - suffix]?.text) {
    suffix += 1;
  }
  const prevMiddle = previous.slice(prefix, previous.length - suffix);
  const nextMiddle = next.slice(prefix, next.length - suffix);
  const left = prevMiddle[0];
  const nextLeft = nextMiddle[0];
  if (left === undefined || nextLeft === undefined
    || left.manuscript_block_id !== nextLeft.manuscript_block_id) {
    return undefined;
  }
  const previousIds = new Set(previous.map((block) => block.manuscript_block_id));
  if (nextMiddle.slice(1).some((block) => previousIds.has(block.manuscript_block_id))) {
    return undefined;
  }
  const primitives: ContiguousReplacementPrimitive[] = [];
  let joined = left.text;
  for (const removed of prevMiddle.slice(1)) {
    primitives.push({
      kind: "join_blocks",
      left_manuscript_block_id: left.manuscript_block_id,
      right_manuscript_block_id: removed.manuscript_block_id,
    });
    joined += removed.text;
  }
  const nextJoined = nextMiddle.map((block) => block.text).join("");
  const replace = contiguousUtf16Replace(joined, nextJoined);
  if (replace === undefined) return undefined;
  primitives.push({
    kind: "replace_block_selection",
    manuscript_block_id: left.manuscript_block_id,
    from: replace.from,
    to: replace.to,
    text: replace.text,
  });
  let splitTarget = left.manuscript_block_id;
  for (let index = 1; index < nextMiddle.length; index += 1) {
    const created = nextMiddle[index]!;
    primitives.push({
      kind: "split_block",
      manuscript_block_id: splitTarget,
      offset: nextMiddle[index - 1]!.text.length,
      new_manuscript_block_id: created.manuscript_block_id,
    });
    splitTarget = created.manuscript_block_id;
  }
  if (primitives.length <= 1) return undefined;
  const resultingBlocks = next.map((block) => ({ ...block }));
  return {
    kind: "contiguous_replacement",
    primitives,
    from: replace.from,
    to: replace.to,
    resultingBlocks,
    resultingBody: flattenChapterBody(resultingBlocks),
  };
}

export function paragraphsFromPlainTextReplacement(
  blocks: readonly ManuscriptParagraph[],
  startIndex: number,
  startOffset: number,
  endIndex: number,
  endOffset: number,
  text: string,
  mintId: () => string,
): ManuscriptParagraph[] | undefined {
  const start = blocks[startIndex];
  const end = blocks[endIndex];
  if (start === undefined || end === undefined
    || startIndex > endIndex
    || startOffset < 0 || startOffset > start.text.length
    || endOffset < 0 || endOffset > end.text.length) {
    return undefined;
  }
  const head = start.text.slice(0, startOffset);
  const tail = end.text.slice(endOffset);
  const parts = text.split(/\r\n|\n|\r/);
  const prefix = blocks.slice(0, startIndex).map((block) => ({ ...block }));
  const suffix = blocks.slice(endIndex + 1).map((block) => ({ ...block }));
  if (parts.length <= 1) {
    return [
      ...prefix,
      { manuscript_block_id: start.manuscript_block_id, text: head + (parts[0] ?? "") + tail },
      ...suffix,
    ];
  }
  return [
    ...prefix,
    { manuscript_block_id: start.manuscript_block_id, text: head + parts[0] },
    ...parts.slice(1, -1).map((part) => ({
      manuscript_block_id: mintId(),
      text: part,
    })),
    { manuscript_block_id: mintId(), text: `${parts[parts.length - 1] ?? ""}${tail}` },
    ...suffix,
  ];
}

