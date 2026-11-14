//! The current authority of one ToolCall and the row writes of its resolution (ADR 0034,
//! ADR 0084, ADR 0090). The Worker reads the authority again in each transaction that can
//! dispatch: the current research compatibility Decision of the Project, the use binding with its
//! Credential Reference and grant, the evidence endpoint, and the revision lifecycle.

use serde_json::{Value, json};
use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    ContractDifference, GrantScope, ToolCallPlan, ToolCallState, ToolCompatibilityDecision,
    ToolDispatchFacts, ToolDispatchRefusal, ToolRegistrationLifecycle,
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
                  -- A Host ToolCall goes after the other ToolCalls of its response, and
                  -- `finish` goes last (ADR 0091).
                  ORDER BY decision_position, registration_id = $4,
                           tool_spec_revision = $5, call_position
                  LIMIT 1 FOR UPDATE"
            ),
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &storyos_core::HOST_TOOL_REGISTRATION.id,
                &storyos_core::FINISH_TOOL_SPEC.revision,
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
    compatibility: Option<ToolCompatibilityDecision>,
    pinned_revision: Option<String>,
    pub credential_reference: Option<String>,
    grant_scope: Option<GrantScope>,
    pub grant_id: Option<String>,
    same_scope: bool,
    run_grant_names_tool: bool,
    pub endpoint: Option<String>,
    lifecycle: ToolRegistrationLifecycle,
    /// The exact research compatibility Decision, external-use binding, Processing
    /// Destination Identity, evidence revision, and grant that this authority comes from.
    pub evidence: Value,
}

impl ToolAuthority {
    /// The Core admission of the ToolCall with this authority.
    pub fn admit(&self, row: &ToolCallRow, plan: &ToolCallPlan) -> Result<(), ToolDispatchRefusal> {
        storyos_core::admit_tool_dispatch(&ToolDispatchFacts {
            same_scope: self.same_scope,
            run_grant_names_tool: self.run_grant_names_tool,
            lifecycle: self.lifecycle,
            compatibility: self.compatibility,
            binding_pins_revision: self.pinned_revision.as_deref()
                == Some(row.registration_revision.as_str()),
            credential_reference: self.credential_reference.as_deref(),
            grant_scope: self.grant_scope,
            effect: &plan.effect,
        })
    }
}

/// Reads the authority of the ToolCall `row` in the transaction of the caller. A revision without
/// a lifecycle row is the active constant, because the row appears at the first binding.
pub(crate) async fn read_authority(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
) -> Result<ToolAuthority, CompleteAgentRunError> {
    let row = client
        .query_one(
            "SELECT decision.decision_kind,
                    binding.tool_registration_revision::text, binding.credential_reference,
                    grant_row.grant_scope, grant_row.grant_id::text,
                    COALESCE(grant_row.grant_kind = 'research_tool_use'
                      AND grant_row.processing_destination_identity =
                          binding.processing_destination_identity, false),
                    evidence.endpoint,
                    (SELECT lifecycle FROM storyos.tool_registration_revisions
                      WHERE tool_registration_revision = $3::text::uuid),
                    jsonb_build_object(
                      'external_compatibility_decision',
                      decision.external_compatibility_decision,
                      'project_model_use_binding_revision',
                      binding.project_model_use_binding_revision,
                      'processing_destination_identity', binding.processing_destination_identity,
                      'evidence_revision', binding.evidence_revision,
                      'grant_id', binding.grant_id)::text,
                    EXISTS (
                      SELECT 1 FROM storyos.tool_calls AS call
                        JOIN storyos.model_attempts AS attempt
                          ON (attempt.owner_user_id, attempt.project_id, attempt.model_attempt_id)
                             = (call.owner_user_id, call.project_id, call.model_attempt_id)
                       CROSS JOIN LATERAL jsonb_array_elements(
                         CASE WHEN jsonb_typeof(attempt.payload
                                     #> '{step_snapshot,tool_exposure,tools}') = 'array'
                              THEN attempt.payload #> '{step_snapshot,tool_exposure,tools}'
                              ELSE '[]'::jsonb END) AS tool
                       WHERE call.owner_user_id = $1::text::uuid
                         AND call.project_id = $2::text::uuid
                         AND call.tool_call_id = $4::text::uuid
                         AND tool ->> 'name' = call.function_name
                         AND tool ->> 'registration_id' = call.registration_id
                         AND tool ->> 'registration_revision' = call.tool_registration_revision::text
                         AND tool ->> 'spec_revision' = call.tool_spec_revision)
               FROM (SELECT 1) AS one
               LEFT JOIN LATERAL (
                 SELECT * FROM storyos.external_contract_compatibility_decisions
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND decision_kind = 'tool_registration_compatible'
                  ORDER BY external_compatibility_decision DESC LIMIT 1) AS decision ON true
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
                &row.registration_revision,
                &row.tool_call_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    let lifecycle = match row.get::<_, Option<&str>>(/*idx*/ 7) {
        None => ToolRegistrationLifecycle::Active,
        Some(lifecycle) => ToolRegistrationLifecycle::parse(lifecycle)
            .ok_or_else(|| damaged("Tool Registration is damaged"))?,
    };
    Ok(ToolAuthority {
        compatibility: row
            .get::<_, Option<&str>>(/*idx*/ 0)
            .and_then(ToolCompatibilityDecision::parse),
        pinned_revision: row.get(/*idx*/ 1),
        credential_reference: row.get(/*idx*/ 2),
        grant_scope: row
            .get::<_, Option<&str>>(/*idx*/ 3)
            .and_then(GrantScope::parse),
        grant_id: row.get(/*idx*/ 4),
        same_scope: row.get(/*idx*/ 5),
        endpoint: row.get(/*idx*/ 6),
        lifecycle,
        run_grant_names_tool: row.get(/*idx*/ 9),
        evidence: serde_json::from_str(row.get(/*idx*/ 8))
            .map_err(|_| damaged("Tool authority evidence is damaged"))?,
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
    let differences = storage_safe(
        serde_json::to_value(differences)
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?,
    );
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

/// The observed server strings in a form that PostgreSQL stores. A string with U+0000 becomes
/// `{"nul_separated": [<segments>]}`, which keeps each character of the observed value.
fn storage_safe(value: Value) -> Value {
    match value {
        Value::String(text) if text.contains('\0') => {
            json!({ "nul_separated": text.split('\0').collect::<Vec<_>>() })
        }
        Value::Array(items) => Value::Array(items.into_iter().map(storage_safe).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(name, field)| (name, storage_safe(field)))
                .collect(),
        ),
        other => other,
    }
}

pub(crate) async fn append_event(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    kind: &str,
    payload: &Value,
) -> Result<(), CompleteAgentRunError> {
    client
        .execute(
            // The serializable transaction makes the next position of the Run unique.
            "INSERT INTO storyos.agent_run_events
               (owner_user_id, project_id, run_id, event_id, event_position, event_kind, payload)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                    COALESCE(max(event_position) + 1, 0), $5, $6::text::jsonb
               FROM storyos.agent_run_events
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid",
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
