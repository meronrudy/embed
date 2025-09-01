# Architecture: Embeddable Hypergraph SNN (Zero-dependency core)

This document provides a deep-dive into the architecture, design trade-offs, and extensibility points of the embeddable hypergraph-based Spiking Neural Network (SNN) runtime and its researcher-friendly TUI.

Contents
- Goals and constraints
- Layered system architecture
- Core data model (hypergraph + events)
- Execution model (time wheel)
- Complexity and performance
- Fixed-point arithmetic strategy
- Memory model and no_std path
- TUI architecture and backend abstraction
- Extensibility roadmap (sparse, plasticity, MLIR-like IR)
- Determinism and testing notes

Goals and constraints
- Zero-dependency core: The runtime compiles with the Rust standard library and no external crates for maximum portability/auditability.
- Embeddable design: Compact memory layout, static dispatch compatibility, fixed-point math for microcontrollers.
- Hypergraph connectivity: Synapses modeled as hyperedges connecting multiple sources to multiple targets.
- Event-driven execution: Spikes are timestamped events; a time wheel provides O(1) amortized scheduling in bounded delay horizons.
- TUI ergonomics: A separate crate (with dependencies) provides visualization and interactive control, decoupled via a backend trait.

Layered system architecture

┌───────────────────────────────┐
│        TUI Frontend           │  ratatui + crossterm
│  - Raster (time × neurons)    │  [snn-tui/src/ui.rs](../snn-tui/src/ui.rs)
│  - Controls (step/run/quit)   │
│  - App state (circular raster)│  [Rust.struct App](../snn-tui/src/app.rs:6)
│  - Backend abstraction         │  [Rust.trait SnnBackend](../snn-tui/src/backend.rs:6)
└───────────────┬───────────────┘
                │ trait-based
┌───────────────▼───────────────┐
│          Core Backend          │
│  - CoreBackend adapter         │  [Rust.struct CoreBackend](../snn-tui/src/backend.rs:14)
│  - Drives snn-core runtime     │  [Rust.fn CoreBackend::new()](../snn-tui/src/backend.rs:19)
└───────────────┬───────────────┘
                │ calls into
┌───────────────▼───────────────┐
│            snn-core           │  (zero-dependency)
│  - Time wheel event queue     │  [Rust.struct TimeWheel](../snn-core/src/event_queue.rs:9)
│  - Hypergraph edges           │  [Rust.struct HyperEdge](../snn-core/src/hypergraph.rs:3)
│  - Neuron state (fixed-point) │  [Rust.struct Neuron](../snn-core/src/neuron.rs:5)
│  - Fixed-point Q16.16         │  [Rust.fn fixed_mul()](../snn-core/src/fixed.rs:18)
│  - Runtime step semantics     │  [Rust.fn SnnRuntime::step_once()](../snn-core/src/runtime.rs:41)
└───────────────────────────────┘

Core data model (hypergraph + events)
- Spikes are events with a firing neuron and a timestamp:
  - [Rust.struct SpikeEvent](../snn-core/src/event_queue.rs:3)
- Hyperedges connect sets of source neurons to sets of target neurons with a single weight and delay:
  - [Rust.struct HyperEdge](../snn-core/src/hypergraph.rs:3)
- Neuron state uses fixed-point membrane potentials with threshold and an optional refractory period:
  - [Rust.struct Neuron](../snn-core/src/neuron.rs:5)
  - Update path (simple integrate-and-fire): [Rust.fn Neuron::inject()](../snn-core/src/neuron.rs:24)

Execution model (time wheel)
- The time wheel is a ring buffer of buckets, each bucket holding all spike events scheduled at a particular time slot within a fixed horizon (wheel size).
  - Define wheel: [Rust.struct TimeWheel](../snn-core/src/event_queue.rs:9)
  - Create wheel: [Rust.fn TimeWheel::new()](../snn-core/src/event_queue.rs:16)
  - Schedule event: [Rust.fn TimeWheel::schedule()](../snn-core/src/event_queue.rs:29)
  - Pop/advance: [Rust.fn TimeWheel::next()](../snn-core/src/event_queue.rs:35)

Step semantics
- The runtime API advances one tick at a time and returns the spikes that occurred at the current tick:
  - [Rust.struct SnnRuntime](../snn-core/src/runtime.rs:5)
  - [Rust.fn SnnRuntime::step_once()](../snn-core/src/runtime.rs:41)
