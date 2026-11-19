use crate::engine::{AtomicF32, DspModule, MAX_PORTS_PER_MODULE};
use std::f32::consts::PI;
use std::sync::Arc;

pub struct VcfParams {
    /// Fréquence de coupure en Hz (20.0 .. 18000.0)
    pub cutoff: AtomicF32,
    /// Résonance (0.0 .. 1.0)
    pub resonance: AtomicF32,
    /// Saturation d'entrée (0.5 .. 4.0)
    pub drive: AtomicF32,
    /// Profondeur de modulation CV (-1.0 .. +1.0)
    pub cv_amount: AtomicF32,
}

impl VcfParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cutoff: AtomicF32::new(1400.0),
            resonance: AtomicF32::new(0.25),
            drive: AtomicF32::new(1.2),
            cv_amount: AtomicF32::new(0.5),
        })
    }
}

/// Module 3 : Filtre Multimode 24dB/oct Zero-Delay Feedback Ladder.
/// Entrées : 0: IN, 1: CUT CV, 2: RES CV
/// Sorties : 0: LP24, 1: BP12, 2: HP24
pub struct VcfModule {
    params: Arc<VcfParams>,
    s: [f32; 4],
}

impl VcfModule {
    pub fn new(params: Arc<VcfParams>) -> Self {
        Self {
            params,
            s: [0.0; 4],
        }
    }
}

impl DspModule for VcfModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        sample_rate: f32,
    ) {
        let drive = self.params.drive.load();
        let x = (inputs[0] * 0.2 * drive).tanh();

        let base_cut = self.params.cutoff.load();
        let cv_amt = self.params.cv_amount.load();
        let cut_oct_mod = inputs[1] * cv_amt;
        let fc = (base_cut * 2.0f32.powf(cut_oct_mod)).clamp(20.0, sample_rate * 0.45);

        let res = (self.params.resonance.load() + inputs[2] * 0.2).clamp(0.0, 0.99);
        let k = res * 3.92;

        let g = (PI * fc / sample_rate).tan().min(1.25);
        let big_g = g / (1.0 + g);
        let g2 = big_g * big_g;
        let g3 = g2 * big_g;
        let g4 = g2 * g2;

        let s_sum = g3 * self.s[0] + g2 * self.s[1] + big_g * self.s[2] + self.s[3];
        let u = (x - k * s_sum) / (1.0 + k * g4);

        let mut y = [0.0f32; 4];
        let mut stage_in = u;
        for i in 0..4 {
            let v = big_g * (stage_in - self.s[i]);
            let out = v + self.s[i];
            self.s[i] = out + v;
            y[i] = out;
            stage_in = out;
        }

        let lp24 = y[3] * 5.0;
        let bp12 = (y[1] - y[2]) * 10.0;
        let hp24 = (u - 4.0 * y[0] + 6.0 * y[1] - 4.0 * y[2] + y[3]) * 5.0;

        outputs[0] = lp24.clamp(-8.0, 8.0);
        outputs[1] = bp12.clamp(-8.0, 8.0);
        outputs[2] = hp24.clamp(-8.0, 8.0);
    }
}