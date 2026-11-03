use serde_json::{Value, json};

use super::EventStream;

/// Gives `chunks` to one event stream, one read at a time, and returns its messages.
fn messages(chunks: &[&[u8]]) -> Vec<Value> {
    let mut stream = EventStream::default();
    let mut buffer = Vec::new();
    let mut messages = Vec::new();
    for chunk in chunks {
        buffer.extend_from_slice(chunk);
        while let Some(message) = stream.next_message(&mut buffer).ok().flatten() {
            messages.push(message);
        }
    }
    messages
}

#[test]
fn a_line_end_across_two_reads_keeps_the_blank_line_that_follows() {
    let data: &[u8] = b"data: {\"id\":1}\r";
    for chunks in [
        &[data, b"\n", b"\n"][..],
        &[data, b"\r"],
        &[data, b"\n\r\n"],
    ] {
        assert_eq!(messages(chunks), vec![json!({"id": 1})]);
    }
}
