import type { Node } from "@tiptap/pm/model";

import type { ReplaceSelectionEdit } from "./editor-types.ts";

export function manuscriptJson(blockId: string, body: string) {
  return {
    type: "doc",
    content: [{
      type: "paragraph",
      attrs: { id: blockId },
      ...(body.length === 0 ? {} : { content: [{ type: "text", text: body }] }),
    }],
  };
}

function isUtf16Boundary(body: string, offset: number): boolean {
  if (!Number.isSafeInteger(offset) || offset < 0 || offset > body.length) return false;
  if (offset === 0 || offset === body.length) return true;
  const prior = body.charCodeAt(offset - 1);
  const next = body.charCodeAt(offset);
  return !(prior >= 0xd800 && prior <= 0xdbff && next >= 0xdc00 && next <= 0xdfff);
}

export function paragraphUtf16(doc: Node): string | undefined {
  if (doc.childCount !== 1) return undefined;
  const paragraph = doc.firstChild;
  if (paragraph === null || paragraph.type.name !== "paragraph") return undefined;
  const id: unknown = paragraph.attrs.id;
  if (typeof id !== "string") return undefined;
  let text = "";
  for (let offset = 0; offset < paragraph.childCount; offset += 1) {
    const child = paragraph.child(offset);
    if (!child.isText || child.marks.length > 0) return undefined;
    text += child.text ?? "";
  }
  return text;
}

export function isSupportedManuscriptDoc(previous: Node, next: Node, blockId: string): boolean {
  const nextText = paragraphUtf16(next);
  if (nextText === undefined) return false;
  const nextId: unknown = next.firstChild?.attrs.id;
  if (nextId !== blockId) return false;
  if (previous.childCount === 0) return true;
  const previousId: unknown = previous.firstChild?.attrs.id;
  return previousId === undefined || previousId === blockId;
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
