pub use storyos_contracts::ProjectActivityKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityAggregateRef {
    pub kind: String,
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectActivityEvent {
    pub event_id: String,
    pub kind: ProjectActivityKind,
    pub event_schema: String,
    pub project_sequence: u64,
    pub stream_sequence: u64,
    pub command_id: String,
    pub correlation_id: String,
    pub receipt_id: String,
    pub occurred_at: String,
    pub aggregate: ActivityAggregateRef,
    pub payload_json: String,
}

#[cfg(test)]
#[path = "project_activity_tests.rs"]
mod tests;
