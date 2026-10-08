//! The Proposal, Operation, and Chapter facts that one Acceptance classifies.

use storyos_application::{AcceptProposalInput, ProjectCommandError, ProjectScope};

use crate::command_sequence::unavailable;

pub(super) struct LoadedProposal {
    pub(super) current_revision_id: String,
    pub(super) generation: String,
    pub(super) closure: String,
    pub(super) validation_current: bool,
    pub(super) candidate_text: String,
    pub(super) chapter_id: String,
    pub(super) operations: Vec<LoadedOperation>,
    pub(super) bundle_policy: String,
    pub(super) receipt_id: Option<String>,
    pub(super) receipt_result: Option<String>,
    pub(super) receipt_revision_id: Option<String>,
    pub(super) receipt_candidate_text: Option<String>,
    pub(super) current_head_revision_id: Option<String>,
    pub(super) validated_target_matches_head: bool,
    kind: String,
    chapter_blocks: Vec<storyos_core::ManuscriptBlock>,
    manuscript_block_id: String,
    inline_from: Option<u32>,
    inline_to: Option<u32>,
    inline_digest: Option<String>,
    pub(super) inline_base_slice_matches: bool,
}

pub(super) struct LoadedOperation {
    pub(super) operation_id: String,
    manuscript_block_id: String,
    pub(super) resolution: String,
    candidate_text: String,
    pub(super) predecessor_operation_ids: Vec<String>,
}

impl LoadedProposal {
    pub(super) fn accepted_body(
        &self,
        selected_ids: &[String],
    ) -> Result<String, ProjectCommandError> {
        if self.kind != "inline_edit" {
            return Ok(self.composed_block_body(selected_ids));
        }
        let (Some(from), Some(to)) = (self.inline_from, self.inline_to) else {
            return Err(unavailable("inline Acceptance needs exact Anchors"));
        };
        let mut blocks = self.chapter_blocks.clone();
        let Some(block) = blocks
            .iter_mut()
            .find(|block| block.manuscript_block_id == self.manuscript_block_id)
        else {
            return Err(unavailable("inline Acceptance needs its canonical Block"));
        };
        block.text = storyos_core::splice_utf16_range(&block.text, from, to, &self.candidate_text)
            .map_err(|error| unavailable(format!("{error:?}")))?;
        Ok(crate::manuscript_block::persist_canonical_bytes(&blocks))
    }

    fn composed_block_body(&self, selected_ids: &[String]) -> String {
        let selected: std::collections::BTreeSet<&str> =
            selected_ids.iter().map(String::as_str).collect();
        if self.chapter_blocks.is_empty() {
            return self
                .operations
                .iter()
                .find(|operation| selected.contains(operation.operation_id.as_str()))
                .map(|operation| operation.candidate_text.clone())
                .unwrap_or_else(|| self.candidate_text.clone());
        }
        let mut blocks = self.chapter_blocks.clone();
        for block in &mut blocks {
            if let Some(operation) = self.operations.iter().find(|operation| {
                selected.contains(operation.operation_id.as_str())
                    && operation.manuscript_block_id == block.manuscript_block_id
            }) {
                block.text = operation.candidate_text.clone();
            }
        }
        crate::manuscript_block::persist_canonical_bytes(&blocks)
    }

    fn inline_slice_matches(&self) -> bool {
        if self.kind != "inline_edit" {
            return true;
        }
        let (Some(from), Some(to), Some(digest)) = (
            self.inline_from,
            self.inline_to,
            self.inline_digest.as_deref(),
        ) else {
            return false;
        };
        let Some(block) = self
            .chapter_blocks
            .iter()
            .find(|block| block.manuscript_block_id == self.manuscript_block_id)
        else {
            return false;
        };
        let units: Vec<u16> = block.text.encode_utf16().collect();
        let (Ok(start), Ok(end)) = (usize::try_from(from), usize::try_from(to)) else {
            return false;
        };
        let Some(slice) = units
            .get(start..end)
            .and_then(|range| String::from_utf16(range).ok())
        else {
            return false;
        };
        storyos_core::proposal_anchor_base_slice_digest(
            &self.manuscript_block_id,
            match block.block_kind {
                storyos_core::ManuscriptBlockKind::Paragraph => "paragraph",
                storyos_core::ManuscriptBlockKind::Heading => "heading",
            },
            /*manuscript_schema_version*/ 1,
            storyos_core::PROSEMIRROR_TOKEN_UTF16_V1,
            from,
            to,
            &slice,
        ) == digest
    }
}

