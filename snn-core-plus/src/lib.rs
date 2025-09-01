//! snn-core-plus: Extended runtime atop snn-core (keeps snn-core unchanged)
//!
//! Additions:
//! - Adjacency index (source -> edges) to avoid O(E) scans
//! - Budgeted stepping API surface (hooks present; conservative defaults)
//! - Optional plasticity trait (feature "plasticity")
//!
//! This crate composes snn-core's types and reuses its event/time semantics.

pub mod runtime_plus;
#[cfg(feature = "plasticity")]
pub mod plasticity;

// Re-exports
pub use runtime_plus::{SnnRuntimePlus, StepBudgets};

#[cfg(feature = "plasticity")]
pub use plasticity::{PlasticityRule, QuantizedStdp};