import { useEffect, useRef, useSyncExternalStore } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { TextSelection } from "@tiptap/pm/state";

import { firstOwnerRange } from "./first-owner-range.ts";

import type { EditorSessionWritingController } from "./editor-session-writing.ts";
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

import type { ProposalFocus } from "./proposal-navigation.ts";
import { AddReferenceControl, type EditorReferenceSource } from "./selection-reference-control.tsx";
import {
  rebaseDraftReferences, trackableDraftReferences, trackDraftReferences,
} from "./selection-reference-draft.ts";

export interface ManuscriptEditorProps {
  focusProposal?: ProposalFocus | undefined;
  blocks: readonly ManuscriptParagraph[];
  proposals?: readonly BlockProposalProjection[];
  editable: boolean;
  /** The writing controller of the current-writer Editor Session. */
  writing?: EditorSessionWritingController | undefined;
  onCandidateSettled?: (proposalId?: string) => void;
  /** Receives the Current Chapter after an Author Undo compensation. */
  onUndoCurrentChapter?: (chapterId: string) => void;
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
  onWithdrawProposal?: (target: {
    proposalId: string;
    operationId: string;
    revisionId: string;
    text: string;
  }) => void;
  onCopyProposal?: (proposalId: string) => void;
  /** The source of the references that the author adds from a selection. */
  referenceSource?: EditorReferenceSource | undefined;
}

