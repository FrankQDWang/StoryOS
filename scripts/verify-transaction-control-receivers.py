#!/usr/bin/env python3
"""Text check of the connection-reuse rules in crates/storyos-adapter-postgres/AGENTS.md."""

import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIR = ROOT / "crates/storyos-adapter-postgres/src"
CONNECT_OWNERS = {"connection_pool.rs", "storage_activation.rs"}
STARTUP_GATE = "storage_activation_proof.rs"
POOLED_SOURCES = re.compile(r"\.(connect|connect_challenge|checkout)\s*\(")
# `ROLLBACK TO` keeps the transaction open.
TRANSACTION_CONTROL = re.compile(
    r"^\s*(BEGIN|START\s+TRANSACTION|COMMIT|END|ROLLBACK(?!\s+TO\b)|ABORT)\b", re.IGNORECASE
)
STATEMENT_CALL = re.compile(
    r'\.\s*(batch_execute|execute|query|query_one|query_opt|query_raw|simple_query)'
    r'\s*\(\s*(?:r#*)?"((?:[^"\\]|\\.)*)"',
    re.DOTALL,
)
OPAQUE_STATEMENT_CALL = re.compile(
    r"\.\s*(batch_execute|simple_query)\s*\(\s*(?![r\"])[^)]"
)
RECEIVER = re.compile(r"([A-Za-z_][A-Za-z0-9_]*(?:\s*\.\s*[A-Za-z_][A-Za-z0-9_]*)*)\s*$")
FUNCTION = re.compile(r"\bfn\s+[A-Za-z_][A-Za-z0-9_]*\s*")


def balanced_end(text: str, start: int, opening: str, closing: str) -> int:
    depth = 0
    for index in range(start, len(text)):
        if text[index] == opening:
            depth += 1
        elif text[index] == closing:
            depth -= 1
            if depth == 0:
                return index
    return -1


def parameters(text: str, call_start: int) -> dict[str, str]:
    functions = [match for match in FUNCTION.finditer(text, 0, call_start)]
    if not functions:
        return {}
    opening = functions[-1].end()
    if text[opening] == "<":
        opening = balanced_end(text, opening, "<", ">") + 1
        while text[opening].isspace():
            opening += 1
    if text[opening] != "(":
        return {}
    index = balanced_end(text, opening, "(", ")")
    if index < 0:
        return {}
    result: dict[str, str] = {}
    for part in text[opening + 1 : index].split(","):
        name, separator, type_ = part.partition(":")
        if separator:
            result[name.strip().removeprefix("mut ").strip()] = type_.strip()
    return result


def receiver_kind(text: str, call_start: int, receiver: str) -> str:
    root = receiver.split(".")[0].strip()
    if root == "self":
        field = receiver.split(".")[1].strip() if "." in receiver else ""
        if field and re.search(rf"\b{field}\s*:\s*PooledClient\b", text):
            return "pooled"
        return f"self field `{receiver}` is not a `PooledClient`"
    params = parameters(text, call_start)
    if root in params:
        if "PooledClient" in params[root]:
            return "pooled"
        return f"parameter `{root}: {params[root]}` is not a `PooledClient`"
    bindings = list(re.finditer(rf"\blet\s+(?:mut\s+)?{root}\b[^;]*;", text[:call_start]))
    if not bindings:
        return f"receiver `{receiver}` has no local binding or parameter"
    if POOLED_SOURCES.search(bindings[-1].group(0)):
        return "pooled"
    return f"local `{root}` is not bound from `connect()`, `connect_challenge()`, or `checkout()`"


def check(source_dir: Path) -> tuple[list[str], int]:
    violations: list[str] = []
    checked = 0
    for path in sorted(source_dir.rglob("*.rs")):
        if path.name.endswith("_tests.rs"):
            continue
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(source_dir)
        if path.name in CONNECT_OWNERS:
            continue
        if "tokio_postgres::connect" in text:
            line = text[: text.index("tokio_postgres::connect")].count("\n") + 1
            violations.append(
                f"{relative}:{line}: `tokio_postgres::connect` outside the pool; use `connect()`"
            )
        if path.name != STARTUP_GATE and "connection_pool::open" in text:
            line = text[: text.index("connection_pool::open")].count("\n") + 1
            violations.append(
                f"{relative}:{line}: `connection_pool::open` outside the startup gate; use `connect()`"
            )
        for match in OPAQUE_STATEMENT_CALL.finditer(text):
            line = text[: match.start()].count("\n") + 1
            violations.append(
                f"{relative}:{line}: `{match.group(1)}` argument is not a string literal; "
                "the check cannot read it"
            )
        for match in STATEMENT_CALL.finditer(text):
            method, statement = match.group(1), match.group(2)
            if not TRANSACTION_CONTROL.match(statement):
                continue
            checked += 1
            line = text[: match.start()].count("\n") + 1
            first_word = statement.split()[0].upper()
            if method != "batch_execute":
                violations.append(
                    f"{relative}:{line}: `{first_word}` runs through `{method}`; "
                    "use `batch_execute` so the pool records it"
                )
                continue
            receiver_match = RECEIVER.search(text, 0, match.start())
            receiver = re.sub(r"\s+", "", receiver_match.group(1)) if receiver_match else ""
            kind = receiver_kind(text, match.start(), receiver)
            if kind != "pooled":
                violations.append(f"{relative}:{line}: `{first_word}` on {kind}")
    return violations, checked


