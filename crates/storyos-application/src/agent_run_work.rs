use std::future::Future;

use crate::{ProjectReadError, ProjectScope};

/// One fenced Worker claim of a queued AgentRun.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedAgentRun {
    pub project_scope: ProjectScope,
    pub run_id: String,
    pub fence_token: i64,
}

/// Settlement of one claimed fake-model AgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompleteAgentRun {
    Settled,
    AlreadySettled,
}

/// Failure while completing one claimed AgentRun.
#[derive(Debug)]
pub enum CompleteAgentRunError {
    StaleFence,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for CompleteAgentRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleFence => formatter.write_str("The AgentRun fence is stale"),
            Self::Unavailable(_) => formatter.write_str("AgentRun work is unavailable"),
        }
    }
}

impl std::error::Error for CompleteAgentRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::StaleFence => None,
        }
    }
}

/// Claims and settles one focused AgentRun work family.
pub trait AgentRunWorkStore: Sync {
    fn claim_next_agent_run(
        &self,
    ) -> impl Future<Output = Result<Option<ClaimedAgentRun>, ProjectReadError>> + Send;

    fn complete_agent_run(
        &self,
        claim: &ClaimedAgentRun,
    ) -> impl Future<Output = Result<CompleteAgentRun, CompleteAgentRunError>> + Send;
}

pub async fn claim_next_agent_run(
    store: &impl AgentRunWorkStore,
) -> Result<Option<ClaimedAgentRun>, ProjectReadError> {
    store.claim_next_agent_run().await
}

pub async fn complete_agent_run(
    store: &impl AgentRunWorkStore,
    claim: &ClaimedAgentRun,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    store.complete_agent_run(claim).await
}
