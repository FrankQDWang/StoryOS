import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import { createRefusedEditDraft } from "./proposal-contract.js";

export const proposalPluginKey = new PluginKey("storyosProposalProjection");
export const PROPOSAL_META = "storyosProposalMeta";
export const OWNERSHIP_META = "storyosOwnership";

function topLevelBlocks(doc, proposalIds) {
  const ids = new Set(proposalIds);
  const blocks = [];

  doc.forEach((node, offset) => {
    blocks.push({
      from: offset,
      to: offset + node.nodeSize,
      blockId: node.attrs?.blockId,
      proposal: ids.has(node.attrs?.blockId),
      node,
    });
  });

  return blocks;
}

export function findProposalEnvelope(doc, proposalIds) {
  const selected = topLevelBlocks(doc, proposalIds).filter(
    (block) => block.proposal,
  );

  if (!selected.length) return null;

  return {
    from: selected[0].from,
    to: selected[selected.length - 1].to,
    blocks: selected,
  };
}

function insertionOwnership(position, blocks) {
  const proposalBlocks = blocks.filter((block) => block.proposal);
  const firstProposal = proposalBlocks[0];
  const lastProposal = proposalBlocks.at(-1);
  if (
    !firstProposal ||
    position === firstProposal.from ||
    position === lastProposal.to
  ) {
    return "authority";
  }

  const next = blocks.find((block) => block.from === position);
  if (next?.proposal) return "proposal";

  const current = blocks.find(
    (block) => position > block.from && position < block.to,
  );
  if (current) return current.proposal ? "proposal" : "authority";

  const previous = [...blocks].reverse().find((block) => block.to === position);
  return previous?.proposal ? "proposal" : "authority";
}

function spanOwnership(from, to, blocks) {
  if (from === to) return insertionOwnership(from, blocks);

  const touched = blocks.filter(
    (block) => from < block.to && to > block.from,
  );
  const hasProposal = touched.some((block) => block.proposal);
  const hasAuthority = touched.some((block) => !block.proposal);

  if (hasProposal && hasAuthority) return "mixed";
  if (hasProposal) return "proposal";
  return "authority";
}

export function classifyTransaction(transaction, doc, proposalIds) {
  const blocks = topLevelBlocks(doc, proposalIds);
  const ownerships = new Set();

  transaction.steps.forEach((step) => {
    step.getMap().forEach((oldStart, oldEnd) => {
      ownerships.add(spanOwnership(oldStart, oldEnd, blocks));
    });
  });

  if (ownerships.has("mixed")) return "mixed";
  if (ownerships.has("proposal") && ownerships.has("authority")) return "mixed";
  if (ownerships.has("proposal")) return "proposal";
  if (ownerships.has("authority")) return "authority";
  return "none";
}

function buildDecorations(doc, blockIds) {
  const selected = topLevelBlocks(doc, blockIds).filter(
    (block) => block.proposal,
  );
  const decorations = selected.map((block, index) => {
    const classes = ["proposal-block"];
    if (index === 0) classes.push("proposal-block-first");
    if (index === selected.length - 1) classes.push("proposal-block-last");

    return Decoration.node(block.from, block.to, {
      class: classes.join(" "),
      "data-proposal-block": block.blockId,
    });
  });

  return DecorationSet.create(doc, decorations);
}

export const ProposalProjection = Extension.create({
  name: "storyosProposalProjection",

  addOptions() {
    return {
      initialBlockIds: [],
      onAuthorSignal: () => {},
      onRefusedTransaction: () => {},
      onUndoRequest: () => false,
      onRedoRequest: () => false,
    };
  },

  addProseMirrorPlugins() {
    const options = this.options;

    return [
      new Plugin({
        key: proposalPluginKey,
        state: {
          init: () => ({ blockIds: options.initialBlockIds }),
          apply(transaction, value) {
            const meta = transaction.getMeta(proposalPluginKey);
            if (meta?.type === "setProjection") {
              return { blockIds: meta.blockIds };
            }
            return value;
          },
        },
        filterTransaction(transaction, state) {
          if (!transaction.docChanged) return true;

          const commandMeta = transaction.getMeta(PROPOSAL_META);
          if (commandMeta?.allow) return true;

          const { blockIds } = proposalPluginKey.getState(state);
          if (!blockIds.length) return true;

          const ownership = classifyTransaction(
            transaction,
            state.doc,
            blockIds,
          );
          transaction.setMeta(OWNERSHIP_META, ownership);

          if (ownership !== "mixed") return true;

          const draft = createRefusedEditDraft({
            intent: "mixed_owner_transaction",
            beforeDoc: state.doc,
            afterDoc: transaction.doc,
            beforeSelection: state.selection.toJSON(),
            afterSelection: transaction.selection.toJSON(),
            steps: transaction.steps.map((step) => step.toJSON()),
          });
          queueMicrotask(() => options.onRefusedTransaction(draft));
          return false;
        },
        props: {
          decorations(state) {
            const { blockIds } = proposalPluginKey.getState(state);
            return buildDecorations(state.doc, blockIds);
          },
          handleDOMEvents: {
            compositionstart() {
              options.onAuthorSignal("compositionstart");
              return false;
            },
            beforeinput(_view, event) {
              if (event.inputType === "historyUndo" && options.onUndoRequest()) {
                event.preventDefault();
                return true;
              }
              if (event.inputType === "historyRedo" && options.onRedoRequest()) {
                event.preventDefault();
                return true;
              }
              options.onAuthorSignal(event.inputType || "beforeinput");
              return false;
            },
            paste() {
              options.onAuthorSignal("paste");
              return false;
            },
            drop() {
              options.onAuthorSignal("drop");
              return false;
            },
            cut() {
              options.onAuthorSignal("cut");
              return false;
            },
          },
          handleKeyDown(_view, event) {
            if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "z") {
              return false;
            }
            if (event.shiftKey ? options.onRedoRequest() : options.onUndoRequest()) {
              event.preventDefault();
              return true;
            }
            return false;
          },
        },
      }),
    ];
  },
});
