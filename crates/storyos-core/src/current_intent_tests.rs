use super::{MemoryVersionFacts, MemoryVersionKind, current_intent, current_intents};

const MANUSCRIPT: &str = "manuscript-part";
const AUTHOR: &str = "author-part";

/// One version at the Memory revision `revision` of the entry `id`, a current character entry
/// with the title "Lin Xue". The entry `AUTHOR` has the origin of a request of the author.
fn version(
    id: &'static str,
    number: u64,
    revision: u64,
    kind: MemoryVersionKind<'static>,
) -> MemoryVersionFacts<'static> {
    MemoryVersionFacts {
        memory_entry_id: id,
        version: number,
        memory_revision: revision,
        state: "current",
        entry_type: "character",
        author: id == AUTHOR,
        title: "Lin Xue",
        kind,
    }
}

fn change(id: &'static str, number: u64, revision: u64) -> MemoryVersionFacts<'static> {
    version(id, number, revision, MemoryVersionKind::Change)
}

fn flag(id: &'static str, number: u64, revision: u64) -> MemoryVersionFacts<'static> {
    version(id, number, revision, MemoryVersionKind::Flag)
}

fn deleted(id: &'static str, number: u64, revision: u64) -> MemoryVersionFacts<'static> {
    MemoryVersionFacts {
        state: "deleted",
        ..change(id, number, revision)
    }
}

/// The version `number` of the part of the background work with a result that names the version
/// `author` of the part of the author.
fn intent(number: u64, revision: u64, author: Option<u64>) -> MemoryVersionFacts<'static> {
    let author = author.map(|version| (AUTHOR, version));
    version(
        MANUSCRIPT,
        number,
        revision,
        MemoryVersionKind::Intent { author },
    )
}

/// The version of the result of the two parts at the Memory revision `revision`.
fn result(
    entry: &[MemoryVersionFacts<'static>],
    author: &[MemoryVersionFacts<'static>],
    revision: u64,
) -> Option<u64> {
    current_intent(entry, author, revision).map(|result| result.version)
}

/// The part of the author at the Memory revision 1, and the part of the background work with a
/// result at the Memory revisions 2 and 3.
fn parts() -> (
    Vec<MemoryVersionFacts<'static>>,
    Vec<MemoryVersionFacts<'static>>,
) {
    (
        vec![change(MANUSCRIPT, 1, 2), intent(2, 3, Some(1))],
        vec![change(AUTHOR, 1, 1)],
    )
}

/// The result is current when the three conditions of ADR 0101 section 4 are true.
#[test]
fn the_result_is_current_when_each_condition_is_true() {
    let (entry, author) = parts();
    assert_eq!(result(&entry, &author, 3), Some(2));
    let alone = [change(MANUSCRIPT, 1, 1), intent(2, 2, None)];
    assert_eq!(result(&alone, &[], 2), Some(2));
}

/// Condition 1: the part of the background work is a current character entry.
#[test]
fn a_deleted_part_of_the_background_work_has_no_result() {
    let (mut entry, author) = parts();
    entry.push(deleted(MANUSCRIPT, 3, 4));
    assert_eq!(result(&entry, &author, 4), None);
}

/// Condition 2: the newest version that is not a flag version has an intent.
#[test]
fn a_change_of_the_part_of_the_background_work_ends_the_result() {
    let (mut entry, author) = parts();
    entry.push(change(MANUSCRIPT, 3, 4));
    assert_eq!(result(&entry, &author, 4), None);
}

/// Condition 3: the part of the author that the result names is current at the version that the
/// result names, or the result names no part and the Project has none.
#[test]
fn a_change_a_delete_or_a_new_part_of_the_author_ends_the_result() {
    let (entry, mut author) = parts();
    author.push(change(AUTHOR, 2, 4));
    assert_eq!(result(&entry, &author, 4), None);
    let (entry, mut author) = parts();
    author.push(deleted(AUTHOR, 2, 4));
    assert_eq!(result(&entry, &author, 4), None);
    let alone = [change(MANUSCRIPT, 1, 1), intent(2, 2, None)];
    assert_eq!(result(&alone, &[change(AUTHOR, 1, 3)], 3), None);
}

/// Case (a): a flag version of the part of the background work changes no text.
#[test]
fn a_flag_of_the_part_of_the_background_work_keeps_the_result() {
    let (mut entry, author) = parts();
    entry.push(flag(MANUSCRIPT, 3, 4));
    assert_eq!(result(&entry, &author, 4), Some(2));
}

/// Case (b): a flag version of the part of the author changes no text, so the version of its last
/// change stays the version that the result names.
#[test]
fn a_flag_of_the_part_of_the_author_keeps_the_result() {
    let (entry, mut author) = parts();
    author.push(flag(AUTHOR, 2, 4));
    assert_eq!(result(&entry, &author, 4), Some(2));
}

/// A version after the given Memory revision is not a part of the Memory, so a Run that pinned an
/// earlier Memory revision keeps the result that was current at that revision.
#[test]
fn the_rule_reads_only_the_versions_at_the_given_memory_revision() {
    let (mut entry, mut author) = parts();
    author.push(change(AUTHOR, 2, 4));
    entry.push(intent(3, 5, Some(2)));
    let shown = [2, 3, 4, 5].map(|revision| result(&entry, &author, revision));
    assert_eq!(shown, [None, Some(2), None, Some(3)]);
}

/// The read of a Project gives the result of each character with a current result, and no result
/// for the part of the author.
#[test]
fn the_read_of_a_project_finds_the_part_of_the_author_by_its_title() {
    let (entry, author) = parts();
    let versions = [author, entry].concat();
    assert_eq!(current_intents(&versions, 3), [(MANUSCRIPT, 2)]);
    let other = MemoryVersionFacts {
        title: "Ah Qing",
        ..change(AUTHOR, 2, 4)
    };
    let renamed = [versions, vec![other]].concat();
    assert_eq!(current_intents(&renamed, 4), []);
}
