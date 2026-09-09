use super::*;
use std::path::PathBuf;
use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, CreateProjectCommand,
    EditorClientBinding, IssueCreateProjectChallenge, ProjectId, ProjectScope, UserId,
    create_project, issue_create_project_challenge,
};
use tokio_postgres::{Client, NoTls};

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const TITLE: &str = "Lawful Empty";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const CATALOG: &str = "storyos.persistence.catalog.release-1.v3";
const METHOD: &str = "pg_basebackup";
const ISOLATED_TARGET: &str = "storyos-recovery-hold-empty-null";
const CHAIN_SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const WAL_MEMBER: &str = "000000010000000000000001";
const TARGET_LSN: &str = "0/1";
const BROKEN_CHAPTER: &str = "018f0000-0000-7001-8000-00000000f8aa";

#[derive(Debug, PartialEq, Eq)]
struct EmptyProjectFacts {
    current_chapter_id: Option<String>,
    manuscript_object_count: i64,
    project_present: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct HoldObservation {
    restore_state: String,
    runtime_can_login: bool,
    proof_written: bool,
}

struct IsolatedProofDatabase {
    cluster_admin_url: String,
    database_name: String,
}

impl Drop for IsolatedProofDatabase {
    fn drop(&mut self) {
        let cluster_admin_url = self.cluster_admin_url.clone();
        let database_name = self.database_name.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("restore runtime");
            runtime.block_on(async {
                let (client, connection) = tokio_postgres::connect(&cluster_admin_url, NoTls)
                    .await
                    .expect("restore login connect");
                tokio::spawn(async move {
                    let _ = connection.await;
                });
                client
                    .batch_execute("ALTER ROLE storyos_runtime LOGIN")
                    .await
                    .expect("restore runtime login");
                client
                    .execute(
                        &format!("DROP DATABASE IF EXISTS {database_name} WITH (FORCE)"),
                        &[],
                    )
                    .await
                    .expect("drop isolated proof database");
            });
        })
        .join()
        .expect("restore login thread");
    }
}

fn with_database_name(url: &str, database_name: &str) -> String {
    let Some((prefix, _)) = url.rsplit_once('/') else {
        panic!("database URL must include a database name");
    };
    format!("{prefix}/{database_name}")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("adapter crate lives under the repository root")
}

fn catalogued_schema_sql() -> String {
    let root = repo_root();
    let catalog: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("docs/foundation/postgresql-release-1-persistence-catalog.json"))
            .expect("read persistence catalog"),
    )
    .expect("persistence catalog must be JSON");
    let mut sql = String::from("BEGIN;\n");
    for source in catalog["migration_chain"]["bootstrap"]["sources"]
        .as_array()
        .expect("catalogued bootstrap sources")
    {
        let path = source["path"].as_str().expect("bootstrap source path");
        if path.ends_with("/0000_roles.sql") {
            continue;
        }
        sql.push_str(
            &std::fs::read_to_string(root.join(path)).unwrap_or_else(|error| {
                panic!("read {path}: {error}");
            }),
        );
        sql.push('\n');
    }
    sql.push_str("COMMIT;\n");
    sql
}

async fn open_isolated_proof(
    database_name: &str,
) -> (IsolatedProofDatabase, PostgresProjectReader, Client) {
    assert!(
        database_name
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'),
        "isolated proof database name must be a safe SQL identifier"
    );
    let cluster_admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let cluster_runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let cluster = connect_admin(&cluster_admin_url).await;
    cluster
        .execute(
            &format!("DROP DATABASE IF EXISTS {database_name} WITH (FORCE)"),
            &[],
        )
        .await
        .unwrap();
    cluster
        .execute(&format!("CREATE DATABASE {database_name}"), &[])
        .await
        .unwrap();
    let isolated_admin_url = with_database_name(&cluster_admin_url, database_name);
    let isolated_runtime_url = with_database_name(&cluster_runtime_url, database_name);
    let admin = connect_admin(&isolated_admin_url).await;
    admin.batch_execute(&catalogued_schema_sql()).await.unwrap();
    admin
        .batch_execute(
            &std::fs::read_to_string(
                repo_root().join("crates/storyos-adapter-postgres/tests/fixture.sql"),
            )
            .expect("read fixture SQL"),
        )
        .await
        .unwrap();
    (
        IsolatedProofDatabase {
            cluster_admin_url,
            database_name: database_name.to_owned(),
        },
        PostgresProjectReader::new(isolated_runtime_url),
        admin,
    )
}

