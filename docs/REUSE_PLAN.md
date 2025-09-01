# Code Reuse and Improvement Plan
Analysis of archived-shnn-embedded for features to reuse or adapt in snn-core

Summary
- archived-shnn-embedded provides mature, embedded-focused building blocks: fixed-point trait and types, neuron model abstractions (LIF, Izhikevich), synapse with delayed spike buffering, hypergraph adjacency, event-driven bounded processing, simple STDP plasticity with traces, partitioning for scalable scheduling, statistics, and builder patterns.
- snn-core can reuse/adapt several of these to harden architecture without adding dependencies and while keeping the core minimal. This plan outlines what to port, minimal surface changes, and phased integration.

Cross-project anchors
- Current snn-core components:
  - Time wheel queue: [snn-core/src/event_queue.rs](snn-core/src/event_queue.rs)
  - Fixed Q16.16 helpers: [snn-core/src/fixed.rs](snn-core/src/fixed.rs)
  - Hyperedge struct: [snn-core/src/hypergraph.rs](snn-core/src/hypergraph.rs)
  - Neuron struct (simple I&F): [snn-core/src/neuron.rs](snn-core/src/neuron.rs)
  - Runtime loop: [snn-core/src/runtime.rs](snn-core/src/runtime.rs)
- Key archived modules to mine:
  - Fixed-point trait and Q16.16 impl: [Rust.trait FixedPoint](archived-shnn-embedded/src/fixed_point.rs:14), [Rust.struct Q16_16](archived-shnn-embedded/src/fixed_point.rs:71)
  - Embedded neuron traits and models: [Rust.trait EmbeddedNeuron](archived-shnn-embedded/src/embedded_neuron.rs:20), [Rust.struct EmbeddedLIFNeuron](archived-shnn-embedded/src/embedded_neuron.rs:42), [Rust.struct EmbeddedIzhikevichNeuron](archived-shnn-embedded/src/embedded_neuron.rs:170)
  - Synapse with delayed buffer: [Rust.struct EmbeddedSynapse](archived-shnn-embedded/src/embedded_neuron.rs:304)
  - Hypergraph adjacency and weight/activation management: [Rust.struct EmbeddedHypergraph](archived-shnn-embedded/src/embedded_memory.rs:27)
  - Spike ring buffer: [Rust.struct EmbeddedSpikeBuffer](archived-shnn-embedded/src/embedded_memory.rs:201)
  - Partitioning utilities: [archived-shnn-embedded/src/partitioning.rs](archived-shnn-embedded/src/partitioning.rs)
  - Event-driven bounded processing + STDP: [Rust.fn EmbeddedSNN::process_active_budget()](archived-shnn-embedded/src/embedded_network.rs:365), [Rust.fn EmbeddedSNN::apply_stdp()](archived-shnn-embedded/src/embedded_network.rs:559)
  - Network builder pattern: [Rust.struct EmbeddedNetworkBuilder](archived-shnn-embedded/src/embedded_network.rs:746)

Recommended reuse and adaptations

1) Fixed-point Interface (unify and feature-gate)
- What to reuse:
  - The FixedPoint trait abstraction with saturating/checked ops and basic math helpers: [Rust.trait FixedPoint](archived-shnn-embedded/src/fixed_point.rs:14). This provides a concrete, testable numeric contract instead of raw i32.
  - The Q16_16 type and conversions: [Rust.struct Q16_16](archived-shnn-embedded/src/fixed_point.rs:71).
- How to integrate:
  - Add a numeric abstraction behind a cargo feature in snn-core:
    - Keep existing minimal helpers for default builds (float or i32 path in [snn-core/src/fixed.rs](snn-core/src/fixed.rs)).
    - Introduce an optional “fixed-point” feature exporting a FixedPoint-like trait and Q16_16 newtype (ported directly, minus non-core funcs like sigmoid/ln if preferred).
  - Benefits:
    - Deterministic arithmetic semantics standardized across core and embedded paths.
    - Smooth future no_std gating with minimal churn.
- Minimal surface change:
  - Internal numeric use in [snn-core/src/neuron.rs](snn-core/src/neuron.rs), [snn-core/src/runtime.rs](snn-core/src/runtime.rs) refactor to be generic over a Number trait (subset of FixedPoint), with default type alias to current Fixed.

2) Neuron Model Abstraction (pluggable dynamics)
- What to reuse:
  - Trait-based neuron update API: [Rust.trait EmbeddedNeuron](archived-shnn-embedded/src/embedded_neuron.rs:20) with update/reset/potential accessors.
  - Concrete models: LIF: [Rust.struct EmbeddedLIFNeuron](archived-shnn-embedded/src/embedded_neuron.rs:42); Izhikevich: [Rust.struct EmbeddedIzhikevichNeuron](archived-shnn-embedded/src/embedded_neuron.rs:170).
