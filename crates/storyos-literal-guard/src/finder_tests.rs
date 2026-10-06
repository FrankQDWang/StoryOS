use std::collections::BTreeSet;

use super::{Finding, find};

fn findings(source: &str, exempt_methods: &[&str]) -> Vec<Finding> {
    let exempt_methods = exempt_methods
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    find(source, &exempt_methods).expect("the snippet parses")
}

fn finding(line: usize, column: usize, callee: &str, literal: &str) -> Finding {
    Finding {
        line,
        column,
        callee: callee.to_owned(),
        literal: literal.to_owned(),
    }
}

#[test]
fn bare_opaque_literals_fire() {
    let source = "fn f() {\n    open(None, true, 3, -2.5);\n    client.send(false);\n    run(|| step(0));\n    (callback)(false);\n    handlers[0](3);\n}\n";
    assert_eq!(
        findings(source, &[]),
        vec![
            finding(/*line*/ 2, /*column*/ 10, "open", "None"),
            finding(/*line*/ 2, /*column*/ 16, "open", "true"),
            finding(/*line*/ 2, /*column*/ 22, "open", "3"),
            finding(/*line*/ 2, /*column*/ 25, "open", "-2.5"),
            finding(/*line*/ 3, /*column*/ 17, "send", "false"),
            finding(/*line*/ 4, /*column*/ 17, "step", "0"),
            finding(/*line*/ 5, /*column*/ 16, "(callback)", "false"),
            finding(/*line*/ 6, /*column*/ 17, "handlers[0]", "3"),
        ]
    );
}

#[test]
fn commented_literals_pass() {
    let source = "fn f() {\n    open(/*path*/ None, /*create*/true,\n        /*mode*/ 3);\n    client.send(/*retry*/ false);\n}\n";
    assert_eq!(findings(source, &[]), vec![]);
}

#[test]
fn skipped_shapes_do_not_fire() {
    let source = r#"fn f() {
    assert_eq!(open(true), 1);
    let (a, 2) = pair(x);
    if let Some(3) = value(y) {}
    take((1, true));
    take([0; 4]);
    build(Config { retries: 3, verbose: false });
    write("text", 'c');
    item(list[0], 0..5);
    count = 7;
    let wrapped = Some(1);
    let parsed = Ok::<_, ()>(true);
}
"#;
    assert_eq!(findings(source, &[]), vec![]);
}

#[test]
fn exemption_list_passes_only_sole_argument_methods_it_names() {
    let source = "fn f() {\n    widget.enabled(false);\n    widget.visible(true);\n    widget.enabled(true, 2);\n    enabled(false);\n}\n";
    assert_eq!(
        findings(source, &["enabled"]),
        vec![
            finding(/*line*/ 3, /*column*/ 20, "visible", "true"),
            finding(/*line*/ 4, /*column*/ 20, "enabled", "true"),
            finding(/*line*/ 4, /*column*/ 26, "enabled", "2"),
            finding(/*line*/ 5, /*column*/ 13, "enabled", "false"),
        ]
    );
}
