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

fn expected(spans: Vec<ReplacementSpan>) -> RevisionComparison {
    RevisionComparison {
        base_revision_id: "base-revision".to_owned(),
        candidate_revision_id: "candidate-revision".to_owned(),
        operation_id: "operation".to_owned(),
        spans,
    }
}

fn source(base: &str, candidate: &str) -> ExactRevisionTexts {
    ExactRevisionTexts {
        base_revision_id: "base-revision".to_owned(),
        candidate_revision_id: "candidate-revision".to_owned(),
        operation_id: "operation".to_owned(),
        base_text: base.to_owned(),
        candidate_text: candidate.to_owned(),
    }
}

#[test]
fn adjacent_fragmented_matches_become_one_replacement_span() {
    let texts = source("abcde", "axcye");
    let before = texts.clone();
    let comparison = inspect_revision_comparison(RevisionComparisonAccess::SameScope, &texts);
    assert_eq!(texts, before);
    assert_eq!(
        comparison,
        Some(expected(vec![span(1, 4, 1, 4, "bcd", "xcy")]))
    );
}

#[test]
fn a_long_equal_separator_stays_between_two_spans() {
    let comparison = inspect_revision_comparison(
        RevisionComparisonAccess::SameScope,
        &source("ab HELLO cd", "ax HELLO cy"),
    );
    assert_eq!(
        comparison,
        Some(expected(vec![
            span(1, 2, 1, 2, "b", "x"),
            span(10, 11, 10, 11, "d", "y"),
        ]))
    );
}

#[test]
fn identical_revisions_have_no_replacement_span() {
    let comparison =
        inspect_revision_comparison(RevisionComparisonAccess::SameScope, &source("abc", "abc"));
    assert_eq!(comparison, Some(expected(Vec::new())));
}

#[test]
fn comparison_binds_the_supplied_revision_and_operation_identities() {
    let mut texts = source("The cat sat.", "The dog sat.");
    texts.base_revision_id = "authority-revision".to_owned();
    texts.candidate_revision_id = "candidate-revision-2".to_owned();
    texts.operation_id = "stable-operation".to_owned();
    let before = texts.clone();
    let comparison = inspect_revision_comparison(RevisionComparisonAccess::SameScope, &texts);
    assert_eq!(texts, before);
    assert_eq!(
        comparison,
        Some(RevisionComparison {
            base_revision_id: "authority-revision".to_owned(),
            candidate_revision_id: "candidate-revision-2".to_owned(),
            operation_id: "stable-operation".to_owned(),
            spans: vec![span(4, 7, 4, 7, "cat", "dog")],
        })
    );
}

#[test]
fn an_empty_base_is_one_insertion_span() {
    let comparison =
        inspect_revision_comparison(RevisionComparisonAccess::SameScope, &source("", "Guard"));
    assert_eq!(
        comparison,
        Some(expected(vec![span(0, 0, 0, 5, "", "Guard")]))
    );
}

#[test]
fn utf16_offsets_count_a_supplementary_character_as_two_units() {
    let comparison =
        inspect_revision_comparison(RevisionComparisonAccess::SameScope, &source("a😀b", "a😀c"));
    assert_eq!(comparison, Some(expected(vec![span(3, 4, 3, 4, "b", "c")])));
}

#[test]
fn a_long_equal_run_stays_outside_the_two_edge_spans() {
    let middle = "x".repeat(4_000);
    let base = format!("a{middle}b");
    let candidate = format!("c{middle}d");
    let comparison = inspect_revision_comparison(
        RevisionComparisonAccess::SameScope,
        &source(&base, &candidate),
    );
    assert_eq!(
        comparison,
        Some(expected(vec![
            span(0, 1, 0, 1, "a", "c"),
            span(4_001, 4_002, 4_001, 4_002, "b", "d"),
        ]))
    );
}

#[test]
fn a_large_unrelated_pair_is_one_replacement_span() {
    let base = "a".repeat(8_000);
    let candidate = "b".repeat(8_000);
    let comparison = inspect_revision_comparison(
        RevisionComparisonAccess::SameScope,
        &source(&base, &candidate),
    );
    assert_eq!(
        comparison,
        Some(expected(vec![span(0, 8_000, 0, 8_000, &base, &candidate)]))
    );
}

#[test]
fn a_shared_suffix_stays_outside_the_replacement_span() {
    let comparison =
        inspect_revision_comparison(RevisionComparisonAccess::SameScope, &source("xa", "aa"));
    assert_eq!(comparison, Some(expected(vec![span(0, 1, 0, 1, "x", "a")])));
}

#[test]
fn a_wrong_scope_returns_no_comparison() {
    let texts = source("abcde", "axcye");
    let before = texts.clone();
    assert_eq!(
        inspect_revision_comparison(RevisionComparisonAccess::WrongScope, &texts),
        None
    );
    assert_eq!(texts, before);
}
