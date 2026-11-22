//! The input, the start rule, and the outcome of an intent pass (ADR 0101 section 4): the Context
//! Assembly of its RunSteps from the Character File that the store loads (ADR 0039), the decision
//! of a claimed schedule row, the outcome of a settled call, and the outcome of a pass whose Run
//! ends with no settled call.

use std::time::{Duration, SystemTime};

use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextCause, ContextPurpose, ContextSourceClass,
    ContextSufficiency, CurrentPassageAssemblyRecord, DestinationIo, HostControlRecord,
    InstructionBindingInput, ManifestCommit, OperationRequirementRecord, ProjectionMode,
    SelectedProjection, TOKEN_COUNTING_ALGORITHM_REVISION, TOKEN_COUNTING_PROFILE_REVISION,
    count_context_item_tokens,
};
use crate::record_character_intent::INTENT_RECORDED;
use crate::{
    CaptureRefusal, CaptureRunEnd, ToolCallState, ToolResultCandidate, ToolResultText,
    host_tool_result,
};

/// The most RunSteps of one intent pass: one request, and one more after a refused call.
pub const INTENT_PASS_MAX_RUN_STEPS: u64 = 2;

/// Decides the start of an intent pass with the admission rule of a capture. The scope of the
/// grant names the Purpose `intent_pass`.
pub fn admit_intent_pass(
    binding: bool,
    assistance_available: bool,
    grant_scope: Option<&str>,
) -> Result<(), CaptureRefusal> {
    crate::memory_consolidation::admit_background_purpose(
        ContextPurpose::IntentPass,
        binding,
        assistance_available,
        grant_scope,
    )
}

/// The Character File of one character: its name, the text of the part of the author when the
/// Project has one, and the text of the part of the background work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterFile<'a> {
    pub name: &'a str,
    pub author_text: Option<&'a str>,
    pub manuscript_text: &'a str,
}

impl CharacterFile<'_> {
    /// The exact text of the file item: the line `{"name": name}`, then each part with text under
    /// its heading.
    pub fn text(&self) -> String {
        let name = crate::canonical_json(&serde_json::json!({ "name": self.name }));
        let parts = [
            ("Told by the author", self.author_text),
            ("From the manuscript", Some(self.manuscript_text)),
        ];
        std::iter::once(name)
            .chain(parts.into_iter().filter_map(|(heading, text)| {
                text.filter(|text| !text.is_empty())
                    .map(|text| format!("{heading}:\n{text}"))
            }))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// The identities and the input of one RunStep of an intent pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentPassAssembly<'a> {
    pub operation_requirement_id: String,
    pub run_id: String,
    pub owner_user_id: String,
    pub project_id: String,
    pub destination_identity: String,
    pub file: CharacterFile<'a>,
}

/// The Context Assembly of an intent pass: the Purpose `intent_pass`, the cause `host_schedule`,
/// and the one `character_file` item. The pass has no Memory Index, no Chapter, and no other entry
/// (ADR 0101 section 4).
pub fn assemble_intent_pass_context(
    input: &IntentPassAssembly<'_>,
) -> CurrentPassageAssemblyRecord {
    let content = input.file.text();
    let item = SelectedProjection {
        source_class: ContextSourceClass::CharacterFile,
        source_version: format!("sha256:{}", crate::hex_sha256(content.as_bytes())),
        projection_mode: ProjectionMode::ExactRequired,
        token_count: count_context_item_tokens(&content),
        content,
    };
    CurrentPassageAssemblyRecord {
        operation_requirement: OperationRequirementRecord {
            operation_requirement_id: input.operation_requirement_id.clone(),
            input_snapshot_id: input.operation_requirement_id.clone(),
            run_id: input.run_id.clone(),
            owner_user_id: input.owner_user_id.clone(),
            project_id: input.project_id.clone(),
            purpose: ContextPurpose::IntentPass,
            cause: ContextCause::HostSchedule,
            chapter_id: None,
            chapter_revision_id: None,
            proposal_target_block_ids: None,
            passage_targets: None,
            candidate_target: None,
            selection_references: None,
            ordinary_resolution: None,
            tool_results: None,
            earlier_author_messages: None,
            skill_requirements: None,
            skill_catalog: None,
            memory_revision: None,
            instruction: InstructionBindingInput::Absent,
            destination_identity: input.destination_identity.clone(),
            item_token_limit: CONTEXT_ITEM_TOKEN_LIMIT,
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
        },
        sufficiency: ContextSufficiency::Complete,
        considered: vec![ConsideredSource {
            source_class: item.source_class,
            source_version: item.source_version.clone(),
            token_count: item.token_count,
            eligible: true,
        }],
        selected: vec![item],
        rejected: Vec::new(),
        host_control: HostControlRecord {
            distinct_from_destination: true,
            destination_visible: false,
        },
        token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
        token_counting_algorithm_revision: TOKEN_COUNTING_ALGORITHM_REVISION.to_owned(),
        manifests: ManifestCommit {
            assembly: true,
            destination_and_outbound: false,
        },
        destination_io: DestinationIo::None,
    }
}

