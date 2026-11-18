//! The return of the observations of a refused operation of a pass (ADR 0094 section 6). The stored
//! pass rows give each fact: the observations of the input of a pass and the result of each of its
//! operations. An observation returns at most one time.

use serde_json::{Value, json};
use storyos_application::ProjectScope;
use storyos_core::{PassOperationResult, observation_return_refusal, observation_returns};
use tokio_postgres::Client;

use crate::memory_consolidation_versions::PassItem;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The label of the refusal of each of `keys` that a settled pass of the Project refused: the
/// first refused operation that names it. The pass in progress has no outcome yet, so this is the
/// fact "a refused operation of an earlier pass names it". A pass with the outcome `stale` or
/// `failed` refuses no operation.
async fn earlier_refusals(
    client: &Client,
    scope: &ProjectScope,
    keys: &[&[String; 2]],
) -> Result<Vec<([String; 2], String)>, BoxError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let rows = client
        .query(
            "SELECT input.key ->> 0, input.key ->> 1, input.number::text, pass.operations::text
               FROM storyos.memory_consolidation_passes AS pass,
                    jsonb_array_elements(pass.observations) WITH ORDINALITY AS input(key, number)
              WHERE (pass.owner_user_id, pass.project_id) = ($1::text::uuid, $2::text::uuid)
                AND pass.outcome IN ('applied', 'no_operation')
                AND pass.operations IS NOT NULL
                AND input.key IN (SELECT jsonb_array_elements($3::text::jsonb))
              ORDER BY pass.started_at, pass.pass_id",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &json!(keys).to_string(),
            ],
        )
        .await?;
    let mut refusals: Vec<([String; 2], String)> = Vec::new();
    for row in &rows {
        let key: [String; 2] = [row.get(0), row.get(1)];
        let number: u64 = row.get::<_, &str>(2).parse()?;
        let results: Vec<PassOperationResult> = serde_json::from_str(row.get(3))?;
        if let Some(label) = observation_return_refusal(&results, number)
            && !refusals.iter().any(|(earlier, _)| *earlier == key)
        {
            refusals.push((key, label.to_owned()));
        }
    }
    Ok(refusals)
}

/// The keys of the observations of `input` that stay `waiting` at the settlement of a pass. This
/// function gives the stored facts to `observation_returns` of Core. The number of an observation
/// is its position in `input`, from 1.
pub(crate) async fn returned_observations(
    client: &Client,
    scope: &ProjectScope,
    input: &[[String; 2]],
    results: &[Value],
) -> Result<Vec<[String; 2]>, BoxError> {
    let results: Vec<PassOperationResult> = serde_json::from_value(Value::from(results.to_vec()))?;
    let keys: Vec<&[String; 2]> = input.iter().collect();
    let earlier = earlier_refusals(client, scope, &keys).await?;
    Ok((1_u64..)
        .zip(input)
        .filter(|(number, key)| {
            let refused_earlier = earlier.iter().any(|(earlier, _)| earlier == *key);
            observation_returns(&results, *number, refused_earlier)
        })
        .map(|(_, key)| key.clone())
        .collect())
}

/// Gives each observation of `items` that returned the label of its refusal.
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
    let refusals = earlier_refusals(client, scope, &keys).await?;
    for (key, label) in refusals {
        let item = items.iter_mut().find(|(item_key, _)| *item_key == key);
        if let Some((_, PassItem::Observation(observation))) = item {
            observation.refused = Some(label);
        }
    }
    Ok(())
}
