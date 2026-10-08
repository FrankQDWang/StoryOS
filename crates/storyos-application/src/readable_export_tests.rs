use storyos_core::{READABLE_EXPORT_UNAVAILABLE_MARKER, render_readable_manuscript};

use super::*;
use crate::{
    CanonicalSnapshot, ChapterFact, ChapterId, ProjectId, UserId, VolumeFact, VolumeId,
    manuscript_search::{ManuscriptSearchBlockFact, ManuscriptSearchChapterFact},
};

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot_id: "snapshot".to_owned(),
        project_activity_position: 2,
        replay_generation: 1,
        floor_position: 0,
        redaction_profile: "storyos.author.v1".to_owned(),
        schema_profile: "storyos.public.release.1".to_owned(),
        created_at: "2026-08-31T00:00:00.000Z".to_owned(),
        expires_at: None,
    }
}

fn owned_scope() -> ProjectScope {
    ProjectScope::new(UserId::new("user"), ProjectId::new("project"))
}

fn tree(chapters: Vec<(&str, &str)>) -> CanonicalTreeFacts {
    CanonicalTreeFacts {
        project_scope: owned_scope(),
        snapshot: snapshot(),
        tree_revision: 2,
        volumes: vec![VolumeFact {
            project_scope: owned_scope(),
            volume_id: VolumeId::new("volume-a"),
            title: "Volume A".to_owned(),
            order: 1,
            chapters: chapters
                .into_iter()
                .enumerate()
                .map(|(index, (id, title))| ChapterFact {
                    project_scope: owned_scope(),
                    chapter_id: ChapterId::new(id),
                    title: title.to_owned(),
                    order: index as u64 + 1,
                })
                .collect(),
        }],
    }
}

fn search_chapter(id: &str, text: &str) -> ManuscriptSearchChapterFact {
    ManuscriptSearchChapterFact {
        project_scope: owned_scope(),
        chapter_id: ChapterId::new(id),
        volume_order: 1,
        chapter_order: 1,
        blocks: vec![ManuscriptSearchBlockFact {
            manuscript_block_id: format!("{id}-block"),
            text: text.to_owned(),
        }],
    }
}

#[test]
fn live_tree_order_joins_block_text_and_marks_unavailable_chapters() {
    let volumes = readable_volumes_from_canonical_facts(
        &tree(vec![("chapter-a", "Chapter A"), ("chapter-b", "Chapter B")]),
        &[search_chapter("chapter-a", "Hello world")],
    );
    assert_eq!(
        render_readable_manuscript(&volumes),
        format!(
            "# Volume A\n\n## Chapter A\n\nHello world\n\n## Chapter B\n\n{READABLE_EXPORT_UNAVAILABLE_MARKER}\n"
        )
    );
}