SAMPLES = {
    "pass_pool.rs": """
impl PostgresProjectReader {
    async fn read(&self, scope: &ProjectScope) -> Result<(), ProjectReadError> {
        let client = self.connect().await?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(read_error)?;
        finish(&client).await
    }
}

async fn finish(client: &PooledClient) -> Result<(), ProjectReadError> {
    client.batch_execute(r"COMMIT").await.map_err(read_error)
}

pub struct Settled {
    client: PooledClient,
}

impl Settled {
    pub async fn rollback(self) -> Result<(), ProjectReadError> {
        self.client.batch_execute("ROLLBACK").await.map_err(read_error)
    }
}
""",
    "fail_parameter.rs": """
async fn read_settlement(
    client: &tokio_postgres::Client,
    receipt_id: &str,
) -> Result<(), ProjectReadError> {
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(read_error)
}
""",
    "fail_generic.rs": """
async fn run<C: Deref<Target = Client>>(client: &C) -> Result<(), tokio_postgres::Error> {
    client.batch_execute("ROLLBACK").await
}
""",
    "fail_open.rs": """
async fn gate(url: &str) -> Result<(), tokio_postgres::Error> {
    let client = crate::connection_pool::open(url).await?;
    client.batch_execute("ROLLBACK TO checkpoint").await
}
""",
    "fail_method.rs": """
async fn begin(client: &PooledClient) -> Result<u64, tokio_postgres::Error> {
    client.execute("BEGIN", &[]).await
}
""",
    "fail_connect.rs": """
async fn open(url: &str) -> Result<tokio_postgres::Client, tokio_postgres::Error> {
    let (client, connection) = tokio_postgres::connect(url, NoTls).await?;
    Ok(client)
}
""",
    "fail_local.rs": """
async fn replay(store: &Store) -> Result<(), ProjectReadError> {
    let client = store.raw_client().await?;
    client.batch_execute("BEGIN").await.map_err(read_error)
}
""",
    "fail_opaque.rs": """
async fn apply(client: &PooledClient, statement: &str) -> Result<(), tokio_postgres::Error> {
    client.batch_execute(statement).await
}
""",
    "ignored_tests.rs": """
async fn raw(client: &tokio_postgres::Client) {
    client.batch_execute("BEGIN").await.unwrap();
}
""",
    "storage_activation.rs": """
async fn apply(client: &tokio_postgres::Client) -> Result<(), tokio_postgres::Error> {
    let (client, connection) = tokio_postgres::connect(admin_url, NoTls).await?;
    client.batch_execute("BEGIN").await
}
""",
}


def self_test() -> None:
    with tempfile.TemporaryDirectory() as directory:
        source_dir = Path(directory)
        for name, body in SAMPLES.items():
            (source_dir / name).write_text(body, encoding="utf-8")
        violations, checked = check(source_dir)
    assert checked == 7, checked
    expected = [
        "fail_connect.rs:3: `tokio_postgres::connect` outside the pool; use `connect()`",
        "fail_generic.rs:3: `ROLLBACK` on parameter `client: &C` is not a `PooledClient`",
        "fail_local.rs:4: `BEGIN` on local `client` is not bound from `connect()`, "
        "`connect_challenge()`, or `checkout()`",
        "fail_method.rs:3: `BEGIN` runs through `execute`; use `batch_execute` so the pool records it",
        "fail_opaque.rs:3: `batch_execute` argument is not a string literal; the check cannot read it",
        "fail_open.rs:3: `connection_pool::open` outside the startup gate; use `connect()`",
        "fail_parameter.rs:7: `BEGIN` on parameter `client: &tokio_postgres::Client` is not a `PooledClient`",
    ]
    assert violations == expected, violations
    print("verify-transaction-control-receivers self-test passed")


def main() -> None:
    if sys.argv[1:] == ["--self-test"]:
        self_test()
        return
    violations, checked = check(SOURCE_DIR)
    if violations:
        raise SystemExit(
            "Transaction control must run through PooledClient::batch_execute:\n"
            + "\n".join(violations)
        )
    print(f"verified {checked} transaction-control statements on PooledClient receivers")


if __name__ == "__main__":
    main()
