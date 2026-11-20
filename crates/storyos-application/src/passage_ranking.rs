//! The facts of a ranking of the passages of the whole manuscript for a question (ADR 0095
//! sections 1 and 2).

use storyos_core::{ChapterPassages, NearestPassage};

/// The vector of the question and the Embedding Registration that made it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuestionVector<'a> {
    pub registration_revision: &'a str,
    pub values: &'a [f32],
}

/// What the store reads in one transaction: the current passages of each live Chapter in
/// manuscript order, and the nearest vector rows of the current passages in distance order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassageRankingFacts {
    pub chapters: Vec<ChapterPassages>,
    pub nearest: Vec<NearestPassage>,
}
