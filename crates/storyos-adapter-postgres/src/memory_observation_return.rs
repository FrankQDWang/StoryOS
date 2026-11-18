//! The return of the observations of a refused operation of a pass (ADR 0094 section 6). The stored
//! pass rows give each fact: the observations of the input of a pass and the result of each of its
//! operations. An observation returns at most one time.

use serde_json::{Value, json};
use storyos_application::ProjectScope;
use storyos_core::{PassOperationResult, observation_return_refusal};
use tokio_postgres::Client;

use crate::memory_consolidation_versions::PassItem;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The keys of the observations of `input` that stay `waiting` at the settlement of the pass
/// `pass_id`: a refused operation names the observation, no applied operation names it, and no
/// earlier pass of the Project had it in its input. The number of an observation is its position
/// in `input`, from 1.
pub(crate) async fn returned_observations(
    client: &Client,
    scope: &ProjectScope,
    pass_id: &str,
    input: &[[String; 2]],
    results: &[Value],
) -> Result<Vec<[String; 2]>, BoxError> {
    let results: Vec<PassOperationResult> = serde_json::from_value(Value::from(results.to_vec()))?;
    let refused: Vec<&[String; 2]> = (1_u64..)
        .zip(input)
        .filter(|(number, _)| observation_return_refusal(&results, *number).is_some())
        .map(|(_, key)| key)
        .collect();
    if refused.is_empty() {
        return Ok(Vec::new());
    }
    let rows = client
        .query(
            "SELECT input.key ->> 0, input.key ->> 1
               FROM storyos.memory_consolidation_passes AS pass,
                    jsonb_array_elements(pass.observations) AS input(key)
              WHERE (pass.owner_user_id, pass.project_id) = ($1::text::uuid, $2::text::uuid)
                AND pass.pass_id <> $3::text::uuid
                AND input.key IN (SELECT jsonb_array_elements($4::text::jsonb))",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &pass_id,
                &json!(refused).to_string(),
            ],
        )
        .await?;
    let earlier: Vec<[String; 2]> = rows.iter().map(|row| [row.get(0), row.get(1)]).collect();
    Ok(refused
        .into_iter()
        .filter(|key| !earlier.contains(key))
        .cloned()
        .collect())
}

/// Gives each observation of `items` that returned the label of its refusal. Only the pass that
/// refused a returned observation settled with it in its input and left it `waiting`.
pub(crate) async fn add_return_refusals(
    client: &Client,
    scope: &ProjectScope,
    items: &mut [([String; 2], PassItem)],
) -> Result<(), BoxError> {
    let keys: Vec<&[String; 2]> = items
        .iter()
        .filter(|(_, item)| matches!(item, PassItem::Observation(_)))
        .map(|(key, _)| key)
        .collect();
    if keys.is_empty() {
        return Ok(());
    }
    let rows = client
        .query(
            "SELECT input.key ->> 0, input.key ->> 1, input.number::text, pass.operations::text
               FROM storyos.memory_consolidation_passes AS pass,
                    jsonb_array_elements(pass.observations) WITH ORDINALITY AS input(key, number)
              WHERE (pass.owner_user_id, pass.project_id) = ($1::text::uuid, $2::text::uuid)
                AND pass.outcome IN ('applied', 'no_operation')
                AND pass.operations IS NOT NULL
                AND input.key IN (SELECT jsonb_array_elements($3::text::jsonb))",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &json!(keys).to_string(),
            ],
        )
        .await?;
    for row in &rows {
        let key: [String; 2] = [row.get(0), row.get(1)];
        let number: u64 = row.get::<_, &str>(2).parse()?;
        let results: Vec<PassOperationResult> = serde_json::from_str(row.get(3))?;
        let item = items.iter_mut().find(|(item_key, _)| *item_key == key);
        if let Some((_, PassItem::Observation(observation))) = item {
            observation.refused = observation_return_refusal(&results, number).map(str::to_owned);
        }
    }
    Ok(())
}
