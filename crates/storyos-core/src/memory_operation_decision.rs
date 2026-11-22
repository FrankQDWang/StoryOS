//! The validation of the operations of one `apply_memory_operations` call in list order (ADR 0039,
//! ADR 0094 sections 6 and 8). It checks the shape of each action and the list. It applies the
//! write rule of S7-D1a2 and the delete marks, and it gives the versions of a merge. A conflict
//! flag has its own module.

use crate::{
    AdmittedOperation, CurrentMemoryEntry, MEMORY_DESCRIPTION_MAX_CHARS, MemoryDeleteMark,
    MemoryEntryOrigin, MemoryEntryType, MemoryOperationAction, MemoryOperationCandidate,
    MemoryOperationRefusal, MemoryWrite, MemoryWriteAction, MemoryWriteRefusal, MemoryWriter,
    PassMemoryEntry, PassObservation,
};

/// The write of `candidate`, or the refusal of its shape. Create needs a type, update and merge
/// need no type, and delete needs a reason and no values. An update needs one or more of the
/// title, the description, and the text, and a null value keeps the current value of the entry. A
/// merge needs the description and the text, and a null title keeps the title of its first entry.
/// Only a merge has entries, and the write of a merge is the change of its first entry. Only a
/// delete has a reason, and the reason has the limit of a description (#1696).
fn memory_write(candidate: &MemoryOperationCandidate) -> Result<MemoryWrite, MemoryWriteRefusal> {
    let action = match candidate.action {
        MemoryOperationAction::Create => MemoryWriteAction::Create,
        MemoryOperationAction::Update
        | MemoryOperationAction::Merge
        | MemoryOperationAction::FlagConflict => MemoryWriteAction::Change,
        MemoryOperationAction::Delete => MemoryWriteAction::Delete,
    };
    let given = [&candidate.title, &candidate.description, &candidate.text];
    let merge = candidate.action == MemoryOperationAction::Merge;
    let shape = candidate.entries.is_some() == merge
        && match candidate.action {
            MemoryOperationAction::Create => candidate.reason.is_none(),
            MemoryOperationAction::Update => {
                candidate.entry_type.is_none()
                    && candidate.reason.is_none()
                    && given.iter().any(|value| value.is_some())
            }
            MemoryOperationAction::Merge => {
                candidate.entry.is_none()
                    && candidate.entry_type.is_none()
                    && candidate.reason.is_none()
                    && candidate.description.is_some()
                    && candidate.text.is_some()
            }
            // A flag is not a write (`memory_conflict_flag`).
            MemoryOperationAction::FlagConflict => false,
            MemoryOperationAction::Delete => {
                candidate.entry_type.is_none()
                    && candidate.reason.as_deref().is_some_and(|reason| {
                        !reason.trim().is_empty() && !reason.chars().any(char::is_control)
                    })
            }
        };
    let long_reason = (candidate.reason.as_deref())
        .is_some_and(|reason| reason.chars().count() > MEMORY_DESCRIPTION_MAX_CHARS);
    if shape && long_reason {
        return Err(MemoryWriteRefusal::TooLong);
    }
    let entry_type = match candidate.entry_type.as_deref() {
        Some(token) => Some(MemoryEntryType::parse(token).ok_or(MemoryWriteRefusal::Invalid)?),
        None => None,
    };
    match shape {
        true => Ok(MemoryWrite {
            action,
            entry: match merge {
                true => candidate
                    .entries
                    .as_ref()
                    .and_then(|entries| entries.first().copied()),
                false => candidate.entry,
            },
            entry_type,
            title: candidate.title.clone(),
            description: candidate.description.clone(),
            text: candidate.text.clone(),
        }),
        false => Err(MemoryWriteRefusal::Invalid),
    }
}

