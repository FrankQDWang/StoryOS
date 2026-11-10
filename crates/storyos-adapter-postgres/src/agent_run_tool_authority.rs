//! The current authority of one ToolCall and the row writes of its resolution (ADR 0034,
//! ADR 0084). The Worker reads the authority again in each transaction that can dispatch: the
//! latest Project Tool Enablement of the Registration, its compatibility Decision, the use binding
//! with its Credential Reference and grant, the evidence endpoint, and the revision lifecycle.

use serde_json::{Value, json};
use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    ContractDifference, GrantScope, ToolCallPlan, ToolCallState, ToolCompatibilityDecision,
    ToolDispatchFacts, ToolDispatchRefusal, ToolEnablement, ToolRegistrationLifecycle,
};
use uuid::Uuid;

use crate::agent_run_work::complete_database_error;

/// One ToolCall row that the transaction locked.
pub(crate) struct ToolCallRow {
    pub tool_call_id: String,
    pub call_id: String,
    pub state: ToolCallState,
    pub registration_revision: String,
    pub spec_revision: String,
    pub arguments: Value,
    pub destination_attempt_id: Option<String>,
    pub attempt: Option<Value>,
}

const ROW_COLUMNS: &str = "tool_call_id::text, call_id, state, tool_registration_revision::text,
    tool_spec_revision, arguments::text, destination_attempt_id::text, attempt::text";

impl ToolCallRow {
    fn decode(row: &tokio_postgres::Row) -> Result<Self, CompleteAgentRunError> {
        let json = |text: Option<String>| {
            text.map(|text| serde_json::from_str(&text))
                .transpose()
                .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))
        };
        Ok(Self {
            tool_call_id: row.get(/*idx*/ 0),
            call_id: row.get(/*idx*/ 1),
            state: ToolCallState::parse(row.get(/*idx*/ 2))
                .ok_or_else(|| damaged("The ToolCall state is damaged"))?,
            registration_revision: row.get(/*idx*/ 3),
            spec_revision: row.get(/*idx*/ 4),
            arguments: json(row.get(/*idx*/ 5))?.unwrap_or(Value::Null),
            destination_attempt_id: row.get(/*idx*/ 6),
            attempt: json(row.get(/*idx*/ 7))?,
        })
    }

    /// The dispatch plan of the stored pins. A row that no plan derives from is damaged.
    pub fn plan(&self) -> Result<ToolCallPlan, CompleteAgentRunError> {
        ToolCallPlan::derive(
            &self.registration_revision,
            &self.spec_revision,
            &self.arguments,
        )
        .ok_or_else(|| damaged("The ToolCall pins no dispatch plan"))
    }
}

/// The first unresolved ToolCall of the Run in Decision and item order, locked.
pub(crate) async fn next_unresolved(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Option<ToolCallRow>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            &format!(
                "SELECT {ROW_COLUMNS} FROM storyos.tool_calls
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid AND state IN ('pending', 'dispatched')
                  ORDER BY decision_position, call_position LIMIT 1 FOR UPDATE"
            ),
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    row.as_ref().map(ToolCallRow::decode).transpose()
}

/// The ToolCall `tool_call_id` of the claimed Run in `state`, locked.
pub(crate) async fn locked(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    tool_call_id: &str,
    state: ToolCallState,
) -> Result<Option<ToolCallRow>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            &format!(
                "SELECT {ROW_COLUMNS} FROM storyos.tool_calls
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid AND tool_call_id = $4::text::uuid AND state = $5
                  FOR UPDATE"
            ),
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &tool_call_id,
                &state.as_str(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    row.as_ref().map(ToolCallRow::decode).transpose()
}

/// The current authority of the Registration of one ToolCall.
pub(crate) struct ToolAuthority {
    enablement: Option<ToolEnablement>,
    compatibility: Option<ToolCompatibilityDecision>,
    pinned_revision: Option<String>,
    pub credential_reference: Option<String>,
    grant_scope: Option<GrantScope>,
    pub grant_id: Option<String>,
    same_scope: bool,
    pub endpoint: Option<String>,
    lifecycle: ToolRegistrationLifecycle,
}

impl ToolAuthority {
    /// The Core admission of the ToolCall with this authority.
    pub fn admit(&self, row: &ToolCallRow, plan: &ToolCallPlan) -> Result<(), ToolDispatchRefusal> {
        storyos_core::admit_tool_dispatch(&ToolDispatchFacts {
            same_scope: self.same_scope,
            lifecycle: self.lifecycle,
            enablement: self.enablement,
            compatibility: self.compatibility,
            binding_pins_revision: self.pinned_revision.as_deref()
                == Some(row.registration_revision.as_str()),
            credential_reference: self.credential_reference.as_deref(),
            grant_scope: self.grant_scope,
            effect: &plan.effect,
        })
    }
}

