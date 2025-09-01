// Backend abstraction for the TUI so we can swap different SNN engines.

use snn_core::{SnnRuntime, SpikeEvent};

/// Common interface for any SNN backend that can drive the TUI.
pub trait SnnBackend {
    /// Advance the simulation by one tick and return all spikes emitted during that tick.
    fn step(&mut self) -> Vec<SpikeEvent>;
    /// Number of neurons in the model (rows in the raster).
    fn neurons(&self) -> usize;
}

/// Implementation backed by the zero-dependency snn-core crate.
pub struct CoreBackend {
    runtime: SnnRuntime,
}

impl CoreBackend {
    pub fn new() -> Self {
        // Simple 3-neuron demo network
        let mut rt = SnnRuntime::new(32);
        let n0 = rt.add_neuron(1.0);
        let n1 = rt.add_neuron(1.0);
        let n2 = rt.add_neuron(1.0);
        rt.add_edge(vec![n0], vec![n1, n2], 1.0, 1);

        // Seed an initial spike at time 0
        rt.queue.schedule(SpikeEvent { neuron_id: n0, time: 0 });

        Self { runtime: rt }
    }
}

impl SnnBackend for CoreBackend {
    fn step(&mut self) -> Vec<SpikeEvent> {
        self.runtime.step_once()
    }

    fn neurons(&self) -> usize {
        self.runtime.neurons.len()
    }
}