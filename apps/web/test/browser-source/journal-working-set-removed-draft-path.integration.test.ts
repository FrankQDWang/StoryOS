import { expect, it } from "vitest";
import { digestApplyAuthorEdit }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { freezeOneIntentSubmission } from "../../src/author-edit-submission.ts";
import { openEditorWorkspace, appendAuthorEdit, submitOnePendingAuthorEdit }
  from "../../src/editor-session.ts";
import { collectEligibleJournalPayload } from "../../src/journal-payload-collection.ts";
import type { JournalIntentRecord, JournalSubmissionGroup } from "../../src/editor-types.ts";
import { createJournalUuid, readJournalSnapshot } from "../../src/local-edit-journal.ts";
import { appliedAuthorEditTransport } from "./journal-working-set-transport.ts";
import {
  OWNER, PROJECT, createBrowserScenario, deleteJournal, requireEditorReady, requestResult,
} from "./scenario.ts";

it("retires a stored record and group of a removed Draft path with the collected prefix", async () => {
  const scenario = createBrowserScenario();
  let body = "Base";
  const { fetchImpl } = appliedAuthorEditTransport(scenario, () => body);
  await deleteJournal(scenario.journalName);
  const workspace = await openEditorWorkspace({ baseUrl: location.origin,
    project: scenario.project, chapter: scenario.chapter, profile: scenario.profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  requireEditorReady(workspace);
  const retrySource = { kind: "draft_retry", source_draft_kind: "refused_edit",
    source_draft_id: "018f0000-0000-7001-8000-000000000090",
    source_current_draft_revision_id: "018f0000-0000-7001-8000-000000000090",
    source_draft_payload_digest: "a".repeat(64), expected_source_draft_closure: "open",
    selected_payload_range: { kind: "exact_structured_range",
      coordinate_profile: "storyos.draft-replacement.block-utf16.v1", from: { block_index: 0, offset: 0 },
      to: { block_index: 0, offset: 1 }, slice_digest: "b".repeat(64) } } as const;
  let createdAt = Date.UTC(2026, 7, 15, 8);
  let undoGroupId = createJournalUuid();
  const append = (text: string) => appendAuthorEdit(workspace, { from: body.length, to: body.length, text,
    resultingBody: body + text, inputOrigin: "typing", undoGroupId, createdAt: new Date(createdAt++).toISOString() });
  // An earlier client stored a Draft retry. The Server never applied its text, so the body stays.
  const storeDraftRetry = async (group?: JournalSubmissionGroup) => {
    const transaction = workspace.database.transaction(["intents", "submission_groups"], "readwrite");
    const records = await requestResult(transaction.objectStore("intents").getAll()) as JournalIntentRecord[];
    transaction.objectStore("intents").put({ ...records.at(-1)!, retry_source: retrySource });
    if (group) {
      const request = { ...group.frozen_request_body,
        retry_source: retrySource } as JournalSubmissionGroup["frozen_request_body"];
      transaction.objectStore("submission_groups").put({ ...group, frozen_request_body: request,
        frozen_request_digest: await digestApplyAuthorEdit(request, crypto) });
    }
    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onabort = () => reject(transaction.error);
    });
  };
  try {
    await append("x");
    await storeDraftRetry();
    for (const [index, size] of [238, 0, ...Array.from({ length: 9 }, () => 240)].entries()) {
      undoGroupId = createJournalUuid();
      if (index === 1) {
        await append("x");
        await storeDraftRetry(await freezeOneIntentSubmission(workspace));
        continue;
      }
      for (let offset = 0; offset < size; offset++) {
        await append("a");
        body += "a";
      }
      await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto });
      await collectEligibleJournalPayload(workspace);
    }
    const saved = await readJournalSnapshot(workspace);
    expect(saved.records).toHaveLength(2400);
    expect(saved.groups).toHaveLength(11);

    expect(await append("+")).toMatchObject({ body: `${body}+`, unsettled_intent_count: 1 });
    const continued = await readJournalSnapshot(workspace);
    expect(continued.workingBoundary?.last_sequence).toBe(2400);
    expect(continued.records).toHaveLength(1);
    expect(continued.payloadChains).toHaveLength(1);
    expect(continued.groups).toEqual([]);
  } finally {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 180000);
