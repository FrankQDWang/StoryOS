import { inlineProposalDecoration, routeInlineEdgeInsertion } from "./inline-proposal-decoration.ts";
import { Extension, type Editor } from "@tiptap/core";
import { Fragment, type Node as ProseMirrorNode } from "@tiptap/pm/model";
import Document from "@tiptap/extension-document";
import Heading from "@tiptap/extension-heading";
import Paragraph from "@tiptap/extension-paragraph";
import Text from "@tiptap/extension-text";
import UniqueID from "@tiptap/extension-unique-id";
import { Plugin, PluginKey, TextSelection, type EditorState, type Transaction } from "@tiptap/pm/state";
import { ReplaceStep } from "@tiptap/pm/transform";
import type { EditorView } from "@tiptap/pm/view";

import type { InputOrigin } from "./editor-types.ts";
import { blockProposalDecoration, capturedCandidateEdit } from "./block-proposal-decoration.ts";
import { createJournalUuid } from "./local-edit-journal.ts";
import {
  captureManuscriptChange,
  type CapturedManuscriptEdit,
  type ManuscriptParagraph,
  manuscriptBlocksJson,
  manuscriptJson,
  paragraphsEqual,
  paragraphsFromPlainTextReplacement,
  readManuscriptParagraphs,
} from "./manuscript-doc.ts";

import { firstOwnerRange, narrowToFirstOwner } from "./first-owner-range.ts";

const STORYOS_HYDRATE = "storyos.hydrate";
const STORYOS_ORIGIN = "storyos.origin";
const STORYOS_CAPTURED_EDIT = "storyos.capturedEdit";
const STORYOS_CANDIDATE_EDIT = "storyos.candidateEdit";

export function capturedCandidateEditFromTransaction(
  transaction: { getMeta: (key: string) => unknown },
) {
  return transaction.getMeta(STORYOS_CANDIDATE_EDIT) as
    | ReturnType<typeof capturedCandidateEdit>["edit"] | undefined;
}

export function capturedManuscriptEditFromTransaction(
  transaction: { getMeta: (key: string) => unknown },
): CapturedManuscriptEdit | undefined {
  const value = transaction.getMeta(STORYOS_CAPTURED_EDIT);
  if (value === null || typeof value !== "object" || !("kind" in value)) {
    return undefined;
  }
  switch (value.kind) {
    case "replace_block_selection":
    case "split_block":
    case "join_blocks":
    case "contiguous_replacement":
    case "move_block":
    case "retype_block":
      return value as CapturedManuscriptEdit;
    default:
      return undefined;
  }
}

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
  if (origin === "paste" || origin === "cut" || origin === "drop"
    || origin === "move_block" || origin === "retype_block" || origin === "composition_confirmation") {
    return origin;
  }
  if (edit.text.length === 0 && edit.to > edit.from) return "deletion";
  if (edit.to > edit.from) return "selection_replacement";
  return "typing";
}

function isBlockNode(name: string): boolean {
  return name === "paragraph" || name === "heading";
}

function manuscriptIndex(doc: ProseMirrorNode, childIndex: number): number {
  let index = 0;
  for (let child = 0; child < childIndex; child += 1) {
    if (doc.child(child).type.name !== "blockProposal") index += 1;
  }
  return index;
}

function manuscriptContentWithCandidates(
  doc: ProseMirrorNode,
  blocks: readonly ManuscriptParagraph[],
): Fragment {
  const manuscript = doc.type.schema.nodeFromJSON(manuscriptBlocksJson(blocks));
  const attached = new Map<string, ProseMirrorNode[]>();
  let anchor = "";
  doc.forEach((node) => {
    if (node.type.name !== "blockProposal") {
      anchor = node.attrs.id as string;
    } else {
      const nodes = attached.get(anchor) ?? [];
      nodes.push(node);
      attached.set(anchor, nodes);
    }
  });
  const nodes: ProseMirrorNode[] = [];
  manuscript.forEach((node) => {
    nodes.push(node, ...(attached.get(node.attrs.id as string) ?? []));
  });
  return Fragment.fromArray(nodes);
}

/** The position at `offset` in the Chapter block at `index`. Candidate blocks do not count. */
function manuscriptPosition(doc: ProseMirrorNode, index: number, offset: number): number {
  let position = doc.content.size;
  let seen = 0;
  doc.forEach((node, start) => {
    if (node.type.name === "blockProposal") return;
    if (seen === index) position = start + 1 + Math.min(offset, node.content.size);
    seen += 1;
  });
  return position;
}

function replacesCrossingSelection(state: EditorState, transaction: Transaction): boolean {
  const [step] = transaction.steps;
  const { from, to, empty } = state.selection;
  if (empty || transaction.steps.length !== 1 || !(step instanceof ReplaceStep)
    || step.from !== from || step.to !== to) return false;
  const range = firstOwnerRange(state.doc, from, to);
  return range.from !== from || range.to !== to;
}

