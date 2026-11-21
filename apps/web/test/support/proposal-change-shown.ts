import type { ProposalChangeRun } from "../../src/proposal-change-runs.ts";

/** The texts of row 1 of the fixture table of S10-06: the target Block and the candidate. */
export const ROW_1 = {
  old: "他站在门口，深吸了一口气，终于把那句话说了出来：“妈，先借我两千。”母亲的眼中闪过一丝复杂的情绪，她没有问为什么，只是默默地转身进了里屋。他听见抽屉被拉开的声音，心中涌起一股难以言说的酸楚。这不是一次简单的借钱，而是他第一次向母亲承认自己的失败。",
  new: "他站在门口，把钥匙从左手换到右手，说：“妈，先借我两千。”母亲看了他一眼，没有问为什么，转身进了里屋。他听见抽屉被拉开，又听见她在里面翻了很久。他低头去看自己的鞋尖，鞋带松了一边。",
};

/** The candidate text with `【…】` around each mark and `▏` at each tick, as the fixture table of S10-06 shows it. */
export function shownCandidate(candidate: string, runs: readonly ProposalChangeRun[] | undefined): string {
  if (runs === undefined) return "no mark";
  return runs.map((run) => {
    const text = candidate.slice(run.new.from, run.new.to);
    if (!run.changed) return text;
    return text === "" ? "▏" : `【${text}】`;
  }).join("");
}
