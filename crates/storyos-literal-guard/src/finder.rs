//! Finds positional opaque literal arguments that have no `/*param*/` comment.

use std::collections::BTreeSet;

use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprLit, ExprPath, Lit, UnOp};

/// One call argument that is a bare opaque literal without a `/*param*/` comment.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Finding {
    /// One-based line of the argument.
    pub(crate) line: usize,
    /// One-based character column of the argument.
    pub(crate) column: usize,
    pub(crate) callee: String,
    pub(crate) literal: String,
}

/// Returns the findings of one Rust source file in source order.
pub(crate) fn find(source: &str, exempt_methods: &BTreeSet<String>) -> syn::Result<Vec<Finding>> {
    let file = syn::parse_str::<syn::File>(source)?;
    let mut finder = Finder {
        source,
        exempt_methods,
        findings: Vec::new(),
    };
    finder.visit_file(&file);
    Ok(finder.findings)
}

struct Finder<'a> {
    source: &'a str,
    exempt_methods: &'a BTreeSet<String>,
    findings: Vec<Finding>,
}

impl Finder<'_> {
    fn check_arguments<'e>(&mut self, callee: &str, arguments: impl Iterator<Item = &'e Expr>) {
        for argument in arguments {
            if !is_opaque_literal(argument) {
                continue;
            }
            let span = argument.span();
            let range = span.byte_range();
            if has_name_comment(&self.source[..range.start]) {
                continue;
            }
            let start = span.start();
            self.findings.push(Finding {
                line: start.line,
                column: start.column + 1,
                callee: callee.to_owned(),
                literal: self.source[range].to_owned(),
            });
        }
    }
}

impl<'ast> Visit<'ast> for Finder<'_> {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        let callee = match &*call.func {
            // A callee with an upper-case last segment is a tuple struct or enum variant, such as `Some(1)`.
            Expr::Path(ExprPath { path, .. }) => path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .filter(|name| !name.starts_with(|first: char| first.is_ascii_uppercase())),
            callee => Some(self.source[callee.span().byte_range()].to_owned()),
        };
        if let Some(callee) = callee {
            self.check_arguments(&callee, call.args.iter());
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let method = call.method.to_string();
        if !(call.args.len() == 1 && self.exempt_methods.contains(&method)) {
            self.check_arguments(&method, call.args.iter());
        }
        visit::visit_expr_method_call(self, call);
    }
}

fn is_opaque_literal(argument: &Expr) -> bool {
    match argument {
        Expr::Lit(ExprLit {
            lit: Lit::Bool(_) | Lit::Int(_) | Lit::Float(_),
            ..
        }) => true,
        Expr::Unary(unary) => {
            matches!(unary.op, UnOp::Neg(_))
                && matches!(
                    &*unary.expr,
                    Expr::Lit(ExprLit {
                        lit: Lit::Int(_) | Lit::Float(_),
                        ..
                    })
                )
        }
        Expr::Path(ExprPath {
            qself: None, path, ..
        }) => path.is_ident("None"),
        _ => false,
    }
}

fn has_name_comment(before: &str) -> bool {
    let Some(body) = before.trim_end().strip_suffix("*/") else {
        return false;
    };
    let Some(start) = body.rfind("/*") else {
        return false;
    };
    let name = &body[start + 2..];
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
#[path = "finder_tests.rs"]
mod tests;
