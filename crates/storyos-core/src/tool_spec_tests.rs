use serde_json::json;

use super::*;
use crate::EXA_TOOL_REGISTRATION;

#[test]
fn a_search_intake_takes_one_bounded_query_and_an_optional_count_of_at_most_ten() {
    let longest = "q".repeat(512);
    let cases = [
        (
            json!({"query": longest, "result_count": 10}),
            Ok(ToolIntake::WebSearch {
                query: longest.clone(),
                result_count: Some(10),
            }),
        ),
        (
            json!({"query": "storyos"}),
            Ok(ToolIntake::WebSearch {
                query: "storyos".to_owned(),
                result_count: None,
            }),
        ),
        (json!({"query": "q".repeat(513)}), Err(IntakeRefusal::Query)),
        (json!({"query": "  "}), Err(IntakeRefusal::Query)),
        (
            json!({"query": "storyos", "result_count": 11}),
            Err(IntakeRefusal::ResultCount),
        ),
        (
            json!({"query": "storyos", "result_count": 0}),
            Err(IntakeRefusal::ResultCount),
        ),
        (
            json!({"query": "storyos", "objective": "x"}),
            Err(IntakeRefusal::Malformed),
        ),
        (
            json!({"urls": ["https://a.example"]}),
            Err(IntakeRefusal::Malformed),
        ),
    ];
    for (arguments, expected) in cases {
        assert_eq!(
            WEB_SEARCH_TOOL_SPEC.intake(&arguments),
            expected,
            "{arguments}"
        );
    }
}

#[test]
fn a_fetch_intake_takes_at_most_five_absolute_web_urls() {
    let five: Vec<String> = (1..=5).map(|n| format!("https://a.example/{n}")).collect();
    let six: Vec<String> = (1..=6).map(|n| format!("http://a.example/{n}")).collect();
    let cases = [
        (
            json!({"urls": five}),
            Ok(ToolIntake::WebFetch { urls: five.clone() }),
        ),
        (json!({"urls": six}), Err(IntakeRefusal::UrlCount)),
        (json!({"urls": []}), Err(IntakeRefusal::UrlCount)),
        (json!({"urls": ["/relative"]}), Err(IntakeRefusal::Url)),
        (
            json!({"urls": ["ftp://a.example"]}),
            Err(IntakeRefusal::Url),
        ),
        (json!({"urls": ["https://"]}), Err(IntakeRefusal::Url)),
        (
            json!({"urls": ["https://user:key@a.example"]}),
            Err(IntakeRefusal::Url),
        ),
        (
            json!({"urls": ["https://a.example/a b"]}),
            Err(IntakeRefusal::Url),
        ),
        (json!({"query": "storyos"}), Err(IntakeRefusal::Malformed)),
    ];
    for (arguments, expected) in cases {
        assert_eq!(
            WEB_FETCH_TOOL_SPEC.intake(&arguments),
            expected,
            "{arguments}"
        );
    }
}

#[test]
fn the_registration_derives_the_required_search_goal_from_the_query_only() {
    let search = EXA_TOOL_REGISTRATION.tool("web_search_exa").unwrap();
    let fetch = EXA_TOOL_REGISTRATION.tool("web_fetch_exa").unwrap();
    let query = ToolIntake::WebSearch {
        query: "harbor towns of 1820".to_owned(),
        result_count: Some(3),
    };
    let urls = ToolIntake::WebFetch {
        urls: vec!["https://a.example/1".to_owned()],
    };

    assert_eq!(
        search.arguments.map(&query).map(Value::Object),
        Some(json!({
            "query": "harbor towns of 1820",
            "objective": "Rank first the pages that answer this search: harbor towns of 1820",
            "numResults": 3,
        }))
    );
    assert_eq!(
        fetch.arguments.map(&urls).map(Value::Object),
        Some(json!({"urls": ["https://a.example/1"]}))
    );
    assert_eq!(search.arguments.map(&urls), None);
}
