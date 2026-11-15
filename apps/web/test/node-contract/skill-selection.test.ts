import assert from "node:assert/strict";
import { test } from "vitest";

import { filterSkills, skillTrigger } from "../../src/skill-selection.ts";

const skill = (name: string) => ({ reference: { name, package_digest: `sha256:${"a".repeat(64)}` },
  description: `The ${name} Skill.` });
const SKILLS = [skill("dialogue-style-guide"), skill("internal-comms"), skill("scene-outline")];

test("the $ picker filters the Skills by a part of the name and leaves out each selected Skill", () => {
  const names = (query: string, selected: string[] = []) =>
    filterSkills(SKILLS, query, selected).map((entry) => entry.reference.name);

  assert.deepEqual({
    empty: names(""),
    part: names("comm"),
    inner: names("e-o"),
    upper: names("DIA"),
    selected: names("", ["internal-comms"]),
    none: names("plot"),
  }, {
    empty: ["dialogue-style-guide", "internal-comms", "scene-outline"],
    part: ["internal-comms"],
    inner: ["scene-outline"],
    upper: ["dialogue-style-guide"],
    selected: ["dialogue-style-guide", "scene-outline"],
    none: [],
  });
});

test("a trigger is a $ or a ￥ at the start of the text or after white space, up to the caret", () => {
  const at = (text: string, caret = text.length) => skillTrigger(text, caret);

  assert.deepEqual([
    at("$"), at("$dia"), at("Help ￥int"), at("Line\n$s"), at("$dia rest", 4),
    at("US$5"), at("$dia "), at("Help"), at("$dia", 0),
  ], [
    { start: 0, query: "" }, { start: 0, query: "dia" }, { start: 5, query: "int" }, { start: 5, query: "s" },
    { start: 0, query: "dia" },
    undefined, undefined, undefined, undefined,
  ]);
});
