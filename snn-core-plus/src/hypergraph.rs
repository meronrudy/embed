//! Hypergraph connectivity: hyperedges connect multiple sources to multiple targets.

pub struct HyperEdge {
    pub id: u32,
    pub sources: Vec<u32>,
    pub targets: Vec<u32>,
    pub weight: i32, // fixed-point
    pub delay: u64,  // ticks
}