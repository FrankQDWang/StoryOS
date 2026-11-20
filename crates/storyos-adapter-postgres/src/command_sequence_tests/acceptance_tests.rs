use storyos_core::{AcceptProposalRefusal, AcceptanceTargetResolution, TransitionOutcome};
use uuid::Uuid;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::{
    ACCEPT_PROPOSAL, accept_proposal, accept_proposal_call, acceptable_proposal,
    advance_chapter_head, seed_condition,
};
use crate::shared_tests::command_sequence_damaged_evidence::{ReplayError, damaged_replay};
use crate::shared_tests::command_sequence_replay::move_to_earlier_base;
use crate::shared_tests::command_sequence_support::{issued, stores};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_damaged_applied_acceptance_replay_is_a_store_fault() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (base, damage) in [
        (
            0xa880,
            (|receipt_id: &str| {
                format!(
                    "DELETE FROM storyos.project_activity_events WHERE receipt_id = '{receipt_id}'"
                )
            }) as fn(&str) -> String,
        ),
        (0xa890, |receipt_id: &str| {
            format!("DELETE FROM storyos.authoritative_commits WHERE receipt_id = '{receipt_id}'")
        }),
        (0xa8a0, |receipt_id: &str| {
            format!(
                "UPDATE storyos.author_action_entries
                    SET disposition = 'compensation', compensated_source_sequence = 1
                  WHERE receipt_id = '{receipt_id}'"
            )
        }),
    ] {
        let call = accept_proposal_call(&store, &admin, base).await;
        observed.push(damaged_replay(&store, &admin, &call, damage).await);
    }
    assert_eq!(observed, vec![ReplayError::Unavailable; 3]);
}

/// The Chapter head of an Acceptance on an earlier base.
#[derive(Clone, Copy)]
enum EarlierHead {
    /// A copy of the base.
    Copy,
    /// The target Block and a new Block.
    AddedBlock,
    /// Only a new Block.
    TargetRemoved,
}

/// An Acceptance on an earlier base reads the Acceptance condition of the Revision and the Blocks
/// of the current Chapter head, not those of the base.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_earlier_base_acceptance_reads_the_condition_and_the_blocks_of_the_head() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let new_block = (Uuid::now_v7().to_string(), "A new paragraph".to_owned());
    let mut observed = Vec::new();
    let mut targets = Vec::new();
    for (base, condition, head) in [
        (
            0x8a80,
            Some("proposal_recovery_conflict"),
            EarlierHead::Copy,
        ),
        (0x8aa0, Some("proposal_conflict"), EarlierHead::AddedBlock),
        (0x8ac0, None, EarlierHead::TargetRemoved),
    ] {
        let (scope, mut input, _chapter_a_head) = acceptable_proposal(&store, &admin, base).await;
        move_to_earlier_base(&admin, &input).await;
        if let Some(condition_kind) = condition {
            seed_condition(
                &admin,
                &scope,
                &input.proposal_id,
                &input.proposal_revision_id,
                condition_kind,
            )
            .await;
        }
        let target: String = admin
            .query_one(
                "SELECT manuscript_block_id::text FROM storyos.proposals
                  WHERE proposal_id = $1::text::uuid",
                &[&input.proposal_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        targets.push(target.clone());
        let blocks = match head {
            EarlierHead::Copy => None,
            EarlierHead::AddedBlock => Some(vec![
                (target, "The author text".to_owned()),
                new_block.clone(),
            ]),
            EarlierHead::TargetRemoved => Some(vec![new_block.clone()]),
        };
        if let Some(blocks) = blocks {
            input.expected_authoritative_revision_id =
                advance_chapter_head(&admin, &input.expected_authoritative_revision_id, &blocks)
                    .await;
        }
        let call = issued(&store, &scope, base + 9, &ACCEPT_PROPOSAL, input).await;
        observed.push(
            match accept_proposal(&store, &call).await.unwrap().outcome {
                TransitionOutcome::Applied(applied) => Ok((
                    applied.effect,
                    applied
                        .blocks
                        .into_iter()
                        .map(|block| (block.manuscript_block_id, block.text))
                        .collect::<Vec<_>>(),
                )),
                TransitionOutcome::Refused(reason) => Err(reason),
                other => panic!("the Acceptance must apply or refuse, got {other:?}"),
            },
        );
    }

    assert_eq!(
        observed,
        vec![
            Err(AcceptProposalRefusal::NotEligible),
            Ok((
                AcceptanceTargetResolution::BlockReplacement,
                vec![(targets[1].clone(), "Accepted text".to_owned()), new_block],
            )),
            Err(AcceptProposalRefusal::MissingTarget),
        ]
    );
}
