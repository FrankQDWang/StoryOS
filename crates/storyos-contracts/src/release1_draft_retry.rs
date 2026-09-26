use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename = "draft_retry", deny_unknown_fields)]
pub struct DraftRetry {
    pub source_draft_kind: RetryDraftKind,
    pub source_draft_id: String,
    pub source_current_draft_revision_id: String,
    pub source_draft_payload_digest: String,
    pub expected_source_draft_closure: RetryDraftClosure,
    pub selected_payload_range: ExactStructuredRange,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RetryDraftKind {
    RefusedEdit,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RetryDraftClosure {
    Open,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename = "exact_structured_range", deny_unknown_fields)]
pub struct ExactStructuredRange {
    pub coordinate_profile: String,
    pub from: DraftPayloadPosition,
    pub to: DraftPayloadPosition,
    pub slice_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DraftPayloadPosition {
    pub block_index: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceDraftDisposition {
    Unchanged {
        source_draft_kind: RetryDraftKind,
        source_draft_id: String,
        requested_source_draft_revision_id: String,
        current_source_draft_revision_id: String,
        current_source_draft_payload_digest: String,
        current_closure: ObservedDraftClosure,
    },
    ClosedSuperseded {
        source_draft_kind: RetryDraftKind,
        source_draft_id: String,
        source_draft_revision_id: String,
        source_draft_payload_digest: String,
        prior_closure: RetryDraftClosure,
        resulting_closure: String,
        close_reason: String,
        closure_event_ref: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObservedDraftClosure {
    Open,
    Closed {
        close_reason: String,
        closure_event_ref: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DraftRetryReplacement {
    pub source_draft_id: String,
    pub source_draft_revision_id: String,
    pub source_draft_payload_digest: String,
    pub closure_event_ref: String,
    pub selected_payload_range: ExactStructuredRange,
}