/// The numbers of the entries that `candidate` names: the entry of an update, a delete, or a flag,
/// and each entry of a merge. A create names no entry.
fn named_entries(candidate: &MemoryOperationCandidate) -> Vec<u64> {
    let numbers = match candidate.action {
        MemoryOperationAction::Create => &[][..],
        MemoryOperationAction::Update
        | MemoryOperationAction::Delete
        | MemoryOperationAction::FlagConflict => candidate.entry.as_slice(),
        MemoryOperationAction::Merge => candidate.entries.as_deref().unwrap_or_default(),
    };
    numbers
        .iter()
        .filter_map(|number| u64::try_from(*number).ok())
        .collect()
}

/// Validates each operation of one call in list order (ADR 0039, ADR 0094 sections 6 and 8).
/// `entries` are the current entries at the pinned Memory revision, `observations` are the
/// observations of the input, from number 1, and `marks` are the delete marks of the author. Each
/// admitted operation counts for the operations after it, and a refused operation does not stop
/// them. The second value is the current entries after the admitted operations.
pub(crate) fn decide_with_entries(
    candidates: &[MemoryOperationCandidate],
    entries: &[PassMemoryEntry],
    next_entry_number: u64,
    observations: &[PassObservation],
    marks: &[MemoryDeleteMark],
) -> (
    Vec<Result<AdmittedOperation, MemoryOperationRefusal>>,
    Vec<PassMemoryEntry>,
) {
    let mut current = entries.to_vec();
    let mut next_entry_number = next_entry_number;
    let mut named = Vec::new();
    let mut decisions = Vec::new();
    for candidate in candidates {
        let facts = (current.as_slice(), next_entry_number, observations, marks);
        let decision = decide_one(candidate, facts, &named);
        // An update, a merge, a delete, or a flag names its entries also when the Host refuses it.
        match (&decision, candidate.action) {
            (Ok(admitted), MemoryOperationAction::Create) => named.push(admitted.version.number),
            (Err(_), MemoryOperationAction::Create) => {}
            (_, _) => named.extend(named_entries(candidate)),
        }
        if let Ok(admitted) = &decision {
            let number = admitted.version.number;
            current.retain(|entry| {
                entry.entry.number != number
                    && !admitted
                        .merged
                        .iter()
                        .any(|m| m.number == entry.entry.number)
            });
            if admitted.version.action == MemoryWriteAction::Create {
                next_entry_number += 1;
            }
            if admitted.version.action != MemoryWriteAction::Delete {
                current.push(PassMemoryEntry {
                    entry: CurrentMemoryEntry {
                        memory_entry_id: admitted
                            .version
                            .memory_entry_id
                            .clone()
                            .unwrap_or_default(),
                        number,
                        version: admitted.version.version,
                        entry_type: admitted.version.entry_type,
                        title: admitted.version.title.clone(),
                        description: admitted.version.description.clone(),
                        text: admitted.version.text.clone(),
                        // A flag version has its flag, and each other written version removes it.
                        flagged: admitted.version.flag.is_some(),
                        author_part: admitted.version.entry_type == MemoryEntryType::Character
                            && admitted.origin == MemoryEntryOrigin::AuthorRequest,
                    },
                    origin: admitted.origin,
                });
            }
        }
        decisions.push(decision);
    }
    (decisions, current)
}

/// The facts of one decision: the current entries, the next entry number, the observations of
/// the input, and the delete marks.
type DecisionFacts<'a> = (
    &'a [PassMemoryEntry],
    u64,
    &'a [PassObservation],
    &'a [MemoryDeleteMark],
);

