use super::*;
use sha2::{Digest as _, Sha256};

const SKILL_MD: &[u8] = b"---\nname: plot-notes\ndescription: Use it for plot notes.\n---\nBody\n";

fn text(value: &str) -> FrontmatterValue {
    FrontmatterValue::Text(value.to_owned())
}

fn standard_fields() -> Vec<(String, FrontmatterValue)> {
    vec![
        ("name".to_owned(), text("plot-notes")),
        ("description".to_owned(), text("Use it for plot notes.")),
    ]
}

fn file(path: &str, bytes: &[u8]) -> SkillFileInput {
    SkillFileInput {
        path: path.to_owned(),
        bytes: bytes.to_vec(),
    }
}

fn input(files: Vec<SkillFileInput>) -> SkillPackageInput {
    SkillPackageInput {
        directory_name: "plot-notes".to_owned(),
        files,
        frontmatter: Some(standard_fields()),
    }
}

fn with_field(key: &str, value: FrontmatterValue) -> SkillPackageInput {
    let mut package = input(vec![file(SKILL_MD_PATH, SKILL_MD)]);
    let fields = package.frontmatter.as_mut().expect("standard fields");
    fields.retain(|(name, _)| name != key);
    fields.push((key.to_owned(), value));
    package
}

fn refusal(input: SkillPackageInput) -> SkillPackageRefusal {
    admit_skill_package(input).expect_err("the package should be refused")
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn a_package_gives_each_file_record_in_path_order_and_the_inventory_digest() {
    let script = b"echo plot\n";
    let package = admit_skill_package(input(vec![
        file("scripts/run.sh", script),
        file(SKILL_MD_PATH, SKILL_MD),
        file("assets/cover.bin", &[0xff, 0xfe]),
    ]))
    .expect("valid package");
    let record = |path: &str, bytes: &[u8], tokens: Option<u64>| SkillFile {
        path: path.to_owned(),
        bytes: bytes.to_vec(),
        byte_length: bytes.len() as u64,
        sha256: sha256(bytes),
        tokens,
        readable: tokens.is_some(),
    };
    let files = vec![
        record("SKILL.md", SKILL_MD, Some(66)),
        record("assets/cover.bin", &[0xff, 0xfe], None),
        record("scripts/run.sh", script, Some(10)),
    ];
    let inventory = format!(
        r#"[{{"byte_length":66,"path":"SKILL.md","sha256":"{}"}},{{"byte_length":2,"path":"assets/cover.bin","sha256":"{}"}},{{"byte_length":10,"path":"scripts/run.sh","sha256":"{}"}}]"#,
        files[0].sha256, files[1].sha256, files[2].sha256
    );
    assert_eq!(
        package,
        SkillPackage {
            name: "plot-notes".to_owned(),
            description: "Use it for plot notes.".to_owned(),
            license: None,
            compatibility: None,
            metadata: Vec::new(),
            allowed_tools: None,
            files,
            package_digest: format!("sha256:{}", sha256(inventory.as_bytes())),
        }
    );
}

#[test]
fn the_digest_depends_only_on_the_files() {
    let digest = |files| admit_skill_package(input(files)).map(|package| package.package_digest);
    let first = digest(vec![file(SKILL_MD_PATH, SKILL_MD), file("a.md", b"one")]);
    let same = digest(vec![file("a.md", b"one"), file(SKILL_MD_PATH, SKILL_MD)]);
    let changed = digest(vec![file(SKILL_MD_PATH, SKILL_MD), file("a.md", b"onf")]);
    assert_eq!(first, same);
    assert_ne!(first, changed);
}

#[test]
fn a_file_is_readable_only_when_it_is_utf8_text_within_the_token_limit() {
    let limit = CONTEXT_ITEM_TOKEN_LIMIT as usize;
    let package = admit_skill_package(input(vec![
        file(SKILL_MD_PATH, SKILL_MD),
        file("at-limit.md", "a".repeat(limit).as_bytes()),
        file("above-limit.md", "a".repeat(limit + 1).as_bytes()),
        file("binary.bin", &[0xc3, 0x28]),
    ]))
    .expect("an unreadable file does not refuse the package");
    let marks: Vec<_> = package
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.tokens, file.readable))
        .collect();
    assert_eq!(
        marks,
        [
            ("SKILL.md", Some(66), true),
            ("above-limit.md", Some(32_001), false),
            ("at-limit.md", Some(32_000), true),
            ("binary.bin", None, false),
        ]
    );
}

