use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One step of the plan of a Run (ADR 0103 section 2), in the order of the list.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PlanStepInspect {
    pub text: String,
    pub status: PlanStepStatusInspect,
}

/// The state of one step of a plan.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatusInspect {
    Pending,
    InProgress,
    Completed,
}
