use super::{RangeMapping, map_unchanged_range};

fn unchanged(from: u32, to: u32) -> RangeMapping {
    RangeMapping::Unchanged { from, to }
}

#[test]
fn edge_insertions_keep_or_move_the_range() {
    assert_eq!(
        map_unchanged_range("The cat sat.", "The cats sat.", 4, 7),
        unchanged(4, 7)
    );
    assert_eq!(
        map_unchanged_range("The cat sat.", "The big cat sat.", 4, 7),
        unchanged(8, 11)
    );
    assert_eq!(
        map_unchanged_range("The cat sat.", "The big cats sat.", 4, 7),
        unchanged(8, 11)
    );
}

#[test]
fn a_repeated_scalar_at_an_edge_slides_outside_the_range() {
    assert_eq!(map_unchanged_range("xabc", "xaabc", 1, 4), unchanged(2, 5));
    assert_eq!(map_unchanged_range("abcx", "abccx", 0, 3), unchanged(0, 3));
    assert_eq!(
        map_unchanged_range("go now go", "go  now  go", 3, 6),
        unchanged(4, 7)
    );
    assert_eq!(
        map_unchanged_range("xabcx", "xaabccx", 1, 4),
        unchanged(2, 5)
    );
}

#[test]
fn a_change_inside_the_range_is_changed() {
    assert_eq!(
        map_unchanged_range("The cat sat.", "The coat sat.", 4, 7),
        RangeMapping::Changed
    );
    assert_eq!(
        map_unchanged_range("The cat sat.", "The cot sat.", 4, 7),
        RangeMapping::Changed
    );
    assert_eq!(
        map_unchanged_range("The cat sat.", "The sat.", 4, 7),
        RangeMapping::Changed
    );
    assert_eq!(
        map_unchanged_range("a😀b", "a😀b", 2, 3),
        RangeMapping::Changed
    );
}

#[test]
fn a_long_edge_insertion_is_exact_and_a_large_scattered_change_is_coarse() {
    let inserted = "z".repeat(/*n*/ 1_000);
    assert_eq!(
        map_unchanged_range("xabcy", &format!("xabc{inserted}y"), 1, 4),
        unchanged(1, 4)
    );
    assert_eq!(
        map_unchanged_range("xabcy", &format!("x{inserted}abcy"), 1, 4),
        unchanged(1_001, 1_004)
    );
    let base = format!("<{}>", "a".repeat(500));
    let scattered = format!("<{}>", "ab".repeat(500));
    assert_eq!(
        map_unchanged_range(&base, &scattered, 100, 110),
        RangeMapping::Coarse
    );
}
