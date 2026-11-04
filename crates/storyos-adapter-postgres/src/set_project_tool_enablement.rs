//! Project Tool Enablement of the research Tool Registration (ADR 0083). An enabled revision
//! names one compatibility Decision of Project-scoped binding records: the Processing Destination
//! Identity of the server, the Project Destination Grant `research_tool_v1`, the use binding with
//! the Credential Reference, and the Decision from the pinned active Registration revision. The
//! Server performs no MCP exchange; the Worker checks the pinned digests before each dispatch.

use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, ProjectReadError, ProjectScope,
    SetProjectToolEnablementInput, SetProjectToolEnablementSettlement,
};
use storyos_contracts::CommandKind;
use storyos_core::{
    CurrentEnablement, EnablementBinding, GrantScope, PinnedEnablement, ProjectToolState,
    ResearchToolOffer, SetProjectToolEnablement as CoreSetProjectToolEnablement,
    SetProjectToolEnablementApplied, ToolEnablement, ToolRegistrationLifecycle,
    set_project_tool_enablement,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, AppliedVariant, Classification,
    CommandResponse, CommandSpec, CommandWrite, LockedProject, ProjectCommand, ReplayEffect,
    settle_project_command, unavailable,
};
use crate::{PostgresProjectReader, read_error, set_scope};

impl PostgresProjectReader {
    /// Settles one enablement setting of a Tool Registration for a Project.
    pub async fn set_project_tool_enablement(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &SetProjectToolEnablementInput,
    ) -> Result<SetProjectToolEnablementSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }

    /// The research tool state of the Project for the assistance query. `offer` is the research
    /// destination that the deployment offers.
    pub async fn project_tool_state(
        &self,
        scope: &ProjectScope,
        offer: Option<&ResearchToolOffer>,
    ) -> Result<ProjectToolState, ProjectReadError> {
        let Some(offer) = offer else {
            return Ok(ProjectToolState::NotOffered);
        };
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let offered = transaction
            .query_opt(
                "SELECT lifecycle FROM storyos.tool_registration_revisions
                  WHERE tool_registration_revision = $1::text::uuid",
                &[&offer.registration.revision],
            )
            .await
            .map_err(read_error)?
            .and_then(|row| ToolRegistrationLifecycle::parse(row.get(/*idx*/ 0)))
            .ok_or_else(|| {
                ProjectReadError::unavailable(std::io::Error::other(
                    "the offered Tool Registration revision is missing",
                ))
            })?;
        let row = transaction
            .query_opt(
                "SELECT enablement.enablement, registration.lifecycle
                   FROM storyos.project_tool_enablements AS enablement
                   LEFT JOIN storyos.external_contract_compatibility_decisions AS decision
                     ON (decision.owner_user_id, decision.project_id,
                         decision.external_compatibility_decision) =
                        (enablement.owner_user_id, enablement.project_id,
                         enablement.external_compatibility_decision)
                   LEFT JOIN storyos.project_external_use_binding_revisions AS binding
                     ON (binding.owner_user_id, binding.project_id,
                         binding.project_model_use_binding_revision) =
                        (decision.owner_user_id, decision.project_id,
                         decision.project_model_use_binding_revision)
                   LEFT JOIN storyos.tool_registration_revisions AS registration
                     ON registration.tool_registration_revision =
                        binding.tool_registration_revision
                  WHERE enablement.owner_user_id = $1::text::uuid
                    AND enablement.project_id = $2::text::uuid
                    AND enablement.registration_id = $3
                  ORDER BY enablement.enablement_revision DESC
                  LIMIT 1",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &offer.registration.id,
                ],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        let current = row
            .map(|row| {
                let lifecycle = row
                    .get::<_, Option<&str>>(/*idx*/ 1)
                    .and_then(ToolRegistrationLifecycle::parse);
                match (ToolEnablement::parse(row.get(/*idx*/ 0)), lifecycle) {
                    (Some(ToolEnablement::Disabled), _) => Ok(PinnedEnablement::Disabled),
                    (Some(ToolEnablement::Enabled), Some(lifecycle)) => {
                        Ok(PinnedEnablement::Enabled(lifecycle))
                    }
                    (Some(ToolEnablement::Enabled) | None, _) => {
                        Err(ProjectReadError::unavailable(std::io::Error::other(
                            "Project Tool Enablement is damaged",
                        )))
                    }
                }
            })
            .transpose()?;
        Ok(ProjectToolState::offered(offered, current))
    }
}

/// The Decision of the latest binding records when they match the offer. An enablement reuses
/// it; otherwise it writes new binding records.
type ReusableDecision = Option<String>;

