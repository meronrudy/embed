# Embeddable Hypergraph SNN — Core + TUI (Zero-dependency core)

This workspace contains:
- snn-core: zero-dependency, embeddable spiking neural network (SNN) runtime with hypergraph connectivity and an O(1) time-wheel event queue.
- snn-tui: researcher-friendly terminal UI (ratatui + crossterm) that renders a 2D spike raster (time on X, neuron IDs on Y) and controls to step or run the simulation.

Architecture is designed for portability (desktop/WASM/embedded), auditability, and extensibility to richer SNN backends without changing the TUI.

Quick start
- Build workspace:
  - cargo build --workspace
- Run TUI (2D raster, controls [s] step, [r] run/pause, [q] quit):
  - cargo run -p snn-tui

Repository layout
- [Cargo.toml](Cargo.toml)
- snn-core (zero-dependency runtime)
  - [snn-core/Cargo.toml](snn-core/Cargo.toml)
  - [snn-core/src/lib.rs](snn-core/src/lib.rs)
  - Core modules:
    - Event queue: [snn-core/src/event_queue.rs](snn-core/src/event_queue.rs)
    - Fixed-point math: [snn-core/src/fixed.rs](snn-core/src/fixed.rs)
    - Sparse CSR (optional building block): [snn-core/src/sparse.rs](snn-core/src/sparse.rs)
    - Minimal IR for future codegen: [snn-core/src/ir.rs](snn-core/src/ir.rs)
    - Neuron model (simple I&F): [snn-core/src/neuron.rs](snn-core/src/neuron.rs)
    - Hypergraph edges: [snn-core/src/hypergraph.rs](snn-core/src/hypergraph.rs)
    - Runtime engine: [snn-core/src/runtime.rs](snn-core/src/runtime.rs)
- snn-tui (researcher-focused TUI)
  - [snn-tui/Cargo.toml](snn-tui/Cargo.toml)
  - [snn-tui/src/backend.rs](snn-tui/src/backend.rs)
  - [snn-tui/src/app.rs](snn-tui/src/app.rs)
  - [snn-tui/src/ui.rs](snn-tui/src/ui.rs)
  - [snn-tui/src/main.rs](snn-tui/src/main.rs)

High-level architecture

┌───────────────────────────────┐
│        TUI Frontend           │  ratatui + crossterm
│  - Raster (time × neurons)    │  [snn-tui/src/ui.rs](snn-tui/src/ui.rs)
│  - Controls (step/run/quit)   │
│  - App state (circular raster)│  [Rust.struct App](snn-tui/src/app.rs:6)
│  - Backend abstraction         │  [Rust.trait SnnBackend](snn-tui/src/backend.rs:6)
└───────────────┬───────────────┘
                │ trait-based
┌───────────────▼───────────────┐
│           Core Backend         │
│  - CoreBackend adapter         │  [Rust.struct CoreBackend](snn-tui/src/backend.rs:14)
│  - Drives snn-core runtime     │  [Rust.fn CoreBackend::new()](snn-tui/src/backend.rs:19)
└───────────────┬───────────────┘
                │ calls into
┌───────────────▼───────────────┐
│            snn-core           │  (zero-dependency)
│  - Time wheel event queue     │  [Rust.struct TimeWheel](snn-core/src/event_queue.rs:9)
│  - Hypergraph edges           │  [Rust.struct HyperEdge](snn-core/src/hypergraph.rs:3)
│  - Neuron state (fixed-point) │  [Rust.struct Neuron](snn-core/src/neuron.rs:5)
│  - Fixed-point Q16.16         │  [Rust.fn fixed_mul()](snn-core/src/fixed.rs:18)
│  - Runtime step semantics     │  [Rust.fn SnnRuntime::step_once()](snn-core/src/runtime.rs:41)
└───────────────────────────────┘

Core design (snn-core)
1) Event-driven simulation via time wheel
- Spikes are discrete events with time stamps: [Rust.struct SpikeEvent](snn-core/src/event_queue.rs:3)
- O(1) amortized scheduling in bounded delay horizons using a ring of buckets:
  - Create wheel: [Rust.fn TimeWheel::new()](snn-core/src/event_queue.rs:16)
  - Schedule event: [Rust.fn TimeWheel::schedule()](snn-core/src/event_queue.rs:29)
  - Pop current slot and advance time: [Rust.fn TimeWheel::next()](snn-core/src/event_queue.rs:35)
- Suited for embedded use where synaptic delays are bounded.

2) Hypergraph connectivity
- One hyperedge connects multiple sources to multiple targets with a weight and delay:
  - [Rust.struct HyperEdge](snn-core/src/hypergraph.rs:3)
- Naive scan on edges for simplicity/zero-deps; future: source→edge index for O(outdegree) delivery.

3) Neuron model (simple integrate-and-fire)
- Fixed-point membrane with thresholding:
  - [Rust.fn Neuron::inject()](snn-core/src/neuron.rs:24)
