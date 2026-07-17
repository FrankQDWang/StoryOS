import { Extension } from "@tiptap/core";
import { Plugin, PluginKey, TextSelection } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import {
  classifyDocumentDiff,
  classifyInlineTransaction,
  createRefusedEditDraft,
  mapInlineRange,
} from "./proposal-contract.js";

export const inlineProposalPluginKey = new PluginKey(
  "storyosInlineProposalContractProbe",
);
export const INLINE_PROPOSAL_META = "storyosInlineProposalMeta";

function buildDraft(transaction, state, intent) {
  return createRefusedEditDraft({
    intent,
    beforeDoc: transaction.before,
    afterDoc: transaction.doc,
    beforeSelection: state.selection.toJSON(),
    afterSelection: transaction.selection.toJSON(),
    steps: transaction.steps.map((step) => step.toJSON()),
  });
}

function createDecoration(doc, range, mode) {
  if (range.from >= range.to || range.to > doc.content.size) {
    return DecorationSet.empty;
  }

  return DecorationSet.create(doc, [
    Decoration.inline(
      range.from,
      range.to,
      {
        class: `contract-proposal contract-proposal-${mode}`,
        "data-proposal-owner": "proposal",
      },
      { inclusiveStart: false, inclusiveEnd: false },
    ),
  ]);
}

/**
 * Disposable Tiptap contract probe for exclusive inline Proposal ownership.
 * It refuses mixed-owner transactions atomically and defers IME ownership
 * classification until the browser completes one composition intent.
 */
export const InlineProposalContract = Extension.create({
  name: "storyosInlineProposalContract",

  addOptions() {
    return {
      initialRange: { from: 1, to: 1 },
      initialMode: "full",
      onEvent: () => {},
      onRefusedDraft: () => {},
      onCompositionStart: () => {},
      onCompositionEnd: () => {},
    };
  },

  addProseMirrorPlugins() {
    const options = this.options;
    let composition = null;

    return [
      new Plugin({
        key: inlineProposalPluginKey,
        state: {
          init: () => ({
            range: options.initialRange,
            mode: options.initialMode,
            lastOwnership: "none",
          }),
          apply(transaction, value) {
            const meta = transaction.getMeta(inlineProposalPluginKey);
            if (meta?.type === "reset") {
              return {
                range: meta.range,
                mode: meta.mode,
                lastOwnership: "none",
              };
            }
            if (meta?.type === "setMode") {
              return { ...value, mode: meta.mode };
            }

            return {
              ...value,
              range: transaction.docChanged
                ? mapInlineRange(value.range, transaction.mapping)
                : value.range,
              lastOwnership:
                transaction.getMeta(INLINE_PROPOSAL_META)?.ownership ??
                value.lastOwnership,
            };
          },
        },
        filterTransaction(transaction, state) {
          if (!transaction.docChanged) return true;

          const command = transaction.getMeta(INLINE_PROPOSAL_META);
          if (command?.allow) return true;

          const probe = inlineProposalPluginKey.getState(state);
          const ownership = classifyInlineTransaction(transaction, probe.range);
          transaction.setMeta(INLINE_PROPOSAL_META, {
            ownership,
            intent: command?.intent ?? "editor_transaction",
          });

          if (composition) {
            options.onEvent({ type: "composition_update", ownership });
            return true;
          }

          if (probe.mode === "safe" && ownership !== "authority") {
            const draft = buildDraft(transaction, state, "safe_mode_refusal");
            queueMicrotask(() => options.onRefusedDraft(draft));
            return false;
          }

          if (ownership !== "mixed") return true;

          const draft = buildDraft(
            transaction,
            state,
            command?.intent ?? "mixed_owner_transaction",
          );
          queueMicrotask(() => options.onRefusedDraft(draft));
          return false;
        },
        props: {
          decorations(state) {
            const { range, mode } = inlineProposalPluginKey.getState(state);
            return createDecoration(state.doc, range, mode);
          },
          handleDOMEvents: {
            paste() {
              options.onEvent({ type: "native_paste" });
              return false;
            },
            cut() {
              options.onEvent({ type: "native_cut" });
              return false;
            },
            dragstart() {
              options.onEvent({ type: "native_dragstart" });
              return false;
            },
            drop() {
              options.onEvent({ type: "native_drop" });
              return false;
            },
            compositionstart(view) {
              const { range } = inlineProposalPluginKey.getState(view.state);
              composition = {
                beforeDoc: view.state.doc,
                beforeSelection: view.state.selection.toJSON(),
                range,
              };
              options.onCompositionStart();
              options.onEvent({ type: "composition_start" });
              return false;
            },
            compositionend(view) {
              const session = composition;
              composition = null;
              if (!session) return false;

              window.setTimeout(() => {
                const ownership = classifyDocumentDiff(
                  session.beforeDoc,
                  view.state.doc,
                  session.range,
                );
                const probe = inlineProposalPluginKey.getState(view.state);
                const refused =
                  ownership === "mixed" ||
                  (probe.mode === "safe" && ownership !== "authority");

                if (refused) {
                  const attemptedDoc = view.state.doc;
                  const attemptedSelection = view.state.selection.toJSON();
                  const transaction = view.state.tr
                    .replaceWith(
                      0,
                      view.state.doc.content.size,
                      session.beforeDoc.content,
                    )
                    .setMeta(inlineProposalPluginKey, {
                      type: "reset",
                      range: session.range,
                      mode: probe.mode,
                    })
                    .setMeta(INLINE_PROPOSAL_META, {
                      allow: true,
                      ownership: "mixed",
                      intent: "composition_recovery",
                    })
                    .setMeta("addToHistory", false);
                  transaction.setSelection(
                    TextSelection.create(
                      transaction.doc,
                      session.beforeSelection.anchor,
                      session.beforeSelection.head,
                    ),
                  );
                  view.dispatch(transaction);
                  options.onRefusedDraft(
                    createRefusedEditDraft({
                      intent: "composition",
                      beforeDoc: session.beforeDoc,
                      afterDoc: attemptedDoc,
                      beforeSelection: session.beforeSelection,
                      afterSelection: attemptedSelection,
                    }),
                  );
                }

                options.onCompositionEnd({ ownership, refused });
                options.onEvent({
                  type: "composition_end",
                  ownership,
                  refused,
                });
              }, 0);
              return false;
            },
          },
        },
      }),
    ];
  },
});

export function resetInlineProposal(editor, { document, range, mode }) {
  const nextDocument = editor.schema.nodeFromJSON(document);
  const transaction = editor.state.tr
    .replaceWith(0, editor.state.doc.content.size, nextDocument.content)
    .setMeta(inlineProposalPluginKey, { type: "reset", range, mode })
    .setMeta(INLINE_PROPOSAL_META, { allow: true, intent: "probe_reset" })
    .setMeta("addToHistory", false);
  editor.view.dispatch(transaction);
}
