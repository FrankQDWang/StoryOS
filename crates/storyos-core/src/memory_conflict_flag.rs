//! The conflict flag of a consolidation pass (ADR 0094 section 7): the operation `flag_conflict`
//! names one current entry of the author, a short reason, and one or more observations. The Host
//! does not change the entry. It writes one flag version with the same values as the version
//! before it, and with the reason and the source of the flag.

use crate::{
    AdmittedOperation, MEMORY_DESCRIPTION_MAX_CHARS, MemoryEntryOrigin, MemoryOperationCandidate,
    MemoryOperationRefusal, MemorySourceReference, MemoryVersionWrite, MemoryWriteAction,
    MemoryWriteRefusal, PassMemoryEntry, PassObservation,
};

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
    // A control character at an end is a line break too, so the check reads the given reason.
    let given = candidate.reason.as_deref();
    let reason = given.map(str::trim);
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
        || given.is_some_and(|given| given.chars().any(char::is_control))
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
    let last = &entry.entry;
    Ok(AdmittedOperation {
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
            flag: Some(MemoryConflictFlag {
                reason: reason.to_owned(),
                source: source.source.clone(),
            }),
        },
        origin: entry.origin,
        sources: Vec::new(),
        merged: Vec::new(),
    })
}
