import { Node as TiptapNode } from "@tiptap/core";
import { Fragment, type Node as ProseMirrorNode } from "@tiptap/pm/model";
import { ReplaceStep } from "@tiptap/pm/transform";
import { TextSelection, type EditorState, type Transaction } from "@tiptap/pm/state";
import type { BlockProposalInspect, SelectedEditSource } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { isUtf16Boundary, type ManuscriptParagraph } from "./manuscript-doc.ts";
import { PROPOSAL_ATTRIBUTES, type BlockProposalProjection } from "./block-proposal-decoration.ts";

export function inlineProjectionAnchor(proposal: BlockProposalInspect,
  blocks: readonly ManuscriptParagraph[], revisionId: string) {
  if (proposal.kind !== "inline_edit" || proposal.anchors.length !== 1) return undefined;
  const anchor = proposal.anchors[0]!;
  const block = blocks.find((item) => item.manuscript_block_id === proposal.manuscript_block_id);
  if (block === undefined || anchor.manuscript_block_id !== block.manuscript_block_id
    || anchor.base_authoritative_revision_id !== revisionId
    || anchor.coordinate_profile !== "prosemirror-token-utf16.v1"
    || anchor.boundary_profile !== "exclusive-authoritative-edges.v1"
    || anchor.manuscript_schema_version !== 1
    || !Number.isSafeInteger(anchor.from) || !Number.isSafeInteger(anchor.to)
    || anchor.from < 0 || anchor.to <= anchor.from || anchor.to > block.text.length
    || !isUtf16Boundary(block.text, anchor.from) || !isUtf16Boundary(block.text, anchor.to)) return undefined;
  return { from: anchor.from, to: anchor.to, sourceText: block.text.slice(anchor.from, anchor.to) };
}

export function projectInlineCandidate(node: ProseMirrorNode, proposal: BlockProposalProjection,
  attrs: Record<string, unknown>, text: string): ProseMirrorNode {
  const anchor = proposal.inlineAnchor;
  const candidateType = node.type.schema.nodes.inlineProposal;
  if (anchor === undefined || candidateType === undefined) return node;
  const children: ProseMirrorNode[] = [];
  if (anchor.from > 0) children.push(node.type.schema.text(node.textContent.slice(0, anchor.from)));
  children.push(candidateType.create({ ...attrs, inlineFrom: anchor.from,
    inlineTo: anchor.to, sourceText: anchor.sourceText }, text ? node.type.schema.text(text) : undefined));
  if (anchor.to < node.textContent.length) children.push(node.type.schema.text(node.textContent.slice(anchor.to)));
  return node.copy(Fragment.fromArray(children));
}

export function restoreInlineSource(node: ProseMirrorNode): ProseMirrorNode {
  const children: ProseMirrorNode[] = [];
  node.forEach((child) => {
    if (child.type.name !== "inlineProposal") children.push(child);
    else if (child.attrs.sourceText) children.push(node.type.schema.text(child.attrs.sourceText as string));
  });
  return node.copy(Fragment.fromArray(children));
}

export function inlineSelectionSources(node: ProseMirrorNode, position: number, from: number, to: number) {
  if (!Array.from({ length: node.childCount }, (_, index) => node.child(index)).some((child) => child.type.name === "inlineProposal")) return undefined;
  const sourceText = restoreInlineSource(node).textContent;
  const sources: SelectedEditSource[] = [];
  let sourceOffset = 0;
  let valid = true;
  node.forEach((child, childPosition) => {
    const candidate = child.type.name === "inlineProposal";
    const start = position + 1 + childPosition + (candidate ? 1 : 0);
    const end = start + child.textContent.length;
    const sourceStart = candidate ? 0 : sourceOffset;
    if (from < end && to > start) {
      if (candidate && child.attrs.eligible !== true) valid = false;
      sources.push({
        owner: candidate ? { kind: "proposal", proposal_id: child.attrs.proposalId as string,
          operation_id: child.attrs.operationId as string, revision_id: child.attrs.revisionId as string,
          manuscript_block_id: child.attrs.blockId as string }
          : { kind: "manuscript", manuscript_block_id: node.attrs.id as string },
        coordinate_profile: candidate ? "storyos.editor.utf16-code-unit.v1" : "prosemirror-token-utf16.v1",
        from: sourceStart + Math.max(0, from - start),
        to: sourceStart + Math.min(child.textContent.length, to - start),
        block_kind: node.type.name === "heading" ? "heading" : "paragraph",
        source_text: candidate ? child.textContent : sourceText,
      });
    }
    sourceOffset += candidate ? (child.attrs.sourceText as string).length : child.textContent.length;
  });
  return { sources, valid };
}

export function routeInlineEdgeInsertion(transaction: Transaction, state: EditorState): boolean {
  const [step] = transaction.steps;
  if (!(step instanceof ReplaceStep) || transaction.steps.length !== 1 || step.from !== step.to) return false;
  const position = state.doc.resolve(step.from);
  if (position.parent.type.name === "inlineProposal") {
    if (position.parentOffset !== 0 && position.parentOffset !== position.parent.content.size) return false;
    const outside = position.parentOffset === 0 ? position.before() : position.after();
    transaction.replaceWith(0, transaction.doc.content.size, state.doc.content);
    transaction.replaceWith(outside, outside, step.slice.content);
    transaction.setSelection(TextSelection.create(transaction.doc, outside + step.slice.content.size));
    transaction.setMeta("storyos.inlineEdgeHeads", position.parent.attrs.expectedHeads);
  } else {
    const candidate = position.nodeAfter?.type.name === "inlineProposal" ? position.nodeAfter
      : position.nodeBefore?.type.name === "inlineProposal" ? position.nodeBefore : undefined;
    if (candidate === undefined) return false;
    transaction.setMeta("storyos.inlineEdgeHeads", candidate.attrs.expectedHeads);
  }
  return true;
}

export const inlineProposalDecoration = TiptapNode.create({
  name: "inlineProposal", group: "inline", inline: true, content: "text*",
  selectable: false, isolating: true,
  addAttributes() {
    return Object.fromEntries(PROPOSAL_ATTRIBUTES.map((key) => [key, { default: null }]));
  },
  parseHTML() { return [{ tag: "span[data-inline-proposal-id]" }]; },
  renderHTML({ node }) {
    return ["span", { class: "inline-proposal", "data-inline-proposal-id": node.attrs.proposalId,
      "data-candidate-proposal-id": node.attrs.proposalId,
      "data-proposal-operation-id": node.attrs.operationId,
      "data-proposal-revision-id": node.attrs.revisionId,
      "data-proposal-target-id": node.attrs.blockId,
      "aria-label": "候选文字，尚未成为正文",
      ...(node.attrs.eligible === true ? {} : { contenteditable: "false" }) }, 0];
  },
});
