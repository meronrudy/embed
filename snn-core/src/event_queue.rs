//! Time wheel (calendar queue) and spike event

#[derive(Clone, Copy, Debug)]
pub struct SpikeEvent {
    pub neuron_id: u32,
    pub time: u64,
}

pub struct TimeWheel {
    buckets: Vec<Vec<SpikeEvent>>,
    pub current_time: u64,
    wheel_size: u64,
}

impl TimeWheel {
    pub fn new(wheel_size: u64) -> Self {
        let mut buckets = Vec::with_capacity(wheel_size as usize);
        for _ in 0..wheel_size {
            buckets.push(Vec::new());
        }
        Self {
            buckets,
            current_time: 0,
            wheel_size,
        }
    }

    #[inline]
    pub fn schedule(&mut self, event: SpikeEvent) {
        let slot = (event.time % self.wheel_size) as usize;
        self.buckets[slot].push(event);
    }

    /// Return all events scheduled at the current time slot, then advance time by 1 tick.
    pub fn next(&mut self) -> Vec<SpikeEvent> {
        let slot = (self.current_time % self.wheel_size) as usize;
        let events = core::mem::take(&mut self.buckets[slot]);
        self.current_time = self.current_time.saturating_add(1);
        events
    }
}