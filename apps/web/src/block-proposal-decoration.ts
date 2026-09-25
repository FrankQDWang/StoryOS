import { Node as TiptapNode, type Editor } from "@tiptap/core";
import { Fragment, type Node as ProseMirrorNode } from "@tiptap/pm/model";

import { contiguousUtf16Replace } from "./manuscript-doc.ts";

export type BlockProposalProjection = {
  proposalId: string;
  operationId: string;
  revisionId: string;
  blockId: string;
  sourceRunId: string;
  sourceDecisionId: string;
  text: string;
  eligible: boolean;
  expectedHeads: string[];
  localPending?: boolean;
};

const ATTRIBUTES = [
  "proposalId", "operationId", "revisionId", "blockId", "sourceRunId",
  "sourceDecisionId", "eligible", "expectedHeads",
] as const;

function candidateNodes(doc: ProseMirrorNode): ProseMirrorNode[] {
  const nodes: ProseMirrorNode[] = [];
  doc.forEach((node) => {
    if (node.type.name === "blockProposal") nodes.push(node);
  });
  return nodes;
}

function candidateAnchorsValid(doc: ProseMirrorNode): boolean {
  let blockId = "";
  let valid = true;
  doc.forEach((node) => {
    if (node.type.name === "paragraph" || node.type.name === "heading") {
      blockId = node.attrs.id as string;
    } else if (node.type.name === "blockProposal"
      && (blockId === "" || node.attrs.blockId !== blockId)) {
      valid = false;
    }
  });
  return valid;
}

export function capturedCandidateEdit(previous: ProseMirrorNode, next: ProseMirrorNode) {
  const before = candidateNodes(previous);
  const after = candidateNodes(next);
  if (before.length !== after.length || !candidateAnchorsValid(previous)
    || !candidateAnchorsValid(next)) return { valid: false as const };
  const currentById = new Map(after.map((node) => [node.attrs.proposalId as string, node]));
  if (currentById.size !== after.length) return { valid: false as const };
  let changed: { proposal: BlockProposalProjection; priorText: string;
    from: number; to: number; text: string; resultingBody: string } | undefined;
  for (const node of before) {
    const current = currentById.get(node.attrs.proposalId as string);
    if (current === undefined || ATTRIBUTES.some((key) =>
      JSON.stringify(node.attrs[key]) !== JSON.stringify(current.attrs[key]))) {
      return { valid: false as const };
    }
    if (node.textContent === current.textContent) continue;
    if (changed !== undefined) return { valid: false as const };
    const replacement = contiguousUtf16Replace(node.textContent, current.textContent);
    if (replacement === undefined) return { valid: false as const };
    changed = {
      proposal: {
        proposalId: node.attrs.proposalId as string,
        operationId: node.attrs.operationId as string,
        revisionId: node.attrs.revisionId as string,
        blockId: node.attrs.blockId as string,
        sourceRunId: node.attrs.sourceRunId as string,
        sourceDecisionId: node.attrs.sourceDecisionId as string,
        text: current.textContent,
        eligible: node.attrs.eligible as boolean,
        expectedHeads: node.attrs.expectedHeads as string[],
      },
      priorText: node.textContent,
      ...replacement,
    };
    if (!changed.proposal.eligible) return { valid: false as const };
  }
  return { valid: true as const, edit: changed };
}

export function projectBlockProposals(editor: Editor, proposals: readonly BlockProposalProjection[]): void {
  const schema = editor.state.schema;
  const candidateType = schema.nodes.blockProposal;
  if (candidateType === undefined) return;
  const existing = new Map(candidateNodes(editor.state.doc).map((node) =>
    [node.attrs.proposalId as string, node]));
  const byBlock = new Map<string, BlockProposalProjection[]>();
  for (const proposal of proposals) {
    const items = byBlock.get(proposal.blockId) ?? [];
    items.push(proposal);
    byBlock.set(proposal.blockId, items);
  }
  const next: ProseMirrorNode[] = [];
  editor.state.doc.forEach((node) => {
    if (node.type.name === "blockProposal") return;
    next.push(node);
    for (const proposal of byBlock.get(node.attrs.id as string) ?? []) {
      const current = existing.get(proposal.proposalId);
      const text = !proposal.localPending && current?.attrs.revisionId === proposal.revisionId
        ? current.textContent : proposal.text;
      next.push(candidateType.create({
        proposalId: proposal.proposalId,
        operationId: proposal.operationId,
        revisionId: proposal.revisionId,
        blockId: proposal.blockId,
        sourceRunId: proposal.sourceRunId,
        sourceDecisionId: proposal.sourceDecisionId,
        eligible: proposal.eligible,
        expectedHeads: proposal.expectedHeads,
      }, text.length ? schema.text(text) : undefined));
    }
  });
  if (next.length === editor.state.doc.childCount
    && next.every((node, index) => node.eq(editor.state.doc.child(index)))) return;
  const transaction = editor.state.tr.replaceWith(0, editor.state.doc.content.size,
    Fragment.fromArray(next));
  transaction.setMeta("storyos.hydrate", true);
  transaction.setMeta("addToHistory", false);
  editor.view.dispatch(transaction);
}

export const blockProposalDecoration = TiptapNode.create({
  name: "blockProposal",
  group: "block",
  content: "text*",
  selectable: false,
  isolating: true,
  addAttributes() {
    return Object.fromEntries(ATTRIBUTES.map((key) => [key, { default: null }]));
  },
  parseHTML() {
    return [{ tag: "div[data-proposal-id]" }];
  },
  renderHTML({ node }) {
    const eligible = node.attrs.eligible === true;
    return ["div", {
      class: "block-proposal",
      "data-proposal-id": node.attrs.proposalId,
      "data-proposal-operation-id": node.attrs.operationId,
      "data-proposal-revision-id": node.attrs.revisionId,
      "data-proposal-source-run-id": node.attrs.sourceRunId,
      "data-proposal-source-decision-id": node.attrs.sourceDecisionId,
      "data-proposal-target-id": node.attrs.blockId,
      "data-proposal-eligibility": eligible ? "eligible" : "ineligible",
      role: "group",
      "aria-label": eligible ? "候选文字，尚未成为正文" : "候选文字，暂不可接受",
      ...(eligible ? {} : { contenteditable: "false" }),
    }, ["span", { class: "block-proposal-label", contenteditable: "false" },
      eligible ? "候选文字 · 尚未成为正文" : "候选文字 · 暂不可接受"],
    ["p", { class: "block-proposal-text" }, 0]];
  },
});
