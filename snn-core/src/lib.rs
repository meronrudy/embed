//! snn-core: Zero-dependency hypergraph-based SNN runtime (embeddable)

pub mod event_queue;
pub mod fixed;
pub mod sparse;
pub mod ir;
pub mod neuron;
pub mod hypergraph;
pub mod runtime;

// Re-exports
pub use event_queue::{SpikeEvent, TimeWheel};
pub use fixed::{Fixed, FRACTIONAL_BITS, SCALE, to_fixed, from_fixed, fixed_mul};
pub use sparse::CsrMatrix;
pub use ir::{SnnOp, Program};
pub use neuron::Neuron;
pub use hypergraph::HyperEdge;
pub use runtime::SnnRuntime;