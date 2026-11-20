use std::fmt::Debug;

use storyos_application::{ProjectCommandError, RefusableCommandError};

use crate::shared_tests::command_sequence_support::SequenceError;

impl SequenceError for ProjectCommandError {
    fn sequence(self) -> Self {
        self
    }
}

impl<R: Debug> SequenceError for RefusableCommandError<R> {
    fn sequence(self) -> ProjectCommandError {
        match self {
            Self::Command(error) => error,
            Self::RefusedBeforeAdmission(refusal) => {
                panic!("the command refused before its Admission: {refusal:?}")
            }
        }
    }
}
