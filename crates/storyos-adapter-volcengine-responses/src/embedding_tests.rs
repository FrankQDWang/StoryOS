use std::time::Duration;

use serde_json::json;
use storyos_application::CredentialReference;

use super::{AgentPlanEmbeddings, EmbeddingObservation, EmbeddingRequest};
use crate::tests::{CanaryResolver, Reply, destination};

#[tokio::test]
async fn one_embedding_request_sends_the_text_items_once_with_the_resolved_credential() {
    let reply = json!({
        "id": "021700000000000",
        "object": "list",
        "data": {"object": "embedding", "embedding": [0.25, -0.5]},
        "usage": {"prompt_tokens": 9, "total_tokens": 9},
    });
    let (base, served) = destination(Reply::Http(200, reply.to_string())).await;
    let embeddings = AgentPlanEmbeddings {
        resolver: CanaryResolver::default(),
    };
    let request = EmbeddingRequest {
        endpoint: base.clone(),
        model: "doubao-embedding-vision-251215".to_owned(),
        instructions: "Instruction:Compress the text into one word.\nQuery:".to_owned(),
        texts: vec!["first".to_owned(), "second".to_owned()],
        dimensions: Some(1024),
        timeout: Duration::from_secs(/*secs*/ 5),
    };
    let reference = CredentialReference("settings:STORYOS_AGENT_PLAN_API_KEY".to_owned());

    let prepared = embeddings.prepare(&request, &reference).await.unwrap();
    let observation = embeddings.exchange(prepared).await;

    let (request_line, authorization, body) = served.await.unwrap();
    assert_eq!(
        request_line,
        "POST /api/plan/v3/embeddings/multimodal HTTP/1.1"
    );
    assert_eq!(
        authorization.as_deref(),
        Some("Bearer storyos-credential-canary-5e21")
    );
    assert_eq!(
        body,
        storyos_core::canonical_json(&json!({
            "model": "doubao-embedding-vision-251215",
            "instructions": "Instruction:Compress the text into one word.\nQuery:",
            "input": [
                {"type": "text", "text": "first"},
                {"type": "text", "text": "second"},
            ],
            "encoding_format": "float",
            "dimensions": 1024,
        }))
    );
    assert_eq!(
        observation,
        EmbeddingObservation::Answered {
            status: 200,
            request_id: None,
            body: reply,
        }
    );
}
