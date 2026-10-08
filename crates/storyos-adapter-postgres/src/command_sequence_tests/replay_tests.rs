use storyos_application::{
    AcceptProposalInput, ArchiveProjectInput, CloseEditorFlowDraftInput, CreateChapterInput,
    CreateVolumeInput, DeleteChapterInput, DeleteVolumeInput,
    ExpandRefusedEditDraftToProposalInput, RejectProposalOperationsInput,
    ReopenWithdrawnProposalInput, SetCurrentChapterInput, UpdateChapterInput,
    UpdateProjectAssistanceInput, UpdateProjectInput, UpdateVolumeInput, WithdrawProposalInput,
};
use storyos_application::{
    CancelAgentRunInput, ChapterId, EditorSessionId, PauseAgentRunInput, VolumeId,
};
use storyos_application::{CompleteReadyPartialProposalInput, ContinueProposalGenerationInput};
use storyos_core::{AssistanceAvailability, CreateChapterPlacement, OpenInlineProposalAnchor};
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::update_volume_tests::seed_project;

use super::acceptance::{ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal, acceptance_rows};
use super::agent_run::{
    CANCEL_AGENT_RUN, PAUSE_AGENT_RUN, cancel_agent_run, create_agent_run, create_agent_run_call,
    park_run, pause_agent_run, seed_run,
};
use super::draft::{
    CLOSE_EDITOR_FLOW_DRAFT, EXPAND_REFUSED_EDIT_DRAFT, close_editor_flow_draft, discard_call,
    expand_refused_edit_draft, expansion_call, refused_edit_draft, refused_edit_expansion,
};
use super::project_session::{
    ARCHIVE_PROJECT, TAKE_OVER_PROJECT_WRITER, UPDATE_PROJECT, UPDATE_PROJECT_ASSISTANCE,
    take_over_project_writer, take_over_project_writer_call,
};
use super::proposal_decision::{
    REJECT_PROPOSAL_OPERATIONS, REOPEN_REJECTED_OPERATIONS, REOPEN_WITHDRAWN_PROPOSAL,
    REPLAN_PROPOSAL, WITHDRAW_PROPOSAL, conflicted_proposal, move_head_away, pending_proposal,
    reject_proposal_operations, rejected_operation, reopen_rejected_operations,
    reopen_withdrawn_proposal, replan_proposal, withdraw_proposal, withdrawable_proposal,
    withdrawn_proposal,
};
use super::proposal_generation::{
    COMPLETE_READY_PARTIAL_PROPOSAL, CONTINUE_PROPOSAL_GENERATION, complete_ready_partial_proposal,
    continue_proposal_generation, ready_partial_proposal,
};
use super::steer_agent_run::{STEER_AGENT_RUN, steer_agent_run, steering};
use super::structure::{
    CREATE_CHAPTER, CREATE_VOLUME, DELETE_CHAPTER, DELETE_VOLUME, SET_CURRENT_CHAPTER,
    UPDATE_CHAPTER, UPDATE_VOLUME, create_chapter, create_volume, delete_chapter, delete_volume,
    new_chapter, new_volume, update_chapter, update_volume,
};
use super::support::{CommandCall, issued, replayed_outcome, stores, two_chapter_writer};

