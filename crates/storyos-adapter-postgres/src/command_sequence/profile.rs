//! The settlement profiles of the sequence: the authority records that an applied project
//! command allocates and writes (ADR 0076).

use std::future::Future;

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError, ProjectScope};
use tokio_postgres::Client;

use super::{AppliedRecord, AppliedVariant, LockedProject};
use crate::command_replay::{CommandReplay, ReplayFault};

/// The authority records that an applied project command allocates and writes (ADR 0076).
///
/// A command names one profile. The sequence allocates the profile sequences through the plan
/// (`SelectsRecords`) before the Domain Receipt, and calls `persist` after the command effect rows.
/// On an exact retry, it calls `replay` instead. Implementations never run transaction control
/// and never write the Admission, Receipt, settlement link, or idempotency rows.
pub(crate) trait SettlementProfile {
    type Sequences: Send + Sync;
    /// The applied writes that a command of this profile returns.
    type Write<E: Send>: Send;
    /// The applied value of the settlement, equal on first delivery and replay.
    type Applied<E: Send>: Send + AppliedRecord<Effect = E>;
    /// The settlement facts besides the envelope that `persist` reads.
    type Input: PersistInput + Send;

    /// The Authoritative Commit identities that the Domain Receipt binds.
    fn commit_ids(sequences: &Self::Sequences) -> Vec<String>;

    /// The Authoritative Revisions that the applied outcome creates. The Domain Receipt binds
    /// them and gives them as its resulting heads.
    fn revision_ids(_sequences: &Self::Sequences) -> Vec<String> {
        Vec::new()
    }

    fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        input: Self::Input,
        sequences: Self::Sequences,
        write: Self::Write<E>,
    ) -> impl Future<Output = Result<Self::Applied<E>, ProjectCommandError>> + Send;

    /// Checks the authority evidence of the profile, and only then runs `decode`, so a command
    /// decode never hides damaged authority evidence.
    fn replay<E: Send>(
        decode: impl FnOnce() -> Result<E, ReplayFault>,
        replay: &CommandReplay,
    ) -> Result<Self::Applied<E>, ReplayFault>;
}

/// The settlement facts of one applied outcome besides the envelope.
pub(crate) struct PersistFacts<'a> {
    pub(crate) project: &'a LockedProject,
    pub(crate) activity_kind: &'static str,
    pub(crate) variant: AppliedVariant,
}

/// The input of `persist` of one profile: only the settlement facts that the profile reads.
pub(crate) trait PersistInput {
    fn from_facts(facts: &PersistFacts<'_>) -> Self;
}

/// A profile that reads only the envelope.
impl PersistInput for () {
    fn from_facts(_facts: &PersistFacts<'_>) -> Self {}
}

/// A profile whose records record the Receipt result kind of the applied variant.
impl PersistInput for AppliedVariant {
    fn from_facts(facts: &PersistFacts<'_>) -> Self {
        facts.variant
    }
}

/// A settlement profile of one record set. Every plan allocates its sequences.
pub(crate) trait SingleRecordSet: SettlementProfile {
    fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<Self::Sequences, ProjectCommandError>> + Send;
}

/// Allocates the sequences of the record set of the settlement profile `P` that one applied plan
/// selects.
///
/// A profile of one record set has a blanket implementation for every plan. A profile of more
/// than one record set selects the set from the plan.
pub(crate) trait SelectsRecords<P: SettlementProfile> {
    fn allocate_records(
        &self,
        client: &Client,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<P::Sequences, ProjectCommandError>> + Send;
}

impl<P: SingleRecordSet, T> SelectsRecords<P> for T {
    fn allocate_records(
        &self,
        client: &Client,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<P::Sequences, ProjectCommandError>> + Send {
        P::allocate(client, scope)
    }
}
