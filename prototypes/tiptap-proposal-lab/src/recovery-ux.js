import { proposalText } from "./data.js";

export const UX_SCENARIO_IDS = Object.freeze([
  "authoritative",
  "proposal",
  "refused",
  "conflicted",
  "no-effect",
  "recovery-draft",
  "proposal-recovery-conflict",
]);

export const UX_ACTIONS = Object.freeze({
  accept: { label: "接受", nextScenarioId: "authoritative" },
  reject: { label: "拒绝这份提案", nextScenarioId: "no-effect" },
  narrow: { label: "缩小编辑范围", nextPhase: "narrowing" },
  retry: { label: "重试保存", nextScenarioId: "authoritative" },
  copy: { label: "复制全文" },
  expand: { label: "扩展提案后重试", nextScenarioId: "proposal" },
  replan: { label: "基于当前正文重新规划", nextScenarioId: "proposal" },
  withdraw: { label: "撤回这份提案", nextScenarioId: "no-effect" },
  discard: { label: "丢弃草稿", nextScenarioId: "no-effect" },
});

const refusedAttempt = [
  "雨下得更急了，他把信压进袖中。",
  "更鼓将尽，苏砚收起信，撑伞往西。",
  "街角的灯火在水里碎开，他没有回头。",
].join("\n");

const recoveryAttempt = [
  "他推开旧仓侧门，铁锈落在指节上。",
  "梁下的雨滴一声一声敲着空桶。",
  "门后的脚步停住了。",
].join("\n");

const scenarios = Object.freeze({
  authoritative: Object.freeze({
    id: "authoritative",
    result: "authoritative",
    tone: "settled",
    eyebrow: "已写入正文",
    title: "正文已保存",
    description: "这次编辑完整写入当前章节，没有留下需要处理的草稿。",
    attemptedText: null,
    actions: Object.freeze([]),
  }),
  proposal: Object.freeze({
    id: "proposal",
    result: "proposal",
    tone: "proposal",
    eyebrow: null,
    title: null,
    description: null,
    attemptedText: null,
    actions: Object.freeze(["accept", "reject"]),
  }),
  refused: Object.freeze({
    id: "refused",
    result: "refused",
    artifactKind: "Refused Edit Draft",
    tone: "attention",
    eyebrow: "未写入 · Refused Edit Draft",
    title: "这次编辑跨过了正文和提案边界",
    description:
      "StoryOS 没有写入其中任何一部分。完整尝试已保存在下面，你可以先缩小范围，或把它扩展为一份提案。",
    attemptedText: refusedAttempt,
    textLabel: "完整编辑尝试",
    actions: Object.freeze(["narrow", "copy", "expand", "discard"]),
  }),
  conflicted: Object.freeze({
    id: "conflicted",
    result: "conflicted",
    artifactKind: "Proposal Conflict",
    tone: "attention",
    eyebrow: "目标已变化 · Proposal Conflict",
    title: "这份提案不能按原位置接受",
    description:
      "当前正文已经变化。StoryOS 没有猜测插入位置，也没有部分写入；提案全文仍保留。",
    attemptedText: proposalText,
    textLabel: "保留的完整提案",
    actions: Object.freeze(["replan", "copy", "reject"]),
  }),
  "no-effect": Object.freeze({
    id: "no-effect",
    result: "no-effect",
    tone: "neutral",
    eyebrow: "没有写入",
    title: "当前内容已经是这个结果",
    description: "这次操作没有改变正文或提案，也没有需要恢复的草稿。",
    attemptedText: null,
    actions: Object.freeze([]),
  }),
  "recovery-draft": Object.freeze({
    id: "recovery-draft",
    result: "recovery",
    artifactKind: "Recovery Draft",
    tone: "attention",
    eyebrow: "上次编辑未写入 · Recovery Draft",
    title: "这段文字还没有保存到正文",
    description:
      "StoryOS 已完成对账，没有找到已提交的结果，也没有自动重放。完整草稿仍在这里。",
    attemptedText: recoveryAttempt,
    textLabel: "完整恢复草稿",
    actions: Object.freeze(["retry", "copy", "discard"]),
  }),
  "proposal-recovery-conflict": Object.freeze({
    id: "proposal-recovery-conflict",
    result: "recovery",
    artifactKind: "Proposal Recovery Conflict",
    tone: "attention",
    eyebrow: "恢复需要决定 · Proposal Recovery Conflict",
    title: "无法确认续写暂停前的最后位置",
    description:
      "StoryOS 没有自动恢复、合并或接受任何内容。完整提案仍保留，需要基于当前正文重新规划或撤回。",
    attemptedText: proposalText,
    textLabel: "保留的完整提案",
    actions: Object.freeze(["replan", "copy", "withdraw"]),
  }),
});

export function getUxScenario(id) {
  return scenarios[id] ?? scenarios.proposal;
}

export function resolveUxAction({
  scenarioId,
  actionId,
  attemptedText,
}) {
  const scenario = getUxScenario(scenarioId);
  if (!scenario.actions.includes(actionId)) {
    throw new Error(`Action ${actionId} is not available for ${scenario.id}`);
  }

  const preservedText = attemptedText ?? scenario.attemptedText ?? null;
  const action = UX_ACTIONS[actionId];
  return {
    actionId,
    preservedText,
    nextScenarioId: action.nextScenarioId ?? scenario.id,
    nextPhase: action.nextPhase ?? "review",
  };
}

export function countCharacters(text) {
  return Array.from(text ?? "").length;
}

export function preservedTextToParagraphs(text, blockIdPrefix) {
  return text.split("\n").map((line, index) => ({
    type: "paragraph",
    attrs: { blockId: `${blockIdPrefix}-${index + 1}` },
    ...(line ? { content: [{ type: "text", text: line }] } : {}),
  }));
}
