//! Map the complete message text of one response to a decision candidate.

use storyos_application::DeclaredTarget;
use storyos_core::{DecisionCandidate, ModelOutput, OutputPhase, ProseChangeCandidate};

/// Text that satisfies the reply contract maps to its candidate, and other text to an Advisory.
/// Core validation of the complete candidate decides whether it becomes the Agent Decision.
pub(crate) fn map_output(text: &str, targets: &[DeclaredTarget]) -> ModelOutput {
    let (candidate, prose_changes) = match contract_candidate(text, targets) {
        Some(mapped) => mapped,
        None => (
            DecisionCandidate::Advisory {
                text: text.to_owned(),
            },
            None,
        ),
    };
    ModelOutput {
        phase: OutputPhase::FinalAnswer,
        candidate,
        prose_changes,
    }
}

type Mapped = (DecisionCandidate, Option<Vec<ProseChangeCandidate>>);

fn contract_candidate(text: &str, targets: &[DeclaredTarget]) -> Option<Mapped> {
    let reply: serde_json::Value = serde_json::from_str(without_code_fence(text)).ok()?;
    let field = |name: &str| reply.get(name)?.as_str().map(str::to_owned);
    match reply.get("kind")?.as_str()? {
        "advisory" => Some((
            DecisionCandidate::Advisory {
                text: field("text")?,
            },
            None,
        )),
        "clarification" => Some((
            DecisionCandidate::Clarification {
                question: field("question")?,
            },
            None,
        )),
        "prose_change" if !targets.is_empty() => {
            let changes = reply
                .get("changes")?
                .as_array()?
                .iter()
                .map(|change| prose_change(change, targets))
                .collect::<Option<Vec<_>>>()?;
            Some((
                DecisionCandidate::ProseChange {
                    text: field("summary")?,
                },
                Some(changes),
            ))
        }
        _ => None,
    }
}

/// A change for an undeclared block keeps an empty base revision, so core validation refuses it.
fn prose_change(
    change: &serde_json::Value,
    targets: &[DeclaredTarget],
) -> Option<ProseChangeCandidate> {
    let field = |name: &str| change.get(name)?.as_str().map(str::to_owned);
    let block_id = field("block_id")?;
    let target = targets.iter().find(|target| target.block_id == block_id);
    Some(ProseChangeCandidate {
        chapter_id: target
            .map(|target| target.chapter_id.clone())
            .unwrap_or_default(),
        base_authoritative_revision_id: target
            .map(|target| target.base_revision_id.clone())
            .unwrap_or_default(),
        manuscript_block_id: block_id,
        candidate_text: field("candidate_text")?,
        explanation: field("explanation")?,
    })
}

/// The JSON inside one Markdown code fence, or the trimmed text.
fn without_code_fence(text: &str) -> &str {
    let trimmed = text.trim();
    trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|inner| inner.strip_suffix("```"))
        .map_or(trimmed, str::trim)
}
