use std::error::Error as _;

use storyos_application::{ProjectId, ProjectScope, ProposalAnchorRecord, ProposalReader, UserId};
use tokio_postgres::error::SqlState;

use crate::recovery_visibility_proof_tests::{open_isolated_proof, repo_root};

const BINDING_MIGRATION: &str = "0094_bind_proposal_anchor_revisions.sql";
const OWNER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";
const CHAPTER: &str = "018f0000-0000-7001-8000-000000000003";
const OPENING_BASE: &str = "018f0000-0000-7001-8000-000000000005";
const BLOCK: &str = "018f0000-0000-7001-8000-0000000000b1";
const PROPOSAL: &str = "018f0000-0000-7001-8000-0000000a9401";
const OPENING_REVISION: &str = "018f0000-0000-7001-8000-0000000a9402";
const EDITED_REVISION: &str = "018f0000-0000-7001-8000-0000000a9403";
const OPERATION: &str = "018f0000-0000-7001-8000-0000000a9404";
const REPLANNED_REVISION: &str = "018f0000-0000-7001-8000-0000000a9405";
const REPLANNED_BASE: &str = "018f0000-0000-7001-8000-0000000a9406";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn legacy_anchor_rows_bind_to_the_opening_revision_and_a_different_base_fails_closed() {
    let (_isolated, reader, admin) =
        open_isolated_proof("storyos_anchor_binding", Some(BINDING_MIGRATION)).await;
    let digest = storyos_core::proposal_anchor_base_slice_digest(
        BLOCK,
        "paragraph",
        /*manuscript_schema_version*/ 1,
        "prosemirror-token-utf16.v1",
        /*from*/ 0,
        /*to*/ 4,
        "Auth",
    );
    admin
        .batch_execute(&format!(
            "BEGIN; SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{OWNER}', '{PROJECT}', '{PROPOSAL}', 'inline_edit', '{CHAPTER}', '{BLOCK}',
                     gen_random_uuid(), gen_random_uuid());
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
             VALUES ('{OWNER}', '{PROJECT}', '{PROPOSAL}', '{OPENING_REVISION}', 'ready', 'valid',
                     'open', 'Wrote', '{OPENING_BASE}', NULL),
                    ('{OWNER}', '{PROJECT}', '{PROPOSAL}', '{EDITED_REVISION}', 'ready', 'pending',
                     'open', 'Wrote it', '{OPENING_BASE}', '{OPENING_REVISION}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{OWNER}', '{PROJECT}', '{PROPOSAL}', '{EDITED_REVISION}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{OWNER}', '{PROJECT}', '{PROPOSAL}', '{OPERATION}', '{BLOCK}', 'pending',
                     'unresolved', 'Wrote it');
             INSERT INTO storyos.proposal_anchors
               (owner_user_id, project_id, proposal_id, operation_id, anchor_order,
                manuscript_block_id, base_authoritative_revision_id, manuscript_schema_version,
                coordinate_profile, range_from, range_to, boundary_profile, base_slice_digest)
             VALUES ('{OWNER}', '{PROJECT}', '{PROPOSAL}', '{OPERATION}', 1, '{BLOCK}',
                     '{OPENING_BASE}', 1, 'prosemirror-token-utf16.v1', 0, 4,
                     'exclusive-authoritative-edges.v1', '{digest}');
             COMMIT;"
        ))
        .await
        .unwrap();
    // The reader of this code needs the binding migration and each later migration.
    let directory = repo_root().join("crates/storyos-adapter-postgres/migrations");
    let mut names = std::fs::read_dir(&directory)
        .expect("read the migration directory")
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".sql") && name.as_str() >= BINDING_MIGRATION)
        .collect::<Vec<_>>();
    names.sort();
    for name in names {
        let migration = std::fs::read_to_string(directory.join(&name)).expect("read a migration");
        admin
            .batch_execute(&format!("BEGIN;\n{migration}\nCOMMIT;"))
            .await
            .unwrap();
    }
    let scope = ProjectScope::new(UserId::new(OWNER), ProjectId::new(PROJECT));

    let migrated = reader.read_proposal(&scope, PROPOSAL).await.unwrap();
    assert_eq!(
        migrated.map(|record| (record.revision_id, record.anchors)),
        Some((
            EDITED_REVISION.to_owned(),
            vec![ProposalAnchorRecord {
                manuscript_block_id: BLOCK.to_owned(),
                base_authoritative_revision_id: OPENING_BASE.to_owned(),
                manuscript_schema_version: 1,
                coordinate_profile: "prosemirror-token-utf16.v1".to_owned(),
                from: 0,
                to: 4,
                boundary_profile: "exclusive-authoritative-edges.v1".to_owned(),
                base_slice_digest: digest,
            }],
        ))
    );

    admin
        .batch_execute(&format!(
            "BEGIN; SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
             VALUES ('{OWNER}', '{PROJECT}', '{PROPOSAL}', '{REPLANNED_REVISION}', 'ready',
                     'pending', 'open', 'Wrote it', '{REPLANNED_BASE}', '{EDITED_REVISION}');
             UPDATE storyos.proposal_heads SET current_revision_id = '{REPLANNED_REVISION}'
              WHERE proposal_id = '{PROPOSAL}';
             COMMIT;"
        ))
        .await
        .unwrap();
    let refused = reader
        .read_proposal(&scope, PROPOSAL)
        .await
        .expect_err("an Anchor set on a different base must fail closed");
    let database_error = refused
        .source()
        .and_then(|source| source.downcast_ref::<tokio_postgres::Error>())
        .and_then(tokio_postgres::Error::code);
    assert_eq!(database_error, Some(&SqlState::DATA_CORRUPTED));
}
