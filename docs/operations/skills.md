# Provide the Skills of a Deployment

The operator of a deployment provides the Skills ([ADR 0089](../adr/0089-deliver-standard-skills-from-the-deployment-and-load-them-with-host-tools.md)). Each Skill is one standard SkillPackage of the [Agent Skills format](https://agentskills.io/specification).

## The setting

Set `STORYOS_SKILLS_DIR` in the environment of `storyos-server` to the absolute path of one directory. Only the Server reads it. The Worker does not read it. When the setting is absent, the deployment has no Skill.

## The SkillPackages of the StoryOS distribution

StoryOS comes with standard SkillPackages for the work of a novel author ([ADR 0100](../adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md) sections 2 and 3). Each package is one `SKILL.md` with no script.

| Package | What the assistant does |
| --- | --- |
| `conflict-check` | It compares a text with the earlier Chapters and Agent Memory, and it reports each conflict with its source. It changes no text. |
| `local-revision` | It finds the most obvious machine-voice places of a text (at most 5, or the number that the author gives), and it sends one Proposal with one small change for each place. The author accepts, changes, or refuses each change. All other text stays as it is. |
| `scene-brief` | When the author asks to agree on a new scene first, it gives a brief of 4 to 6 lines and one question, and it writes the scene only when a later message of the author tells it to write. A request that gives each item of the brief starts the writing at once. |

- **In the repository:** the directory `skill-packages/` at the root.
- **In the release package:** the directory `skill-packages/`, beside `web/`. `python3 scripts/package-release.py` copies the tracked files of `skill-packages/` into it with the same bytes.
- **To use them:** set `STORYOS_SKILLS_DIR` to the absolute path of the directory `skill-packages/` of the release package, and start the Server. The Server has no default Skill directory: with no setting, the deployment has no Skill.
- **To use your own packages with them:** make one directory, copy each package directory of `skill-packages/` and each of your own package directories into it, and set `STORYOS_SKILLS_DIR` to that directory. A package name is unique in one directory, so give your own package a name that is not the name of a package of the distribution.

## The directory layout

```text
<STORYOS_SKILLS_DIR>/
  dialogue-style-guide/
    SKILL.md
    references/examples.md
  internal-comms/
    SKILL.md
    LICENSE.txt
    examples/...
```

- Each child directory is one package. Its name is the Skill name.
- `SKILL.md` is directly in the package directory. It starts with one YAML mapping between two `---` lines. `name` is the same as the directory name: 1 to 64 characters of `a-z`, `0-9`, and `-`, with no hyphen at the start or the end and no double hyphen. `description` is text of 1 to 1024 characters. `license`, `compatibility` (at most 500 characters), `metadata` (a mapping from text to text), and `allowed-tools` are optional. `allowed-tools` grants nothing.
- The `SKILL.md` text, and each text of its YAML mapping after the decoding, has no character U+0000. A YAML escape such as `"\0"` gives this character. Such a package stops the start with the rule `skill_md_not_text`, `description_invalid`, or `optional_field_invalid`.
- The Server skips a top-level file, a top-level directory whose name starts with `.`, and a `.DS_Store` file in a package.
- A symbolic link, and each entry that is not a file or a directory, stops the start. Copy the files into the directory.
- Each file path is ASCII and has no `.` or `..` segment. Two paths that differ only in letter case stop the start.
- A file under `scripts/` is a file. StoryOS does not run it.

## The limits

| Item | Limit |
| --- | --- |
| One package | 1 to 64 files, and at most 1,048,576 bytes |
| `SKILL.md` | UTF-8 text of at most 32,000 tokens, with no U+0000 |
| Skills of one deployment | 32 |
| The catalog text of one deployment | 32,000 tokens |

The catalog text has one line for each Skill, with its name and its description. A set of Skills whose catalog text is above the limit stops the start with the rule `skill_catalog_limit_exceeded`.

A resource file that is not UTF-8 text, that has the character U+0000, or that is above 32,000 tokens, stays in the package, but the Agent cannot read it.

## The start

The Server reads the directory one time, before it loads the Web assets and binds. A package that fails a rule stops the start. The process exits with code 1 and prints no `STORYOS_SERVER_URL=` line. The error names the package directory and the rule, for example `the Skill package "plot-notes" is refused: name_directory_mismatch`.

A change to the directory has no effect until the next start of the Server. After a change, start the Server again.

## The Snapshots of a Project

Each `createAgentRun` gives the Skills of the Server to the Project. In the transaction of the Run, the Project gets a SkillPackage Snapshot of each Skill whose package digest it does not have: the complete package with the exact bytes of each file. The Run pins its catalog: the name and the package digest of each Skill. Each later step of the Run reads only these database rows, never the directory. An earlier Run keeps its catalog and its Snapshots after a change, and the Project Export Archive carries them.
