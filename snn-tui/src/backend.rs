// Backend abstraction for the TUI so we can swap different SNN engines.

use snn_core::SpikeEvent;
use snn_core_plus::{SnnRuntimePlus, StepBudgets};

/// Common interface for any SNN backend that can drive the TUI.
pub trait SnnBackend {
    /// Advance the simulation by one tick and return all spikes emitted during that tick.
    fn step(&mut self) -> Vec<SpikeEvent>;
    /// Number of neurons in the model (rows in the raster).
    fn neurons(&self) -> usize;

    /// Configure per-tick processing budgets (None = unbounded). Default no-op for backends that ignore budgets.
    fn set_budgets(&mut self, _budgets: Option<StepBudgets>) {}

    /// Query current budgets (None if unbounded). Default None for backends that ignore budgets.
    fn get_budgets(&self) -> Option<StepBudgets> { None }

    /// Optional plasticity controls (feature-gated); default no-ops/reports disabled.
    #[cfg(feature = "plasticity")]
    fn enable_default_plasticity(&mut self) {}

    #[cfg(feature = "plasticity")]
    fn plasticity_enabled(&self) -> bool { false }
}

/// Implementation backed by snn-core-plus (adjacency + optional budgets/plasticity).
pub struct CoreBackend {
    runtime: SnnRuntimePlus,
    budgets: Option<StepBudgets>,
    #[cfg(feature = "plasticity")]
    plast_on: bool,
}

impl CoreBackend {
    pub fn new() -> Self {
        // Simple 3-neuron demo network
        let mut rt = SnnRuntimePlus::new(32);
        let n0 = rt.add_neuron(1.0);
        let n1 = rt.add_neuron(1.0);
        let n2 = rt.add_neuron(1.0);
        rt.add_edge(vec![n0], vec![n1, n2], 1.0, 1);

        // Seed an initial spike at time 0
        rt.queue().schedule(SpikeEvent { neuron_id: n0, time: 0 });

        Self {
            runtime: rt,
            budgets: None,
            #[cfg(feature = "plasticity")]
            plast_on: false,
        }
    }

    /// Configure per-tick processing budgets (None = unbounded)
    pub fn set_budgets(&mut self, budgets: Option<StepBudgets>) {
        self.budgets = budgets;
    }

    /// Access the inner time wheel (for advanced seeding/scheduling)
    pub fn queue_mut(&mut self) -> &mut snn_core::TimeWheel {
        self.runtime.queue()
    }

    #[cfg(feature = "plasticity")]
    /// Enable default plasticity (Quantized STDP)
    pub fn enable_default_plasticity(&mut self) {
        self.runtime.set_plasticity(snn_core_plus::QuantizedStdp::with_defaults());
    }
}

impl SnnBackend for CoreBackend {
    fn step(&mut self) -> Vec<SpikeEvent> {
        match self.budgets {
            Some(b) => self.runtime.step_once_with_budgets(b),
            None => self.runtime.step_once(),
        }
    }

    fn neurons(&self) -> usize {
        self.runtime.neurons().len()
    }

    fn set_budgets(&mut self, budgets: Option<StepBudgets>) {
        self.budgets = budgets;
    }

    fn get_budgets(&self) -> Option<StepBudgets> {
        self.budgets
    }

    #[cfg(feature = "plasticity")]
    fn enable_default_plasticity(&mut self) {
        if !self.plast_on {
            self.runtime.set_plasticity(snn_core_plus::QuantizedStdp::with_defaults());
            self.plast_on = true;
        }
    }

    #[cfg(feature = "plasticity")]
    fn plasticity_enabled(&self) -> bool {
        self.plast_on
    }
}