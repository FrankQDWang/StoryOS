import { Extension, type Editor } from "@tiptap/core";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import { Plugin, PluginKey } from "@tiptap/pm/state";

export type BlockProposalProjection = {
  proposalId: string;
  operationId: string;
  revisionId: string;
  blockId: string;
  sourceRunId: string;
  sourceDecisionId: string;
  text: string;
  eligible: boolean;
};

const projectionKey = new PluginKey<readonly BlockProposalProjection[]>("storyosBlockProposals");

export function projectBlockProposals(editor: Editor, proposals: readonly BlockProposalProjection[]): void {
  const current = projectionKey.getState(editor.state) ?? [];
  if (current.length === proposals.length && current.every((item, index) =>
    JSON.stringify(item) === JSON.stringify(proposals[index]))) return;
  editor.view.dispatch(editor.state.tr.setMeta(projectionKey, proposals));
}

export const blockProposalDecoration = Extension.create({
  name: "storyosBlockProposalDecoration",
  addProseMirrorPlugins() {
    return [new Plugin<readonly BlockProposalProjection[]>({
      key: projectionKey,
      state: {
        init: () => [],
        apply(transaction, previous) {
          return transaction.getMeta(projectionKey) ?? previous;
        },
      },
      props: {
        decorations(state) {
          const proposals = projectionKey.getState(state) ?? [];
          const decorations: Decoration[] = [];
          state.doc.descendants((node, position) => {
            if (node.type.name !== "paragraph" && node.type.name !== "heading") return;
            for (const proposal of proposals) {
              if (node.attrs.id !== proposal.blockId) continue;
              decorations.push(Decoration.widget(position + node.nodeSize, () => {
                const surface = document.createElement("div");
                surface.className = "block-proposal";
                surface.contentEditable = "false";
                surface.setAttribute("role", "note");
                surface.setAttribute("aria-label", "候选文字，尚未成为正文");
                surface.dataset.proposalId = proposal.proposalId;
                surface.dataset.proposalOperationId = proposal.operationId;
                surface.dataset.proposalRevisionId = proposal.revisionId;
                surface.dataset.proposalSourceRunId = proposal.sourceRunId;
                surface.dataset.proposalSourceDecisionId = proposal.sourceDecisionId;
                surface.dataset.proposalTargetId = proposal.blockId;
                surface.dataset.proposalEligibility = proposal.eligible ? "eligible" : "ineligible";
                const label = document.createElement("span");
                label.className = "block-proposal-label";
                label.textContent = proposal.eligible
                  ? "候选文字 · 尚未成为正文" : "候选文字 · 暂不可接受";
                const text = document.createElement("p");
                text.textContent = proposal.text;
                surface.append(label, text);
                return surface;
              }, { key: `${proposal.proposalId}:${proposal.revisionId}:${proposal.eligible}`, side: -1 }));
            }
          });
          return DecorationSet.create(state.doc, decorations);
        },
      },
    })];
  },
});