struct ProofIds {
    copy: String,
    restore: String,
    visibility: String,
}

fn proof_ids(copy: &str, restore: &str, visibility: &str) -> ProofIds {
    ProofIds {
        copy: copy.to_owned(),
        restore: restore.to_owned(),
        visibility: visibility.to_owned(),
    }
}

fn expected_hold() -> HoldObservation {
    HoldObservation {
        restore_state: "recovery_hold".to_owned(),
        runtime_can_login: false,
        proof_written: false,
    }
}

fn expected_empty_project() -> EmptyProjectFacts {
    EmptyProjectFacts {
        current_chapter_id: None,
        manuscript_object_count: 0,
        project_present: true,
    }
}

async fn connect_admin(admin_url: &str) -> Client {
    let (admin, connection) = tokio_postgres::connect(admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        connection.await.unwrap();
    });
    admin
}

async fn create_empty_project(store: &PostgresProjectReader, suffix: &str) -> String {
    let digest = format!("sha256:storyos.command.createProject.jcs.v1:{suffix:0<64}");
    let issue = IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(USER_A),
            prospective_project_id: ProjectId::new(format!(
                "018f0000-0000-7001-8000-00000000{suffix}"
            )),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects".to_owned(),
            command_schema: "storyos.command.create-project.request.v1".to_owned(),
            command_kind: "createProject".to_owned(),
            create_input_digest: format!("{suffix:0<64}"),
            canonical_command_digest: digest,
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        },
        nonce_digest: format!("sha256:nonce-{suffix}"),
    };
    let issued = issue_create_project_challenge(store, &issue).await.unwrap();
    let mut binding = issue.binding.clone();
    binding.prospective_project_id = issued.prospective_project_id.clone();
    binding.canonical_command_digest = issued.canonical_command_digest.clone();
    let project_scope = ProjectScope::new(
        binding.owner_user_id.clone(),
        binding.prospective_project_id.clone(),
    );
    create_project(
        store,
        &CreateProjectCommand {
            project_scope,
            client_binding: EditorClientBinding {
                binding_ref: binding.client_session_binding_digest.clone(),
                session_generation: binding.client_session_generation,
                client_contract_revision: binding.client_contract_revision.clone(),
                security_policy_revision: binding.security_policy_revision.clone(),
            },
            challenge_binding: binding.clone(),
            nonce_digest: issue.nonce_digest.clone(),
            canonical_command_bytes: br#"{"title":"Lawful Empty"}"#.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            title: TITLE.to_owned(),
            ids: AuthorCommandAdmissionIds {
                command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
                author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
                receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
            },
        },
    )
    .await
    .unwrap();
    binding.prospective_project_id.as_ref().to_owned()
}

async fn empty_project_facts(admin: &Client, project_id: &str) -> EmptyProjectFacts {
    let row = admin
        .query_one(
            "SELECT current_chapter_id::text,
                    (SELECT count(*) FROM storyos.manuscript_objects
                      WHERE project_id = $1::text::uuid),
                    EXISTS(SELECT 1 FROM storyos.projects WHERE project_id = $1::text::uuid)
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[&project_id],
        )
        .await
        .unwrap();
    EmptyProjectFacts {
        current_chapter_id: row.get(0),
        manuscript_object_count: row.get(1),
        project_present: row.get(2),
    }
}