#[test]
fn the_file_count_and_the_byte_sum_refuse_above_their_limits() {
    let files = |count: usize| {
        let mut files = vec![file(SKILL_MD_PATH, SKILL_MD)];
        files.extend((1..count).map(|index| file(&format!("references/{index}.md"), b"x")));
        files
    };
    assert!(admit_skill_package(input(files(SKILL_PACKAGE_FILE_LIMIT))).is_ok());
    assert_eq!(
        refusal(input(files(SKILL_PACKAGE_FILE_LIMIT + 1))),
        SkillPackageRefusal::FileLimitExceeded
    );
    let padding = |extra: usize| {
        let length = SKILL_PACKAGE_BYTE_LIMIT as usize - SKILL_MD.len() + extra;
        input(vec![
            file(SKILL_MD_PATH, SKILL_MD),
            file("assets/padding.bin", &vec![0; length]),
        ])
    };
    assert!(admit_skill_package(padding(0)).is_ok());
    assert_eq!(refusal(padding(1)), SkillPackageRefusal::SizeLimitExceeded);
}

#[test]
fn an_unsafe_path_or_a_path_collision_refuses_the_package() {
    for path in [
        "../escape.md",
        "/absolute.md",
        "references\\x.md",
        "r\u{e9}sum\u{e9}.md",
    ] {
        let package = input(vec![file(SKILL_MD_PATH, SKILL_MD), file(path, b"x")]);
        assert_eq!(refusal(package), SkillPackageRefusal::PathUnsafe, "{path}");
    }
    let collision = input(vec![
        file(SKILL_MD_PATH, SKILL_MD),
        file("Notes.md", b"x"),
        file("notes.md", b"y"),
    ]);
    assert_eq!(refusal(collision), SkillPackageRefusal::PathUnsafe);
}

#[test]
fn skill_md_is_text_of_at_most_the_token_limit_directly_in_the_package() {
    assert_eq!(
        refusal(input(Vec::new())),
        SkillPackageRefusal::SkillMdMissing
    );
    assert_eq!(
        refusal(input(vec![file("references/SKILL.md", SKILL_MD)])),
        SkillPackageRefusal::SkillMdMissing
    );
    assert_eq!(
        refusal(input(vec![file(SKILL_MD_PATH, &[b'-', 0xc3, 0x28])])),
        SkillPackageRefusal::SkillMdNotText
    );
    let long = |extra: usize| {
        let mut bytes = SKILL_MD.to_vec();
        bytes.resize(CONTEXT_ITEM_TOKEN_LIMIT as usize + extra, b'x');
        input(vec![file(SKILL_MD_PATH, &bytes)])
    };
    assert!(admit_skill_package(long(0)).is_ok());
    assert_eq!(refusal(long(1)), SkillPackageRefusal::SkillMdTooLong);
}

#[test]
fn skill_md_starts_with_one_mapping_between_two_marker_lines() {
    assert_eq!(
        skill_md_frontmatter("---\r\nname: a\r\n---\r\nBody"),
        Some("name: a\r\n")
    );
    for skill_md in [
        "name: plot-notes\n---\nBody\n",
        " ---\nname: plot-notes\n---\n",
        "---\nname: plot-notes\n",
    ] {
        let package = input(vec![file(SKILL_MD_PATH, skill_md.as_bytes())]);
        assert_eq!(
            refusal(package),
            SkillPackageRefusal::FrontmatterInvalid,
            "{skill_md}"
        );
    }
    let mut undecoded = input(vec![file(SKILL_MD_PATH, SKILL_MD)]);
    undecoded.frontmatter = None;
    assert_eq!(refusal(undecoded), SkillPackageRefusal::FrontmatterInvalid);
}

