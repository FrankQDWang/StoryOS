import { useEffect, useRef } from "react";
import { EditorContent, useEditor } from "@tiptap/react";

import { collectEligibleJournalPayload } from "./journal-payload-collection.ts";
import { createAuthorEditIdleController, type AuthorEditIdleController }
  from "./author-edit-idle.ts";
import type { EditorReadyState, PendingEditProjection } from "./editor-types.ts";
import type { ManualInputController } from "./manual-input.ts";
import {
  contiguousUtf16Replace,
  manuscriptJson,
  paragraphUtf16,
} from "./manuscript-doc.ts";
import {
  hydrateManuscript,
  isStoryosHydrateTransaction,
  originFromTransaction,
  storyosEditorProps,
  storyosManuscriptExtensions,
} from "./manuscript-tiptap-adapter.ts";

export interface ManuscriptEditorProps {
  body: string;
  blockId: string;
  editable: boolean;
  persistWorkspace: EditorReadyState | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  controllerRef: { current: ManualInputController | null };
  onProjection: (projection: PendingEditProjection) => void;
  onFailure: () => void;
}

function syncManuscriptSurface(dom: HTMLElement, body: string, blockId: string): void {
  dom.setAttribute("data-manuscript-body", body);
  dom.setAttribute("data-manuscript-block-id", blockId);
}

export function ManuscriptEditor({
  body,
  blockId,
  editable,
  persistWorkspace,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  controllerRef,
  onProjection,
  onFailure,
}: ManuscriptEditorProps) {
  const observedBodyRef = useRef(body);
  const composingRef = useRef(false);
  const idleRef = useRef<AuthorEditIdleController | null>(null);
  const onProjectionRef = useRef(onProjection);
  const onFailureRef = useRef(onFailure);
  onProjectionRef.current = onProjection;
  onFailureRef.current = onFailure;
  const editor = useEditor({
    extensions: storyosManuscriptExtensions(blockId),
    content: manuscriptJson(blockId, body),
    editable,
    injectCSS: false,
    enableInputRules: false,
    enablePasteRules: false,
    immediatelyRender: true,
    shouldRerenderOnTransaction: false,
    editorProps: storyosEditorProps(blockId),
    onCreate({ editor: created }) {
      const rendered = paragraphUtf16(created.state.doc);
      if (rendered !== body) hydrateManuscript(created, blockId, body);
      observedBodyRef.current = paragraphUtf16(created.state.doc) ?? body;
      syncManuscriptSurface(created.view.dom, observedBodyRef.current, blockId);
    },
    onTransaction({ editor: current, transaction }) {
      const nextBody = paragraphUtf16(current.state.doc);
      if (nextBody === undefined) return;
      syncManuscriptSurface(current.view.dom, nextBody, blockId);
      if (isStoryosHydrateTransaction(transaction) || !transaction.docChanged) {
        observedBodyRef.current = nextBody;
        return;
      }
      if (current.view.composing || composingRef.current) return;
      if (nextBody === observedBodyRef.current) return;
      const edit = contiguousUtf16Replace(observedBodyRef.current, nextBody);
      observedBodyRef.current = nextBody;
      if (edit === undefined) {
        idleRef.current?.fail(new Error("Manuscript replacement is not a supported Block edit"));
        return;
      }
      const createdAt = new Date().toISOString();
      void idleRef.current?.persist(edit, originFromTransaction(transaction, edit), createdAt);
    },
  }, []);

  useEffect(() => {
    editor?.setEditable(editable);
  }, [editable, editor]);

  useEffect(() => {
    if (editor === null) return;
    const rendered = paragraphUtf16(editor.state.doc);
    if (rendered !== body) hydrateManuscript(editor, blockId, body);
    observedBodyRef.current = paragraphUtf16(editor.state.doc) ?? body;
    syncManuscriptSurface(editor.view.dom, observedBodyRef.current, blockId);
  }, [blockId, editor]);

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
      onFailure: () => { onFailureRef.current(); },
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
      const nextBody = paragraphUtf16(editor.state.doc);
      if (nextBody === undefined || nextBody === observedBodyRef.current) return;
      const edit = contiguousUtf16Replace(observedBodyRef.current, nextBody);
      observedBodyRef.current = nextBody;
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
