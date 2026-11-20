//! The shared test support from `create_agent_run_skill_tests.rs`.

use std::path::Path;
use std::sync::Arc;
use storyos_core::{
    FrontmatterValue, SKILL_MD_PATH, SkillFileInput, SkillPackage, SkillPackageInput, SkillSet,
    admit_skill_package, skill_md_frontmatter,
};

/// One package of `fixtures/skill-packages/`.
pub(crate) fn fixture(name: &str) -> SkillPackage {
    fn collect(root: &Path, directory: &Path, files: &mut Vec<SkillFileInput>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(root, &path, files);
            } else if !path.ends_with(".DS_Store") {
                files.push(SkillFileInput {
                    path: path
                        .strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    bytes: std::fs::read(&path).unwrap(),
                });
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/skill-packages")
        .join(name);
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    package(name, files)
}

/// Admits one package. Each frontmatter line `key: value` is one text field.
pub(crate) fn package(name: &str, files: Vec<SkillFileInput>) -> SkillPackage {
    let skill_md = files
        .iter()
        .find(|file| file.path == SKILL_MD_PATH)
        .unwrap();
    let frontmatter = skill_md_frontmatter(std::str::from_utf8(&skill_md.bytes).unwrap())
        .unwrap()
        .lines()
        .filter_map(|line| line.split_once(": "))
        .map(|(key, value)| (key.to_owned(), FrontmatterValue::Text(value.to_owned())))
        .collect();
    admit_skill_package(SkillPackageInput {
        directory_name: name.to_owned(),
        files,
        frontmatter: Some(frontmatter),
    })
    .unwrap()
}

pub(crate) fn skill_md(name: &str) -> SkillFileInput {
    SkillFileInput {
        path: SKILL_MD_PATH.to_owned(),
        bytes: format!("---\nname: {name}\ndescription: Use it for {name}.\n---\nBody\n")
            .into_bytes(),
    }
}

pub(crate) fn set(packages: Vec<SkillPackage>) -> Arc<SkillSet> {
    Arc::new(SkillSet::new(packages).unwrap())
}
