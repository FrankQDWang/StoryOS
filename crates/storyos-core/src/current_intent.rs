//! The rule of the current result of an intent pass at a Memory revision (ADR 0101 section 4). A
//! result is current only while the two parts of the Character File are the parts that the pass
//! read. The settings page reads the rule at the current Memory revision, and a Run of the author
//! reads it at the Memory revision that the Run pinned.

/// The kind of one version of a Memory Entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryVersionKind<'a> {
    /// A version that changes the entry: not a flag version and not a version with an intent.
    Change,
    /// A flag version (ADR 0094 section 7). It changes no text.
    Flag,
    /// A version with an intent. `author` is the entry of the author and the version of it that
    /// the result names, when the pass read one.
    Intent { author: Option<(&'a str, u64)> },
}

/// The facts of one version of a Memory Entry of a Project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryVersionFacts<'a> {
    pub memory_entry_id: &'a str,
    pub version: u64,
    pub memory_revision: u64,
    pub state: &'a str,
    pub entry_type: &'a str,
    /// The version has the origin of a request of the author.
    pub author: bool,
    pub title: &'a str,
    pub kind: MemoryVersionKind<'a>,
}

/// The versions of one entry at `memory_revision`: a version after that revision is not a part of
/// the Memory.
fn at<'v, 'a>(
    versions: &'v [MemoryVersionFacts<'a>],
    memory_revision: u64,
) -> impl Iterator<Item = &'v MemoryVersionFacts<'a>> {
    (versions.iter()).filter(move |version| version.memory_revision <= memory_revision)
}

fn newest<'v, 'a>(
    versions: impl Iterator<Item = &'v MemoryVersionFacts<'a>>,
) -> Option<&'v MemoryVersionFacts<'a>>
where
    'a: 'v,
{
    versions.max_by_key(|version| version.memory_revision)
}

/// The newest version of the entry at `memory_revision`, when it is a current character entry.
fn current_character<'v, 'a>(
    versions: &'v [MemoryVersionFacts<'a>],
    memory_revision: u64,
) -> Option<&'v MemoryVersionFacts<'a>> {
    newest(at(versions, memory_revision))
        .filter(|latest| latest.state == "current" && latest.entry_type == "character")
}

/// The version with the current result of one character at `memory_revision`, or `None`.
/// `entry` has the versions of the part of the background work. `author` has the versions of the
/// part of the author with the same title, or no version when the Project has no such part.
///
/// The result is current when (1) the part of the background work is a current character entry,
/// (2) its newest version that is not a flag version is a version with an intent, and (3) the part
/// of the author that the result names is current at the version of its last change that the
/// result names, or the result names no part and the Project has no current part of the author. A
/// flag version changes no text, so it does not end a result. The version of the last change is
/// the newest version that is not a flag version and not a version with an intent. The rule reads
/// the state at the revision, and not the history: when the two parts are again the parts that
/// the pass read, the result is current again.
pub(crate) fn current_intent<'v, 'a>(
    entry: &'v [MemoryVersionFacts<'a>],
    author: &[MemoryVersionFacts<'a>],
    memory_revision: u64,
) -> Option<&'v MemoryVersionFacts<'a>> {
    let background = current_character(entry, memory_revision).filter(|latest| !latest.author)?;
    let result = newest(at(entry, memory_revision).filter(|v| v.kind != MemoryVersionKind::Flag))?;
    let MemoryVersionKind::Intent { author: named } = result.kind else {
        return None;
    };
    let part = current_character(author, memory_revision)
        .filter(|latest| latest.author && latest.title == background.title);
    let last_change =
        newest(at(author, memory_revision).filter(|v| v.kind == MemoryVersionKind::Change));
    let read = match (named, part) {
        (None, None) => true,
        (Some((memory_entry_id, version)), Some(part)) => {
            part.memory_entry_id == memory_entry_id
                && last_change.map(|change| change.version) == Some(version)
        }
        _ => false,
    };
    read.then_some(result)
}

/// The ID and the version of the result of each character of the Project with a current result at
/// `memory_revision`, by `current_intent`. `versions` has the versions of the entries of one
/// Project.
pub fn current_intents<'a>(
    versions: &[MemoryVersionFacts<'a>],
    memory_revision: u64,
) -> Vec<(&'a str, u64)> {
    let mut entries: Vec<Vec<MemoryVersionFacts<'a>>> = Vec::new();
    for version in versions {
        match (entries.iter_mut()).find(|entry| entry[0].memory_entry_id == version.memory_entry_id)
        {
            Some(entry) => entry.push(*version),
            None => entries.push(vec![*version]),
        }
    }
    let author = |title: &str| {
        (entries.iter())
            .find(|entry| {
                current_character(entry, memory_revision)
                    .is_some_and(|latest| latest.author && latest.title == title)
            })
            .map_or(&[][..], Vec::as_slice)
    };
    (entries.iter())
        .filter_map(|entry| {
            let title = current_character(entry, memory_revision)?.title;
            current_intent(entry, author(title), memory_revision)
        })
        .map(|result| (result.memory_entry_id, result.version))
        .collect()
}

#[cfg(test)]
#[path = "current_intent_tests.rs"]
mod tests;