const MISSING_VOLUME: &str = "018f0000-0000-7001-8000-00000000ffff";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_outcome_replays_its_first_settlement_and_writes_only_its_profile_records() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let missing_volume = VolumeId::new(MISSING_VOLUME);
    let missing_chapter = ChapterId::new(MISSING_VOLUME);
    let mut observed = Vec::new();

    let scope = seed_project(&store, "5d00").await;
    for (suffix, title, expected_tree_revision) in [
        (0x5d01, "Volume", 1),
        (0x5d02, "Volume", 1),
        (0x5d03, "", 2),
    ] {
        let input = CreateVolumeInput {
            title: title.to_owned(),
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &CREATE_VOLUME, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, create_volume).await;
        observed.push((CREATE_VOLUME.kind, outcome));
    }

    let scope = seed_project(&store, "5d10").await;
    let volume = new_volume(
        &store, &scope, /*suffix*/ 0x5d11, /*expected_tree_revision*/ 1,
    )
    .await;
    for (suffix, volume_id, expected_tree_revision) in [
        (0x5d12, &volume, 2),
        (0x5d13, &volume, 3),
        (0x5d14, &volume, 2),
        (0x5d15, &missing_volume, 3),
    ] {
        let input = UpdateVolumeInput {
            volume_id: volume_id.clone(),
            title: "Renamed".to_owned(),
            order: 1,
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &UPDATE_VOLUME, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, update_volume).await;
        observed.push((UPDATE_VOLUME.kind, outcome));
    }

    let scope = seed_project(&store, "5d20").await;
    let volume = new_volume(
        &store, &scope, /*suffix*/ 0x5d21, /*expected_tree_revision*/ 1,
    )
    .await;
    for (suffix, volume_id, expected_tree_revision) in [
        (0x5d22, &volume, 2),
        (0x5d23, &volume, 3),
        (0x5d24, &volume, 2),
        (0x5d25, &missing_volume, 3),
    ] {
        let input = DeleteVolumeInput {
            volume_id: volume_id.clone(),
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &DELETE_VOLUME, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, delete_volume).await;
        observed.push((DELETE_VOLUME.kind, outcome));
    }

    let scope = seed_project(&store, "5d30").await;
    let volume = new_volume(
        &store, &scope, /*suffix*/ 0x5d31, /*expected_tree_revision*/ 1,
    )
    .await;
    for (suffix, volume_id, expected_tree_revision) in [
        (0x5d32, &volume, 2),
        (0x5d33, &volume, 2),
        (0x5d34, &missing_volume, 3),
    ] {
        let input = CreateChapterInput {
            volume_id: volume_id.as_ref().to_owned(),
            title: "Chapter".to_owned(),
            placement: CreateChapterPlacement::Append,
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &CREATE_CHAPTER, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, create_chapter).await;
        observed.push((CREATE_CHAPTER.kind, outcome));
    }

    let scope = seed_project(&store, "5d40").await;
    let volume = new_volume(
        &store, &scope, /*suffix*/ 0x5d41, /*expected_tree_revision*/ 1,
    )
    .await;
    let chapter = new_chapter(
        &store, &scope, /*suffix*/ 0x5d42, &volume, /*expected_tree_revision*/ 2,
    )
    .await;
    for (suffix, chapter_id, expected_tree_revision) in [
        (0x5d43, &chapter, 3),
        (0x5d44, &chapter, 4),
        (0x5d45, &chapter, 3),
        (0x5d46, &missing_chapter, 4),
    ] {
        let input = UpdateChapterInput {
            chapter_id: chapter_id.clone(),
            title: "Renamed".to_owned(),
            order: 1,
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &UPDATE_CHAPTER, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, update_chapter).await;
        observed.push((UPDATE_CHAPTER.kind, outcome));
    }

    let scope = seed_project(&store, "5d50").await;
    let volume = new_volume(
        &store, &scope, /*suffix*/ 0x5d51, /*expected_tree_revision*/ 1,
    )
    .await;
    new_chapter(
        &store, &scope, /*suffix*/ 0x5d52, &volume, /*expected_tree_revision*/ 2,
    )
    .await;
    let chapter = new_chapter(
        &store, &scope, /*suffix*/ 0x5d53, &volume, /*expected_tree_revision*/ 3,
    )
    .await;
    for (suffix, chapter_id, expected_tree_revision) in [
        (0x5d54, &chapter, 4),
        (0x5d55, &chapter, 5),
        (0x5d56, &chapter, 4),
        (0x5d57, &missing_chapter, 5),
    ] {
        let input = DeleteChapterInput {
            chapter_id: chapter_id.clone(),
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &DELETE_CHAPTER, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, delete_chapter).await;
        observed.push((DELETE_CHAPTER.kind, outcome));
    }

    let scope = seed_project(&store, "5d60").await;
    for (suffix, title, expected_revision) in [
        (0x5d61, "Renamed", 1),
        (0x5d62, "Renamed", 2),
        (0x5d63, "Other", 1),
    ] {
        let input = UpdateProjectInput {
            title: title.to_owned(),
            expected_revision,
        };
        let call = issued(&store, &scope, suffix, &UPDATE_PROJECT, input).await;
        let outcome = replayed_outcome(
            &store,
            &admin,
            &call,
            async |store: &PostgresProjectReader, call: &CommandCall<UpdateProjectInput>| {
                store.update_project(&call.envelope, &call.input).await
            },
        )
        .await;
        observed.push((UPDATE_PROJECT.kind, outcome));
    }

    let scope = seed_project(&store, "5d70").await;
    for (suffix, expected_revision) in [(0x5d71, 2), (0x5d72, 1), (0x5d73, 2)] {
        let input = ArchiveProjectInput { expected_revision };
        let call = issued(&store, &scope, suffix, &ARCHIVE_PROJECT, input).await;
        let outcome = replayed_outcome(
            &store,
            &admin,
            &call,
            async |store: &PostgresProjectReader, call: &CommandCall<ArchiveProjectInput>| {
                store.archive_project(&call.envelope, &call.input).await
            },
        )
        .await;
        observed.push((ARCHIVE_PROJECT.kind, outcome));
    }

    let (scope, chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(&store, /*base*/ 0x5d80).await;
    for (suffix, chapter_id, expected_current_chapter_id) in [
        (0x5d89, &chapter_b, &chapter_a),
        (0x5d8a, &chapter_b, &chapter_b),
        (0x5d8b, &chapter_a, &chapter_a),
        (0x5d8c, &MISSING_VOLUME.to_owned(), &chapter_b),
    ] {
        let input = SetCurrentChapterInput {
            editor_session_id: EditorSessionId::new(editor_session_id.clone()),
            chapter_id: chapter_id.clone(),
            expected_current_chapter_id: expected_current_chapter_id.clone(),
            expected_target_revision_id: revision_b.clone(),
        };
        let call = issued(&store, &scope, suffix, &SET_CURRENT_CHAPTER, input).await;
        let outcome = replayed_outcome(
            &store,
            &admin,
            &call,
            async |store: &PostgresProjectReader, call: &CommandCall<SetCurrentChapterInput>| {
                store.set_current_chapter(&call.envelope, &call.input).await
            },
        )
        .await;
        observed.push((SET_CURRENT_CHAPTER.kind, outcome));
    }

    let scope = seed_project(&store, "5d90").await;
    for (suffix, availability, expected_revision) in [
        (0x5d91, AssistanceAvailability::Available, 0),
        (0x5d92, AssistanceAvailability::Available, 1),
        (0x5d93, AssistanceAvailability::Unavailable, 0),
        (0x5d94, AssistanceAvailability::Unavailable, 1),
    ] {
        let input = UpdateProjectAssistanceInput {
            availability,
            expected_revision,
        };
        let call = issued(&store, &scope, suffix, &UPDATE_PROJECT_ASSISTANCE, input).await;
        let outcome = replayed_outcome(
            &store,
            &admin,
            &call,
            async |store: &PostgresProjectReader,
                   call: &CommandCall<UpdateProjectAssistanceInput>| {
                store
                    .update_project_assistance(&call.envelope, &call.input)
                    .await
            },
        )
        .await;
        observed.push((UPDATE_PROJECT_ASSISTANCE.kind, outcome));
    }

    let call = take_over_project_writer_call(&store, /*base*/ 0x5da0).await;
    let outcome = replayed_outcome(&store, &admin, &call, take_over_project_writer).await;
    observed.push((TAKE_OVER_PROJECT_WRITER.kind, outcome));

    let (scope, withdrawn) = withdrawn_proposal(&store, &admin, /*base*/ 0x5db0).await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x5db9,
        &REOPEN_WITHDRAWN_PROPOSAL,
        withdrawn.clone(),
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, reopen_withdrawn_proposal).await;
    observed.push((REOPEN_WITHDRAWN_PROPOSAL.kind, outcome));
    let reopened_revision_id: String = admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.proposal_heads
              WHERE proposal_id = $1::text::uuid",
            &[&withdrawn.proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    for (suffix, proposal_revision_id) in [
        (0x5dba, reopened_revision_id),
        (0x5dbb, withdrawn.proposal_revision_id.clone()),
    ] {
        let input = ReopenWithdrawnProposalInput {
            proposal_revision_id,
            ..withdrawn.clone()
        };
        let call = issued(&store, &scope, suffix, &REOPEN_WITHDRAWN_PROPOSAL, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, reopen_withdrawn_proposal).await;
        observed.push((REOPEN_WITHDRAWN_PROPOSAL.kind, outcome));
    }
    let (scope, withdrawn) = withdrawn_proposal(&store, &admin, /*base*/ 0x5dc0).await;
    move_head_away(
        &admin,
        &scope,
        &withdrawn.expected_authoritative_revision_id,
    )
    .await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x5dc9,
        &REOPEN_WITHDRAWN_PROPOSAL,
        withdrawn,
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, reopen_withdrawn_proposal).await;
    observed.push((REOPEN_WITHDRAWN_PROPOSAL.kind, outcome));

    let mut rejection_records = Vec::new();
    let (scope, pending) = pending_proposal(&store, &admin, /*base*/ 0x7b00).await;
    let stale = RejectProposalOperationsInput {
        proposal_revision_id: Uuid::now_v7().to_string(),
        ..pending.clone()
    };
    let (changed_scope, changed) = pending_proposal(&store, &admin, /*base*/ 0x7b10).await;
    move_head_away(
        &admin,
        &changed_scope,
        &changed.expected_authoritative_revision_id,
    )
    .await;
    for (scope, suffix, input) in [
        (&scope, 0x7b09, pending.clone()),
        (&scope, 0x7b0a, pending),
        (&scope, 0x7b0b, stale),
        (&changed_scope, 0x7b19, changed),
    ] {
        let call = issued(&store, scope, suffix, &REJECT_PROPOSAL_OPERATIONS, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, reject_proposal_operations).await;
        observed.push((REJECT_PROPOSAL_OPERATIONS.kind, outcome));
        rejection_records.push(
            admin
                .query_one(
                    "SELECT count(*) FROM storyos.proposal_rejection_receipts
                      WHERE rejection_receipt_id = $1::text::uuid",
                    &[&call.envelope.ids.receipt_id],
                )
                .await
                .unwrap()
                .get::<_, i64>(/*idx*/ 0),
        );
    }

    // The second exact call names a Proposal Revision that the first call superseded.
    let (scope, conflicted) = conflicted_proposal(&store, &admin, /*base*/ 0x7200).await;
    for suffix in [0x7209, 0x720a] {
        let call = issued(&store, &scope, suffix, &REPLAN_PROPOSAL, conflicted.clone()).await;
        let outcome = replayed_outcome(&store, &admin, &call, replan_proposal).await;
        observed.push((REPLAN_PROPOSAL.kind, outcome));
    }
    let (scope, conflicted) = conflicted_proposal(&store, &admin, /*base*/ 0x7210).await;
    move_head_away(
        &admin,
        &scope,
        &conflicted.expected_authoritative_revision_id,
    )
    .await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x7219,
        &REPLAN_PROPOSAL,
        conflicted,
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, replan_proposal).await;
    observed.push((REPLAN_PROPOSAL.kind, outcome));
    let (scope, rejected) = rejected_operation(&store, &admin, /*base*/ 0x7220).await;
    for suffix in [0x7229, 0x722a] {
        let call = issued(
            &store,
            &scope,
            suffix,
            &REOPEN_REJECTED_OPERATIONS,
            rejected.clone(),
        )
        .await;
        let outcome = replayed_outcome(&store, &admin, &call, reopen_rejected_operations).await;
        observed.push((REOPEN_REJECTED_OPERATIONS.kind, outcome));
    }
    let (scope, rejected) = rejected_operation(&store, &admin, /*base*/ 0x7230).await;
    move_head_away(&admin, &scope, &rejected.expected_authoritative_revision_id).await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x7239,
        &REOPEN_REJECTED_OPERATIONS,
        rejected,
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, reopen_rejected_operations).await;
    observed.push((REOPEN_REJECTED_OPERATIONS.kind, outcome));

    let (scope, open) = withdrawable_proposal(&store, &admin, /*base*/ 0x7440).await;
    let mut withdrawal_receipts = Vec::new();
    for (suffix, proposal_revision_id) in [
        (0x7449, open.proposal_revision_id.clone()),
        (0x744a, open.proposal_revision_id.clone()),
        (0x744b, Uuid::now_v7().to_string()),
    ] {
        let input = WithdrawProposalInput {
            proposal_revision_id,
            ..open.clone()
        };
        let call = issued(&store, &scope, suffix, &WITHDRAW_PROPOSAL, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, withdraw_proposal).await;
        observed.push((WITHDRAW_PROPOSAL.kind, outcome));
        withdrawal_receipts.push(call.envelope.ids.receipt_id);
    }
    let (scope, open) = withdrawable_proposal(&store, &admin, /*base*/ 0x7450).await;
    move_head_away(&admin, &scope, &open.expected_authoritative_revision_id).await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x7459,
        &WITHDRAW_PROPOSAL,
        open,
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, withdraw_proposal).await;
    observed.push((WITHDRAW_PROPOSAL.kind, outcome));
    withdrawal_receipts.push(call.envelope.ids.receipt_id);
    let mut withdrawal_records = Vec::new();
    for receipt_id in &withdrawal_receipts {
        let records: i64 = admin
            .query_one(
                "SELECT count(*) FROM storyos.proposal_withdrawals
                  WHERE withdrawal_receipt_id = $1::text::uuid",
                &[receipt_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        withdrawal_records.push(records);
    }
    let (scope, open) = refused_edit_draft(&store, &admin, /*base*/ 0x7d00, "retained").await;
    let changed = CloseEditorFlowDraftInput {
        source_current_draft_revision_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let (archived_scope, archived) =
        refused_edit_draft(&store, &admin, /*base*/ 0x7d10, "archived").await;
    for (scope, suffix, input) in [
        (&scope, 0x7d09, open.clone()),
        (&scope, 0x7d0a, open),
        (&scope, 0x7d0b, changed),
        (&archived_scope, 0x7d19, archived),
    ] {
        let call = discard_call(&store, scope, suffix, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, close_editor_flow_draft).await;
        observed.push((CLOSE_EDITOR_FLOW_DRAFT.kind, outcome));
    }
    let (scope, open) = refused_edit_expansion(&store, &admin, /*base*/ 0x8e00, "retained").await;
    let changed = ExpandRefusedEditDraftToProposalInput {
        source_current_draft_revision_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let missing_target_ref = Uuid::now_v7().to_string();
    let missing_target = ExpandRefusedEditDraftToProposalInput {
        target_ref: missing_target_ref.clone(),
        anchor: OpenInlineProposalAnchor {
            manuscript_block_id: missing_target_ref,
            ..open.anchor.clone()
        },
        ..open.clone()
    };
    let (archived_scope, archived) =
        refused_edit_expansion(&store, &admin, /*base*/ 0x8e10, "archived").await;
    let mut expansion_records = Vec::new();
    for (scope, suffix, input) in [
        (&scope, 0x8e09, open.clone()),
        (&scope, 0x8e0a, changed),
        (&scope, 0x8e0b, missing_target),
        (&archived_scope, 0x8e19, archived),
    ] {
        let call = expansion_call(&store, scope, suffix, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, expand_refused_edit_draft).await;
        observed.push((EXPAND_REFUSED_EDIT_DRAFT.kind, outcome));
        // The Proposals, superseding close events, and closed source Drafts of the Receipt.
        let row = admin
            .query_one(
                "SELECT (SELECT count(*) FROM storyos.proposals AS proposal
                           JOIN storyos.domain_receipts AS receipt
                             ON receipt.result_payload->>'proposal_id' = proposal.proposal_id::text
                          WHERE receipt.receipt_id = $1::text::uuid
                            AND proposal.source_draft_id = $2::text::uuid),
                        (SELECT count(*) FROM storyos.draft_close_events
                          WHERE receipt_id = $1::text::uuid AND close_reason = 'superseded'),
                        (SELECT count(*) FROM storyos.draft_artifacts AS draft
                           JOIN storyos.draft_close_events AS event
                             ON event.event_id = draft.close_event_id
                          WHERE event.receipt_id = $1::text::uuid AND draft.closure = 'closed')",
                &[&call.envelope.ids.receipt_id, &call.input.draft_id],
            )
            .await
            .unwrap();
        expansion_records.push([0, 1, 2].map(|index| row.get::<_, i64>(index)));
    }

    let (scope, generation) = ready_partial_proposal(&store, &admin, /*base*/ 0x7500).await;
    for (suffix, expected_authoritative_revision_id) in [
        (0x750a, &generation.other_chapter_revision_id),
        (
            0x750b,
            &generation.complete.expected_authoritative_revision_id,
        ),
        (
            0x750c,
            &generation.complete.expected_authoritative_revision_id,
        ),
    ] {
        let input = CompleteReadyPartialProposalInput {
            expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
            ..generation.complete.clone()
        };
        let call = issued(
            &store,
            &scope,
            suffix,
            &COMPLETE_READY_PARTIAL_PROPOSAL,
            input,
        )
        .await;
        let outcome =
            replayed_outcome(&store, &admin, &call, complete_ready_partial_proposal).await;
        observed.push((COMPLETE_READY_PARTIAL_PROPOSAL.kind, outcome));
    }
    for (suffix, expected_authoritative_revision_id) in [
        (0x750d, &generation.other_chapter_revision_id),
        (
            0x750e,
            &generation.complete.expected_authoritative_revision_id,
        ),
        (
            0x750f,
            &generation.complete.expected_authoritative_revision_id,
        ),
    ] {
        let input = ContinueProposalGenerationInput {
            expected_generation_state: "ready".to_owned(),
            expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
            ..generation.continuation.clone()
        };
        let call = issued(&store, &scope, suffix, &CONTINUE_PROPOSAL_GENERATION, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, continue_proposal_generation).await;
        observed.push((CONTINUE_PROPOSAL_GENERATION.kind, outcome));
    }

    let scope = seed_project(&store, "b210").await;
    let (waiting, completed) = (Uuid::now_v7().to_string(), Uuid::now_v7().to_string());
    seed_run(&admin, &scope, &waiting, "waiting").await;
    seed_run(&admin, &scope, &completed, "completed").await;
    for (suffix, run_id) in [(0xb211, &waiting), (0xb212, &waiting), (0xb213, &completed)] {
        let input = PauseAgentRunInput {
            run_id: run_id.clone(),
        };
        let call = issued(&store, &scope, suffix, &PAUSE_AGENT_RUN, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, pause_agent_run).await;
        observed.push((PAUSE_AGENT_RUN.kind, outcome));
    }

    let scope = seed_project(&store, "d510").await;
    let (waiting, completed) = (Uuid::now_v7().to_string(), Uuid::now_v7().to_string());
    seed_run(&admin, &scope, &waiting, "waiting").await;
    seed_run(&admin, &scope, &completed, "completed").await;
    for (suffix, run_id) in [(0xd511, &waiting), (0xd512, &waiting), (0xd513, &completed)] {
        let input = CancelAgentRunInput {
            run_id: run_id.clone(),
        };
        let call = issued(&store, &scope, suffix, &CANCEL_AGENT_RUN, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, cancel_agent_run).await;
        observed.push((CANCEL_AGENT_RUN.kind, outcome));
    }

    let call = create_agent_run_call(&store, /*base*/ 0xb320).await;
    let outcome = replayed_outcome(&store, &admin, &call, create_agent_run).await;
    park_run(&admin, &call).await;
    observed.push((
        call.envelope.challenge_binding.command_kind.as_str(),
        outcome,
    ));

    let scope = seed_project(&store, "c710").await;
    let (waiting, completed) = (Uuid::now_v7().to_string(), Uuid::now_v7().to_string());
    seed_run(&admin, &scope, &waiting, "waiting").await;
    seed_run(&admin, &scope, &completed, "completed").await;
    for (suffix, run_id) in [(0xc711, &waiting), (0xc712, &waiting), (0xc713, &completed)] {
        let input = steering(&admin, run_id, /*characters*/ 12).await;
        let call = issued(&store, &scope, suffix, &STEER_AGENT_RUN, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, steer_agent_run).await;
        observed.push((STEER_AGENT_RUN.kind, outcome));
    }

    // An invalid or conflicted Acceptance makes its Proposal ineligible, so each one has its own
    // Proposal.
    let mut acceptance_records = Vec::new();
    for base in [0xa800, 0xa810, 0xa860] {
        let (scope, acceptance, chapter_a_head) = acceptable_proposal(&store, &admin, base).await;
        let inputs = match base {
            0xa800 => vec![AcceptProposalInput {
                validation_receipt_id: Uuid::now_v7().to_string(),
                ..acceptance
            }],
            0xa810 => vec![AcceptProposalInput {
                expected_authoritative_revision_id: chapter_a_head,
                ..acceptance
            }],
            _ => vec![
                AcceptProposalInput {
                    proposal_revision_id: Uuid::now_v7().to_string(),
                    ..acceptance.clone()
                },
                acceptance,
            ],
        };
        for (offset, input) in (9..).zip(inputs) {
            let call = issued(&store, &scope, base + offset, &ACCEPT_PROPOSAL, input).await;
            let outcome = replayed_outcome(&store, &admin, &call, accept_proposal).await;
            acceptance_records.push(acceptance_rows(&admin, &call.envelope.ids.receipt_id).await);
            observed.push((ACCEPT_PROPOSAL.kind, outcome));
        }
    }

    // Receipt, Author Action, Activity, Commit, and Snapshot rows of each outcome.
    let structural_applied = ("authoritative_applied", [1, 1, 1, 1, 1]);
    let chapter_selection_applied = ("authoritative_applied", [1, 1, 1, 0, 1]);
    let activity_applied = ("authoritative_applied", [1, 0, 1, 0, 0]);
    let proposal_revised = ("proposal_revised", [1, 1, 0, 0, 0]);
    let proposal_closure_changed = ("proposal_closure_changed", [1, 1, 0, 0, 0]);
    let operations_resolved = ("proposal_operations_resolved", [1, 1, 0, 0, 0]);
    let generation_completed = ("proposal_generation_completed", [1, 1, 0, 0, 0]);
    let generation_started = ("proposal_generation_started", [1, 1, 0, 0, 0]);
    let draft_closure_changed = ("draft_closure_changed", [1, 1, 0, 0, 0]);
    let proposal_created_from_draft = ("proposal_created_from_draft", [1, 1, 0, 0, 0]);
    let no_effect = ("no_effect", [1, 0, 0, 0, 0]);
    let invalid = ("invalid", [1, 0, 0, 0, 0]);
    let writer_takeover = ("no_effect", [1, 0, 1, 0, 1]);
    let steering_retained = ("no_effect", [1, 0, 1, 0, 0]);
    let conflicted = ("conflicted", [1, 0, 0, 0, 0]);
    let refused = ("refused", [1, 0, 0, 0, 0]);
    assert_eq!(
        observed
            .iter()
            .map(|(kind, (result_kind, rows))| (*kind, (result_kind.as_str(), *rows)))
            .collect::<Vec<_>>(),
        vec![
            ("createVolume", structural_applied),
            ("createVolume", conflicted),
            ("createVolume", refused),
            ("updateVolume", structural_applied),
            ("updateVolume", no_effect),
            ("updateVolume", conflicted),
            ("updateVolume", refused),
            ("deleteVolume", structural_applied),
            ("deleteVolume", no_effect),
            ("deleteVolume", conflicted),
            ("deleteVolume", refused),
            ("createChapter", structural_applied),
            ("createChapter", conflicted),
            ("createChapter", refused),
            ("updateChapter", structural_applied),
            ("updateChapter", no_effect),
            ("updateChapter", conflicted),
            ("updateChapter", refused),
            ("deleteChapter", structural_applied),
            ("deleteChapter", no_effect),
            ("deleteChapter", conflicted),
            ("deleteChapter", refused),
            ("updateProject", activity_applied),
            ("updateProject", no_effect),
            ("updateProject", conflicted),
            ("archiveProject", conflicted),
            ("archiveProject", activity_applied),
            ("archiveProject", no_effect),
            ("setCurrentChapter", chapter_selection_applied),
            ("setCurrentChapter", no_effect),
            ("setCurrentChapter", conflicted),
            ("setCurrentChapter", refused),
            ("updateProjectAssistance", activity_applied),
            ("updateProjectAssistance", no_effect),
            ("updateProjectAssistance", conflicted),
            ("updateProjectAssistance", activity_applied),
            ("takeOverProjectWriter", writer_takeover),
            ("reopenWithdrawnProposal", proposal_revised),
            ("reopenWithdrawnProposal", no_effect),
            ("reopenWithdrawnProposal", refused),
            ("reopenWithdrawnProposal", conflicted),
            ("rejectProposalOperations", operations_resolved),
            ("rejectProposalOperations", refused),
            ("rejectProposalOperations", refused),
            ("rejectProposalOperations", conflicted),
            ("replanProposal", proposal_revised),
            ("replanProposal", refused),
            ("replanProposal", conflicted),
            ("reopenRejectedOperations", proposal_revised),
            ("reopenRejectedOperations", refused),
            ("reopenRejectedOperations", conflicted),
            ("withdrawProposal", proposal_closure_changed),
            ("withdrawProposal", no_effect),
            ("withdrawProposal", refused),
            ("withdrawProposal", conflicted),
            ("closeEditorFlowDraft", draft_closure_changed),
            ("closeEditorFlowDraft", refused),
            ("closeEditorFlowDraft", conflicted),
            ("closeEditorFlowDraft", refused),
            (
                "expandRefusedEditDraftToProposal",
                proposal_created_from_draft
            ),
            ("expandRefusedEditDraftToProposal", conflicted),
            ("expandRefusedEditDraftToProposal", refused),
            ("expandRefusedEditDraftToProposal", refused),
            ("completeReadyPartialProposal", conflicted),
            ("completeReadyPartialProposal", generation_completed),
            ("completeReadyPartialProposal", refused),
            ("continueProposalGeneration", conflicted),
            ("continueProposalGeneration", generation_started),
            ("continueProposalGeneration", refused),
            ("pauseAgentRun", activity_applied),
            ("pauseAgentRun", no_effect),
            ("pauseAgentRun", conflicted),
            ("cancelAgentRun", activity_applied),
            ("cancelAgentRun", no_effect),
            ("cancelAgentRun", conflicted),
            ("createAgentRun", activity_applied),
            ("steerAgentRun", steering_retained),
            ("steerAgentRun", steering_retained),
            ("steerAgentRun", conflicted),
            ("acceptProposal", invalid),
            ("acceptProposal", conflicted),
            ("acceptProposal", refused),
            ("acceptProposal", structural_applied),
        ]
    );
    // The child Receipt and validation condition rows of each Acceptance outcome.
    assert_eq!(acceptance_records, vec![[1, 1], [1, 1], [1, 0], [1, 0]]);
    assert_eq!(rejection_records, vec![1; 4]);
    assert_eq!(withdrawal_records, vec![1, 0, 0, 0]);
    assert_eq!(expansion_records, vec![[1, 1, 1], [0; 3], [0; 3], [0; 3]]);
}
