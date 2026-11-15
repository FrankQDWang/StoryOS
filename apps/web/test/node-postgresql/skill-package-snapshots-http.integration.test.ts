// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { appendFileSync, cpSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { TestServer, admitRun, archivedTables, createStory, drainLeftoverWork, restartServer, startServer,
} from "../support/http-harness.ts";
import type { TestStory } from "../support/http-harness.ts";
import { queryStoryOSPostgres, stopStoryOSServer } from "../support/node-integration.ts";

// ADR 0089 section 2: createAgentRun stores the Snapshots of the Skill set of the Server and pins the catalog.
const FIXTURES = fileURLToPath(new URL("../../../../fixtures/skill-packages", import.meta.url));
const DIALOGUE = "dialogue-style-guide|sha256:e03399e0385ea904f5c37265661480113ef0125e3c7f837cf04875d5cbe8178b";
const COMMS = "internal-comms|sha256:3ccc24f2e8e7c47369c7a2cf8456271c6338b8a83d1917756c3584b2466c67e9";
const SKILL_TABLES = ["skill_package_snapshots", "skill_package_snapshot_files", "agent_run_skill_catalog_entries"];

/** The name and the package digest of each catalog entry of one Run. */
function catalog(runId: string): Promise<string> {
  return queryStoryOSPostgres(`
    SELECT skill_name || '|' || package_digest FROM storyos.agent_run_skill_catalog_entries
     WHERE run_id = '${runId}'::uuid ORDER BY skill_name COLLATE "C"`);
}

/** Each Snapshot of the Project: its reference, its file count, its stored file rows, and its SKILL.md bytes. */
function snapshots(projectId: string): Promise<string> {
  return queryStoryOSPostgres(`
    SELECT snapshot.skill_name || '|' || snapshot.package_digest || '|' || snapshot.file_count || '|'
           || (SELECT count(*) FROM storyos.skill_package_snapshot_files AS file
                WHERE (file.project_id, file.package_digest) = (snapshot.project_id, snapshot.package_digest))
           || '|' || (SELECT encode(file.bytes, 'hex') FROM storyos.skill_package_snapshot_files AS file
                       WHERE (file.project_id, file.package_digest, file.path)
                           = (snapshot.project_id, snapshot.package_digest, 'SKILL.md'))
      FROM storyos.skill_package_snapshots AS snapshot
     WHERE snapshot.project_id = '${projectId}'::uuid
     ORDER BY snapshot.skill_name COLLATE "C", snapshot.package_digest COLLATE "C"`);
}

function snapshot(reference: string, files: number, directory: string): string {
  const name = reference.split("|")[0] ?? "";
  return `${reference}|${files}|${files}|${readFileSync(join(directory, name, "SKILL.md")).toString("hex")}`;
}

async function admit(story: TestStory): Promise<string> {
  const created = await admitRun(story.baseUrl,
    { projectId: story.projectId, fetchImpl: story.fetchImpl, chapterId: story.chapterIds[0] ?? "" },
    story.key(), "Revise this passage: keep the voice.");
  if (created.effect.kind !== "admitted") throw new Error("expected an admitted Run");
  return created.effect.run_id;
}

test("each createAgentRun stores the Snapshots of the Server Skill set and pins the catalog of the Run", async () => {
  await drainLeftoverWork();
  const changed = mkdtempSync(join(tmpdir(), "storyos-skills-"));
  cpSync(join(FIXTURES, "dialogue-style-guide"), join(changed, "dialogue-style-guide"), { recursive: true });
  appendFileSync(join(changed, "dialogue-style-guide", "SKILL.md"), "\n5. 每句对白只有一个说话人。\n");
  let started = await startServer({ extraEnv: { STORYOS_SKILLS_DIR: FIXTURES } });
  try {
    const server = TestServer.at(started.baseUrl);
    const story = await createStory(server, { title: "Skill Novel", assistance: true });
    const first = await admit(story);
    const firstSnapshots = [snapshot(DIALOGUE, 2, FIXTURES), snapshot(COMMS, 6, FIXTURES)];
    assert.equal(await snapshots(story.projectId), firstSnapshots.join("\n"));
    assert.equal(await catalog(first), [DIALOGUE, COMMS].join("\n"));

    started = await restartServer(started, { extraEnv: { STORYOS_SKILLS_DIR: changed } });
    const second = await admit(story);
    const changedReference = await catalog(second);
    assert.match(changedReference, /^dialogue-style-guide\|sha256:[0-9a-f]{64}$/);
    assert.notEqual(changedReference, DIALOGUE);
    assert.equal(await catalog(first), [DIALOGUE, COMMS].join("\n"));
    assert.equal(await snapshots(story.projectId),
      [...firstSnapshots, snapshot(changedReference, 2, changed)].sort().join("\n"));

    const archived = await archivedTables(story);
    const references = (table: string) => (archived(table) as { skill_name: string; package_digest: string }[])
      .map((row) => `${row.skill_name}|${row.package_digest}`).sort();
    assert.deepEqual(references("skill_package_snapshots"), [DIALOGUE, changedReference, COMMS].sort());
    assert.deepEqual(references("agent_run_skill_catalog_entries"), [DIALOGUE, changedReference, COMMS].sort());
    assert.equal(archived("skill_package_snapshot_files").length, 10);

    started = await restartServer(started, {});
    const plain = await createStory(server, { title: "Plain Novel", assistance: true });
    await admit(plain);
    assert.equal(await queryStoryOSPostgres(`SELECT ${SKILL_TABLES.map((table) =>
      `(SELECT count(*) FROM storyos.${table} WHERE project_id = '${plain.projectId}'::uuid)`).join(" + ")}`), "0");
  } finally {
    await stopStoryOSServer(started.server);
    rmSync(changed, { recursive: true, force: true });
  }
  await drainLeftoverWork();
});
