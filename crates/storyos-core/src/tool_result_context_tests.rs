use super::*;
use crate::shared_tests::harbour_passage;
use crate::shared_tests::tool_result_candidate;
use crate::{
    CitableSource, CitedSource, SourceGapReason, decode_assembly_record, encode_assembly_record,
};

fn source(tool_call_id: &str, state: &str, kept_result_tokens: Option<u64>) -> ToolResultSource {
    ToolResultSource {
        tool_call_id: tool_call_id.to_owned(),
        call_id: tool_call_id.to_owned(),
        model_attempt_id: "attempt-1".to_owned(),
        decision_id: "decision-1".to_owned(),
        state: state.to_owned(),
        trust: "data_only".to_owned(),
        kept_result_tokens,
        source_snapshot_ids: Vec::new(),
        skill: None,
        resource_path: None,
    }
}

/// The source version, projection mode, and content of each selected `tool_result` item.
fn tool_items(record: &CurrentPassageAssemblyRecord) -> Vec<(String, ProjectionMode, String)> {
    record
        .selected
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::ToolResult)
        .map(|item| {
            (
                item.source_version.clone(),
                item.projection_mode,
                item.content.clone(),
            )
        })
        .collect()
}

#[test]
fn each_resolved_tool_call_is_one_candidate_named_by_its_snapshots_with_its_citable_ids() {
    let mut record = harbour_passage();
    let snapshots = ["s1".to_owned(), "s2".to_owned()];
    let cited = |id: &str, cited: CitedSource| CitableSource {
        id: id.to_owned(),
        cited,
        locator: format!("https://example.org/{id}"),
        title: None,
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        run_id: "run-1".to_owned(),
    };
    let sources = [
        cited("s1", CitedSource::Snapshot),
        cited("s2", CitedSource::Snapshot),
        cited("g1", CitedSource::Gap(SourceGapReason::Failed)),
    ];

    assemble_tool_result_continuation(
        &mut record,
        &[
            ToolResultCandidate {
                sources: &sources,
                ..tool_result_candidate("c1", ToolCallState::Settled, Some("Built in 1890."))
            },
            tool_result_candidate("c2", ToolCallState::Failed, None),
            tool_result_candidate("c3", ToolCallState::Unknown, None),
            tool_result_candidate("c4", ToolCallState::Refused, None),
        ],
    );

    let exact = ProjectionMode::ExactRequired;
    assert_eq!(
        (
            record.operation_requirement.cause,
            record.operation_requirement.tool_results.clone(),
            tool_items(&record),
            record.sufficiency.clone(),
        ),
        (
            ContextCause::ToolResultContinuation,
            Some(vec![
                ToolResultSource {
                    source_snapshot_ids: snapshots.to_vec(),
                    ..source("c1", "ok", None)
                },
                source("c2", "unavailable", None),
                source("c3", "unavailable", None),
                source("c4", "rejected", None),
            ]),
            vec![
                (
                    "s1,s2".to_owned(),
                    exact,
                    "<tool_result source=\"c1\" state=\"ok\">Built in 1890.</tool_result>\
                     <tool_sources source=\"c1\"><source id=\"s1\" \
                     locator=\"https://example.org/s1\"/><source id=\"s2\" \
                     locator=\"https://example.org/s2\"/><gap id=\"g1\" \
                     locator=\"https://example.org/g1\" reason=\"failed\"/></tool_sources>"
                        .to_owned()
                ),
                (
                    "c2".to_owned(),
                    exact,
                    "<tool_result source=\"c2\" state=\"unavailable\">The call failed, so no \
                     result is available, and this is not an empty result.</tool_result>"
                        .to_owned()
                ),
                (
                    "c3".to_owned(),
                    exact,
                    "<tool_result source=\"c3\" state=\"unavailable\">The outcome of the call is \
                     unknown, so no result is available, and this is not an empty \
                     result.</tool_result>"
                        .to_owned()
                ),
                (
                    "c4".to_owned(),
                    exact,
                    "<tool_result source=\"c4\" state=\"rejected\">The Host refused this call \
                     before it was sent, so no result is available, and this is not an empty \
                     result.</tool_result>"
                        .to_owned()
                ),
            ],
            ContextSufficiency::Complete,
        )
    );
    assert_eq!(
        decode_assembly_record(&encode_assembly_record(&record)),
        Some(record)
    );
}