- How to integrate:
  - Introduce a light NeuronModel trait in snn-core with a minimal step/update signature returning spike/no spike and next refractory, and with an associated numeric type (or use the fixed-point trait if enabled).
  - Provide a simple I&F model equivalent to current [snn-core/src/neuron.rs](snn-core/src/neuron.rs) as the default NeuronModel; add Izhikevich behind a cargo feature flag or separate module.
  - Runtime calls model.step(input, time) to decide spike and scheduling (no change in outer time-wheel semantics).
- Benefits:
  - Researchers can swap models without touching runtime and hypergraph layers.
  - Enables richer models later (Hodgkin–Huxley approximations, adaptive LIF).

3) Synapse with explicit delayed spike buffering
- What to reuse:
  - Per-synapse delayed spike buffer pattern: [Rust.struct EmbeddedSynapse](archived-shnn-embedded/src/embedded_neuron.rs:304) with receive_spike() and get_output_current(): [Rust.fn EmbeddedSynapse::receive_spike()](archived-shnn-embedded/src/embedded_neuron.rs:333), [Rust.fn EmbeddedSynapse::get_output_current()](archived-shnn-embedded/src/embedded_neuron.rs:349).
- How to integrate:
  - For snn-core’s hypergraph edges, time-wheel already handles delivery at time T+delay. However, the buffer pattern is valuable for plasticity windows and for accumulating multiple spikes with different intra-edge delays if needed.
  - Introduce optional per-edge recent-spike ring buffer to power STDP and windowed effects. Use a small Vec or array-backed ring to stay zero-deps.
- Benefits:
  - Unlocks local learning rules and temporally extended effects without touching the global queue.

4) Hypergraph adjacency index (source → edges)
- What to reuse:
  - Node connection index (node → edge(s)): [Rust.struct EmbeddedHypergraph](archived-shnn-embedded/src/embedded_memory.rs:27), with node_connections map and add_hyperedge() building indexes: [Rust.fn EmbeddedHypergraph::add_hyperedge()](archived-shnn-embedded/src/embedded_memory.rs:81).
- How to integrate:
  - Add a source_to_edges adjacency Vec<Vec<edge_id>> built alongside edges in snn-core, enabling O(outdegree) delivery in [snn-core/src/runtime.rs](snn-core/src/runtime.rs) instead of scanning all edges per event.
  - This is the single largest runtime performance win with trivial complexity.
- Minimal changes:
  - Update add_edge() to populate adjacency.
  - Update step_once() to iterate over adjacency[source].

5) Event-driven bounded processing
- What to reuse:
  - Budgeted per-tick processing and queued actives: [Rust.fn EmbeddedSNN::process_active_budget()](archived-shnn-embedded/src/embedded_network.rs:365).
- How to integrate:
  - For snn-core, retain global time semantics via TimeWheel, but add an optional processing budget per tick (max edges visited, max spikes scheduled).
  - Useful for real-time TUI demos and embedded targets to prevent frame overruns.
- Implementation note:
  - Maintain counters inside step_once() and early-stop propagation when budgets are reached; remaining work will complete in subsequent ticks.

6) Plasticity hooks and simple STDP
- What to reuse:
  - STDP with pre/post traces and clamped updates: [Rust.fn EmbeddedSNN::apply_stdp()](archived-shnn-embedded/src/embedded_network.rs:559), traces fields initialization/decay: [Rust.fn EmbeddedSNN::decay_traces()](archived-shnn-embedded/src/embedded_network.rs:549).
- How to integrate:
  - Add PlasticityRule trait in snn-core with hooks:
    - on_pre_spike(neuron_id, t), on_post_spike(neuron_id, t), update_weight(edge_id) or update(pre, post, edge_id)
  - Provide a minimal quantized STDP implementation gated by a feature.
  - Wire hooks in runtime delivery path where spikes are received by targets.
- Benefit:
  - Learning support without changing simulation semantics; optional and off by default.

7) Partitioning (optional, feature-gated)
- What to reuse:
  - Partition map and per-partition spike queues: [archived-shnn-embedded/src/partitioning.rs](archived-shnn-embedded/src/partitioning.rs).
- How to integrate:
  - Feature “partitioning” to allow sharding networks; defer cross-partition delivery by a tick or route through per-partition queues.
  - Not necessary for desktop but valuable for scaling or mapping to multi-core RTOS tasks.

8) Statistics, diagnostics, and builder ergonomics
- What to reuse:
  - Stats struct and update logic: [Rust.struct NetworkStatistics](archived-shnn-embedded/src/embedded_network.rs:65), [Rust.fn EmbeddedSNN::update_statistics()](archived-shnn-embedded/src/embedded_network.rs:619).
  - Builder pattern for common topologies: [Rust.struct EmbeddedNetworkBuilder](archived-shnn-embedded/src/embedded_network.rs:746).
- How to integrate:
  - Provide a light SnnBuilder in snn-core for rapidly assembling small networks (layers, default weights/delays).
  - Add optional stats counters in runtime behind a “stats” feature. Expose total spikes, activity ratios, simple energy proxy.

Proposed phased plan

Phase 1: Performance and API foundations (low risk)
- Add adjacency index in runtime:
  - Update [snn-core/src/runtime.rs](snn-core/src/runtime.rs) to keep Vec<Vec<u32>> source_to_edges; change step_once() to iterate only edges from the source.
