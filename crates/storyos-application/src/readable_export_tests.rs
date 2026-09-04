use std::sync::Mutex;

use storyos_core::{READABLE_EXPORT_UNAVAILABLE_MARKER, render_readable_manuscript};

use super::*;
use crate::{
    CanonicalSnapshot, ChapterFact, ChapterId, ProjectId, UserId, VolumeFact, VolumeId,
    manuscript_search::{ManuscriptSearchBlockFact, ManuscriptSearchChapterFact},
};

struct Store(Mutex<usize>);

impl ExportHumanReadableManuscriptStore for Store {
    async fn export_human_readable_manuscript(
        &self,
        command: &ExportHumanReadableManuscriptCommand,
    ) -> Result<ExportHumanReadableManuscriptAdmission, ExportHumanReadableManuscriptError> {
        *self.0.lock().unwrap() += 1;
        Ok(ExportHumanReadableManuscriptAdmission {
            ids: command.ids.clone(),
            export_id: command.export_id.clone(),
            effect: ExportHumanReadableManuscriptAdmissionEffect::Admitted {
                source_snapshot: Box::new(snapshot()),
            },
        })
    }
}

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

fn command() -> ExportHumanReadableManuscriptCommand {
    let project_scope = owned_scope();
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    let canonical_command_bytes =
        br#"{"command_schema":"storyos.command.export-human-readable-manuscript.request.v1"}"#
            .to_vec();
    let command_digest = {
        use sha2::{Digest as _, Sha256};
        let value = Sha256::digest(&canonical_command_bytes).iter().fold(
            String::with_capacity(64),
            |mut value, byte| {
                use std::fmt::Write as _;
                write!(value, "{byte:02x}").expect("writing to String cannot fail");
                value
            },
        );
        format!("sha256:{HUMAN_READABLE_EXPORT_DIGEST_PROFILE}:{value}")
    };
    ExportHumanReadableManuscriptCommand {
        project_scope: project_scope.clone(),
        client_binding: client_binding.clone(),
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope,
            client_session_binding_digest: client_binding.binding_ref,
            client_session_generation: 1,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
            limit_profile_revision: "limit".to_owned(),
            challenge_rate_policy_revision: "rate".to_owned(),
            method: "POST".to_owned(),
            route_template: HUMAN_READABLE_EXPORT_ROUTE.to_owned(),
            command_schema: HUMAN_READABLE_EXPORT_REQUEST_SCHEMA.to_owned(),
            command_kind: HUMAN_READABLE_EXPORT_COMMAND_KIND.to_owned(),
            canonical_command_digest: command_digest,
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes,
        correlation_id: "correlation".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
        export_id: "export".to_owned(),
    }
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

#[tokio::test]
async fn an_exact_export_binding_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let admission = request_human_readable_manuscript_export(&store, &command())
        .await
        .unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(admission.export_id, "export");
}

#[tokio::test]
async fn a_changed_export_binding_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.challenge_binding.command_kind = "createVolume".to_owned();
    assert!(matches!(
        request_human_readable_manuscript_export(&store, &changed).await,
        Err(ExportHumanReadableManuscriptError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
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
