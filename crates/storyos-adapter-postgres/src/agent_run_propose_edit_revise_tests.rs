//! `propose_edit` calls that revise a referenced pending candidate or grow its Proposal over
//! Joinable Blocks (ADR 0062, ADR 0071), with the fake of each profile.

use serde_json::{Value, json};
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ProjectScope,
};
use storyos_core::DeploymentDestination;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_propose_edit::{
    REVISE, opened, proposal_records, propose_edit_calls,
};
use crate::shared_tests::agent_run_propose_edit_revise::Fixture;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;

/// The locations of the public `prose_change` of the Run.
async fn locations(store: &PostgresProjectReader, scope: &ProjectScope, run_id: &str) -> Value {
    let record = store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let AgentRunDecisionInspect::ProseChange {
        locations: Some(locations),
        ..
    } = record.decision
    else {
        panic!("{:?}", record.decision);
    };
    json!(locations)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_call_revises_the_candidate_or_grows_its_proposal() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let grow = format!("{REVISE} SCRIPT:grow_proposal");
    let cases = [
        ("revise", 0xd10, REVISE, false),
        ("grow", 0xd30, grow.as_str(), true),
    ];
    for (name, lead, message, joined) in cases {
        let fixture = Fixture::new(
            &store,
            &admin,
            lead,
            DeploymentDestination::HostFakeToolLoop,
        )
        .await;
        let mut references = fixture.candidates(&admin).await;
        if joined {
            references.insert(0, fixture.chapter_text(1, 2));
        }
        let run_id = &fixture.run(&store, &admin, 2, message, references).await;
        let records = proposal_records(&admin, &fixture.source_run).await;
        let kinds: Vec<Value> = locations(&store, &fixture.scope, run_id)
            .await
            .as_array()
            .unwrap()
            .iter()
            .map(|location| location["outcome"]["kind"].clone())
            .collect();
        let blocks = if joined { 3 } else { 1 };
        let bundle = if joined { "atomic" } else { "none" };

        assert_eq!(
            (
                kinds,
                records[0]["proposal"].clone(),
                records[0]["revisions"].as_array().unwrap().len(),
                records[0]["operations"].as_array().unwrap().len(),
                propose_edit_calls(&admin, run_id).await,
            ),
            (
                vec![json!("revised"); blocks],
                json!(["block_edit", bundle]),
                2,
                blocks,
                vec![opened(0)],
            ),
            "{name}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_call_that_breaks_the_growth_rule_is_refused_and_no_proposal_changes() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0x430, tool_loop.clone()).await;
    let refused = json!([
        0,
        "refused",
        "change_shape",
        "The Host refused this call because it changes a pending candidate together with \
         blocks that cannot join its Proposal, so no Proposal opened."
    ]);
    let before = proposal_records(&admin, &fixture.source_run).await;
    let mut observed = Vec::new();
    // A Block that is not joinable, and a joinable Block that leaves a gap.
    for (offset, script) in [(2, "grow_unjoinable"), (3, "grow_gap")] {
        let mut references = fixture.candidates(&admin).await;
        references.insert(0, fixture.chapter_text(1, 2));
        let message = format!("{REVISE} SCRIPT:{script}");
        let run_id = fixture
            .run(&store, &admin, offset, &message, references)
            .await;
        observed.push((
            script,
            propose_edit_calls(&admin, &run_id).await,
            proposal_records(&admin, &run_id).await,
            proposal_records(&admin, &fixture.source_run).await,
        ));
    }
    // Two candidates of a grown Proposal.
    let grown = Fixture::new(&store, &admin, 0x438, tool_loop).await;
    let mut references = grown.candidates(&admin).await;
    references.insert(0, grown.chapter_text(1, 2));
    let grow = format!("{REVISE} SCRIPT:grow_proposal");
    grown.run(&store, &admin, 2, &grow, references).await;
    let grown_before = proposal_records(&admin, &grown.source_run).await;
    let message = format!("{REVISE} SCRIPT:propose_two_candidates");
    let references = grown.candidates(&admin).await[1..].to_vec();
    let run_id = grown.run(&store, &admin, 3, &message, references).await;
    observed.push((
        "two_candidates",
        propose_edit_calls(&admin, &run_id).await,
        proposal_records(&admin, &run_id).await,
        proposal_records(&admin, &grown.source_run).await,
    ));

    assert_eq!(
        observed,
        vec![
            (
                "grow_unjoinable",
                vec![refused.clone()],
                json!([]),
                before.clone()
            ),
            ("grow_gap", vec![refused.clone()], json!([]), before),
            ("two_candidates", vec![refused], json!([]), grown_before),
        ]
    );
}
