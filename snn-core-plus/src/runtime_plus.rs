//! Extended runtime that composes snn-core and adds:
//! - source->edges adjacency index (avoids O(E) scans)
//! - optional per-tick processing budgets
//! - optional plasticity hooks (behind the "plasticity" feature)
//!
//! Semantics:
//! - step_once() returns "spikes at current tick" (the events popped from the wheel),
//!   while scheduling any newly generated spikes for future ticks.

use snn_core::{HyperEdge, Neuron, SpikeEvent, TimeWheel};
use snn_core::SnnRuntime; // reuse inner data and time semantics

#[derive(Clone, Copy, Debug, Default)]
pub struct StepBudgets {
    pub max_edge_visits: Option<usize>,
    pub max_spikes_scheduled: Option<usize>,
}

pub struct SnnRuntimePlus {
    pub inner: SnnRuntime,
    // Adjacency: for each source neuron id -> list of edge ids originating from it
    source_to_edges: Vec<Vec<u32>>,

    #[cfg(feature = "plasticity")]
    plasticity: Option<Box<dyn crate::plasticity::PlasticityRule>>,
}

impl SnnRuntimePlus {
    pub fn new(wheel_size: u64) -> Self {
        Self {
            inner: SnnRuntime::new(wheel_size),
            source_to_edges: Vec::new(),
            #[cfg(feature = "plasticity")]
            plasticity: None,
        }
    }

    pub fn from_inner(inner: SnnRuntime) -> Self {
        let mut me = Self {
            inner,
            source_to_edges: Vec::new(),
            #[cfg(feature = "plasticity")]
            plasticity: None,
        };
        // Build adjacency from existing edges if any
        me.rebuild_adjacency();
        me
    }

    fn ensure_neuron_capacity(&mut self, id: u32) {
        let len_needed = (id as usize) + 1;
        if self.source_to_edges.len() < len_needed {
            self.source_to_edges.resize_with(len_needed, || Vec::new());
        }
    }

    fn rebuild_adjacency(&mut self) {
        // Build into a temporary to avoid aliasing & self borrows
        let mut adj: Vec<Vec<u32>> = vec![Vec::new(); self.inner.neurons.len()];
        for edge in &self.inner.edges {
            for &s in &edge.sources {
                let idx = s as usize;
                if idx < adj.len() {
                    adj[idx].push(edge.id);
                }
            }
        }
        self.source_to_edges = adj;
    }

    pub fn add_neuron(&mut self, threshold: f32) -> u32 {
        let id = self.inner.add_neuron(threshold);
        self.ensure_neuron_capacity(id);
        id
    }

    pub fn add_edge(&mut self, sources: Vec<u32>, targets: Vec<u32>, weight: f32, delay: u64) {
        // Edge id equals index in snn-core (by construction)
        let next_id = self.inner.edges.len() as u32;
        self.inner.add_edge(sources.clone(), targets, weight, delay);
        // Update adjacency
        for s in sources {
            self.ensure_neuron_capacity(s);
            self.source_to_edges[s as usize].push(next_id);
        }
    }

    pub fn queue(&mut self) -> &mut TimeWheel {
        &mut self.inner.queue
    }

    pub fn neurons(&self) -> &Vec<Neuron> {
        &self.inner.neurons
    }

    pub fn neurons_mut(&mut self) -> &mut Vec<Neuron> {
        &mut self.inner.neurons
    }

    pub fn edges(&self) -> &Vec<HyperEdge> {
        &self.inner.edges
    }

    pub fn edges_mut(&mut self) -> &mut Vec<HyperEdge> {
        &mut self.inner.edges
    }

    #[cfg(feature = "plasticity")]
    pub fn set_plasticity<R: crate::plasticity::PlasticityRule + 'static>(&mut self, rule: R) {
        self.plasticity = Some(Box::new(rule));
    }

    /// Advance one tick with optional processing budgets.
    /// Returns the spikes that occurred at the current tick (the popped events).
    pub fn step_once_with_budgets(&mut self, budgets: StepBudgets) -> Vec<SpikeEvent> {
        let mut edge_visits: usize = 0;
        let mut spikes_scheduled: usize = 0;

        #[cfg(feature = "plasticity")]
        if let Some(p) = self.plasticity.as_mut() {
            p.decay();
        }

        // Pop current slot events (these are the spikes at current time)
        let events = self.inner.queue.next();

        // Deliver effects and schedule newly fired spikes for their delivery times
        'events_loop: for ev in &events {
            let src = ev.neuron_id as usize;

            #[cfg(feature = "plasticity")]
            if let Some(p) = self.plasticity.as_mut() {
                p.on_pre_spike(ev.neuron_id, ev.time);
            }

            let maybe_edges = self.source_to_edges.get(src);
            if maybe_edges.is_none() {
                continue;
            }
            let edge_ids = maybe_edges.unwrap();

            for &eid in edge_ids {
                // Budget: edge visits
                if let Some(max_visits) = budgets.max_edge_visits {
                    if edge_visits >= max_visits {
                        break 'events_loop;
                    }
                }
                edge_visits += 1;

                // In snn-core, id == index
                if let Some(edge) = self.inner.edges.get(eid as usize) {
                    let deliver_time = ev.time.saturating_add(edge.delay);

                    for &tgt in &edge.targets {
                        if let Some(n) = self.inner.neurons.get_mut(tgt as usize) {
                            let fired = n.inject(edge.weight, deliver_time);
                            if fired {
                                let spike = SpikeEvent { neuron_id: tgt, time: deliver_time };

                                // Budget: scheduled spikes
                                if let Some(max_spikes) = budgets.max_spikes_scheduled {
                                    if spikes_scheduled >= max_spikes {
                                        // Do not schedule further spikes this tick
                                        break 'events_loop;
                                    }
                                }
                                self.inner.queue.schedule(spike);
                                spikes_scheduled += 1;

                                #[cfg(feature = "plasticity")]
                                if let Some(p) = self.plasticity.as_mut() {
                                    p.on_post_spike(tgt, deliver_time);
                                    // Allow rule to update weight
                                    // SAFETY: we have &mut self, then immediate second borrow by index
                                    if let Some(edge_mut) = self.inner.edges.get_mut(eid as usize) {
                                        p.apply_edge(ev.neuron_id, tgt, &mut edge_mut.weight);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        events
    }

    /// Advance one tick without budgets; returns the spikes that occurred this tick.
    pub fn step_once(&mut self) -> Vec<SpikeEvent> {
        self.step_once_with_budgets(StepBudgets::default())
    }

    /// Run until the given tick (inclusive), ignoring budgets.
    pub fn run_until(&mut self, until: u64) {
        while self.inner.queue.current_time <= until {
            let _ = self.step_once();
        }
    }

    /// Run a fixed number of ticks, ignoring budgets.
    pub fn run_ticks(&mut self, ticks: u64) {
        let until = self.inner.queue.current_time.saturating_add(ticks);
        self.run_until(until);
    }
}