import { useEffect, useState } from "react";
import type { Editor } from "@tiptap/core";
import type { ProjectScope } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readManuscriptParagraphs } from "./manuscript-doc.ts";
import { referenceLabel, selectionReferences, type ChapterOrder } from "./selection-reference.ts";
import {
  addDraftReferences, copiedReferences, draftCanAdd, REFERENCE_CLIPBOARD_TYPE, useReferenceDraft,
  type DraftReference,
} from "./selection-reference-draft.ts";

/** The Chapter of the editor document and its Revision, for the references of the next message (ADR 0062). */
export type EditorReferenceSource = {
  scope: ProjectScope;
  chapterId: string;
  revisionId: string;
  /** The editor document is the Revision, with no unsaved author edits. */
  saved: boolean;
  tree?: ChapterOrder | undefined;
};

/** The references of the editor selection with their labels. A selection with a part that has no label has none. */
function selectedEntries(editor: Editor, source: EditorReferenceSource): Omit<DraftReference, "key">[] {
  const { from, to } = editor.state.selection;
  const blocks = readManuscriptParagraphs(editor.state.doc);
  if (blocks === undefined || source.tree === undefined) return [];
  const entries: Omit<DraftReference, "key">[] = [];
  for (const selected of selectionReferences(editor.state.doc, { from, to }, source.chapterId, source.revisionId)) {
    const label = referenceLabel(source.tree, blocks, selected);
    if (label === undefined) return [];
    entries.push({ ...selected, label, moved: selected.reference.kind === "chapter_text" && !source.saved });
  }
  return entries;
}

/**
 * The control at the end of an editor selection that adds its text to the next assistant message. A copy
 * of the selection also gives its references to a paste in the composer.
 */
export function AddReferenceControl({ editor, source, tracked }: {
  editor: Editor;
  source: EditorReferenceSource;
  tracked: Set<string>;
}) {
  const draft = useReferenceDraft(source.scope);
  const [anchor, setAnchor] = useState<{ top: number; left: number }>();
  useEffect(() => {
    const update = () => {
      const { to, empty } = editor.state.selection;
      if (empty || !editor.isFocused || selectedEntries(editor, source).length === 0) {
        setAnchor(undefined);
        return;
      }
      const end = editor.view.coordsAtPos(to);
      setAnchor({ top: end.bottom + 6, left: end.left });
    };
    const hide = () => setAnchor(undefined);
    const copy = (event: ClipboardEvent) => {
      const entries = selectedEntries(editor, source);
      if (entries.length > 0) {
        event.clipboardData?.setData(REFERENCE_CLIPBOARD_TYPE, copiedReferences(source.scope, tracked, entries,
          () => editor.state.doc));
      }
    };
    editor.on("selectionUpdate", update);
    editor.on("focus", update);
    // A new tree or Revision can give a label to the selection that is already there.
    update();
    editor.on("blur", hide);
    editor.view.dom.addEventListener("copy", copy);
    globalThis.addEventListener("scroll", update, true);
    return () => {
      editor.off("selectionUpdate", update);
      editor.off("focus", update);
      editor.off("blur", hide);
      editor.view.dom.removeEventListener("copy", copy);
      globalThis.removeEventListener("scroll", update, true);
    };
  }, [editor, source.chapterId, source.revisionId, source.saved, source.tree]);
  const entries = anchor === undefined ? [] : selectedEntries(editor, source);
  if (entries.length === 0 || !draftCanAdd(draft, entries.map((entry) => entry.reference))) return null;
  return <button type="button" className="add-selection-reference" data-add-selection-reference=""
    style={{ top: anchor!.top, left: anchor!.left }}
    // The editor keeps its selection while the author clicks the control.
    onMouseDown={(event) => event.preventDefault()}
    onClick={() => {
      addDraftReferences(source.scope, selectedEntries(editor, source), tracked);
      setAnchor(undefined);
    }}>添加到对话</button>;
}
