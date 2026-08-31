use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, CreateProjectCommand,
    EditorClientBinding, IssueCreateProjectChallenge, ManuscriptSearchCompleteness,
    ManuscriptSearchRequest, ManuscriptSearchSelection, ProjectId, ProjectScope, SearchManuscript,
    UserId, create_project, issue_create_project_challenge, search_manuscript,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B: &str = "018f0000-0000-7001-8000-000000000102";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";

fn issue_request(idempotency_key: &str, project_suffix: &str) -> IssueCreateProjectChallenge {
    let digest = format!("sha256:storyos.command.createProject.jcs.v1:{project_suffix:0<64}");
    IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(USER_A),
            prospective_project_id: ProjectId::new(format!(
                "018f0000-0000-7001-8000-00000000{project_suffix}"
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
            create_input_digest: format!("{project_suffix:0<64}"),
            canonical_command_digest: digest,
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: format!("sha256:nonce-{project_suffix}"),
    }
}

fn command(
    binding: CreateProjectChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
) -> CreateProjectCommand {
    let project_scope = ProjectScope::new(
        binding.owner_user_id.clone(),
        binding.prospective_project_id.clone(),
    );
    CreateProjectCommand {
        project_scope,
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: br#"{"title":"Search Novel"}"#.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        title: "Search Novel".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

fn manuscript_query(query_text: &str) -> ManuscriptSearchRequest {
    ManuscriptSearchRequest {
        query_text: query_text.to_owned(),
        selection: ManuscriptSearchSelection::Manuscript,
        required_watermark: None,
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn search_rebuilds_from_canonical_facts_without_a_write_and_separates_not_ready_from_zero() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let issue = issue_request("018f0000-0000-7001-8000-000000000d90", "0d90");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut binding = issue.binding.clone();
    binding.prospective_project_id = issued.prospective_project_id.clone();
    binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &command(binding.clone(), &issue.nonce_digest, "0d91"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        binding.owner_user_id.clone(),
        binding.prospective_project_id.clone(),
    );

    let first = search_manuscript(&store, &scope, &manuscript_query("alpha"))
        .await
        .unwrap();
    let SearchManuscript::Ready(page) = &first else {
        panic!("an empty active Project must return a ready search page");
    };
    assert_eq!(page.project_scope, scope);
    assert_eq!(page.items, Vec::new());
    assert_eq!(page.completeness, ManuscriptSearchCompleteness::Complete);
    assert_eq!(page.lag, 0);
    let watermark = page.projection_watermark;

    let rebuilt = search_manuscript(&store, &scope, &manuscript_query("alpha"))
        .await
        .unwrap();
    assert_eq!(rebuilt, first);

    assert_eq!(
        search_manuscript(
            &store,
            &ProjectScope::new(UserId::new(USER_B), scope.project_id.clone()),
            &manuscript_query("alpha"),
        )
        .await
        .unwrap(),
        SearchManuscript::Missing
    );
    assert_eq!(
        search_manuscript(
            &store,
            &ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT_B)),
            &manuscript_query("alpha"),
        )
        .await
        .unwrap(),
        SearchManuscript::Missing
    );

    let not_ready = search_manuscript(
        &store,
        &scope,
        &ManuscriptSearchRequest {
            query_text: "alpha".to_owned(),
            selection: ManuscriptSearchSelection::Manuscript,
            required_watermark: Some(watermark + 1),
        },
    )
    .await
    .unwrap();
    match not_ready {
        SearchManuscript::ProjectionNotReady {
            projection_watermark,
            required_watermark,
            ..
        } => {
            assert_eq!(projection_watermark, watermark);
            assert_eq!(required_watermark, watermark + 1);
        }
        other => panic!("expected projection-not-ready, got {other:?}"),
    }

    let after = search_manuscript(&store, &scope, &manuscript_query("alpha"))
        .await
        .unwrap();
    let SearchManuscript::Ready(after_page) = after else {
        panic!("a later search must remain ready");
    };
    assert_eq!(after_page.projection_watermark, watermark);

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER_A, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(
        search_manuscript(&store, &scope, &manuscript_query("alpha"))
            .await
            .unwrap(),
        SearchManuscript::Missing
    );
}