async fn insert_hold_evidence(admin: &Client, ids: &ProofIds) {
    admin
        .execute(
            "INSERT INTO storyos.recovery_copies (
               recovery_copy_id, method, chain_sha256, required_wal_member, recovery_target_lsn
             ) VALUES ($1::text::uuid, $2, $3, $4, $5)",
            &[&ids.copy, &METHOD, &CHAIN_SHA256, &WAL_MEMBER, &TARGET_LSN],
        )
        .await
        .unwrap();
    admin
        .execute(
            "INSERT INTO storyos.backup_wal_evidence (recovery_copy_id, path, sha256)
             VALUES ($1::text::uuid, 'chain_manifest', $2)",
            &[&ids.copy, &CHAIN_SHA256],
        )
        .await
        .unwrap();
    admin
        .execute(
            "INSERT INTO storyos.restore_proofs (
               restore_proof_id, recovery_copy_id, isolated_target_identity,
               recovery_target_lsn, state
             ) VALUES ($1::text::uuid, $2::text::uuid, $3, $4, 'recovery_hold')",
            &[&ids.restore, &ids.copy, &ISOLATED_TARGET, &TARGET_LSN],
        )
        .await
        .unwrap();
}

async fn call_proof(admin: &Client, ids: &ProofIds) -> Result<String, tokio_postgres::Error> {
    admin
        .query_one(
            "SELECT storyos.pass_recovery_visibility_proof(
               $1::text::uuid, $2::text::uuid, $3::text::uuid, $4, $5, $6, $7, $8, $9
             )",
            &[
                &ids.copy,
                &ids.restore,
                &ids.visibility,
                &ISOLATED_TARGET,
                &CHAIN_SHA256,
                &WAL_MEMBER,
                &TARGET_LSN,
                &METHOD,
                &CATALOG,
            ],
        )
        .await
        .map(|row| row.get(0))
}

async fn hold_observation(admin: &Client, ids: &ProofIds) -> HoldObservation {
    let row = admin
        .query_one(
            "SELECT restore.state,
                    runtime.rolcanlogin,
                    EXISTS(
                      SELECT 1 FROM storyos.recovery_visibility_proofs
                       WHERE visibility_proof_id = $1::text::uuid
                    )
               FROM storyos.restore_proofs AS restore
               JOIN pg_roles AS runtime ON runtime.rolname = 'storyos_runtime'
              WHERE restore.restore_proof_id = $2::text::uuid",
            &[&ids.visibility, &ids.restore],
        )
        .await
        .unwrap();
    HoldObservation {
        restore_state: row.get(0),
        runtime_can_login: row.get(1),
        proof_written: row.get(2),
    }
}

