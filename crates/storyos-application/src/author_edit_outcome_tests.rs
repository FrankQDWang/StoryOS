use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Resolver {
    calls: Mutex<Vec<ResolveApplyAuthorEditOutcome>>,
}

impl ApplyAuthorEditOutcomeResolver for Resolver {
    async fn resolve_apply_author_edit_outcome(
        &self,
        query: &ResolveApplyAuthorEditOutcome,
    ) -> Result<ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeResolveError> {
        self.calls.lock().unwrap().push(query.clone());
        Ok(ApplyAuthorEditOutcome::StillUnknown {
            observation: ApplyAuthorEditUnknownObservation::AdmissionCommitted {
                command_id: "command".to_owned(),
                author_command_admission_id: "admission".to_owned(),
            },
        })
    }
}

fn query() -> ResolveApplyAuthorEditOutcome {
    ResolveApplyAuthorEditOutcome {
        project_scope: ProjectScope::new(UserId::new("user"), ProjectId::new("project")),
        client_binding: EditorClientBinding {
            binding_ref: "binding".to_owned(),
            session_generation: 7,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
        },
        limit_profile_revision: "limit".to_owned(),
        idempotency_key: "key".to_owned(),
        nonce_digest: "nonce-digest".to_owned(),
    }
}

#[tokio::test]
async fn outcome_query_resolves_once_without_retrying_a_command() {
    let resolver = Resolver {
        calls: Mutex::new(Vec::new()),
    };
    let query = query();

    let outcome = get_apply_author_edit_outcome(&resolver, &query)
        .await
        .unwrap();

    assert_eq!(
        outcome,
        ApplyAuthorEditOutcome::StillUnknown {
            observation: ApplyAuthorEditUnknownObservation::AdmissionCommitted {
                command_id: "command".to_owned(),
                author_command_admission_id: "admission".to_owned(),
            },
        }
    );
    assert_eq!(*resolver.calls.lock().unwrap(), vec![query]);
}
