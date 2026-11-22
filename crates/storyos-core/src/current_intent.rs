//! The rule of the current result of an intent pass at a Memory revision (ADR 0101 section 4). A
//! result is current only while the two parts of the Character File are the parts that the pass
//! read. The settings page reads the rule at the current Memory revision, and a Run of the author
//! reads it at the Memory revision that the Run pinned.

/// The kind of one version of a Memory Entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryVersionKind<'a> {
    /// A version that changes the entry: not a flag version and not an intent version.
    Change,
    /// A flag version (ADR 0094 section 7). It does not change the text of the entry.
    Flag,
    /// An intent version: the result of the intent pass that read the version `entry_version` of
    /// the part of the background work and the version `author` of the part of the author, when
    /// the pass read one.
    Intent {
        entry_version: u64,
        author: Option<(&'a str, u64)>,
    },
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

/// The facts of one entry at a Memory revision: its newest version, the version of its last
/// change, and its newest intent version.
struct EntryAt<'a> {
    latest: MemoryVersionFacts<'a>,
    change: Option<MemoryVersionFacts<'a>>,
    intent: Option<MemoryVersionFacts<'a>>,
}

impl EntryAt<'_> {
    fn character(&self) -> bool {
        self.latest.state == "current" && self.latest.entry_type == "character"
    }

    fn change_version(&self) -> Option<u64> {
        self.change.map(|change| change.version)
    }
}

/// The ID and the intent version of each character entry of the background work that has a current
/// result at `memory_revision`, in the order of `versions`. `versions` has the versions of the
/// entries of one Project. A version after `memory_revision` is not a part of the Memory.
///
/// The result of the newest intent version of an entry is current when the entry is a current
/// character entry of the background work, the pass read the version of the last change of the
/// entry, and the part of the author is the part that the pass read: the same entry at the version
/// of its last change, or no current character entry of the author with the title when the pass
/// read none. A flag version changes no part, so it does not change the result.
pub fn current_intents<'a>(
    versions: &[MemoryVersionFacts<'a>],
    memory_revision: u64,
) -> Vec<(&'a str, u64)> {
    let mut entries: Vec<EntryAt<'a>> = Vec::new();
    for version in versions
        .iter()
        .filter(|v| v.memory_revision <= memory_revision)
    {
        let at = match entries
            .iter_mut()
            .find(|entry| entry.latest.memory_entry_id == version.memory_entry_id)
        {
            Some(at) => at,
            None => {
                entries.push(EntryAt {
                    latest: *version,
                    change: None,
                    intent: None,
                });
                entries.last_mut().expect("the entry was pushed")
            }
        };
        let newer = |kept: Option<MemoryVersionFacts<'a>>| {
            kept.is_none_or(|kept| kept.memory_revision < version.memory_revision)
        };
        if at.latest.memory_revision < version.memory_revision {
            at.latest = *version;
        }
        match version.kind {
            MemoryVersionKind::Change if newer(at.change) => at.change = Some(*version),
            MemoryVersionKind::Intent { .. } if newer(at.intent) => at.intent = Some(*version),
            _ => {}
        }
    }
    let author_part = |title: &str| {
        (entries.iter())
            .find(|entry| entry.character() && entry.latest.author && entry.latest.title == title)
    };
    (entries.iter())
        .filter(|entry| entry.character() && !entry.latest.author)
        .filter_map(|entry| {
            let intent = entry.intent?;
            let MemoryVersionKind::Intent {
                entry_version,
                author,
            } = intent.kind
            else {
                return None;
            };
            let author_now = author_part(entry.latest.title);
            let author_read = match (author, author_now) {
                (None, None) => true,
                (Some((id, version)), Some(now)) => {
                    now.latest.memory_entry_id == id && now.change_version() == Some(version)
                }
                _ => false,
            };
            (entry.change_version() == Some(entry_version) && author_read)
                .then_some((intent.memory_entry_id, intent.version))
        })
        .collect()
}

#[cfg(test)]
#[path = "current_intent_tests.rs"]
mod tests;
