use super::settle;

fn payload(network_io: bool) -> serde_json::Value {
    serde_json::json!({
        "execution_profile": { "network_io": network_io },
        "evidence": [
            {"kind": "sent_content", "attempt_id": "a1", "availability": "current", "content": "{}"},
            {"kind": "provider_report", "attempt_id": "a1", "availability": "current",
             "report": super::PENDING_REPORT},
            {"kind": "provider_opaque", "attempt_id": "a1", "availability": "unknown",
             "unknown_facts": ["provider_internal_content"]}
        ]
    })
}

fn evidence_after(
    network_io: bool,
    usage_kind: &str,
    reference: Option<&str>,
) -> serde_json::Value {
    let mut value = payload(network_io);
    settle(&mut value, usage_kind, reference);
    value["evidence"].clone()
}

#[test]
fn a_real_attempt_reports_its_usage_kind_and_its_response_reference() {
    let report = |kind: &str| {
        serde_json::json!({
            "kind": "provider_report", "attempt_id": "a1", "availability": "current",
            "report": format!("provider_usage_{kind}")
        })
    };
    let sent = payload(/*network_io*/ true)["evidence"][0].clone();
    let opaque = payload(/*network_io*/ true)["evidence"][2].clone();
    let reference = serde_json::json!({
        "kind": "stored_reference", "attempt_id": "a1", "availability": "current",
        "reference_id": "resp_01"
    });

    assert_eq!(
        [
            evidence_after(true, "reported", Some("resp_01")),
            evidence_after(true, "estimated", None),
            evidence_after(true, "unknown", None),
            evidence_after(false, "unknown", Some("host-fake-reference")),
        ],
        [
            serde_json::json!([sent, reference, report("reported"), opaque]),
            serde_json::json!([sent, report("estimated"), opaque]),
            serde_json::json!([sent, report("unknown"), opaque]),
            payload(false)["evidence"].clone(),
        ]
    );
}
