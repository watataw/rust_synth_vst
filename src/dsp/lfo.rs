use crate::engine::{AtomicF32, DspModule, MAX_PORTS_PER_MODULE};
use std::f64::consts::TAU;
use std::sync::Arc;

pub struct LfoParams {
    /// Fréquence en Hz (0.05 .. 50.0)
    pub rate: AtomicF32,
    /// Sélection d'onde (0: Sine, 1: Triangle, 2: Saw, 3: Square, 4: S&H)
    pub waveform: AtomicF32,
    /// Polarité (0.0 = Bipolaire ±5V, 1.0 = Unipolaire 0..+5V)
    pub unipolar: AtomicF32,
}

impl LfoParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            rate: AtomicF32::new(1.8),
            waveform: AtomicF32::new(0.0),
            unipolar: AtomicF32::new(0.0),
        })
    }
}

/// Module 5 : LFO (0.05 Hz à 50 Hz) avec formes d'ondes et Sample & Hold.
/// Entrées : 0: RST (Reset phase), 1: RATE CV (1V/Oct)
/// Sorties : 0: OUT, 1: S&H, 2: INV, 3: Phase normalisée (0.0..1.0 pour synchro UI)
pub struct LfoModule {
    params: Arc<LfoParams>,
    phase: f64,
    prev_rst_high: bool,
    rng_state: u32,
    sh_value: f32,
}

impl LfoModule {
    pub fn new(params: Arc<LfoParams>) -> Self {
        Self {
            params,
            phase: 0.0,
            prev_rst_high: false,
            rng_state: 0x1234_5678,
            sh_value: 0.0,
        }
    }

    #[inline(always)]
    fn next_random_bipolar(&mut self) -> f32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        ((x as f32) / (u32::MAX as f32)) * 2.0 - 1.0
    }
}

impl DspModule for LfoModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        sample_rate: f32,
    ) {
        let rst_high = inputs[0] > 1.0;
        if rst_high && !self.prev_rst_high {
            self.phase = 0.0;
            self.sh_value = self.next_random_bipolar();
        }
        self.prev_rst_high = rst_high;

        let base_rate = self.params.rate.load();
        let freq = (base_rate * 2.0f32.powf(inputs[1])).clamp(0.05, 50.0) as f64;
        let dt = freq / (sample_rate as f64);

        let t = self.phase;
        let wave_idx = self.params.waveform.load().round() as i32;

        let raw_bipolar = match wave_idx {
            0 => (t * TAU).sin() as f32,
            1 => {
                if t < 0.5 {
                    (4.0 * t - 1.0) as f32
                } else {
                    (3.0 - 4.0 * t) as f32
                }
            }
            2 => (2.0 * t - 1.0) as f32,
            3 => {
                if t < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            _ => self.sh_value,
        };

        let unipolar = self.params.unipolar.load() > 0.5;
        let out_volts = if unipolar {
            (raw_bipolar + 1.0) * 2.5
        } else {
            raw_bipolar * 5.0
        };

        let sh_volts = if unipolar {
            (self.sh_value + 1.0) * 2.5
        } else {
            self.sh_value * 5.0
        };

        outputs[0] = out_volts;
        outputs[1] = sh_volts;
        outputs[2] = if unipolar { 5.0 - out_volts } else { -out_volts };
        outputs[3] = self.phase as f32;

        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.sh_value = self.next_random_bipolar();
        }
    }
}