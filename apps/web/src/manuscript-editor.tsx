import { useEffect, useRef } from "react";
import { EditorContent, useEditor } from "@tiptap/react";

import { collectEligibleJournalPayload } from "./journal-payload-collection.ts";
import { createAuthorEditIdleController, type AuthorEditIdleController }
  from "./author-edit-idle.ts";
import type { EditorReadyState, PendingEditProjection } from "./editor-types.ts";
import { rebuildPendingProjection } from "./local-edit-journal.ts";
import type { ManualInputController, BoundReplacementMatch } from "./manual-input.ts";
import {
  captureManuscriptChange,
  flattenChapterBody,
  type ManuscriptParagraph,
  manuscriptBlocksJson,
  paragraphsEqual,
  readManuscriptParagraphs,
} from "./manuscript-doc.ts";
import {
  capturedManuscriptEditFromTransaction,
  capturedCandidateEditFromTransaction,
  hydrateManuscriptBlocks,
  isStoryosHydrateTransaction,
  originFromTransaction,
  storyosEditorProps,
  storyosManuscriptExtensions,
} from "./manuscript-tiptap-adapter.ts";
import {
  closeProposalAgentWriteGate,
  createProposalAgentWriteGate,
} from "./proposal-agent-write-gate.ts";
import {
  blockProposalDecoration, projectBlockProposals, type BlockProposalProjection,
} from "./block-proposal-decoration.ts";
import { undoOwnedLatestAuthorAction } from "./undo-latest-author-action.ts";

export interface ManuscriptEditorProps {
  blocks: readonly ManuscriptParagraph[];
  proposals?: readonly BlockProposalProjection[];
  editable: boolean;
  persistWorkspace: EditorReadyState | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  controllerRef: { current: ManualInputController | null };
  onProjection: (projection: PendingEditProjection, source?: "local") => void;
  onFailure: (error: unknown) => void;
  onCandidateSettled?: () => void;
}

function syncManuscriptSurface(
  dom: HTMLElement,
  blocks: readonly ManuscriptParagraph[],
): void {
  const first = blocks[0];
  if (first !== undefined) {
    dom.setAttribute("data-manuscript-block-id", first.manuscript_block_id);
  }
  dom.setAttribute(
    "data-manuscript-block-ids",
    blocks.map((block) => block.manuscript_block_id).join(" "),
  );
  dom.setAttribute(
    "data-manuscript-block-kinds",
    blocks.map((block) => block.block_kind ?? "paragraph").join(" "),
  );
}

function applyBoundReplaces(
  blocks: readonly ManuscriptParagraph[],
  matches: BoundReplacementMatch[],
  text: string,
): ManuscriptParagraph[] {
  const next = blocks.map((block) => ({ ...block }));
  const ordered = [...matches].sort((left, right) => right.start - left.start);
  for (const match of ordered) {
    const block = next.find((item) => item.manuscript_block_id === match.manuscriptBlockId);
    if (block === undefined) continue;
    block.text = `${block.text.slice(0, match.start)}${text}${block.text.slice(match.end)}`;
  }
  return next;
}

function projectLocalPending(
  workspace: EditorReadyState,
  blocks: readonly ManuscriptParagraph[],
): PendingEditProjection | undefined {
  if (workspace.pending.save_state === "needs_attention") return undefined;
  return {
    ...workspace.pending,
    body: flattenChapterBody(blocks),
    blocks: blocks.map((block) => ({
      manuscript_block_id: block.manuscript_block_id,
      block_kind: block.block_kind === "heading" ? "heading" as const : "paragraph" as const,
      text: block.text,
    })),
  };
}

