//! The read of the Skill directory of the deployment at the start of the Server (ADR 0089
//! section 2). The Server reads it one time, and the Worker never reads it.

use std::fmt;
use std::fs;
use std::io::Read as _;
use std::path::Path;

use storyos_core::{
    FrontmatterValue, SKILL_CATALOG_TOKEN_LIMIT, SKILL_MD_PATH, SKILL_PACKAGE_BYTE_LIMIT,
    SKILL_PACKAGE_FILE_LIMIT, SKILL_SET_LIMIT, SkillFileInput, SkillPackage, SkillPackageInput,
    SkillPackageRefusal, SkillSet, SkillSetRefusal, admit_skill_package, skill_md_frontmatter,
};
use yaml_rust2::{Yaml, YamlLoader};

const SKIPPED_FILE: &str = ".DS_Store";

/// A refusal that stops the start of the Server. Its `Debug` text is its `Display` text, because
/// the Server prints the `Debug` text of the error that stops its start.
#[derive(Eq, PartialEq)]
pub enum SkillDirectoryRefusal {
    /// The setting does not name a directory that the Server can read, or an entry of the named
    /// package cannot be read.
    Unreadable {
        package: Option<String>,
    },
    /// A symbolic link, or an entry that is not a file or a directory (`entry_unsupported`).
    EntryUnsupported {
        package: String,
    },
    Package {
        package: String,
        rule: SkillPackageRefusal,
    },
    SkillLimitExceeded,
    /// The catalog text of the Skill set is above its token limit (ADR 0089 section 9).
    CatalogLimitExceeded,
}

impl fmt::Display for SkillDirectoryRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { package: None } => write!(
                formatter,
                "STORYOS_SKILLS_DIR does not name a directory that the Server can read"
            ),
            Self::Unreadable {
                package: Some(package),
            } => write!(
                formatter,
                "STORYOS_SKILLS_DIR: the Server cannot read the Skill package {package:?}"
            ),
            Self::EntryUnsupported { package } => write!(
                formatter,
                "STORYOS_SKILLS_DIR: the Skill package {package:?} is refused: entry_unsupported"
            ),
            Self::Package { package, rule } => write!(
                formatter,
                "STORYOS_SKILLS_DIR: the Skill package {package:?} is refused: {}",
                rule.label()
            ),
            Self::SkillLimitExceeded => write!(
                formatter,
                "STORYOS_SKILLS_DIR: the Skill directory has more than {SKILL_SET_LIMIT} packages: {}",
                SkillSetRefusal::SkillLimitExceeded.label()
            ),
            Self::CatalogLimitExceeded => write!(
                formatter,
                "STORYOS_SKILLS_DIR: the Skill catalog text is above {SKILL_CATALOG_TOKEN_LIMIT} tokens: {}",
                SkillSetRefusal::CatalogLimitExceeded.label()
            ),
        }
    }
}

impl fmt::Debug for SkillDirectoryRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl std::error::Error for SkillDirectoryRefusal {}

/// Reads and validates each package of `directory`. No setting gives an empty Skill set.
pub fn read_skill_directory(directory: Option<&Path>) -> Result<SkillSet, SkillDirectoryRefusal> {
    let Some(directory) = directory else {
        return Ok(SkillSet::default());
    };
    let unreadable = |_| SkillDirectoryRefusal::Unreadable { package: None };
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let kind = entry.file_type().map_err(unreadable)?;
        let name = entry.file_name();
        if kind.is_file() || (kind.is_dir() && name.as_encoded_bytes().starts_with(b".")) {
            continue;
        }
        let Ok(name) = name.into_string() else {
            return Err(SkillDirectoryRefusal::Package {
                package: entry.file_name().to_string_lossy().into_owned(),
                rule: SkillPackageRefusal::NameInvalid,
            });
        };
        if !kind.is_dir() {
            return Err(SkillDirectoryRefusal::EntryUnsupported { package: name });
        }
        if names.len() == SKILL_SET_LIMIT {
            return Err(SkillDirectoryRefusal::SkillLimitExceeded);
        }
        names.push(name);
    }
    names.sort();
    let packages = names
        .into_iter()
        .map(|name| read_package(&directory.join(&name), name))
        .collect::<Result<_, _>>()?;
    SkillSet::new(packages).map_err(|refusal| match refusal {
        SkillSetRefusal::SkillLimitExceeded => SkillDirectoryRefusal::SkillLimitExceeded,
        SkillSetRefusal::CatalogLimitExceeded => SkillDirectoryRefusal::CatalogLimitExceeded,
    })
}