impl ProjectCommand for SetProjectToolEnablementInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: CommandKind::SetProjectToolEnablement,
        applied: &[AppliedVariant::NoAuthorAction],
        activity_kind: "project_tool_enablement_set",
        replay_effect: ReplayEffect::NoQuery,
        response: CommandResponse::Project,
        contention_refusal: None,
        receipt_payload: "{}",
    };
    type Error = ProjectCommandError;
    type Profile = ActivityOnly;
    type Applied = SetProjectToolEnablementApplied;
    type Plan = ReusableDecision;
    type Settlement = SetProjectToolEnablementSettlement;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let mut command = CoreSetProjectToolEnablement {
            offered: None,
            current: None,
            requested: self.requested,
        };
        let mut reusable = None;
        if let Some(offer) = offered(self) {
            crate::tool_registration::insert_registration_rows(client, offer.registration)
                .await
                .map_err(unavailable)?;
            let facts = read_enablement_facts(client, &envelope.project_scope, offer).await?;
            command.offered = Some(facts.lifecycle);
            command.current = facts.current;
            reusable = facts.reusable;
        }
        Ok(Classification::project_command(
            set_project_tool_enablement(&command).map_applied(|applied| (applied, reusable)),
        ))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ActivitySequences,
        reusable: ReusableDecision,
        applied: SetProjectToolEnablementApplied,
    ) -> Result<CommandWrite<'_, Self>, ProjectCommandError> {
        let decision = match (applied.enablement, reusable) {
            (ToolEnablement::Disabled, _) => None,
            (ToolEnablement::Enabled, Some(decision)) => Some(decision),
            (ToolEnablement::Enabled, None) => {
                let offer = offered(self).ok_or(ProjectCommandError::MissingProject)?;
                Some(
                    insert_tool_binding(client, envelope, offer)
                        .await
                        .map_err(unavailable)?,
                )
            }
        };
        client
            .execute(
                "INSERT INTO storyos.project_tool_enablements
                   (owner_user_id, project_id, registration_id, enablement_revision, enablement,
                    external_compatibility_decision, receipt_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3, $4::text::numeric, $5,
                         $6::text::uuid, $7::text::uuid)",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &self.registration_id,
                    &applied.revision.to_string(),
                    &applied.enablement.as_str(),
                    &decision,
                    &envelope.ids.receipt_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(CommandWrite {
            profile: ActivityWrite {
                effect: applied,
                activity: serde_json::json!({
                    "enablement": applied.enablement.as_str(),
                    "revision": applied.revision.to_string(),
                }),
            },
            after_authority: None,
        })
    }

    fn decode(
        &self,
        replay: &CommandReplay,
    ) -> Result<SetProjectToolEnablementApplied, ReplayFault> {
        let enablement = ToolEnablement::parse(&replay.activity_text("enablement")?)
            .ok_or_else(|| ReplayFault::Unavailable("the enablement Activity is damaged".into()))?;
        Ok(SetProjectToolEnablementApplied {
            enablement,
            revision: replay.activity_u64("revision")?,
        })
    }
}

/// The offered research destination when it is the requested Registration.
fn offered(input: &SetProjectToolEnablementInput) -> Option<&'static ResearchToolOffer> {
    input
        .offer
        .filter(|offer| offer.registration.id == input.registration_id)
}

struct EnablementFacts {
    lifecycle: ToolRegistrationLifecycle,
    current: Option<CurrentEnablement>,
    reusable: ReusableDecision,
}

