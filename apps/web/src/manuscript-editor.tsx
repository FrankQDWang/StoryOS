import { useEffect, useRef } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";

import { captureStructuredSelection, type StructuredSelectionEdit } from "./structured-edit-capture.ts";

import type { EditorSessionWritingController } from "./editor-session-writing.ts";
import type { EditorReadyState } from "./editor-types.ts";
import type { ManualInputController } from "./manual-input.ts";
import {
  captureManuscriptChange,
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
  capturedCandidateEdit, projectBlockProposals,
  type BlockProposalProjection,
} from "./block-proposal-decoration.ts";
import { undoOwnedLatestAuthorAction } from "./undo-latest-author-action.ts";
import { createChallengeAdmissionWait, type ChallengeAdmissionTimers } from "./challenge-admission-wait.ts";

import type { ProposalFocus } from "./proposal-navigation.ts";

export interface ManuscriptEditorProps {
  focusProposal?: ProposalFocus | undefined;
  onCandidateFocus?: ((focus: ProposalFocus | undefined) => void) | undefined;
  blocks: readonly ManuscriptParagraph[];
  proposals?: readonly BlockProposalProjection[];
  editable: boolean;
  persistWorkspace: EditorReadyState | undefined;
  /** The writing controller of `persistWorkspace`. */
  writing?: EditorSessionWritingController | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  controllerRef: { current: ManualInputController | null };
  onFailure: (error: unknown) => void;
  onCandidateSettled?: (proposalId?: string) => void;
  onAcceptProposal?: (target: {
    proposalId: string;
    operationId: string;
    revisionId: string;
    text: string;
    operationIds?: readonly string[] | undefined;
  }) => void;
  onRejectProposal?: (target: {
    proposalId: string;
    operationId: string;
    revisionId: string;
    text: string;
    operationIds?: readonly string[] | undefined;
  }) => void;
  onReplanProposal?: (target: {
    proposalId: string;
    operationId: string;
    revisionId: string;
    text: string;
  }) => void;
  onWithdrawProposal?: (target: {
    proposalId: string;
    operationId: string;
    revisionId: string;
    text: string;
  }) => void;
  onCopyProposal?: (proposalId: string) => void;
  undoChallengeTimers?: ChallengeAdmissionTimers;
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

export function ManuscriptEditor({
  blocks,
  proposals = [],
  editable,
  persistWorkspace,
  writing,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  controllerRef,
  onFailure,
  onCandidateSettled, focusProposal, onCandidateFocus,
  onAcceptProposal,
  onRejectProposal,
  onReplanProposal,
  onWithdrawProposal,
  onCopyProposal,
  undoChallengeTimers,
}: ManuscriptEditorProps) {
  const observedBlocksRef = useRef<ManuscriptParagraph[]>(blocks.map((block) => ({ ...block })));
  const capturedInputRef = useRef(0);
  const composingRef = useRef(false);
  const focusedProposalRef = useRef<string | undefined>(undefined);
  const mixedCompositionRef = useRef<StructuredSelectionEdit | undefined>(undefined);
  const mixedCompositionStartRef = useRef<ProseMirrorNode | null>(null);
  const candidateCompositionStartRef = useRef<ProseMirrorNode | null>(null);
  const candidateCompositionBlockedRef = useRef(false);
  const candidateCompositionDirtyRef = useRef(false);
  const writingRef = useRef(writing);
  writingRef.current = writing;
  const onFailureRef = useRef(onFailure);
  const onCandidateSettledRef = useRef(onCandidateSettled);
  const onCandidateFocusRef = useRef(onCandidateFocus);
  onCandidateFocusRef.current = onCandidateFocus;
  const proposalsRef = useRef(proposals);
  proposalsRef.current = proposals;
  const onAcceptProposalRef = useRef(onAcceptProposal);
  const onRejectProposalRef = useRef(onRejectProposal);
  const onReplanProposalRef = useRef(onReplanProposal);
  const onWithdrawProposalRef = useRef(onWithdrawProposal);
  const onCopyProposalRef = useRef(onCopyProposal);
  const persistWorkspaceRef = useRef(persistWorkspace);
  const onAuthorUndoRef = useRef<() => boolean>(() => true);
  const abandonUndoRef = useRef<(() => void) | undefined>(undefined);
  const firstBlockId = blocks[0]?.manuscript_block_id ?? "";
  onFailureRef.current = onFailure;
  onCandidateSettledRef.current = onCandidateSettled;
  onAcceptProposalRef.current = onAcceptProposal;
  onRejectProposalRef.current = onRejectProposal;
  onReplanProposalRef.current = onReplanProposal;
  onWithdrawProposalRef.current = onWithdrawProposal;
  onCopyProposalRef.current = onCopyProposal;
  persistWorkspaceRef.current = persistWorkspace;
  const editor = useEditor({
    extensions: [
      ...storyosManuscriptExtensions(firstBlockId,
        () => onAuthorUndoRef.current(),
        (hardBoundary, candidate) => !candidateCompositionBlockedRef.current
          && writingRef.current?.canAcceptInput(hardBoundary, candidate) === true,
        () => composingRef.current && mixedCompositionRef.current !== undefined),
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
        if (!current.view.composing && !composingRef.current) observedBlocksRef.current = nextBlocks;
        return;
      }
      // New input changes the Author Undo Frontier, so an Undo in progress can only conflict.
      abandonUndoRef.current?.();
      const mixed = transaction.getMeta("storyos.structuredEdit") as StructuredSelectionEdit | undefined;
      if (mixed !== undefined) {
        if (current.view.composing || composingRef.current) {
          mixedCompositionRef.current ??= mixed;
          return;
        }
        const primitive = mixed.authorEditUnit.normalized_primitives[0];
        const text = primitive?.kind === "replace_structured_selection"
          ? primitive.replacement.map((block) => block.text).join("\n") : "";
        capturedInputRef.current += 1;
        writingRef.current?.capture(mixed, originFromTransaction(transaction, { from: 0, to: 1, text }));
        return;
      }
      const candidate = capturedCandidateEditFromTransaction(transaction);
      if (candidate !== undefined) {
        const { proposal, priorText, from, to, text, resultingBody } = candidate;
        const origin = originFromTransaction(transaction, { from, to, text });
        const composing = current.view.composing || composingRef.current;
        if (composing && candidateCompositionDirtyRef.current) return;
        if (composing) {
          candidateCompositionDirtyRef.current = true;
          writingRef.current?.showSaving();
          return;
        }
        writingRef.current?.capture({
          kind: "candidate_selection",
          target: {
            proposal_id: proposal.proposalId,
            operation_id: proposal.operationId,
            revision_id: proposal.revisionId,
            manuscript_block_id: proposal.blockId,
          },
          expectedProposalHeads: proposal.expectedHeads,
          priorText, from, to, text, resultingBody,
        }, origin);
        return;
      }
      if (current.view.composing || composingRef.current) {
        writingRef.current?.showSaving(nextBlocks);
        return;
      }
      if (paragraphsEqual(nextBlocks, observedBlocksRef.current)) return;
      const edit = capturedManuscriptEditFromTransaction(transaction);
      observedBlocksRef.current = nextBlocks;
      capturedInputRef.current += 1;
      if (edit === undefined) {
        writingRef.current?.fail(new Error("Manuscript replacement is not a supported Block edit"));
        return;
      }
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
      const edgeHeads = transaction.getMeta("storyos.inlineEdgeHeads") as string[] | undefined;
      writingRef.current?.capture(edgeHeads === undefined ? edit
        : { ...edit, expectedProposalHeads: edgeHeads }, origin, nextBlocks);
    },
  }, []);