- Detailed flow:
  1) Pop current slot events -> these are the “spikes at time T.” Returned to caller.
  2) For each popped event, deliver along all hyperedges that include the source.
  3) For each target neuron, inject the edge weight at time T + delay; on threshold crossing, schedule a new SpikeEvent at that delivery time.
  4) Newly generated spikes are scheduled for future slots and will be returned when their time arrives.
- Convenience runners:
  - [Rust.fn SnnRuntime::run_until()](../snn-core/src/runtime.rs:74)
  - [Rust.fn SnnRuntime::run_ticks()](../snn-core/src/runtime.rs:81)

Complexity and performance
- Scheduling: O(1) amortized (ring index = time % wheel_size).
- Pop: O(1) amortized to take a bucket.
- Delivery: Naive O(E) per event due to scanning all hyperedges. With source→edges indexing, delivery becomes O(outdegree).
- Memory: The wheel holds buckets as Vec<SpikeEvent>; capacity grows with workload. For embedded/no_std, replace with fixed-capacity circular buffers.
- Recommended next optimization:
  - Maintain an adjacency index Vec<Vec<edge_id>> keyed by source neuron to eliminate edge scans (kept out of zero-deps MVP for simplicity).

Fixed-point arithmetic strategy
- Q16.16 fixed-point, type alias i32:
  - Constants and helpers: [Rust.const FRACTIONAL_BITS](../snn-core/src/fixed.rs:4), [Rust.fn to_fixed()](../snn-core/src/fixed.rs:8), [Rust.fn from_fixed()](../snn-core/src/fixed.rs:13), [Rust.fn fixed_mul()](../snn-core/src/fixed.rs:18)
- Rationale:
  - Deterministic math without relying on FPUs.
  - Simplified porting to microcontrollers and WASM.
- Guidelines:
  - Ensure thresholds/weights fit chosen range; consider saturating arithmetic where overflow risks exist.
  - FRACTIONAL_BITS can be tuned; consider Q8.24 (i32) or Q16.16 (i32) based on dynamic range.

Memory model and no_std path
- Current core uses Vec and std; to migrate to no_std:
  - Replace Vec with fixed-capacity ring buffers or array-backed collections.
  - Pre-allocate neuron/edge arrays and manage free-lists for reconfiguration.
  - Feature-gate std usage and opt-in to allocator-free builds.
- Arena patterns:
  - Manual index-based arenas with slab-like arrays to avoid dynamic allocation overhead in constrained targets.

TUI architecture and backend abstraction
- Decoupling via trait:
  - [Rust.trait SnnBackend](../snn-tui/src/backend.rs:6) defines two required methods:
    - step(): advance by one tick and return spikes for the current tick.
    - neurons(): return neuron count (rows in raster).
- Core-backed implementation:
  - [Rust.struct CoreBackend](../snn-tui/src/backend.rs:14)
  - Construction seeds a tiny demo network: [Rust.fn CoreBackend::new()](../snn-tui/src/backend.rs:19)
- App state maintains a circular raster buffer:
  - [Rust.struct App](../snn-tui/src/app.rs:6)
  - [Rust.fn App::step()](../snn-tui/src/app.rs:31) writes dots “•” at (row=neuron, col=tick % width)
- Rendering:
  - [Rust.fn draw()](../snn-tui/src/ui.rs:20) composes a two-pane UI:
    - Top: 2D raster (time →, neurons ↓)
    - Bottom: status bar with tick, neuron count, and controls
- Controls and loop:
  - [snn-tui/src/main.rs](../snn-tui/src/main.rs) handles:
    - s: single-step, r: run/pause, q: quit
    - 100 ms tick interval in run mode

Determinism and testing notes
- Determinism:
  - Fixed-point arithmetic, explicit event times, and an O(1) scheduling strategy yield deterministic behavior across runs.
  - Bucket ordering within the same slot is the sequence of scheduling; if strict FIFO is needed, enforce a queue invariant per slot.
- Suggested tests:
  - Event scheduling: schedule overlapping and spaced events; assert current_time monotonicity and correct pop times.
  - Firing thresholds: inject weights below/at/above threshold; assert membrane and spike emission behavior.
  - Delays: verify that delivery occurs at source_event.time + edge.delay.
