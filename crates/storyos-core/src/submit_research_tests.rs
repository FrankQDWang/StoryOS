use serde_json::json;

use super::*;

#[test]
fn the_intake_takes_a_summary_and_claims_and_refuses_a_text_that_the_store_cannot_keep() {
    let claim = |text: &str| json!({"text": text, "citations": ["s1", "g1"]});
    let cases = [
        (
            json!({"summary": "The tides.", "claims": [claim("Two high waters.")]}),
            Ok(ToolIntake::SubmitResearch {
                summary: "The tides.".to_owned(),
                claims: vec![ClaimCandidate {
                    text: "Two high waters.".to_owned(),
                    citations: vec!["s1".to_owned(), "g1".to_owned()],
                }],
            }),
        ),
        // The validation of ADR 0087 section 3 refuses an empty synthesis, not the intake.
        (
            json!({"summary": " ", "claims": []}),
            Ok(ToolIntake::SubmitResearch {
                summary: " ".to_owned(),
                claims: Vec::new(),
            }),
        ),
        (
            json!({"summary": "The tides.", "claims": [{"text": "One.", "citations": [7]}]}),
            Err(IntakeRefusal::Malformed),
        ),
        (
            json!({"summary": "The tides.", "claims": [], "sources": []}),
            Err(IntakeRefusal::Malformed),
        ),
        (
            json!({"summary": "The\u{0}tides.", "claims": [claim("One.")]}),
            Err(IntakeRefusal::UnstorableText),
        ),
        (
            json!({"summary": "The tides.", "claims": [claim("O\u{0}ne.")]}),
            Err(IntakeRefusal::UnstorableText),
        ),
    ];
    for (arguments, expected) in cases {
        assert_eq!(
            SUBMIT_RESEARCH_TOOL_SPEC.intake(&arguments),
            expected,
            "{arguments}"
        );
    }
}
