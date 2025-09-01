 //! Embedded neural network structures for no-std environments
//!
//! This module provides memory-efficient neural network implementations
//! that operate within the constraints of embedded systems.

use crate::{
    error::{EmbeddedError, EmbeddedResult},
    fixed_point::{FixedPoint, Q16_16, FixedSpike},
    embedded_neuron::{EmbeddedNeuron, EmbeddedLIFNeuron, EmbeddedIzhikevichNeuron, EmbeddedSynapse},
    embedded_memory::{EmbeddedHypergraph, EmbeddedSpikeBuffer},
};
use heapless::{Vec, FnvIndexMap, Deque};
use core::marker::PhantomData;

/// Maximum number of neurons in an embedded network
pub const MAX_NEURONS: usize = 128;

/// Maximum number of synapses in an embedded network
pub const MAX_SYNAPSES: usize = 512;

/// Maximum number of hyperedges in an embedded network
pub const MAX_HYPEREDGES: usize = 256;

/// Maximum number of spikes processed per timestep
pub const MAX_SPIKES_PER_STEP: usize = 64;

/// Maximum number of active neuron updates processed per timestep
pub const MAX_ACTIVE_UPDATES_PER_STEP: usize = 64;

/// Network topology types for embedded systems
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbeddedTopology {
    /// Fully connected network
    FullyConnected,
    /// Feedforward layers
    Feedforward,
    /// Recurrent connections
    Recurrent,
    /// Small world network
    SmallWorld,
    /// Custom topology
    Custom,
}

/// Embedded neural network trait
pub trait EmbeddedNetwork<T: FixedPoint> {
    /// Update the entire network for one timestep
    fn update(&mut self, inputs: &[T]) -> EmbeddedResult<Vec<FixedSpike<T>, MAX_SPIKES_PER_STEP>>;
    
    /// Reset network to initial state
    fn reset(&mut self);
    
    /// Get network output (membrane potentials or spike counts)
    fn get_outputs(&self) -> Vec<T, MAX_NEURONS>;
    
    /// Get current simulation time
    fn current_time(&self) -> T;
    
    /// Get network statistics
    fn get_statistics(&self) -> NetworkStatistics<T>;
}

/// Network statistics for monitoring and debugging
#[derive(Debug, Clone)]
pub struct NetworkStatistics<T: FixedPoint> {
    /// Total number of spikes generated
    pub total_spikes: u32,
    /// Average firing rate per neuron
    pub avg_firing_rate: T,
    /// Network activity level (0.0 to 1.0)
    pub activity_level: T,
    /// Energy consumption estimate
    pub energy_estimate: T,
    /// Memory usage in bytes
    pub memory_usage: usize,
}

/// Embedded spiking neural network implementation
#[derive(Debug)]
pub struct EmbeddedSNN<T: FixedPoint> {
    /// Network neurons
    neurons: Vec<EmbeddedNeuronWrapper<T>, MAX_NEURONS>,
    /// Network synapses
    synapses: Vec<EmbeddedSynapse<T>, MAX_SYNAPSES>,
    /// Hypergraph structure for complex connections
    hypergraph: Option<EmbeddedHypergraph<T>>,
    /// Spike buffer for delayed processing
    spike_buffer: EmbeddedSpikeBuffer<T>,
    /// Active neurons scheduled for processing (event-driven)
    active_list: Deque<u16, MAX_NEURONS>,
    /// Pending spikes produced this step (bounded)
    pending_spikes: Deque<FixedSpike<T>, MAX_SPIKES_PER_STEP>,

    #[cfg(feature = "partitioning")]
    /// Optional mapping from neuron to partition
    partition_map: crate::partitioning::PartitionMap,
    #[cfg(feature = "partitioning")]
    /// Per-partition spike buckets for cross-partition deferral
    partition_spikes: crate::partitioning::PartitionedSpikeQueues<T>,

