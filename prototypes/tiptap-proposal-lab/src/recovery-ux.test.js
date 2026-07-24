import assert from "node:assert/strict";
import test from "node:test";
import {
  getUxScenario,
  preservedTextToParagraphs,
  resolveUxAction,
  UX_ACTIONS,
  UX_SCENARIO_IDS,
} from "./recovery-ux.js";
import { proposalText } from "./data.js";

test("covers the complete production result matrix and recovery artifacts", () => {
  assert.deepEqual(
    UX_SCENARIO_IDS.slice(0, 5).map((id) => getUxScenario(id).result),
    ["authoritative", "proposal", "refused", "conflicted", "no-effect"],
  );
  assert.deepEqual(
    UX_SCENARIO_IDS.slice(2)
      .map((id) => getUxScenario(id).artifactKind)
      .filter(Boolean),
    [
      "Refused Edit Draft",
      "Proposal Conflict",
      "Recovery Draft",
      "Proposal Recovery Conflict",
    ],
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
