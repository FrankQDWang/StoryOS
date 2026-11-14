//! The stub of one Tool result after a compaction (ADR 0093 section 5). A stub keeps the
//! `tool_result` boundary with the identity of its call and the citable sources of ADR 0087, so
//! that the model can cite a source and can read the full result again with `read_tool_result`.

use crate::assemble_context::count_context_item_tokens;
use crate::tool_result::escape;
use crate::{CitableSource, CitedSource};

/// The upper size of one stub, in counted tokens (ADR 0052). A result that is not larger keeps
/// its exact form.
pub const TOOL_RESULT_STUB_TOKEN_LIMIT: u64 = 2_000;

/// The mark at the end of the arguments that the stub cut to stay in its limit.
pub(crate) const ARGUMENTS_CUT_MARK: &str = " [The Host cut these arguments here.]";

/// The facts of one stored call that its stub gives.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StubCall<'a> {
    pub call_id: &'a str,
    pub tool_name: &'a str,
    /// The canonical arguments of the call.
    pub arguments: &'a str,
    /// The Source Snapshots and then the Source Gaps of the result.
    pub sources: &'a [CitableSource],
    /// For a `read_tool_result` call: the call that it read.
    pub read_call_id: Option<&'a str>,
}

/// The stub text of `call`, at most `TOOL_RESULT_STUB_TOKEN_LIMIT` counted tokens. When the
/// source lines do not fit, the stub keeps the first lines and says how many it left out.
pub(crate) fn tool_result_stub(call: &StubCall<'_>) -> String {
    let lines: Vec<String> = call.sources.iter().map(source_line).collect();
    let tag = |kept: &[String]| match kept.is_empty() {
        true => String::new(),
        false => format!(
            "<tool_sources source=\"{}\">\n{}\n</tool_sources>",
            escape(call.call_id),
            kept.join("\n")
        ),
    };
    let arguments = bounded_arguments(call, lines.first().map(|line| tag(&[line.clone()])));
    let mut kept = lines.len();
    loop {
        let stub = format!(
            "{}{}",
            boundary(call, &arguments, kept, lines.len()),
            tag(&lines[..kept])
        );
        if kept == 0 || count_context_item_tokens(&stub) <= TOOL_RESULT_STUB_TOKEN_LIMIT {
            return stub;
        }
        kept -= 1;
    }
}

/// The `tool_result` boundary of the stub, when the tag keeps `kept` of `total` source lines.
fn boundary(call: &StubCall<'_>, arguments: &str, kept: usize, total: usize) -> String {
    let read = match call.read_call_id {
        Some(target) => format!(
            " This result was the full result of the call {target}, and the stub of that call \
             gives its sources. To read the full result again, call read_tool_result with the \
             call_id {target}.",
            target = escape(target)
        ),
        None => format!(
            " To read the full result again, call read_tool_result with the call_id {}.",
            escape(call.call_id)
        ),
    };
    let left_out = match total - kept {
        0 => String::new(),
        others => format!(
            " The tool_sources tag gives only the first {kept} of the {total} sources of the \
             result. Read the full result again before you cite one of the other {others} \
             sources."
        ),
    };
    format!(
        "<tool_result source=\"{}\" state=\"ok\" form=\"stub\">The Host replaced this result with \
         a stub after a compaction of the conversation. Tool: {}. Arguments: {}.{read}{left_out}\
         </tool_result>",
        escape(call.call_id),
        escape(call.tool_name),
        arguments
    )
}

/// The escaped arguments, cut so that the boundary, the longest left-out sentence, and the
/// first source line `first_tag` fit in the limit.
fn bounded_arguments(call: &StubCall<'_>, first_tag: Option<String>) -> String {
    let escaped = escape(call.arguments);
    let total = call.sources.len();
    let fixed = count_context_item_tokens(&boundary(call, "", 0, total))
        + first_tag.as_deref().map_or(0, count_context_item_tokens);
    let budget = TOOL_RESULT_STUB_TOKEN_LIMIT.saturating_sub(fixed);
    if count_context_item_tokens(&escaped) <= budget {
        return escaped;
    }
    let keep = budget.saturating_sub(count_context_item_tokens(ARGUMENTS_CUT_MARK));
    let mut cut = String::new();
    for character in call.arguments.chars() {
        let next = escape(&character.to_string());
        if count_context_item_tokens(&cut) + count_context_item_tokens(&next) > keep {
            break;
        }
        cut.push_str(&next);
    }
    format!("{cut}{ARGUMENTS_CUT_MARK}")
}

/// One line of the tag: a Snapshot with its identity, title, and locator, or a Gap with its
/// identity, locator, and reason.
fn source_line(source: &CitableSource) -> String {
    match source.cited {
        CitedSource::Snapshot => match &source.title {
            Some(title) => format!(
                "<source id=\"{}\" title=\"{}\" locator=\"{}\"/>",
                escape(&source.id),
                escape(title),
                escape(&source.locator)
            ),
            None => format!(
                "<source id=\"{}\" locator=\"{}\"/>",
                escape(&source.id),
                escape(&source.locator)
            ),
        },
        CitedSource::Gap(reason) => format!(
            "<gap id=\"{}\" locator=\"{}\" reason=\"{}\"/>",
            escape(&source.id),
            escape(&source.locator),
            reason.as_str()
        ),
    }
}

#[cfg(test)]
#[path = "tool_result_stub_tests.rs"]
mod tests;
