use std::sync::LazyLock;

use crate::Release1ProtocolProfile;
use crate::release1_operation_registry::{FixtureMembership, RELEASE1_OPERATIONS};

use super::GeneratedFile;

/// The generated golden files and the digest inputs follow the registry fixture order.
pub(super) fn fixture_corpus_membership() -> &'static [FixtureMembership] {
    static MEMBERSHIP: LazyLock<Vec<FixtureMembership>> = LazyLock::new(|| {
        RELEASE1_OPERATIONS
            .iter()
            .flat_map(|artifacts| artifacts.fixtures.iter().copied())
            .collect()
    });
    &MEMBERSHIP
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
