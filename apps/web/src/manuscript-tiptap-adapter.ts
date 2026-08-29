import { Extension, type Editor } from "@tiptap/core";
import Document from "@tiptap/extension-document";
import Paragraph from "@tiptap/extension-paragraph";
import Text from "@tiptap/extension-text";
import UniqueID from "@tiptap/extension-unique-id";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";

import type { InputOrigin } from "./editor-types.ts";
import { isSupportedManuscriptDoc, manuscriptJson } from "./manuscript-doc.ts";

const STORYOS_HYDRATE = "storyos.hydrate";
const STORYOS_ORIGIN = "storyos.origin";

export function isStoryosHydrateTransaction(transaction: { getMeta: (key: string) => unknown }): boolean {
  return transaction.getMeta(STORYOS_HYDRATE) === true;
}

export function hydrateManuscript(editor: Editor, blockId: string, body: string): void {
  const next = editor.schema.nodeFromJSON(manuscriptJson(blockId, body));
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
  if (origin === "paste" || origin === "cut") return origin;
  if (edit.text.length === 0 && edit.to > edit.from) return "deletion";
  if (edit.to > edit.from) return "selection_replacement";
  return "typing";
}

function insertNewline(view: EditorView): boolean {
  const { from, to } = view.state.selection;
  view.dispatch(view.state.tr.insertText("\n", from, to));
  return true;
}

function insertPlainText(view: EditorView, text: string, origin: InputOrigin): boolean {
  const { from, to } = view.state.selection;
  const transaction = view.state.tr.insertText(text, from, to);
  transaction.setMeta(STORYOS_ORIGIN, origin);
  view.dispatch(transaction);
  return true;
}

export function storyosManuscriptExtensions(blockId: string) {
  return [
    Document.extend({ content: "paragraph" }),
    Paragraph,
    Text,
    UniqueID.configure({
      attributeName: "id",
      types: ["paragraph"],
      updateDocument: false,
    }),
    Extension.create({
      name: "storyosManuscriptAdapter",
      addKeyboardShortcuts() {
        return {
          Enter: () => insertNewline(this.editor.view),
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
              return isSupportedManuscriptDoc(state.doc, transaction.doc, blockId);
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
      return insertPlainText(view, event.clipboardData?.getData("text/plain") ?? "", "paste");
    },
    handleDOMEvents: {
      cut: (view: EditorView, event: Event) => {
        if (!(event instanceof ClipboardEvent)) return false;
        const { from, to } = view.state.selection;
        event.clipboardData?.setData("text/plain", view.state.doc.textBetween(from, to, "\n"));
        event.preventDefault();
        const transaction = view.state.tr.delete(from, to);
        transaction.setMeta(STORYOS_ORIGIN, "cut");
        view.dispatch(transaction);
        return true;
      },
    },
  };
}
