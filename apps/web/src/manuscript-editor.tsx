import { useEffect, useRef } from "react";
import { EditorContent, useEditor } from "@tiptap/react";

import { collectEligibleJournalPayload } from "./journal-payload-collection.ts";
import { createAuthorEditIdleController, type AuthorEditIdleController }
  from "./author-edit-idle.ts";
import type { EditorReadyState, PendingEditProjection } from "./editor-types.ts";
import type { ManualInputController } from "./manual-input.ts";
import {
  captureManuscriptChange,
  flattenChapterBody,
  type ManuscriptParagraph,
  manuscriptBlocksJson,
  paragraphsEqual,
  readManuscriptParagraphs,
} from "./manuscript-doc.ts";
import {
  hydrateManuscriptBlocks,
  isStoryosHydrateTransaction,
  originFromTransaction,
  storyosEditorProps,
  storyosManuscriptExtensions,
} from "./manuscript-tiptap-adapter.ts";

export interface ManuscriptEditorProps {
  blocks: readonly ManuscriptParagraph[];
  editable: boolean;
  persistWorkspace: EditorReadyState | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  controllerRef: { current: ManualInputController | null };
  onProjection: (projection: PendingEditProjection) => void;
  onFailure: (error: unknown) => void;
}

function syncManuscriptSurface(
  dom: HTMLElement,
  blocks: readonly ManuscriptParagraph[],
): void {
  const first = blocks[0];
  dom.setAttribute("data-manuscript-body", flattenChapterBody(blocks));
  if (first !== undefined) {
    dom.setAttribute("data-manuscript-block-id", first.manuscript_block_id);
  }
  dom.setAttribute(
    "data-manuscript-block-ids",
    blocks.map((block) => block.manuscript_block_id).join(" "),
  );
}

