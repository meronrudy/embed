#![cfg_attr(not(feature = "std"), no_std)]
//! snn-core-plus: Extended runtime atop snn-core (keeps snn-core unchanged)
//!
//! Additions:
//! - Adjacency index (source -> edges) to avoid O(E) scans
//! - Budgeted stepping API surface (hooks present; conservative defaults)
//! - Optional plasticity trait (feature "plasticity")
//! - Optional embedded/no_std modules behind feature "embedded"
//!
//! This crate composes snn-core's types and reuses its event/time semantics.

#[cfg(feature = "std")]
pub mod runtime_plus;
#[cfg(feature = "plasticity")]
pub mod plasticity;

// Embedded/no_std modules (only compiled when feature = "embedded")
#[cfg(feature = "embedded")]
pub mod fixed_point;
#[cfg(feature = "embedded")]
pub mod embedded_neuron;
#[cfg(feature = "embedded")]
pub mod embedded_memory;
#[cfg(feature = "embedded")]
pub mod embedded_network;
#[cfg(feature = "embedded")]
pub mod rtic_support;

// Error module (no_std friendly)
pub mod error;

// Re-exports
pub use error::{EmbeddedError, EmbeddedResult};
#[cfg(feature = "std")]
pub use runtime_plus::{SnnRuntimePlus, StepBudgets};

#[cfg(feature = "plasticity")]
pub use plasticity::{PlasticityRule, QuantizedStdp};