//! The two queries of the Project Conversations of a Project (ADR 0094 section 10). The Host
//! bounds and the label rule are here.

use std::future::Future;

use storyos_contracts::{
    PROJECT_CONVERSATION_LABEL_MAX_CHARACTERS, PROJECT_CONVERSATION_LIST_MAX_ENTRIES,
    PROJECT_CONVERSATION_RUNS_MAX_ENTRIES, ProjectConversationCompleteness,
};
use storyos_core::AgentRunStatus;

use crate::{ProjectReadError, ProjectScope};

/// One Run of a Project Conversation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectConversationRunItem {
    pub run_id: String,
    pub author_message: String,
    pub chapter_id: String,
    pub status: AgentRunStatus,
    pub created_at: String,
}

/// Reads the Project Conversations under an already authenticated exact Project Scope. Each read
/// writes no row.
pub trait ProjectConversationReader: Sync {
    /// Gives at most `limit` Conversations, the Conversation with the newest Run first. Gives
    /// `None` when the Project is not visible in the Project Scope.
    fn read_project_conversations(
        &self,
        scope: &ProjectScope,
        limit: usize,
    ) -> impl Future<Output = Result<Option<Vec<ProjectConversationListItem>>, ProjectReadError>> + Send;

    /// Gives at most `limit` Runs of the Conversation, the newest first in Run identity order.
    /// Gives `None` when the Conversation is not visible in the Project Scope.
    fn read_project_conversation(
        &self,
        scope: &ProjectScope,
        conversation_id: &str,
        limit: usize,
    ) -> impl Future<Output = Result<Option<Vec<ProjectConversationRunItem>>, ProjectReadError>> + Send;
}

/// One entry of the Conversation list. The first Run and the newest Run are the first and the last
/// Run in Run identity order. The times are the Receipt times of their `createAgentRun` commands.
/// The reader gives the whole first author message as `label`; the list cuts it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectConversationListItem {
    pub conversation_id: String,
    pub label: String,
    pub first_run_created_at: String,
    pub newest_run_created_at: String,
    pub newest_run_status: AgentRunStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectConversationList {
    pub items: Vec<ProjectConversationListItem>,
    pub completeness: ProjectConversationCompleteness,
}

/// The Runs of one Conversation, the oldest first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectConversationRead {
    pub runs: Vec<ProjectConversationRunItem>,
    pub completeness: ProjectConversationCompleteness,
}

/// The label of a Conversation: the start of its first author message, at most
/// `PROJECT_CONVERSATION_LABEL_MAX_CHARACTERS` characters, cut at a character boundary.
pub fn project_conversation_label(first_author_message: &str) -> String {
    first_author_message
        .chars()
        .take(PROJECT_CONVERSATION_LABEL_MAX_CHARACTERS)
        .collect()
}

/// Keeps the first `max` items of a list in newest-first order.
fn bounded<T>(mut items: Vec<T>, max: usize) -> (Vec<T>, ProjectConversationCompleteness) {
    if items.len() > max {
        items.truncate(max);
        (items, ProjectConversationCompleteness::Truncated)
    } else {
        (items, ProjectConversationCompleteness::Complete)
    }
}

pub async fn list_project_conversations(
    reader: &impl ProjectConversationReader,
    scope: &ProjectScope,
) -> Result<Option<ProjectConversationList>, ProjectReadError> {
    let Some(stored) = reader
        .read_project_conversations(scope, PROJECT_CONVERSATION_LIST_MAX_ENTRIES + 1)
        .await?
    else {
        return Ok(None);
    };
    let (mut items, completeness) = bounded(stored, PROJECT_CONVERSATION_LIST_MAX_ENTRIES);
    for item in &mut items {
        item.label = project_conversation_label(&item.label);
    }
    let list = ProjectConversationList {
        items,
        completeness,
    };
    Ok(Some(list))
}

pub async fn read_project_conversation(
    reader: &impl ProjectConversationReader,
    scope: &ProjectScope,
    conversation_id: &str,
) -> Result<Option<ProjectConversationRead>, ProjectReadError> {
    let Some(newest_first) = reader
        .read_project_conversation(
            scope,
            conversation_id,
            PROJECT_CONVERSATION_RUNS_MAX_ENTRIES + 1,
        )
        .await?
    else {
        return Ok(None);
    };
    let (mut runs, completeness) = bounded(newest_first, PROJECT_CONVERSATION_RUNS_MAX_ENTRIES);
    runs.reverse();
    Ok(Some(ProjectConversationRead { runs, completeness }))
}

#[cfg(test)]
#[path = "project_conversations_tests.rs"]
mod tests;
