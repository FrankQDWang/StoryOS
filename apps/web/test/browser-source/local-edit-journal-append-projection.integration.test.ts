import { expect, it } from "vitest";

import { persistReplaceSelection } from "../../src/editor-session.ts";
import { rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import {
  FIRST_APPEND_EDIT,
  SECOND_APPEND_EDIT,
  openJournalAppendTestWorkspace,
  withDigestBudget,
} from "./local-edit-journal-append-fixture.ts";

it("returns the valid append projection without a second Journal reconstruction", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    await persistReplaceSelection(test.workspace, FIRST_APPEND_EDIT);
    test.workspace.cryptoImpl = withDigestBudget(crypto, 2);

    const projection = await persistReplaceSelection(test.workspace, SECOND_APPEND_EDIT);
    test.workspace.cryptoImpl = crypto;

    expect(projection).toEqual({
      body: "Base!?",
      blocks: [{
        ...test.workspace.session.base_snapshot.materialized_revision.blocks[0]!,
        text: "Base!?",
      }],
      save_state: "saving",
      unsettled_intent_count: 2,
      authoritative_revision_id:
        test.workspace.session.base_snapshot.authoritative_head_revision_id,
    });
    expect(await rebuildPendingProjection(test.workspace)).toEqual(projection);
  } finally {
    test.workspace.cryptoImpl = crypto;
    await test.close();
  }
});
