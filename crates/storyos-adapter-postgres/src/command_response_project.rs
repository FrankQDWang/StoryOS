use storyos_application::{ChapterId, Project, ProjectId};

pub(crate) const COMMAND_RESPONSE_PROJECT_FORMAT: &str = "command_response_project.v1";

pub(crate) enum CommandResponseProjectEvidence {
    HistoricalUnavailable,
    Captured(Project),
}

pub(crate) fn encode_command_response_project(project: &Project) -> String {
    let open = match &project.current_chapter_id {
        Some(chapter_id) => serde_json::json!({
            "kind": "current_chapter",
            "current_chapter_id": chapter_id.as_ref(),
        }),
        None => serde_json::json!({ "kind": "empty" }),
    };
    serde_json::json!({
        "project_id": project.project_id.as_ref(),
        "title": project.title,
        "open": open,
    })
    .to_string()
}

pub(crate) fn read_command_response_project(
    format: Option<&str>,
    payload: Option<&str>,
) -> Result<CommandResponseProjectEvidence, ()> {
    match (format, payload) {
        (None, None) => Ok(CommandResponseProjectEvidence::HistoricalUnavailable),
        (Some(COMMAND_RESPONSE_PROJECT_FORMAT), Some(payload)) => Ok(
            CommandResponseProjectEvidence::Captured(parse_command_response_project(payload)?),
        ),
        _ => Err(()),
    }
}

fn parse_command_response_project(payload: &str) -> Result<Project, ()> {
    let value: serde_json::Value = serde_json::from_str(payload).map_err(|_| ())?;
    let object = value.as_object().ok_or(())?;
    if object.len() != 3 {
        return Err(());
    }
    let project_id = object.get("project_id").and_then(serde_json::Value::as_str);
    let title = object.get("title").and_then(serde_json::Value::as_str);
    let open = object.get("open").and_then(serde_json::Value::as_object);
    let (Some(project_id), Some(title), Some(open)) = (project_id, title, open) else {
        return Err(());
    };
    let current_chapter_id = match open.get("kind").and_then(serde_json::Value::as_str) {
        Some("empty") if open.len() == 1 => None,
        Some("current_chapter") if open.len() == 2 => Some(ChapterId::new(
            open.get("current_chapter_id")
                .and_then(serde_json::Value::as_str)
                .ok_or(())?,
        )),
        _ => return Err(()),
    };
    Ok(Project {
        project_id: ProjectId::new(project_id),
        title: title.to_owned(),
        current_chapter_id,
    })
}