  const undoLifetime = useRef(0);
  useEffect(() => { undoLifetime.current += 1;
    return () => { undoLifetime.current += 1; abandonUndoRef.current?.(); };
  }, [persistWorkspace, editor]);
  onAuthorUndoRef.current = () => {
    const workspace = persistWorkspaceRef.current;
    // One Author Undo is in progress from the key press until it settles.
    if (abandonUndoRef.current !== undefined || editor === null || workspace === undefined) return true;
    const started = undoLifetime.current;
    let abandoned = false;
    const isCurrent = () => !abandoned && started === undoLifetime.current
      && persistWorkspaceRef.current === workspace && !editor.isDestroyed;
    const challengeAdmission = createChallengeAdmissionWait(undoChallengeTimers);
    const abandon = () => {
      abandoned = true;
      challengeAdmission.cancel();
      if (abandonUndoRef.current === abandon) abandonUndoRef.current = undefined;
    };
    abandonUndoRef.current = abandon;
    void (async () => {
      await writingRef.current?.flush();
      if (!isCurrent()) return;
      let waited = false;
      try {
        const settled = await undoOwnedLatestAuthorAction({
          workspace,
          baseUrl,
          fetchImpl,
          cryptoImpl, isCurrent, challengeAdmission,
          onChallengeWait: () => {
            if (!waited) writingRef.current?.showSaving();
            waited = true;
          },
        });
        if (!isCurrent()) return;
        if (settled !== undefined && (settled.effect.kind === "draft_compensated" || settled.effect.kind === "draft_reconciled")) {
          await writingRef.current?.refresh();
          if (!isCurrent()) return;
          onCandidateSettledRef.current?.(); return;
        }
        if (settled !== undefined && settled.effect.kind === "reversal_required") {
          if (waited) await writingRef.current?.refresh();
          onCandidateSettledRef.current?.("proposal_id" in settled ? settled.proposal_id ?? undefined : undefined); return;
        }
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
        await writingRef.current?.refresh();
        onCandidateSettledRef.current?.("proposal_id" in settled ? settled.proposal_id ?? undefined : undefined);
      } catch (error) {
        if (isCurrent()) onFailureRef.current(error);
      }
    })().finally(() => { if (abandonUndoRef.current === abandon) abandonUndoRef.current = undefined; });
    return true;
  };

