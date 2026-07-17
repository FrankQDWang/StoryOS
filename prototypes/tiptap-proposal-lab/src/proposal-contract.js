export const PROPOSAL_EDITING_PROFILE = Object.freeze({
  browser: "Chrome",
  authorInputLanguages: ["zh", "en"],
  tiptap: "3.27.3",
  prosemirrorModel: "1.25.11",
  prosemirrorState: "1.4.4",
  prosemirrorView: "1.42.1",
  coordinateSpace: "prosemirror-token-offset-v1",
});

export function classifyInlineSpan(range, from, to) {
  if (from === to) {
    return from > range.from && from < range.to ? "proposal" : "authority";
  }

  const touchesProposal = from < range.to && to > range.from;
  const touchesAuthority = from < range.from || to > range.to;

  if (touchesProposal && touchesAuthority) return "mixed";
  if (touchesProposal) return "proposal";
  return "authority";
}

export function classifyInlineTransaction(transaction, range) {
  const ownerships = new Set();
  let mappedRange = range;

  transaction.steps.forEach((step) => {
    step.getMap().forEach((oldStart, oldEnd) => {
      ownerships.add(classifyInlineSpan(mappedRange, oldStart, oldEnd));
    });
    mappedRange = mapInlineRange(mappedRange, step.getMap());
  });

  if (ownerships.has("mixed")) return "mixed";
  if (ownerships.has("proposal") && ownerships.has("authority")) return "mixed";
  if (ownerships.has("proposal")) return "proposal";
  if (ownerships.has("authority")) return "authority";
  return "none";
}

export function classifyDocumentDiff(beforeDoc, afterDoc, range) {
  const from = beforeDoc.content.findDiffStart(afterDoc.content);
  if (from == null) return "none";

  const ends = beforeDoc.content.findDiffEnd(afterDoc.content);
  const to = ends?.a ?? from;
  return classifyInlineSpan(range, from, to);
}

export function mapInlineRange(range, mapping) {
  return {
    from: mapping.map(range.from, 1),
    to: mapping.map(range.to, -1),
  };
}

export function createRefusedEditDraft({
  intent,
  beforeDoc,
  afterDoc,
  beforeSelection,
  afterSelection,
  steps = [],
}) {
  return {
    kind: "refused_edit_draft",
    createdAt: new Date().toISOString(),
    intent,
    beforeSelection,
    afterSelection,
    beforeText: beforeDoc.textContent,
    attemptedText: afterDoc.textContent,
    steps,
    recoveryChoices: ["narrow_and_retry", "expand_proposal", "discard"],
  };
}

export function decideProposalEditingAdmission({
  supportProfileMatches = true,
  compatibilityEvidenceMatches,
  runtimeCapabilitiesPass,
  invariantViolation = false,
}) {
  const full =
    supportProfileMatches &&
    compatibilityEvidenceMatches &&
    runtimeCapabilitiesPass &&
    !invariantViolation;

  return {
    mode: full ? "full" : "safe",
    reason: full
      ? null
      : invariantViolation
        ? "invariant_violation"
        : !supportProfileMatches
          ? "unsupported_environment"
        : compatibilityEvidenceMatches
          ? "runtime_capability_mismatch"
          : "compatibility_evidence_mismatch",
  };
}

export function expectedSplitIds(originalId, newRightId) {
  return [originalId, newRightId];
}

export function expectedJoinIds(leftId) {
  return [leftId];
}

export function expectedTransferId({ blockId, kind, newId }) {
  return kind === "move" ? blockId : newId;
}

export function expectedRetypeId(blockId) {
  return blockId;
}

export function expectedRedoIds(previousIds, stateDrifted) {
  return stateDrifted ? null : [...previousIds];
}

export function admitProposalOperation({
  unresolvedOperationIds,
  candidateOperationId,
}) {
  const competing = unresolvedOperationIds.filter(
    (operationId) => operationId !== candidateOperationId,
  );
  return {
    admitted: competing.length === 0,
    reason: competing.length === 0 ? null : "proposal_block_exclusivity",
  };
}

export function resolveProposalStructuralReshape({ operationId }) {
  return {
    preserveAuthorEdit: true,
    createProposalRevision: true,
    validation: "conflicted",
    replan: {
      required: true,
      operationId,
      choices: ["retain_operation_id", "replace_operation_id"],
    },
  };
}
