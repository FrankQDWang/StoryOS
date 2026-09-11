use storyos_application::{UndoLatestAuthorActionCommand, UndoLatestAuthorActionError};
use storyos_core::AuthorUndoFrontierKind;

pub(super) enum ObservedFrontier {
    Prose(ObservedProseFrontier),
    Structure(ObservedStructureFrontier),
    Barrier { sequence: u64 },
}

pub(super) struct ObservedProseFrontier {
    pub sequence: u64,
    pub chapter_id: String,
    pub resulting_revision_id: String,
    pub prior_revision_id: String,
    pub prior_payload: String,
    pub current_head_revision_id: String,
}

pub(super) enum ObservedStructureIdentity {
    Volume {
        volume_id: String,
    },
    VolumeUpdate {
        volume_id: String,
        prior_title: String,
        prior_order: u64,
    },
    Chapter {
        chapter_id: String,
    },
    ChapterDelete {
        chapter_id: String,
        prior_current_chapter_id: Option<String>,
    },
    ChapterUpdate {
        chapter_id: String,
        prior_title: String,
        prior_order: u64,
    },
}

pub(super) struct ObservedStructureFrontier {
    pub sequence: u64,
    pub prior_manuscript_tree_revision: u64,
    pub resulting_manuscript_tree_revision: u64,
    pub identity: ObservedStructureIdentity,
}

pub(super) struct LoadedUndoFrontier {
    pub lifecycle_state: String,
    pub observed: Option<ObservedFrontier>,
}

pub(super) async fn load_observed_frontier(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
) -> Result<LoadedUndoFrontier, UndoLatestAuthorActionError> {
    let row = client
        .query_opt(
            "SELECT project.lifecycle_state,
                    frontier.author_action_sequence::text,
                    frontier.manuscript_object_id::text,
                    frontier.resulting_revision_id::text,
                    frontier.prior_revision_id::text,
                    convert_from(prior_payload.canonical_bytes, 'UTF8'),
                    head.current_revision_id::text,
                    frontier.prior_manuscript_tree_revision::text,
                    frontier.resulting_manuscript_tree_revision::text,
                    frontier.affected_volume_id::text,
                    frontier.affected_chapter_id::text,
                    frontier.command_kind,
                    frontier.prior_title,
                    frontier.prior_order,
                    frontier.prior_current_chapter_id
               FROM storyos.projects AS project
          LEFT JOIN LATERAL (
                SELECT action.author_action_sequence,
                       commit.manuscript_object_id,
                       commit.resulting_revision_id,
                       commit.prior_revision_id,
                       commit.prior_manuscript_tree_revision,
                       commit.resulting_manuscript_tree_revision,
                       commit.affected_volume_id,
                       commit.affected_chapter_id,
                       receipt.command_kind,
                       payload.payload->>'prior_title' AS prior_title,
                       payload.payload->>'prior_order' AS prior_order,
                       payload.payload->>'prior_current_chapter_id' AS prior_current_chapter_id
                  FROM storyos.author_action_entries AS action
                  JOIN storyos.authoritative_commits AS commit
                    ON (commit.owner_user_id, commit.project_id, commit.receipt_id,
                        commit.receipt_result_kind, commit.authoritative_commit_id) =
                       (action.owner_user_id, action.project_id, action.receipt_id,
                        action.receipt_result_kind, action.authoritative_commit_id)
                  JOIN storyos.domain_receipts AS receipt
                    ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                       (action.owner_user_id, action.project_id, action.receipt_id)
             LEFT JOIN storyos.project_activity_event_payloads AS payload
                    ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                       (action.owner_user_id, action.project_id, action.receipt_id)
             LEFT JOIN storyos.author_action_entries AS compensation
                    ON compensation.owner_user_id = action.owner_user_id
                   AND compensation.project_id = action.project_id
                   AND compensation.disposition = 'compensation'
                   AND compensation.compensated_source_sequence = action.author_action_sequence
                 WHERE action.owner_user_id = project.owner_user_id
                   AND action.project_id = project.project_id
                   AND action.disposition = 'forward'
                   AND compensation.author_action_sequence IS NULL
              ORDER BY action.author_action_sequence DESC
                 LIMIT 1
          ) AS frontier ON true
          LEFT JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (project.owner_user_id, project.project_id, frontier.manuscript_object_id)
          LEFT JOIN storyos.authoritative_revisions AS prior_revision
                 ON (prior_revision.owner_user_id, prior_revision.project_id,
                     prior_revision.manuscript_object_id, prior_revision.revision_id) =
                    (project.owner_user_id, project.project_id,
                     frontier.manuscript_object_id, frontier.prior_revision_id)
          LEFT JOIN storyos.authoritative_payloads AS prior_payload
                 ON (prior_payload.owner_user_id, prior_payload.project_id,
                     prior_payload.payload_id) =
                    (prior_revision.owner_user_id, prior_revision.project_id,
                     prior_revision.payload_id)
              WHERE project.owner_user_id = $1::text::uuid
                AND project.project_id = $2::text::uuid
              FOR UPDATE OF project",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let Some(row) = row else {
        return Err(UndoLatestAuthorActionError::MissingProject);
    };
    Ok(LoadedUndoFrontier {
        lifecycle_state: row.get(0),
        observed: observed_frontier(&row)?,
    })
}

