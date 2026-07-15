import { PROPOSAL_META } from "./proposal-extension.js";

export const REJECTED_REVISION_VERSION = 1;

function topLevelBlocks(doc) {
  const blocks = [];

  doc.forEach((node, offset, index) => {
    blocks.push({
      index,
      from: offset,
      to: offset + node.nodeSize,
      blockId: node.attrs?.blockId ?? null,
      node,
    });
  });

  return blocks;
}

export function captureRejectedProposalRevision({
  doc,
  proposalId,
  proposalRevision,
  chapterId,
  blockIds,
}) {
  const requestedIds = new Set(blockIds);
  if (!requestedIds.size) return { ok: false, reason: "empty_proposal" };

  const blocks = topLevelBlocks(doc);
  const selected = blocks.filter((block) => requestedIds.has(block.blockId));

  if (selected.length !== requestedIds.size) {
    return { ok: false, reason: "proposal_block_missing" };
  }

  const firstIndex = selected[0].index;
  const contiguous = selected.every(
    (block, index) => block.index === firstIndex + index,
  );
  if (!contiguous) return { ok: false, reason: "proposal_not_contiguous" };

  const previousBlock = blocks[firstIndex - 1] ?? null;
  const nextBlock = blocks[firstIndex + selected.length] ?? null;

  return {
    ok: true,
    revision: {
      schemaVersion: REJECTED_REVISION_VERSION,
      proposalId,
      proposalRevision,
      chapterId,
      blocks: selected.map((block) => block.node.toJSON()),
      placement: {
        previousBlockId: previousBlock?.blockId ?? null,
        nextBlockId: nextBlock?.blockId ?? null,
      },
    },
  };
}

function resolveInsertionPosition(doc, placement) {
  const blocks = topLevelBlocks(doc);
  const previousIndex = placement.previousBlockId
    ? blocks.findIndex((block) => block.blockId === placement.previousBlockId)
    : -1;
  const nextIndex = placement.nextBlockId
    ? blocks.findIndex((block) => block.blockId === placement.nextBlockId)
    : -1;

  if (placement.previousBlockId && previousIndex === -1) {
    return { ok: false, reason: "previous_anchor_missing" };
  }
  if (placement.nextBlockId && nextIndex === -1) {
    return { ok: false, reason: "next_anchor_missing" };
  }

  if (placement.previousBlockId && placement.nextBlockId) {
    if (nextIndex !== previousIndex + 1) {
      return { ok: false, reason: "anchors_not_adjacent" };
    }
    return { ok: true, position: blocks[nextIndex].from };
  }

  if (placement.previousBlockId) {
    if (previousIndex !== blocks.length - 1) {
      return { ok: false, reason: "end_anchor_moved" };
    }
    return { ok: true, position: blocks[previousIndex].to };
  }

  if (placement.nextBlockId) {
    if (nextIndex !== 0) {
      return { ok: false, reason: "start_anchor_moved" };
    }
    return { ok: true, position: blocks[nextIndex].from };
  }

  return blocks.length
    ? { ok: false, reason: "unanchored_nonempty_document" }
    : { ok: true, position: 0 };
}

export function createReopenProposalTransaction(state, rejectedRevision) {
  if (
    rejectedRevision?.schemaVersion !== REJECTED_REVISION_VERSION ||
    !Array.isArray(rejectedRevision.blocks) ||
    !rejectedRevision.blocks.length ||
    !rejectedRevision.placement
  ) {
    return { ok: false, reason: "invalid_rejected_revision" };
  }

  const blockIds = rejectedRevision.blocks.map(
    (block) => block.attrs?.blockId ?? null,
  );
  if (blockIds.some((blockId) => !blockId) || new Set(blockIds).size !== blockIds.length) {
    return { ok: false, reason: "invalid_candidate_blocks" };
  }

  const existingIds = new Set(
    topLevelBlocks(state.doc).map((block) => block.blockId).filter(Boolean),
  );
  if (blockIds.some((blockId) => existingIds.has(blockId))) {
    return { ok: false, reason: "candidate_already_present" };
  }

  const insertion = resolveInsertionPosition(
    state.doc,
    rejectedRevision.placement,
  );
  if (!insertion.ok) return insertion;

  let candidateDocument;
  try {
    candidateDocument = state.schema.nodeFromJSON({
      type: "doc",
      content: rejectedRevision.blocks,
    });
  } catch {
    return { ok: false, reason: "invalid_candidate_document" };
  }

  const transaction = state.tr
    .insert(insertion.position, candidateDocument.content)
    .setMeta(PROPOSAL_META, { allow: true, source: "reopen_rejected" })
    .setMeta("addToHistory", false);

  return { ok: true, transaction, blockIds };
}
