import assert from "node:assert/strict";
import { test } from "vitest";

import { proposalChangeRuns } from "../../src/proposal-change-runs.ts";
import { ROW_1, shownCandidate } from "../support/proposal-change-shown.ts";

// The fixture table of S10-06 (#1766): the text of the target Block, the candidate text, and the shown candidate.
const FIXTURES: [number, string, string, string][] = [
  [1, ROW_1.old, ROW_1.new,
    "他站在门口，【把钥匙从左手换到右手，说】：“妈，先借我两千。”母亲【看了他一眼，】没有问为什么，▏转身进了里屋。他听见抽屉被拉开【，又听见她在里面翻了很久。他低头去看自己的鞋尖，鞋带松了一边】。"],
  [2, "他慢慢地走，轻轻地说。", "他走，说。", "他▏走，▏说。"],
  [3, "母亲把存折放在桌上。", "母亲把存折放在桌上，又往他那边推了推。", "母亲把存折放在桌上【，又往他那边推了推】。"],
  [4, "他看着她说话。", "他低头看着她，小声说话。", "他【低头】看着她【，小声】说话。"],
  [5, "她的声音很轻。", "她的声音很低。", "她的声音很【低】。"],
  [6, "雨越下越大，街上的行人纷纷加快了脚步。", "雨越下越大，街上的人都跑了起来。", "雨越下越大，街上的【人都跑了起来】。"],
  [7, "窗外的雨还在下，他坐在桌前，看着手机屏幕上的那条短信，心里五味杂陈。", "窗外的雨还在下。他坐在桌前，把那条短信又看了一遍。",
    "窗外的雨还在下【。】他坐在桌前，【把】那条短信【又看了一遍】。"],
  [8, "他先喝了口水，然后开口说话。", "他开口之前，先喝了口水。", "他【开口之前，】先喝了口水▏。"],
  [9, "然而，他没有回头。", "他没有回头。", "▏他没有回头。"],
  [10, "他走了走了。", "他走了。", "他走了▏。"],
  [11, "他笑了笑😀。", "他笑了笑😄。", "他笑了笑【😄】。"],
  [12, "他点了点头。", "他点了点头。门外有人喊他的名字，喊了两声，他没有应，只是把桌上的碗一只一只摞起来，端进了厨房。",
    "他点了点头。【门外有人喊他的名字，喊了两声，他没有应，只是把桌上的碗一只一只摞起来，端进了厨房。】"],
  [13, "甲丁甲乙，甲乙，，甲", "甲丙甲乙，甲丙", "甲【丙】甲乙，【甲丙】"],
  [14, "她笑了笑，没有说话。", "窗外的车一辆接一辆地过去，谁也没有再开口。", "no mark"],
  [15, "他说。", "他低声说了一句。", "no mark"],
  [16, "", "天亮的时候，他已经走了。", "no mark"],
  [17, "雨停了。", "雨停了。", "no mark"],
  [18, "雨停了。", "", "no mark"],
];

test.each(FIXTURES)("fixture row %i shows the marks of the table", (_row, old, candidate, shown) => {
  assert.equal(shownCandidate(candidate, proposalChangeRuns(old, candidate)), shown);
});

test("the four pairs of fixture row 1 have their old text and new text with their UTF-16 ranges", () => {
  const pairs = proposalChangeRuns(ROW_1.old, ROW_1.new)!.filter((run) => run.changed).map((run) => ({
    old: ROW_1.old.slice(run.old.from, run.old.to), oldRange: run.old,
    new: ROW_1.new.slice(run.new.from, run.new.to), newRange: run.new }));
  assert.deepEqual(pairs, [
    { old: "深吸了一口气，终于把那句话说了出来", oldRange: { from: 6, to: 23 },
      new: "把钥匙从左手换到右手，说", newRange: { from: 6, to: 18 } },
    { old: "的眼中闪过一丝复杂的情绪，她", oldRange: { from: 36, to: 50 },
      new: "看了他一眼，", newRange: { from: 31, to: 37 } },
    { old: "只是默默地", oldRange: { from: 57, to: 62 }, new: "", newRange: { from: 44, to: 44 } },
    { old: "的声音，心中涌起一股难以言说的酸楚。这不是一次简单的借钱，而是他第一次向母亲承认自己的失败",
      oldRange: { from: 77, to: 122 },
      new: "，又听见她在里面翻了很久。他低头去看自己的鞋尖，鞋带松了一边", newRange: { from: 59, to: 89 } },
  ]);
});

test("a code point outside the Basic Multilingual Plane keeps its surrogate pair in one range", () => {
  const runs = proposalChangeRuns("他笑了笑😀。", "他笑了笑😄。")!;
  assert.deepEqual(runs, [
    { changed: false, old: { from: 0, to: 4 }, new: { from: 0, to: 4 } },
    { changed: true, old: { from: 4, to: 6 }, new: { from: 4, to: 6 } },
    { changed: false, old: { from: 6, to: 7 }, new: { from: 6, to: 7 } },
  ]);
});

test("middle parts above the table limit give one changed run for the middle", () => {
  // 1,001 by 1,000 code points is more than 1,000,000 cells. The middle parts have common code points.
  const start = "开头。".repeat(100);
  const old = `${start}${"甲乙".repeat(500)}丙结尾。`;
  const candidate = `${start}${"乙甲".repeat(500)}结尾。`;
  assert.deepEqual(proposalChangeRuns(old, candidate), [
    { changed: false, old: { from: 0, to: 300 }, new: { from: 0, to: 300 } },
    { changed: true, old: { from: 300, to: 1301 }, new: { from: 300, to: 1300 } },
    { changed: false, old: { from: 1301, to: 1304 }, new: { from: 1300, to: 1303 } },
  ]);
});
