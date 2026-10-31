//! The classification, effect rows, scope isolation, and Author Undo Compensation of Create
//! Volume and Create Chapter. `command_sequence_tests/` owns replay, rollback, and evidence.

use storyos_application::{
    AuthorEditSettlementEffect, CanonicalManuscriptTree, ChapterCreated, ChapterId, ChapterNode,
    CreateChapterInput, CreateChapterPublicOrder, CreateChapterSettlement, CreateVolumeInput,
    CreateVolumePublicOrder, CreateVolumeSettlement, GetManuscriptTree, OpenChapter, ProjectScope,
    SnapshotLookup, UndoApplied, UndoRecords, UserId, VolumeCreated, VolumeId, VolumeNode,
    get_manuscript_tree, get_snapshot, open_chapter, open_current_chapter, open_project,
};
use storyos_core::{
    CreateChapterCurrent, CreateChapterPlacement, ManuscriptBlockKind, ReceiptResult,
    TransitionOutcome,
};

use crate::PostgresProjectReader;
use crate::command_sequence::tests::{
    CREATE_CHAPTER, CREATE_VOLUME, applied, create_chapter, create_volume, named_call,
    outcome_code, seed_owned_project, seed_project, stores, titled_chapter, titled_volume,
    undo_named,
};
use crate::create_volume_authority_tests::{NamedEdit, apply_named_edit};
use crate::set_current_chapter_authority_tests::open_session;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const MISSING: &str = "018f0000-0000-7001-8000-00000000ffff";

async fn settle_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    title: &str,
    expected_tree_revision: u64,
) -> CreateVolumeSettlement {
    let input = CreateVolumeInput {
        title: title.to_owned(),
        expected_tree_revision,
    };
    let call = named_call(store, scope, suffix, &CREATE_VOLUME, input).await;
    create_volume(store, &call).await.unwrap()
}

async fn settle_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    volume_id: &str,
    title: &str,
    expected_tree_revision: u64,
) -> CreateChapterSettlement {
    let input = CreateChapterInput {
        volume_id: volume_id.to_owned(),
        title: title.to_owned(),
        placement: CreateChapterPlacement::Append,
        expected_tree_revision,
    };
    let call = named_call(store, scope, suffix, &CREATE_CHAPTER, input).await;
    create_chapter(store, &call).await.unwrap()
}

async fn tree(store: &PostgresProjectReader, scope: &ProjectScope) -> CanonicalManuscriptTree {
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(store, scope).await.unwrap() else {
        panic!("the Project has a Canonical Manuscript Tree");
    };
    *tree
}

fn chapter_node(chapter_id: &str, title: &str, order: u64) -> ChapterNode {
    ChapterNode {
        chapter_id: ChapterId::new(chapter_id),
        title: title.to_owned(),
        order,
    }
}

