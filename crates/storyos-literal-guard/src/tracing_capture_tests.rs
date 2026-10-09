use super::{FORMAT, Finding, MISSING_SKIP_ALL, RECORDED_RESULT, SHORTHAND, SIGIL, VALUE, find};

fn reasons(source: &str) -> Vec<&'static str> {
    find(source)
        .expect("the snippet parses")
        .into_iter()
        .map(|Finding { reason, .. }| reason)
        .collect()
}

#[test]
fn instrument_needs_skip_all() {
    assert_eq!(
        reasons(
            "#[tracing::instrument]\nasync fn a() {}\n#[instrument(fields(x = 1))]\nasync fn b() {}\n"
        ),
        vec![MISSING_SKIP_ALL, MISSING_SKIP_ALL]
    );
    assert_eq!(
        reasons(
            "#[tracing::instrument(skip_all, name = \"a\", level = \"debug\")]\nasync fn a() {}\n"
        ),
        Vec::<&str>::new()
    );
}

#[test]
fn instrument_refuses_err_and_ret() {
    assert_eq!(
        reasons("#[tracing::instrument(skip_all, err, ret)]\nasync fn a() {}\n"),
        vec![RECORDED_RESULT, RECORDED_RESULT]
    );
}

#[test]
fn sigil_captures_are_refused() {
    assert_eq!(
        reasons(
            "fn f() {\n    tracing::info!(?body, \"x\");\n    tracing::warn!(error = %error, \"y\");\n}\n#[tracing::instrument(skip_all, fields(text = ?text))]\nasync fn a() {}\n"
        ),
        vec![SIGIL, SIGIL, SIGIL]
    );
}

#[test]
fn field_values_are_literals_empty_or_diagnostic() {
    assert_eq!(
        reasons(
            "fn f() {\n    tracing::info!(project_id = project.diagnostic(), attempt = 2, status = tracing::field::Empty, \"done\");\n    tracing::Span::current().record(\"status\", status.diagnostic());\n    store.record(\"key\", body);\n}\n"
        ),
        Vec::<&str>::new()
    );
    assert_eq!(
        reasons(
            "fn f() {\n    tracing::info!(text = block.text, \"x\");\n    tracing::Span::current().record(\"status\", body);\n}\n#[tracing::instrument(skip_all, fields(text = request.body()))]\nasync fn a() {}\n"
        ),
        vec![VALUE, VALUE, VALUE]
    );
}

#[test]
fn shorthand_fields_are_refused() {
    assert_eq!(
        reasons("fn f() {\n    tracing::info!(body, \"x\");\n}\n"),
        vec![SHORTHAND]
    );
    assert_eq!(
        reasons(
            "fn f() {\n    tracing::event!(tracing::Level::INFO, target: \"storyos\", \"x\");\n}\n"
        ),
        Vec::<&str>::new()
    );
}

#[test]
fn messages_do_not_format_values() {
    assert_eq!(
        reasons(
            "fn f() {\n    tracing::info!(\"got {body}\");\n    tracing::warn!(\"got {}\", body);\n}\n"
        ),
        // The placeholder and the argument of the second event are two findings.
        vec![FORMAT, FORMAT, FORMAT]
    );
    assert_eq!(
        reasons(
            "fn f() {\n    tracing::info!(\"literal {{braces}}\");\n    let _span = tracing::info_span!(\"name\", run_id = run.diagnostic());\n}\n"
        ),
        Vec::<&str>::new()
    );
}
