//! Partitioning utilities for embedded event-driven scheduling
//!
//! Provides:
//! - PartitionId: small integer partition identifiers
//! - PartitionMap: neuron_id -> partition mapping
//! - PartitionedSpikeQueues: per-partition fixed-capacity spike buckets
//!
//! This module is gated by the `partitioning` feature.

use crate::fixed_point::{FixedPoint, FixedSpike};
use heapless::{Deque, FnvIndexMap, Vec};

/// Maximum number of partitions supported.
pub const MAX_PARTITIONS: usize = 8;

/// Maximum spikes stored per partition per step (bounded, drop-oldest policy).
pub const PER_PARTITION_SPIKES: usize = 64;

/// Capacity for the neuron->partition map (upper bound on distinct neuron IDs).
const MAX_PARTITION_MAP: usize = 256;

/// Small integer type for partition identifiers
pub type PartitionId = u8;

/// Mapping from neuron ID to partition ID with fixed capacity.
#[derive(Debug, Clone)]
pub struct PartitionMap {
    map: FnvIndexMap<u16, PartitionId, MAX_PARTITION_MAP>,
}

impl PartitionMap {
    /// Create a new empty PartitionMap
    pub fn new() -> Self {
        Self { map: FnvIndexMap::new() }
    }

    /// Assign a neuron to a partition. Overwrites if previously assigned.
    ///
    /// Returns true on success, false if capacity is full and insertion failed.
    pub fn assign(&mut self, neuron_id: u16, pid: PartitionId) -> bool {
        // If key exists, update in place
        if let Some(slot) = self.map.get_mut(&neuron_id) {
            *slot = pid;
            return true;
        }
        self.map.insert(neuron_id, pid).is_ok()
    }

    /// Get the partition for the given neuron, if assigned.
    pub fn get(&self, neuron_id: u16) -> Option<PartitionId> {
        self.map.get(&neuron_id).copied()
    }

    /// Clear all assignments.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Number of assigned entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Fixed-capacity per-partition spike queues.
/// Implements drop-oldest on overflow to preserve most recent activity.
#[derive(Debug)]
pub struct PartitionedSpikeQueues<T: FixedPoint> {
    buckets: [Deque<FixedSpike<T>, PER_PARTITION_SPIKES>; MAX_PARTITIONS],
}

impl<T: FixedPoint> PartitionedSpikeQueues<T> {
    /// Create queues with all partitions initialized empty.
    pub fn new() -> Self {
        let buckets = core::array::from_fn(|_| Deque::new());
        Self { buckets }
    }

    /// Push a spike into the queue for the given partition.
    /// Returns true if enqueued (possibly after dropping oldest), false if pid is out of range.
    pub fn push(&mut self, pid: PartitionId, spike: FixedSpike<T>) -> bool {
        let idx = pid as usize;
        if idx >= MAX_PARTITIONS {
            return false;
        }
        let q = &mut self.buckets[idx];
        if q.push_back(spike).is_err() {
            // Drop oldest to make room
            let _ = q.pop_front();
            // Retry (should succeed now)
            let _ = q.push_back(spike);
        }
        true
    }

    /// Drain all spikes from a partition into a bounded Vec and clear the bucket.
    pub fn drain(&mut self, pid: PartitionId) -> Vec<FixedSpike<T>, PER_PARTITION_SPIKES> {
        let idx = pid as usize;
        let mut out = Vec::new();
        if idx >= MAX_PARTITIONS {
            return out;
        }
        let q = &mut self.buckets[idx];
        while let Some(sp) = q.pop_front() {
            let _ = out.push(sp);
        }
        out
    }

    /// Length of a partition bucket.
    pub fn len(&self, pid: PartitionId) -> usize {
        let idx = pid as usize;
        if idx >= MAX_PARTITIONS {
            return 0;
        }
        self.buckets[idx].len()
    }

    /// True if a partition bucket is empty.
    pub fn is_empty(&self, pid: PartitionId) -> bool {
        self.len(pid) == 0
    }

    /// Clear all buckets.
    pub fn clear_all(&mut self) {
        for b in &mut self.buckets {
            while b.pop_front().is_some() {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed_point::Q16_16;

    #[test]
    fn test_partition_map_assign_and_get() {
        let mut pm = PartitionMap::new();
        assert!(pm.assign(10, 2));
        assert_eq!(pm.get(10), Some(2));
        assert!(pm.assign(10, 3));
        assert_eq!(pm.get(10), Some(3));
        assert_eq!(pm.len(), 1);
    }

    #[test]
    fn test_partitioned_spike_queues_push_and_drain() {
        let mut qs = PartitionedSpikeQueues::<Q16_16>::new();
        let pid: PartitionId = 1;
        assert!(qs.is_empty(pid));
        for i in 0..(PER_PARTITION_SPIKES as u16 + 5) {
            let sp = FixedSpike::new(i, Q16_16::from_float(i as f32), Q16_16::one());
            assert!(qs.push(pid, sp));
        }
        // Overflow handled with drop-oldest; length is bounded
        assert_eq!(qs.len(pid), PER_PARTITION_SPIKES);
        let drained = qs.drain(pid);
        assert!(drained.len() <= PER_PARTITION_SPIKES);
        assert!(qs.is_empty(pid));
    }
}