use std::time::{Duration, SystemTime};

use storyos_contracts::{
    PROJECT_CONVERSATION_LABEL_MAX_CHARACTERS as LABEL, PROJECT_CONVERSATION_LIST_MAX_ENTRIES,
    PROJECT_CONVERSATION_RUNS_MAX_ENTRIES, ProjectConversationCompleteness as Completeness,
};
use storyos_core::{AGENT_PLAN_STORED_RESPONSE_RETENTION as RETENTION, AgentRunStatus};

use super::{
    ProjectConversationReader, ProjectConversationRunItem, StoredProjectConversation,
    StoredProjectConversationRuns, list_project_conversations, project_conversation_label,
    read_project_conversation,
};
use crate::{ProjectId, ProjectReadError, ProjectScope, UserId};

/// Gives the first `limit` of `count` stored items, as the store does. The newest Run has
/// `status` and ended `elapsed` before the read.
struct FixtureReader {
    count: usize,
    status: AgentRunStatus,
    elapsed: Duration,
}

fn reader(count: usize) -> FixtureReader {
    let (status, elapsed) = (AgentRunStatus::Completed, Duration::ZERO);
    FixtureReader {
        count,
        status,
        elapsed,
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
        assert_eq!(limit, PROJECT_CONVERSATION_LIST_MAX_ENTRIES + 1);
        let entry = |index| StoredProjectConversation {
            conversation_id: format!("conversation-{index}"),
            first_author_message: format!("message {index}"),
            first_run_created_at: "2026-11-16T09:30:00.000Z".to_owned(),
            newest_run_created_at: "2026-11-16T10:15:00.000Z".to_owned(),
            newest_run_status: AgentRunStatus::Completed,
        };
        Ok(Some((0..self.count.min(limit)).map(entry).collect()))
    }

    async fn read_project_conversation(
        &self,
        _scope: &ProjectScope,
        _conversation_id: &str,
        limit: usize,
    ) -> Result<Option<StoredProjectConversationRuns>, ProjectReadError> {
        assert_eq!(limit, PROJECT_CONVERSATION_RUNS_MAX_ENTRIES + 1);
        let run = |index| ProjectConversationRunItem {
            run_id: format!("run-{}", self.count - index),
            author_message: format!("message {index}"),
            chapter_id: "chapter-a".to_owned(),
            status: [self.status, AgentRunStatus::Completed][usize::from(index > 0)],
            created_at: "2026-11-16T10:15:00.000Z".to_owned(),
        };
        Ok(Some(StoredProjectConversationRuns {
            newest_first: (0..self.count.min(limit)).map(run).collect(),
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
    let max = PROJECT_CONVERSATION_LIST_MAX_ENTRIES;
    let mut results = Vec::new();
    for count in [0, max, max + 1] {
        let list = list_project_conversations(&reader(count), &scope()).await;
        let list = list.unwrap().unwrap();
        let last = list.items.last().map(|item| item.conversation_id.clone());
        results.push((list.items.len(), last, list.completeness));
    }

    let last = Some("conversation-63".to_owned());
    assert_eq!(
        results,
        [
            (0, None, Completeness::Complete),
            (64, last.clone(), Completeness::Complete),
            (64, last, Completeness::Truncated),
        ]
    );
}

#[tokio::test]
async fn the_read_keeps_the_newest_runs_at_its_bound_with_the_oldest_of_them_first() {
    let max = PROJECT_CONVERSATION_RUNS_MAX_ENTRIES;
    let mut results = Vec::new();
    for count in [1, max, max + 1] {
        let read = read_project_conversation(&reader(count), &scope(), "conversation-a").await;
        let read = read.unwrap().unwrap();
        let ids = [read.runs.first(), read.runs.last()].map(|run| run.unwrap().run_id.clone());
        results.push((read.runs.len(), ids, read.completeness));
    }

    let ids = |first: usize, last: usize| [format!("run-{first}"), format!("run-{last}")];
    assert_eq!(
        results,
        [
            (1, ids(1, 1), Completeness::Complete),
            (64, ids(1, 64), Completeness::Complete),
            (64, ids(2, 65), Completeness::Truncated),
        ]
    );
}

#[tokio::test]
async fn the_read_gives_the_core_expiry_fact_of_the_newest_run() {
    let longer = RETENTION + Duration::from_millis(/*millis*/ 1);
    let mut results = Vec::new();
    for (status, elapsed) in [
        (AgentRunStatus::Completed, RETENTION),
        (AgentRunStatus::Completed, longer),
        (AgentRunStatus::Paused, longer),
    ] {
        let fixture = FixtureReader {
            count: 2,
            status,
            elapsed,
        };
        let read = read_project_conversation(&fixture, &scope(), "conversation-a").await;
        results.push(read.unwrap().unwrap().earlier_turns_expired);
    }

    assert_eq!(results, [false, true, false]);
}

#[test]
fn the_label_is_the_start_of_the_first_message_cut_at_a_character_boundary() {
    let at_bound = "雨".repeat(LABEL);
    let labels = [
        "Help with this passage.",
        &at_bound,
        &format!("{at_bound}落"),
    ]
    .map(project_conversation_label);

    assert_eq!(labels, ["Help with this passage.", &at_bound, &at_bound]);
}
