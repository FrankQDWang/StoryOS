use storyos_application::{
    ProjectCommandError, ProjectScope, SetProjectToolEnablementInput,
    SetProjectToolEnablementSettlement,
};
use storyos_core::{
    ContractDifference, EXA_TOOL_REGISTRATION, ProjectToolState, RESEARCH_TOOL_OFFER,
    ResearchToolOffer, SetProjectToolEnablementApplied, SetProjectToolEnablementNoEffect,
    SetProjectToolEnablementRefusal, ToolEnablement, ToolRegistration, TransitionOutcome,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_sequence::tests::{SET_PROJECT_TOOL_ENABLEMENT, issued, seed_project, stores};

const SUCCESSOR: ToolRegistration = ToolRegistration {
    revision: "018f0000-0000-7001-8000-0000000e0b51",
    ..EXA_TOOL_REGISTRATION
};
const QUARANTINED: ToolRegistration = ToolRegistration {
    revision: "018f0000-0000-7001-8000-0000000e0b52",
    ..EXA_TOOL_REGISTRATION
};
const SUCCESSOR_OFFER: ResearchToolOffer = ResearchToolOffer {
    registration: &SUCCESSOR,
    ..RESEARCH_TOOL_OFFER
};
const REBOUND_OFFER: ResearchToolOffer = ResearchToolOffer {
    credential_reference: "settings:STORYOS_EXA_API_KEY_SECOND",
    ..SUCCESSOR_OFFER
};
const QUARANTINED_OFFER: ResearchToolOffer = ResearchToolOffer {
    registration: &QUARANTINED,
    ..RESEARCH_TOOL_OFFER
};

type Outcome = TransitionOutcome<
    SetProjectToolEnablementApplied,
    SetProjectToolEnablementNoEffect,
    std::convert::Infallible,
    SetProjectToolEnablementRefusal,
>;

fn input(
    registration_id: &str,
    requested: ToolEnablement,
    offer: Option<&'static ResearchToolOffer>,
) -> SetProjectToolEnablementInput {
    SetProjectToolEnablementInput {
        registration_id: registration_id.to_owned(),
        requested,
        offer,
    }
}

fn enable(offer: &'static ResearchToolOffer) -> SetProjectToolEnablementInput {
    input("web_research", ToolEnablement::Enabled, Some(offer))
}

fn disable() -> SetProjectToolEnablementInput {
    input(
        "web_research",
        ToolEnablement::Disabled,
        Some(&RESEARCH_TOOL_OFFER),
    )
}

/// Settles each input in order. The Command Challenge of input `n` has the suffix
/// `first_suffix + n`.
async fn settle_each(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    first_suffix: u16,
    inputs: Vec<SetProjectToolEnablementInput>,
) -> Vec<Outcome> {
    let mut outcomes = Vec::new();
    for (suffix, input) in (first_suffix..).zip(inputs) {
        let call = issued(store, scope, suffix, &SET_PROJECT_TOOL_ENABLEMENT, input).await;
        let settled: SetProjectToolEnablementSettlement = store
            .set_project_tool_enablement(&call.envelope, &call.input)
            .await
            .unwrap();
        outcomes.push(settled.outcome.map_applied(|applied| applied.effect));
    }
    outcomes
}

fn applied(enablement: ToolEnablement, revision: u64) -> Outcome {
    TransitionOutcome::Applied(SetProjectToolEnablementApplied {
        enablement,
        revision,
    })
}

const UNCHANGED: Outcome =
    TransitionOutcome::NoEffect(SetProjectToolEnablementNoEffect::EnablementUnchanged);

/// The binding records of the Project, one text row for each, in insert order.
async fn binding_records(admin: &Client, scope: &ProjectScope) -> Vec<String> {
    admin
        .query(
            "SELECT concat_ws(' ', identity.destination_kind, identity.control_classification,
                              evidence.evidence_revision, evidence.evidence_kind, evidence.endpoint,
                              destination_grant.grant_kind, destination_grant.grant_scope,
                              binding.tool_registration_revision, binding.credential_source,
                              binding.credential_reference, binding.budget_bounds,
                              decision.decision_kind)
               FROM storyos.external_contract_compatibility_decisions AS decision
               JOIN storyos.project_external_use_binding_revisions AS binding
                 USING (owner_user_id, project_id, project_model_use_binding_revision)
               JOIN storyos.project_destination_grants AS destination_grant
                 USING (owner_user_id, project_id, grant_id, processing_destination_identity)
               JOIN storyos.processing_destination_identity_evidence_revisions AS evidence
                 USING (owner_user_id, project_id, processing_destination_identity,
                        evidence_revision)
               JOIN storyos.processing_destination_identities AS identity
                 USING (owner_user_id, project_id, processing_destination_identity)
              WHERE decision.project_id = $1::text::uuid
              ORDER BY decision.external_compatibility_decision",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}

fn record(revision: &str, credential_reference: &str) -> String {
    format!(
        "mcp_remote external 1 mcp_remote_boundary https://mcp.exa.ai/mcp research_tool_use \
         research_tool_v1 {revision} operator_quota {credential_reference} not_required \
         tool_registration_compatible"
    )
}

/// The enablement revisions of the Project and whether each names a Decision.
async fn enablement_revisions(admin: &Client, scope: &ProjectScope) -> Vec<(String, bool)> {
    admin
        .query(
            "SELECT enablement, external_compatibility_decision IS NOT NULL
               FROM storyos.project_tool_enablements
              WHERE project_id = $1::text::uuid
              ORDER BY enablement_revision",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn enablement_records_its_binding_once_and_disablement_keeps_it_as_history() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "b500").await;
    let state = async || {
        store
            .project_tool_state(&scope, Some(&RESEARCH_TOOL_OFFER))
            .await
            .unwrap()
    };
    assert_eq!(state().await, ProjectToolState::Disabled);
    let first = settle_each(
        &store,
        &scope,
        /*first_suffix*/ 0xb501,
        vec![enable(&RESEARCH_TOOL_OFFER), enable(&RESEARCH_TOOL_OFFER)],
    )
    .await;
    assert_eq!(first, vec![applied(ToolEnablement::Enabled, 1), UNCHANGED]);
    assert_eq!(state().await, ProjectToolState::Enabled);
    let later = settle_each(
        &store,
        &scope,
        /*first_suffix*/ 0xb503,
        vec![disable(), disable(), enable(&RESEARCH_TOOL_OFFER)],
    )
    .await;
    assert_eq!(
        later,
        vec![
            applied(ToolEnablement::Disabled, 2),
            UNCHANGED,
            applied(ToolEnablement::Enabled, 3),
        ]
    );
    assert_eq!(
        binding_records(&admin, &scope).await,
        vec![record(
            EXA_TOOL_REGISTRATION.revision,
            "settings:STORYOS_EXA_API_KEY"
        )]
    );
    assert_eq!(
        enablement_revisions(&admin, &scope).await,
        [("enabled", true), ("disabled", false), ("enabled", true)]
            .map(|(enablement, named)| (enablement.to_owned(), named))
            .to_vec()
    );
    assert_eq!(
        store.project_tool_state(&scope, None).await.unwrap(),
        ProjectToolState::NotOffered
    );

    let families = crate::project_archive_build::collect_exportable_families(&admin, &scope)
        .await
        .unwrap();
    let exported = |table: &str| {
        families
            .iter()
            .find(|family| family.table == table)
            .map(|family| family.rows_json.clone())
            .unwrap_or_default()
    };
    for table in [
        "project_tool_enablements",
        "project_external_use_binding_revisions",
        "external_contract_compatibility_decisions",
        "project_destination_grants",
    ] {
        assert!(storyos_core::PROJECT_EXPORT_TABLES.contains(&table));
        assert!(exported(table).contains(&scope.project_id.as_ref().to_owned()));
    }
    assert!(
        exported("project_external_use_binding_revisions")
            .contains("\"credential_reference\":\"settings:STORYOS_EXA_API_KEY\"")
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_changed_registration_or_credential_reference_makes_new_binding_records() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "b510").await;
    let outcomes = settle_each(
        &store,
        &scope,
        /*first_suffix*/ 0xb511,
        vec![
            enable(&RESEARCH_TOOL_OFFER),
            enable(&SUCCESSOR_OFFER),
            enable(&REBOUND_OFFER),
            enable(&REBOUND_OFFER),
            enable(&RESEARCH_TOOL_OFFER),
        ],
    )
    .await;
    assert_eq!(
        outcomes,
        vec![
            applied(ToolEnablement::Enabled, 1),
            applied(ToolEnablement::Enabled, 2),
            applied(ToolEnablement::Enabled, 3),
            UNCHANGED,
            applied(ToolEnablement::Enabled, 4),
        ]
    );
    assert_eq!(
        binding_records(&admin, &scope).await,
        vec![
            record(
                EXA_TOOL_REGISTRATION.revision,
                "settings:STORYOS_EXA_API_KEY"
            ),
            record(SUCCESSOR.revision, "settings:STORYOS_EXA_API_KEY"),
            record(SUCCESSOR.revision, "settings:STORYOS_EXA_API_KEY_SECOND"),
            // A change back to earlier values makes new records too.
            record(
                EXA_TOOL_REGISTRATION.revision,
                "settings:STORYOS_EXA_API_KEY"
            ),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_wrong_scope_refuses_and_a_quarantined_revision_shows_as_quarantined() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "b520").await;
    let other_scope = seed_project(&store, "b530").await;
    let outcomes = settle_each(
        &store,
        &scope,
        /*first_suffix*/ 0xb521,
        vec![
            input("web_research", ToolEnablement::Enabled, None),
            input(
                "other_research",
                ToolEnablement::Enabled,
                Some(&RESEARCH_TOOL_OFFER),
            ),
            enable(&QUARANTINED_OFFER),
        ],
    )
    .await;
    assert_eq!(
        outcomes[..2],
        [
            TransitionOutcome::Refused(SetProjectToolEnablementRefusal::RegistrationNotOffered),
            TransitionOutcome::Refused(SetProjectToolEnablementRefusal::RegistrationNotOffered),
        ]
    );
    assert_eq!(outcomes[2], applied(ToolEnablement::Enabled, 1));
    store
        .quarantine_tool_registration(
            QUARANTINED.revision,
            &[ContractDifference::ToolRemoved {
                tool: "web_fetch_exa".to_owned(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .project_tool_state(&scope, Some(&QUARANTINED_OFFER))
            .await
            .unwrap(),
        ProjectToolState::Quarantined
    );
    assert_eq!(
        settle_each(
            &store,
            &scope,
            /*first_suffix*/ 0xb524,
            vec![enable(&QUARANTINED_OFFER)]
        )
        .await,
        vec![TransitionOutcome::Refused(
            SetProjectToolEnablementRefusal::RegistrationUnavailable
        )]
    );
    // A Project without an enablement record: the refusal and the read after it agree.
    assert_eq!(
        settle_each(
            &store,
            &other_scope,
            /*first_suffix*/ 0xb526,
            vec![enable(&QUARANTINED_OFFER)]
        )
        .await,
        vec![TransitionOutcome::Refused(
            SetProjectToolEnablementRefusal::RegistrationUnavailable
        )]
    );
    assert_eq!(
        store
            .project_tool_state(&other_scope, Some(&QUARANTINED_OFFER))
            .await
            .unwrap(),
        ProjectToolState::Quarantined
    );

    let mut foreign = issued(
        &store,
        &other_scope,
        /*suffix*/ 0xb531,
        &SET_PROJECT_TOOL_ENABLEMENT,
        enable(&RESEARCH_TOOL_OFFER),
    )
    .await;
    foreign.envelope.project_scope = scope.clone();
    assert!(matches!(
        store
            .set_project_tool_enablement(&foreign.envelope, &foreign.input)
            .await,
        Err(ProjectCommandError::InvalidChallenge)
    ));
    assert_eq!(enablement_revisions(&admin, &other_scope).await, Vec::new());
    assert_eq!(enablement_revisions(&admin, &scope).await.len(), 1);
}
