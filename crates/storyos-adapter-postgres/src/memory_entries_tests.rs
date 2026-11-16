//! The records of Agent Memory (ADR 0094 section 3): the grants of the runtime role, the Scope
//! of a read, and the Project Export Archive.

use serde_json::{Value, json};
use storyos_application::{
    IssueProjectCommandChallenge, ProjectScope, VerifiedExportArchive, claim_next_archive_export,
    complete_archive_export, get_verified_export_archive, issue_project_command_challenge,
};
use storyos_core::{MemoryEntryType, MemoryIndexEntry};
use tokio_postgres::{Client, NoTls};

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::command_sequence::tests::seed_project;
use crate::project_archive_build::tests::{export_command, zip_store_files};

/// One version row: the entry number, the version, the Memory revision, the state, the type, the
/// title, and the description. The text is the description.
pub(crate) type Version<'a> = (u64, u64, u64, &'a str, &'a str, &'a str, &'a str);

/// Writes each version of `versions` in the Project of `scope` with SQL, and moves the head of
/// the Project to the highest revision and the next number. The values are test constants.
pub(crate) async fn write_versions(admin: &Client, scope: &ProjectScope, versions: &[Version<'_>]) {
    let (owner, project) = (scope.owner_user_id.as_ref(), scope.project_id.as_ref());
    for &(number, version, revision, state, kind, title, description) in versions {
        admin
            .batch_execute(&format!(
                "INSERT INTO storyos.memory_entry_versions
                     (owner_user_id, project_id, memory_entry_id, entry_number, version,
                      memory_revision, state, entry_type, origin, title, description, text, cause,
                      source_references)
                 VALUES ('{owner}', '{project}', md5('{project}:{number}')::uuid, {number},
                         {version}, {revision}, '{state}', '{kind}', 'author_request', '{title}',
                         '{description}', '{description}', '{{}}', '[]');
                 INSERT INTO storyos.project_memory_heads AS head
                 VALUES ('{owner}', '{project}', {revision}, {number} + 1)
                 ON CONFLICT (owner_user_id, project_id) DO UPDATE
                    SET memory_revision = GREATEST(head.memory_revision, EXCLUDED.memory_revision),
                        next_entry_number =
                          GREATEST(head.next_entry_number, EXCLUDED.next_entry_number)"
            ))
            .await
            .unwrap();
    }
}

/// The Project Export Archive of the Project of `scope`, with the export identities of `lead`:
/// each version row as its number, version, Memory revision, and state, in text order, and each
/// head row as its Memory revision and next number.
pub(crate) async fn archived(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    lead: &str,
) -> (Vec<Value>, Vec<Value>) {
    let (export, input) = export_command(scope, lead);
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: export.challenge_binding.clone(),
            nonce: format!("nonce-export-{lead}"),
            nonce_digest: export.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    store.export_project_archive(&export, &input).await.unwrap();
    let claimed = claim_next_archive_export(store).await.unwrap().unwrap();
    complete_archive_export(store, &claimed).await.unwrap();
    let VerifiedExportArchive::Ready(zip) =
        get_verified_export_archive(store, scope, &input.export_id)
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

    (versions, heads)
}

/// The result of each statement of `statements` for the runtime role in the Scope of `scope`:
/// the count of the rows or the SQL state of the refusal.
async fn as_runtime(scope: &ProjectScope, statements: &[&str]) -> Vec<Result<u64, String>> {
    let url = std::env::var("STORYOS_TEST_DATABASE_URL").expect("run through the script");
    let (mut runtime, connection) = tokio_postgres::connect(&url, NoTls).await.unwrap();
    tokio::spawn(connection);
    let mut results = Vec::new();
    for statement in statements {
        let transaction = runtime.transaction().await.unwrap();
        let scoped = "SELECT set_config('storyos.owner_user_id', $1, true), \
                      set_config('storyos.project_id', $2, true)";
        let values = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
        transaction
            .execute(scoped, &[&values[0], &values[1]])
            .await
            .unwrap();
        let result = transaction.execute(*statement, &[]).await;
        results.push(result.map_err(|error| {
            error
                .code()
                .map(|code| code.code().to_owned())
                .unwrap_or_default()
        }));
    }
    results
}

/// The grants of ADR 0094 section 3, and the Scope of a read: two Projects of one User each read
/// only their own entries, and a read in the Scope of one Project sees no row of the other.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_runtime_role_cannot_change_a_version_and_each_project_reads_only_its_entries() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let first = seed_project(&store, "c510").await;
    let second = seed_project(&store, "c520").await;
    let versions = [(1, 1, 1, "current", "character", "林舟", "船夫。")];
    write_versions(&admin, &first, &versions).await;
    let versions = [(1, 1, 1, "current", "other", "渡口", "旧渡口。")];
    write_versions(&admin, &second, &versions).await;
    let index = async |scope: &ProjectScope| {
        let entries = crate::memory_entries::read_memory_index(&admin, scope, 1).await;
        entries.unwrap()
    };
    let entry = |entry_type, title: &str, description: &str| MemoryIndexEntry {
        number: 1,
        entry_type,
        title: title.to_owned(),
        description: description.to_owned(),
    };
    let statements = [
        "SELECT 1 FROM storyos.memory_entry_versions
          WHERE project_id <> current_setting('storyos.project_id')::uuid",
        "SELECT 1 FROM storyos.memory_entry_versions",
        "UPDATE storyos.memory_entry_versions SET title = 'x'",
        "DELETE FROM storyos.memory_entry_versions",
        "DELETE FROM storyos.project_memory_heads",
    ];
    let refused = || Err("42501".to_owned());

    assert_eq!(
        (
            index(&first).await,
            index(&second).await,
            as_runtime(&first, &statements).await
        ),
        (
            vec![entry(MemoryEntryType::Character, "林舟", "船夫。")],
            vec![entry(MemoryEntryType::Other, "渡口", "旧渡口。")],
            vec![Ok(0), Ok(1), refused(), refused(), refused()],
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
    let scope = seed_project(&store, "c530").await;
    let versions = [
        (1, 1, 1, "current", "foreshadowing", "铃", "铃声。"),
        (2, 1, 2, "current", "character", "阿青", "妹妹。"),
        (1, 2, 3, "deleted", "foreshadowing", "铃", "铃声。"),
    ];
    write_versions(&admin, &scope, &versions).await;
    let (versions, heads) = archived(&store, &scope, "cf").await;

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
