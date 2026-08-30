use storyos_core::{ManuscriptBlock, ManuscriptBlockKind};

use super::{display_body_from_stored, parse_versioned_payload, persist_canonical_bytes};

#[test]
fn two_paragraphs_including_empty_right_roundtrip_through_canonical_bytes() {
    let blocks = vec![
        ManuscriptBlock {
            manuscript_block_id: "018f0000-0000-7001-8000-0000000000b1".to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: "Hello".to_owned(),
        },
        ManuscriptBlock {
            manuscript_block_id: "018f0000-0000-7001-8000-0000000000b2".to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: String::new(),
        },
    ];
    let stored = persist_canonical_bytes(&blocks);
    let parsed = parse_versioned_payload(&stored).expect("versioned payload");
    assert_eq!(parsed, blocks);
    assert_eq!(display_body_from_stored(&stored, &parsed), "Hello\n");
}