/// The Context Assembly of the second RunStep of an intent pass: the file item of the RunStep
/// `before` and the result of each resolved call of the Run.
pub fn next_intent_pass_context(
    before: &CurrentPassageAssemblyRecord,
    operation_requirement_id: String,
    candidates: &[ToolResultCandidate<'_>],
) -> CurrentPassageAssemblyRecord {
    let mut record = before.clone();
    let requirement = &mut record.operation_requirement;
    requirement
        .input_snapshot_id
        .clone_from(&operation_requirement_id);
    requirement.operation_requirement_id = operation_requirement_id;
    let file = |class: ContextSourceClass| class == ContextSourceClass::CharacterFile;
    record.selected.retain(|item| file(item.source_class));
    record.considered.retain(|source| file(source.source_class));
    crate::assemble_tool_result_continuation(&mut record, candidates);
    record
}

/// Whether an intent pass sends its second request after a RunStep whose calls have the states
/// `states`. Only a call that the Host refused gives the second request, with the refusal as its
/// Tool result. A reply with no call ends the pass (ADR 0101 section 4).
pub fn intent_pass_sends_again(states: impl IntoIterator<Item = ToolCallState>) -> bool {
    states
        .into_iter()
        .any(|state| state == ToolCallState::Refused)
}

/// The Chapter of an intent pass: the last Chapter of `chapters` that has text when the pass
/// starts. `chapters` has each live Chapter in Chapter order, with its ID and the text of each of
/// its blocks. A block of only white space has no text.
pub fn intent_pass_chapter<'a>(chapters: &[(&'a str, Vec<&str>)]) -> Option<&'a str> {
    (chapters.iter().rev())
        .find(|(_, blocks)| blocks.iter().any(|text| !text.trim().is_empty()))
        .map(|(chapter_id, _)| *chapter_id)
}

/// The outcome of an intent pass whose call settled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentPassOutcome {
    /// The result is a version of the part of the background work.
    Recorded,
    /// A part of the Character File changed after the start, so the Host wrote no version, and
    /// the entry needs a pass again.
    NotKept,
}

impl IntentPassOutcome {
    /// The persisted name of the outcome, for `intent_passes.outcome`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::NotKept => "not_kept",
        }
    }
}

/// The failure reason of a running intent pass from the facts of its Run. `None`: the pass
/// continues. A pass that ends with no settled `record_character_intent` call fails, and it
/// records no result.
pub fn intent_pass_failure(end: CaptureRunEnd<'_>) -> Option<&str> {
    match end {
        // The last RunStep ended with no call, or with a call that the Host refused.
        CaptureRunEnd::LastRunStepEnded
        | CaptureRunEnd::Status {
            status: "completed",
            ..
        } => Some("no_intent_call"),
        CaptureRunEnd::Status {
            status: "queued" | "claimed",
            ..
        } => None,
        CaptureRunEnd::Status { status, reason } => Some(reason.unwrap_or(status)),
    }
}

/// The shortest time from the start of one intent pass of a Project to the start of the next
/// (ADR 0101 section 4).
pub(crate) const INTENT_PASS_INTERVAL: Duration = Duration::from_secs(/*1 minute*/ 60);

/// The facts of one claimed intent pass row of a Project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentPassFacts {
    /// A character entry of the background work needs a pass.
    pub entry_needs_pass: bool,
    /// An intent pass of the Project is in progress.
    pub pass_in_progress: bool,
    /// An observation of a request waits in the Project. A conflict item is not one.
    pub observation_waits: bool,
    /// A consolidation pass of the Project is in progress.
    pub consolidation_in_progress: bool,
    /// The time of the claim.
    pub now: SystemTime,
    /// The start of the last intent pass of the Project.
    pub last_pass_start: Option<SystemTime>,
}

