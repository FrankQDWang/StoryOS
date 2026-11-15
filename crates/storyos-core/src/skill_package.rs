//! The rules of a standard SkillPackage and of the Skill set of one deployment (ADR 0089 sections
//! 2 and 9). The rules read no file: the Server reader gives the bytes and the decoded frontmatter.

use std::collections::BTreeSet;

use crate::assemble_context::count_context_item_tokens;
use crate::{CONTEXT_ITEM_TOKEN_LIMIT, admit_archive_path, canonical_json, hex_sha256};

pub const SKILL_PACKAGE_FILE_LIMIT: usize = 64;
pub const SKILL_PACKAGE_BYTE_LIMIT: u64 = 1_048_576;
pub const SKILL_SET_LIMIT: usize = 32;
/// The most counted tokens of the catalog text of one deployment (ADR 0089 section 9).
pub const SKILL_CATALOG_TOKEN_LIMIT: u64 = 32_000;
pub const SKILL_PACKAGE_INVENTORY_PROFILE: &str = "storyos.skill-package.inventory.sha256.v1";
pub const SKILL_MD_PATH: &str = "SKILL.md";
const NAME_MAX_CHARACTERS: usize = 64;
const DESCRIPTION_MAX_CHARACTERS: usize = 1024;
const COMPATIBILITY_MAX_CHARACTERS: usize = 500;

/// One file of a package directory: its relative path with `/` separators, and its bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillFileInput {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// One decoded YAML value of the frontmatter. `Other` is each value that is not text or a
/// mapping with text keys, for example a number, a list, or a null.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrontmatterValue {
    Text(String),
    Mapping(Vec<(String, FrontmatterValue)>),
    Other,
}

/// The facts of one package directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillPackageInput {
    pub directory_name: String,
    pub files: Vec<SkillFileInput>,
    /// The decoded block of `skill_md_frontmatter`, or `None` when it is not one YAML mapping.
    pub frontmatter: Option<Vec<(String, FrontmatterValue)>>,
}

/// One file of a validated package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub byte_length: u64,
    /// The lowercase hexadecimal SHA-256 of the bytes.
    pub sha256: String,
    /// The counted tokens, when the bytes are UTF-8.
    pub tokens: Option<u64>,
    /// UTF-8 text of at most `CONTEXT_ITEM_TOKEN_LIMIT` tokens. Only such a file can be read.
    pub readable: bool,
}

/// The immutable reference to one package: its name and its package digest.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillPackageReference {
    pub name: String,
    pub package_digest: String,
}

/// One validated standard SkillPackage. Its files are in path order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillPackage {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: Vec<(String, String)>,
    /// The `allowed-tools` text. It grants nothing.
    pub allowed_tools: Option<String>,
    pub files: Vec<SkillFile>,
    /// `sha256:` and the SHA-256 of the inventory of `SKILL_PACKAGE_INVENTORY_PROFILE`.
    pub package_digest: String,
}

impl SkillPackage {
    pub fn reference(&self) -> SkillPackageReference {
        SkillPackageReference {
            name: self.name.clone(),
            package_digest: self.package_digest.clone(),
        }
    }

    fn catalog_entry(&self) -> SkillCatalogEntry {
        SkillCatalogEntry {
            reference: self.reference(),
            description: self.description.clone(),
        }
    }
}

/// The rule that refuses a package. `label` gives the text of the startup refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillPackageRefusal {
    FileLimitExceeded,
    SizeLimitExceeded,
    PathUnsafe,
    SkillMdMissing,
    /// The `SKILL.md` is not UTF-8 text, or it has U+0000.
    SkillMdNotText,
    SkillMdTooLong,
    FrontmatterInvalid,
    NameInvalid,
    NameDirectoryMismatch,
    /// Also a decoded description with U+0000.
    DescriptionInvalid,
    /// Also a decoded optional text, metadata key, or metadata value with U+0000.
    OptionalFieldInvalid,
}

impl SkillPackageRefusal {
    pub fn label(self) -> &'static str {
        match self {
            Self::FileLimitExceeded => "file_limit_exceeded",
            Self::SizeLimitExceeded => "size_limit_exceeded",
            Self::PathUnsafe => "path_unsafe",
            Self::SkillMdMissing => "skill_md_missing",
            Self::SkillMdNotText => "skill_md_not_text",
            Self::SkillMdTooLong => "skill_md_too_long",
            Self::FrontmatterInvalid => "frontmatter_invalid",
            Self::NameInvalid => "name_invalid",
            Self::NameDirectoryMismatch => "name_directory_mismatch",
            Self::DescriptionInvalid => "description_invalid",
            Self::OptionalFieldInvalid => "optional_field_invalid",
        }
    }
}

