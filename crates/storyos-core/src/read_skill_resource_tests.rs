use serde_json::json;

use super::{
    READ_SKILL_RESOURCE_TOOL_SPEC, SKILL_RESOURCE_READ_LIMIT, SkillResourceReadFacts,
    SkillResourceRefusal, admit_skill_resource_read,
};
use crate::{IntakeRefusal, SkillPackageReference, ToolIntake, host_tool_result};

fn reference(name: &str) -> SkillPackageReference {
    SkillPackageReference {
        name: name.to_owned(),
        package_digest: format!("sha256:{}", "d".repeat(64)),
    }
}

#[test]
fn the_intake_takes_a_skill_and_a_path_and_refuses_u0000() {
    let intake = |arguments| READ_SKILL_RESOURCE_TOOL_SPEC.intake(&arguments);
    assert_eq!(
        (
            intake(json!({"skill": "comms", "path": "examples/a.md"})),
            intake(json!({"skill": "comms", "path": "a\0.md"})),
            intake(json!({"skill": "comms"})),
            intake(json!({"skill": "comms", "path": "a.md", "reason": "x"})),
        ),
        (
            Ok(ToolIntake::ReadSkillResource {
                skill: "comms".to_owned(),
                path: "examples/a.md".to_owned(),
            }),
            Err(IntakeRefusal::UnstorableText),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
        )
    );
}

#[test]
fn a_read_is_decided_in_the_order_of_the_refusal_table() {
    let catalog = [reference("comms"), reference("dialogue")];
    let selected = ["dialogue".to_owned()];
    let loaded = ["comms".to_owned()];
    let text = "Use <this> & \"that\".".as_bytes();
    let long = "a ".repeat(32_001);
    let base = SkillResourceReadFacts {
        catalog: &catalog,
        selected: &selected,
        loaded: &loaded,
        settled_reads: SKILL_RESOURCE_READ_LIMIT - 1,
        file: Some(text),
    };
    let read = |facts: SkillResourceReadFacts<'_>, skill: &str, path: &str| {
        admit_skill_resource_read(&facts, skill, path)
    };
    let at_limit = SkillResourceReadFacts {
        settled_reads: SKILL_RESOURCE_READ_LIMIT,
        ..base
    };
    let no_file = SkillResourceReadFacts { file: None, ..base };

    assert_eq!(
        [
            read(base, "comms", "examples/a.md"),
            read(base, "dialogue", "SKILL.md"),
            // A Skill of the catalog that the Run did not load, and a name of no Skill. The
            // store gives a load of the same response as not loaded.
            read(
                SkillResourceReadFacts {
                    loaded: &[],
                    ..base
                },
                "comms",
                "a.md"
            ),
            read(base, "other", "a.md"),
            // The Skill comes first: a Skill that the Run does not have at the read limit.
            read(
                SkillResourceReadFacts {
                    loaded: &[],
                    ..at_limit
                },
                "comms",
                "a.md"
            ),
            read(at_limit, "comms", "examples/a.md"),
            // The limit comes before the path.
            read(at_limit, "comms", "/a.md"),
            read(no_file, "comms", "examples/b.md"),
            // A path that is not relative is not found, also when the store gives a file.
            read(base, "comms", "/examples/a.md"),
            read(base, "comms", "examples/../a.md"),
            read(base, "comms", "examples//a.md"),
            read(base, "comms", "examples/"),
            read(base, "comms", ""),
            read(base, "comms", "examples\\a.md"),
            read(
                SkillResourceReadFacts {
                    file: Some(&[0xff, 0xfe]),
                    ..base
                },
                "comms",
                "a"
            ),
            read(
                SkillResourceReadFacts {
                    file: Some(b"a\0b"),
                    ..base
                },
                "comms",
                "a"
            ),
            read(
                SkillResourceReadFacts {
                    file: Some(long.as_bytes()),
                    ..base
                },
                "comms",
                "a.md"
            ),
        ],
        [
            Ok(host_tool_result("Use <this> & \"that\".")),
            Ok(host_tool_result("Use <this> & \"that\".")),
            Err(SkillResourceRefusal::NotLoaded),
            Err(SkillResourceRefusal::NotLoaded),
            Err(SkillResourceRefusal::NotLoaded),
            Err(SkillResourceRefusal::ReadLimit),
            Err(SkillResourceRefusal::ReadLimit),
            Err(SkillResourceRefusal::NotFound),
            Err(SkillResourceRefusal::NotFound),
            Err(SkillResourceRefusal::NotFound),
            Err(SkillResourceRefusal::NotFound),
            Err(SkillResourceRefusal::NotFound),
            Err(SkillResourceRefusal::NotFound),
            Err(SkillResourceRefusal::NotFound),
            Err(SkillResourceRefusal::NotReadable),
            Err(SkillResourceRefusal::NotReadable),
            Err(SkillResourceRefusal::NotReadable),
        ]
    );
}
