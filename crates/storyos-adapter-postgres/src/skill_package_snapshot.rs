//! The SkillPackage Snapshots of a Project and the Skill catalog of each Run (ADR 0089 section
//! 2). `createAgentRun` writes them in the transaction of the Run row. A Snapshot is immutable and
//! content-addressed by its package digest, and the runtime role can only read and insert them.

use std::collections::BTreeSet;

use crate::statement_cache::Client;
use storyos_application::{ProjectCommandEnvelope, ProjectCommandError, ProjectScope};
use storyos_core::{
    SKILL_MD_PATH, SkillCatalogEntry, SkillInstructionSource, SkillPackage, SkillPackageReference,
    SkillRequirement, SkillSet, plan_skill_catalog,
};

use crate::command_sequence::unavailable;

/// Writes the Snapshot of each package of `offered` whose digest the Project does not have, and
/// pins the reference of each package of `offered` as the Skill catalog of `run_id`.
pub(crate) async fn pin_skill_catalog(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    run_id: &str,
    offered: &SkillSet,
) -> Result<(), ProjectCommandError> {
    if offered.packages().is_empty() {
        return Ok(());
    }
    let scope = &envelope.project_scope;
    let stored = stored_digests(client, scope, offered).await?;
    let plan = plan_skill_catalog(offered, &stored);
    for package in plan.snapshots {
        insert_snapshot(client, envelope, package).await?;
    }
    insert_catalog(client, scope, run_id, &plan.catalog).await
}