pub(super) async fn load_proposal(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    input: &AcceptProposalInput,
) -> Result<Option<LoadedProposal>, ProjectCommandError> {
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.closure,
                    revision.candidate_text, proposal.chapter_id::text,
                    receipt.validation_receipt_id::text, receipt.result,
                    receipt.proposal_revision_id::text, receipt.candidate_text,
                    chapter_head.current_revision_id::text,
                    revision.validation = 'valid' AND NOT EXISTS (SELECT 1 FROM storyos.proposal_validation_conditions AS condition
                      WHERE (condition.owner_user_id, condition.project_id, condition.proposal_id,
                             condition.proposal_revision_id) =
                            (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id)),
                    COALESCE(receipt.base_authoritative_revision_id = chapter_head.current_revision_id
                      AND revision.base_authoritative_revision_id = chapter_head.current_revision_id, false),
                    proposal.kind, convert_from(payload.canonical_bytes, 'UTF8'),
                    proposal.manuscript_block_id::text, anchor.range_from, anchor.range_to,
                    anchor.base_slice_digest, proposal.bundle_policy
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
               LEFT JOIN LATERAL (
                 SELECT operation.operation_id
                   FROM storyos.proposal_operations AS operation
                  WHERE (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                  ORDER BY operation.operation_id
                  LIMIT 1
               ) AS primary_operation ON true
               LEFT JOIN storyos.validation_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.validation_receipt_id) =
                    (proposal.owner_user_id, proposal.project_id, $4::text::uuid)
               LEFT JOIN storyos.authoritative_heads AS chapter_head
                 ON (chapter_head.owner_user_id, chapter_head.project_id,
                     chapter_head.manuscript_object_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.chapter_id)
               LEFT JOIN storyos.authoritative_revisions AS chapter_revision
                 ON (chapter_revision.owner_user_id, chapter_revision.project_id,
                     chapter_revision.manuscript_object_id, chapter_revision.revision_id) =
                    (chapter_head.owner_user_id, chapter_head.project_id,
                     chapter_head.manuscript_object_id, chapter_head.current_revision_id)
               LEFT JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (chapter_revision.owner_user_id, chapter_revision.project_id,
                     chapter_revision.payload_id)
               LEFT JOIN storyos.proposal_anchors AS anchor
                 ON (anchor.owner_user_id, anchor.project_id, anchor.proposal_id,
                     anchor.operation_id, anchor.anchor_order) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id,
                     primary_operation.operation_id, 1)
              WHERE proposal.owner_user_id = $1::text::uuid
                AND proposal.project_id = $2::text::uuid
                AND proposal.proposal_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &input.proposal_id,
                &input.validation_receipt_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let chapter_id: String = row.get(/*idx*/ 4);
    let chapter_body: String = row.get::<_, Option<String>>(/*idx*/ 13).unwrap_or_default();
    let current_head_revision_id: Option<String> = row.get(/*idx*/ 9);
    let operations = load_operations(
        client,
        scope.owner_user_id.as_ref(),
        scope.project_id.as_ref(),
        &input.proposal_id,
    )
    .await?;
    let chapter_blocks = match current_head_revision_id.as_deref() {
        Some(revision_id) => crate::manuscript_block::load_revision_blocks(
            client,
            scope.owner_user_id.as_ref(),
            scope.project_id.as_ref(),
            &chapter_id,
            revision_id,
            &chapter_body,
        )
        .await
        .map_err(unavailable)?,
        None => Vec::new(),
    };
    let mut loaded = LoadedProposal {
        current_revision_id: row.get(/*idx*/ 0),
        generation: row.get(/*idx*/ 1),
        closure: row.get(/*idx*/ 2),
        candidate_text: row.get(/*idx*/ 3),
        chapter_id,
        operations,
        bundle_policy: row.get(/*idx*/ 18),
        receipt_id: row.get(/*idx*/ 5),
        receipt_result: row.get(/*idx*/ 6),
        receipt_revision_id: row.get(/*idx*/ 7),
        receipt_candidate_text: row.get(/*idx*/ 8),
        current_head_revision_id,
        validation_current: row.get(/*idx*/ 10),
        validated_target_matches_head: row.get(/*idx*/ 11),
        kind: row.get(/*idx*/ 12),
        chapter_blocks,
        manuscript_block_id: row.get::<_, Option<String>>(/*idx*/ 14).unwrap_or_default(),
        inline_from: row
            .get::<_, Option<i32>>(/*idx*/ 15)
            .and_then(|value| u32::try_from(value).ok()),
        inline_to: row
            .get::<_, Option<i32>>(/*idx*/ 16)
            .and_then(|value| u32::try_from(value).ok()),
        inline_digest: row.get(/*idx*/ 17),
        inline_base_slice_matches: true,
    };
    loaded.validated_target_matches_head &=
        !crate::validation_history::unavailable_revisions(client, scope)
            .await
            .map_err(unavailable)?
            .contains(&loaded.current_revision_id);
    loaded.inline_base_slice_matches = loaded.inline_slice_matches();
    Ok(Some(loaded))
}

async fn load_operations(
    client: &tokio_postgres::Client,
    owner_user_id: &str,
    project_id: &str,
    proposal_id: &str,
) -> Result<Vec<LoadedOperation>, ProjectCommandError> {
    let rows = client
        .query(
            "SELECT operation_id::text, manuscript_block_id::text, resolution, candidate_text,
                    COALESCE(predecessor_operation_ids::text[], '{}')
               FROM storyos.proposal_operations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid
              ORDER BY operation_id",
            &[&owner_user_id, &project_id, &proposal_id],
        )
        .await
        .map_err(unavailable)?;
    Ok(rows
        .into_iter()
        .map(|row| LoadedOperation {
            operation_id: row.get(/*idx*/ 0),
            manuscript_block_id: row.get(/*idx*/ 1),
            resolution: row.get(/*idx*/ 2),
            candidate_text: row.get(/*idx*/ 3),
            predecessor_operation_ids: row.get(/*idx*/ 4),
        })
        .collect())
}
