//! The records of one embedding request (ADR 0095 section 3): the dispatch claim that writes one
//! Embedding Attempt before I/O, and the record of its observation. An embedding request is not
//! a Run.

use serde_json::json;
use storyos_application::{
    EmbeddingClaim, EmbeddingDispatchStore, EmbeddingProjection, EmbeddingRecord, EmbeddingUsage,
    ProjectReadError, ProjectScope,
};
use storyos_core::{
    EmbeddingAdmissionFacts, EmbeddingBindingFacts, EmbeddingInput, EmbeddingRefusal,
    EmbeddingRegistration, MANUSCRIPT_EMBEDDING_PURPOSE, admit_embedding,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::project_embedding_binding::CURRENT_EMBEDDING_BINDING;

/// Whether each reference of `input` is in the Project Scope.
async fn source_in_scope(
    client: &Client,
    scope: &ProjectScope,
    input: &EmbeddingInput,
) -> Result<bool, tokio_postgres::Error> {
    let owner = scope.owner_user_id.as_ref();
    let project = scope.project_id.as_ref();
    let row = match input {
        EmbeddingInput::Passage {
            chapter_id,
            revision_id,
            ..
        } => {
            client
                .query_one(
                    "SELECT EXISTS (
                       SELECT 1 FROM storyos.authoritative_revisions AS revision
                         JOIN storyos.manuscript_objects AS chapter
                           ON (chapter.owner_user_id, chapter.project_id,
                               chapter.manuscript_object_id) =
                              (revision.owner_user_id, revision.project_id,
                               revision.manuscript_object_id)
                        WHERE revision.owner_user_id = $1::text::uuid
                          AND revision.project_id = $2::text::uuid
                          AND revision.revision_id::text = $3
                          AND chapter.manuscript_object_id::text = $4
                          AND chapter.object_kind = 'chapter')",
                    &[&owner, &project, revision_id, chapter_id],
                )
                .await?
        }
        EmbeddingInput::Question { tool_call_id, .. } => {
            client
                .query_one(
                    "SELECT EXISTS (
                       SELECT 1 FROM storyos.tool_calls
                        WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                          AND tool_call_id::text = $3)",
                    &[&owner, &project, tool_call_id],
                )
                .await?
        }
    };
    Ok(row.get(/*idx*/ 0))
}

fn read_error(error: impl std::error::Error + Send + Sync + 'static) -> ProjectReadError {
    ProjectReadError::unavailable(error)
}

impl EmbeddingDispatchStore for PostgresProjectReader {
    #[tracing::instrument(skip_all, level = "debug")]
    async fn commit_embedding_claim(
        &self,
        scope: &ProjectScope,
        registration: Option<&'static EmbeddingRegistration>,
        input: &EmbeddingInput,
        projection: &EmbeddingProjection,
    ) -> Result<Result<EmbeddingClaim, EmbeddingRefusal>, ProjectReadError> {
        let transaction = self
            .begin_serializable_project_command_transaction(scope)
            .await
            .map_err(read_error)?;
        let client = &transaction.client;
        let binding = client
            .query_opt(
                CURRENT_EMBEDDING_BINDING,
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?;
        let in_scope = source_in_scope(client, scope, input)
            .await
            .map_err(read_error)?;
        let admission = admit_embedding(&EmbeddingAdmissionFacts {
            registration,
            binding: binding.as_ref().map(|row| EmbeddingBindingFacts {
                registration_revision: row.get(/*idx*/ 1),
                grant_scope: row.get(/*idx*/ 3),
            }),
            source_in_scope: in_scope,
            input,
        });
        let admitted = admission.and_then(|()| {
            binding
                .zip(registration)
                .ok_or(EmbeddingRefusal::BindingAbsent)
        });
        let (binding, registration) = match admitted {
            Ok(admitted) => admitted,
            Err(refusal) => {
                transaction.rollback().await.map_err(read_error)?;
                return Ok(Err(refusal));
            }
        };
        let embedding_attempt_id = Uuid::now_v7().to_string();
        let cause = input.cause();
        let attempt = json!({
            "admission_decision": {
                "decision": "admitted",
                "grant_id": binding.get::<_, &str>(/*idx*/ 2),
                "grant_scope": binding.get::<_, &str>(/*idx*/ 3),
                "use_binding_revision": binding.get::<_, &str>(/*idx*/ 0),
                "destination_identity": binding.get::<_, &str>(/*idx*/ 4),
                "evidence_revision": binding.get::<_, &str>(/*idx*/ 5),
                "embedding_registration": registration.revision,
                "request_bounds": registration.request_bounds,
            },
            "operation_input_snapshot": input.snapshot(),
            "wire_payload_projection": {
                "mapping_revision": projection.mapping_revision,
                "digest": projection.digest,
            },
            "destination_attempt": {
                "destination_attempt_id": Uuid::now_v7().to_string(),
                "settlement": null,
            },
            "outbound_disclosure_event": {
                "event_id": Uuid::now_v7().to_string(),
                "purpose": MANUSCRIPT_EMBEDDING_PURPOSE,
                "source_class": cause.source_class(),
                "state": "outcome_unknown",
            },
        });
        client
            .execute(
                "INSERT INTO storyos.embedding_attempts
                   (owner_user_id, project_id, embedding_attempt_id, cause, grant_id,
                    project_model_use_binding_revision, attempt)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4, $5::text::uuid,
                         $6::text::uuid, $7::text::jsonb)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &embedding_attempt_id,
                    &cause.as_str(),
                    &binding.get::<_, &str>(/*idx*/ 2),
                    &binding.get::<_, &str>(/*idx*/ 0),
                    &attempt.to_string(),
                ],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        Ok(Ok(EmbeddingClaim {
            embedding_attempt_id,
        }))
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn record_embedding(
        &self,
        scope: &ProjectScope,
        claim: &EmbeddingClaim,
        record: &EmbeddingRecord<'_>,
    ) -> Result<(), ProjectReadError> {
        let usage = match record.usage {
            EmbeddingUsage::Reported { input_tokens } => {
                json!({"kind": "reported", "input_tokens": input_tokens})
            }
            EmbeddingUsage::Unknown => json!({"kind": "unknown"}),
        };
        let patch = json!({"usage": usage, "rejection": record.rejection_reason});
        let transaction = self
            .begin_serializable_project_command_transaction(scope)
            .await
            .map_err(read_error)?;
        transaction
            .client
            .execute(
                "UPDATE storyos.embedding_attempts
                    SET settlement = $4,
                        attempt = jsonb_set(attempt, '{destination_attempt,settlement}',
                                            to_jsonb($4::text))
                                  || $5::text::jsonb
                                  || jsonb_build_object('disclosure_confirmation',
                                       jsonb_build_object(
                                         'event_id',
                                         attempt->'outbound_disclosure_event'->'event_id',
                                         'state', $6::text))
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND embedding_attempt_id = $3::text::uuid AND settlement IS NULL",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &claim.embedding_attempt_id,
                    &record.settlement.as_str(),
                    &patch.to_string(),
                    &record.settlement.disclosure(),
                ],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)
    }
}

#[cfg(test)]
#[path = "embedding_attempt_tests.rs"]
mod tests;
