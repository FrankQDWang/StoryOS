//! Finds `tracing` forms that can record author text or a secret (ADR 0047).

use proc_macro2::{Spacing, TokenStream, TokenTree};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Lit, Meta};

/// One `tracing` form that can record the `Display` or `Debug` text of a value.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Finding {
    /// One-based line of the form.
    pub(crate) line: usize,
    /// One-based character column of the form.
    pub(crate) column: usize,
    pub(crate) reason: &'static str,
}

pub(crate) const MISSING_SKIP_ALL: &str = "#[tracing::instrument] needs skip_all";
pub(crate) const RECORDED_RESULT: &str = "err and ret record the Display or Debug text of a result";
pub(crate) const SIGIL: &str = "a ? or % capture records the Debug or Display text of a value";
pub(crate) const SHORTHAND: &str =
    "a shorthand field records a variable; write name = value.diagnostic()";
pub(crate) const VALUE: &str =
    "a field value must be a literal, tracing::field::Empty, or a DiagnosticField value";
pub(crate) const FORMAT: &str = "a message must not format a value";

const EVENTS: &[&str] = &["trace", "debug", "info", "warn", "error", "event"];
const SPANS: &[&str] = &[
    "span",
    "trace_span",
    "debug_span",
    "info_span",
    "warn_span",
    "error_span",
];
const DIRECTIVES: &[&str] = &["target", "parent", "name"];

/// Returns the findings of one Rust source file in source order.
pub(crate) fn find(source: &str) -> syn::Result<Vec<Finding>> {
    let file = syn::parse_str::<syn::File>(source)?;
    let mut finder = Finder {
        findings: Vec::new(),
    };
    finder.visit_file(&file);
    finder
        .findings
        .sort_by_key(|finding| (finding.line, finding.column));
    Ok(finder.findings)
}

struct Finder {
    findings: Vec<Finding>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    Event,
    Span,
    Fields,
}

impl Finder {
    fn report(&mut self, spanned: &impl Spanned, reason: &'static str) {
        let start = spanned.span().start();
        self.findings.push(Finding {
            line: start.line,
            column: start.column + 1,
            reason,
        });
    }

    fn check_instrument(&mut self, attribute: &syn::Attribute) {
        let Meta::List(list) = &attribute.meta else {
            self.report(attribute, MISSING_SKIP_ALL);
            return;
        };
        let mut skip_all = false;
        for segment in segments(list.tokens.clone()) {
            match segment.as_slice() {
                [TokenTree::Ident(ident), ..] if ident == "skip_all" => skip_all = true,
                [TokenTree::Ident(ident), ..] if ident == "err" || ident == "ret" => {
                    self.report(ident, RECORDED_RESULT);
                }
                [TokenTree::Ident(ident), TokenTree::Group(group)] if ident == "fields" => {
                    self.check_fields(group.stream(), Form::Fields);
                }
                _ => {}
            }
        }
        if !skip_all {
            self.report(attribute, MISSING_SKIP_ALL);
        }
    }

    fn check_fields(&mut self, tokens: TokenStream, form: Form) {
        let mut message = false;
        for segment in segments(tokens) {
            let Some(first) = segment.first() else {
                continue;
            };
            if message {
                self.report(first, FORMAT);
                continue;
            }
            if let [TokenTree::Ident(ident), TokenTree::Punct(colon), ..] = segment.as_slice()
                && colon.as_char() == ':'
                && colon.spacing() == Spacing::Alone
                && DIRECTIVES.iter().any(|name| ident == name)
            {
                continue;
            }
            if is_sigil(first) {
                self.report(first, SIGIL);
                continue;
            }
            if let Some(index) = assignment(&segment) {
                let value = &segment[index + 1..];
                match value.first() {
                    Some(sigil) if is_sigil(sigil) => self.report(sigil, SIGIL),
                    Some(start) => {
                        let allowed = syn::parse2::<Expr>(value.iter().cloned().collect())
                            .is_ok_and(|expr| allowed_value(&expr));
                        if !allowed {
                            self.report(start, VALUE);
                        }
                    }
                    None => self.report(first, VALUE),
                }
                continue;
            }
            match syn::parse2::<Expr>(segment.iter().cloned().collect()) {
                Ok(Expr::Lit(literal)) => {
                    if let Lit::Str(text) = &literal.lit
                        && text.value().replace("{{", "").contains('{')
                    {
                        self.report(first, FORMAT);
                    }
                    message = form == Form::Event;
                }
                // A level argument, such as `Level::INFO`, has more than one segment.
                Ok(Expr::Path(path)) if path.path.segments.len() > 1 => {}
                _ => self.report(first, SHORTHAND),
            }
        }
    }
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        let path = attribute.path();
        let instrument = path
            .segments
            .last()
            .is_some_and(|last| last.ident == "instrument")
            && (path.segments.len() == 1
                || (path.segments.len() == 2 && path.segments[0].ident == "tracing"));
        if instrument {
            self.check_instrument(attribute);
        }
        visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let path = &invocation.path;
        let tracing = path.segments.len() == 1
            || (path.segments.len() == 2 && path.segments[0].ident == "tracing");
        if let Some(last) = path.segments.last().filter(|_| tracing) {
            if EVENTS.iter().any(|name| last.ident == name) {
                self.check_fields(invocation.tokens.clone(), Form::Event);
            } else if SPANS.iter().any(|name| last.ident == name) {
                self.check_fields(invocation.tokens.clone(), Form::Span);
            }
        }
        visit::visit_macro(self, invocation);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let current_span = matches!(
            &*call.receiver,
            Expr::Call(receiver) if matches!(
                &*receiver.func,
                Expr::Path(path) if path.path.segments.len() >= 2
                    && path.path.segments.iter().rev().take(2).map(|segment| segment.ident.to_string()).eq(["current", "Span"])
            )
        );
        if current_span
            && call.method == "record"
            && let Some(value) = call.args.iter().nth(/*n*/ 1)
            && !allowed_value(value)
        {
            self.report(value, VALUE);
        }
        visit::visit_expr_method_call(self, call);
    }
}

/// Splits a token stream at its top-level commas.
fn segments(tokens: TokenStream) -> Vec<Vec<TokenTree>> {
    let mut segments = vec![Vec::new()];
    for token in tokens {
        match &token {
            TokenTree::Punct(punct) if punct.as_char() == ',' => segments.push(Vec::new()),
            _ => segments.last_mut().expect("one segment").push(token),
        }
    }
    segments
}

/// Returns the index of the `=` of a `name = value` field.
fn assignment(segment: &[TokenTree]) -> Option<usize> {
    segment.iter().enumerate().position(|(index, token)| {
        matches!(token, TokenTree::Punct(punct) if punct.as_char() == '=' && punct.spacing() == Spacing::Alone)
            && !matches!(
                index.checked_sub(1).and_then(|previous| segment.get(previous)),
                Some(TokenTree::Punct(previous)) if previous.spacing() == Spacing::Joint
            )
    })
}

fn is_sigil(token: &TokenTree) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.as_char() == '?' || punct.as_char() == '%')
}

fn allowed_value(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) => true,
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|last| last.ident == "Empty"),
        Expr::MethodCall(call) => call.method == "diagnostic" && call.args.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
#[path = "tracing_capture_tests.rs"]
mod tests;