#[test]
fn an_unresolved_tool_call_or_one_of_another_run_blocks_the_continuation() {
    let mut record = harbour_passage();
    let other_run = ToolResultCandidate {
        run_id: "run-2",
        ..tool_result_candidate("c2", ToolCallState::Settled, Some("Built in 1890."))
    };

    assemble_tool_result_continuation(
        &mut record,
        &[
            tool_result_candidate("c1", ToolCallState::Dispatched, None),
            other_run,
        ],
    );

    let rejected = |id: &str| RejectedSource {
        source_class: ContextSourceClass::ToolResult,
        source_version: id.to_owned(),
        token_count: 0,
        reason: RejectionReason::ToolResultUnavailable,
    };
    assert_eq!(
        (
            tool_items(&record),
            record.rejected.clone(),
            record.sufficiency.clone()
        ),
        (
            Vec::new(),
            vec![rejected("c1"), rejected("c2")],
            ContextSufficiency::Blocked {
                reasons: vec![ContextBlockReason::ToolResultUnavailable],
            },
        )
    );
}

#[test]
fn a_result_above_the_item_limit_keeps_a_prefix_and_an_explicit_truncation_mark() {
    let mut record = harbour_passage();
    record.operation_requirement.item_token_limit = 200;
    let long = "a<b".repeat(40);

    assemble_tool_result_continuation(
        &mut record,
        &[tool_result_candidate(
            "c1",
            ToolCallState::Settled,
            Some(&long),
        )],
    );

    let item = record
        .selected
        .iter()
        .find(|item| item.source_class == ContextSourceClass::ToolResult)
        .unwrap();
    let kept = record.operation_requirement.tool_results.as_ref().unwrap()[0].kept_result_tokens;
    let prefix = format!("{}a", "a&lt;b".repeat(13));
    assert_eq!(
        (
            item.projection_mode,
            kept,
            item.content.as_str(),
            item.token_count <= 200,
            item.token_count == count_context_item_tokens(&item.content),
        ),
        (
            ProjectionMode::DeterministicExcerpt,
            Some(40),
            format!(
                "<tool_result source=\"c1\" state=\"ok\">{prefix} [The Host cut this result \
                 here. It keeps 40 of its 120 characters.]</tool_result>"
            )
            .as_str(),
            true,
            true,
        )
    );
}

#[test]
fn a_cut_result_keeps_each_citable_id_whole_inside_the_item_limit() {
    let mut record = harbour_passage();
    record.operation_requirement.item_token_limit = 300;
    let long = "a<b".repeat(40);
    let sources = [CitableSource {
        id: "018f0000-0000-7001-8000-000000000a01".to_owned(),
        cited: CitedSource::Snapshot,
        locator: "https://example.org/tides".to_owned(),
        title: None,
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        run_id: "run-1".to_owned(),
    }];
    let tag = crate::citable_sources_tag("c1", &sources);

    assemble_tool_result_continuation(
        &mut record,
        &[ToolResultCandidate {
            sources: &sources,
            ..tool_result_candidate("c1", ToolCallState::Settled, Some(&long))
        }],
    );

    let item = record
        .selected
        .iter()
        .find(|item| item.source_class == ContextSourceClass::ToolResult)
        .unwrap();
    let kept = record.operation_requirement.tool_results.as_ref().unwrap()[0].kept_result_tokens;
    assert_eq!(
        (
            item.content.ends_with(&format!("</tool_result>{tag}")),
            item.token_count <= 300,
            kept.is_some_and(|kept| kept < 40),
        ),
        (true, true, true)
    );
}