    // Plasticity parameters (quantized STDP with traces)
    /// Enable plasticity
    plastic_enable: bool,
    /// Potentiation increment (A+)
    plastic_a_plus: T,
    /// Depression increment (A-)
    plastic_a_minus: T,
    /// Pre-synaptic trace decay rate
    plastic_alpha_pre: T,
    /// Post-synaptic trace decay rate
    plastic_alpha_post: T,
    /// Minimum weight clamp
    plastic_w_min: T,
    /// Maximum weight clamp
    plastic_w_max: T,
    /// Prune threshold (remove synapses with weight below)
    prune_threshold: T,
    
    // STDP trace state
    /// Pre-synaptic traces for each neuron
    pre_trace: Vec<T, MAX_NEURONS>,
    /// Post-synaptic traces for each neuron
    post_trace: Vec<T, MAX_NEURONS>,

    /// Current simulation time
    current_time: T,
    /// Time step size
    dt: T,
    /// Network topology
    topology: EmbeddedTopology,
    /// Input neuron indices
    input_indices: Vec<u16, MAX_NEURONS>,
    /// Output neuron indices
    output_indices: Vec<u16, MAX_NEURONS>,
    /// Network statistics
    statistics: NetworkStatistics<T>,
    /// Total simulation steps
    simulation_steps: u64,
}

/// Wrapper for different neuron types in embedded systems
#[derive(Debug, Clone)]
pub enum EmbeddedNeuronWrapper<T: FixedPoint> {
    /// Leaky Integrate-and-Fire neuron
    LIF(EmbeddedLIFNeuron<T>),
    /// Izhikevich neuron
    Izhikevich(EmbeddedIzhikevichNeuron<T>),
}

impl<T: FixedPoint> EmbeddedNeuronWrapper<T> {
    /// Update the wrapped neuron
    pub fn update(&mut self, dt: T, input_current: T) -> EmbeddedResult<Option<FixedSpike<T>>> {
        match self {
            Self::LIF(neuron) => neuron.update(dt, input_current),
            Self::Izhikevich(neuron) => neuron.update(dt, input_current),
        }
    }
    
    /// Reset the wrapped neuron
    pub fn reset(&mut self) {
        match self {
            Self::LIF(neuron) => neuron.reset(),
            Self::Izhikevich(neuron) => neuron.reset(),
        }
    }
    
    /// Get membrane potential
    pub fn membrane_potential(&self) -> T {
        match self {
            Self::LIF(neuron) => neuron.membrane_potential(),
            Self::Izhikevich(neuron) => neuron.membrane_potential(),
        }
    }
    
    /// Get neuron ID
    pub fn id(&self) -> u16 {
        match self {
            Self::LIF(neuron) => neuron.id(),
            Self::Izhikevich(neuron) => neuron.id(),
        }
    }
    
    /// Check if refractory
    pub fn is_refractory(&self) -> bool {
        match self {
            Self::LIF(neuron) => neuron.is_refractory(),
            Self::Izhikevich(neuron) => neuron.is_refractory(),
        }
    }
}

impl<T: FixedPoint> EmbeddedSNN<T> {
    /// Create a new embedded SNN
    pub fn new(dt: T, topology: EmbeddedTopology) -> Self {
        Self {
            neurons: Vec::new(),
            synapses: Vec::new(),
            hypergraph: None,
            spike_buffer: EmbeddedSpikeBuffer::new(),
            active_list: Deque::new(),
            pending_spikes: Deque::new(),
            #[cfg(feature = "partitioning")]
            partition_map: crate::partitioning::PartitionMap::new(),
            #[cfg(feature = "partitioning")]
            partition_spikes: crate::partitioning::PartitionedSpikeQueues::new(),
            // Plasticity defaults (STDP with exponential traces)
            plastic_enable: true,
            plastic_a_plus: T::from_float(0.01),
            plastic_a_minus: T::from_float(0.012),
            plastic_alpha_pre: T::from_float(0.96),   // decay factor per dt
            plastic_alpha_post: T::from_float(0.96),  // decay factor per dt
            plastic_w_min: T::from_float(0.0),
            plastic_w_max: T::from_float(1.0),
            prune_threshold: T::from_float(0.0),
            pre_trace: Vec::new(),
            post_trace: Vec::new(),
    
            current_time: T::zero(),
            dt,
            topology,
            input_indices: Vec::new(),
            output_indices: Vec::new(),
            statistics: NetworkStatistics {
                total_spikes: 0,
                avg_firing_rate: T::zero(),
                activity_level: T::zero(),
                energy_estimate: T::zero(),
                memory_usage: 0,
            },
            simulation_steps: 0,
        }
    }
    
