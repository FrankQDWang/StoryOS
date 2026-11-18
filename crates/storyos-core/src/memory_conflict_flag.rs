//! The conflict flag of a consolidation pass (ADR 0094 section 7): the operation `flag_conflict`
//! names one current entry of the author, a short reason, and one or more observations. The Host
//! does not change the entry. It writes one flag version with the same values as the version
//! before it, and with the reason and the source of the flag.

use crate::{
    AdmittedOperation, MEMORY_DESCRIPTION_MAX_CHARS, MemoryEntryOrigin, MemoryObservation,
    MemoryOperationCandidate, MemoryOperationRefusal, MemorySourceReference, MemoryVersionWrite,
    MemoryWriteAction, MemoryWriteRefusal, ObservationCandidate, ObservationRefusal, PassConflict,
    PassMemoryEntry, PassObservation,
};

/// The reason of a flag as one line with no space at its ends, or `None` when it is empty or has
/// a control character. A line break at an end is a control character too.
fn reason_line(given: &str) -> Option<&str> {
    let reason = given.trim();
    match reason.is_empty() || given.chars().any(char::is_control) {
        true => None,
        false => Some(reason),
    }
}

/// The validation of one conflict item of a capture at the position `position`: an entry, a
/// reason of one line, and no other value. The entry is one of `author_entries`, and the reason
/// obeys the limit of a description.
pub(crate) fn capture_conflict(
    item: &ObservationCandidate,
    position: usize,
    author_entries: &[u64],
) -> Result<MemoryObservation, ObservationRefusal> {
    let fact = [&item.entry_type, &item.title, &item.text];
    let reason = item.reason.as_deref().and_then(reason_line);
    let (Some(entry), Some(reason)) = (item.entry, reason) else {
        return Err(ObservationRefusal::Invalid { position });
    };
    if fact.iter().any(|value| value.is_some()) {
        return Err(ObservationRefusal::Invalid { position });
    }
    let entry = u64::try_from(entry)
        .ok()
        .filter(|entry| author_entries.contains(entry))
        .ok_or(ObservationRefusal::ConflictEntryUnavailable { position })?;
    if reason.chars().count() > MEMORY_DESCRIPTION_MAX_CHARS {
        return Err(ObservationRefusal::ReasonTooLong { position });
    }
    Ok(MemoryObservation::Conflict {
        entry,
        reason: reason.to_owned(),
    })
}

/// The flag of a flag version: a short reason, and the Project Conversation or the Chapter of the
/// first observation of the operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryConflictFlag {
    pub reason: String,
    pub source: MemorySourceReference,
}

/// The decision of one `flag_conflict` operation after the checks of the list. The operation has
/// an entry, a reason, and one or more observations, and no other value. The entry must be current
/// and must have the origin `author_request`. The reason is one line, and it obeys the limit of a
/// description.
pub(crate) fn decide_flag(
    candidate: &MemoryOperationCandidate,
    current: &[PassMemoryEntry],
    observations: &[PassObservation],
) -> Result<AdmittedOperation, MemoryOperationRefusal> {
    let invalid = MemoryOperationRefusal::Write(MemoryWriteRefusal::Invalid);
    let values = [
        &candidate.entry_type,
        &candidate.title,
        &candidate.description,
        &candidate.text,
    ];
    let reason = candidate
        .reason
        .as_deref()
        .map(|given| reason_line(given).unwrap_or(""));
    let source = candidate
        .observations
        .first()
        .and_then(|number| usize::try_from(*number).ok())
        .and_then(|number| number.checked_sub(1))
        .and_then(|index| observations.get(index));
    let (Some(number), Some(reason), Some(source)) = (candidate.entry, reason, source) else {
        return Err(invalid);
    };
    if candidate.entries.is_some()
        || values.iter().any(|value| value.is_some())
        || reason.is_empty()
    {
        return Err(invalid);
    }
    let entry = current
        .iter()
        .find(|entry| i64::try_from(entry.entry.number).is_ok_and(|current| current == number))
        .ok_or(MemoryOperationRefusal::Write(MemoryWriteRefusal::NotFound))?;
    if entry.origin != MemoryEntryOrigin::AuthorRequest {
        return Err(MemoryOperationRefusal::FlagNeedsAuthorEntry);
    }
    if reason.chars().count() > MEMORY_DESCRIPTION_MAX_CHARS {
        return Err(MemoryOperationRefusal::Write(MemoryWriteRefusal::TooLong));
    }
    let flag = MemoryConflictFlag {
        reason: reason.to_owned(),
        source: source.source.clone(),
    };
    Ok(flag_version(entry, flag))
}

/// The flag version of `entry` with `flag`: the values of its current version and the next
/// version number.
fn flag_version(entry: &PassMemoryEntry, flag: MemoryConflictFlag) -> AdmittedOperation {
    let last = &entry.entry;
    AdmittedOperation {
        version: MemoryVersionWrite {
            action: MemoryWriteAction::Change,
            memory_entry_id: Some(last.memory_entry_id.clone()),
            number: last.number,
            version: last.version + 1,
            state: crate::MemoryEntryState::Current,
            entry_type: last.entry_type,
            title: last.title.clone(),
            description: last.description.clone(),
            text: last.text.clone(),
            flag: Some(flag),
        },
        origin: entry.origin,
        sources: Vec::new(),
        merged: Vec::new(),
    }
}

/// The flags that the Host writes from the conflict observations of a pass, after the operations
/// of the model (ADR 0094 section 7). `current` are the current entries after these operations.
/// A conflict whose entry is not a current entry of the author gives no flag. Each flag counts
/// for the flags after it.
pub(crate) fn host_flags(
    conflicts: &[PassConflict],
    current: &[PassMemoryEntry],
) -> Vec<AdmittedOperation> {
    let mut current = current.to_vec();
    let mut flags = Vec::new();
    for conflict in conflicts {
        let Some(entry) = current.iter_mut().find(|entry| {
            entry.entry.number == conflict.entry && entry.origin == MemoryEntryOrigin::AuthorRequest
        }) else {
            continue;
        };
        let flag = MemoryConflictFlag {
            reason: conflict.reason.clone(),
            source: conflict.source.clone(),
        };
        let admitted = flag_version(entry, flag);
        entry.entry.version = admitted.version.version;
        flags.push(admitted);
    }
    flags
}
