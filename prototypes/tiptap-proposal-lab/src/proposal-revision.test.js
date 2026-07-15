import assert from "node:assert/strict";
import test from "node:test";
import { Schema } from "@tiptap/pm/model";
import { EditorState } from "@tiptap/pm/state";
import {
  captureRejectedProposalRevision,
  createReopenProposalTransaction,
} from "./proposal-revision.js";

const schema = new Schema({
  nodes: {
    doc: { content: "block*" },
    paragraph: {
      group: "block",
      content: "inline*",
      attrs: { blockId: { default: null } },
      toDOM: (node) => ["p", { "data-block-id": node.attrs.blockId }, 0],
      parseDOM: [{ tag: "p" }],
    },
    text: { group: "inline" },
  },
});

function paragraph(blockId, text) {
  return schema.nodes.paragraph.create(
    { blockId },
    text ? schema.text(text) : undefined,
  );
}

function documentWith(...blocks) {
  return schema.nodes.doc.create(null, blocks);
}

function capture(doc, blockIds = ["proposal-1", "proposal-2"]) {
  return captureRejectedProposalRevision({
    doc,
    proposalId: "proposal-rain-night-continuation",
    proposalRevision: 7,
    chapterId: "chapter-12",
    blockIds,
  });
}

test("captures only the rejected candidate and its adjacent placement anchors", () => {
  const result = capture(
    documentWith(
      paragraph("authority-1", "正文一"),
      paragraph("proposal-1", "提案一"),
      paragraph("proposal-2", "提案二"),
      paragraph("authority-2", "正文二"),
    ),
  );

  assert.equal(result.ok, true);
  assert.deepEqual(result.revision.placement, {
    previousBlockId: "authority-1",
    nextBlockId: "authority-2",
  });
  assert.deepEqual(
    result.revision.blocks.map((block) => block.attrs.blockId),
    ["proposal-1", "proposal-2"],
  );
});

test("reopens the candidate without overwriting later authoritative edits", () => {
  const original = documentWith(
    paragraph("authority-1", "原正文"),
    paragraph("proposal-1", "提案一"),
    paragraph("proposal-2", "提案二"),
  );
  const captured = capture(original);
  assert.equal(captured.ok, true);

  const editedAuthority = documentWith(paragraph("authority-1", "作者后来修改的正文"));
  const state = EditorState.create({ schema, doc: editedAuthority });
  const reopened = createReopenProposalTransaction(state, captured.revision);

  assert.equal(reopened.ok, true);
  const nextState = state.apply(reopened.transaction);
  assert.deepEqual(nextState.doc.toJSON(), {
    type: "doc",
    content: [
      paragraph("authority-1", "作者后来修改的正文").toJSON(),
      paragraph("proposal-1", "提案一").toJSON(),
      paragraph("proposal-2", "提案二").toJSON(),
    ],
  });
});

test("reports a conflict when an end placement anchor has moved", () => {
  const captured = capture(
    documentWith(
      paragraph("authority-1", "正文"),
      paragraph("proposal-1", "提案一"),
      paragraph("proposal-2", "提案二"),
    ),
  );
  assert.equal(captured.ok, true);

  const state = EditorState.create({
    schema,
    doc: documentWith(
      paragraph("authority-1", "正文"),
      paragraph("authority-2", "作者新增正文"),
    ),
  });

  assert.deepEqual(
    createReopenProposalTransaction(state, captured.revision),
    { ok: false, reason: "end_anchor_moved" },
  );
});

test("reports a conflict instead of guessing when an anchor is missing", () => {
  const captured = capture(
    documentWith(
      paragraph("authority-1", "正文一"),
      paragraph("proposal-1", "提案一"),
      paragraph("proposal-2", "提案二"),
      paragraph("authority-2", "正文二"),
    ),
  );
  assert.equal(captured.ok, true);

  const state = EditorState.create({
    schema,
    doc: documentWith(paragraph("authority-2", "正文二")),
  });

  assert.deepEqual(
    createReopenProposalTransaction(state, captured.revision),
    { ok: false, reason: "previous_anchor_missing" },
  );
});
