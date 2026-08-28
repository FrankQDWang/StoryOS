use std::fs;
use std::path::PathBuf;

use storyos_contracts::{
    WEB_ASSET_MANIFEST, WebAssetManifest, web_asset_digest, write_web_asset_manifest,
};
use uuid::Uuid;

use super::*;

const SCRIPT: &str = "assets/index-12345678.js";
const INDEX: &[u8] =
    b"<!doctype html><script type=\"module\" src=\"/assets/index-12345678.js\"></script>";

struct Fixture {
    root: PathBuf,
    digest: String,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("storyos-web-{}", Uuid::now_v7()));
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("index.html"), INDEX).unwrap();
        fs::write(root.join(SCRIPT), b"document.title = 'StoryOS';").unwrap();
        let digest =
            write_web_asset_manifest(&root, &"a".repeat(/*n*/ 40), &"b".repeat(/*n*/ 40)).unwrap();
        Self { root, digest }
    }

    fn change_manifest(&mut self, change: impl FnOnce(&mut WebAssetManifest)) {
        let path = self.root.join(WEB_ASSET_MANIFEST);
        let mut manifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        change(&mut manifest);
        let bytes = serde_json::to_vec(&manifest).unwrap();
        fs::write(path, &bytes).unwrap();
        self.digest = web_asset_digest(&bytes);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn validated_resources_survive_directory_changes() {
    let fixture = Fixture::new();
    let assets = WebAssetSet::load_bound(&fixture.root, &fixture.digest).unwrap();
    fs::write(fixture.root.join("index.html"), b"substituted").unwrap();
    fs::remove_file(fixture.root.join(SCRIPT)).unwrap();
    let expected = WebResource::from_bytes("index.html".to_owned(), INDEX).unwrap();
    assert_eq!(
        assets.resource("index.html"),
        Some((&expected, Bytes::from_static(INDEX)))
    );
    assert!(assets.resource(SCRIPT).is_some());
    assert!(assets.resource(WEB_ASSET_MANIFEST).is_none());
}

#[test]
fn refuses_missing_extra_altered_and_mixed_resources() {
    let mutations: [fn(&mut Fixture); 5] = [
        |f| {
            fs::remove_file(f.root.join(SCRIPT)).unwrap();
        },
        |f| {
            fs::write(f.root.join("assets/extra-12345678.js"), b"extra").unwrap();
        },
        |f| {
            fs::write(f.root.join(SCRIPT), b"substituted").unwrap();
        },
        |f| {
            fs::write(f.root.join(WEB_ASSET_MANIFEST), b"{}").unwrap();
        },
        |f| {
            let server_digest = f.digest.clone();
            f.change_manifest(|m| m.source_commit = "c".repeat(/*n*/ 40));
            f.digest = server_digest;
        },
    ];
    for mutate in mutations {
        let mut fixture = Fixture::new();
        mutate(&mut fixture);
        assert!(WebAssetSet::load_bound(&fixture.root, &fixture.digest).is_err());
    }
}

#[test]
fn refuses_invalid_manifest_even_with_its_exact_digest() {
    let mutations: [fn(&mut WebAssetManifest); 7] = [
        |m| m.resources.push(m.resources[0].clone()),
        |m| m.resources[0].path = "../outside.js".to_owned(),
        |m| m.resources[0].mime_type = "text/plain".to_owned(),
        |m| m.resources[0].byte_length += 1,
        |m| m.resources[0].sha256 = web_asset_digest(b"wrong bytes"),
        |m| m.client_contract_revision = "another contract".to_owned(),
        |m| m.security_policy_revision = "another policy".to_owned(),
    ];
    for mutate in mutations {
        let mut fixture = Fixture::new();
        fixture.change_manifest(mutate);
        assert!(WebAssetSet::load_bound(&fixture.root, &fixture.digest).is_err());
    }
}

#[cfg(unix)]
#[test]
fn refuses_symbolic_links_and_unlisted_directories() {
    use std::os::unix::fs::symlink;
    for relative in [SCRIPT, WEB_ASSET_MANIFEST, "assets"] {
        let fixture = Fixture::new();
        let path = fixture.root.join(relative);
        let held = fixture.root.with_extension("held");
        fs::rename(&path, &held).unwrap();
        symlink(&held, &path).unwrap();
        let rejected = WebAssetSet::load_bound(&fixture.root, &fixture.digest).is_err();
        fs::remove_file(&path).unwrap();
        fs::rename(held, path).unwrap();
        assert!(rejected);
    }
    let fixture = Fixture::new();
    let link = fixture.root.with_extension("link");
    symlink(&fixture.root, &link).unwrap();
    let rejected = WebAssetSet::load_bound(&link, &fixture.digest).is_err();
    fs::remove_file(link).unwrap();
    assert!(rejected);
    fs::create_dir(fixture.root.join("extra")).unwrap();
    assert!(WebAssetSet::load_bound(&fixture.root, &fixture.digest).is_err());
}
