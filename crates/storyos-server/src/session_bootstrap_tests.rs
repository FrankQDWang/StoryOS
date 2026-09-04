use super::{
    MultipleMappingAllowance, PackagedSessionBootstrapError, TrustedLocalSessionBootstrap,
    packaged_session_mappings,
};

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const OTHER: &str = "018f0000-0000-7001-8000-000000000101";

#[test]
fn packaged_production_requires_exactly_one_handle() {
    assert_eq!(
        packaged_session_mappings(/*raw*/ None, MultipleMappingAllowance::Refuse),
        Err(PackagedSessionBootstrapError::Missing)
    );
    assert_eq!(
        packaged_session_mappings(/*raw*/ Some("{}"), MultipleMappingAllowance::Refuse),
        Err(PackagedSessionBootstrapError::Missing)
    );
    assert_eq!(
        packaged_session_mappings(
            /*raw*/
            Some(&format!(
                r#"{{"session-a":"{USER}","session-b":"{OTHER}"}}"#
            )),
            MultipleMappingAllowance::Refuse,
        ),
        Err(PackagedSessionBootstrapError::MappingCount)
    );
    assert_eq!(
        packaged_session_mappings(/*raw*/ Some("{"), MultipleMappingAllowance::Refuse),
        Err(PackagedSessionBootstrapError::InvalidJson)
    );
    assert_eq!(
        packaged_session_mappings(
            /*raw*/ Some(&format!(r#"{{"":"{USER}"}}"#)),
            MultipleMappingAllowance::Refuse,
        ),
        Err(PackagedSessionBootstrapError::EmptyHandle)
    );
    assert_eq!(
        packaged_session_mappings(
            /*raw*/ Some(&format!(r#"{{"session a":"{USER}"}}"#)),
            MultipleMappingAllowance::Refuse,
        ),
        Err(PackagedSessionBootstrapError::InvalidHandle)
    );
}

#[test]
fn one_handle_issues_that_cookie_value() {
    let (mappings, bootstrap) = packaged_session_mappings(
        /*raw*/ Some(&format!(r#"{{"session-a":"{USER}"}}"#)),
        MultipleMappingAllowance::Refuse,
    )
    .expect("one mapping is the production path");
    assert_eq!(mappings.get("session-a").map(String::as_str), Some(USER));
    assert_eq!(
        bootstrap,
        TrustedLocalSessionBootstrap::IssueTheSingleConfiguredHandle
    );
}

#[test]
fn multiple_mapping_test_allowance_disables_issuance() {
    let (mappings, bootstrap) = packaged_session_mappings(
        /*raw*/
        Some(&format!(
            r#"{{"session-a":"{USER}","session-b":"{OTHER}"}}"#
        )),
        MultipleMappingAllowance::IsolationTestsDisableIssuance,
    )
    .expect("isolation tests may load two Users");
    assert_eq!(mappings.len(), 2);
    assert_eq!(bootstrap, TrustedLocalSessionBootstrap::Disabled);
    assert_eq!(
        packaged_session_mappings(
            /*raw*/ Some(&format!(r#"{{"session-a":"{USER}"}}"#)),
            MultipleMappingAllowance::IsolationTestsDisableIssuance,
        )
        .expect("allowance with one mapping still starts")
        .1,
        TrustedLocalSessionBootstrap::Disabled
    );
}
