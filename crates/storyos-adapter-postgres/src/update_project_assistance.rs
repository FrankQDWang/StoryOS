use std::convert::Infallible;

use storyos_application::{
    ProjectAssistanceRecord, ProjectCommandEnvelope, ProjectCommandError, ProjectReadError,
    ProjectScope, UpdateProjectAssistanceInput, UpdateProjectAssistanceSettlement,
};
use storyos_core::{
    AssistanceAvailability, AssistanceBindingPresence,
    UpdateProjectAssistance as CoreUpdateProjectAssistance, UpdateProjectAssistanceApplied,
    UpdateProjectAssistanceConflict, UpdateProjectAssistanceNoEffect, update_project_assistance,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, AppliedResult, Classification,
    CommandIsolation, CommandSpec, LockedProject, MissingAdmission, ProjectAssistanceResponse,
    ProjectCommand, RateLimitedChallenge, ReplayEffect, settle_project_command, unavailable,
};
use crate::{PostgresProjectReader, read_error};

pub(crate) const HOST_FAKE_MODEL_REGISTRATION_REVISION: &str =
    "018f0000-0000-7001-8000-00000000fa01";

impl PostgresProjectReader {
    /// Settles one Project assistance setting and keeps the assistance record for exact retry.
    pub async fn update_project_assistance(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &UpdateProjectAssistanceInput,
    ) -> Result<UpdateProjectAssistanceSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

impl ProjectCommand for UpdateProjectAssistanceInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "updateProjectAssistance",
        applied_result: AppliedResult::AUTHORITATIVE_APPLIED,
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: "project_assistance_updated",
        replay_effect: ReplayEffect::NoQuery,
    };
    type Error = ProjectCommandError;
    type Profile = ActivityOnly;
    type Response = ProjectAssistanceResponse;
    type ZeroEffect = ();
    type Applied = UpdateProjectAssistanceApplied;
    type Plan = ();
    type Effect = UpdateProjectAssistanceApplied;
    type NoEffect = UpdateProjectAssistanceNoEffect;
    type Conflict = UpdateProjectAssistanceConflict;
    type Refusal = Infallible;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        if project.lifecycle == storyos_core::ProjectLifecycle::Archived {
            return Err(ProjectCommandError::BindingConflict);
        }
        let current = read_assistance_record(client, &envelope.project_scope)
            .await
            .map_err(unavailable)?;
        let classified = update_project_assistance(&CoreUpdateProjectAssistance {
            binding: match &current {
                None => AssistanceBindingPresence::Uninitialized,
                Some(record) => AssistanceBindingPresence::Initialized {
                    availability: record.availability,
                    revision: record.revision,
                },
            },
            expected_revision: self.expected_revision,
            requested: self.availability,
        });
        Ok(Classification::project_command(
            classified.map_applied(|applied| (applied, ())),
        ))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ActivitySequences,
        _plan: (),
        applied: UpdateProjectAssistanceApplied,
    ) -> Result<ActivityWrite<UpdateProjectAssistanceApplied>, ProjectCommandError> {
        let (availability, revision) = match applied {
            UpdateProjectAssistanceApplied::Initialized {
                availability,
                revision,
            } => {
                initialize_host_fake_binding(client, envelope, availability, revision).await?;
                (availability, revision)
            }
            UpdateProjectAssistanceApplied::Changed {
                availability,
                revision,
            } => {
                insert_policy_revision(client, envelope, availability, revision).await?;
                (availability, revision)
            }
        };
        Ok(ActivityWrite {
            effect: applied,
            activity: serde_json::json!({
                "availability": availability_text(availability),
                "revision": revision.to_string(),
            }),
        })
    }

    fn decode(
        &self,
        replay: &CommandReplay,
    ) -> Result<UpdateProjectAssistanceApplied, ReplayFault> {
        let availability = parse_availability(&replay.activity_text("availability")?)
            .map_err(|error| ReplayFault::Unavailable(Box::new(error)))?;
        let revision = replay.activity_u64("revision")?;
        Ok(if self.expected_revision == 0 && revision == 1 {
            UpdateProjectAssistanceApplied::Initialized {
                availability,
                revision,
            }
        } else {
            UpdateProjectAssistanceApplied::Changed {
                availability,
                revision,
            }
        })
    }
}

