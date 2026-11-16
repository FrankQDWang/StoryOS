//! One embedding request of the probe of ADR 0095 section 5 (ADR 0049 amendment of
//! 2026-11-16). It sends constant text only, reads no database, and writes the observation as
//! JSON to stdout. S7-C2a2 removes this command.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use storyos_adapter_volcengine_responses::{
    AgentPlanEmbeddings, EmbeddingObservation, EmbeddingRequest,
};
use storyos_application::{CredentialReference, PreDispatchRefusal};
use storyos_core::{AGENT_PLAN_CREDENTIAL_REFERENCE, AGENT_PLAN_ENDPOINT};

use crate::credential::SettingsCredentials;

const MODEL: &str = "doubao-embedding-vision-251215";
const TIMEOUT: Duration = Duration::from_secs(/*secs*/ 120);

/// The corpus and the query instruction of the retrieval form of the model documents.
const PASSAGE_INSTRUCTIONS: &str = "Instruction:Compress the text into one word.\nQuery:";
const QUESTION_INSTRUCTIONS: &str =
    "Target_modality: text.\nInstruction:Find the passage that answers this question\nQuery:";

const PASSAGE_A: &str =
    "The lighthouse keeper rowed to the island at dawn and found that the lamp was broken.";
const PASSAGE_B: &str = "The bakery sold all of its rye bread before noon on market day.";
const QUESTION: &str = "Who went to the island to repair the light?";

/// 25,000 copies give approximately 175,000 tokens, above the 128K window of the model list.
const OVER_LIMIT_SENTENCE: &str = "The probe repeats one constant sentence. ";
const OVER_LIMIT_COPIES: usize = 25_000;

/// The exit code: 0 for an answer of the route, 1 for no answer, 2 for a refusal before dispatch
/// or an unknown case.
pub(crate) async fn probe_embedding(resolver: SettingsCredentials, case: Option<&str>) -> i32 {
    let Some((case, (instructions, texts, dimensions))) =
        case.and_then(|case| Some((case, probe_case(case)?)))
    else {
        tracing::error!(reason = "unknown_case", "embedding probe refused");
        return 2;
    };
    let embeddings = AgentPlanEmbeddings { resolver };
    let request = EmbeddingRequest {
        endpoint: AGENT_PLAN_ENDPOINT.to_owned(),
        model: MODEL.to_owned(),
        instructions: instructions.to_owned(),
        texts,
        dimensions,
        timeout: TIMEOUT,
    };
    let reference = CredentialReference(AGENT_PLAN_CREDENTIAL_REFERENCE.to_owned());
    let prepared = match embeddings.prepare(&request, &reference).await {
        Ok(prepared) => prepared,
        Err(PreDispatchRefusal::CredentialUnavailable) => {
            tracing::error!(reason = "credential_unavailable", "embedding probe refused");
            return 2;
        }
        Err(PreDispatchRefusal::UnsupportedRequest) => {
            tracing::error!(reason = "unsupported_request", "embedding probe refused");
            return 2;
        }
    };
    let url = prepared.url().to_owned();
    let body = prepared.body().to_owned();
    let started = Instant::now();
    let observation = embeddings.exchange(prepared).await;
    let elapsed_ms = started.elapsed().as_millis();
    let (code, observed) = match observation {
        EmbeddingObservation::NotSubmitted => (1, json!({"outcome": "not_submitted"})),
        EmbeddingObservation::OutcomeUnknown => (1, json!({"outcome": "outcome_unknown"})),
        EmbeddingObservation::Answered {
            status,
            request_id,
            body,
        } => (
            0,
            json!({
                "outcome": "answered",
                "status": status,
                "request_id": request_id,
                "body": body,
            }),
        ),
    };
    let record = json!({
        "case": case,
        "url": url,
        "request_digest": format!("sha256:{}", storyos_core::hex_sha256(body.as_bytes())),
        "request_bytes": body.len(),
        "request": serde_json::from_str::<Value>(&body).unwrap_or(Value::Null),
        "elapsed_ms": elapsed_ms,
        "observation": observed,
    });
    println!("{record}");
    code
}

/// The instructions, the text items, and the dimension option of a case.
type ProbeCase = (&'static str, Vec<String>, Option<u32>);

fn probe_case(case: &str) -> Option<ProbeCase> {
    let one = |text: &str| vec![text.to_owned()];
    Some(match case {
        "passage-1024" => (PASSAGE_INSTRUCTIONS, one(PASSAGE_A), Some(1024)),
        "passage-default" => (PASSAGE_INSTRUCTIONS, one(PASSAGE_A), None),
        "question-1024" => (QUESTION_INSTRUCTIONS, one(QUESTION), Some(1024)),
        "other-passage-1024" => (PASSAGE_INSTRUCTIONS, one(PASSAGE_B), Some(1024)),
        "two-items-1024" => (
            PASSAGE_INSTRUCTIONS,
            vec![PASSAGE_A.to_owned(), PASSAGE_B.to_owned()],
            Some(1024),
        ),
        "over-limit-1024" => (
            PASSAGE_INSTRUCTIONS,
            vec![OVER_LIMIT_SENTENCE.repeat(OVER_LIMIT_COPIES)],
            Some(1024),
        ),
        _ => return None,
    })
}
