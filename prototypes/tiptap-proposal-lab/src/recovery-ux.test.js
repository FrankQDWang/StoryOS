import assert from "node:assert/strict";
import test from "node:test";
import {
  getUxScenario,
  preservedTextToParagraphs,
  resolveNarrowedRetry,
  resolveUxAction,
  settleWithoutProposalCandidate,
  UX_ACTIONS,
  UX_SCENARIO_IDS,
} from "./recovery-ux.js";
import { proposalText } from "./data.js";

test("covers the result matrix with exactly two Draft Artifacts", () => {
  assert.deepEqual(
    UX_SCENARIO_IDS.slice(0, 5).map((id) => getUxScenario(id).result),
    ["authoritative", "proposal", "refused", "conflicted", "no-effect"],
  );
  assert.deepEqual(
    UX_SCENARIO_IDS.slice(2)
      .map((id) => getUxScenario(id).draftArtifact)
      .filter(Boolean),
    ["Refused Edit Draft", "Recovery Draft"],
  );
  assert.deepEqual(
    [
      getUxScenario("conflicted").condition,
      getUxScenario("proposal-recovery-conflict").condition,
    ],
    ["Proposal Conflict", "Proposal Recovery Conflict"],
  );
  assert.deepEqual(
    [
      getUxScenario("proposal").preservedSurface,
      getUxScenario("conflicted").preservedSurface,
      getUxScenario("proposal-recovery-conflict").preservedSurface,
    ],
    ["Proposal", "Proposal", "Proposal"],
  );
});

test("exposes only the actions that apply to each result", () => {
  assert.deepEqual(getUxScenario("authoritative").actions, []);
  assert.deepEqual(getUxScenario("proposal").actions, ["accept", "reject"]);
  assert.deepEqual(getUxScenario("refused").actions, [
    "narrow",
    "copy",
    "expand",
    "discard",
  ]);
  assert.deepEqual(getUxScenario("conflicted").actions, [
    "replan",
    "copy",
    "reject",
  ]);
  assert.deepEqual(getUxScenario("no-effect").actions, []);
  assert.deepEqual(getUxScenario("recovery-draft").actions, [
    "retry",
    "copy",
    "discard",
  ]);
  assert.deepEqual(getUxScenario("proposal-recovery-conflict").actions, [
    "replan",
    "copy",
    "withdraw",
  ]);
});

test("carries the complete attempted text through every applicable transition", () => {
  assert.equal(getUxScenario("conflicted").attemptedText, proposalText);
  assert.equal(
    getUxScenario("proposal-recovery-conflict").attemptedText,
    proposalText,
  );

  const cases = [
    ["refused", "narrow"],
    ["refused", "expand"],
    ["refused", "discard"],
    ["conflicted", "replan"],
    ["conflicted", "reject"],
    ["recovery-draft", "retry"],
    ["recovery-draft", "discard"],
    ["proposal-recovery-conflict", "replan"],
    ["proposal-recovery-conflict", "withdraw"],
  ];

  for (const [scenarioId, actionId] of cases) {
    const attemptedText = getUxScenario(scenarioId).attemptedText;
    assert.equal(
      resolveUxAction({ scenarioId, actionId }).preservedText,
      attemptedText,
    );
  }
});

test("maps actions to a fresh result and rejects stale controls", () => {
  assert.equal(
    resolveUxAction({
      scenarioId: "recovery-draft",
      actionId: "retry",
    }).nextScenarioId,
    "authoritative",
  );
  assert.equal(
    resolveUxAction({
      scenarioId: "conflicted",
      actionId: "replan",
    }).nextScenarioId,
    "proposal",
  );
  assert.equal(
    resolveUxAction({
      scenarioId: "proposal-recovery-conflict",
      actionId: "withdraw",
    }).nextScenarioId,
    "no-effect",
  );
  assert.throws(
    () =>
      resolveUxAction({
        scenarioId: "authoritative",
        actionId: "retry",
      }),
    /not available/,
  );
  assert.throws(
    () =>
      resolveUxAction({
        scenarioId: "conflicted",
        actionId: "accept",
      }),
    /not available/,
  );
  assert.equal(UX_ACTIONS.replan.label, "基于当前正文重新规划");
});

