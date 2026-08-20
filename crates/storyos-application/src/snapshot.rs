use std::future::Future;

use crate::ProjectScope;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSnapshot {
    pub snapshot_id: String,
    pub project_activity_position: u64,
    pub replay_generation: u64,
    pub floor_position: u64,
    pub redaction_profile: String,
    pub schema_profile: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotLookup {
    pub project_scope: ProjectScope,
    pub snapshot_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedAuthorEditActivity {
    pub event_id: String,
    pub project_sequence: u64,
    pub stream_sequence: u64,
    pub command_id: String,
    pub correlation_id: String,
    pub receipt_id: String,
    pub chapter_id: String,
    pub authoritative_revision_id: String,
    pub authoritative_commit_id: String,
    pub author_action_sequence: String,
    pub occurred_at: String,
}

#[derive(Debug)]
pub enum SnapshotReadError {
    Expired,
    CursorTooOld,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for SnapshotReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Expired => formatter.write_str("snapshot expired"),
            Self::CursorTooOld => formatter.write_str("activity cursor too old"),
            Self::Unavailable(source) => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for SnapshotReadError {}

/// Reads catalogued Canonical Query Snapshots and Activity replay eligibility.
pub trait SnapshotStore {
    fn read_snapshot(
        &self,
        lookup: &SnapshotLookup,
    ) -> impl Future<Output = Result<Option<CanonicalSnapshot>, SnapshotReadError>> + Send;

    fn activity_resume(
        &self,
        lookup: &SnapshotLookup,
    ) -> impl Future<Output = Result<CanonicalSnapshot, SnapshotReadError>> + Send;

    fn list_applied_author_edit_activity(
        &self,
        lookup: &SnapshotLookup,
        after_position: u64,
    ) -> impl Future<Output = Result<Vec<AppliedAuthorEditActivity>, SnapshotReadError>> + Send;
}

pub async fn get_snapshot(
    store: &impl SnapshotStore,
    lookup: &SnapshotLookup,
) -> Result<Option<CanonicalSnapshot>, SnapshotReadError> {
    store.read_snapshot(lookup).await
}

pub async fn activity_stream_resume(
    store: &impl SnapshotStore,
    lookup: &SnapshotLookup,
) -> Result<CanonicalSnapshot, SnapshotReadError> {
    store.activity_resume(lookup).await
}