async fn read_enablement_facts(
    client: &Client,
    scope: &ProjectScope,
    offer: &ResearchToolOffer,
) -> Result<EnablementFacts, ProjectCommandError> {
    let row = client
        .query_one(
            "WITH current_revision AS (
               SELECT enablement_revision, enablement, external_compatibility_decision
                 FROM storyos.project_tool_enablements
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND registration_id = $3
                ORDER BY enablement_revision DESC
                LIMIT 1
             ), latest_binding AS (
               SELECT owner_user_id, project_id, external_compatibility_decision
                 FROM storyos.project_tool_enablements
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND registration_id = $3 AND external_compatibility_decision IS NOT NULL
                ORDER BY enablement_revision DESC
                LIMIT 1
             ), matching AS (
               SELECT enablement.external_compatibility_decision
                 FROM latest_binding AS enablement
                 JOIN storyos.external_contract_compatibility_decisions AS decision
                   ON (decision.owner_user_id, decision.project_id,
                       decision.external_compatibility_decision) =
                      (enablement.owner_user_id, enablement.project_id,
                       enablement.external_compatibility_decision)
                 JOIN storyos.project_external_use_binding_revisions AS binding
                   ON (binding.owner_user_id, binding.project_id,
                       binding.project_model_use_binding_revision) =
                      (decision.owner_user_id, decision.project_id,
                       decision.project_model_use_binding_revision)
                 JOIN storyos.processing_destination_identity_evidence_revisions AS evidence
                   ON (evidence.owner_user_id, evidence.project_id,
                       evidence.processing_destination_identity, evidence.evidence_revision) =
                      (binding.owner_user_id, binding.project_id,
                       binding.processing_destination_identity, binding.evidence_revision)
                WHERE binding.tool_registration_revision = $4::text::uuid
                  AND binding.credential_reference = $5
                  AND evidence.endpoint = $6
                  AND evidence.account_boundary = $7
             )
             SELECT (SELECT lifecycle FROM storyos.tool_registration_revisions
                      WHERE tool_registration_revision = $4::text::uuid),
                    current_revision.enablement_revision::text,
                    current_revision.enablement,
                    current_revision.external_compatibility_decision =
                      (SELECT external_compatibility_decision FROM matching),
                    (SELECT external_compatibility_decision::text FROM matching)
               FROM (SELECT 1) AS one
               LEFT JOIN current_revision ON true",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &offer.registration.id,
                &offer.registration.revision,
                &offer.credential_reference,
                &offer.registration.server_url,
                &offer.account_boundary,
            ],
        )
        .await
        .map_err(unavailable)?;
    let damaged = || unavailable("Project Tool Enablement is damaged");
    let lifecycle = row
        .get::<_, Option<&str>>(/*idx*/ 0)
        .and_then(ToolRegistrationLifecycle::parse)
        .ok_or_else(damaged)?;
    let current = match row.get::<_, Option<String>>(/*idx*/ 1) {
        None => None,
        Some(revision) => Some(CurrentEnablement {
            revision: revision.parse().map_err(|_| damaged())?,
            enablement: ToolEnablement::parse(row.get(/*idx*/ 2)).ok_or_else(damaged)?,
            binding: match row.get::<_, Option<bool>>(/*idx*/ 3) {
                Some(true) => EnablementBinding::Current,
                Some(false) | None => EnablementBinding::Changed,
            },
        }),
    };
    Ok(EnablementFacts {
        lifecycle,
        current,
        reusable: row.get(/*idx*/ 4),
    })
}

/// Writes a new Processing Destination Identity with its evidence, the Project Destination Grant
/// `research_tool_v1`, the use binding with the Credential Reference, and the compatibility
/// Decision from the pinned active Registration revision, and returns the Decision.
async fn insert_tool_binding(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    offer: &ResearchToolOffer,
) -> Result<String, tokio_postgres::Error> {
    let [identity, grant, binding, decision] = [(); 4].map(|()| Uuid::now_v7().to_string());
    client
        .execute(
            "WITH identity AS (
               INSERT INTO storyos.processing_destination_identities
                 (owner_user_id, project_id, processing_destination_identity, destination_kind,
                  control_classification)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'mcp_remote', 'external')
             ), evidence AS (
               INSERT INTO storyos.processing_destination_identity_evidence_revisions
                 (owner_user_id, project_id, processing_destination_identity, evidence_revision,
                  evidence_kind, endpoint, account_boundary)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 1, 'mcp_remote_boundary',
                       $7, $8)
             ), destination_grant AS (
               INSERT INTO storyos.project_destination_grants
                 (owner_user_id, project_id, grant_id, processing_destination_identity,
                  grant_kind, grant_scope)
               VALUES ($1::text::uuid, $2::text::uuid, $4::text::uuid, $3::text::uuid,
                       'research_tool_use', $11)
             ), binding AS (
               INSERT INTO storyos.project_external_use_binding_revisions
                 (owner_user_id, project_id, project_model_use_binding_revision,
                  processing_destination_identity, evidence_revision, grant_id,
                  tool_registration_revision, credential_source, credential_reference,
                  budget_bounds)
               VALUES ($1::text::uuid, $2::text::uuid, $5::text::uuid, $3::text::uuid, 1,
                       $4::text::uuid, $9::text::uuid, 'operator_quota', $10, 'not_required')
             )
             INSERT INTO storyos.external_contract_compatibility_decisions
               (owner_user_id, project_id, external_compatibility_decision,
                project_model_use_binding_revision, decision_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $6::text::uuid, $5::text::uuid,
                     'tool_registration_compatible')",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &identity,
                &grant,
                &binding,
                &decision,
                &offer.registration.server_url,
                &offer.account_boundary,
                &offer.registration.revision,
                &offer.credential_reference,
                &GrantScope::ResearchToolV1.as_str(),
            ],
        )
        .await?;
    Ok(decision)
}

#[cfg(test)]
#[path = "set_project_tool_enablement_tests.rs"]
mod tests;
