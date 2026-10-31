use axum::http::{HeaderMap, HeaderValue, header};

use super::*;

const ALLOWED_ORIGIN: &str = "https://example.com";

fn headers_with(name: header::HeaderName, value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(name, value.parse().expect("test header should be valid"));
    headers
}

fn allowed_origin(raw: &str) -> TupleOrigin {
    TupleOrigin::from_allowed_origin(raw).expect("test Origin should be accepted")
}

#[test]
fn tuple_origins_keep_scheme_host_and_non_default_port_distinct() {
    let expected = allowed_origin(ALLOWED_ORIGIN);

    for different in [
        "http://example.com",
        "https://other.example",
        "https://example.com:444",
    ] {
        assert_ne!(expected, allowed_origin(different), "{different}");
    }
}

#[test]
fn valid_origin_is_authoritative_over_referer() {
    let mut headers = headers_with(header::ORIGIN, ALLOWED_ORIGIN);
    headers.insert(
        header::REFERER,
        "https://foreign.example/projects/one"
            .parse()
            .expect("test Referer should be valid"),
    );

    assert_eq!(
        request_origin(
            &headers,
            RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
        ),
        Some(allowed_origin(ALLOWED_ORIGIN)),
    );
}

#[test]
fn empty_repeated_and_non_utf8_referer_values_refuse() {
    let mut repeated = headers_with(header::REFERER, "https://example.com/one");
    repeated.append(
        header::REFERER,
        "https://example.com/two"
            .parse()
            .expect("test Referer should be valid"),
    );
    let empty = headers_with(header::REFERER, "");
    let mut non_utf8 = HeaderMap::new();
    non_utf8.insert(
        header::REFERER,
        HeaderValue::from_bytes(b"https://example.com/\x80")
            .expect("obs-text is a valid HTTP field value"),
    );

    for headers in [&repeated, &empty, &non_utf8] {
        assert_eq!(
            request_origin(
                headers,
                RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
            ),
            None,
        );
    }
}

#[test]
fn every_representable_syntax_violation_class_refuses() {
    for (class, raw) in [
        ("Backslash", "https:\\\\example.com\\story"),
        ("C0SpaceIgnored", " https://example.com/story"),
        ("EmbeddedCredentials", "https://user:pass@example.com/story"),
        ("ExpectedDoubleSlash", "https:example.com/story"),
        ("ExpectedFileDoubleSlash", "file:example/story"),
        ("FileWithHostAndWindowsDrive", "file://host/C:/story"),
        ("NonUrlCodePoint", "https://example.com/{story"),
        ("NullInFragment", "https://example.com/#\0"),
        ("PercentDecode", "https://example.com/%story"),
        ("TabOrNewlineIgnored", "https://example.com/\tstory"),
        ("UnencodedAtSign", "https://user@name@example.com/story"),
    ] {
        assert_eq!(TupleOrigin::parse(raw, UrlInput::Referer), None, "{class}",);
    }
}

#[test]
fn configured_and_header_origins_require_origin_only_shape() {
    for raw in [
        "https://example.com/",
        "https://example.com/.",
        "https://example.com/foo/..",
        "https://example.com/%2e",
        "https://example.com/path",
        "https://example.com?view=editor",
        "https://example.com#fragment",
        "https://user@example.com",
        "https://example.com:",
        "not-an-origin",
    ] {
        assert_eq!(TupleOrigin::from_allowed_origin(raw), None, "{raw}");
    }

    for raw in [
        "https://example.com/",
        "https://example.com/.",
        "https://example.com/foo/..",
        "https://example.com/%2e",
        "https://example.com/path",
        "https://example.com?view=editor",
        "https://example.com:",
    ] {
        let headers = headers_with(header::ORIGIN, raw);
        assert_eq!(
            request_origin(
                &headers,
                RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
            ),
            None,
            "{raw}",
        );
    }
}
