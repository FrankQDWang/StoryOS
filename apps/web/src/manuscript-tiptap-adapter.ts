import { Extension, type Editor } from "@tiptap/core";
import Document from "@tiptap/extension-document";
import Paragraph from "@tiptap/extension-paragraph";
import Text from "@tiptap/extension-text";
import UniqueID from "@tiptap/extension-unique-id";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";

import type { InputOrigin } from "./editor-types.ts";
import {
  contiguousUtf16Replace,
  isSupportedManuscriptDoc,
  manuscriptJson,
  paragraphUtf16,
} from "./manuscript-doc.ts";

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
              if (!isSupportedManuscriptDoc(state.doc, transaction.doc, blockId)) {
                return false;
              }
              const nextText = paragraphUtf16(transaction.doc);
              if (nextText === undefined) return false;
              const previousText = paragraphUtf16(state.doc);
              if (previousText === undefined) return state.doc.childCount === 0;
              return previousText === nextText
                || contiguousUtf16Replace(previousText, nextText) !== undefined;
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
      const { from, to } = view.state.selection;
      const transaction = view.state.tr.insertText(
        event.clipboardData?.getData("text/plain") ?? "",
        from,
        to,
      );
      transaction.setMeta(STORYOS_ORIGIN, "paste");
      view.dispatch(transaction);
      return true;
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
