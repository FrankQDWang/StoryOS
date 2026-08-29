use storyos_core::{ManuscriptBlock, upgrade_legacy_manuscript};
use tokio_postgres::GenericClient;
use uuid::Uuid;

pub(crate) async fn insert_paragraph_block(
    client: &impl GenericClient,
    owner_user_id: &str,
    project_id: &str,
    chapter_id: &str,
    revision_id: &str,
    manuscript_block_id: &str,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO storyos.manuscript_blocks
               (owner_user_id, project_id, manuscript_block_id, manuscript_object_id, block_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'paragraph')",
            &[
                &owner_user_id,
                &project_id,
                &manuscript_block_id,
                &chapter_id,
            ],
        )
        .await?;
    client
        .execute(
            "INSERT INTO storyos.manuscript_revision_members
               (owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id,
                block_order)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid,
                     1)",
            &[
                &owner_user_id,
                &project_id,
                &chapter_id,
                &revision_id,
                &manuscript_block_id,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn load_revision_blocks(
    client: &impl GenericClient,
    owner_user_id: &str,
    project_id: &str,
    chapter_id: &str,
    revision_id: &str,
    body: &str,
) -> Result<Vec<ManuscriptBlock>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT block.manuscript_block_id::text
               FROM storyos.manuscript_revision_members AS member
               JOIN storyos.manuscript_blocks AS block
                 ON (block.owner_user_id, block.project_id, block.manuscript_block_id) =
                    (member.owner_user_id, member.project_id, member.manuscript_block_id)
              WHERE member.owner_user_id = $1::text::uuid
                AND member.project_id = $2::text::uuid
                AND member.manuscript_object_id = $3::text::uuid
                AND member.revision_id = $4::text::uuid
              ORDER BY member.block_order",
            &[&owner_user_id, &project_id, &chapter_id, &revision_id],
        )
        .await?;
    let Some(row) = rows.first() else {
        return Ok(Vec::new());
    };
    Ok(upgrade_legacy_manuscript(body, &row.get::<_, String>(0)).blocks)
}

pub(crate) async fn load_or_upgrade_blocks(
    client: &impl GenericClient,
    owner_user_id: &str,
    project_id: &str,
    chapter_id: &str,
    revision_id: &str,
    body: &str,
) -> Result<Vec<ManuscriptBlock>, tokio_postgres::Error> {
    let blocks = load_revision_blocks(
        client,
        owner_user_id,
        project_id,
        chapter_id,
        revision_id,
        body,
    )
    .await?;
    if !blocks.is_empty() {
        return Ok(blocks);
    }
    let manuscript_block_id = Uuid::now_v7().to_string();
    insert_paragraph_block(
        client,
        owner_user_id,
        project_id,
        chapter_id,
        revision_id,
        &manuscript_block_id,
    )
    .await?;
    Ok(upgrade_legacy_manuscript(body, &manuscript_block_id).blocks)
}

pub(crate) async fn copy_or_upgrade_revision_members(
    client: &impl GenericClient,
    owner_user_id: &str,
    project_id: &str,
    chapter_id: &str,
    from_revision_id: &str,
    to_revision_id: &str,
) -> Result<u64, tokio_postgres::Error> {
    for already_upgraded in [false, true] {
        let copied = client
            .execute(
                "INSERT INTO storyos.manuscript_revision_members
                   (owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id,
                    block_order)
                 SELECT owner_user_id, project_id, manuscript_object_id, $5::text::uuid,
                        manuscript_block_id, block_order
                   FROM storyos.manuscript_revision_members
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND manuscript_object_id = $3::text::uuid
                    AND revision_id = $4::text::uuid",
                &[
                    &owner_user_id,
                    &project_id,
                    &chapter_id,
                    &from_revision_id,
                    &to_revision_id,
                ],
            )
            .await?;
        if copied > 0 {
            return Ok(copied);
        }
        if already_upgraded {
            return Ok(0);
        }
        let manuscript_block_id = Uuid::now_v7().to_string();
        insert_paragraph_block(
            client,
            owner_user_id,
            project_id,
            chapter_id,
            from_revision_id,
            &manuscript_block_id,
        )
        .await?;
    }
    Ok(0)
}
