//! The read of the Skill directory of the deployment at the start of the Server (ADR 0089
//! section 2). The Server reads it one time, and the Worker never reads it.

use std::fmt;
use std::fs;
use std::io::{self, Read as _};
use std::path::Path;

use storyos_core::{
    ARCHIVE_PATH_MAX_SEGMENTS, FrontmatterValue, SKILL_MD_PATH, SKILL_PACKAGE_BYTE_LIMIT,
    SKILL_PACKAGE_FILE_LIMIT, SkillFileInput, SkillPackage, SkillPackageInput, SkillPackageRefusal,
    SkillSet, admit_skill_package, skill_md_frontmatter,
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
                "STORYOS_SKILLS_DIR: the Skill directory has more than {} packages: skill_limit_exceeded",
                storyos_core::SKILL_SET_LIMIT
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
    let mut packages = Vec::new();
    for (name, kind) in sorted_entries(directory).map_err(unreadable)? {
        if (name.starts_with('.') && kind.is_dir()) || kind.is_file() {
            continue;
        }
        if !kind.is_dir() {
            return Err(SkillDirectoryRefusal::EntryUnsupported { package: name });
        }
        packages.push(read_package(&directory.join(&name), name)?);
    }
    SkillSet::new(packages).map_err(|_| SkillDirectoryRefusal::SkillLimitExceeded)
}

/// The entries of one directory in name order, with the type of each entry itself, so that a
/// symbolic link stays a symbolic link. A name that is not UTF-8 is `InvalidData`.
fn sorted_entries(directory: &Path) -> io::Result<Vec<(String, fs::FileType)>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        entries.push((name, entry.file_type()?));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

fn read_package(root: &Path, package: String) -> Result<SkillPackage, SkillDirectoryRefusal> {
    let mut reader = PackageReader {
        package,
        files: Vec::new(),
        remaining: SKILL_PACKAGE_BYTE_LIMIT,
    };
    reader.read_directory(root, "", 0)?;
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

    /// Stops at the 65th file and at the first byte above the size limit, so that a large tree
    /// does not go into memory.
    fn read_directory(
        &mut self,
        directory: &Path,
        prefix: &str,
        depth: usize,
    ) -> Result<(), SkillDirectoryRefusal> {
        if depth >= ARCHIVE_PATH_MAX_SEGMENTS {
            return Err(self.refused(SkillPackageRefusal::PathUnsafe));
        }
        let entries = sorted_entries(directory).map_err(|error| {
            if error.kind() == io::ErrorKind::InvalidData {
                self.refused(SkillPackageRefusal::PathUnsafe)
            } else {
                self.unreadable()
            }
        })?;
        for (name, kind) in entries {
            let path = format!("{prefix}{name}");
            if kind.is_dir() {
                self.read_directory(&directory.join(&name), &format!("{path}/"), depth + 1)?;
            } else if !kind.is_file() {
                return Err(SkillDirectoryRefusal::EntryUnsupported {
                    package: self.package.clone(),
                });
            } else if name != SKIPPED_FILE {
                if self.files.len() == SKILL_PACKAGE_FILE_LIMIT {
                    return Err(self.refused(SkillPackageRefusal::FileLimitExceeded));
                }
                let mut bytes = Vec::new();
                fs::File::open(directory.join(&name))
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
