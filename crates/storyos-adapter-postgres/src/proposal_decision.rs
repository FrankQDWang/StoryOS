//! The steps that the ActionOnly Proposal decisions share (ADR 0080).

use storyos_application::EditorSessionId;

use crate::command_replay::{CommandReplay, ReplayFault, StateAxis};
use crate::command_sequence::{
    ActionOnly, Admission, Classification, Classified, EditorAdmission, ProjectCommand,
    ReceiptHeads, ZeroReceipt,
};

/// The Classification of one ActionOnly Proposal decision. The decision is an explicit editor
/// command of one Chapter, and its Receipt records the expected Chapter head as the expected,
/// prior, and resulting head.
pub(crate) fn decision_classification<C: ProjectCommand<Profile = ActionOnly>>(
    outcome: Classified<C>,
    editor_session_id: &EditorSessionId,
    chapter_object_id: String,
    expected_authoritative_revision_id: &str,
) -> Classification<C> {
    let head = vec![expected_authoritative_revision_id.to_owned()];
    Classification {
        outcome,
        admission: Admission::ExplicitEditorCommand(EditorAdmission {
            editor_session_id: editor_session_id.as_ref().to_owned(),
            chapter_object_id: Some(chapter_object_id),
            expected_authoritative_revision_id: Some(expected_authoritative_revision_id.to_owned()),
            target_refs: Vec::new(),
        }),
        heads: ReceiptHeads {
            expected: head.clone(),
            prior: head.clone(),
            resulting: head,
        },
        zero_receipt: ZeroReceipt::Reason,
    }
}

/// The effect record of one applied Proposal decision that replay reads.
pub(crate) struct DecisionRecord<'a> {
    replay: &'a CommandReplay,
}

impl DecisionRecord<'_> {
    /// One required string field of the record. An absent field is damaged evidence.
    pub(crate) fn text(&self, key: &str) -> Result<String, ReplayFault> {
        self.replay.effect_text(key)?.ok_or_else(|| {
            ReplayFault::Unavailable("the applied decision record is missing or damaged".into())
        })
    }

    /// One required Proposal state of the record on `axis`.
    pub(crate) fn state(&self, key: &str, axis: StateAxis) -> Result<String, ReplayFault> {
        axis.preserved(self.text(key)?)
    }

    /// The Proposal state on `axis` that the decision preserved, from its `preserved_*` field.
    pub(crate) fn preserved(&self, axis: StateAxis) -> Result<String, ReplayFault> {
        let key = match axis {
            StateAxis::Generation => "preserved_generation",
            StateAxis::Validation => "preserved_validation",
            StateAxis::Closure => "preserved_closure",
            StateAxis::OperationResolution => "preserved_operation_resolution",
        };
        self.state(key, axis)
    }
}

/// Decodes the applied effect of one Proposal decision from its replayed Receipt and its effect
/// record. The Receipt payload must hold the value `transition.1` in the field `transition.0`.
///
/// A pre-capture record holds a value in each other field and no value in its `preserved_*`
/// fields. Its acknowledgement is historical. `effect` reads the fields of every other record, and reports a missing value
/// as damaged evidence.
pub(crate) fn decode_decision<E>(
    replay: &CommandReplay,
    transition: (&str, &str),
    effect: impl FnOnce(&DecisionRecord<'_>) -> Result<E, ReplayFault>,
) -> Result<E, ReplayFault> {
    let (key, value) = transition;
    replay.require_receipt_text(key, value)?;
    let fields = replay.effect().fields();
    let preserved = |column: &String| column.starts_with("preserved_");
    if fields.keys().any(preserved)
        && fields
            .iter()
            .all(|(column, value)| preserved(column) == value.is_null())
    {
        return Err(ReplayFault::HistoricalAcknowledgementUnavailable);
    }
    effect(&DecisionRecord { replay })
}