  useEffect(() => {
    editor?.setEditable(editable);
  }, [editable, editor]);

  useEffect(() => {
    if (editor === null) return;
    projectBlockProposals(editor, proposals);
    if (!editable) { focusedProposalRef.current = undefined; return; }
    if (focusProposal === undefined) return;
    editor.state.doc.descendants((node, position) => {
      if (node.type.name !== "blockProposal" || node.attrs.proposalId !== focusProposal.proposalId
        || node.attrs.operationId !== focusProposal.operationId || node.attrs.revisionId !== focusProposal.revisionId
        || node.attrs.blockId !== focusProposal.blockId) return;
      const identity = `${focusProposal.proposalId}:${focusProposal.operationId}:${focusProposal.revisionId}`;
      if (focusedProposalRef.current !== identity) {
        editor.commands.setTextSelection(position + 1);
        editor.view.focus();
        focusedProposalRef.current = identity;
        onCandidateFocusRef.current?.(focusProposal);
      }
      const candidate = editor.view.nodeDOM(position);
      if (candidate instanceof HTMLElement) {
        candidate.scrollIntoView({ block: "nearest" });
      }
    });
  }, [editor, proposals, focusProposal, editable]);

  useEffect(() => {
    if (editor === null) return;
    const onClick = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const focused = target.closest<HTMLElement>("[data-candidate-proposal-id]");
      onCandidateFocusRef.current?.(focused === null ? undefined : {
        proposalId: focused.dataset.candidateProposalId!, operationId: focused.dataset.proposalOperationId!,
        revisionId: focused.dataset.proposalRevisionId!, blockId: focused.dataset.proposalTargetId!,
      });
      const acceptButton = target.closest<HTMLButtonElement>("button[data-proposal-accept]");
      const rejectButton = target.closest<HTMLButtonElement>("button[data-proposal-reject]");
      const replanButton = target.closest<HTMLButtonElement>("button[data-proposal-replan]");
      const withdrawButton = target.closest<HTMLButtonElement>("button[data-proposal-withdraw]");
      const copyButton = target.closest<HTMLButtonElement>("button[data-proposal-copy]");
      const button = acceptButton ?? rejectButton ?? replanButton ?? withdrawButton ?? copyButton;
      const proposal = button?.closest<HTMLElement>("[data-proposal-id]");
      const proposalId = proposal?.dataset.proposalId;
      const inline = proposalId === undefined ? null
        : editor.view.dom.querySelector(`[data-inline-proposal-id="${proposalId}"]`);
      const text = inline?.textContent ?? proposal?.querySelector(".block-proposal-text")?.textContent;
      if (button === null || button === undefined || proposal === null
        || proposal === undefined || text === null || text === undefined) return;
      const decision = {
        proposalId: proposal.dataset.proposalId ?? "",
        operationId: proposal.dataset.proposalOperationId ?? "",
        revisionId: proposal.dataset.proposalRevisionId ?? "",
        text,
        ...(button.hasAttribute("data-proposal-all") ? { operationIds: proposalsRef.current.find((item) =>
          item.proposalId === proposal.dataset.proposalId && item.operationId === proposal.dataset.proposalOperationId)?.pendingOperationIds } : {}),
      };
      if (copyButton !== null) onCopyProposalRef.current?.(decision.proposalId);
      else if (replanButton !== null) onReplanProposalRef.current?.(decision);
      else if (withdrawButton !== null) onWithdrawProposalRef.current?.(decision);
      else if (acceptButton !== null) onAcceptProposalRef.current?.(decision);
      else onRejectProposalRef.current?.(decision);
    };
    editor.view.dom.addEventListener("click", onClick);
    return () => { editor.view.dom.removeEventListener("click", onClick); };
  }, [editor]);

  const inputAtRender = capturedInputRef.current;
  useEffect(() => {
    if (editor === null || editor.view.composing || composingRef.current) return;
    const identityKey = blocks.map((block) =>
      `${block.manuscript_block_id}:${block.block_kind ?? "paragraph"}`).join(" ");
    const rendered = readManuscriptParagraphs(editor.state.doc);
    const renderedKey = rendered?.map((block) =>
      `${block.manuscript_block_id}:${block.block_kind ?? "paragraph"}`).join(" ");
    // Author input after this render is newer than these blocks. The render for that input does this check again.
    if (capturedInputRef.current === inputAtRender && (renderedKey !== identityKey || (rendered !== undefined
      && !paragraphsEqual(rendered, blocks) && persistWorkspace?.pending.save_state === "saved"
      && persistWorkspace.pending.unsettled_intent_count === 0))) {
      hydrateManuscriptBlocks(editor, blocks);
      projectBlockProposals(editor, proposals);
    }
    observedBlocksRef.current = readManuscriptParagraphs(editor.state.doc)
      ?? blocks.map((block) => ({ ...block }));
    syncManuscriptSurface(editor.view.dom, observedBlocksRef.current);
  }, [blocks.map((block) =>
    `${block.manuscript_block_id}:${block.block_kind ?? "paragraph"}:${block.text}`).join(" "),
    persistWorkspace?.pending.save_state, editor]);

  useEffect(() => {
    if (editor === null || writing === undefined) {
      const detached: ManualInputController = {
        flush: () => Promise.resolve(),
        whenIdle: () => Promise.resolve(),
        installProjection: async () => {},
        hasIncompleteSemanticIntent: () => composingRef.current,
        close() {},
      };
      controllerRef.current = detached;
      return () => {
        if (controllerRef.current === detached) controllerRef.current = null;
      };
    }
    const controller: ManualInputController = {
      installProjection: async () => { await writing.refresh(); },
      flush: () => writing.flush(),
      whenIdle: () => writing.whenIdle(),
      hasIncompleteSemanticIntent: () => composingRef.current || editor.view.composing || writing.holdsInput(),
      close() {},
    };
    controllerRef.current = controller;
    const { dom } = editor.view;
    const onCompositionStart = (): void => {
      composingRef.current = true;
      mixedCompositionRef.current = captureStructuredSelection(editor.state, editor.state.tr.deleteSelection());
      if (mixedCompositionRef.current !== undefined && !writing.canAcceptInput(true)) mixedCompositionRef.current = undefined;
      mixedCompositionStartRef.current = mixedCompositionRef.current === undefined ? null : editor.state.doc;
      const parent = editor.state.selection.$from.parent;
      const candidateSelected = parent.type.name === "blockProposal" || parent.type.name === "inlineProposal";
      candidateCompositionBlockedRef.current = candidateSelected && !writing.canAcceptInput(true,
        { proposalId: String(parent.attrs.proposalId), operationId: String(parent.attrs.operationId) });
      candidateCompositionStartRef.current = candidateSelected
        && !candidateCompositionBlockedRef.current ? editor.state.doc : null;
      candidateCompositionDirtyRef.current = false;
      writing.setHoldSubmission(true);
    };
    const onCompositionEnd = (event: CompositionEvent): void => {
      composingRef.current = false;
      writing.setHoldSubmission(false);
      const candidateStart = candidateCompositionStartRef.current;
      candidateCompositionStartRef.current = null;
      candidateCompositionBlockedRef.current = false;
      candidateCompositionDirtyRef.current = false;
      const mixed = mixedCompositionRef.current;
      mixedCompositionRef.current = undefined;
      const mixedStart = mixedCompositionStartRef.current;
      mixedCompositionStartRef.current = null;
      if (mixed !== undefined) {
        if (mixedStart !== null) editor.view.dispatch(editor.state.tr
          .replaceWith(0, editor.state.doc.content.size, mixedStart.content).setMeta("storyos.hydrate", true));
        if (event.data !== "") {
          const primitive = mixed.authorEditUnit.normalized_primitives[0];
          if (primitive?.kind === "replace_structured_selection") {
            primitive.replacement = [{ block_kind: primitive.replacement[0]!.block_kind, text: event.data }];
            writing.capture(mixed, "composition_confirmation");
          }
        }
        return;
      }
      if (candidateStart !== null) {
        const captured = capturedCandidateEdit(candidateStart, editor.state.doc);
        if (!captured.valid) {
          writing.fail(new Error("Candidate composition is not a supported edit"));
          return;
        }
        if (captured.edit !== undefined) {
          const { proposal, priorText, from, to, text, resultingBody } = captured.edit;
          writing.capture({
            kind: "candidate_selection",
            target: {
              proposal_id: proposal.proposalId,
              operation_id: proposal.operationId,
              revision_id: proposal.revisionId,
              manuscript_block_id: proposal.blockId,
            },
            expectedProposalHeads: proposal.expectedHeads,
            priorText, from, to, text, resultingBody,
          }, "composition_confirmation");
        }
        return;
      }
      const nextBlocks = readManuscriptParagraphs(editor.state.doc);
      if (nextBlocks === undefined || paragraphsEqual(nextBlocks, observedBlocksRef.current)) {
        return;
      }
      const edit = captureManuscriptChange(observedBlocksRef.current, nextBlocks);
      observedBlocksRef.current = nextBlocks;
      capturedInputRef.current += 1;
      if (edit === undefined) {
        writing.fail(new Error("Manuscript replacement is not a supported Block edit"));
        return;
      }
      writing.capture(edit, "composition_confirmation", nextBlocks);
    };
    dom.addEventListener("compositionstart", onCompositionStart, true);
    dom.addEventListener("compositionend", onCompositionEnd);
    return () => {
      dom.removeEventListener("compositionstart", onCompositionStart, true);
      dom.removeEventListener("compositionend", onCompositionEnd);
      if (controllerRef.current === controller) controllerRef.current = null;
    };
  }, [controllerRef, editor, writing]);

  if (editor === null) return null;
  return <EditorContent editor={editor} />;
}
