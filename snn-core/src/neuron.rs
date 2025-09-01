//! Spiking neuron with fixed-point membrane potential

use crate::fixed::{Fixed, to_fixed};

pub struct Neuron {
    pub id: u32,
    pub membrane: Fixed,
    pub threshold: Fixed,
    pub refractory_until: u64,
}

impl Neuron {
    pub fn new(id: u32, threshold: f32) -> Self {
        Self {
            id,
            membrane: 0,
            threshold: to_fixed(threshold),
            refractory_until: 0,
        }
    }

    /// Inject input (fixed-point) at a given time. Returns true if neuron fires.
    /// On fire, membrane resets to 0. Caller is responsible for scheduling the spike event.
    pub fn inject(&mut self, input: Fixed, time: u64) -> bool {
        if time < self.refractory_until {
            return false;
        }
        // Simple integrate-and-fire without leak
        self.membrane = self.membrane.saturating_add(input);
        if self.membrane >= self.threshold {
            self.membrane = 0;
            // Optional: set refractory
            // self.refractory_until = time + 1;
            return true;
        }
        false
    }
}