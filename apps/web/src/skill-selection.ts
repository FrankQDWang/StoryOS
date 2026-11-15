import { StoryOSProtocolError } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { SkillCatalogEntry } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

/** The most Skills that one message selects (ADR 0089 section 9). */
export const SKILL_SELECTION_LIMIT = 8;

/** The sentence when the deployment no longer offers a selected Skill (ADR 0089 section 7). */
export const SKILL_UNAVAILABLE_SENTENCE = "所选技能现在不可用，消息没有发送。请重新选择技能，或直接发送。";

/** `￥` is the character that a Chinese input method types for the `$` key. */
const TRIGGERS = new Set(["$", "￥"]);

/**
 * The trigger of the `$` picker that ends at the caret: a trigger character at the start of the text or
 * after white space, and the characters after it up to the caret, with no white space.
 */
export function skillTrigger(text: string, caret: number): { start: number; query: string } | undefined {
  for (let start = caret - 1; start >= 0; start -= 1) {
    const character = text[start]!;
    if (/\s/.test(character)) return undefined;
    if (TRIGGERS.has(character) && (start === 0 || /\s/.test(text[start - 1]!))) {
      return { start, query: text.slice(start + 1, caret) };
    }
  }
  return undefined;
}

/** The Skills whose name has the query as a part, without the Skills that the message already selects. */
export function filterSkills(skills: readonly SkillCatalogEntry[], query: string,
  selected: readonly string[]): SkillCatalogEntry[] {
  const part = query.toLowerCase();
  return skills.filter((skill) => skill.reference.name.includes(part) && !selected.includes(skill.reference.name));
}

/** True when `createAgentRun` refused because the deployment does not offer a selected Skill. */
export function skillSelectionUnavailable(error: unknown): boolean {
  if (!(error instanceof StoryOSProtocolError) || error.status !== 422 || typeof error.responseBody !== "string") {
    return false;
  }
  try {
    return (JSON.parse(error.responseBody) as { code?: string }).code === "skill_selection_unavailable";
  } catch {
    return false;
  }
}