/// Why a claimed intent pass row started no pass. The due time becomes empty, except after
/// `RateLimited`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentPassSkip {
    /// The next settlement that makes the row due sets the due time again.
    NoEntryNeedsPass,
    /// The settlement of that pass sets the due time again when an entry still needs a pass.
    PassInProgress,
    /// The settlement of the consolidation pass of the observation sets the due time again.
    ObservationWaits,
    /// The settlement of that consolidation pass sets the due time again.
    ConsolidationInProgress,
    /// The row is due at `due_at`, the first time that the rate permits a start.
    RateLimited { due_at: SystemTime },
    /// The admission refused the start.
    StartRefused,
}

impl IntentPassSkip {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoEntryNeedsPass => "no_entry_needs_pass",
            Self::PassInProgress => "pass_in_progress",
            Self::ObservationWaits => "observation_waits",
            Self::ConsolidationInProgress => "consolidation_in_progress",
            Self::RateLimited { .. } => "rate_limited",
            Self::StartRefused => "start_refused",
        }
    }
}

/// Decides whether a claimed intent pass row starts one pass. `Ok`: one pass starts, when the
/// admission of the start permits it. The consolidation work of the Project goes first, and a
/// Project starts at most one intent pass in `INTENT_PASS_INTERVAL` (ADR 0101 section 4).
pub fn decide_intent_pass(facts: &IntentPassFacts) -> Result<(), IntentPassSkip> {
    if !facts.entry_needs_pass {
        return Err(IntentPassSkip::NoEntryNeedsPass);
    }
    if facts.pass_in_progress {
        return Err(IntentPassSkip::PassInProgress);
    }
    if facts.observation_waits {
        return Err(IntentPassSkip::ObservationWaits);
    }
    if facts.consolidation_in_progress {
        return Err(IntentPassSkip::ConsolidationInProgress);
    }
    let due_at = intent_pass_due(facts.now, facts.last_pass_start);
    match due_at > facts.now {
        true => Err(IntentPassSkip::RateLimited { due_at }),
        false => Ok(()),
    }
}

/// The due time of the intent pass row of a Project that a due mark at `now` writes: `now`, or the
/// start of the last pass `last_pass_start` and `INTENT_PASS_INTERVAL` if that is later. Thus a
/// due mark never makes the row due before the rate permits a start.
pub fn intent_pass_due(now: SystemTime, last_pass_start: Option<SystemTime>) -> SystemTime {
    let earliest = last_pass_start.map(|start| start + INTENT_PASS_INTERVAL);
    earliest.map_or(now, |earliest| now.max(earliest))
}

/// Whether the applied Memory write `version` can change a Character File: the type of the version
/// or of the entry before it in `entries` is `character` (a change can change the type).
pub fn write_can_change_character(
    version: &crate::MemoryVersionWrite,
    entries: &[crate::CurrentMemoryEntry],
) -> bool {
    let character = crate::MemoryEntryType::Character;
    let before = (entries.iter())
        .find(|entry| version.memory_entry_id.as_ref() == Some(&entry.memory_entry_id));
    version.entry_type == character || before.is_some_and(|entry| entry.entry_type == character)
}

/// The facts of one Memory Entry of a Project for the start rule of an intent pass: the state and
/// the type of its last version, and the facts of the version of its last change, which is not a
/// flag version and not an intent version (ADR 0094 section 7). `author`: the last change has the
/// origin of a request of the author.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentPassEntryFacts {
    pub memory_entry_id: String,
    pub entry_number: u64,
    pub state: String,
    pub entry_type: String,
    pub author: bool,
    pub title: String,
    pub version: String,
    pub text: String,
}

/// The entries and versions that one intent pass read: the background work, and the author part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentPassRead {
    pub entry: (String, String),
    pub author: Option<(String, String)>,
}

/// One intent pass of the Project: the versions that it read, and its outcome label, or `None`
/// while it runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentPassRecord {
    pub read: IntentPassRead,
    pub outcome: Option<String>,
}

