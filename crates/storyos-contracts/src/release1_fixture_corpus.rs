use std::sync::LazyLock;

use serde_json::json;

use crate::Release1ProtocolProfile;
use crate::release1::PUBLIC_PROTOCOL_RELEASE;
use crate::release1_operation_registry::{FixtureMembership, RELEASE1_OPERATIONS};

use super::{FIXTURE_DIGEST_PLACEHOLDER, GeneratedFile};
use crate::release1_wire::json_bytes;

/// Catalog paths, catalog entries, generated golden files, and digest inputs
/// all derive from the registry fixture order.
fn fixture_corpus_membership() -> &'static [FixtureMembership] {
    static MEMBERSHIP: LazyLock<Vec<FixtureMembership>> = LazyLock::new(|| {
        RELEASE1_OPERATIONS
            .iter()
            .flat_map(|artifacts| (artifacts.fixtures)())
            .collect()
    });
    &MEMBERSHIP
}

pub(super) fn fixture_catalog_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    let membership = fixture_corpus_membership();
    json_bytes(&json!({
        "schema_id": "storyos.fixture-catalog.release-1.v1",
        "public_protocol_release": PUBLIC_PROTOCOL_RELEASE,
        "corpus_digest": profile.release_identity.fixture_corpus_digest,
        "digest_scope": {
            "paths": membership.iter().map(|entry| entry.path).collect::<Vec<_>>(),
            "normalization": format!(
                "replace every release_identity.fixture_corpus_digest with {FIXTURE_DIGEST_PLACEHOLDER}"
            )
        },
        "fixtures": membership.iter().map(|entry| json!({
            "fixture_id": entry.fixture_id,
            "classification": entry.classification,
            "operation_id": entry.operation_id,
            "path": entry.path
        })).collect::<Vec<_>>()
    }))
}

pub(super) fn fixture_corpus_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    fixture_corpus_membership()
        .iter()
        .flat_map(|entry| (entry.bytes)(profile))
        .collect()
}

pub(super) fn generated_fixture_files(profile: &Release1ProtocolProfile) -> Vec<GeneratedFile> {
    fixture_corpus_membership()
        .iter()
        .map(|entry| (entry.path, (entry.bytes)(profile)))
        .collect()
}
