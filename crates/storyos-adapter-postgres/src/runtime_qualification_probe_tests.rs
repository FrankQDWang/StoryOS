//! The function calling probe through the Agent Plan dispatch path (ADR 0049).

use storyos_application::{CreatePurpose, ToolProbeRequest};
use storyos_core::{NativeStreamItem, StreamItemRole, StreamItemState, ToolProbe};

use super::*;

/// Pins the Registration with the pending function calling entry to `run_id` and its Project,
/// with an authorized qualification Decision, and makes it the head of its adapter.
async fn authorize_tool_probes(admin: &Client, scope: &ProjectScope, run_id: &str) {
    let pending = &storyos_core::AGENT_PLAN_FUNCTION_CALLING_PENDING_REGISTRATION;
    let profile = serde_json::to_string(pending.capability_profile).unwrap();
    admin
        .batch_execute(&format!(
            "INSERT INTO storyos.model_capability_profiles (capability_profile_revision, profile)
             VALUES ('{profile_revision}', '{profile}'::jsonb) ON CONFLICT DO NOTHING;
             INSERT INTO storyos.model_registration_revisions
               (model_registration_revision, model_kind, api_surface, provider_model_id,
                capability_profile_revision)
             VALUES ('{revision}', 'volcengine_agent_plan_responses', '{surface}',
                     '{model}', '{profile_revision}') ON CONFLICT DO NOTHING;
             SET session_replication_role = replica;
             UPDATE storyos.project_external_use_binding_revisions
                SET model_registration_revision = '{revision}'
              WHERE project_id = '{project}' AND credential_source = 'operator_quota';
             UPDATE storyos.agent_runs SET model_registration_revision = '{revision}'
              WHERE run_id = '{run_id}';
             UPDATE storyos.external_contract_compatibility_decisions
                SET runtime_qualification = 'qualification_authorized',
                    qualification_authorization = '{AUTHORIZATION}'
              WHERE project_id = '{project}' AND decision_kind = 'volcengine_agent_plan_compatible';
             SET session_replication_role = DEFAULT;
             UPDATE storyos.model_registration_heads SET model_registration_revision = '{revision}'
              WHERE model_kind = 'volcengine_agent_plan_responses';",
            profile_revision = pending.capability_profile.revision,
            revision = pending.revision,
            surface = pending.api_surface,
            model = pending.provider_model_id,
            project = scope.project_id.as_ref(),
        ))
        .await
        .unwrap();
}

