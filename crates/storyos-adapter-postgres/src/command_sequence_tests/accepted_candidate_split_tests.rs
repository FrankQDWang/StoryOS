//! An Acceptance splits its candidate at each line feed into Blocks (ADR 0106, section 2).

use storyos_application::{AcceptProposalInput, ProjectScope};
use storyos_core::{AcceptanceTargetResolution, TransitionOutcome};
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::{
    ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal_with, advance_chapter_head,
};
use crate::shared_tests::command_sequence_proposal_undo_chain::{head_closure, undo_call};
use crate::shared_tests::command_sequence_replay::move_to_earlier_base;
use crate::shared_tests::command_sequence_secondary_operation::chapter_blocks;
use crate::shared_tests::command_sequence_support::{issued, stores};
use crate::shared_tests::command_sequence_undo::{edit_in_chapter_of, undo};
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
                &[
                    (fixture.target.clone(), "Author text".to_owned()),
                    after.clone(),
                ],
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

/// An empty part makes no Block: two line feeds together, and a line feed at the start or at the
/// end. When no part has text, the target Block stays with empty text.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_empty_part_of_an_accepted_candidate_makes_no_block() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (base, candidate) in [(0xe740_u16, "\nOne.\n\nTwo.\n"), (0xe760, "\n\n")] {
        let fixture = split_fixture(&store, &admin, base, candidate).await;
        let (_, blocks) = accept_and_read(&store, &fixture, base + 9).await;
        new_ids(&blocks, &[&fixture.target]);
        observed.push(
            blocks
                .into_iter()
                .map(|(id, text)| (id == fixture.target, text))
                .collect::<Vec<_>>(),
        );
    }

    assert_eq!(
        observed,
        vec![
            vec![(true, "One.".to_owned()), (false, "Two.".to_owned())],
            vec![(true, String::new())],
        ]
    );
}

/// A line feed in a Block that is not the target of the Acceptance stays a line feed.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_line_feed_outside_the_accepted_candidate_stays_in_its_block() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut fixture = split_fixture(&store, &admin, 0xe780, "New.\nNext.").await;
    let other = (Uuid::now_v7().to_string(), "Keep\nthis line.".to_owned());
    move_to_earlier_base(&admin, &fixture.input).await;
    fixture.input.expected_authoritative_revision_id = advance_chapter_head(
        &admin,
        &fixture.input.expected_authoritative_revision_id,
        &[
            (fixture.target.clone(), "Old\ntext.".to_owned()),
            other.clone(),
        ],
    )
    .await;
    let (_, blocks) = accept_and_read(&store, &fixture, 0xe789).await;
    let added = new_ids(&blocks, &[&fixture.target, &other.0]);

    assert_eq!(
        blocks,
        vec![
            (fixture.target.clone(), "New.".to_owned()),
            (added[0].clone(), "Next.".to_owned()),
            other,
        ]
    );
}