- Add processing budgets (optional parameters to step_once()):
  - max_edge_visits_per_tick, max_spikes_per_tick with saturating behavior and counters.
- Documentation:
  - Update [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) with adjacency and budget semantics.

Phase 2: Numeric abstraction and neuron models (medium risk)
- Introduce a Number or FixedPoint-lite trait in snn-core with feature fixed-point toggling to Q16_16, ported minimal API from [Rust.trait FixedPoint](archived-shnn-embedded/src/fixed_point.rs:14).
- Add NeuronModel trait and refactor [snn-core/src/neuron.rs](snn-core/src/neuron.rs) to a default I&F model that implements it.
- Optional: port Izhikevich model in a separate module gated by “izhikevich” feature.

Phase 3: Plasticity hooks and ring buffer (optional)
- Add a small per-edge spike ring buffer (array-backed) to support plasticity windows akin to [Rust.struct EmbeddedSpikeBuffer](archived-shnn-embedded/src/embedded_memory.rs:201).
- Add PlasticityRule trait and a minimal STDP implementation inspired by [Rust.fn EmbeddedSNN::apply_stdp()](archived-shnn-embedded/src/embedded_network.rs:559).

Phase 4: Partitioning and builder (optional)
- Port partitioning primitives under a feature flag.
- Add SnnBuilder inspired by [Rust.struct EmbeddedNetworkBuilder](archived-shnn-embedded/src/embedded_network.rs:746) for quick topologies.

Concrete change sketches (snn-core)

A) Adjacency index (source → edges)
- In [snn-core/src/runtime.rs](snn-core/src/runtime.rs):
  - Add field: source_to_edges: Vec<Vec<u32>>
  - In add_edge(): for each source s in sources, push edge_id into source_to_edges[s].
  - In step_once(): replace scanning all self.edges with iterating source_to_edges[ev.neuron_id as usize] and indexing edges by id.

B) Budgets
- Add step_once_with_budget(max_edges: Option<usize>, max_spikes: Option<usize>) and call from step_once() with None/None.

C) NeuronModel trait
- New module [snn-core/src/model.rs](snn-core/src/model.rs) with a trait:
  - step(dt, input, time) -> fired?, with get/set membrane helpers if needed.
- Make the current neuron implementation a Model impl and store model enum or generic parameter in runtime; keep a simple default path for today’s API.

D) Plasticity hooks
- Add trait PlasticityRule with pre/post hooks; runtime invokes on deliveries and spikes. Reference logic from [Rust.fn EmbeddedSNN::decay_traces()](archived-shnn-embedded/src/embedded_network.rs:549) and [Rust.fn EmbeddedSNN::apply_stdp()](archived-shnn-embedded/src/embedded_network.rs:559).

Risk and compatibility
- Adjacency and budgets are internal and safe; external API unchanged.
- Introducing traits can be hidden behind features to preserve current type signatures.
- Plasticity and partitioning are purely optional features.

Why these choices
- We keep snn-core zero-deps and minimal by:
  - Using Vec-based structures only (no heapless).
  - Gating advanced capabilities behind features.
  - Retaining event-driven semantics and simple APIs.
- We leverage proven patterns from archived-shnn-embedded that are battle-tested for embedded constraints while adapting to the simpler hypergraph/event runtime in snn-core.

Traceability map (archived → snn-core targets)
- FixedPoint/Q16_16 → [snn-core/src/fixed.rs](snn-core/src/fixed.rs) (feature “fixed-point”)
- EmbeddedNeuron/LIF/Izhikevich → new model trait/module in snn-core; refactor [snn-core/src/neuron.rs](snn-core/src/neuron.rs)
- EmbeddedSynapse delayed buffer → optional per-edge ring buffer inside [snn-core/src/hypergraph.rs](snn-core/src/hypergraph.rs)
- EmbeddedHypergraph node_connections → source_to_edges adjacency in [snn-core/src/runtime.rs](snn-core/src/runtime.rs)
- process_active_budget → step_once budgeted variant in [snn-core/src/runtime.rs](snn-core/src/runtime.rs)
- apply_stdp/decay_traces → PlasticityRule and hooks in runtime loop (feature “plasticity”)
- Partitioning queues → feature “partitioning”, adapted to snn-core if desired
- Network builder → optional SnnBuilder module for user ergonomics

Validation strategy
- Port targeted unit tests with small adjustments:
  - Neuron spike behavior mirrors archived tests using default dt/current injection.
  - Adjacency correctness: ensure events propagate identical to pre-adjacency version.
  - Budget limits: assert caps on spikes and work per tick.
  - STDP smoke tests: weight moves within bounds upon pre/post spikes (when feature enabled).

Outcome
- snn-core gains performance (adjacency), extensibility (model/plasticity traits), and embedded-readiness (numeric abstraction) while remaining minimal and dependency-free by default. This reuse plan incrementally imports well-scoped, field-tested ideas from archived-shnn-embedded and adapts them to snn-core’s hypergraph-first, event-driven runtime.