import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  STATISTICS_COUNTING_PROFILE,
  countStoredText,
} from "../support/statistics-unicode-profile.ts";

describe("the Unicode 16.0.0 statistics profile", () => {
  it("matches the pinned Rust/TypeScript golden cases", async () => {
    const goldenBytes = await readFile(new URL(
      "../../../../crates/storyos-core/src/statistics_unicode_16_0_0_v1.golden.json",
      import.meta.url,
    ));
    const golden: {
      profile: string;
      cases: { name: string; text: string; word_count: number; character_count: number }[];
    } = JSON.parse(goldenBytes.toString("utf8"));
    expect(golden.profile).toBe(STATISTICS_COUNTING_PROFILE);
    expect(STATISTICS_COUNTING_PROFILE).toBe("storyos.statistics.unicode-16.0.0.v1");
    for (const testCase of golden.cases) {
      expect(countStoredText(testCase.text), testCase.name).toEqual({
        word_count: testCase.word_count,
        character_count: testCase.character_count,
      });
    }
  });
});