/// The character that needs a pass: its entry of the background work, and the author entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeededCharacter<'a> {
    pub entry: &'a IntentPassEntryFacts,
    pub author: Option<&'a IntentPassEntryFacts>,
}

impl NeededCharacter<'_> {
    /// The entries and versions that a pass of this character reads.
    pub fn read(&self) -> IntentPassRead {
        let key =
            |entry: &IntentPassEntryFacts| (entry.memory_entry_id.clone(), entry.version.clone());
        IntentPassRead {
            entry: key(self.entry),
            author: self.author.map(key),
        }
    }
}

/// Each character of `entries`: each current character entry of the background work, with the
/// current character entry of the author with the same title.
fn characters(entries: &[IntentPassEntryFacts]) -> impl Iterator<Item = NeededCharacter<'_>> {
    let character =
        |entry: &&IntentPassEntryFacts| entry.state == "current" && entry.entry_type == "character";
    (entries.iter().filter(character))
        .filter(|entry| !entry.author)
        .map(move |entry| NeededCharacter {
            entry,
            author: (entries.iter().filter(character))
                .find(|author| author.author && author.title == entry.title),
        })
}

/// The character that the next intent pass is for: the current character entry of the background
/// work with the lowest number that needs a pass. The two versions of a character are the version
/// of the last change of the entry and the version of the last change of the current character
/// entry of the author with the same title. An entry needs a pass unless the last pass of the
/// entry that recorded a result read the current two versions, or a failed pass later than that
/// pass read the current two versions. A pass that is not kept does not count as a pass that
/// recorded a result or as a pass that failed (#1790 items 4 and 5). A character with only the
/// entry of the author needs no pass (ADR 0101 section 4). `passes` is in start order.
pub fn select_intent_pass_entry<'a>(
    entries: &'a [IntentPassEntryFacts],
    passes: &[IntentPassRecord],
) -> Option<NeededCharacter<'a>> {
    let ended = |pass: &IntentPassRecord, outcome: &str| pass.outcome.as_deref() == Some(outcome);
    let recorded = IntentPassOutcome::Recorded.label();
    characters(entries)
        .filter(|needed| {
            let read = needed.read();
            let own = (passes.iter())
                .filter(|pass| pass.read.entry.0 == read.entry.0)
                .collect::<Vec<_>>();
            let last = own.iter().rposition(|pass| ended(pass, recorded));
            // The passes of the entry after its last recorded pass.
            let after = &own[last.map_or(0, |last| last + 1)..];
            let failed = (after.iter()).any(|pass| ended(pass, "failed") && pass.read == read);
            !failed && last.is_none_or(|last| own[last].read != read)
        })
        .min_by_key(|needed| needed.entry.entry_number)
}

/// The outcome of a pass whose call is valid, and the result text of the call. The result is a
/// version of the entry only while the versions that the pass `read` are the current versions of
/// the character in `entries`, and while no consolidation pass of the Project is in progress. Else
/// no part of the result is kept: a change, a delete, or a new entry of the author after the start
/// gives `NotKept`. The consolidation work comes first: a version would move the Memory revision
/// and make the running consolidation pass stale (ADR 0101 section 4).
pub fn settle_intent_result(
    read: &IntentPassRead,
    entries: &[IntentPassEntryFacts],
    consolidation_in_progress: bool,
) -> (IntentPassOutcome, ToolResultText) {
    let current = characters(entries).find(|needed| needed.entry.memory_entry_id == read.entry.0);
    let (outcome, text) = match current.is_some_and(|needed| needed.read() == *read) {
        _ if consolidation_in_progress => (IntentPassOutcome::NotKept, INTENT_CONSOLIDATION_FIRST),
        true => (IntentPassOutcome::Recorded, INTENT_RECORDED),
        false => (IntentPassOutcome::NotKept, INTENT_NOT_KEPT),
    };
    (outcome, host_tool_result(text))
}

/// The result text of a valid call whose Character File changed after the start of the pass.
const INTENT_NOT_KEPT: &str = "The Host did not keep the intent: the character file changed after \
                               the pass read it.";

/// The result text of a valid call while a consolidation pass of the Project is in progress.
const INTENT_CONSOLIDATION_FIRST: &str = "The Host did not keep the intent: a consolidation pass \
                                          of the Memory is in progress.";

#[cfg(test)]
#[path = "intent_pass_tests.rs"]
mod tests;
