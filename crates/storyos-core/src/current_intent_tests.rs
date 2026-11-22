use super::{MemoryVersionFacts, MemoryVersionKind, current_intents};

const MANUSCRIPT: &str = "manuscript-part";
const AUTHOR: &str = "author-part";

/// One version at the Memory revision `revision` of the entry `id`, a current character entry
/// with the title "Lin Xue". The entry `AUTHOR` has the origin of a request of the author.
fn version(
    id: &'static str,
    version: u64,
    revision: u64,
    kind: MemoryVersionKind<'static>,
) -> MemoryVersionFacts<'static> {
    MemoryVersionFacts {
        memory_entry_id: id,
        version,
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

/// The intent version `number` of the part of the background work, from a pass that read its
/// version `read` and the version `author` of the part of the author.
fn intent(
    number: u64,
    revision: u64,
    read: u64,
    author: Option<u64>,
) -> MemoryVersionFacts<'static> {
    let author = author.map(|version| (AUTHOR, version));
    let kind = MemoryVersionKind::Intent {
        entry_version: read,
        author,
    };
    version(MANUSCRIPT, number, revision, kind)
}

/// The two parts of the file at the Memory revisions 1 and 2, and the result of a pass that read
/// both at the Memory revision 3.
fn file_with_result() -> Vec<MemoryVersionFacts<'static>> {
    vec![
        change(AUTHOR, 1, 1),
        change(MANUSCRIPT, 1, 2),
        intent(2, 3, 1, Some(1)),
    ]
}

/// A result is current while the two parts are the parts that the pass read (ADR 0101 section 4).
#[test]
fn the_result_is_current_while_the_pass_read_the_current_parts() {
    assert_eq!(current_intents(&file_with_result(), 3), [(MANUSCRIPT, 2)]);
    let alone = [change(MANUSCRIPT, 1, 1), intent(2, 2, 1, None)];
    assert_eq!(current_intents(&alone, 2), [(MANUSCRIPT, 2)]);
}

/// A change of one of the two parts, a new part of the author, or a delete of a part ends the
/// result (ADR 0101 section 4).
#[test]
fn a_change_of_a_part_ends_the_result() {
    let deleted = |id| MemoryVersionFacts {
        state: "deleted",
        ..change(id, 3, 4)
    };
    let changes = [
        change(MANUSCRIPT, 3, 4),
        change(AUTHOR, 2, 4),
        deleted(MANUSCRIPT),
        deleted(AUTHOR),
    ];
    for later in changes {
        let mut versions = file_with_result();
        versions.push(later);
        assert_eq!(current_intents(&versions, 4), [], "{later:?}");
    }
    let new_author = [
        change(MANUSCRIPT, 1, 1),
        intent(2, 2, 1, None),
        change(AUTHOR, 1, 3),
    ];
    assert_eq!(current_intents(&new_author, 3), []);
}

/// A flag version changes no part, so the result stays current (ADR 0094 section 7).
#[test]
fn a_flag_version_keeps_the_result() {
    for id in [MANUSCRIPT, AUTHOR] {
        let mut versions = file_with_result();
        let number = if id == AUTHOR { 2 } else { 3 };
        versions.push(version(id, number, 4, MemoryVersionKind::Flag));
        assert_eq!(current_intents(&versions, 4), [(MANUSCRIPT, 2)], "{id}");
    }
}

/// When the part of the background work changed between the start of the pass and its result,
/// the result is from the earlier text, and it is not current.
#[test]
fn a_change_during_the_pass_ends_the_result() {
    let versions = [
        change(MANUSCRIPT, 1, 1),
        change(MANUSCRIPT, 2, 2),
        intent(3, 3, 1, None),
    ];
    assert_eq!(current_intents(&versions, 3), []);
}

/// A version after the given Memory revision is not a part of the Memory, so a Run that pinned an
/// earlier Memory revision keeps the result that was current at that revision.
#[test]
fn the_rule_reads_only_the_versions_at_the_given_memory_revision() {
    let mut versions = file_with_result();
    versions.push(change(AUTHOR, 2, 4));
    versions.push(intent(3, 5, 1, Some(2)));
    assert_eq!(current_intents(&versions, 2), []);
    assert_eq!(current_intents(&versions, 3), [(MANUSCRIPT, 2)]);
    assert_eq!(current_intents(&versions, 4), []);
    assert_eq!(current_intents(&versions, 5), [(MANUSCRIPT, 3)]);
}

/// Only a character entry of the background work has a result.
#[test]
fn an_entry_with_no_intent_version_has_no_result() {
    let versions = [change(AUTHOR, 1, 1), change(MANUSCRIPT, 1, 2)];
    assert_eq!(current_intents(&versions, 2), []);
}
