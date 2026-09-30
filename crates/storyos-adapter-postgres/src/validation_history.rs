use storyos_application::ProjectScope;

pub(super) async fn unavailable_revisions(
    client: &impl tokio_postgres::GenericClient,
    scope: &ProjectScope,
) -> Result<Vec<String>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT DISTINCT acceptance.proposal_revision_id::text
               FROM storyos.acceptance_receipts AS acceptance
               JOIN storyos.domain_receipts AS domain
                 ON (domain.owner_user_id, domain.project_id, domain.receipt_id) =
                    (acceptance.owner_user_id, acceptance.project_id, acceptance.acceptance_receipt_id)
               LEFT JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id) =
                    (acceptance.owner_user_id, acceptance.project_id, acceptance.proposal_id, acceptance.proposal_revision_id)
               LEFT JOIN storyos.validation_receipts AS validation
                 ON (validation.owner_user_id, validation.project_id, validation.validation_receipt_id,
                     validation.proposal_id, validation.proposal_revision_id) =
                    (acceptance.owner_user_id, acceptance.project_id, acceptance.validation_receipt_id,
                     acceptance.proposal_id, acceptance.proposal_revision_id)
              WHERE acceptance.owner_user_id = $1::text::uuid
                AND acceptance.project_id = $2::text::uuid AND acceptance.result = 'authoritative_applied'
                AND (revision.revision_id IS NULL OR validation.validation_receipt_id IS NULL
                  OR validation.reservation_state <> 'unresolved'
                  OR domain.prior_heads <> ARRAY[revision.base_authoritative_revision_id]
                  OR domain.prior_heads <> ARRAY[validation.base_authoritative_revision_id])",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

pub(super) fn archive_history_available(
    load_rows: impl Fn(&str) -> Option<Vec<serde_json::Value>>,
) -> bool {
    let available = || -> Option<()> {
        let key = |row: &serde_json::Value, field: &str| {
            Some((
                row.get("owner_user_id")?.as_str()?.to_owned(),
                row.get("project_id")?.as_str()?.to_owned(),
                row.get(field)?.as_str()?.to_owned(),
            ))
        };
        let index = |rows: Vec<serde_json::Value>, field: &str| {
            let count = rows.len();
            let indexed = rows
                .into_iter()
                .map(|row| Some((key(&row, field)?, row)))
                .collect::<Option<std::collections::BTreeMap<_, _>>>()?;
            (indexed.len() == count).then_some(indexed)
        };
        let revisions = index(load_rows("proposal_revisions")?, "revision_id")?;
        let validations = index(load_rows("validation_receipts")?, "validation_receipt_id")?;
        let domains = index(load_rows("domain_receipts")?, "receipt_id")?;
        for acceptance in load_rows("acceptance_receipts")? {
            if acceptance.get("result")?.as_str()? != "authoritative_applied" {
                continue;
            }
            let revision = revisions.get(&key(&acceptance, "proposal_revision_id")?)?;
            let validation = validations.get(&key(&acceptance, "validation_receipt_id")?)?;
            let domain = domains.get(&key(&acceptance, "acceptance_receipt_id")?)?;
            let base = revision.get("base_authoritative_revision_id")?.as_str()?;
            if domain.get("prior_heads")? != &serde_json::json!([base])
                || validation.get("base_authoritative_revision_id")?.as_str()? != base
                || validation.get("reservation_state")?.as_str()? != "unresolved"
                || validation.get("proposal_revision_id")?
                    != acceptance.get("proposal_revision_id")?
                || validation.get("proposal_id")? != acceptance.get("proposal_id")?
            {
                return None;
            }
        }
        Some(())
    };
    available().is_some()
}