/// The status of one AgentRun and the decision, probe, call, and qualification facts of its
/// decision Attempt.
async fn probe_evidence(admin: &Client, run_id: &str) -> [Option<String>; 5] {
    let row = admin
        .query_one(
            "SELECT run.status, attempt.decision_id::text,
                    attempt.payload->>'tool_qualification',
                    attempt.payload#>>'{items,0,call_id}',
                    attempt.payload->>'runtime_qualification'
               FROM storyos.agent_runs AS run
               LEFT JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4].map(|index| row.get(index))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn only_an_authorized_pending_tool_entry_dispatches_its_probe_with_no_decision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, _) = queued_run(&store, "b4a", authorized()).await;
    let response = |reference: &str, items| {
        ScriptedAgentPlan::new(Observation::Terminal(ModelResponse {
            items,
            output: Some(ModelOutput {
                phase: OutputPhase::FinalAnswer,
                candidate: DecisionCandidate::Advisory {
                    text: "storyos-probe".to_owned(),
                },
                prose_changes: None,
            }),
            usage: ModelUsage::Reported {
                input_tokens: 90,
                output_tokens: 30,
            },
            cache_use: storyos_application::CacheUse::Unknown,
            response_reference: Some(reference.to_owned()),
        }))
    };
    let function_call = vec![NativeStreamItem {
        item_id: "fc_b4b".to_owned(),
        role: StreamItemRole::Tool,
        state: StreamItemState::Complete,
        text: None,
        summary: None,
        call_id: Some("call_b4b".to_owned()),
        arguments: Some("{\"text\":\"storyos-probe\"}".to_owned()),
        refusal: None,
        native_type: Some("function_call".to_owned()),
    }];
    let call = admit_run(
        &store,
        &scope,
        &chapter_id,
        "b4b",
        authorized(),
        ToolProbe::FunctionCall.instruction(),
    )
    .await;
    authorize_tool_probes(&admin, &scope, &call).await;
    let call_adapter = response("resp_b4b", function_call);
    settle(&store, &admin, &scope, &call, &call_adapter).await;
    let output = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "b4c",
        authorized(),
        ToolProbe::FunctionCallOutput.instruction(),
        ConversationSelection::Existing {
            conversation_id: "018f0000-0000-7001-8000-00000006b4b6".to_owned(),
        },
    )
    .await;
    authorize_tool_probes(&admin, &scope, &output).await;
    let output_adapter = response("resp_b4c", Vec::new());
    settle(&store, &admin, &scope, &output, &output_adapter).await;
    // A probe has no recovery: a rejected continuation and an unknown outcome refuse the run.
    let mut refused = Vec::new();
    for (prefix, observation) in [
        (
            "b4e",
            Observation::Rejected {
                reason: "continuation_unusable:previous_response_not_found".to_owned(),
            },
        ),
        (
            "b4f",
            Observation::OutcomeUnknown {
                response_reference: None,
            },
        ),
    ] {
        let run = admit_run_in(
            &store,
            &scope,
            &chapter_id,
            prefix,
            authorized(),
            ToolProbe::FunctionCallOutput.instruction(),
            ConversationSelection::Existing {
                conversation_id: "018f0000-0000-7001-8000-00000006b4b6".to_owned(),
            },
        )
        .await;
        authorize_tool_probes(&admin, &scope, &run).await;
        settle(
            &store,
            &admin,
            &scope,
            &run,
            &ScriptedAgentPlan::new(observation),
        )
        .await;
        refused.push(probe_evidence(&admin, &run).await);
    }
    // The head qualifies function calling, so the same message is an ordinary request.
    let ordinary = admit_run(
        &store,
        &scope,
        &chapter_id,
        "b4d",
        authorized(),
        ToolProbe::FunctionCall.instruction(),
    )
    .await;
    let ordinary_adapter = response("resp_b4d", Vec::new());
    settle(&store, &admin, &scope, &ordinary, &ordinary_adapter).await;

    let sent = |adapter: ScriptedAgentPlan| {
        adapter
            .seen
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|request| (request.purpose, request.previous_response_reference))
            .collect::<Vec<_>>()
    };
    let text = |value: &str| Some(value.to_owned());
    let ordinary_evidence = probe_evidence(&admin, &ordinary).await;
    assert_eq!(
        (
            sent(call_adapter),
            sent(output_adapter),
            sent(ordinary_adapter),
            probe_evidence(&admin, &call).await,
            probe_evidence(&admin, &output).await,
            refused,
            [ordinary_evidence[0].clone(), ordinary_evidence[2].clone()],
            ordinary_evidence[1].is_some(),
        ),
        (
            vec![(
                CreatePurpose::ToolQualification(ToolProbeRequest::FunctionCall),
                None
            )],
            vec![(
                CreatePurpose::ToolQualification(ToolProbeRequest::FunctionCallOutput {
                    call_id: Some("call_b4b".to_owned())
                }),
                text("resp_b4b")
            )],
            vec![(CreatePurpose::Decision, None)],
            [
                text("completed"),
                None,
                text("function_call"),
                text("call_b4b"),
                text("qualification_authorized"),
            ],
            [
                text("completed"),
                None,
                text("function_call_output"),
                None,
                text("qualification_authorized"),
            ],
            vec![
                [
                    text("refused"),
                    None,
                    text("function_call_output"),
                    None,
                    text("qualification_authorized"),
                ];
                2
            ],
            [text("completed"), None],
            true,
        )
    );
}
