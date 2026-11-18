use storyos_core::{EmbeddingSettlement, HOST_FAKE_EMBEDDING_REGISTRATION};

use super::{EmbeddingObservation, EmbeddingUsage, settle};

/// A vector with one component above the range of `f32` is the invalid result, and the caller
/// gets no vector (#1696). An adapter casts each component to `f32`, so the component is infinite.
/// A vector of finite components with the count of the dimensions settles as a vector.
#[test]
fn a_vector_with_a_component_that_is_not_finite_is_an_invalid_result() {
    let registration = &HOST_FAKE_EMBEDDING_REGISTRATION;
    let usage = EmbeddingUsage::Reported { input_tokens: 3 };
    let finite = vec![0.5_f32; registration.dimensions];
    let mut above = finite.clone();
    above[7] = 1e39_f64 as f32;
    let settled = |values: Vec<f32>| {
        let observation = EmbeddingObservation::Vector { values, usage };
        let (record, vector) = settle(registration, &observation);
        (record.settlement, record.usage, vector.is_some())
    };

    assert_eq!(
        (settled(finite), settled(above)),
        (
            (EmbeddingSettlement::Vector, usage, true),
            (EmbeddingSettlement::InvalidResult, usage, false)
        )
    );
}
