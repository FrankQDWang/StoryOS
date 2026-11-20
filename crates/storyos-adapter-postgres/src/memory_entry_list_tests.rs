//! The query of the Memory section of the settings page (ADR 0094 section 9): the order of the
//! Memory Index, the current version of each entry, the source and its availability, the Project
//! Scope, the Conversation Memory Settings, and the Host limits.

use storyos_application::{
    MemoryEntrySourceItem, ProjectId, ProjectScope, UserId, list_memory_entries,
};
use storyos_core::{
    MEMORY_CHARACTER_TEXT_MAX_CHARS, MEMORY_DESCRIPTION_MAX_CHARS, MEMORY_TITLE_MAX_CHARS,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::queued;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_structure::titled_chapter;
use crate::shared_tests::command_sequence_support::seed_project;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::delete_structure::settle_delete_chapter;

/// Writes each version of `rows` in the Project of `scope` with SQL, and moves the head of the
/// Project. A row is one line of the number, the version, the Memory revision, the state, the
/// type, the origin, the title, and the text, with the source references as JSON. The
/// description is the title with a mark. The values are test constants.
async fn write(admin: &Client, scope: &ProjectScope, rows: &[(&str, String)]) {
    let (owner, project) = (scope.owner_user_id.as_ref(), scope.project_id.as_ref());
    for (line, sources) in rows {
        let fields: Vec<&str> = line.split(' ').collect();
        admin
            .execute(
                "INSERT INTO storyos.memory_entry_versions
                     (owner_user_id, project_id, memory_entry_id, entry_number, version,
                      memory_revision, state, entry_type, origin, title, description, text, cause,
                      source_references)
                 VALUES ($1::text::uuid, $2::text::uuid, md5($2 || ':' || $3)::uuid,
                         $3::text::numeric, $4::text::numeric, $5::text::numeric, $6, $7, $8, $9,
                         $9 || '。', $10, '{}', $11::text::jsonb)",
                &[
                    &owner, &project, &fields[0], &fields[1], &fields[2], &fields[3], &fields[4],
                    &fields[5], &fields[6], &fields[7], sources,
                ],
            )
            .await
            .unwrap();
    }
    admin
        .execute(
            "INSERT INTO storyos.project_memory_heads
             SELECT owner_user_id, project_id, max(memory_revision), max(entry_number) + 1
               FROM storyos.memory_entry_versions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              GROUP BY owner_user_id, project_id
             ON CONFLICT (owner_user_id, project_id) DO UPDATE
                SET memory_revision = EXCLUDED.memory_revision,
                    next_entry_number = EXCLUDED.next_entry_number",
            &[&owner, &project],
        )
        .await
        .unwrap();
}

fn conversation(id: &str) -> String {
    format!(r#"[{{"kind":"conversation","conversation_id":"{id}"}}]"#)
}

fn chapter(id: &str) -> String {
    format!(r#"[{{"kind":"chapter","chapter_id":"{id}"}}]"#)
}

/// Each listed entry as one line: the number, the version, the type, the origin, the title, the
/// text, and the source with its availability.
async fn listed(store: &PostgresProjectReader, scope: &ProjectScope) -> Option<Vec<String>> {
    let entries = list_memory_entries(store, scope).await.unwrap()?;
    let line = |entry: storyos_application::MemoryEntryListItem| {
        let source = match entry.source {
            None => "none".to_owned(),
            Some(MemoryEntrySourceItem::Conversation {
                conversation_id,
                available,
            }) => {
                format!("conversation {conversation_id} {available}")
            }
            Some(MemoryEntrySourceItem::Chapter {
                chapter_id,
                title,
                available,
            }) => {
                format!("chapter {chapter_id} {title:?} {available}")
            }
        };
        let (kind, origin) = (entry.entry_type.as_str(), entry.origin.as_str());
        let (number, version, title, text) = (entry.number, entry.version, entry.title, entry.text);
        format!("{number} {version} {kind} {origin} {title} {text} {source}")
    };
    Some(entries.into_iter().map(line).collect())
}

/// The Conversation of the Run `run_id`.
async fn conversation_of(admin: &Client, run_id: &str) -> String {
    admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(0)
}

/// The list gives each current entry in the order of the Memory Index (the type list, then the
/// number) with its current version. A deleted entry and an earlier version are not in it. The
/// source is the first reference: a Conversation of the Project and a live Chapter are available
/// with the title of the Chapter, and a deleted Chapter and a Conversation that the Project does
/// not have are unavailable and do not fail the query. A version with no reference has no source.
/// A Project with no entry gives an empty list, and a Project that is not visible gives none.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_list_has_the_current_entries_in_index_order_with_their_sources() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, run_id) = queued(&store, "b71").await;
    complete(&store, &admin, &scope, &run_id).await;
    let talk = conversation_of(&admin, &run_id).await;
    let volume: String = admin
        .query_one(
            "SELECT parent_volume_id::text FROM storyos.manuscript_objects
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(0);
    let removed = titled_chapter(&store, &scope, "b750", &volume, "Chapter B", 3).await;
    settle_delete_chapter(&store, &scope, "b751", &removed, 4).await;
    let other = "018f0000-0000-7001-8000-0000000b71ff";
    let rows = [
        (
            "1 1 1 current character author_request 林雪 旧文",
            conversation(&talk),
        ),
        (
            "2 1 2 current other chapter 旧渡口 北岸",
            chapter(&chapter_id),
        ),
        (
            "3 1 3 current foreshadowing chapter 铜铃 铃响",
            chapter(&removed),
        ),
        (
            "4 1 4 current writing_preference conversation 短句 多用短句",
            conversation(other),
        ),
        (
            "5 1 5 current major_event conversation 大水 三十年前",
            conversation(&talk),
        ),
        (
            "1 2 6 current character author_request 林雪 新文",
            conversation(&talk),
        ),
        (
            "5 2 7 deleted major_event conversation 大水 三十年前",
            conversation(&talk),
        ),
        (
            "6 1 8 current relationship conversation 姐妹 林舟与林雪",
            "[]".to_owned(),
        ),
    ];
    write(&admin, &scope, &rows).await;
    let empty = seed_project(&store, "b740").await;
    let hidden = ProjectScope::new(
        UserId::new(scope.owner_user_id.as_ref()),
        ProjectId::new("018f0000-0000-7001-8000-0000000b74ff"),
    );
    let entries = list_memory_entries(&store, &scope).await.unwrap().unwrap();
    let times: Vec<String> = entries.into_iter().map(|entry| entry.changed_at).collect();

    assert_eq!(
        (
            listed(&store, &scope).await,
            listed(&store, &empty).await,
            listed(&store, &hidden).await,
            times
                .iter()
                .all(|time| time.len() == 24 && time.ends_with('Z') && time.as_bytes()[10] == b'T'),
        ),
        (
            Some(vec![
                format!(
                    "4 1 writing_preference conversation 短句 多用短句 conversation {other} false"
                ),
                format!("3 1 foreshadowing chapter 铜铃 铃响 chapter {removed} None false"),
                format!("1 2 character author_request 林雪 新文 conversation {talk} true"),
                "6 1 relationship conversation 姐妹 林舟与林雪 none".to_owned(),
                format!(
                    "2 1 other chapter 旧渡口 北岸 chapter {chapter_id} Some(\"Chapter A\") true"
                ),
            ]),
            Some(Vec::new()),
            None,
            true,
        )
    );
}

/// The query does not read the Conversation Memory Settings: a Project whose Conversation has
/// `use_enabled = false` (a row that the test writes with SQL) gives the same list.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_conversation_with_no_memory_use_does_not_change_the_list() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued(&store, "b72").await;
    complete(&store, &admin, &scope, &run_id).await;
    let talk = conversation_of(&admin, &run_id).await;
    let row = (
        "1 1 1 current character author_request 林雪 船工",
        conversation(&talk),
    );
    write(&admin, &scope, &[row]).await;
    let before = listed(&store, &scope).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.conversation_memory_settings SET is_current = FALSE
              WHERE project_id = '{project}'::uuid;
             INSERT INTO storyos.conversation_memory_settings
             SELECT (jsonb_populate_record(NULL::storyos.conversation_memory_settings,
               (to_jsonb(settings) - 'is_current') || jsonb_build_object(
                 'memory_settings_revision', gen_random_uuid(), 'use_enabled', false))).*
               FROM storyos.conversation_memory_settings AS settings
              WHERE project_id = '{project}'::uuid",
            project = scope.project_id.as_ref()
        ))
        .await
        .unwrap();
    let without_use: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.conversation_memory_settings
              WHERE project_id = $1::text::uuid AND NOT use_enabled",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(0);

    assert_eq!(
        (
            without_use,
            before.as_ref().map(Vec::len),
            listed(&store, &scope).await
        ),
        (1, Some(1), before)
    );
}

