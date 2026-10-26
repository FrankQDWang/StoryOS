//! The stored evidence of a settled Author Undo that an exact retry reads (ADR 0044). The
//! decoder comes from the command kind of the source Receipt.

use storyos_application::UndoRecords;

use crate::command_replay::{CommandReplay, ReplayFault};
use crate::undo_compensation::{
    AcceptanceChildReplay, CompensationReplay, ForwardCommand, UndoDisposition,
};

/// The effect query of an Author Undo. It takes the owner, Project, and Receipt identities.
pub(super) const UNDO_REPLAY_EFFECT: &str = "SELECT jsonb_build_object(
        'compensated_source_sequence', action.compensated_source_sequence::text,
        'authoritative_commit_id', authoritative_commit.authoritative_commit_id::text,
        'source_command_kind', source_receipt.command_kind,
        'source_result_kind', source_receipt.result_kind,
        'restored_proposal_revision_id', restored_proposal.revision_id::text,
        'snapshot_id', compensation_snapshot.snapshot_id::text,
        'snapshot_position', compensation_snapshot.project_activity_position::text,
        'acceptance_outcome', child.outcome,
        'acceptance_source_sequence', child.source_author_action_sequence::text,
        'acceptance_proposal_id', child.proposal_id::text,
        'acceptance_proposal_revision_id', child.proposal_revision_id::text,
        'author_undo_frontier_sequence', (
          SELECT forward.author_action_sequence::text
            FROM storyos.author_action_entries AS forward
            JOIN storyos.domain_receipts AS forward_receipt
              ON (forward_receipt.owner_user_id, forward_receipt.project_id,
                  forward_receipt.receipt_id) =
                 (forward.owner_user_id, forward.project_id, forward.receipt_id)
       LEFT JOIN storyos.author_action_entries AS compensation
              ON compensation.owner_user_id = forward.owner_user_id
             AND compensation.project_id = forward.project_id
             AND compensation.disposition = 'compensation'
             AND compensation.compensated_source_sequence = forward.author_action_sequence
       LEFT JOIN storyos.domain_receipts AS compensation_receipt
              ON (compensation_receipt.owner_user_id, compensation_receipt.project_id,
                  compensation_receipt.receipt_id) =
                 (compensation.owner_user_id, compensation.project_id, compensation.receipt_id)
             AND compensation_receipt.created_at <= receipt.created_at
           WHERE (forward.owner_user_id, forward.project_id) =
                 (receipt.owner_user_id, receipt.project_id)
             AND forward.disposition = 'forward'
             AND forward_receipt.created_at <= receipt.created_at
             AND compensation_receipt.receipt_id IS NULL
        ORDER BY forward.author_action_sequence DESC
           LIMIT 1))::text
   FROM storyos.domain_receipts AS receipt
LEFT JOIN storyos.author_action_entries AS action
     ON (action.owner_user_id, action.project_id, action.receipt_id) =
        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
LEFT JOIN storyos.authoritative_commits AS authoritative_commit
     ON (authoritative_commit.owner_user_id, authoritative_commit.project_id,
         authoritative_commit.receipt_id) =
        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
LEFT JOIN storyos.author_action_entries AS source_action
     ON (source_action.owner_user_id, source_action.project_id,
         source_action.author_action_sequence) =
        (action.owner_user_id, action.project_id, action.compensated_source_sequence)
    AND source_action.disposition = 'forward'
LEFT JOIN storyos.domain_receipts AS source_receipt
     ON (source_receipt.owner_user_id, source_receipt.project_id, source_receipt.receipt_id) =
        (source_action.owner_user_id, source_action.project_id, source_action.receipt_id)
LEFT JOIN storyos.proposal_revisions AS source_proposal
     ON (source_proposal.owner_user_id, source_proposal.project_id, source_proposal.revision_id) =
        (source_receipt.owner_user_id, source_receipt.project_id,
         source_receipt.proposal_revision_ids[1])
-- The parent of the appended Revision is the Proposal head of the Compensation (ADR 0068).
LEFT JOIN storyos.proposal_revisions AS restored_proposal
     ON (restored_proposal.owner_user_id, restored_proposal.project_id,
         restored_proposal.proposal_id) =
        (source_proposal.owner_user_id, source_proposal.project_id,
         source_proposal.proposal_id)
    AND restored_proposal.revision_id::text = receipt.result_payload->>'proposal_revision_id'
    AND restored_proposal.parent_revision_id::text =
        receipt.result_payload->>'source_proposal_revision_id'
LEFT JOIN storyos.undo_acceptance_receipts AS child
     ON (child.owner_user_id, child.project_id, child.author_undo_receipt_id) =
        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
