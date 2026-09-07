use std::sync::LazyLock;

use serde_json::json;

use crate::Release1ProtocolProfile;
use crate::release1::{
    CREATE_EDITOR_SESSION, CREATE_PROJECT_COMMAND_CHALLENGE, GET_CHAPTER, GET_EDITOR_SESSION,
    GET_PROJECT, GET_PROTOCOL_PROFILE, PUBLIC_PROTOCOL_RELEASE, QueryOperation,
};
use crate::release1_archive_project::ARCHIVE_PROJECT;
use crate::release1_archive_project_artifacts as archive_project_artifacts;
use crate::release1_author_edit::APPLY_AUTHOR_EDIT;
use crate::release1_author_edit_artifacts as author_edit_artifacts;
use crate::release1_author_edit_outcome::GET_APPLY_AUTHOR_EDIT_OUTCOME;
use crate::release1_author_edit_outcome_artifacts as author_edit_outcome_artifacts;
use crate::release1_create_chapter::CREATE_CHAPTER;
use crate::release1_create_chapter_artifacts as create_chapter_artifacts;
use crate::release1_create_project::{CREATE_PROJECT, CREATE_PROJECT_CHALLENGE};
use crate::release1_create_project_artifacts as create_project_artifacts;
use crate::release1_create_volume::CREATE_VOLUME;
use crate::release1_create_volume_artifacts as create_volume_artifacts;
use crate::release1_delete_chapter::DELETE_CHAPTER;
use crate::release1_delete_chapter_artifacts as delete_chapter_artifacts;
use crate::release1_delete_volume::DELETE_VOLUME;
use crate::release1_delete_volume_artifacts as delete_volume_artifacts;
use crate::release1_list_projects::LIST_PROJECTS;
use crate::release1_list_projects_artifacts as list_projects_artifacts;
use crate::release1_manuscript_search::SEARCH_MANUSCRIPT;
use crate::release1_manuscript_search_artifacts as manuscript_search_artifacts;
use crate::release1_manuscript_statistics::GET_STATISTICS;
use crate::release1_manuscript_statistics_artifacts as manuscript_statistics_artifacts;
use crate::release1_manuscript_tree::GET_MANUSCRIPT_TREE;
use crate::release1_manuscript_tree_artifacts as manuscript_tree_artifacts;
use crate::release1_project_export::EXPORT_PROJECT_ARCHIVE;
use crate::release1_project_export_artifacts as project_export_artifacts;
use crate::release1_project_export_query::GET_EXPORT_OPERATION;
use crate::release1_project_export_query_artifacts as project_export_query_artifacts;
use crate::release1_readable_export::EXPORT_HUMAN_READABLE_MANUSCRIPT;
use crate::release1_readable_export_artifacts as readable_export_artifacts;
use crate::release1_readable_export_query::GET_HUMAN_READABLE_MANUSCRIPT_EXPORT;
use crate::release1_readable_export_query_artifacts as readable_export_query_artifacts;
use crate::release1_set_current_chapter::SET_CURRENT_CHAPTER;
use crate::release1_set_current_chapter_artifacts as set_current_chapter_artifacts;
use crate::release1_snapshot::{ACTIVITY_STREAM, GET_SNAPSHOT};
use crate::release1_snapshot_artifacts as snapshot_artifacts;
use crate::release1_takeover::TAKE_OVER_PROJECT_WRITER;
use crate::release1_takeover_artifacts as takeover_artifacts;
use crate::release1_undo_latest_author_action::UNDO_LATEST_AUTHOR_ACTION;
use crate::release1_undo_latest_author_action_artifacts as undo_latest_author_action_artifacts;
use crate::release1_update_chapter::UPDATE_CHAPTER;
use crate::release1_update_chapter_artifacts as update_chapter_artifacts;
use crate::release1_update_project::UPDATE_PROJECT;
use crate::release1_update_project_artifacts as update_project_artifacts;
use crate::release1_update_volume::UPDATE_VOLUME;
use crate::release1_update_volume_artifacts as update_volume_artifacts;

