//! SNN runtime: manages neurons, hyperedges, and event-driven execution via time wheel.

use crate::{Neuron, HyperEdge, SpikeEvent, TimeWheel, to_fixed};

pub struct SnnRuntime {
    pub neurons: Vec<Neuron>,
    pub edges: Vec<HyperEdge>,
    pub queue: TimeWheel,
}

impl SnnRuntime {
    pub fn new(wheel_size: u64) -> Self {
        Self {
            neurons: Vec::new(),
            edges: Vec::new(),
            queue: TimeWheel::new(wheel_size),
        }
    }

    pub fn add_neuron(&mut self, threshold: f32) -> u32 {
        let id = self.neurons.len() as u32;
        self.neurons.push(Neuron::new(id, threshold));
        id
    }

    pub fn add_edge(&mut self, sources: Vec<u32>, targets: Vec<u32>, weight: f32, delay: u64) {
        let id = self.edges.len() as u32;
        self.edges.push(HyperEdge {
            id,
            sources,
            targets,
            weight: to_fixed(weight),
            delay,
        });
    }

    /// Advance the simulation by one tick (consumes the current slot of the time wheel)
    /// and returns the spikes that occurred in this tick (the events popped from the wheel).
    /// Newly generated spikes are scheduled for future ticks but not returned here, so the
    /// caller can treat the return value as "spikes at current time".
    pub fn step_once(&mut self) -> Vec<SpikeEvent> {
        let events = self.queue.next(); // advances current_time internally

        // Deliver effects of spikes from this tick, scheduling any resulting spikes
        // at their (possibly future) delivery time.
        for ev in &events {
            // Deliver along any hyperedge that includes this source neuron
            for edge in &self.edges {
                // Naive scan for now (zero-deps). Could index by source->edges later.
                if !edge.sources.iter().any(|&s| s == ev.neuron_id) {
                    continue;
                }

                let deliver_time = ev.time.saturating_add(edge.delay);

                for &tgt in &edge.targets {
                    if let Some(n) = self.neurons.get_mut(tgt as usize) {
                        let fired = n.inject(edge.weight, deliver_time);
                        if fired {
                            let spike = SpikeEvent { neuron_id: tgt, time: deliver_time };
                            // schedule the spike event at its time (may be current or future slot)
                            self.queue.schedule(spike);
                        }
                    }
                }
            }
        }

        // Return the spikes that occurred at this tick.
        events
    }

    /// Run until the given tick (inclusive). Does not return emitted spikes.
    pub fn run_until(&mut self, until: u64) {
        while self.queue.current_time <= until {
            let _ = self.step_once();
        }
    }

    /// Convenience: run a fixed number of ticks.
    pub fn run_ticks(&mut self, ticks: u64) {
        let until = self.queue.current_time.saturating_add(ticks);
        self.run_until(until);
    }
}