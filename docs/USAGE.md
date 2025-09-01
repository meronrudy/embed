# Usage Guide

This guide covers how to build/run the workspace, use the zero-dependency core programmatically, and understand the TUI’s behavior and controls. It also explains the snn-core-plus integration used by the TUI for adjacency, budgets, and optional plasticity.

Contents
- Build and run
- TUI usage
- Programmatic core usage
- Event semantics and timing
- Expected output and verification
- Troubleshooting

Build and run
- Build everything:
  - cargo build --workspace
- Run the TUI (2D raster plot of spikes):
  - cargo run -p snn-tui

TUI runtime features (via snn-core-plus)
- Budgets: limit per-tick work deterministically
  - Keys: +/- to change max edge visits; [ / ] to change max scheduled spikes
  - Environment variables at launch:
    - SNN_TUI_BUDGET_EDGES=100
    - SNN_TUI_BUDGET_SPIKES=200
- Plasticity (Quantized STDP), feature-gated:
  - Build feature: cargo run -p snn-tui --features plasticity
  - Toggle at runtime with p
  - Enable by default at launch: SNN_TUI_PLASTICITY=1 cargo run -p snn-tui --features plasticity

TUI usage
- Controls in the TUI:
  - s: step once (advance one tick)
  - r: run/pause
  - q: quit
- The top pane shows a raster:
  - Rows: neuron IDs n00, n01, n02, …
  - Columns: time, wrapping modulo the configured width
  - Cell: “•” indicates a spike happened at that neuron in the current column’s tick
- The bottom pane shows status and controls hints.

Where to adjust TUI settings
- The raster width is configured when constructing the app:
  - [Rust.fn App::new()](../snn-tui/src/app.rs:15)
- The run loop tick rate is set in main:
  - [Rust.fn main()](../snn-tui/src/main.rs:29) → see the tick_rate near initialization

Programmatic core usage
The core is designed to be used as a library (zero dependencies). Typical flow:

Note: The TUI uses snn-core-plus SnnRuntimePlus which composes snn-core to add adjacency, budgets, and optional plasticity while keeping snn-core unchanged.

- Create a runtime with a time wheel dimension large enough to cover your maximum synaptic delay:
  - [Rust.fn SnnRuntime::new()](../snn-core/src/runtime.rs:12)
- Add neurons:
  - [Rust.fn SnnRuntime::add_neuron()](../snn-core/src/runtime.rs:20)
- Add hyperedges (source set, target set, weight, delay):
  - [Rust.fn SnnRuntime::add_edge()](../snn-core/src/runtime.rs:26)
- Seed one or more initial spikes:
  - [Rust.struct SpikeEvent](../snn-core/src/event_queue.rs:3)
  - [Rust.fn TimeWheel::schedule()](../snn-core/src/event_queue.rs:29)
- Step the runtime (single tick) and receive spikes for “this” tick:
  - [Rust.fn SnnRuntime::step_once()](../snn-core/src/runtime.rs:41)

Minimal example (library usage)
This example mirrors the TUI’s demo network n0 → [n1, n2] with unit weight and delay = 1 tick.

```rust
use snn_core::{SnnRuntime, SpikeEvent};

fn main() {
    let mut snn = SnnRuntime::new(32); // wheel size (max delay horizon)

    // Add 3 neurons (threshold 1.0 each)
    let n0 = snn.add_neuron(1.0);
    let n1 = snn.add_neuron(1.0);
    let n2 = snn.add_neuron(1.0);

    // Hyperedge: n0 → [n1, n2], weight = 1.0 (Q16.16) and delay = 1
    snn.add_edge(vec![n0], vec![n1, n2], 1.0, 1);

    // Seed an initial spike at t = 0 on neuron 0
    snn.queue.schedule(SpikeEvent { neuron_id: n0, time: 0 });

    // Step several ticks; step_once() returns spikes that happened at the current tick
    for _ in 0..5 {
        let spikes = snn.step_once();
        for sp in spikes {
            println!("t={} neuron {} fired", snn.queue.current_time.saturating_sub(1), sp.neuron_id);
        }
    }
}
```

Event semantics and timing
- Spikes are discrete events associated with a time (tick).
  - [Rust.struct SpikeEvent](../snn-core/src/event_queue.rs:3)
- The time wheel is a ring of buckets; scheduling is O(1) amortized:
  - [Rust.struct TimeWheel](../snn-core/src/event_queue.rs:9)
  - [Rust.fn TimeWheel::new()](../snn-core/src/event_queue.rs:16)
  - [Rust.fn TimeWheel::schedule()](../snn-core/src/event_queue.rs:29)
  - [Rust.fn TimeWheel::next()](../snn-core/src/event_queue.rs:35)
- Step semantics:
  - [Rust.fn SnnRuntime::step_once()](../snn-core/src/runtime.rs:41)
  - It pops all events scheduled at the current time T and returns them to the caller (these are the spikes at time T).
  - While processing these events, the runtime delivers synaptic effects and schedules any newly fired spikes at (T + delay). These spikes show up in future calls when the time wheel reaches those times.

Neuron model and fixed-point
- Neuron is a simple fixed-point integrate-and-fire unit:
  - [Rust.struct Neuron](../snn-core/src/neuron.rs:5)
  - [Rust.fn Neuron::inject()](../snn-core/src/neuron.rs:24)
- Fixed-point helpers:
  - [Rust.fn to_fixed()](../snn-core/src/fixed.rs:8), [Rust.fn from_fixed()](../snn-core/src/fixed.rs:13), [Rust.fn fixed_mul()](../snn-core/src/fixed.rs:18)
- Thresholds and weights are Q16.16 fixed-point in the core; exposures in the runtime API accept f32 and convert via to_fixed().

Expected output and verification
- With the demo network:
  - At t=0, n0 has a spike (seeded).
  - At t=1, n1 and n2 spike due to the hyperedge (weight 1.0 ≥ threshold 1.0).
- In the TUI, stepping once places “•” at row n00, the next step places “•” at rows n01 and n02, etc.
- In programmatic mode, printing spikes each tick should show lines like:
  - t=0 neuron 0 fired
  - t=1 neuron 1 fired
  - t=1 neuron 2 fired

Troubleshooting
- Empty raster initially:
  - Press s (step) or r (run). The first dot appears after one step because the first seed is at t=0, which is consumed when stepping.
- No spikes visible after many steps:
  - Verify that thresholds and weights are compatible and that at least one initial spike is scheduled:
    - [Rust.fn SnnRuntime::add_neuron()](../snn-core/src/runtime.rs:20)
    - [Rust.fn SnnRuntime::add_edge()](../snn-core/src/runtime.rs:26)
    - [Rust.fn TimeWheel::schedule()](../snn-core/src/event_queue.rs:29)
- Keybindings do not work:
  - Ensure the terminal supports raw mode (works in most terminal emulators and VSCode’s integrated terminal).
- Performance considerations:
  - For large networks, naive O(E) delivery will be a bottleneck; move to a source→edges adjacency index (see Architecture and Extensibility docs).