function insertNewline(view: EditorView): boolean {
  const { from, to } = view.state.selection;
  view.dispatch(view.state.tr.insertText("\n", from, to));
  return true;
}

function splitParagraphTransaction(state: EditorState, newId: string): Transaction | undefined {
  if (!isBlockNode(state.selection.$from.parent.type.name)) return undefined;
  if (!state.selection.empty) return undefined;
  const splitPos = state.selection.from;
  const transaction = state.tr.split(splitPos);
  const newParagraphPos = splitPos + 1;
  const newParagraph = transaction.doc.nodeAt(newParagraphPos);
  const paragraph = transaction.doc.type.schema.nodes.paragraph;
  if (newParagraph === null || !isBlockNode(newParagraph.type.name)
    || paragraph === undefined) {
    return undefined;
  }
  return transaction.setNodeMarkup(newParagraphPos, paragraph, {
    id: newId,
  });
}

function moveCurrentBlock(view: EditorView, delta: -1 | 1): boolean {
  const blocks = readManuscriptParagraphs(view.state.doc);
  if (blocks === undefined) return true;
  const $from = view.state.selection.$from;
  if (!isBlockNode($from.parent.type.name)) return true;
  const fromIndex = manuscriptIndex(view.state.doc, $from.index(0));
  const toIndex = fromIndex + delta;
  if (toIndex < 0 || toIndex >= blocks.length) return true;
  const next = blocks.map((block) => ({ ...block }));
  const [block] = next.splice(fromIndex, 1);
  if (block === undefined) return true;
  next.splice(toIndex, 0, block);
  const content = manuscriptContentWithCandidates(view.state.doc, next);
  const transaction = view.state.tr.replaceWith(0, view.state.doc.content.size, content);
  transaction.setMeta(STORYOS_ORIGIN, "move_block");
  view.dispatch(transaction);
  return true;
}

function retypeCurrentBlock(view: EditorView): boolean {
  const $from = view.state.selection.$from;
  if (!isBlockNode($from.parent.type.name)) return true;
  const heading = view.state.schema.nodes.heading;
  const paragraph = view.state.schema.nodes.paragraph;
  if (heading === undefined || paragraph === undefined) return true;
  const nextType = $from.parent.type.name === "heading" ? paragraph : heading;
  const attrs = nextType.name === "heading"
    ? { ...$from.parent.attrs, level: 1 }
    : { id: $from.parent.attrs.id };
  const transaction = view.state.tr.setNodeMarkup($from.before($from.depth), nextType, attrs);
  transaction.setMeta(STORYOS_ORIGIN, "retype_block");
  view.dispatch(transaction);
  return true;
}