- On threshold crossing, membrane resets to 0; refractory is provisioned but disabled by default.

4) Fixed-point arithmetic (Q16.16)
- Deterministic, FPU-free friendly:
  - [Rust.fn to_fixed()](snn-core/src/fixed.rs:8)
  - [Rust.fn from_fixed()](snn-core/src/fixed.rs:13)
  - [Rust.fn fixed_mul()](snn-core/src/fixed.rs:18)
- Swap precision globally by adjusting FRACTIONAL_BITS and type alias.

5) Runtime step semantics
- Single-tick stepping drives everything:
  - [Rust.fn SnnRuntime::step_once()](snn-core/src/runtime.rs:41)
    - Pops events for the current tick
    - Delivers along hyperedges (weight, delay)
    - Schedules newly fired spikes at their delivery time
    - Returns only the spikes that occurred at “this” tick (events popped), which the TUI uses to mark the current column.
- Convenience runners:
  - [Rust.fn SnnRuntime::run_until()](snn-core/src/runtime.rs:74)
  - [Rust.fn SnnRuntime::run_ticks()](snn-core/src/runtime.rs:81)

6) Optional sparse building block (CSR)
- Included for future vectorized or hybrid simulation modes: [snn-core/src/sparse.rs](snn-core/src/sparse.rs)
- Example multiply: y = A*x in fixed-point for batch-style propagation.

TUI design (snn-tui)
1) Backend abstraction for extensibility
- Swap different SNN engines without UI changes:
  - [Rust.trait SnnBackend](snn-tui/src/backend.rs:6) → step() and neurons()
  - Core backend adapter: [Rust.struct CoreBackend](snn-tui/src/backend.rs:14)

2) App-level circular raster buffer
- 2D char matrix [neuron][time mod width] updated per tick:
  - [Rust.struct App](snn-tui/src/app.rs:6)
  - Step logic: [Rust.fn App::step()](snn-tui/src/app.rs:31)

3) Raster rendering and status panel
- draw() returns a 2-pane layout:
  - [Rust.fn draw()](snn-tui/src/ui.rs:20)
    - Top: raster rows “nXX |•• ••”
    - Bottom: status and controls

4) Controls and event loop
- [snn-tui/src/main.rs](snn-tui/src/main.rs)
- Controls: s (step), r (run/pause), q (quit)
- 100ms tick for run mode by default; change width in App::new(…, width).

Simulation semantics (important details)
- Causality: Events scheduled at time T are popped by TimeWheel::next() when current_time == T. Any spikes produced from delivering these events are scheduled at (T + delay) and not returned to the caller of step_once(); they will appear when the wheel reaches their time slot.
- Determinism: Fixed-point math and explicit scheduling removes FPU variability. For full determinism across targets, avoid integer overflow via saturating arithmetic and choose conservative ranges for weights/thresholds.
- Complexity:
  - Schedule: O(1) amortized
  - Pop: O(1) amortized
  - Delivery per event: O(E) naive where E is number of hyperedges; recommended next step is indexing edges by source to achieve O(outdegree).

Extending the core
- Faster edge delivery:
  - Maintain source→edges index (Vec<Vec<edge_id>>) to avoid scanning all edges.
- Rich neuron models:
  - Replace Neuron::inject() with a small trait (e.g., NeuronModel) and keep a model enum or static dispatch in no_std.
- Synaptic plasticity:
  - Add PlasticityRule update hooks when pre/post spikes occur.
- Hybrid vector/event execution:
  - Use CSR to batch when spike rates are high; fall back to event-driven in sparse regimes.
- no_std and allocator-free:
  - Replace Vec with fixed-capacity buffers, preallocate all structures, and gate std usage with features.

Building and running
- Prerequisites: Rust stable (1.70+ recommended)
- Build everything:
  - cargo build --workspace
- Run the TUI:
  - cargo run -p snn-tui
- Adjust raster width:
  - In [snn-tui/src/main.rs](snn-tui/src/main.rs), change the width argument in App::new(…, width).
- Example network:
  - CoreBackend seeds n0 at t=0, wired to [n1, n2] with weight 1.0 and delay 1; the raster should show a dot on row 0 at t=0, then rows 1 and 2 at t=1, etc.

Testing checklist
- Time wheel:
  - Schedule multiple events in same slot and across slots; verify pop order per-slot is FIFO-agnostic (unordered per bucket) but time-correct.
- Thresholds:
  - Verify that membrane resets on firing and spikes only when ≥ threshold.
- Delay semantics:
  - Deliveries occur at ev.time + edge.delay.

Roadmap
- Source→edge adjacency for O(outdegree) delivery
- Optional leak/refractory dynamics and richer models
- Plasticity rules (e.g., STDP) with pre/post spike windows
- Feature-gated no_std
- WASM compile target with a minimal JS TUI
- Optional MLIR-like mini IR lowering into specialized kernels (keeping runtime zero-dep)

License
- MIT or Apache-2.0 (dual); see crate Cargo.toml for identifiers.