async fn stored_digests(
    client: &Client,
    scope: &ProjectScope,
    offered: &SkillSet,
) -> Result<BTreeSet<String>, ProjectCommandError> {
    let digests: Vec<&str> = offered
        .packages()
        .iter()
        .map(|package| package.package_digest.as_str())
        .collect();
    let rows = client
        .query(
            "SELECT package_digest
               FROM storyos.skill_package_snapshots
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND package_digest = ANY($3::text[])",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &digests,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(rows.iter().map(|row| row.get(/*idx*/ 0)).collect())
}

/// Writes the Snapshot row of `package` and one row for each of its files, with the exact bytes.
async fn insert_snapshot(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    package: &SkillPackage,
) -> Result<(), ProjectCommandError> {
    let files = &package.files;
    let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    let bytes: Vec<&[u8]> = files.iter().map(|file| file.bytes.as_slice()).collect();
    let lengths = files
        .iter()
        .map(|file| i32::try_from(file.byte_length))
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    let sha256: Vec<&str> = files.iter().map(|file| file.sha256.as_str()).collect();
    let tokens = files
        .iter()
        .map(|file| file.tokens.map(i64::try_from).transpose())
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    let readable: Vec<bool> = files.iter().map(|file| file.readable).collect();
    let file_count = i32::try_from(files.len()).map_err(unavailable)?;
    let byte_count: i32 = lengths.iter().sum();
    client
        .execute(
            "WITH snapshot AS (
               INSERT INTO storyos.skill_package_snapshots
                 (owner_user_id, project_id, package_digest, skill_name, description, file_count,
                  byte_count, receipt_id)
               VALUES ($1::text::uuid, $2::text::uuid, $3, $4, $5, $6, $7, $8::text::uuid)
             )
             INSERT INTO storyos.skill_package_snapshot_files
               (owner_user_id, project_id, package_digest, path, bytes, byte_length, sha256,
                tokens, readable)
             SELECT $1::text::uuid, $2::text::uuid, $3, file.path, file.bytes, file.byte_length,
                    file.sha256, file.tokens, file.readable
               FROM unnest($9::text[], $10::bytea[], $11::int[], $12::text[], $13::bigint[],
                           $14::bool[])
                    AS file(path, bytes, byte_length, sha256, tokens, readable)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &package.package_digest,
                &package.name,
                &package.description,
                &file_count,
                &byte_count,
                &envelope.ids.receipt_id,
                &paths,
                &bytes,
                &lengths,
                &sha256,
                &tokens,
                &readable,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

async fn insert_catalog(
    client: &Client,
    scope: &ProjectScope,
    run_id: &str,
    catalog: &[SkillPackageReference],
) -> Result<(), ProjectCommandError> {
    let names: Vec<&str> = catalog.iter().map(|entry| entry.name.as_str()).collect();
    let digests: Vec<&str> = catalog
        .iter()
        .map(|entry| entry.package_digest.as_str())
        .collect();
    client
        .execute(
            "INSERT INTO storyos.agent_run_skill_catalog_entries
               (owner_user_id, project_id, run_id, skill_name, package_digest)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, entry.skill_name,
                    entry.package_digest
               FROM unnest($4::text[], $5::text[]) AS entry(skill_name, package_digest)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
                &names,
                &digests,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

/// The `SKILL.md` bytes of the Snapshot of each requirement, in requirement order. The read
/// takes only Snapshot rows of the Project, never a directory (ADR 0089 section 2).
pub(crate) async fn skill_instruction_sources(
    client: &Client,
    scope: &ProjectScope,
    requirements: &[SkillRequirement],
) -> Result<Vec<SkillInstructionSource>, tokio_postgres::Error> {
    let mut sources = Vec::with_capacity(requirements.len());
    for requirement in requirements {
        let reference = SkillPackageReference {
            name: requirement.skill_name.clone(),
            package_digest: requirement.package_digest.clone(),
        };
        sources.push(SkillInstructionSource {
            requirement: requirement.clone(),
            skill_md: skill_md(client, scope, &reference).await?,
        });
    }
    Ok(sources)
}

/// The `SKILL.md` bytes of the Snapshot of `reference` in the Project, or `None` when the
/// Snapshot has no such file. The read takes only Snapshot rows (ADR 0089 section 2).
pub(crate) async fn skill_md(
    client: &Client,
    scope: &ProjectScope,
    reference: &SkillPackageReference,
) -> Result<Option<Vec<u8>>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT file.bytes
               FROM storyos.skill_package_snapshots AS snapshot
               JOIN storyos.skill_package_snapshot_files AS file
                 ON (file.owner_user_id, file.project_id, file.package_digest) =
                    (snapshot.owner_user_id, snapshot.project_id, snapshot.package_digest)
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
                AND snapshot.package_digest = $3 AND snapshot.skill_name = $4
                AND file.path = $5",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &reference.package_digest,
                &reference.name,
                &SKILL_MD_PATH,
            ],
        )
        .await?;
    Ok(row.map(|row| row.get(/*idx*/ 0)))
}

/// The pinned catalog of `run_id`, with the description of the Snapshot of each entry, in the
/// order of the Skill names. A Run with no catalog rows gives an empty list.
pub(crate) async fn run_skill_catalog(
    client: &Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Vec<SkillCatalogEntry>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT entry.skill_name, entry.package_digest, snapshot.description
               FROM storyos.agent_run_skill_catalog_entries AS entry
               JOIN storyos.skill_package_snapshots AS snapshot
                 ON (snapshot.owner_user_id, snapshot.project_id, snapshot.package_digest,
                     snapshot.skill_name) =
                    (entry.owner_user_id, entry.project_id, entry.package_digest,
                     entry.skill_name)
              WHERE entry.owner_user_id = $1::text::uuid AND entry.project_id = $2::text::uuid
                AND entry.run_id = $3::text::uuid
              ORDER BY entry.skill_name COLLATE \"C\"",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|row| SkillCatalogEntry {
            reference: SkillPackageReference {
                name: row.get(/*idx*/ 0),
                package_digest: row.get(/*idx*/ 1),
            },
            description: row.get(/*idx*/ 2),
        })
        .collect())
}

#[cfg(test)]
#[path = "skill_instruction_context_tests.rs"]
mod tests;
