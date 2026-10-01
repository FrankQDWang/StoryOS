use storyos_application::ProjectScope;

pub(crate) async fn write_chapter_order(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    parent_volume_id: &str,
    ids: &[String],
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE storyos.manuscript_objects
                SET tree_order = tree_order + 1000000
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND object_kind = 'chapter' AND parent_volume_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &parent_volume_id,
            ],
        )
        .await?;
    if ids.len() <= 1_000_000 {
        let chapter_ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        client
            .execute(
                "UPDATE storyos.manuscript_objects AS chapter
                    SET tree_order = ranked.tree_order
                  FROM unnest($3::text[]) WITH ORDINALITY AS ranked(chapter_id, tree_order)
                 WHERE chapter.owner_user_id = $1::text::uuid AND chapter.project_id = $2::text::uuid
                   AND chapter.manuscript_object_id = ranked.chapter_id::uuid
                   AND chapter.object_kind = 'chapter' AND chapter.parent_volume_id = $4::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &chapter_ids,
                    &parent_volume_id,
                ],
            )
            .await
            ?;
    } else {
        for (index, chapter_id) in ids.iter().enumerate() {
            let tree_order = (index + 1).to_string();
            client
                .execute(
                    "UPDATE storyos.manuscript_objects
                        SET tree_order = $3::text::bigint
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND manuscript_object_id = $4::text::uuid AND object_kind = 'chapter'",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &tree_order,
                        chapter_id,
                    ],
                )
                .await?;
        }
    }
    Ok(())
}
