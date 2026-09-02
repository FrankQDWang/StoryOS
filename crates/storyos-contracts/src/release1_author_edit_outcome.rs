use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::release1::{ProjectScope, QueryOperation};
use crate::release1_author_edit::ApplyAuthorEditResponse;

pub const GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST_SCHEMA_ID: &str =
    "storyos.query.apply-author-edit-outcome.request.v1";
pub const GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.apply-author-edit-outcome.response.v1";

pub(super) const GET_APPLY_AUTHOR_EDIT_OUTCOME: QueryOperation = QueryOperation {
    operation_id: "getApplyAuthorEditOutcome",
    method: "GET",
    path: "/api/v1/projects/{project_id}/manuscript/author-edit-outcomes/{idempotency_key}",
    request_schema: GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST_SCHEMA_ID,
    response_schema: GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Current Apply Author Edit outcome"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Outcome unavailable"),
        (405, "Method not allowed"),
        (409, "Protocol or cursor conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Request refused"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.getApplyAuthorEditOutcome.positive.v1",
        "storyos.golden.getApplyAuthorEditOutcome.invalid.v1",
        "storyos.golden.getApplyAuthorEditOutcome.boundary.v1",
    ],
};

pub const GET_APPLY_AUTHOR_EDIT_OUTCOME_PATH: &str = GET_APPLY_AUTHOR_EDIT_OUTCOME.path;
pub const GET_APPLY_AUTHOR_EDIT_OUTCOME_METHOD: &str = GET_APPLY_AUTHOR_EDIT_OUTCOME.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetApplyAuthorEditOutcomeRequest {
    pub project_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApplyAuthorEditRejectionReason {
    ChallengeExpiredUnconsumed,
}

/// The required reconciliation wire literal. A false value is not representable.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReconciliationRequired;

impl Serialize for ReconciliationRequired {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for ReconciliationRequired {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if bool::deserialize(deserializer)? {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(
                "reconciliation_required must be true",
            ))
        }
    }
}

fn reconciliation_required_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type": "boolean", "const": true})
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(
    tag = "observation_kind",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ApplyAuthorEditUnknownObservation {
    ChallengeIssued {
        expires_at: String,
    },
    AdmissionCommitted {
        command_id: String,
        author_command_admission_id: String,
        #[schemars(schema_with = "reconciliation_required_schema")]
        #[ts(type = "true")]
        reconciliation_required: ReconciliationRequired,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApplyAuthorEditReconfirmationReason {
    AdmissionExpired,
    BindingChanged,
    DirectEditIntentUnrecoverable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "outcome_kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplyAuthorEditOutcome {
    Committed {
        response: Box<ApplyAuthorEditResponse>,
    },
    Rejected {
        reason: ApplyAuthorEditRejectionReason,
    },
    RequiresReconfirmation {
        command_id: String,
        author_command_admission_id: String,
        reconfirmation_reason: ApplyAuthorEditReconfirmationReason,
        recovery_draft_ref: Option<String>,
    },
    StillUnknown {
        observation: ApplyAuthorEditUnknownObservation,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetApplyAuthorEditOutcomeResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub outcome: ApplyAuthorEditOutcome,
}
