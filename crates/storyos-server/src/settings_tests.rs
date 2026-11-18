use serde_json::json;
use storyos_core::{
    AGENT_PLAN_EMBEDDING_REGISTRATION, DeploymentDestination, HOST_FAKE_EMBEDDING_REGISTRATION,
};

use super::ServerSettings;

fn settings(embedding: Option<&str>) -> ServerSettings {
    serde_json::from_value(json!({"embedding_destination": embedding})).unwrap()
}

#[test]
fn the_embedding_destination_is_a_known_value_with_its_model_destination() {
    let fake = DeploymentDestination::HostFakeToolLoop;
    let plan = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };
    let refused = |embedding, model: Option<&DeploymentDestination>| {
        settings(embedding)
            .embedding_registration(model)
            .unwrap_err()
            .to_string()
    };

    assert_eq!(settings(None).embedding_registration(Some(&plan)), Ok(None));
    assert_eq!(
        settings(Some("host_fake")).embedding_registration(None),
        Ok(Some(&HOST_FAKE_EMBEDDING_REGISTRATION))
    );
    assert_eq!(
        settings(Some("volcengine_agent_plan")).embedding_registration(Some(&plan)),
        Ok(Some(&AGENT_PLAN_EMBEDDING_REGISTRATION))
    );
    for (embedding, model) in [
        (Some("volcengine_agent_plan"), Some(&fake)),
        (Some("volcengine_agent_plan"), None),
        (Some("openai"), Some(&plan)),
    ] {
        let message = refused(embedding, model);
        assert!(message.contains("STORYOS_EMBEDDING_DESTINATION"));
        assert!(!message.contains("openai"));
    }
}
