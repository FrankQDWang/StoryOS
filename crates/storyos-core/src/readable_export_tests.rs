use super::*;

#[test]
fn utf8_lf_profile_matches_pinned_golden_cases() {
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("readable_export_utf8_lf_v1.golden.json"))
            .expect("the readable-export golden file must be JSON");
    assert_eq!(
        golden["profile"],
        serde_json::Value::String(READABLE_EXPORT_PROFILE.to_owned())
    );
    for case in golden["cases"]
        .as_array()
        .expect("golden cases must be an array")
    {
        let name = case["name"].as_str().expect("case name");
        let volumes = case["volumes"]
            .as_array()
            .expect("volumes")
            .iter()
            .map(|volume| ReadableExportVolume {
                title: volume["title"].as_str().expect("volume title").to_owned(),
                chapters: volume["chapters"]
                    .as_array()
                    .expect("chapters")
                    .iter()
                    .map(|chapter| ReadableExportChapter {
                        title: chapter["title"].as_str().expect("chapter title").to_owned(),
                        body: chapter["body"].as_str().map(str::to_owned),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let rendered = render_readable_manuscript(&volumes);
        assert!(
            rendered.ends_with('\n'),
            "{name} must end with one LF newline"
        );
        assert!(!rendered.contains('\r'), "{name} must not contain CR");
        assert_eq!(
            rendered,
            case["expected"].as_str().expect("expected"),
            "{name}"
        );
    }
}
