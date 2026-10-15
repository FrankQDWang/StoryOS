import { useEffect, useState } from "react";
import type { Editor } from "@tiptap/core";
import type { ProjectScope } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readManuscriptParagraphs } from "./manuscript-doc.ts";
import { chapterTextReference, referenceLabel, type ChapterOrder } from "./selection-reference.ts";
import { addDraftReference, draftCanAdd, useReferenceDraft } from "./selection-reference-draft.ts";

/** The Chapter of the editor document and its Revision, for the references of the next message (ADR 0062). */
export type EditorReferenceSource = {
  scope: ProjectScope;
  chapterId: string;
  revisionId: string;
  /** The editor document is the Revision, with no unsaved author edits. */
  saved: boolean;
  tree?: ChapterOrder | undefined;
};

/** The control at the end of an editor selection that adds its text to the next assistant message. */
export function AddReferenceControl({ editor, source, tracked }: {
  editor: Editor;
  source: EditorReferenceSource;
  tracked: Set<string>;
}) {
  const draft = useReferenceDraft(source.scope);
  const [anchor, setAnchor] = useState<{ top: number; left: number }>();
  useEffect(() => {
    const update = () => {
      const { from, to, empty } = editor.state.selection;
      if (empty || !editor.isFocused
        || chapterTextReference(editor.state.doc, { from, to }, source.chapterId, source.revisionId) === undefined) {
        setAnchor(undefined);
        return;
      }
      const end = editor.view.coordsAtPos(to);
      setAnchor({ top: end.bottom + 6, left: end.left });
    };
    const hide = () => setAnchor(undefined);
    editor.on("selectionUpdate", update);
    editor.on("focus", update);
    editor.on("blur", hide);
    globalThis.addEventListener("scroll", update, true);
    return () => {
      editor.off("selectionUpdate", update);
      editor.off("focus", update);
      editor.off("blur", hide);
      globalThis.removeEventListener("scroll", update, true);
    };
  }, [editor, source.chapterId, source.revisionId]);
  if (anchor === undefined || source.tree === undefined || !draftCanAdd(draft)) return null;
  return <button type="button" className="add-selection-reference" data-add-selection-reference=""
    style={{ top: anchor.top, left: anchor.left }}
    // The editor keeps its selection while the author clicks the control.
    onMouseDown={(event) => event.preventDefault()}
    onClick={() => {
      const { from, to } = editor.state.selection;
      const reference = chapterTextReference(editor.state.doc, { from, to }, source.chapterId, source.revisionId);
      const blocks = readManuscriptParagraphs(editor.state.doc);
      const label = reference === undefined || blocks === undefined || source.tree === undefined ? undefined
        : referenceLabel(source.tree, blocks, reference);
      if (reference === undefined || label === undefined) return;
      const key = addDraftReference(source.scope, { reference, label, moved: !source.saved });
      if (key !== undefined) tracked.add(key);
      setAnchor(undefined);
    }}>添加到对话</button>;
}
