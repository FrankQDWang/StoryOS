use serde_json::json;

use super::*;
use crate::shared_tests::exposure_facts;
use crate::{
    AGENT_PLAN_REGISTRATION, ContextSourceClass, ExposureFacts, HOST_FAKE_TOOL_LOOP_REGISTRATION,
    LocatedPassage, PASSAGE_TERM_LIMIT, PASSAGE_TERM_MAX_CHARS, ToolExposure,
};

fn names(revision: &str, hybrid_search: bool) -> Vec<&'static str> {
    let exposure = ToolExposure::compute(&ExposureFacts {
        hybrid_search,
        ..exposure_facts(revision)
    });
    exposure.tools.iter().map(|tool| tool.name).collect()
}

/// ADR 0095 sections 1 and 4: the current mapping and the fake tool loop mapping declare
/// `hybrid_search_manuscript` after `search_manuscript` and before `finish` only when the search
/// by meaning is possible. The description tells the model to use the keyword search first.
#[test]
fn each_current_mapping_declares_the_tool_only_when_the_search_by_meaning_is_possible() {
    for revision in [
        AGENT_PLAN_REGISTRATION.revision,
        HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
    ] {
        let exposure = ToolExposure::compute(&ExposureFacts {
            hybrid_search: true,
            ..exposure_facts(revision)
        });
        let declaration = exposure.tools[5].declaration();
        assert_eq!(
            (
                names(revision, true),
                names(revision, false),
                declaration.name,
                declaration.description,
                declaration.parameters,
            ),
            (
                vec![
                    "propose_edit",
                    "submit_research",
                    "read_tool_result",
                    "read_chapter",
                    "search_manuscript",
                    "hybrid_search_manuscript",
                    "write_memory",
                    "finish"
                ],
                vec![
                    "propose_edit",
                    "submit_research",
                    "read_tool_result",
                    "read_chapter",
                    "search_manuscript",
                    "write_memory",
                    "finish"
                ],
                "hybrid_search_manuscript",
                "Finds the passages of the manuscript that answer a question in ordinary words, \
                 also when the passages do not have the words of the question. Use \
                 search_manuscript first. Use this Tool when search_manuscript finds nothing, or \
                 when the question has no exact term. Give the exact terms that you know, for \
                 example names, in terms. Then read the Blocks near a passage with read_chapter.",
                json!({
                    "type": "object",
                    "properties": {
                        "question": {
                            "type": "string",
                            "description": "The question in ordinary words."
                        },
                        "terms": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "The exact words or phrases that the passages can \
                                            have, with the same letter case. The list can be \
                                            empty."
                        }
                    },
                    "required": ["question", "terms"],
                    "additionalProperties": false
                })
            )
        );
    }
    // A background Run gets only the Tools of its kind, and a capture does not get the search by
    // meaning.
    let capture = ToolExposure::compute(&ExposureFacts {
        hybrid_search: true,
        run_kind: crate::AgentRunKind::MemoryCapture,
        ..exposure_facts(AGENT_PLAN_REGISTRATION.revision)
    });
    assert_eq!(
        (capture.tools.iter().map(|tool| tool.name)).collect::<Vec<_>>(),
        [
            "read_chapter",
            "search_manuscript",
            "read_memory_entry",
            "record_memory_observations"
        ]
    );
}

/// The question has the query rule of `web_search`. The terms can be empty, have at most 8
/// terms of 1 to 64 characters, and a repeated term is removed.
#[test]
fn the_intake_takes_a_question_and_terms_at_their_bounds() {
    let intake = |arguments| HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC.intake(&arguments);
    let longest = "雾".repeat(512);
    let term = "潮".repeat(PASSAGE_TERM_MAX_CHARS);
    let most: Vec<String> = (0..PASSAGE_TERM_LIMIT).map(|n| format!("词{n}")).collect();
    let above: Vec<String> = (0..=PASSAGE_TERM_LIMIT).map(|n| format!("词{n}")).collect();
    let ok = |question: &str, terms: &[&str]| {
        Ok(ToolIntake::HybridSearchManuscript {
            question: question.to_owned(),
            terms: terms.iter().map(|term| (*term).to_owned()).collect(),
        })
    };
    assert_eq!(
        [
            intake(json!({"question": "谁在渡口等船？", "terms": []})),
            intake(json!({"question": longest, "terms": ["林雪", "林雪"]})),
            intake(json!({"question": "灯塔", "terms": [term]})),
            intake(json!({"question": format!("{longest}雾"), "terms": []})),
            intake(json!({"question": " \n ", "terms": []})),
            intake(json!({"question": "a\tb", "terms": []})),
            intake(json!({"question": "灯塔", "terms": above})),
            intake(json!({"question": "灯塔", "terms": [""]})),
            intake(json!({"question": "灯塔", "terms": [format!("{term}潮")]})),
            intake(json!({"question": "灯塔"})),
            intake(json!({"question": "灯塔", "terms": [], "query": "x"})),
            intake(json!({"question": "灯塔", "terms": "林雪"})),
        ],
        [
            ok("谁在渡口等船？", &[]),
            ok(&longest, &["林雪"]),
            ok("灯塔", &[&term]),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Terms),
            Err(IntakeRefusal::Terms),
            Err(IntakeRefusal::Terms),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
        ],
    );
    assert!(intake(json!({"question": "灯塔", "terms": most})).is_ok());
}

