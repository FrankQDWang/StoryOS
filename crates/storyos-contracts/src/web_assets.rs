use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WEB_ASSET_MANIFEST: &str = "manifest.json";
pub const WEB_ASSET_SCHEMA: &str = "storyos.web-assets.v1";
/// The reviewed browser security policy accepted by this Server release.
pub const RELEASE_1_SECURITY_POLICY_REVISION: &str = "storyos.web-security-policy.release-1.v1";

/// The exact packaged Web resources, separate from public protocol compatibility.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebAssetManifest {
    pub schema_id: String,
    pub source_commit: String,
    pub source_tree: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub resources: Vec<WebResource>,
}

/// One immutable resource in the controlled Web build.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebResource {
    pub path: String,
    pub byte_length: u64,
    pub mime_type: String,
    pub sha256: String,
}

/// Hash the exact bytes, including manifest whitespace, with a named algorithm.
pub fn web_asset_digest(bytes: &[u8]) -> String {
    crate::digest::sha256_prefixed(bytes)
}

impl WebResource {
    /// Describe bytes only at an allowed production resource path.
    pub fn from_bytes(path: String, bytes: &[u8]) -> io::Result<Self> {
        Ok(Self {
            mime_type: resource_mime(&path)?.to_owned(),
            path,
            byte_length: bytes.len() as u64,
            sha256: web_asset_digest(bytes),
        })
    }
}

impl WebAssetManifest {
    /// Refuse a different format, release contract, policy, or invalid source identity.
    pub fn validate_identity(&self) -> io::Result<()> {
        let valid_git_id = |value: &str| {
            value.len() == 40
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        if self.schema_id != WEB_ASSET_SCHEMA
            || self.client_contract_revision != crate::release1::WEB_CLIENT_CONTRACT_REVISION
            || self.security_policy_revision != RELEASE_1_SECURITY_POLICY_REVISION
            || !valid_git_id(&self.source_commit)
            || !valid_git_id(&self.source_tree)
        {
            return Err(io::Error::other(
                "Web manifest identity does not match this release",
            ));
        }
        Ok(())
    }
}

fn resource_mime(path: &str) -> io::Result<&'static str> {
    if path == "index.html" {
        return Ok("text/html; charset=utf-8");
    }
    let asset = path.strip_prefix("assets/").unwrap_or_default();
    let (stem, extension) = asset.rsplit_once('.').unwrap_or_default();
    // Vite emits an eight-character content hash before the extension.
    if stem.len() < 10
        || !stem.is_ascii()
        || stem.as_bytes()[stem.len() - 9] != b'-'
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err(io::Error::other(format!(
            "Illegal Web resource path: {path}"
        )));
    }
    match extension {
        "js" => Ok("text/javascript; charset=utf-8"),
        "css" => Ok("text/css; charset=utf-8"),
        "png" => Ok("image/png"),
        "jpg" => Ok("image/jpeg"),
        "svg" => Ok("image/svg+xml"),
        "webp" => Ok("image/webp"),
        "woff2" => Ok("font/woff2"),
        _ => Err(io::Error::other(format!(
            "Unsupported Web resource type: {path}"
        ))),
    }
}

/// Enumerate the package layout without following symbolic links or exposing build inputs.
/// The manifest is metadata, not a resource. All other files must be index or hashed assets.
pub fn web_resource_paths(root: &Path) -> io::Result<BTreeSet<String>> {
    if !fs::symlink_metadata(root)?.file_type().is_dir() {
        return Err(io::Error::other("Web root must be a real directory"));
    }
    let mut paths = BTreeSet::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(io::Error::other)?;
            let name = relative
                .to_str()
                .ok_or_else(|| io::Error::other("Non-UTF-8 Web path"))?;
            let kind = entry.file_type()?;
            if kind.is_dir() && name == "assets" {
                directories.push(path);
            } else if kind.is_file() {
                if name != WEB_ASSET_MANIFEST {
                    resource_mime(name)?;
                    paths.insert(name.to_owned());
                }
            } else {
                return Err(io::Error::other(format!("Unsupported Web entry: {name}")));
            }
        }
    }
    if !paths.contains("index.html") {
        return Err(io::Error::other("Web index is missing"));
    }
    Ok(paths)
}

/// Generate a build manifest from the exact resource bytes and verified Git provenance.
pub fn write_web_asset_manifest(root: &Path, commit: &str, tree: &str) -> io::Result<String> {
    let resources = web_resource_paths(root)?
        .into_iter()
        .map(|path| WebResource::from_bytes(path.clone(), &fs::read(root.join(&path))?))
        .collect::<io::Result<Vec<_>>>()?;
    let manifest = WebAssetManifest {
        schema_id: WEB_ASSET_SCHEMA.to_owned(),
        source_commit: commit.to_owned(),
        source_tree: tree.to_owned(),
        client_contract_revision: crate::release1::WEB_CLIENT_CONTRACT_REVISION.to_owned(),
        security_policy_revision: RELEASE_1_SECURITY_POLICY_REVISION.to_owned(),
        resources,
    };
    manifest.validate_identity()?;
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    fs::write(root.join(WEB_ASSET_MANIFEST), &bytes)?;
    Ok(web_asset_digest(&bytes))
}