async fn prove_keeps_hold(admin: &Client, ids: &ProofIds) {
    insert_hold_evidence(admin, ids).await;
    assert!(call_proof(admin, ids).await.is_err());
    assert_eq!(hold_observation(admin, ids).await, expected_hold());
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn recovery_visibility_proof_accepts_lawful_null_current_chapter() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (_isolated, store, admin) = open_isolated_proof("storyos_rvp_null").await;
    let project_id = create_empty_project(&store, "f801").await;
    assert_eq!(
        empty_project_facts(&admin, &project_id).await,
        expected_empty_project()
    );

    admin
        .batch_execute("ALTER ROLE storyos_runtime NOLOGIN")
        .await
        .unwrap();
    let ids = proof_ids(
        "018f0000-0000-7001-8000-00000000f8c1",
        "018f0000-0000-7001-8000-00000000f8c2",
        "018f0000-0000-7001-8000-00000000f8c3",
    );
    insert_hold_evidence(&admin, &ids).await;
    let outcome = call_proof(&admin, &ids).await.unwrap();
    let proof = admin
        .query_one(
            "SELECT restore.state,
                    runtime.rolcanlogin,
                    proof.search_rebuild::text,
                    (proof.statistics_rebuild->'chapters')::text
               FROM storyos.recovery_visibility_proofs AS proof
               JOIN storyos.restore_proofs AS restore USING (restore_proof_id)
               JOIN pg_roles AS runtime ON runtime.rolname = 'storyos_runtime'
              WHERE proof.visibility_proof_id = $1::text::uuid",
            &[&ids.visibility],
        )
        .await
        .unwrap();
    let search_text = proof.get::<_, String>(2);
    let stats_text = proof.get::<_, String>(3);
    assert_eq!(
        (
            outcome,
            proof.get::<_, String>(0),
            proof.get::<_, bool>(1),
            empty_project_facts(&admin, &project_id).await,
            search_text.contains(&project_id),
            stats_text.contains(&project_id)
        ),
        (
            "visible".to_owned(),
            "visible".to_owned(),
            true,
            expected_empty_project(),
            false,
            false
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn recovery_visibility_proof_keeps_hold_for_broken_required_checks() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (_isolated, store, admin) = open_isolated_proof("storyos_rvp_hold").await;
    let project_id = create_empty_project(&store, "f811").await;
    admin
        .batch_execute("ALTER ROLE storyos_runtime NOLOGIN")
        .await
        .unwrap();

    let broken = proof_ids(
        "018f0000-0000-7001-8000-00000000f8c4",
        "018f0000-0000-7001-8000-00000000f8c5",
        "018f0000-0000-7001-8000-00000000f8c6",
    );
    insert_hold_evidence(&admin, &broken).await;
    admin
        .batch_execute("BEGIN; SET CONSTRAINTS ALL DEFERRED")
        .await
        .unwrap();
    admin
        .execute(
            "UPDATE storyos.projects
                SET current_chapter_id = $2::text::uuid
              WHERE project_id = $1::text::uuid",
            &[&project_id, &BROKEN_CHAPTER],
        )
        .await
        .unwrap();
    let broken_outcome = call_proof(&admin, &broken).await;
    admin.batch_execute("ROLLBACK").await.unwrap();
    assert!(broken_outcome.is_err());
    assert_eq!(hold_observation(&admin, &broken).await, expected_hold());

    admin
        .execute(
            "DELETE FROM storyos.replay_floors WHERE project_id = $1::text::uuid",
            &[&project_id],
        )
        .await
        .unwrap();
    prove_keeps_hold(
        &admin,
        &proof_ids(
            "018f0000-0000-7001-8000-00000000f8c7",
            "018f0000-0000-7001-8000-00000000f8c8",
            "018f0000-0000-7001-8000-00000000f8c9",
        ),
    )
    .await;
    admin
        .execute(
            "INSERT INTO storyos.replay_floors
               (owner_user_id, project_id, replay_generation, floor_position)
             VALUES ($1::text::uuid, $2::text::uuid, 1, 0)",
            &[&USER_A, &project_id],
        )
        .await
        .unwrap();

    admin
        .batch_execute("ALTER TABLE storyos.projects NO FORCE ROW LEVEL SECURITY")
        .await
        .unwrap();
    prove_keeps_hold(
        &admin,
        &proof_ids(
            "018f0000-0000-7001-8000-00000000f8d1",
            "018f0000-0000-7001-8000-00000000f8d2",
            "018f0000-0000-7001-8000-00000000f8d3",
        ),
    )
    .await;
    admin
        .batch_execute("ALTER TABLE storyos.projects FORCE ROW LEVEL SECURITY")
        .await
        .unwrap();

    admin
        .execute(
            "UPDATE storyos.projects
                SET lifecycle_state = 'archived'
              WHERE project_id = $1::text::uuid",
            &[&project_id],
        )
        .await
        .unwrap();
    prove_keeps_hold(
        &admin,
        &proof_ids(
            "018f0000-0000-7001-8000-00000000f8d4",
            "018f0000-0000-7001-8000-00000000f8d5",
            "018f0000-0000-7001-8000-00000000f8d6",
        ),
    )
    .await;
    admin
        .execute(
            "UPDATE storyos.projects
                SET lifecycle_state = 'active'
              WHERE project_id = $1::text::uuid",
            &[&project_id],
        )
        .await
        .unwrap();
    assert_eq!(
        empty_project_facts(&admin, &project_id).await,
        expected_empty_project()
    );
}
