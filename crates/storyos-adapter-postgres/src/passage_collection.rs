use crate::create_agent_run::context::PassageContextInput;
use storyos_application::CreateAgentRunError;
use storyos_core::{CurrentPassageAssembly, CurrentPassageAssemblyRecord, PassageContextTarget};

pub(crate) async fn assemble(
    client: &tokio_postgres::Client,
    command: &PassageContextInput<'_>,
    source: &CurrentPassageAssembly,
    targets: &[PassageContextTarget],
) -> Result<CurrentPassageAssemblyRecord, CreateAgentRunError> {
    let mut passages = Vec::new();
    for target in targets {
        let (revision, body) = super::create_agent_run::context::load_working_target(
            client,
            command.project_scope,
            &target.chapter_id,
        )
        .await?;
        let members = client
            .query(
                "SELECT manuscript_block_id::text FROM storyos.manuscript_revision_members
              WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid
                AND manuscript_object_id=$3::text::uuid AND revision_id=$4::text::uuid",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &target.chapter_id,
                    &revision,
                ],
            )
            .await
            .map_err(super::create_agent_run::agent_run_database_error)?;
        let ids: std::collections::BTreeSet<String> =
            members.iter().map(|row| row.get(0)).collect();
        let available = revision.as_deref() == Some(&target.base_authoritative_revision_id)
            && target
                .manuscript_block_ids
                .iter()
                .all(|id| ids.contains(id));
        passages.push(CurrentPassageAssembly {
            chapter_id: target.chapter_id.clone(),
            chapter_revision_id: revision.filter(|_| available),
            chapter_body: if available { body } else { String::new() },
            proposal_target_block_ids: Some(target.manuscript_block_ids.clone()),
            ..source.clone()
        });
    }
    Ok(storyos_core::assemble_passage_collection(
        source,
        targets.to_vec(),
        &passages,
    ))
}

/// The provider-neutral passage collection input that the destination receives, if any.
pub(crate) fn passage_input(
    record: &serde_json::Value,
    author_message: &str,
) -> Option<serde_json::Value> {
    if record
        .pointer("/operation_requirement/candidate_target")
        .is_none()
        && record
            .pointer("/operation_requirement/passage_targets")
            .is_none()
    {
        return None;
    }
    let mut input = serde_json::json!({
        "author_message": author_message,
        "source_chapter_id": record["operation_requirement"]["chapter_id"],
        "targets": record["operation_requirement"]["passage_targets"],
        "selected": record["selected"],
    });
    if let Some(target) = record.pointer("/operation_requirement/candidate_target") {
        input["candidate_target"] = target.clone();
    }
    Some(input)
}
