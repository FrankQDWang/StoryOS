//! The settlement type that the application crate declares for one command (ADR 0080).

use storyos_application::{
    ActionApplied, ActivityApplied, ChapterSelectionApplied, ProjectCommandSettlement,
    RevisionApplied, StructureApplied, UndoApplied,
};
use storyos_core::ReasonCode;

use super::response::ResponseValue;
use crate::author_edit::AuthorEditProfileApplied;

/// The parameters of the settlement type of one command.
///
/// `ProjectCommandSettlement` implements it once. A command names its settlement type in
/// `ProjectCommand::Settlement`, and the sequence reads the command effect, the three reason
/// types, the response record, and the zero-authority effect from it.
pub(crate) trait SettlementShape {
    type Effect: Send;
    type NoEffect: ReasonCode + Send + Sync;
    type Conflict: ReasonCode + Send + Sync;
    type Refusal: ReasonCode + Send + Sync;
    type Response: TryFrom<ResponseValue> + Send;
    type ZeroEffect: Send;
}

impl<A, N, C, R, P, Z> SettlementShape for ProjectCommandSettlement<A, N, C, R, P, Z>
where
    A: AppliedRecord,
    N: ReasonCode + Send + Sync,
    C: ReasonCode + Send + Sync,
    R: ReasonCode + Send + Sync,
    P: TryFrom<ResponseValue> + Send,
    Z: Send,
{
    type Effect = A::Effect;
    type NoEffect = N;
    type Conflict = C;
    type Refusal = R;
    type Response = P;
    type ZeroEffect = Z;
}

/// The applied value of one settlement profile, which holds the effect of the command.
///
/// Each profile declares its applied value with this trait, so that the sequence reads the
/// command effect from the settlement type.
pub(crate) trait AppliedRecord {
    type Effect: Send;
}

impl<E: Send> AppliedRecord for ActionApplied<E> {
    type Effect = E;
}

impl<E: Send> AppliedRecord for ActivityApplied<E> {
    type Effect = E;
}

impl<E: Send> AppliedRecord for RevisionApplied<E> {
    type Effect = E;
}

impl<E: Send> AppliedRecord for StructureApplied<E> {
    type Effect = E;
}

impl<E: Send> AppliedRecord for ChapterSelectionApplied<E> {
    type Effect = E;
}

impl<E: Send> AppliedRecord for UndoApplied<E> {
    type Effect = E;
}

impl<E: Send> AppliedRecord for AuthorEditProfileApplied<E> {
    type Effect = E;
}
