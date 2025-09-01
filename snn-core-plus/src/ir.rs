//! Minimal internal IR for describing neurons, synapses, and spikes.

#[derive(Debug, Clone)]
pub enum SnnOp {
    NeuronUpdate { id: u32 },
    SynapseUpdate { src: u32, dst: u32, weight: i32 },
    SpikeEmit { id: u32, time: u64 },
}

/// A simple linear "program" for potential future codegen/optimizations.
pub struct Program {
    pub ops: Vec<SnnOp>,
}

impl Program {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn push(&mut self, op: SnnOp) {
        self.ops.push(op);
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }
}