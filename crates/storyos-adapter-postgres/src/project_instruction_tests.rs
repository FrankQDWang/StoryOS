use std::convert::Infallible;

use storyos_application::{
    ProjectInstructionRevision, ProjectInstructionSaved, ProjectScope, RefusableCommandError,
    UpdateProjectInstructionInput, open_project_instruction,
};
use storyos_core::{
    CONTEXT_ITEM_TOKEN_LIMIT, TransitionOutcome, UpdateProjectInstructionConflict,
    UpdateProjectInstructionNoEffect, UpdateProjectInstructionRefusal,
};
use tokio_postgres::{Client, NoTls};

use crate::PostgresProjectReader;
use crate::command_sequence::tests::{
    UPDATE_PROJECT_INSTRUCTION, issued, seed_owned_project, seed_project, stores,
};

const OTHER_USER: &str = "018f0000-0000-7001-8000-000000000101";

type Outcome = Result<
    TransitionOutcome<
        ProjectInstructionSaved,
        UpdateProjectInstructionNoEffect,
        UpdateProjectInstructionConflict,
        Infallible,
    >,
    UpdateProjectInstructionRefusal,
>;

/// Saves `text` in the Project of `scope` with the Command Challenge `suffix`.
pub(crate) async fn save(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    text: &str,
    expected_revision: u64,
) -> Outcome {
    let input = UpdateProjectInstructionInput {
        text: text.to_owned(),
        expected_revision,
    };
    let call = issued(store, scope, suffix, &UPDATE_PROJECT_INSTRUCTION, input).await;
    match store
        .update_project_instruction(&call.envelope, &call.input)
        .await
    {
        Ok(settlement) => Ok(settlement.outcome.map_applied(|applied| applied.effect)),
        Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => Err(refusal),
        Err(RefusableCommandError::Command(error)) => panic!("the save must settle: {error:?}"),
    }
}

pub(crate) fn applied(outcome: &Outcome) -> ProjectInstructionSaved {
    match outcome {
        Ok(TransitionOutcome::Applied(saved)) => saved.clone(),
        other => panic!("expected an applied save, got {other:?}"),
    }
}

/// The Revision rows of the Project of `scope`, in revision order, read as the admin role.
async fn revision_rows(admin: &Client, scope: &ProjectScope) -> Vec<ProjectInstructionRevision> {
    admin
        .query(
            "SELECT instruction_revision_id::text, revision::text, text
               FROM storyos.project_instruction_revisions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              ORDER BY revision",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| ProjectInstructionRevision {
            instruction_revision_id: row.get(0),
            revision: row.get::<_, String>(1).parse().unwrap(),
            text: row.get(2),
        })
        .collect()
}

fn revision(saved: &ProjectInstructionSaved, text: &str) -> ProjectInstructionRevision {
    ProjectInstructionRevision {
        instruction_revision_id: saved.instruction_revision_id.clone(),
        revision: saved.revision,
        text: text.to_owned(),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_save_keeps_its_exact_text_as_one_immutable_revision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "ee00").await;
    assert_eq!(
        open_project_instruction(&store, &scope).await.unwrap(),
        None
    );
    let above_limit = "字".repeat(CONTEXT_ITEM_TOKEN_LIMIT as usize + 1);
    let mut outcomes = Vec::new();
    for (suffix, text, expected_revision) in [
        (0xee01, " Write in the past tense.\n", 0),
        (0xee02, "Keep the first person.", 1),
        (0xee03, "A stale text.", 1),
        (0xee04, "Keep the first person.", 2),
        (0xee05, above_limit.as_str(), 2),
        (0xee06, "", 2),
    ] {
        outcomes.push(save(&store, &scope, suffix, text, expected_revision).await);
    }
    let first = applied(&outcomes[0]);
    let second = applied(&outcomes[1]);
    let emptied = applied(&outcomes[5]);
    let receipts: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.domain_receipts WHERE project_id = $1::text::uuid
                AND command_kind = 'updateProjectInstruction'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(0);

    assert_eq!(
        (
            outcomes[2..5].to_vec(),
            receipts,
            [first.revision, second.revision, emptied.revision],
            revision_rows(&admin, &scope).await,
            open_project_instruction(&store, &scope).await.unwrap(),
        ),
        (
            vec![
                Ok(TransitionOutcome::Conflicted(
                    UpdateProjectInstructionConflict::StaleInstructionRevision
                )),
                Ok(TransitionOutcome::NoEffect(
                    UpdateProjectInstructionNoEffect::TextUnchanged
                )),
                Err(UpdateProjectInstructionRefusal::InstructionOverItemLimit),
            ],
            5,
            [1, 2, 3],
            vec![
                revision(&first, " Write in the past tense.\n"),
                revision(&second, "Keep the first person."),
                revision(&emptied, ""),
            ],
            Some(revision(&emptied, "")),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_save_writes_and_reads_only_its_own_project_scope() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let project_a = seed_project(&store, "ee10").await;
    let project_b = seed_project(&store, "ee20").await;
    let foreign = seed_owned_project(&store, OTHER_USER, "ee30").await;
    let saved = applied(&save(&store, &project_a, 0xee11, "Project A text.", 0).await);

    assert_eq!(
        (
            save(&store, &project_b, 0xee21, "Project A text.", 1).await,
            save(&store, &foreign, 0xee31, "", 0).await,
            open_project_instruction(&store, &project_b).await.unwrap(),
            open_project_instruction(&store, &foreign).await.unwrap(),
            revision_rows(&admin, &project_a).await,
            revision_rows(&admin, &project_b).await.len(),
            revision_rows(&admin, &foreign).await.len(),
        ),
        (
            Ok(TransitionOutcome::Conflicted(
                UpdateProjectInstructionConflict::StaleInstructionRevision
            )),
            Ok(TransitionOutcome::NoEffect(
                UpdateProjectInstructionNoEffect::TextUnchanged
            )),
            None,
            None,
            vec![revision(&saved, "Project A text.")],
            0,
            0,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_runtime_role_cannot_change_or_delete_a_revision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "ee40").await;
    let saved = applied(&save(&store, &scope, 0xee41, "Kept text.", 0).await);
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (mut runtime, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(connection);
    let mut refused = Vec::new();
    for statement in [
        "UPDATE storyos.project_instruction_revisions SET text = 'Changed text.'",
        "DELETE FROM storyos.project_instruction_revisions",
    ] {
        let transaction = runtime.transaction().await.unwrap();
        transaction
            .execute(
                "SELECT set_config('storyos.user_id', $1, true),
                        set_config('storyos.owner_user_id', $1, true),
                        set_config('storyos.project_id', $2, true)",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .unwrap();
        refused.push(
            transaction
                .execute(statement, &[])
                .await
                .map_err(|error| crate::sql_state(&error)),
        );
    }

    assert_eq!(
        (refused, revision_rows(&admin, &scope).await),
        (
            vec![Err(Some("42501".to_owned())); 2],
            vec![revision(&saved, "Kept text.")],
        )
    );
}
