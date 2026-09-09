use super::{
    PackagedPublicOrigin, PackagedTransportError, PackagedTransportPlan, packaged_transport_plan,
};

#[test]
fn a_configured_http_origin_refuses_startup() {
    assert_eq!(
        packaged_transport_plan(
            /*public_origin*/ Some("http://example.com"),
            "127.0.0.1:3000"
        ),
        Err(PackagedTransportError::InvalidPublicOrigin),
    );
}

#[test]
fn a_configured_https_origin_is_the_printed_site() {
    assert_eq!(
        packaged_transport_plan(
            /*public_origin*/ Some("https://EXAMPLE.com:443"),
            "127.0.0.1:3000",
        ),
        Ok(PackagedTransportPlan::PublicHttps(PackagedPublicOrigin {
            allowed_host: "example.com".to_owned(),
            allowed_origin: "https://example.com".to_owned(),
        })),
    );
}

#[test]
fn an_invalid_configured_origin_refuses_startup() {
    for raw in [
        "",
        "https://example.com/path",
        "https://example.com?view=editor",
        "https://example.com#fragment",
        "https://user@example.com",
        "https://127.0.0.1",
        "https://[::1]",
        "not-an-origin",
    ] {
        assert_eq!(
            packaged_transport_plan(/*public_origin*/ Some(raw), "127.0.0.1:3000"),
            Err(PackagedTransportError::InvalidPublicOrigin),
            "{raw}",
        );
    }
}

#[test]
fn a_public_origin_refuses_a_non_loopback_listen() {
    for bind in ["0.0.0.0:3000", "192.168.1.8:3000", "[::]:3000"] {
        assert_eq!(
            packaged_transport_plan(/*public_origin*/ Some("https://example.com"), bind),
            Err(PackagedTransportError::NonLoopbackListen),
            "{bind}",
        );
    }
}

#[test]
fn unset_public_origin_keeps_the_local_http_profile() {
    assert_eq!(
        packaged_transport_plan(/*public_origin*/ None, "0.0.0.0:3000"),
        Ok(PackagedTransportPlan::LocalHttp),
    );
}

#[test]
fn a_public_origin_admits_loopback_localhost_and_unix_listens() {
    let expected = PackagedTransportPlan::PublicHttps(PackagedPublicOrigin {
        allowed_host: "example.com:8443".to_owned(),
        allowed_origin: "https://example.com:8443".to_owned(),
    });
    for bind in [
        "127.0.0.1:0",
        "[::1]:3000",
        "localhost:3000",
        "/run/storyos.sock",
    ] {
        assert_eq!(
            packaged_transport_plan(
                /*public_origin*/ Some("https://example.com:8443"),
                bind
            ),
            Ok(expected.clone()),
            "{bind}",
        );
    }
}
