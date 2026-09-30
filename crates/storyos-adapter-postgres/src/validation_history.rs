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