/// The checks of one operation. The checks of the list come before the write rule of S7-D1a2, and
/// the check of the delete marks comes after it.
fn decide_one(
    candidate: &MemoryOperationCandidate,
    (current, next_entry_number, observations, marks): DecisionFacts<'_>,
    named: &[u64],
) -> Result<AdmittedOperation, MemoryOperationRefusal> {
    let create = candidate.action == MemoryOperationAction::Create;
    let origin_of = |number: &i64| {
        usize::try_from(*number)
            .ok()
            .and_then(|number| number.checked_sub(1))
            .and_then(|index| observations.get(index))
    };
    let first_origin = candidate
        .observations
        .first()
        .and_then(origin_of)
        .map(PassObservation::origin);
    if !candidate
        .observations
        .iter()
        .all(|number| origin_of(number).is_some())
    {
        return Err(MemoryOperationRefusal::ObservationUnavailable);
    }
    let targets = named_entries(candidate);
    if targets.iter().any(|number| named.contains(number)) {
        return Err(MemoryOperationRefusal::EntryNamedTwice);
    }
    if candidate.action == MemoryOperationAction::FlagConflict {
        return crate::memory_conflict_flag::decide_flag(candidate, current, observations);
    }
    // A merge has two or more entries, each one time and each a number of an entry.
    let merge = candidate.action == MemoryOperationAction::Merge;
    let given = candidate.entries.as_ref().map_or(0, Vec::len);
    let distinct = targets
        .iter()
        .enumerate()
        .all(|(index, number)| !targets[..index].contains(number));
    if merge && (targets.len() < 2 || targets.len() != given || !distinct) {
        return Err(MemoryOperationRefusal::Write(MemoryWriteRefusal::Invalid));
    }
    let found: Vec<&PassMemoryEntry> = targets
        .iter()
        .filter_map(|number| current.iter().find(|e| e.entry.number == *number))
        .collect();
    let origin = match (create, found.first()) {
        (true, _) => first_origin.ok_or(MemoryOperationRefusal::CreateWithoutSource)?,
        (false, None) => {
            return Err(MemoryOperationRefusal::Write(MemoryWriteRefusal::NotFound));
        }
        (false, Some(_)) if found.len() < targets.len() => {
            return Err(MemoryOperationRefusal::Write(MemoryWriteRefusal::NotFound));
        }
        (false, Some(_))
            if found
                .iter()
                .any(|entry| entry.origin == MemoryEntryOrigin::AuthorRequest) =>
        {
            return Err(MemoryOperationRefusal::AuthorEntry);
        }
        (false, Some(entry)) => entry.origin,
    };
    if found
        .iter()
        .any(|entry| entry.entry.entry_type != found[0].entry.entry_type)
    {
        return Err(MemoryOperationRefusal::MergeTypeMismatch);
    }
    let write = memory_write(candidate).map_err(MemoryOperationRefusal::Write)?;
    // The other entries of a merge are not in the checks of the result, so a merge makes room.
    let others = match merge {
        true => &targets[1..],
        false => &[][..],
    };
    let entries: Vec<CurrentMemoryEntry> = current
        .iter()
        .filter(|e| !others.contains(&e.entry.number))
        .map(|e| e.entry.clone())
        .collect();
    let background = MemoryWriter::Background;
    let version = crate::decide_memory_write(background, &write, &entries, next_entry_number)
        .map_err(MemoryOperationRefusal::Write)?;
    // The mark of the part of the author of a character does not stop the part of the background.
    let marked = marks.iter().any(|mark| {
        !mark.author_part
            && mark.entry_type == version.entry_type
            && mark.title.trim() == version.title.trim()
    });
    if marked && version.action != MemoryWriteAction::Delete {
        return Err(MemoryOperationRefusal::DeletedByAuthor);
    }
    let mut merged = Vec::new();
    for entry in found.iter().filter(|e| others.contains(&e.entry.number)) {
        let delete = MemoryWrite {
            action: MemoryWriteAction::Delete,
            entry: i64::try_from(entry.entry.number).ok(),
            entry_type: None,
            title: None,
            description: None,
            text: None,
        };
        let entries = [entry.entry.clone()];
        merged.push(
            crate::decide_memory_write(background, &delete, &entries, next_entry_number)
                .map_err(MemoryOperationRefusal::Write)?,
        );
    }
    let mut sources = Vec::new();
    for source in candidate.observations.iter().filter_map(origin_of) {
        if !sources.contains(&source.source) {
            sources.push(source.source.clone());
        }
    }
    Ok(AdmittedOperation {
        version,
        origin,
        sources,
        merged,
    })
}
