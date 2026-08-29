use super::{ChapterQueryFacts, ChapterQueryReader, OpenChapter, open_chapter};
use crate::{
    CanonicalSnapshot, Chapter, ChapterId, ProjectId, ProjectReadError, ProjectScope, RevisionId,
    UserId,
};
use storyos_core::{ManuscriptBlock, ManuscriptBlockKind};

struct FixtureReader {
    read: OpenChapter,
}

impl ChapterQueryReader for FixtureReader {
    async fn read_chapter_query(
        &self,
        _scope: &ProjectScope,
        _chapter_id: &ChapterId,
    ) -> Result<OpenChapter, ProjectReadError> {
        Ok(self.read.clone())
    }
}

fn owned_scope() -> ProjectScope {
    ProjectScope::new(UserId::new("user-a"), ProjectId::new("project-a"))
}

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot_id: "snapshot-a".to_owned(),
        project_activity_position: 4,
        replay_generation: 1,
        floor_position: 0,
        redaction_profile: "storyos.author.v1".to_owned(),
        schema_profile: "storyos.public.release.1".to_owned(),
        created_at: "2026-08-27T00:00:00.000Z".to_owned(),
        expires_at: None,
    }
}

fn later_chapter() -> Chapter {
    Chapter {
        chapter_id: ChapterId::new("chapter-b"),
        title: "Chapter B".to_owned(),
        revision_id: RevisionId::new("revision-b"),
        body: String::new(),
        blocks: vec![ManuscriptBlock {
            manuscript_block_id: "block-b".to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: String::new(),
        }],
        project_activity_position: 4,
    }
}

fn found(facts: ChapterQueryFacts) -> OpenChapter {
    OpenChapter::Found(Box::new(facts))
}

#[tokio::test]
async fn a_later_in_scope_chapter_returns_its_snapshot_and_head() {
    let scope = owned_scope();
    let snapshot = snapshot();
    let chapter = later_chapter();
    let facts = ChapterQueryFacts {
        project_scope: scope.clone(),
        snapshot,
        chapter,
    };
    let reader = FixtureReader {
        read: found(facts.clone()),
    };

    let opened = open_chapter(&reader, &scope, &ChapterId::new("chapter-b"))
        .await
        .unwrap();

    assert_eq!(opened, found(facts));
}

#[tokio::test]
async fn a_chapter_outside_project_scope_fails_closed() {
    let scope = owned_scope();
    let reader = FixtureReader {
        read: found(ChapterQueryFacts {
            project_scope: ProjectScope::new(UserId::new("user-b"), ProjectId::new("project-a")),
            snapshot: snapshot(),
            chapter: later_chapter(),
        }),
    };

    let opened = open_chapter(&reader, &scope, &ChapterId::new("chapter-b"))
        .await
        .unwrap();

    assert_eq!(opened, OpenChapter::Missing);
}

#[tokio::test]
async fn an_expired_snapshot_fails_closed() {
    let opened = open_chapter(
        &FixtureReader {
            read: OpenChapter::SnapshotExpired,
        },
        &owned_scope(),
        &ChapterId::new("chapter-b"),
    )
    .await
    .unwrap();

    assert_eq!(opened, OpenChapter::SnapshotExpired);
}

#[tokio::test]
async fn a_missing_chapter_fails_closed() {
    let opened = open_chapter(
        &FixtureReader {
            read: OpenChapter::Missing,
        },
        &owned_scope(),
        &ChapterId::new("chapter-b"),
    )
    .await
    .unwrap();

    assert_eq!(opened, OpenChapter::Missing);
}

#[tokio::test]
async fn a_chapter_identity_mismatch_fails_closed() {
    let scope = owned_scope();
    let reader = FixtureReader {
        read: found(ChapterQueryFacts {
            project_scope: scope.clone(),
            snapshot: snapshot(),
            chapter: later_chapter(),
        }),
    };

    let opened = open_chapter(&reader, &scope, &ChapterId::new("chapter-a"))
        .await
        .unwrap();

    assert_eq!(opened, OpenChapter::Missing);
}
