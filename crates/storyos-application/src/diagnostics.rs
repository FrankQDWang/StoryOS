//! Values that a Diagnostic Projection may record (ADR 0047).

/// A safe identifier, a static category, or a counter that a `tracing` field may record.
///
/// A type that can hold author text, a prompt, or a secret must not implement this trait.
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