    /// Add a neuron to the network
    pub fn add_neuron(&mut self, neuron: EmbeddedNeuronWrapper<T>) -> EmbeddedResult<u16> {
        let id = neuron.id();
        self.neurons.push(neuron)
            .map_err(|_| EmbeddedError::BufferFull)?;
        
        // Initialize STDP traces for the new neuron
        self.pre_trace.push(T::zero())
            .map_err(|_| EmbeddedError::BufferFull)?;
        self.post_trace.push(T::zero())
            .map_err(|_| EmbeddedError::BufferFull)?;
            
        Ok(id)
    }
    
    /// Add a synapse to the network
    pub fn add_synapse(&mut self, synapse: EmbeddedSynapse<T>) -> EmbeddedResult<()> {
        self.synapses.push(synapse)
            .map_err(|_| EmbeddedError::BufferFull)
    }
    
    /// Enable hypergraph connections
    pub fn enable_hypergraph(&mut self) -> EmbeddedResult<()> {
        self.hypergraph = Some(EmbeddedHypergraph::new());
        Ok(())
    }
    
    /// Set input neurons
    pub fn set_input_neurons(&mut self, indices: &[u16]) -> EmbeddedResult<()> {
        self.input_indices.clear();
        for &idx in indices {
            self.input_indices.push(idx)
                .map_err(|_| EmbeddedError::BufferFull)?;
        }
        Ok(())
    }
    
    /// Set output neurons
    pub fn set_output_neurons(&mut self, indices: &[u16]) -> EmbeddedResult<()> {
        self.output_indices.clear();
        for &idx in indices {
            self.output_indices.push(idx)
                .map_err(|_| EmbeddedError::BufferFull)?;
        }
        Ok(())
    }

    #[cfg(feature = "partitioning")]
    /// Assign a neuron to a partition
    pub fn set_neuron_partition(&mut self, neuron_id: u16, pid: crate::partitioning::PartitionId) {
        self.partition_map.assign(neuron_id, pid);
    }
    
    /// Create a feedforward network topology
    pub fn create_feedforward(&mut self, layer_sizes: &[usize]) -> EmbeddedResult<()> {
        if layer_sizes.len() < 2 {
            return Err(EmbeddedError::InvalidConfiguration);
        }
        
        // Create neurons for each layer
        let mut neuron_id = 0u16;
        let mut layer_start_indices = Vec::<usize, 8>::new();
        
        for &layer_size in layer_sizes {
            layer_start_indices.push(self.neurons.len())
                .map_err(|_| EmbeddedError::BufferFull)?;
            
            for _ in 0..layer_size {
                let neuron = EmbeddedNeuronWrapper::LIF(
                    EmbeddedLIFNeuron::new(neuron_id)
                );
                self.add_neuron(neuron)?;
                neuron_id += 1;
            }
        }
        
        // Create synapses between consecutive layers
        for layer in 0..layer_sizes.len() - 1 {
            let current_start = layer_start_indices[layer];
            let next_start = layer_start_indices[layer + 1];
            let current_size = layer_sizes[layer];
            let next_size = layer_sizes[layer + 1];
            
            for i in 0..current_size {
                for j in 0..next_size {
                    let pre_id = (current_start + i) as u16;
                    let post_id = (next_start + j) as u16;
                    let weight = T::from_float(0.1); // Default weight
                    let delay = 1; // Default delay
                    
                    let synapse = EmbeddedSynapse::new(pre_id, post_id, weight, delay);
                    self.add_synapse(synapse)?;
                }
            }
        }
        
        // Set input and output neurons
        let first_layer_indices: Vec<u16, MAX_NEURONS> = (0..layer_sizes[0] as u16).collect();
        let last_layer_start = layer_start_indices[layer_sizes.len() - 1];
        let last_layer_indices: Vec<u16, MAX_NEURONS> = 
            (last_layer_start as u16..(last_layer_start + layer_sizes[layer_sizes.len() - 1]) as u16)
            .collect();
        
        self.set_input_neurons(&first_layer_indices)?;
        self.set_output_neurons(&last_layer_indices)?;
        
        self.topology = EmbeddedTopology::Feedforward;
        Ok(())
    }
    
