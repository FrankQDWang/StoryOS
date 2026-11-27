import assert from "node:assert/strict";
import { test } from "vitest";

import { blocksOnCurrent, stepsOnCurrent } from "../../src/earlier-base-catch-up.ts";
import type { ManuscriptParagraph } from "../../src/manuscript-doc.ts";

const id = (suffix: string) => `018f0000-0000-7001-8000-000000000${suffix}`;
const [A, B, C, ADDED, MINTED] = [id("e01"), id("e02"), id("e03"), id("e04"), id("e05")];
const block = (blockId: string, text: string, kind: "paragraph" | "heading" = "paragraph"): ManuscriptParagraph =>
  ({ manuscript_block_id: blockId, text, block_kind: kind });
const mint = () => MINTED;

/** The blocks after each step of `stepsOnCurrent`, and the kind of each step. */
function applySteps(current: ManuscriptParagraph[], result: ManuscriptParagraph[]) {
  const steps = stepsOnCurrent(current, result);
  return { kinds: steps.map((edit) => edit.kind), blocks: steps.at(-1)?.resultingBlocks };
}

test("a block that only the other tab changed keeps its text, and a block of the tab replaces its current text", () => {
  const base = [block(A, "雨夜。"), block(B, "渡口。")];
  const result = blocksOnCurrent({ base, shown: [block(A, "雨夜。"), block(B, "渡口无人。")] },
    [block(A, "雨夜，风起。"), block(B, "渡口。")], mint);

  assert.deepEqual(result, [block(A, "雨夜，风起。"), block(B, "渡口无人。")]);
  assert.deepEqual(applySteps([block(A, "雨夜，风起。"), block(B, "渡口。")], result),
    { kinds: ["replace_block_selection"], blocks: result });
});

test("the later edit of the same block wins", () => {
  assert.deepEqual(blocksOnCurrent({ base: [block(A, "雨夜。")], shown: [block(A, "雨夜，船来了。")] },
    [block(A, "雨夜，灯灭了。")], mint), [block(A, "雨夜，船来了。")]);
});

test("a changed block that the current Chapter does not have stays as a paragraph at its place", () => {
  const current = [block(A, "雨夜。"), block(C, "天亮了。")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。", "heading"), block(C, "天亮了。")],
    shown: [block(A, "雨夜。"), block(B, "渡口无人。", "heading"), block(C, "天亮了。")] }, current, mint);

  assert.deepEqual(result, [block(A, "雨夜。"), block(MINTED, "渡口无人。"), block(C, "天亮了。")]);
  assert.deepEqual(applySteps(current, result),
    { kinds: ["split_block", "replace_block_selection"], blocks: result });
});

test("a block that the tab added keeps its id, and an empty added block does not go in", () => {
  assert.deepEqual(blocksOnCurrent({ base: [block(A, "雨夜。")],
    shown: [block(A, "雨夜。"), block(ADDED, "船来了。"), block(B, "")] }, [block(A, "雨夜，风起。")], mint),
  [block(A, "雨夜，风起。"), block(ADDED, "船来了。")]);
});

test("a block that the tab deleted leaves only when the other tab did not change it", () => {
  const text = { base: [block(A, "雨夜。"), block(B, "渡口。")], shown: [block(A, "雨夜。")] };
  const unchanged = [block(A, "雨夜，风起。"), block(B, "渡口。")];

  assert.deepEqual(blocksOnCurrent(text, unchanged, mint), [block(A, "雨夜，风起。")]);
  assert.deepEqual(applySteps(unchanged, [block(A, "雨夜，风起。")]), {
    kinds: ["replace_block_selection", "join_blocks"],
    blocks: [block(A, "雨夜，风起。")] });
  assert.deepEqual(blocksOnCurrent(text, [block(A, "雨夜。"), block(B, "渡口无人。")], mint),
    [block(A, "雨夜。"), block(B, "渡口无人。")]);
});

test("a missing block at the start takes the first block, and the first text goes to a new block after it", () => {
  const current = [block(B, "渡口。", "heading")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。", "heading")],
    shown: [block(A, "雨夜，船来了。"), block(B, "渡口。", "heading")] }, current, mint);

  assert.deepEqual(result, [block(B, "雨夜，船来了。"), block(MINTED, "渡口。", "heading")]);
  assert.deepEqual(applySteps(current, result), { kinds: ["replace_block_selection",
    "retype_block", "split_block", "replace_block_selection",
    "retype_block"],
  blocks: result });
});

test("a removal of the first block has no Author Edit form", () => {
  assert.throws(() => stepsOnCurrent([block(A, "雨夜。"), block(B, "渡口。")], [block(B, "渡口。")]),
    /no Author Edit form/);
});