test("keeps narrowed authoritative retry distinct from Proposal expansion", () => {
  const scenario = getUxScenario("refused");
  const narrowed = resolveNarrowedRetry({
    scenarioId: scenario.id,
    attemptedText: scenario.attemptedText,
    retryText: scenario.narrowedRetry.initialText,
  });
  const expanded = resolveUxAction({
    scenarioId: scenario.id,
    actionId: "expand",
  });

  assert.deepEqual(narrowed, {
    preservedDraftText: scenario.attemptedText,
    retryText: scenario.narrowedRetry.initialText,
    target: {
      initialText: scenario.narrowedRetry.initialText,
      targetId: "chapter-12-authority-end",
      targetLabel: "当前章节正文 · 第十二章末尾",
      authorLabel: "第十二章末尾 · 仅正文",
      ownership: "authoritative-only",
      representativeResult: "authoritative",
    },
    nextScenarioId: "authoritative",
  });
  assert.equal(expanded.preservedText, scenario.attemptedText);
  assert.equal(expanded.nextScenarioId, "proposal");
  assert.notEqual(narrowed.retryText, expanded.preservedText);
});

test("refuses an empty, unchanged, or unsupported narrowed retry", () => {
  const scenario = getUxScenario("refused");
  for (const retryText of ["", scenario.attemptedText]) {
    assert.throws(
      () =>
        resolveNarrowedRetry({
          scenarioId: scenario.id,
          attemptedText: scenario.attemptedText,
          retryText,
        }),
      /smaller nonempty edit/,
    );
  }
  assert.throws(
    () =>
      resolveNarrowedRetry({
        scenarioId: "recovery-draft",
        retryText: "较小文本",
      }),
    /not available/,
  );
});

test("authoritative retries settle without fabricating Proposal acceptance", () => {
  const proposal = {
    id: "proposal-rain-night-continuation",
    blockIds: ["proposal-block-1"],
    validation: "conflicted",
    resolution: "pending",
    closure: "open",
    acceptance: { receiptId: "stale-receipt" },
    acceptanceRedoAvailable: true,
    editorHistoryDepth: 2,
    rejectedRevision: 2,
    conflictReason: "target_revision_changed",
    creator: "agent",
    authorAction: { kind: "acceptance", safe: true },
  };

  for (const authorActionKind of [
    "narrowed_refused_edit_retry_authoritative",
    "retry_recovery_draft",
  ]) {
    assert.deepEqual(
      settleWithoutProposalCandidate(proposal, authorActionKind),
      {
        ...proposal,
        blockIds: [],
        validation: "valid",
        resolution: "none",
        closure: "closed",
        acceptance: null,
        acceptanceRedoAvailable: false,
        editorHistoryDepth: 0,
        rejectedRevision: null,
        conflictReason: null,
        creator: "author",
        authorAction: { kind: authorActionKind, safe: true },
      },
    );
  }
});

test("keeps blank lines when preserved text becomes editor paragraphs", () => {
  const preservedText = "\n第一段\n\n第三段\n";
  const paragraphs = preservedTextToParagraphs(preservedText, "recovered");

  assert.equal(paragraphs.length, 5);
  assert.equal(
    paragraphs
      .map((paragraph) => paragraph.content?.[0]?.text ?? "")
      .join("\n"),
    preservedText,
  );
  assert.deepEqual(
    paragraphs.map((paragraph) => paragraph.attrs.blockId),
    [
      "recovered-1",
      "recovered-2",
      "recovered-3",
      "recovered-4",
      "recovered-5",
    ],
  );
});
