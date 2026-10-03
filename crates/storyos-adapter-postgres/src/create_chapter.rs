use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};
use storyos_application::{
    ChapterId, CreateChapterAuthority, CreateChapterCommand, CreateChapterError,
    CreateChapterPublicOrder, CreateChapterSettlement, CreateChapterSettlementEffect,
    CreateChapterStore, Project, ProjectCommandChallengeError, ProjectCommandChallengeUse,
};
use storyos_core::{
    CreateChapter as CoreCreateChapter, CreateChapterApplied, CreateChapterCurrent,
    CreateChapterOpen, CreateChapterResult, ProjectLifecycle, VolumeJoin,
    create_chapter as classify_create_chapter,
};
use uuid::Uuid;

use crate::structural_authority_settlement::receipt_reason_payload;

use super::*;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
use storyos_core::TransitionOutcome;

mod persist;
use persist::{
    insert_create_chapter_admission, insert_created_chapter_object, next_chapter_order,
    persist_created_chapter,
};

impl CreateChapterStore for PostgresProjectReader {
    async fn create_chapter(
        &self,
        command: &CreateChapterCommand,
    ) -> Result<CreateChapterSettlement, CreateChapterError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(create_chapter_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(create_chapter_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(create_chapter_challenge_error)?;
                read_create_chapter_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(create_chapter_challenge_error)?;
                Err(CreateChapterError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_create_chapter(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(create_chapter_challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        let _rollback = transaction.rollback().await;
                        Err(error)
                    }
                }
            }
        }
    }
}

