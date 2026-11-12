use serde_json::{Value, json};
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ConversationSelection,
    EditorSessionStore, ExportProjectArchiveInput, IssueProjectCommandChallenge, NoContractFaults,
    ProjectScope, RejectProposalOperationsInput, RejectionNote, ResearchArtifactReadStore,
    ResearchSourceReadStore, VerifiedExportArchive, claim_next_archive_export, complete_agent_run,
    complete_agent_run_with_tools, complete_archive_export, get_verified_export_archive,
    issue_project_command_challenge,
};
use storyos_contracts::ResearchArtifactInspect;
use storyos_core::{
    Citation, DeploymentDestination, ResearchClaim, ResearchSynthesis, ToolEnablement,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, admit_run_in, claim_run, stores};
use crate::agent_run_retrieval::tests::{adapter, broken_stream, destination, real_run, stored};
use crate::agent_run_tool_call::tests::set_research_tool;
use crate::agent_run_tool_dispatch::tests::{Hooked, exa_tools, mcp_replies, point_mcp, reply};
use crate::agent_run_tool_exposure::tests::sse;
use crate::command_sequence::tests::{REJECT_PROPOSAL_OPERATIONS, issued, seed_project};
use crate::project_archive_build::tests::{export_command, zip_store_files};
use crate::research_source::tests::{SEARCH_TEXT, calls_stream};
use crate::set_current_chapter_authority_tests::open_session_request;

const SEARCH: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;
const FETCH: &str = r#"{"urls":["https://example.org/c","https://example.org/d"]}"#;
const FAILED_PAGE: &str = "Error fetching https://example.org/c: CRAWL_NOT_FOUND";

/// One reply of the model destination, made from the JSON body of its request.
pub(crate) type Reply = Box<dyn FnOnce(&Value) -> String + Send>;

/// The claims of a synthesis, from the tagged Snapshot and Gap identities, in that order.
type Cite = fn(&[String], &[String]) -> Vec<(&'static str, Vec<String>)>;

/// A model destination that reads each complete request and answers it with the next reply.
/// Returns the base URL and the request lines.
pub(crate) async fn computed(
    replies: Vec<Reply>,
) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    let served = tokio::spawn(async move {
        let mut lines = Vec::new();
        for reply in replies {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 8192];
            let head_end = loop {
                if let Some(end) = request
                    .windows(/*size*/ 4)
                    .position(|window| window == b"\r\n\r\n")
                {
                    break end + 4;
                }
                let read = stream.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..read]);
            };
            let raw = String::from_utf8_lossy(&request[..head_end]).into_owned();
            lines.push(raw.lines().next().unwrap_or_default().to_owned());
            let head = raw.to_lowercase();
            let length: usize = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .map_or(0, |value| value.trim().parse().unwrap());
            while request.len() < head_end + length {
                let read = stream.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..read]);
            }
            let body = serde_json::from_slice(&request[head_end..]).unwrap_or(Value::Null);
            stream.write_all(reply(&body).as_bytes()).await.unwrap();
        }
        lines
    });
    (base, served)
}

/// A stream that reports `reference` and ends with one message of `text` in `status`.
pub(crate) fn message_stream(reference: &str, text: &str, status: &str) -> String {
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": status, "content": [{"type": "output_text", "text": text}]});
    sse(&[
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": message}),
        json!({"type": format!("response.{status}"), "response": {
            "id": reference, "status": status, "output": [message],
            "usage": {"input_tokens": 40, "output_tokens": 12}}}),
    ])
}

