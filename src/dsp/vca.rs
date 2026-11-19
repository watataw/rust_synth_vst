use crate::engine::{AtomicF32, DspModule, MAX_PORTS_PER_MODULE};
use std::sync::Arc;

pub struct VcaParams {
    /// Niveau initial (0.0 .. 1.0)
    pub level: AtomicF32,
    /// Atténuateur CV (0.0 .. 1.0)
    pub cv_amount: AtomicF32,
    /// Réponse (0.0 = Linéaire, 1.0 = Exponentielle)
    pub response: AtomicF32,
}

impl VcaParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            level: AtomicF32::new(0.0),
            cv_amount: AtomicF32::new(1.0),
            response: AtomicF32::new(0.35),
        })
    }
}

/// Module 6 : Amplificateur Contrôlé en Tension (VCA).
/// Entrées : 0: IN, 1: CV
/// Sorties : 0: OUT
pub struct VcaModule {
    params: Arc<VcaParams>,
}

impl VcaModule {
    pub fn new(params: Arc<VcaParams>) -> Self {
        Self { params }
    }
}

impl DspModule for VcaModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        _sample_rate: f32,
    ) {
        let bias = self.params.level.load();
        let cv_amt = self.params.cv_amount.load();
        let exp_blend = self.params.response.load().clamp(0.0, 1.0);

        let cv_norm = (inputs[1] / 5.0).max(0.0) * cv_amt;
        let lin_gain = (bias + cv_norm).clamp(0.0, 1.5);
        let exp_gain = lin_gain * lin_gain;
        let final_gain = lin_gain * (1.0 - exp_blend) + exp_gain * exp_blend;

        outputs[0] = inputs[0] * final_gain;
    }
}