use std::collections::BTreeMap;

use serde_json::Value;

use super::{FIXTURE_CATALOG_PATH, FIXTURE_DIGEST_PLACEHOLDER, release1_protocol_profile};
use crate::digest::sha256_prefixed;

const INCOMPLETE_FIXTURE_CORPUS_DIGEST: &str =
    "sha256:ae4c01ee1cd84dc779177dcae27abd1e1856fe3b0608848938027d024f4d4071";
const GOLDEN_WIRE_PREFIX: &str = "generated/golden-wire/storyos-public-release-1/";
const PREVIOUSLY_OMITTED_OPERATIONS: [&str; 6] = [
    "listProjects",
    "searchManuscript",
    "getStatistics",
    "deleteVolume",
    "updateChapter",
    "deleteChapter",
];

#[test]
fn fixture_corpus_digest_covers_every_declared_catalog_fixture() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let catalog: Value = serde_json::from_slice(&generated[FIXTURE_CATALOG_PATH])
        .expect("fixture catalog must be JSON");
    let digest_paths = catalog["digest_scope"]["paths"]
        .as_array()
        .expect("digest_scope.paths must be an array")
        .iter()
        .map(|path| path.as_str().expect("digest path must be a string"))
        .collect::<Vec<_>>();
    let fixture_paths = catalog["fixtures"]
        .as_array()
        .expect("fixture catalog entries must be an array")
        .iter()
        .map(|fixture| {
            fixture["path"]
                .as_str()
                .expect("fixture path must be a string")
        })
        .collect::<Vec<_>>();
    let mut golden_paths = generated
        .keys()
        .copied()
        .filter(|path| path.starts_with(GOLDEN_WIRE_PREFIX))
        .collect::<Vec<_>>();
    let mut declared_paths = digest_paths.clone();

    assert_eq!(digest_paths, fixture_paths);
    declared_paths.sort_unstable();
    golden_paths.sort_unstable();
    assert_eq!(declared_paths, golden_paths);

    let published = catalog["corpus_digest"]
        .as_str()
        .expect("fixture catalog must publish corpus_digest");
    assert_eq!(
        published,
        release1_protocol_profile()
            .release_identity
            .fixture_corpus_digest
    );
    assert_ne!(published, INCOMPLETE_FIXTURE_CORPUS_DIGEST);

    let reconstructed = reconstruct_fixture_corpus_digest(&generated, &digest_paths, published);
    assert_eq!(reconstructed, published);

    for operation_id in PREVIOUSLY_OMITTED_OPERATIONS {
        let operation_paths = catalog["fixtures"]
            .as_array()
            .expect("fixture catalog entries must be an array")
            .iter()
            .filter(|fixture| fixture["operation_id"] == operation_id)
            .map(|fixture| {
                fixture["path"]
                    .as_str()
                    .expect("fixture path must be a string")
            })
            .collect::<Vec<_>>();
        assert_eq!(operation_paths.len(), 3, "{operation_id}");
        for path in operation_paths {
            let mut files = generated.clone();
            files
                .get_mut(path)
                .expect("declared fixture must be generated")
                .push(b'\n');
            assert_ne!(
                reconstruct_fixture_corpus_digest(&files, &digest_paths, published),
                published,
                "{operation_id} fixture {path} must affect the corpus digest"
            );
        }
    }
}

fn reconstruct_fixture_corpus_digest(
    generated: &BTreeMap<&str, Vec<u8>>,
    paths: &[&str],
    published_digest: &str,
) -> String {
    let mut corpus = Vec::new();
    for path in paths {
        let bytes = generated
            .get(path)
            .unwrap_or_else(|| panic!("declared fixture {path} must be generated"));
        corpus.extend(normalize_emitted_fixture(bytes, published_digest));
    }
    sha256_prefixed(corpus)
}

fn normalize_emitted_fixture(bytes: &[u8], published_digest: &str) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(bytes).expect("generated fixture must be JSON");
    if let Some(digest) = value.pointer_mut("/release_identity/fixture_corpus_digest") {
        assert_eq!(digest.as_str(), Some(published_digest));
        *digest = Value::String(FIXTURE_DIGEST_PLACEHOLDER.to_owned());
        let mut normalized = serde_json::to_vec_pretty(&value).expect("normalized fixture JSON");
        normalized.push(b'\n');
        return normalized;
    }
    bytes.to_vec()
}