#[test]
fn a_citable_tag_above_the_item_limit_rejects_the_result_and_blocks_the_continuation() {
    let mut record = harbour_passage();
    record.operation_requirement.item_token_limit = 300;
    let sources = [CitableSource {
        id: "018f0000-0000-7001-8000-000000000a01".to_owned(),
        cited: CitedSource::Snapshot,
        locator: format!("https://example.org/{}", "a".repeat(2000)),
        title: None,
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        run_id: "run-1".to_owned(),
    }];

    assemble_tool_result_continuation(
        &mut record,
        &[ToolResultCandidate {
            sources: &sources,
            ..tool_result_candidate("c1", ToolCallState::Settled, Some("High water at 10.42."))
        }],
    );

    assert_eq!(
        (
            record
                .selected
                .iter()
                .any(|item| item.source_class == ContextSourceClass::ToolResult),
            record
                .rejected
                .iter()
                .filter(|item| item.source_class == ContextSourceClass::ToolResult)
                .map(|item| item.reason.clone())
                .collect::<Vec<_>>(),
            record.sufficiency.clone(),
        ),
        (
            false,
            vec![RejectionReason::OverItemTokenLimit],
            ContextSufficiency::Blocked {
                reasons: vec![ContextBlockReason::ExactRequiredOverLimit {
                    source_class: ContextSourceClass::ToolResult,
                }],
            },
        )
    );
}

#[test]
fn only_a_long_research_result_or_read_in_the_stub_form_becomes_a_stub() {
    let long = "Tide. ".repeat(500);
    let research = |id, text, form| ToolResultCandidate {
        form,
        ..tool_result_candidate(id, ToolCallState::Settled, Some(text))
    };
    let candidates = [
        research("call_long", &long, ToolResultForm::Stub),
        research("call_short", "High water at 10.42.", ToolResultForm::Stub),
        research("call_exact", &long, ToolResultForm::Exact),
        ToolResultCandidate {
            output: ToolResultOutput::HostTool(crate::ContextSourceClass::ToolResult),
            function_name: "propose_edit",
            ..research("call_host", &long, ToolResultForm::Stub)
        },
        ToolResultCandidate {
            output: ToolResultOutput::StoredRead,
            function_name: "read_tool_result",
            arguments: r#"{"call_id":"call_long"}"#,
            ..research("call_read", &long, ToolResultForm::Stub)
        },
    ];
    let mut record = harbour_passage();
    assemble_tool_result_continuation(&mut record, &candidates);
    let modes: Vec<ProjectionMode> = tool_items(&record).into_iter().map(|item| item.1).collect();
    let read = &tool_items(&record)[4].2;

    assert_eq!(
        (
            modes,
            stub_tool_call_ids(&record),
            read.contains("the full result of the call call_long"),
            record.considered[record.considered.len() - 1].token_count
                > crate::TOOL_RESULT_STUB_TOKEN_LIMIT,
        ),
        (
            vec![
                ProjectionMode::ToolResultStub,
                ProjectionMode::ExactRequired,
                ProjectionMode::ExactRequired,
                ProjectionMode::ExactRequired,
                ProjectionMode::ToolResultStub,
            ],
            vec!["call_long".to_owned(), "call_read".to_owned()],
            true,
            true,
        )
    );
}

#[test]
fn the_form_of_a_result_follows_the_installed_stubs_and_the_waiting_decision() {
    let decision = |installed_stub| ToolResultForm::in_decision_request(installed_stub);
    let later = |installed_stub, waiting| {
        ToolResultForm::in_later_request(installed_stub, "decision-1", waiting)
    };

    assert_eq!(
        (
            // A decision request: only a result that an installed compaction sent as a stub.
            [decision(true), decision(false)],
            // The later request of a compaction: each result that an earlier decision request
            // sent, and the outputs of the response that the summary request continued wait.
            [
                later(false, Some("decision-0")),
                later(false, Some("decision-1")),
                later(true, Some("decision-1")),
                later(false, None),
            ],
        ),
        (
            [ToolResultForm::Stub, ToolResultForm::Exact],
            [
                ToolResultForm::Stub,
                ToolResultForm::Exact,
                ToolResultForm::Stub,
                ToolResultForm::Stub,
            ],
        )
    );
}

