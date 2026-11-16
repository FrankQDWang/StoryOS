//! The records of Agent Memory (ADR 0094 section 3): the grants of the runtime role, the Scope
//! of a read, and the Project Export Archive.

use serde_json::{Value, json};
use storyos_application::{
    IssueProjectCommandChallenge, ProjectScope, VerifiedExportArchive, claim_next_archive_export,
    complete_archive_export, get_verified_export_archive, issue_project_command_challenge,
};
use storyos_core::{MemoryEntryType, MemoryIndexEntry};
use tokio_postgres::{Client, NoTls};

use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_steering::tests::queued;
use crate::project_archive_build::tests::{export_command, zip_store_files};

/// One version row: the entry number, the version, the Memory revision, the state, the type, the
/// title, and the description. The text is the description.
pub(crate) type Version<'a> = (u64, u64, u64, &'a str, &'a str, &'a str, &'a str);

/// Writes each version of `versions` in the Project of `scope` with SQL, and moves the head of
/// the Project to the highest revision and the next number.
pub(crate) async fn write_versions(admin: &Client, scope: &ProjectScope, versions: &[Version<'_>]) {
    for &(number, version, revision, state, entry_type, title, description) in versions {
        let (number, version, revision) = (
            number.to_string(),
            version.to_string(),
            revision.to_string(),
        );
        admin
            .execute(
                "INSERT INTO storyos.memory_entry_versions
                     (owner_user_id, project_id, memory_entry_id, entry_number, version,
                      memory_revision, state, entry_type, origin, title, description, text, cause,
                      source_references)
                 VALUES ($1::text::uuid, $2::text::uuid, md5($2 || ':' || $3)::uuid,
                         $3::text::numeric, $4::text::numeric, $5::text::numeric, $6, $7,
                         'author_request', $8, $9, $9, '{}'::jsonb, '[]'::jsonb)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &number,
                    &version,
                    &revision,
                    &state,
                    &entry_type,
                    &title,
                    &description,
                ],
            )
            .await
            .unwrap();
        admin
            .execute(
                "INSERT INTO storyos.project_memory_heads AS head
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::numeric + 1)
                 ON CONFLICT (owner_user_id, project_id) DO UPDATE
                    SET memory_revision = GREATEST(head.memory_revision, EXCLUDED.memory_revision),
                        next_entry_number = GREATEST(head.next_entry_number,
                                                     EXCLUDED.next_entry_number)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &revision,
                    &number,
                ],
            )
            .await
            .unwrap();
    }
}

/// The result of `statement` for the runtime role in the Scope of `scope`: the count of the rows
/// or the SQL state of the refusal.
async fn as_runtime(
    runtime: &mut Client,
    scope: &ProjectScope,
    statement: &str,
) -> Result<u64, String> {
    let transaction = runtime.transaction().await.unwrap();
    transaction
        .execute(
            "SELECT set_config('storyos.owner_user_id', $1, true),
                    set_config('storyos.project_id', $2, true)",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    transaction.execute(statement, &[]).await.map_err(|error| {
        error
            .code()
            .map(|code| code.code().to_owned())
            .unwrap_or_default()
    })
}

