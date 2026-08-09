//! StoryOS public HTTP boundary.

use axum::{Json, Router, http::Method, routing};
use storyos_contracts as contracts;

/// Build the one StoryOS Server router used by production and boundary tests.
pub fn router() -> Router {
    let method = Method::from_bytes(contracts::GET_PROTOCOL_PROFILE_METHOD.as_bytes())
        .expect("contract operation method must be valid HTTP");
    let filter = routing::MethodFilter::try_from(method)
        .expect("contract operation method must be routable");
    Router::new().route(
        contracts::GET_PROTOCOL_PROFILE_PATH,
        routing::on(filter, get_protocol_profile),
    )
}

async fn get_protocol_profile() -> Json<contracts::Release1ProtocolProfile> {
    Json(contracts::release1_protocol_profile())
}