pub(crate) async fn read_assistance_record(
    client: &Client,
    scope: &ProjectScope,
) -> Result<Option<ProjectAssistanceRecord>, ProjectReadError> {
    let row = client
        .query_opt(
            "SELECT policy.availability,
                    policy.policy_revision::text,
                    binding.model_registration_revision::text,
                    identity.processing_destination_identity::text,
                    evidence.evidence_revision::text,
                    binding.project_model_use_binding_revision::text,
                    binding.grant_id::text,
                    decision.external_compatibility_decision::text
               FROM storyos.project_policy_revisions AS policy
               JOIN storyos.processing_destination_identities AS identity
                 ON (identity.owner_user_id, identity.project_id) =
                    (policy.owner_user_id, policy.project_id)
               JOIN storyos.project_external_use_binding_revisions AS binding
                 ON (binding.owner_user_id, binding.project_id) =
                    (policy.owner_user_id, policy.project_id)
               JOIN storyos.processing_destination_identity_evidence_revisions AS evidence
                 ON (evidence.owner_user_id, evidence.project_id,
                     evidence.processing_destination_identity, evidence.evidence_revision) =
                    (binding.owner_user_id, binding.project_id,
                     binding.processing_destination_identity, binding.evidence_revision)
               JOIN storyos.external_contract_compatibility_decisions AS decision
                 ON (decision.owner_user_id, decision.project_id) =
                    (policy.owner_user_id, policy.project_id)
              WHERE policy.owner_user_id = $1::text::uuid
                AND policy.project_id = $2::text::uuid
                AND policy.policy_revision = (
                      SELECT max(current.policy_revision)
                        FROM storyos.project_policy_revisions AS current
                       WHERE current.owner_user_id = policy.owner_user_id
                         AND current.project_id = policy.project_id
                    )",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(ProjectAssistanceRecord {
        availability: parse_availability(&row.get::<_, String>(0))
            .map_err(ProjectReadError::unavailable)?,
        revision: parse_u64(row.get::<_, String>(1)).map_err(ProjectReadError::unavailable)?,
        model_registration_revision: row.get(2),
        processing_destination_identity: row.get(3),
        processing_destination_identity_evidence_revision: parse_u64(row.get::<_, String>(4))
            .map_err(ProjectReadError::unavailable)?,
        project_model_use_binding_revision: row.get(5),
        grant_id: row.get(6),
        external_compatibility_decision: row.get(7),
    }))
}

async fn initialize_host_fake_binding(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    availability: AssistanceAvailability,
    revision: u64,
) -> Result<(), ProjectCommandError> {
    client
        .execute(
            "INSERT INTO storyos.model_registration_revisions
               (model_registration_revision, model_kind)
             VALUES ($1::text::uuid, 'host_fake')
             ON CONFLICT (model_registration_revision) DO NOTHING",
            &[&HOST_FAKE_MODEL_REGISTRATION_REVISION],
        )
        .await
        .map_err(unavailable)?;
    client
        .execute(
            "INSERT INTO storyos.model_registration_heads
               (model_kind, model_registration_revision)
             VALUES ('host_fake', $1::text::uuid)
             ON CONFLICT (model_kind) DO NOTHING",
            &[&HOST_FAKE_MODEL_REGISTRATION_REVISION],
        )
        .await
        .map_err(unavailable)?;
    let identity = Uuid::now_v7().to_string();
    let grant_id = Uuid::now_v7().to_string();
    let binding_revision = Uuid::now_v7().to_string();
    let decision = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.processing_destination_identities
               (owner_user_id, project_id, processing_destination_identity, destination_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'host_fake')",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &identity,
            ],
        )
        .await
        .map_err(unavailable)?;
    client
        .execute(
            "INSERT INTO storyos.processing_destination_identity_evidence_revisions
               (owner_user_id, project_id, processing_destination_identity,
                evidence_revision, evidence_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 1, 'host_fake_boundary')",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &identity,
            ],
        )
        .await
        .map_err(unavailable)?;
    client
        .execute(
            "INSERT INTO storyos.project_destination_grants
               (owner_user_id, project_id, grant_id, processing_destination_identity, grant_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'host_fake_use')",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &grant_id,
                &identity,
            ],
        )
        .await
        .map_err(unavailable)?;
    client
        .execute(
            "INSERT INTO storyos.project_external_use_binding_revisions
               (owner_user_id, project_id, project_model_use_binding_revision,
                processing_destination_identity, evidence_revision, grant_id,
                model_registration_revision)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 1,
                     $5::text::uuid, $6::text::uuid)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &binding_revision,
                &identity,
                &grant_id,
                &HOST_FAKE_MODEL_REGISTRATION_REVISION,
            ],
        )
        .await
        .map_err(unavailable)?;
    client
        .execute(
            "INSERT INTO storyos.external_contract_compatibility_decisions
               (owner_user_id, project_id, external_compatibility_decision,
                project_model_use_binding_revision, decision_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     'host_fake_compatible')",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &decision,
                &binding_revision,
            ],
        )
        .await
        .map_err(unavailable)?;
    insert_policy_revision(client, envelope, availability, revision).await
}

async fn insert_policy_revision(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    availability: AssistanceAvailability,
    revision: u64,
) -> Result<(), ProjectCommandError> {
    client
        .execute(
            "INSERT INTO storyos.project_policy_revisions
               (owner_user_id, project_id, policy_revision, availability, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4, $5::text::uuid)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &revision.to_string(),
                &availability_text(availability),
                &envelope.ids.receipt_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

pub(crate) fn availability_text(availability: AssistanceAvailability) -> &'static str {
    match availability {
        AssistanceAvailability::Available => "available",
        AssistanceAvailability::Unavailable => "unavailable",
    }
}

pub(crate) fn parse_availability(value: &str) -> Result<AssistanceAvailability, std::io::Error> {
    match value {
        "available" => Ok(AssistanceAvailability::Available),
        "unavailable" => Ok(AssistanceAvailability::Unavailable),
        _ => Err(std::io::Error::other(
            "Project assistance availability is damaged",
        )),
    }
}

pub(crate) fn parse_u64(value: String) -> Result<u64, std::io::Error> {
    value
        .parse()
        .map_err(|_| std::io::Error::other("Project assistance revision is damaged"))
}
