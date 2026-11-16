/// Each part of a range keeps its sign (#1613 round 1).
#[test]
fn each_part_of_a_range_is_the_json_text_of_its_argument() {
    let parsed: Vec<serde_json::Value> = ["2", "1:2-3", "1:-2-3", "1:2--3", "1:3-null", "1:4"]
        .into_iter()
        .map(|token| serde_json::from_str(&super::arguments(token)).unwrap())
        .collect();
    assert_eq!(
        parsed,
        [
            serde_json::json!({"chapter": 2}),
            serde_json::json!({"chapter": 1, "first_block": 2, "last_block": 3}),
            serde_json::json!({"chapter": 1, "first_block": -2, "last_block": 3}),
            serde_json::json!({"chapter": 1, "first_block": 2, "last_block": -3}),
            serde_json::json!({"chapter": 1, "first_block": 3, "last_block": null}),
            serde_json::json!({"chapter": 1, "first_block": 4, "last_block": null}),
        ]
    );
}