/// The members, with their order, and the stored payload of one Chapter Revision.
async fn stored_revision(admin: &Client, revision_id: &str) -> (Vec<(String, i64)>, String) {
    let members = admin
        .query(
            "SELECT manuscript_block_id::text, block_order::bigint
               FROM storyos.manuscript_revision_members
              WHERE revision_id = $1::text::uuid ORDER BY block_order",
            &[&revision_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
        .collect();
    let payload = admin
        .query_one(
            "SELECT convert_from(payload.canonical_bytes, 'UTF8')
               FROM storyos.authoritative_revisions AS revision
               JOIN storyos.authoritative_payloads AS payload
                 ON (payload.project_id, payload.payload_id) =
                    (revision.project_id, revision.payload_id)
              WHERE revision.revision_id = $1::text::uuid",
            &[&revision_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    (members, payload)
}

async fn chapter_head(admin: &Client, chapter_id: &str) -> String {
    admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.authoritative_heads
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// A candidate with no line feed keeps the Blocks, the members, and the stored text of the rule
/// before the split, on the exact base and on an earlier base.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_accepted_candidate_with_no_line_feed_keeps_the_blocks_and_the_stored_text() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    let mut wanted = Vec::new();
    for (base, earlier_base) in [(0xe7a0_u16, false), (0xe7c0, true)] {
        let mut fixture = split_fixture(&store, &admin, base, "Plain text.").await;
        let after = Uuid::now_v7().to_string();
        if earlier_base {
            move_to_earlier_base(&admin, &fixture.input).await;
            fixture.input.expected_authoritative_revision_id = advance_chapter_head(
                &admin,
                &fixture.input.expected_authoritative_revision_id,
                &[
                    (fixture.target.clone(), "Author text".to_owned()),
                    (after.clone(), "After.".to_owned()),
                ],
            )
            .await;
        }
        let (prior_members, _) =
            stored_revision(&admin, &fixture.input.expected_authoritative_revision_id).await;
        let (_, blocks) = accept_and_read(&store, &fixture, base + 9).await;
        let (members, payload) =
            stored_revision(&admin, &chapter_head(&admin, &fixture.chapter_id).await).await;
        let payload = if earlier_base {
            serde_json::from_str::<serde_json::Value>(&payload).unwrap()["blocks"].clone()
        } else {
            serde_json::Value::String(payload)
        };
        observed.push((blocks, members, payload));
        let target = fixture.target.clone();
        wanted.push(if earlier_base {
            (
                vec![
                    (target.clone(), "Plain text.".to_owned()),
                    (after.clone(), "After.".to_owned()),
                ],
                prior_members,
                serde_json::json!([
                    {"manuscript_block_id": target, "block_kind": "paragraph",
                     "text": "Plain text."},
                    {"manuscript_block_id": after, "block_kind": "paragraph", "text": "After."},
                ]),
            )
        } else {
            (
                vec![(target, "Plain text.".to_owned())],
                prior_members,
                serde_json::Value::String("Plain text.".to_owned()),
            )
        });
    }

    assert_eq!(observed, wanted);
}

/// Author Undo of a split Acceptance restores the earlier Blocks with their identities and
/// reopens the Proposal with its one candidate of the target Block.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_a_split_acceptance_restores_the_blocks_and_reopens_the_proposal() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let fixture = split_fixture(&store, &admin, 0xe7e0, "One.\nTwo.").await;
    // The writer edits Chapter B, so the Acceptance moves its base and Author Undo applies.
    edit_in_chapter_of(
        &admin,
        fixture.input.editor_session_id.as_ref(),
        &fixture.input.expected_authoritative_revision_id,
    )
    .await;
    let before = chapter_blocks(&store, &fixture.scope, &fixture.chapter_id).await;
    let (_, accepted) = accept_and_read(&store, &fixture, 0xe7e9).await;
    assert_eq!(accepted.len(), 2);
    let call = undo_call(
        &store,
        &admin,
        &fixture.scope,
        fixture.input.editor_session_id.as_ref(),
        &chapter_head(&admin, &fixture.chapter_id).await,
        /*suffix*/ 0xe7ea,
    )
    .await;
    let settled = undo(&store, &call).await.unwrap();
    assert!(
        matches!(settled.outcome, TransitionOutcome::Applied(_)),
        "the Undo must compensate, got {:?}",
        settled.outcome
    );
    let operations: Vec<(String, String, String)> = admin
        .query(
            "SELECT manuscript_block_id::text, resolution, candidate_text
               FROM storyos.proposal_operations WHERE proposal_id = $1::text::uuid",
            &[&fixture.input.proposal_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1), row.get(/*idx*/ 2)))
        .collect();

    assert_eq!(
        (
            chapter_blocks(&store, &fixture.scope, &fixture.chapter_id).await,
            head_closure(&admin, &fixture.input.proposal_id).await,
            operations,
        ),
        (
            before,
            "open".to_owned(),
            vec![(
                fixture.target.clone(),
                "pending".to_owned(),
                "One.\nTwo.".to_owned()
            )],
        )
    );
}
