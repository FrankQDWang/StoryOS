//! Values that a Diagnostic Projection may record (ADR 0047).

use std::error::Error;
use std::sync::OnceLock;

/// A safe identifier, a static category, or a counter that a `tracing` field may record.
///
/// The value never derives from author text, a prompt, a payload, or a secret.
pub trait DiagnosticField {
    type Value<'a>: tracing::Value
    where
        Self: 'a;

    fn diagnostic(&self) -> Self::Value<'_>;
}

impl DiagnosticField for &'static str {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        self
    }
}

impl DiagnosticField for u16 {
    type Value<'a> = u16;

    fn diagnostic(&self) -> Self::Value<'_> {
        *self
    }
}

impl DiagnosticField for u64 {
    type Value<'a> = u64;

    fn diagnostic(&self) -> Self::Value<'_> {
        *self
    }
}

impl DiagnosticField for crate::ProjectId {
    type Value<'a> = &'a str;

    fn diagnostic(&self) -> Self::Value<'_> {
        &self.0
    }
}

impl DiagnosticField for i64 {
    type Value<'a> = i64;

    fn diagnostic(&self) -> Self::Value<'_> {
        *self
    }
}

/// An identifier that StoryOS assigned or validated, such as an export or a run ID.
pub struct DiagnosticId<'a>(pub &'a str);

impl DiagnosticField for DiagnosticId<'_> {
    type Value<'b>
        = &'b str
    where
        Self: 'b;

    fn diagnostic(&self) -> Self::Value<'_> {
        self.0
    }
}

static SQL_STATE: OnceLock<SqlStateLookup> = OnceLock::new();

/// Returns the SQLSTATE code of one error, without the error itself or its source.
pub type SqlStateLookup = fn(&(dyn Error + 'static)) -> Option<String>;

/// Sets the SQLSTATE lookup of the persistence adapter. A composition root calls it once.
pub fn register_sql_state(lookup: SqlStateLookup) {
    let _ = SQL_STATE.set(lookup);
}

/// The SQLSTATE code of the first error in a source chain that has one.
pub struct SqlState<'a>(pub &'a (dyn Error + 'static));

impl DiagnosticField for SqlState<'_> {
    type Value<'b>
        = Option<String>
    where
        Self: 'b;

    fn diagnostic(&self) -> Self::Value<'_> {
        let lookup = SQL_STATE.get()?;
        let mut error = Some(self.0);
        while let Some(current) = error {
            if let Some(code) = lookup(current) {
                return Some(code);
            }
            error = current.source();
        }
        None
    }
}

impl DiagnosticField for crate::ProjectReadError {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        "unavailable"
    }
}

impl DiagnosticField for crate::CompleteReadableExportError {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::StaleFence => "stale_fence",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

impl DiagnosticField for crate::CompleteArchiveExportError {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::StaleFence => "stale_fence",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

impl DiagnosticField for crate::CompleteAgentRunError {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::StaleFence => "stale_fence",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

impl DiagnosticField for crate::CompleteReadableExport {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::SettledReady => "settled_ready",
            Self::SettledFailed => "settled_failed",
            Self::AlreadySettled => "already_settled",
        }
    }
}

impl DiagnosticField for crate::CompleteArchiveExport {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::SettledReady => "settled_ready",
            Self::SettledFailed => "settled_failed",
            Self::AlreadySettled => "already_settled",
        }
    }
}

impl DiagnosticField for crate::CompleteAgentRun {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::Settled => "settled",
            Self::AlreadySettled => "already_settled",
        }
    }
}

impl DiagnosticField for crate::ProjectCommandError {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::BindingConflict => "binding_conflict",
            Self::HistoricalAcknowledgementUnavailable => "historical_acknowledgement_unavailable",
            Self::InvalidChallenge => "invalid_challenge",
            Self::MissingProject => "missing_project",
            Self::WriterIneligible => "writer_ineligible",
            Self::Unavailable(_) => "unavailable",
        }
    }
}
