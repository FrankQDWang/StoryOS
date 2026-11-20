//! The shared test support from `command_sequence_tests/replay_tests.rs`.

use storyos_application::AcceptProposalInput;
use uuid::Uuid;

/// Gives the Proposal Revision and its Validation Receipt a copy of the Chapter Head as base.
/// Thus the Head is not the base.
pub(crate) async fn move_to_earlier_base(
    admin: &tokio_postgres::Client,
    acceptance: &AcceptProposalInput,
) {
    let earlier = Uuid::now_v7();
    admin
        .batch_execute(&format!(
            "INSERT INTO storyos.authoritative_revisions
             SELECT owner_user_id, project_id, manuscript_object_id, '{earlier}'::uuid, payload_id
               FROM storyos.authoritative_revisions WHERE revision_id = '{head}'::uuid;
             INSERT INTO storyos.manuscript_revision_members
             SELECT owner_user_id, project_id, manuscript_object_id, '{earlier}'::uuid,
                    manuscript_block_id, block_order
               FROM storyos.manuscript_revision_members WHERE revision_id = '{head}'::uuid;
             UPDATE storyos.proposal_revisions SET base_authoritative_revision_id = '{earlier}'::uuid
              WHERE revision_id = '{revision}'::uuid;
             UPDATE storyos.validation_receipts SET base_authoritative_revision_id = '{earlier}'::uuid
              WHERE validation_receipt_id = '{validation}'::uuid;",
            head = acceptance.expected_authoritative_revision_id,
            revision = acceptance.proposal_revision_id,
            validation = acceptance.validation_receipt_id,
        ))
        .await
        .unwrap();
}
