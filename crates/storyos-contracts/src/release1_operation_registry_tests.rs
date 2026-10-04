use std::collections::HashMap;

use super::RELEASE1_OPERATIONS;

#[test]
fn registry_follows_the_reviewed_route_catalog_order() {
    let catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../docs/foundation/versioned-protocol-release-1-route-catalog.json"
    ))
    .expect("reviewed route catalog must be JSON");
    let positions = catalog["operations"]
        .as_array()
        .expect("reviewed operations must be an array")
        .iter()
        .enumerate()
        .map(|(position, entry)| (entry["operation_id"].as_str(), position))
        .collect::<HashMap<_, _>>();
    let registered = RELEASE1_OPERATIONS
        .iter()
        .map(|artifacts| {
            artifacts
                .operations
                .iter()
                .map(|registered| positions[&Some(registered.operation.operation_id)])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut catalog_order = registered.clone();
    catalog_order
        .iter_mut()
        .for_each(|module| module.sort_unstable());
    catalog_order.sort();
    assert_eq!(registered, catalog_order);
}
