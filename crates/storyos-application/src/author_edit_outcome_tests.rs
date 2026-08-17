use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Reader {
    calls: Mutex<Vec<ReadApplyAuthorEditOutcome>>,
}

impl ApplyAuthorEditOutcomeReader for Reader {
    async fn read_apply_author_edit_outcome(
        &self,
        query: &ReadApplyAuthorEditOutcome,
    ) -> Result<ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeReadError> {
        self.calls.lock().unwrap().push(query.clone());
        Ok(ApplyAuthorEditOutcome::StillUnknown {
            observation: ApplyAuthorEditUnknownObservation::AdmissionCommitted {
                command_id: "command".to_owned(),
                author_command_admission_id: "admission".to_owned(),
            },
        })
    }
}

fn query() -> ReadApplyAuthorEditOutcome {
    ReadApplyAuthorEditOutcome {
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
async fn outcome_query_reads_once_without_invoking_or_retrying_a_command() {
    let reader = Reader {
        calls: Mutex::new(Vec::new()),
    };
    let query = query();

    let outcome = get_apply_author_edit_outcome(&reader, &query)
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
    assert_eq!(*reader.calls.lock().unwrap(), vec![query]);
}
