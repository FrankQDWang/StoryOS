//! An Acceptance splits its candidate at each line feed into Blocks (ADR 0106, section 2).

use storyos_application::{AcceptProposalInput, ProjectScope};
use storyos_core::{AcceptanceTargetResolution, TransitionOutcome};
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::{
    ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal_with, advance_chapter_head,
};
use crate::shared_tests::command_sequence_replay::move_to_earlier_base;
use crate::shared_tests::command_sequence_secondary_operation::chapter_blocks;
use crate::shared_tests::command_sequence_support::{issued, stores};
use crate::statement_cache::Client;

/// The Proposal fixture of `candidate` for the only Block of Chapter B, with its target Block and
/// its Chapter.
struct SplitFixture {
    scope: ProjectScope,
    input: AcceptProposalInput,
    target: String,
    chapter_id: String,
}

async fn split_fixture(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
    candidate: &str,
) -> SplitFixture {
    let (scope, input, _chapter_a_head) =
        acceptable_proposal_with(store, admin, base, candidate).await;
    let row = admin
        .query_one(
            "SELECT manuscript_block_id::text, chapter_id::text FROM storyos.proposals
              WHERE proposal_id = $1::text::uuid",
            &[&input.proposal_id],
        )
        .await
        .unwrap();
    SplitFixture {
        scope,
        input,
        target: row.get(/*idx*/ 0),
        chapter_id: row.get(/*idx*/ 1),
    }
}

/// Accepts the fixture and returns the resolution and the Blocks of a new read of the Chapter.
async fn accept_and_read(
    store: &PostgresProjectReader,
    fixture: &SplitFixture,
    suffix: u16,
) -> (AcceptanceTargetResolution, Vec<(String, String)>) {
    let call = issued(
        store,
        &fixture.scope,
        suffix,
        &ACCEPT_PROPOSAL,
        fixture.input.clone(),
    )
    .await;
    let TransitionOutcome::Applied(applied) = accept_proposal(store, &call).await.unwrap().outcome
    else {
        panic!("the Acceptance must apply");
    };
    (
        applied.effect,
        chapter_blocks(store, &fixture.scope, &fixture.chapter_id).await,
    )
}

/// The Block ids of `blocks` that are not `known`. Each must be a UUID v7.
fn new_ids(blocks: &[(String, String)], known: &[&str]) -> Vec<String> {
    let ids: Vec<String> = blocks
        .iter()
        .map(|(id, _)| id.clone())
        .filter(|id| !known.contains(&id.as_str()))
        .collect();
    for id in &ids {
        assert_eq!(Uuid::parse_str(id).unwrap().get_version_num(), 7);
    }
    ids
}

/// Each line feed of the candidate starts a new paragraph Block after the target Block, on the
/// exact base and as a Block replacement on an earlier base.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_accepted_candidate_with_two_line_feeds_gives_three_blocks_in_each_resolution_mode() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (mut observed, mut wanted) = (Vec::new(), Vec::new());
    for (base, earlier_base) in [(0xe700_u16, false), (0xe720, true)] {
        let mut fixture = split_fixture(&store, &admin, base, "One.\nTwo.\nThree.").await;
        let after = (Uuid::now_v7().to_string(), "After.".to_owned());
        if earlier_base {
            move_to_earlier_base(&admin, &fixture.input).await;
            fixture.input.expected_authoritative_revision_id = advance_chapter_head(
                &admin,
                &fixture.input.expected_authoritative_revision_id,
                &[(fixture.target.clone(), "Author text".to_owned()), after.clone()],
            )
            .await;
        }
        let (resolution, blocks) = accept_and_read(&store, &fixture, base + 9).await;
        let added = new_ids(&blocks, &[&fixture.target, &after.0]);
        assert_eq!(added.len(), 2);
        assert_ne!(added[0], added[1]);
        let mut expected = vec![
            (fixture.target.clone(), "One.".to_owned()),
            (added[0].clone(), "Two.".to_owned()),
            (added[1].clone(), "Three.".to_owned()),
        ];
        if earlier_base {
            expected.push(after);
        }
        observed.push((resolution, blocks));
        wanted.push(expected);
    }

    let [exact, replacement] = <[Vec<(String, String)>; 2]>::try_from(wanted).unwrap();
    assert_eq!(
        observed,
        vec![
            (AcceptanceTargetResolution::ExactBase, exact),
            (AcceptanceTargetResolution::BlockReplacement, replacement),
        ]
    );
}