export function storyosManuscriptExtensions(
  blockId: string,
  onAuthorUndo?: () => boolean,
  canAcceptCandidateInput?: (hardBoundary: boolean, candidate?: { proposalId: string; operationId: string }) => boolean,
  hasCrossingComposition?: () => boolean,
) {
  return [
    Document.extend({ content: "(paragraph | heading | blockProposal)+" }),
    Paragraph,
    Heading.configure({ levels: [1] }),
    Text,
    blockProposalDecoration,
    inlineProposalDecoration,
    UniqueID.configure({
      attributeName: "id",
      types: ["paragraph", "heading"],
      generateID: () => createJournalUuid(),
      updateDocument: false,
    }),
    Extension.create({
      name: "storyosManuscriptAdapter",
      addKeyboardShortcuts() {
        return {
          Enter: () => {
            narrowToFirstOwner(this.editor.view);
            if (this.editor.state.selection.$from.parent.type.name === "blockProposal"
              || this.editor.state.selection.$from.parent.type.name === "inlineProposal") {
              return insertNewline(this.editor.view);
            }
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
          "Mod-ArrowUp": () => moveCurrentBlock(this.editor.view, -1),
          "Mod-ArrowDown": () => moveCurrentBlock(this.editor.view, 1),
          "Mod-Alt-1": () => retypeCurrentBlock(this.editor.view),
          "Mod-b": () => true,
          "Mod-i": () => true,
          "Mod-u": () => true,
          "Mod-z": () => onAuthorUndo?.() ?? true,
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
              if (hasCrossingComposition?.() === true) return true;
              // Each input narrows a crossing selection first, so a replacement that still crosses owners is refused.
              if (replacesCrossingSelection(state, transaction)) return false;
              routeInlineEdgeInsertion(transaction, state);
              const next = readManuscriptParagraphs(transaction.doc);
              if (next === undefined) return false;
              const previous = readManuscriptParagraphs(state.doc);
              if (previous === undefined) {
                return state.doc.childCount === 0
                  && next.length === 1
                  && next[0]?.manuscript_block_id === blockId;
              }
              const candidate = capturedCandidateEdit(state.doc, transaction.doc);
              if (!candidate.valid) return false;
              if (candidate.edit !== undefined) {
                if (!paragraphsEqual(previous, next)) return false;
                const origin = transaction.getMeta(STORYOS_ORIGIN);
                const hardBoundary = origin === "paste" || origin === "cut"
                  || origin === "drop";
                if (canAcceptCandidateInput?.(hardBoundary, candidate.edit.proposal) !== true) return false;
                transaction.setMeta(STORYOS_CANDIDATE_EDIT, candidate.edit);
                return true;
              }
              const edit = captureManuscriptChange(previous, next);
              if (edit !== undefined) {
                transaction.setMeta(STORYOS_CAPTURED_EDIT, edit);
                return true;
              }
              return paragraphsEqual(previous, next);
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
    handleKeyDown: (view: EditorView, event: KeyboardEvent) => {
      if (event.key === "Backspace" || event.key === "Delete") narrowToFirstOwner(view);
      return false;
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
      const dropPos = view.posAtCoords({ left: event.clientX, top: event.clientY });
      if (dropPos !== null) {
        const $pos = view.state.doc.resolve(dropPos.pos);
        if ($pos.parent.type.name === "paragraph" || $pos.parent.type.name === "heading"
          || $pos.parent.type.name === "blockProposal" || $pos.parent.type.name === "inlineProposal") {
          view.dispatch(view.state.tr.setSelection(
            TextSelection.create(view.state.doc, dropPos.pos),
          ));
        }
      }
      dispatchPlainTextReplacement(
        view,
        "drop",
        event.dataTransfer?.getData("text/plain") ?? "",
      );
      return true;
    },
    handleDOMEvents: {
      beforeinput: (view: EditorView, event: Event) => {
        if (!(event instanceof InputEvent) || event.isComposing) return false;
        if (event.inputType.startsWith("delete") && narrowToFirstOwner(view)) {
          event.preventDefault();
          view.dispatch(view.state.tr.deleteSelection());
          return true;
        }
        if (event.inputType !== "insertText" || event.data === null) return false;
        const narrowed = narrowToFirstOwner(view);
        const transaction = view.state.tr.insertText(event.data);
        if (!routeInlineEdgeInsertion(transaction, view.state) && !narrowed) return false;
        event.preventDefault();
        view.dispatch(transaction);
        return true;
      },
      dragover: (_view: EditorView, event: Event) => {
        event.preventDefault();
        return true;
      },
      cut: (view: EditorView, event: Event) => {
        if (!(event instanceof ClipboardEvent)) return false;
        narrowToFirstOwner(view);
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
  narrowToFirstOwner(view);
  const { from, to } = view.state.selection;
  const $from = view.state.doc.resolve(from);
  const $to = view.state.doc.resolve(to);
  if (($from.parent.type.name === "blockProposal" || $from.parent.type.name === "inlineProposal")
    && $to.parent.type.name === $from.parent.type.name
    && $from.before($from.depth) === $to.before($to.depth)) {
    const transaction = view.state.tr.insertText(text, from, to);
    transaction.setMeta(STORYOS_ORIGIN, origin);
    view.dispatch(transaction);
    return;
  }
  if ($from.parent.type.name !== "paragraph" && $from.parent.type.name !== "heading"
    || $to.parent.type.name !== "paragraph" && $to.parent.type.name !== "heading") {
    return;
  }
  const lines = text.split(/\r\n|\n|\r/);
  if ($from.sameParent($to) && lines.length === 1) {
    // One line in one block keeps the inline candidates of the block, and the caret stays at its end.
    const transaction = view.state.tr.insertText(text, from, to);
    transaction.setMeta(STORYOS_ORIGIN, origin);
    view.dispatch(transaction);
    return;
  }
  const startIndex = manuscriptIndex(view.state.doc, $from.index(0));
  const next = paragraphsFromPlainTextReplacement(
    previous,
    startIndex,
    $from.parentOffset,
    manuscriptIndex(view.state.doc, $to.index(0)),
    $to.parentOffset,
    text,
    createJournalUuid,
  );
  if (next === undefined) return;
  const content = manuscriptContentWithCandidates(view.state.doc, next);
  const transaction = view.state.tr.replaceWith(0, view.state.doc.content.size, content);
  // The caret stays at the end of the replacement, in the first owner (ADR 0072).
  const endOffset = lines.length === 1 ? $from.parentOffset + text.length : lines.at(-1)!.length;
  transaction.setSelection(TextSelection.create(transaction.doc,
    manuscriptPosition(transaction.doc, startIndex + lines.length - 1, endOffset)));
  transaction.setMeta(STORYOS_ORIGIN, origin);
  view.dispatch(transaction);
}

export { manuscriptJson };
