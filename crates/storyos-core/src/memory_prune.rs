//! The pruning fact of a consolidation pass (ADR 0094 section 8): the mark `unused` of a
//! background entry that no Run of the author read for a long time and that did not change in
//! that time. The Host deletes no entry by age. The mark is a fact for the model, and a delete is
//! an operation of the model that the Host validates.

use crate::MemoryEntryOrigin;

/// The days with no read and no change after which a background entry gets the mark `unused`
/// (ADR 0094 section 8). An initial value that S7-D7 can change.
pub const MEMORY_UNUSED_AFTER_DAYS: i64 = 90;

const DAY_MILLISECONDS: i64 = 86_400_000;

/// The milliseconds since 1970-01-01 of a UTC time in the store form `YYYY-MM-DDTHH:MI:SS.MSZ`,
/// or `None` for another text.
fn utc_milliseconds(time: &str) -> Option<i64> {
    let bytes = time.as_bytes();
    let form = bytes.len() == 24
        && [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'.'),
            (23, b'Z'),
        ]
        .iter()
        .all(|(index, byte)| bytes[*index] == *byte);
    if !form {
        return None;
    }
    let field = |start: usize, end: usize| -> Option<i64> {
        let text = time.get(start..end)?;
        text.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| text.parse().ok())?
    };
    let (year, month, day) = (field(0, 4)?, field(5, 7)?, field(8, 10)?);
    let (hour, minute, second, milli) = (
        field(11, 13)?,
        field(14, 16)?,
        field(17, 19)?,
        field(20, 23)?,
    );
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    // The days since 1970-01-01 of a date in the proleptic Gregorian calendar.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000 + milli)
}

/// Decides the mark `unused` of one current entry from the time of its last read by a Run of
/// the author, the time of its last change, and the time now, each in the store form. An entry
/// of the author never gets the mark. A background entry gets it when its last change and its
/// last read, if any, are `MEMORY_UNUSED_AFTER_DAYS` days or more before now. A time in another
/// form gives no mark.
pub fn memory_entry_unused(
    origin: MemoryEntryOrigin,
    last_read: Option<&str>,
    changed_at: &str,
    now: &str,
) -> bool {
    let limit = MEMORY_UNUSED_AFTER_DAYS * DAY_MILLISECONDS;
    let old = |time: &str| {
        utc_milliseconds(time)
            .zip(utc_milliseconds(now))
            .is_some_and(|(time, now)| now - time >= limit)
    };
    origin != MemoryEntryOrigin::AuthorRequest && old(changed_at) && last_read.is_none_or(old)
}

#[cfg(test)]
#[path = "memory_prune_tests.rs"]
mod tests;
