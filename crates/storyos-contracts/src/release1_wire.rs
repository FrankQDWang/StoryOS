//! Shared helpers for the generated Release 1 wire artifacts.

use schemars::schema_for;
use serde_json::Value;

/// The JSON Schema pattern of a canonical decimal `u64` wire string.
pub(super) const U64_WIRE: &str = "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$";

pub(super) fn schema_value<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Value {
    let mut schema = serde_json::to_value(schema_for!(T)).expect("contract schema serializes");
    schema["$id"] = Value::String(schema_id.to_owned());
    schema["title"] = Value::String(title.to_owned());
    schema
}

pub(super) fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}

pub(super) fn generated_ref(path: &str) -> &str {
    path.strip_prefix("generated/")
        .expect("schema is a generated artifact")
}
