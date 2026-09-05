import { performance } from "node:perf_hooks";

import { expect, it } from "vitest";

import type {
  AuthorEditPrimitive,
  ManuscriptBlock,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyAuthorEditPrimitive } from "../../src/author-edit-primitive.ts";

const INITIAL_BLOCKS = 120;
const OUTPUT_BLOCKS = 121;

function blockId(index: number): string {
  return `018f0000-0000-7001-8000-${index.toString(16).padStart(12, "0")}`;
}

function cloneBlocks(blocks: readonly ManuscriptBlock[]): ManuscriptBlock[] {
  return blocks.map((block) => ({ ...block }));
}

function representativeChapter(): ManuscriptBlock[] {
  return Array.from({ length: INITIAL_BLOCKS }, (_, index) => ({
    manuscript_block_id: blockId(index),
    block_kind: "paragraph" as const,
    text: `Block ${index} representative prose.`,
  }));
}

function representativePrimitives(blocks: readonly ManuscriptBlock[]): AuthorEditPrimitive[] {
  const primitives: AuthorEditPrimitive[] = [];
  const liveIds = blocks.map((block) => block.manuscript_block_id);
  while (liveIds.length > 1) {
    primitives.push({
      kind: "join_blocks",
      left_manuscript_block_id: liveIds[0]!,
      right_manuscript_block_id: liveIds[1]!,
    });
    liveIds.splice(1, 1);
  }
  primitives.push({
    kind: "replace_block_selection",
    manuscript_block_id: liveIds[0]!,
    from: 0,
    to: 1,
    text: "X",
  });
  for (let index = 0; index < OUTPUT_BLOCKS - 1; index += 1) {
    const newId = blockId(1000 + index);
    primitives.push({
      kind: "split_block",
      manuscript_block_id: liveIds.at(-1)!,
      offset: 1,
      new_manuscript_block_id: newId,
    });
    liveIds.push(newId);
  }
  return primitives;
}

function applyWithPerPrimitiveCopy(
  blocks: readonly ManuscriptBlock[],
  primitives: readonly AuthorEditPrimitive[],
): ManuscriptBlock[] {
  let working = cloneBlocks(blocks);
  for (const primitive of primitives) {
    working = cloneBlocks(working);
    applyAuthorEditPrimitive(working, primitive);
  }
  return working;
}

function applyOnOwnedArray(
  blocks: readonly ManuscriptBlock[],
  primitives: readonly AuthorEditPrimitive[],
): ManuscriptBlock[] {
  const working = cloneBlocks(blocks);
  for (const primitive of primitives) {
    applyAuthorEditPrimitive(working, primitive);
  }
  return working;
}

function measure(rounds: number, apply: () => ManuscriptBlock[]): [number, ManuscriptBlock[]] {
  let result: ManuscriptBlock[] = [];
  let best = Number.POSITIVE_INFINITY;
  for (let sample = 0; sample < 7; sample += 1) {
    const start = performance.now();
    for (let round = 0; round < rounds; round += 1) {
      result = apply();
    }
    best = Math.min(best, (performance.now() - start) / rounds);
  }
  return [best, result];
}

it("applies a representative contiguous replacement on one owned Block array", () => {
  const chapter = representativeChapter();
  const primitives = representativePrimitives(chapter);
  expect(primitives).toHaveLength(INITIAL_BLOCKS + OUTPUT_BLOCKS - 1);

  const streaming = applyOnOwnedArray(chapter, primitives);
  const copied = applyWithPerPrimitiveCopy(chapter, primitives);
  expect(streaming).toEqual(copied);
  expect(streaming).toHaveLength(OUTPUT_BLOCKS);

  const [ownedMs, ownedResult] = measure(3, () => applyOnOwnedArray(chapter, primitives));
  const [copiedMs, copiedResult] = measure(3, () => applyWithPerPrimitiveCopy(chapter, primitives));
  expect(ownedResult).toEqual(copiedResult);
  console.log(
    `B=${INITIAL_BLOCKS} R=${INITIAL_BLOCKS} N=${OUTPUT_BLOCKS} P=${primitives.length}`
      + ` owned=${ownedMs.toFixed(3)}ms copied=${copiedMs.toFixed(3)}ms`,
  );
});
