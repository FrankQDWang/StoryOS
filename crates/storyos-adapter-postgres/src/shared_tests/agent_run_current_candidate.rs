//! The shared test support from `agent_run_current_candidate_tests.rs`.

use crate::PostgresProjectReader;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ProjectScope,
};

/// The status and the public decision of the Run.
pub(crate) async fn settled(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    run_id: &str,
) -> (String, AgentRunDecisionInspect) {
    let record = store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    (format!("{:?}", record.status), record.decision)
}