/// The YAML text between the first line `---` and the next line `---` of a `SKILL.md`, or
/// `None` when the text does not start with such a block.
pub fn skill_md_frontmatter(text: &str) -> Option<&str> {
    let body = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some(&body[..offset]);
        }
        offset += line.len();
    }
    None
}

/// Validates one package directory against the Agent Skills format and the limits of ADR 0089.
pub fn admit_skill_package(input: SkillPackageInput) -> Result<SkillPackage, SkillPackageRefusal> {
    let SkillPackageInput {
        directory_name,
        mut files,
        frontmatter,
    } = input;
    if files.len() > SKILL_PACKAGE_FILE_LIMIT {
        return Err(SkillPackageRefusal::FileLimitExceeded);
    }
    let total: u64 = files.iter().map(|file| file.bytes.len() as u64).sum();
    if total > SKILL_PACKAGE_BYTE_LIMIT {
        return Err(SkillPackageRefusal::SizeLimitExceeded);
    }
    let mut collision_keys = BTreeSet::new();
    for file in &files {
        let admitted =
            admit_archive_path(&file.path).map_err(|_| SkillPackageRefusal::PathUnsafe)?;
        if !collision_keys.insert(admitted.collision_key) {
            return Err(SkillPackageRefusal::PathUnsafe);
        }
    }
    let skill_md = files
        .iter()
        .find(|file| file.path == SKILL_MD_PATH)
        .ok_or(SkillPackageRefusal::SkillMdMissing)?;
    let text =
        std::str::from_utf8(&skill_md.bytes).map_err(|_| SkillPackageRefusal::SkillMdNotText)?;
    if !is_storable_text(text) {
        return Err(SkillPackageRefusal::SkillMdNotText);
    }
    if count_context_item_tokens(text) > CONTEXT_ITEM_TOKEN_LIMIT {
        return Err(SkillPackageRefusal::SkillMdTooLong);
    }
    let fields = skill_md_frontmatter(text)
        .and(frontmatter)
        .ok_or(SkillPackageRefusal::FrontmatterInvalid)?;
    let field = |key: &str| {
        fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    };
    let name = match field("name") {
        Some(FrontmatterValue::Text(name)) if is_skill_name(name) => name.clone(),
        _ => return Err(SkillPackageRefusal::NameInvalid),
    };
    if name != directory_name {
        return Err(SkillPackageRefusal::NameDirectoryMismatch);
    }
    let description = match field("description") {
        Some(FrontmatterValue::Text(text))
            if (1..=DESCRIPTION_MAX_CHARACTERS).contains(&text.chars().count())
                && is_storable_text(text) =>
        {
            text.clone()
        }
        _ => return Err(SkillPackageRefusal::DescriptionInvalid),
    };
    let optional_text = |key: &str, max_characters: usize| match field(key) {
        None => Ok(None),
        Some(FrontmatterValue::Text(text))
            if text.chars().count() <= max_characters && is_storable_text(text) =>
        {
            Ok(Some(text.clone()))
        }
        Some(_) => Err(SkillPackageRefusal::OptionalFieldInvalid),
    };
    let license = optional_text("license", usize::MAX)?;
    let compatibility = optional_text("compatibility", COMPATIBILITY_MAX_CHARACTERS)?;
    let allowed_tools = optional_text("allowed-tools", usize::MAX)?;
    let metadata = match field("metadata") {
        None => Vec::new(),
        Some(FrontmatterValue::Mapping(entries)) => entries
            .iter()
            .map(|(key, value)| match value {
                FrontmatterValue::Text(text) if is_storable_text(key) && is_storable_text(text) => {
                    Ok((key.clone(), text.clone()))
                }
                FrontmatterValue::Text(_)
                | FrontmatterValue::Mapping(_)
                | FrontmatterValue::Other => Err(SkillPackageRefusal::OptionalFieldInvalid),
            })
            .collect::<Result<_, _>>()?,
        Some(FrontmatterValue::Text(_) | FrontmatterValue::Other) => {
            return Err(SkillPackageRefusal::OptionalFieldInvalid);
        }
    };
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let files: Vec<SkillFile> = files.into_iter().map(skill_file).collect();
    Ok(SkillPackage {
        package_digest: package_digest(&files),
        name,
        description,
        license,
        compatibility,
        metadata,
        allowed_tools,
        files,
    })
}

/// Text without U+0000. The record of a Run keeps the `SKILL.md` text and each decoded
/// frontmatter text as database text, and database text cannot hold U+0000. A YAML escape
/// can decode to U+0000 when the `SKILL.md` bytes do not have it.
fn is_storable_text(text: &str) -> bool {
    !text.contains('\0')
}