/// The order of one Receipt payload and the order of its Activity payload.
async fn recorded_orders(admin: &tokio_postgres::Client, receipt_id: &str) -> (String, String) {
    let row = admin
        .query_one(
            "SELECT receipt.result_payload::text, payload.payload->>'order'
               FROM storyos.domain_receipts AS receipt
               JOIN storyos.project_activity_event_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              WHERE receipt.receipt_id = $1::text::uuid",
            &[&receipt_id],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
}

/// Expires only the latest Snapshot. The older Snapshots stay valid, and a read must not fall back to them.
async fn expire_latest_snapshot(
    admin: &tokio_postgres::Client,
    scope: &ProjectScope,
    snapshot_id: &str,
) {
    admin
        .execute(
            "UPDATE storyos.project_snapshots
                SET expires_at = clock_timestamp() - interval '1 second'
              WHERE project_id = $1::text::uuid AND snapshot_id = $2::text::uuid",
            &[&scope.project_id.as_ref(), &snapshot_id],
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_volume_classifies_its_outcomes_and_writes_its_effect_rows() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "9620").await;
    let first = settle_volume(&store, &scope, "9621", "Volume A", 1).await;
    let second = settle_volume(&store, &scope, "9622", "Volume B", 2).await;
    let (first_effect, _) = applied(&first);
    let (second_effect, second_authority) = applied(&second);
    assert_eq!(
        [first_effect.clone(), second_effect.clone()],
        [
            VolumeCreated {
                tree_revision: 2,
                order: CreateVolumePublicOrder::CanonicalSiblingOrder(1),
                ..first_effect.clone()
            },
            VolumeCreated {
                tree_revision: 3,
                order: CreateVolumePublicOrder::CanonicalSiblingOrder(2),
                ..second_effect.clone()
            },
        ]
    );
    assert_eq!(
        (
            second_authority.prior_manuscript_tree_revision,
            second_authority.resulting_manuscript_tree_revision,
            second_authority.author_action_sequence
        ),
        (2, 3, 2)
    );
    let live = tree(&store, &scope).await;
    assert_eq!(
        (
            live.tree_revision,
            &live.snapshot.snapshot_id,
            live.volumes.clone()
        ),
        (
            3,
            &second_authority.snapshot_id,
            [(&first_effect, "Volume A"), (&second_effect, "Volume B")]
                .into_iter()
                .zip(1..)
                .map(|((effect, title), order)| VolumeNode {
                    volume_id: VolumeId::new(effect.volume_id.clone()),
                    title: title.to_owned(),
                    order,
                    chapters: Vec::new(),
                })
                .collect()
        )
    );
    assert_eq!(
        recorded_orders(&admin, &second.ids.receipt_id).await,
        (r#"{"order": "2"}"#.to_owned(), "2".to_owned())
    );
    assert_eq!(
        get_manuscript_tree(
            &store,
            &ProjectScope::new(UserId::new(USER_B), scope.project_id.clone()),
        )
        .await
        .unwrap(),
        GetManuscriptTree::Missing
    );

    let stale = settle_volume(&store, &scope, "9623", "Volume C", 1).await;
    assert_eq!(tree(&store, &scope).await.volumes, live.volumes);

    // Recovery reads the latest Snapshot until it expires.
    let recovered = get_snapshot(
        &store,
        &SnapshotLookup {
            project_scope: scope.clone(),
            snapshot_id: live.snapshot.snapshot_id.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(recovered.as_ref(), Some(&live.snapshot));
    expire_latest_snapshot(&admin, &scope, &live.snapshot.snapshot_id).await;
    assert_eq!(
        get_manuscript_tree(&store, &scope).await.unwrap(),
        GetManuscriptTree::SnapshotExpired
    );
    assert_eq!(
        outcome_code(&stale.outcome),
        (ReceiptResult::Conflicted, Some("stale_tree_revision"))
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_chapter_classifies_its_outcomes_and_writes_its_effect_rows() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "9630").await;
    let foreign = ProjectScope::new(UserId::new(USER_B), scope.project_id.clone());
    let volume_id = titled_volume(&store, &scope, "9631", "Volume A", 1).await;
    let first = settle_chapter(&store, &scope, "9632", &volume_id, "Chapter A", 2).await;
    let second = settle_chapter(&store, &scope, "9633", &volume_id, "Chapter B", 3).await;
    let (first_effect, _) = applied(&first);
    let (second_effect, second_authority) = applied(&second);
    let (chapter_a, chapter_b) = (&first_effect.chapter_id, &second_effect.chapter_id);
    assert_eq!(
        [first_effect.clone(), second_effect.clone()],
        [
            ChapterCreated {
                chapter_id: chapter_a.clone(),
                tree_revision: 3,
                current: CreateChapterCurrent::SelectCreated,
                order: CreateChapterPublicOrder::CanonicalSiblingOrder(1),
            },
            ChapterCreated {
                chapter_id: chapter_b.clone(),
                tree_revision: 4,
                current: CreateChapterCurrent::PreserveExisting,
                order: CreateChapterPublicOrder::CanonicalSiblingOrder(2),
            },
        ]
    );
    assert_eq!(
        (
            second_authority.prior_manuscript_tree_revision,
            second_authority.resulting_manuscript_tree_revision,
            second_authority.author_action_sequence
        ),
        (3, 4, 3)
    );
    let live = tree(&store, &scope).await;
    assert_eq!(
        (
            live.tree_revision,
            &live.snapshot.snapshot_id,
            live.volumes.clone()
        ),
        (
            4,
            &second_authority.snapshot_id,
            vec![VolumeNode {
                volume_id: VolumeId::new(volume_id.clone()),
                title: "Volume A".to_owned(),
                order: 1,
                chapters: vec![
                    chapter_node(chapter_a, "Chapter A", 1),
                    chapter_node(chapter_b, "Chapter B", 2),
                ],
            }]
        )
    );
    assert_eq!(
        recorded_orders(&admin, &second.ids.receipt_id).await,
        (r#"{"order": "2"}"#.to_owned(), "2".to_owned())
    );

    // The first Chapter becomes the current Chapter. A later Chapter opens from its Head.
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project is in exact Scope")
            .current_chapter_id,
        Some(ChapterId::new(chapter_a.clone()))
    );
    let current = open_current_chapter(&store, &scope, &ChapterId::new(chapter_a.clone()))
        .await
        .unwrap()
        .expect("Chapter A is the current Chapter");
    assert_eq!(
        (current.title.as_str(), current.body.as_str()),
        ("Chapter A", "")
    );
    let OpenChapter::Found(later) =
        open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter B opens from its Snapshot and Head");
    };
    assert_eq!(
        (
            later.chapter.title.as_str(),
            later.chapter.body.as_str(),
            later
                .chapter
                .blocks
                .iter()
                .map(|block| (block.block_kind.clone(), block.text.as_str()))
                .collect::<Vec<_>>()
        ),
        ("Chapter B", "", vec![(ManuscriptBlockKind::Paragraph, "")])
    );
    let genesis = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.manuscript_objects
                      WHERE project_id = $1::text::uuid AND object_kind = 'chapter'),
                    (SELECT count(*) FROM storyos.authoritative_heads
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_payloads
                      WHERE project_id = $1::text::uuid AND octet_length(canonical_bytes) = 0)",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(
        [0, 1, 2].map(|index| genesis.get::<_, i64>(index)),
        [2, 2, 2]
    );

    // Scope isolation.
    assert_eq!(
        open_current_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        open_current_chapter(&store, &foreign, &ChapterId::new(chapter_a.clone()))
            .await
            .unwrap(),
        None
    );
    for (reader_scope, chapter_id) in [(&foreign, chapter_b.as_str()), (&scope, MISSING)] {
        assert_eq!(
            open_chapter(&store, reader_scope, &ChapterId::new(chapter_id))
                .await
                .unwrap(),
            OpenChapter::Missing
        );
    }
    assert_eq!(
        get_manuscript_tree(&store, &foreign).await.unwrap(),
        GetManuscriptTree::Missing
    );

    let stale = settle_chapter(&store, &scope, "9634", &volume_id, "Chapter C", 2).await;
    // The invalid join has a stale tree revision too: the refusal comes before the conflict.
    let missing = settle_chapter(&store, &scope, "9635", MISSING, "Chapter C", 2).await;
    assert_eq!(tree(&store, &scope).await.volumes, live.volumes);
    expire_latest_snapshot(&admin, &scope, &live.snapshot.snapshot_id).await;
    assert_eq!(
        open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap(),
        OpenChapter::SnapshotExpired
    );
    assert_eq!(
        [&stale, &missing].map(|settlement| outcome_code(&settlement.outcome)),
        [
            (ReceiptResult::Conflicted, Some("stale_tree_revision")),
            (ReceiptResult::Refused, Some("invalid_volume_join")),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_project_keeps_its_own_structure_author_action_sequence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, _admin) = stores().await;
    let mut observed = Vec::new();
    for (owner, project) in [(USER_A, "9640"), (USER_A, "9643"), (USER_B, "9646")] {
        let scope = seed_owned_project(&store, owner, project).await;
        let suffix =
            |offset: u16| format!("{:04x}", u16::from_str_radix(project, 16).unwrap() + offset);
        let volume = settle_volume(&store, &scope, &suffix(1), "Volume A", 1).await;
        let chapter = settle_chapter(
            &store,
            &scope,
            &suffix(2),
            &applied(&volume).0.volume_id,
            "Chapter A",
            2,
        )
        .await;
        observed.push([applied(&volume).1, applied(&chapter).1]);
    }
    assert_eq!(
        observed
            .iter()
            .map(|authorities| authorities
                .each_ref()
                .map(|authority| authority.author_action_sequence))
            .collect::<Vec<_>>(),
        vec![[1, 2]; 3]
    );
    let mut commits = observed
        .iter()
        .flatten()
        .map(|authority| authority.authoritative_commit_id.clone())
        .collect::<Vec<_>>();
    commits.sort();
    commits.dedup();
    assert_eq!(commits.len(), 6);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_create_volume_after_a_later_edit_is_undone() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "9650").await;
    let volume_a = titled_volume(&store, &scope, "9651", "Volume A", 1).await;
    let chapter_id = titled_chapter(&store, &scope, "9652", &volume_a, "Chapter A", 2).await;
    let editor_session_id = open_session(&store, &scope, "9653").await;
    let OpenChapter::Found(opened) =
        open_chapter(&store, &scope, &ChapterId::new(chapter_id.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter A must open");
    };
    let edit = async |suffix: &str, expected_revision_id: &str, sequence: u64, text: &str| {
        let settlement = apply_named_edit(
            &store,
            &scope,
            NamedEdit {
                editor_session_id: &editor_session_id,
                chapter_id: &chapter_id,
                expected_revision_id,
                suffix,
                local_intent_sequence: sequence,
                text,
                proposal_target: None,
            },
        )
        .await;
        let AuthorEditSettlementEffect::AuthoritativeApplied {
            ids,
            author_action_sequence,
            ..
        } = settlement.effect
        else {
            panic!("the Author Edit must apply");
        };
        (ids.revision_id, author_action_sequence)
    };
    let (first_revision, _) = edit("9654", opened.chapter.revision_id.as_ref(), 1, "x").await;
    let before_volume = tree(&store, &scope).await;
    let volume_b = settle_volume(&store, &scope, "9655", "Volume B", 3).await;
    let (_, volume_b_authority) = applied(&volume_b);
    let (later_revision, later_action) = edit("9656", &first_revision, 2, "y").await;

    // The later prose edit is the latest Forward Author Action, so Undo reverses it first.
    let edit_undo = undo_named(
        &store,
        &scope,
        &editor_session_id,
        later_action,
        &later_revision,
        "9657",
    )
    .await;
    assert!(matches!(
        edit_undo.outcome,
        TransitionOutcome::Applied(UndoApplied { source_sequence, records: UndoRecords::Revision { .. }, .. })
            if source_sequence == later_action
    ));
    let after_edit_undo = tree(&store, &scope).await;
    assert_eq!(
        (
            after_edit_undo.tree_revision,
            after_edit_undo.volumes[1].volume_id.as_ref()
        ),
        (4, applied(&volume_b).0.volume_id.as_str())
    );
    let volume_undo = undo_named(
        &store,
        &scope,
        &editor_session_id,
        volume_b_authority.author_action_sequence,
        opened.chapter.revision_id.as_ref(),
        "9658",
    )
    .await;
    let TransitionOutcome::Applied(UndoApplied {
        source_sequence,
        records: UndoRecords::Structure { snapshot_id, .. },
        ..
    }) = volume_undo.outcome
    else {
        panic!("Create Volume Undo must write structure Compensation");
    };
    assert_eq!(source_sequence, volume_b_authority.author_action_sequence);
    let restored = tree(&store, &scope).await;
    assert_eq!(
        (
            restored.tree_revision,
            restored.snapshot.snapshot_id,
            restored.volumes
        ),
        (
            before_volume.tree_revision,
            snapshot_id,
            before_volume.volumes
        )
    );
    // One Compensation for the later edit and one for the Volume.
    assert_eq!(
        compensation_rows(&admin, &scope, "deleteVolume").await,
        serde_json::json!({ "compensations": 2, "removal_receipts": 0 })
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_create_chapter_and_keeps_the_initial_revision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "9660").await;
    let volume_id = titled_volume(&store, &scope, "9661", "Volume A", 1).await;
    let empty_volume = tree(&store, &scope).await;
    let chapter_a = settle_chapter(&store, &scope, "9662", &volume_id, "Chapter A", 2).await;
    let only_a = tree(&store, &scope).await;
    let chapter_b = settle_chapter(&store, &scope, "9663", &volume_id, "Chapter B", 3).await;
    let (
        ChapterCreated {
            chapter_id: a_id, ..
        },
        a_authority,
    ) = applied(&chapter_a);
    let (
        ChapterCreated {
            chapter_id: b_id, ..
        },
        b_authority,
    ) = applied(&chapter_b);
    let editor_session_id = open_session(&store, &scope, "9664").await;
    let OpenChapter::Found(opened) = open_chapter(&store, &scope, &ChapterId::new(a_id.clone()))
        .await
        .unwrap()
    else {
        panic!("Chapter A must open");
    };
    assert_eq!(
        Some(opened.chapter.revision_id.as_ref()),
        a_authority.resulting_revision_id.as_deref()
    );
    for (authority, suffix, prior, removed) in [
        (&b_authority, "9665", &only_a, &b_id),
        (&a_authority, "9666", &empty_volume, &a_id),
    ] {
        let undone = undo_named(
            &store,
            &scope,
            &editor_session_id,
            authority.author_action_sequence,
            opened.chapter.revision_id.as_ref(),
            suffix,
        )
        .await;
        assert!(matches!(
            undone.outcome,
            TransitionOutcome::Applied(UndoApplied { source_sequence, records: UndoRecords::Structure { .. }, .. })
                if source_sequence == authority.author_action_sequence
        ));
        let restored = tree(&store, &scope).await;
        assert_eq!(
            (restored.tree_revision, restored.volumes),
            (prior.tree_revision, prior.volumes.clone())
        );
        assert_eq!(
            open_chapter(&store, &scope, &ChapterId::new(removed.clone()))
                .await
                .unwrap(),
            OpenChapter::Missing
        );
    }
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project is in exact Scope")
            .current_chapter_id,
        None
    );
    assert_eq!(
        compensation_rows(&admin, &scope, "deleteChapter").await,
        serde_json::json!({ "compensations": 2, "removal_receipts": 0 })
    );
    // The Compensation Commits bind no Revision pair, and the initial Revision remains.
    let kept = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.authoritative_revisions
                      WHERE project_id = $1::text::uuid AND revision_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_commits AS made
                       JOIN storyos.author_action_entries AS action
                         ON (action.owner_user_id, action.project_id,
                             action.authoritative_commit_id) =
                            (made.owner_user_id, made.project_id, made.authoritative_commit_id)
                      WHERE made.project_id = $1::text::uuid
                        AND action.disposition = 'compensation'
                        AND made.affected_chapter_id IS NOT NULL
                        AND made.manuscript_object_id IS NULL
                        AND made.prior_revision_id IS NULL
                        AND made.resulting_revision_id IS NULL)",
            &[
                &scope.project_id.as_ref(),
                &a_authority.resulting_revision_id,
            ],
        )
        .await
        .unwrap();
    assert_eq!([0, 1].map(|index| kept.get::<_, i64>(index)), [1, 2]);
}

/// The Compensation Author Actions of the Project and the Receipts of `removal_kind`. A
/// Compensation of a creation is not a forward removal command.
async fn compensation_rows(
    admin: &tokio_postgres::Client,
    scope: &ProjectScope,
    removal_kind: &str,
) -> serde_json::Value {
    let json: String = admin
        .query_one(
            "SELECT jsonb_build_object(
                      'compensations', (SELECT count(*) FROM storyos.author_action_entries
                        WHERE project_id = $1::text::uuid AND disposition = 'compensation'),
                      'removal_receipts', (SELECT count(*) FROM storyos.domain_receipts
                        WHERE project_id = $1::text::uuid AND command_kind = $2)
                    )::text",
            &[&scope.project_id.as_ref(), &removal_kind],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    serde_json::from_str(&json).unwrap()
}
