//! Writes the TypeScript client function and declaration of each operation from its registry entry.

use crate::release1_operation_registry::{ClientOverride, OperationKind, RegisteredOperation};

/// The request digest function of a command, then the operation function, each after an empty line.
pub(super) fn client_source(registered: &RegisteredOperation) -> String {
    if let ClientOverride::Source { client, .. } = registered.client {
        return client();
    }
    let shape = Shape::of(registered);
    let id = shape.id;
    let mut source = String::new();
    if let Some(canonical) = &shape.digest {
        let digest = format!("digest{}", shape.type_prefix);
        source.push_str(&format!(
            concat!(
                "\nexport async function {digest}(request, cryptoImpl = globalThis.crypto) {{\n",
                "  if (!request || typeof request !== \"object\") throw new TypeError(\"{digest} requires request\");\n",
                "  const canonical = {canonical};\n",
                "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
                "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
                "  return {{ algorithm: \"sha256\", profile: \"storyos.command.{id}.jcs.v1\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            ),
            digest = digest,
            canonical = canonical,
            id = id,
        ));
    }
    let mut parameters = shape
        .path_parameters
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if shape.has_body {
        parameters.push("request");
    }
    if shape.kind == OperationKind::Command {
        parameters.extend(["idempotencyKey", "antiForgery"]);
    }
    if let Some((name, _)) = &shape.query_parameter {
        parameters.push(name);
    }
    if parameters.is_empty() {
        source.push_str(&format!(
            "\nexport async function {id}(options = {{}}) {{\n"
        ));
    } else {
        source.push_str(&format!(
            "\nexport async function {id}({{ {}, ...options }} = {{}}) {{\n",
            parameters.join(", ")
        ));
    }
    for parameter in &shape.path_parameters {
        source.push_str(&format!(
            "  if (typeof {parameter} !== \"string\" || {parameter}.length === 0) throw new TypeError(\"{id} requires {parameter}\");\n"
        ));
    }
    if shape.has_body {
        source.push_str(&format!(
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"{id} requires request\");\n"
        ));
    }
    let mut headers = "";
    if shape.kind == OperationKind::Command {
        if shape.optional_anti_forgery {
            source.push_str(&format!(
                concat!(
                    "  if (typeof idempotencyKey !== \"string\") throw new TypeError(\"{id} requires an idempotency key\");\n",
                    "  const commandHeaders = {{ \"idempotency-key\": idempotencyKey }};\n",
                    "  if (typeof antiForgery === \"string\") commandHeaders[\"x-storyos-anti-forgery\"] = antiForgery;\n",
                ),
                id = id,
            ));
            headers = ", commandHeaders";
        } else {
            source.push_str(&format!(
                "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"{id} requires security bindings\");\n"
            ));
            headers = ", commandHeaders: { \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }";
        }
    }
    if let Some((name, wire)) = &shape.query_parameter {
        source.push_str(&format!(
            "  const query = {name} == null || {name} === \"\" ? \"\" : `?{wire}=${{encodeURIComponent({name})}}`;\n"
        ));
    }
    let path = shape.path_expression();
    let call = match shape.kind {
        OperationKind::Query if shape.has_body => {
            format!("queryPostJson({{ ...options, path: {path}, body: request }})")
        }
        OperationKind::Query => format!("queryJson({{ ...options, path: {path} }})"),
        OperationKind::Command | OperationKind::Challenge => format!(
            "commandJson({{ ...options, method: \"{}\", path: {path}, body: request{headers} }})",
            shape.method
        ),
        OperationKind::Stream => panic!("{id}: a stream client needs ClientOverride::Source"),
    };
    source.push_str(&format!("  return {call};\n}}\n"));
    source
}

/// The declarations of the functions that `client_source` writes, in the same order.
pub(super) fn declaration_source(registered: &RegisteredOperation) -> String {
    if let ClientOverride::Source { declaration, .. } = registered.client {
        return declaration.to_owned();
    }
    let shape = Shape::of(registered);
    let prefix = &shape.type_prefix;
    let mut source = String::new();
    if shape.digest.is_some() {
        source.push_str(&format!(
            "export declare function digest{prefix}(request: {prefix}Request, cryptoImpl?: Crypto): Promise<DigestValue>;\n"
        ));
    }
    let mut fields = shape
        .path_parameters
        .iter()
        .map(|parameter| format!("{parameter}: string"))
        .collect::<Vec<_>>();
    if shape.has_body {
        fields.push(format!("request: {prefix}Request"));
    }
    if shape.kind == OperationKind::Command {
        fields.push("idempotencyKey: string".to_owned());
        fields.push(if shape.optional_anti_forgery {
            "antiForgery?: string".to_owned()
        } else {
            "antiForgery: string".to_owned()
        });
    }
    if let Some((name, _)) = &shape.query_parameter {
        fields.push(format!("{name}?: string | null"));
    }
    let options = if fields.is_empty() {
        "StoryOSQueryOptions".to_owned()
    } else {
        format!("StoryOSQueryOptions & {{ {} }}", fields.join("; "))
    };
    source.push_str(&format!(
        "export declare function {}(options: {options}): Promise<{}>;\n",
        shape.id, shape.response_type
    ));
    source
}

/// The client facts of one operation after its `ClientOverride` applies.
struct Shape {
    id: &'static str,
    kind: OperationKind,
    method: &'static str,
    path: &'static str,
    /// The operation ID with an uppercase first letter: the prefix of its type names.
    type_prefix: String,
    response_type: String,
    path_parameters: Vec<String>,
    /// Each command and challenge, and a query that posts, sends the request as the body.
    has_body: bool,
    /// The canonical JavaScript value of the request digest of a command.
    digest: Option<String>,
    optional_anti_forgery: bool,
    /// The TypeScript name and the wire name of the optional query parameter.
    query_parameter: Option<(String, &'static str)>,
}

impl Shape {
    fn of(registered: &RegisteredOperation) -> Self {
        let operation = registered.operation;
        let id = operation.operation_id;
        let kind = registered.kind;
        let mut type_prefix = id[..1].to_ascii_uppercase();
        type_prefix.push_str(&id[1..]);
        let mut shape = Self {
            id,
            kind,
            method: operation.method,
            path: operation.path,
            response_type: format!("{type_prefix}Response"),
            type_prefix,
            path_parameters: operation
                .path
                .split('/')
                .filter_map(path_parameter)
                .map(camel_case)
                .collect(),
            has_body: match kind {
                OperationKind::Command | OperationKind::Challenge => true,
                OperationKind::Query => operation.method == "POST",
                OperationKind::Stream => false,
            },
            digest: (kind == OperationKind::Command).then(|| "canonicalJson(request)".to_owned()),
            optional_anti_forgery: false,
            query_parameter: None,
        };
        match registered.client {
            ClientOverride::None | ClientOverride::Source { .. } => {}
            ClientOverride::ResponseType(response_type) => {
                shape.response_type = response_type.to_owned();
            }
            ClientOverride::NoDigest => shape.digest = None,
            ClientOverride::DigestFields(fields) => {
                let fields = fields
                    .iter()
                    .map(|field| format!("{field}: request.{field}"))
                    .collect::<Vec<_>>();
                shape.digest = Some(format!("{{ {} }}", fields.join(", ")));
            }
            ClientOverride::OptionalAntiForgery => shape.optional_anti_forgery = true,
            ClientOverride::OptionalQueryParameter(wire) => {
                shape.query_parameter = Some((camel_case(wire), wire));
            }
        }
        shape
    }

    /// The JavaScript expression of the request path.
    fn path_expression(&self) -> String {
        if self.path_parameters.is_empty() && self.query_parameter.is_none() {
            return format!("\"{}\"", self.path);
        }
        let mut path = self
            .path
            .split('/')
            .map(|segment| match path_parameter(segment) {
                Some(name) => format!("${{encodeURIComponent({})}}", camel_case(name)),
                None => segment.to_owned(),
            })
            .collect::<Vec<_>>()
            .join("/");
        if self.query_parameter.is_some() {
            path.push_str("${query}");
        }
        format!("`{path}`")
    }
}

/// The name of a `{name}` path segment.
fn path_parameter(segment: &str) -> Option<&str> {
    segment.strip_prefix('{')?.strip_suffix('}')
}

/// `project_id` becomes `projectId`.
fn camel_case(snake: &str) -> String {
    let mut parts = snake.split('_');
    let mut camel = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let mut characters = part.chars();
        if let Some(first) = characters.next() {
            camel.push(first.to_ascii_uppercase());
            camel.extend(characters);
        }
    }
    camel
}
