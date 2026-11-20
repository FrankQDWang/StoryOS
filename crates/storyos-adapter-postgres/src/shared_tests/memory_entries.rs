//! The shared test support from `memory_entries_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::project_archive_build::{export_command, zip_store_files};
use serde_json::{Value, json};
use storyos_application::{
    IssueProjectCommandChallenge, ProjectScope, VerifiedExportArchive, claim_next_archive_export,
    complete_archive_export, get_verified_export_archive, issue_project_command_challenge,
};
use tokio_postgres::{Client, NoTls};

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

/// The path and the bytes of each file of the Project Export Archive of the Project of `scope`,
/// with the export identities of `lead`.
pub(crate) async fn archived_files(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    lead: &str,
) -> Vec<(String, Vec<u8>)> {
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
    zip_store_files(&zip)
}

/// The bytes of the file `canonical/<name>.json` of `files`.
pub(crate) fn canonical<'a>(files: &'a [(String, Vec<u8>)], name: &str) -> &'a [u8] {
    let path = format!("canonical/{name}.json");
    &files.iter().find(|(file, _)| *file == path).unwrap().1
}

/// The rows of each file `canonical/<name>.json` of `names`, in order, of the Project Export
/// Archive of the Project of `scope`, with the export identities of `lead`.
pub(crate) async fn archived_rows<const N: usize>(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    lead: &str,
    names: [&str; N],
) -> [Vec<Value>; N] {
    let files = archived_files(store, scope, lead).await;
    names.map(|name| serde_json::from_slice(canonical(&files, name)).unwrap())
}

/// The Project Export Archive of the Project of `scope`, with the export identities of `lead`:
/// each version row as its number, version, Memory revision, and state, in text order, and each
/// head row as its Memory revision and next number.
pub(crate) async fn archived(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    lead: &str,
) -> (Vec<Value>, Vec<Value>) {
    let [versions, heads] = archived_rows(
        store,
        scope,
        lead,
        ["memory_entry_versions", "project_memory_heads"],
    )
    .await;
    let mut versions: Vec<Value> = versions
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
    let heads: Vec<Value> = heads
        .iter()
        .map(|row| json!([row["memory_revision"], row["next_entry_number"]]))
        .collect();

    (versions, heads)
}

/// The result of each statement of `statements` for the runtime role in the Scope of `scope`:
/// the count of the rows or the SQL state of the refusal.
pub(crate) async fn as_runtime(
    scope: &ProjectScope,
    statements: &[&str],
) -> Vec<Result<u64, String>> {
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
