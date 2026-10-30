//! The settlement profile of `applyAuthorEdit` (ADR 0076).

use storyos_application::{
    ActionApplied, AuthorEditError, ProjectCommandEnvelope, ProjectCommandError, ProjectScope,
    RevisionApplied,
};
use tokio_postgres::Client;

use super::sequence::AuthorEditPlan;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActionOnly, ActionSequence, AppliedVariant, AuthoritativeRevision, RevisionSequences,
    RevisionWrite, SelectsRecords, SettlementProfile, SingleRecordSet, unavailable,
};

/// The record set of an applied Author Edit.
#[derive(Clone, Copy)]
pub(crate) enum AuthorEditSelector {
    /// A new Authoritative Revision.
    Revision,
    /// A new Proposal Revision and a Forward Author Action.
    Proposal,
}

impl SelectsRecords<AuthorEditProfile> for AuthorEditPlan {
    async fn allocate_records(
        &self,
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<AuthorEditSequences, ProjectCommandError> {
        Ok(match self.kind() {
            AuthorEditSelector::Revision => {
                AuthorEditSequences::Revision(AuthoritativeRevision::allocate(client, scope).await?)
            }
            AuthorEditSelector::Proposal => {
                AuthorEditSequences::Proposal(ActionOnly::allocate(client, scope).await?)
            }
        })
    }
}

pub(crate) enum AuthorEditSequences {
    Revision(RevisionSequences),
    Proposal(ActionSequence),
}

/// The applied writes of an Author Edit.
pub(crate) enum AuthorEditWrite<E> {
    Revision(Box<RevisionWrite<E>>),
    Proposal { effect: E },
}

/// The applied value of an Author Edit settlement.
#[derive(Debug)]
pub(crate) enum AuthorEditProfileApplied<E> {
    Revision(RevisionApplied<E>),
    Proposal(ActionApplied<E>),
}

/// The settlement profile of `applyAuthorEdit`: `AuthoritativeRevision` for an applied
/// Authoritative Revision, and `ActionOnly` for a revised Proposal candidate.
pub(crate) struct AuthorEditProfile;

impl SettlementProfile for AuthorEditProfile {
    type Sequences = AuthorEditSequences;
    type Write<E: Send> = AuthorEditWrite<E>;
    type Applied<E: Send> = AuthorEditProfileApplied<E>;
    /// The applied variant that `ActionOnly` reads.
    type Input = AppliedVariant;

    fn commit_ids(sequences: &AuthorEditSequences) -> Vec<String> {
        match sequences {
            AuthorEditSequences::Revision(sequences) => {
                AuthoritativeRevision::commit_ids(sequences)
            }
            AuthorEditSequences::Proposal(sequence) => ActionOnly::commit_ids(sequence),
        }
    }

    fn revision_ids(sequences: &AuthorEditSequences) -> Vec<String> {
        match sequences {
            AuthorEditSequences::Revision(sequences) => {
                AuthoritativeRevision::revision_ids(sequences)
            }
            AuthorEditSequences::Proposal(_) => Vec::new(),
        }
    }

    async fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        variant: AppliedVariant,
        sequences: AuthorEditSequences,
        write: AuthorEditWrite<E>,
    ) -> Result<AuthorEditProfileApplied<E>, ProjectCommandError> {
        match (sequences, write) {
            (AuthorEditSequences::Revision(sequences), AuthorEditWrite::Revision(revision)) => {
                let applied =
                    AuthoritativeRevision::persist(client, envelope, (), sequences, *revision)
                        .await?;
                Ok(AuthorEditProfileApplied::Revision(applied))
            }
            (AuthorEditSequences::Proposal(sequence), AuthorEditWrite::Proposal { effect }) => {
                let applied =
                    ActionOnly::persist(client, envelope, variant, sequence, effect).await?;
                Ok(AuthorEditProfileApplied::Proposal(applied))
            }
            (AuthorEditSequences::Revision(_), AuthorEditWrite::Proposal { .. })
            | (AuthorEditSequences::Proposal(_), AuthorEditWrite::Revision { .. }) => Err(
                unavailable("the Author Edit write does not match its records"),
            ),
        }
    }

    fn replay<E: Send>(
        decode: impl FnOnce() -> Result<E, ReplayFault>,
        replay: &CommandReplay,
    ) -> Result<AuthorEditProfileApplied<E>, ReplayFault> {
        match replay.result_kind() {
            "authoritative_applied" => AuthoritativeRevision::replay(decode, replay)
                .map(AuthorEditProfileApplied::Revision),
            "proposal_revised" => {
                ActionOnly::replay(decode, replay).map(AuthorEditProfileApplied::Proposal)
            }
            _ => Err(ReplayFault::BindingConflict),
        }
    }
}

pub(super) fn project_error(error: AuthorEditError) -> ProjectCommandError {
    match error {
        AuthorEditError::BindingConflict => ProjectCommandError::BindingConflict,
        AuthorEditError::InvalidChallenge => ProjectCommandError::InvalidChallenge,
        AuthorEditError::StaleWriter => ProjectCommandError::WriterIneligible,
        AuthorEditError::AdmissionExpired => unavailable(error),
        AuthorEditError::Unavailable(source) => ProjectCommandError::Unavailable(source),
    }
}