/// The ToolCall row cannot hold U+0000, so a term with it refuses the call (#1694).
#[test]
fn a_term_with_u0000_refuses_the_call_as_unstorable_text() {
    let intake = |arguments| HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC.intake(&arguments);
    assert_eq!(
        [
            intake(json!({"question": "灯塔", "terms": ["港\u{0}口"]})),
            intake(json!({"question": "灯塔", "terms": ["林雪", "\u{0}"]})),
        ],
        [
            Err(IntakeRefusal::UnstorableText),
            Err(IntakeRefusal::UnstorableText),
        ],
    );
}

/// The refusal table of ADR 0095 section 1: an intake refusal of the field, and
/// `hybrid_search_unavailable` when the configuration or the vector store is not available, or
/// when the admission refuses the embedding request of the question.
#[test]
fn the_host_refuses_a_call_with_no_configuration_or_no_vector_store() {
    let refusal = |refused: IntakeRefusal| crate::CallValidationRefusal::Intake(refused).label();
    let unavailable = HybridSearchRefusal::Unavailable;
    assert_eq!(
        (
            refusal(IntakeRefusal::Query),
            refusal(IntakeRefusal::Terms),
            admit_hybrid_search(true, true),
            admit_hybrid_search(false, true),
            admit_hybrid_search(true, false),
            [
                crate::EmbeddingRefusal::NotConfigured,
                crate::EmbeddingRefusal::BindingAbsent,
                crate::EmbeddingRefusal::DisclosureOutsideGrant,
                crate::EmbeddingRefusal::InputOutOfBounds,
                crate::EmbeddingRefusal::CredentialUnavailable,
                crate::EmbeddingRefusal::UnsupportedRequest,
            ]
            .map(HybridSearchRefusal::of_embedding),
            unavailable.label(),
            unavailable.result().text,
        ),
        (
            "intake_query",
            "intake_terms",
            Ok(()),
            Err(unavailable),
            Err(unavailable),
            [unavailable; 6],
            "hybrid_search_unavailable",
            "The Host refused this call because the search by meaning is not available now: use \
             search_manuscript, so no result is available, and this is not an empty result."
                .to_owned(),
        )
    );
}

/// With no vector of the question, the result is the exact-term list, and its first line says
/// that the search by meaning was not available. With the nearest rows, the result joins the two
/// lists and has no such line. The result has the class of a manuscript search.
#[test]
fn the_result_joins_the_vector_list_or_says_that_the_search_by_meaning_was_not_available() {
    let chapter = ChapterPassages {
        chapter_id: "chapter-1".to_owned(),
        title: "渡口".to_owned(),
        passages: (["风很大。", "林雪在渡口等船。"].iter().enumerate())
            .map(|(index, text)| LocatedPassage {
                first_block: index + 1,
                last_block: index + 1,
                text: (*text).to_owned(),
                digest: format!("sha256:{index}"),
            })
            .collect(),
    };
    let terms = PassageTerms::new(&["林雪".to_owned()]).unwrap();
    let none = PassageTerms::new(&[]).unwrap();
    // The nearest row of the question is the first passage, which has no term. The two passages
    // have equal scores, so they go in manuscript order.
    let nearest = [crate::NearestPassage {
        chapter_id: "chapter-1".to_owned(),
        passage_digest: "sha256:0".to_owned(),
    }];
    let places = |text: String| {
        let lines = (text.lines().filter(|line| line.starts_with("[block"))).map(str::to_owned);
        (
            lines.collect::<Vec<_>>(),
            text.starts_with(NO_SEARCH_BY_MEANING),
        )
    };

    assert_eq!(
        (
            hybrid_search_result(std::slice::from_ref(&chapter), None, &terms, "call-1").text,
            (hybrid_search_result(std::slice::from_ref(&chapter), None, &none, "call-1").text).starts_with(&format!(
                "{NO_SEARCH_BY_MEANING}\n{{\"passages\":0,\"passages_returned\":0,\"terms\":[]}}\n"
            )),
            crate::host_tool_result_class(&HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC),
            places(hybrid_search_result(&[chapter], Some(&nearest), &terms, "call-1").text),
        ),
        (
            format!(
                "{NO_SEARCH_BY_MEANING}\n{{\"passages\":1,\"passages_returned\":1,\"terms\":[\"林雪\"]}}\n\
                 {{\"chapter\":1,\"chapter_id\":\"chapter-1\",\"title\":\"渡口\"}}\n[block 2]\n\
                 林雪在渡口等船。"
            ),
            true,
            ContextSourceClass::ManuscriptSearchResult,
            (vec!["[block 1]".to_owned(), "[block 2]".to_owned()], false),
        )
    );
}