use super::{
    BOUNDARY_PROFILE_PATH, CHALLENGE_FIXTURE_PATHS, CHAPTER_FIXTURE_PATHS,
    CREATE_EDITOR_SESSION_FIXTURE_PATHS, FIXTURE_DIGEST_PLACEHOLDER,
    GET_EDITOR_SESSION_FIXTURE_PATHS, GOLDEN_PROFILE_PATH, GeneratedFile, INVALID_PROFILE_PATH,
    PROJECT_FIXTURE_PATHS, boundary_challenge_fixture_bytes, boundary_chapter_fixture_bytes,
    boundary_create_editor_session_fixture_bytes, boundary_get_editor_session_fixture_bytes,
    boundary_profile_bytes, boundary_project_fixture_bytes, challenge_fixture_bytes,
    chapter_fixture_bytes, create_editor_session_fixture_bytes, get_editor_session_fixture_bytes,
    golden_profile_bytes, invalid_challenge_fixture_bytes, invalid_chapter_fixture_bytes,
    invalid_create_editor_session_fixture_bytes, invalid_get_editor_session_fixture_bytes,
    invalid_profile_bytes, invalid_project_fixture_bytes, json_bytes, project_fixture_bytes,
};

/// One ordered owner of active Release 1 fixture membership.
/// Catalog paths, catalog entries, generated golden files, and digest inputs
/// all derive from this list.
struct FixtureMembership {
    path: &'static str,
    fixture_id: &'static str,
    classification: &'static str,
    operation_id: &'static str,
    bytes: fn(&Release1ProtocolProfile) -> Vec<u8>,
}

fn fixture_corpus_membership() -> &'static [FixtureMembership] {
    static MEMBERSHIP: LazyLock<Vec<FixtureMembership>> =
        LazyLock::new(build_fixture_corpus_membership);
    &MEMBERSHIP
}

pub(super) fn fixture_catalog_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    let membership = fixture_corpus_membership();
    json_bytes(&json!({
        "schema_id": "storyos.fixture-catalog.release-1.v1",
        "public_protocol_release": PUBLIC_PROTOCOL_RELEASE,
        "corpus_digest": profile.release_identity.fixture_corpus_digest,
        "digest_scope": {
            "paths": membership.iter().map(|entry| entry.path).collect::<Vec<_>>(),
            "normalization": format!(
                "replace every release_identity.fixture_corpus_digest with {FIXTURE_DIGEST_PLACEHOLDER}"
            )
        },
        "fixtures": membership.iter().map(|entry| json!({
            "fixture_id": entry.fixture_id,
            "classification": entry.classification,
            "operation_id": entry.operation_id,
            "path": entry.path
        })).collect::<Vec<_>>()
    }))
}

pub(super) fn fixture_corpus_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    fixture_corpus_membership()
        .iter()
        .flat_map(|entry| (entry.bytes)(profile))
        .collect()
}

pub(super) fn generated_fixture_files(profile: &Release1ProtocolProfile) -> Vec<GeneratedFile> {
    fixture_corpus_membership()
        .iter()
        .map(|entry| (entry.path, (entry.bytes)(profile)))
        .collect()
}

