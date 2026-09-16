use super::{CreateAgentRunCommand, CreateAgentRunError, request_create_agent_run};
use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, ProjectCommandChallengeBinding, ProjectId,
    ProjectScope, UserId,
};

struct RejectStore;

impl super::CreateAgentRunStore for RejectStore {
    async fn create_agent_run(
        &self,
        _command: &CreateAgentRunCommand,
    ) -> Result<super::CreateAgentRunAdmission, CreateAgentRunError> {
        unreachable!("binding conflict must fail before the store")
    }

    async fn read_agent_run(
        &self,
        _scope: &ProjectScope,
        _run_id: &str,
    ) -> Result<Option<super::AgentRunRecord>, CreateAgentRunError> {
        unreachable!("binding conflict must fail before the store")
    }
}

fn command() -> CreateAgentRunCommand {
    let scope = ProjectScope {
        owner_user_id: UserId::new("018f0000-0000-7001-8000-000000000001"),
        project_id: ProjectId::new("018f0000-0000-7001-8000-000000000201"),
    };
    CreateAgentRunCommand {
        project_scope: scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: "binding".to_owned(),
            session_generation: 1,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
        },
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope: scope,
            client_session_binding_digest: "binding".to_owned(),
            client_session_generation: 1,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
            limit_profile_revision: "limit".to_owned(),
            challenge_rate_policy_revision: "rate".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects/{project_id}/agent-runs".to_owned(),
            command_schema: "storyos.command.create-agent-run.request.v2".to_owned(),
            command_kind: "createAgentRun".to_owned(),
            canonical_command_digest: "wrong".to_owned(),
            idempotency_key: "018f0000-0000-7001-8000-000000000a40".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes: b"{}".to_vec(),
        correlation_id: "018f0000-0000-7001-8000-000000000a41".to_owned(),
        conversation: super::ConversationSelection::New,
        author_message: "Help with this passage.".to_owned(),
        chapter_id: "018f0000-0000-7001-8000-000000000301".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: "018f0000-0000-7001-8000-000000000a42".to_owned(),
            author_command_admission_id: "018f0000-0000-7001-8000-000000000a43".to_owned(),
            receipt_id: "018f0000-0000-7001-8000-000000000a44".to_owned(),
        },
        run_id: "018f0000-0000-7001-8000-000000000a45".to_owned(),
        conversation_id: "018f0000-0000-7001-8000-000000000a46".to_owned(),
        project_agent_id: "018f0000-0000-7001-8000-000000000a47".to_owned(),
    }
}

#[tokio::test]
async fn rejects_a_digest_mismatch_before_the_store() {
    let error = request_create_agent_run(&RejectStore, &command())
        .await
        .expect_err("changed digest must refuse");
    assert!(matches!(error, CreateAgentRunError::BindingConflict));
}
