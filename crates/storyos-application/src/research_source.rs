//! The read port of the research source queries (S5-06). The Source Snapshots and the Source
//! Gaps are Operational Records that the settlement of a research ToolCall writes; no port
//! updates or removes one.

use storyos_contracts::{
    ResearchArtifactInspect, SourceGapInspect, SourceSnapshotInspect, UnknownResearchCallInspect,
};

use crate::{CreateAgentRunError, ProjectScope};

/// The Source Snapshots, with no captured text, the Source Gaps, and the research calls with
/// an unknown outcome of one AgentRun.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchSources {
    pub snapshots: Vec<SourceSnapshotInspect>,
    pub gaps: Vec<SourceGapInspect>,
    pub unknown_calls: Vec<UnknownResearchCallInspect>,
}

/// One Source Snapshot revision with its captured text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchSourceText {
    pub snapshot: SourceSnapshotInspect,
    pub text: String,
}

/// Reads the research sources in exact Project Scope. A Run or a Snapshot of another Scope reads
/// as `None`, so the caller refuses the request.
pub trait ResearchSourceReadStore: Sync {
    fn list_research_sources(
        &self,
        scope: &ProjectScope,
        run_id: &str,
    ) -> impl Future<Output = Result<Option<ResearchSources>, CreateAgentRunError>> + Send;

    fn get_research_source(
        &self,
        scope: &ProjectScope,
        source_snapshot_id: &str,
    ) -> impl Future<Output = Result<Option<ResearchSourceText>, CreateAgentRunError>> + Send;
}

/// Reads one immutable Research Artifact in exact Project Scope, with each citation resolved to
/// its Source Snapshot or Source Gap. An Artifact of another Scope reads as `None`.
pub trait ResearchArtifactReadStore: Sync {
    fn get_research_artifact(
        &self,
        scope: &ProjectScope,
        research_artifact_id: &str,
    ) -> impl Future<Output = Result<Option<ResearchArtifactInspect>, CreateAgentRunError>> + Send;
}