export function ManuscriptEditor({
  blocks,
  proposals = [],
  editable,
  persistWorkspace,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  controllerRef,
  onProjection,
  onFailure,
  onCandidateSettled,
}: ManuscriptEditorProps) {
  const observedBlocksRef = useRef<ManuscriptParagraph[]>(blocks.map((block) => ({ ...block })));
  const composingRef = useRef(false);
  const idleRef = useRef<AuthorEditIdleController | null>(null);
  const onProjectionRef = useRef(onProjection);
  const onFailureRef = useRef(onFailure);
  const onCandidateSettledRef = useRef(onCandidateSettled);
  const persistWorkspaceRef = useRef(persistWorkspace);
  const onAuthorUndoRef = useRef<() => boolean>(() => true);
  const firstBlockId = blocks[0]?.manuscript_block_id ?? "";
  onProjectionRef.current = onProjection;
  onFailureRef.current = onFailure;
  onCandidateSettledRef.current = onCandidateSettled;
  persistWorkspaceRef.current = persistWorkspace;
  const editor = useEditor({
    extensions: [
      ...storyosManuscriptExtensions(firstBlockId, () => onAuthorUndoRef.current()),
      blockProposalDecoration,
    ],
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
      const candidate = capturedCandidateEditFromTransaction(transaction);
      if (candidate !== undefined) {
        const { proposal, priorText, from, to, text, resultingBody } = candidate;
        const origin = originFromTransaction(transaction, { from, to, text });
        void idleRef.current?.persist({
          kind: "candidate_selection",
          target: {
            proposal_id: proposal.proposalId,
            operation_id: proposal.operationId,
            revision_id: proposal.revisionId,
            manuscript_block_id: proposal.blockId,
          },
          expectedProposalHeads: proposal.expectedHeads,
          priorText, from, to, text, resultingBody,
        }, origin, new Date().toISOString());
        return;
      }
      if (current.view.composing || composingRef.current) {
        const workspace = persistWorkspaceRef.current;
        const local = workspace === undefined
          ? undefined
          : projectLocalPending(workspace, nextBlocks);
        if (local !== undefined) onProjectionRef.current(local, "local");
        return;
      }
      if (paragraphsEqual(nextBlocks, observedBlocksRef.current)) return;
      const edit = capturedManuscriptEditFromTransaction(transaction);
      observedBlocksRef.current = nextBlocks;
      if (edit === undefined) {
        idleRef.current?.fail(new Error("Manuscript replacement is not a supported Block edit"));
        return;
      }
      const createdAt = new Date().toISOString();
      const workspace = persistWorkspaceRef.current;
      const local = workspace === undefined ? undefined : projectLocalPending(workspace, nextBlocks);
      if (local !== undefined) onProjectionRef.current(local, "local");
      const origin = edit.kind === "split_block"
        || edit.kind === "join_blocks"
        || edit.kind === "move_block"
        || edit.kind === "retype_block"
        ? edit.kind
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

  onAuthorUndoRef.current = () => {
    void (async () => {
      const workspace = persistWorkspaceRef.current;
      if (editor === null || workspace === undefined) return;
      await idleRef.current?.flush();
      try {
        const settled = await undoOwnedLatestAuthorAction({
          workspace,
          baseUrl,
          fetchImpl,
          cryptoImpl,
        });
        if (settled === undefined || settled.effect.kind !== "compensated") {
          if (settled !== undefined) {
            onFailureRef.current(new Error("Author Undo did not compensate"));
          }
          return;
        }
        const restored = workspace.session.base_snapshot.materialized_revision.blocks;
        hydrateManuscriptBlocks(editor, restored);
        observedBlocksRef.current = restored.map((block) => ({ ...block }));
        syncManuscriptSurface(editor.view.dom, observedBlocksRef.current);
        onProjectionRef.current(await rebuildPendingProjection(workspace));
      } catch (error) {
        onFailureRef.current(error);
      }
    })();
    return true;
  };

  useEffect(() => {
    editor?.setEditable(editable);
  }, [editable, editor]);

  useEffect(() => {
    if (editor !== null) projectBlockProposals(editor, proposals);
  }, [editor, proposals]);

  useEffect(() => {
    if (editor === null) return;
    const identityKey = blocks.map((block) =>
      `${block.manuscript_block_id}:${block.block_kind ?? "paragraph"}`).join(" ");
    const rendered = readManuscriptParagraphs(editor.state.doc);
    const renderedKey = rendered?.map((block) =>
      `${block.manuscript_block_id}:${block.block_kind ?? "paragraph"}`).join(" ");
    if (renderedKey !== identityKey) {
      hydrateManuscriptBlocks(editor, blocks);
    }
    observedBlocksRef.current = readManuscriptParagraphs(editor.state.doc)
      ?? blocks.map((block) => ({ ...block }));
    syncManuscriptSurface(editor.view.dom, observedBlocksRef.current);
  }, [blocks.map((block) =>
    `${block.manuscript_block_id}:${block.block_kind ?? "paragraph"}`).join(" "), editor]);

  useEffect(() => {
    if (editor === null || persistWorkspace === undefined) {
      const detached: ManualInputController = {
        flush: () => Promise.resolve(),
        whenIdle: () => Promise.resolve(),
        hasIncompleteSemanticIntent: () => composingRef.current,
        close() {},
        replaceBound: async () => "refused",
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
      afterAppliedSettlement: async (workspace) => {
        await collectEligibleJournalPayload(workspace);
        onCandidateSettledRef.current?.();
      },
      onProjection: (projection) => { onProjectionRef.current(projection); },
      onFailure: (error) => { onFailureRef.current(error); },
    });
    idleRef.current = idle;
    const controller: ManualInputController = {
      flush: () => idle.flush(),
      whenIdle: () => idle.whenIdle(),
      hasIncompleteSemanticIntent: () => composingRef.current || editor.view.composing,
      close: () => idle.close(),
      async replaceBound({ kind, matches, text }) {
        if (persistWorkspaceRef.current === undefined) return "refused";
        await idle.flush();
        const workspace = persistWorkspaceRef.current;
        if (workspace === undefined) return "refused";
        const chapterId = workspace.session.base_snapshot.chapter_id;
        const current = workspace.pending.blocks.map((block) => ({
          manuscript_block_id: block.manuscript_block_id,
          block_kind: block.block_kind === "heading" ? "heading" as const : "paragraph" as const,
          text: block.text,
        }));
        const match = matches.find((item) => item.chapterId === chapterId);
        const createdAt = new Date().toISOString();
        const beforeRevision = workspace.pending.authoritative_revision_id;
        if (kind === "one" && match !== undefined && matches.length === 1) {
          const currentBlock = current.find((block) =>
            block.manuscript_block_id === match.manuscriptBlockId);
          if (currentBlock === undefined
            || !Number.isSafeInteger(match.start)
            || !Number.isSafeInteger(match.end)
            || match.start < 0
            || match.end < match.start
            || match.end > currentBlock.text.length
            || currentBlock.text.slice(match.start, match.end) !== match.queryText) {
            return "stale";
          }
          const resultingBlocks = applyBoundReplaces(current, [match], text);
          await idle.persist({
            kind: "replace_block_selection",
            manuscript_block_id: match.manuscriptBlockId,
            from: match.start,
            to: match.end,
            text,
            resultingBlocks,
            resultingBody: flattenChapterBody(resultingBlocks),
          }, "selection_replacement", createdAt);
          await idle.flush();
          const pending = persistWorkspaceRef.current?.pending;
          if (pending?.save_state !== "saved") {
            hydrateManuscriptBlocks(editor, observedBlocksRef.current);
            syncManuscriptSurface(editor.view.dom, observedBlocksRef.current);
            return "refused";
          }
          hydrateManuscriptBlocks(editor, resultingBlocks);
          observedBlocksRef.current = resultingBlocks.map((block) => ({ ...block }));
          syncManuscriptSurface(editor.view.dom, observedBlocksRef.current);
          return pending.authoritative_revision_id === beforeRevision ? "unchanged" : "applied";
        }
        const currentMatches = matches.filter((item) => item.chapterId === chapterId);
        const first = current[0];
        if (first === undefined) return "refused";
        const broader = currentMatches.length >= 2
          ? [...currentMatches]
            .sort((left, right) => right.start - left.start)
            .slice(0, 2)
          : [
            {
              chapterId,
              manuscriptBlockId: first.manuscript_block_id,
              start: 0,
              end: 0,
              queryText: "",
            },
            {
              chapterId,
              manuscriptBlockId: first.manuscript_block_id,
              start: 0,
              end: 0,
              queryText: "",
            },
          ];
        const resultingBlocks = currentMatches.length >= 2
          ? applyBoundReplaces(current, broader, text)
          : current.map((block) => ({ ...block }));
        await idle.persist({
          kind: "contiguous_replacement",
          primitives: broader.map((item) => ({
            kind: "replace_block_selection" as const,
            manuscript_block_id: item.manuscriptBlockId,
            from: item.start,
            to: item.end,
            text: currentMatches.length >= 2 ? text : "",
          })),
          from: broader.at(-1)?.start ?? 0,
          to: broader[0]?.end ?? 0,
          resultingBlocks,
          resultingBody: flattenChapterBody(resultingBlocks),
        }, "selection_replacement", createdAt);
        await idle.flush();
        hydrateManuscriptBlocks(editor, observedBlocksRef.current);
        syncManuscriptSurface(editor.view.dom, observedBlocksRef.current);
        return "refused";
      },
    };
    controllerRef.current = controller;
    const { dom } = editor.view;
    const agentWriteGate = createProposalAgentWriteGate();
    const onFirstAuthorInput = (): void => {
      closeProposalAgentWriteGate(agentWriteGate);
    };
    const onCompositionStart = (): void => {
      onFirstAuthorInput();
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
      const workspace = persistWorkspaceRef.current;
      const local = workspace === undefined ? undefined : projectLocalPending(workspace, nextBlocks);
      if (local !== undefined) onProjectionRef.current(local, "local");
      void idle.persist(edit, "composition_confirmation", new Date().toISOString());
    };
    dom.addEventListener("beforeinput", onFirstAuthorInput);
    dom.addEventListener("compositionstart", onCompositionStart);
    dom.addEventListener("compositionend", onCompositionEnd);
    return () => {
      dom.removeEventListener("beforeinput", onFirstAuthorInput);
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