fn read_package(root: &Path, package: String) -> Result<SkillPackage, SkillDirectoryRefusal> {
    let mut reader = PackageReader {
        package,
        files: Vec::new(),
        remaining: SKILL_PACKAGE_BYTE_LIMIT,
    };
    reader.read_directory(root, "")?;
    let PackageReader { package, files, .. } = reader;
    let frontmatter = files
        .iter()
        .find(|file| file.path == SKILL_MD_PATH)
        .and_then(|file| std::str::from_utf8(&file.bytes).ok())
        .and_then(skill_md_frontmatter)
        .and_then(decode_frontmatter);
    admit_skill_package(SkillPackageInput {
        directory_name: package.clone(),
        files,
        frontmatter,
    })
    .map_err(|rule| SkillDirectoryRefusal::Package { package, rule })
}

struct PackageReader {
    package: String,
    files: Vec<SkillFileInput>,
    remaining: u64,
}

impl PackageReader {
    fn refused(&self, rule: SkillPackageRefusal) -> SkillDirectoryRefusal {
        SkillDirectoryRefusal::Package {
            package: self.package.clone(),
            rule,
        }
    }

    fn unreadable(&self) -> SkillDirectoryRefusal {
        SkillDirectoryRefusal::Unreadable {
            package: Some(self.package.clone()),
        }
    }

    /// Reads one entry at a time, and stops at the 65th file and at the first byte above the
    /// size limit, so that a large tree does not go into memory.
    fn read_directory(
        &mut self,
        directory: &Path,
        prefix: &str,
    ) -> Result<(), SkillDirectoryRefusal> {
        for entry in fs::read_dir(directory).map_err(|_| self.unreadable())? {
            let entry = entry.map_err(|_| self.unreadable())?;
            let kind = entry.file_type().map_err(|_| self.unreadable())?;
            if kind.is_file() && entry.file_name() == SKIPPED_FILE {
                continue;
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| self.refused(SkillPackageRefusal::PathUnsafe))?;
            let path = format!("{prefix}{name}");
            if kind.is_dir() {
                self.read_directory(&entry.path(), &format!("{path}/"))?;
            } else if !kind.is_file() {
                return Err(SkillDirectoryRefusal::EntryUnsupported {
                    package: self.package.clone(),
                });
            } else {
                if self.files.len() == SKILL_PACKAGE_FILE_LIMIT {
                    return Err(self.refused(SkillPackageRefusal::FileLimitExceeded));
                }
                let mut bytes = Vec::new();
                fs::File::open(entry.path())
                    .and_then(|file| file.take(self.remaining + 1).read_to_end(&mut bytes))
                    .map_err(|_| self.unreadable())?;
                self.remaining = self
                    .remaining
                    .checked_sub(bytes.len() as u64)
                    .ok_or_else(|| self.refused(SkillPackageRefusal::SizeLimitExceeded))?;
                self.files.push(SkillFileInput { path, bytes });
            }
        }
        Ok(())
    }
}

/// The frontmatter as plain values, or `None` when it is not one YAML mapping with text keys.
fn decode_frontmatter(block: &str) -> Option<Vec<(String, FrontmatterValue)>> {
    match YamlLoader::load_from_str(block).ok()?.as_slice() {
        [document] => match plain_value(document) {
            FrontmatterValue::Mapping(fields) => Some(fields),
            FrontmatterValue::Text(_) | FrontmatterValue::Other => None,
        },
        _ => None,
    }
}

fn plain_value(value: &Yaml) -> FrontmatterValue {
    match value {
        Yaml::String(text) => FrontmatterValue::Text(text.clone()),
        Yaml::Hash(entries) => entries
            .iter()
            .map(|(key, value)| match key {
                Yaml::String(key) => Some((key.clone(), plain_value(value))),
                _ => None,
            })
            .collect::<Option<_>>()
            .map_or(FrontmatterValue::Other, FrontmatterValue::Mapping),
        Yaml::Real(_)
        | Yaml::Integer(_)
        | Yaml::Boolean(_)
        | Yaml::Array(_)
        | Yaml::Alias(_)
        | Yaml::Null
        | Yaml::BadValue => FrontmatterValue::Other,
    }
}

#[cfg(test)]
#[path = "skill_directory_tests.rs"]
mod tests;
