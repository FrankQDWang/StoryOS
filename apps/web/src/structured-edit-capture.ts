import type { EditorState, Transaction } from "@tiptap/pm/state";
import { ReplaceStep } from "@tiptap/pm/transform";
import type { AuthorEditUnit, SelectedEditSource, ReplacementBlock }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export interface StructuredSelectionEdit {
  kind: "structured_selection";
  authorEditUnit: AuthorEditUnit;
  expectedProposalHeads: string[];
}

export function captureStructuredSelection(state: EditorState, transaction: Transaction)
  : StructuredSelectionEdit | undefined {
  const [step] = transaction.steps;
  if (!(step instanceof ReplaceStep) || transaction.steps.length !== 1
    || state.selection.empty || step.from !== state.selection.from
    || step.to !== state.selection.to) return undefined;
  const sources: SelectedEditSource[] = [];
  const expectedHeads = new Set<string>();
  let valid = true;
  state.doc.forEach((node, position) => {
    if (node.type.name === "blockProposal") {
      for (const head of node.attrs.expectedHeads as string[]) expectedHeads.add(head);
    }
    const start = position + 1;
    const end = start + node.textContent.length;
    if (step.from > end || step.to < start) return;
    const candidate = node.type.name === "blockProposal";
    if (candidate && node.attrs.eligible !== true
      || !candidate && node.type.name !== "paragraph" && node.type.name !== "heading") valid = false;
    sources.push({
      owner: candidate ? { kind: "proposal", proposal_id: node.attrs.proposalId as string,
        operation_id: node.attrs.operationId as string, revision_id: node.attrs.revisionId as string,
        manuscript_block_id: node.attrs.blockId as string }
        : { kind: "manuscript", manuscript_block_id: node.attrs.id as string },
      coordinate_profile: candidate ? "storyos.editor.utf16-code-unit.v1" : "prosemirror-token-utf16.v1",
      from: Math.max(0, step.from - start), to: Math.min(node.textContent.length, step.to - start),
      block_kind: node.type.name === "heading" ? "heading" : "paragraph", source_text: node.textContent,
    });
  });
  const owners = new Set(sources.map((source) => source.owner.kind === "manuscript"
    ? "manuscript" : source.owner.proposal_id));
  if (!valid || owners.size < 2 || !sources.some((source) => source.owner.kind === "proposal")) return undefined;
  const replacement: ReplacementBlock[] = [];
  step.slice.content.forEach((node) => {
    if (!node.isText && node.type.name !== "paragraph" && node.type.name !== "heading") valid = false;
    const clipboard = transaction.getMeta("storyos.origin") === "paste"
      || transaction.getMeta("storyos.origin") === "drop";
    const parts = node.isText && clipboard ? node.textContent.split(/\r\n|\n|\r/) : [node.textContent];
    for (const [index, text] of parts.entries()) replacement.push({
      block_kind: node.type.name === "heading" || (node.isText && index === 0
        && sources[0]!.block_kind === "heading") ? "heading" : "paragraph", text,
    });
  });
  if (!valid) return undefined;
  if (replacement.length === 0) replacement.push({ block_kind: sources[0]!.block_kind, text: "" });
  const first = { source_index: 0, source_offset: sources[0]!.from };
  const last = { source_index: sources.length - 1, source_offset: sources.at(-1)!.to };
  const anchor = state.selection.anchor <= state.selection.head ? first : last;
  const head = state.selection.anchor <= state.selection.head ? last : first;
  return { kind: "structured_selection", expectedProposalHeads: [...expectedHeads].sort(),
    authorEditUnit: { normalized_primitives: [{ kind: "replace_structured_selection", replacement }],
      selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1",
        from: anchor.source_offset, to: head.source_offset,
        ordered_selection: { sources, anchor, head } } } };
}
