use serde_json::json;
use storyos_application::{
    CreateRequest, DestinationRequest, ModelProviderAdapter, RequestAttempt, RequestRoute,
};
use storyos_core::{
    EarlierMessageKind, EarlierTurn, EarlierTurnMessage, EarlierTurnSource, EarlierTurns,
};

use crate::AgentPlanResponses;
use crate::tests::{CanaryResolver, create, route};

/// The input that `prepare` commits for the request with `earlier` on the current mapping.
async fn input(earlier: Option<EarlierTurns>) -> serde_json::Value {
    let route = RequestRoute {
        mapping_revision: storyos_core::AGENT_PLAN_EARLIER_TURNS_MAPPING_REVISION.to_owned(),
        ..route("http://127.0.0.1:9")
    };
    let DestinationRequest::Create(request) = create(route, RequestAttempt::New) else {
        unreachable!("create() makes a Create request")
    };
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let prepared = adapter
        .prepare(&DestinationRequest::Create(CreateRequest {
            earlier_turns: earlier,
            ..request
        }))
        .await
        .ok()
        .unwrap();
    let body: serde_json::Value =
        serde_json::from_str(&prepared.projection.serialized_payload.unwrap()).unwrap();
    body["input"].clone()
}

fn turn(run_id: &str, messages: &[(EarlierMessageKind, &str)]) -> EarlierTurn {
    EarlierTurn {
        messages: (messages.iter())
            .map(|(message_kind, text)| EarlierTurnMessage {
                source: EarlierTurnSource {
                    run_id: run_id.to_owned(),
                    message_id: format!("{run_id}-{text}"),
                    message_kind: *message_kind,
                },
                text: (*text).to_owned(),
            })
            .collect(),
    }
}

/// ADR 0102 section 5: the section of the earlier turns comes directly before the author request,
/// with its exact heading, the line of the left-out turns when the limit left out a turn, the
/// number of each turn, and the speaker of each message.
#[tokio::test]
async fn the_request_sends_the_earlier_turns_directly_before_the_author_request() {
    use EarlierMessageKind::{Answer, Author};
    let turns = vec![
        turn(
            "run-1",
            &[(Author, "Who is Mara?"), (Answer, "Mara keeps the inn.")],
        ),
        turn(
            "run-2",
            &[
                (Author, "Make her older."),
                (Author, "Keep her voice.\nOnly that."),
            ],
        ),
    ];
    let text = |left_out: &str| {
        json!([{"role": "user", "content": [{"type": "input_text", "text": format!(
            "Earlier turns of this conversation, oldest first. Each turn has the exact words \
             of the author and your final answer. The Tool calls and the Tool results of those \
             turns are not here:\n{left_out}[turn 1]\nAuthor:\nWho is Mara?\n\nAssistant:\nMara \
             keeps the inn.\n\n[turn 2]\nAuthor:\nMake her older.\n\nAuthor:\nKeep her \
             voice.\nOnly that.\n\nAuthor request:\nTighten this paragraph.\n\nWorking \
             text:\nThe rain fell hard.\n\nTarget blocks:\n[block_id: \
             018f0000-0000-7001-8000-0000000000b1]\nThe rain fell hard on the old roof."
        )}]}])
    };

    assert_eq!(
        [
            input(Some(EarlierTurns {
                turns: turns.clone(),
                older_turns_left_out: false,
            }))
            .await,
            input(Some(EarlierTurns {
                turns,
                older_turns_left_out: true,
            }))
            .await,
        ],
        [text(""), text("Older turns are left out.\n\n")]
    );
}