/// Reads the authority of `plan` in the transaction of the caller. A revision without a lifecycle
/// row is the active constant, because the row appears at the first enablement.
pub(crate) async fn read_authority(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    plan: &ToolCallPlan,
) -> Result<ToolAuthority, CompleteAgentRunError> {
    let row = client
        .query_one(
            "SELECT enablement.enablement, decision.decision_kind,
                    binding.tool_registration_revision::text, binding.credential_reference,
                    grant_row.grant_scope, grant_row.grant_id::text,
                    COALESCE(grant_row.grant_kind = 'research_tool_use'
                      AND grant_row.processing_destination_identity =
                          binding.processing_destination_identity, false),
                    evidence.endpoint,
                    (SELECT lifecycle FROM storyos.tool_registration_revisions
                      WHERE tool_registration_revision = $4::text::uuid)
               FROM (SELECT 1) AS one
               LEFT JOIN LATERAL (
                 SELECT * FROM storyos.project_tool_enablements
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND registration_id = $3
                  ORDER BY enablement_revision DESC LIMIT 1) AS enablement ON true
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
               LEFT JOIN storyos.project_destination_grants AS grant_row
                 ON (grant_row.owner_user_id, grant_row.project_id, grant_row.grant_id) =
                    (binding.owner_user_id, binding.project_id, binding.grant_id)
               LEFT JOIN storyos.processing_destination_identity_evidence_revisions AS evidence
                 ON (evidence.owner_user_id, evidence.project_id,
                     evidence.processing_destination_identity, evidence.evidence_revision) =
                    (binding.owner_user_id, binding.project_id,
                     binding.processing_destination_identity, binding.evidence_revision)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &plan.registration.id,
                &row.registration_revision,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    let lifecycle = match row.get::<_, Option<&str>>(/*idx*/ 8) {
        None => ToolRegistrationLifecycle::Active,
        Some(lifecycle) => ToolRegistrationLifecycle::parse(lifecycle)
            .ok_or_else(|| damaged("Tool Registration is damaged"))?,
    };
    Ok(ToolAuthority {
        enablement: row
            .get::<_, Option<&str>>(/*idx*/ 0)
            .and_then(ToolEnablement::parse),
        compatibility: row
            .get::<_, Option<&str>>(/*idx*/ 1)
            .and_then(ToolCompatibilityDecision::parse),
        pinned_revision: row.get(/*idx*/ 2),
        credential_reference: row.get(/*idx*/ 3),
        grant_scope: row
            .get::<_, Option<&str>>(/*idx*/ 4)
            .and_then(GrantScope::parse),
        grant_id: row.get(/*idx*/ 5),
        same_scope: row.get(/*idx*/ 6),
        endpoint: row.get(/*idx*/ 7),
        lifecycle,
    })
}

/// The columns of one resolution of a ToolCall row.
pub(crate) struct Resolution<'a> {
    pub state: ToolCallState,
    pub refusal: Option<&'a str>,
    pub attempt: Option<&'a Value>,
    pub outcome: Option<Value>,
    pub result: Option<&'a storyos_core::ToolResultText>,
}

/// Writes the resolution of `row` and appends its one `tool_call_settled` Activity Run Event.
pub(crate) async fn resolve(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    resolution: Resolution<'_>,
) -> Result<(), CompleteAgentRunError> {
    let bytes = resolution
        .result
        .map(|result| i32::try_from(result.bytes))
        .transpose()
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    client
        .execute(
            "UPDATE storyos.tool_calls
                SET state = $4, refusal = $5, attempt = $6::text::jsonb,
                    effect_outcome = $7::text::jsonb, result_text = $8, result_digest = $9,
                    result_bytes = $10
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND tool_call_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &row.tool_call_id,
                &resolution.state.as_str(),
                &resolution.refusal,
                &resolution.attempt.map(Value::to_string),
                &resolution.outcome.as_ref().map(Value::to_string),
                &resolution.result.map(|result| result.text.as_str()),
                &resolution.result.map(|result| result.digest.as_str()),
                &bytes,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    append_event(
        client,
        claim,
        "tool_call_settled",
        &json!({
            "tool_call_id": row.tool_call_id,
            "call_id": row.call_id,
            "tool_registration_revision": row.registration_revision,
            "tool_spec_revision": row.spec_revision,
            "destination_attempt_id": row.destination_attempt_id,
            "state": resolution.state.as_str(),
            "refusal": resolution.refusal,
            "outcome": resolution.outcome,
            "result_bytes": bytes,
        }),
    )
    .await
}

/// Writes a refusal before the dispatch claim: no Destination Attempt and no Disclosure Event.
pub(crate) async fn refuse(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    state: ToolCallState,
    code: &str,
) -> Result<(), CompleteAgentRunError> {
    resolve(
        client,
        claim,
        row,
        Resolution {
            state,
            refusal: Some(code),
            attempt: None,
            outcome: None,
            result: None,
        },
    )
    .await
}

/// Moves an active revision to quarantined after a dispatch found Tool Contract Drift, and
/// appends the `tool_registration_quarantined` event of the Run. A revision that is already
/// unavailable does not change.
pub(crate) async fn quarantine(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    differences: &[ContractDifference],
) -> Result<(), CompleteAgentRunError> {
    let differences = serde_json::to_value(differences)
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let changed = client
        .execute(
            "UPDATE storyos.tool_registration_revisions
                SET lifecycle = 'quarantined', quarantine_differences = $2::text::jsonb,
                    quarantined_at = clock_timestamp()
              WHERE tool_registration_revision = $1::text::uuid AND lifecycle = 'active'",
            &[&row.registration_revision, &differences.to_string()],
        )
        .await
        .map_err(complete_database_error)?;
    if changed == 0 {
        return Ok(());
    }
    append_event(
        client,
        claim,
        "tool_registration_quarantined",
        &json!({
            "tool_registration_revision": row.registration_revision,
            "tool_call_id": row.tool_call_id,
            "differences": differences,
        }),
    )
    .await
}

async fn append_event(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    kind: &str,
    payload: &Value,
) -> Result<(), CompleteAgentRunError> {
    client
        .execute(
            "INSERT INTO storyos.agent_run_events
               (owner_user_id, project_id, run_id, event_id, event_kind, payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                     $6::text::jsonb)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &Uuid::now_v7().to_string(),
                &kind,
                &payload.to_string(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(())
}

pub(crate) fn damaged(message: &'static str) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(message)))
}
