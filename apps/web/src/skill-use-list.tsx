import type {
  GetAgentRunResponse, SkillUseInspect,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { settledRun } from "./assistant-run-outcome.ts";

function skillUseLine(use: SkillUseInspect): string {
  switch (use.provenance) {
    case "author_selected":
      return `使用了你选择的技能「${use.name}」`;
    case "agent_selected":
      return `使用了技能「${use.name}」：${use.reason}`;
  }
}

/**
 * The Skills that a settled Run used, as plain lines in the order of the Run read (ADR 0089 section 8). The panel
 * shows the list for the current turn and for each history turn, from the Run read that it holds for the turn. A Run
 * with no Skill shows no list. The list has no control and shows no `SKILL.md` (ADR 0040).
 */
export function SkillUseList({ run }: { run: GetAgentRunResponse }) {
  const uses = settledRun(run) ? run.skill_uses ?? [] : [];
  return uses.length === 0 ? null
    : <ul className="assistant-skill-uses" aria-label="本次使用的技能" data-assistant-skill-uses="">
      {uses.map((use, index) => <li key={index}>{skillUseLine(use)}</li>)}
    </ul>;
}
