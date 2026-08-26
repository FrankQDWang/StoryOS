use super::{
    CanonicalManuscriptTree, CanonicalSnapshot, CanonicalTreeFacts, ChapterFact,
    ManuscriptTreeReader, VolumeFact, VolumeId, get_manuscript_tree,
};
use crate::{ChapterId, ProjectId, ProjectReadError, ProjectScope, UserId};

struct EmptyTreeReader {
    facts: CanonicalTreeFacts,
}

impl ManuscriptTreeReader for EmptyTreeReader {
    async fn read_canonical_tree_facts(
        &self,
        _scope: &ProjectScope,
    ) -> Result<Option<CanonicalTreeFacts>, ProjectReadError> {
        Ok(Some(self.facts.clone()))
    }
}

fn empty_snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot_id: "snapshot-empty".to_owned(),
        project_activity_position: 1,
        replay_generation: 1,
        floor_position: 0,
        redaction_profile: "storyos.author.v1".to_owned(),
        schema_profile: "storyos.public.release.1".to_owned(),
        created_at: "2026-08-26T00:00:00.000Z".to_owned(),
        expires_at: None,
    }
}

#[tokio::test]
async fn an_empty_active_project_returns_an_ordered_canonical_tree_with_zero_volumes_and_zero_chapters()
 {
    let scope = ProjectScope::new(UserId::new("user-a"), ProjectId::new("project-a"));
    let snapshot = empty_snapshot();
    let reader = EmptyTreeReader {
        facts: CanonicalTreeFacts {
            snapshot: snapshot.clone(),
            tree_revision: 1,
            volumes: Vec::new(),
        },
    };

    let tree = get_manuscript_tree(&reader, &scope).await.unwrap();

    assert_eq!(
        tree,
        Some(CanonicalManuscriptTree {
            project_scope: scope,
            snapshot,
            tree_revision: 1,
            volumes: Vec::new(),
        })
    );
}

#[tokio::test]
async fn a_wrong_owner_volume_returns_no_tree_fact() {
    let scope = ProjectScope::new(UserId::new("user-a"), ProjectId::new("project-a"));
    let reader = EmptyTreeReader {
        facts: CanonicalTreeFacts {
            snapshot: empty_snapshot(),
            tree_revision: 1,
            volumes: vec![VolumeFact {
                object_scope: ProjectScope::new(UserId::new("user-b"), ProjectId::new("project-a")),
                volume_id: VolumeId::new("volume-foreign"),
                title: "Foreign".to_owned(),
                order: 1,
                chapters: Vec::new(),
            }],
        },
    };

    let tree = get_manuscript_tree(&reader, &scope).await.unwrap();

    assert_eq!(tree, None);
}

#[tokio::test]
async fn a_wrong_owner_chapter_returns_no_tree_fact() {
    let scope = ProjectScope::new(UserId::new("user-a"), ProjectId::new("project-a"));
    let reader = EmptyTreeReader {
        facts: CanonicalTreeFacts {
            snapshot: empty_snapshot(),
            tree_revision: 1,
            volumes: vec![VolumeFact {
                object_scope: scope.clone(),
                volume_id: VolumeId::new("volume-owned"),
                title: "Owned".to_owned(),
                order: 1,
                chapters: vec![ChapterFact {
                    object_scope: ProjectScope::new(
                        UserId::new("user-b"),
                        ProjectId::new("project-a"),
                    ),
                    chapter_id: ChapterId::new("chapter-foreign"),
                    title: "Foreign".to_owned(),
                    order: 1,
                }],
            }],
        },
    };

    let tree = get_manuscript_tree(&reader, &scope).await.unwrap();

    assert_eq!(tree, None);
}
