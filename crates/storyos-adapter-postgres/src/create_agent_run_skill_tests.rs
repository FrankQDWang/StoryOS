//! The SkillPackage Snapshots and the Skill catalog that `createAgentRun` writes (ADR 0089
//! section 2).

use std::sync::Arc;

use storyos_application::ProjectCommandError;
use storyos_core::{
    SKILL_PACKAGE_BYTE_LIMIT, SKILL_PACKAGE_FILE_LIMIT, SKILL_SET_LIMIT, SkillFile, SkillFileInput,
    SkillPackage, SkillPackageReference, SkillSet,
};
use tokio_postgres::error::SqlState;

use super::*;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::create_agent_run::{
    CHAPTER, USER_A, admitted, connect_admin, issue_run, prepare_project, refused, run_command,
};
use crate::shared_tests::create_agent_run_skill::{fixture, package, set, skill_md};

const DIALOGUE_DIGEST: &str =
    "sha256:e03399e0385ea904f5c37265661480113ef0125e3c7f837cf04875d5cbe8178b";
const COMMS_DIGEST: &str =
    "sha256:3ccc24f2e8e7c47369c7a2cf8456271c6338b8a83d1917756c3584b2466c67e9";

fn project(lead: &str) -> String {
    format!("018f0000-0000-7001-8000-0000000011{lead}")
}

fn chapter(lead: &str) -> String {
    format!("018f0000-0000-7001-8000-00000000c1{lead}")
}

/// One createAgentRun in a new Conversation of `project` that offers `skills`. The command ids
/// derive from the three-digit `suffix`.
fn skill_run(
    project: &str,
    chapter: &str,
    suffix: &str,
    skills: &Arc<SkillSet>,
) -> (
    IssueProjectCommandChallenge,
    CommandCall<CreateAgentRunInput>,
) {
    let issue = issue_run(project, &format!("{suffix}1"));
    let mut call = run_command(
        issue.binding.clone(),
        &issue.nonce_digest,
        &format!("{suffix}2"),
        ConversationSelection::New,
        &format!("018f0000-0000-7001-8000-00000006{suffix}2"),
        chapter,
    );
    call.input.skills = Arc::clone(skills);
    (issue, call)
}

