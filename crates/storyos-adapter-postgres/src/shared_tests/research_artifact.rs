//! The shared test support from `research_artifact_tests.rs`.

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// One reply of the model destination, made from the JSON body of its request.
pub(crate) type Reply = Box<dyn FnOnce(&Value) -> String + Send>;

/// A model destination that reads each complete request and answers it with the next reply.
/// Returns the base URL and the request lines.
pub(crate) async fn computed(
    replies: Vec<Reply>,
) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    let served = tokio::spawn(async move {
        let mut lines = Vec::new();
        for reply in replies {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 8192];
            let head_end = loop {
                if let Some(end) = request
                    .windows(/*size*/ 4)
                    .position(|window| window == b"\r\n\r\n")
                {
                    break end + 4;
                }
                let read = stream.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..read]);
            };
            let raw = String::from_utf8_lossy(&request[..head_end]).into_owned();
            lines.push(raw.lines().next().unwrap_or_default().to_owned());
            let head = raw.to_lowercase();
            let length: usize = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .map_or(0, |value| value.trim().parse().unwrap());
            while request.len() < head_end + length {
                let read = stream.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..read]);
            }
            let body = serde_json::from_slice(&request[head_end..]).unwrap_or(Value::Null);
            stream.write_all(reply(&body).as_bytes()).await.unwrap();
        }
        lines
    });
    (base, served)
}

/// The identities that the `tool_sources` tags of the function outputs of `body` name after
/// `tag`, in order.
pub(crate) fn tagged(body: &Value, tag: &str) -> Vec<String> {
    body["input"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["output"].as_str())
        .flat_map(|output| {
            output
                .split(tag)
                .skip(1)
                .map(|rest| rest.split('"').next().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}
