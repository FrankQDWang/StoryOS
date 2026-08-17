use std::sync::Mutex;

use super::*;

struct Store {
    calls: Mutex<Vec<AppendAuthorCommandOutcomeUnknown>>,
    result: AuthorCommandOutcomeUnknownObservation,
}

impl AuthorCommandOutcomeUnknownStore for Store {
    async fn append_author_command_outcome_unknown(
        &self,
        request: &AppendAuthorCommandOutcomeUnknown,
    ) -> Result<AuthorCommandOutcomeUnknownObservation, AuthorCommandOutcomeUnknownError> {
        self.calls.lock().unwrap().push(request.clone());
        Ok(self.result.clone())
    }
}

#[tokio::test]
async fn append_delegates_once_and_returns_the_complete_immutable_observation() {
    let scope = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-000000000001"),
        ProjectId::new("018f0000-0000-7001-8000-000000000002"),
    );
    let request = AppendAuthorCommandOutcomeUnknown {
        project_scope: scope.clone(),
        author_command_admission_id: "018f0000-0000-7001-8000-000000000012".to_owned(),
        observation_id: "018f0000-0000-7001-8000-000000000013".to_owned(),
        last_provable_boundary: AuthorCommandOutcomeUnknownBoundary::AdmissionCommitted,
        reason: AuthorCommandOutcomeUnknownReason::AcknowledgementMissing,
    };
    let expected = AuthorCommandOutcomeUnknownObservation {
        project_scope: scope,
        observation_id: request.observation_id.clone(),
        observation_sequence: 1,
        author_command_admission_id: request.author_command_admission_id.clone(),
        command_id: "018f0000-0000-7001-8000-000000000011".to_owned(),
        command_kind: "applyAuthorEdit".to_owned(),
        canonical_command_digest: "sha256:command".to_owned(),
        idempotency_key: "018f0000-0000-7001-8000-000000000010".to_owned(),
        last_provable_boundary: request.last_provable_boundary,
        reason: request.reason,
        reconciliation_required: ReconciliationRequired,
        observed_at: "2026-08-17T00:00:00.000Z".to_owned(),
    };
    let store = Store {
        calls: Mutex::new(Vec::new()),
        result: expected.clone(),
    };
    assert_eq!(
        append_author_command_outcome_unknown(&store, &request)
            .await
            .unwrap(),
        expected
    );
    assert_eq!(*store.calls.lock().unwrap(), vec![request]);
}
