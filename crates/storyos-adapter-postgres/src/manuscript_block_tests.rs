use storyos_core::{ManuscriptBlock, ManuscriptBlockKind};
use tokio_postgres::NoTls;

use super::{
    display_body_from_stored, parse_versioned_payload, persist_canonical_bytes,
    persist_revision_members_from_blocks,
};

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";
const CHAPTER: &str = "018f0000-0000-7001-8000-000000000003";
const EXISTING_BLOCK: &str = "018f0000-0000-7001-8000-0000000000b1";
const SECOND_BLOCK: &str = "018f0000-0000-7001-8000-0000000000b2";
const THIRD_BLOCK: &str = "018f0000-0000-7001-8000-0000000000b3";
const PAYLOAD: &str = "018f0000-0000-7001-8000-0000000000c4";
const REVISION: &str = "018f0000-0000-7001-8000-0000000000c5";

#[test]
fn two_paragraphs_including_empty_right_roundtrip_through_canonical_bytes() {
    let blocks = vec![
        ManuscriptBlock {
            manuscript_block_id: "018f0000-0000-7001-8000-0000000000b1".to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: "Hello".to_owned(),
        },
        ManuscriptBlock {
            manuscript_block_id: "018f0000-0000-7001-8000-0000000000b2".to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: String::new(),
        },
    ];
    let stored = persist_canonical_bytes(&blocks);
    let parsed = parse_versioned_payload(&stored).expect("versioned payload");
    assert_eq!(parsed, blocks);
    assert_eq!(display_body_from_stored(&stored, &parsed), "Hello\n");
}

#[test]
fn one_heading_roundtrips_through_versioned_payload_bytes() {
    let blocks = vec![ManuscriptBlock {
        manuscript_block_id: "018f0000-0000-7001-8000-0000000000b1".to_owned(),
        block_kind: ManuscriptBlockKind::Heading,
        text: "Hello".to_owned(),
    }];
    let stored = persist_canonical_bytes(&blocks);
    let parsed = parse_versioned_payload(&stored).expect("versioned payload");
    assert_eq!(parsed, blocks);
    assert_eq!(display_body_from_stored(&stored, &parsed), "Hello");
}

#[derive(Debug, Eq, PartialEq)]
struct OrderedRevisionMember {
    manuscript_block_id: String,
    block_order: String,
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn several_ordered_blocks_persist_complete_payload_and_member_rows() {
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
    let blocks = vec![
        ManuscriptBlock {
            manuscript_block_id: EXISTING_BLOCK.to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: "Alpha".to_owned(),
        },
        ManuscriptBlock {
            manuscript_block_id: SECOND_BLOCK.to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: "Beta".to_owned(),
        },
        ManuscriptBlock {
            manuscript_block_id: THIRD_BLOCK.to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: "Gamma".to_owned(),
        },
    ];
    let stored = persist_canonical_bytes(&blocks);
    transaction
        .execute(
            "INSERT INTO storyos.authoritative_payloads
               (owner_user_id, project_id, payload_id, canonical_bytes)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, convert_to($4, 'UTF8'))",
            &[&USER, &PROJECT, &PAYLOAD, &stored],
        )
        .await
        .unwrap();
    transaction
        .execute(
            "INSERT INTO storyos.authoritative_revisions
               (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid)",
            &[&USER, &PROJECT, &CHAPTER, &REVISION, &PAYLOAD],
        )
        .await
        .unwrap();
    super::membership_sql_statement_count::reset();
    let inserted = persist_revision_members_from_blocks(
        &transaction,
        USER,
        PROJECT,
        CHAPTER,
        REVISION,
        &blocks,
    )
    .await
    .unwrap();
    assert_eq!(super::membership_sql_statement_count::take(), 2);
    assert_eq!(inserted, 3);
    let payload = transaction
        .query_one(
            "SELECT convert_from(payload.canonical_bytes, 'UTF8')
               FROM storyos.authoritative_payloads AS payload
              WHERE payload.owner_user_id = $1::text::uuid
                AND payload.project_id = $2::text::uuid
                AND payload.payload_id = $3::text::uuid",
            &[&USER, &PROJECT, &PAYLOAD],
        )
        .await
        .unwrap()
        .get::<_, String>(0);
    assert_eq!(payload, stored);
    assert_eq!(
        parse_versioned_payload(&payload).expect("versioned payload"),
        blocks
    );
    let members: Vec<OrderedRevisionMember> = transaction
        .query(
            "SELECT member.manuscript_block_id::text, member.block_order::text
               FROM storyos.manuscript_revision_members AS member
              WHERE member.owner_user_id = $1::text::uuid
                AND member.project_id = $2::text::uuid
                AND member.manuscript_object_id = $3::text::uuid
                AND member.revision_id = $4::text::uuid
              ORDER BY member.block_order",
            &[&USER, &PROJECT, &CHAPTER, &REVISION],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| OrderedRevisionMember {
            manuscript_block_id: row.get(0),
            block_order: row.get(1),
        })
        .collect();
    assert_eq!(
        members,
        vec![
            OrderedRevisionMember {
                manuscript_block_id: EXISTING_BLOCK.to_owned(),
                block_order: "1".to_owned(),
            },
            OrderedRevisionMember {
                manuscript_block_id: SECOND_BLOCK.to_owned(),
                block_order: "2".to_owned(),
            },
            OrderedRevisionMember {
                manuscript_block_id: THIRD_BLOCK.to_owned(),
                block_order: "3".to_owned(),
            },
        ]
    );
    transaction.rollback().await.unwrap();
}
