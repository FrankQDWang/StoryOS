import type {
  AuthorEditPrimitive,
  ManuscriptBlock,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

function replaceOnText(body: string, from: number, to: number, text: string): string {
  if (!Number.isSafeInteger(from)
    || !Number.isSafeInteger(to)
    || from < 0
    || to < from
    || to > body.length
    || typeof text !== "string") {
    throw new Error("Local Edit Journal reconstruction failed");
  }
  return `${body.slice(0, from)}${text}${body.slice(to)}`;
}

/** Apply one Author Edit primitive to a caller-owned Block array.
 *
 * The caller must isolate the array before the first primitive. This helper
 * validates the primitive before it mutates that array and does not copy the
 * complete array.
 */
export function applyAuthorEditPrimitive(
  blocks: ManuscriptBlock[],
  primitive: AuthorEditPrimitive,
): void {
  if (primitive.kind === "replace_selection") {
    if (blocks.length !== 1 || blocks[0] === undefined) {
      throw new Error("Local Edit Journal reconstruction failed");
    }
    blocks[0].text = replaceOnText(blocks[0].text, primitive.from, primitive.to, primitive.text);
    return;
  }
  if (primitive.kind === "replace_block_selection") {
    const block = blocks.find((item) => item.manuscript_block_id === primitive.manuscript_block_id);
    if (block === undefined) {
      throw new Error("Local Edit Journal reconstruction failed");
    }
    block.text = replaceOnText(block.text, primitive.from, primitive.to, primitive.text);
    return;
  }
  if (primitive.kind === "split_block") {
    const index = blocks.findIndex((item) => item.manuscript_block_id === primitive.manuscript_block_id);
    if (index < 0
      || blocks.some((item) => item.manuscript_block_id === primitive.new_manuscript_block_id)) {
      throw new Error("Local Edit Journal reconstruction failed");
    }
    const block = blocks[index]!;
    if (primitive.offset < 0 || primitive.offset > block.text.length) {
      throw new Error("Local Edit Journal reconstruction failed");
    }
    const right = block.text.slice(primitive.offset);
    block.text = block.text.slice(0, primitive.offset);
    blocks.splice(index + 1, 0, {
      manuscript_block_id: primitive.new_manuscript_block_id,
      block_kind: "paragraph",
      text: right,
    });
    return;
  }
  if (primitive.kind === "join_blocks") {
    const leftIndex = blocks.findIndex((item) =>
      item.manuscript_block_id === primitive.left_manuscript_block_id);
    const rightIndex = blocks.findIndex((item) =>
      item.manuscript_block_id === primitive.right_manuscript_block_id);
    if (leftIndex < 0 || rightIndex !== leftIndex + 1) {
      throw new Error("Local Edit Journal reconstruction failed");
    }
    const left = blocks[leftIndex]!;
    const right = blocks[rightIndex]!;
    left.text += right.text;
    blocks.splice(rightIndex, 1);
    return;
  }
  if (primitive.kind === "move_block") {
    if (primitive.to_index < 0 || primitive.to_index >= blocks.length) {
      throw new Error("Local Edit Journal reconstruction failed");
    }
    const fromIndex = blocks.findIndex((item) =>
      item.manuscript_block_id === primitive.manuscript_block_id);
    if (fromIndex < 0) {
      throw new Error("Local Edit Journal reconstruction failed");
    }
    const [block] = blocks.splice(fromIndex, 1);
    if (block === undefined) {
      throw new Error("Local Edit Journal reconstruction failed");
    }
    blocks.splice(primitive.to_index, 0, block);
    return;
  }
  if (primitive.kind === "retype_block") {
    const block = blocks.find((item) => item.manuscript_block_id === primitive.manuscript_block_id);
    if (block === undefined) {
      throw new Error("Local Edit Journal reconstruction failed");
    }
    block.block_kind = primitive.block_kind;
    return;
  }
  throw new Error("Local Edit Journal reconstruction failed");
}