#[test]
fn the_name_follows_the_format_and_matches_the_directory() {
    let longest = "a".repeat(64);
    let mut at_limit = with_field("name", text(&longest));
    at_limit.directory_name = longest.clone();
    assert_eq!(
        admit_skill_package(at_limit).map(|package| package.name),
        Ok(longest)
    );
    for name in [
        "",
        "Plot",
        "-plot",
        "plot-",
        "plot--notes",
        "plot_notes",
        &"a".repeat(65),
    ] {
        assert_eq!(
            refusal(with_field("name", text(name))),
            SkillPackageRefusal::NameInvalid,
            "{name}"
        );
    }
    assert_eq!(
        refusal(with_field("name", FrontmatterValue::Other)),
        SkillPackageRefusal::NameInvalid
    );
    assert_eq!(
        refusal(with_field("name", text("plot-ideas"))),
        SkillPackageRefusal::NameDirectoryMismatch
    );
}

#[test]
fn the_description_is_text_of_1_to_1024_characters() {
    let longest = "\u{4e00}".repeat(1024);
    assert_eq!(
        admit_skill_package(with_field("description", text(&longest)))
            .map(|package| package.description),
        Ok(longest)
    );
    for value in [
        text(""),
        text(&"a".repeat(1025)),
        FrontmatterValue::Other,
        FrontmatterValue::Mapping(Vec::new()),
    ] {
        assert_eq!(
            refusal(with_field("description", value)),
            SkillPackageRefusal::DescriptionInvalid
        );
    }
    let mut missing = input(vec![file(SKILL_MD_PATH, SKILL_MD)]);
    missing.frontmatter = Some(vec![("name".to_owned(), text("plot-notes"))]);
    assert_eq!(refusal(missing), SkillPackageRefusal::DescriptionInvalid);
}

#[test]
fn the_optional_fields_keep_their_text_and_an_unknown_key_is_ignored() {
    let mut package = input(vec![file(SKILL_MD_PATH, SKILL_MD)]);
    package.frontmatter.as_mut().expect("fields").extend([
        ("license".to_owned(), text("Apache-2.0")),
        ("compatibility".to_owned(), text(&"c".repeat(500))),
        ("allowed-tools".to_owned(), text("Bash(git:*) Read")),
        (
            "metadata".to_owned(),
            FrontmatterValue::Mapping(vec![("author".to_owned(), text("StoryOS"))]),
        ),
        ("x-unknown".to_owned(), FrontmatterValue::Other),
    ]);
    let package = admit_skill_package(package).expect("valid package");
    assert_eq!(
        (
            package.license,
            package.compatibility,
            package.allowed_tools,
            package.metadata
        ),
        (
            Some("Apache-2.0".to_owned()),
            Some("c".repeat(500)),
            Some("Bash(git:*) Read".to_owned()),
            vec![("author".to_owned(), "StoryOS".to_owned())],
        )
    );
}

#[test]
fn an_optional_field_of_the_wrong_form_refuses_the_package() {
    let invalid = [
        ("license", FrontmatterValue::Other),
        ("compatibility", text(&"c".repeat(501))),
        ("allowed-tools", FrontmatterValue::Mapping(Vec::new())),
        ("metadata", text("author")),
        (
            "metadata",
            FrontmatterValue::Mapping(vec![("version".to_owned(), FrontmatterValue::Other)]),
        ),
    ];
    for (key, value) in invalid {
        assert_eq!(
            refusal(with_field(key, value)),
            SkillPackageRefusal::OptionalFieldInvalid,
            "{key}"
        );
    }
}

#[test]
fn a_skill_set_has_at_most_32_packages_in_the_order_of_their_names() {
    let package = |index: usize| {
        let name = format!("skill-{index:02}");
        let mut package = with_field("name", text(&name));
        package.directory_name = name;
        admit_skill_package(package).expect("valid package")
    };
    let packages: Vec<_> = (0..SKILL_SET_LIMIT).rev().map(package).collect();
    let names: Vec<_> = SkillSet::new(packages)
        .expect("32 packages")
        .packages()
        .iter()
        .map(|package| package.name.clone())
        .collect();
    let expected: Vec<_> = (0..SKILL_SET_LIMIT)
        .map(|index| format!("skill-{index:02}"))
        .collect();
    assert_eq!(names, expected);
    assert_eq!(
        SkillSet::new((0..=SKILL_SET_LIMIT).map(package).collect()),
        Err(SkillLimitExceeded)
    );
}
