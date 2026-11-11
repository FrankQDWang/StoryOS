//! The research synthesis of an AgentRun (S5-07, ADR 0087). The model gives a summary and an
//! ordered list of claims, and each claim cites Source Snapshots or Source Gaps of this AgentRun
//! by their identities. Core checks each citation against the sources that the Run captured. The
//! Host does no claim splitting and no support classification.

use crate::SourceGapReason;

/// What a citable identity names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CitedSource {
    Snapshot,
    Gap(SourceGapReason),
}

/// One Source Snapshot or Source Gap that a ToolCall captured, as the store reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CitableSource {
    pub id: String,
    pub cited: CitedSource,
    pub locator: String,
    pub owner_user_id: String,
    pub project_id: String,
    pub run_id: String,
}

/// The sources that the AgentRun `run_id` of one exact Project Scope can cite.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CitableSources {
    pub owner_user_id: String,
    pub project_id: String,
    pub run_id: String,
    pub sources: Vec<CitableSource>,
}

/// One claim as the model gave it: its text and the identities that it cites.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimCandidate {
    pub text: String,
    pub citations: Vec<String>,
}

/// One citation of a valid claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Citation {
    Snapshot { source_snapshot_id: String },
    Gap { source_gap_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchClaim {
    pub text: String,
    pub citations: Vec<Citation>,
}

/// A valid research synthesis. A claim that cites only Gaps, or a summary that states an unmet
/// objective, is a valid partial deliverable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchSynthesis {
    pub summary: String,
    pub claims: Vec<ResearchClaim>,
}

/// Why Core refuses a research synthesis. The whole Decision is then invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SynthesisRefusal {
    NoClaim,
    EmptyText,
    UncitedClaim,
    /// A citation names no Snapshot and no Gap that this AgentRun captured.
    UncapturedCitation,
    /// A citation names a source of another Project Scope or another AgentRun.
    OtherScopeCitation,
}

impl SynthesisRefusal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoClaim => "no_claim",
            Self::EmptyText => "empty_text",
            Self::UncitedClaim => "uncited_claim",
            Self::UncapturedCitation => "uncaptured_citation",
            Self::OtherScopeCitation => "other_scope_citation",
        }
    }
}

/// Validates the synthesis of the model against the sources that its AgentRun captured.
pub fn validate_research_synthesis(
    summary: &str,
    claims: &[ClaimCandidate],
    citable: &CitableSources,
) -> Result<ResearchSynthesis, SynthesisRefusal> {
    if claims.is_empty() {
        return Err(SynthesisRefusal::NoClaim);
    }
    if summary.trim().is_empty() || claims.iter().any(|claim| claim.text.trim().is_empty()) {
        return Err(SynthesisRefusal::EmptyText);
    }
    let claims = claims
        .iter()
        .map(|claim| {
            if claim.citations.is_empty() {
                return Err(SynthesisRefusal::UncitedClaim);
            }
            let citations = claim
                .citations
                .iter()
                .map(|id| citation(id, citable))
                .collect::<Result<_, _>>()?;
            Ok(ResearchClaim {
                text: claim.text.clone(),
                citations,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(ResearchSynthesis {
        summary: summary.to_owned(),
        claims,
    })
}

fn citation(id: &str, citable: &CitableSources) -> Result<Citation, SynthesisRefusal> {
    let source = citable
        .sources
        .iter()
        .find(|source| source.id == id)
        .ok_or(SynthesisRefusal::UncapturedCitation)?;
    if (&source.owner_user_id, &source.project_id, &source.run_id)
        != (&citable.owner_user_id, &citable.project_id, &citable.run_id)
    {
        return Err(SynthesisRefusal::OtherScopeCitation);
    }
    Ok(match source.cited {
        CitedSource::Snapshot => Citation::Snapshot {
            source_snapshot_id: id.to_owned(),
        },
        CitedSource::Gap(_) => Citation::Gap {
            source_gap_id: id.to_owned(),
        },
    })
}

impl ResearchSynthesis {
    /// The stored content of the Research Artifact: the summary and the claims in order.
    pub fn content(&self) -> serde_json::Value {
        let claims: Vec<_> = self
            .claims
            .iter()
            .map(|claim| {
                let citations: Vec<_> = claim
                    .citations
                    .iter()
                    .map(|citation| match citation {
                        Citation::Snapshot { source_snapshot_id } => serde_json::json!({
                            "kind": "snapshot", "source_snapshot_id": source_snapshot_id
                        }),
                        Citation::Gap { source_gap_id } => {
                            serde_json::json!({"kind": "gap", "source_gap_id": source_gap_id})
                        }
                    })
                    .collect();
                serde_json::json!({"text": claim.text, "citations": citations})
            })
            .collect();
        serde_json::json!({"summary": self.summary, "claims": claims})
    }

    /// The SHA-256 of the canonical JSON of `content`.
    pub fn content_digest(&self) -> String {
        let content = crate::canonical_json(&self.content());
        format!("sha256:{}", crate::hex_sha256(content.as_bytes()))
    }
}

/// The sentence of the Host instructions that tells the model how to cite.
pub const CITABLE_SOURCES_INSTRUCTION: &str = "A tool_sources tag after a tool_result lists the \
id of each source and each gap of that result. A gap is a requested page that is not available.";

/// The citable identities of one ToolCall output, as data in their own tag after the result
/// boundary. The Host made each identity, so the model can copy it exactly. A ToolCall with no
/// Snapshot and no Gap has no tag.
pub fn citable_sources_tag(call_id: &str, sources: &[CitableSource]) -> String {
    if sources.is_empty() {
        return String::new();
    }
    let escape = crate::tool_result::escape;
    let entries: String = sources
        .iter()
        .map(|source| match source.cited {
            CitedSource::Snapshot => format!(
                "<source id=\"{}\" locator=\"{}\"/>",
                escape(&source.id),
                escape(&source.locator)
            ),
            CitedSource::Gap(reason) => format!(
                "<gap id=\"{}\" locator=\"{}\" reason=\"{}\"/>",
                escape(&source.id),
                escape(&source.locator),
                reason.as_str()
            ),
        })
        .collect();
    format!(
        "<tool_sources source=\"{}\">{entries}</tool_sources>",
        escape(call_id)
    )
}

#[cfg(test)]
#[path = "research_synthesis_tests.rs"]
mod tests;
