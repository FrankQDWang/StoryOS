use std::cell::Cell;

use axum::http::{HeaderMap, header};
use url::{Origin, SyntaxViolation, Url};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestOriginPolicy {
    SensitiveSafeReadWithRefererFallback,
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "Issue 119 excludes a state-changing route")
    )]
    StateChanging,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TupleOrigin(Origin);

impl TupleOrigin {
    pub(super) fn from_allowed_origin(raw: &str) -> Option<Self> {
        Self::parse(raw, UrlInput::SerializedOrigin)
    }

    fn parse(raw: &str, input: UrlInput) -> Option<Self> {
        let saw_syntax_violation = Cell::new(false);
        let report_violation = |_: SyntaxViolation| saw_syntax_violation.set(true);
        let parsed = Url::options()
            .syntax_violation_callback(Some(&report_violation))
            .parse(raw)
            .ok()?;
        let has_serialized_origin_shape = raw.split_once("://").is_some_and(|(_, authority)| {
            !authority.is_empty()
                && !authority.ends_with(':')
                && !authority.contains(['/', '?', '#'])
        });

        if saw_syntax_violation.get()
            || !matches!(parsed.scheme(), "http" | "https")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
            || matches!(input, UrlInput::SerializedOrigin) && !has_serialized_origin_shape
        {
            return None;
        }

        match parsed.origin() {
            origin @ Origin::Tuple(..) => Some(Self(origin)),
            Origin::Opaque(_) => None,
        }
    }
}

#[derive(Clone, Copy)]
enum UrlInput {
    SerializedOrigin,
    Referer,
}

pub(super) fn request_origin(
    headers: &HeaderMap,
    policy: RequestOriginPolicy,
) -> Option<TupleOrigin> {
    let mut origin_headers = headers.get_all(header::ORIGIN).iter();
    if let Some(origin) = origin_headers.next() {
        if origin_headers.next().is_some() {
            return None;
        }
        return TupleOrigin::parse(origin.to_str().ok()?, UrlInput::SerializedOrigin);
    }

    match policy {
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback => {
            let mut referer_headers = headers.get_all(header::REFERER).iter();
            let referer = referer_headers.next()?;
            if referer_headers.next().is_some() {
                return None;
            }
            TupleOrigin::parse(referer.to_str().ok()?, UrlInput::Referer)
        }
        RequestOriginPolicy::StateChanging => None,
    }
}

#[cfg(test)]
#[path = "request_origin_tests.rs"]
mod tests;
