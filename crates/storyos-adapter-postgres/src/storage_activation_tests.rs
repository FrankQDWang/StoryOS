use sha2::{Digest, Sha256};

use super::{identity, packaged_catalog, packaged_identity};

const GENERATED_ROUTE_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/route-catalog/versioned-protocol-release-1-route-catalog.json"
));

#[test]
fn the_packaged_route_catalog_digest_is_the_digest_of_the_generated_route_catalog() {
    let catalog = packaged_catalog().expect("the packaged persistence catalog must load");
    let packaged = packaged_identity(&catalog).expect("the packaged identity must build");
    let digest = Sha256::digest(GENERATED_ROUTE_CATALOG.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    assert_eq!(packaged.route_catalog_sha256, format!("sha256:{digest}"));
    assert!(
        catalog["schema_identity"]["protocol_binding"]
            .get("route_catalog_sha256")
            .is_none(),
        "the persistence catalog must not hold a copy of the route catalog digest"
    );
}

#[test]
fn a_changed_generated_route_catalog_changes_the_packaged_identity() {
    let catalog = packaged_catalog().expect("the packaged persistence catalog must load");
    let packaged = packaged_identity(&catalog).expect("the packaged identity must build");
    let changed_catalog = GENERATED_ROUTE_CATALOG.replacen(
        "\"error_profile\": \"query_protocol\"",
        "\"error_profile\": \"query_canonical\"",
        1,
    );
    assert_ne!(changed_catalog, GENERATED_ROUTE_CATALOG);

    let changed = identity(&catalog, &changed_catalog).expect("the changed identity must build");

    assert_ne!(changed.route_catalog_sha256, packaged.route_catalog_sha256);
    assert_ne!(
        changed, packaged,
        "a stored identity of the changed catalog must refuse with identity_mismatch"
    );
}
