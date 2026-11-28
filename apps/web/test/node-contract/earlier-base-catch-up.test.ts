import assert from "node:assert/strict";
import { test } from "vitest";

import { blocksOnCurrent, stepsOnCurrent } from "../../src/earlier-base-catch-up.ts";
import type { ManuscriptParagraph } from "../../src/manuscript-doc.ts";

const id = (suffix: string) => `018f0000-0000-7001-8000-000000000${suffix}`;
const [A, B, C, ADDED, MINTED, EMPTY] = [id("e01"), id("e02"), id("e03"), id("e04"), id("e05"), id("e06")];
/** A Block ID of a list of many blocks. */
const many = (index: number) => `018f0000-0000-7001-8000-0000000e${index.toString(16).padStart(4, "0")}`;
const block = (blockId: string, text: string, kind: "paragraph" | "heading" = "paragraph"): ManuscriptParagraph =>
  ({ manuscript_block_id: blockId, text, block_kind: kind });
const mint = () => MINTED;

/** The blocks after the last step of `stepsOnCurrent`, and the kind of each step. */
function applySteps(current: ManuscriptParagraph[], result: ManuscriptParagraph[]) {
  const steps = stepsOnCurrent(current, result);
  assert.ok(steps !== undefined);
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

test("a block that the tab added keeps its Block ID, its kind, and its place, also when it is empty", () => {
  const current = [block(A, "雨夜，风起。"), block(B, "渡口。")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。")],
    shown: [block(A, "雨夜。"), block(ADDED, "船来了。", "heading"), block(EMPTY, ""), block(B, "渡口。")] },
  current, mint);

  assert.deepEqual(result,
    [block(A, "雨夜，风起。"), block(ADDED, "船来了。", "heading"), block(EMPTY, ""), block(B, "渡口。")]);
  assert.deepEqual(applySteps(current, result).blocks, result);
});

test("a block that the tab emptied and the other tab deleted does not go in", () => {
  assert.deepEqual(blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。")],
    shown: [block(A, "雨夜。"), block(B, "")] }, [block(A, "雨夜，风起。")], mint), [block(A, "雨夜，风起。")]);
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

test("a block that the tab moved keeps its move and the text of the other tab", () => {
  const current = [block(A, "雨夜。"), block(B, "渡口，风起。"), block(C, "天亮了。")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。"), block(C, "天亮了。")],
    shown: [block(B, "渡口。"), block(A, "雨夜。"), block(C, "天亮了。")] }, current, mint);

  assert.deepEqual(result, [block(B, "渡口，风起。"), block(A, "雨夜。"), block(C, "天亮了。")]);
  assert.deepEqual(applySteps(current, result), { kinds: ["move_block"], blocks: result });
});

test("a missing block at the start goes in before the first block, and each block keeps its Block ID", () => {
  const current = [block(B, "渡口。", "heading")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。", "heading")],
    shown: [block(A, "雨夜，船来了。"), block(B, "渡口。", "heading")] }, current, mint);

  assert.deepEqual(result, [block(MINTED, "雨夜，船来了。"), block(B, "渡口。", "heading")]);
  assert.deepEqual(applySteps(current, result),
    { kinds: ["split_block", "move_block", "replace_block_selection"], blocks: result });
});

test("a join of the tab applies when the other tab moved the joined block to the start", () => {
  const current = [block(B, "渡口。"), block(A, "雨夜。"), block(C, "天亮了。")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。"), block(C, "天亮了。")],
    shown: [block(A, "雨夜。渡口。"), block(C, "天亮了。")] }, current, mint);

  assert.deepEqual(result, [block(A, "雨夜。渡口。"), block(C, "天亮了。")]);
  assert.deepEqual(applySteps(current, result), { kinds: ["move_block", "replace_block_selection", "move_block",
    "replace_block_selection", "join_blocks"], blocks: result });
});

test("a block that the other tab deleted replaces the only current block when the tab joined into it", () => {
  const current = [block(A, "雨夜。")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。")],
    shown: [block(B, "渡口。雨夜。")] }, current, mint);

  assert.deepEqual(result, [block(MINTED, "渡口。雨夜。")]);
  assert.deepEqual(applySteps(current, result), { kinds: ["split_block", "move_block", "replace_block_selection",
    "replace_block_selection", "join_blocks"], blocks: result });
});

// The tab joined the empty block into the first block, and the other tab joined the first block into the empty block.
test("the result keeps the text of the tab when the two tabs joined the blocks in each direction", () => {
  const current = [block(B, "")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "")], shown: [block(A, "雨夜。")] }, current,
    mint);

  assert.deepEqual(result, [block(MINTED, "雨夜。")]);
  assert.deepEqual(applySteps(current, result).blocks, result);
});

test("a paste of many paragraphs is one replacement", () => {
  const pasted = Array.from({ length: 121 }, (_, index) => block(many(index), `第${index}段。`));
  const current = [block(A, "雨夜。"), block(B, "渡口，风起。")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。")],
    shown: [block(A, "雨夜。船来了。"), ...pasted, block(B, "渡口。")] }, current, mint);

  assert.deepEqual(result, [block(A, "雨夜。船来了。"), ...pasted, block(B, "渡口，风起。")]);
  assert.deepEqual(applySteps(current, result), { kinds: ["contiguous_replacement"], blocks: result });
});

test("a paste of many paragraphs is one replacement also when the other tab deleted the block of the paste", () => {
  const pasted = Array.from({ length: 121 }, (_, index) => block(many(index), `第${index}段。`));
  const current = [block(B, "渡口。")];
  const result = blocksOnCurrent({ base: [block(A, "雨夜。"), block(B, "渡口。")],
    shown: [block(A, "雨夜。船来了。"), ...pasted, block(B, "渡口。")] }, current, mint);

  assert.deepEqual(result, [block(MINTED, "雨夜。船来了。"), ...pasted, block(B, "渡口。")]);
  assert.deepEqual(applySteps(current, result),
    { kinds: ["split_block", "move_block", "contiguous_replacement"], blocks: result });
});

test("edits that are more than one Author Edit can have give no steps", () => {
  const current = Array.from({ length: 241 }, (_, index) => block(many(index), "雨夜。"));

  assert.equal(stepsOnCurrent(current, current.map((item) => ({ ...item, text: "雨夜，风起。" })))?.length, undefined);
});
