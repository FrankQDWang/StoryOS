/// ADR 0103 section 3A: the script gives at most 5 calls, one more than the Subrun Requests of one
/// Run. A larger count, or a count that is not a number, does not get the Subrun script.
#[test]
fn the_script_gives_at_most_five_calls() {
    let counts = ["", " 1", " 5", " 6", " 18446744073709551616", " x", "x"]
        .map(|suffix| super::call_count(&format!("SCRIPT:start_subrun{suffix}")));
    assert_eq!(
        counts,
        [
            Some(None),
            Some(Some(1)),
            Some(Some(5)),
            None,
            None,
            None,
            None
        ]
    );
}
