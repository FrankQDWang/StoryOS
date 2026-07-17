import assert from "node:assert/strict";
import test from "node:test";
import {
  admitProposalOperation,
  classifyInlineSpan,
  decideProposalEditingAdmission,
  expectedJoinIds,
  expectedRedoIds,
  expectedRetypeId,
  expectedSplitIds,
  expectedTransferId,
  mapInlineRange,
  resolveProposalStructuralReshape,
} from "./proposal-contract.js";

const range = { from: 10, to: 20 };

test("treats both exact inline Proposal boundaries as authoritative", () => {
  assert.equal(classifyInlineSpan(range, 10, 10), "authority");
  assert.equal(classifyInlineSpan(range, 20, 20), "authority");
  assert.equal(classifyInlineSpan(range, 15, 15), "proposal");
});

test("refuses a span that crosses Proposal ownership", () => {
  assert.equal(classifyInlineSpan(range, 9, 11), "mixed");
  assert.equal(classifyInlineSpan(range, 19, 21), "mixed");
  assert.equal(classifyInlineSpan(range, 10, 20), "proposal");
});

test("maps exclusive boundaries without absorbing adjacent insertions", () => {
  const mapping = {
    map(position, association) {
      if (position === 10 && association === 1) return 12;
      if (position === 20 && association === -1) return 20;
      return position;
    },
  };

  assert.deepEqual(mapInlineRange(range, mapping), { from: 12, to: 20 });
});

test("admits full Proposal editing only when every evidence gate passes", () => {
  assert.deepEqual(
    decideProposalEditingAdmission({
      supportProfileMatches: false,
      compatibilityEvidenceMatches: true,
      runtimeCapabilitiesPass: true,
    }),
    { mode: "safe", reason: "unsupported_environment" },
  );
  assert.deepEqual(
    decideProposalEditingAdmission({
      compatibilityEvidenceMatches: true,
      runtimeCapabilitiesPass: true,
    }),
    { mode: "full", reason: null },
  );
  assert.deepEqual(
    decideProposalEditingAdmission({
      compatibilityEvidenceMatches: false,
      runtimeCapabilitiesPass: true,
    }),
    { mode: "safe", reason: "compatibility_evidence_mismatch" },
  );
});

test("preserves deterministic block identity across structural actions", () => {
  assert.deepEqual(expectedSplitIds("left", "right"), ["left", "right"]);
  assert.deepEqual(expectedJoinIds("left"), ["left"]);
  assert.equal(
    expectedTransferId({ blockId: "block", kind: "move", newId: "copy" }),
    "block",
  );
  assert.equal(
    expectedTransferId({ blockId: "block", kind: "copy", newId: "copy" }),
    "copy",
  );
  assert.equal(expectedRetypeId("block"), "block");
  assert.deepEqual(expectedRedoIds(["left", "right"], false), ["left", "right"]);
  assert.equal(expectedRedoIds(["left", "right"], true), null);
});

test("admits at most one unresolved Proposal operation per block", () => {
  assert.deepEqual(
    admitProposalOperation({
      unresolvedOperationIds: ["operation-a"],
      candidateOperationId: "operation-a",
    }),
    { admitted: true, reason: null },
  );
  assert.deepEqual(
    admitProposalOperation({
      unresolvedOperationIds: ["operation-a"],
      candidateOperationId: "operation-b",
    }),
    { admitted: false, reason: "proposal_block_exclusivity" },
  );
});

test("preserves a structural author edit while requiring explicit replanning", () => {
  assert.deepEqual(resolveProposalStructuralReshape({ operationId: "operation-a" }), {
    preserveAuthorEdit: true,
    createProposalRevision: true,
    validation: "conflicted",
    replan: {
      required: true,
      operationId: "operation-a",
      choices: ["retain_operation_id", "replace_operation_id"],
    },
  });
});