async fn persist_create_chapter(
    client: &tokio_postgres::Client,
    command: &CreateChapterCommand,
) -> Result<CreateChapterSettlement, CreateChapterError> {
    let row = client
        .query_opt(
            "SELECT lifecycle_state, tree_revision::text, current_chapter_id::text, title
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(create_chapter_database_error)?;
    let Some(row) = row else {
        return Err(CreateChapterError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(CreateChapterError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let current_tree_revision = row
        .get::<_, String>(1)
        .parse::<u64>()
        .map_err(create_chapter_parse_error)?;
    let current_chapter_id = row.get::<_, Option<String>>(2);
    let current_title = row.get::<_, String>(3);
    let volume_join = if client
        .query_opt(
            "SELECT manuscript_object_id
               FROM storyos.manuscript_objects AS volume
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $3::text::uuid AND object_kind = 'volume'
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.volume_removal_decisions AS removal
                   WHERE removal.owner_user_id = volume.owner_user_id
                     AND removal.project_id = volume.project_id
                     AND removal.volume_id = volume.manuscript_object_id
                )",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.volume_id,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?
        .is_some()
    {
        VolumeJoin::ExactScope
    } else {
        VolumeJoin::Invalid
    };
    let mut ordered_chapter_ids = client.query(
        "SELECT manuscript_object_id::text FROM storyos.manuscript_objects AS chapter
          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
            AND object_kind = 'chapter' AND parent_volume_id = $3::text::uuid
            AND NOT EXISTS (SELECT 1 FROM storyos.chapter_removal_decisions AS removal
              WHERE removal.owner_user_id = chapter.owner_user_id AND removal.project_id = chapter.project_id
                AND removal.chapter_id = chapter.manuscript_object_id)
          ORDER BY tree_order FOR UPDATE",
        &[&command.project_scope.owner_user_id.as_ref(), &command.project_scope.project_id.as_ref(), &command.volume_id],
    ).await.map_err(create_chapter_database_error)?.iter().map(|row| row.get::<_, String>(0)).collect::<Vec<_>>();
    let classified = classify_create_chapter(&CoreCreateChapter {
        volume_join,
        expected_tree_revision: command.expected_tree_revision,
        current_tree_revision,
        current_lifecycle,
        current_open: match current_chapter_id {
            None => CreateChapterOpen::Empty,
            Some(_) => CreateChapterOpen::CurrentChapter,
        },
        title: command.title.clone(),
        placement: command.placement.clone(),
        ordered_chapter_ids: ordered_chapter_ids.clone(),
    });
    let receipt_result = classified.receipt_result().code();
    let receipt_reason = classified.reason_code();
    let effect = match classified {
        CreateChapterResult::Applied(CreateChapterApplied {
            tree_revision,
            current,
            order,
        }) => {
            let tree_order = next_chapter_order(client, command).await?;
            let chapter_id = Uuid::now_v7().to_string();
            insert_created_chapter_object(client, command, &chapter_id, tree_order).await?;
            if command.placement != storyos_core::CreateChapterPlacement::Append {
                ordered_chapter_ids.insert((order - 1) as usize, chapter_id.clone());
                crate::update_chapter::sibling_order::write_chapter_order(
                    client,
                    &command.project_scope,
                    &command.volume_id,
                    &ordered_chapter_ids,
                )
                .await
                .map_err(create_chapter_database_error)?;
            }
            CreateChapterSettlementEffect::Applied {
                tree_revision,
                chapter_id,
                current,
                order: CreateChapterPublicOrder::CanonicalSiblingOrder(order),
            }
        }
        CreateChapterResult::NoEffect(reason) => match reason {},
        CreateChapterResult::Conflicted(reason) => {
            CreateChapterSettlementEffect::Conflicted { reason }
        }
        CreateChapterResult::Refused(reason) => CreateChapterSettlementEffect::Refused { reason },
    };
    insert_create_chapter_admission(client, command).await?;
    let authority_sequences = match &effect {
        CreateChapterSettlementEffect::Applied { .. } => {
            let sequences =
                crate::structural_authority_settlement::allocate_structure_transition_sequences(
                    client,
                    &command.project_scope,
                )
                .await
                .map_err(CreateChapterError::Unavailable)?;
            Some(sequences)
        }
        CreateChapterSettlementEffect::Conflicted { .. }
        | CreateChapterSettlementEffect::Refused { .. } => None,
    };
    let receipt_payload = match &effect {
        CreateChapterSettlementEffect::Applied {
            order: CreateChapterPublicOrder::CanonicalSiblingOrder(order),
            ..
        } => format!(r#"{{"order":"{order}"}}"#),
        CreateChapterSettlementEffect::Applied {
            order: CreateChapterPublicOrder::HistoricalCreateChapterAck(_),
            ..
        } => {
            return Err(CreateChapterError::Unavailable(Box::new(
                std::io::Error::other("Create Chapter first use cannot replay a historical ack"),
            )));
        }
        CreateChapterSettlementEffect::Conflicted { .. }
        | CreateChapterSettlementEffect::Refused { .. } => receipt_reason_payload(receipt_reason),
    };
    let commit_ids = authority_sequences
        .as_ref()
        .map(|sequences| vec![sequences.authoritative_commit_id.clone()])
        .unwrap_or_default();
    let receipt_created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'createChapter', $6, $7::text::uuid,
                     'author_command_admission', '{}'::uuid[], '{}'::uuid[], '{}'::uuid[],
                     '{}'::uuid[], '{}'::uuid[], $10::text[]::uuid[], '{}'::text[], '{}'::text[],
                     '{}'::text[], $8, $9::text::jsonb)
          RETURNING to_char(created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &receipt_result,
                &receipt_payload,
                &commit_ids,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?
        .get::<_, String>(0);
    client
        .execute(
            "INSERT INTO storyos.author_command_admission_settlements
               (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'receipt_settled', $4::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    let mut authority = None;
    let mut response_chapter_id = current_chapter_id.clone().map(ChapterId::new);
    if let (
        CreateChapterSettlementEffect::Applied {
            tree_revision,
            chapter_id,
            current,
            order: CreateChapterPublicOrder::CanonicalSiblingOrder(order),
        },
        Some(sequences),
    ) = (&effect, authority_sequences)
    {
        let resulting_revision_id =
            persist_created_chapter(client, command, tree_revision, chapter_id, current).await?;
        project_activity_position = sequences.project_activity_position;
        project_activity_event_id = sequences.project_activity_event_id.clone();
        let resulting_current = client
            .query_one(
                "SELECT current_chapter_id::text FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                ],
            )
            .await
            .map_err(create_chapter_database_error)?
            .get::<_, String>(0);
        response_chapter_id = Some(ChapterId::new(resulting_current.clone()));
        let payload = serde_json::json!({
            "kind": "chapter_created",
            "volume_id": command.volume_id,
            "chapter_id": chapter_id,
            "title": command.title,
            "tree_revision": tree_revision.to_string(),
            "order": order.to_string(),
            "current_chapter_id": resulting_current,
        })
        .to_string();
        client
            .execute(
                "INSERT INTO storyos.project_activity_event_payloads
                   (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                    event_kind, receipt_id, receipt_result_kind, payload)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                         'chapter_created', $5::text::uuid, 'authoritative_applied',
                         $6::text::jsonb)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &project_activity_position.to_string(),
                    &project_activity_event_id,
                    &command.ids.receipt_id,
                    &payload,
                ],
            )
            .await
            .map_err(create_chapter_database_error)?;
        crate::structural_authority_settlement::persist_structure_commit(
            client,
            &command.project_scope,
            &sequences,
            &command.ids.author_command_admission_id,
            &command.ids.receipt_id,
            crate::structural_authority_settlement::StructureCommitBinding {
                prior_manuscript_tree_revision: command.expected_tree_revision,
                resulting_manuscript_tree_revision: *tree_revision,
                identity:
                    crate::structural_authority_settlement::StructureAffectedIdentity::ChapterInitialRevision {
                        chapter_id,
                        resulting_revision_id: &resulting_revision_id,
                    },
            },
        )
        .await
        .map_err(create_chapter_database_error)?;
        crate::structural_authority_settlement::persist_forward_author_action(
            client,
            &command.project_scope,
            &sequences,
            &command.ids.receipt_id,
        )
        .await
        .map_err(create_chapter_database_error)?;
        crate::snapshot::persist_canonical_snapshot(
            client,
            &command.project_scope,
            &sequences.snapshot_id,
            project_activity_position,
        )
        .await
        .map_err(create_chapter_database_error)?;
        authority = Some(CreateChapterAuthority {
            authoritative_commit_id: sequences.authoritative_commit_id,
            author_action_sequence: sequences.author_action_sequence,
            snapshot_id: sequences.snapshot_id,
            prior_manuscript_tree_revision: command.expected_tree_revision,
            resulting_manuscript_tree_revision: *tree_revision,
            resulting_revision_id,
        });
    }
    let response_project = Project {
        project_id: command.project_scope.project_id.clone(),
        title: current_title,
        current_chapter_id: response_chapter_id,
    };
    let encoded_project = encode_command_response_project(&response_project);
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled',
                    result_reference = $3,
                    acknowledgement_format = $5,
                    response_project = $6::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'createChapter' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
                &COMMAND_RESPONSE_PROJECT_FORMAT,
                &encoded_project,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?;
    Ok(CreateChapterSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
        authority,
        response_project,
    })
}

async fn read_create_chapter_settlement(
    store: &PostgresProjectReader,
    command: &CreateChapterCommand,
    receipt_id: &str,
) -> Result<CreateChapterSettlement, CreateChapterError> {
    read_command_replay(store, &command.challenge_binding, receipt_id)
        .await
        .and_then(create_chapter_replay)
        .map_err(|fault| match fault {
            ReplayFault::BindingConflict => CreateChapterError::BindingConflict,
            ReplayFault::HistoricalAcknowledgementUnavailable => {
                CreateChapterError::HistoricalAcknowledgementUnavailable
            }
            ReplayFault::Unavailable(source) => CreateChapterError::Unavailable(source),
        })
}

fn create_chapter_replay(replay: CommandReplay) -> Result<CreateChapterSettlement, ReplayFault> {
    let effect = match replay.outcome::<std::convert::Infallible, _, _>()? {
        TransitionOutcome::Applied(()) => {
            let tree_revision = replay.activity_u64("tree_revision")?;
            let chapter_id = replay.activity_text("chapter_id")?;
            let resulting_current = replay.activity_text("current_chapter_id")?;
            let activity_order = replay.activity_u64("order")?;
            let order = match replay.receipt_text("order") {
                Some(order) => {
                    let rank = order
                        .parse::<u64>()
                        .map_err(|error| ReplayFault::Unavailable(Box::new(error)))?;
                    if rank < 1 {
                        return Err(ReplayFault::BindingConflict);
                    }
                    CreateChapterPublicOrder::CanonicalSiblingOrder(rank)
                }
                None => CreateChapterPublicOrder::HistoricalCreateChapterAck(activity_order),
            };
            CreateChapterSettlementEffect::Applied {
                tree_revision,
                current: if resulting_current == chapter_id {
                    CreateChapterCurrent::SelectCreated
                } else {
                    CreateChapterCurrent::PreserveExisting
                },
                chapter_id,
                order,
            }
        }
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(reason) => {
            CreateChapterSettlementEffect::Conflicted { reason }
        }
        TransitionOutcome::Refused(reason) => CreateChapterSettlementEffect::Refused { reason },
    };
    let response_project = replay.response_project()?;
    Ok(CreateChapterSettlement {
        authority: replay.authority.and_then(|authority| {
            Some(CreateChapterAuthority {
                resulting_revision_id: authority.resulting_revision_id?,
                authoritative_commit_id: authority.authoritative_commit_id,
                author_action_sequence: authority.author_action_sequence,
                snapshot_id: authority.snapshot_id,
                prior_manuscript_tree_revision: authority.prior_manuscript_tree_revision,
                resulting_manuscript_tree_revision: authority.resulting_manuscript_tree_revision,
            })
        }),
        ids: replay.ids,
        receipt_created_at: replay.receipt_created_at,
        effect,
        project_activity_position: replay.project_activity_position,
        project_activity_event_id: replay.project_activity_event_id,
        response_project,
    })
}

pub(super) fn create_chapter_challenge_error(
    error: ProjectCommandChallengeError,
) -> CreateChapterError {
    match error {
        ProjectCommandChallengeError::BindingConflict => CreateChapterError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => CreateChapterError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            CreateChapterError::Unavailable(Box::new(error))
        }
    }
}

pub(super) fn create_chapter_database_error(error: tokio_postgres::Error) -> CreateChapterError {
    CreateChapterError::Unavailable(Box::new(error))
}

pub(super) fn create_chapter_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> CreateChapterError {
    CreateChapterError::Unavailable(Box::new(error))
}