/// ADR 0095 section 1: the settled result of `read_chapter` is a `manuscript_read_result` item,
/// and the settled result of `search_manuscript` is a `manuscript_search_result` item, each with
/// the trust `data_only` and the ToolCall identity as its source version. Each is a function
/// output, it keeps its exact form in the stub form, and the bound cut gives the mode
/// `deterministic_excerpt`. The output of a refused read stays a `tool_result` item.
#[test]
fn a_settled_manuscript_read_or_search_keeps_its_exact_form_in_its_class() {
    fn read<'a>(id: &'a str, state: ToolCallState, text: &'a str) -> ToolResultCandidate<'a> {
        ToolResultCandidate {
            output: ToolResultOutput::of_spec(Some(crate::READ_CHAPTER_TOOL_SPEC.revision)),
            function_name: "read_chapter",
            arguments: "{\"chapter\":2}",
            form: ToolResultForm::Stub,
            ..tool_result_candidate(id, state, Some(text))
        }
    }
    fn search<'a>(id: &'a str, text: &'a str) -> ToolResultCandidate<'a> {
        ToolResultCandidate {
            output: ToolResultOutput::of_spec(Some(crate::SEARCH_MANUSCRIPT_TOOL_SPEC.revision)),
            function_name: "search_manuscript",
            arguments: "{\"query\":\"Rain\"}",
            ..read(id, ToolCallState::Settled, text)
        }
    }
    // A result above the stub limit, and a result above the item limit.
    let (long, above) = ("Rain. ".repeat(5_000), "a".repeat(40_000));
    let (long, above) = (long.as_str(), above.as_str());
    let candidates = [
        read("read-1", ToolCallState::Settled, long),
        read("read-2", ToolCallState::Settled, above),
        read(
            "read-3",
            ToolCallState::Refused,
            "The Host refused this call.",
        ),
        search("search-1", long),
        search("search-2", above),
    ];
    let mut record = harbour_passage();
    assemble_tool_result_continuation(&mut record, &candidates);
    stub_selected_results(&mut record, &candidates);
    let outputs: Vec<_> = function_outputs(&record)
        .into_iter()
        .map(|(source, item)| {
            (
                source.trust.as_str(),
                item.source_class,
                item.source_version.clone(),
                item.projection_mode,
                item.content.starts_with("<tool_result source=\""),
            )
        })
        .collect();

    assert_eq!(
        outputs,
        [
            (
                "data_only",
                ContextSourceClass::ManuscriptReadResult,
                crate::tool_result_source_version("read-1", &[]),
                ProjectionMode::ExactRequired,
                true,
            ),
            (
                "data_only",
                ContextSourceClass::ManuscriptReadResult,
                crate::tool_result_source_version("read-2", &[]),
                ProjectionMode::DeterministicExcerpt,
                true,
            ),
            (
                "data_only",
                ContextSourceClass::ToolResult,
                crate::tool_result_source_version("read-3", &[]),
                ProjectionMode::ExactRequired,
                true,
            ),
            (
                "data_only",
                ContextSourceClass::ManuscriptSearchResult,
                crate::tool_result_source_version("search-1", &[]),
                ProjectionMode::ExactRequired,
                true,
            ),
            (
                "data_only",
                ContextSourceClass::ManuscriptSearchResult,
                crate::tool_result_source_version("search-2", &[]),
                ProjectionMode::DeterministicExcerpt,
                true,
            ),
        ]
    );
}
