use std::future::Future;

use crate::{CanonicalSnapshot, ChapterId, ProjectReadError, ProjectScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeId(String);

impl VolumeId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for VolumeId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterNode {
    pub chapter_id: ChapterId,
    pub title: String,
    pub order: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeNode {
    pub volume_id: VolumeId,
    pub title: String,
    pub order: u64,
    pub chapters: Vec<ChapterNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalManuscriptTree {
    pub project_scope: ProjectScope,
    pub snapshot: CanonicalSnapshot,
    pub tree_revision: u64,
    pub volumes: Vec<VolumeNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterFact {
    pub project_scope: ProjectScope,
    pub chapter_id: ChapterId,
    pub title: String,
    pub order: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeFact {
    pub project_scope: ProjectScope,
    pub volume_id: VolumeId,
    pub title: String,
    pub order: u64,
    pub chapters: Vec<ChapterFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTreeFacts {
    pub project_scope: ProjectScope,
    pub snapshot: CanonicalSnapshot,
    pub tree_revision: u64,
    pub volumes: Vec<VolumeFact>,
}

/// Live tree facts bound to one Snapshot, or a closed missing/resync outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalTreeRead {
    Found(Box<CanonicalTreeFacts>),
    Missing,
    SnapshotExpired,
}

/// Canonical Manuscript Tree read: live structure with the latest Snapshot, or resync.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GetManuscriptTree {
    Found(Box<CanonicalManuscriptTree>),
    Missing,
    SnapshotExpired,
}

/// Reads canonical manuscript tree facts under one already authenticated exact Project Scope.
pub trait ManuscriptTreeReader: Sync {
    fn read_canonical_tree_facts(
        &self,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<CanonicalTreeRead, ProjectReadError>> + Send;
}

pub async fn get_manuscript_tree(
    reader: &impl ManuscriptTreeReader,
    scope: &ProjectScope,
) -> Result<GetManuscriptTree, ProjectReadError> {
    match reader.read_canonical_tree_facts(scope).await? {
        CanonicalTreeRead::Missing => Ok(GetManuscriptTree::Missing),
        CanonicalTreeRead::SnapshotExpired => Ok(GetManuscriptTree::SnapshotExpired),
        CanonicalTreeRead::Found(facts)
            if &facts.project_scope == scope
                && facts.volumes.iter().all(|volume| {
                    &volume.project_scope == scope
                        && volume
                            .chapters
                            .iter()
                            .all(|chapter| &chapter.project_scope == scope)
                }) =>
        {
            let facts = *facts;
            Ok(GetManuscriptTree::Found(Box::new(
                CanonicalManuscriptTree {
                    project_scope: scope.clone(),
                    snapshot: facts.snapshot,
                    tree_revision: facts.tree_revision,
                    volumes: facts
                        .volumes
                        .into_iter()
                        .map(|volume| VolumeNode {
                            volume_id: volume.volume_id,
                            title: volume.title,
                            order: volume.order,
                            chapters: volume
                                .chapters
                                .into_iter()
                                .map(|chapter| ChapterNode {
                                    chapter_id: chapter.chapter_id,
                                    title: chapter.title,
                                    order: chapter.order,
                                })
                                .collect(),
                        })
                        .collect(),
                },
            )))
        }
        CanonicalTreeRead::Found(_) => Ok(GetManuscriptTree::Missing),
    }
}

#[cfg(test)]
#[path = "manuscript_tree_tests.rs"]
mod tests;
