use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::*;

const VALID: &str = "---\nname: plot-notes\ndescription: Use it for plot notes.\n---\nBody\n";

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("storyos-skills-{}", Uuid::now_v7()));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn write(&self, path: &str, bytes: impl AsRef<[u8]>) -> &Self {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
        self
    }

    fn read(&self) -> Result<SkillSet, SkillDirectoryRefusal> {
        read_skill_directory(Some(&self.0))
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn package_refusal(package: &str, rule: SkillPackageRefusal) -> SkillDirectoryRefusal {
    SkillDirectoryRefusal::Package {
        package: package.to_owned(),
        rule,
    }
}

#[test]
fn the_fixture_directory_gives_the_two_stage_6_skills() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/skill-packages");
    let set = read_skill_directory(Some(&fixtures)).expect("valid fixture packages");
    let summary: Vec<_> = set
        .packages()
        .iter()
        .map(|package| {
            let files: Vec<_> = package
                .files
                .iter()
                .map(|file| (file.path.as_str(), file.readable))
                .collect();
            (
                package.name.as_str(),
                package.description.as_str(),
                files,
                package.package_digest.as_str(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (
                "dialogue-style-guide",
                "小说对白的写法规范。当作者要求写、改写或润色人物对白，或者要求检查一段对话是否自然时，使用这个技能。",
                vec![("SKILL.md", true), ("references/examples.md", true)],
                "sha256:e03399e0385ea904f5c37265661480113ef0125e3c7f837cf04875d5cbe8178b",
            ),
            (
                "internal-comms",
                "A set of resources to help me write all kinds of internal communications, using the formats that my company likes to use. Claude should use this skill whenever asked to write some sort of internal communications (status reports, leadership updates, 3P updates, company newsletters, FAQs, incident reports, project updates, etc.).",
                vec![
                    ("LICENSE.txt", true),
                    ("SKILL.md", true),
                    ("examples/3p-updates.md", true),
                    ("examples/company-newsletter.md", true),
                    ("examples/faq-answers.md", true),
                    ("examples/general-comms.md", true),
                ],
                "sha256:3ccc24f2e8e7c47369c7a2cf8456271c6338b8a83d1917756c3584b2466c67e9",
            ),
        ]
    );
}

#[test]
fn the_distribution_directory_gives_exactly_its_named_packages() {
    // ADR 0100 section 3: the release package copies this directory.
    let distribution = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skill-packages");
    let set = read_skill_directory(Some(&distribution)).expect("valid distribution packages");
    let names: Vec<_> = set
        .packages()
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    assert_eq!(names, ["conflict-check"]);
}

#[test]
fn no_setting_and_a_directory_with_no_package_give_an_empty_skill_set() {
    assert_eq!(read_skill_directory(None), Ok(SkillSet::default()));
    let directory = Directory::new();
    directory.write("README.md", "notes");
    fs::create_dir_all(directory.0.join(".git/objects")).unwrap();
    assert_eq!(directory.read(), Ok(SkillSet::default()));
}

#[test]
fn a_value_that_is_not_a_readable_directory_names_the_setting() {
    let directory = Directory::new();
    directory.write("file.txt", "x");
    for value in [directory.0.join("file.txt"), directory.0.join("missing")] {
        let refusal = read_skill_directory(Some(&value)).expect_err("not a directory");
        assert_eq!(refusal, SkillDirectoryRefusal::Unreadable { package: None });
        assert!(refusal.to_string().starts_with("STORYOS_SKILLS_DIR "));
    }
}

#[test]
fn the_reader_skips_desktop_metadata_files_in_a_package() {
    let directory = Directory::new();
    directory
        .write("plot-notes/SKILL.md", VALID)
        .write("plot-notes/.DS_Store", [0u8; 8])
        .write("plot-notes/references/.DS_Store", [0u8; 8])
        .write("plot-notes/references/.hidden.md", "kept");
    let set = directory.read().expect("valid package");
    let paths: Vec<_> = set.packages()[0]
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(paths, ["SKILL.md", "references/.hidden.md"]);
}

#[test]
fn a_symbolic_link_is_an_unsupported_entry() {
    let directory = Directory::new();
    directory.write("plot-notes/SKILL.md", VALID);
    std::os::unix::fs::symlink(
        directory.0.join("plot-notes/SKILL.md"),
        directory.0.join("plot-notes/link.md"),
    )
    .unwrap();
    let unsupported = |package: &str| SkillDirectoryRefusal::EntryUnsupported {
        package: package.to_owned(),
    };
    assert_eq!(directory.read(), Err(unsupported("plot-notes")));
    fs::remove_file(directory.0.join("plot-notes/link.md")).unwrap();
    std::os::unix::fs::symlink(directory.0.join("plot-notes"), directory.0.join("linked")).unwrap();
    assert_eq!(directory.read(), Err(unsupported("linked")));
}

#[test]
fn empty_nested_directories_do_not_count_and_a_file_too_deep_is_unsafe() {
    let directory = Directory::new();
    let deep = vec!["d"; 16].join("/");
    directory.write("plot-notes/SKILL.md", VALID);
    fs::create_dir_all(directory.0.join("plot-notes").join(&deep)).unwrap();
    assert_eq!(directory.read().map(|set| set.packages().len()), Ok(1));
    directory.write(&format!("plot-notes/{deep}/x.md"), "x");
    assert_eq!(
        directory.read(),
        Err(package_refusal(
            "plot-notes",
            SkillPackageRefusal::PathUnsafe
        ))
    );
}

#[test]
fn the_reader_stops_at_the_file_limit_and_at_the_size_limit() {
    let directory = Directory::new();
    directory.write("many/SKILL.md", VALID);
    for index in 0..SKILL_PACKAGE_FILE_LIMIT {
        directory.write(&format!("many/references/{index:02}.md"), "x");
    }
    directory.write("large/SKILL.md", VALID).write(
        "large/assets/large.bin",
        vec![0; SKILL_PACKAGE_BYTE_LIMIT as usize],
    );
    assert_eq!(
        directory.read(),
        Err(package_refusal(
            "large",
            SkillPackageRefusal::SizeLimitExceeded
        ))
    );
    fs::remove_dir_all(directory.0.join("large")).unwrap();
    assert_eq!(
        directory.read(),
        Err(package_refusal(
            "many",
            SkillPackageRefusal::FileLimitExceeded
        ))
    );
}

#[test]
fn block_quoted_and_plain_scalars_decode_to_the_description() {
    let directory = Directory::new();
    directory
        .write(
            "block/SKILL.md",
            "---\nname: block\ndescription: >-\n  Use it for\n  plot notes.\n---\n",
        )
        .write(
            "quoted/SKILL.md",
            "---\nname: \"quoted\"\ndescription: 'Use it: for ''plot'' notes.'\n---\n",
        )
        .write(
            "plain/SKILL.md",
            "---\nname: plain\ndescription: Use it for plot notes.\nmetadata:\n  author: StoryOS\n---\n",
        );
    let set = directory.read().expect("valid packages");
    let descriptions: Vec<_> = set
        .packages()
        .iter()
        .map(|package| (package.name.as_str(), package.description.as_str()))
        .collect();
    assert_eq!(
        descriptions,
        [
            ("block", "Use it for plot notes."),
            ("plain", "Use it for plot notes."),
            ("quoted", "Use it: for 'plot' notes."),
        ]
    );
}

#[test]
fn a_yaml_escape_that_decodes_to_u0000_refuses_the_package() {
    for (frontmatter, rule) in [
        (
            "name: a\ndescription: \"Use it for\\0plot notes.\"\n",
            SkillPackageRefusal::DescriptionInvalid,
        ),
        (
            "name: a\ndescription: Use it.\nlicense: \"Apache\\x00\"\n",
            SkillPackageRefusal::OptionalFieldInvalid,
        ),
        (
            "name: a\ndescription: Use it.\nmetadata:\n  author: \"Story\\0OS\"\n",
            SkillPackageRefusal::OptionalFieldInvalid,
        ),
    ] {
        let directory = Directory::new();
        directory.write("a/SKILL.md", format!("---\n{frontmatter}---\n"));
        assert_eq!(
            directory.read(),
            Err(package_refusal("a", rule)),
            "{frontmatter:?}"
        );
    }
}

#[test]
fn frontmatter_that_is_not_one_mapping_of_text_keys_is_invalid() {
    for frontmatter in [
        "name: a\nname: a\n",
        "- name\n",
        "name: [unclosed\n",
        "1: one\n",
        "",
    ] {
        let directory = Directory::new();
        directory.write("a/SKILL.md", format!("---\n{frontmatter}---\n"));
        assert_eq!(
            directory.read(),
            Err(package_refusal(
                "a",
                SkillPackageRefusal::FrontmatterInvalid
            )),
            "{frontmatter}"
        );
    }
}

#[test]
fn a_package_refusal_names_the_package_directory_and_the_rule() {
    let directory = Directory::new();
    directory.write("plot-ideas/SKILL.md", VALID);
    let refusal = directory.read().expect_err("the name differs");
    assert_eq!(
        refusal.to_string(),
        "STORYOS_SKILLS_DIR: the Skill package \"plot-ideas\" is refused: name_directory_mismatch"
    );
}

#[test]
fn a_directory_above_32_packages_is_refused() {
    let directory = Directory::new();
    for index in 0..=storyos_core::SKILL_SET_LIMIT {
        let name = format!("skill-{index:02}");
        directory.write(
            &format!("{name}/SKILL.md"),
            format!("---\nname: {name}\ndescription: One.\n---\n"),
        );
    }
    assert_eq!(
        directory.read(),
        Err(SkillDirectoryRefusal::SkillLimitExceeded)
    );
    fs::remove_dir_all(directory.0.join("skill-00")).unwrap();
    assert_eq!(directory.read().map(|set| set.packages().len()), Ok(32));
}