fn fixture_triple(
    paths: [&'static str; 3],
    operation: &QueryOperation,
    producers: [fn(&Release1ProtocolProfile) -> Vec<u8>; 3],
) -> [FixtureMembership; 3] {
    [
        FixtureMembership {
            path: paths[0],
            fixture_id: operation.fixtures[0],
            classification: "positive",
            operation_id: operation.operation_id,
            bytes: producers[0],
        },
        FixtureMembership {
            path: paths[1],
            fixture_id: operation.fixtures[1],
            classification: "invalid",
            operation_id: operation.operation_id,
            bytes: producers[1],
        },
        FixtureMembership {
            path: paths[2],
            fixture_id: operation.fixtures[2],
            classification: "boundary",
            operation_id: operation.operation_id,
            bytes: producers[2],
        },
    ]
}

fn build_fixture_corpus_membership() -> Vec<FixtureMembership> {
    let mut membership = Vec::with_capacity(93);
    membership.extend(fixture_triple(
        [
            GOLDEN_PROFILE_PATH,
            INVALID_PROFILE_PATH,
            BOUNDARY_PROFILE_PATH,
        ],
        &GET_PROTOCOL_PROFILE,
        [
            golden_profile_bytes,
            invalid_profile_bytes,
            boundary_profile_bytes,
        ],
    ));
    membership.extend(fixture_triple(
        PROJECT_FIXTURE_PATHS,
        &GET_PROJECT,
        [
            |_profile| project_fixture_bytes(),
            |_profile| invalid_project_fixture_bytes(),
            |_profile| boundary_project_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        CHAPTER_FIXTURE_PATHS,
        &GET_CHAPTER,
        [
            |_profile| chapter_fixture_bytes(),
            |_profile| invalid_chapter_fixture_bytes(),
            |_profile| boundary_chapter_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        CHALLENGE_FIXTURE_PATHS,
        &CREATE_PROJECT_COMMAND_CHALLENGE,
        [
            |_profile| challenge_fixture_bytes(),
            |_profile| invalid_challenge_fixture_bytes(),
            |_profile| boundary_challenge_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        create_project_artifacts::CHALLENGE_FIXTURE_PATHS,
        &CREATE_PROJECT_CHALLENGE,
        [
            |_profile| create_project_artifacts::challenge_fixture_bytes(),
            |_profile| create_project_artifacts::challenge_invalid_fixture_bytes(),
            |_profile| create_project_artifacts::challenge_boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        create_project_artifacts::FIXTURE_PATHS,
        &CREATE_PROJECT,
        [
            |_profile| create_project_artifacts::fixture_bytes(),
            |_profile| create_project_artifacts::invalid_fixture_bytes(),
            |_profile| create_project_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        list_projects_artifacts::FIXTURE_PATHS,
        &LIST_PROJECTS,
        [
            |_profile| list_projects_artifacts::fixture_bytes(),
            |_profile| list_projects_artifacts::invalid_fixture_bytes(),
            |_profile| list_projects_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        manuscript_tree_artifacts::FIXTURE_PATHS,
        &GET_MANUSCRIPT_TREE,
        [
            |_profile| manuscript_tree_artifacts::fixture_bytes(),
            |_profile| manuscript_tree_artifacts::invalid_fixture_bytes(),
            |_profile| manuscript_tree_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        manuscript_search_artifacts::FIXTURE_PATHS,
        &SEARCH_MANUSCRIPT,
        [
            |_profile| manuscript_search_artifacts::fixture_bytes(),
            |_profile| manuscript_search_artifacts::invalid_fixture_bytes(),
            |_profile| manuscript_search_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        manuscript_statistics_artifacts::FIXTURE_PATHS,
        &GET_STATISTICS,
        [
            |_profile| manuscript_statistics_artifacts::fixture_bytes(),
            |_profile| manuscript_statistics_artifacts::invalid_fixture_bytes(),
            |_profile| manuscript_statistics_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        readable_export_artifacts::FIXTURE_PATHS,
        &EXPORT_HUMAN_READABLE_MANUSCRIPT,
        [
            |_profile| readable_export_artifacts::fixture_bytes(),
            |_profile| readable_export_artifacts::invalid_fixture_bytes(),
            |_profile| readable_export_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        readable_export_query_artifacts::FIXTURE_PATHS,
        &GET_HUMAN_READABLE_MANUSCRIPT_EXPORT,
        [
            |_profile| readable_export_query_artifacts::fixture_bytes(),
            |_profile| readable_export_query_artifacts::invalid_fixture_bytes(),
            |_profile| readable_export_query_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        project_export_artifacts::FIXTURE_PATHS,
        &EXPORT_PROJECT_ARCHIVE,
        [
            |_profile| project_export_artifacts::fixture_bytes(),
            |_profile| project_export_artifacts::invalid_fixture_bytes(),
            |_profile| project_export_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        project_export_query_artifacts::FIXTURE_PATHS,
        &GET_EXPORT_OPERATION,
        [
            |_profile| project_export_query_artifacts::fixture_bytes(),
            |_profile| project_export_query_artifacts::invalid_fixture_bytes(),
            |_profile| project_export_query_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        update_project_artifacts::FIXTURE_PATHS,
        &UPDATE_PROJECT,
        [
            |_profile| update_project_artifacts::fixture_bytes(),
            |_profile| update_project_artifacts::invalid_fixture_bytes(),
            |_profile| update_project_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        archive_project_artifacts::FIXTURE_PATHS,
        &ARCHIVE_PROJECT,
        [
            |_profile| archive_project_artifacts::fixture_bytes(),
            |_profile| archive_project_artifacts::invalid_fixture_bytes(),
            |_profile| archive_project_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        create_volume_artifacts::FIXTURE_PATHS,
        &CREATE_VOLUME,
        [
            |_profile| create_volume_artifacts::fixture_bytes(),
            |_profile| create_volume_artifacts::invalid_fixture_bytes(),
            |_profile| create_volume_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        update_volume_artifacts::FIXTURE_PATHS,
        &UPDATE_VOLUME,
        [
            |_profile| update_volume_artifacts::fixture_bytes(),
            |_profile| update_volume_artifacts::invalid_fixture_bytes(),
            |_profile| update_volume_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        delete_volume_artifacts::FIXTURE_PATHS,
        &DELETE_VOLUME,
        [
            |_profile| delete_volume_artifacts::fixture_bytes(),
            |_profile| delete_volume_artifacts::invalid_fixture_bytes(),
            |_profile| delete_volume_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        create_chapter_artifacts::FIXTURE_PATHS,
        &CREATE_CHAPTER,
        [
            |_profile| create_chapter_artifacts::fixture_bytes(),
            |_profile| create_chapter_artifacts::invalid_fixture_bytes(),
            |_profile| create_chapter_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        update_chapter_artifacts::FIXTURE_PATHS,
        &UPDATE_CHAPTER,
        [
            |_profile| update_chapter_artifacts::fixture_bytes(),
            |_profile| update_chapter_artifacts::invalid_fixture_bytes(),
            |_profile| update_chapter_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        delete_chapter_artifacts::FIXTURE_PATHS,
        &DELETE_CHAPTER,
        [
            |_profile| delete_chapter_artifacts::fixture_bytes(),
            |_profile| delete_chapter_artifacts::invalid_fixture_bytes(),
            |_profile| delete_chapter_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        set_current_chapter_artifacts::FIXTURE_PATHS,
        &SET_CURRENT_CHAPTER,
        [
            |_profile| set_current_chapter_artifacts::fixture_bytes(),
            |_profile| set_current_chapter_artifacts::invalid_fixture_bytes(),
            |_profile| set_current_chapter_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        undo_latest_author_action_artifacts::FIXTURE_PATHS,
        &UNDO_LATEST_AUTHOR_ACTION,
        [
            |_profile| undo_latest_author_action_artifacts::fixture_bytes(),
            |_profile| undo_latest_author_action_artifacts::invalid_fixture_bytes(),
            |_profile| undo_latest_author_action_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        CREATE_EDITOR_SESSION_FIXTURE_PATHS,
        &CREATE_EDITOR_SESSION,
        [
            |_profile| create_editor_session_fixture_bytes(),
            |_profile| invalid_create_editor_session_fixture_bytes(),
            |_profile| boundary_create_editor_session_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        GET_EDITOR_SESSION_FIXTURE_PATHS,
        &GET_EDITOR_SESSION,
        [
            |_profile| get_editor_session_fixture_bytes(),
            |_profile| invalid_get_editor_session_fixture_bytes(),
            |_profile| boundary_get_editor_session_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        author_edit_artifacts::FIXTURE_PATHS,
        &APPLY_AUTHOR_EDIT,
        [
            |_profile| author_edit_artifacts::fixture_bytes(),
            |_profile| author_edit_artifacts::invalid_fixture_bytes(),
            |_profile| author_edit_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        author_edit_outcome_artifacts::FIXTURE_PATHS,
        &GET_APPLY_AUTHOR_EDIT_OUTCOME,
        [
            |_profile| author_edit_outcome_artifacts::fixture_bytes(),
            |_profile| author_edit_outcome_artifacts::invalid_fixture_bytes(),
            |_profile| author_edit_outcome_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS,
        &GET_SNAPSHOT,
        [
            |_profile| snapshot_artifacts::snapshot_fixture_bytes(),
            |_profile| snapshot_artifacts::snapshot_invalid_fixture_bytes(),
            |_profile| snapshot_artifacts::snapshot_boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS,
        &ACTIVITY_STREAM,
        [
            |_profile| snapshot_artifacts::activity_stream_fixture_bytes(),
            |_profile| snapshot_artifacts::activity_stream_invalid_fixture_bytes(),
            |_profile| snapshot_artifacts::activity_stream_boundary_fixture_bytes(),
        ],
    ));
    membership.extend(fixture_triple(
        takeover_artifacts::FIXTURE_PATHS,
        &TAKE_OVER_PROJECT_WRITER,
        [
            |_profile| takeover_artifacts::fixture_bytes(),
            |_profile| takeover_artifacts::invalid_fixture_bytes(),
            |_profile| takeover_artifacts::boundary_fixture_bytes(),
        ],
    ));
    membership
}