/// The grants of ADR 0094 section 3, and the Scope of a read: two Projects of one User each read
/// only their own entries, and a read in the Scope of one Project sees no row of the other.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_runtime_role_cannot_change_a_version_and_each_project_reads_only_its_entries() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (_, admin) = stores().await;
    let (mut runtime, connection) = tokio_postgres::connect(
        &std::env::var("STORYOS_TEST_DATABASE_URL")
            .expect("run through scripts/verify-project-scope.sh"),
        NoTls,
    )
    .await
    .unwrap();
    tokio::spawn(connection);
    let (store, _) = stores().await;
    let (first, _, _) = queued(&store, "dd1").await;
    let (second, _, _) = queued(&store, "dd2").await;
    write_versions(
        &admin,
        &first,
        &[(1, 1, 1, "current", "character", "林舟", "船夫。")],
    )
    .await;
    write_versions(
        &admin,
        &second,
        &[(1, 1, 1, "current", "other", "渡口", "旧渡口。")],
    )
    .await;
    let index = |scope: ProjectScope| {
        let admin = &admin;
        async move {
            crate::memory_entries::read_memory_index(admin, &scope, 1)
                .await
                .unwrap()
        }
    };
    let rows = "SELECT 1 FROM storyos.memory_entry_versions WHERE project_id <> \
                current_setting('storyos.project_id')::uuid";
    let entry = |entry_type, title: &str, description: &str| MemoryIndexEntry {
        number: 1,
        entry_type,
        title: title.to_owned(),
        description: description.to_owned(),
    };

    assert_eq!(
        (
            index(first.clone()).await,
            index(second.clone()).await,
            as_runtime(&mut runtime, &first, rows).await,
            as_runtime(
                &mut runtime,
                &first,
                "SELECT 1 FROM storyos.memory_entry_versions"
            )
            .await,
            as_runtime(
                &mut runtime,
                &first,
                "UPDATE storyos.memory_entry_versions SET title = 'x'"
            )
            .await,
            as_runtime(
                &mut runtime,
                &first,
                "DELETE FROM storyos.memory_entry_versions"
            )
            .await,
            as_runtime(
                &mut runtime,
                &first,
                "DELETE FROM storyos.project_memory_heads"
            )
            .await,
        ),
        (
            vec![entry(MemoryEntryType::Character, "林舟", "船夫。")],
            vec![entry(MemoryEntryType::Other, "渡口", "旧渡口。")],
            Ok(0),
            Ok(1),
            Err("42501".to_owned()),
            Err("42501".to_owned()),
            Err("42501".to_owned()),
        )
    );
}

/// The archive of a Project with entries has each version row, the deleted versions included,
/// and the head of the Project.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_archive_of_a_project_has_each_version_and_its_memory_head() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, _) = queued(&store, "dd3").await;
    write_versions(
        &admin,
        &scope,
        &[
            (
                1,
                1,
                1,
                "current",
                "foreshadowing",
                "铜铃",
                "铃声预示船沉。",
            ),
            (2, 1, 2, "current", "character", "阿青", "林舟的妹妹。"),
            (
                1,
                2,
                3,
                "deleted",
                "foreshadowing",
                "铜铃",
                "铃声预示船沉。",
            ),
        ],
    )
    .await;
    let (export, input) = export_command(&scope, "dd");
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: export.challenge_binding.clone(),
            nonce: "nonce-export-dd".to_owned(),
            nonce_digest: export.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    store.export_project_archive(&export, &input).await.unwrap();
    let claimed = claim_next_archive_export(&store).await.unwrap().unwrap();
    complete_archive_export(&store, &claimed).await.unwrap();
    let VerifiedExportArchive::Ready(zip) =
        get_verified_export_archive(&store, &scope, &input.export_id)
            .await
            .unwrap()
    else {
        panic!("the Project Export must return verified ZIP bytes");
    };
    let files = zip_store_files(&zip);
    let packed = |name: &str| -> Vec<Value> {
        let rows = &files.iter().find(|(file, _)| file == name).unwrap().1;
        serde_json::from_slice(rows).unwrap()
    };
    let mut versions: Vec<Value> = packed("canonical/memory_entry_versions.json")
        .iter()
        .map(|row| {
            json!([
                row["entry_number"],
                row["version"],
                row["memory_revision"],
                row["state"]
            ])
        })
        .collect();
    versions.sort_by_key(Value::to_string);
    let heads: Vec<Value> = packed("canonical/project_memory_heads.json")
        .iter()
        .map(|row| json!([row["memory_revision"], row["next_entry_number"]]))
        .collect();

    assert_eq!(
        (versions, heads),
        (
            vec![
                json!([1, 1, 1, "current"]),
                json!([1, 2, 3, "deleted"]),
                json!([2, 1, 2, "current"]),
            ],
            vec![json!([3, 3])],
        )
    );
}