const ignoreChanges = () => () => {};

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
  writing,
  onCandidateSettled, onUndoCurrentChapter, focusProposal,
  onAcceptProposal,
  onRejectProposal,
  onWithdrawProposal,
  onCopyProposal,
  referenceSource,
}: ManuscriptEditorProps) {
  const observedBlocksRef = useRef<ManuscriptParagraph[]>(blocks.map((block) => ({ ...block })));
  const capturedInputRef = useRef(0);
  const composingRef = useRef(false);
  const focusedProposalRef = useRef<string | undefined>(undefined);
  // The first owner part of a composition over a crossing selection, and the document at its start.
  const crossingCompositionRef = useRef<{ from: number; to: number; anchor: number; head: number } | undefined>(undefined);
  const crossingCompositionStartRef = useRef<ProseMirrorNode | null>(null);
  const candidateCompositionStartRef = useRef<ProseMirrorNode | null>(null);
  const candidateCompositionBlockedRef = useRef(false);
  const candidateCompositionDirtyRef = useRef(false);
  const writingRef = useRef(writing);
  writingRef.current = writing;
  const onCandidateSettledRef = useRef(onCandidateSettled);
  const onUndoCurrentChapterRef = useRef(onUndoCurrentChapter);
  const proposalsRef = useRef(proposals);
  proposalsRef.current = proposals;
  const onAcceptProposalRef = useRef(onAcceptProposal);
  const onRejectProposalRef = useRef(onRejectProposal);
  const onWithdrawProposalRef = useRef(onWithdrawProposal);
  const onCopyProposalRef = useRef(onCopyProposal);
  const onAuthorUndoRef = useRef<() => boolean>(() => true);
  const referenceSourceRef = useRef(referenceSource);
  referenceSourceRef.current = referenceSource;
  // The draft references of this Chapter that the editor moves with the author edits.
  const trackedReferencesRef = useRef(new Set<string>());
  const firstBlockId = blocks[0]?.manuscript_block_id ?? "";
  onCandidateSettledRef.current = onCandidateSettled;
  onUndoCurrentChapterRef.current = onUndoCurrentChapter;
  onAcceptProposalRef.current = onAcceptProposal;
  onRejectProposalRef.current = onRejectProposal;
  onWithdrawProposalRef.current = onWithdrawProposal;
  onCopyProposalRef.current = onCopyProposal;
  const editor = useEditor({
    extensions: [
      ...storyosManuscriptExtensions(firstBlockId,
        () => onAuthorUndoRef.current(),
        (hardBoundary, candidate) => !candidateCompositionBlockedRef.current
          && writingRef.current?.canAcceptInput(hardBoundary, candidate) === true,
        () => composingRef.current && crossingCompositionRef.current !== undefined),
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
      const source = referenceSourceRef.current;
      if (source !== undefined && transaction.docChanged) {
        trackDraftReferences(source.scope, source.chapterId, trackedReferencesRef.current, transaction, source.tree);
        // A hydrate of a saved editor shows its Revision, so the kept references take it as their base.
        // The editor also tracks the references at that Revision, for example of an accepted candidate.
        if (source.saved && isStoryosHydrateTransaction(transaction)) {
          rebaseDraftReferences(source.scope, source.chapterId, trackedReferencesRef.current, source.revisionId);
          for (const key of trackableDraftReferences(source.scope, source.chapterId, source.revisionId)) {
            trackedReferencesRef.current.add(key);
          }
        }
      }
      syncManuscriptSurface(current.view.dom, nextBlocks);
      if (isStoryosHydrateTransaction(transaction) || !transaction.docChanged) {
        if (!current.view.composing && !composingRef.current) observedBlocksRef.current = nextBlocks;
        return;
      }
      // New input changes the Author Undo Frontier, so an Undo in progress can only conflict.
      writingRef.current?.abandonUndo();
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

  onAuthorUndoRef.current = () => {
    const current = writingRef.current;
    if (editor === null || current === undefined) return true;
    void current.undo().then((outcome) => {
      if (outcome !== undefined) onCandidateSettledRef.current?.(outcome.proposalId);
      if (outcome?.currentChapterId !== undefined) onUndoCurrentChapterRef.current?.(outcome.currentChapterId);
    });
    return true;
  };

  useEffect(() => {
    editor?.setEditable(editable);
  }, [editable, editor]);

  useEffect(() => {
    if (referenceSource === undefined) return;
    const tracked = trackedReferencesRef.current;
    tracked.clear();
    for (const key of trackableDraftReferences(referenceSource.scope, referenceSource.chapterId,
      referenceSource.revisionId)) tracked.add(key);
  }, [editor, referenceSource?.chapterId]);
  useEffect(() => {
    if (referenceSource?.saved !== true) return;
    rebaseDraftReferences(referenceSource.scope, referenceSource.chapterId, trackedReferencesRef.current,
      referenceSource.revisionId);
  }, [referenceSource?.saved, referenceSource?.revisionId]);

  useEffect(() => {
    // A projection during IME composition replaces the composed text. The capture at its end renders again.
    if (editor === null || editor.view.composing || composingRef.current) return;
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
      const acceptButton = target.closest<HTMLButtonElement>("button[data-proposal-accept]");
      const rejectButton = target.closest<HTMLButtonElement>("button[data-proposal-reject]");
      const withdrawButton = target.closest<HTMLButtonElement>("button[data-proposal-withdraw]");
      const copyButton = target.closest<HTMLButtonElement>("button[data-proposal-copy]");
      const button = acceptButton ?? rejectButton ?? withdrawButton ?? copyButton;
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
      else if (withdrawButton !== null) onWithdrawProposalRef.current?.(decision);
      else if (acceptButton !== null) onAcceptProposalRef.current?.(decision);
      else onRejectProposalRef.current?.(decision);
    };
    editor.view.dom.addEventListener("click", onClick);
    return () => { editor.view.dom.removeEventListener("click", onClick); };
  }, [editor]);

  const written = useSyncExternalStore(writing?.subscribe ?? ignoreChanges, () => writing?.snapshot());
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
      && !paragraphsEqual(rendered, blocks) && written?.projection.save_state === "saved"
      && written.projection.unsettled_intent_count === 0))) {
      hydrateManuscriptBlocks(editor, blocks);
      projectBlockProposals(editor, proposals);
    }
    observedBlocksRef.current = readManuscriptParagraphs(editor.state.doc)
      ?? blocks.map((block) => ({ ...block }));
    syncManuscriptSurface(editor.view.dom, observedBlocksRef.current);
  }, [blocks.map((block) =>
    `${block.manuscript_block_id}:${block.block_kind ?? "paragraph"}:${block.text}`).join(" "),
    written?.projection.save_state, editor]);

  useEffect(() => {
    if (editor === null || writing === undefined) return;
    const { dom } = editor.view;
    const onCompositionStart = (): void => {
      composingRef.current = true;
      const { from, to, anchor, head } = editor.state.selection;
      const range = firstOwnerRange(editor.state.doc, from, to);
      const crossing = range.from !== from || range.to !== to;
      crossingCompositionRef.current = crossing ? { ...range, anchor, head } : undefined;
      crossingCompositionStartRef.current = crossing ? editor.state.doc : null;
      const parent = editor.state.selection.$from.parent;
      const candidateSelected = !crossing
        && (parent.type.name === "blockProposal" || parent.type.name === "inlineProposal");
      candidateCompositionBlockedRef.current = candidateSelected && !writing.canAcceptInput(true,
        { proposalId: String(parent.attrs.proposalId), operationId: String(parent.attrs.operationId) });
      candidateCompositionStartRef.current = candidateSelected
        && !candidateCompositionBlockedRef.current ? editor.state.doc : null;
      candidateCompositionDirtyRef.current = false;
      writing.setComposing(true);
    };
    const onCompositionEnd = (event: CompositionEvent): void => {
      composingRef.current = false;
      writing.setComposing(false);
      const candidateStart = candidateCompositionStartRef.current;
      candidateCompositionStartRef.current = null;
      candidateCompositionBlockedRef.current = false;
      candidateCompositionDirtyRef.current = false;
      const crossing = crossingCompositionRef.current;
      crossingCompositionRef.current = undefined;
      const crossingStart = crossingCompositionStartRef.current;
      crossingCompositionStartRef.current = null;
      if (crossing !== undefined && crossingStart !== null) {
        // The confirmed text replaces only the first owner part, as one ordinary edit, and the caret stays
        // at its end (ADR 0072). A cancelled composition restores the start selection.
        const restored = editor.state.tr.replaceWith(0, editor.state.doc.content.size, crossingStart.content);
        editor.view.dispatch(restored.setSelection(TextSelection.create(restored.doc, crossing.anchor, crossing.head))
          .setMeta("storyos.hydrate", true));
        if (event.data === "") return;
        const confirmed = editor.state.tr.insertText(event.data, crossing.from, crossing.to);
        editor.view.dispatch(confirmed.setSelection(TextSelection.create(confirmed.doc, crossing.from + event.data.length))
          .setMeta("storyos.origin", "composition_confirmation"));
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
      // A closed editor abandons its Author Undo (ADR 0038).
      writing.abandonUndo();
    };
  }, [editor, writing]);

  if (editor === null) return null;
  return <>
    <EditorContent editor={editor} />
    {referenceSource === undefined ? null
      : <AddReferenceControl editor={editor} source={referenceSource} tracked={trackedReferencesRef.current} />}
  </>;
}