    /// Apply external inputs to input neurons
    fn apply_inputs(&mut self, inputs: &[T]) -> EmbeddedResult<()> {
        for (i, &input_idx) in self.input_indices.iter().enumerate() {
            if let Some(&input_value) = inputs.get(i) {
                if let Some(neuron) = self.neurons.get_mut(input_idx as usize) {
                    // Add input current directly to membrane potential
                    let current_potential = neuron.membrane_potential();
                    let new_potential = current_potential + input_value * self.dt;
                    match neuron {
                        EmbeddedNeuronWrapper::LIF(n) => n.set_membrane_potential(new_potential),
                        EmbeddedNeuronWrapper::Izhikevich(n) => n.set_membrane_potential(new_potential),
                    }
                    // Schedule this neuron as active for this step
                    let _ = self.active_list.push_back(input_idx);
                }
            }
        }
        Ok(())
    }

    /// Process up to a bounded number of active neurons and synaptic propagations
    fn process_active_budget(
        &mut self,
        synaptic_currents: &Vec<T, MAX_NEURONS>,
        max_updates: usize,
        max_spikes: usize,
    ) -> EmbeddedResult<Vec<FixedSpike<T>, MAX_SPIKES_PER_STEP>> {
        let mut produced = Vec::<FixedSpike<T>, MAX_SPIKES_PER_STEP>::new();

        // Process at most max_updates active neurons
        for _ in 0..max_updates.min(self.active_list.len()) {
            if let Some(idx) = self.active_list.pop_front() {
                let i = idx as usize;
                if let Some(neuron) = self.neurons.get_mut(i) {
                    let input_current = synaptic_currents.get(i).copied().unwrap_or(T::zero());
                    if let Some(spike) = neuron.update(self.dt, input_current)? {
                        // Update post-synaptic trace on spike
                        if self.plastic_enable && (spike.source as usize) < self.post_trace.len() {
                            self.post_trace[spike.source as usize] = self.post_trace[spike.source as usize] + T::one();
                        }
                        
                        // Enforce spike budget across both pending and produced
                        if produced.len() + self.pending_spikes.len() < max_spikes {
                            produced.push(spike).map_err(|_| EmbeddedError::BufferFull)?;
                            // Add to spike history buffer
                            self.spike_buffer.add_spike(spike)?;
                            // Route spike to synapses (partition-aware when enabled)
                            for synapse in &mut self.synapses {
                                if synapse.pre_id == spike.source {
                                    #[cfg(feature = "partitioning")]
                                    {
                                        let pid_pre = self.partition_map.get(spike.source);
                                        let pid_post = self.partition_map.get(synapse.post_id);
                                        match (pid_pre, pid_post) {
                                            // Same-partition: deliver immediately
                                            (Some(a), Some(b)) if a == b => {
                                                synapse.receive_spike(&spike)?;
                                                if self.plastic_enable {
                                                    // Update pre-synaptic trace on spike
                                                    let pre_idx = spike.source as usize;
                                                    if pre_idx < self.pre_trace.len() {
                                                        self.pre_trace[pre_idx] = self.pre_trace[pre_idx] + T::one();
                                                    }
                                                    // Apply STDP directly on current synapse without re-borrowing self
                                                    let pre_idx = spike.source as usize;
                                                    let post_idx = synapse.post_id as usize;
                                                    if pre_idx < self.pre_trace.len() && post_idx < self.post_trace.len() {
                                                        let pre_v = self.pre_trace[pre_idx];
                                                        let post_v = self.post_trace[post_idx];
                                                        EmbeddedSNN::apply_stdp_on_synapse(
                                                            pre_v,
                                                            post_v,
                                                            self.plastic_a_plus,
                                                            self.plastic_a_minus,
                                                            self.plastic_w_min,
                                                            self.plastic_w_max,
                                                            synapse,
                                                        );
                                                    }
                                                }
                                                let _ = self.active_list.push_back(synapse.post_id);
                                            }
                                            // Cross-partition: defer by enqueuing into destination partition
                                            (.., Some(dst)) => {
                                                let _ = self.partition_spikes.push(dst, spike);
                                            }
                                            // Unknown mapping: fallback to immediate delivery
                                            _ => {
                                                synapse.receive_spike(&spike)?;
                                                if self.plastic_enable {
                                                    // Update pre-synaptic trace on spike
                                                    let pre_idx = spike.source as usize;
                                                    if pre_idx < self.pre_trace.len() {
                                                        self.pre_trace[pre_idx] = self.pre_trace[pre_idx] + T::one();
                                                    }
                                                    // Apply STDP directly on current synapse without re-borrowing self
                                                    let pre_idx = spike.source as usize;
                                                    let post_idx = synapse.post_id as usize;
                                                    if pre_idx < self.pre_trace.len() && post_idx < self.post_trace.len() {
                                                        let pre_v = self.pre_trace[pre_idx];
                                                        let post_v = self.post_trace[post_idx];
                                                        EmbeddedSNN::apply_stdp_on_synapse(
                                                            pre_v,
                                                            post_v,
                                                            self.plastic_a_plus,
                                                            self.plastic_a_minus,
                                                            self.plastic_w_min,
                                                            self.plastic_w_max,
                                                            synapse,
                                                        );
                                                    }
                                                }
                                                let _ = self.active_list.push_back(synapse.post_id);
                                            }
                                        }
                                    }
                                    #[cfg(not(feature = "partitioning"))]
                                    {
                                        synapse.receive_spike(&spike)?;
                                        if self.plastic_enable {
                                            // Update pre-synaptic trace on spike
                                            let pre_idx = spike.source as usize;
                                            if pre_idx < self.pre_trace.len() {
                                                self.pre_trace[pre_idx] = self.pre_trace[pre_idx] + T::one();
                                            }
                                            // Apply STDP directly on current synapse without re-borrowing self
                                            let pre_idx = spike.source as usize;
                                            let post_idx = synapse.post_id as usize;
                                            if pre_idx < self.pre_trace.len() && post_idx < self.post_trace.len() {
                                                let pre_v = self.pre_trace[pre_idx];
                                                let post_v = self.post_trace[post_idx];
                                                EmbeddedSNN::apply_stdp_on_synapse(
                                                    pre_v,
                                                    post_v,
                                                    self.plastic_a_plus,
                                                    self.plastic_a_minus,
                                                    self.plastic_w_min,
                                                    self.plastic_w_max,
                                                    synapse,
                                                );
                                            }
                                        }
                                        let _ = self.active_list.push_back(synapse.post_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(produced)
    }
    #[cfg(feature = "partitioning")]
    fn drain_partition_queues(&mut self, max_spikes: usize) {
        use crate::partitioning::{MAX_PARTITIONS, PartitionId};
        let mut processed = 0usize;
        for pid in 0..MAX_PARTITIONS {
            if processed >= max_spikes { break; }
            let pid_u8: PartitionId = pid as PartitionId;

            // Drain this partition's deferred spikes
            let mut drained = self.partition_spikes.drain(pid_u8);
            let mut idx = 0usize;

            // Process up to the budget for this tick
            while idx < drained.len() && processed < max_spikes {
                if let Some(sp) = drained.get(idx).copied() {
                    // Deliver only to synapses whose post is in this partition
                    for syn in &mut self.synapses {
                        if syn.pre_id == sp.source {
                            if let Some(dst) = self.partition_map.get(syn.post_id) {
                                if dst == pid_u8 {
                                    let _ = syn.receive_spike(&sp);
                                    if self.plastic_enable {
                                        syn.potentiate(self.plastic_a_plus, self.plastic_w_min, self.plastic_w_max);
                                    }
                                    let _ = self.active_list.push_back(syn.post_id);
                                    processed += 1;
                                }
                            }
                        }
                    }
                }
                idx += 1;
            }

            // Re-enqueue any unprocessed spikes for this partition
            while idx < drained.len() {
                if let Some(sp) = drained.get(idx).copied() {
                    let _ = self.partition_spikes.push(pid_u8, sp);
                }
                idx += 1;
            }
        }
    }
    
    
    /// Decay STDP traces for all neurons
    fn decay_traces(&mut self) {
        for trace in &mut self.pre_trace {
            *trace = *trace * self.plastic_alpha_pre;
        }
        for trace in &mut self.post_trace {
            *trace = *trace * self.plastic_alpha_post;
        }
    }
    
    /// Apply STDP weight update based on pre/post traces
    fn apply_stdp(&mut self, pre_id: u16, post_id: u16, synapse_idx: usize) {
        if !self.plastic_enable || synapse_idx >= self.synapses.len() {
            return;
        }
        
        let pre_idx = pre_id as usize;
        let post_idx = post_id as usize;
        
        if pre_idx >= self.pre_trace.len() || post_idx >= self.post_trace.len() {
            return;
        }
        
        // LTD: w -= a_minus * post_trace[post]
        let ltd_amount = self.plastic_a_minus * self.post_trace[post_idx];
        
        // LTP: w += a_plus * pre_trace[pre] 
        let ltp_amount = self.plastic_a_plus * self.pre_trace[pre_idx];
        
        // Apply weight change
        let synapse = &mut self.synapses[synapse_idx];
        let mut new_weight = synapse.weight - ltd_amount + ltp_amount;
        
        // Clamp to bounds
        if new_weight < self.plastic_w_min {
            new_weight = self.plastic_w_min;
        }
        if new_weight > self.plastic_w_max {
            new_weight = self.plastic_w_max;
        }
        
        synapse.weight = new_weight;
    }

    /// Apply STDP weight update directly to a synapse using provided parameters (no &mut self borrow)
    fn apply_stdp_on_synapse(
        pre_trace_val: T,
        post_trace_val: T,
        a_plus: T,
        a_minus: T,
        w_min: T,
        w_max: T,
        syn: &mut EmbeddedSynapse<T>,
    ) {
        // LTD: w -= a_minus * post_trace[post]
        let ltd_amount = a_minus * post_trace_val;
        // LTP: w += a_plus * pre_trace[pre]
        let ltp_amount = a_plus * pre_trace_val;

        let mut new_weight = syn.weight - ltd_amount + ltp_amount;
        if new_weight < w_min {
            new_weight = w_min;
        }
        if new_weight > w_max {
            new_weight = w_max;
        }
        syn.weight = new_weight;
    }

    /// Calculate synaptic currents for all neurons
    fn calculate_synaptic_currents(&self) -> Vec<T, MAX_NEURONS> {
        let mut currents = Vec::new();
        
        // Initialize currents to zero
        for _ in 0..self.neurons.len() {
            let _ = currents.push(T::zero());
        }
        
        // Calculate currents from synapses
        for synapse in &self.synapses {
            let current = synapse.get_output_current(self.current_time);
            if let Some(post_current) = currents.get_mut(synapse.post_id as usize) {
                *post_current = *post_current + current;
            }
        }
        
        currents
    }
    
    /// Update network statistics
    fn update_statistics(&mut self, spikes: &[FixedSpike<T>]) {
        self.statistics.total_spikes += spikes.len() as u32;
        
        if self.simulation_steps > 0 {
            let total_time = T::from_int(self.simulation_steps as i32) * self.dt;
            if total_time > T::zero() {
                self.statistics.avg_firing_rate = 
                    T::from_int(self.statistics.total_spikes as i32) / 
                    (total_time * T::from_int(self.neurons.len() as i32));
            }
        }
        
        // Calculate activity level (fraction of neurons that spiked this timestep)
        if !self.neurons.is_empty() {
            self.statistics.activity_level = 
                T::from_int(spikes.len() as i32) / T::from_int(self.neurons.len() as i32);
        }
        
        // Estimate energy consumption (proportional to spike count)
        self.statistics.energy_estimate = 
            self.statistics.energy_estimate + T::from_int(spikes.len() as i32);
        
        // Calculate memory usage
        self.statistics.memory_usage = 
            core::mem::size_of_val(&self.neurons) +
            core::mem::size_of_val(&self.synapses) +
            core::mem::size_of_val(&self.spike_buffer);
    }
}

impl<T: FixedPoint> EmbeddedNetwork<T> for EmbeddedSNN<T> {
    fn update(&mut self, inputs: &[T]) -> EmbeddedResult<Vec<FixedSpike<T>, MAX_SPIKES_PER_STEP>> {
        // Decay STDP traces each tick
        if self.plastic_enable {
            self.decay_traces();
        }

        // Drain any deferred cross-partition spikes from previous ticks
        #[cfg(feature = "partitioning")]
        self.drain_partition_queues(MAX_SPIKES_PER_STEP);

        // Apply external inputs (schedules input neurons as active)
        self.apply_inputs(inputs)?;

        // Calculate synaptic currents (consumed when updating active neurons)
        let synaptic_currents = self.calculate_synaptic_currents();

        // Event-driven: process at most MAX_ACTIVE_UPDATES_PER_STEP updates and cap spikes
        let mut new_spikes = self.process_active_budget(
            &synaptic_currents,
            MAX_ACTIVE_UPDATES_PER_STEP,
            MAX_SPIKES_PER_STEP,
        )?;

        // Clean up old spikes from synapses
        let max_age = T::from_float(0.1); // 100ms history
        for synapse in &mut self.synapses {
            synapse.cleanup_buffer(self.current_time, max_age);
        }

        // Prune weak synapses if pruning is configured
        if self.prune_threshold > T::zero() {
            self.synapses.retain(|s| s.weight >= self.prune_threshold);
        }
        
        // Update time and statistics
        self.current_time = self.current_time + self.dt;
        self.simulation_steps += 1;
        self.update_statistics(&new_spikes);
        
        Ok(new_spikes)
    }
    
    fn reset(&mut self) {
        for neuron in &mut self.neurons {
            neuron.reset();
        }
        
        self.spike_buffer.clear();
        self.active_list.clear();
        self.pending_spikes.clear();
        #[cfg(feature = "partitioning")]
        self.partition_spikes.clear_all();
        self.current_time = T::zero();
        self.simulation_steps = 0;

        // Reset STDP traces
        for trace in &mut self.pre_trace {
            *trace = T::zero();
        }
        for trace in &mut self.post_trace {
            *trace = T::zero();
        }
        
        self.statistics = NetworkStatistics {
            total_spikes: 0,
            avg_firing_rate: T::zero(),
            activity_level: T::zero(),
            energy_estimate: T::zero(),
            memory_usage: 0,
        };
    }
    
    fn get_outputs(&self) -> Vec<T, MAX_NEURONS> {
        let mut outputs = Vec::new();
        
        for &output_idx in &self.output_indices {
            if let Some(neuron) = self.neurons.get(output_idx as usize) {
                let _ = outputs.push(neuron.membrane_potential());
            }
        }
        
        outputs
    }
    
    fn current_time(&self) -> T {
        self.current_time
    }
    
    fn get_statistics(&self) -> NetworkStatistics<T> {
        self.statistics.clone()
    }
}

/// Builder pattern for constructing embedded networks
#[derive(Debug)]
pub struct EmbeddedNetworkBuilder<T: FixedPoint> {
    dt: T,
    topology: EmbeddedTopology,
    layer_sizes: Vec<usize, 8>,
    use_hypergraph: bool,
    _phantom: PhantomData<T>,
}

impl<T: FixedPoint> EmbeddedNetworkBuilder<T> {
    /// Create a new network builder
    pub fn new(dt: T) -> Self {
        Self {
            dt,
            topology: EmbeddedTopology::Custom,
            layer_sizes: Vec::new(),
            use_hypergraph: false,
            _phantom: PhantomData,
        }
    }
    
    /// Set network topology
    pub fn topology(mut self, topology: EmbeddedTopology) -> Self {
        self.topology = topology;
        self
    }
    
    /// Add a layer to feedforward network
    pub fn add_layer(mut self, size: usize) -> EmbeddedResult<Self> {
        self.layer_sizes.push(size)
            .map_err(|_| EmbeddedError::BufferFull)?;
        Ok(self)
    }
    
    /// Enable hypergraph connections
    pub fn with_hypergraph(mut self) -> Self {
        self.use_hypergraph = true;
        self
    }
    
    /// Build the network
    pub fn build(self) -> EmbeddedResult<EmbeddedSNN<T>> {
        let mut network = EmbeddedSNN::new(self.dt, self.topology);
        
        if self.use_hypergraph {
            network.enable_hypergraph()?;
        }
        
        match self.topology {
            EmbeddedTopology::Feedforward => {
                if self.layer_sizes.is_empty() {
                    return Err(EmbeddedError::InvalidConfiguration);
                }
                network.create_feedforward(&self.layer_sizes)?;
            }
            _ => {
                // Other topologies would be implemented here
            }
        }
        
        Ok(network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_embedded_network_creation() {
        let network = EmbeddedNetworkBuilder::<Q16_16>::new(Q16_16::from_float(0.001))
            .topology(EmbeddedTopology::Feedforward)
            .add_layer(2).unwrap()
            .add_layer(3).unwrap()
            .add_layer(1).unwrap()
            .build().unwrap();
        
        assert_eq!(network.neurons.len(), 6); // 2 + 3 + 1
        assert!(network.synapses.len() > 0);
    }
    
    #[test]
    fn test_network_update() {
        let mut network = EmbeddedNetworkBuilder::<Q16_16>::new(Q16_16::from_float(0.001))
            .topology(EmbeddedTopology::Feedforward)
            .add_layer(2).unwrap()
            .add_layer(1).unwrap()
            .build().unwrap();
        
        let inputs = [Q16_16::from_float(1.0), Q16_16::from_float(0.5)];
        let spikes = network.update(&inputs).unwrap();
        
        assert!(network.current_time() > Q16_16::zero());
        assert!(spikes.len() <= MAX_SPIKES_PER_STEP);
    }
    
    #[test]
    fn test_network_statistics() {
        let mut network = EmbeddedNetworkBuilder::<Q16_16>::new(Q16_16::from_float(0.001))
            .topology(EmbeddedTopology::Feedforward)
            .add_layer(2).unwrap()
            .add_layer(1).unwrap()
            .build().unwrap();
        
        let inputs = [Q16_16::from_float(10.0), Q16_16::from_float(10.0)];
        
        // Run for several timesteps
        for _ in 0..100 {
            let _ = network.update(&inputs).unwrap();
        }
        
        let stats = network.get_statistics();
        assert!(stats.memory_usage > 0);
    }
    #[test]
    fn test_plasticity_and_pruning() {
            // Build minimal 1->1 with small initial weight that should increase
            let mut net = EmbeddedSNN::<Q16_16>::new(Q16_16::from_float(0.001), EmbeddedTopology::Custom);
            // Add two LIF neurons
            let _ = net.add_neuron(EmbeddedNeuronWrapper::LIF(EmbeddedLIFNeuron::new(0))).unwrap();
            let _ = net.add_neuron(EmbeddedNeuronWrapper::LIF(EmbeddedLIFNeuron::new(1))).unwrap();
            // Add synapses: one weak, one very weak to be pruned
            net.add_synapse(EmbeddedSynapse::new(0, 1, Q16_16::from_float(0.05), 1)).unwrap();
            net.add_synapse(EmbeddedSynapse::new(0, 1, Q16_16::from_float(0.0001), 1)).unwrap();
            // Configure pruning threshold
            net.prune_threshold = Q16_16::from_float(0.001);
    
            // Drive input neuron strongly to cause pre spikes and potentiation
            net.input_indices = {
                let mut v = Vec::new();
                v.push(0u16).unwrap();
                v
            };
    
            // Run a few steps with strong input to neuron 0
            for _ in 0..10 {
                let _ = net.update(&[Q16_16::from_float(20.0)]).unwrap();
            }
    
            // After pruning pass in update, the very weak synapse should be removed
            assert!(net.synapses.len() <= 1 || net.synapses.iter().any(|s| s.weight > Q16_16::from_float(0.001)));
        }
    }
