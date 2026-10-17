use super::{
    ExactRevisionTexts, ReplacementSpan, RevisionComparison, RevisionComparisonAccess,
    inspect_revision_comparison,
};

fn span(
    base_from: u32,
    base_to: u32,
    candidate_from: u32,
    candidate_to: u32,
    base_text: &str,
    candidate_text: &str,
) -> ReplacementSpan {
    ReplacementSpan {
        base_from,
        base_to,
        candidate_from,
        candidate_to,
        base_text: base_text.to_owned(),
        candidate_text: candidate_text.to_owned(),
    }
}

fn inspect(base: &str, candidate: &str) -> Option<RevisionComparison> {
    inspect_revision_comparison(
        RevisionComparisonAccess::SameScope,
        &ExactRevisionTexts {
            base_revision_id: "base-revision".to_owned(),
            candidate_revision_id: "candidate-revision".to_owned(),
            operation_id: "operation".to_owned(),
            base_text: base.to_owned(),
            candidate_text: candidate.to_owned(),
        },
    )
}

fn expect(spans: Vec<ReplacementSpan>) -> Option<RevisionComparison> {
    Some(RevisionComparison {
        base_revision_id: "base-revision".to_owned(),
        candidate_revision_id: "candidate-revision".to_owned(),
        operation_id: "operation".to_owned(),
        spans,
    })
}

#[test]
fn comparison_binds_exact_revisions_and_normalizes_adjacent_fragments() {
    let texts = ExactRevisionTexts {
        base_revision_id: "authority-revision".to_owned(),
        candidate_revision_id: "candidate-revision-2".to_owned(),
        operation_id: "stable-operation".to_owned(),
        base_text: "The cat sat.".to_owned(),
        candidate_text: "The dog sat.".to_owned(),
    };
    let before = texts.clone();
    assert_eq!(
        inspect_revision_comparison(RevisionComparisonAccess::SameScope, &texts),
        Some(RevisionComparison {
            base_revision_id: "authority-revision".to_owned(),
            candidate_revision_id: "candidate-revision-2".to_owned(),
            operation_id: "stable-operation".to_owned(),
            spans: vec![span(4, 7, 4, 7, "cat", "dog")],
        })
    );
    assert_eq!(texts, before);
    assert_eq!(
        inspect("abcde", "axcye"),
        expect(vec![span(1, 4, 1, 4, "bcd", "xcy")])
    );
    assert_eq!(
        inspect("ab HELLO cd", "ax HELLO cy"),
        expect(vec![
            span(1, 2, 1, 2, "b", "x"),
            span(10, 11, 10, 11, "d", "y")
        ])
    );
    assert_eq!(inspect("abc", "abc"), expect(Vec::new()));
    assert_eq!(
        inspect("", "Guard"),
        expect(vec![span(0, 0, 0, 5, "", "Guard")])
    );
    assert_eq!(
        inspect("a😀b", "a😀c"),
        expect(vec![span(3, 4, 3, 4, "b", "c")])
    );
}

fn replace_scalar(text: &str, index: usize) -> String {
    let mut scalars: Vec<char> = text.chars().collect();
    scalars[index] = 'X';
    scalars.into_iter().collect()
}

#[test]
fn comparison_keeps_spans_for_long_texts_and_the_trace_limit_fallback() {
    let long: String = "abcdefghij".chars().cycle().take(/*n*/ 32_000).collect();
    assert_eq!(inspect(&long, &long), expect(Vec::new()));

    assert_eq!(
        inspect(&long, &replace_scalar(&long, 16_000)),
        expect(vec![span(16_000, 16_001, 16_000, 16_001, "a", "X")])
    );

    let astral = "😀a".repeat(/*n*/ 16_000);
    assert_eq!(
        inspect(&astral, &replace_scalar(&astral, 16_001)),
        expect(vec![span(24_002, 24_003, 24_002, 24_003, "a", "X")])
    );

    let deleted = "b".repeat(/*n*/ 400);
    let inserted = "c".repeat(/*n*/ 400);
    assert_eq!(
        inspect(
            &format!("{}{deleted}", "a".repeat(9_000)),
            &format!("{}{inserted}", "a".repeat(9_000))
        ),
        expect(vec![span(9_000, 9_400, 9_000, 9_400, &deleted, &inserted)])
    );
    assert_eq!(
        inspect(
            &format!("{}{deleted}", "😀".repeat(4_000)),
            &format!("{}{inserted}", "😀".repeat(4_000))
        ),
        expect(vec![span(8_000, 8_400, 8_000, 8_400, &deleted, &inserted)])
    );
}
