//! Plasticity primitives for snn-core-plus (feature "plasticity").
//!
//! - Trait PlasticityRule: hooks for decay and pre/post spikes, with edge weight update.
//! - QuantizedStdp: minimal STDP with fixed-point style parameters (Q16.16 i32),
//!   maintaining per-neuron pre/post traces and clamping weights.
//!
//! This module intentionally uses the same fixed representation as snn-core (i32 Q16.16)
//! and avoids external dependencies.

/// Q16.16 helpers (mirrors snn-core fixed.rs constants/behavior)
const FRAC_BITS: i32 = 16;
const ONE: i32 = 1 << FRAC_BITS;

#[inline]
fn fx_from_f32(x: f32) -> i32 {
    (x * (ONE as f32)) as i32
}

#[inline]
fn fx_to_f32(x: i32) -> f32 {
    (x as f32) / (ONE as f32)
}

#[inline]
fn fx_mul(a: i32, b: i32) -> i32 {
    ((a as i64 * b as i64) >> FRAC_BITS) as i32
}

#[inline]
fn fx_add_sat(a: i32, b: i32) -> i32 {
    a.saturating_add(b)
}

#[inline]
fn fx_sub_sat(a: i32, b: i32) -> i32 {
    a.saturating_sub(b)
}

/// Plasticity rule interface
pub trait PlasticityRule {
    /// Decay internal state each tick (e.g., exponential decay of traces)
    fn decay(&mut self);

    /// Called when a pre-synaptic neuron spikes at time `t`
    fn on_pre_spike(&mut self, pre: u32, t: u64);

    /// Called when a post-synaptic neuron spikes at time `t`
    fn on_post_spike(&mut self, post: u32, t: u64);

    /// Apply update to an edge weight that connects `pre -> post`.
    /// Weight is i32 Q16.16; implementation applies clamping.
    fn apply_edge(&mut self, pre: u32, post: u32, weight: &mut i32);
}

/// Minimal quantized STDP rule:
/// - Pre and post traces (Q16.16) with exponential-like decay via multiply by alpha in (0,1)
/// - On updates, weight = clamp(w - a_minus * post_trace + a_plus * pre_trace)
pub struct QuantizedStdp {
    // Parameters (Q16.16)
    a_plus: i32,
    a_minus: i32,
    alpha_pre: i32,
    alpha_post: i32,
    w_min: i32,
    w_max: i32,

    // State traces per neuron (Q16.16)
    pre_trace: Vec<i32>,
    post_trace: Vec<i32>,
}

impl QuantizedStdp {
    /// Create with floating parameters (converted to Q16.16).
    /// Typical defaults:
    /// - a_plus ~ 0.01, a_minus ~ 0.012
    /// - alpha_pre/post ~ 0.96 per tick
    /// - w_min = 0.0, w_max = 1.0 (for normalized weights)
    pub fn new(a_plus: f32, a_minus: f32, alpha_pre: f32, alpha_post: f32, w_min: f32, w_max: f32) -> Self {
        Self {
            a_plus: fx_from_f32(a_plus),
            a_minus: fx_from_f32(a_minus),
            alpha_pre: fx_from_f32(alpha_pre),
            alpha_post: fx_from_f32(alpha_post),
            w_min: fx_from_f32(w_min),
            w_max: fx_from_f32(w_max),
            pre_trace: Vec::new(),
            post_trace: Vec::new(),
        }
    }

    /// Convenience defaults matching archived embedded settings
    pub fn with_defaults() -> Self {
        Self::new(0.01, 0.012, 0.96, 0.96, 0.0, 1.0)
    }

    fn ensure_neuron(&mut self, id: u32) {
        let need = id as usize + 1;
        if self.pre_trace.len() < need {
            self.pre_trace.resize(need, 0);
        }
        if self.post_trace.len() < need {
            self.post_trace.resize(need, 0);
        }
    }

    /// Inspect traces (for tests/diagnostics)
    pub fn traces(&self, id: u32) -> (i32, i32) {
        let idx = id as usize;
        let pre = *self.pre_trace.get(idx).unwrap_or(&0);
        let post = *self.post_trace.get(idx).unwrap_or(&0);
        (pre, post)
    }
}

impl PlasticityRule for QuantizedStdp {
    fn decay(&mut self) {
        // Exponential-like decay: trace *= alpha (0 < alpha < 1 in Q16.16)
        for tr in &mut self.pre_trace {
            *tr = fx_mul(*tr, self.alpha_pre);
        }
        for tr in &mut self.post_trace {
            *tr = fx_mul(*tr, self.alpha_post);
        }
    }

    fn on_pre_spike(&mut self, pre: u32, _t: u64) {
        self.ensure_neuron(pre);
        // Increment pre trace by 1.0 on spike
        self.pre_trace[pre as usize] = fx_add_sat(self.pre_trace[pre as usize], ONE);
    }

    fn on_post_spike(&mut self, post: u32, _t: u64) {
        self.ensure_neuron(post);
        // Increment post trace by 1.0 on spike
        self.post_trace[post as usize] = fx_add_sat(self.post_trace[post as usize], ONE);
    }

    fn apply_edge(&mut self, pre: u32, post: u32, weight: &mut i32) {
        self.ensure_neuron(pre);
        self.ensure_neuron(post);

        let pre_tr = self.pre_trace[pre as usize];
        let post_tr = self.post_trace[post as usize];

        // Δw = +a_plus*pre_tr - a_minus*post_tr
        let ltp = fx_mul(self.a_plus, pre_tr);
        let ltd = fx_mul(self.a_minus, post_tr);
        let mut new_w = fx_add_sat(*weight, ltp);
        new_w = fx_sub_sat(new_w, ltd);

        // Clamp
        if new_w < self.w_min {
            new_w = self.w_min;
        }
        if new_w > self.w_max {
            new_w = self.w_max;
        }
        *weight = new_w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_params_and_basic_update() {
        let mut stdp = QuantizedStdp::with_defaults();
        let mut w = fx_from_f32(0.5);

        // Pre spike on neuron 1, small decay, then post spike on neuron 2
        stdp.on_pre_spike(1, 0);
        stdp.decay();
        stdp.on_post_spike(2, 1);

        stdp.apply_edge(1, 2, &mut w);
        // Expect weight within [0,1]
        assert!(w >= fx_from_f32(0.0) && w <= fx_from_f32(1.0));
    }

    #[test]
    fn test_traces_increase_and_decay() {
        let mut stdp = QuantizedStdp::with_defaults();

        stdp.on_pre_spike(5, 0);
        stdp.on_post_spike(5, 0);

        let (pre0, post0) = stdp.traces(5);
        assert!(pre0 >= ONE && post0 >= ONE);

        stdp.decay();
        let (pre1, post1) = stdp.traces(5);
        assert!(pre1 <= pre0 && post1 <= post0);
    }

    #[test]
    fn test_clamp_bounds() {
        let mut stdp = QuantizedStdp::new(1.0, 0.0, 1.0, 1.0, 0.25, 0.75);
        let mut w = fx_from_f32(0.7);
        stdp.on_pre_spike(0, 0);
        stdp.apply_edge(0, 1, &mut w);
        // Should clamp to 0.75 max
        assert!((fx_to_f32(w) - 0.75).abs() < 1e-3);
    }
}