/// 1 to 64 characters of `a-z`, `0-9`, and `-`, with no hyphen at an end and no double hyphen.
pub(crate) fn is_skill_name(name: &str) -> bool {
    (1..=NAME_MAX_CHARACTERS).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

fn skill_file(input: SkillFileInput) -> SkillFile {
    let tokens = std::str::from_utf8(&input.bytes)
        .ok()
        .map(count_context_item_tokens);
    SkillFile {
        byte_length: input.bytes.len() as u64,
        sha256: hex_sha256(&input.bytes),
        readable: tokens.is_some_and(|tokens| tokens <= CONTEXT_ITEM_TOKEN_LIMIT),
        tokens,
        path: input.path,
        bytes: input.bytes,
    }
}

/// The digest of `SKILL_PACKAGE_INVENTORY_PROFILE`: the canonical JSON list of the files in path
/// order, with the path, the byte length, and the SHA-256 of each file.
fn package_digest(files: &[SkillFile]) -> String {
    let inventory = files
        .iter()
        .map(|file| {
            serde_json::json!({
                "path": file.path,
                "byte_length": file.byte_length,
                "sha256": file.sha256,
            })
        })
        .collect();
    let inventory = canonical_json(&serde_json::Value::Array(inventory));
    format!("sha256:{}", hex_sha256(inventory.as_bytes()))
}

/// The validated Skills of one deployment, in the order of their names.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SkillSet {
    packages: Vec<SkillPackage>,
}

/// The rule that refuses a Skill set. `label` gives the text of the startup refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillSetRefusal {
    /// More than `SKILL_SET_LIMIT` packages.
    SkillLimitExceeded,
    /// A catalog text above `SKILL_CATALOG_TOKEN_LIMIT` counted tokens.
    CatalogLimitExceeded,
}

impl SkillSetRefusal {
    pub fn label(self) -> &'static str {
        match self {
            Self::SkillLimitExceeded => "skill_limit_exceeded",
            Self::CatalogLimitExceeded => "skill_catalog_limit_exceeded",
        }
    }
}

impl SkillSet {
    pub fn new(mut packages: Vec<SkillPackage>) -> Result<Self, SkillSetRefusal> {
        if packages.len() > SKILL_SET_LIMIT {
            return Err(SkillSetRefusal::SkillLimitExceeded);
        }
        packages.sort_by(|left, right| left.name.cmp(&right.name));
        let catalog: Vec<_> = packages.iter().map(SkillPackage::catalog_entry).collect();
        if count_context_item_tokens(&skill_catalog_text(&catalog)) > SKILL_CATALOG_TOKEN_LIMIT {
            return Err(SkillSetRefusal::CatalogLimitExceeded);
        }
        Ok(Self { packages })
    }

    pub fn packages(&self) -> &[SkillPackage] {
        &self.packages
    }
}

/// One Skill Catalog Entry of a Run: the reference and the description of one Skill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillCatalogEntry {
    pub reference: SkillPackageReference,
    pub description: String,
}

/// The text of the `skill_catalog` item: one canonical JSON line with the `name` and the
/// `description` of each entry, in the order of the Skill names. The text has no digest, because
/// the model names a Skill by its name (ADR 0089 section 3).
pub fn skill_catalog_text(entries: &[SkillCatalogEntry]) -> String {
    let mut lines: Vec<_> = entries
        .iter()
        .map(|entry| {
            let line = serde_json::json!({
                "name": entry.reference.name,
                "description": entry.description,
            });
            (&entry.reference.name, canonical_json(&line))
        })
        .collect();
    lines.sort();
    lines
        .into_iter()
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// What one `createAgentRun` writes for the Skill set of the deployment (ADR 0089 section 2).
#[derive(Debug, Eq, PartialEq)]
pub struct SkillCatalogPlan<'a> {
    /// Each offered package whose digest the Project does not have, in name order.
    pub snapshots: Vec<&'a SkillPackage>,
    /// The reference of each offered package, in name order: the catalog that the Run pins.
    pub catalog: Vec<SkillPackageReference>,
}

/// The Snapshots to write and the catalog to pin for `offered` in a Project that has the
/// Snapshots of `stored_digests`.
pub fn plan_skill_catalog<'a>(
    offered: &'a SkillSet,
    stored_digests: &BTreeSet<String>,
) -> SkillCatalogPlan<'a> {
    SkillCatalogPlan {
        snapshots: offered
            .packages()
            .iter()
            .filter(|package| !stored_digests.contains(&package.package_digest))
            .collect(),
        catalog: offered
            .packages()
            .iter()
            .map(SkillPackage::reference)
            .collect(),
    }
}

#[cfg(test)]
#[path = "skill_package_tests.rs"]
mod tests;