/// Admits one such Run and pauses it, because later HTTP files drain all queued work.
async fn admit(
    store: &PostgresProjectReader,
    admin: &Client,
    project: &str,
    suffix: &str,
    skills: &Arc<SkillSet>,
) -> CommandCall<CreateAgentRunInput> {
    let (issue, call) = skill_run(project, &chapter(&project[34..]), suffix, skills);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    admitted(
        &store
            .create_agent_run(&call.envelope, &call.input)
            .await
            .unwrap(),
    );
    admin
        .execute(
            "UPDATE storyos.agent_runs
                SET status = 'paused', lease_expires_at = NULL, wakeup_pending = false
              WHERE run_id = $1::text::uuid",
            &[&call.input.run_id],
        )
        .await
        .unwrap();
    call
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredSnapshot {
    reference: SkillPackageReference,
    description: String,
    file_count: i32,
    byte_count: i32,
    receipt_id: String,
    files: Vec<SkillFile>,
}

fn expected(package: &SkillPackage, receipt_id: &str) -> StoredSnapshot {
    let bytes: u64 = package.files.iter().map(|file| file.byte_length).sum();
    StoredSnapshot {
        reference: package.reference(),
        description: package.description.clone(),
        file_count: i32::try_from(package.files.len()).unwrap(),
        byte_count: i32::try_from(bytes).unwrap(),
        receipt_id: receipt_id.to_owned(),
        files: package.files.clone(),
    }
}

/// Each Snapshot of `project` with its files, in reference order.
async fn snapshots(admin: &Client, project: &str) -> Vec<StoredSnapshot> {
    let rows = admin
        .query(
            "SELECT skill_name, package_digest, description, file_count, byte_count,
                    receipt_id::text
               FROM storyos.skill_package_snapshots WHERE project_id = $1::text::uuid
              ORDER BY skill_name COLLATE \"C\", package_digest COLLATE \"C\"",
            &[&project],
        )
        .await
        .unwrap();
    let mut snapshots = Vec::new();
    for row in rows {
        let reference = SkillPackageReference {
            name: row.get(/*idx*/ 0),
            package_digest: row.get(/*idx*/ 1),
        };
        let files = admin
            .query(
                "SELECT path, bytes, byte_length, sha256, tokens, readable
                   FROM storyos.skill_package_snapshot_files
                  WHERE project_id = $1::text::uuid AND package_digest = $2
                  ORDER BY path COLLATE \"C\"",
                &[&project, &reference.package_digest],
            )
            .await
            .unwrap()
            .iter()
            .map(|file| SkillFile {
                path: file.get(/*idx*/ 0),
                bytes: file.get(/*idx*/ 1),
                byte_length: u64::try_from(file.get::<_, i32>(/*idx*/ 2)).unwrap(),
                sha256: file.get(/*idx*/ 3),
                tokens: file
                    .get::<_, Option<i64>>(/*idx*/ 4)
                    .map(|tokens| u64::try_from(tokens).unwrap()),
                readable: file.get(/*idx*/ 5),
            })
            .collect();
        snapshots.push(StoredSnapshot {
            reference,
            description: row.get(/*idx*/ 2),
            file_count: row.get(/*idx*/ 3),
            byte_count: row.get(/*idx*/ 4),
            receipt_id: row.get(/*idx*/ 5),
            files,
        });
    }
    snapshots
}

/// The pinned Skill catalog of one Run, in name order.
async fn catalog(admin: &Client, run_id: &str) -> Vec<SkillPackageReference> {
    admin
        .query(
            "SELECT skill_name, package_digest FROM storyos.agent_run_skill_catalog_entries
              WHERE run_id = $1::text::uuid ORDER BY skill_name COLLATE \"C\"",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| SkillPackageReference {
            name: row.get(/*idx*/ 0),
            package_digest: row.get(/*idx*/ 1),
        })
        .collect()
}

/// A runtime-role transaction in the Scope of `project`.
async fn scoped<'a>(runtime: &'a mut Client, project: &str) -> tokio_postgres::Transaction<'a> {
    let transaction = runtime.transaction().await.unwrap();
    transaction
        .execute(
            "SELECT set_config('storyos.owner_user_id', $1, true),
                    set_config('storyos.project_id', $2, true)",
            &[&USER_A, &project],
        )
        .await
        .unwrap();
    transaction
}

