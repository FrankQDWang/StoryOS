//! The Tool Discovery Record of a `Discover` exchange, and the shape check of a `Call` result.

use serde::Serialize;
use serde_json::Value;

/// The bound of the total text of one Call result.
pub(crate) const MAX_TEXT_BYTES: usize = storyos_core::TOOL_RESULT_TEXT_MAX_BYTES;

/// What one `Discover` exchange observed. It is untrusted: it has no Registration identity, no
/// enablement, and no execution authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToolDiscoveryRecord {
    pub(crate) server_url: String,
    pub(crate) server_name: String,
    pub(crate) server_version: String,
    pub(crate) protocol_revision: String,
    pub(crate) tools: Vec<DiscoveredTool>,
}

/// One tool as the server describes it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoveredTool {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) input_schema: Value,
    /// `sha256:` and the hex SHA-256 of the canonical JSON of the input schema.
    pub(crate) input_schema_digest: String,
    pub(crate) output_schema: Option<Value>,
    pub(crate) annotations: Option<Value>,
}

impl ToolDiscoveryRecord {
    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    pub fn server_version(&self) -> &str {
        &self.server_version
    }

    pub fn protocol_revision(&self) -> &str {
        &self.protocol_revision
    }

    pub fn tools(&self) -> &[DiscoveredTool] {
        &self.tools
    }
}

impl DiscoveredTool {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn input_schema(&self) -> &Value {
        &self.input_schema
    }

    pub fn input_schema_digest(&self) -> &str {
        &self.input_schema_digest
    }

    pub fn output_schema(&self) -> Option<&Value> {
        self.output_schema.as_ref()
    }

    pub fn annotations(&self) -> Option<&Value> {
        self.annotations.as_ref()
    }
}

/// A Call result that passed the shape check. Semantic validation belongs to S5-03.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallResult {
    pub text: Vec<String>,
    pub is_error: bool,
    /// The `structuredContent` of the result, kept as data.
    pub structured_content: Option<Value>,
}

/// The server name, the server version, and the protocol revision of an `initialize` result.
pub(crate) fn server_identity(result: &Value) -> Result<(String, String, String), &'static str> {
    if result.pointer("/capabilities/tools").is_none() {
        return Err("tools_capability");
    }
    let text = |pointer| result.pointer(pointer).and_then(Value::as_str);
    match (
        text("/serverInfo/name"),
        text("/serverInfo/version"),
        text("/protocolVersion"),
    ) {
        (Some(name), Some(version), Some(revision)) => {
            Ok((name.to_owned(), version.to_owned(), revision.to_owned()))
        }
        (_, _, None) => Err("protocol_revision"),
        _ => Err("server_info"),
    }
}

/// Appends the tools of one `tools/list` page, and returns the cursor of the next page.
pub(crate) fn tools_page(
    page: &Value,
    tools: &mut Vec<DiscoveredTool>,
) -> Result<Option<String>, &'static str> {
    let listed = page.get("tools").and_then(Value::as_array).ok_or("tools")?;
    for tool in listed {
        let tool = discovered_tool(tool).ok_or("invalid_tool")?;
        if tools.iter().any(|known| known.name == tool.name) {
            return Err("duplicate_tool");
        }
        tools.push(tool);
    }
    match page.get("nextCursor") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(cursor)) => Ok(Some(cursor.clone())),
        Some(_) => Err("next_cursor"),
    }
}

fn discovered_tool(tool: &Value) -> Option<DiscoveredTool> {
    let object = |field| match tool.get(field) {
        None => Some(None),
        Some(value @ Value::Object(_)) => Some(Some(value.clone())),
        Some(_) => None,
    };
    let description = match tool.get("description") {
        None => None,
        Some(Value::String(description)) => Some(description.clone()),
        Some(_) => return None,
    };
    let input_schema = tool
        .get("inputSchema")
        .filter(|schema| schema.is_object())?;
    Some(DiscoveredTool {
        name: tool.get("name")?.as_str()?.to_owned(),
        description,
        input_schema: input_schema.clone(),
        input_schema_digest: digest(input_schema),
        output_schema: object("outputSchema")?,
        annotations: object("annotations")?,
    })
}

/// `sha256:` and the hex SHA-256 of the canonical JSON of a value.
pub(crate) fn digest(value: &Value) -> String {
    let canonical = storyos_core::canonical_json(value);
    format!("sha256:{}", storyos_core::hex_sha256(canonical.as_bytes()))
}

/// Text blocks only, a boolean or absent `isError`, and at most `MAX_TEXT_BYTES` of text. The
/// body decoder already refused bytes that are not UTF-8.
pub(crate) fn call_result(mut result: Value) -> Result<CallResult, &'static str> {
    let blocks = result
        .get("content")
        .and_then(Value::as_array)
        .ok_or("content")?;
    let mut text = Vec::with_capacity(blocks.len());
    for block in blocks {
        match (block.get("type").and_then(Value::as_str), block.get("text")) {
            (Some("text"), Some(Value::String(block))) => text.push(block.clone()),
            _ => return Err("non_text_block"),
        }
    }
    if text.iter().map(String::len).sum::<usize>() > MAX_TEXT_BYTES {
        return Err("text_too_large");
    }
    let is_error = match result.get("isError") {
        None => false,
        Some(Value::Bool(is_error)) => *is_error,
        Some(_) => return Err("is_error"),
    };
    Ok(CallResult {
        text,
        is_error,
        structured_content: result.get_mut("structuredContent").map(Value::take),
    })
}
