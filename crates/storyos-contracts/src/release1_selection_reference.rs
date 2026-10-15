//! The selection references of an assistant message (ADR 0062).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1_agent_run::SourceAvailability;

/// One exact text range that the author added to the message (ADR 0062).
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SelectionReference {
    ChapterText {
        chapter_id: String,
        base_authoritative_revision_id: String,
        coordinate_profile: SelectionCoordinateProfile,
        start: SelectionPoint,
        end: SelectionPoint,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub enum SelectionCoordinateProfile {
    #[serde(rename = "storyos.editor.utf16-code-unit.v1")]
    Utf16CodeUnitV1,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SelectionPoint {
    pub manuscript_block_id: String,
    pub offset: u32,
}

/// One selection reference of the Working Target, the blocks it covers, and its availability.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SelectionReferenceInspect {
    pub reference: SelectionReference,
    pub manuscript_block_ids: Vec<String>,
    pub availability: SourceAvailability,
}