/// The rows of the three Skill tables of `project`.
async fn skill_rows(client: &impl tokio_postgres::GenericClient, project: &str) -> i64 {
    client
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.skill_package_snapshots
                      WHERE project_id = $1::text::uuid)
                  + (SELECT count(*) FROM storyos.skill_package_snapshot_files
                      WHERE project_id = $1::text::uuid)
                  + (SELECT count(*) FROM storyos.agent_run_skill_catalog_entries
                      WHERE project_id = $1::text::uuid)",
            &[&project],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_run_pins_the_set_of_its_admission_and_only_new_digests_get_a_snapshot() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (runtime_url, mut admin) = connect_admin().await;
    let store = PostgresProjectReader::new(runtime_url);
    let project = project("a1");
    prepare_project(&mut admin, &store, &project, &chapter("a1"), "11a").await;
    let dialogue = fixture("dialogue-style-guide");
    let comms = fixture("internal-comms");
    assert_eq!(
        [
            dialogue.package_digest.as_str(),
            comms.package_digest.as_str()
        ],
        [DIALOGUE_DIGEST, COMMS_DIGEST]
    );
    let first_set = set(vec![comms.clone(), dialogue.clone()]);
    let first_catalog = vec![dialogue.reference(), comms.reference()];

    let first = admit(&store, &admin, &project, "12a", &first_set).await;
    let receipt = &first.envelope.ids.receipt_id;
    let stored = vec![expected(&dialogue, receipt), expected(&comms, receipt)];
    assert_eq!(snapshots(&admin, &project).await, stored);
    assert_eq!(catalog(&admin, &first.input.run_id).await, first_catalog);

    let second = admit(&store, &admin, &project, "12b", &first_set).await;
    assert_eq!(snapshots(&admin, &project).await, stored);
    assert_eq!(catalog(&admin, &second.input.run_id).await, first_catalog);

    let mut files: Vec<SkillFileInput> = dialogue
        .files
        .iter()
        .map(|file| SkillFileInput {
            path: file.path.clone(),
            bytes: file.bytes.clone(),
        })
        .collect();
    files[0]
        .bytes
        .extend_from_slice("\n5. 每句对白只有一个说话人。\n".as_bytes());
    let changed = package("dialogue-style-guide", files);
    let added = package("plot-notes", vec![skill_md("plot-notes")]);
    let third = admit(
        &store,
        &admin,
        &project,
        "12c",
        &set(vec![changed.clone(), added.clone()]),
    )
    .await;
    let mut all = stored.clone();
    all.extend([
        expected(&changed, &third.envelope.ids.receipt_id),
        expected(&added, &third.envelope.ids.receipt_id),
    ]);
    all.sort_by(|left, right| left.reference.cmp(&right.reference));
    assert_eq!(snapshots(&admin, &project).await, all);
    assert_eq!(
        catalog(&admin, &third.input.run_id).await,
        vec![changed.reference(), added.reference()]
    );
    assert_eq!(catalog(&admin, &first.input.run_id).await, first_catalog);
    assert_eq!(catalog(&admin, &second.input.run_id).await, first_catalog);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_project_has_its_own_snapshots_and_the_runtime_role_cannot_change_a_skill_row() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (runtime_url, mut admin) = connect_admin().await;
    let store = PostgresProjectReader::new(runtime_url.clone());
    let (own, other) = (project("b1"), project("b2"));
    prepare_project(&mut admin, &store, &own, &chapter("b1"), "11b").await;
    prepare_project(&mut admin, &store, &other, &chapter("b2"), "11c").await;
    let dialogue = fixture("dialogue-style-guide");
    let offered = set(vec![dialogue.clone()]);
    let own_run = admit(&store, &admin, &own, "12d", &offered).await;
    let other_run = admit(&store, &admin, &other, "12e", &offered).await;
    assert_eq!(
        snapshots(&admin, &own).await,
        vec![expected(&dialogue, &own_run.envelope.ids.receipt_id)]
    );
    assert_eq!(
        snapshots(&admin, &other).await,
        vec![expected(&dialogue, &other_run.envelope.ids.receipt_id)]
    );

    let (mut runtime, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let transaction = scoped(&mut runtime, &own).await;
    let seen = [
        skill_rows(&transaction, &own).await,
        skill_rows(&transaction, &other).await,
    ];
    transaction.rollback().await.unwrap();
    assert_eq!(seen, [4, 0]);
    for statement in [
        "UPDATE storyos.skill_package_snapshots SET description = description",
        "DELETE FROM storyos.skill_package_snapshots",
        "UPDATE storyos.skill_package_snapshot_files SET readable = readable",
        "DELETE FROM storyos.skill_package_snapshot_files",
        "UPDATE storyos.agent_run_skill_catalog_entries SET skill_name = skill_name",
        "DELETE FROM storyos.agent_run_skill_catalog_entries",
    ] {
        let transaction = scoped(&mut runtime, &own).await;
        let refused = transaction.execute(statement, &[]).await.unwrap_err();
        assert_eq!(
            refused.code(),
            Some(&SqlState::INSUFFICIENT_PRIVILEGE),
            "{statement}"
        );
        transaction.rollback().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn no_offer_a_refusal_an_exact_retry_and_a_failed_transaction_write_no_skill_row() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (runtime_url, mut admin) = connect_admin().await;
    let store = PostgresProjectReader::new(runtime_url);
    let project = project("d1");
    prepare_project(&mut admin, &store, &project, &chapter("d1"), "11d").await;
    let offered = set(vec![fixture("internal-comms")]);

    admit(&store, &admin, &project, "12f", &Arc::default()).await;
    assert_eq!(skill_rows(&admin, &project).await, 0);

    let (issue, call) = skill_run(&project, CHAPTER, "13a", &offered);
    issue_project_command_challenge(&store, &issue)
        .await
        .unwrap();
    let refusal = store.create_agent_run(&call.envelope, &call.input).await;
    assert_eq!(refused(refusal), CreateAgentRunRefusal::InvalidChapterJoin);
    assert_eq!(skill_rows(&admin, &project).await, 0);

    let first = admit(&store, &admin, &project, "13b", &offered).await;
    assert_eq!(skill_rows(&admin, &project).await, 8);
    let retried = store
        .create_agent_run(&first.envelope, &first.input)
        .await
        .unwrap();
    assert_eq!(admitted(&retried).run_id, first.input.run_id);
    assert_eq!(skill_rows(&admin, &project).await, 8);

    let failing = set(vec![package("plot-notes", vec![skill_md("plot-notes")])]);
    let (issue, call) = skill_run(&project, &chapter("d1"), "13c", &failing);
    issue_project_command_challenge(&store, &issue)
        .await
        .unwrap();
    admin
        .batch_execute(
            "ALTER TABLE storyos.agent_run_skill_catalog_entries
               ADD CONSTRAINT test_refuses_each_catalog_entry CHECK (false) NOT VALID",
        )
        .await
        .unwrap();
    let failed = store.create_agent_run(&call.envelope, &call.input).await;
    admin
        .batch_execute(
            "ALTER TABLE storyos.agent_run_skill_catalog_entries
               DROP CONSTRAINT test_refuses_each_catalog_entry",
        )
        .await
        .unwrap();
    assert!(
        matches!(
            failed,
            Err(RefusableCommandError::Command(
                ProjectCommandError::Unavailable(_)
            ))
        ),
        "{failed:?}"
    );
    let runs: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&call.input.run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    assert_eq!((runs, skill_rows(&admin, &project).await), (0, 8));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_first_run_stores_a_set_at_the_limits() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (runtime_url, mut admin) = connect_admin().await;
    let store = PostgresProjectReader::new(runtime_url);
    let project = project("e1");
    prepare_project(&mut admin, &store, &project, &chapter("e1"), "11e").await;
    let mut files = vec![skill_md("large-notes")];
    let others = SKILL_PACKAGE_FILE_LIMIT - 1;
    let budget = usize::try_from(SKILL_PACKAGE_BYTE_LIMIT).unwrap() - files[0].bytes.len();
    files.extend((0..others).map(|index| SkillFileInput {
        path: format!("notes/{index:02}.txt"),
        bytes: vec![
            b'a' + u8::try_from(index % 26).unwrap();
            budget / others + usize::from(index < budget % others)
        ],
    }));
    let large = package("large-notes", files);
    let bytes: u64 = large.files.iter().map(|file| file.byte_length).sum();
    assert_eq!(
        (large.files.len(), bytes),
        (SKILL_PACKAGE_FILE_LIMIT, SKILL_PACKAGE_BYTE_LIMIT)
    );
    let mut packages = vec![large];
    packages.extend((1..SKILL_SET_LIMIT).map(|index| {
        let name = format!("skill-{index:02}");
        package(&name, vec![skill_md(&name)])
    }));
    let offered = set(packages);

    let run = admit(&store, &admin, &project, "13d", &offered).await;
    let receipt = &run.envelope.ids.receipt_id;
    assert_eq!(
        snapshots(&admin, &project).await,
        offered
            .packages()
            .iter()
            .map(|package| expected(package, receipt))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        catalog(&admin, &run.input.run_id).await,
        offered
            .packages()
            .iter()
            .map(SkillPackage::reference)
            .collect::<Vec<_>>()
    );
}
