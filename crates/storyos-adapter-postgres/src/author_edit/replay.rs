//! The replay checks and the effect query of `applyAuthorEdit` (ADR 0043, ADR 0044).

use storyos_contracts::SourceDraftDisposition;

use super::sequence::AuthorEditRecord;
use crate::command_replay::{CommandReplay, ReplayFault};

/// Requires the Receipt and authority records of a settled Author Edit, as `main` wrote them.
/// Another record is damaged evidence.
pub(super) fn check_replay_records(replay: &CommandReplay) -> Result<(), ReplayFault> {
    let effect = replay.effect();
    let flag = |key: &str| effect.fields().get(key) == Some(&serde_json::Value::Bool(true));
    let damaged = |what: &str| {
        Err(ReplayFault::Unavailable(
            format!("the Author Edit Receipt has {what}").into(),
        ))
    };
    if !flag("heads_shape") {
        return damaged("damaged head arrays");
    }
    let prior = effect.required("prior_head")?;
    let resulting = replay.resulting_heads.first().cloned().unwrap_or_default();
    let applied = replay.result_kind() == "authoritative_applied";
    if applied {
        if !flag("applied_records") {
            return damaged("damaged authority records");
        }
        if effect.text("commit_prior_revision_id")?.as_deref() != Some(prior.as_str()) {
            return damaged("a Commit of another prior Revision");
        }
    } else {
        if !flag("zero_authority_records") {
            return damaged("authority records of a zero-authority outcome");
        }
        if prior != resulting {
            return damaged("a zero-authority outcome that moved its head");
        }
    }
    // A missing reason is a binding conflict, which the outcome decode reports.
    let reason_recorded = replay
        .receipt_fields()
        .get("reason")
        .is_some_and(serde_json::Value::is_string);
    if replay.result_kind() == "conflicted" && reason_recorded {
        let current = replay
            .receipt_fields()
            .get("current_authoritative_revision_id");
        let stale = replay.receipt_fields().get("reason")
            == Some(&serde_json::Value::from("stale_authoritative_head"));
        if current != Some(&serde_json::Value::from(resulting.as_str()))
            || (stale && effect.required("expected_head")? == resulting)
        {
            return damaged("an impossible conflict");
        }
    }
    Ok(())
}

/// The recorded facts of a settled Author Edit.
pub(super) fn replayed_record(
    replay: &CommandReplay,
    refused_to_draft: bool,
) -> Result<AuthorEditRecord, ReplayFault> {
    let effect = replay.effect();
    let damaged =
        |what: &str| ReplayFault::Unavailable(format!("the Author Edit {what} is damaged").into());
    let source: Option<SourceDraftDisposition> =
        match effect.fields().get("source_draft_disposition") {
            None | Some(serde_json::Value::Null) => None,
            Some(value) => Some(
                serde_json::from_value(value.clone())
                    .map_err(|_| damaged("source Draft disposition"))?,
            ),
        };
    let retry: Option<storyos_contracts::DraftRetry> = match effect.fields().get("retry_source") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => {
            Some(serde_json::from_value(value.clone()).map_err(|_| damaged("retry source"))?)
        }
    };
    Ok(AuthorEditRecord {
        replacement_provenance: if refused_to_draft {
            crate::draft_retry::replacement_provenance(source.as_ref(), retry.as_ref())
        } else {
            None
        },
        source_draft_disposition: source,
        completed_intent_record_id: effect.required("completed_intent_record_id")?,
        local_intent_sequence: effect.required_u64("local_intent_sequence")?,
    })
}

