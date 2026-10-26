import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";

function isCandidate(node: ProseMirrorNode): boolean {
  return node.type.name === "blockProposal" || node.type.name === "inlineProposal";
}

/**
 * The part of a selection in its first owner (ADR 0072). The first owner is the Chapter text or the
 * candidate of the first selected character in document order. The part stops at the next text of another
 * owner, so a selection that crosses no owner boundary is the complete range.
 */
export function firstOwnerRange(doc: ProseMirrorNode, from: number, to: number): { from: number; to: number } {
  let first: number | undefined;
  doc.nodesBetween(from, to, (node, position) => {
    if (first === undefined && node.isText && Math.max(from, position) < Math.min(to, position + node.nodeSize)) {
      first = Math.max(from, position);
    }
    return first === undefined;
  });
  if (first === undefined) return { from, to };
  const $first = doc.resolve(first);
  for (let depth = $first.depth; depth > 0; depth -= 1) {
    if (isCandidate($first.node(depth))) {
      return { from: Math.max(from, $first.start(depth)), to: Math.min(to, $first.end(depth)) };
    }
  }
  const parent = doc.resolve(from).parent.type.name;
  const start = parent === "paragraph" || parent === "heading" ? from : first;
  let end: number | undefined;
  doc.nodesBetween(start, to, (node, position) => {
    if (end === undefined && isCandidate(node) && position >= start) {
      // A block candidate follows the closing token of the Chapter block before it.
      end = Math.min(to, node.type.name === "blockProposal" ? position - 1 : position);
    }
    return end === undefined;
  });
  return { from: start, to: end ?? to };
}

/** Narrows a selection that crosses owners to the part in its first owner. Returns true when it narrowed it. */
export function narrowToFirstOwner(view: EditorView): boolean {
  const { from, to, empty } = view.state.selection;
  if (empty) return false;
  const range = firstOwnerRange(view.state.doc, from, to);
  if (range.from === from && range.to === to) return false;
  view.dispatch(view.state.tr.setSelection(TextSelection.create(view.state.doc, range.from, range.to)));
  return true;
}
