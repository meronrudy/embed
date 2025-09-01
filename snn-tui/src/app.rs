// Application state for the TUI, including a circular 2D spike raster.

use snn_core::SpikeEvent;
use crate::backend::SnnBackend;

pub struct App<B: SnnBackend> {
    pub backend: B,
    pub tick: u64,
    pub width: usize,             // number of columns (time window)
    pub raster: Vec<Vec<char>>,   // [neuron][col]
    pub running: bool,
}

impl<B: SnnBackend> App<B> {
    pub fn new(backend: B, width: usize) -> Self {
        let n = backend.neurons();
        Self {
            backend,
            tick: 0,
            width,
            raster: vec![vec![' '; width]; n],
            running: false,
        }
    }

    pub fn toggle_running(&mut self) {
        self.running = !self.running;
    }

    /// Advance simulation by one tick and update the raster for the current column.
    pub fn step(&mut self) {
        // Step the backend; contract: returns spikes for "this" tick
        let spikes: Vec<SpikeEvent> = self.backend.step();

        // Advance the wall-clock tick for the UI
        self.tick = self.tick.saturating_add(1);

        // Compute which column to write into (circular buffer)
        let col = (self.tick as usize) % self.width;

        // Clear the column
        for row in 0..self.raster.len() {
            self.raster[row][col] = ' ';
        }

        // Mark spikes in this column
        for sp in spikes {
            let row = sp.neuron_id as usize;
            if row < self.raster.len() {
                self.raster[row][col] = '•';
            }
        }
    }
}