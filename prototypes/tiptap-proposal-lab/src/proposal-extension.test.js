import assert from "node:assert/strict";
import test from "node:test";
import { Schema } from "@tiptap/pm/model";
import { StepMap } from "@tiptap/pm/transform";
import { classifyTransaction } from "./proposal-extension.js";

const schema = new Schema({
  nodes: {
    doc: { content: "paragraph+" },
    paragraph: {
      content: "text*",
      attrs: { blockId: { default: null } },
    },
    text: {},
  },
});

const doc = schema.node("doc", null, [
  schema.node("paragraph", { blockId: "authority-before" }, schema.text("甲")),
  schema.node("paragraph", { blockId: "proposal-a" }, schema.text("乙")),
  schema.node("paragraph", { blockId: "proposal-b" }, schema.text("丙")),
  schema.node("paragraph", { blockId: "authority-after" }, schema.text("丁")),
]);

function insertionAt(position) {
  return {
    steps: [{ getMap: () => new StepMap([position, 0, 1]) }],
  };
}

test("keeps the outer block Proposal boundaries authoritative", () => {
  const firstProposalFrom = doc.child(0).nodeSize;
  const lastProposalTo =
    firstProposalFrom + doc.child(1).nodeSize + doc.child(2).nodeSize;

  assert.equal(
    classifyTransaction(
      insertionAt(firstProposalFrom),
      doc,
      ["proposal-a", "proposal-b"],
    ),
    "authority",
  );
  assert.equal(
    classifyTransaction(
      insertionAt(lastProposalTo),
      doc,
      ["proposal-a", "proposal-b"],
    ),
    "authority",
  );
});

test("keeps an internal seam between Proposal blocks in Proposal ownership", () => {
  const internalSeam = doc.child(0).nodeSize + doc.child(1).nodeSize;
  assert.equal(
    classifyTransaction(
      insertionAt(internalSeam),
      doc,
      ["proposal-a", "proposal-b"],
    ),
    "proposal",
  );
});
