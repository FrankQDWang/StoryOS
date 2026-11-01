use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("adapter crate must live at crates/<name>");
    let migrations_dir = manifest_dir.join("migrations");
    let route_catalog_path =
        repo_root.join("generated/route-catalog/versioned-protocol-release-1-route-catalog.json");
    println!("cargo:rerun-if-changed={}", migrations_dir.display());
    println!("cargo:rerun-if-changed={}", route_catalog_path.display());

    let mut files: Vec<PathBuf> = fs::read_dir(&migrations_dir)
        .expect("read migrations directory")
        .map(|entry| entry.expect("read migrations directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect();
    files.sort_by(|left, right| left.file_name().cmp(&right.file_name()));

    let mut rust = String::from(
        "pub struct EmbeddedSource {\n    pub path: &'static str,\n    pub lf_sha256: &'static str,\n    pub sql: &'static str,\n}\n\n",
    );
    rust.push_str("pub const EMBEDDED_SOURCES: &[EmbeddedSource] = &[\n");
    let mut chain = String::new();
    for file in &files {
        let path = file
            .strip_prefix(repo_root)
            .expect("migration lives under the repository root")
            .to_str()
            .expect("migration path is UTF-8")
            .replace('\\', "/");
        let sql = fs::read_to_string(file).unwrap_or_else(|error| panic!("read {path}: {error}"));
        let checksum = lf_sha256(&sql);
        writeln!(chain, "{path}\n{checksum}").expect("write chain pair");
        let include_path = file
            .canonicalize()
            .unwrap_or_else(|error| panic!("canonicalize {path}: {error}"));
        rust.push_str("    EmbeddedSource {\n");
        writeln!(rust, "        path: \"{path}\",").expect("write path");
        writeln!(rust, "        lf_sha256: \"{checksum}\",").expect("write checksum");
        writeln!(
            rust,
            "        sql: include_str!(\"{}\"),",
            include_path.display().to_string().replace('\\', "/")
        )
        .expect("write include");
        rust.push_str("    },\n");
    }
    rust.push_str("];\n\n");
    writeln!(
        rust,
        "pub const MIGRATION_CHAIN_DIGEST: &str = \"{}\";",
        lf_sha256(&chain)
    )
    .expect("write migration chain digest");

    let route_catalog: serde_json::Value = serde_json::from_slice(
        &fs::read(&route_catalog_path).expect("read generated route catalog"),
    )
    .expect("generated route catalog must be JSON");
    let route_catalog_id = route_catalog["catalog_id"]
        .as_str()
        .expect("generated route catalog identifier");
    writeln!(
        rust,
        "pub const ROUTE_CATALOG_ID: &str = \"{route_catalog_id}\";"
    )
    .expect("write route catalog identifier");
    let route_catalog_include = route_catalog_path
        .canonicalize()
        .expect("canonicalize generated route catalog");
    writeln!(
        rust,
        "pub const ROUTE_CATALOG_JSON: &str = include_str!(\"{}\");",
        route_catalog_include
            .display()
            .to_string()
            .replace('\\', "/")
    )
    .expect("write route catalog include");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("embedded_bootstrap.rs"), rust).expect("write embedded bootstrap");
}

fn lf_sha256(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    Sha256::digest(normalized.as_bytes()).iter().fold(
        String::from("sha256:"),
        |mut encoded, byte| {
            write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
            encoded
        },
    )
}
