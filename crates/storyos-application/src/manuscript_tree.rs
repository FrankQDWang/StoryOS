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
    pub object_scope: ProjectScope,
    pub chapter_id: ChapterId,
    pub title: String,
    pub order: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeFact {
    pub object_scope: ProjectScope,
    pub volume_id: VolumeId,
    pub title: String,
    pub order: u64,
    pub chapters: Vec<ChapterFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTreeFacts {
    pub snapshot: CanonicalSnapshot,
    pub tree_revision: u64,
    pub volumes: Vec<VolumeFact>,
}

/// Reads canonical manuscript tree facts under one already authenticated exact Project Scope.
pub trait ManuscriptTreeReader: Sync {
    fn read_canonical_tree_facts(
        &self,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<Option<CanonicalTreeFacts>, ProjectReadError>> + Send;
}

pub async fn get_manuscript_tree(
    reader: &impl ManuscriptTreeReader,
    scope: &ProjectScope,
) -> Result<Option<CanonicalManuscriptTree>, ProjectReadError> {
    let Some(facts) = reader.read_canonical_tree_facts(scope).await? else {
        return Ok(None);
    };
    if facts.volumes.iter().any(|volume| {
        &volume.object_scope != scope
            || volume
                .chapters
                .iter()
                .any(|chapter| &chapter.object_scope != scope)
    }) {
        return Ok(None);
    }
    Ok(Some(CanonicalManuscriptTree {
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
    }))
}

#[cfg(test)]
#[path = "manuscript_tree_tests.rs"]
mod tests;
