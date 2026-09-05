use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, CreateProjectChallengeBinding, CreateProjectCommand,
    EditorClientBinding, IssueCreateProjectChallenge, ManuscriptSearchCompleteness,
    ManuscriptSearchRequest, ManuscriptSearchSelection, ProjectId, ProjectScope, SearchManuscript,
    UserId, create_project, issue_create_project_challenge, search_manuscript,
};
use storyos_core::{ManuscriptBlock, ManuscriptBlockKind};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B: &str = "018f0000-0000-7001-8000-000000000102";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";

fn issue_request(idempotency_key: &str, project_suffix: &str) -> IssueCreateProjectChallenge {
    let digest = format!("sha256:storyos.command.createProject.jcs.v1:{project_suffix:0<64}");
    IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(USER_A),
            prospective_project_id: ProjectId::new(format!(
                "018f0000-0000-7001-8000-00000000{project_suffix}"
            )),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects".to_owned(),
            command_schema: "storyos.command.create-project.request.v1".to_owned(),
            command_kind: "createProject".to_owned(),
            create_input_digest: format!("{project_suffix:0<64}"),
            canonical_command_digest: digest,
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: format!("sha256:nonce-{project_suffix}"),
    }
}

fn command(
    binding: CreateProjectChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
) -> CreateProjectCommand {
    let project_scope = ProjectScope::new(
        binding.owner_user_id.clone(),
        binding.prospective_project_id.clone(),
    );
    CreateProjectCommand {
        project_scope,
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: br#"{"title":"Search Novel"}"#.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        title: "Search Novel".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

fn manuscript_query(query_text: &str) -> ManuscriptSearchRequest {
    ManuscriptSearchRequest {
        query_text: query_text.to_owned(),
        selection: ManuscriptSearchSelection::Manuscript,
        required_watermark: None,
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn search_rebuilds_from_canonical_facts_without_a_write_and_separates_not_ready_from_zero() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let issue = issue_request("018f0000-0000-7001-8000-000000000d90", "0d90");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut binding = issue.binding.clone();
    binding.prospective_project_id = issued.prospective_project_id.clone();
    binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &command(binding.clone(), &issue.nonce_digest, "0d91"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        binding.owner_user_id.clone(),
        binding.prospective_project_id.clone(),
    );

    let first = search_manuscript(&store, &scope, &manuscript_query("alpha"))
        .await
        .unwrap();
    let SearchManuscript::Ready(page) = &first else {
        panic!("an empty active Project must return a ready search page");
    };
    assert_eq!(page.project_scope, scope);
    assert_eq!(page.items, Vec::new());
    assert_eq!(page.completeness, ManuscriptSearchCompleteness::Complete);
    assert_eq!(page.lag, 0);
    let watermark = page.projection_watermark;

    let rebuilt = search_manuscript(&store, &scope, &manuscript_query("alpha"))
        .await
        .unwrap();
    assert_eq!(rebuilt, first);

    assert_eq!(
        search_manuscript(
            &store,
            &ProjectScope::new(UserId::new(USER_B), scope.project_id.clone()),
            &manuscript_query("alpha"),
        )
        .await
        .unwrap(),
        SearchManuscript::Missing
    );
    assert_eq!(
        search_manuscript(
            &store,
            &ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT_B)),
            &manuscript_query("alpha"),
        )
        .await
        .unwrap(),
        SearchManuscript::Missing
    );

    let not_ready = search_manuscript(
        &store,
        &scope,
        &ManuscriptSearchRequest {
            query_text: "alpha".to_owned(),
            selection: ManuscriptSearchSelection::Manuscript,
            required_watermark: Some(watermark + 1),
        },
    )
    .await
    .unwrap();
    match not_ready {
        SearchManuscript::ProjectionNotReady {
            projection_watermark,
            required_watermark,
            ..
        } => {
            assert_eq!(projection_watermark, watermark);
            assert_eq!(required_watermark, watermark + 1);
        }
        other => panic!("expected projection-not-ready, got {other:?}"),
    }

    let after = search_manuscript(&store, &scope, &manuscript_query("alpha"))
        .await
        .unwrap();
    let SearchManuscript::Ready(after_page) = after else {
        panic!("a later search must remain ready");
    };
    assert_eq!(after_page.projection_watermark, watermark);

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER_A, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(
        search_manuscript(&store, &scope, &manuscript_query("alpha"))
            .await
            .unwrap(),
        SearchManuscript::Missing
    );
}

const FIXTURE_USER: &str = "018f0000-0000-7001-8000-000000000001";
const FIXTURE_PROJECT: &str = "018f0000-0000-7001-8000-000000000002";
const SEARCH_VOLUME: &str = "018f0000-0000-7001-8000-000000000e10";
const MATCH_CHAPTER: &str = "018f0000-0000-7001-8000-000000000e11";
const MISMATCH_CHAPTER: &str = "018f0000-0000-7001-8000-000000000e12";
const LEGACY_CHAPTER: &str = "018f0000-0000-7001-8000-000000000e13";
const MATCH_PAYLOAD: &str = "018f0000-0000-7001-8000-000000000e14";
const MISMATCH_PAYLOAD: &str = "018f0000-0000-7001-8000-000000000e15";
const LEGACY_PAYLOAD: &str = "018f0000-0000-7001-8000-000000000e16";
const MATCH_REVISION: &str = "018f0000-0000-7001-8000-000000000e17";
const MISMATCH_REVISION: &str = "018f0000-0000-7001-8000-000000000e18";
const LEGACY_REVISION: &str = "018f0000-0000-7001-8000-000000000e19";
const BLOCK_A: &str = "018f0000-0000-7001-8000-000000000e21";
const BLOCK_B: &str = "018f0000-0000-7001-8000-000000000e22";
const BLOCK_C: &str = "018f0000-0000-7001-8000-000000000e23";
const BLOCK_D: &str = "018f0000-0000-7001-8000-000000000e24";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn several_live_chapters_load_membership_in_one_statement() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (mut runtime, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let transaction = runtime.transaction().await.unwrap();
    transaction
        .batch_execute(
            "SET LOCAL storyos.owner_user_id = '018f0000-0000-7001-8000-000000000001';
             SET LOCAL storyos.project_id = '018f0000-0000-7001-8000-000000000002';",
        )
        .await
        .unwrap();
    let matched = vec![
        ManuscriptBlock {
            manuscript_block_id: BLOCK_A.to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: "Alpha".to_owned(),
        },
        ManuscriptBlock {
            manuscript_block_id: BLOCK_B.to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: "Beta".to_owned(),
        },
    ];
    let mismatched = vec![
        ManuscriptBlock {
            manuscript_block_id: BLOCK_C.to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: "Gamma".to_owned(),
        },
        ManuscriptBlock {
            manuscript_block_id: BLOCK_D.to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: "Delta".to_owned(),
        },
    ];
    let matched_payload = crate::manuscript_block::persist_canonical_bytes(&matched);
    let mismatched_payload = crate::manuscript_block::persist_canonical_bytes(&mismatched);
    transaction
        .execute(
            "INSERT INTO storyos.manuscript_objects
               (owner_user_id, project_id, manuscript_object_id, object_kind, title, tree_order)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'volume', 'Search Volume', 50)",
            &[&FIXTURE_USER, &FIXTURE_PROJECT, &SEARCH_VOLUME],
        )
        .await
        .unwrap();
    for (chapter_id, title, order) in [
        (MATCH_CHAPTER, "Match Chapter", "1"),
        (MISMATCH_CHAPTER, "Mismatch Chapter", "2"),
        (LEGACY_CHAPTER, "Legacy Chapter", "3"),
    ] {
        transaction
            .execute(
                "INSERT INTO storyos.manuscript_objects
                   (owner_user_id, project_id, manuscript_object_id, object_kind, title, tree_order,
                    parent_volume_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'chapter', $4, $5::text::bigint,
                         $6::text::uuid)",
                &[
                    &FIXTURE_USER,
                    &FIXTURE_PROJECT,
                    &chapter_id,
                    &title,
                    &order,
                    &SEARCH_VOLUME,
                ],
            )
            .await
            .unwrap();
    }
    transaction
        .execute(
            "INSERT INTO storyos.authoritative_payloads
               (owner_user_id, project_id, payload_id, canonical_bytes)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, convert_to($4, 'UTF8')),
                    ($1::text::uuid, $2::text::uuid, $5::text::uuid, convert_to($6, 'UTF8')),
                    ($1::text::uuid, $2::text::uuid, $7::text::uuid, convert_to('Gamma', 'UTF8'))",
            &[
                &FIXTURE_USER,
                &FIXTURE_PROJECT,
                &MATCH_PAYLOAD,
                &matched_payload,
                &MISMATCH_PAYLOAD,
                &mismatched_payload,
                &LEGACY_PAYLOAD,
            ],
        )
        .await
        .unwrap();
    transaction
        .execute(
            "INSERT INTO storyos.authoritative_revisions
               (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid),
                    ($1::text::uuid, $2::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid),
                    ($1::text::uuid, $2::text::uuid, $9::text::uuid, $10::text::uuid, $11::text::uuid)",
            &[
                &FIXTURE_USER,
                &FIXTURE_PROJECT,
                &MATCH_CHAPTER,
                &MATCH_REVISION,
                &MATCH_PAYLOAD,
                &MISMATCH_CHAPTER,
                &MISMATCH_REVISION,
                &MISMATCH_PAYLOAD,
                &LEGACY_CHAPTER,
                &LEGACY_REVISION,
                &LEGACY_PAYLOAD,
            ],
        )
        .await
        .unwrap();
    transaction
        .execute(
            "INSERT INTO storyos.authoritative_heads
               (owner_user_id, project_id, manuscript_object_id, current_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid),
                    ($1::text::uuid, $2::text::uuid, $5::text::uuid, $6::text::uuid),
                    ($1::text::uuid, $2::text::uuid, $7::text::uuid, $8::text::uuid)",
            &[
                &FIXTURE_USER,
                &FIXTURE_PROJECT,
                &MATCH_CHAPTER,
                &MATCH_REVISION,
                &MISMATCH_CHAPTER,
                &MISMATCH_REVISION,
                &LEGACY_CHAPTER,
                &LEGACY_REVISION,
            ],
        )
        .await
        .unwrap();
    transaction
        .execute(
            "INSERT INTO storyos.manuscript_blocks
               (owner_user_id, project_id, manuscript_block_id, manuscript_object_id, block_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'paragraph'),
                    ($1::text::uuid, $2::text::uuid, $5::text::uuid, $4::text::uuid, 'paragraph'),
                    ($1::text::uuid, $2::text::uuid, $6::text::uuid, $7::text::uuid, 'paragraph'),
                    ($1::text::uuid, $2::text::uuid, $8::text::uuid, $7::text::uuid, 'paragraph')",
            &[
                &FIXTURE_USER,
                &FIXTURE_PROJECT,
                &BLOCK_A,
                &MATCH_CHAPTER,
                &BLOCK_B,
                &BLOCK_C,
                &MISMATCH_CHAPTER,
                &BLOCK_D,
            ],
        )
        .await
        .unwrap();
    transaction
        .execute(
            "INSERT INTO storyos.manuscript_revision_members
               (owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id,
                block_order)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid, 1),
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $6::text::uuid, 2),
                    ($1::text::uuid, $2::text::uuid, $7::text::uuid, $8::text::uuid, $9::text::uuid, 1),
                    ($1::text::uuid, $2::text::uuid, $7::text::uuid, $8::text::uuid, $10::text::uuid, 2)",
            &[
                &FIXTURE_USER,
                &FIXTURE_PROJECT,
                &MATCH_CHAPTER,
                &MATCH_REVISION,
                &BLOCK_A,
                &BLOCK_B,
                &MISMATCH_CHAPTER,
                &MISMATCH_REVISION,
                &BLOCK_D,
                &BLOCK_C,
            ],
        )
        .await
        .unwrap();
    let scope = ProjectScope::new(UserId::new(FIXTURE_USER), ProjectId::new(FIXTURE_PROJECT));
    crate::manuscript_block::revision_member_read_sql_statement_count::reset();
    let chapters = super::read_live_chapter_blocks(
        &transaction,
        &scope,
        super::LiveChapterReadExtent::AllChapters,
    )
    .await
    .unwrap();
    assert_eq!(
        crate::manuscript_block::revision_member_read_sql_statement_count::take(),
        1
    );
    let match_chapter = chapters
        .iter()
        .find(|chapter| chapter.chapter_id == ChapterId::new(MATCH_CHAPTER))
        .expect("matched Chapter");
    let mismatch_chapter = chapters
        .iter()
        .find(|chapter| chapter.chapter_id == ChapterId::new(MISMATCH_CHAPTER))
        .expect("mismatched Chapter");
    let legacy_chapter = chapters
        .iter()
        .find(|chapter| chapter.chapter_id == ChapterId::new(LEGACY_CHAPTER))
        .expect("legacy Chapter");
    assert_eq!(
        match_chapter.blocks,
        vec![
            super::ManuscriptSearchBlockFact {
                manuscript_block_id: BLOCK_A.to_owned(),
                text: "Alpha".to_owned(),
            },
            super::ManuscriptSearchBlockFact {
                manuscript_block_id: BLOCK_B.to_owned(),
                text: "Beta".to_owned(),
            },
        ]
    );
    assert_eq!(mismatch_chapter.blocks, Vec::new());
    assert_eq!(legacy_chapter.blocks, Vec::new());
    assert!(match_chapter.chapter_order < mismatch_chapter.chapter_order);
    assert!(mismatch_chapter.chapter_order < legacy_chapter.chapter_order);

    let current_chapter_id = ChapterId::new(MATCH_CHAPTER);
    crate::manuscript_block::revision_member_read_sql_statement_count::reset();
    let current_chapters = super::read_live_chapter_blocks(
        &transaction,
        &scope,
        super::LiveChapterReadExtent::OneChapter {
            chapter_id: &current_chapter_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        crate::manuscript_block::revision_member_read_sql_statement_count::take(),
        1
    );
    assert_eq!(chapters.len(), 3);
    assert_eq!(
        current_chapters.len(),
        1,
        "CurrentChapter must return one Chapter row, not the full live manuscript"
    );
    assert_eq!(
        current_chapters[0].chapter_id,
        ChapterId::new(MATCH_CHAPTER)
    );
    assert_eq!(current_chapters[0].blocks, match_chapter.blocks);
    transaction.rollback().await.unwrap();
}
