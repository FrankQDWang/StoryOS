import { Extension, type Editor } from "@tiptap/core";
import Document from "@tiptap/extension-document";
import Paragraph from "@tiptap/extension-paragraph";
import Text from "@tiptap/extension-text";
import UniqueID from "@tiptap/extension-unique-id";
import { Plugin, PluginKey, type EditorState, type Transaction } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";

import type { InputOrigin } from "./editor-types.ts";
import { createJournalUuid } from "./local-edit-journal.ts";
import {
  captureManuscriptChange,
  type ManuscriptParagraph,
  manuscriptBlocksJson,
  manuscriptJson,
  paragraphsEqual,
  paragraphsFromPlainTextReplacement,
  readManuscriptParagraphs,
} from "./manuscript-doc.ts";

const STORYOS_HYDRATE = "storyos.hydrate";
const STORYOS_ORIGIN = "storyos.origin";

export function isStoryosHydrateTransaction(transaction: { getMeta: (key: string) => unknown }): boolean {
  return transaction.getMeta(STORYOS_HYDRATE) === true;
}

export function hydrateManuscript(editor: Editor, blockId: string, body: string): void {
  hydrateManuscriptBlocks(editor, [{ manuscript_block_id: blockId, text: body }]);
}

export function hydrateManuscriptBlocks(editor: Editor, blocks: readonly ManuscriptParagraph[]): void {
  const next = editor.schema.nodeFromJSON(manuscriptBlocksJson(blocks));
  const transaction = editor.state.tr.replaceWith(0, editor.state.doc.content.size, next);
  transaction.setMeta(STORYOS_HYDRATE, true);
  transaction.setMeta("addToHistory", false);
  editor.view.dispatch(transaction);
}

export function originFromTransaction(
  transaction: { getMeta: (key: string) => unknown },
  edit: { from: number; to: number; text: string },
): InputOrigin {
  const origin: unknown = transaction.getMeta(STORYOS_ORIGIN);
  if (origin === "paste" || origin === "cut" || origin === "drop") return origin;
  if (edit.text.length === 0 && edit.to > edit.from) return "deletion";
  if (edit.to > edit.from) return "selection_replacement";
  return "typing";
}

function insertNewline(view: EditorView): boolean {
  const { from, to } = view.state.selection;
  view.dispatch(view.state.tr.insertText("\n", from, to));
  return true;
}

function splitParagraphTransaction(state: EditorState, newId: string): Transaction | undefined {
  if (state.selection.$from.parent.type.name !== "paragraph") return undefined;
  if (!state.selection.empty) return undefined;
  const splitPos = state.selection.from;
  const transaction = state.tr.split(splitPos);
  const newParagraphPos = splitPos + 1;
  const newParagraph = transaction.doc.nodeAt(newParagraphPos);
  if (newParagraph?.type.name !== "paragraph") {
    return undefined;
  }
  return transaction.setNodeMarkup(newParagraphPos, undefined, {
    ...newParagraph.attrs,
    id: newId,
  });
}

export function storyosManuscriptExtensions(blockId: string) {
  return [
    Document.extend({ content: "paragraph+" }),
    Paragraph,
    Text,
    UniqueID.configure({
      attributeName: "id",
      types: ["paragraph"],
      generateID: () => createJournalUuid(),
      updateDocument: false,
    }),
    Extension.create({
      name: "storyosManuscriptAdapter",
      addKeyboardShortcuts() {
        return {
          Enter: () => {
            if (!this.editor.state.selection.empty) {
              this.editor.commands.command(({ tr, dispatch }) => {
                dispatch?.(tr.deleteSelection());
                return true;
              });
            }
            const newId = createJournalUuid();
            return this.editor.commands.command(({ state, dispatch }) => {
              const transaction = splitParagraphTransaction(state, newId);
              if (transaction === undefined) return true;
              dispatch?.(transaction);
              return true;
            });
          },
          "Shift-Enter": () => insertNewline(this.editor.view),
          "Mod-b": () => true,
          "Mod-i": () => true,
          "Mod-u": () => true,
          "Mod-z": () => true,
          "Mod-y": () => true,
          "Shift-Mod-z": () => true,
        };
      },
      addProseMirrorPlugins() {
        return [
          new Plugin({
            key: new PluginKey("storyosManuscriptAdapter"),
            filterTransaction: (transaction, state) => {
              if (transaction.getMeta(STORYOS_HYDRATE) === true || !transaction.docChanged) {
                return true;
              }
              const next = readManuscriptParagraphs(transaction.doc);
              if (next === undefined) return false;
              const previous = readManuscriptParagraphs(state.doc);
              if (previous === undefined) {
                return state.doc.childCount === 0
                  && next.length === 1
                  && next[0]?.manuscript_block_id === blockId;
              }
              return captureManuscriptChange(previous, next) !== undefined
                || paragraphsEqual(previous, next);
            },
          }),
        ];
      },
    }),
  ];
}

export function storyosEditorProps(blockId: string) {
  return {
    attributes: {
      class: "manuscript-editor",
      "data-manuscript-editor": "",
      "data-manuscript-block-id": blockId,
    },
    handlePaste: (view: EditorView, event: ClipboardEvent) => {
      event.preventDefault();
      dispatchPlainTextReplacement(
        view,
        "paste",
        event.clipboardData?.getData("text/plain") ?? "",
      );
      return true;
    },
    handleDrop: (view: EditorView, event: DragEvent) => {
      event.preventDefault();
      dispatchPlainTextReplacement(
        view,
        "drop",
        event.dataTransfer?.getData("text/plain") ?? "",
      );
      return true;
    },
    handleDOMEvents: {
      dragover: (_view: EditorView, event: Event) => {
        event.preventDefault();
        return true;
      },
      cut: (view: EditorView, event: Event) => {
        if (!(event instanceof ClipboardEvent)) return false;
        const { from, to } = view.state.selection;
        event.clipboardData?.setData("text/plain", view.state.doc.textBetween(from, to, "\n"));
        event.preventDefault();
        dispatchPlainTextReplacement(view, "cut", "");
        return true;
      },
    },
  };
}

function dispatchPlainTextReplacement(
  view: EditorView,
  origin: "paste" | "cut" | "drop",
  text: string,
): void {
  const previous = readManuscriptParagraphs(view.state.doc);
  if (previous === undefined) return;
  const { from, to } = view.state.selection;
  const $from = view.state.doc.resolve(from);
  const $to = view.state.doc.resolve(to);
  if ($from.parent.type.name !== "paragraph" || $to.parent.type.name !== "paragraph") {
    return;
  }
  const next = paragraphsFromPlainTextReplacement(
    previous,
    $from.index(0),
    $from.parentOffset,
    $to.index(0),
    $to.parentOffset,
    text,
    createJournalUuid,
  );
  if (next === undefined) return;
  const node = view.state.schema.nodeFromJSON(manuscriptBlocksJson(next));
  const transaction = view.state.tr.replaceWith(0, view.state.doc.content.size, node.content);
  transaction.setMeta(STORYOS_ORIGIN, origin);
  view.dispatch(transaction);
}

export { manuscriptJson };
