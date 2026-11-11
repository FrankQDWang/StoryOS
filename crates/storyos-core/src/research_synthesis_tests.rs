use super::*;

fn source(id: &str, cited: CitedSource, project_id: &str) -> CitableSource {
    CitableSource {
        id: id.to_owned(),
        cited,
        locator: format!("https://example.org/{id}"),
        owner_user_id: "user-1".to_owned(),
        project_id: project_id.to_owned(),
        run_id: "run-1".to_owned(),
    }
}

fn citable() -> CitableSources {
    CitableSources {
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        run_id: "run-1".to_owned(),
        sources: vec![
            source("s1", CitedSource::Snapshot, "proj-1"),
            source("g1", CitedSource::Gap(SourceGapReason::Failed), "proj-1"),
            source("x1", CitedSource::Snapshot, "proj-2"),
        ],
    }
}

fn claim(text: &str, citations: &[&str]) -> ClaimCandidate {
    ClaimCandidate {
        text: text.to_owned(),
        citations: citations.iter().map(|id| (*id).to_owned()).collect(),
    }
}

#[test]
fn a_synthesis_that_cites_captured_sources_or_only_gaps_is_valid() {
    let summary = "High water was at 10.42. The port log is not available, so the second \
                   question stays open.";

    assert_eq!(
        validate_research_synthesis(
            summary,
            &[
                claim("High water was at 10.42.", &["s1"]),
                claim("The port log could not be read.", &["g1"]),
            ],
            &citable(),
        ),
        Ok(ResearchSynthesis {
            summary: summary.to_owned(),
            claims: vec![
                ResearchClaim {
                    text: "High water was at 10.42.".to_owned(),
                    citations: vec![Citation::Snapshot {
                        source_snapshot_id: "s1".to_owned()
                    }],
                },
                ResearchClaim {
                    text: "The port log could not be read.".to_owned(),
                    citations: vec![Citation::Gap {
                        source_gap_id: "g1".to_owned()
                    }],
                },
            ],
        })
    );
}

#[test]
fn a_synthesis_with_an_uncited_claim_an_uncaptured_citation_or_an_unstorable_text_is_refused() {
    let refusal = |summary: &str, claims: &[ClaimCandidate]| {
        validate_research_synthesis(summary, claims, &citable()).err()
    };

    assert_eq!(
        [
            refusal("Tides.", &[]),
            refusal(" ", &[claim("High water.", &["s1"])]),
            refusal(
                "Tides.",
                &[claim("High water.", &["s1"]), claim("", &["s1"])]
            ),
            refusal(
                "Tides.",
                &[claim("High water.", &["s1"]), claim("Low water.", &[])]
            ),
            refusal("Tides.", &[claim("High water.", &["s1", "s9"])]),
            refusal("Tides.", &[claim("High water.", &["x1"])]),
            refusal("Tides.\0", &[claim("High water.", &["s1"])]),
            refusal("Tides.", &[claim("High\0water.", &["s1"])]),
        ],
        [
            Some(SynthesisRefusal::NoClaim),
            Some(SynthesisRefusal::EmptyText),
            Some(SynthesisRefusal::EmptyText),
            Some(SynthesisRefusal::UncitedClaim),
            Some(SynthesisRefusal::UncapturedCitation),
            Some(SynthesisRefusal::OtherScopeCitation),
            Some(SynthesisRefusal::UnstorableText),
            Some(SynthesisRefusal::UnstorableText),
        ]
    );
}

#[test]
fn the_citable_tag_escapes_each_value_so_that_no_locator_closes_it() {
    let sources = [
        CitableSource {
            locator: "https://example.org/a\"/><x".to_owned(),
            ..source("s1", CitedSource::Snapshot, "proj-1")
        },
        source(
            "g1",
            CitedSource::Gap(SourceGapReason::Unattributed),
            "proj-1",
        ),
    ];

    assert_eq!(
        (
            citable_sources_tag("call_01", &sources),
            citable_sources_tag("call_01", &[])
        ),
        (
            "<tool_sources source=\"call_01\"><source id=\"s1\" \
             locator=\"https://example.org/a&quot;/&gt;&lt;x\"/><gap id=\"g1\" \
             locator=\"https://example.org/g1\" reason=\"unattributed\"/></tool_sources>"
                .to_owned(),
            String::new()
        )
    );
}