/// The identities that the `tool_sources` tags of the function outputs of `body` name after
/// `tag`, in order.
pub(crate) fn tagged(body: &Value, tag: &str) -> Vec<String> {
    body["input"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["output"].as_str())
        .flat_map(|output| {
            output
                .split(tag)
                .skip(1)
                .map(|rest| rest.split('"').next().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// A synthesis reply in `status` whose claims cite the tagged identities that `cite` selects.
fn synthesis(reference: &str, status: &'static str, cite: Cite) -> Reply {
    let reference = reference.to_owned();
    Box::new(move |body| {
        let claims = cite(&tagged(body, "<source id=\""), &tagged(body, "<gap id=\""));
        let text = json!({"kind": "research_synthesis", "summary": "The tides of 1890.",
            "claims": claims.iter()
                .map(|(text, citations)| json!({"text": text, "citations": citations}))
                .collect::<Vec<_>>()});
        message_stream(&reference, &text.to_string(), status)
    })
}

/// Settles one research Run whose first response requests `call`. The fake MCP server answers
/// with `text`, and `later` gives the next replies of the model destination, from the target
/// block of the Run.
async fn synthesis_run(
    prefix: &str,
    suffix: u16,
    call: (&str, &str),
    text: &str,
    later: impl FnOnce(&str) -> Vec<Reply>,
) -> (PostgresProjectReader, Client, ProjectScope, String) {
    let (store, admin) = stores().await;
    let (scope, run_id) = research_run(&store, &admin, prefix, "http://127.0.0.1:9/api/plan/v3").await;
    let block: String = admin
        .query_one(
            "SELECT payload #>> '{operation_requirement,proposal_target_block_ids,0}'
               FROM storyos.operation_requirements
              WHERE run_id = $1::text::uuid AND decision_position = 0",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let first = calls_stream(&format!("resp_{prefix}"), &[("call_01", call.0, call.1)]);
    let mut replies: Vec<Reply> = vec![Box::new(move |_| first)];
    replies.extend(later(&block));
    let (model_base, _model) = computed(replies).await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2
              WHERE project_id = $1::text::uuid AND evidence_kind = 'volcengine_agent_plan_boundary'",
            &[&scope.project_id.as_ref(), &model_base],
        )
        .await
        .unwrap();
    let result = reply(
        /*id*/ 0,
        json!({"content": [{"type": "text", "text": text}], "isError": false}),
    );
    let (mcp_base, _mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(result))).await;
    point_mcp(&admin, &scope, &mcp_base).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let tools = Hooked {
        admin: &admin,
        after_prepare: String::new(),
        after_exchange: String::new(),
    };
    complete_agent_run_with_tools(
        &store,
        &adapter(),
        &tools,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    (store, admin, scope, run_id)
}

/// The Research Artifact identity of the settled Run, from its Decision in `getAgentRun`.
async fn artifact_id(store: &PostgresProjectReader, scope: &ProjectScope, run_id: &str) -> String {
    let record = store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let AgentRunDecisionInspect::ResearchSynthesis {
        selected: true,
        research_artifact_id,
        continuation_binding_id: Some(_),
        ..
    } = record.decision
    else {
        panic!(
            "the Run must select one research synthesis: {:?}",
            record.decision
        );
    };
    research_artifact_id
}

/// The expected Artifact of the Run, with no creation time and no content digest: its lineage
/// from the stored rows, `claims` as the
/// numbered claims, and the listed sources that `cited` selects in citation order.
async fn expected(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
    artifact_id: &str,
    claims: Value,
    cited: fn(&storyos_application::ResearchSources) -> Vec<Value>,
) -> Value {
    let row = admin
        .query_one(
            "SELECT requirement.operation_requirement_id::text,
                    attempt.model_attempt_id::text, attempt.decision_id::text,
                    call.tool_call_id::text
               FROM storyos.research_artifacts AS artifact
               JOIN storyos.operation_requirements AS requirement
                 ON requirement.run_id = artifact.run_id AND requirement.decision_position = 1
                AND requirement.requirement_role = 'primary'
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = artifact.run_id AND attempt.decision_position = 1
                AND attempt.attempt_role = 'decision'
               JOIN storyos.tool_calls AS call ON call.run_id = artifact.run_id
              WHERE artifact.research_artifact_id = $1::text::uuid",
            &[&artifact_id],
        )
        .await
        .unwrap();
    let sources = store
        .list_research_sources(scope, run_id)
        .await
        .unwrap()
        .unwrap();
    json!({
        "research_artifact_id": artifact_id,
        "lineage": {"run_id": run_id, "decision_position": "1",
                    "operation_requirement_id": row.get::<_, String>(0),
                    "model_attempt_id": row.get::<_, String>(1),
                    "decision_id": row.get::<_, String>(2),
                    "tool_call_ids": [row.get::<_, String>(3)]},
        "summary": "The tides of 1890.",
        "claims": claims,
        "citations": cited(&sources),
    })
}

/// The Artifact as JSON with no creation time and no content digest, and its content digest. The
/// creation time must be an RFC 3339 UTC time.
fn without_digest(artifact: &ResearchArtifactInspect) -> (Value, String) {
    assert!(
        artifact.created_at.ends_with('Z'),
        "{}",
        artifact.created_at
    );
    let mut value = serde_json::to_value(artifact).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("created_at");
    let digest = object.remove("content_digest").unwrap();
    (value, digest.as_str().unwrap().to_owned())
}

async fn artifact_count(admin: &Client, run_id: &str) -> i64 {
    admin
        .query_one(
            "SELECT count(*) FROM storyos.research_artifacts WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

fn two_snapshot_claims(sources: &[String], _gaps: &[String]) -> Vec<(&'static str, Vec<String>)> {
    vec![
        ("High water came at 10.42.", vec![sources[0].clone()]),
        (
            "The port log agrees.",
            vec![sources[1].clone(), sources[0].clone()],
        ),
    ]
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_valid_synthesis_becomes_one_immutable_artifact_with_its_lineage_and_citations() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin, scope, run_id) =
        synthesis_run("c31", 0xc31a, ("web_search", SEARCH), SEARCH_TEXT, |_| {
            vec![synthesis("resp_c31_2", "completed", two_snapshot_claims)]
        })
        .await;
    let id = artifact_id(&store, &scope, &run_id).await;
    let artifact = store
        .get_research_artifact(&scope, &id)
        .await
        .unwrap()
        .expect("the Artifact is in Scope");
    let snapshot = |sources: &storyos_application::ResearchSources| {
        sources
            .snapshots
            .iter()
            .map(|snapshot| json!({"kind": "snapshot", "snapshot": snapshot}))
            .collect()
    };
    let claims = json!([{"text": "High water came at 10.42.", "citations": [1]},
                        {"text": "The port log agrees.", "citations": [2, 1]}]);
    let want = expected(&store, &admin, &scope, &run_id, &id, claims, snapshot).await;
    let ids: Vec<String> = artifact
        .citations
        .iter()
        .map(|citation| match citation {
            storyos_contracts::ResearchCitationInspect::Snapshot { snapshot } => {
                snapshot.source_snapshot_id.clone()
            }
            storyos_contracts::ResearchCitationInspect::Gap { gap } => gap.source_gap_id.clone(),
        })
        .collect();
    let snapshot_citation = |index: usize| Citation::Snapshot {
        source_snapshot_id: ids[index].clone(),
    };
    let content = ResearchSynthesis {
        summary: "The tides of 1890.".to_owned(),
        claims: vec![
            ResearchClaim {
                text: "High water came at 10.42.".to_owned(),
                citations: vec![snapshot_citation(0)],
            },
            ResearchClaim {
                text: "The port log agrees.".to_owned(),
                citations: vec![snapshot_citation(1), snapshot_citation(0)],
            },
        ],
    };
    let runtime = store.connect().await.unwrap();
    let mut refused = Vec::new();
    for statement in [
        "UPDATE storyos.research_artifacts SET content = '{}' WHERE research_artifact_id = $1::text::uuid",
        "DELETE FROM storyos.research_artifacts WHERE research_artifact_id = $1::text::uuid",
    ] {
        // Each statement runs in the exact Project Scope, so that only the grant can refuse it.
        runtime.batch_execute("BEGIN").await.unwrap();
        crate::set_challenge_scope_on_client(&runtime, &scope)
            .await
            .unwrap();
        refused.push(
            runtime
                .execute(statement, &[&id])
                .await
                .map_err(|error| crate::sql_state(&error)),
        );
        runtime.batch_execute("ROLLBACK").await.unwrap();
    }
    let other = seed_project(&store, "c3f0").await;

    assert_eq!(
        (
            without_digest(&artifact),
            refused,
            store.get_research_artifact(&other, &id).await.unwrap(),
            artifact_count(&admin, &run_id).await,
        ),
        (
            (want, content.content_digest()),
            vec![Err(Some("42501".to_owned())); 2],
            None,
            1,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_synthesis_that_cites_only_gaps_is_a_partial_artifact() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin, scope, run_id) =
        synthesis_run("c32", 0xc32a, ("web_fetch", FETCH), FAILED_PAGE, |_| {
            vec![synthesis("resp_c32_2", "completed", |_, gaps| {
                vec![("No tide table is available.", gaps.to_vec())]
            })]
        })
        .await;
    let id = artifact_id(&store, &scope, &run_id).await;
    let artifact = store
        .get_research_artifact(&scope, &id)
        .await
        .unwrap()
        .unwrap();
    let gaps = |sources: &storyos_application::ResearchSources| {
        sources
            .gaps
            .iter()
            .map(|gap| json!({"kind": "gap", "gap": gap}))
            .collect()
    };
    let claims = json!([{"text": "No tide table is available.", "citations": [1, 2]}]);

    assert_eq!(
        without_digest(&artifact).0,
        expected(&store, &admin, &scope, &run_id, &id, claims, gaps).await
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_invalid_or_incomplete_synthesis_writes_no_artifact() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let mut observed = Vec::new();
    for (prefix, suffix, reply) in [
        (
            "c33",
            0xc33a,
            synthesis("resp_c33_2", "completed", |sources, _| {
                vec![
                    ("High water came at 10.42.", vec![sources[0].clone()]),
                    (
                        "The run never read this page.",
                        vec!["018f0000-0000-7001-8000-0000000c33ff".to_owned()],
                    ),
                ]
            }),
        ),
        (
            "c34",
            0xc34a,
            synthesis("resp_c34_2", "incomplete", two_snapshot_claims),
        ),
    ] {
        let (_store, admin, _scope, run_id) =
            synthesis_run(prefix, suffix, ("web_search", SEARCH), SEARCH_TEXT, |_| {
                vec![reply]
            })
            .await;
        let row = admin
            .query_one(
                "SELECT run.status, attempt.payload #>> '{decision,kind}',
                        attempt.payload ->> 'synthesis_refusal'
                   FROM storyos.agent_runs AS run
                   JOIN storyos.model_attempts AS attempt
                     ON attempt.run_id = run.run_id AND attempt.decision_position = 1
                  WHERE run.run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
            .unwrap();
        observed.push((
            row.get::<_, String>(0),
            row.get::<_, Option<String>>(1),
            row.get::<_, Option<String>>(2),
            artifact_count(&admin, &run_id).await,
        ));
    }

    assert_eq!(
        observed,
        vec![
            (
                "completed".to_owned(),
                None,
                Some("uncaptured_citation".to_owned()),
                0
            ),
            ("completed".to_owned(), None, None, 0),
        ]
    );
}

/// The facts of "Use this in the story": the settled synthesis Run and its Artifact, and the
/// second Run of the conversation with its Proposal row. The Proposal row has its identity, its
/// lineage, its head revision, its operations, and the identities of the Operation Requirement and
/// of the Model Attempt that produced it. With `successor`, the create of the second Run has an
/// unknown outcome, and its successor produces the Proposal.
struct Story {
    store: PostgresProjectReader,
    admin: Client,
    scope: ProjectScope,
    run_id: String,
    artifact_id: String,
    story_run: String,
    proposal: tokio_postgres::Row,
}

async fn use_in_the_story(prefix: &str, story: &str, successor: bool) -> Story {
    let reference = |suffix: &str| format!("resp_{prefix}{suffix}");
    let (store, admin, scope, run_id) = synthesis_run(
        prefix,
        u16::from_str_radix(&format!("{prefix}a"), 16).unwrap(),
        ("web_search", SEARCH),
        SEARCH_TEXT,
        |block| {
            let change = json!({"kind": "prose_change", "summary": "Added the tides.",
                "changes": [{"block_id": block, "candidate_text": "High water came at 10.42.",
                             "explanation": "The research gives the tides."}]})
            .to_string();
            let (lost, retrieved, late) = (reference("_3"), reference("_3"), reference("_3"));
            let story = reference("_4");
            let mut replies: Vec<Reply> = vec![synthesis(
                &reference("_2"),
                "completed",
                two_snapshot_claims,
            )];
            if successor {
                replies.push(Box::new(move |_| broken_stream(&lost)));
                replies.push(Box::new(move |_| stored(&retrieved, "in_progress")));
                replies.push(Box::new(move |_| {
                    message_stream(&story, &change, "completed")
                }));
                replies.push(Box::new(move |_| stored(&late, "completed")));
            } else {
                replies.push(Box::new(move |_| {
                    message_stream(&story, &change, "completed")
                }));
            }
            replies
        },
    )
    .await;
    let artifact_id = artifact_id(&store, &scope, &run_id).await;
    let run: tokio_postgres::Row = admin
        .query_one(
            "SELECT chapter_id::text, conversation_id::text FROM storyos.agent_runs
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    let story_run = admit_run_in(
        &store,
        &scope,
        run.get(/*idx*/ 0),
        story,
        DeploymentDestination::VolcengineAgentPlan {
            qualification_authorization: None,
        },
        "Use this research in the story.",
        ConversationSelection::Existing {
            conversation_id: run.get(/*idx*/ 1),
        },
        None,
    )
    .await;
    let claim = claim_run(&admin, scope.clone(), story_run.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let proposal = admin
        .query_one(
            "SELECT proposal.proposal_id::text, proposal.source_lineage::text,
                    head.current_revision_id::text,
                    ARRAY(SELECT operation.operation_id::text
                            FROM storyos.proposal_operations AS operation
                           WHERE operation.proposal_id = proposal.proposal_id),
                    requirement.operation_requirement_id::text, attempt.model_attempt_id::text
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head USING (owner_user_id, project_id, proposal_id)
               JOIN storyos.operation_requirements AS requirement
                 ON requirement.run_id = proposal.source_run_id
                AND requirement.decision_position = 0 AND requirement.requirement_role = 'primary'
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = proposal.source_run_id AND attempt.decision_position = 0
                AND attempt.attempt_role = $2
              WHERE proposal.source_run_id = $1::text::uuid",
            &[
                &story_run,
                &if successor { "successor" } else { "decision" },
            ],
        )
        .await
        .unwrap();
    Story {
        store,
        admin,
        scope,
        run_id,
        artifact_id,
        story_run,
        proposal,
    }
}

/// The lineage that the Proposal of `story` must have: the RunStep of the Proposal, the Model
/// Attempt that produced it, and the Artifact that it read.
fn story_lineage(story: &Story) -> Value {
    json!({"kind": "research_artifact", "run_id": story.story_run, "decision_position": 0,
           "operation_requirement_id": story.proposal.get::<_, String>(4),
           "model_attempt_id": story.proposal.get::<_, String>(5),
           "research_artifact_id": story.artifact_id})
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn use_in_the_story_opens_a_proposal_with_the_artifact_and_a_rejection_keeps_it() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let story = use_in_the_story("c35", "c36", /*successor*/ false).await;
    let Story {
        store,
        scope,
        run_id,
        artifact_id: id,
        proposal,
        ..
    } = &story;
    let before = (
        store.get_research_artifact(scope, id).await.unwrap(),
        store.list_research_sources(scope, run_id).await.unwrap(),
    );
    let request = open_session_request(store, scope, "c3c0").await;
    let session = store.create_editor_session(&request).await.unwrap();
    let rejection = issued(
        store,
        scope,
        /*suffix*/ 0xc3ca,
        &REJECT_PROPOSAL_OPERATIONS,
        RejectProposalOperationsInput {
            editor_session_id: request.editor_session_id.clone(),
            proposal_id: proposal.get(/*idx*/ 0),
            proposal_revision_id: proposal.get(/*idx*/ 2),
            selected_pending_operation_ids: proposal.get(/*idx*/ 3),
            expected_authoritative_revision_id: session.base_snapshot.authoritative_revision_id,
            rejection_note: RejectionNote::Omitted,
        },
    )
    .await;
    store
        .reject_proposal_operations(&rejection.envelope, &rejection.input)
        .await
        .unwrap();
    let after = (
        store.get_research_artifact(scope, id).await.unwrap(),
        store.list_research_sources(scope, run_id).await.unwrap(),
    );

    assert_eq!(
        (
            serde_json::from_str::<Value>(proposal.get::<_, &str>(1)).unwrap(),
            after,
        ),
        (story_lineage(&story), before)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_successor_proposal_names_the_successor_attempt_and_the_artifact() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let story = use_in_the_story("c39", "c3b", /*successor*/ true).await;
    let roles: String = story
        .admin
        .query_one(
            "SELECT string_agg(attempt_role, ',' ORDER BY model_attempt_id)
               FROM storyos.model_attempts WHERE run_id = $1::text::uuid",
            &[&story.story_run],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (
            serde_json::from_str::<Value>(story.proposal.get::<_, &str>(1)).unwrap(),
            roles,
        ),
        (
            story_lineage(&story),
            "decision,retrieval,successor,late_retrieval".to_owned()
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_project_export_packs_the_artifact_and_a_reload_reads_it_unchanged() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin, scope, run_id) =
        synthesis_run("c37", 0xc37a, ("web_search", SEARCH), SEARCH_TEXT, |_| {
            vec![synthesis("resp_c37_2", "completed", two_snapshot_claims)]
        })
        .await;
    let id = artifact_id(&store, &scope, &run_id).await;
    let before = store.get_research_artifact(&scope, &id).await.unwrap();
    let (export, input): (_, ExportProjectArchiveInput) = export_command(&scope, "c3");
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: export.challenge_binding.clone(),
            nonce: "nonce-export-c3".to_owned(),
            nonce_digest: export.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    store.export_project_archive(&export, &input).await.unwrap();
    let claimed = claim_next_archive_export(&store).await.unwrap().unwrap();
    complete_archive_export(&store, &claimed).await.unwrap();
    let VerifiedExportArchive::Ready(zip) =
        get_verified_export_archive(&store, &scope, &input.export_id)
            .await
            .unwrap()
    else {
        panic!("the Project Export must return verified ZIP bytes");
    };
    let files = zip_store_files(&zip);
    let rows = &files
        .iter()
        .find(|(name, _)| *name == "canonical/research_artifacts.json")
        .unwrap()
        .1;
    let packed: Vec<Value> = serde_json::from_slice(rows).unwrap();
    // A restore loads the packed rows again in place of the stored rows.
    admin
        .execute(
            "DELETE FROM storyos.research_artifacts WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    admin
        .execute(
            "INSERT INTO storyos.research_artifacts
             SELECT * FROM jsonb_populate_recordset(NULL::storyos.research_artifacts,
                                                    $1::text::jsonb)",
            &[&std::str::from_utf8(rows).unwrap()],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            packed.len(),
            store.get_research_artifact(&scope, &id).await.unwrap(),
        ),
        (1, before)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_retrieved_synthesis_of_the_first_runstep_writes_no_artifact_and_keeps_its_refusal() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The first RunStep has no ToolResult, so the Run captured no source to cite.
    let text = json!({"kind": "research_synthesis", "summary": "The tides of 1890.",
        "claims": [{"text": "High water came at 10.42.",
                    "citations": ["018f0000-0000-7001-8000-0000000c38ff"]}]})
    .to_string();
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": text}]});
    let body = json!({"id": "resp_c38", "status": "completed", "output": [message],
        "usage": {"input_tokens": 40, "output_tokens": 12}})
    .to_string();
    let (base, _served) = destination(vec![
        broken_stream("resp_c38"),
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
             connection: close\r\n\r\n{body}",
            body.len()
        ),
    ])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "c38", &base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let row = admin
        .query_one(
            "SELECT attempt.payload ->> 'result', attempt.payload ->> 'synthesis_refusal'
               FROM storyos.model_attempts AS attempt
              WHERE attempt.run_id = $1::text::uuid AND attempt.attempt_role = 'retrieval'",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            row.get::<_, Option<String>>(0),
            row.get::<_, Option<String>>(1),
            artifact_count(&admin, &run_id).await,
        ),
        (
            Some("incomplete".to_owned()),
            Some("uncaptured_citation".to_owned()),
            0
        )
    );
}