fn observed_frontier(
    row: &tokio_postgres::Row,
) -> Result<Option<ObservedFrontier>, UndoLatestAuthorActionError> {
    let Some(sequence) = row.get::<_, Option<String>>(1) else {
        return Ok(None);
    };
    let sequence = sequence.parse().map_err(undo_parse_error)?;
    Ok(Some(
        match (
            row.get::<_, Option<String>>(2),
            row.get::<_, Option<String>>(3),
            row.get::<_, Option<String>>(4),
            row.get::<_, Option<String>>(5),
            row.get::<_, Option<String>>(6),
            row.get::<_, Option<String>>(7),
            row.get::<_, Option<String>>(8),
            row.get::<_, Option<String>>(9),
            row.get::<_, Option<String>>(10),
            row.get::<_, Option<String>>(11),
            row.get::<_, Option<String>>(12),
            row.get::<_, Option<String>>(13),
        ) {
            (
                Some(chapter_id),
                Some(resulting_revision_id),
                Some(prior_revision_id),
                Some(prior_payload),
                Some(current_head_revision_id),
                _,
                _,
                _,
                _,
                _,
                _,
                _,
            ) => ObservedFrontier::Prose(ObservedProseFrontier {
                sequence,
                chapter_id,
                resulting_revision_id,
                prior_revision_id,
                prior_payload,
                current_head_revision_id,
            }),
            (
                None,
                None,
                None,
                None,
                None,
                Some(prior_tree),
                Some(resulting_tree),
                Some(affected_volume_id),
                None,
                Some(command_kind),
                _,
                _,
            ) if command_kind == "createVolume" => {
                ObservedFrontier::Structure(ObservedStructureFrontier {
                    sequence,
                    prior_manuscript_tree_revision: prior_tree.parse().map_err(undo_parse_error)?,
                    resulting_manuscript_tree_revision: resulting_tree
                        .parse()
                        .map_err(undo_parse_error)?,
                    identity: ObservedStructureIdentity::Volume {
                        volume_id: affected_volume_id,
                    },
                })
            }
            (
                None,
                None,
                None,
                None,
                None,
                Some(prior_tree),
                Some(resulting_tree),
                Some(affected_volume_id),
                None,
                Some(command_kind),
                Some(prior_title),
                Some(prior_order),
            ) if command_kind == "updateVolume" => {
                ObservedFrontier::Structure(ObservedStructureFrontier {
                    sequence,
                    prior_manuscript_tree_revision: prior_tree.parse().map_err(undo_parse_error)?,
                    resulting_manuscript_tree_revision: resulting_tree
                        .parse()
                        .map_err(undo_parse_error)?,
                    identity: ObservedStructureIdentity::VolumeUpdate {
                        volume_id: affected_volume_id,
                        prior_title,
                        prior_order: prior_order.parse().map_err(undo_parse_error)?,
                    },
                })
            }
            (
                None,
                None,
                None,
                None,
                None,
                Some(prior_tree),
                Some(resulting_tree),
                None,
                Some(affected_chapter_id),
                Some(command_kind),
                Some(prior_title),
                Some(prior_order),
            ) if command_kind == "updateChapter" => {
                ObservedFrontier::Structure(ObservedStructureFrontier {
                    sequence,
                    prior_manuscript_tree_revision: prior_tree.parse().map_err(undo_parse_error)?,
                    resulting_manuscript_tree_revision: resulting_tree
                        .parse()
                        .map_err(undo_parse_error)?,
                    identity: ObservedStructureIdentity::ChapterUpdate {
                        chapter_id: affected_chapter_id,
                        prior_title,
                        prior_order: prior_order.parse().map_err(undo_parse_error)?,
                    },
                })
            }
            (
                None,
                None,
                None,
                None,
                None,
                Some(prior_tree),
                Some(resulting_tree),
                None,
                Some(affected_chapter_id),
                Some(command_kind),
                _,
                _,
            ) if command_kind == "deleteChapter" => {
                ObservedFrontier::Structure(ObservedStructureFrontier {
                    sequence,
                    prior_manuscript_tree_revision: prior_tree.parse().map_err(undo_parse_error)?,
                    resulting_manuscript_tree_revision: resulting_tree
                        .parse()
                        .map_err(undo_parse_error)?,
                    identity: ObservedStructureIdentity::ChapterDelete {
                        chapter_id: affected_chapter_id,
                        prior_current_chapter_id: row.get(14),
                    },
                })
            }
            (
                Some(object_id),
                Some(_resulting_revision_id),
                None,
                None,
                Some(_current_head),
                Some(prior_tree),
                Some(resulting_tree),
                None,
                Some(affected_chapter_id),
                _,
                _,
                _,
            ) if object_id == affected_chapter_id => {
                ObservedFrontier::Structure(ObservedStructureFrontier {
                    sequence,
                    prior_manuscript_tree_revision: prior_tree.parse().map_err(undo_parse_error)?,
                    resulting_manuscript_tree_revision: resulting_tree
                        .parse()
                        .map_err(undo_parse_error)?,
                    identity: ObservedStructureIdentity::Chapter {
                        chapter_id: affected_chapter_id,
                    },
                })
            }
            _ => ObservedFrontier::Barrier { sequence },
        },
    ))
}

impl ObservedFrontier {
    pub(super) fn sequence(&self) -> u64 {
        match self {
            Self::Prose(frontier) => frontier.sequence,
            Self::Structure(frontier) => frontier.sequence,
            Self::Barrier { sequence } => *sequence,
        }
    }

    pub(super) fn kind(&self) -> AuthorUndoFrontierKind {
        match self {
            Self::Prose(frontier) => AuthorUndoFrontierKind::ReversibleDirectAuthorAction {
                resulting_revision_id: frontier.resulting_revision_id.clone(),
            },
            Self::Structure(_) => AuthorUndoFrontierKind::ReversibleStructureTransition,
            Self::Barrier { .. } => AuthorUndoFrontierKind::Barrier,
        }
    }

    pub(super) fn prose_head(&self) -> Option<&str> {
        match self {
            Self::Prose(frontier) => Some(frontier.current_head_revision_id.as_str()),
            Self::Structure(_) | Self::Barrier { .. } => None,
        }
    }
}

fn undo_database_error(error: tokio_postgres::Error) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}

fn undo_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}