export function ManuscriptEditor({
  blocks,
  editable,
  persistWorkspace,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  controllerRef,
  onProjection,
  onFailure,
}: ManuscriptEditorProps) {
  const observedBlocksRef = useRef<ManuscriptParagraph[]>(blocks.map((block) => ({ ...block })));
  const composingRef = useRef(false);
  const idleRef = useRef<AuthorEditIdleController | null>(null);
  const onProjectionRef = useRef(onProjection);
  const onFailureRef = useRef(onFailure);
  const firstBlockId = blocks[0]?.manuscript_block_id ?? "";
  onProjectionRef.current = onProjection;
  onFailureRef.current = onFailure;
  const editor = useEditor({
    extensions: storyosManuscriptExtensions(firstBlockId),
    content: manuscriptBlocksJson(blocks),
    editable,
    injectCSS: false,
    enableInputRules: false,
    enablePasteRules: false,
    immediatelyRender: true,
    shouldRerenderOnTransaction: false,
    editorProps: storyosEditorProps(firstBlockId),
    onCreate({ editor: created }) {
      const rendered = readManuscriptParagraphs(created.state.doc);
      if (rendered === undefined || !paragraphsEqual(rendered, blocks)) {
        hydrateManuscriptBlocks(created, blocks);
      }
      observedBlocksRef.current = readManuscriptParagraphs(created.state.doc)
        ?? blocks.map((block) => ({ ...block }));
      syncManuscriptSurface(created.view.dom, observedBlocksRef.current);
    },
    onTransaction({ editor: current, transaction }) {
      const nextBlocks = readManuscriptParagraphs(current.state.doc);
      if (nextBlocks === undefined) return;
      syncManuscriptSurface(current.view.dom, nextBlocks);
      if (isStoryosHydrateTransaction(transaction) || !transaction.docChanged) {
        observedBlocksRef.current = nextBlocks;
        return;
      }
      if (current.view.composing || composingRef.current) return;
      if (paragraphsEqual(nextBlocks, observedBlocksRef.current)) return;
      const edit = captureManuscriptChange(observedBlocksRef.current, nextBlocks);
      observedBlocksRef.current = nextBlocks;
      if (edit === undefined) {
        idleRef.current?.fail(new Error("Manuscript replacement is not a supported Block edit"));
        return;
      }
      const createdAt = new Date().toISOString();
      const origin = edit.kind === "split_block"
        ? "split_block"
        : edit.kind === "join_blocks"
          ? "join_blocks"
          : originFromTransaction(transaction, edit.kind === "contiguous_replacement"
            ? {
              from: edit.from,
              to: edit.to,
              text: edit.primitives.find((primitive) =>
                primitive.kind === "replace_block_selection")?.text ?? "",
            }
            : edit);
      void idleRef.current?.persist(edit, origin, createdAt);
    },
  }, []);

  useEffect(() => {
    editor?.setEditable(editable);
  }, [editable, editor]);

  useEffect(() => {
    if (editor === null) return;
    const identityKey = blocks.map((block) => block.manuscript_block_id).join(" ");
    const rendered = readManuscriptParagraphs(editor.state.doc);
    const renderedKey = rendered?.map((block) => block.manuscript_block_id).join(" ");
    if (renderedKey !== identityKey) {
      hydrateManuscriptBlocks(editor, blocks);
    }
    observedBlocksRef.current = readManuscriptParagraphs(editor.state.doc)
      ?? blocks.map((block) => ({ ...block }));
    syncManuscriptSurface(editor.view.dom, observedBlocksRef.current);
  }, [blocks.map((block) => block.manuscript_block_id).join(" "), editor]);

  useEffect(() => {
    if (editor === null || persistWorkspace === undefined) {
      const detached: ManualInputController = {
        flush: () => Promise.resolve(),
        whenIdle: () => Promise.resolve(),
        hasIncompleteSemanticIntent: () => composingRef.current,
        close() {},
      };
      controllerRef.current = detached;
      return () => {
        if (controllerRef.current === detached) controllerRef.current = null;
      };
    }
    const idle = createAuthorEditIdleController({
      workspace: persistWorkspace,
      baseUrl,
      fetchImpl,
      cryptoImpl,
      afterAppliedSettlement: collectEligibleJournalPayload,
      onProjection: (projection) => { onProjectionRef.current(projection); },
      onFailure: (error) => { onFailureRef.current(error); },
    });
    idleRef.current = idle;
    const controller: ManualInputController = {
      flush: () => idle.flush(),
      whenIdle: () => idle.whenIdle(),
      hasIncompleteSemanticIntent: () => composingRef.current || editor.view.composing,
      close: () => idle.close(),
    };
    controllerRef.current = controller;
    const { dom } = editor.view;
    const onCompositionStart = (): void => {
      composingRef.current = true;
      idle.setHoldSubmission(true);
    };
    const onCompositionEnd = (): void => {
      composingRef.current = false;
      idle.setHoldSubmission(false);
      const nextBlocks = readManuscriptParagraphs(editor.state.doc);
      if (nextBlocks === undefined || paragraphsEqual(nextBlocks, observedBlocksRef.current)) {
        return;
      }
      const edit = captureManuscriptChange(observedBlocksRef.current, nextBlocks);
      observedBlocksRef.current = nextBlocks;
      if (edit === undefined) {
        idle.fail(new Error("Manuscript replacement is not a supported Block edit"));
        return;
      }
      void idle.persist(edit, "composition_confirmation", new Date().toISOString());
    };
    dom.addEventListener("compositionstart", onCompositionStart);
    dom.addEventListener("compositionend", onCompositionEnd);
    return () => {
      dom.removeEventListener("compositionstart", onCompositionStart);
      dom.removeEventListener("compositionend", onCompositionEnd);
      idle.close();
      idleRef.current = null;
      if (controllerRef.current === controller) controllerRef.current = null;
    };
  }, [baseUrl, controllerRef, cryptoImpl, editor, fetchImpl, persistWorkspace]);

  if (editor === null) return null;
  return <EditorContent editor={editor} />;
}