LEFT JOIN LATERAL (
         SELECT snapshot.snapshot_id, snapshot.project_activity_position
           FROM storyos.project_snapshots AS snapshot
          WHERE (snapshot.owner_user_id, snapshot.project_id) =
                (receipt.owner_user_id, receipt.project_id)
            AND snapshot.snapshot_kind = 'canonical'
            AND snapshot.created_at >= receipt.created_at
          ORDER BY snapshot.project_activity_position, snapshot.created_at
          LIMIT 1
       ) AS compensation_snapshot ON true
  WHERE receipt.owner_user_id = $1::text::uuid
    AND receipt.project_id = $2::text::uuid
    AND receipt.receipt_id = $3::text::uuid";

/// The decoded effect row of one settled Author Undo.
pub(super) struct UndoReplay {
    fields: serde_json::Map<String, serde_json::Value>,
}

fn damaged() -> ReplayFault {
    ReplayFault::Unavailable("the Author Undo evidence is damaged".into())
}

impl UndoReplay {
    pub(super) fn parse(replay: &CommandReplay) -> Result<Self, ReplayFault> {
        Ok(Self {
            fields: replay.effect().fields().clone(),
        })
    }

    fn text(&self, key: &str) -> Result<Option<String>, ReplayFault> {
        match self.fields.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(text)) => Ok(Some(text.clone())),
            Some(_) => Err(damaged()),
        }
    }

    fn decimal(&self, key: &str) -> Result<Option<u64>, ReplayFault> {
        self.text(key)?
            .map(|text| text.parse().map_err(|_| damaged()))
            .transpose()
    }

    /// The Author Undo Disposition of the source Forward action, or `None` for the Forward
    /// action of a Reversal.
    pub(super) fn disposition(&self) -> Result<Option<UndoDisposition>, ReplayFault> {
        match (
            self.text("source_command_kind")?,
            self.text("source_result_kind")?,
        ) {
            (Some(command_kind), Some(result_kind)) => {
                ForwardCommand::from_receipt(&command_kind, &result_kind)
                    .map(|forward| Some(forward.disposition()))
                    .ok_or_else(damaged)
            }
            (None, None) => Ok(None),
            (Some(_), None) | (None, Some(_)) => Err(damaged()),
        }
    }

    pub(super) fn compensated_source_sequence(&self) -> Result<u64, ReplayFault> {
        self.decimal("compensated_source_sequence")?
            .ok_or_else(damaged)
    }

    pub(super) fn compensation<'a>(
        &'a self,
        replay: &'a CommandReplay,
    ) -> Result<CompensationReplay<'a>, ReplayFault> {
        Ok(CompensationReplay {
            snapshot: match (
                self.text("snapshot_id")?,
                self.decimal("snapshot_position")?,
            ) {
                (Some(snapshot_id), Some(position)) => Some((snapshot_id, position)),
                (None, None) => None,
                (Some(_), None) | (None, Some(_)) => return Err(damaged()),
            },
            authoritative_commit_id: self.text("authoritative_commit_id")?,
            restored_proposal_revision_id: self.text("restored_proposal_revision_id")?,
            receipt_payload: replay.receipt_fields(),
            revision: replay.revision.as_ref(),
            acceptance: self.acceptance()?,
        })
    }

    fn acceptance(&self) -> Result<Option<AcceptanceChildReplay>, ReplayFault> {
        let Some(outcome) = self.text("acceptance_outcome")? else {
            return Ok(None);
        };
        Ok(Some(AcceptanceChildReplay {
            outcome,
            source_sequence: self
                .decimal("acceptance_source_sequence")?
                .ok_or_else(damaged)?,
            proposal_id: self.text("acceptance_proposal_id")?,
            proposal_revision_id: self.text("acceptance_proposal_revision_id")?,
        }))
    }

    /// The Forward Author Action of an Undo Acceptance that requires a Reversal Proposal.
    pub(super) fn reversal(&self) -> Result<(u64, UndoRecords, Option<u64>), ReplayFault> {
        let child = self.acceptance()?.ok_or_else(damaged)?;
        let (Some(proposal_id), Some(proposal_revision_id), "reversal_required") = (
            child.proposal_id,
            child.proposal_revision_id,
            child.outcome.as_str(),
        ) else {
            return Err(damaged());
        };
        Ok((
            child.source_sequence,
            UndoRecords::ReversalRequired {
                proposal_id,
                proposal_revision_id,
            },
            None,
        ))
    }
}

/// The Author Undo Frontier as of the Receipt of a settled Author Undo.
pub(super) fn frontier_as_of_receipt(replay: &CommandReplay) -> Result<Option<u64>, ReplayFault> {
    UndoReplay::parse(replay)?.decimal("author_undo_frontier_sequence")
}
