use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use axum::body::Bytes;
use storyos_contracts::{
    WEB_ASSET_MANIFEST, WebAssetManifest, WebResource, web_asset_digest, web_resource_paths,
};

/// The validated, immutable resource snapshot for one packaged Server.
pub struct WebAssetSet {
    resources: BTreeMap<String, (WebResource, Bytes)>,
}

impl WebAssetSet {
    /// Load only the asset manifest bound into this Server by the release build.
    pub fn load(root: &Path) -> io::Result<Self> {
        let digest = option_env!("STORYOS_WEB_MANIFEST_SHA256")
            .ok_or_else(|| io::Error::other("Use make release-package to build the Server"))?;
        Self::load_bound(root, digest)
    }

    pub(crate) fn load_bound(root: &Path, digest: &str) -> io::Result<Self> {
        let actual_paths = web_resource_paths(root)?;
        let manifest_bytes = fs::read(root.join(WEB_ASSET_MANIFEST))?;
        if web_asset_digest(&manifest_bytes) != digest {
            return Err(io::Error::other(
                "Web manifest does not match the packaged Server",
            ));
        }
        let manifest: WebAssetManifest = serde_json::from_slice(&manifest_bytes)?;
        manifest.validate_identity()?;
        let expected_paths = manifest
            .resources
            .iter()
            .map(|record| record.path.clone())
            .collect::<BTreeSet<_>>();
        if expected_paths.len() != manifest.resources.len() || expected_paths != actual_paths {
            return Err(io::Error::other(
                "Web resources are missing, extra, or duplicated",
            ));
        }
        let mut resources = BTreeMap::new();
        for record in manifest.resources {
            let path = root.join(&record.path);
            if fs::metadata(&path)?.len() != record.byte_length {
                return Err(io::Error::other(format!(
                    "Web resource length differs: {}",
                    record.path
                )));
            }
            let bytes = fs::read(path)?;
            if WebResource::from_bytes(record.path.clone(), &bytes)? != record {
                return Err(io::Error::other(format!(
                    "Web resource differs: {}",
                    record.path
                )));
            }
            resources.insert(record.path.clone(), (record, Bytes::from(bytes)));
        }
        Ok(Self { resources })
    }

    /// Read validated bytes without reopening the deployment directory.
    pub fn resource(&self, path: &str) -> Option<(&WebResource, Bytes)> {
        self.resources
            .get(path)
            .map(|(record, bytes)| (record, bytes.clone()))
    }
}

#[cfg(test)]
#[path = "web_assets_tests.rs"]
mod tests;
