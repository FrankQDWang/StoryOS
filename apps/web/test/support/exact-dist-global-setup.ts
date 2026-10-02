import assert from "node:assert/strict";

import { queryStoryOSPostgres } from "./node-integration";
import { requiredGlobalTeardown } from "./required-global-teardown";

const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const CHAPTER = "018f0000-0000-7001-8000-000000000003";

export default function exactDistGlobalSetup(): (() => Promise<void>) | undefined {
  if (process.env.STORYOS_STAGE1_AUTHORITY_ORACLE !== "1") return undefined;
  return requiredGlobalTeardown(async () => {
    const authorityJson = await queryStoryOSPostgres(`
      WITH production AS (
        SELECT owner_user_id, project_id, current_chapter_id FROM storyos.projects
        WHERE owner_user_id = '${USER_A}'::uuid AND title = 'Production host acceptance'
      ), prose_request AS (
        SELECT owner_user_id, project_id FROM storyos.projects
        WHERE owner_user_id = '${USER_A}'::uuid AND title LIKE 'Prose request %'
      ), inline_proposal AS (
        SELECT owner_user_id, project_id FROM storyos.projects
        WHERE owner_user_id = '${USER_A}'::uuid AND title LIKE 'Inline Proposal %'
      ), captured_memory AS (
        SELECT owner_user_id, project_id FROM storyos.projects
        WHERE owner_user_id = '${USER_A}'::uuid AND title LIKE 'Captured Memory acceptance %'
      ), run_evidence AS (
        SELECT owner_user_id, project_id FROM storyos.projects
        WHERE owner_user_id = '${USER_A}'::uuid
          AND (title = 'Run evidence acceptance' OR title ~ '^Run evidence [2-9a-d]$')
      ), composer AS (
        SELECT owner_user_id, project_id FROM storyos.projects
        WHERE owner_user_id = '${USER_A}'::uuid AND title LIKE 'Composer %'
      )
      SELECT json_build_object(
        'prose_request', json_build_object(
          'project_count', (SELECT count(*) FROM prose_request),
          'receipts', (SELECT json_object_agg(command_kind, count) FROM (
            SELECT command_kind, count(*) FROM storyos.domain_receipts
            JOIN prose_request USING (owner_user_id, project_id)
            GROUP BY command_kind
          ) AS receipts),
          'author_action_count', (SELECT count(*) FROM storyos.author_action_entries
            JOIN prose_request USING (owner_user_id, project_id))
        ),
        'inline_proposal', json_build_object(
          'project_count', (SELECT count(*) FROM inline_proposal),
          'receipts', (SELECT json_object_agg(command_kind, count) FROM (
            SELECT command_kind, count(*) FROM storyos.domain_receipts
            JOIN inline_proposal USING (owner_user_id, project_id) GROUP BY command_kind
          ) AS receipts),
          'author_action_count', (SELECT count(*) FROM storyos.author_action_entries
            JOIN inline_proposal USING (owner_user_id, project_id))
        ),
        'captured_memory', json_build_object(
          'project_count', (SELECT count(*) FROM captured_memory),
          'receipts', (SELECT json_object_agg(command_kind, count) FROM (
            SELECT command_kind, count(*) FROM storyos.domain_receipts
            JOIN captured_memory USING (owner_user_id, project_id) GROUP BY command_kind
          ) AS receipts),
          'author_action_count', (SELECT count(*) FROM storyos.author_action_entries
            JOIN captured_memory USING (owner_user_id, project_id))
        ),
        'run_evidence', json_build_object(
          'project_count', (SELECT count(*) FROM run_evidence),
          'receipts', (SELECT json_object_agg(command_kind, count) FROM (
            SELECT command_kind, count(*) FROM storyos.domain_receipts
            JOIN run_evidence USING (owner_user_id, project_id) GROUP BY command_kind
          ) AS receipts),
          'author_action_count', (SELECT count(*) FROM storyos.author_action_entries
            JOIN run_evidence USING (owner_user_id, project_id))
        ),
        'composer', json_build_object(
          'project_count', (SELECT count(*) FROM composer),
          'receipts', (SELECT json_object_agg(command_kind, count) FROM (
            SELECT command_kind, count(*) FROM storyos.domain_receipts
            JOIN composer USING (owner_user_id, project_id) GROUP BY command_kind
          ) AS receipts),
          'author_action_count', (SELECT count(*) FROM storyos.author_action_entries
            JOIN composer USING (owner_user_id, project_id))
        ),
        'production_host', json_build_object(
          'project_count', (SELECT count(*) FROM production),
          'receipts', (SELECT json_object_agg(command_kind, count) FROM (
            SELECT command_kind, count(*) FROM storyos.domain_receipts
            JOIN production USING (owner_user_id, project_id) GROUP BY command_kind
          ) AS receipts),
          'session_count', (SELECT count(*) FROM storyos.editor_sessions
            JOIN production USING (owner_user_id, project_id)),
          'writer_generations', (SELECT json_agg(writer_generation::text ORDER BY writer_generation)
            FROM storyos.project_writer_generations JOIN production USING (owner_user_id, project_id)),
          'author_action_count', (SELECT count(*) FROM storyos.author_action_entries
            JOIN production USING (owner_user_id, project_id)),
          'manuscript_body', (SELECT convert_from(payload.canonical_bytes, 'UTF8')
            FROM production
            JOIN storyos.authoritative_heads AS head
              ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                 (production.owner_user_id, production.project_id, production.current_chapter_id)
            JOIN storyos.authoritative_revisions AS revision
              ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                  revision.revision_id) =
                 (head.owner_user_id, head.project_id, head.manuscript_object_id,
                  head.current_revision_id)
            JOIN storyos.authoritative_payloads AS payload
              ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                 (revision.owner_user_id, revision.project_id, revision.payload_id))
        ),
        'receipt_count', (SELECT count(*) FROM storyos.domain_receipts AS receipt
          WHERE receipt.owner_user_id = '${USER_A}'::uuid
            AND receipt.project_id = '${PROJECT_A}'::uuid),
        'activity_count', (SELECT count(*) FROM storyos.project_activity_events AS activity
          WHERE activity.owner_user_id = '${USER_A}'::uuid
            AND activity.project_id = '${PROJECT_A}'::uuid),
        'author_action_count', (SELECT count(*) FROM storyos.author_action_entries AS action
          WHERE action.owner_user_id = '${USER_A}'::uuid
            AND action.project_id = '${PROJECT_A}'::uuid),
        'project_activity_position', (SELECT max(activity.project_activity_position)::text
          FROM storyos.project_activity_events AS activity
          WHERE activity.owner_user_id = '${USER_A}'::uuid
            AND activity.project_id = '${PROJECT_A}'::uuid),
        'manuscript_body', (SELECT convert_from(payload.canonical_bytes, 'UTF8')
          FROM storyos.authoritative_heads AS head
          JOIN storyos.authoritative_revisions AS revision
            ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                revision.revision_id) =
               (head.owner_user_id, head.project_id, head.manuscript_object_id,
                head.current_revision_id)
          JOIN storyos.authoritative_payloads AS payload
            ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
               (revision.owner_user_id, revision.project_id, revision.payload_id)
          WHERE head.owner_user_id = '${USER_A}'::uuid
            AND head.project_id = '${PROJECT_A}'::uuid
            AND head.manuscript_object_id = '${CHAPTER}'::uuid),
        'foreign_user_receipt_count', (SELECT count(*) FROM storyos.domain_receipts
          WHERE owner_user_id <> '${USER_A}'::uuid),
        'activity_consumer_receipt_count', (SELECT count(*) FROM storyos.domain_receipts
          WHERE owner_user_id = '${USER_A}'::uuid
            AND (command_kind, idempotency_key) IN (
              ('updateProjectAssistance', '018f0000-0000-7001-8000-00000000f802'::uuid),
              ('createAgentRun', '018f0000-0000-7001-8000-00000000f804'::uuid))),
        'non_fixture_non_create_project_receipt_count', (SELECT count(*) FROM storyos.domain_receipts
          WHERE owner_user_id = '${USER_A}'::uuid
            AND project_id <> '${PROJECT_A}'::uuid
            AND project_id NOT IN (SELECT project_id FROM production)
            AND project_id NOT IN (SELECT project_id FROM prose_request)
            AND project_id NOT IN (SELECT project_id FROM inline_proposal)
            AND project_id NOT IN (SELECT project_id FROM captured_memory)
            AND project_id NOT IN (SELECT project_id FROM run_evidence)
            AND project_id NOT IN (SELECT project_id FROM composer)
            AND project_id NOT IN (SELECT project_id FROM storyos.projects
              WHERE owner_user_id = '${USER_A}'::uuid AND title LIKE 'Refused edit %')
            AND (command_kind, idempotency_key) NOT IN (
              ('updateProjectAssistance', '018f0000-0000-7001-8000-00000000f802'::uuid),
              ('createAgentRun', '018f0000-0000-7001-8000-00000000f804'::uuid))
            AND command_kind <> 'createProject'
            AND command_kind <> 'updateProject'
            AND command_kind <> 'archiveProject'
            AND command_kind <> 'createVolume'
            AND command_kind <> 'createChapter'
            AND command_kind <> 'updateVolume'
            AND command_kind <> 'updateChapter'
            AND command_kind <> 'applyAuthorEdit'
            AND command_kind <> 'setCurrentChapter'
            AND command_kind <> 'undoLatestAuthorAction'
            AND command_kind <> 'deleteChapter'
            AND command_kind <> 'deleteVolume'
            AND command_kind <> 'exportHumanReadableManuscript'
            AND command_kind <> 'exportProjectArchive')
      )::text`);
    const authority: unknown = JSON.parse(authorityJson);
    assert.deepEqual(authority, {
      prose_request: {
        project_count: 2,
        receipts: { createProject: 2, createVolume: 2, createChapter: 2,
          applyAuthorEdit: 8, undoLatestAuthorAction: 1,
          updateProjectAssistance: 2, createAgentRun: 5, acceptProposal: 2,
          rejectProposalOperations: 1, replanProposal: 1, takeOverProjectWriter: 1 },
        author_action_count: 16,
      },
      inline_proposal: {
        project_count: 1,
        receipts: { createProject: 1, createVolume: 1, createChapter: 5,
          updateProjectAssistance: 1, setCurrentChapter: 4, createAgentRun: 6,
          applyAuthorEdit: 17, acceptProposal: 1, rejectProposalOperations: 1,
          takeOverProjectWriter: 1 },
        author_action_count: 27,
      },
      captured_memory: {
        project_count: 1,
        receipts: { createProject: 1, createVolume: 1, createChapter: 1,
          updateProjectAssistance: 1, createAgentRun: 2, cancelAgentRun: 1 },
        author_action_count: 2,
      },
      run_evidence: {
        project_count: 13,
        receipts: { createProject: 13, createVolume: 13, createChapter: 13,
          updateProjectAssistance: 13, applyAuthorEdit: 4, createAgentRun: 17,
          cancelAgentRun: 2 },
        author_action_count: 30,
      },
      composer: {
        project_count: 1,
        receipts: { createProject: 1, createVolume: 1, createChapter: 1,
          updateProjectAssistance: 1, applyAuthorEdit: 1, createAgentRun: 3,
          pauseAgentRun: 2, steerAgentRun: 1, cancelAgentRun: 1, takeOverProjectWriter: 1 },
        author_action_count: 3,
      },
      production_host: {
        project_count: 1,
        receipts: { createProject: 1, createVolume: 1, createChapter: 1,
          applyAuthorEdit: 3, takeOverProjectWriter: 1 },
        session_count: 2,
        writer_generations: ["1", "2"],
        author_action_count: 5,
        manuscript_body: "Saved by the new production writer.",
      },
      receipt_count: 4,
      activity_count: 4,
      author_action_count: 4,
      project_activity_position: "4",
      manuscript_body: "Authoritative A Hello中文 EN!",
      foreign_user_receipt_count: 0,
      activity_consumer_receipt_count: 2,
      non_fixture_non_create_project_receipt_count: 0,
    });
  });
}