/// The effect query of an Author Edit. It takes the owner, Project, and Receipt identities.
pub(super) const AUTHOR_EDIT_REPLAY_EFFECT: &str = "SELECT jsonb_build_object(
        'expected_heads', to_jsonb(receipt.expected_heads::text[]),
        'expected_head', receipt.expected_heads[1]::text,
        'prior_head', receipt.prior_heads[1]::text,
        'heads_shape', coalesce(
          array_lower(receipt.expected_heads, 1) = 1
            AND array_lower(receipt.prior_heads, 1) = 1
            AND array_lower(receipt.resulting_heads, 1) = 1
            AND cardinality(receipt.expected_heads) = 1
            AND cardinality(receipt.prior_heads) = 1
            AND cardinality(receipt.resulting_heads) = 1
            AND receipt.expected_heads[1] IS NOT NULL
            AND receipt.prior_heads[1] IS NOT NULL
            AND receipt.resulting_heads[1] IS NOT NULL, false),
        'applied_records', coalesce(
          cardinality(receipt.authoritative_commit_ids) = 1
            AND array_lower(receipt.authoritative_commit_ids, 1) = 1
            AND (SELECT count(*) FROM storyos.authoritative_commits AS commit
                  WHERE (commit.owner_user_id, commit.project_id,
                         commit.author_command_admission_id) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id)) = 1
            AND (SELECT count(*) FROM storyos.authoritative_revision_envelopes AS envelope
                  WHERE (envelope.owner_user_id, envelope.project_id, envelope.creator_ref) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id)) = 1
            AND EXISTS (
              SELECT 1 FROM storyos.authoritative_commits AS commit
                JOIN storyos.authoritative_revision_envelopes AS envelope
                  ON (envelope.owner_user_id, envelope.project_id,
                      envelope.manuscript_object_id, envelope.revision_id) =
                     (commit.owner_user_id, commit.project_id, commit.manuscript_object_id,
                      commit.resulting_revision_id)
                JOIN storyos.authoritative_revisions AS revision
                  ON (revision.owner_user_id, revision.project_id,
                      revision.manuscript_object_id, revision.revision_id) =
                     (commit.owner_user_id, commit.project_id, commit.manuscript_object_id,
                      commit.resulting_revision_id)
                JOIN storyos.authoritative_payloads AS payload
                  ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                     (revision.owner_user_id, revision.project_id, revision.payload_id)
               WHERE (commit.owner_user_id, commit.project_id, commit.receipt_id) =
                     (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                 AND commit.manuscript_object_id = admission.chapter_object_id
                 AND commit.resulting_revision_id = receipt.resulting_heads[1]
                 AND envelope.parent_revision_id = commit.prior_revision_id
                 AND envelope.payload_digest = encode(sha256(payload.canonical_bytes), 'hex')),
          false),
        'commit_prior_revision_id', (
          SELECT commit.prior_revision_id::text FROM storyos.authoritative_commits AS commit
           WHERE (commit.owner_user_id, commit.project_id, commit.receipt_id) =
                 (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)),
        'zero_authority_records', coalesce(
          cardinality(receipt.authoritative_commit_ids) = 0
            AND NOT EXISTS (
              SELECT 1 FROM storyos.authoritative_commits AS commit
               WHERE (commit.owner_user_id, commit.project_id) =
                     (receipt.owner_user_id, receipt.project_id)
                 AND (commit.receipt_id = receipt.receipt_id
                      OR commit.author_command_admission_id = receipt.author_command_admission_id))
            AND NOT EXISTS (
              SELECT 1 FROM storyos.authoritative_revision_envelopes AS envelope
               WHERE (envelope.owner_user_id, envelope.project_id) =
                     (receipt.owner_user_id, receipt.project_id)
                 AND (envelope.receipt_id = receipt.receipt_id
                      OR envelope.creator_ref = receipt.author_command_admission_id))
            AND NOT EXISTS (
              SELECT 1 FROM storyos.project_activity_events AS activity
               WHERE (activity.owner_user_id, activity.project_id, activity.receipt_id) =
                     (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)),
          false),
        'admission_chapter_id', admission.chapter_object_id::text,
        'admission_expected_revision_id', admission.expected_authoritative_revision_id::text,
        'admission_target_refs', to_jsonb(admission.target_refs),
        'source_draft_disposition', receipt.source_draft_disposition,
        'retry_source', admission.command_payload->'retry_source',
        'completed_intent_record_id', admission.completed_intent_record_id::text,
        'local_intent_sequence', admission.local_intent_sequence::text,
        'proposal_revision_id', receipt.proposal_revision_ids[1]::text,
        'draft_id', draft.draft_id::text,
        'draft_revision_id', draft.revision_id::text,
        'creation_event_id', draft.creation_event_id::text)::text
   FROM storyos.domain_receipts AS receipt
   JOIN storyos.author_command_admissions AS admission
     ON (admission.owner_user_id, admission.project_id, admission.author_command_admission_id) =
        (receipt.owner_user_id, receipt.project_id, receipt.author_command_admission_id)
LEFT JOIN storyos.draft_lifecycle_events AS draft
     ON (draft.owner_user_id, draft.project_id, draft.receipt_id) =
        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
  WHERE receipt.owner_user_id = $1::text::uuid
    AND receipt.project_id = $2::text::uuid
    AND receipt.receipt_id = $3::text::uuid";
