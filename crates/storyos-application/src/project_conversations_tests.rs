use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use storyos_contracts::{
    PROJECT_CONVERSATION_LABEL_MAX_CHARACTERS, PROJECT_CONVERSATION_LIST_MAX_ENTRIES,
    PROJECT_CONVERSATION_RUNS_MAX_ENTRIES, ProjectConversationCompleteness,
};
use storyos_core::{AGENT_PLAN_STORED_RESPONSE_RETENTION, AgentRunStatus};

use super::{
    ProjectConversationReader, ProjectConversationRunItem, StoredProjectConversation,
    StoredProjectConversationRuns, list_project_conversations, project_conversation_label,
    read_project_conversation,
};
use crate::{ProjectId, ProjectReadError, ProjectScope, UserId};

/// Gives the first `limit` stored items, as the store does, and keeps each `limit` it got.
struct FixtureReader {
    conversations: usize,
    runs: usize,
    newest_status: AgentRunStatus,
    elapsed: Duration,
    limits: Mutex<Vec<usize>>,
}

impl FixtureReader {
    fn new(conversations: usize, runs: usize) -> Self {
        Self {
            conversations,
            runs,
            newest_status: AgentRunStatus::Completed,
            elapsed: Duration::ZERO,
            limits: Mutex::new(Vec::new()),
        }
    }
}

fn end() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(/*secs*/ 1_800_000_000)
}

impl ProjectConversationReader for FixtureReader {
    async fn read_project_conversations(
        &self,
        _scope: &ProjectScope,
        limit: usize,
    ) -> Result<Option<Vec<StoredProjectConversation>>, ProjectReadError> {
        self.limits.lock().unwrap().push(limit);
        Ok(Some(
            (0..self.conversations.min(limit))
                .map(|index| StoredProjectConversation {
                    conversation_id: format!("conversation-{index}"),
                    first_author_message: format!("message {index}"),
                    first_run_created_at: "2026-11-16T09:30:00.000Z".to_owned(),
                    newest_run_created_at: "2026-11-16T10:15:00.000Z".to_owned(),
                    newest_run_status: AgentRunStatus::Completed,
                })
                .collect(),
        ))
    }

    async fn read_project_conversation(
        &self,
        _scope: &ProjectScope,
        _conversation_id: &str,
        limit: usize,
    ) -> Result<Option<StoredProjectConversationRuns>, ProjectReadError> {
        self.limits.lock().unwrap().push(limit);
        let newest_first = (0..self.runs.min(limit))
            .map(|index| ProjectConversationRunItem {
                run_id: format!("run-{}", self.runs - index),
                author_message: format!("message {index}"),
                chapter_id: "chapter-a".to_owned(),
                status: if index == 0 {
                    self.newest_status
                } else {
                    AgentRunStatus::Completed
                },
                created_at: "2026-11-16T10:15:00.000Z".to_owned(),
            })
            .collect();
        Ok(Some(StoredProjectConversationRuns {
            newest_first,
            newest_run_last_recorded_at: end(),
            read_time: end() + self.elapsed,
        }))
    }
}

fn scope() -> ProjectScope {
    ProjectScope::new(UserId::new("user-a"), ProjectId::new("project-a"))
}

#[tokio::test]
async fn the_list_keeps_the_newest_entries_at_its_bound() {
    let mut results = Vec::new();
    for count in [
        0,
        PROJECT_CONVERSATION_LIST_MAX_ENTRIES,
        PROJECT_CONVERSATION_LIST_MAX_ENTRIES + 1,
    ] {
        let reader = FixtureReader::new(count, 0);
        let list = list_project_conversations(&reader, &scope())
            .await
            .unwrap()
            .unwrap();
        results.push((
            list.items.len(),
            list.items.last().map(|item| item.conversation_id.clone()),
            list.completeness,
            reader.limits.into_inner().unwrap(),
        ));
    }

    assert_eq!(
        results,
        [
            (0, None, ProjectConversationCompleteness::Complete, vec![65]),
            (
                64,
                Some("conversation-63".to_owned()),
                ProjectConversationCompleteness::Complete,
                vec![65]
            ),
            (
                64,
                Some("conversation-63".to_owned()),
                ProjectConversationCompleteness::Truncated,
                vec![65]
            ),
        ]
    );
}

#[tokio::test]
async fn the_read_keeps_the_newest_runs_at_its_bound_with_the_oldest_of_them_first() {
    let mut results = Vec::new();
    for count in [
        1,
        PROJECT_CONVERSATION_RUNS_MAX_ENTRIES,
        PROJECT_CONVERSATION_RUNS_MAX_ENTRIES + 1,
    ] {
        let reader = FixtureReader::new(0, count);
        let read = read_project_conversation(&reader, &scope(), "conversation-a")
            .await
            .unwrap()
            .unwrap();
        results.push((
            read.runs.len(),
            read.runs.first().unwrap().run_id.clone(),
            read.runs.last().unwrap().run_id.clone(),
            read.completeness,
            reader.limits.into_inner().unwrap(),
        ));
    }

    assert_eq!(
        results,
        [
            (
                1,
                "run-1".to_owned(),
                "run-1".to_owned(),
                ProjectConversationCompleteness::Complete,
                vec![65]
            ),
            (
                64,
                "run-1".to_owned(),
                "run-64".to_owned(),
                ProjectConversationCompleteness::Complete,
                vec![65]
            ),
            (
                64,
                "run-2".to_owned(),
                "run-65".to_owned(),
                ProjectConversationCompleteness::Truncated,
                vec![65]
            ),
        ]
    );
}

#[tokio::test]
async fn the_read_gives_the_core_expiry_fact_of_the_newest_run() {
    let mut results = Vec::new();
    let longer = AGENT_PLAN_STORED_RESPONSE_RETENTION + Duration::from_millis(/*millis*/ 1);
    for (status, elapsed) in [
        (
            AgentRunStatus::Completed,
            AGENT_PLAN_STORED_RESPONSE_RETENTION,
        ),
        (AgentRunStatus::Completed, longer),
        (AgentRunStatus::Paused, longer),
    ] {
        let reader = FixtureReader {
            newest_status: status,
            elapsed,
            ..FixtureReader::new(0, 2)
        };
        let read = read_project_conversation(&reader, &scope(), "conversation-a")
            .await
            .unwrap()
            .unwrap();
        results.push(read.earlier_turns_expired);
    }

    assert_eq!(results, [false, true, false]);
}

#[test]
fn the_label_is_the_start_of_the_first_message_cut_at_a_character_boundary() {
    let at_bound = "雨".repeat(PROJECT_CONVERSATION_LABEL_MAX_CHARACTERS);
    let longer = format!("{at_bound}落");

    assert_eq!(
        [
            project_conversation_label("Help with this passage."),
            project_conversation_label(&at_bound),
            project_conversation_label(&longer),
        ],
        [
            "Help with this passage.".to_owned(),
            at_bound.clone(),
            at_bound,
        ]
    );
}
