import { expect, it } from "vitest";

import { openEditorWorkspace, submitOnePendingAuthorEdit } from "../../src/editor-session.ts";
import { createEditorSessionWritingController, type EditorSessionWritingController }
  from "../../src/editor-session-writing.ts";
import type { EditorReadyState, JournalSubmissionGroup } from "../../src/editor-types.ts";
import { readJournalSnapshot } from "../../src/local-edit-journal.ts";
import {
  OWNER,
  PROJECT,
  SESSION,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  requireEditorReady,
  requestResult,
} from "./scenario.ts";

it.each([
  { label: "validated stale refusal", schema: "storyos.problem.v1", fenced: true, earlier: "none" },
  { label: "unrecognized Problem", schema: "unknown.problem.v1", fenced: false, earlier: "none" },
  { label: "an earlier challenge observation", schema: "storyos.problem.v1", fenced: true, earlier: "challenge" },
  { label: "an earlier admission observation", schema: "storyos.problem.v1", fenced: true, earlier: "admission" },
  { label: "an earlier terminal observation", schema: "storyos.problem.v1", fenced: true, earlier: "rejected" },
])("preserves input and outcome evidence after $label", async ({ schema, fenced, earlier }) => {
  const scenario = createBrowserScenario();
  let releaseOutcome!: () => void;
  let observeOutcome!: () => void;
  let releaseCommand!: () => void;
  let observeCommand!: () => void;
  const heldOutcome = new Promise<void>((resolve) => { releaseOutcome = resolve; });
  const outcomeStarted = new Promise<void>((resolve) => { observeOutcome = resolve; });
  const heldCommand = new Promise<void>((resolve) => { releaseCommand = resolve; });
  const commandStarted = new Promise<void>((resolve) => { observeCommand = resolve; });
  const counts = { commands: 0, outcomes: 0 };
  const expiresAt = "2026-08-13T08:05:00.000Z";
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: expiresAt,
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (path.endsWith("/manuscript/author-edits")) {
      counts.commands += 1;
      observeCommand();
      await heldCommand;
      return jsonResponse({ schema_id: schema, code: "editor_writer_stale",
        message: "The Editor Session is not the current writer." }, 412);
    }
    if (path.includes("/manuscript/author-edit-outcomes/")) {
      counts.outcomes += 1;
      if (earlier !== "none" && counts.outcomes === 1) {
        const observation = earlier === "admission"
          ? { observation_kind: "admission_committed", reconciliation_required: true,
            command_id: "018f0000-0000-7001-8000-000000000092",
            author_command_admission_id: "018f0000-0000-7001-8000-000000000093" }
          : { observation_kind: "challenge_issued", expires_at: expiresAt };
        return jsonResponse({ schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000091",
          project_scope: scenario.project.project_scope,
          outcome: earlier === "rejected"
            ? { outcome_kind: "rejected", reason: "challenge_expired_unconsumed" }
            : { outcome_kind: "still_unknown", observation } });
      }
      observeOutcome();
      await heldOutcome;
      return jsonResponse({ schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000091",
        project_scope: scenario.project.project_scope,
        outcome: { outcome_kind: "still_unknown",
          observation: { observation_kind: "challenge_issued", expires_at: expiresAt } } });
    }
    throw new Error(`unexpected request: ${path}`);
  };
  await deleteJournal(scenario.journalName);
  let workspace: EditorReadyState | undefined;
  let recoveryReader: EditorReadyState | undefined;
  let controller: EditorSessionWritingController | undefined;
  let submission: Promise<void> | undefined;
  const failures: unknown[] = [];
  try {
    const state = await openEditorWorkspace({ baseUrl: location.origin,
      project: scenario.project, chapter: scenario.chapter, profile: scenario.profile, fetchImpl });
    requireEditorReady(state);
    workspace = state;
    const oldPartition = structuredClone(state.partition);
    controller = createEditorSessionWritingController({ workspace, baseUrl: location.origin, fetchImpl,
      setTimeoutImpl: () => 1, clearTimeoutImpl: () => {},
      onFailure: (error) => { failures.push(error); } });
    controller.capture({ from: 4, to: 4, text: " retained", resultingBody: "Base retained" }, "typing");
    await controller.whenIdle();
    const retained = await readJournalSnapshot(workspace);
    submission = controller.flush();
    await commandStarted;
    let earlierGroups: JournalSubmissionGroup[] | undefined;
    if (earlier !== "none") {
      const reader = await openEditorWorkspace({ baseUrl: location.origin,
        project: scenario.project, chapter: scenario.chapter, profile: scenario.profile, fetchImpl });
      requireEditorReady(reader);
      recoveryReader = reader;
      await submitOnePendingAuthorEdit({ workspace: reader, baseUrl: location.origin, fetchImpl });
      earlierGroups = (await readJournalSnapshot(reader)).groups;
    }
    releaseCommand();
    await Promise.race([outcomeStarted, submission]);
    // The editor accepts no more input while the submission is unresolved or the writer is fenced.
    expect(controller.canAcceptInput(false)).toBe(false);
    expect(workspace.partition).toEqual(fenced
      ? { ...oldPartition, disposition: "read_only_observer" } : oldPartition);
    expect(await requestResult(workspace.database.transaction("partitions")
      .objectStore("partitions").get(oldPartition.journal_partition_id)))
      .toEqual(workspace.partition);
    const frozen = (await readJournalSnapshot(workspace)).groups;
    expect(frozen).toHaveLength(1);
    expect(controller.snapshot().projection.body).toBe("Base retained");
    releaseOutcome();
    await submission;
    if (fenced) await controller.flush();
    const after = await readJournalSnapshot(workspace);
    expect({ ...after, groups: [] }).toEqual(retained);
    expect(after.groups).toEqual(earlierGroups ?? [{ ...frozen[0], reconciliation: {
      kind: "outcome_query_unresolved",
      strongest: { kind: "challenge_issued", expires_at: expiresAt },
    } }]);
    expect(counts).toEqual({ commands: 1,
      outcomes: earlier === "none" || earlier === "rejected" ? 1 : 2 });
    expect(failures).toHaveLength(fenced ? 1 : 0);
  } finally {
    releaseCommand();
    releaseOutcome();
    await submission;
    controller?.close();
    workspace?.database.close();
    recoveryReader?.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
