use crate::create_agent_run::context::PassageContextInput;
use storyos_application::CreateAgentRunError;
use storyos_core::{HumanChapterReference, HumanPassageReference, PassageContextTarget};

pub(crate) async fn resolve(
    client: &tokio_postgres::Client,
    command: &PassageContextInput<'_>,
    references: &[HumanPassageReference],
) -> Result<Option<Vec<PassageContextTarget>>, CreateAgentRunError> {
    let mut targets: Vec<PassageContextTarget> = Vec::new();
    let mut ordered =
        std::collections::BTreeMap::<String, std::collections::BTreeMap<i64, String>>::new();
    let mut total = 0;
    for reference in references {
        total += reference.last - reference.first + 1;
        if total > storyos_core::CONTEXT_ITEM_TOKEN_LIMIT as usize + 1 {
            return Ok(None);
        }
        let (title, chapter, offset) = match &reference.chapter {
            HumanChapterReference::Title(title) => (Some(title.as_str()), None, 0),
            HumanChapterReference::Ordinal(ordinal) => (None, None, (*ordinal - 1) as i64),
            HumanChapterReference::Current => (None, Some(command.chapter_id), 0),
        };
        let rows = client
            .query(
                "SELECT chapter.manuscript_object_id::text, head.current_revision_id::text
               FROM storyos.manuscript_objects AS chapter
               JOIN storyos.manuscript_objects AS volume
                 ON (volume.owner_user_id, volume.project_id, volume.manuscript_object_id) =
                    (chapter.owner_user_id, chapter.project_id, chapter.parent_volume_id)
               JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id)
              WHERE chapter.owner_user_id=$1::text::uuid AND chapter.project_id=$2::text::uuid
                AND chapter.object_kind='chapter' AND volume.object_kind='volume'
                AND ($3::text IS NULL OR chapter.title=$3)
                AND ($4::text IS NULL OR chapter.manuscript_object_id=$4::text::uuid)
                AND NOT EXISTS (SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                    WHERE (removal.owner_user_id, removal.project_id, removal.chapter_id) =
                          (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id))
                AND NOT EXISTS (SELECT 1 FROM storyos.volume_removal_decisions AS removal
                    WHERE (removal.owner_user_id, removal.project_id, removal.volume_id) =
                          (volume.owner_user_id, volume.project_id, volume.manuscript_object_id))
              ORDER BY volume.tree_order, chapter.tree_order LIMIT 2 OFFSET $5",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &title,
                    &chapter,
                    &offset,
                ],
            )
            .await
            .map_err(super::create_agent_run::agent_run_database_error)?;
        if rows.is_empty() || (title.is_some() && rows.len() != 1) {
            return Ok(None);
        }
        let chapter_id: String = rows[0].get(0);
        let revision_id: String = rows[0].get(1);
        let limit = (reference.last - reference.first + 1) as i64;
        let offset = (reference.first - 1) as i64;
        let rows = client.query(
            "SELECT manuscript_block_id::text, block_order::bigint FROM storyos.manuscript_revision_members
              WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid
                AND manuscript_object_id=$3::text::uuid AND revision_id=$4::text::uuid
              ORDER BY CASE WHEN $5 THEN -block_order ELSE block_order END LIMIT $6 OFFSET $7",
            &[&command.project_scope.owner_user_id.as_ref(), &command.project_scope.project_id.as_ref(), &chapter_id, &revision_id, &reference.from_end, &limit, &offset],
        ).await.map_err(super::create_agent_run::agent_run_database_error)?;
        if rows.len() != limit as usize {
            return Ok(None);
        }
        ordered
            .entry(chapter_id.clone())
            .or_default()
            .extend(rows.into_iter().map(|row| (row.get(1), row.get(0))));
        if !targets.iter().any(|target| target.chapter_id == chapter_id) {
            targets.push(PassageContextTarget {
                chapter_id,
                base_authoritative_revision_id: revision_id,
                manuscript_block_ids: Vec::new(),
            });
        }
    }
    for target in &mut targets {
        target.manuscript_block_ids = ordered
            .remove(&target.chapter_id)
            .expect("resolved Chapter")
            .into_values()
            .collect();
    }
    Ok(Some(targets))
}