- Diagnostics:
  - Add optional compile-time logging macros or a “trace” feature that records spike trains for off-line validation.

Extensibility roadmap
- Source→edge adjacency index
  - Store for each source neuron a compact list of outgoing hyperedges; reduces delivery from O(E) to O(outdegree).
- Richer neuron models and plasticity hooks
  - Introduce a small trait (compile-time/static-disp) for neuron dynamics.
  - Add pre/post spike hooks for synaptic plasticity (e.g., STDP).
- Sparse vectorization
  - Provide CSR-driven batch operations for regimes with high concurrent spiking.
  - Hybrid stepping: event-driven when sparse, vectorized when dense.
- Mini IR and compilation path
  - The IR scaffold allows future transformation of model descriptions into optimized kernels:
    - [snn-core/src/ir.rs](../snn-core/src/ir.rs)
  - Pipeline concept:
    1) Build SNN description as IR ops.
    2) Apply simple passes (constant folding, dead op elimination).
    3) Lower into specialized runtime kernels (still zero-dep) or to platform-optimized code via a build step.
- no_std feature
  - Add feature flag to switch to allocator-free operation with fixed capacities for embedded targets.
- WASM support
  - The core design is compatible with wasm32-unknown-unknown; provide a small JS wrapper for a web-based raster visualization.

Key file/API index (for quick navigation)
- Runtime entrypoints:
  - [Rust.fn SnnRuntime::new()](../snn-core/src/runtime.rs:12)
  - [Rust.fn SnnRuntime::add_neuron()](../snn-core/src/runtime.rs:20)
  - [Rust.fn SnnRuntime::add_edge()](../snn-core/src/runtime.rs:26)
  - [Rust.fn SnnRuntime::step_once()](../snn-core/src/runtime.rs:41)
  - [Rust.fn SnnRuntime::run_until()](../snn-core/src/runtime.rs:74), [Rust.fn SnnRuntime::run_ticks()](../snn-core/src/runtime.rs:81)
- Time wheel:
  - [Rust.struct TimeWheel](../snn-core/src/event_queue.rs:9)
  - [Rust.fn TimeWheel::new()](../snn-core/src/event_queue.rs:16), [Rust.fn TimeWheel::schedule()](../snn-core/src/event_queue.rs:29), [Rust.fn TimeWheel::next()](../snn-core/src/event_queue.rs:35)
- Neuron/edges:
  - [Rust.struct Neuron](../snn-core/src/neuron.rs:5), [Rust.fn Neuron::inject()](../snn-core/src/neuron.rs:24)
  - [Rust.struct HyperEdge](../snn-core/src/hypergraph.rs:3)
- Fixed-point:
  - [Rust.const FRACTIONAL_BITS](../snn-core/src/fixed.rs:4), [Rust.fn to_fixed()](../snn-core/src/fixed.rs:8), [Rust.fn from_fixed()](../snn-core/src/fixed.rs:13), [Rust.fn fixed_mul()](../snn-core/src/fixed.rs:18)
- TUI:
  - [Rust.trait SnnBackend](../snn-tui/src/backend.rs:6), [Rust.struct CoreBackend](../snn-tui/src/backend.rs:14), [Rust.fn CoreBackend::new()](../snn-tui/src/backend.rs:19)
  - [Rust.struct App](../snn-tui/src/app.rs:6), [Rust.fn App::step()](../snn-tui/src/app.rs:31)
  - [Rust.fn draw()](../snn-tui/src/ui.rs:20)
  - Controls in main: [snn-tui/src/main.rs](../snn-tui/src/main.rs)

Appendix: sequence of one tick
1) UI calls backend.step():
   - Core backend calls [Rust.fn SnnRuntime::step_once()](../snn-core/src/runtime.rs:41).
2) Runtime:
   - Pops current slot events via [Rust.fn TimeWheel::next()](../snn-core/src/event_queue.rs:35).
   - For each popped event e = (neuron_id, time=T):
     - For each hyperedge with neuron_id ∈ sources:
       - For each target:
         - Inject weight at time T + delay; if fired, schedule new event at T + delay.
3) Backend returns popped events (spikes at T) to the App.
4) App writes these spikes as “•” into column (T mod width) for the respective neuron rows.
5) UI redraws.
