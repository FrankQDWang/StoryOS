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
    let catalog_path =
        repo_root.join("docs/foundation/postgresql-release-1-persistence-catalog.json");
    println!("cargo:rerun-if-changed={}", catalog_path.display());
    println!(
        "cargo:rerun-if-changed={}/migrations",
        manifest_dir.display()
    );

    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(&catalog_path).expect("read persistence catalog"))
            .expect("persistence catalog must be JSON");
    let sources = catalog["migration_chain"]["bootstrap"]["sources"]
        .as_array()
        .expect("catalogued bootstrap sources");

    let mut rust = String::from(
        "pub struct EmbeddedSource {\n    pub path: &'static str,\n    pub lf_sha256: &'static str,\n    pub sql: &'static str,\n}\n\n",
    );
    rust.push_str("pub const EMBEDDED_SOURCES: &[EmbeddedSource] = &[\n");
    for source in sources {
        let path = source["path"].as_str().expect("bootstrap source path");
        let expected = source["lf_sha256"]
            .as_str()
            .expect("bootstrap source checksum");
        let file = repo_root.join(path);
        println!("cargo:rerun-if-changed={}", file.display());
        let sql = fs::read(&file).unwrap_or_else(|error| panic!("read {path}: {error}"));
        let actual = lf_sha256(&sql);
        if actual != expected {
            panic!("bootstrap SQL checksum mismatch for {path}: catalog {expected}, file {actual}");
        }
        let include_path = file
            .canonicalize()
            .unwrap_or_else(|error| panic!("canonicalize {path}: {error}"));
        rust.push_str("    EmbeddedSource {\n");
        writeln!(rust, "        path: \"{path}\",").expect("write path");
        writeln!(rust, "        lf_sha256: \"{expected}\",").expect("write checksum");
        writeln!(
            rust,
            "        sql: include_str!(\"{}\"),",
            include_path.display().to_string().replace('\\', "/")
        )
        .expect("write include");
        rust.push_str("    },\n");
    }
    rust.push_str("];\n\n");
    let catalog_include = catalog_path
        .canonicalize()
        .expect("canonicalize persistence catalog");
    writeln!(
        rust,
        "pub const CATALOG_JSON: &str = include_str!(\"{}\");",
        catalog_include.display().to_string().replace('\\', "/")
    )
    .expect("write catalog include");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("embedded_bootstrap.rs"), rust).expect("write embedded bootstrap");
}

fn lf_sha256(bytes: &[u8]) -> String {
    let normalized = String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    Sha256::digest(normalized.as_bytes()).iter().fold(
        String::from("sha256:"),
        |mut encoded, byte| {
            write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
            encoded
        },
    )
}
