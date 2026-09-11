//! Deterministic musical/absolute time conversion, not tempo inference.
mod tempo;

pub use tempo::{TempoCurve, TempoError, TempoEvent, TempoMap};
