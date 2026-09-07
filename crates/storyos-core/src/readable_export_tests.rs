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

const REPRESENTATIVE_BODY: &str = "The rain kept the same beat on the window ledge.";
const PREVIOUS_BODY_COPIES: usize = 3;

#[test]
fn representative_manuscript_appends_each_chapter_body_once() {
    let volumes = [ReadableExportVolume {
        title: "Volume A".to_owned(),
        chapters: vec![ReadableExportChapter {
            title: "Chapter A".to_owned(),
            body: Some(REPRESENTATIVE_BODY.to_owned()),
        }],
    }];
    let expected =
        "# Volume A\n\n## Chapter A\n\nThe rain kept the same beat on the window ledge.\n";
    assert_eq!(render_readable_manuscript(&volumes), expected);

    let mut record = BodyWriteRecord {
        payload: REPRESENTATIVE_BODY,
        payload_write_bytes: 0,
        sink: String::new(),
    };
    append_readable_manuscript(&mut record, &volumes)
        .expect("the public renderer writes into one buffer");
    assert_eq!(record.sink, expected);
    assert_eq!(record.payload_write_bytes, REPRESENTATIVE_BODY.len());
    assert!(record.payload_write_bytes < PREVIOUS_BODY_COPIES * REPRESENTATIVE_BODY.len());
}

struct BodyWriteRecord<'a> {
    payload: &'a str,
    payload_write_bytes: usize,
    sink: String,
}

impl std::fmt::Write for BodyWriteRecord<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if text == self.payload {
            self.payload_write_bytes += text.len();
        }
        self.sink.push_str(text);
        Ok(())
    }
}
