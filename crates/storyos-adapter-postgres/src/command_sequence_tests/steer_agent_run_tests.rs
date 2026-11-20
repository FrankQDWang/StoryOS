use storyos_application::{
    AgentRunControlRefusal, RefusableCommandError, SteerAgentRunInput, SteeringRetained,
};
use uuid::Uuid;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_agent_run::{request_rows, seed_run};
use crate::shared_tests::command_sequence_steer_agent_run::{
    STEER_AGENT_RUN, steer_agent_run, steering,
};
use crate::shared_tests::command_sequence_support::{issued, seed_project, stores};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_steering_refusal_before_admission_writes_no_row_and_keeps_the_challenge_unused() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "c700").await;
    let run_id = Uuid::now_v7().to_string();
    seed_run(&admin, &scope, &run_id, "waiting").await;
    for suffix in [0xc701, 0xc706, 0xc707] {
        let retained = issued(
            &store,
            &scope,
            suffix,
            &STEER_AGENT_RUN,
            steering(&admin, &run_id, /*characters*/ 8000).await,
        )
        .await;
        steer_agent_run(&store, &retained).await.unwrap();
    }
    // The Run message and the three inputs count 8 + 3 * (8000 + 1) characters, so 7988 more
    // characters reach the Context item bound.
    let within = steering(&admin, &run_id, /*characters*/ 7988).await;
    let refused = [
        SteerAgentRunInput {
            run_id: Uuid::now_v7().to_string(),
            ..within.clone()
        },
        SteerAgentRunInput {
            conversation_id: Uuid::now_v7().to_string(),
            ..within.clone()
        },
        SteerAgentRunInput {
            author_message: "a".repeat(/*n*/ 7989),
            ..within.clone()
        },
    ];
    let mut observed = Vec::new();
    for (suffix, input) in (0xc702..).zip(refused) {
        let call = issued(&store, &scope, suffix, &STEER_AGENT_RUN, input).await;
        let refusal = match store.steer_agent_run(&call.envelope, &call.input).await {
            Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => Some(refusal),
            Err(RefusableCommandError::Command(_)) | Ok(_) => None,
        };
        let key = &call.envelope.challenge_binding.idempotency_key;
        observed.push((refusal, request_rows(&admin, key).await));
    }
    let boundary = issued(
        &store,
        &scope,
        /*suffix*/ 0xc705,
        &STEER_AGENT_RUN,
        within,
    )
    .await;
    let settled = steer_agent_run(&store, &boundary).await.unwrap();
    let unused = [0, 0, 0, 1];
    assert_eq!(
        observed,
        vec![
            (Some(AgentRunControlRefusal::MissingRun), unused),
            (Some(AgentRunControlRefusal::MissingRun), unused),
            (Some(AgentRunControlRefusal::InputLimit), unused),
        ]
    );
    assert_eq!(
        settled.zero_authority_effect,
        Some(SteeringRetained {
            run_id,
            steering_input_id: boundary.envelope.ids.author_command_admission_id,
            input_position: 4,
        })
    );
}

/// One Chapter text reference in the first block of a fixed Chapter.
fn chapter_reference(start: u32) -> storyos_core::SelectionReference {
    let point = |offset| storyos_core::TextPoint {
        manuscript_block_id: "018f0000-0000-7001-8000-00000000c9b1".to_owned(),
        offset,
    };
    storyos_core::SelectionReference::ChapterText {
        chapter_id: "018f0000-0000-7001-8000-00000000c9c1".to_owned(),
        base_authoritative_revision_id: "018f0000-0000-7001-8000-00000000c9d1".to_owned(),
        start: point(start),
        end: point(start + 1),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn steering_inputs_add_distinct_references_up_to_eight_for_each_run() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "c900").await;
    let run_id = Uuid::now_v7().to_string();
    seed_run(&admin, &scope, &run_id, "waiting").await;
    let with = |starts: &[u32], input: &SteerAgentRunInput| SteerAgentRunInput {
        selection_references: Some(starts.iter().copied().map(chapter_reference).collect()),
        ..input.clone()
    };
    let plain = steering(&admin, &run_id, /*characters*/ 4).await;
    // The second input repeats one reference, so the third input fills the eighth place.
    let inputs = [
        (0xc901, with(&[0, 1, 2, 3, 4], &plain)),
        (0xc902, with(&[4, 5, 6], &plain)),
        (0xc903, plain.clone()),
        (0xc904, with(&[7, 0], &plain)),
        (0xc905, with(&[8], &plain)),
        (0xc906, with(&[3], &plain)),
    ];
    let mut observed = Vec::new();
    for (suffix, input) in inputs {
        let call = issued(&store, &scope, suffix, &STEER_AGENT_RUN, input).await;
        observed.push(
            match store.steer_agent_run(&call.envelope, &call.input).await {
                Ok(settled) => settled
                    .zero_authority_effect
                    .map(|effect| effect.input_position),
                Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => {
                    assert_eq!(refusal, AgentRunControlRefusal::ReferenceLimit);
                    assert_eq!(
                        request_rows(&admin, &call.envelope.challenge_binding.idempotency_key)
                            .await,
                        [0, 0, 0, 1]
                    );
                    None
                }
                Err(RefusableCommandError::Command(error)) => panic!("{error:?}"),
            },
        );
    }
    let stored: Vec<Option<usize>> = admin
        .query(
            "SELECT jsonb_array_length(payload->'selection_references')
               FROM storyos.project_activity_event_payloads
              WHERE event_kind = 'agent_run_steering_retained' AND payload->>'run_id' = $1
              ORDER BY (payload->>'input_position')::numeric",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| {
            row.get::<_, Option<i32>>(/*idx*/ 0)
                .map(|length| length as usize)
        })
        .collect();

    assert_eq!(
        observed,
        [Some(1), Some(2), Some(3), Some(4), None, Some(5)]
    );
    assert_eq!(stored, [Some(5), Some(3), None, Some(2), Some(1)]);
}