/// The Host limits bound the list, so it has no pages: 300 entries of the longest permitted title,
/// description, and text are one response.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_list_holds_300_entries_of_the_longest_permitted_text() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "b730").await;
    admin
        .execute(
            "INSERT INTO storyos.memory_entry_versions
                 (owner_user_id, project_id, memory_entry_id, entry_number, version,
                  memory_revision, state, entry_type, origin, title, description, text, cause,
                  source_references)
             SELECT $1::text::uuid, $2::text::uuid, md5($2 || ':' || number)::uuid, number, 1,
                    number, 'current', 'character', 'author_request',
                    lpad(number::text, $3::int, '题'), repeat('述', $4::int), repeat('文', $5::int),
                    '{}', '[]'
               FROM generate_series(1, 300) AS number;
             ",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &i32::try_from(MEMORY_TITLE_MAX_CHARS).unwrap(),
                &i32::try_from(MEMORY_DESCRIPTION_MAX_CHARS).unwrap(),
                &i32::try_from(MEMORY_CHARACTER_TEXT_MAX_CHARS).unwrap(),
            ],
        )
        .await
        .unwrap();
    let entries = list_memory_entries(&store, &scope).await.unwrap().unwrap();
    let longest = entries.iter().all(|entry| {
        entry.title.chars().count() == MEMORY_TITLE_MAX_CHARS
            && entry.description.chars().count() == MEMORY_DESCRIPTION_MAX_CHARS
            && entry.text.chars().count() == MEMORY_CHARACTER_TEXT_MAX_CHARS
    });
    let numbers: Vec<u64> = entries.iter().map(|entry| entry.number).collect();

    assert_eq!((longest, numbers), (true, (1..=300).collect::<Vec<u64>>()));
}
